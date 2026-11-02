# Track D — Performance Log

The measurements below are local results on the public seed-1 dataset. They are not official board results. No live-board run ID is recorded in this checkout; add the board run ID and measured result for each optimization step when available.

## Run 1 (Initial Correct & Optimised Implementation)
- **run-id**: Not recorded
- **approach**: Compact CSR (forward + reverse), parallel-edge dedup keeping lightest, bidirectional Dijkstra with provable bound-based stopping ($d_F(top_F) + d_R(top_R) \ge \mu$), duplicate-query caching, and 12-thread worker pool.
- **local verification (Seed 1 Full — 5,000,000 edges, 2,000 queries)**:
  - Local runtime: **0.88 s** (machine-dependent; not comparable as an official score)
  - Output SHA-256: `a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e` (Exact Match)
  - Memory Peak: ~480 MB (within <= 500 MB Target)
- **notes**: Local output matched the recorded expected digest and passed the listed local checks.

## Mechanism & Design Decisions
1. **Compact Dual CSR**:
   Forward and reverse adjacency stored in contiguous `u32` vectors. Avoids pointer chasing and heap fragmentation.
2. **Parallel-Edge Deduplication**:
   Initial sort by `(u, v)` collapses duplicate edges, storing only the minimum weight regardless of file order.
3. **Bound-Based Stopping Rule**:
   Halts when $top_F + top_R \ge \mu$. Guaranteed byte-exact distances with no premature termination.
4. **Sparse State Resets**:
   Thread-local touched-node buffers avoid resetting 1.74M elements on each query.
5. **Duplicate-Query Deduplication**:
   Prevents re-running identical Dijkstra queries.
