# Decision Log — Speed Demon 2026 Submission

This document records architectural decisions, trade-offs, non-goals, and deliberate omissions across the Speed Demon track implementations.

---

## 1. Win Thesis: Measured Speed First
- **Decision:** Prioritize wall-clock execution time ($T$) above all else, while using zero-dependency Rust ($D=1$) and staying within the source line budget ($S$).
- **Rationale:** The scoring formula weights wall time most heavily. Source size and dependency count can reduce the efficiency coefficient but cannot improve it below their reference baselines. Optimization choices prioritize measured runtime improvements while keeping these costs in view.

## 2. Zero External Dependencies Everywhere ($D = 1$)
- **Decision:** Use only standard library Rust (`std`) for all four tracks. No external crates are declared.
- **Rationale:** A direct runtime dependency reduces the dependency factor in the efficiency coefficient. The implementations use custom byte-level routines where those avoid adding runtime dependencies.

## 3. Track-Specific Architectural Decisions

### Track A (Ingest)
- **Decision:** Fixed-offset 13-byte slice timestamp extraction and `select_nth_unstable` percentile partitioning.
- **Omission / Non-Goal:** Generic date/time libraries are not used; the input timestamp format permits direct byte parsing.
- **Omission / Non-Goal:** Hourly buckets are not flushed when the hour changes in the input stream because shard records may be interleaved.

### Track B (Index)
- **Decision:** Query-selective vocabulary generation before corpus tokenization.
- **Rationale:** Because all queries are known upfront at process execution, indexing only terms that appear in queries reduces the posting list table size from $>600,000$ unique vocabulary terms down to the few thousand terms actually evaluated.
- **Omission / Non-Goal:** Posting compression is not implemented. The current approach favors direct `u32` posting-list traversal within the event memory limit.

### Track C (Join)
- **Decision:** Streaming byte-level finite state machine with 64-bit integer cents accumulation and dense array region lookups.
- **Omission / Non-Goal:** No third-party CSV parser is used. A custom state machine handles the specified multiline records, BOMs, mixed line endings, and significant trailing spaces.

### Track D (Traverse)
- **Decision:** Compact forward and reverse CSR adjacency with bound-based bidirectional Dijkstra stopping ($top_F + top_R \ge \mu$).
- **Omission / Non-Goal:** The search does not stop at the first frontier intersection; that condition is not sufficient for exact shortest paths on general weighted graphs.
- **Omission / Non-Goal:** Contraction Hierarchies and Hub Labeling are not implemented. Their preprocessing cost is included in the timed task and may not amortize for the supplied query set.
