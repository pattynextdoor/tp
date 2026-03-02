# Design

Technical decisions and why they were made.

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
