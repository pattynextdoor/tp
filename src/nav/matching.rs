/// Detect whether a query string is a literal filesystem path.
///
/// Returns true for `.`, `..`, `-`, `~`, `/foo`, `./foo`, `../foo`,
/// any string starting with `~/`, or any bare name that exists as a
/// directory relative to the current working directory (e.g. `src`).
pub fn is_literal_path(query: &str) -> bool {
    matches!(query, "." | ".." | "-" | "~")
        || query.starts_with('/')
        || query.starts_with("./")
        || query.starts_with("../")
        || query.starts_with("~/")
        || std::path::Path::new(query).is_dir()
}

/// Score how well a query matches a given path.
///
/// Returns a value between 0.0 and 1.0:
/// - 1.0: exact last component match
/// - 0.9: last component ends with query (suffix)
/// - 0.7: query is a substring of the path
/// - 0.6: multi-word query where all words match
/// - 0.0: no match
pub fn fuzzy_score(query: &str, path: &str) -> f64 {
    let query_lower = query.to_lowercase();
    let path_lower = path.to_lowercase();

    let last_component = path_lower.rsplit('/').next().unwrap_or(&path_lower);

    // Exact last component match
    if last_component == query_lower {
        return 1.0;
    }

    // Suffix match on last component
    if last_component.ends_with(&query_lower) {
        return 0.9;
    }

    // Substring match anywhere in path
    if path_lower.contains(&query_lower) {
        return 0.7;
    }

    // Multi-word: all words must appear somewhere in the path
    let words: Vec<&str> = query_lower.split_whitespace().collect();
    if words.len() > 1 && words.iter().all(|w| path_lower.contains(w)) {
        return 0.6;
    }

    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_literal_path tests ---

    #[test]
    fn test_literal_dot() {
        assert!(is_literal_path("."));
    }

    #[test]
    fn test_literal_dotdot() {
        assert!(is_literal_path(".."));
    }

    #[test]
    fn test_literal_dash() {
        assert!(is_literal_path("-"));
    }

    #[test]
    fn test_literal_tilde() {
        assert!(is_literal_path("~"));
    }

    #[test]
    fn test_literal_absolute() {
        assert!(is_literal_path("/usr/local"));
        assert!(is_literal_path("/"));
    }

    #[test]
    fn test_literal_relative() {
        assert!(is_literal_path("./src"));
        assert!(is_literal_path("../parent"));
    }

    #[test]
    fn test_literal_home() {
        assert!(is_literal_path("~/Documents"));
    }

    #[test]
    fn test_not_literal() {
        assert!(!is_literal_path("api"));
        assert!(!is_literal_path("my project"));
        assert!(!is_literal_path("nonexistent_dir_xyz_123"));
    }

    #[test]
    fn test_literal_existing_relative_dir() {
        // A bare name that exists as a directory should be treated as literal
        let tmp = tempfile::tempdir().unwrap();
        let subdir = tmp.path().join("mydir");
        std::fs::create_dir(&subdir).unwrap();

        // Change to the temp dir so "mydir" resolves relatively
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        assert!(is_literal_path("mydir"));
        std::env::set_current_dir(original).unwrap();
    }

    // --- fuzzy_score tests ---

    #[test]
    fn test_fuzzy_exact_component() {
        assert_eq!(fuzzy_score("api", "/home/user/projects/api"), 1.0);
        assert_eq!(fuzzy_score("API", "/home/user/projects/api"), 1.0);
    }

    #[test]
    fn test_fuzzy_suffix() {
        assert_eq!(fuzzy_score("api", "/home/user/projects/my-api"), 0.9);
    }

    #[test]
    fn test_fuzzy_substring() {
        assert_eq!(fuzzy_score("proj", "/home/user/projects/api"), 0.7);
    }

    #[test]
    fn test_fuzzy_multiword() {
        assert_eq!(fuzzy_score("user api", "/home/user/projects/api"), 0.6);
    }

    #[test]
    fn test_fuzzy_no_match() {
        assert_eq!(fuzzy_score("zzz", "/home/user/projects/api"), 0.0);
    }

    #[test]
    fn test_fuzzy_case_insensitive() {
        assert_eq!(fuzzy_score("API", "/home/user/projects/api"), 1.0);
        assert_eq!(fuzzy_score("api", "/home/user/Projects/API"), 1.0);
    }

    #[test]
    fn test_fuzzy_empty_query() {
        // Empty string matches as a suffix of the last component (ends_with("") is true)
        assert_eq!(fuzzy_score("", "/home/user"), 0.9);
    }

    #[test]
    fn test_fuzzy_root_path() {
        assert_eq!(fuzzy_score("home", "/home"), 1.0);
    }
}
