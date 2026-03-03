# Installation

## From crates.io

```sh
cargo install tp-nav
```

## Via Homebrew

```sh
brew install pattynextdoor/tap/tp
```

## From Source

```sh
git clone https://github.com/pattynextdoor/tp.git
cd tp
cargo install --path .
```

## Shell Setup

Add one line to your shell configuration file. This sets up the shell function that wraps `tp` so it can actually change your working directory.

### Bash

Add to `~/.bashrc`:

```sh
eval "$(tp init bash)"
```

### Zsh

Add to `~/.zshrc`:

```sh
eval "$(tp init zsh)"
```

### Fish

Add to `~/.config/fish/config.fish`:

```sh
tp init fish | source
```

### PowerShell

Add to `$PROFILE`:

```powershell
Invoke-Expression (& { tp init powershell } | Out-String)
```

### Nushell

Add to `~/.config/nushell/env.nu`:

```nu
tp init nushell | save -f ~/.cache/tp/init.nu; source ~/.cache/tp/init.nu
```

### Elvish

Add to `~/.config/elvish/rc.elv`:

```elvish
eval (tp init elvish | slurp)
```

## Custom Command Name

If you prefer a different command name (e.g., `j` instead of `tp`), use the `--cmd` flag:

```sh
eval "$(tp init bash --cmd j)"
```

This works with any shell.

## Bootstrap

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

**tp is useful from the first command.** On first run, it automatically indexes your shell history, imports from zoxide (if installed), and discovers projects under your home directory. No cold start, no manual setup.
