# Track B — Performance Log

The measurements below are local results on the public seed-1 dataset. They are not official board results. No live-board run ID is recorded in this checkout; add the board run ID and measured result for each optimization step when available.

## Run 1 (Initial Correct & Optimised Implementation)
- **run-id**: Not recorded
- **approach**: Query-selective inverted index, zero-allocation byte-ASCII tokenizer (§2.2), linear two-pointer slice merges, anchor-first candidate filtering, and 8-thread worker pool.
- **local verification (Seed 1 Full — 250,000 docs, 50,000 queries)**:
  - Local runtime: **2.10 s** (machine-dependent; not comparable as an official score)
  - Output SHA-256: `f86973c210228141bcb6f1f1b10188ad2a9b65f71e7cd7cf4590b38de4e80927` (Exact Match)
  - Memory Peak: ~280 MB (Target <= 600 MB)
- **notes**: Local output matched the recorded expected digest and passed the listed local checks.

## Mechanism & Design Decisions
1. **Query-Selective Vocabulary**:
   Builds inverted index postings only for terms present in queries, avoiding indexing millions of unique body tokens.
2. **Byte-Level Tokenization**:
   Splits on non-alphanumeric ASCII bytes, matching `Zürich -> z + rich`.
3. **Linear Postings Merging**:
   Standard sorted `u32` slice intersections and unions with branch-predictable loops.
4. **Anchor-First Phrases**:
   Filters candidate documents using term intersection before verifying exact word ordering in doc slices.
