use anyhow::{Context, Result};
use rusqlite::Connection;

/// Print all waypoints to stderr (UI output goes to stderr, paths to stdout).
pub fn list_waypoints(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT name, path FROM waypoints ORDER BY name")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut count = 0;
    for row in rows {
        let (name, path) = row?;
        eprintln!("  !{:<20} {}", name, path);
        count += 1;
    }

    if count == 0 {
        eprintln!("No waypoints set. Use `tp --mark <name> [path]` to create one.");
    }

    Ok(())
}

/// Create or replace a waypoint. The path is canonicalized to an absolute path.
pub fn add_waypoint(conn: &Connection, name: &str, path: &str) -> Result<()> {
    let canonical =
        std::fs::canonicalize(path).with_context(|| format!("could not resolve path: {}", path))?;
    let path_str = canonical.to_string_lossy();

    conn.execute(
        "INSERT OR REPLACE INTO waypoints (name, path) VALUES (?1, ?2)",
        rusqlite::params![name, path_str.as_ref()],
    )?;

    eprintln!("Waypoint !{} → {}", name, path_str);
    Ok(())
}

/// Remove a waypoint by name.
pub fn remove_waypoint(conn: &Connection, name: &str) -> Result<()> {
    let changed = conn.execute("DELETE FROM waypoints WHERE name = ?1", [name])?;
    if changed == 0 {
        anyhow::bail!("waypoint '{}' not found", name);
    }
    eprintln!("Removed waypoint !{}", name);
    Ok(())
}

/// Resolve a waypoint name to its path and print to stdout (for shell wrapper cd).
#[allow(dead_code)]
pub fn jump_to_waypoint(conn: &Connection, name: &str) -> Result<()> {
    match resolve_waypoint(conn, name)? {
        Some(p) => {
            println!("{}", p);
            Ok(())
        }
        None => anyhow::bail!("waypoint '{}' not found", name),
    }
}

/// Internal: resolve a waypoint name to a path string.
pub fn resolve_waypoint(conn: &Connection, name: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT path FROM waypoints WHERE name = ?1")?;
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
    fn test_add_and_resolve_waypoint() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_str().unwrap();

        add_waypoint(&conn, "test", dir).unwrap();
        let resolved = resolve_waypoint(&conn, "test").unwrap();
        assert!(resolved.is_some());
    }

    #[test]
    fn test_remove_waypoint() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_str().unwrap();

        add_waypoint(&conn, "remove_me", dir).unwrap();
        assert!(resolve_waypoint(&conn, "remove_me").unwrap().is_some());

        remove_waypoint(&conn, "remove_me").unwrap();
        assert!(resolve_waypoint(&conn, "remove_me").unwrap().is_none());
    }

    #[test]
    fn test_remove_nonexistent_waypoint() {
        let conn = db::open_memory().unwrap();
        let result = remove_waypoint(&conn, "nope");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_waypoints_empty() {
        let conn = db::open_memory().unwrap();
        list_waypoints(&conn).unwrap();
    }

    #[test]
    fn test_waypoint_replace() {
        let conn = db::open_memory().unwrap();
        let tmp1 = tempfile::tempdir().unwrap();
        let tmp2 = tempfile::tempdir().unwrap();

        add_waypoint(&conn, "swap", tmp1.path().to_str().unwrap()).unwrap();
        add_waypoint(&conn, "swap", tmp2.path().to_str().unwrap()).unwrap();

        let resolved = resolve_waypoint(&conn, "swap").unwrap().unwrap();
        // Should point to the second directory (INSERT OR REPLACE)
        let expected = std::fs::canonicalize(tmp2.path()).unwrap();
        assert_eq!(resolved, expected.to_string_lossy());
    }
}
