pub mod frecency;
pub mod matching;
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
) -> Result<Option<NavResult>> {
    if query.is_empty() && !interactive {
        return Ok(None);
    }

    // `tp -i` with no query — show the top entries in the TUI picker.
    if query.is_empty() && interactive {
        let mut stmt = conn.prepare(
            "SELECT path, frecency, last_access, access_count, project_root
             FROM directories ORDER BY frecency DESC LIMIT 100",
        )?;
        let candidates: Vec<frecency::Candidate> = stmt
            .query_map([], |row| {
                Ok(frecency::Candidate {
                    path: row.get(0)?,
                    score: row.get::<_, f64>(1)?,
                    frecency: row.get(1)?,
                    last_access: row.get(2)?,
                    access_count: row.get(3)?,
                    project_root: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

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
    let candidates = frecency::query_frecency(conn, &joined, project_scope.as_deref())?;

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
        let result = navigate(&conn, &[], false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_navigate_literal_path() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        let result = navigate(&conn, &[path], false).unwrap();
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().match_type, "literal");
    }

    #[test]
    fn test_navigate_waypoint() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_str().unwrap();

        waypoints::add_waypoint(&conn, "test", dir).unwrap();

        let result = navigate(&conn, &["!test".to_string()], false).unwrap();
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

        let result = navigate(&conn, &["@myproject".to_string()], false).unwrap();
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().match_type, "project");
    }

    #[test]
    fn test_navigate_frecency() {
        let conn = db::open_memory().unwrap();
        // Add a directory with high frecency
        frecency::record_visit(&conn, "/home/user/projects/api", None).unwrap();
        frecency::record_visit(&conn, "/home/user/projects/api", None).unwrap();
        frecency::record_visit(&conn, "/home/user/projects/api", None).unwrap();

        let result = navigate(&conn, &["api".to_string()], false).unwrap();
        assert!(result.is_some());
        // match_type will be "frecency" if score > 0.8
        let mt = &result.unwrap().match_type;
        assert!(mt == "frecency" || mt == "fallback");
    }

    #[test]
    fn test_navigate_no_match() {
        let conn = db::open_memory().unwrap();
        let result = navigate(&conn, &["nonexistent_xyz_123".to_string()], false).unwrap();
        assert!(result.is_none());
    }
}
