use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

/// Directories to always skip when walking a project tree.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "dist",
    "build",
    ".next",
    ".cache",
    "vendor",
    ".idea",
    ".vscode",
    ".DS_Store",
    "venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    "coverage",
    ".turbo",
];

/// Maximum directory depth to walk (relative to project root).
const MAX_DEPTH: usize = 3;

/// Parse a `.gitignore` file and extract simple directory-name patterns.
///
/// Skips comment lines (`#`), negation lines (`!`), glob patterns (`*`),
/// and lines with internal `/` (nested paths). Strips leading `/` and
/// trailing `/` so only bare directory names remain.
pub fn parse_gitignore(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with('#'))
        .filter(|l| !l.starts_with('!'))
        .filter(|l| !l.contains('*'))
        .map(|l| {
            let l = l.strip_prefix('/').unwrap_or(l);
            let l = l.strip_suffix('/').unwrap_or(l);
            l.to_string()
        })
        .filter(|l| !l.contains('/'))
        .collect()
}

/// Walk a project directory tree up to `MAX_DEPTH` levels, collecting
/// relative directory paths. Skips `IGNORED_DIRS` and any extra ignores.
pub fn walk_tree(root: &Path, extra_ignores: &[String]) -> Vec<String> {
    let mut results = Vec::new();
    walk_recursive(root, root, 0, extra_ignores, &mut results);
    results.sort();
    results
}

/// Recursive helper for `walk_tree`.
fn walk_recursive(
    root: &Path,
    current: &Path,
    depth: usize,
    extra_ignores: &[String],
    results: &mut Vec<String>,
) {
    if depth >= MAX_DEPTH {
        return;
    }

    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Skip ignored directories
        if IGNORED_DIRS.contains(&name.as_str()) {
            continue;
        }
        if extra_ignores.iter().any(|ig| ig == &name) {
            continue;
        }

        // Build relative path
        if let Ok(rel) = path.strip_prefix(root) {
            if let Some(rel_str) = rel.to_str() {
                results.push(rel_str.to_string());
            }
        }

        walk_recursive(root, &path, depth + 1, extra_ignores, results);
    }
}

/// Build the AI prompt that asks for directory descriptions.
pub fn build_index_prompt(
    project_name: &str,
    project_kind: Option<&str>,
    dirs: &[String],
) -> String {
    let mut prompt = String::new();
    prompt.push_str(&format!("Project: {}\n", project_name));
    if let Some(kind) = project_kind {
        prompt.push_str(&format!("Type: {}\n", kind));
    }
    prompt.push_str("\nDirectory tree:\n");
    for d in dirs {
        prompt.push_str(&format!("  {}\n", d));
    }
    prompt.push_str(
        "\nReturn a JSON object mapping each directory path to a 5-15 word description \
         of what it contains or is used for. Output only valid JSON, no extra text.",
    );
    prompt
}

/// Parse the AI response, which may be plain JSON or wrapped in markdown
/// code fences (```json ... ```).
pub fn parse_index_response(text: &str) -> Option<HashMap<String, String>> {
    let trimmed = text.trim();

    // Try to extract from code fences first
    let json_str = if trimmed.starts_with("```") {
        // Find end of first line (the ```json line)
        let after_fence = trimmed.find('\n').map(|i| &trimmed[i + 1..])?;
        // Find closing fence
        let end = after_fence.rfind("```")?;
        after_fence[..end].trim()
    } else {
        trimmed
    };

    serde_json::from_str(json_str).ok()
}

/// Delete old index entries for this project root, then insert new ones.
/// Also updates the `projects` table with the current `indexed_at` timestamp.
pub fn save_index(
    conn: &Connection,
    project_root: &str,
    entries: &HashMap<String, String>,
) -> Result<()> {
    // Remove previous entries
    conn.execute(
        "DELETE FROM project_dirs WHERE project_root = ?1",
        [project_root],
    )?;

    // Insert new entries, stripping trailing slashes from rel_path
    let mut stmt = conn.prepare(
        "INSERT INTO project_dirs (project_root, rel_path, description) VALUES (?1, ?2, ?3)",
    )?;
    for (path, description) in entries {
        let clean_path = path.strip_suffix('/').unwrap_or(path);
        stmt.execute(rusqlite::params![project_root, clean_path, description])?;
    }

    // Upsert into projects table
    let name = crate::project::project_name(project_root);
    let kind = crate::project::project_kind(project_root);
    conn.execute(
        "INSERT INTO projects (path, name, kind, indexed_at) VALUES (?1, ?2, ?3, strftime('%s','now'))
         ON CONFLICT(path) DO UPDATE SET name = ?2, kind = ?3, indexed_at = strftime('%s','now')",
        rusqlite::params![project_root, name, kind],
    )?;

    Ok(())
}

/// Index a project: walk its directory tree, send to AI for descriptions,
/// and save results to the database.
#[cfg(feature = "ai")]
pub fn index_project(conn: &Connection, target: &str) -> Result<()> {
    // Canonicalize the target path
    let canonical = std::fs::canonicalize(target)
        .map_err(|e| anyhow::anyhow!("cannot resolve path '{}': {}", target, e))?;
    let canonical_str = canonical
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8"))?;

    // Detect project root
    let project_root = crate::project::detect_project_root(canonical_str)
        .ok_or_else(|| anyhow::anyhow!("no project root found from '{}'", canonical_str))?;

    let name = crate::project::project_name(&project_root);
    let kind = crate::project::project_kind(&project_root);

    // Parse .gitignore if it exists
    let gitignore_path = Path::new(&project_root).join(".gitignore");
    let extra_ignores = if gitignore_path.exists() {
        let content = std::fs::read_to_string(&gitignore_path).unwrap_or_default();
        parse_gitignore(&content)
    } else {
        Vec::new()
    };

    // Walk the tree
    let dirs = walk_tree(Path::new(&project_root), &extra_ignores);

    if dirs.is_empty() {
        eprintln!("No directories found to index.");
        return Ok(());
    }

    // Check for API key
    let (api_key, _source) = super::detect_api_key()
        .ok_or_else(|| anyhow::anyhow!("no API key found — run `tp --setup-ai` first"))?;

    // Build prompt
    let prompt = build_index_prompt(&name, kind, &dirs);

    let model =
        std::env::var("TP_AI_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string());
    let timeout_ms: u64 = std::env::var("TP_AI_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10000);

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "system": "You are a code project analyzer. Given a project's directory tree, describe what each directory contains or is used for. Return only valid JSON.",
        "messages": [
            { "role": "user", "content": prompt }
        ]
    });

    let spinner = crate::style::Spinner::start("indexing project...");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()?;

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()?;

    let json: serde_json::Value = resp.json()?;
    let text = json["content"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("unexpected API response format"))?;

    spinner.stop();

    let entries = parse_index_response(text)
        .ok_or_else(|| anyhow::anyhow!("failed to parse AI response as JSON"))?;

    save_index(conn, &project_root, &entries)?;

    let count = entries.len();
    eprintln!("Indexed {} directories for '{}'", count, name);

    // Show first 5 entries as a sample
    let mut sample: Vec<_> = entries.iter().collect();
    sample.sort_by_key(|(k, _)| (*k).clone());
    for (path, desc) in sample.iter().take(5) {
        eprintln!("  {} — {}", path, desc);
    }
    if count > 5 {
        eprintln!("  ... and {} more", count - 5);
    }

    Ok(())
}

/// Fallback when the `ai` feature is disabled.
#[cfg(not(feature = "ai"))]
pub fn index_project(_conn: &Connection, _target: &str) -> Result<()> {
    eprintln!("AI features are not enabled");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_gitignore_simple() {
        let content = "node_modules\ntarget/\n*.log\n# comment\n!important\n";
        let result = parse_gitignore(content);
        assert!(result.contains(&"node_modules".to_string()));
        assert!(result.contains(&"target".to_string()));
        assert!(!result.iter().any(|s| s.contains("log")));
        assert!(!result.iter().any(|s| s.contains("important")));
    }

    #[test]
    fn test_parse_gitignore_slashes() {
        let content = "/build\ndist/\n";
        let result = parse_gitignore(content);
        assert!(result.contains(&"build".to_string()));
        assert!(result.contains(&"dist".to_string()));
    }

    #[test]
    fn test_parse_gitignore_skips_nested() {
        let content = "logs/debug\nsimple\n";
        let result = parse_gitignore(content);
        assert!(result.contains(&"simple".to_string()));
        assert!(!result.iter().any(|s| s.contains("logs")));
    }

    #[test]
    fn test_walk_tree() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        // Create test directories
        std::fs::create_dir_all(root.join("src/ai")).unwrap();
        std::fs::create_dir_all(root.join("src/db")).unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/foo")).unwrap();
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();

        let result = walk_tree(root, &[]);
        assert!(result.contains(&"src".to_string()));
        assert!(result.contains(&"src/ai".to_string()));
        assert!(result.contains(&"src/db".to_string()));
        assert!(result.contains(&"docs".to_string()));
        assert!(!result.iter().any(|s| s.starts_with("node_modules")));
        assert!(!result.iter().any(|s| s.starts_with(".git")));
    }

    #[test]
    fn test_walk_tree_respects_extra_ignores() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("custom_build")).unwrap();

        let extra = vec!["custom_build".to_string()];
        let result = walk_tree(root, &extra);
        assert!(result.contains(&"src".to_string()));
        assert!(!result.contains(&"custom_build".to_string()));
    }

    #[test]
    fn test_walk_tree_depth_limit() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        std::fs::create_dir_all(root.join("a/b/c/d/e")).unwrap();

        let result = walk_tree(root, &[]);
        assert!(result.contains(&"a".to_string()));
        assert!(result.contains(&"a/b".to_string()));
        assert!(result.contains(&"a/b/c".to_string()));
        assert!(!result.contains(&"a/b/c/d".to_string()));
    }

    #[test]
    fn test_build_index_prompt() {
        let dirs = vec!["src/ai".to_string(), "docs/book".to_string()];
        let prompt = build_index_prompt("my-project", Some("rust"), &dirs);

        assert!(prompt.contains("my-project"));
        assert!(prompt.contains("rust"));
        assert!(prompt.contains("src/ai"));
        assert!(prompt.contains("docs/book"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn test_parse_index_response_clean_json() {
        let json = r#"{"src": "source code", "docs": "documentation"}"#;
        let result = parse_index_response(json).unwrap();
        assert_eq!(result.get("src").unwrap(), "source code");
        assert_eq!(result.get("docs").unwrap(), "documentation");
    }

    #[test]
    fn test_parse_index_response_with_code_fences() {
        let text = "```json\n{\"src\": \"source code\", \"docs\": \"documentation\"}\n```";
        let result = parse_index_response(text).unwrap();
        assert_eq!(result.get("src").unwrap(), "source code");
        assert_eq!(result.get("docs").unwrap(), "documentation");
    }

    #[test]
    fn test_parse_index_response_invalid() {
        let result = parse_index_response("this is not json");
        assert!(result.is_none());
    }

    #[test]
    fn test_save_index() {
        let conn = crate::db::open_memory().unwrap();
        let mut entries = HashMap::new();
        entries.insert("src/".to_string(), "source code".to_string());
        entries.insert("docs".to_string(), "documentation".to_string());

        save_index(&conn, "/tmp/test-project", &entries).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_dirs WHERE project_root = ?1",
                ["/tmp/test-project"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        // Verify trailing slash was stripped
        let desc: String = conn
            .query_row(
                "SELECT description FROM project_dirs WHERE project_root = ?1 AND rel_path = ?2",
                ["/tmp/test-project", "src"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(desc, "source code");
    }

    #[test]
    fn test_save_index_replaces_previous() {
        let conn = crate::db::open_memory().unwrap();

        // First save: 1 entry
        let mut entries1 = HashMap::new();
        entries1.insert("src".to_string(), "source code".to_string());
        save_index(&conn, "/tmp/test-project", &entries1).unwrap();

        // Second save: 2 entries (should replace, not append)
        let mut entries2 = HashMap::new();
        entries2.insert("lib".to_string(), "library code".to_string());
        entries2.insert("docs".to_string(), "documentation".to_string());
        save_index(&conn, "/tmp/test-project", &entries2).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_dirs WHERE project_root = ?1",
                ["/tmp/test-project"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2); // 2, not 3
    }
}
