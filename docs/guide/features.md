# Features

## Frecency Navigation

tp learns from your habits. Every time you navigate to a directory, it records the visit. Directories you visit frequently and recently score higher.

```sh
tp api             # jumps to the "api" directory you visit most
tp api gateway     # multi-token: matches paths containing both "api" and "gateway"
```

The scoring uses time-decay weights:
- Visited in the last 5 minutes → 4x weight
- Last hour → 2x
- Last day → 1x
- Last week → 0.5x
- Older → 0.25x

## Project-Scoped Search

When you're deep in a monorepo and there are 12 directories called `tests/`:

```sh
tp -p tests        # only searches within your current project
```

tp detects project boundaries by walking up the directory tree looking for markers like `.git`, `Cargo.toml`, `package.json`, `go.mod`, and [12 others](../../README.md#project-markers).

## Project Switching

Jump directly to a project root:

```sh
tp @payments       # jumps to the project root matching "payments"
tp @               # list all known projects
```

## Waypoints

Pin directories you visit often:

```sh
tp --mark deploy ~/infra/k8s/deploy    # create a waypoint
tp :deploy                              # instant jump
tp --waypoints                          # list all waypoints
tp --unmark deploy                      # remove a waypoint
```

Waypoints are absolute — they work from anywhere, always.

## Interactive Picker

When you want to browse instead of guess:

```sh
tp -i              # show all directories
tp -i api          # show directories matching "api"
```

The picker shows:
- Full path
- Project name
- Git branch (if in a git repo)
- How long ago you last visited

Navigate with arrow keys or Ctrl+j/k. Type to filter. Enter to select. Esc to cancel.

## AI Features (Optional)

tp can use Claude Haiku to break ties when two directories score similarly.

### Setup

```sh
export ANTHROPIC_API_KEY=sk-ant-...    # or set TP_API_KEY
tp --setup-ai                           # verify the connection
```

### How It Works

AI is **never the first choice**. It only activates when:
- Two or more candidates score within 20% of each other
- An API key is available
- The request completes within 2 seconds (configurable via `TP_AI_TIMEOUT`)

If the AI is slow, unreachable, or returns garbage, tp silently falls back to the frecency-based result. Navigation never blocks on a network request.

### Session Recall

```sh
tp --recall        # "where was I working?"
```

Shows your last 24 hours of navigation, grouped by project. With an API key, it provides an AI-generated summary of your work session.

### Cost

AI reranking uses Claude Haiku (`claude-haiku-4-5-20251001`). Each query uses ~150 tokens, costing less than $0.001. Results are cached for 24 hours, so repeated queries don't hit the API.

## Zoxide Migration

Already using zoxide? Bring your history:

```sh
tp import --from=zoxide               # auto-detects zoxide database
tp import --from=zoxide path/to/db    # or specify a file
```

tp also auto-imports from zoxide on first run if it's installed.

## Shell Support

tp generates shell hooks for 6 shells:

| Shell | Config file | Init command |
|-------|------------|--------------|
| zsh | `~/.zshrc` | `eval "$(tp init zsh)"` |
| bash | `~/.bashrc` | `eval "$(tp init bash)"` |
| fish | `~/.config/fish/config.fish` | `tp init fish \| source` |
| PowerShell | `$PROFILE` | `Invoke-Expression (& { tp init powershell } \| Out-String)` |
| Nushell | `~/.config/nushell/env.nu` | `tp init nushell \| save -f ~/.cache/tp/init.nu` |
| Elvish | `~/.config/elvish/rc.elv` | `eval (tp init elvish \| slurp)` |

Want a different command name?

```sh
eval "$(tp init zsh --cmd j)"     # use "j" instead of "tp"
```
