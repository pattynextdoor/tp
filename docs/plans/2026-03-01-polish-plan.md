# v1.0 Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Polish tp for v1.0: add typo fallback to `tp query`, implement `TP_EXCLUDE_DIRS` filtering, move `navigate_back` to nav module with tests, and add comprehensive integration tests including AI feature coverage.

**Architecture:** Four independent changes to existing modules, plus a new `tests/integration.rs` file. Each task is self-contained and can be committed independently. Integration tests use `std::process::Command` with `TP_DATA_DIR` env override for isolation — no new dev-dependencies.

**Tech Stack:** Rust (clap derive), rusqlite, shellexpand (already a dep), std::process::Command for integration tests

---

### Task 1: Add typo fallback to `tp query`

The `Query` handler in `src/cli.rs` calls `query_frecency()` directly and exits 1 when empty — skipping typo tolerance entirely. Add a fallback to `query_frecency_typo`.

**Files:**
- Modify: `src/cli.rs:352-367` (Query handler)

**Step 1: Implement the fallback**

In `src/cli.rs`, replace the Query handler:

```rust
Commands::Query { terms, score } => {
    let conn = db::open()?;
    let joined = terms.join(" ");
    let mut candidates = frecency::query_frecency(&conn, &joined, None)?;
    // Typo-tolerant fallback when fuzzy matching finds nothing
    if candidates.is_empty() {
        candidates = frecency::query_frecency_typo(&conn, &joined, None)?;
    }
    if candidates.is_empty() {
        std::process::exit(1);
    }
    for c in &candidates {
        if *score {
            println!("{:>8.1}  {}", c.score, c.path);
        } else {
            println!("{}", c.path);
        }
    }
    Ok(())
}
```

**Step 2: Run tests**

Run: `cargo test --all-features`
Expected: All pass (this is a wiring change, existing tests cover the functions).

**Step 3: Commit**

```bash
git add src/cli.rs
git commit -m "🐛 fix(cli): add typo fallback to tp query command"
```

---

### Task 2: Implement `TP_EXCLUDE_DIRS` filtering

Add two helper functions in `src/nav/frecency.rs` and apply them in `record_visit`, `query_frecency`, `query_frecency_typo`, and `query_all`.

**Files:**
- Modify: `src/nav/frecency.rs` (add helpers + apply in 4 functions)

**Step 1: Write failing tests**

Add to the `tests` module at the bottom of `src/nav/frecency.rs`:

```rust
#[test]
fn test_excluded_prefixes_empty() {
    // When env var is not set, should return empty
    std::env::remove_var("TP_EXCLUDE_DIRS");
    let prefixes = excluded_prefixes();
    assert!(prefixes.is_empty());
}

#[test]
fn test_excluded_prefixes_parses() {
    std::env::set_var("TP_EXCLUDE_DIRS", "/tmp,/var/folders");
    let prefixes = excluded_prefixes();
    assert!(prefixes.contains(&"/tmp".to_string()));
    assert!(prefixes.contains(&"/var/folders".to_string()));
    std::env::remove_var("TP_EXCLUDE_DIRS");
}

#[test]
fn test_is_excluded() {
    let prefixes = vec!["/tmp".to_string(), "/var/folders".to_string()];
    assert!(is_excluded("/tmp/foo/bar", &prefixes));
    assert!(is_excluded("/var/folders/abc", &prefixes));
    assert!(!is_excluded("/home/user/projects", &prefixes));
}

#[test]
fn test_record_visit_excludes() {
    std::env::set_var("TP_EXCLUDE_DIRS", "/excluded");
    let conn = db::open_memory().unwrap();
    record_visit(&conn, "/excluded/something", None).unwrap();

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0, "excluded path should not be recorded");
    std::env::remove_var("TP_EXCLUDE_DIRS");
}

#[test]
fn test_query_frecency_excludes() {
    let conn = db::open_memory().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let good_dir = tmp.path().join("projects");
    let bad_dir = tmp.path().join("excluded");
    std::fs::create_dir(&good_dir).unwrap();
    std::fs::create_dir(&bad_dir).unwrap();

    // Record visits directly (bypass record_visit to avoid the insert filter)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO directories (path, frecency, last_access, access_count)
         VALUES (?1, 10.0, ?2, 5)",
        rusqlite::params![good_dir.to_str().unwrap(), now],
    ).unwrap();
    conn.execute(
        "INSERT INTO directories (path, frecency, last_access, access_count)
         VALUES (?1, 10.0, ?2, 5)",
        rusqlite::params![bad_dir.to_str().unwrap(), now],
    ).unwrap();

    std::env::set_var("TP_EXCLUDE_DIRS", bad_dir.to_str().unwrap());
    let results = query_frecency(&conn, "e", None).unwrap();
    // Only the non-excluded dir should appear
    assert!(
        !results.iter().any(|c| c.path.starts_with(bad_dir.to_str().unwrap())),
        "excluded paths should not appear in results"
    );
    std::env::remove_var("TP_EXCLUDE_DIRS");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --all-features -- excluded`
Expected: FAIL — `excluded_prefixes` and `is_excluded` don't exist.

**Step 3: Implement the helpers**

Add near the top of `src/nav/frecency.rs` (after the imports):

```rust
/// Read `TP_EXCLUDE_DIRS` env var and return expanded path prefixes.
/// The var is comma-separated; `~` is expanded via shellexpand.
fn excluded_prefixes() -> Vec<String> {
    match std::env::var("TP_EXCLUDE_DIRS") {
        Ok(val) if !val.is_empty() => val
            .split(',')
            .map(|s| shellexpand::tilde(s.trim()).to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Check if a path starts with any excluded prefix.
fn is_excluded(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}
```

**Step 4: Apply filtering in `record_visit`**

At the top of `record_visit`, before the transaction:

```rust
let excluded = excluded_prefixes();
if is_excluded(path, &excluded) {
    return Ok(());
}
```

**Step 5: Apply filtering in `query_frecency`**

After the dead-paths pruning block and before the sort, add:

```rust
let excluded = excluded_prefixes();
if !excluded.is_empty() {
    candidates.retain(|c| !is_excluded(&c.path, &excluded));
}
```

**Step 6: Apply filtering in `query_frecency_typo`**

Same as step 5 — after pruning, before sort:

```rust
let excluded = excluded_prefixes();
if !excluded.is_empty() {
    candidates.retain(|c| !is_excluded(&c.path, &excluded));
}
```

**Step 7: Apply filtering in `query_all`**

Same pattern — after pruning, before sort:

```rust
let excluded = excluded_prefixes();
if !excluded.is_empty() {
    candidates.retain(|c| !is_excluded(&c.path, &excluded));
}
```

**Step 8: Run tests**

Run: `cargo test --all-features`
Expected: All pass.

**Step 9: Commit**

```bash
git add src/nav/frecency.rs
git commit -m "✨ feat(nav): implement TP_EXCLUDE_DIRS filtering"
```

---

### Task 3: Move `navigate_back` to nav module + add tests

Move the private `navigate_back` function from `src/cli.rs` to `src/nav/mod.rs` as a public function, then add unit tests.

**Files:**
- Modify: `src/cli.rs:201-237` (remove function, update call site)
- Modify: `src/nav/mod.rs` (add function + tests)

**Step 1: Write failing tests in `src/nav/mod.rs`**

Add these tests to the existing `mod tests` block in `src/nav/mod.rs`:

```rust
#[test]
fn test_navigate_back_empty_history() {
    let conn = db::open_memory().unwrap();
    let result = navigate_back(&conn, 1).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_navigate_back_one_step() {
    let conn = db::open_memory().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let dir_a = tmp.path().join("a");
    let dir_b = tmp.path().join("b");
    std::fs::create_dir(&dir_a).unwrap();
    std::fs::create_dir(&dir_b).unwrap();

    // Simulate navigation: from a → b, then from b → somewhere
    conn.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
        rusqlite::params![dir_a.to_str().unwrap(), dir_b.to_str().unwrap()],
    ).unwrap();
    conn.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
        rusqlite::params![dir_b.to_str().unwrap(), dir_a.to_str().unwrap()],
    ).unwrap();

    let result = navigate_back(&conn, 1).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_navigate_back_two_steps() {
    let conn = db::open_memory().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let dir_a = tmp.path().join("a");
    let dir_b = tmp.path().join("b");
    let dir_c = tmp.path().join("c");
    std::fs::create_dir(&dir_a).unwrap();
    std::fs::create_dir(&dir_b).unwrap();
    std::fs::create_dir(&dir_c).unwrap();

    conn.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
        rusqlite::params![dir_a.to_str().unwrap(), dir_b.to_str().unwrap()],
    ).unwrap();
    conn.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
        rusqlite::params![dir_b.to_str().unwrap(), dir_c.to_str().unwrap()],
    ).unwrap();
    conn.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
        rusqlite::params![dir_c.to_str().unwrap(), dir_a.to_str().unwrap()],
    ).unwrap();

    // back(2) should skip the most recent and return the second
    let result = navigate_back(&conn, 2).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_navigate_back_deduplicates() {
    let conn = db::open_memory().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let dir_a = tmp.path().join("a");
    std::fs::create_dir(&dir_a).unwrap();

    // Same from_path appears multiple times
    for _ in 0..5 {
        conn.execute(
            "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
            rusqlite::params![dir_a.to_str().unwrap(), "/tmp/whatever"],
        ).unwrap();
    }

    // back(2) should return None — only one unique path
    let result = navigate_back(&conn, 2).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_navigate_back_skips_dead_paths() {
    let conn = db::open_memory().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("good");
    std::fs::create_dir(&good).unwrap();

    // Insert a dead path first (most recent), then a good one
    conn.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
        rusqlite::params![good.to_str().unwrap(), "/tmp/whatever"],
    ).unwrap();
    conn.execute(
        "INSERT INTO sessions (from_path, to_path, match_type) VALUES (?1, ?2, 'visit')",
        rusqlite::params!["/nonexistent/dead/path", "/tmp/whatever"],
    ).unwrap();

    let result = navigate_back(&conn, 1).unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), good.to_str().unwrap());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --all-features -- navigate_back`
Expected: FAIL — `navigate_back` not found in `nav` module.

**Step 3: Move function to `src/nav/mod.rs`**

Add this public function in `src/nav/mod.rs` (before the `resolve_project` function):

```rust
/// Navigate back N steps in session history.
/// Looks at the sessions table for recent from_path entries,
/// skipping the current directory, dead paths, and deduplicating.
pub fn navigate_back(conn: &Connection, steps: usize) -> Result<Option<String>> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut stmt = conn.prepare(
        "SELECT DISTINCT from_path FROM sessions
         WHERE from_path IS NOT NULL AND from_path != ''
         ORDER BY timestamp DESC
         LIMIT 100",
    )?;

    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<String> = Vec::new();

    for row in rows {
        let path = row?;
        // Skip current directory and duplicates
        if path == cwd || !seen.insert(path.clone()) {
            continue;
        }
        // Only include paths that still exist
        if std::path::Path::new(&path).exists() {
            stack.push(path);
            if stack.len() >= steps {
                break;
            }
        }
    }

    Ok(stack.into_iter().last())
}
```

**Step 4: Update `src/cli.rs`**

Remove the `navigate_back` function (lines 201-237) from `src/cli.rs`.

Update the Back handler to call the nav module version:

```rust
Commands::Back { steps } => {
    let conn = db::open()?;
    match crate::nav::navigate_back(&conn, *steps)? {
```

**Step 5: Run tests**

Run: `cargo test --all-features`
Expected: All pass.

**Step 6: Commit**

```bash
git add src/cli.rs src/nav/mod.rs
git commit -m "♻️ refactor(nav): move navigate_back to nav module with tests"
```

---

### Task 4: Update docs for `TP_EXCLUDE_DIRS`

Document the variable behavior in the README config table and mdbook configuration page.

**Files:**
- Modify: `README.md` (config table)
- Modify: `docs/book/src/configuration.md` (config table)

**Step 1: Update README config table**

Add a row after `TP_AI_TIMEOUT`:

```
| `TP_EXCLUDE_DIRS` | — | Comma-separated path prefixes to ignore (supports `~`) |
```

**Step 2: Update mdbook config page**

Add the same row to the table in `docs/book/src/configuration.md`.

**Step 3: Commit**

```bash
git add README.md docs/book/src/configuration.md
git commit -m "📝 docs: document TP_EXCLUDE_DIRS in config tables"
```

---

### Task 5: Integration tests — core happy paths

Create `tests/integration.rs` with a helper function and the core happy-path tests.

**Files:**
- Create: `tests/integration.rs`

**Step 1: Create the file with helper + first batch of tests**

```rust
//! Integration tests for the `tp` binary.
//!
//! Each test gets a fresh temporary directory as `TP_DATA_DIR`
//! so tests are isolated from each other and the user's real database.

use std::path::PathBuf;
use std::process::Command;

/// Get the path to the compiled `tp` binary.
fn tp_bin() -> PathBuf {
    // cargo test builds the binary in target/debug
    let mut path = PathBuf::from(env!("CARGO_BIN_EXE_tp"));
    // Fallback for older cargo versions
    if !path.exists() {
        path = PathBuf::from("target/debug/tp");
    }
    path
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
    assert!(stdout.contains("tp-nav"));
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
    // Score output has a number followed by path
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
    // Should show suggestions or "No suggestions" (path might be too shallow)
    assert!(
        stderr.contains("Suggested waypoints") || stderr.contains("No suggestions"),
        "suggest should output something, got: {}",
        stderr
    );
}
```

**Step 2: Run tests**

Run: `cargo test --all-features --test integration`
Expected: All pass.

**Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "✅ test: add integration tests for core happy paths"
```

---

### Task 6: Integration tests — error and edge cases

Add error/edge case tests to `tests/integration.rs`.

**Files:**
- Modify: `tests/integration.rs`

**Step 1: Add error/edge case tests**

Append to `tests/integration.rs`:

```rust
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
        stderr.contains("Unsupported") || stderr.contains("unsupported") || stderr.contains("Error"),
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

    // Query with TP_EXCLUDE_DIRS set — excluded dir should not appear
    let (_, stderr, _) = run_tp_with_env(
        tmp.path(),
        &["ls"],
        &[("TP_EXCLUDE_DIRS", bad.to_str().unwrap())],
    );
    assert!(
        !stderr.contains("excluded-dir"),
        "excluded dir should not appear in ls output"
    );
    assert!(
        stderr.contains("good-project"),
        "non-excluded dir should still appear"
    );
}

#[test]
fn test_back_empty_history() {
    let tmp = tempfile::tempdir().unwrap();
    // Seed DB
    run_tp(tmp.path(), &["add", "/tmp"]);

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
```

**Step 2: Run tests**

Run: `cargo test --all-features --test integration`
Expected: All pass.

**Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "✅ test: add integration tests for error and edge cases"
```

---

### Task 7: Integration tests — AI features (no API key)

Add tests verifying AI features degrade gracefully without an API key.

**Files:**
- Modify: `tests/integration.rs`

**Step 1: Add AI degradation tests**

Append to `tests/integration.rs`:

```rust
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
        "index should show stub or API key message"
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
        "analyze should show stub or API key message"
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
        "doctor should report missing API key"
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
```

**Step 2: Run tests**

Run: `cargo test --all-features --test integration`
Expected: All pass.

**Step 3: Run full CI check**

Run: `cargo fmt --check && cargo clippy --all-features -- -D warnings && cargo test --all-features`
Expected: All clean.

**Step 4: Commit**

```bash
git add tests/integration.rs
git commit -m "✅ test: add integration tests for AI feature graceful degradation"
```
