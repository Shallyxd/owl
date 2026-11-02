# Track D — Traverse (Shortest Paths)

Computes single-pair shortest-path distances across a 5,000,000-edge directed weighted graph (1,740,512 nodes) for 2,000 queries, writing answers to `/out/distances.txt`.

## Approach in Prose

The reference runs Dijkstra separately for each query. This Rust implementation uses bidirectional search and standard-library data structures, with no external dependencies ($D=1$):

1. **Compact Compressed Sparse Row (CSR)**:
   The graph is parsed into forward and reverse CSR representation. Edges are sorted by `(u, v)` and deduplicated, keeping the minimum edge weight regardless of input file order. Reverse edges are constructed symmetrically without additional allocations.
2. **Bidirectional Dijkstra with Provable Stopping Rule**:
   Searches simultaneously from source $s$ (forward) and target $t$ (reverse). Frontier expansion follows equal-work alternation (expanding whichever frontier has the lighter top priority).
   Stopping rule: The search halts when $d_F(top_F) + d_R(top_R) \ge \mu$, where $\mu$ tracks the minimum weight of any completed path seen so far. Stopping at first intersection is provably incorrect; bound-based stopping guarantees exact shortest paths.
3. **Sparse State Resets**:
   Rather than zeroing or re-allocating 1.74M-element distance arrays on each query, touched node IDs are stored in thread-local vectors so only visited indices are reset to `INF`, maintaining $O(\text{visited})$ query overhead.
4. **Duplicate Query Cache**:
   The full-scale input contains 20 duplicate queries. Unique `(s, t)` pairs are indexed up front, solved once, and mapped back into input order.
5. **12-Thread Query Pool**:
   Unique queries are distributed across up to 12 worker threads, matching the event's available thread limit.

## Dependency List
- **External Dependencies**: None ($D = 1$). Standard library (`std`) only.

## Correctness Traps Handled
- **Strictly Directed**: Reverse edges exist for ~97% of node pairs; one-way streets are preserved without artificial symmetrization.
- **Weight 0**: Handled natively by non-negative priority queue.
- **64-bit Overflow**: Distances exceeding $2^{32} - 1$ are accumulated as 64-bit unsigned integers and output as signed 64-bit integers.
- **Trivial & Unreachable Queries**: $s = t \implies 0$ (even on isolated nodes); unreachable $\implies -1$.

## Further optimization opportunities

Potential follow-up work includes:
1. **Custom 2-Level Radix Heap**: Replace `BinaryHeap` with a flat monotone radix heap using bit-manipulation (`lzcnt`), achieving $O(1)$ amortized push/pop.
2. **Contiguous Node Layout by Degree**: Renumber nodes so high-degree vertices have cache-aligned CSR pointers.
3. **SIMD Weight Relaxation**: Vectorize edge relaxation using AVX2 when a node has multiple outgoing edges.
