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
2. **Waypoint lookup** — `!` prefix triggers a pin lookup
3. **Project lookup** — `@` prefix triggers a project root lookup
4. **Frecency + fuzzy match** — the local scoring engine handles ~95% of queries
5. **AI reranking** — fires only when local scores are ambiguous (~150 tokens, <300ms)
6. **TUI picker** — last resort, the user picks from a ranked list

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
