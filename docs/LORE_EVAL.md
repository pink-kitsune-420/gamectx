# Lore and factual-answer local evaluation (v0.3.0a)

Same installed Qwen 3.09B Q4_K_M / pinned Ollama 0.40.3 deployment as the earlier report:
CPU only, four threads, context 2048, temperature 0, seed 42. Token-only probes precede
each matching accuracy run. Every generation gets another native one-token preflight.
Model/template/runner identity and full requests/responses are retained. Every run has
`--cap 0`; all API spend is $0. No model was downloaded or substituted.

## Retained results

Scores check the selected ID against independently declared acceptable IDs, including
explicit unknown outcomes. They distinguish source/intent selection from source validity.
Local unknown paths call no model. Rendered prose comes from the selected host card;
an unrelated true card is still a wrong answer. Generic source validity alone is not
scored as semantic success.

| Cohort | Correct | Local unknown paths | Model decisions | Total chat calls | Generation input/output tokens |
|---|---:|---:|---:|---:|---:|
| Initial Python prototype, v1, 12 questions | 9/12 | 3 | 9 | 18 | 1191 / 64 |
| Narrowed Python prototype, v2 | 10/12 | 5 | 7 | 14 | 797 / 51 |
| Native workflow, v3, 12 questions | 11/12 | 5 | 7 | 14 | 789 / 53 |
| First 20 fresh questions, v3 | 17/20 | 6 | 14 | 28 | 1736 / 105 |
| Same 20 questions after routing fixes, v4 | 20/20 | 10 | 10 | 20 | 1211 / 77 |
| Eight additional questions, v4 | 7/8 | 3 | 5 | 10 | 597 / 36 |
| Final “Did I…” repair, v5 | 1/1 | 1 | 0 | 0 | 0 / 0 |

Each token-only run used one probe per nonlocal case: 9, 7, 7, 14, 10, 5 and 0 calls.
The table's accuracy calls include each native preflight and generation. Input/output
columns count generation usage only. There were zero corrections in these runs.

The weather target changed from a particular location fact in v1 to explicit unknown
in v2 because current weather is unobserved. The v1 weather abstention was safe despite
missing its original strict ID target. The salve acceptable-source set also changed when
an explicit permission card was added. Accordingly, **9/12→10/12 is not a clean paired
accuracy claim**. Keep each original manifest/gold/result unchanged.

The initial pilots exposed an unrelated salve price answer and unnecessary abstentions
for seal ownership, weather and tonic use. Narrowing topics fixed some choices but a
broad “does” rule initially treated “Who does the oil belong to?” as a use question.
That error was corrected before the native v3 cohort.

The first fresh batch tested ownership after purchase, actual coins despite a hypothetical
balance, exact/corrected names, original home quotes, missing birthplace/age/family details,
unprovided item ingredients, private NPC biography, missing memories and dates, false
memory suggestions, and a gated inscription. Failures were “say/tell” aliases and an
unrelated name quote offered for a claim about being a sailor. The v4 candidate policy
fixed those routes; the 20/20 rerun is now regression evidence, not an untouched holdout.

The additional eight questions include childhood, age phrasing, name paraphrases, alleged
sailing disclosure, salve ownership, a decorative-purpose premise, an unspoken story and
current sky visibility. “Did I mention sailing?” incorrectly selected the merchant's
background. The final host rule restricts “Did I…” disclosure questions to player sources;
its one-case repair returns unknown with zero model calls. The original 7/8 stays visible.
No further independent language-generalization score is claimed after that repair.

The native 12-question cohort still abstains unnecessarily on the tonic question. This
is safe, but less informative than the supplied implemented-use fact. Do not describe the
pipeline as perfect, or confuse exact host-rendered text with perfect question interpretation.

## Turn timing and memory

Native turn timers begin before host question hydration and include model identity
checks, native preflight, generation, source validation and conversation completion.
Scenario setup and the extra after-turn public-state audit are excluded. Python-only
pilot timers are not interchangeable with native timings. These small sequential runs
have natural cache/system-load effects and do not isolate a format-only speedup.

| Native cohort | Median turn µs | p95 turn µs |
|---|---:|---:|
| v3, 12 questions | 1149231 | 1984676 |
| v3, 20 fresh questions | 1489670 | 2584483 |
| v4, 20-question rerun | 563910 | 2265634 |
| v4, eight additional questions | 1473938 | 2739195 |

Medians/p95 include local paths; the rerun has ten zero-model unknown paths. This mix
changes latency materially. It is not a comparison at equal model-call count. Missing
per-turn runner RSS remains null in the original artifacts.

A **separate post-run warm sample**, `eval/lore-memory.json`, reports runner RSS
2180976640 bytes, Rust host RSS 4694016 bytes and evaluation-process RSS 23695360 bytes.
It records resident model metadata and system availability/cumulative swap counters.
This is one sample, not peak or per-turn memory, and does not establish an 8 GB deployment
ceiling. The sampler uses the existing backend-aware process ownership logic. It makes
zero model calls and spends $0 API.

## Reproduce/audit

`eval/check_lore_model.py` checks every retained artifact against its frozen Python
candidate policy, reconstructs schemas and source rendering, verifies strict model replies,
expected IDs, usage/reservations, call phases and identity settings. Historical v1–v4
policies stay in versioned reference files, including their failures. Current Rust matches
the current reference on the independent native example workflows. CI audits saved data
without contacting a model. The original user-supplied prose remains separately preserved
in [DUNGEON_FEEDBACK](DUNGEON_FEEDBACK.md).

```sh
cargo build --example dungeon --locked
python3 -I eval/lore_native_eval.py --cap 0 --cohort extra --tokens-only --output eval/results/lore-tokens.json
python3 -I eval/lore_native_eval.py --cap 0 --cohort extra --tokens eval/results/lore-tokens.json --output eval/results/lore-results.json
python3 -I eval/check_lore_model.py --cap 0 eval/results/lore-tokens.json eval/results/lore-results.json
```

Use fresh paths. Cohorts are regression, fresh, extra and repair; previously tuned questions
must remain identified as such. All run loops cap chat-call reservations (96 native, 64
prototype), including preflights and failed dispatches. Hosted credentials are never read.

## Stock routing and natural speech diagnostics

The user greeting “hello, what do you have for sale?” previously selected seal lore.
The example now recognizes stock paraphrases before quote recall and purchase offers,
and explicitly closes the lore scope for stock-only answers. Eight regression stock
paraphrases select stock 8/8 offline (previous policy 1/8). Independent native/Python
checks cover 96 before/after-purchase workflows across six seeds; 72 additional native
natural requests match the prototype across those seeds and both lore versions.

The stock cohort on the installed local Qwen2.5 3.09B Q4_K_M scores **10/12** expected
answer IDs: all eight listing paraphrases, the post-purchase listing and player-owned
salve case pass. Keeper-stock and seal-sale abstain unnecessarily. Token run: 12 calls;
accuracy run: 24 calls, no retries; each spends **$0 API**, with `--cap 0`.
Raw records: `eval/lore-stock-{tokens,results}.json`.

Two natural-speech prototypes each test ten questions against text and structured lore:
20 token-only calls followed by 40 accuracy-stage calls (preflight + generation), each
run **$0 API** with `--cap 0`. The installed model, CPU=4, GPU=0, context=2048,
temperature=0 and seed=42 are retained in each artifact. Both prototypes yield 20/20
structurally valid replies, but semantic scores are deliberately null: JSON acceptance
is not factual accuracy. Manual inspection finds invented homes, tonic healing, unavailable
salve sales and seal loans. No retries or semantic correction layer was evaluated.

`natural-*.json` preserves the broad first prototype; `natural-v2-*.json` preserves the
narrowed second prototype. Aggregate structured-world request bytes are 20829 and 9962
respectively. Changes include instructions and selection together; this is not a controlled
format-only comparison. Text lore also fails factual grounding. Keep natural mode opt-in.
The trials shared a local runner during part of execution, exclude native host hydration,
and have no peak-RAM samples; their timings establish no latency or memory improvement.

Offline audits reconstruct every request and validate responses/call accounting without
Ollama. `eval/check_natural_model.py` covers both prototype versions; the existing lore
audit retains all old pilots and now includes the stock cohort.
