pub mod cache;
pub mod recall;

use anyhow::Result;

/// Check environment variables for an API key in priority order:
/// 1. `TP_API_KEY` (explicit tp config)
/// 2. `ANTHROPIC_API_KEY` (shared with Claude Code, etc.)
/// 3. `OPENAI_API_KEY`
///
/// Returns the key and which variable it came from.
pub fn detect_api_key() -> Option<(String, &'static str)> {
    for var in &["TP_API_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY"] {
        if let Ok(key) = std::env::var(var) {
            if !key.is_empty() {
                return Some((key, var));
            }
        }
    }
    None
}

/// Interactive API key setup — detects existing keys and tests connectivity.
///
/// When an API key is found, sends a minimal request to Anthropic to verify
/// the key is valid and the network is reachable. This gives users immediate
/// feedback when running `tp --setup-ai`.
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
                    eprintln!("API returned status {}. Check your key.", resp.status());
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

/// Build the prompt sent to the AI model for reranking.
///
/// The prompt lists numbered candidate paths alongside the query and current
/// working directory, and asks the model to return *only* the 0-based index
/// of the best match. Keeping the expected response to a single number makes
/// parsing trivial and minimizes token usage.
fn build_rerank_prompt(
    query: &str,
    candidates: &[crate::nav::frecency::Candidate],
    cwd: Option<&str>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(&format!("Query: {}\n", query));
    if let Some(cwd) = cwd {
        prompt.push_str(&format!("Current directory: {}\n", cwd));
    }
    prompt.push_str("Candidates:\n");
    for (i, c) in candidates.iter().enumerate() {
        prompt.push_str(&format!("  {}: {}\n", i, c.path));
    }
    prompt.push_str("\nReturn ONLY the 0-based index of the best match. No explanation.");
    prompt
}

/// Rerank candidates using AI. Returns the path of the best match, or `None`
/// if AI reranking is unavailable or fails for any reason.
///
/// This function is designed to be *fail-safe*: network errors, timeouts,
/// malformed responses, and missing API keys all result in a silent `None`
/// so that navigation always falls back to the frecency-based ranking.
#[cfg(feature = "ai")]
pub fn rerank(query: &str, candidates: &[crate::nav::frecency::Candidate]) -> Option<String> {
    // Not enough candidates to justify an API call.
    if candidates.len() < 2 {
        return None;
    }

    let (api_key, _source) = detect_api_key()?;

    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(String::from));

    // Only consider the top 10 candidates to keep the prompt small and fast.
    let top: Vec<&crate::nav::frecency::Candidate> = candidates.iter().take(10).collect();

    // Check cache first — avoids a network round-trip for repeated queries.
    let cache_key = cache::make_key(query, &top);
    if let Some(cached_path) = cache::get(&cache_key) {
        return Some(cached_path);
    }

    let prompt = build_rerank_prompt(query, &candidates[..top.len()], cwd.as_deref());

    let model =
        std::env::var("TP_AI_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string());

    let timeout_ms: u64 = std::env::var("TP_AI_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 50,
        "system": "You are a directory navigation assistant. Given a query and candidate directory paths, pick the single best match.",
        "messages": [
            { "role": "user", "content": prompt }
        ]
    });

    let spinner = crate::style::Spinner::start("consulting the oracle...");

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

        // Cache the result so we don't call the API again for the same query.
        cache::set(&cache_key, &chosen.path);

        Some(chosen.path.clone())
    })();

    spinner.stop();
    result
}

/// Fallback when the `ai` feature is disabled: always returns `None`.
#[cfg(not(feature = "ai"))]
pub fn rerank(_query: &str, _candidates: &[crate::nav::frecency::Candidate]) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::frecency::Candidate;

    /// Helper: build a minimal Candidate for testing.
    fn candidate(path: &str) -> Candidate {
        Candidate {
            path: path.to_string(),
            score: 0.0,
            frecency: 0.0,
            last_access: 0,
            access_count: 0,
            project_root: None,
        }
    }

    #[test]
    fn test_detect_api_key_none() {
        // In a clean test environment, we just verify it doesn't panic.
        let _ = detect_api_key();
    }

    #[test]
    fn test_rerank_returns_none() {
        // With no API key in the test environment, rerank should return None
        // for empty candidates and for populated candidates alike.
        let result = rerank("api", &[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_rerank_returns_none_with_candidates() {
        let c1 = candidate("/home/user/projects/api");
        let c2 = candidate("/home/user/projects/web");
        let result = rerank("api", &[c1, c2]);
        // No API key in test env → None.
        assert!(result.is_none());
    }

    #[test]
    fn test_build_rerank_prompt() {
        let c1 = candidate("/home/user/projects/api");
        let c2 = candidate("/home/user/docs");
        let prompt = build_rerank_prompt("api", &[c1, c2], Some("/home/user"));

        assert!(
            prompt.contains("Query: api"),
            "prompt must contain the query"
        );
        assert!(
            prompt.contains("/home/user/projects/api"),
            "prompt must contain candidate paths"
        );
        assert!(
            prompt.contains("/home/user/docs"),
            "prompt must contain all candidate paths"
        );
        assert!(
            prompt.contains("Current directory: /home/user"),
            "prompt must contain the cwd"
        );
        assert!(
            prompt.contains("0-based index"),
            "prompt must ask for a 0-based index"
        );
    }

    #[test]
    fn test_build_rerank_prompt_no_cwd() {
        let c1 = candidate("/tmp/foo");
        let prompt = build_rerank_prompt("foo", &[c1], None);

        assert!(prompt.contains("Query: foo"));
        assert!(!prompt.contains("Current directory:"));
    }

    #[test]
    fn test_setup_key_runs() {
        setup_key().unwrap();
    }
}
