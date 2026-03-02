use std::collections::HashSet;
use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use super::waypoints;

/// A suggested waypoint derived from visit history.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub name: String,
    pub path: String,
    pub access_count: i64,
    #[expect(
        dead_code,
        reason = "populated during generation, kept for future consumers"
    )]
    pub project_root: Option<String>,
}

/// Generic path components that don't make good waypoint names on their own.
const GENERIC_COMPONENTS: &[&str] = &[
    "src", "lib", "app", "pkg", "cmd", "internal", "bin", "dist", "build", "out", "test", "tests",
];

/// Minimum path depth below $HOME to be considered meaningful.
const MIN_DEPTH_BELOW_HOME: usize = 3;

/// Minimum visit count to be considered a suggestion candidate.
const MIN_VISITS: i64 = 3;

/// Query directories ordered by access_count DESC (habitual paths).
fn query_most_visited(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<(String, i64, Option<String>)>> {
    let mut stmt = conn.prepare(
        "SELECT path, access_count, project_root
         FROM directories
         WHERE access_count >= ?1
         ORDER BY access_count DESC
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(rusqlite::params![MIN_VISITS, limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Normalize a string into a valid waypoint name: lowercase, special chars → hyphens,
/// collapse runs, trim leading/trailing hyphens.
pub fn normalize_name(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_hyphen = false;

    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            result.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen {
            result.push('-');
            prev_hyphen = true;
        }
    }

    result.trim_matches('-').to_string()
}

/// Generate a deterministic waypoint name from a path.
///
/// - If path IS the project root → project basename
/// - If last component is generic (src, lib, etc.) → walk up one level
/// - Normalize result
pub fn generate_name(path: &str, project_root: Option<&str>) -> String {
    let p = Path::new(path);

    // If path is the project root, use the project basename
    if let Some(root) = project_root {
        if path == root {
            let name = Path::new(root)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            return normalize_name(&name);
        }
    }

    // Get the last component
    let last = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // If last component is generic, walk up one level and combine
    let last_lower = last.to_lowercase();
    if GENERIC_COMPONENTS.contains(&last_lower.as_str()) {
        if let Some(parent) = p.parent() {
            let parent_name = parent
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !parent_name.is_empty() {
                return normalize_name(&format!("{}-{}", parent_name, last));
            }
        }
    }

    normalize_name(&last)
}

/// Count path components below `$HOME`. Accepts the home dir string to
/// avoid re-resolving it on each call when used in a loop.
fn depth_below_home(path: &str, home: &str) -> usize {
    if home.is_empty() || !path.starts_with(home) {
        // Not under home — count all components as "deep enough"
        return MIN_DEPTH_BELOW_HOME;
    }

    let relative = &path[home.len()..];
    relative.split('/').filter(|s| !s.is_empty()).count()
}

/// Get all existing waypoint names and paths from the database.
fn existing_waypoints(conn: &Connection) -> Result<(HashSet<String>, HashSet<String>)> {
    let mut stmt = conn.prepare("SELECT name, path FROM waypoints")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut names = HashSet::new();
    let mut paths = HashSet::new();
    for row in rows {
        let (name, path) = row?;
        names.insert(name);
        paths.insert(path);
    }

    Ok((names, paths))
}

/// Generate waypoint suggestions from visit history.
///
/// Fetches top directories by access_count, filters out existing waypoints,
/// shallow paths, and low-visit paths, then generates deterministic names
/// with collision resolution.
pub fn generate_suggestions(conn: &Connection, limit: usize) -> Result<Vec<Suggestion>> {
    let (wp_names, wp_paths) = existing_waypoints(conn)?;

    // Over-fetch 3x to account for filtering
    let candidates = query_most_visited(conn, limit * 3)?;

    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut suggestions: Vec<Suggestion> = Vec::new();
    let mut used_names: HashSet<String> = wp_names;

    for (path, access_count, project_root) in candidates {
        // Skip paths that already have waypoints
        if wp_paths.contains(&path) {
            continue;
        }

        // Skip shallow paths
        if depth_below_home(&path, &home) < MIN_DEPTH_BELOW_HOME {
            continue;
        }

        // Skip paths that no longer exist
        if !Path::new(&path).exists() {
            continue;
        }

        let mut name = generate_name(&path, project_root.as_deref());

        // Resolve name collision by prefixing with project basename
        if used_names.contains(&name) {
            if let Some(ref root) = project_root {
                let project_name = Path::new(root)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let prefixed = normalize_name(&format!("{}-{}", project_name, name));
                if !used_names.contains(&prefixed) {
                    name = prefixed;
                } else {
                    continue; // Still collides, skip
                }
            } else {
                continue; // No project root to disambiguate
            }
        }

        used_names.insert(name.clone());
        suggestions.push(Suggestion {
            name,
            path,
            access_count,
            project_root,
        });

        if suggestions.len() >= limit {
            break;
        }
    }

    Ok(suggestions)
}

/// Contract a path with `~` for display if it's under the home directory.
fn tilde_contract(path: &str, home: &str) -> String {
    if !home.is_empty() && path.starts_with(home) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

/// Display suggestions in a formatted table to stderr.
pub fn display_suggestions(suggestions: &[Suggestion]) {
    if suggestions.is_empty() {
        eprintln!("No suggestions — keep navigating to build up visit history.");
        return;
    }

    eprintln!("Suggested waypoints (based on your most visited paths):");
    eprintln!();

    // Tilde-contract the home directory for display
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let max_name = suggestions.iter().map(|s| s.name.len()).max().unwrap_or(0);

    for s in suggestions {
        let display_path = tilde_contract(&s.path, &home);
        eprintln!(
            "  !{:<width$}  →  {:<40} ({} visits)",
            s.name,
            display_path,
            s.access_count,
            width = max_name,
        );
    }

    eprintln!();
    eprintln!("Apply a suggestion:  tp --mark <name> <path>");
    eprintln!("Apply all:           tp suggest --apply");
}

/// Interactively walk through suggestions and apply them.
///
/// For each suggestion, prompts `[y/n/custom name/q]`:
/// - `y` → create waypoint with suggested name
/// - `n` → skip
/// - `q` → stop
/// - anything else → use as custom name
pub fn apply_suggestions(conn: &Connection, suggestions: &[Suggestion]) -> Result<()> {
    if suggestions.is_empty() {
        eprintln!("No suggestions to apply.");
        return Ok(());
    }

    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    for s in suggestions {
        let display_path = tilde_contract(&s.path, &home);
        eprint!("  !{} → {}  [y/n/custom name/q]: ", s.name, display_path);
        io::stderr().flush()?;

        let input = match lines.next() {
            Some(Ok(line)) => line.trim().to_string(),
            _ => break,
        };

        match input.as_str() {
            "y" | "Y" | "yes" => {
                waypoints::add_waypoint(conn, &s.name, &s.path)?;
            }
            "n" | "N" | "no" | "" => {
                continue;
            }
            "q" | "Q" | "quit" => {
                eprintln!("Stopped.");
                break;
            }
            custom => {
                let name = normalize_name(custom);
                if name.is_empty() {
                    eprintln!("  (invalid name, skipping)");
                    continue;
                }
                waypoints::add_waypoint(conn, &name, &s.path)?;
            }
        }
    }

    Ok(())
}

/// Quick count of how many suggestions are available (for `tp doctor` hint).
pub fn suggestion_count(conn: &Connection) -> usize {
    generate_suggestions(conn, 20).map(|s| s.len()).unwrap_or(0)
}

/// AI-enhanced waypoint name suggestions.
///
/// Sends paths to Claude Haiku for creative name suggestions.
/// Feature-gated behind `ai`. Falls back silently on any failure.
#[cfg(feature = "ai")]
pub fn ai_enhance_names(suggestions: &mut [Suggestion]) {
    let (api_key, _source) = match crate::ai::detect_api_key() {
        Some(pair) => pair,
        None => return,
    };

    let model =
        std::env::var("TP_AI_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string());
    let timeout_ms: u64 = std::env::var("TP_AI_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    // Build a prompt listing all paths
    let paths_text: Vec<String> = suggestions
        .iter()
        .map(|s| format!("- {} (current name: {})", s.path, s.name))
        .collect();

    let prompt = format!(
        "You are naming waypoints (bookmarks) for a terminal directory navigator. \
         For each path below, suggest a short, memorable name (1-2 words, lowercase, hyphens ok). \
         The name should be more descriptive than the auto-generated one. \
         Return ONLY a JSON array of strings, one name per path, in the same order.\n\n{}",
        paths_text.join("\n")
    );

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 256,
        "messages": [{"role": "user", "content": prompt}]
    });

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(body.to_string())
        .send();

    let text = match resp {
        Ok(r) => match r.text() {
            Ok(t) => t,
            Err(_) => return,
        },
        Err(_) => return,
    };

    // Parse the response — extract the text content
    let parsed: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return,
    };

    let content_text = parsed["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|block| block["text"].as_str())
        .unwrap_or("");

    // Try to parse the JSON array from the response
    let names: Vec<String> = match serde_json::from_str(content_text) {
        Ok(v) => v,
        Err(_) => {
            // Try to find a JSON array within the text
            if let Some(start) = content_text.find('[') {
                if let Some(end) = content_text.rfind(']') {
                    match serde_json::from_str(&content_text[start..=end]) {
                        Ok(v) => v,
                        Err(_) => return,
                    }
                } else {
                    return;
                }
            } else {
                return;
            }
        }
    };

    // Apply AI names where they're valid
    for (suggestion, ai_name) in suggestions.iter_mut().zip(names.iter()) {
        let normalized = normalize_name(ai_name);
        if !normalized.is_empty() && normalized.len() <= 30 {
            suggestion.name = normalized;
        }
    }
}

/// No-op fallback when AI feature is disabled.
#[cfg(not(feature = "ai"))]
pub fn ai_enhance_names(_suggestions: &mut [Suggestion]) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    mod normalize_name {
        use super::*;

        #[test]
        fn preserves_simple_lowercase() {
            assert_eq!(normalize_name("hello"), "hello");
        }

        #[test]
        fn lowercases_and_hyphenates_spaces() {
            assert_eq!(normalize_name("Hello World"), "hello-world");
        }

        #[test]
        fn replaces_special_chars_with_hyphens() {
            assert_eq!(normalize_name("my_project.v2"), "my-project-v2");
        }

        #[test]
        fn trims_and_collapses_leading_trailing_hyphens() {
            assert_eq!(normalize_name("--leading--trailing--"), "leading-trailing");
        }

        #[test]
        fn collapses_consecutive_separators() {
            assert_eq!(normalize_name("foo///bar"), "foo-bar");
        }

        #[test]
        fn lowercases_all_caps() {
            assert_eq!(normalize_name("UPPER"), "upper");
        }
    }

    mod generate_name {
        use super::*;

        #[test]
        fn returns_project_basename_when_path_is_root() {
            let name = generate_name("/home/user/dev/myapp", Some("/home/user/dev/myapp"));
            assert_eq!(name, "myapp");
        }

        #[test]
        fn prefixes_generic_src_with_parent() {
            assert_eq!(generate_name("/home/user/dev/myapp/src", None), "myapp-src");
        }

        #[test]
        fn prefixes_generic_lib_with_parent() {
            assert_eq!(generate_name("/home/user/dev/myapp/lib", None), "myapp-lib");
        }

        #[test]
        fn prefixes_generic_app_with_parent() {
            assert_eq!(generate_name("/home/user/dev/myapp/app", None), "myapp-app");
        }

        #[test]
        fn keeps_meaningful_last_component() {
            assert_eq!(
                generate_name("/home/user/dev/myapp/src/handlers", None),
                "handlers"
            );
        }
    }

    #[test]
    fn test_generate_suggestions_empty_db() {
        let conn = db::open_memory().unwrap();
        let suggestions = generate_suggestions(&conn, 10).unwrap();
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_generate_suggestions_filters_existing_waypoints() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Create a deep-enough path structure
        let deep = tmp.path().join("dev").join("myapp").join("handlers");
        std::fs::create_dir_all(&deep).unwrap();
        let deep_str = deep.to_str().unwrap();

        // Record enough visits to qualify
        for _ in 0..5 {
            crate::nav::frecency::record_visit(&conn, deep_str, None).unwrap();
        }

        // Create a waypoint for the same path
        conn.execute(
            "INSERT INTO waypoints (name, path) VALUES (?1, ?2)",
            rusqlite::params!["handlers", deep_str],
        )
        .unwrap();

        let suggestions = generate_suggestions(&conn, 10).unwrap();
        // The path with an existing waypoint should be filtered out
        assert!(
            !suggestions.iter().any(|s| s.path == deep_str),
            "path with existing waypoint should be excluded"
        );
    }

    #[test]
    fn test_generate_suggestions_collision_resolution() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Create two "handlers" dirs in different projects
        let proj_a = tmp.path().join("project-a");
        let proj_b = tmp.path().join("project-b");
        let handlers_a = proj_a.join("code").join("handlers");
        let handlers_b = proj_b.join("code").join("handlers");
        std::fs::create_dir_all(&handlers_a).unwrap();
        std::fs::create_dir_all(&handlers_b).unwrap();

        let a_str = handlers_a.to_str().unwrap();
        let b_str = handlers_b.to_str().unwrap();
        let root_a = proj_a.to_str().unwrap();
        let root_b = proj_b.to_str().unwrap();

        // Record visits with different project roots
        for _ in 0..10 {
            crate::nav::frecency::record_visit(&conn, a_str, Some(root_a)).unwrap();
        }
        for _ in 0..8 {
            crate::nav::frecency::record_visit(&conn, b_str, Some(root_b)).unwrap();
        }

        let suggestions = generate_suggestions(&conn, 10).unwrap();

        // Both should appear, one with a project prefix
        let names: Vec<&str> = suggestions.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"handlers"),
            "first handlers should get plain name, got: {:?}",
            names
        );
        // The second one should be prefixed with its project name
        let prefixed = suggestions
            .iter()
            .any(|s| s.name.contains("project-") && s.name.contains("handlers"));
        assert!(
            prefixed,
            "second handlers should get project-prefixed name, got: {:?}",
            names
        );
    }

    #[test]
    fn test_suggestion_count() {
        let conn = db::open_memory().unwrap();
        assert_eq!(suggestion_count(&conn), 0);
    }
}
