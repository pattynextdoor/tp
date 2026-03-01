use anyhow::Result;
use rusqlite::Connection;

use super::matching;

/// A candidate directory returned from a frecency query.
#[derive(Debug, Clone)]
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
        s if s < 300 => 4.0,       // <5 minutes
        s if s < 3600 => 2.0,      // <1 hour
        s if s < 86400 => 1.0,     // <1 day
        s if s < 604800 => 0.5,    // <1 week
        _ => 0.25,                  // older
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
pub fn record_visit(
    conn: &Connection,
    path: &str,
    project_root: Option<&str>,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO directories (path, frecency, last_access, access_count, project_root)
         VALUES (?1, 1.0, ?2, 1, ?3)
         ON CONFLICT(path) DO UPDATE SET
           frecency = frecency + 1.0,
           last_access = ?2,
           access_count = access_count + 1,
           project_root = COALESCE(?3, project_root)",
        rusqlite::params![path, now, project_root],
    )?;

    conn.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (NULL, ?1, 'visit')",
        [path],
    )?;

    let total: f64 = conn.query_row(
        "SELECT COALESCE(SUM(frecency), 0.0) FROM directories",
        [],
        |row| row.get(0),
    )?;

    if total > 10_000.0 {
        age_scores(conn, now)?;
    }

    Ok(())
}

/// Recalculate all frecency scores based on current time and prune stale entries.
///
/// Entries with frecency < 0.1 and no access in 30 days are removed.
fn age_scores(conn: &Connection, now: i64) -> Result<()> {
    let thirty_days_ago = now - 30 * 86400;

    let mut stmt =
        conn.prepare("SELECT id, access_count, last_access FROM directories")?;
    let rows: Vec<(i64, i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    for (id, count, last) in &rows {
        let new_score = calculate_frecency(*count, *last, now);
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

    for row in rows {
        let (path, frecency, last_access, access_count, project_root) = row?;

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

    candidates
        .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn test_time_weight() {
        assert_eq!(time_weight(60), 4.0);       // <5min
        assert_eq!(time_weight(1800), 2.0);     // <1hr
        assert_eq!(time_weight(43200), 1.0);    // <1day
        assert_eq!(time_weight(259200), 0.5);   // <1week
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
        record_visit(&conn, "/home/user/projects/api", None).unwrap();
        record_visit(&conn, "/home/user/projects/web", None).unwrap();

        let results = query_frecency(&conn, "api", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "/home/user/projects/api");
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
            .query_row("SELECT SUM(frecency) FROM directories", [], |row| row.get(0))
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
}
