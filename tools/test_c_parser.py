#!/usr/bin/env python3
"""Track C hostile-input test suite.
Run: python3 tools/test_c_parser.py
All tests must PASS before any parser is trusted.
Each test prints PASS/FAIL and the suite exits 1 on any failure.

Covers every hostile CSV trap from the Track C spec:
  1. Multi-line quoted fields
  2. UTF-8 BOM at file start and right after a record terminator
  3. Mixed CRLF / LF terminators
  4. Trailing spaces in names are significant
  5. Money = integer cents, never float
  6. 64-bit id_sum / amount_sum
  7. _UNMATCHED group (sorted last)
  8. Edge-case fields: empty, embedded quotes, embedded commas, embedded newlines,
     leading-zero money, comma-separated thousands in money
"""

import csv
import io
import sys
from collections import defaultdict

PASS = 0
FAIL = 0


def check(name: str, got, want):
    global PASS, FAIL
    if got == want:
        print(f"PASS  {name}")
        PASS += 1
    else:
        print(f"FAIL  {name}")
        print(f"      got:  {got!r}")
        print(f"      want: {want!r}")
        FAIL += 1


# ---------------------------------------------------------------------------
# Reference parser functions
# ---------------------------------------------------------------------------

def parse_money_cents(s: str) -> int:
    """Parse '$12.34' or '$1,234.56' → integer cents.  Never uses float."""
    s = s.strip().lstrip('$').replace(',', '')
    if '.' in s:
        left, right = s.split('.', 1)
        # Pad / truncate fractional part to exactly 2 digits
        right = (right + '00')[:2]
        left = left if left else '0'
        return int(left) * 100 + int(right)
    return int(s) * 100


def strip_bom(text: str) -> str:
    """Remove UTF-8 BOM (\ufeff) only from the very start of the text."""
    if text.startswith('\ufeff'):
        return text[1:]
    return text


def remove_mid_file_boms(text: str) -> str:
    """
    The spec says BOM appears:
      • At file start (handled by strip_bom)
      • Right after a record terminator (\r\n or \n)
    We must strip those mid-file BOMs so the CSV reader never sees them
    embedded in what looks like a field value.
    """
    # Replace BOM immediately after \r\n or \n (but NOT the leading one —
    # strip_bom already handles that).
    import re
    return re.sub(r'(?<=[\n])\ufeff', '', text)


def preprocess(text: str) -> str:
    """Full BOM-stripping pre-pass before handing to the CSV reader."""
    text = strip_bom(text)
    text = remove_mid_file_boms(text)
    return text


def parse_csv_records(text: str) -> list:
    """
    Parse CSV text handling:
      • Multiline quoted fields
      • CRLF and LF terminators (mixed)
      • Embedded quotes (RFC 4180 doubling)
      • Embedded commas
      • Embedded newlines inside quoted fields
      • BOMs at file start and right after terminators
    Trailing spaces in fields are PRESERVED (per spec).
    Returns list of lists of strings.
    """
    text = preprocess(text)
    # Python's csv module handles all RFC 4180 quoting and multiline fields.
    # We use StringIO so CRLF and LF both work (universal newlines are
    # handled by csv itself when reading from a string via StringIO).
    reader = csv.reader(io.StringIO(text), dialect='excel')
    return [row for row in reader]


# ---------------------------------------------------------------------------
# Grouping / aggregation logic (the join + aggregate step)
# ---------------------------------------------------------------------------

def join_and_aggregate(transaction_records: list, lookup_records: list) -> dict:
    """
    transaction_records: list of [id, amount_str, ...] rows (no header)
    lookup_records:      list of [id, category] rows (no header)

    Returns dict { category: {'count': int, 'id_sum': int, 'amount_sum': int} }
    Rows without a matching lookup key → category '_UNMATCHED'.
    id_sum and amount_sum are Python ints (arbitrary precision ≥ 64-bit).
    """
    lookup = {}
    for row in lookup_records:
        if len(row) >= 2:
            lookup[row[0]] = row[1]

    agg = defaultdict(lambda: {'count': 0, 'id_sum': 0, 'amount_sum': 0})
    for row in transaction_records:
        if len(row) < 2:
            continue
        tid, amount_str = row[0], row[1]
        category = lookup.get(tid, '_UNMATCHED')
        cents = parse_money_cents(amount_str)
        agg[category]['count'] += 1
        agg[category]['id_sum'] += int(tid)
        agg[category]['amount_sum'] += cents

    return dict(agg)


def format_aggregates(agg: dict) -> str:
    """
    Format the aggregates dict as lines: category,count,id_sum,amount_sum
    Sorted alphabetically by category, with _UNMATCHED last.
    """
    def sort_key(cat):
        return (cat == '_UNMATCHED', cat)

    lines = []
    for cat in sorted(agg.keys(), key=sort_key):
        g = agg[cat]
        lines.append(f"{cat},{g['count']},{g['id_sum']},{g['amount_sum']}")
    return '\n'.join(lines)


# ===========================================================================
# TESTS
# ===========================================================================

# ---------------------------------------------------------------------------
# 1-5: Money parsing
# ---------------------------------------------------------------------------
check("money_basic",         parse_money_cents("$12.34"),    1234)
check("money_thousands",     parse_money_cents("$1,234.56"), 123456)
check("money_zero_cents",    parse_money_cents("$5.00"),     500)
check("money_leading_zero",  parse_money_cents("$00.05"),    5)
check("money_no_decimal",    parse_money_cents("$100"),      10000)
check("money_single_digit",  parse_money_cents("$0.01"),     1)
check("money_large",         parse_money_cents("$99,999.99"), 9999999)

# ---------------------------------------------------------------------------
# 6: strip_bom — leading BOM only
# ---------------------------------------------------------------------------
check("bom_strip_leading",
      strip_bom('\ufeffhello'),
      'hello')
check("bom_no_leading",
      strip_bom('hello'),
      'hello')

# ---------------------------------------------------------------------------
# 7: BOM right after a record terminator (mid-file BOM)
# ---------------------------------------------------------------------------
raw_mid_bom = "id,cat\n\ufeffABC,Food\n"
check("bom_mid_file_removed",
      preprocess(raw_mid_bom),
      "id,cat\nABC,Food\n")

# ---------------------------------------------------------------------------
# 8: BOM at file start (parse_csv_records round-trip)
# ---------------------------------------------------------------------------
bom_csv = '\ufeffid,amount\n1,$1.00\n'
records = parse_csv_records(bom_csv)
check("bom_file_start_parse",
      records[0][0],   # first field of header row must NOT contain BOM
      'id')

# ---------------------------------------------------------------------------
# 9: CRLF terminator
# ---------------------------------------------------------------------------
crlf_csv = "a,b\r\nc,d\r\n"
check("crlf_terminator",
      parse_csv_records(crlf_csv),
      [['a', 'b'], ['c', 'd']])

# ---------------------------------------------------------------------------
# 10: LF-only terminator
# ---------------------------------------------------------------------------
lf_csv = "a,b\nc,d\n"
check("lf_terminator",
      parse_csv_records(lf_csv),
      [['a', 'b'], ['c', 'd']])

# ---------------------------------------------------------------------------
# 11: Mixed CRLF and LF in the same file
# ---------------------------------------------------------------------------
mixed_csv = "a,b\r\nc,d\ne,f\r\n"
check("mixed_crlf_lf",
      parse_csv_records(mixed_csv),
      [['a', 'b'], ['c', 'd'], ['e', 'f']])

# ---------------------------------------------------------------------------
# 12: Empty quoted field
# ---------------------------------------------------------------------------
empty_field_csv = 'a,"",c\n'
check("empty_quoted_field",
      parse_csv_records(empty_field_csv),
      [['a', '', 'c']])

# ---------------------------------------------------------------------------
# 13: Embedded double-quote (RFC 4180 doubling)
# ---------------------------------------------------------------------------
embedded_quote_csv = 'a,"he said ""hello""",c\n'
check("embedded_quote",
      parse_csv_records(embedded_quote_csv),
      [['a', 'he said "hello"', 'c']])

# ---------------------------------------------------------------------------
# 14: Embedded comma inside quoted field
# ---------------------------------------------------------------------------
embedded_comma_csv = '"Smith, Jr.",42\n'
check("embedded_comma",
      parse_csv_records(embedded_comma_csv),
      [['Smith, Jr.', '42']])

# ---------------------------------------------------------------------------
# 15: Embedded newline inside quoted field (multiline record)
# ---------------------------------------------------------------------------
multiline_csv = '"line1\nline2",value\n'
check("embedded_newline",
      parse_csv_records(multiline_csv),
      [['line1\nline2', 'value']])

# ---------------------------------------------------------------------------
# 16: Trailing space in field is SIGNIFICANT
# ---------------------------------------------------------------------------
trailing_space_csv = '"Alice ",action\n'
rows = parse_csv_records(trailing_space_csv)
check("trailing_space_preserved",
      rows[0][0],
      'Alice ')          # must NOT be stripped to 'Alice'

# ---------------------------------------------------------------------------
# 17: Trailing space in category makes it a distinct group
# ---------------------------------------------------------------------------
txns = [['1', '$1.00'], ['2', '$2.00']]
lkp  = [['1', 'Food '], ['2', 'Food']]   # 'Food ' ≠ 'Food'
agg  = join_and_aggregate(txns, lkp)
check("trailing_space_distinct_category",
      set(agg.keys()),
      {'Food ', 'Food'})

# ---------------------------------------------------------------------------
# 18: _UNMATCHED group for rows with no lookup hit
# ---------------------------------------------------------------------------
txns2 = [['99', '$5.00'], ['1', '$1.00']]
lkp2  = [['1', 'Food']]
agg2  = join_and_aggregate(txns2, lkp2)
check("unmatched_group_exists",
      '_UNMATCHED' in agg2,
      True)
check("unmatched_count",
      agg2['_UNMATCHED']['count'],
      1)

# ---------------------------------------------------------------------------
# 19: _UNMATCHED sorts LAST in output
# ---------------------------------------------------------------------------
txns3 = [['1', '$1.00'], ['99', '$9.00'], ['2', '$2.00']]
lkp3  = [['1', 'Zeta'], ['2', 'Alpha']]
agg3  = join_and_aggregate(txns3, lkp3)
formatted = format_aggregates(agg3)
lines = formatted.splitlines()
check("unmatched_sorted_last",
      lines[-1].startswith('_UNMATCHED'),
      True)

# ---------------------------------------------------------------------------
# 20: id_sum and amount_sum are 64-bit-capable integers (no float overflow)
# ---------------------------------------------------------------------------
big_txns = [
    [str(10**15), '$99,999,999.99'],
    [str(10**15 + 1), '$0.01'],
]
big_lkp = [[str(10**15), 'BigCat'], [str(10**15 + 1), 'BigCat']]
agg_big = join_and_aggregate(big_txns, big_lkp)
check("id_sum_64bit",
      agg_big['BigCat']['id_sum'],
      10**15 + (10**15 + 1))
check("amount_sum_64bit",
      agg_big['BigCat']['amount_sum'],
      9999999999 + 1)   # 9999999999 cents + 1 cent

# ---------------------------------------------------------------------------
# 21: Money with leading zero in cents, e.g. $00.05 → 5
# ---------------------------------------------------------------------------
check("money_leading_zero_cents", parse_money_cents("$00.05"), 5)

# ---------------------------------------------------------------------------
# 22: Money with comma-separated thousands, e.g. $1,234.56 → 123456
# ---------------------------------------------------------------------------
check("money_comma_thousands_2", parse_money_cents("$1,234.56"), 123456)

# ---------------------------------------------------------------------------
# 23: BOM immediately after \r\n terminator (CRLF variant)
# ---------------------------------------------------------------------------
raw_crlf_mid_bom = "id,cat\r\n\ufeffABC,Food\r\n"
processed = preprocess(raw_crlf_mid_bom)
# After CRLF the \n triggers the regex; BOM should be gone
check("bom_after_crlf_removed",
      '\ufeff' not in processed,
      True)

# ---------------------------------------------------------------------------
# 24: Full round-trip with BOM at start AND mid-file BOM after LF
# ---------------------------------------------------------------------------
full_bom_csv = '\ufeffid,amount\n\ufeff1,$2.50\n\ufeff2,$0.50\n'
full_records = parse_csv_records(full_bom_csv)
check("full_bom_roundtrip_header",
      full_records[0],
      ['id', 'amount'])
check("full_bom_roundtrip_row1",
      full_records[1],
      ['1', '$2.50'])
check("full_bom_roundtrip_row2",
      full_records[2],
      ['2', '$0.50'])

# ---------------------------------------------------------------------------
# 25: Multiline field with CRLF inside the quoted value
# ---------------------------------------------------------------------------
multiline_crlf_csv = '"line1\r\nline2",val\n'
check("embedded_crlf_in_field",
      parse_csv_records(multiline_crlf_csv),
      [['line1\r\nline2', 'val']])

# ---------------------------------------------------------------------------
# 26: format_aggregates — alphabetical order with _UNMATCHED last
# ---------------------------------------------------------------------------
agg_ord = {'Zeta': {'count': 1, 'id_sum': 1, 'amount_sum': 100},
            'Alpha': {'count': 2, 'id_sum': 3, 'amount_sum': 200},
            '_UNMATCHED': {'count': 1, 'id_sum': 9, 'amount_sum': 50}}
out_lines = format_aggregates(agg_ord).splitlines()
check("format_order_alpha_first",  out_lines[0].startswith('Alpha'),    True)
check("format_order_zeta_second",  out_lines[1].startswith('Zeta'),     True)
check("format_order_unmatched_last", out_lines[2].startswith('_UNMATCHED'), True)

# ===========================================================================
# Summary
# ===========================================================================
print()
if FAIL > 0:
    print(f"{FAIL} FAILED, {PASS} passed")
    sys.exit(1)
else:
    print(f"All {PASS} tests passed")
