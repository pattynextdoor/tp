<p align="center">

<img src="https://media1.tenor.com/m/mX9Rxbo-VvsAAAAd/toji-toji-season-2.gif" width="400" alt="tp — teleport anywhere" />

<br/>

<h3>🎮 Unlock fast travel in your terminal.</h3>

[![Rust](https://img.shields.io/badge/Built_with-Rust-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)
[![Beta](https://img.shields.io/badge/Status-Beta-yellow?style=flat-square)](#development-status)

</p>

---

Your terminal knows where you've been. `tp` knows where you're going.

Project-aware navigation that combines frecency with context — so when you type `tp src`, it picks the `src/` in the project you're actually working in. Built in Rust, works in six shells, useful from the first command.

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

**tp is useful from the first command.** On first run, it automatically indexes your shell history, imports from zoxide (if installed), and discovers projects under your home directory. No cold start, no manual setup.

---

## Why tp?

If you juggle multiple projects, tp was built for you.

| What you get | How it works |
|-------------|-------------|
| **Project-scoped search** | `tp -p tests` finds `tests/` within your current project, not globally |
| **Project jumping** | `tp @payments-service` switches to a project by name |
| **Waypoints** | `tp !deploy` — pin paths that frecency would forget |
| **Self-healing database** | Dead paths pruned automatically, never suggested |
| **Zero cold start** | Imports shell history, zoxide data, and discovers projects on first run |
| **AI tiebreaker** | When two paths score equally, an optional AI oracle picks the right one |

## Features

### Core Navigation

> Free, open source, works entirely offline. No accounts, no cloud, no strings.

- **Frecency scoring** — frequency + recency with time-decay weighting that matches or exceeds zoxide
- **Multi-token fuzzy matching** — `tp foo bar` matches paths containing both tokens
- **Typo tolerance** — `tp projetcs` still finds `projects` via Damerau-Levenshtein fallback (5+ char queries)
- **Project awareness** — auto-detection via `.git`, `Cargo.toml`, `package.json`, `go.mod`, and [12 more markers](#project-markers)
- **Project-scoped search** — `tp -p tests` stays inside your project boundaries
- **Cross-project switching** — `tp @payments-service` jumps to a known project root
- **Waypoints** — `tp --mark deploy` pins a directory; `tp !deploy` teleports there instantly
- **Smart cold start** — bootstraps from shell history, git repos, and existing zoxide databases
- **Built-in TUI picker** — interactive fuzzy finder showing project name, last modified, and git branch
- **Full `cd` compatibility** — relative paths, `..`, `-`, `~`, absolute paths all just work
- **6 shells** — bash, zsh, fish, PowerShell, Nushell, Elvish

### AI Features (BYOK)

> Bring your own API key. The tool never phones home unless you ask it to.

- **AI reranking** — when frecency scores are tied, AI considers your cwd and candidate paths to break the tie
- **Session recall** — `tp --recall` answers the Monday morning question: *"where was I?"*
- **Semantic project indexing** *(coming soon)* — search across projects by concept: `tp the service that handles webhook retries`
- **Workflow prediction** *(coming soon)* — spots recurring navigation sequences and nudges you toward the next destination
- **Natural language nav** *(planned)* — `tp the auth service terraform module` resolves even when none of those words appear in the path
- **Smart aliasing** — `tp suggest` recommends waypoint names for your most-visited directories, with optional AI-enhanced naming

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
            │ no matches?
 ┌──────────▼──────────┐
 │ 4b. Typo tolerance   │──▶ Damerau-Levenshtein fallback
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

**tp auto-bootstraps on first run.** The first time you navigate with an empty database, tp silently:

1. Imports your zoxide database (if zoxide is installed)
2. Parses your shell history (`~/.zsh_history`, `~/.bash_history`, fish history) for `cd` targets
3. Scans common code directories (`~/code`, `~/projects`, `~/repos`, etc.) for project roots

This takes <500ms and means your first `tp` command already has context.

You can also trigger it manually or import from a specific source:

```sh
tp init --bootstrap                                    # re-run auto-discovery
tp import --from=zoxide                                # import from zoxide
tp import --from=zoxide ~/.local/share/zoxide/db.zo    # import from file
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
tp import --from=zoxide Import from zoxide

tp suggest              Suggest waypoint names for frequent paths
tp suggest --apply      Interactively apply suggestions
tp suggest --ai         Use AI for creative waypoint names

tp index [path]         AI: semantic index a project
tp analyze              AI: extract workflow patterns
tp --recall             AI: "where was I?" session digest
tp --setup-ai           Configure API key for AI features

tp sync                 Force cloud sync (Pro)
tp doctor               Diagnose configuration issues

tp ls [-n COUNT]        List top directories by frecency
tp back [STEPS]         Jump back in navigation history
tp completions <shell>  Generate shell completions
```

## Configuration

All via environment variables. Sane defaults — most users won't need to touch these.

| Variable | Default | Description |
|----------|---------|-------------|
| `TP_DATA_DIR` | `$XDG_DATA_HOME/tp` | Database and config location |
| `TP_API_KEY` | — | Anthropic API key for AI features |
| `TP_AI_MODEL` | `claude-haiku-4-5-20251001` | AI model override |
| `TP_AI_TIMEOUT` | `2000` | AI request timeout (ms) |
| `TP_EXCLUDE_DIRS` | — | Comma-separated path prefixes to ignore (supports `~`) |

## Project Markers

`tp` walks up the directory tree looking for these files to detect project boundaries:

`.git` `Cargo.toml` `package.json` `go.mod` `pyproject.toml` `setup.py` `Gemfile` `pom.xml` `build.gradle` `CMakeLists.txt` `Makefile` `.project` `composer.json` `mix.exs` `deno.json` `flake.nix`

## Benchmarks

Measured with [hyperfine](https://github.com/sharkdp/hyperfine) on a MacBook Pro M4 Pro, 200+ runs each. The benchmark suite tests four scenarios to give a fair picture of both tools.

### Core queries (500 entries, flat seeding)

Raw query speed with 1 visit per path — the simplest comparison.

<p align="center">
<img src="bench/charts/core.svg" alt="Core benchmarks" width="600" />
</p>

### Realistic visit patterns (hot/warm/cold)

300 directories with varied visit counts: 50 "hot" paths (20 visits), 100 "warm" (5 visits), 150 "cold" (1 visit). This exercises frecency ranking under real-world conditions.

<p align="center">
<img src="bench/charts/varied.svg" alt="Varied visit pattern benchmarks" width="600" />
</p>

### Stale path handling

200 directories, 40% deleted after seeding. tp checks `Path::exists()` on every candidate and self-heals stale entries — this costs extra I/O but keeps your results clean.

<p align="center">
<img src="bench/charts/stale.svg" alt="Stale path benchmarks" width="600" />
</p>

### Scale (5,000 entries)

Does the speed gap hold at scale?

<p align="center">
<img src="bench/charts/scale.svg" alt="Scale benchmarks" width="600" />
</p>

> **Note:** tp's `add` does more work than zoxide's — it detects project roots by walking up the tree for `.git`, `Cargo.toml`, etc., and logs session data. This is the cost of project-scoped search and session recall.

Run the benchmarks yourself:

```sh
cargo build --release
./bench/bench.sh
python3 bench/chart.py   # generate SVG charts
```

## Architecture

<p align="center">
<img src="docs/architecture.svg" alt="tp architecture diagram" width="574" />
</p>

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

`tp` is in **beta**. Core navigation, frecency scoring, project detection, waypoints, shell integration, AI reranking, TUI picker, and session recall are all implemented and working.

| Phase | Status | Shipping |
|-------|--------|----------|
| **Alpha** | ✅ Complete | Core binary: frecency, project detection, waypoints, 6-shell integration, bootstrap, zoxide import |
| **Beta** | ✅ Complete | AI reranking (BYOK), TUI picker, session recall, smart aliasing, query/remove/doctor commands. CI on 3 platforms. |
| **v1.0** | Planned | Semantic project indexing, workflow prediction, natural language nav, VS Code extension. |
| **Pro** | Planned | Cloud sync, team waypoints, onboarding mode, analytics. |

## License

[MIT](LICENSE)
