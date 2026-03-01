/// Marker files/directories that indicate a project root,
/// paired with the project kind they represent.
const PROJECT_MARKERS: &[(&str, &str)] = &[
    (".git", "git"),
    ("Cargo.toml", "rust"),
    ("package.json", "node"),
    ("go.mod", "go"),
    ("pyproject.toml", "python"),
    ("setup.py", "python"),
    ("Gemfile", "ruby"),
    ("pom.xml", "java"),
    ("build.gradle", "java"),
    ("CMakeLists.txt", "cmake"),
    ("Makefile", "make"),
    (".project", "eclipse"),
    ("composer.json", "php"),
    ("mix.exs", "elixir"),
    ("deno.json", "deno"),
    ("flake.nix", "nix"),
];

/// Maximum number of parent directories to traverse upward.
const MAX_DEPTH: usize = 20;

/// Walk up from `path` looking for project root markers.
///
/// Returns the path to the project root directory, or None if
/// no markers are found within MAX_DEPTH levels.
pub fn detect_project_root(path: &str) -> Option<String> {
    let mut current = std::path::PathBuf::from(path);

    for _ in 0..MAX_DEPTH {
        for (marker, _kind) in PROJECT_MARKERS {
            if current.join(marker).exists() {
                return Some(current.to_string_lossy().to_string());
            }
        }
        if !current.pop() {
            break;
        }
    }

    None
}

/// Derive the project name from the root directory name.
#[allow(dead_code)]
pub fn project_name(root: &str) -> String {
    std::path::Path::new(root)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root.to_string())
}

/// Detect the project type from which marker file was found at the root.
#[allow(dead_code)]
pub fn project_kind(root: &str) -> Option<&'static str> {
    let root_path = std::path::Path::new(root);
    for (marker, kind) in PROJECT_MARKERS {
        if root_path.join(marker).exists() {
            return Some(kind);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_git_project() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        let sub = tmp.path().join("src").join("deep");
        std::fs::create_dir_all(&sub).unwrap();

        let root = detect_project_root(sub.to_str().unwrap());
        assert_eq!(root.unwrap(), tmp.path().to_str().unwrap());
    }

    #[test]
    fn test_detect_cargo_project() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        let sub = tmp.path().join("src");
        std::fs::create_dir_all(&sub).unwrap();

        let root = detect_project_root(sub.to_str().unwrap());
        assert_eq!(root.unwrap(), tmp.path().to_str().unwrap());
    }

    #[test]
    fn test_detect_node_project() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join("package.json"), "{}").unwrap();

        let root = detect_project_root(tmp.path().to_str().unwrap());
        assert_eq!(root.unwrap(), tmp.path().to_str().unwrap());
    }

    #[test]
    fn test_no_project_in_empty_dir() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("empty");
        std::fs::create_dir_all(&sub).unwrap();

        // detect_project_root walks up, so it may find markers above tmpdir.
        // We just verify it doesn't panic and doesn't return our tmpdir.
        let root = detect_project_root(sub.to_str().unwrap());
        if let Some(r) = root {
            assert_ne!(r, sub.to_str().unwrap());
        }
    }

    #[test]
    fn test_project_name() {
        assert_eq!(project_name("/home/user/my-project"), "my-project");
        assert_eq!(project_name("/home/user/tp"), "tp");
    }

    #[test]
    fn test_project_name_root() {
        // Root path has no file_name component
        let name = project_name("/");
        assert_eq!(name, "/");
    }

    #[test]
    fn test_project_kind_rust() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(project_kind(tmp.path().to_str().unwrap()), Some("rust"));
    }

    #[test]
    fn test_project_kind_node() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join("package.json"), "").unwrap();
        assert_eq!(project_kind(tmp.path().to_str().unwrap()), Some("node"));
    }

    #[test]
    fn test_project_kind_none() {
        let tmp = tempdir().unwrap();
        assert_eq!(project_kind(tmp.path().to_str().unwrap()), None);
    }

    #[test]
    fn test_git_takes_priority() {
        // When both .git and Cargo.toml exist, .git is checked first
        let tmp = tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(project_kind(tmp.path().to_str().unwrap()), Some("git"));
    }
}
