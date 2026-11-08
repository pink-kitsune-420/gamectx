# Turn-aware speech compiler and fresh local follow-up

The new version-2 speech path addresses v1's creative-invitation, schema-wall and
first-person subject ambiguity without replacing its artifacts or legacy formats.
`Life::context_for_turn` and `Life::compile_speech` use host-declared profile, known-state,
speaker, question subject and individual subject-scoped facts. The existing v1 context
methods retain their byte contracts. The dungeon life demo and `character-speech` example
use the new path; applications opt in explicitly.

## Deterministic behavior

Focused profiles derived from 1B/1.5B/3B dialogue hints suppress balanced improvisation.
Unknown and partial factual turns suppress it at every tier, including explicitly creative
turns. Expanded known turns, or explicitly creative known turns, may use authored balanced
expression only when the feature is enabled. The compact directive is:

> State only the supplied facts, attitude, and unknown status in one short sentence;
> do not offer unlisted stock or speculate about unknown weather/history.

Known-state is explicit host metadata, not inferred by English keywords or selected by a
model. A `known` turn cannot contain an unknown fact; `partial` requires both kinds and
`unknown` requires at least one unknown. Fact subjects must be approved setting entities;
speaker must be in the current roster. Keys are unique, reserved metadata keys cannot be
shadowed, and unknown status requires null rather than an invented value. Subject-scoped
unknowns render as `player_birthplace:"unknown_to_merchant"` with `question_subject:"player"`,
`utterance_speaker:"player"` and a fact-to-subject map. This declares NPC knowledge, not that
the canonical birthplace does not exist. Source visibility and truth remain host duties.

Stable rules/character/scene prefix precedes changing context, whole selected exchanges,
subject instructions and the final user line `Current request:\n…` outside nested JSON.
Full native requests must be rendered and counted again. V2 does not register tools or
change execution permissions. `Coordinator::select_context_for_turn` provides a v2 packet;
scene facts and question facts share the fact cap, and inspection remains host-only.

`assess_sentence` reports character count, terminal punctuation, a single punctuation-run
sentence, schema-wall equality, over-limit text and suspected wall cutoff. It never trims,
repairs, appends punctuation, retries a model or hides raw text. Decimal points are handled;
quotes and several terminal punctuation forms are recognized. Abbreviations, fragments and
stylistic punctuation can fool this mechanical heuristic. It is neither grammatical nor
semantic validation. A full sentence exactly at 240 characters is distinguished from an
unfinished clause at the same cap. Hosts may require the diagnostics before queueing speech;
interrupted/heard prefixes remain separate delivery events.

An independent Python prototype precedes the Rust implementation. Rust matches its context
and provider-neutral request bytes on **32 compiler workflows**, with **16 sentence/bound
checks**, malformed cases, persistent CLI/native checks and mutation testing. No old expected
output is blessed. See `eval/character/life_speech_cases.py`, `life_speech_reference.py` and the tested
`examples/character/character-speech.rs`.

## Fresh paired local diagnostic

The three v1 `eval/life-model-{tokens,results,review}.json` files stay byte-identical and are
SHA-256-bound in every v2 artifact. The installed model/digest, GGUF quantization, tokenizer/
template identity and runner match v1. Fresh v1-style replies reproduce all **4/4** original
candidate raw responses exactly. The follow-up uses those four regression questions plus
four predeclared fresh holdouts, paired with the v2 compiler. The holdouts use new phrasing
for origin, partial scene knowledge, wick preference and cautious trust. There is one
sample per arm per case; these are not independent sessions or a roleplay benchmark.

The character facts, model, sampling and 240-character schema remain matched. V2 changes
policy, subject representation and rendering together, so no individual effect is isolated.
The short-sentence instruction asks for under 120 characters; it does not silently increase
the schema wall. Context **2,048**, CPU threads **4**, GPU **0**, temperature **0**, seed **42**,
reserved generation **96**, safety **256**. All 16 token-only checks precede generation;
each of 16 generation turns repeats its own actual preflight. **48 local calls**, **$0 API**,
no retries, downloads, additional model loads or service-setting changes. All complete
requests fit, including output/safety reservation. Raw chunks, first text, complete-turn
latency, input/output tokens, runner/harness RSS, system memory and swap remain recorded.

| Mechanical result / 8 replies | Fresh v1 candidate | V2 candidate |
|---|---:|---:|
| Valid response shape | 8/8 | 8/8 |
| Ends at a terminal punctuation boundary | 5/8 | 8/8 |
| Single sentence by punctuation heuristic | 0/8 | 5/8 |
| At 240-character wall | 3/8 | 0/8 |
| Suspected mid-clause wall cutoff | 3/8 | 0/8 |
| Under instructed 120 characters | 0/8 | 7/8 |
| Input tokens including per-turn preflights | 6,670 | 7,178 |
| Output tokens including preflights | 460 | 199 |
| Median complete diagnostic turn, microseconds | 9,605,727.5 | 8,659,833 |

Regression endings improve **2/4 → 4/4**; holdout endings **3/4 → 4/4**. Both v2 origin
replies acknowledge unknown information without inventing a birthplace; one adds a generic
follow-up rather than explicitly naming the player attribute. This does not prove that a
small model can never confuse subjects.

Remaining failures are retained: v2's partial regression says current sky is unknown and
then claims it is clear; the amber-wick holdout introduces an unsupplied standard option
and dark blue wick. Three v2 replies violate the one-sentence heuristic; one exceeds the
120-character instruction while still below the schema cap. The partial holdout says no
clouds are visible from here, consistent with occlusion but without explaining it.

`life-model-v2-review.json` labels assistant manual observations separately from mechanical
metrics and leaves independent human review null. No model judge, low token count or
punctuation score establishes grounding. Input grew with explicit metadata even though
output shrank. Timings have uncontrolled desktop/cache effects and eight samples per arm;
no production latency, cache-hit or general accuracy improvement follows. There was no
retuned rerun. Source-selected factual answers remain the default when correctness matters.

Artifacts: `eval/character/life-model-v2-tokens.json`, `life-model-v2-results.json`,
`life-model-v2-review.json`. The v1 artifacts, scripts and unsuccessful experiments remain
available. Audit with `life_model_v2_eval.py --cap 0 --check FILE` and
`life_model_v2_review.py --cap 0 --check`.
