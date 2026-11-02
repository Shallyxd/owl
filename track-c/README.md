# Track C — Join (Hostile CSV & Aggregates)

Performs a relational join between 3,320,000 transaction records and region lookups, grouping by country code and computing financial sums and counts, writing exactly 240 output lines to `/out/aggregates.txt`.

## Approach in Prose

The input includes adversarial CSV cases, including multiline fields, BOMs, mixed CRLF/LF endings, and significant trailing whitespace. This Rust implementation handles them with a byte-level state machine and no external dependencies ($D=1$):

1. **Hostile-Proof State Machine**:
   A custom byte-level scanner operates over the raw buffer. It models in-quote vs out-of-quote transitions, escaped double quotes (`""`), and record delimiters without creating temporary strings or lines.
2. **Selective Byte-Level BOM Stripping**:
   BOM bytes (`0xEF, 0xBB, 0xBF`) are stripped only if they appear at index 0 of the file or immediately after a record terminator (`\r\n` or `\n`), matching the RFC specification and preserving mid-field bytes.
3. **Exact 64-bit Fixed-Point Cents**:
   Amounts are parsed into integer cents (e.g. `1420.00` -> `142000`). Totals are accumulated in 64-bit signed integers, avoiding all floating-point precision artifacts.
4. **Direct-Indexed Region Join**:
   Region mapping (`region_id` -> `country_code`) is loaded into a flat array, enabling $O(1)$ lookup per transaction record.
5. **Exact Output Formatting**:
   Emits 240 summary lines sorted alphabetically by country code, with unmatched records aggregated into `_UNMATCHED` sorting strictly last.

## Dependency List
- **External Dependencies**: None ($D = 1$). Standard library (`std`) only.

## Correctness Traps Handled
- **Multiline Notes**: Embedded newlines inside quoted fields do not terminate records.
- **Mid-File BOMs**: Non-initial BOMs inside fields are preserved verbatim.
- **Mixed Line Endings**: Correctly parses arbitrary sequences of CRLF and LF.
- **Trailing Spaces**: Trailing whitespace in name fields is significant and preserved.
- **Unmatched Group**: Records with unknown region IDs are assigned to `_UNMATCHED`.

## Further optimization opportunities

Potential follow-up work includes:
1. **AVX2 SIMD Quote & Delimiter Indexing**: Use SIMD bitmask scanning to jump across unquoted text chunks at 32 bytes per cycle.
2. **Partitioned Multithreaded Ingest**: Partition the raw CSV by scanning for record boundaries at chunk intervals, aggregating thread-local counts in parallel.
