# Beta Features Design

Date: 2026-03-01

## Overview

Five features to take tp from alpha to beta:
1. TUI Picker (interactive fuzzy finder)
2. AI Reranking (semantic candidate scoring)
3. Session Recall (`tp --recall`)
4. Zoxide Import (`tp import --from=zoxide`)
5. AI Setup validation (`tp --setup-ai`)

---

## 1. TUI Picker (`tp -i`)

**File:** `src/tui/mod.rs` (~200-300 lines)

**Input:** `&[Candidate]` — already scored/sorted by the nav engine.

**UI Layout (ratatui + crossterm, alternate screen):**
```
┌─ tp ─────────────────────────────────────┐
│ > myproj_                                │
│                                          │
│   /home/user/projects/myproject          │
│     myproject · main · 2m ago            │
│ → /home/user/projects/myproject-api      │
│     myproject-api · feat/auth · 15m ago  │
│   /home/user/code/myproject-old          │
│     myproject-old · main · 3d ago        │
│                                          │
│ 3/47 matches                             │
└──────────────────────────────────────────┘
```

**Each entry (2 lines):**
- Line 1: Full path
- Line 2 (dimmed): project name · git branch · relative time since last access

**Key bindings:**
- `Up/Down` or `k/j` — move selection
- `Enter` — select and return path
- `Esc` or `Ctrl+C` — cancel (return `None`)
- Typing — filters candidates in real-time using existing `fuzzy_score`

**Git branch detection:** Run `git rev-parse --abbrev-ref HEAD` in each candidate's project root. Cache per project root to avoid redundant calls.

**Integration:** `navigate()` in `src/nav/mod.rs` calls `tui::pick()` at Step 6 when `--interactive` flag is set or as the final fallback.

---

## 2. AI Reranking

**File:** `src/ai/mod.rs`

**Trigger:** Step 5 of nav cascade — only when:
- Top 2+ candidates have scores within 20% of each other
- An API key is detected via `detect_api_key()`

**HTTP call (blocking reqwest):**
- Endpoint: `https://api.anthropic.com/v1/messages`
- Model: `TP_AI_MODEL` env var or `claude-haiku-4-5-20251001`
- Timeout: `TP_AI_TIMEOUT` env var or 2000ms
- Max tokens: 50
- Payload: query, top 10 candidate paths, cwd, project context
- System prompt: "You are a directory navigation assistant. Given a query and candidate paths, return the 0-based index of the best match. Reply with only the number."

**Response:** Parse integer index, validate in bounds, return `Some(path)`. Any error → `None`.

**Cache:** JSON file at `$TP_DATA_DIR/ai_cache.json`.
- Key: hash of `(query, sorted candidate paths)`
- Value: `(chosen_path, timestamp)`
- TTL: 24 hours
- Max entries: 500 (LRU eviction)
- Checked before HTTP call, written after successful response

**Dependency change:** `reqwest = { features = ["blocking", "json", "rustls-tls"] }`. Drop `tokio` dep.

---

## 3. Session Recall (`tp --recall`)

**File:** `src/ai/recall.rs`

**Flow:**
1. Query `sessions` table for last 24h of navigation
2. Group by project root, count visits, find most recent paths
3. If AI key available: send summary to Claude with prompt "Summarize this developer's recent navigation session. What were they working on?"
4. Print AI response to stderr
5. Fallback (no AI key): print raw stats — top 10 paths grouped by project with visit counts

**Requires:** `db::open()` connection passed in (update function signature).

---

## 4. Zoxide Import (`tp import --from=zoxide`)

**File:** `src/cli.rs` (import handler)

**zoxide DB format** (`~/.local/share/zoxide/db.zo`):
- Header line (skip)
- Each line: `path|frecency_score|last_accessed_epoch`

**Flow:**
1. Auto-detect path: `~/.local/share/zoxide/db.zo` (or user-provided)
2. Parse each line, skip header
3. Insert into `directories` table with parsed frecency and access time
4. Detect project root for each path
5. Print count of imported entries

---

## 5. AI Setup (`tp --setup-ai`)

**File:** `src/ai/mod.rs` (update `setup_key()`)

**Flow:**
1. Call existing `detect_api_key()`
2. If found: report which var, test with a 1-token API call, report success/failure
3. If not found: print setup instructions for `TP_API_KEY` or `ANTHROPIC_API_KEY`
4. No interactive prompts — fully env-var based

---

## Dependency Changes

```toml
# Remove tokio entirely
# Change reqwest features:
reqwest = { version = "0.12", features = ["blocking", "json", "rustls-tls"], default-features = false, optional = true }
```

## Implementation Order

1. **Import** — smallest, no new deps needed
2. **AI module** (reranking + cache + setup) — foundation for recall
3. **Session recall** — builds on AI module
4. **TUI picker** — largest, independent of AI work
