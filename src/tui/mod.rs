use anyhow::Result;

use crate::nav::frecency::Candidate;

/// Stub: interactive TUI picker.
///
/// The real implementation will use ratatui + crossterm to present
/// a fuzzy-filterable list of candidates. For now, just picks the
/// first candidate (highest-scored).
pub fn pick(candidates: &[Candidate]) -> Result<Option<String>> {
    match candidates.first() {
        Some(c) => Ok(Some(c.path.clone())),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_empty() {
        let result = pick(&[]).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_pick_first() {
        let candidates = vec![Candidate {
            path: "/home/user/test".to_string(),
            score: 1.0,
            frecency: 1.0,
            last_access: 0,
            access_count: 1,
            project_root: None,
        }];
        let result = pick(&candidates).unwrap();
        assert_eq!(result, Some("/home/user/test".to_string()));
    }

    #[test]
    fn test_pick_returns_highest_scored() {
        let candidates = vec![
            Candidate {
                path: "/first".to_string(),
                score: 10.0,
                frecency: 10.0,
                last_access: 0,
                access_count: 5,
                project_root: None,
            },
            Candidate {
                path: "/second".to_string(),
                score: 5.0,
                frecency: 5.0,
                last_access: 0,
                access_count: 2,
                project_root: None,
            },
        ];
        let result = pick(&candidates).unwrap();
        // Should return the first candidate (assumed to be pre-sorted by score)
        assert_eq!(result, Some("/first".to_string()));
    }
}
