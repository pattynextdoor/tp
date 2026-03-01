# tp Project Rules

## Pre-Push CI Checks

Before committing or pushing Rust code, **always** run these checks — they mirror CI exactly:

```sh
cargo fmt --check        # formatting (fix with: cargo fmt)
cargo clippy --all-features -- -D warnings  # lints
cargo test --all-features                   # tests
```

If `cargo fmt --check` fails, run `cargo fmt` to fix, then re-stage.

When dispatching subagents to write Rust code, always instruct them to run all three checks before committing.

## Git Hygiene

- **Never use `git add -A` or `git add .`** — always add specific files to avoid committing `.DS_Store`, `node_modules/`, or other junk.
- No `Co-Authored-By` lines in commit messages.
- Use gitmoji conventional commits (see global CLAUDE.md for the table).

## Keep Docs in Sync

When adding, changing, or removing a feature, **always update the relevant documentation in the same commit or PR**:

- **`README.md`** — usage examples, feature lists, command reference, config table, development status
- **`docs/book/src/`** — the corresponding mdbook page (usage.md, configuration.md, ai-features.md, etc.)
- **CLI help text** — clap doc comments on structs/variants in `src/cli.rs`

If a new subcommand, flag, or config variable is added, it must appear in all three places. If a feature is removed or renamed, remove/update all references. Don't leave docs promising things the code doesn't do.

## Project Structure

- Rust CLI tool using clap (derive mode) + rusqlite (SQLite with WAL)
- Feature flags: `ai` (reqwest) and `tui` (ratatui/crossterm), both on by default
- AI code is guarded with `#[cfg(feature = "ai")]` with a no-op fallback
- TUI code is guarded with `#[cfg(feature = "tui")]`
- Benchmarks: `bench/bench.sh` (hyperfine), `bench/chart.py` (matplotlib SVGs)
- Docs: `docs/book/` (mdbook), deployed via `.github/workflows/docs.yml`
