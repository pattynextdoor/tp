//! Integration tests for the `tp` binary.
//!
//! Each test gets a fresh temporary directory as `TP_DATA_DIR`
//! so tests are isolated from each other and the user's real database.

use std::path::PathBuf;
use std::process::Command;

/// Get the path to the compiled `tp` binary.
fn tp_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tp"))
}

/// Run `tp` with the given args and a fresh TP_DATA_DIR.
/// Returns (stdout, stderr, exit_code).
fn run_tp(tmp: &std::path::Path, args: &[&str]) -> (String, String, i32) {
    let output = Command::new(tp_bin())
        .args(args)
        .env("TP_DATA_DIR", tmp)
        // Clear any API key so AI features degrade gracefully
        .env_remove("TP_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("TP_EXCLUDE_DIRS")
        .output()
        .expect("failed to execute tp binary");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Run `tp` with custom env vars set.
fn run_tp_with_env(
    tmp: &std::path::Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> (String, String, i32) {
    let mut cmd = Command::new(tp_bin());
    cmd.args(args)
        .env("TP_DATA_DIR", tmp)
        .env_remove("TP_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("TP_EXCLUDE_DIRS");

    for (key, val) in env {
        cmd.env(key, val);
    }

    let output = cmd.output().expect("failed to execute tp binary");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

// ============================================================
// Happy path tests
// ============================================================

#[test]
fn test_help() {
    let tmp = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_tp(tmp.path(), &["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("tp"));
}

#[test]
fn test_version() {
    let tmp = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_tp(tmp.path(), &["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("tp"));
}

#[test]
fn test_init_zsh() {
    let tmp = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_tp(tmp.path(), &["init", "zsh"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("function"), "init should output shell code");
}

#[test]
fn test_init_bash() {
    let tmp = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_tp(tmp.path(), &["init", "bash"]);
    assert_eq!(code, 0);
    assert!(!stdout.is_empty());
}

#[test]
fn test_init_fish() {
    let tmp = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_tp(tmp.path(), &["init", "fish"]);
    assert_eq!(code, 0);
    assert!(!stdout.is_empty());
}

#[test]
fn test_add_and_query_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("myproject");
    std::fs::create_dir(&dir).unwrap();
    let dir_str = dir.to_str().unwrap();

    // Add the directory
    let (_, _, code) = run_tp(tmp.path(), &["add", dir_str]);
    assert_eq!(code, 0);

    // Query for it
    let (stdout, _, code) = run_tp(tmp.path(), &["query", "myproject"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains(dir_str),
        "query should find the added directory"
    );
}

#[test]
fn test_query_with_score() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("scored-project");
    std::fs::create_dir(&dir).unwrap();
    let dir_str = dir.to_str().unwrap();

    run_tp(tmp.path(), &["add", dir_str]);

    let (stdout, _, code) = run_tp(tmp.path(), &["query", "--score", "scored"]);
    assert_eq!(code, 0);
    assert!(stdout.contains(dir_str));
}

#[test]
fn test_query_typo_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("projects");
    std::fs::create_dir(&dir).unwrap();
    let dir_str = dir.to_str().unwrap();

    // Add enough visits so it has a real frecency score
    run_tp(tmp.path(), &["add", dir_str]);
    run_tp(tmp.path(), &["add", dir_str]);

    // "projetcs" is a transposition of "projects"
    let (stdout, _, code) = run_tp(tmp.path(), &["query", "projetcs"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("projects"),
        "typo query should find 'projects' via fallback"
    );
}

#[test]
fn test_ls_shows_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("listed-dir");
    std::fs::create_dir(&dir).unwrap();
    let dir_str = dir.to_str().unwrap();

    run_tp(tmp.path(), &["add", dir_str]);

    let (_, stderr, code) = run_tp(tmp.path(), &["ls"]);
    assert_eq!(code, 0);
    assert!(
        stderr.contains(dir_str),
        "ls should show the added directory"
    );
}

#[test]
fn test_remove_and_verify() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("to-remove");
    std::fs::create_dir(&dir).unwrap();
    let dir_str = dir.to_str().unwrap();

    run_tp(tmp.path(), &["add", dir_str]);

    // Remove it
    let (_, stderr, code) = run_tp(tmp.path(), &["remove", dir_str]);
    assert_eq!(code, 0);
    assert!(stderr.contains("Removed"));

    // Query should now fail
    let (_, _, code) = run_tp(tmp.path(), &["query", "to-remove"]);
    assert_eq!(code, 1);
}

#[test]
fn test_waypoint_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("pinned");
    std::fs::create_dir(&dir).unwrap();
    let dir_str = dir.to_str().unwrap();

    // Mark
    let (_, _, code) = run_tp(tmp.path(), &["--mark", "mypin", dir_str]);
    assert_eq!(code, 0);

    // List waypoints
    let (_, stderr, code) = run_tp(tmp.path(), &["--waypoints"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("mypin"));

    // Unmark
    let (_, _, code) = run_tp(tmp.path(), &["--unmark", "mypin"]);
    assert_eq!(code, 0);

    // Verify gone
    let (_, stderr, _) = run_tp(tmp.path(), &["--waypoints"]);
    assert!(!stderr.contains("mypin"));
}

#[test]
fn test_doctor() {
    let tmp = tempfile::tempdir().unwrap();

    // Seed the DB first so doctor has something to report
    let dir = tmp.path().join("doctor-test");
    std::fs::create_dir(&dir).unwrap();
    run_tp(tmp.path(), &["add", dir.to_str().unwrap()]);

    let (_, stderr, code) = run_tp(tmp.path(), &["doctor"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("tp doctor"));
    assert!(stderr.contains("Database:"));
    assert!(stderr.contains("Features:"));
    assert!(stderr.contains("AI"));
}

#[test]
fn test_suggest_after_visits() {
    let tmp = tempfile::tempdir().unwrap();

    // Create a deep-enough path structure
    let deep = tmp.path().join("dev").join("myapp").join("handlers");
    std::fs::create_dir_all(&deep).unwrap();
    let deep_str = deep.to_str().unwrap();

    // Record enough visits to qualify for suggestion (MIN_VISITS = 3)
    for _ in 0..5 {
        run_tp(tmp.path(), &["add", deep_str]);
    }

    let (_, stderr, code) = run_tp(tmp.path(), &["suggest"]);
    assert_eq!(code, 0);
    assert!(
        stderr.contains("Suggested waypoints") || stderr.contains("No suggestions"),
        "suggest should output something, got: {}",
        stderr
    );
}

// ============================================================
// Error / edge case tests
// ============================================================

#[test]
fn test_query_nonexistent_exits_1() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, _, code) = run_tp(tmp.path(), &["query", "nonexistent_xyz_abc_123"]);
    assert_eq!(code, 1);
}

#[test]
fn test_remove_nonexistent_prints_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    // Seed the DB so it exists
    run_tp(tmp.path(), &["add", "/tmp"]);

    let (_, stderr, code) = run_tp(tmp.path(), &["remove", "/never/existed/path"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("Not found"));
}

#[test]
fn test_init_invalid_shell() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_tp(tmp.path(), &["init", "invalid_shell_xyz"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("Unsupported")
            || stderr.contains("unsupported")
            || stderr.contains("Error")
            || stderr.contains("error"),
        "should print an error for invalid shell, got: {}",
        stderr
    );
}

#[test]
fn test_import_unsupported_tool() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_tp(tmp.path(), &["import", "--from", "unsupported_tool"]);
    assert_eq!(code, 0); // prints message but doesn't error
    assert!(stderr.contains("not yet supported"));
}

#[test]
fn test_ls_empty_db() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_tp(tmp.path(), &["ls"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("No directories tracked"));
}

#[test]
fn test_exclude_dirs_filtering() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("good-project");
    let bad = tmp.path().join("excluded-dir");
    std::fs::create_dir(&good).unwrap();
    std::fs::create_dir(&bad).unwrap();

    // Add both
    run_tp(tmp.path(), &["add", good.to_str().unwrap()]);
    run_tp(tmp.path(), &["add", bad.to_str().unwrap()]);

    // Query with TP_EXCLUDE_DIRS set — excluded dir should not appear in ls
    let (_, stderr, _) = run_tp_with_env(
        tmp.path(),
        &["ls"],
        &[("TP_EXCLUDE_DIRS", bad.to_str().unwrap())],
    );
    assert!(
        !stderr.contains("excluded-dir"),
        "excluded dir should not appear in ls output, got: {}",
        stderr
    );
    assert!(
        stderr.contains("good-project"),
        "non-excluded dir should still appear, got: {}",
        stderr
    );
}

#[test]
fn test_back_empty_history() {
    let tmp = tempfile::tempdir().unwrap();
    // No visits recorded — back should report empty history
    let (_, stderr, code) = run_tp(tmp.path(), &["back"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("No navigation history"));
}

#[test]
fn test_short_typo_no_false_match() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("src");
    std::fs::create_dir(&dir).unwrap();

    run_tp(tmp.path(), &["add", dir.to_str().unwrap()]);

    // "scr" is 3 chars — below typo tolerance threshold
    // It also won't substring-match "src" since "scr" is not in "src"
    let (_, _, code) = run_tp(tmp.path(), &["query", "scr"]);
    assert_eq!(code, 1, "short typo should not match");
}

#[test]
fn test_sync_stub() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_tp(tmp.path(), &["sync"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("Pro feature"));
}

// ============================================================
// AI feature tests (no API key — graceful degradation)
// ============================================================

#[test]
fn test_recall_no_api_key() {
    let tmp = tempfile::tempdir().unwrap();
    // Seed some session data
    run_tp(tmp.path(), &["add", "/tmp"]);

    let (_, stderr, code) = run_tp(tmp.path(), &["--recall"]);
    assert_eq!(code, 0);
    // Without visits in last 24h to real paths, should say "No navigation history"
    // OR print raw stats fallback. Either way, should not crash.
    assert!(
        stderr.contains("No navigation history") || stderr.contains("Session recall"),
        "recall should handle missing API key gracefully, got: {}",
        stderr
    );
}

#[test]
fn test_index_stub() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_tp(tmp.path(), &["index"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("semantic project indexing"));
    assert!(
        stderr.contains("coming") || stderr.contains("API key"),
        "index should show stub or API key message, got: {}",
        stderr
    );
}

#[test]
fn test_index_with_path() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("my-project");
    std::fs::create_dir(&target).unwrap();

    let (_, stderr, code) = run_tp(tmp.path(), &["index", target.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stderr.contains("Target:"));
}

#[test]
fn test_analyze_stub() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_tp(tmp.path(), &["analyze"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("workflow pattern extraction"));
    assert!(
        stderr.contains("coming") || stderr.contains("API key"),
        "analyze should show stub or API key message, got: {}",
        stderr
    );
}

#[test]
fn test_suggest_no_api_key() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_tp(tmp.path(), &["suggest"]);
    assert_eq!(code, 0);
    // With empty DB, should say "No suggestions"
    assert!(stderr.contains("No suggestions"));
}

#[test]
fn test_suggest_ai_flag_without_key() {
    let tmp = tempfile::tempdir().unwrap();
    // Even with --ai flag, should not crash without API key
    let (_, stderr, code) = run_tp(tmp.path(), &["suggest", "--ai"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("No suggestions"));
}

#[test]
fn test_doctor_no_api_key() {
    let tmp = tempfile::tempdir().unwrap();
    run_tp(tmp.path(), &["add", "/tmp"]);

    let (_, stderr, code) = run_tp(tmp.path(), &["doctor"]);
    assert_eq!(code, 0);
    assert!(
        stderr.contains("not set") || stderr.contains("API key"),
        "doctor should report missing API key, got: {}",
        stderr
    );
}

#[test]
fn test_setup_ai_no_key() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_tp(tmp.path(), &["--setup-ai"]);
    assert_eq!(code, 0);
    assert!(
        stderr.contains("No API key") || stderr.contains("TP_API_KEY"),
        "setup-ai should explain how to set the key, got: {}",
        stderr
    );
}
