# Track A — Ingest

Parses 3,100,000 HTTP request log lines across 8 interleaved shards and reports per-(hour, endpoint) request count, p50, p95, p99 latencies, and 5xx error rates to `/out/summary.txt`.

## Approach in Prose

The reference implementation spends most of its time parsing timestamps and sorting latency values. This implementation uses Rust's standard library and targets those costs without external dependencies ($D=1$):

1. **Zero-Allocation Byte Parsing**:
   The RFC 3339 timestamp format (`YYYY-MM-DDTHH:MM:SS.uuuuuuZ`) places the UTC hour prefix in bytes 0..13. A 13-byte `HourKey` is extracted directly from the buffer without heap allocation or calendar conversion.
2. **Reverse Field Tokenization**:
   Parsing status and latency from the end of the line avoids scanning arbitrary-length endpoint strings. Latency is parsed directly via ASCII integer accumulation.
3. **Chunk-Streamed I/O**:
   A 4 MiB buffer reads raw chunks from disk, finding newline boundaries with single-pass byte matching.
4. **Partitioned Selection**:
   Percentiles follow the exact nearest-rank specification (`ceil(p * n / 100)`). For groups with $n > 20$, `select_nth_unstable` partitions the latencies in $O(n)$ average time rather than performing full $O(n \log n)$ sorts.
5. **Deterministic Ordering**:
   Group keys are sorted chronologically by hour and unsigned lexicographical byte order by endpoint template before emitting lines to `/out/summary.txt`.

## Dependency List
- **External Dependencies**: None ($D = 1$). Standard library (`std`) only.

## Correctness Traps Handled
- **HTTP 499**: Client aborts are explicitly not server errors. Only statuses 500..599 increment error counters.
- **Integer Floor PPM**: `error_ppm` is computed via integer division `errors * 1_000_000 / count`.
- **Unbounded Disorder**: Shard arrivals interleave timestamps across days and hours; no premature bucket eviction.
- **Unsigned-Byte Ordering**: Endpoints sort by raw byte value (prefix-first), ensuring byte-exact determinism.

## Further optimization opportunities

Potential follow-up work includes:
1. **Custom Open-Addressing Flat Hash Map**: Replace `std::collections::HashMap` with a contiguous flat table using Robin Hood hashing or quadratic probing, avoiding pointer indirection.
2. **AVX2 SIMD Newline Vectorization**: Scan 32 bytes per cycle using `_mm256_cmpeq_epi8` to find newlines and spaces simultaneously.
3. **Parallel Shard Worker Pool**: Process individual chunks across multiple threads with thread-local hash aggregation, merging buckets prior to final selection.
