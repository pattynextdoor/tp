# Architecture

`tp` is built as a lean Rust binary with optional compile-time feature flags for AI and TUI capabilities.

## Overview

- **Core** — Rust, <5MB static binary, <5ms local navigation
- **Database** — SQLite with WAL mode, automatic migrations, index optimization
- **AI & TUI** — compile-time feature flags (`--features ai,tui`), both on by default
- **Local-first** — fully functional offline; AI is the fallback, never the hot path

The architecture diagram is available in the [GitHub repository](https://github.com/pattynextdoor/tp/blob/main/docs/architecture.svg).

## Resolution Pipeline

When you type `tp <query>`, the binary runs through a six-step pipeline (see [Introduction](./introduction.md#how-it-works) for the full diagram):

1. **Exact/relative path** — if the query is a valid path, `cd` directly
2. **Waypoint lookup** — `:` prefix triggers a pin lookup
3. **Project lookup** — `@` prefix triggers a project root lookup
4. **Frecency + fuzzy match** — the local scoring engine handles ~95% of queries
4b. **Typo tolerance** — Damerau-Levenshtein fallback when fuzzy matching returns nothing
5. **AI reranking** — fires only when local scores are ambiguous (~150 tokens, <300ms)
6. **TUI picker** — last resort, the user picks from a ranked list

## Under the Hood: Scoring Algorithm

Every query that reaches step 4 of the pipeline goes through a multi-layered scoring system. Understanding how it works helps explain why `tp` picks the paths it does.

### Frecency

Frecency combines **frequency** (how often you visit) with **recency** (how recently you visited). The formula:

```
frecency = access_count × time_weight(seconds_since_last_visit)
```

Time weights decay in buckets:

| Last visited | Weight |
|--------------|--------|
| < 5 minutes  | 4.0×   |
| < 1 hour     | 2.0×   |
| < 1 day      | 1.0×   |
| < 1 week     | 0.5×   |
| older         | 0.25×  |

When total frecency across all entries exceeds 10,000, scores are recalculated and entries older than 30 days with near-zero scores are pruned automatically.

### Fuzzy Matching

The query is matched against each candidate path in descending tiers:

| Tier | Score | Example (`query` → `path`) |
|------|-------|---------------------------|
| Exact last component | 1.0 | `api` → `/home/user/projects/api` |
| Suffix of last component | 0.9 | `api` → `/home/user/projects/my-api` |
| Substring anywhere | 0.7 | `proj` → `/home/user/projects/api` |
| Multi-word (all tokens present) | 0.6 | `user api` → `/home/user/projects/api` |
| No match | 0.0 | `zzz` → `/home/user/projects/api` |

Matching is case-insensitive throughout.

### Typo Tolerance

When fuzzy matching returns **zero results**, a fallback pass uses [Damerau-Levenshtein distance](https://en.wikipedia.org/wiki/Damerau%E2%80%93Levenshtein_distance) on the last path component. This catches the four most common typo types: insertions, deletions, substitutions, and **transpositions** (e.g. `projetcs` → `projects`).

| Query length | Max allowed edits |
|--------------|-------------------|
| < 5 chars    | *(skipped — too many false positives)* |
| 5–8 chars    | 1 |
| 9+ chars     | 2 |

Typo matches score **0.4** — below all fuzzy tiers, so a real substring match always wins. This is strictly a "last resort before returning nothing" layer.

### Final Score

The final score for each candidate combines all signals:

```
score = frecency × fuzzy_score × proximity_boost
```

- **`proximity_boost`** = 1.5× if the candidate is in the same project as your current working directory, 1.0× otherwise.
- The candidate with the highest score wins. If that score exceeds 0.8, `tp` navigates immediately. Otherwise, the results flow into AI reranking (step 5) or the TUI picker (step 6).

## Design Principles

- **Local-first, always.** Every navigation is instant by default. The network is a luxury, not a dependency.
- **Invisible intelligence.** Frecency, project context, and AI blend seamlessly. No "modes" to think about.
- **Zero-config magic, full-config power.** Works the moment you install it. Everything overridable for the curious.
- **Respect the developer.** No telemetry without consent. No nagging. No forced accounts. The free tier is the real product.

## Database

`tp` uses SQLite with WAL (Write-Ahead Logging) mode for concurrent reads and fast writes. The database stores:

- **Directory entries** — paths with frecency scores and visit metadata
- **Project roots** — detected project boundaries and their markers
- **Waypoints** — user-pinned directory aliases
- **Session data** — navigation history for AI session recall

The database location defaults to `$XDG_DATA_HOME/tp` and can be overridden with `TP_DATA_DIR` (see [Configuration](./configuration.md)).

## Feature Flags

AI and TUI features are controlled by compile-time Cargo feature flags:

```sh
# Build with all features (default)
cargo build --release

# Build without AI
cargo build --release --no-default-features --features tui

# Build without TUI
cargo build --release --no-default-features --features ai

# Minimal build (no AI, no TUI)
cargo build --release --no-default-features
```

This keeps the core binary small and dependency-free for environments where AI or TUI support isn't needed.
