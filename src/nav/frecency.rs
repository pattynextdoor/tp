use anyhow::Result;
use rusqlite::Connection;

use super::matching;

/// Read `TP_EXCLUDE_DIRS` env var and return expanded path prefixes.
/// The var is comma-separated; `~` is expanded via shellexpand.
fn excluded_prefixes() -> Vec<String> {
    match std::env::var("TP_EXCLUDE_DIRS") {
        Ok(val) if !val.is_empty() => val
            .split(',')
            .map(|s| shellexpand::tilde(s.trim()).to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Check if a path starts with any excluded prefix.
fn is_excluded(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

/// A candidate directory returned from a frecency query.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Candidate {
    pub path: String,
    pub score: f64,
    pub frecency: f64,
    pub last_access: i64,
    pub access_count: i64,
    pub project_root: Option<String>,
}

/// Time decay weight buckets for frecency calculation.
///
/// Recent visits are worth more — this mirrors zoxide's approach
/// but with our own weight values.
fn time_weight(elapsed_secs: i64) -> f64 {
    match elapsed_secs {
        s if s < 300 => 4.0,    // <5 minutes
        s if s < 3600 => 2.0,   // <1 hour
        s if s < 86400 => 1.0,  // <1 day
        s if s < 604800 => 0.5, // <1 week
        _ => 0.25,              // older
    }
}

/// Calculate a frecency score from access count, last access time, and current time.
pub fn calculate_frecency(access_count: i64, last_access: i64, now: i64) -> f64 {
    let elapsed = (now - last_access).max(0);
    let weight = time_weight(elapsed);
    access_count as f64 * weight
}

/// Record a visit to a directory. Upserts the directory row,
/// increments its access count, and logs a session entry.
/// Triggers aging if total score exceeds 10,000.
pub fn record_visit(conn: &Connection, path: &str, project_root: Option<&str>) -> Result<()> {
    let excluded = excluded_prefixes();
    if is_excluded(path, &excluded) {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    tx.execute(
        "INSERT INTO directories (path, frecency, last_access, access_count, project_root)
         VALUES (?1, 1.0, ?2, 1, ?3)
         ON CONFLICT(path) DO UPDATE SET
           frecency = frecency + 1.0,
           last_access = ?2,
           access_count = access_count + 1,
           project_root = COALESCE(?3, project_root)",
        rusqlite::params![path, now, project_root],
    )?;

    tx.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (NULL, ?1, 'visit')",
        [path],
    )?;

    let total: f64 = tx.query_row(
        "SELECT COALESCE(SUM(frecency), 0.0) FROM directories",
        [],
        |row| row.get(0),
    )?;

    if total > 10_000.0 {
        age_scores(&tx, now)?;
    }

    tx.commit()?;
    Ok(())
}

/// Recalculate all frecency scores based on current time and prune stale entries.
///
/// Entries with frecency < 0.1 and no access in 30 days are removed.
fn age_scores(conn: &Connection, now: i64) -> Result<()> {
    let thirty_days_ago = now - 30 * 86400;

    let rows: Vec<(i64, i64, i64)> = {
        let mut stmt = conn.prepare("SELECT id, access_count, last_access FROM directories")?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    for (id, count, last) in rows {
        let new_score = calculate_frecency(count, last, now);
        conn.execute(
            "UPDATE directories SET frecency = ?1 WHERE id = ?2",
            rusqlite::params![new_score, id],
        )?;
    }

    conn.execute(
        "DELETE FROM directories WHERE frecency < 0.1 AND last_access < ?1",
        [thirty_days_ago],
    )?;

    Ok(())
}

/// Query the database for directories matching the query string.
///
/// Applies LIKE-based filtering, fuzzy scoring, time decay,
/// and project proximity boost (1.5x if candidate shares project root with scope).
pub fn query_frecency(
    conn: &Connection,
    query: &str,
    project_scope: Option<&str>,
) -> Result<Vec<Candidate>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    let like_pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT path, frecency, last_access, access_count, project_root
         FROM directories
         WHERE path LIKE ?1
         ORDER BY frecency DESC
         LIMIT 100",
    )?;

    let rows = stmt.query_map([&like_pattern], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut dead_paths: Vec<String> = Vec::new();

    for row in rows {
        let (path, frecency, last_access, access_count, project_root) = row?;

        // Self-healing: skip and queue removal for paths that no longer exist
        if !std::path::Path::new(&path).exists() {
            dead_paths.push(path);
            continue;
        }

        let decayed = calculate_frecency(access_count, last_access, now);
        let fuzzy = matching::fuzzy_score(query, &path);

        let proximity_boost = match (&project_root, project_scope) {
            (Some(pr), Some(scope)) if pr == scope => 1.5,
            _ => 1.0,
        };

        let score = decayed * fuzzy * proximity_boost;

        candidates.push(Candidate {
            path,
            score,
            frecency,
            last_access,
            access_count,
            project_root,
        });
    }

    // Silently prune dead paths from the database
    if !dead_paths.is_empty() {
        prune_paths(conn, &dead_paths)?;
    }

    let excluded = excluded_prefixes();
    if !excluded.is_empty() {
        candidates.retain(|c| !is_excluded(&c.path, &excluded));
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(candidates)
}

/// Remove a path from the directories table and its session history.
pub fn remove_path(conn: &Connection, path: &str) -> Result<u64> {
    let deleted = conn.execute("DELETE FROM directories WHERE path = ?1", [path])?;
    conn.execute("DELETE FROM sessions WHERE to_path = ?1", [path])?;
    Ok(deleted as u64)
}

/// Remove a list of paths from the directories table.
/// Called silently during queries to self-heal stale entries.
fn prune_paths(conn: &Connection, paths: &[String]) -> Result<()> {
    for path in paths {
        conn.execute("DELETE FROM directories WHERE path = ?1", [path])?;
        conn.execute("DELETE FROM sessions WHERE to_path = ?1", [path])?;
    }
    Ok(())
}

/// Typo-tolerant fallback query. Scans all paths in the database and scores
/// them with Damerau-Levenshtein distance. Only called when `query_frecency`
/// returns zero results.
pub fn query_frecency_typo(
    conn: &Connection,
    query: &str,
    project_scope: Option<&str>,
) -> Result<Vec<Candidate>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    let mut stmt = conn.prepare(
        "SELECT path, frecency, last_access, access_count, project_root
         FROM directories
         ORDER BY frecency DESC
         LIMIT 500",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut dead_paths: Vec<String> = Vec::new();

    for row in rows {
        let (path, frecency, last_access, access_count, project_root) = row?;

        if !std::path::Path::new(&path).exists() {
            dead_paths.push(path);
            continue;
        }

        let typo = matching::typo_score(query, &path);
        if typo == 0.0 {
            continue;
        }

        let decayed = calculate_frecency(access_count, last_access, now);

        let proximity_boost = match (&project_root, project_scope) {
            (Some(pr), Some(scope)) if pr == scope => 1.5,
            _ => 1.0,
        };

        let score = decayed * typo * proximity_boost;

        candidates.push(Candidate {
            path,
            score,
            frecency,
            last_access,
            access_count,
            project_root,
        });
    }

    if !dead_paths.is_empty() {
        prune_paths(conn, &dead_paths)?;
    }

    let excluded = excluded_prefixes();
    if !excluded.is_empty() {
        candidates.retain(|c| !is_excluded(&c.path, &excluded));
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(candidates)
}

/// Query all directories ordered by frecency, pruning dead paths.
/// Used by the TUI picker and `tp ls` when no query is provided.
pub fn query_all(conn: &Connection, limit: usize) -> Result<Vec<Candidate>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    let mut stmt = conn.prepare(
        "SELECT path, frecency, last_access, access_count, project_root
         FROM directories
         ORDER BY frecency DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map([limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut dead_paths: Vec<String> = Vec::new();

    for row in rows {
        let (path, frecency, last_access, access_count, project_root) = row?;

        if !std::path::Path::new(&path).exists() {
            dead_paths.push(path);
            continue;
        }

        let decayed = calculate_frecency(access_count, last_access, now);

        candidates.push(Candidate {
            path,
            score: decayed,
            frecency,
            last_access,
            access_count,
            project_root,
        });
    }

    if !dead_paths.is_empty() {
        prune_paths(conn, &dead_paths)?;
    }

    let excluded = excluded_prefixes();
    if !excluded.is_empty() {
        candidates.retain(|c| !is_excluded(&c.path, &excluded));
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn test_time_weight() {
        assert_eq!(time_weight(60), 4.0); // <5min
        assert_eq!(time_weight(1800), 2.0); // <1hr
        assert_eq!(time_weight(43200), 1.0); // <1day
        assert_eq!(time_weight(259200), 0.5); // <1week
        assert_eq!(time_weight(1_000_000), 0.25); // older
    }

    #[test]
    fn test_calculate_frecency() {
        let now = 1_000_000;
        // 10 accesses, last 60s ago → 10 * 4.0 = 40.0
        assert_eq!(calculate_frecency(10, now - 60, now), 40.0);
        // 5 accesses, last 2h ago → 5 * 1.0 = 5.0
        assert_eq!(calculate_frecency(5, now - 7200, now), 5.0);
    }

    #[test]
    fn test_record_visit() {
        let conn = db::open_memory().unwrap();
        record_visit(&conn, "/home/user/projects", None).unwrap();
        record_visit(&conn, "/home/user/projects", None).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT access_count FROM directories WHERE path = ?1",
                ["/home/user/projects"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_query_frecency() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let api_dir = tmp.path().join("api");
        let web_dir = tmp.path().join("web");
        std::fs::create_dir(&api_dir).unwrap();
        std::fs::create_dir(&web_dir).unwrap();

        record_visit(&conn, api_dir.to_str().unwrap(), None).unwrap();
        record_visit(&conn, web_dir.to_str().unwrap(), None).unwrap();

        let results = query_frecency(&conn, "api", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, api_dir.to_str().unwrap());
    }

    #[test]
    fn test_aging() {
        let conn = db::open_memory().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // 100 "stale" entries: 90 days old, 0 accesses → after recalc: 0.0 frecency → pruned
        // 100 "recent" entries: now, 10 accesses → after recalc: 40.0 frecency → kept
        for i in 0..200 {
            let (last_access, access_count, frecency) = if i < 100 {
                (now - 90 * 86400, 0i64, 100.0f64)
            } else {
                (now, 10i64, 100.0f64)
            };
            conn.execute(
                "INSERT INTO directories (path, frecency, last_access, access_count)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![format!("/path/{}", i), frecency, last_access, access_count],
            )
            .unwrap();
        }

        let total_before: f64 = conn
            .query_row("SELECT SUM(frecency) FROM directories", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(total_before > 10_000.0);

        record_visit(&conn, "/trigger/aging", None).unwrap();

        let count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))
            .unwrap();
        assert!(
            count_after <= 101,
            "expected pruned count <= 101, got {}",
            count_after
        );
    }

    #[test]
    fn test_remove_path() {
        let conn = db::open_memory().unwrap();
        record_visit(&conn, "/home/user/old-project", None).unwrap();

        let count_before: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM directories WHERE path = ?1",
                ["/home/user/old-project"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_before, 1);

        remove_path(&conn, "/home/user/old-project").unwrap();

        let count_after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM directories WHERE path = ?1",
                ["/home/user/old-project"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_after, 0);
    }

    #[test]
    fn test_excluded_prefixes_empty() {
        std::env::remove_var("TP_EXCLUDE_DIRS");
        let prefixes = excluded_prefixes();
        assert!(prefixes.is_empty());
    }

    #[test]
    fn test_excluded_prefixes_parses() {
        std::env::set_var("TP_EXCLUDE_DIRS", "/tmp,/var/folders");
        let prefixes = excluded_prefixes();
        assert!(prefixes.contains(&"/tmp".to_string()));
        assert!(prefixes.contains(&"/var/folders".to_string()));
        std::env::remove_var("TP_EXCLUDE_DIRS");
    }

    #[test]
    fn test_is_excluded() {
        let prefixes = vec!["/tmp".to_string(), "/var/folders".to_string()];
        assert!(is_excluded("/tmp/foo/bar", &prefixes));
        assert!(is_excluded("/var/folders/abc", &prefixes));
        assert!(!is_excluded("/home/user/projects", &prefixes));
    }

    #[test]
    fn test_record_visit_excludes() {
        std::env::set_var("TP_EXCLUDE_DIRS", "/excluded");
        let conn = db::open_memory().unwrap();
        record_visit(&conn, "/excluded/something", None).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "excluded path should not be recorded");
        std::env::remove_var("TP_EXCLUDE_DIRS");
    }

    #[test]
    fn test_query_frecency_excludes() {
        let conn = db::open_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        let good_dir = tmp.path().join("projects");
        let bad_dir = tmp.path().join("excluded");
        std::fs::create_dir(&good_dir).unwrap();
        std::fs::create_dir(&bad_dir).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO directories (path, frecency, last_access, access_count)
             VALUES (?1, 10.0, ?2, 5)",
            rusqlite::params![good_dir.to_str().unwrap(), now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO directories (path, frecency, last_access, access_count)
             VALUES (?1, 10.0, ?2, 5)",
            rusqlite::params![bad_dir.to_str().unwrap(), now],
        )
        .unwrap();

        std::env::set_var("TP_EXCLUDE_DIRS", bad_dir.to_str().unwrap());
        let results = query_frecency(&conn, "e", None).unwrap();
        assert!(
            !results
                .iter()
                .any(|c| c.path.starts_with(bad_dir.to_str().unwrap())),
            "excluded paths should not appear in results"
        );
        std::env::remove_var("TP_EXCLUDE_DIRS");
    }
}
