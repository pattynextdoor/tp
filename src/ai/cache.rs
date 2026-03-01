use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::nav::frecency::Candidate;

/// Maximum number of cache entries before we start evicting the oldest.
const MAX_ENTRIES: usize = 500;

/// Cache TTL: 24 hours in seconds.
const TTL_SECS: u64 = 86_400;

/// The on-disk cache format: a map of hash keys to timestamped entries.
#[derive(Serialize, Deserialize, Default)]
struct CacheStore {
    entries: HashMap<String, CacheEntry>,
}

/// A single cached AI response — the chosen path plus when it was stored.
#[derive(Serialize, Deserialize)]
struct CacheEntry {
    path: String,
    timestamp: u64,
}

/// Build a deterministic cache key from the query and candidate paths.
///
/// We sort candidate paths before hashing so the key is stable regardless
/// of the order candidates arrive in. The hex digest makes it safe for
/// use as a JSON map key.
pub fn make_key(query: &str, candidates: &[&Candidate]) -> String {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    query.hash(&mut hasher);

    let mut paths: Vec<&str> = candidates.iter().map(|c| c.path.as_str()).collect();
    paths.sort();
    for p in paths {
        p.hash(&mut hasher);
    }

    format!("{:016x}", hasher.finish())
}

/// Look up a cached AI response. Returns `Some(path)` if the key exists
/// and hasn't expired (within 24h TTL), otherwise `None`.
pub fn get(key: &str) -> Option<String> {
    get_at(key, &cache_path()?)
}

/// Store an AI response in the cache. Creates the file if it doesn't exist.
///
/// If the cache exceeds 500 entries, the oldest entry (by timestamp) is
/// evicted to keep the file from growing unbounded.
pub fn set(key: &str, path: &str) {
    let Some(cache_file) = cache_path() else {
        return;
    };
    set_at(key, path, &cache_file);
}

/// Internal get: reads from an explicit cache file path.
fn get_at(key: &str, cache_file: &PathBuf) -> Option<String> {
    let data = std::fs::read_to_string(cache_file).ok()?;
    let store: CacheStore = serde_json::from_str(&data).ok()?;
    let entry = store.entries.get(key)?;

    if now_secs().saturating_sub(entry.timestamp) > TTL_SECS {
        return None;
    }

    Some(entry.path.clone())
}

/// Internal set: writes to an explicit cache file path.
fn set_at(key: &str, path: &str, cache_file: &PathBuf) {
    let mut store: CacheStore = std::fs::read_to_string(cache_file)
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

    // Evict oldest entries if we've exceeded the limit.
    while store.entries.len() > MAX_ENTRIES {
        if let Some(oldest_key) = store
            .entries
            .iter()
            .min_by_key(|(_, e)| e.timestamp)
            .map(|(k, _)| k.clone())
        {
            store.entries.remove(&oldest_key);
        } else {
            break;
        }
    }

    if let Ok(json) = serde_json::to_string(&store) {
        let _ = std::fs::write(cache_file, json);
    }
}

/// Derive the cache file path from the database path.
///
/// We reuse `db::db_path()` (which points to `…/tp/tp.db`) and swap the
/// filename to `ai_cache.json` so the cache lives in the same data directory.
fn cache_path() -> Option<PathBuf> {
    crate::db::db_path()
        .ok()
        .map(|p| p.with_file_name("ai_cache.json"))
}

/// Current UNIX timestamp in seconds.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_make_key_deterministic() {
        let c1 = candidate("/home/user/projects");
        let c2 = candidate("/home/user/docs");
        let refs: Vec<&Candidate> = vec![&c1, &c2];

        let key_a = make_key("proj", &refs);
        let key_b = make_key("proj", &refs);
        assert_eq!(key_a, key_b, "same inputs must produce the same key");
    }

    #[test]
    fn test_make_key_varies_with_query() {
        let c1 = candidate("/home/user/projects");
        let refs: Vec<&Candidate> = vec![&c1];

        let key_a = make_key("proj", &refs);
        let key_b = make_key("docs", &refs);
        assert_ne!(key_a, key_b, "different queries must produce different keys");
    }

    #[test]
    fn test_cache_roundtrip() {
        // Use a temp file directly instead of env vars to avoid parallel test races.
        let tmp = tempfile::tempdir().unwrap();
        let cache_file = tmp.path().join("ai_cache.json");

        let c1 = candidate("/home/user/projects");
        let refs: Vec<&Candidate> = vec![&c1];
        let key = make_key("proj", &refs);

        set_at(&key, "/home/user/projects", &cache_file);
        let result = get_at(&key, &cache_file);
        assert_eq!(result, Some("/home/user/projects".to_string()));
    }

    #[test]
    fn test_cache_miss() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_file = tmp.path().join("ai_cache.json");

        let result = get_at("nonexistent_key_12345", &cache_file);
        assert!(result.is_none(), "missing key should return None");
    }
}
