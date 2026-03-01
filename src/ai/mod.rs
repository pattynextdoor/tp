pub mod recall;

use anyhow::Result;

/// Stub: detect API key. Full implementation in module 9.
pub fn detect_api_key() -> Option<(String, &'static str)> {
    None
}

/// Stub: setup key. Full implementation in module 9.
pub fn setup_key() -> Result<()> {
    eprintln!("AI setup is not yet implemented.");
    Ok(())
}

/// Stub: rerank. Full implementation in module 9.
pub fn rerank(
    _query: &str,
    _candidates: &[crate::nav::frecency::Candidate],
) -> Option<String> {
    None
}
