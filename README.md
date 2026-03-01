# tp

**Teleport anywhere in your codebase.**

Most navigation tools treat your filesystem like a flat list of places you've been. `tp` actually pays attention. It knows which project you're in, remembers your patterns, and — when it's truly stumped — quietly consults an oracle to figure out where you meant to go.

Built in Rust. Works in six shells. Learns from day one. No config required, but every knob is there if you want it.

## Why tp?

Existing tools make you choose: fast-but-dumb (zoxide, z) or precise-but-manual (fastTravelCLI). `tp` refuses to choose.

| Tool | What it does well | Where it falls short |
|------|-------------------|----------------------|
| **zoxide** | Fast (Rust), cross-shell, clean UX | No project awareness, no pinning, no AI, cold start |
| **z.lua** | Context-aware relative jumps | Slower, Lua dependency, no project awareness |
| **fasd** | Tracks files and directories | Unmaintained, complex setup |
| **fastTravelCLI** | Manual waypoints | No learning, fully manual |

`tp` is what happens when you give zoxide a sense of place, a memory, and a mild case of precognition.

## Features

### Core Navigation

*Free, open source, and works entirely offline. No accounts, no cloud, no strings.*

- **Frecency-based ranking** — frequency + recency scoring with time-decay weighting that matches or exceeds zoxide's algorithm
- **Multi-token fuzzy matching** — `tp foo bar` matches directories containing both tokens in the path, with built-in typo tolerance
- **Project awareness** — automatic detection via `.git`, `Cargo.toml`, `package.json`, `go.mod`, `pyproject.toml`, and [12 more markers](#project-markers)
- **Project-scoped search** — `tp -p tests` finds `tests/` within the current project, not the one three repos over
- **Cross-project switching** — `tp @payments-service` jumps to a known project root by name
- **Waypoints** — `tp --mark deploy` pins a directory; `tp !deploy` teleports you there instantly
- **Smart cold start** — bootstraps from shell history, git repos, and existing zoxide/z/autojump databases so it's useful from minute one
- **Built-in TUI picker** — interactive fuzzy finder via `tp -i`, showing project name, last modified time, and git branch
- **Full cd compatibility** — relative paths, `..`, `-`, `~`, absolute paths all just work
- **6-shell support** — bash, zsh, fish, PowerShell, Nushell, Elvish

### AI Features (BYOK)

*Bring your own API key. No credits, no metering, no accounts. The tool never phones home unless you ask it to.*

- **Natural language navigation** — `tp the auth service terraform module` resolves correctly even when none of those words appear in the path
- **Intent-aware disambiguation** — when the top scores are neck and neck, AI considers your cwd, recent jumps, and git branch to break the tie
- **Workflow prediction** — spots recurring navigation sequences and nudges you toward the likely next destination
- **Smart aliasing** — scans project structures and suggests memorable waypoint names (you confirm; it never auto-applies)
- **Session recall** — `tp --recall` answers the Monday morning question: "where was I?"
- **Semantic project indexing** — search across projects by concept: `tp the service that handles webhook retries`

### Pro Tier

*For teams. $5-8/seat/month.*

- **Cross-machine sync** — frecency database, waypoints, and project index synced via end-to-end encrypted cloud storage
- **Team shared waypoints** — canonical navigation shortcuts for the entire org
- **Onboarding mode** — new engineers inherit the team's full navigation index on day one, not day thirty
- **Navigation analytics** — personal and team dashboards showing usage patterns and onboarding friction points

## Installation

### From source

```sh
cargo install --path .
```

### Shell setup

Add one line to your shell config and you're done.

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

Want a different incantation? Use `--cmd` to pick your own command name:

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

## Usage

```sh
tp <query>              # Navigate to best match
tp -i [query]           # Interactive picker mode
tp -p <query>           # Search within current project only
tp @<project>           # Jump to a project root by name
tp !<waypoint>          # Jump to a named waypoint

tp --mark <name> [path] # Pin directory with a name
tp --unmark <name>      # Remove a pin
tp --waypoints          # List all waypoints

tp add <path>           # Manually add a directory
tp remove <path>        # Remove a directory from database
tp query <query>        # Print matches without navigating (for scripting)

tp init <shell>         # Generate shell integration code
tp init --bootstrap     # Bootstrap from history/filesystem
tp import --from=<tool> # Import from zoxide/z/autojump/fasd

tp index [path]         # AI: semantic index a project
tp analyze              # AI: extract workflow patterns
tp --recall             # AI: "where was I?" session digest
tp --setup-ai           # Configure API key for AI features

tp sync                 # Force sync to cloud (Pro)
tp doctor               # Diagnose configuration issues
```

## How Navigation Works

*Six steps from query to destination. Most trips end at step four.*

`tp` resolves queries through a priority cascade, bailing out the moment it's confident:

1. **Exact/relative path** — `.`, `..`, `-`, `~`, `/path` are treated as literal `cd`. No magic needed.
2. **Waypoint lookup** — queries starting with `!` match against your pinned waypoints.
3. **Project-scoped** — `@project` or `--project` scopes the search to project boundaries.
4. **Local frecency + fuzzy** — scores every database entry against your query. If the top result scores above 0.8, it navigates immediately. This handles the vast majority of jumps in under 5ms.
5. **AI reranking** — if the top candidates are too close to call and a BYOK key is configured, the top 10 are sent to Claude Haiku for semantic reranking (~150 tokens, <300ms).
6. **Interactive picker** — if all else fails, the TUI fuzzy finder opens with the ranked candidates.

The design principle: AI is a tiebreaker, not a crutch. Your navigation should never wait on a network request unless it genuinely doesn't know where you want to go.

## Configuration

All configuration is via environment variables. Sane defaults, every knob exposed.

| Variable | Default | Description |
|----------|---------|-------------|
| `TP_DATA_DIR` | `$XDG_DATA_HOME/tp` | Database and config storage location |
| `TP_MAXAGE` | `10000` | Maximum total frecency score before aging |
| `TP_EXCLUDE_DIRS` | `$HOME` | Glob patterns of directories to exclude |
| `TP_PROJECT_MARKERS` | `.git,package.json,...` | Comma-separated project root markers |
| `TP_RESOLVE_SYMLINKS` | `0` | Resolve symlinks before adding to database |
| `TP_FZF_OPTS` | — | Custom fzf options for interactive selection |
| `TP_API_KEY` | — | Anthropic API key for AI features |
| `TP_AI_MODEL` | `claude-haiku-4-5-20251001` | AI model override |
| `TP_AI_ENABLED` | `true` (if key set) | Toggle AI features |
| `TP_AI_TIMEOUT` | `2000` | AI request timeout in milliseconds |
| `TP_ECHO` | `0` | Print matched directory before navigating |

## Project Markers

`tp` walks up the directory tree looking for these files to detect project boundaries:

`.git` `.hg` `.svn` `Cargo.toml` `package.json` `go.mod` `pyproject.toml` `build.gradle` `pom.xml` `Makefile` `CMakeLists.txt` `.project-root` `mix.exs` `deno.json` `flake.nix`

Drop a `.project-root` file in any directory to make `tp` treat it as a project root, or extend the list via `TP_PROJECT_MARKERS`.

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

- **Core engine** — Rust, <5MB static binary, <5ms local navigation
- **Database** — SQLite with WAL mode, automatic migrations, and index optimization
- **AI & TUI** — compile-time feature flags (`--features ai,tui`), both enabled by default
- **Local-first** — fully functional offline. AI is the fallback, never the hot path.

## Design Principles

- **Local-first, always.** Every navigation is instant by default. The network is a luxury, not a dependency.
- **Invisible intelligence.** Frecency, project context, and AI blend seamlessly. There are no "modes" to think about.
- **Zero-config magic, full-config power.** It works perfectly the moment you install it. Every behavior is overridable for the curious.
- **Respect the developer.** No telemetry without consent. No nagging. No forced accounts. The free tier is the real product.

## Development Status

`tp` is in **alpha**. The core navigation engine, frecency scoring, project detection, waypoints, and shell integration are implemented and working. AI features and the TUI picker are stubbed and under active development.

## Roadmap

| Phase | Status | What's shipping |
|-------|--------|-----------------|
| **Alpha** | **In Progress** | Core Rust binary: frecency engine, project detection, waypoints, shell integration (6 shells), bootstrap, import |
| **Beta** | Planned | AI integration (BYOK): natural language nav, semantic reranking, disambiguation. Neovim plugin. Tab completion. |
| **v1.0** | Planned | Polished UX, workflow prediction, session recall, VS Code extension. Public launch. |
| **Pro** | Planned | Cloud sync, team shared waypoints, onboarding mode, analytics dashboard. |

## License

[MIT](LICENSE)
