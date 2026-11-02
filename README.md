# Speed Demon 2026 Submission

Rust implementations for the four Speed Demon tracks. Each track is self-contained and uses only the Rust standard library at runtime.

## Tracks

- `track-a/` — Ingest: parse request logs and aggregate latency statistics.
- `track-b/` — Index: build a text index and answer Boolean and phrase queries.
- `track-c/` — Join: parse CSV data, join region records, and compute aggregates.
- `track-d/` — Traverse: answer shortest-path queries on a directed weighted graph.

Each track directory contains its Dockerfile, Rust manifest, source, approach README, performance log, and `.speed-demon.toml` metadata.

## Local checks

Run Rust unit tests from each track directory:

```sh
(cd track-a && cargo test)
(cd track-b && cargo test)
(cd track-c && cargo test)
(cd track-d && cargo test)
```

Run the Track C hostile CSV parser checks from the repository root:

```sh
python3 tools/test_c_parser.py
```

Run the repository pre-push checks from the repository root:

```sh
tools/prepush_check.sh
```

Local timings are machine-dependent and are not official event scores. The event harness determines correctness gates and performance results.
