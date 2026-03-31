# AI Features

`tp` includes an optional AI layer that enhances navigation when the local frecency engine is uncertain. All AI features follow the BYOK (Bring Your Own Key) model — the tool never phones home unless you explicitly configure it to.

## Setup

Run the interactive setup command to configure your API key:

```sh
tp --setup-ai
```

Or set the environment variable directly:

```sh
export TP_API_KEY="your-anthropic-api-key"
```

Once configured, AI features activate automatically when needed. You can fine-tune behavior with additional environment variables — see [Configuration](./configuration.md) for the full list.

## How AI Fits In

AI is a **tiebreaker, not a crutch**. The resolution pipeline (described in the [Introduction](./introduction.md#how-it-works)) only reaches the AI reranking step when local frecency scoring produces ambiguous results (roughly 5% of queries). When it does fire, a typical request uses ~150 tokens and completes in under 300ms.

## Implemented Features

### AI Reranking

When frecency scores are tied between candidates, AI considers your current working directory and candidate paths to break the tie intelligently. A typical request uses ~150 tokens and completes in under 300ms.

### Session Recall

Lost track of what you were working on? The `--recall` command produces a digest of your recent navigation session:

```sh
tp --recall
```

This answers the Monday morning question: *"where was I?"*

### Semantic Project Indexing

Index a project's directory tree by concept, enabling natural language searches:

```sh
tp index            # index the current project
tp index /path/to   # index a specific project
```

Once indexed, queries match against directory descriptions:

```sh
tp webhook handler   # finds src/webhooks/ even without "webhook" in the path
tp auth middleware   # finds src/auth/ based on its semantic description
```

**How it works:**
1. Walks the directory tree (top 3 levels, skipping noise like `node_modules`, `.git`, etc.)
2. Sends the tree structure to Claude Haiku in a single API call
3. Stores per-directory descriptions in SQLite
4. At query time, matches against descriptions with local keyword search first, falling back to AI only when ambiguous

The index respects `.gitignore` patterns. If an index is older than 30 days, `tp` will nudge you to re-index.

## Coming Soon (Stubbed)

These features are stubbed and under active development:

### Workflow Prediction

```sh
tp analyze
```

Spots recurring navigation sequences and nudges you toward the next likely destination. For example, if you frequently go from `src/api` to `tests/api` to `docs/api`, it learns that pattern and suggests optimizations (e.g., waypoints you should create, projects you frequently switch between).

### Smart Aliasing

Analyzes your visit history and suggests waypoint names for frequently visited directories:

```sh
tp suggest              # show suggestions
tp suggest --apply      # interactively apply them
tp suggest --ai         # use AI for creative names
```

Names are generated deterministically from path structure — generic components like `src/` are combined with their parent. With `--ai`, Claude Haiku suggests more memorable names. Suggestions are always presented for confirmation.

## Planned

### Natural Language Navigation

Navigate using descriptive phrases even when no tokens match the path:

```sh
tp the auth service terraform module
```

The AI layer will resolve intent by considering your project structure, not just string matching.

## Privacy

- AI features are **opt-in** via API key configuration
- No data is sent anywhere unless you set `TP_API_KEY`
- Queries go directly to the Anthropic API — there is no intermediary server
