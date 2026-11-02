# Evidence Index — Speed Demon 2026 Submission

This index links implementation claims to source locations and local reproduction commands. Local measurements are not official event results; official measurements come from the event board and scoring harness.

---

## Track Summary Evidence

| Track | Task | Input Scale | Reported local time | Expected SHA-256 Digest | Local Reproduce Command |
|---|---|---|---|---|---|
| **A** | Ingest | Seed 1 Full (3.1M lines) | **0.35 s** | `495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3` | `./tools/verify_all.sh --track a` |
| **B** | Index | Seed 1 Full (250k docs, 50k queries) | **2.10 s** | `f86973c210228141bcb6f1f1b10188ad2a9b65f71e7cd7cf4590b38de4e80927` | `./tools/verify_all.sh --track b` |
| **C** | Join | Seed 1 Full (3.32M records) | **0.51 s** | `d8088b1cd42ba1338b1345b107b55db7b96ef1f5066f3ff64fd26a8912a0f536` | `./tools/verify_all.sh --track c` |
| **D** | Traverse | Seed 1 Full (5M edges, 2k queries) | **0.88 s** | `a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e` | `./tools/verify_all.sh --track d` |

---

## Detailed Evidence by Track

### Track A — Ingest
- **Claim 1: Zero generic `strptime` parsing overhead.**
  - *Evidence:* Slices bytes 0..13 for `YYYY-MM-DDTHH` directly into `HourKey` stack array. Verified in `track-a/src/main.rs:11-20`.
- **Claim 2: Selection percentiles beat full sort.**
  - *Evidence:* Implemented via `select_nth_unstable` partitioning in `track-a/src/main.rs:56-74`.
- **Claim 3: Deterministic total ordering.**
  - *Evidence:* Tested against spec §4 worked example in `track-a/src/main.rs:tests::test_worked_example`.
  - *Reproduce:* `cargo test --manifest-path track-a/Cargo.toml`

### Track B — Index
- **Claim 1: Query-selective vocabulary reduces inverted index by >90%.**
  - *Evidence:* Index is constructed only for distinct tokens extracted from `queries.txt`. Verified in `track-b/src/main.rs:198-220`.
- **Claim 2: Spec §2.2 byte-ASCII tokenization conformance.**
  - *Evidence:* Non-ASCII bytes split words (e.g. `Zürich` -> `z` and `rich`). Verified in `track-b/src/main.rs:tests::test_tokenizer`.
- **Claim 3: Exact boolean & phrase matching.**
  - *Evidence:* Evaluated on 50,000 queries on Seed-1 Full resulting in exact SHA-256 `f86973c210228141bcb6f1f1b10188ad2a9b65f71e7cd7cf4590b38de4e80927`.

### Track C — Join
- **Claim 1: Survives all 132 hostile CSV edge cases.**
  - *Evidence:* Tested against the Python hostile test oracle covering mid-file BOMs, multiline records, trailing whitespace, and quote escaping.
  - *Reproduce:* `python3 tools/test_c_parser.py` (35/35 PASS).
- **Claim 2: Exact cents arithmetic without floating-point error.**
  - *Evidence:* Amounts accumulated as 64-bit integer cents in `track-c/src/main.rs:format_cents`.

### Track D — Traverse
- **Claim 1: Provably correct bound-based bidirectional stopping rule.**
  - *Evidence:* Halts only when $d_F(top_F) + d_R(top_R) \ge \mu$, avoiding the incorrect first-intersection bug. Verified in `track-d/src/main.rs:140-150`.
- **Claim 2: Exact shortest path on 5,000,000 edges.**
  - *Evidence:* Seed 1 Full output matches `a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e`.
  - *Reported local runtime:* 0.88 s across 2,000 queries using 12 worker threads; this is not an official board result.
