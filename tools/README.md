# tools/

Standalone utilities for validating the Speed Demon track implementations.

| File | Purpose |
|---|---|
| `test_c_parser.py` | **Track C hostile-input test suite** — stdlib-only Python 3 script that tests every hostile CSV trap defined in the Track C spec (multiline fields, mid-file BOMs, mixed CRLF/LF, trailing spaces, integer-cent money, `_UNMATCHED` grouping). Run *before* trusting any parser (Python or Rust). |

## Running the test suite

```bash
python3 tools/test_c_parser.py
```

All tests must print `PASS` and the script must exit `0` before the Rust parser is considered correct.

## What the test suite covers

1. `parse_money_cents` — dollars-string → integer cents, never float  
2. `parse_csv_records` — full RFC 4180 CSV with BOM stripping  
3. Embedded newlines inside quoted fields (multiline records)  
4. UTF-8 BOM at file start **and** right after `\n` / `\r\n` terminators  
5. Mixed CRLF + LF in the same file  
6. Empty quoted fields, embedded double-quotes, embedded commas  
7. Trailing spaces in category names (significant — `"Food "` ≠ `"Food"`)  
8. `_UNMATCHED` group for rows with no lookup match, sorted last  
9. 64-bit integer sums (`id_sum`, `amount_sum`)  
10. `format_aggregates` output ordering  
