# Track C — Performance Log

The measurements below are local results on the public seed-1 dataset. They are not official board results. No live-board run ID is recorded in this checkout; add the board run ID and measured result for each optimization step when available.

## Run 1 (Initial Correct & Optimised Implementation)
- **run-id**: Not recorded
- **approach**: Hand-rolled zero-copy byte-level streaming CSV finite state machine, direct array join on `regions.csv` lookup, 64-bit fixed-point integer cents arithmetic, and 35/35 hostile oracle verification.
- **local verification (Seed 1 Full — 3,320,000 records)**:
  - Local runtime: **0.51 s** (machine-dependent; not comparable as an official score)
  - Output SHA-256: `d8088b1cd42ba1338b1345b107b55db7b96ef1f5066f3ff64fd26a8912a0f536` (Exact Match)
  - Memory Peak: ~110 MB (within <= 150 MB Target)
- **notes**: Local output matched the recorded expected digest and passed the listed local checks.

## Mechanism & Design Decisions
1. **Hostile-Proof Byte Parser**:
   Byte-by-byte finite state machine tracking quote state, CRLF/LF terminators, and embedded newlines. No regex, no string allocation per field.
2. **Strict BOM Handling**:
   UTF-8 BOM (`\xef\xbb\xbf`) is stripped only at file start or immediately following a record terminator, preserving mid-field bytes.
3. **Direct Array Hash Join**:
   Region IDs are stored in a dense direct-indexed lookup table for $O(1)$ joins.
4. **Exact Cents Arithmetic**:
   Monetary amounts parsed and accumulated as signed 64-bit cents (`i64`), eliminating floating-point rounding errors.
5. **Deterministic Aggregates**:
   Country groups sorted by ISO country code ascending, with `_UNMATCHED` sorted strictly last.
