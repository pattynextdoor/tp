# Beta Completion Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement remaining beta CLI commands (`query`, `remove`, `doctor`, `--bootstrap` flag), AI feature stubs (`index`, `analyze`), and set up an mdbook documentation site with GitHub Pages deployment.

**Architecture:** Add new subcommands to `Commands` enum in `src/cli.rs`, implement handlers inline or in existing modules. AI stubs print helpful messages when the feature is disabled and placeholder messages when enabled. mdbook lives in `docs/book/` with content extracted from README.

**Tech Stack:** Rust (clap derive), rusqlite, mdbook, GitHub Actions

---

### Task 1: Add `--bootstrap` flag to `Init` subcommand

The `auto_bootstrap` function already exists in `src/bootstrap.rs` — it seeds from zoxide, shell history, and project discovery. We need a `--bootstrap` flag on `Init` so users can manually re-trigger it.

**Files:**
- Modify: `src/cli.rs:63-70` (Init variant)
- Modify: `src/cli.rs:200-204` (Init handler)
- Modify: `src/bootstrap.rs:18` (add `force_bootstrap` function)
- Test: `src/bootstrap.rs` (add test for force bootstrap)

**Step 1: Write the failing test**

In `src/bootstrap.rs`, add a test that calls `force_bootstrap` — which doesn't exist yet:

```rust
#[test]
fn test_force_bootstrap_always_runs() {
    let conn = db::open_memory().unwrap();
    // Pre-populate so auto_bootstrap would skip
    frecency::record_visit(&conn, "/tmp/existing", None).unwrap();

    // force_bootstrap should still run (returns true)
    let ran = force_bootstrap(&conn).unwrap();
    assert!(ran);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_force_bootstrap_always_runs -- --nocapture`
Expected: FAIL — `force_bootstrap` does not exist.

**Step 3: Implement `force_bootstrap`**

In `src/bootstrap.rs`, add a public function that skips the "is DB empty?" check:

```rust
/// Manually triggered bootstrap — always runs regardless of DB state.
/// Called by `tp init --bootstrap`.
pub fn force_bootstrap(conn: &Connection) -> Result<bool> {
    let mut total = 0u64;
    total += import_from_zoxide(conn).unwrap_or(0);
    total += import_from_shell_history(conn).unwrap_or(0);
    total += discover_projects(conn).unwrap_or(0);

    eprintln!("tp: re-indexed {} directories. Ready.", total);
    Ok(true)
}
```

**Step 4: Add `--bootstrap` flag to CLI**

In `src/cli.rs`, add the flag to `Init`:

```rust
Init {
    shell: String,
    #[arg(long, default_value = "tp")]
    cmd: String,
    /// Re-run bootstrap: import history, zoxide, and discover projects
    #[arg(long)]
    bootstrap: bool,
},
```

And in the handler:

```rust
Commands::Init { shell, cmd, bootstrap } => {
    if *bootstrap {
        let conn = db::open()?;
        crate::bootstrap::force_bootstrap(&conn)?;
        return Ok(());
    }
    let code = shell::generate_init(shell, cmd)?;
    print!("{}", code);
    Ok(())
}
```

**Step 5: Run tests**

Run: `cargo test -- --nocapture`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/cli.rs src/bootstrap.rs
git commit -m "✨ feat(cli): add --bootstrap flag to init subcommand"
```

---

### Task 2: Add `tp query` subcommand

`tp query <terms>` prints matching directories to stdout without navigating — for scripting and piping. It uses the same frecency+fuzzy engine but just prints results.

**Files:**
- Modify: `src/cli.rs:60-111` (Commands enum — add Query variant)
- Modify: `src/cli.rs:198-288` (run() — add Query handler)
- Test: integration-style test in `src/cli.rs` or via `cargo test`

**Step 1: Write the failing test**

Add a test in `src/nav/frecency.rs` (since query_frecency already has tests, this test validates the output format behavior we'll use):

```rust
#[test]
fn test_query_frecency_returns_scored_results() {
    let conn = db::open_memory().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let dir1 = tmp.path().join("alpha-project");
    let dir2 = tmp.path().join("beta-project");
    std::fs::create_dir(&dir1).unwrap();
    std::fs::create_dir(&dir2).unwrap();

    record_visit(&conn, dir1.to_str().unwrap(), None).unwrap();
    record_visit(&conn, dir2.to_str().unwrap(), None).unwrap();

    let results = query_frecency(&conn, "alpha", None).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].score > 0.0);
}
```

**Step 2: Run test to verify it passes (this validates existing infra)**

Run: `cargo test test_query_frecency_returns_scored_results`
Expected: PASS (this confirms the query engine works; the CLI wiring is next).

**Step 3: Add Query subcommand**

In `src/cli.rs`, add to the `Commands` enum:

```rust
/// Print matching directories (for scripting)
Query {
    /// Search terms
    #[arg(required = true)]
    terms: Vec<String>,

    /// Show scores alongside paths
    #[arg(short, long)]
    score: bool,
},
```

And add the handler in `run()`:

```rust
Commands::Query { terms, score } => {
    let conn = db::open()?;
    let joined = terms.join(" ");
    let candidates = frecency::query_frecency(&conn, &joined, None)?;
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

**Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles cleanly.

**Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/cli.rs
git commit -m "✨ feat(cli): add query subcommand for scripting"
```

---

### Task 3: Add `tp remove` subcommand

`tp remove <path>` deletes a directory from the frecency database. Useful for cleaning up paths you don't want tp to suggest.

**Files:**
- Modify: `src/cli.rs` (Commands enum + handler)
- Modify: `src/nav/frecency.rs` (add `remove_path` function)
- Test: `src/nav/frecency.rs`

**Step 1: Write the failing test**

In `src/nav/frecency.rs`:

```rust
#[test]
fn test_remove_path() {
    let conn = db::open_memory().unwrap();
    record_visit(&conn, "/home/user/old-project", None).unwrap();

    let count_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM directories WHERE path = ?1", ["/home/user/old-project"], |row| row.get(0))
        .unwrap();
    assert_eq!(count_before, 1);

    remove_path(&conn, "/home/user/old-project").unwrap();

    let count_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM directories WHERE path = ?1", ["/home/user/old-project"], |row| row.get(0))
        .unwrap();
    assert_eq!(count_after, 0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_remove_path`
Expected: FAIL — `remove_path` does not exist.

**Step 3: Implement `remove_path`**

In `src/nav/frecency.rs`:

```rust
/// Remove a path from the directories table and its session history.
pub fn remove_path(conn: &Connection, path: &str) -> Result<u64> {
    let deleted = conn.execute("DELETE FROM directories WHERE path = ?1", [path])?;
    conn.execute("DELETE FROM sessions WHERE to_path = ?1", [path])?;
    Ok(deleted as u64)
}
```

**Step 4: Wire into CLI**

In `src/cli.rs`, add to `Commands`:

```rust
/// Remove a directory from the database
Remove {
    /// Path to remove
    path: String,
},
```

Handler:

```rust
Commands::Remove { path } => {
    let conn = db::open()?;
    let removed = frecency::remove_path(&conn, path)?;
    if removed > 0 {
        eprintln!("Removed: {}", path);
    } else {
        eprintln!("Not found: {}", path);
    }
    Ok(())
}
```

**Step 5: Run tests**

Run: `cargo test`
Expected: All pass.

**Step 6: Commit**

```bash
git add src/cli.rs src/nav/frecency.rs
git commit -m "✨ feat(cli): add remove subcommand to delete paths from database"
```

---

### Task 4: Add `tp doctor` subcommand

`tp doctor` diagnoses configuration: checks database, shell integration, API key status, feature flags.

**Files:**
- Modify: `src/cli.rs` (Commands enum + handler)

**Step 1: Add Doctor subcommand**

In `src/cli.rs`, add to `Commands`:

```rust
/// Diagnose configuration issues
Doctor,
```

**Step 2: Implement the handler**

```rust
Commands::Doctor => {
    eprintln!("tp doctor");
    eprintln!("=========");
    eprintln!();

    // Database
    match db::db_path() {
        Ok(p) => {
            eprintln!("Database: {}", p.display());
            if p.exists() {
                let conn = db::open()?;
                let dir_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM directories", [], |row| row.get(0)
                )?;
                let wp_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM waypoints", [], |row| row.get(0)
                )?;
                let sess_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sessions", [], |row| row.get(0)
                )?;
                eprintln!("  Directories: {}", dir_count);
                eprintln!("  Waypoints:   {}", wp_count);
                eprintln!("  Sessions:    {}", sess_count);
            } else {
                eprintln!("  (not created yet — navigate once to initialize)");
            }
        }
        Err(e) => eprintln!("Database: ERROR — {}", e),
    }
    eprintln!();

    // Features
    eprintln!("Features:");
    if cfg!(feature = "ai") {
        eprintln!("  AI:  enabled");
    } else {
        eprintln!("  AI:  disabled (rebuild with --features ai)");
    }
    if cfg!(feature = "tui") {
        eprintln!("  TUI: enabled");
    } else {
        eprintln!("  TUI: disabled (rebuild with --features tui)");
    }
    eprintln!();

    // AI key
    eprintln!("AI Configuration:");
    #[cfg(feature = "ai")]
    {
        match crate::ai::detect_api_key() {
            Some((_key, source)) => eprintln!("  API key: found in {}", source),
            None => eprintln!("  API key: not set (run tp --setup-ai)"),
        }
    }
    #[cfg(not(feature = "ai"))]
    {
        eprintln!("  (AI feature not compiled)");
    }
    eprintln!();

    // Shell
    eprintln!("Environment:");
    eprintln!("  SHELL:    {}", std::env::var("SHELL").unwrap_or_else(|_| "(not set)".into()));
    if let Ok(dir) = std::env::var("TP_DATA_DIR") {
        eprintln!("  TP_DATA_DIR: {}", dir);
    }
    if let Ok(exclude) = std::env::var("TP_EXCLUDE_DIRS") {
        eprintln!("  TP_EXCLUDE_DIRS: {}", exclude);
    }

    Ok(())
}
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles cleanly.

**Step 4: Run tests**

Run: `cargo test`
Expected: All pass.

**Step 5: Commit**

```bash
git add src/cli.rs
git commit -m "✨ feat(cli): add doctor subcommand for diagnostics"
```

---

### Task 5: Add `tp index` and `tp analyze` AI stubs

These are promised AI features. For beta, they print helpful messages explaining what they'll do and that they're coming soon.

**Files:**
- Modify: `src/cli.rs` (Commands enum + handlers)

**Step 1: Add both subcommands to `Commands` enum**

```rust
/// AI: build a semantic index of a project (coming soon)
Index {
    /// Path to the project to index (defaults to current directory)
    path: Option<String>,
},

/// AI: extract workflow patterns from navigation history (coming soon)
Analyze,
```

**Step 2: Implement handlers**

```rust
Commands::Index { path } => {
    let target = path.as_deref().unwrap_or(".");
    let abs = std::fs::canonicalize(target)
        .unwrap_or_else(|_| std::path::PathBuf::from(target));
    eprintln!("tp index: semantic project indexing");
    eprintln!("  Target: {}", abs.display());

    #[cfg(feature = "ai")]
    {
        match crate::ai::detect_api_key() {
            Some(_) => {
                eprintln!();
                eprintln!("Semantic indexing is coming in a future release.");
                eprintln!("This will let you search by concept:");
                eprintln!("  tp the service that handles webhook retries");
            }
            None => {
                eprintln!();
                eprintln!("Requires an API key. Run: tp --setup-ai");
            }
        }
    }
    #[cfg(not(feature = "ai"))]
    {
        eprintln!("AI features are not enabled. Rebuild with --features ai");
    }
    Ok(())
}

Commands::Analyze => {
    eprintln!("tp analyze: workflow pattern extraction");

    #[cfg(feature = "ai")]
    {
        match crate::ai::detect_api_key() {
            Some(_) => {
                eprintln!();
                eprintln!("Workflow analysis is coming in a future release.");
                eprintln!("This will identify navigation patterns like:");
                eprintln!("  \"After visiting auth/, you usually go to tests/auth/\"");
            }
            None => {
                eprintln!();
                eprintln!("Requires an API key. Run: tp --setup-ai");
            }
        }
    }
    #[cfg(not(feature = "ai"))]
    {
        eprintln!("AI features are not enabled. Rebuild with --features ai");
    }
    Ok(())
}
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles cleanly.

**Step 4: Run tests**

Run: `cargo test`
Expected: All pass.

**Step 5: Commit**

```bash
git add src/cli.rs
git commit -m "✨ feat(cli): add index and analyze AI stub subcommands"
```

---

### Task 6: Set up mdbook documentation site

Create the mdbook structure with content extracted from README. The book will have chapters for: introduction, installation, usage, configuration, architecture, benchmarks, and AI features.

**Files:**
- Create: `docs/book/book.toml`
- Create: `docs/book/src/SUMMARY.md`
- Create: `docs/book/src/introduction.md`
- Create: `docs/book/src/installation.md`
- Create: `docs/book/src/usage.md`
- Create: `docs/book/src/configuration.md`
- Create: `docs/book/src/ai-features.md`
- Create: `docs/book/src/architecture.md`
- Create: `docs/book/src/benchmarks.md`
- Create: `docs/book/src/contributing.md`

**Step 1: Create `book.toml`**

```toml
[book]
authors = ["pattynextdoor"]
language = "en"
multilingual = false
src = "src"
title = "tp — Teleport Anywhere"
description = "Documentation for tp, an AI-enhanced, project-aware directory navigator"

[build]
build-dir = "../../target/book"

[output.html]
default-theme = "coal"
preferred-dark-theme = "coal"
git-repository-url = "https://github.com/pattynextdoor/tp"
edit-url-template = "https://github.com/pattynextdoor/tp/edit/main/docs/book/src/{path}"
```

**Step 2: Create `SUMMARY.md`**

```markdown
# Summary

[Introduction](./introduction.md)

# User Guide

- [Installation](./installation.md)
- [Usage](./usage.md)
- [Configuration](./configuration.md)
- [AI Features](./ai-features.md)

# Reference

- [Architecture](./architecture.md)
- [Benchmarks](./benchmarks.md)
- [Contributing](./contributing.md)
```

**Step 3: Create content pages**

Extract and restructure content from README into focused chapters. Each chapter should be self-contained with cross-references.

See the individual file contents below — they are extractions from the README reorganized for a docs site.

**Step 4: Build locally**

Run: `cd docs/book && mdbook build && cd ../..`
Expected: Book builds to `target/book/`.

**Step 5: Commit**

```bash
git add docs/book/
git commit -m "📝 docs: set up mdbook documentation site"
```

---

### Task 7: Add GitHub Actions workflow for docs deployment

Create a GitHub Actions workflow that builds the mdbook and deploys to GitHub Pages on push to main.

**Files:**
- Create: `.github/workflows/docs.yml`

**Step 1: Create the workflow**

```yaml
name: Deploy docs

on:
  push:
    branches: [main]
    paths:
      - 'docs/book/**'
      - '.github/workflows/docs.yml'
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install mdbook
        run: |
          mkdir -p $HOME/.local/bin
          curl -sSL https://github.com/rust-lang/mdBook/releases/latest/download/mdbook-v0.4.44-x86_64-unknown-linux-gnu.tar.gz | tar xz -C $HOME/.local/bin
          echo "$HOME/.local/bin" >> $GITHUB_PATH
      - name: Build book
        run: mdbook build docs/book
      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: target/book

  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    runs-on: ubuntu-latest
    needs: build
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

**Step 2: Commit**

```bash
git add .github/workflows/docs.yml
git commit -m "👷 ci: add GitHub Actions workflow for mdbook deployment"
```

---

### Task 8: Final build, test, and push

Run the full test suite, verify everything compiles, and push.

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

**Step 3: Build release**

Run: `cargo build --release`
Expected: Clean build.

**Step 4: Push**

```bash
git push
```
