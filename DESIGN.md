# Design

Technical decisions, architecture, and benchmarks.

## Query Resolution

Six steps from query to destination. Most trips end at step four.

```
 Query
   │
   ▼
 ┌─────────────────────┐
 │  1. Exact/relative?  │──▶ cd directly
 └──────────┬──────────┘
            │ no
 ┌──────────▼──────────┐
 │  2. Waypoint (:)?    │──▶ jump to pin
 └──────────┬──────────┘
            │ no
 ┌──────────▼──────────┐
 │  3. Project (@)?     │──▶ project root
 └──────────┬──────────┘
            │ no
 ┌──────────▼──────────┐
 │  4. Frecency + fuzzy │──▶ score > 0.8 → go  ← 95% of jumps
 └──────────┬──────────┘
            │ close call?
 ┌──────────▼──────────┐
 │ 4b. Typo tolerance   │──▶ Damerau-Levenshtein fallback
 └──────────┬──────────┘
            │ still too close
 ┌──────────▼──────────┐
 │  5. BYOK reranking   │──▶ ~150 tokens, <300ms
 └──────────┬──────────┘
            │ ¯\_(ツ)_/¯
 ┌──────────▼──────────┐
 │  6. TUI picker       │──▶ you choose
 └─────────────────────┘
```

AI is a tiebreaker, not a crutch. Navigation should never wait on a network request unless it genuinely doesn't know where you want to go.

## Architecture

<p align="center">
<img src="docs/architecture.svg" alt="tp architecture diagram" width="574" />
</p>

- **Core** — Rust, <5MB binary, <5ms navigation
- **Database** — SQLite with WAL mode, auto-migrations, indexed
- **TUI & reranking** — compile-time feature flags (`--features ai,tui`), both on by default
- **Local-first** — fully functional offline. Network features are opt-in fallbacks, never the hot path.

## Project Markers

tp walks up the directory tree looking for these files to detect project boundaries:

`.git` `Cargo.toml` `package.json` `go.mod` `pyproject.toml` `setup.py` `Gemfile` `pom.xml` `build.gradle` `CMakeLists.txt` `Makefile` `.project` `composer.json` `mix.exs` `deno.json` `flake.nix`

---

## Technical Decisions

## Frecency: bucketed time decay

tp scores directories using discrete time buckets rather than continuous
exponential decay:

| Recency         | Weight |
|-----------------|--------|
| < 5 minutes     | 4.0    |
| < 1 hour        | 2.0    |
| < 1 day         | 1.0    |
| < 1 week        | 0.5    |
| older            | 0.25   |

Final score = `access_count × weight`.

Buckets are easier to reason about than smooth decay curves. When two
results compete, you can explain *why* one won: "you visited it today
(1.0) vs last week (0.5), and you've been there more often." Continuous
decay is mathematically smoother but makes debugging ranking feel like
guesswork.

The buckets also create natural tiers — a directory from 3 minutes ago
strongly beats one from 2 hours ago, but two directories from last week
compete mostly on visit count. This matches how people think about
recency: "just now," "earlier today," "sometime this week," "a while ago."

## Storage: SQLite with WAL

zoxide uses a custom binary format — fast for a flat list of
path/score pairs. tp uses SQLite because the data is relational:
directories belong to projects, waypoints reference paths, sessions
track transitions between directories. Modeling this in a flat file
means reinventing indexes, transactions, and concurrent access.

WAL (Write-Ahead Logging) mode lets the shell hook write a visit record
in the background while a query reads the database in the foreground,
without either blocking. This matters because the hook fires on every
prompt — if it contended with queries, the shell would stutter.

The overhead is real (~1-3ms per operation vs sub-millisecond for a
flat file), but tp's query benchmarks still come in under 5ms. The
flexibility to add new tables — sessions, projects, waypoints — without
migrating a file format has already paid for itself.

## Path validation: always, not lazily

Every query checks whether each candidate directory still exists on
disk. Dead paths are silently pruned from the database.

This costs ~0.1ms per candidate on SSDs (one `stat()` syscall each).
For a typical query returning 20-50 candidates, that's 2-5ms of I/O.
Worth it. The alternative is occasionally teleporting to a deleted
directory and getting an error, which trains you not to trust the tool.

Most navigation tools either ignore stale entries or provide a manual
cleanup command. tp self-heals because you shouldn't have to maintain
your navigator's database — it should maintain itself.

## Feature flags: compile-time, not runtime

AI reranking and the TUI picker are behind Cargo feature flags. A
minimal build (`--no-default-features`) produces a ~2MB binary with
no network dependencies. This keeps the core tool lightweight for
environments where you just want a fast `cd` and nothing else.

## Shell hooks: fire and forget

`tp init` injects a hook that runs `tp add $PWD` on every directory
change. This has to be invisible — any perceptible delay on the prompt
is unacceptable. The hook runs in the background (`&>/dev/null &`) and
each shell requires a different mechanism: `PROMPT_COMMAND` for bash,
`precmd_functions` for zsh, `--on-variable PWD` for fish. Getting
background execution right across shells was the fiddliest part of
the project.

## Project detection: walk up, don't cache

On every `tp add`, tp walks up from the current directory checking 16
marker files (`.git`, `Cargo.toml`, `package.json`, etc.), capped at
20 levels. The result is stored alongside the directory entry.

A cache would skip the walk but introduces invalidation problems —
`git init` in a new folder, or deleting a `Cargo.toml`, would make
the cache lie. A file watcher (inotify/FSEvents) would be correct and
fast but adds a persistent daemon and platform-specific complexity.
The walk costs ~1-2ms, only runs during `add` (which is backgrounded),
and is always correct. Good tradeoff.

## Benchmarks: why tp is faster on queries

zoxide and tp take fundamentally different approaches to storage and
retrieval. Understanding the architecture explains the benchmark results.

### zoxide's approach

zoxide stores its database as a bincode-serialized flat file. On every
operation — query or add — the entire file is deserialized into a
`Vec<Dir>` in memory. Each entry has three fields: `path`, `rank`,
`last_accessed`.

**Query:** Sort the entire Vec by score (`sort_unstable_by` on all
entries), then linear scan with keyword matching. Every query touches
every entry.

**Add:** Linear scan to find the existing entry (`iter_mut().find()`),
update in place or push. Then serialize and rewrite the entire file.

This is simple and correct. For small databases (<100 entries), the
overhead of sorting and scanning a Vec is negligible. The flat file
also means zero dependency on SQLite or any external library.

### tp's approach

tp uses SQLite with a B-tree index on the `frecency` column. Queries
use `WHERE path LIKE ? ORDER BY frecency DESC LIMIT 100` — the
database engine reads only matching rows, already in ranked order,
and stops after 100.

Adds are a single `INSERT ... ON CONFLICT UPDATE` — a B-tree insertion
plus a WAL append. No full-file rewrite.

### Where each wins

| Operation | zoxide | tp | Why |
|-----------|--------|-----|-----|
| Query (500 entries) | 6.7ms | 2.4ms | zoxide sorts all 500 entries; tp's index makes it sublinear |
| Query (5,000 entries) | 7.4ms | 2.9ms | Gap holds because B-tree scales O(log n), Vec sort is O(n log n) |
| Add (small DB) | Can be faster | Slightly more overhead | tp does project root detection + session logging |
| Add (large DB) | Slows down | Stays constant | zoxide rewrites the entire file; tp appends to WAL |

The crossover point depends on hardware. On fast NVMe (M4 Pro), tp's
SQLite WAL writes are cheap and the query index advantage dominates.
On slower disk I/O (cloud VPS), zoxide's simpler file operations can
win on add at small scale.

### Scoring: nearly identical

Both tools use the same time-bucketed scoring approach:

| Recency | zoxide | tp |
|---------|--------|-----|
| < 5 minutes | — | 4.0 |
| < 1 hour | 4.0 | 2.0 |
| < 1 day | 2.0 | 1.0 |
| < 1 week | 0.5 | 0.5 |
| older | 0.25 | 0.25 |

tp adds a <5 minute bucket (4.0) that zoxide lacks, and splits zoxide's
<1 hour (4.0) into two tiers. This gives tp finer-grained recency
discrimination for rapid context switching — if you were just in a
directory seconds ago, it should strongly outrank one from 45 minutes
ago.

### Path validation: eager vs lazy

zoxide also checks `Path::exists()` during queries, but uses a TTL
strategy — non-existent entries are only removed if they haven't been
accessed in 3 months. Recent stale entries are skipped but kept in the
database, on the assumption you might recreate the directory.

tp prunes immediately. This means more `stat()` syscalls per query
(~0.1ms each on SSD), but the database never accumulates stale entries.
The tradeoff is correctness vs I/O cost — tp chose correctness.

## Frecency aging: the 10,000 ceiling

When total frecency across all entries exceeds 10,000, tp recalculates
every score using current time weights and prunes entries below 0.1
that haven't been accessed in 30 days.

Without a ceiling, a directory visited 1,000 times last year would
permanently outrank one visited 10 times today. The ceiling forces
periodic normalization so scores reflect current relevance. 10,000
triggers aging roughly every couple months for a heavy user (~50
cd's/day). The 30-day grace period on pruning prevents deleting
seasonal paths — like a tax directory you only touch in April.

## Benchmark Charts

Measured with [hyperfine](https://github.com/sharkdp/hyperfine) on a MacBook Pro M4 Pro, 200+ runs each.

### Core queries (500 entries, flat seeding)

Raw query speed with 1 visit per path.

<p align="center">
<img src="bench/charts/core.svg" alt="Core benchmarks" width="600" />
</p>

### Realistic visit patterns (hot/warm/cold)

300 directories with varied visit counts: 50 "hot" paths (20 visits), 100 "warm" (5 visits), 150 "cold" (1 visit).

<p align="center">
<img src="bench/charts/varied.svg" alt="Varied visit pattern benchmarks" width="600" />
</p>

### Stale path handling

200 directories, 40% deleted after seeding. tp validates paths on every query and self-heals — extra I/O, but your results are always clean.

<p align="center">
<img src="bench/charts/stale.svg" alt="Stale path benchmarks" width="600" />
</p>

### Scale (5,000 entries)

<p align="center">
<img src="bench/charts/scale.svg" alt="Scale benchmarks" width="600" />
</p>

> tp's `add` does more work — it detects project roots by walking up the tree and logs session data. That's the cost of project-scoped search and session recall.

Run them yourself:

```sh
cargo build --release
./bench/bench.sh
python3 bench/chart.py   # generate SVG charts
```
