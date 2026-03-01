# Configuration

tp is configured entirely through environment variables. No config files needed.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `TP_DATA_DIR` | `$XDG_DATA_HOME/tp` or `~/.local/share/tp` | Database and cache location |
| `TP_MAXAGE` | `10000` | Max total frecency score before aging kicks in |
| `TP_EXCLUDE_DIRS` | `$HOME` | Glob patterns for directories to exclude |
| `TP_PROJECT_MARKERS` | `.git,Cargo.toml,...` | Custom project root markers |
| `TP_RESOLVE_SYMLINKS` | `0` | Resolve symlinks before storing paths |
| `TP_ECHO` | `0` | Print the matched directory before navigating |
| `TP_API_KEY` | — | Anthropic API key for AI features |
| `TP_AI_MODEL` | `claude-haiku-4-5-20251001` | AI model override |
| `TP_AI_ENABLED` | `true` (if key set) | Toggle AI features on/off |
| `TP_AI_TIMEOUT` | `2000` | AI request timeout in milliseconds |

## Database Location

tp stores its SQLite database at:

- **Linux/macOS:** `~/.local/share/tp/tp.db`
- **With `$XDG_DATA_HOME` set:** `$XDG_DATA_HOME/tp/tp.db`
- **Custom:** Set `TP_DATA_DIR` to any path

The AI response cache lives alongside the database as `ai_cache.json`.

## Excluding Directories

To prevent certain directories from appearing in results:

```sh
export TP_EXCLUDE_DIRS="$HOME:/tmp:/var"
```

## Custom Project Markers

tp detects project roots by looking for these files/directories (in order):

`.git` `.hg` `.svn` `Cargo.toml` `package.json` `go.mod` `pyproject.toml` `build.gradle` `pom.xml` `Makefile` `CMakeLists.txt` `.project-root` `mix.exs` `deno.json` `flake.nix`

To add your own, drop a `.project-root` file in any directory.

## Resetting

To start fresh:

```sh
rm ~/.local/share/tp/tp.db
```

The next `tp` command will re-run auto-bootstrap.
