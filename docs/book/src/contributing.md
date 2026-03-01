# Contributing

Contributions to `tp` are welcome! Here's how to get started.

## Setup

Clone the repository and build:

```sh
git clone https://github.com/pattynextdoor/tp.git
cd tp
cargo build
```

## Running Tests

```sh
cargo test
```

To run tests with all feature flags:

```sh
cargo test --all-features
```

## Running Benchmarks

Build a release binary first, then run the benchmark suite:

```sh
cargo build --release
./bench/bench.sh
python3 bench/chart.py   # generate SVG charts
```

## Project Structure

- `src/` — Rust source code
- `bench/` — Benchmark scripts and chart generation
- `docs/` — Documentation (this mdbook site and architecture diagrams)
- `tests/` — Integration tests

## Code Style

The project uses standard Rust formatting and linting:

```sh
cargo fmt --check
cargo clippy
```

## License

`tp` is licensed under [MIT](https://github.com/pattynextdoor/tp/blob/main/LICENSE). By contributing, you agree that your contributions will be licensed under the same terms.
