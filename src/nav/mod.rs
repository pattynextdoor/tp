pub mod frecency;
pub mod matching;
pub mod suggest;
pub mod waypoints;

use anyhow::Result;
use rusqlite::Connection;

/// The result of a navigation query — a path to cd into.
pub struct NavResult {
    pub path: String,
    #[allow(dead_code)]
    pub match_type: String,
}

/// The 6-step navigation cascade.
///
/// 1. Literal path → pass through (if it's a directory)
/// 2. `!name` → waypoint lookup
/// 3. `@project` → project root jump
/// 4. Frecency + fuzzy → if top score >0.8, navigate immediately
/// 5. AI reranking — if top scores are close, ask AI to break the tie
/// 6. TUI picker or best guess fallback
///
/// When `interactive` is true the TUI picker is shown for ambiguous
/// results (or when there is no query at all, listing all entries).
pub fn navigate(
    conn: &Connection,
    query: &[String],
    interactive: bool,
    project_scoped: bool,
) -> Result<Option<NavResult>> {
    if query.is_empty() && !interactive {
        return Ok(None);
    }

    // `tp` or `tp -i` with no query — show the top entries in the TUI picker.
    if query.is_empty() && interactive {
        let candidates = frecency::query_all(conn, 100)?;

        #[cfg(feature = "tui")]
        {
            if let Some(path) = crate::tui::pick(&candidates)? {
                return Ok(Some(NavResult {
                    path,
                    match_type: "picker".to_string(),
                }));
            }
        }

        // Suppress unused-variable warning when TUI feature is disabled.
        let _ = &candidates;

        return Ok(None);
    }

    let joined = query.join(" ");

    // Step 1: Literal path — if query looks like a filesystem path, pass through
    if matching::is_literal_path(&joined) {
        let expanded = shellexpand::tilde(&joined).to_string();
        let path = std::path::Path::new(&expanded);
        if path.is_dir() {
            return Ok(Some(NavResult {
                path: path
                    .canonicalize()
                    .unwrap_or_else(|_| path.to_path_buf())
                    .to_string_lossy()
                    .to_string(),
                match_type: "literal".to_string(),
            }));
        }
    }

    // Step 2: Waypoint lookup — query starts with !
    if let Some(name) = joined.strip_prefix('!') {
        let name = name.trim();
        if !name.is_empty() {
            if let Some(path) = waypoints::resolve_waypoint(conn, name)? {
                return Ok(Some(NavResult {
                    path,
                    match_type: "waypoint".to_string(),
                }));
            }
        }
    }

    // Step 3: Project root jump — query starts with @
    if let Some(name) = joined.strip_prefix('@') {
        let name = name.trim();
        if !name.is_empty() {
            if let Some(path) = resolve_project(conn, name)? {
                return Ok(Some(NavResult {
                    path,
                    match_type: "project".to_string(),
                }));
            }
        }
    }

    // Step 4: Frecency + fuzzy matching
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()));
    let project_scope = cwd.as_deref().and_then(crate::project::detect_project_root);

    let apply_project_filter = |candidates: &mut Vec<frecency::Candidate>| {
        if project_scoped {
            if let Some(ref scope) = project_scope {
                candidates.retain(|c| c.path.starts_with(scope.as_str()));
            }
        }
    };

    let mut candidates = frecency::query_frecency(conn, &joined, project_scope.as_deref())?;
    apply_project_filter(&mut candidates);

    // Step 4b: Typo-tolerant fallback — if fuzzy matching found nothing,
    // try Damerau-Levenshtein on the last path component
    if candidates.is_empty() {
        candidates = frecency::query_frecency_typo(conn, &joined, project_scope.as_deref())?;
        apply_project_filter(&mut candidates);
    }

    if let Some(best) = candidates.first() {
        if best.score > 0.8 {
            return Ok(Some(NavResult {
                path: best.path.clone(),
                match_type: "frecency".to_string(),
            }));
        }
    }

    // Step 5: AI reranking — if top scores are close, ask AI to break the tie
    #[cfg(feature = "ai")]
    {
        if candidates.len() >= 2 {
            let top = candidates[0].score;
            let second = candidates[1].score;
            // Trigger AI only when scores are within 20% of each other
            if top > 0.0 && (top - second) / top < 0.2 {
                if let Some(path) = crate::ai::rerank(&joined, &candidates) {
                    return Ok(Some(NavResult {
                        path,
                        match_type: "ai".to_string(),
                    }));
                }
            }
        }
    }

    // Step 6: TUI picker or best guess fallback
    #[cfg(feature = "tui")]
    {
        if interactive && !candidates.is_empty() {
            if let Some(path) = crate::tui::pick(&candidates)? {
                return Ok(Some(NavResult {
                    path,
                    match_type: "picker".to_string(),
                }));
            }
            return Ok(None); // User cancelled
        }
    }

    if let Some(best) = candidates.first() {
        return Ok(Some(NavResult {
            path: best.path.clone(),
            match_type: "fallback".to_string(),
        }));
    }

    Ok(None)
}

/// Navigate back N steps in session history.
/// Looks at the sessions table for recent from_path entries,
/// skipping the current directory, dead paths, and deduplicating.
pub fn navigate_back(conn: &Connection, steps: usize) -> Result<Option<String>> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut stmt = conn.prepare(
        "SELECT DISTINCT from_path FROM sessions
         WHERE from_path IS NOT NULL AND from_path != ''
         ORDER BY timestamp DESC
         LIMIT 100",
    )?;

    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<String> = Vec::new();

    for row in rows {
        let path = row?;
        // Skip current directory and duplicates
        if path == cwd || !seen.insert(path.clone()) {
            continue;
        }
        // Only include paths that still exist
        if std::path::Path::new(&path).exists() {
            stack.push(path);
            if stack.len() >= steps {
                break;
            }
        }
    }

    Ok(stack.into_iter().last())
}

/// Look up a project by name from the projects table.
fn resolve_project(conn: &Connection, name: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT path FROM projects WHERE name = ?1 LIMIT 1")?;
    match stmt.query_row([name], |row| row.get::<_, String>(0)) {
        Ok(path) => Ok(Some(path)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn test_navigate_empty_query() {
        let conn = db::open_memory().unwrap();
        let result = navigate(&conn, &[], false, false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_navigate_literal_path() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        let result = navigate(&conn, &[path], false, false).unwrap();
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().match_type, "literal");
    }

    #[test]
    fn test_navigate_waypoint() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_str().unwrap();

        waypoints::add_waypoint(&conn, "test", dir).unwrap();

        let result = navigate(&conn, &["!test".to_string()], false, false).unwrap();
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().match_type, "waypoint");
    }

    #[test]
    fn test_navigate_project() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_str().unwrap();

        conn.execute(
            "INSERT INTO projects (path, name) VALUES (?1, ?2)",
            rusqlite::params![dir, "myproject"],
        )
        .unwrap();

        let result = navigate(&conn, &["@myproject".to_string()], false, false).unwrap();
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().match_type, "project");
    }

    #[test]
    fn test_navigate_frecency() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let api_dir = tmp.path().join("api");
        std::fs::create_dir(&api_dir).unwrap();
        let api_path = api_dir.to_str().unwrap();

        frecency::record_visit(&conn, api_path, None).unwrap();
        frecency::record_visit(&conn, api_path, None).unwrap();
        frecency::record_visit(&conn, api_path, None).unwrap();

        let result = navigate(&conn, &["api".to_string()], false, false).unwrap();
        assert!(result.is_some());
        let mt = &result.unwrap().match_type;
        assert!(mt == "frecency" || mt == "fallback");
    }

    #[test]
    fn test_navigate_project_scoped() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Create two "projects"
        let proj_a = tmp.path().join("project-a");
        let proj_b = tmp.path().join("project-b");
        let dir_a = proj_a.join("src");
        let dir_b = proj_b.join("src");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        // Record visits with different project roots
        frecency::record_visit(
            &conn,
            dir_a.to_str().unwrap(),
            Some(proj_a.to_str().unwrap()),
        )
        .unwrap();
        frecency::record_visit(
            &conn,
            dir_b.to_str().unwrap(),
            Some(proj_b.to_str().unwrap()),
        )
        .unwrap();

        // Without project scoping, "src" should find both
        let results = frecency::query_frecency(&conn, "src", None).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_navigate_no_match() {
        let conn = db::open_memory().unwrap();
        let result = navigate(&conn, &["nonexistent_xyz_123".to_string()], false, false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_navigate_typo_fallback() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir(&projects_dir).unwrap();
        let projects_path = projects_dir.to_str().unwrap();

        // Build up frecency so it has a real score
        frecency::record_visit(&conn, projects_path, None).unwrap();
        frecency::record_visit(&conn, projects_path, None).unwrap();

        // "projetcs" is a transposition of "projects" — should match via typo fallback
        let result = navigate(&conn, &["projetcs".to_string()], false, false).unwrap();
        assert!(result.is_some(), "typo fallback should find 'projects'");
        assert!(result.unwrap().path.contains("projects"));
    }

    #[test]
    fn test_navigate_typo_short_query_no_fallback() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        let src_path = src_dir.to_str().unwrap();

        frecency::record_visit(&conn, src_path, None).unwrap();

        // "scr" is too short (3 chars) for typo tolerance
        let result = navigate(&conn, &["scr".to_string()], false, false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_navigate_back_empty_history() {
        let conn = db::open_memory().unwrap();
        let result = navigate_back(&conn, 1).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_navigate_back_one_step() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir_a = tmp.path().join("a");
        let dir_b = tmp.path().join("b");
        std::fs::create_dir(&dir_a).unwrap();
        std::fs::create_dir(&dir_b).unwrap();

        conn.execute(
            "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
            rusqlite::params![dir_a.to_str().unwrap(), dir_b.to_str().unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
            rusqlite::params![dir_b.to_str().unwrap(), dir_a.to_str().unwrap()],
        )
        .unwrap();

        let result = navigate_back(&conn, 1).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_navigate_back_two_steps() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir_a = tmp.path().join("a");
        let dir_b = tmp.path().join("b");
        let dir_c = tmp.path().join("c");
        std::fs::create_dir(&dir_a).unwrap();
        std::fs::create_dir(&dir_b).unwrap();
        std::fs::create_dir(&dir_c).unwrap();

        conn.execute(
            "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
            rusqlite::params![dir_a.to_str().unwrap(), dir_b.to_str().unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
            rusqlite::params![dir_b.to_str().unwrap(), dir_c.to_str().unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
            rusqlite::params![dir_c.to_str().unwrap(), dir_a.to_str().unwrap()],
        )
        .unwrap();

        let result = navigate_back(&conn, 2).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_navigate_back_deduplicates() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir_a = tmp.path().join("a");
        std::fs::create_dir(&dir_a).unwrap();

        for _ in 0..5 {
            conn.execute(
                "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
                rusqlite::params![dir_a.to_str().unwrap(), "/tmp/whatever"],
            )
            .unwrap();
        }

        // back(2) can only find 1 unique path, so it returns that single path
        // (the function returns whatever it collected, even if fewer than `steps`)
        let result = navigate_back(&conn, 2).unwrap();
        assert_eq!(result.as_deref(), Some(dir_a.to_str().unwrap()));
    }

    #[test]
    fn test_navigate_back_skips_dead_paths() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let good = tmp.path().join("good");
        std::fs::create_dir(&good).unwrap();

        conn.execute(
            "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
            rusqlite::params![good.to_str().unwrap(), "/tmp/whatever"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
            rusqlite::params!["/nonexistent/dead/path", "/tmp/whatever"],
        )
        .unwrap();

        let result = navigate_back(&conn, 1).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), good.to_str().unwrap());
    }
}
