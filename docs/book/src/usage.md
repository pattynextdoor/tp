# Usage

## Quick Start

After [installing](./installation.md) and setting up your shell, you can start navigating immediately:

```sh
tp myproject               # jump to best match
tp -p tests                # find tests/ within the current project
tp @payments-service       # switch to a project by name
tp !deploy                 # teleport to a pinned waypoint
tp -i                      # interactive fuzzy picker
```

## Command Reference

### Navigation

```
tp <query>              Navigate to best match
tp -i [query]           Interactive picker
tp -p <query>           Search within current project
tp @<project>           Jump to project root by name
tp !<waypoint>          Jump to pinned waypoint
```

`tp` also supports full `cd` compatibility — relative paths, `..`, `-`, `~`, and absolute paths all just work.

### Waypoints

Pin frequently used directories for instant access:

```
tp --mark <name> [path] Pin a directory (defaults to cwd)
tp --unmark <name>      Remove a pin
tp --waypoints          List all waypoints
```

Then jump to any pin with the `!` prefix:

```sh
tp !deploy              # teleport to the pinned "deploy" directory
```

### Database Management

```
tp add <path>           Manually add a directory
tp remove <path>        Remove from database
tp query <query>        Print matches (for scripting)
```

### Shell Integration & Import

```
tp init <shell>         Shell integration code
tp init --bootstrap     Bootstrap from history
tp import --from=<tool> Import from zoxide/z/autojump/fasd
```

### AI Commands

These commands require an API key. See [AI Features](./ai-features.md) for setup.

```
tp index [path]         Semantic index a project
tp analyze              Extract workflow patterns
tp --recall             "Where was I?" session digest
tp --setup-ai           Configure API key for AI features
```

### Other

```
tp sync                 Force cloud sync (Pro)
tp doctor               Diagnose configuration issues
```

## Project Markers

`tp` uses project markers to detect project boundaries for project-scoped search (`tp -p`) and cross-project switching (`tp @`). It walks up the directory tree looking for these files:

`.git` `.hg` `.svn` `Cargo.toml` `package.json` `go.mod` `pyproject.toml` `build.gradle` `pom.xml` `Makefile` `CMakeLists.txt` `.project-root` `mix.exs` `deno.json` `flake.nix`

Drop a `.project-root` file in any directory to force project root detection, or extend the list via the `TP_PROJECT_MARKERS` environment variable (see [Configuration](./configuration.md)).
