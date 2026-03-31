use anyhow::Result;
use rusqlite::Connection;

/// A directory that matched a keyword search against the semantic index.
pub struct SemanticMatch {
    /// Full absolute path (project_root + rel_path).
    pub path: String,
    /// Relative path within the project.
    pub rel_path: String,
    /// AI-generated description of this directory.
    pub description: String,
    /// Number of query terms found in the description.
    pub keyword_score: usize,
}

/// Search the semantic index for directories whose descriptions contain the
/// given query terms. Returns matches sorted by score descending, then by
/// rel_path length ascending (shallower directories first).
pub fn keyword_search(
    conn: &Connection,
    project_root: &str,
    query_terms: &[&str],
) -> Result<Vec<SemanticMatch>> {
    let mut stmt =
        conn.prepare("SELECT rel_path, description FROM project_dirs WHERE project_root = ?1")?;

    let root = project_root.trim_end_matches('/');

    let rows = stmt.query_map([project_root], |row| {
        let rel_path: String = row.get(0)?;
        let description: String = row.get(1)?;
        Ok((rel_path, description))
    })?;

    let mut matches = Vec::new();
    for row in rows {
        let (rel_path, description) = row?;
        let desc_lower = description.to_lowercase();
        let score = query_terms
            .iter()
            .filter(|term| desc_lower.contains(&term.to_lowercase()))
            .count();

        if score > 0 {
            matches.push(SemanticMatch {
                path: format!("{}/{}", root, rel_path),
                rel_path,
                description,
                keyword_score: score,
            });
        }
    }

    // Sort: highest score first, then shallowest path (shortest rel_path)
    matches.sort_by(|a, b| {
        b.keyword_score
            .cmp(&a.keyword_score)
            .then_with(|| a.rel_path.len().cmp(&b.rel_path.len()))
    });

    Ok(matches)
}

/// Use AI to pick the single best match from a list of keyword search results.
///
/// Returns `None` if there are no matches, no API key, or the AI call fails
/// for any reason. Designed to be fail-safe so navigation always proceeds.
#[cfg(feature = "ai")]
pub fn ai_semantic_search(query: &str, matches: &[SemanticMatch]) -> Option<String> {
    if matches.is_empty() {
        return None;
    }

    let (api_key, _source) = super::detect_api_key()?;

    let top: &[SemanticMatch] = if matches.len() > 10 {
        &matches[..10]
    } else {
        matches
    };

    let mut prompt = String::new();
    prompt.push_str(&format!("Query: {}\nCandidates:\n", query));
    for (i, m) in top.iter().enumerate() {
        prompt.push_str(&format!("  {}: {} — {}\n", i, m.rel_path, m.description));
    }
    prompt.push_str("\nReturn ONLY the 0-based index of the best match. No explanation.");

    let model =
        std::env::var("TP_AI_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string());

    let timeout_ms: u64 = std::env::var("TP_AI_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 50,
        "system": "You are a directory navigation assistant. Given a query and candidate directories with descriptions, pick the single best match.",
        "messages": [
            { "role": "user", "content": prompt }
        ]
    });

    let spinner = crate::style::Spinner::start("searching index...");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()
        .ok();

    let result = (|| -> Option<String> {
        let resp = client?
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .ok()?;

        let json: serde_json::Value = resp.json().ok()?;
        let text = json["content"][0]["text"].as_str()?;
        let index: usize = text.trim().parse().ok()?;

        let chosen = top.get(index)?;
        Some(chosen.path.clone())
    })();

    spinner.stop();
    result
}

/// Fallback when the `ai` feature is disabled: always returns `None`.
#[cfg(not(feature = "ai"))]
pub fn ai_semantic_search(_query: &str, _matches: &[SemanticMatch]) -> Option<String> {
    None
}

/// Check whether a project's semantic index is stale (older than 30 days).
///
/// Returns `Some(project_name)` if the index is stale, `None` otherwise.
pub fn check_staleness(conn: &Connection, project_root: &str) -> Option<String> {
    let (name, indexed_at) = conn
        .query_row(
            "SELECT name, indexed_at FROM projects WHERE path = ?1 AND indexed_at IS NOT NULL",
            [project_root],
            |row| {
                let name: String = row.get(0)?;
                let indexed_at: i64 = row.get(1)?;
                Ok((name, indexed_at))
            },
        )
        .ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;

    let thirty_days = 30 * 24 * 60 * 60;
    if now - indexed_at > thirty_days {
        Some(name)
    } else {
        None
    }
}

/// Check if any semantic index entries exist for a given project root.
pub fn has_index(conn: &Connection, project_root: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM project_dirs WHERE project_root = ?1)",
        [project_root],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Insert a standard set of indexed directories for testing.
    fn setup_indexed_project(conn: &Connection) {
        conn.execute(
            "INSERT INTO project_dirs (project_root, rel_path, description) VALUES (?1, ?2, ?3)",
            ["/projects/myapp", "src/api", "REST API endpoint handlers"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_dirs (project_root, rel_path, description) VALUES (?1, ?2, ?3)",
            [
                "/projects/myapp",
                "src/auth",
                "Authentication and authorization middleware",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_dirs (project_root, rel_path, description) VALUES (?1, ?2, ?3)",
            [
                "/projects/myapp",
                "src/webhooks",
                "Webhook retry handling and delivery",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_dirs (project_root, rel_path, description) VALUES (?1, ?2, ?3)",
            ["/projects/myapp", "tests", "Test suites and fixtures"],
        )
        .unwrap();
    }

    #[test]
    fn test_keyword_search_single_term() {
        let conn = crate::db::open_memory().unwrap();
        setup_indexed_project(&conn);

        let results = keyword_search(&conn, "/projects/myapp", &["webhook"]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].rel_path, "src/webhooks");
        assert_eq!(results[0].keyword_score, 1);
    }

    #[test]
    fn test_keyword_search_multiple_terms() {
        let conn = crate::db::open_memory().unwrap();
        setup_indexed_project(&conn);

        let results = keyword_search(&conn, "/projects/myapp", &["webhook", "retry"]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].keyword_score, 2);
    }

    #[test]
    fn test_keyword_search_no_match() {
        let conn = crate::db::open_memory().unwrap();
        setup_indexed_project(&conn);

        let results = keyword_search(&conn, "/projects/myapp", &["database"]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_keyword_search_case_insensitive() {
        let conn = crate::db::open_memory().unwrap();
        setup_indexed_project(&conn);

        let results = keyword_search(&conn, "/projects/myapp", &["API"]).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|m| m.rel_path == "src/api"));
    }

    #[test]
    fn test_keyword_search_ranked_by_score() {
        let conn = crate::db::open_memory().unwrap();
        setup_indexed_project(&conn);

        let results = keyword_search(&conn, "/projects/myapp", &["handling", "retry"]).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].rel_path, "src/webhooks");
    }

    #[test]
    fn test_keyword_search_builds_full_path() {
        let conn = crate::db::open_memory().unwrap();
        setup_indexed_project(&conn);

        let results = keyword_search(&conn, "/projects/myapp", &["webhook"]).unwrap();
        assert_eq!(results[0].path, "/projects/myapp/src/webhooks");
    }

    #[test]
    fn test_has_index_true() {
        let conn = crate::db::open_memory().unwrap();
        setup_indexed_project(&conn);

        assert!(has_index(&conn, "/projects/myapp"));
    }

    #[test]
    fn test_has_index_false() {
        let conn = crate::db::open_memory().unwrap();

        assert!(!has_index(&conn, "/projects/myapp"));
    }

    #[test]
    fn test_check_staleness_no_project() {
        let conn = crate::db::open_memory().unwrap();

        assert!(check_staleness(&conn, "/projects/nonexistent").is_none());
    }

    #[test]
    fn test_check_staleness_fresh() {
        let conn = crate::db::open_memory().unwrap();
        conn.execute(
            "INSERT INTO projects (path, name, indexed_at) VALUES (?1, ?2, strftime('%s','now'))",
            ["/projects/myapp", "myapp"],
        )
        .unwrap();

        assert!(check_staleness(&conn, "/projects/myapp").is_none());
    }

    #[test]
    fn test_check_staleness_old() {
        let conn = crate::db::open_memory().unwrap();
        // Insert with indexed_at 31 days ago
        let thirty_one_days_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - (31 * 24 * 60 * 60);
        conn.execute(
            "INSERT INTO projects (path, name, indexed_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["/projects/myapp", "myapp", thirty_one_days_ago],
        )
        .unwrap();

        assert_eq!(
            check_staleness(&conn, "/projects/myapp"),
            Some("myapp".to_string())
        );
    }
}
