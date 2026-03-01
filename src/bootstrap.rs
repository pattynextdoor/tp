//! First-run auto-bootstrap: seeds the database from shell history,
//! existing navigation tools, and project discovery.
//!
//! Triggered automatically on the first navigation attempt when the
//! database is empty. Runs once, takes <500ms, and prints a single
//! summary line to stderr.

use anyhow::Result;
use rusqlite::Connection;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::import;
use crate::project;

/// Check if the database is empty and run bootstrap if so.
/// Returns true if bootstrap ran.
pub fn auto_bootstrap(conn: &Connection) -> Result<bool> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))?;

    if count > 0 {
        return Ok(false); // Already seeded
    }

    let mut total = 0u64;

    // 1. Import from zoxide if available
    total += import_from_zoxide(conn).unwrap_or(0);

    // 2. Parse shell history for cd commands
    total += import_from_shell_history(conn).unwrap_or(0);

    // 3. Discover projects under home directory
    total += discover_projects(conn).unwrap_or(0);

    if total > 0 {
        eprintln!(
            "tp: indexed {} directories from your history. Ready.",
            total
        );
    }

    Ok(true)
}

/// Try to import from zoxide's database silently.
fn import_from_zoxide(conn: &Connection) -> Result<u64> {
    // Try running zoxide — if it's not installed, that's fine
    let output = match std::process::Command::new("zoxide")
        .args(["query", "-l", "-s"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Ok(0),
    };

    let reader = std::io::BufReader::new(std::io::Cursor::new(output.stdout));
    import::import_zoxide(conn, reader)
}

/// Parse shell history files for `cd` commands and extract directory paths.
fn import_from_shell_history(conn: &Connection) -> Result<u64> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(0),
    };

    let history_files = [
        home.join(".zsh_history"),
        home.join(".bash_history"),
        home.join(".local/share/fish/fish_history"),
    ];

    let mut count = 0u64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    for hist_file in &history_files {
        if !hist_file.exists() {
            continue;
        }
        count += parse_history_file(conn, hist_file, &home, now)?;
    }

    Ok(count)
}

/// Parse a single history file and extract directories from cd commands.
fn parse_history_file(conn: &Connection, path: &Path, home: &Path, now: i64) -> Result<u64> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(0),
    };
    let reader = std::io::BufReader::new(file);
    let tx = conn.unchecked_transaction()?;
    let mut count = 0u64;
    let mut seen = std::collections::HashSet::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue, // Skip lines with encoding issues
        };

        let dir = extract_cd_target(&line, home);
        let dir = match dir {
            Some(d) => d,
            None => continue,
        };

        // Skip if we've already seen this path in this file
        if !seen.insert(dir.clone()) {
            continue;
        }

        // Only import if the directory actually exists
        let dir_path = Path::new(&dir);
        if !dir_path.is_dir() {
            continue;
        }

        let project_root = project::detect_project_root(&dir);

        tx.execute(
            "INSERT INTO directories (path, frecency, last_access, access_count, project_root)
             VALUES (?1, 1.0, ?2, 1, ?3)
             ON CONFLICT(path) DO UPDATE SET
               frecency = frecency + 1.0,
               access_count = access_count + 1,
               project_root = COALESCE(?3, project_root)",
            rusqlite::params![dir, now, project_root],
        )?;

        count += 1;
    }

    tx.commit()?;
    Ok(count)
}

/// Extract a directory path from a shell history line containing `cd`.
///
/// Handles:
/// - `cd /some/path`
/// - `cd ~/something` (expands ~)
/// - zsh extended history format: `: 1234567890:0;cd /path`
/// - fish history format: `- cmd: cd /path`
///
/// Skips: `cd` with no args, `cd -`, pipes, `&&` chains after cd
fn extract_cd_target(line: &str, home: &Path) -> Option<String> {
    let trimmed = line.trim();

    // zsh extended history: ": <timestamp>:0;<command>"
    let command = if trimmed.starts_with(": ") {
        trimmed.split_once(';')?.1.trim()
    } else if let Some(stripped) = trimmed.strip_prefix("- cmd: ") {
        // fish history format
        stripped
    } else {
        trimmed
    };

    // Find `cd` command — must be at start or after ; or &&
    let cd_part = if let Some(stripped) = command.strip_prefix("cd ") {
        stripped
    } else if let Some(pos) = command.find("; cd ") {
        &command[pos + 5..]
    } else if let Some(pos) = command.find("&& cd ") {
        &command[pos + 6..]
    } else {
        return None;
    };

    // Handle quoted paths first, then unquoted
    let cd_trimmed = cd_part.trim();
    let target = if cd_trimmed.starts_with('"') {
        // Double-quoted: extract content between quotes
        cd_trimmed.strip_prefix('"')?.split('"').next()?
    } else if cd_trimmed.starts_with('\'') {
        // Single-quoted: extract content between quotes
        cd_trimmed.strip_prefix('\'')?.split('\'').next()?
    } else {
        // Unquoted: take first argument (stop at space, ;, &, |, #)
        cd_trimmed
            .split(|c: char| c.is_whitespace() || c == ';' || c == '&' || c == '|' || c == '#')
            .next()?
    };

    if target.is_empty() || target == "-" || target == "." || target == ".." {
        return None;
    }

    // Expand ~ to home directory
    let expanded = if let Some(rest) = target.strip_prefix("~/") {
        home.join(rest).to_string_lossy().to_string()
    } else if target == "~" {
        home.to_string_lossy().to_string()
    } else if target.starts_with('/') {
        target.to_string()
    } else {
        // Relative path — can't resolve without knowing cwd at time of command
        return None;
    };

    Some(expanded)
}

/// Discover project directories under home (depth 3 max).
/// Looks for directories with project markers (.git, package.json, etc.)
fn discover_projects(conn: &Connection) -> Result<u64> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(0),
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    let tx = conn.unchecked_transaction()?;
    let mut count = 0u64;

    // Common code directories to scan
    let scan_roots: Vec<PathBuf> = ["code", "projects", "repos", "src", "dev", "work", "git"]
        .iter()
        .map(|d| home.join(d))
        .filter(|p| p.is_dir())
        .collect();

    // Also scan ~/Desktop and ~/Documents (some people keep repos there)
    let extra: Vec<PathBuf> = ["Desktop", "Documents"]
        .iter()
        .map(|d| home.join(d))
        .filter(|p| p.is_dir())
        .collect();

    let all_roots: Vec<&PathBuf> = scan_roots.iter().chain(extra.iter()).collect();

    for root in all_roots {
        scan_for_projects(&tx, root, 0, 3, now, &mut count)?;
    }

    tx.commit()?;
    Ok(count)
}

/// Recursively scan for project roots up to max_depth.
fn scan_for_projects(
    conn: &Connection,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    now: i64,
    count: &mut u64,
) -> Result<()> {
    if depth > max_depth {
        return Ok(());
    }

    let dir_str = dir.to_string_lossy();

    // Check if this directory is a project root
    if let Some(ref _root) = project::detect_project_root(&dir_str) {
        if _root == &*dir_str {
            // This directory IS a project root — add it
            conn.execute(
                "INSERT INTO directories (path, frecency, last_access, access_count, project_root)
                 VALUES (?1, 0.5, ?2, 0, ?1)
                 ON CONFLICT(path) DO NOTHING",
                rusqlite::params![&*dir_str, now],
            )?;
            *count += 1;
            // Don't recurse into project directories — they're already found
            return Ok(());
        }
    }

    // Recurse into subdirectories
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Skip hidden directories
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }

        // Skip common non-project directories
        if matches!(
            name_str.as_ref(),
            "node_modules"
                | "target"
                | "venv"
                | ".venv"
                | "__pycache__"
                | "vendor"
                | "dist"
                | "build"
        ) {
            continue;
        }

        let path = entry.path();
        if path.is_dir() {
            scan_for_projects(conn, &path, depth + 1, max_depth, now, count)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::nav::frecency;
    use std::path::PathBuf;

    #[test]
    fn test_extract_cd_simple() {
        let home = PathBuf::from("/home/user");
        assert_eq!(
            extract_cd_target("cd /tmp/foo", &home),
            Some("/tmp/foo".to_string())
        );
    }

    #[test]
    fn test_extract_cd_tilde() {
        let home = PathBuf::from("/home/user");
        assert_eq!(
            extract_cd_target("cd ~/projects", &home),
            Some("/home/user/projects".to_string())
        );
    }

    #[test]
    fn test_extract_cd_zsh_extended() {
        let home = PathBuf::from("/home/user");
        assert_eq!(
            extract_cd_target(": 1234567890:0;cd /tmp/bar", &home),
            Some("/tmp/bar".to_string())
        );
    }

    #[test]
    fn test_extract_cd_fish() {
        let home = PathBuf::from("/home/user");
        assert_eq!(
            extract_cd_target("- cmd: cd /opt/code", &home),
            Some("/opt/code".to_string())
        );
    }

    #[test]
    fn test_extract_cd_chained() {
        let home = PathBuf::from("/home/user");
        assert_eq!(
            extract_cd_target("git pull && cd /tmp/deploy", &home),
            Some("/tmp/deploy".to_string())
        );
    }

    #[test]
    fn test_extract_cd_skip_bare() {
        let home = PathBuf::from("/home/user");
        // "cd" with no argument
        assert_eq!(extract_cd_target("cd", &home), None);
    }

    #[test]
    fn test_extract_cd_skip_dash() {
        let home = PathBuf::from("/home/user");
        assert_eq!(extract_cd_target("cd -", &home), None);
    }

    #[test]
    fn test_extract_cd_skip_relative() {
        let home = PathBuf::from("/home/user");
        // Relative paths can't be resolved without the original cwd
        assert_eq!(extract_cd_target("cd src/lib", &home), None);
    }

    #[test]
    fn test_extract_cd_quoted() {
        let home = PathBuf::from("/home/user");
        assert_eq!(
            extract_cd_target("cd \"/tmp/my dir\"", &home),
            Some("/tmp/my dir".to_string())
        );
    }

    #[test]
    fn test_extract_not_cd() {
        let home = PathBuf::from("/home/user");
        assert_eq!(extract_cd_target("ls -la /tmp", &home), None);
        assert_eq!(extract_cd_target("echo cd /tmp", &home), None);
    }

    #[test]
    fn test_auto_bootstrap_empty_db() {
        let conn = db::open_memory().unwrap();
        // On empty DB, bootstrap should run (returns true)
        // It won't find much in a test env, but shouldn't error
        let ran = auto_bootstrap(&conn).unwrap();
        assert!(ran, "bootstrap should run on empty database");
    }

    #[test]
    fn test_auto_bootstrap_nonempty_db() {
        let conn = db::open_memory().unwrap();
        frecency::record_visit(&conn, "/tmp/existing", None).unwrap();

        let ran = auto_bootstrap(&conn).unwrap();
        assert!(!ran, "bootstrap should NOT run when DB already has entries");
    }
}
