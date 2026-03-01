use anyhow::Result;

use crate::nav::frecency::Candidate;

/// Stub: TUI picker. Full implementation in module 10.
pub fn pick(candidates: &[Candidate]) -> Result<Option<String>> {
    match candidates.first() {
        Some(c) => Ok(Some(c.path.clone())),
        None => Ok(None),
    }
}
