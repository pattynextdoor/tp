# Introduction

**tp** — Teleport anywhere in your codebase.

Most navigation tools treat your filesystem like a flat list of places you've been. `tp` actually pays attention. It knows which project you're in, remembers your patterns, and — when it's truly stumped — quietly consults an oracle to figure out where you meant to go.

Built in Rust. Works in six shells. Learns from day one. No config required, but every knob is there if you want it.

## Why tp?

Existing tools make you choose: fast-but-dumb or precise-but-manual. `tp` refuses to choose.

| Tool | Strength | Weakness |
|------|----------|----------|
| **zoxide** | Fast, Rust, clean UX | No project awareness, no pinning, no AI, cold start |
| **z.lua** | Context-aware jumps | Slower, Lua dependency, no project scoping |
| **fasd** | Tracks files & dirs | Unmaintained since 2015 |
| **fastTravelCLI** | Manual waypoints | No learning, fully manual |

`tp` is what happens when you give zoxide a sense of place, a memory, and a mild case of precognition.

## Features at a Glance

### Core Navigation

Free, open source, works entirely offline. No accounts, no cloud, no strings.

- **Frecency scoring** — frequency + recency with time-decay weighting
- **Multi-token fuzzy matching** — `tp foo bar` matches paths containing both tokens, with typo tolerance
- **Project awareness** — auto-detection via `.git`, `Cargo.toml`, `package.json`, `go.mod`, and more
- **Project-scoped search** — `tp -p tests` stays inside your project boundaries
- **Cross-project switching** — `tp @payments-service` jumps to a known project root
- **Waypoints** — `tp --mark deploy` pins a directory; `tp !deploy` teleports there instantly
- **Smart cold start** — bootstraps from shell history, git repos, and existing zoxide/z/autojump databases
- **Built-in TUI picker** — interactive fuzzy finder showing project name, last modified, and git branch
- **Full `cd` compatibility** — relative paths, `..`, `-`, `~`, absolute paths all just work
- **6 shells** — bash, zsh, fish, PowerShell, Nushell, Elvish

### AI Features (BYOK)

Bring your own API key. The tool never phones home unless you ask it to. See the [AI Features](./ai-features.md) chapter for full details.

### Pro Tier

For teams. Cloud sync, shared waypoints, onboarding mode, and navigation analytics.

## How It Works

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
 │  2. Waypoint (!)?    │──▶ jump to pin
 └──────────┬──────────┘
            │ no
 ┌──────────▼──────────┐
 │  3. Project (@)?     │──▶ project root
 └──────────┬──────────┘
            │ no
 ┌──────────▼──────────┐
 │  4. Frecency + fuzzy │──▶ score > 0.8 → go  ← 95% of jumps
 └──────────┬──────────┘
            │ too close to call
 ┌──────────▼──────────┐
 │  5. AI reranking     │──▶ ~150 tokens, <300ms
 └──────────┬──────────┘
            │ still unsure
 ┌──────────▼──────────┐
 │  6. TUI picker       │──▶ you choose
 └─────────────────────┘
```

The design principle: **AI is a tiebreaker, not a crutch.** Your navigation should never wait on a network request unless it genuinely doesn't know where you want to go.

## Development Status

`tp` is in **alpha**. Core navigation, frecency scoring, project detection, waypoints, and shell integration are implemented and working. AI features and the TUI picker are stubbed and under active development.

| Phase | Status | Shipping |
|-------|--------|----------|
| **Alpha** | Complete | Core binary: frecency, project detection, waypoints, 6-shell integration, bootstrap, import |
| **Beta** | In Progress | AI reranking (BYOK), TUI picker, session recall, zoxide import |
| **v1.0** | Planned | Polished UX, workflow prediction, session recall, VS Code extension |
| **Pro** | Planned | Cloud sync, team waypoints, onboarding mode, analytics |
