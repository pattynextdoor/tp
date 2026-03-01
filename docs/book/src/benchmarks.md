# Benchmarks

All benchmarks measured with [hyperfine](https://github.com/sharkdp/hyperfine) on a MacBook Pro M4 Pro, 200+ runs each. The benchmark suite tests four scenarios to give a fair picture of performance.

> Interactive SVG charts are available in the [GitHub README](https://github.com/pattynextdoor/tp#benchmarks).

## Core Queries (500 entries, flat seeding)

Raw query speed with 1 visit per path — the simplest comparison between `tp` and `zoxide`.

This scenario tests the baseline overhead of the frecency lookup engine with a uniform visit distribution.

## Realistic Visit Patterns (hot/warm/cold)

300 directories with varied visit counts:
- **50 "hot" paths** — 20 visits each
- **100 "warm" paths** — 5 visits each
- **150 "cold" paths** — 1 visit each

This exercises frecency ranking under real-world conditions, where some paths are visited far more often than others.

## Stale Path Handling

200 directories, 40% deleted after seeding. `tp` checks `Path::exists()` on every candidate and self-heals stale entries — this costs extra I/O but keeps your results clean. Stale paths are pruned automatically so they don't pollute future results.

## Scale (5,000 entries)

Tests whether performance holds at larger database sizes. With 5,000 tracked directories, `tp` exercises its SQLite indexes and fuzzy matching at a scale that approximates a power user's database after months of use.

## Note on `add` Performance

`tp`'s `add` command does more work than zoxide's equivalent — it detects project roots by walking up the directory tree for `.git`, `Cargo.toml`, etc., and logs session data. This is the cost of project-scoped search and session recall. Query performance is where it matters most, and `tp` matches or exceeds zoxide there.

## Running Benchmarks Yourself

```sh
cargo build --release
./bench/bench.sh
python3 bench/chart.py   # generate SVG charts
```

The benchmark scripts are in the `bench/` directory. `bench.sh` runs hyperfine comparisons and outputs JSON results. `chart.py` reads those results and generates the SVG charts.
