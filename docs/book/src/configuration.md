# Configuration

All configuration is via environment variables. `tp` ships with sane defaults and requires zero configuration to use, but every knob is exposed if you want to tune it.

## Environment Variables

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

Drop a `.project-root` in any directory to force project root detection. You can also extend or override the marker list with the `TP_PROJECT_MARKERS` variable:

```sh
export TP_PROJECT_MARKERS=".git,package.json,Cargo.toml,my-custom-marker"
```

## AI Configuration

To enable AI features, set your API key:

```sh
tp --setup-ai
```

Or set the environment variable directly:

```sh
export TP_API_KEY="your-anthropic-api-key"
```

You can override the model, disable AI entirely, or adjust the timeout:

```sh
export TP_AI_MODEL="claude-haiku-4-5-20251001"
export TP_AI_ENABLED="false"
export TP_AI_TIMEOUT="3000"
```

See [AI Features](./ai-features.md) for more details on what the AI layer provides.
