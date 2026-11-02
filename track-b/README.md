# Track B — Index (Full-Text Search Engine)

Indexes a 250 MB text corpus (250,000 legal documents) and answers 50,000 boolean and exact-phrase queries, writing results to `/out/results.txt`.

## Approach in Prose

In Speed Demon 2026, all queries are provided upfront at runtime. Indexing only the terms that appear in queries is explicitly legitimate and reduces inverted index size by over 90%:

1. **Selective Term Vocabulary**:
   All 50,000 queries are parsed before corpus indexing. Distinct query terms are assigned dense integer IDs.
2. **Byte-ASCII Tokenizer (§2.2)**:
   Scans raw byte slices, splitting on non-alphanumeric ASCII characters (`is_ascii_alphanumeric`). Case folding converts `b'A'..=b'Z'` to lowercase via `+ 32`. Non-ASCII bytes (e.g. UTF-8 multi-byte sequences) act as delimiters (e.g. `Zürich` -> `z` and `rich`).
3. **Linear Slice Merging**:
   Boolean operators (`AND`, `OR`, `NOT`) are strictly left-associative. Postings are sorted document ID slices merged via branchless two-pointer linear sweeps (`intersect`, `union`, `difference`).
4. **Anchor-First Phrase Verification**:
   For multi-term phrases, candidate documents are filtered first by intersecting postings of the query terms. Only candidate documents that contain all terms undergo full exact phrase sequence verification.
5. **Parallel Multi-Core Execution**:
   Query evaluation is partitioned across worker threads, formatting output lines in chunked buffers.

## Dependency List
- **External Dependencies**: None ($D = 1$). Standard library (`std`) only.

## Correctness Traps Handled
- **Strict Left-Associativity**: Evaluates `A OR B AND C` as `(A OR B) AND C`.
- **Binary NOT**: `A NOT B` evaluated as set difference `A \ B`.
- **Top-10 Document Truncation**: Output format requires `count sum [first 10 doc IDs]`.
- **Byte Tokenization**: Correctly tokenizes international and accented words into ASCII subwords.

## Further optimization opportunities

Potential follow-up work includes:
1. **Galloping Search on Posting Intersections**: Use exponential search + binary search when intersecting posting lists with high length disparity.
2. **SIMD-BP128 Bitpacking**: Compress postings using SIMD bit-packing for memory efficiency.
3. **Anchor Selection by Postings Length**: Pick the rarest phrase term (lowest document frequency) to seed candidate document lists.
