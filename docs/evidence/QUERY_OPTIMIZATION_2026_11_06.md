# Query execution, identifier lookup and optional guidance — 2026-11-06

The local query executor avoids copying unused row data. Table/catalog relevance now
recognizes namespace, dotted and path components. An optional explicit-selection prompt
profile is available; standard guidance remains the default because the model result
was mixed and cost more tokens.

## Paired hosted check

The exact additional paragraph was frozen before requests. Both profiles used the same
six fresh states: cargo 12101/12102 (24/48 rows), teams 12201/12202 (16/32), paths
12301/12302 (20/40). These are fresh row seeds using the previous schema families and
eight question templates per family, not unseen genres or question templates.
Each model/profile receives 48 questions. Profile order alternates by question.

Both profiles use schema/host meanings, the same bounded native query tool and up to one
correction on an error, followed by a final-answer call when execution succeeds. The
explicit profile appends one paragraph to the system prompt; no query rewrite, host
shape contract or answer repair is applied. The default DESCRIPTION, SQL rules and
session envelopes are unchanged. Strict output extraction and scoring reuse the frozen
release evaluator. Lists are unordered but duplicate-sensitive; numbers use its existing
relative tolerance. This is a small synthetic tabular test, not live gameplay.

| Model | Standard first / final | Explicit first / final | Average complete-exchange input standard / explicit |
|---|---:|---:|---:|
| llama-3.2-3b | 40/48 / 46/48 | 42/48 / 48/48 | 956.771 / 1136.938 |
| mistral-small-3-2-24b-instruct | 46/48 / 47/48 | 42/48 / 46/48 | 858.958 / 1078.667 |
| Both | 86/96 / 93/96 | 84/96 / 94/96 | 907.865 / 1107.802 |

Executed-query and final-answer scores are identical: no successful result was copied
incorrectly. Two paired answers improved and one regressed. The extra instructions
removed Llama's unrequested LIMIT 1 on both cargo list questions. Mistral still added an
unrequested `access = 'open'` filter for both top-three route totals with explicit
guidance, versus one state with standard guidance. This supports testing per-model
configuration, not a claim that every weaker model improves.

| Family | Standard final | Explicit final |
|---|---:|---:|
| cargo | 30/32 | 32/32 |
| teams | 32/32 | 32/32 |
| paths | 31/32 | 30/32 |

Complete-exchange input rose 22.0%. Usage-priced completed-exchange cost was
$0.0129111625 standard versus $0.0151921500 explicit. First-query
accuracy fell overall, 86/96→84/96; corrections recovered 93/96→94/96. One extra final
success and mixed per-model outcomes do not justify a universal default change.
`Guidance::ExplicitSelection` is opt-in for hosts that accept the measured tradeoff;
`Guidance::Standard` is the default. No automatic model-size rule is used.

Token-only inputs were 390→475 (3B) and 388→468 (24B). Token check cap $0.01,
spend $0.0012702000; accuracy cap $0.08,
spend $0.0281033125. Total $0.0293735125.
Both runs completed; no ambiguous or interrupted calls. Only the two smaller default
models were queried. All replies, usage and price snapshots are retained in
[raw accuracy results](../../eval/query/query-guidance-results.json) and
[token results](../../eval/query/query-guidance-tokens.json).

## Native execution and lookup

[Five paired release timing fixtures](../../eval/README.md#query-execution-and-identifier-matching)
measured 4.6–32.5× faster parsed execution with fewer allocations. The wide 1,024-row
sum fixture, 1,000 calls, went 1,277,473,628→53,773,386 ns; allocations/query
9,229→11 and peak transient bytes 1,355,791→8,211. These synthetic payloads are
1,024 bytes per row; tiny rows use eight bytes. This is local execution time, excluding
parsing on both sides, not model latency or universal speedup.

Ten lexical identifier regressions improved 2/10→10/10; four catalog cases agree with
Python. Boundaries `: . / - _` preserve full cells/segments and Unicode lowercasing.
Selected IDs, Unicode text and large-number spellings are retained. Numeric arithmetic
and comparisons still use the documented 64-bit float rules.

Query plans expose structure. Optional exact host shapes and row/result-byte bounds are
available for structured requests, not inferred intent or permissions. An oracle-shape
diagnostic rejected 27/96 old requests including 22 correct equivalents. Automatic
flattening of IN/top-N subqueries is also rejected: `[10,9,9,9]` yields top-three sum 28
but IN-membership sum 37. Both findings are retained in the roadmap.

## Reproduce offline

```bash
cargo build --release --locked
rustc --edition=2021 -O eval/query/query_plan_check.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/query-plan-check
rustc --edition=2021 -O eval/query/query_compare.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/query-compare
python3 -I eval/query/test_query_upgrade.py
python3 -I eval/query/query_optimization_check.py
python3 -I eval/query/relevance_eval.py
python3 -I eval/query/query_upgrade_eval.py --check eval/query/query-guidance-results.json --cap 0
python3 -I eval/query/summarize_query_upgrade.py --check
```

Native checks cover 2,024 alpha.4/current result/error comparisons, 3,000 generated
Rust/Python queries, 516 plan/bounds/shape comparisons, 209 fresh feedback envelopes and
105 older feedback envelopes. The supplemental native profile exactly matches the tested
Python paragraph. Thirty-five new conformance cases bring the total to 401; all 366
previous outputs remain byte-identical. Added 20,000 text mutations exercise profiles,
contracts, plans and identifier text. Rust stable and 1.82 checks pass.

Paid repetitions require new output paths, token-only first, with explicit caps:

```bash
python3 -I eval/query/query_upgrade_eval.py --models --tokens-only --cap 0.01 --output eval/results/query-guidance-tokens-new.json
python3 -I eval/query/query_upgrade_eval.py --models --cap 0.08 --output eval/results/query-guidance-results-new.json
```

Reservations checkpoint before each single transport attempt; unknown usage retains its
reservation. A resume cap includes all prior cumulative spend. The report checker audits
frozen evaluator hash, cohort identities, prices, ledger totals and score recomputation.
The older hosted benchmark and evaluator remain frozen.
