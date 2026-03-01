<p align="center">

<img src="https://media1.tenor.com/m/mX9Rxbo-VvsAAAAd/toji-toji-season-2.gif" width="400" alt="tp — teleport anywhere" />

<br/>

<h3>⚡ Teleport anywhere in your codebase.</h3>

[![Rust](https://img.shields.io/badge/Built_with-Rust-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)
[![Alpha](https://img.shields.io/badge/Status-Alpha-orange?style=flat-square)](#development-status)

</p>

---

Most navigation tools treat your filesystem like a flat list of places you've been. `tp` actually pays attention. It knows which project you're in, remembers your patterns, and — when it's truly stumped — quietly consults an oracle to figure out where you meant to go.

Built in Rust. Works in six shells. Learns from day one. No config required, but every knob is there if you want it.

## Quick Start

```sh
cargo install --path .
eval "$(tp init zsh)"     # or bash, fish, powershell, nushell, elvish
tp init --bootstrap        # import your shell history — useful from minute one
```

Then just use it:

```sh
tp myproject               # jump to best match
tp -p tests                # find tests/ within the current project
tp @payments-service       # switch to a project by name
tp !deploy                 # teleport to a pinned waypoint
tp -i                      # interactive fuzzy picker
```

That's it. No config files, no setup wizards, no accounts.

---

## Why tp?

Existing tools make you choose: fast-but-dumb or precise-but-manual. `tp` refuses to choose.

| Tool | Strength | Weakness |
|------|----------|----------|
| **zoxide** | Fast, Rust, clean UX | No project awareness, no pinning, no AI, cold start |
| **z.lua** | Context-aware jumps | Slower, Lua dependency, no project scoping |
| **fasd** | Tracks files & dirs | Unmaintained since 2015 |
| **fastTravelCLI** | Manual waypoints | No learning, fully manual |

`tp` is what happens when you give zoxide a sense of place, a memory, and a mild case of precognition.

## Features

### Core Navigation

> Free, open source, works entirely offline. No accounts, no cloud, no strings.

- **Frecency scoring** — frequency + recency with time-decay weighting that matches or exceeds zoxide
- **Multi-token fuzzy matching** — `tp foo bar` matches paths containing both tokens, with typo tolerance
- **Project awareness** — auto-detection via `.git`, `Cargo.toml`, `package.json`, `go.mod`, and [12 more markers](#project-markers)
- **Project-scoped search** — `tp -p tests` stays inside your project boundaries
- **Cross-project switching** — `tp @payments-service` jumps to a known project root
- **Waypoints** — `tp --mark deploy` pins a directory; `tp !deploy` teleports there instantly
- **Smart cold start** — bootstraps from shell history, git repos, and existing zoxide/z/autojump databases
- **Built-in TUI picker** — interactive fuzzy finder showing project name, last modified, and git branch
- **Full `cd` compatibility** — relative paths, `..`, `-`, `~`, absolute paths all just work
- **6 shells** — bash, zsh, fish, PowerShell, Nushell, Elvish

### AI Features (BYOK)

> Bring your own API key. The tool never phones home unless you ask it to.

- **Natural language nav** — `tp the auth service terraform module` resolves even when none of those words appear in the path
- **Intent-aware disambiguation** — when scores are tied, AI considers your cwd, recent jumps, and git branch to break the tie
- **Workflow prediction** — spots recurring navigation sequences and nudges you toward the next destination
- **Smart aliasing** — suggests memorable waypoint names based on project structure (you confirm; never auto-applied)
- **Session recall** — `tp --recall` answers the Monday morning question: *"where was I?"*
- **Semantic project indexing** — search across projects by concept: `tp the service that handles webhook retries`

### Pro Tier

> For teams. $5–8/seat/month.

- **Cross-machine sync** — frecency database, waypoints, and project index via E2E encrypted cloud storage
- **Team shared waypoints** — canonical navigation shortcuts for the entire org
- **Onboarding mode** — new engineers inherit the team's navigation index on day one
- **Navigation analytics** — personal and team dashboards for usage patterns

---

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

---

## Installation

### From source

```sh
cargo install --path .
```

### Shell setup

One line in your shell config:

```sh
# bash (~/.bashrc)
eval "$(tp init bash)"

# zsh (~/.zshrc)
eval "$(tp init zsh)"

# fish (~/.config/fish/config.fish)
tp init fish | source

# PowerShell ($PROFILE)
Invoke-Expression (& { tp init powershell } | Out-String)

# Nushell (~/.config/nushell/env.nu)
tp init nushell | save -f ~/.cache/tp/init.nu; source ~/.cache/tp/init.nu

# Elvish (~/.config/elvish/rc.elv)
eval (tp init elvish | slurp)
```

Want a different command name? Use `--cmd`:

```sh
eval "$(tp init bash --cmd j)"
```

### Bootstrap

Don't start from zero. Pull in your existing navigation history:

```sh
tp init --bootstrap
```

Already a zoxide user? Bring your muscle memory with you:

```sh
tp import --from=zoxide ~/.local/share/zoxide/db.zo
```

## Usage Reference

```
tp <query>              Navigate to best match
tp -i [query]           Interactive picker
tp -p <query>           Search within current project
tp @<project>           Jump to project root by name
tp !<waypoint>          Jump to pinned waypoint

tp --mark <name> [path] Pin a directory
tp --unmark <name>      Remove a pin
tp --waypoints          List all waypoints

tp add <path>           Manually add a directory
tp remove <path>        Remove from database
tp query <query>        Print matches (for scripting)

tp init <shell>         Shell integration code
tp init --bootstrap     Bootstrap from history
tp import --from=<tool> Import from zoxide/z/autojump/fasd

tp index [path]         AI: semantic index a project
tp analyze              AI: extract workflow patterns
tp --recall             AI: "where was I?" session digest
tp --setup-ai           Configure API key for AI features

tp sync                 Force cloud sync (Pro)
tp doctor               Diagnose configuration issues
```

## Configuration

All via environment variables. Sane defaults, every knob exposed.

| Variable | Default | Description |
|----------|---------|-------------|
| `TP_DATA_DIR` | `$XDG_DATA_HOME/tp` | Database and config location |
| `TP_MAXAGE` | `10000` | Max total frecency score before aging |
| `TP_EXCLUDE_DIRS` | `$HOME` | Glob patterns to exclude |
| `TP_PROJECT_MARKERS` | `.git,package.json,...` | Project root markers |
| `TP_RESOLVE_SYMLINKS` | `0` | Resolve symlinks before storing |
| `TP_FZF_OPTS` | — | Custom options for interactive mode |
| `TP_API_KEY` | — | Anthropic API key for AI features |
| `TP_AI_MODEL` | `claude-haiku-4-5-20251001` | AI model override |
| `TP_AI_ENABLED` | `true` (if key set) | Toggle AI features |
| `TP_AI_TIMEOUT` | `2000` | AI request timeout (ms) |
| `TP_ECHO` | `0` | Print matched dir before navigating |

## Project Markers

`tp` walks up the directory tree looking for these files to detect project boundaries:

`.git` `.hg` `.svn` `Cargo.toml` `package.json` `go.mod` `pyproject.toml` `build.gradle` `pom.xml` `Makefile` `CMakeLists.txt` `.project-root` `mix.exs` `deno.json` `flake.nix`

Drop a `.project-root` in any directory to force project root detection, or extend the list via `TP_PROJECT_MARKERS`.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                    CLI (clap)                    │
├──────────┬──────────┬───────────┬───────────────┤
│ Frecency │ Project  │ Waypoints │ Fuzzy Match   │
│  Engine  │ Detect   │           │               │
├──────────┴──────────┴───────────┴───────────────┤
│              SQLite (rusqlite, WAL)              │
├──────────────────┬──────────────────────────────┤
│   AI Layer       │     TUI Picker              │
│ (feature-gated)  │   (feature-gated)           │
├──────────────────┴──────────────────────────────┤
│          Shell Integration (6 shells)           │
└─────────────────────────────────────────────────┘
```

- **Core** — Rust, <5MB static binary, <5ms local navigation
- **Database** — SQLite with WAL mode, automatic migrations, index optimization
- **AI & TUI** — compile-time feature flags (`--features ai,tui`), both on by default
- **Local-first** — fully functional offline. AI is the fallback, never the hot path.

## Design Principles

- **Local-first, always.** Every navigation is instant by default. The network is a luxury, not a dependency.
- **Invisible intelligence.** Frecency, project context, and AI blend seamlessly. No "modes" to think about.
- **Zero-config magic, full-config power.** Works the moment you install it. Everything overridable for the curious.
- **Respect the developer.** No telemetry without consent. No nagging. No forced accounts. The free tier is the real product.

## Development Status

`tp` is in **alpha**. Core navigation, frecency scoring, project detection, waypoints, and shell integration are implemented and working. AI features and the TUI picker are stubbed and under active development.

| Phase | Status | Shipping |
|-------|--------|----------|
| **Alpha** | **In Progress** | Core binary: frecency, project detection, waypoints, 6-shell integration, bootstrap, import |
| **Beta** | Planned | AI integration (BYOK): NL nav, semantic reranking, disambiguation. Neovim plugin. Tab completion. |
| **v1.0** | Planned | Polished UX, workflow prediction, session recall, VS Code extension. |
| **Pro** | Planned | Cloud sync, team waypoints, onboarding mode, analytics. |

## License

[MIT](LICENSE)
