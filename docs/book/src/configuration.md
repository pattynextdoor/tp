# Configuration

All configuration is via environment variables. `tp` ships with sane defaults and requires zero configuration to use, but every knob is exposed if you want to tune it.

## Environment Variables

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

## AI Configuration

To enable AI features, set your API key:

```sh
tp --setup-ai
```

Or set the environment variable directly:

```sh
export TP_API_KEY="your-anthropic-api-key"
```

You can override the model or adjust the timeout:

```sh
export TP_AI_MODEL="claude-haiku-4-5-20251001"
export TP_AI_TIMEOUT="3000"
```

See [AI Features](./ai-features.md) for more details on what the AI layer provides.
