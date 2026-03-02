<p align="center">

<img src="https://media1.tenor.com/m/mX9Rxbo-VvsAAAAd/toji-toji-season-2.gif" width="400" alt="tp — teleport anywhere" />

<br/>

<h3>⚡ Unlock fast travel in your terminal.</h3>

[![CI](https://img.shields.io/github/actions/workflow/status/pattynextdoor/tp/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/pattynextdoor/tp/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Built_with-Rust-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)
[![Beta](https://img.shields.io/badge/Status-Beta-yellow?style=flat-square)](#status)

</p>

---

Your terminal knows where you've been. `tp` knows where you're going.

<p align="center">
<img src="demo.gif" width="800" alt="tp demo — fuzzy jumping, project switching, waypoints" />
</p>

Project-aware navigation that combines frecency with context — so when you type `tp src`, it picks the `src/` in the project you're actually working in. Built in Rust, works in six shells, useful from the first command.

## Install

```sh
cargo install --path .
eval "$(tp init zsh)"     # or bash, fish, powershell, nushell, elvish
```

That's it. On first run, tp bootstraps itself from your shell history, zoxide (if installed), and common code directories. No cold start.

## Usage

**Jump somewhere:**

```sh
tp myproject          # fuzzy match → best directory
tp api server         # multi-token → matches paths with both "api" and "server"
tp projetcs           # typo? still finds "projects"
```

**Stay inside your project:**

```sh
tp -p tests           # finds tests/ in your current project, not globally
tp -p src utils       # scoped multi-token search
```

**Switch projects:**

```sh
tp @payments-service  # jump to a project root by name
tp @                  # pick from all known projects
```

**Pin important paths:**

```sh
tp --mark deploy ~/infra/k8s/deploy
tp :deploy            # instant teleport, no scoring needed
tp --waypoints        # see all your pins
```

**Browse and backtrack:**

```sh
tp                    # no args → interactive fuzzy picker
tp back               # go back one jump
tp back 3             # go back three jumps
tp ls                 # see your top directories by frecency
```

**Day-end stuff:**

```sh
tp --recall           # "where was I today?" session digest
tp suggest            # recommends waypoint names for frequent paths
tp stats              # full TUI dashboard — heatmaps, project breakdown
```

## Why tp?

| What you get | How it works |
|-------------|-------------|
| **Project-scoped search** | `tp -p tests` finds `tests/` within your current project, not globally |
| **Project jumping** | `tp @payments-service` switches to a project by name |
| **Waypoints** | `tp :deploy` — pin paths that frecency would forget |
| **Self-healing database** | Dead paths pruned automatically, never suggested |
| **Zero cold start** | Imports shell history, zoxide data, and discovers projects on first run |
| **Tiebreaker reranking** | When two paths score equally, an optional BYOK oracle picks the right one |

## Shell Setup

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

Want a different command name? `eval "$(tp init bash --cmd j)"`

## Configuration

All via environment variables. Sane defaults — most people won't touch these.

| Variable | Default | Description |
|----------|---------|-------------|
| `TP_DATA_DIR` | `$XDG_DATA_HOME/tp` | Database and config location |
| `TP_API_KEY` | — | Anthropic API key for BYOK reranking |
| `TP_AI_MODEL` | `claude-haiku-4-5-20251001` | Model override |
| `TP_AI_TIMEOUT` | `2000` | Request timeout (ms) |
| `TP_EXCLUDE_DIRS` | — | Comma-separated path prefixes to ignore (supports `~`) |

## Status

tp is in **beta**. Core navigation, frecency, project detection, waypoints, shell integration, BYOK reranking, TUI picker, and session recall are all working. See [ROADMAP.md](ROADMAP.md) for what's next.

## Deeper

- [DESIGN.md](DESIGN.md) — architecture, scoring, benchmarks vs zoxide
- [ROADMAP.md](ROADMAP.md) — what's planned

## License

[MIT](LICENSE)
