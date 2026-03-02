# Typo Tolerance (Damerau-Levenshtein Fallback)

## Summary

Add typo tolerance to `tp`'s matching system as a fallback layer. When the
existing fuzzy matching tiers (exact, suffix, substring, multi-word) all return
zero results, a second pass uses Damerau-Levenshtein distance to find "close
enough" matches despite typos and transpositions.

## Design

### Algorithm

Damerau-Levenshtein distance on the **last path component** vs the query.
This handles the four most common typo types: insertions, deletions,
substitutions, and transpositions (`projetcs` → `projects`).

### Thresholds

| Query length | Max allowed edits |
|--------------|-------------------|
| < 5 chars    | No typo matching  |
| 5–8 chars    | 1                 |
| 9+ chars     | 2                 |

### Scoring

Returns `0.4` on match, `0.0` on no match. This places typo matches below
all existing tiers (exact=1.0, suffix=0.9, substring=0.7, multi-word=0.6).

### Integration

- Pure fallback: only invoked when `fuzzy_score` yields no candidates
- No config knobs, no feature flags
- Caller in `src/nav/mod.rs` runs a second pass with `typo_score` when the
  first pass returns nothing

### Dependency

`strsim` crate — provides `damerau_levenshtein()` out of the box.

## Files Changed

- `Cargo.toml` — add `strsim = "0.11"`
- `src/nav/matching.rs` — add `typo_score()` function + tests
- `src/nav/mod.rs` — fallback pass when fuzzy matching yields nothing
