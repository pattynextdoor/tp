pub mod frecency;
pub mod matching;
pub mod waypoints;

use anyhow::Result;
use rusqlite::Connection;

/// The result of a navigation query — a path to cd into.
pub struct NavResult {
    pub path: String,
    pub match_type: String,
}

/// Navigate to a directory matching the query.
///
/// Stub: the full 6-step cascade is implemented in module 8.
pub fn navigate(_conn: &Connection, _query: &[String]) -> Result<Option<NavResult>> {
    Ok(None)
}
