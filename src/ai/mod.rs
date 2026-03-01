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

/// Interactive API key setup — detects existing keys and confirms usage.
///
/// Stub implementation: prints what was found (no interactive prompts yet).
pub fn setup_key() -> Result<()> {
    match detect_api_key() {
        Some((_, source)) => {
            eprintln!("Found API key in {}", source);
            eprintln!("AI features are ready to use.");
        }
        None => {
            eprintln!("No API key found.");
            eprintln!("Set one of: TP_API_KEY, ANTHROPIC_API_KEY, or OPENAI_API_KEY");
        }
    }
    Ok(())
}

/// Stub: rerank candidates using AI. Returns None (no-op for now).
///
/// The real implementation will make an HTTP call to Claude Haiku or
/// GPT-4o-mini to semantically rank directory candidates against the query.
pub fn rerank(
    _query: &str,
    _candidates: &[crate::nav::frecency::Candidate],
) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_api_key_none() {
        // In a clean test environment, we just verify it doesn't panic.
        let _ = detect_api_key();
    }

    #[test]
    fn test_rerank_returns_none() {
        let result = rerank("api", &[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_setup_key_runs() {
        setup_key().unwrap();
    }
}
