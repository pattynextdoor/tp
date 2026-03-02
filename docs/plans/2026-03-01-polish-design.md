# v1.0 Polish Design

## Summary

Four polish items to tighten up `tp` before v1.0, plus adding integration
tests to CI.

---

## 1. `tp query` typo fallback

**Problem:** `Commands::Query` calls `query_frecency()` directly. When it
returns empty, the command exits 1 — skipping typo tolerance.

**Fix:** After `query_frecency` returns empty, try `query_frecency_typo`
before giving up. ~5 lines in the Query handler in `src/cli.rs`.

---

## 2. `TP_EXCLUDE_DIRS`

**Problem:** `tp doctor` prints the variable if set, but nothing actually
reads it. Excluded directories still get recorded and returned.

**Design:** Rust-level filtering with two helpers in `src/nav/frecency.rs`:

```rust
fn excluded_prefixes() -> Vec<String>  // reads env, expands ~
fn is_excluded(path: &str, prefixes: &[String]) -> bool  // starts_with check
```

Applied in four places:
- `record_visit` — skip insert if excluded
- `query_frecency` — filter results post-query
- `query_frecency_typo` — same
- `query_all` — same (feeds TUI picker + `tp ls`)

Docs: update configuration table in architecture.md and README.

---

## 3. `tp back` tests

**Refactor:** Move `navigate_back()` from `src/cli.rs` to `src/nav/mod.rs`
as a public function. It's navigation logic, not CLI wiring.

**Tests (~5):**
- Happy path: `back(1)` and `back(2)` return correct entries
- Empty history → `None`
- Deduplication: repeated paths counted once
- Skips current directory
- Dead paths skipped

---

## 4. Integration tests

**Location:** `tests/integration.rs`

**Approach:** `std::process::Command` running the compiled binary with
`TP_DATA_DIR` set to a fresh tempdir per test. No extra dev-dependencies.

**Happy path tests (~12):**
- `tp --help` / `tp --version`
- `tp init zsh` outputs shell code
- `tp init --bootstrap` seeds DB
- `tp add` + `tp query` round-trip
- `tp query --score` includes scores
- `tp query <typo>` finds via fallback
- `tp ls` shows entries
- `tp remove` + verify gone
- `tp --mark` + `tp --waypoints` + `tp --unmark`
- `tp doctor` exits 0
- `tp suggest` shows suggestions after enough visits

**Error/edge case tests (~8-10):**
- `tp query nonexistent` exits 1
- `tp remove /never/existed` prints "Not found"
- `tp init invalid_shell` fails
- `tp import --from unsupported` prints error
- Empty DB: `tp ls` prints "No directories tracked"
- `TP_EXCLUDE_DIRS` filtering works end-to-end
- `tp back` empty history
- Short typo query (3 chars) doesn't false-match

**AI feature tests (no API key, testing graceful degradation):**
- `tp --recall` without API key — prints helpful message, doesn't crash
- `tp --setup-ai` without TTY — handles non-interactive gracefully
- `tp index` — prints "coming soon" stub
- `tp index <path>` — prints target path + stub
- `tp analyze` — prints "coming soon" stub
- `tp suggest` — shows deterministic suggestions after enough visits
- `tp suggest --ai` without API key — falls back to deterministic names
- `tp doctor` with no API key — prints "API key: not set"

**CI:** `cargo test --all-features` auto-discovers `tests/integration.rs`.
No CI workflow changes needed.

---

## Files Changed

- `src/cli.rs` — query typo fallback, move `navigate_back` out
- `src/nav/frecency.rs` — `excluded_prefixes()`, `is_excluded()`, apply in 4 sites
- `src/nav/mod.rs` — receive `navigate_back()`, add back tests
- `tests/integration.rs` — new file, ~28 tests
- `README.md` — mention `TP_EXCLUDE_DIRS` behavior in config table
- `docs/book/src/configuration.md` — document `TP_EXCLUDE_DIRS`
