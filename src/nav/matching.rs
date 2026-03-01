/// Score how well a query matches a given path.
///
/// Returns a value between 0.0 and 1.0. This is a minimal stub
/// for the frecency engine — full implementation comes in module 3.
pub fn fuzzy_score(query: &str, path: &str) -> f64 {
    let query_lower = query.to_lowercase();
    let path_lower = path.to_lowercase();

    let last_component = path_lower.rsplit('/').next().unwrap_or(&path_lower);

    if last_component == query_lower {
        return 1.0;
    }
    if last_component.ends_with(&query_lower) {
        return 0.9;
    }
    if path_lower.contains(&query_lower) {
        return 0.7;
    }

    let words: Vec<&str> = query_lower.split_whitespace().collect();
    if words.len() > 1 && words.iter().all(|w| path_lower.contains(w)) {
        return 0.6;
    }

    0.0
}
