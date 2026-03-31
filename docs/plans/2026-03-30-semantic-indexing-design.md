# Semantic Project Indexing — Design

**Date:** 2026-03-30

## Overview

`tp index [path]` scans a project's directory tree (top 3 levels, excluding noise), sends the tree to Claude Haiku in a single API call, and stores per-directory semantic descriptions in SQLite. At query time, `tp webhook handler` first tries local keyword matching against those descriptions, then falls back to an AI call if no strong match is found.

## Data Model

New table `project_dirs`:

```sql
CREATE TABLE IF NOT EXISTS project_dirs (
    id           INTEGER PRIMARY KEY,
    project_root TEXT NOT NULL,
    rel_path     TEXT NOT NULL,
    description  TEXT NOT NULL,
    indexed_at   INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    UNIQUE(project_root, rel_path)
);
CREATE INDEX IF NOT EXISTS idx_project_dirs_root ON project_dirs(project_root);
```

Each row is one directory within a project. `rel_path` is relative to `project_root` (e.g. `src/services/webhooks`). The existing `projects` table stays untouched — its `indexed_at` column is updated to track when a project was last indexed.

## Indexing Flow (`tp index [path]`)

1. **Resolve project root** — use existing `project::detect_project_root()` on the target path. Error if no project markers found.

2. **Walk the directory tree** — scan up to 3 levels deep. Skip directories that match:
   - Hardcoded noise: `.git`, `node_modules`, `target`, `__pycache__`, `.venv`, `dist`, `build`, `.next`, `.cache`, `vendor`, `.idea`, `.vscode`
   - `.gitignore` patterns (parse the root `.gitignore` — simple directory patterns only)

3. **Build the prompt** — send the tree as a flat list to Claude Haiku:
   ```
   Project: tp (rust)
   Directories:
     src/
     src/ai/
     src/db/
     src/nav/
     src/tui/
     docs/
     docs/book/
     bench/

   For each directory, write a short (5-15 word) description of what it likely
   contains based on its name and position in the tree. Return JSON:
   {"src/ai/": "AI integration module for reranking and session recall", ...}
   ```

4. **Parse response** — extract the JSON map of `rel_path -> description`.

5. **Upsert into `project_dirs`** — clear old entries for this project root, insert the new ones. Update `projects.indexed_at`.

6. **Report** — print how many directories were indexed and a sample.

One API call total. Gated behind `#[cfg(feature = "ai")]` with a no-op message when the feature is off. Uses the existing `detect_api_key()`, spinner, model/timeout env vars.

## Query-Time Integration

When the user types `tp webhook handler` and the normal frecency cascade (steps 1-6 in `nav::navigate`) produces no confident match, a new step fires:

### Step 5.5 — Semantic Index Search

1. **Detect current project** — call `detect_project_root(cwd)`.
2. **Local keyword match** — query `project_dirs` for rows where `description` contains any of the search terms (case-insensitive `LIKE`). Score by number of terms matched. If a single directory scores well above the rest, return it immediately — no API call.
3. **AI fallback** — if multiple directories match similarly (or none match), send the query + matching descriptions to the AI and ask it to pick the best one. Same pattern as the existing reranker.
4. **Resolve to full path** — join `project_root + rel_path`, return as `NavResult` with `match_type: "semantic"`.

This only fires when:
- The user has an indexed project in their cwd
- Normal frecency didn't produce a confident result

### Staleness Nudge

When navigating within a project whose `indexed_at` is older than 30 days, print a one-line hint to stderr: `index for "tp" is 30+ days old — run tp index to refresh`. Shown once per session.

## .gitignore Parsing

Pragmatic 80/20 parse — no new dependency:

- Read the root `.gitignore` file (if it exists)
- Extract lines that are simple directory patterns: `dirname`, `dirname/`, `/dirname`
- Skip `!` negation lines, glob patterns with `*` or `**`
- Combine with the hardcoded ignore list as a set of directory names to skip

## File Layout

### New files
- `src/ai/index.rs` — indexing logic (tree walk, prompt building, API call, DB upsert)
- `src/ai/semantic.rs` — query-time semantic search (keyword match + AI fallback)

### Modified files
- `src/db/schema.rs` — add `project_dirs` table + index to the migration
- `src/cli.rs` — replace `Commands::Index` stub with real logic, wire semantic search
- `src/nav/mod.rs` — add step 5.5 (semantic search) to the cascade
- `src/ai/mod.rs` — add `pub mod index; pub mod semantic;`
- `docs/book/src/ai-features.md` — move semantic indexing from "Coming Soon" to "Implemented"
- `README.md` — update feature list / status

### No new dependencies
Uses `reqwest` (already gated on `ai`), `serde_json` (already present), `rusqlite` (already present). The `.gitignore` parsing is hand-rolled.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Index granularity | Top-level directories (3 levels) | Fast, cheap, sufficient for navigation |
| Storage | New SQLite table `project_dirs` | Consistent with existing pattern |
| AI call strategy | Single batch call per index | Minimizes cost and latency |
| Query-time search | Local keyword first, AI fallback | Mirrors frecency-then-AI philosophy |
| Indexing trigger | Manual with staleness nudge (30 days) | Respects BYOK cost-awareness |
| Ignored dirs | Hardcoded list + `.gitignore` | Good coverage without config surface |
