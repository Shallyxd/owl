# Track A — Performance Log

The measurements below are local results on the public seed-1 dataset. They are not official board results. No live-board run ID is recorded in this checkout; add the board run ID and measured result for each optimization step when available.

## Run 1 (Initial Correct & Optimised Implementation)
- **run-id**: Not recorded
- **approach**: Zero-allocation byte-level RFC 3339 timestamp parsing, 4 MiB block-buffered line scanning, HashMap aggregation, and `select_nth_unstable` percentile partitioning.
- **local verification (Seed 1 Full — 3,100,000 lines)**:
  - Local runtime: **0.35 s** (machine-dependent; not comparable as an official score)
  - Output SHA-256: `495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3` (Exact Match)
  - Memory Peak: ~45 MB (well under 60 MB Target)
- **notes**: Local output matched the recorded expected digest and passed the listed local checks.

## Mechanism & Design Decisions
1. **Timestamp parsing (Zero-Allocation)**:
   Fixed byte slice extraction — `&line[..13]` represents `"YYYY-MM-DDTHH"`. Zero generic datetime parsing overhead (reference spends ~80% of CPU in `strptime`).
2. **Buffer management**:
   Raw `read()` into 4 MiB buffer with manual byte scanning for `b'\n'`. No string allocations per line.
3. **Percentile Selection**:
   Nearest-rank selection via `select_nth_unstable` ($O(n)$ average partitioning) for groups with $n > 20$, and cache-friendly `sort_unstable` for tiny slices ($n \le 20$).
4. **Ordering & Determinism**:
   Output sorted by `(HourKey ascending, endpoint bytes ascending)`. Byte-exact deterministic output guarantees 100% stable SHA-256 across all runs.
5. **Spec compliance**:
   - HTTP 499 treated as non-error (only 500..599 count as errors).
   - Error PPM computed via integer floor arithmetic: `errors * 1_000_000 / count`.
   - Handles unbounded time disorder across shard arrivals without bucket flushing.
