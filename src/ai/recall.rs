use anyhow::Result;
use rusqlite::Connection;

/// Summary of visits to a single directory during a session window.
#[derive(Debug)]
pub struct SessionStat {
    pub path: String,
    pub visit_count: i64,
    pub project_root: Option<String>,
}

/// Query the sessions table for the last 24 hours and return aggregated
/// visit counts per destination path, joined against the directories table
/// to pull in the project root (if known). Results are ordered by visit
/// count descending and capped at 20 rows.
pub fn gather_session_stats(conn: &Connection) -> Result<Vec<SessionStat>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let cutoff = now - 86400; // 24 hours ago

    let mut stmt = conn.prepare(
        "SELECT s.to_path, COUNT(*) as visits, d.project_root
         FROM sessions s
         LEFT JOIN directories d ON s.to_path = d.path
         WHERE s.timestamp > ?1 AND s.to_path IS NOT NULL
         GROUP BY s.to_path
         ORDER BY visits DESC
         LIMIT 20",
    )?;

    let rows = stmt.query_map([cutoff], |row| {
        Ok(SessionStat {
            path: row.get(0)?,
            visit_count: row.get(1)?,
            project_root: row.get(2)?,
        })
    })?;

    let mut stats = Vec::new();
    for row in rows {
        stats.push(row?);
    }
    Ok(stats)
}

/// Format session stats into a human-readable string suitable for sending
/// to the AI model as context. Each line lists the directory, its visit
/// count, and its project root (if known).
fn format_stats_for_ai(stats: &[SessionStat]) -> String {
    let mut out = String::new();
    for stat in stats {
        let project = stat.project_root.as_deref().unwrap_or("(no project)");
        out.push_str(&format!(
            "- {} (visits: {}, project: {})\n",
            stat.path, stat.visit_count, project
        ));
    }
    out
}

/// Call the Anthropic API to generate a short summary of the navigation
/// session. Returns `Some(text)` on success, `None` on any error so that
/// the caller can fall back to raw stats output.
#[cfg(feature = "ai")]
fn call_ai_summary(api_key: &str, stats_text: &str) -> Option<String> {
    let model =
        std::env::var("TP_AI_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string());

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 300,
        "system": "You are a developer productivity assistant. Summarize the navigation session concisely: what projects were they working on, what areas of code, and what might they want to return to. Be brief (3-5 sentences).",
        "messages": [
            { "role": "user", "content": format!("Here are my directory navigation stats from the last 24 hours:\n\n{}", stats_text) }
        ]
    });

    let spinner = crate::style::Spinner::start("recalling your session...");

    let result = (|| -> Option<String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()?;

        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .ok()?;

        let json: serde_json::Value = resp.json().ok()?;
        let text = json["content"][0]["text"].as_str()?;
        Some(text.to_string())
    })();

    spinner.stop();
    result
}

/// AI-powered session recall.
///
/// Gathers navigation stats from the last 24 hours and presents either
/// an AI-generated summary (when an API key is available) or a raw
/// stats breakdown grouped by project.
pub fn session_recall(conn: &Connection) -> Result<()> {
    let stats = gather_session_stats(conn)?;

    if stats.is_empty() {
        eprintln!("No navigation history in the last 24 hours.");
        return Ok(());
    }

    // Try AI summary first (only when the ai feature is compiled in).
    #[cfg(feature = "ai")]
    {
        if let Some((api_key, _source)) = super::detect_api_key() {
            let stats_text = format_stats_for_ai(&stats);
            if let Some(summary) = call_ai_summary(&api_key, &stats_text) {
                eprintln!("{}", summary);
                return Ok(());
            }
            // AI call failed — fall through to raw stats.
        }
    }

    // Fallback: print raw stats grouped by project root.
    print_raw_stats(&stats);
    Ok(())
}

/// Print stats grouped by project root. Used as a fallback when AI is
/// unavailable or the API call fails.
fn print_raw_stats(stats: &[SessionStat]) {
    // Group by project root.
    let mut groups: std::collections::BTreeMap<String, Vec<&SessionStat>> =
        std::collections::BTreeMap::new();

    for stat in stats {
        let key = stat
            .project_root
            .clone()
            .unwrap_or_else(|| "(no project)".to_string());
        groups.entry(key).or_default().push(stat);
    }

    eprintln!("Session recall (last 24 hours):\n");
    for (project, entries) in &groups {
        eprintln!("  {}:", project);
        for entry in entries {
            eprintln!("    {} ({} visits)", entry.path, entry.visit_count);
        }
        eprintln!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::nav::frecency;

    #[test]
    fn test_session_recall_stub() {
        // Updated: now passes a connection instead of being a no-arg stub.
        let conn = db::open_memory().unwrap();
        session_recall(&conn).unwrap();
    }

    #[test]
    fn test_gather_session_stats_empty() {
        // With no sessions recorded, gather_session_stats should return
        // an empty vec (not an error).
        let conn = db::open_memory().unwrap();
        let stats = gather_session_stats(&conn).unwrap();
        assert!(stats.is_empty());
    }

    #[test]
    fn test_gather_session_stats_with_visits() {
        let conn = db::open_memory().unwrap();

        // Record visits: /a gets 3 visits, /b gets 1.
        // frecency::record_visit inserts into both directories and sessions.
        frecency::record_visit(&conn, "/home/user/projects/a", Some("/home/user/projects"))
            .unwrap();
        frecency::record_visit(&conn, "/home/user/projects/a", Some("/home/user/projects"))
            .unwrap();
        frecency::record_visit(&conn, "/home/user/projects/a", Some("/home/user/projects"))
            .unwrap();
        frecency::record_visit(&conn, "/home/user/docs/b", None).unwrap();

        let stats = gather_session_stats(&conn).unwrap();

        // Should have two distinct paths.
        assert_eq!(
            stats.len(),
            2,
            "expected 2 distinct paths, got {}",
            stats.len()
        );

        // Most visited first.
        assert_eq!(stats[0].path, "/home/user/projects/a");
        assert_eq!(stats[0].visit_count, 3);
        assert_eq!(
            stats[0].project_root.as_deref(),
            Some("/home/user/projects")
        );

        assert_eq!(stats[1].path, "/home/user/docs/b");
        assert_eq!(stats[1].visit_count, 1);
        assert!(stats[1].project_root.is_none());
    }
}
