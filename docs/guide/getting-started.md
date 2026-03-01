# Getting Started

## Install

### From source (recommended)

```sh
cargo install tp-nav
```

This installs the `tp` binary.

### From GitHub releases

Download the latest binary for your platform from [Releases](https://github.com/pattynextdoor/tp/releases).

### Homebrew (macOS/Linux)

```sh
brew install pattynextdoor/tap/tp
```

## Shell Setup

Add one line to your shell config:

```sh
# zsh (~/.zshrc)
eval "$(tp init zsh)"

# bash (~/.bashrc)
eval "$(tp init bash)"

# fish (~/.config/fish/config.fish)
tp init fish | source

# PowerShell ($PROFILE)
Invoke-Expression (& { tp init powershell } | Out-String)

# Nushell (~/.config/nushell/env.nu)
tp init nushell | save -f ~/.cache/tp/init.nu; source ~/.cache/tp/init.nu

# Elvish (~/.config/elvish/rc.elv)
eval (tp init elvish | slurp)
```

Restart your shell (or `source` the config file).

## First Run

**tp is useful immediately.** The first time you navigate, tp automatically:

1. **Imports from zoxide** — if you have zoxide installed, your history transfers over
2. **Parses shell history** — extracts directories from `cd` commands in `~/.zsh_history`, `~/.bash_history`, or fish history
3. **Discovers projects** — scans `~/code`, `~/projects`, `~/repos`, `~/src`, `~/dev`, `~/work` for directories with `.git`, `package.json`, `Cargo.toml`, etc.

This happens once, takes less than 500ms, and prints a single line:

```
tp: indexed 47 directories from your history. Ready.
```

After that, every `cd` you make through the shell hook feeds tp's frecency database — it gets smarter over time.

## Basic Usage

```sh
tp myproject          # jump to the best match for "myproject"
tp api                # jump to whatever "api" directory you visit most
tp -p tests           # search only within your current project
tp -i                 # open the interactive picker
```

## What Happens Under the Hood

When you type `tp api`:

1. **Literal check** — is `api` a relative/absolute path? If so, `cd` directly.
2. **Waypoint check** — did you pin something as `!api`? Jump there.
3. **Project check** — is `@api` a known project root? Jump there.
4. **Frecency search** — query all known directories, score by frequency + recency + fuzzy match.
5. **AI reranking** (optional) — if the top two scores are within 20%, ask Claude Haiku to break the tie (<300ms).
6. **TUI picker** — if in interactive mode, show the results for you to choose.

Most navigations resolve at step 4 in under 3ms.
