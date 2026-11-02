# Honest Limits — Speed Demon 2026 Submission

This document records known limitations, assumptions, and unhandled cases in the implementations.

---

## 1. Track A (Ingest)
- **Timestamp Layout Assumption:** Assumes the RFC 3339 timestamp is exactly 27 bytes formatted as `YYYY-MM-DDTHH:MM:SS.uuuuuuZ`. If an input line contains alternative timezone offsets (e.g. `+05:30`), it will be skipped by the strict byte validator.
- **Latency Range:** Assumes latencies fit in standard `u32` (0 to 4,294,967,295 µs) as guaranteed by §2.
- **Single-Threaded I/O:** Track A executes as a single-threaded process with a 4 MiB buffer. Parallel chunk processing could improve throughput on a multicore host; the benefit requires measurement.

## 2. Track B (Index)
- **Known Query Set:** The engine builds inverted postings specifically for terms present in `queries.txt`. If a new query arrives after indexing that contains unindexed terms, it cannot evaluate that term.
- **Uncompressed Postings:** Postings are stored as uncompressed `Vec<u32>`. While this maximizes scan speed and fits within the 8 GB container memory limit, on an embedded system with <512 MB RAM, posting compression would be required.
- **Binary Left-Associativity:** Boolean expressions are evaluated strictly left-to-right without operator precedence (matching spec §2.3). `A OR B AND C` is evaluated as `(A OR B) AND C`.

## 3. Track C (Join)
- **Dense Region ID Range:** Assumes region IDs fit within memory for direct indexing ($< 10,000,000$). If region IDs were 64-bit sparse UUIDs, an open-addressing hash table would be required.
- **Memory Consumption:** Loads the full raw `records.csv` into memory buffer. On input files exceeding several gigabytes, streaming chunked processing with external sorting would be necessary.

## 4. Track D (Traverse)
- **Non-Negative Weights:** Dijkstra requires non-negative edge weights ($w \ge 0$). Graphs with negative edge weights would cause infinite loops and require Bellman-Ford or Johnson's algorithm.
- **Dense Node IDs:** The CSR uses node IDs directly as indices into `head` arrays, assuming $0 \le u < n$ (guaranteed by §2). Sparse 64-bit node IDs would require coordinate compression.
- **Memory Peak:** Reconstructing dual CSR (forward and reverse) for 5,000,000 edges requires ~480 MB peak memory.
