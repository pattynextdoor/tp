# Beta Features Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the 5 beta features — zoxide import, AI reranking with cache, AI setup validation, session recall, and TUI interactive picker — taking tp from alpha to beta.

**Architecture:** Each feature maps to an existing stubbed module. Import is pure CLI logic. AI reranking uses blocking reqwest to hit the Anthropic Messages API with a file-based JSON cache. Session recall queries the sessions table and optionally summarizes via AI. The TUI picker uses ratatui + crossterm in alternate-screen mode with live fuzzy filtering.

**Tech Stack:** Rust, rusqlite, reqwest (blocking), ratatui, crossterm, serde_json (already a dep), Anthropic Messages API.

---

### Task 1: Update Dependencies in Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Step 1: Update reqwest to use blocking feature, drop tokio**

In `Cargo.toml`, change the feature-gated dependencies:

```toml
# Replace these lines:
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false, optional = true }
tokio = { version = "1", features = ["rt", "macros"], optional = true }

# With:
reqwest = { version = "0.12", features = ["blocking", "json", "rustls-tls"], default-features = false, optional = true }
```

Remove `tokio` entirely. Update the `ai` feature flag:

```toml
ai = ["dep:reqwest"]
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: Builds with existing warnings about unused functions (those are the stubs we're about to fill in).

**Step 3: Run tests**

Run: `cargo test`
Expected: All 66 tests pass.

**Step 4: Commit**

```
🔧 chore(deps): switch reqwest to blocking, drop tokio
```

---

### Task 2: Implement Zoxide Import

**Files:**
- Create: `src/import.rs`
- Modify: `src/main.rs` (add `mod import;`)
- Modify: `src/cli.rs:96-100` (replace import stub)
- Test: inline in `src/import.rs`

**Step 1: Write the failing test**

Create `src/import.rs`:

```rust
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::io::BufRead;

use crate::nav::frecency;
use crate::project;

/// Import directories from zoxide's `query --list --score` output format.
///
/// Each line: `  <score> <path>`
/// The score is a float, left-padded with spaces, followed by a space and the path.
pub fn import_zoxide(conn: &Connection, reader: impl BufRead) -> Result<u64> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn test_import_zoxide_basic() {
        let conn = db::open_memory().unwrap();
        let input = "  12.3 /home/user/projects/api\n   5.1 /home/user/code/web\n";
        let count = import_zoxide(&conn, input.as_bytes()).unwrap();
        assert_eq!(count, 2);

        let dir_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(dir_count, 2);
    }

    #[test]
    fn test_import_zoxide_skips_blank_lines() {
        let conn = db::open_memory().unwrap();
        let input = "\n  12.3 /home/user/projects/api\n\n";
        let count = import_zoxide(&conn, input.as_bytes()).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_import_zoxide_empty_input() {
        let conn = db::open_memory().unwrap();
        let count = import_zoxide(&conn, "".as_bytes()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_import_zoxide_deduplicates() {
        let conn = db::open_memory().unwrap();
        // Record a visit first
        frecency::record_visit(&conn, "/home/user/projects/api", None).unwrap();

        let input = "  12.3 /home/user/projects/api\n";
        let count = import_zoxide(&conn, input.as_bytes()).unwrap();
        assert_eq!(count, 1);

        // Should still have only 1 directory entry (upserted)
        let dir_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(dir_count, 1);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test import`
Expected: FAIL with `not yet implemented`

**Step 3: Implement import_zoxide**

Replace the `todo!()` with:

```rust
pub fn import_zoxide(conn: &Connection, reader: impl BufRead) -> Result<u64> {
    let mut count = 0u64;

    for line in reader.lines() {
        let line = line.context("failed to read line")?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Format: "  <score> <path>"
        // Split on first space after the score
        let (score_str, path) = match trimmed.split_once(' ') {
            Some((s, p)) => (s.trim(), p.trim()),
            None => continue,
        };

        let _score: f64 = match score_str.parse() {
            Ok(s) => s,
            Err(_) => continue, // skip unparseable lines
        };

        if path.is_empty() {
            continue;
        }

        let project_root = project::detect_project_root(path);

        conn.execute(
            "INSERT INTO directories (path, frecency, last_access, access_count, project_root)
             VALUES (?1, ?2, strftime('%s','now'), 1, ?3)
             ON CONFLICT(path) DO UPDATE SET
               frecency = MAX(frecency, ?2),
               project_root = COALESCE(?3, project_root)",
            rusqlite::params![path, _score, project_root],
        )?;

        count += 1;
    }

    Ok(count)
}
```

**Step 4: Wire up CLI handler**

In `src/main.rs`, add `mod import;`.

In `src/cli.rs`, replace the `Commands::Import` match arm (lines ~96-100):

```rust
Commands::Import { from, path } => {
    match from.as_str() {
        "zoxide" => {
            let conn = db::open()?;
            // Try `zoxide query -l -s` first, fall back to reading file
            let reader: Box<dyn std::io::BufRead> = if let Some(p) = path {
                let file = std::fs::File::open(p)
                    .context(format!("could not open file: {}", p))?;
                Box::new(std::io::BufReader::new(file))
            } else {
                // Shell out to zoxide
                let output = std::process::Command::new("zoxide")
                    .args(["query", "-l", "-s"])
                    .output()
                    .context("failed to run 'zoxide query -l -s'. Is zoxide installed?")?;
                if !output.status.success() {
                    anyhow::bail!("zoxide query failed: {}", String::from_utf8_lossy(&output.stderr));
                }
                Box::new(std::io::Cursor::new(output.stdout))
            };
            let count = crate::import::import_zoxide(&conn, reader)?;
            eprintln!("Imported {} directories from zoxide.", count);
            Ok(())
        }
        other => {
            eprintln!("Import from '{}' is not yet supported. Supported: zoxide", other);
            Ok(())
        }
    }
}
```

Add `use anyhow::Context;` to the top of `src/cli.rs` if not already present.

**Step 5: Run all tests**

Run: `cargo test`
Expected: All tests pass (old + 4 new import tests).

**Step 6: Commit**

```
✨ feat(import): implement zoxide import via CLI or file
```

---

### Task 3: Implement AI Reranking Core

**Files:**
- Modify: `src/ai/mod.rs` (replace rerank stub)
- Test: inline in `src/ai/mod.rs`

**Step 1: Write the failing test for prompt building**

Add to `src/ai/mod.rs` a helper function and test:

```rust
/// Build the prompt sent to the AI for reranking.
fn build_rerank_prompt(
    query: &str,
    candidates: &[crate::nav::frecency::Candidate],
    cwd: Option<&str>,
) -> String {
    todo!()
}

#[test]
fn test_build_rerank_prompt() {
    let candidates = vec![
        crate::nav::frecency::Candidate {
            path: "/home/user/api".to_string(),
            score: 5.0,
            frecency: 5.0,
            last_access: 0,
            access_count: 3,
            project_root: None,
        },
        crate::nav::frecency::Candidate {
            path: "/home/user/api-old".to_string(),
            score: 4.8,
            frecency: 4.8,
            last_access: 0,
            access_count: 2,
            project_root: None,
        },
    ];
    let prompt = build_rerank_prompt("api", &candidates, Some("/home/user/web"));
    assert!(prompt.contains("/home/user/api"));
    assert!(prompt.contains("/home/user/api-old"));
    assert!(prompt.contains("api"));
    assert!(prompt.contains("/home/user/web"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test build_rerank_prompt`
Expected: FAIL

**Step 3: Implement build_rerank_prompt**

```rust
fn build_rerank_prompt(
    query: &str,
    candidates: &[crate::nav::frecency::Candidate],
    cwd: Option<&str>,
) -> String {
    let mut prompt = format!(
        "A developer typed `tp {}` to navigate to a directory.\n",
        query
    );
    if let Some(cwd) = cwd {
        prompt.push_str(&format!("They are currently in: {}\n", cwd));
    }
    prompt.push_str("\nCandidate directories (0-indexed):\n");
    for (i, c) in candidates.iter().enumerate() {
        prompt.push_str(&format!("  {}: {}\n", i, c.path));
    }
    prompt.push_str("\nReturn ONLY the 0-based index of the best match. No explanation.");
    prompt
}
```

**Step 4: Write the rerank function with HTTP call**

Replace the existing `rerank` stub with:

```rust
use std::collections::HashMap;
use std::time::Duration;

/// Rerank candidates using AI. Returns the path of the best match,
/// or None if AI is unavailable, times out, or fails.
pub fn rerank(
    query: &str,
    candidates: &[crate::nav::frecency::Candidate],
) -> Option<String> {
    // Need at least 2 candidates to rerank
    if candidates.len() < 2 {
        return None;
    }

    let (api_key, _source) = detect_api_key()?;

    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()));

    // Check cache first
    let top_candidates: Vec<&crate::nav::frecency::Candidate> =
        candidates.iter().take(10).collect();
    let cache_key = cache::make_key(query, &top_candidates);
    if let Some(cached_path) = cache::get(&cache_key) {
        return Some(cached_path);
    }

    let user_prompt = build_rerank_prompt(query, &top_candidates.iter().map(|c| (*c).clone()).collect::<Vec<_>>(), cwd.as_deref());

    let model = std::env::var("TP_AI_MODEL")
        .unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string());
    let timeout_ms: u64 = std::env::var("TP_AI_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 50,
        "system": "You are a directory navigation assistant. Given a query and candidate paths, return the 0-based index of the best match. Reply with only the number.",
        "messages": [
            {"role": "user", "content": user_prompt}
        ]
    });

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .timeout(Duration::from_millis(timeout_ms))
        .json(&body)
        .send()
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().ok()?;
    let text = json["content"][0]["text"].as_str()?;
    let index: usize = text.trim().parse().ok()?;

    let path = top_candidates.get(index)?.path.clone();

    // Write to cache
    cache::set(&cache_key, &path);

    Some(path)
}
```

**Step 5: Run tests**

Run: `cargo test -p tp`
Expected: All tests pass. The `test_rerank_returns_none` test still passes because there's no API key in CI.

**Step 6: Commit**

```
✨ feat(ai): implement AI reranking via Anthropic Messages API
```

---

### Task 4: Implement AI Response Cache

**Files:**
- Create: `src/ai/cache.rs`
- Modify: `src/ai/mod.rs` (add `pub mod cache;`)

**Step 1: Write the failing tests**

Create `src/ai/cache.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::nav::frecency::Candidate;

const MAX_ENTRIES: usize = 500;
const TTL_SECS: u64 = 86400; // 24 hours

#[derive(Serialize, Deserialize, Default)]
struct CacheStore {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    path: String,
    timestamp: u64,
}

/// Build a cache key from query + candidate paths.
pub fn make_key(query: &str, candidates: &[&Candidate]) -> String {
    todo!()
}

/// Look up a cached result. Returns None if miss or expired.
pub fn get(key: &str) -> Option<String> {
    todo!()
}

/// Store a result in the cache.
pub fn set(key: &str, path: &str) {
    todo!()
}

fn cache_path() -> Option<std::path::PathBuf> {
    crate::db::db_path().ok().map(|p| p.with_file_name("ai_cache.json"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_key_deterministic() {
        let c1 = Candidate {
            path: "/a".to_string(),
            score: 1.0, frecency: 1.0, last_access: 0,
            access_count: 1, project_root: None,
        };
        let c2 = Candidate {
            path: "/b".to_string(),
            score: 2.0, frecency: 2.0, last_access: 0,
            access_count: 1, project_root: None,
        };
        let key1 = make_key("test", &[&c1, &c2]);
        let key2 = make_key("test", &[&c1, &c2]);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_make_key_varies_with_query() {
        let c = Candidate {
            path: "/a".to_string(),
            score: 1.0, frecency: 1.0, last_access: 0,
            access_count: 1, project_root: None,
        };
        let key1 = make_key("foo", &[&c]);
        let key2 = make_key("bar", &[&c]);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_roundtrip() {
        // Use a temp dir for cache
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("TP_DATA_DIR", tmp.path());

        set("test_key", "/home/user/test");
        let result = get("test_key");
        assert_eq!(result, Some("/home/user/test".to_string()));

        std::env::remove_var("TP_DATA_DIR");
    }

    #[test]
    fn test_cache_miss() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("TP_DATA_DIR", tmp.path());

        let result = get("nonexistent");
        assert!(result.is_none());

        std::env::remove_var("TP_DATA_DIR");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test cache`
Expected: FAIL with `not yet implemented`

**Step 3: Implement the cache functions**

Replace the `todo!()` stubs:

```rust
pub fn make_key(query: &str, candidates: &[&Candidate]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    query.hash(&mut hasher);
    // Sort paths for determinism
    let mut paths: Vec<&str> = candidates.iter().map(|c| c.path.as_str()).collect();
    paths.sort();
    for p in paths {
        p.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

pub fn get(key: &str) -> Option<String> {
    let path = cache_path()?;
    let data = std::fs::read_to_string(&path).ok()?;
    let store: CacheStore = serde_json::from_str(&data).ok()?;
    let entry = store.entries.get(key)?;

    // Check TTL
    if now_secs() - entry.timestamp > TTL_SECS {
        return None;
    }

    Some(entry.path.clone())
}

pub fn set(key: &str, path: &str) {
    let Some(cache_file) = cache_path() else { return };

    let mut store: CacheStore = std::fs::read_to_string(&cache_file)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default();

    store.entries.insert(
        key.to_string(),
        CacheEntry {
            path: path.to_string(),
            timestamp: now_secs(),
        },
    );

    // LRU eviction: if over MAX_ENTRIES, remove oldest
    if store.entries.len() > MAX_ENTRIES {
        if let Some(oldest_key) = store
            .entries
            .iter()
            .min_by_key(|(_, v)| v.timestamp)
            .map(|(k, _)| k.clone())
        {
            store.entries.remove(&oldest_key);
        }
    }

    // Write atomically
    if let Ok(json) = serde_json::to_string(&store) {
        let _ = std::fs::write(&cache_file, json);
    }
}
```

**Step 4: Run tests**

Run: `cargo test cache`
Expected: All 4 cache tests pass.

**Step 5: Commit**

```
✨ feat(ai): add file-based AI response cache with 24h TTL
```

---

### Task 5: Wire AI Reranking into Navigation Cascade

**Files:**
- Modify: `src/nav/mod.rs:88-91` (replace Step 5 stub)

**Step 1: Write the integration**

In `src/nav/mod.rs`, replace the Step 5 comment (line ~89):

```rust
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
```

**Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass. AI reranking is a no-op in tests (no API key).

**Step 3: Commit**

```
✨ feat(nav): wire AI reranking into step 5 of navigation cascade
```

---

### Task 6: Implement AI Setup Validation

**Files:**
- Modify: `src/ai/mod.rs` (replace `setup_key` stub)

**Step 1: Write the improved setup_key**

Replace the existing `setup_key()`:

```rust
/// Interactive API key setup — detects existing keys and validates connectivity.
pub fn setup_key() -> Result<()> {
    match detect_api_key() {
        Some((key, source)) => {
            eprintln!("Found API key in {}", source);
            eprintln!("Testing connection...");

            let client = reqwest::blocking::Client::new();
            let body = serde_json::json!({
                "model": "claude-haiku-4-5-20251001",
                "max_tokens": 1,
                "messages": [{"role": "user", "content": "Hi"}]
            });

            match client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .timeout(std::time::Duration::from_secs(5))
                .json(&body)
                .send()
            {
                Ok(resp) if resp.status().is_success() => {
                    eprintln!("Connection successful! AI features are ready.");
                }
                Ok(resp) => {
                    eprintln!(
                        "API returned status {}. Check your key.",
                        resp.status()
                    );
                }
                Err(e) => {
                    eprintln!("Connection failed: {}. Check network/key.", e);
                }
            }
        }
        None => {
            eprintln!("No API key found.\n");
            eprintln!("To enable AI features, set one of these environment variables:");
            eprintln!("  export TP_API_KEY=sk-ant-...");
            eprintln!("  export ANTHROPIC_API_KEY=sk-ant-...\n");
            eprintln!("Then run `tp --setup-ai` again to verify.");
        }
    }
    Ok(())
}
```

**Step 2: Run tests**

Run: `cargo test setup`
Expected: `test_setup_key_runs` still passes (no API key in test env, hits the `None` branch).

**Step 3: Commit**

```
✨ feat(ai): add connectivity test to setup-ai command
```

---

### Task 7: Implement Session Recall

**Files:**
- Modify: `src/ai/recall.rs` (replace stub)
- Modify: function signature to accept `&Connection`
- Modify: `src/cli.rs:153-163` (pass connection to recall)

**Step 1: Write the failing test**

In `src/ai/recall.rs`, add:

```rust
use anyhow::Result;
use rusqlite::Connection;

/// Gather session stats from the last 24 hours.
pub fn gather_session_stats(conn: &Connection) -> Result<Vec<SessionStat>> {
    todo!()
}

#[derive(Debug)]
pub struct SessionStat {
    pub path: String,
    pub visit_count: i64,
    pub project_root: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::nav::frecency;

    #[test]
    fn test_gather_session_stats_empty() {
        let conn = db::open_memory().unwrap();
        let stats = gather_session_stats(&conn).unwrap();
        assert!(stats.is_empty());
    }

    #[test]
    fn test_gather_session_stats_with_visits() {
        let conn = db::open_memory().unwrap();
        frecency::record_visit(&conn, "/home/user/api", Some("/home/user/api")).unwrap();
        frecency::record_visit(&conn, "/home/user/api", Some("/home/user/api")).unwrap();
        frecency::record_visit(&conn, "/home/user/web", Some("/home/user/web")).unwrap();

        let stats = gather_session_stats(&conn).unwrap();
        assert_eq!(stats.len(), 2);
        // Most visited first
        assert_eq!(stats[0].path, "/home/user/api");
        assert_eq!(stats[0].visit_count, 2);
    }
}
```

**Step 2: Run tests to verify failure**

Run: `cargo test gather_session`
Expected: FAIL

**Step 3: Implement gather_session_stats**

```rust
pub fn gather_session_stats(conn: &Connection) -> Result<Vec<SessionStat>> {
    let twenty_four_hours_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64
        - 86400;

    let mut stmt = conn.prepare(
        "SELECT s.to_path, COUNT(*) as visits, d.project_root
         FROM sessions s
         LEFT JOIN directories d ON s.to_path = d.path
         WHERE s.timestamp > ?1 AND s.to_path IS NOT NULL
         GROUP BY s.to_path
         ORDER BY visits DESC
         LIMIT 20",
    )?;

    let stats = stmt
        .query_map([twenty_four_hours_ago], |row| {
            Ok(SessionStat {
                path: row.get(0)?,
                visit_count: row.get(1)?,
                project_root: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(stats)
}
```

**Step 4: Implement session_recall**

Replace the existing `session_recall` stub:

```rust
/// AI-powered session recall, with fallback to raw stats.
pub fn session_recall(conn: &Connection) -> Result<()> {
    let stats = gather_session_stats(conn)?;

    if stats.is_empty() {
        eprintln!("No navigation history in the last 24 hours.");
        return Ok(());
    }

    // Try AI summary if available
    #[cfg(feature = "ai")]
    {
        if let Some((api_key, _)) = super::detect_api_key() {
            let summary = format_stats_for_ai(&stats);
            if let Some(response) = call_ai_summary(&api_key, &summary) {
                eprintln!("{}", response);
                return Ok(());
            }
        }
    }

    // Fallback: print raw stats
    eprintln!("Last 24h navigation summary:\n");

    // Group by project
    let mut by_project: std::collections::HashMap<String, Vec<&SessionStat>> =
        std::collections::HashMap::new();
    for stat in &stats {
        let project = stat
            .project_root
            .as_deref()
            .unwrap_or("(no project)")
            .to_string();
        by_project.entry(project).or_default().push(stat);
    }

    for (project, entries) in &by_project {
        eprintln!("  {}:", project);
        for entry in entries {
            eprintln!("    {} ({} visits)", entry.path, entry.visit_count);
        }
        eprintln!();
    }

    Ok(())
}

fn format_stats_for_ai(stats: &[SessionStat]) -> String {
    let mut s = String::from("Navigation history (last 24h):\n");
    for stat in stats {
        s.push_str(&format!(
            "  {} — {} visits (project: {})\n",
            stat.path,
            stat.visit_count,
            stat.project_root.as_deref().unwrap_or("none")
        ));
    }
    s
}

#[cfg(feature = "ai")]
fn call_ai_summary(api_key: &str, stats_text: &str) -> Option<String> {
    let body = serde_json::json!({
        "model": std::env::var("TP_AI_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string()),
        "max_tokens": 300,
        "system": "You are a developer productivity assistant. Summarize the navigation session concisely: what projects were they working on, what areas of code, and what might they want to return to. Be brief (3-5 sentences).",
        "messages": [{"role": "user", "content": stats_text}]
    });

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(5))
        .json(&body)
        .send()
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().ok()?;
    json["content"][0]["text"].as_str().map(|s| s.to_string())
}
```

**Step 5: Update CLI to pass connection**

In `src/cli.rs`, update the recall handler (~line 153):

```rust
    if cli.recall {
        #[cfg(feature = "ai")]
        {
            let conn = db::open()?;
            crate::ai::recall::session_recall(&conn)?;
            return Ok(());
        }
        #[cfg(not(feature = "ai"))]
        {
            eprintln!("AI features are not enabled. Rebuild with --features ai");
            return Ok(());
        }
    }
```

**Step 6: Run all tests**

Run: `cargo test`
Expected: All tests pass.

**Step 7: Commit**

```
✨ feat(ai): implement session recall with AI summary and stats fallback
```

---

### Task 8: Implement TUI Picker — App State and Rendering

**Files:**
- Modify: `src/tui/mod.rs` (replace stub with full TUI)

This is the largest task. We'll build it in two commits: state + rendering, then event loop.

**Step 1: Write the app state and helper types**

Replace `src/tui/mod.rs` entirely:

```rust
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;

use crate::nav::frecency::Candidate;
use crate::nav::matching;

struct App {
    input: String,
    all_candidates: Vec<CandidateDisplay>,
    filtered: Vec<usize>, // indices into all_candidates
    list_state: ListState,
}

struct CandidateDisplay {
    path: String,
    project_name: Option<String>,
    git_branch: Option<String>,
    relative_time: String,
}

impl App {
    fn new(candidates: &[Candidate]) -> Self {
        let all_candidates: Vec<CandidateDisplay> = candidates
            .iter()
            .map(|c| CandidateDisplay {
                path: c.path.clone(),
                project_name: c.project_root.as_ref().map(|p| {
                    p.rsplit('/').next().unwrap_or(p).to_string()
                }),
                git_branch: c.project_root.as_deref().and_then(get_git_branch),
                relative_time: format_relative_time(c.last_access),
            })
            .collect();

        let filtered: Vec<usize> = (0..all_candidates.len()).collect();
        let mut list_state = ListState::default();
        if !filtered.is_empty() {
            list_state.select(Some(0));
        }

        App {
            input: String::new(),
            all_candidates,
            filtered,
            list_state,
        }
    }

    fn apply_filter(&mut self) {
        if self.input.is_empty() {
            self.filtered = (0..self.all_candidates.len()).collect();
        } else {
            self.filtered = self
                .all_candidates
                .iter()
                .enumerate()
                .filter(|(_, c)| matching::fuzzy_score(&self.input, &c.path) > 0.0)
                .map(|(i, _)| i)
                .collect();
        }
        // Reset selection
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn move_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected > 0 {
                self.list_state.select(Some(selected - 1));
            }
        }
    }

    fn move_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected + 1 < self.filtered.len() {
                self.list_state.select(Some(selected + 1));
            }
        }
    }

    fn selected_path(&self) -> Option<String> {
        let idx = self.list_state.selected()?;
        let candidate_idx = *self.filtered.get(idx)?;
        Some(self.all_candidates[candidate_idx].path.clone())
    }
}

fn get_git_branch(project_root: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", project_root, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() || branch == "HEAD" {
            None
        } else {
            Some(branch)
        }
    } else {
        None
    }
}

fn format_relative_time(epoch: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let diff = (now - epoch).max(0);

    match diff {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{}m ago", diff / 60),
        3600..=86399 => format!("{}h ago", diff / 3600),
        _ => format!("{}d ago", diff / 86400),
    }
}

/// Interactive TUI picker. Returns the selected path or None if cancelled.
pub fn pick(candidates: &[Candidate]) -> Result<Option<String>> {
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut app = App::new(candidates);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_event_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<Option<String>> {
    loop {
        terminal.draw(|f| render(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Enter => return Ok(app.selected_path()),
                KeyCode::Up => app.move_up(),
                KeyCode::Down => app.move_down(),
                KeyCode::Char('k') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    app.move_up();
                }
                KeyCode::Char('j') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    app.move_down();
                }
                KeyCode::Backspace => {
                    app.input.pop();
                    app.apply_filter();
                }
                KeyCode::Char(c) => {
                    app.input.push(c);
                    app.apply_filter();
                }
                _ => {}
            }
        }
    }
}

fn render(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input
            Constraint::Min(1),   // List
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // Input field
    let input = Paragraph::new(format!("> {}", app.input))
        .block(Block::default().borders(Borders::ALL).title(" tp "));
    f.render_widget(input, chunks[0]);

    // Candidate list
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let c = &app.all_candidates[idx];
            let is_selected = app.list_state.selected() == Some(i);
            let marker = if is_selected { "→ " } else { "  " };

            let path_style = if is_selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let mut meta_parts: Vec<String> = Vec::new();
            if let Some(ref name) = c.project_name {
                meta_parts.push(name.clone());
            }
            if let Some(ref branch) = c.git_branch {
                meta_parts.push(branch.clone());
            }
            meta_parts.push(c.relative_time.clone());
            let meta = meta_parts.join(" · ");

            ListItem::new(vec![
                Line::from(Span::styled(format!("{}{}", marker, c.path), path_style)),
                Line::from(Span::styled(
                    format!("    {}", meta),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect();

    let list = List::new(items);
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Status bar
    let status = Paragraph::new(format!(
        " {}/{} matches",
        app.filtered.len(),
        app.all_candidates.len()
    ))
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_empty() {
        // Can't test full TUI in unit tests, but test that empty returns None
        // without entering alternate screen
        let candidates: Vec<Candidate> = vec![];
        let result = pick(&candidates).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_format_relative_time() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert_eq!(format_relative_time(now), "just now");
        assert_eq!(format_relative_time(now - 120), "2m ago");
        assert_eq!(format_relative_time(now - 7200), "2h ago");
        assert_eq!(format_relative_time(now - 172800), "2d ago");
    }

    #[test]
    fn test_app_filtering() {
        let candidates = vec![
            Candidate {
                path: "/home/user/api".to_string(),
                score: 10.0, frecency: 10.0, last_access: 0,
                access_count: 5, project_root: Some("/home/user/api".to_string()),
            },
            Candidate {
                path: "/home/user/web".to_string(),
                score: 5.0, frecency: 5.0, last_access: 0,
                access_count: 2, project_root: Some("/home/user/web".to_string()),
            },
        ];
        let mut app = App::new(&candidates);
        assert_eq!(app.filtered.len(), 2);

        app.input = "api".to_string();
        app.apply_filter();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.selected_path(), Some("/home/user/api".to_string()));
    }

    #[test]
    fn test_app_navigation() {
        let candidates = vec![
            Candidate {
                path: "/a".to_string(),
                score: 10.0, frecency: 10.0, last_access: 0,
                access_count: 1, project_root: None,
            },
            Candidate {
                path: "/b".to_string(),
                score: 5.0, frecency: 5.0, last_access: 0,
                access_count: 1, project_root: None,
            },
        ];
        let mut app = App::new(&candidates);
        assert_eq!(app.list_state.selected(), Some(0));

        app.move_down();
        assert_eq!(app.list_state.selected(), Some(1));

        app.move_down(); // shouldn't go past end
        assert_eq!(app.list_state.selected(), Some(1));

        app.move_up();
        assert_eq!(app.list_state.selected(), Some(0));

        app.move_up(); // shouldn't go past start
        assert_eq!(app.list_state.selected(), Some(0));
    }
}
```

Note: `j`/`k` without modifier are used for typing in the filter input. Use `Ctrl+j`/`Ctrl+k` for vim-style navigation instead, so typing isn't swallowed.

**Step 2: Run tests**

Run: `cargo test tui`
Expected: All 4 TUI tests pass.

**Step 3: Commit**

```
✨ feat(tui): implement interactive fuzzy picker with ratatui
```

---

### Task 9: Wire TUI Picker into CLI and Navigation

**Files:**
- Modify: `src/nav/mod.rs` (update Step 6 + add interactive mode)
- Modify: `src/cli.rs` (pass interactive flag through navigation)

**Step 1: Update navigate() signature to accept interactive flag**

In `src/nav/mod.rs`, update the function signature:

```rust
pub fn navigate(conn: &Connection, query: &[String], interactive: bool) -> Result<Option<NavResult>> {
```

Update Step 6 (the fallback at the end, ~line 92):

```rust
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
```

**Step 2: Update CLI call site**

In `src/cli.rs`, update the navigate call (~line 174):

```rust
    match crate::nav::navigate(&conn, &cli.query, cli.interactive)? {
```

Also handle `tp -i` with no query (show all):

```rust
    // Main navigation flow
    if cli.query.is_empty() && !cli.interactive {
        eprintln!("Usage: tp <query> — teleport to a directory");
        eprintln!("       tp --help  — show all options");
        return Ok(());
    }

    let conn = db::open()?;
    match crate::nav::navigate(&conn, &cli.query, cli.interactive)? {
```

**Step 3: Update navigate() to handle empty query for interactive mode**

At the top of `navigate()`:

```rust
    if query.is_empty() && !interactive {
        return Ok(None);
    }

    let joined = query.join(" ");
```

When query is empty but interactive is true, skip straight to querying all entries:

```rust
    if query.is_empty() && interactive {
        // Show all entries for interactive picker
        let mut stmt = conn.prepare(
            "SELECT path, frecency, last_access, access_count, project_root
             FROM directories ORDER BY frecency DESC LIMIT 100"
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
        return Ok(None);
    }
```

**Step 4: Fix all tests that call navigate()**

Update every test in `src/nav/mod.rs` to pass `false` for the interactive flag:

```rust
// In each test, change:
navigate(&conn, &[...])
// To:
navigate(&conn, &[...], false)
```

**Step 5: Run all tests**

Run: `cargo test`
Expected: All tests pass.

**Step 6: Commit**

```
✨ feat(nav): wire TUI picker into navigation cascade and CLI
```

---

### Task 10: Final Integration Testing and Cleanup

**Files:**
- Modify: any files with compiler warnings

**Step 1: Build with all features**

Run: `cargo build --all-features`
Expected: Builds cleanly. Count remaining warnings — should be zero or near-zero.

**Step 2: Fix any remaining warnings**

Remove any leftover `#[allow(unused)]` or dead code from stubs that are now implemented.

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 4: Manual smoke test**

Run: `cargo run -- add /tmp/test-dir` (add a test entry)
Run: `cargo run -- -i` (verify TUI opens)
Run: `cargo run -- --setup-ai` (verify setup output)

**Step 5: Commit**

```
🔧 chore: clean up warnings and dead code from beta implementation
```
