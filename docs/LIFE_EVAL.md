# Dynamic character evidence and retained limits

All new paid/API spend is **$0**. No additional model was downloaded or loaded, and no
service configuration changed. The only live diagnostic used the already installed local
Qwen2.5-derived 3B Q4_K_M variant; raw identity, tokenizer/template hashes, runner version,
hardware, per-call requests, sampled memory, first-text timing and usage are retained.

## Deterministic checks

| Independent Python/Rust workload | Workflows |
|---|---:|
| Life definitions, appraisals, disabled/private/nonhuman behavior | 52 |
| Scene dependencies, parallel topics, partial delivery and replay | 42 |
| Checkpoints, archive backpressure and explicit consolidation ranges | 20 |
| Opinions, commitments, accepted promises, event tokens and initiative quotas | 80 |
| Focused/expanded context, six size hints and required-fact failures | 27 |
| Total | 221 |

The episodic workload includes 1,000 events across ten bounded working windows and
cross-session restores. Routine scheduling checks 96 independent character scopes with
16 jobs per tick over six ticks, zero model calls. These are exact host contracts, not
semantic dialogue tests. Four loop types have additional Rust coverage: greeting duplication,
NPC response depth, failure receipts/backoff, and an accepted promise fulfilled before
proposal dispatch. Consequential events require explicit host receipts.

There are **47** additive conformance cases; all **842** previous cases remain
unchanged (889 total). New fixture families include malformed/error cases. Mutation tests
cover proposals, definitions, saved states, coordinator input, initiative and the existing
full runtime parsers. Structural validation adds 99 independent number/depth/size/duplicate
checks, including wide objects and invalid directly constructed Rust `Value::Number` values.
The parser's exact numeric spelling and serialization outputs remain unchanged.

Persistent host operations share **39** independent fixture replies through Python CLI,
Python native, Node and C# native; Python performs **78** fixture checks. C# was tested with
the installed .NET 10 SDK on Linux over the existing ABI. Notebook recovery also retains
accepted promise states, consumed offer IDs and erased-note sequences. Actual Unity/Godot,
mobile, network authentication and external speech systems are not exercised. Twenty-four
new scaled natural requests exercise 1B through 70B hints without any model call; historical
natural/source requests and evaluations remain frozen.

## Synthetic host performance

`eval/life_bench.rs` compares the exact pre-optimization commit `fedab27` with the current
implementation. Each arm ingests 128 original outcome events and applies 128 authored
appraisals; setup is outside the timer. Seven alternating paired process runs verify
identical final serialized state/fingerprint. Exact medians and allocator counts are in
`eval/life-performance.json`; README/CHANGELOG quote those recorded figures. This measures
one CPU/allocator workload, not a model, game frame deadline, or player-facing turn.

`eval/validation_alloc.rs` contrasts 1,000 validations of a 128-row value with the prior
serialize/parse method, using an instrumented standard allocator. The new validator makes
zero heap allocations. It also uses fixed-stack sorted key blocks for wide objects; the
99-check oracle includes 20,000-field inputs and duplicates across blocks. New `to_json`
reserves `json_len()` bytes. Life updates clone only a new event or affected attitude and
preflight exact inspection size with checkpoint overhead, rather than copying full state.

## Retained local speech diagnostic

Four original scenario families each receive baseline and character-context requests:
a known polished lamp plus unknown current sky, implicit wick preference, partial trust
after an intact return, and an unknown player birthplace. Required host facts are matched;
optional context differs. Gold expectations precede generation. This is a narrow diagnostic,
not a 100-turn live roleplay or long-session benchmark. There was no tuning rerun.

Eight token-only calls precede the generation run. Each of eight generated turns includes
its own preflight plus generation: **16** further calls, **24** total. Every full request fits
2,048 context tokens after reserving 96 output and 256 safety tokens. CPU threads 4, GPU 0,
temperature 0, seed 42; no unload, pull, runner restart or setting change. Human review is
**unknown**. `life-model-review.json` explicitly labels assistant review against host
expectations, separate from shape checks.

- Shape validation: **8/8**, which does not prove grounding or sentence completeness.
- Both variants invented birthplace details and answered about the speaker instead of
  the player. The candidate invented clouds after correctly stating current sky uncertainty.
- The candidate's partial-answer and preference replies end unfinished. Its preference
  offer implies available wick stock that the host did not supply; the baseline asserts
  unsupported popularity. Both opinion replies express limited easing of caution.
- All failures, original responses and unused human-review fields remain retained. No
  second model judge or JSON score is presented as semantic success.

Across four turns per arm, including their preflights, baseline input is **1,522 tokens**,
candidate input **3,330**; output is **132 / 231** respectively. Median complete diagnostic
turns are **9,736,693 / 13,329,718 microseconds**. Sampled runner RSS peaks are
**2,218,418,176 / 2,240,212,992 bytes**; harness RSS peaks are **25,452,544 / 25,407,488**.
These are sampled process values under uncontrolled desktop CPU/cache load, not allocator
peaks or causal comparisons. They do not measure Rust engine RSS. System memory and swap
observations remain per-record; missing values stay null. No speed, accuracy or cache-hit
improvement is claimed. Stable-prefix rendering is deterministic ordering only.

Artifacts: `eval/life-model-tokens.json`, `life-model-results.json`, `life-model-review.json`.
Audit with `python3 -I eval/life_model_eval.py --cap 0 --check FILE`. Default factual gameplay
continues to use source-selected answers. Free-form speech and larger size profiles remain
opt-in/experimental. Fresh continuous sessions and independent human review remain next work.

## Completed validation

Root formatting, strict all-target Clippy, locked tests and warning-free rustdoc pass.
Rust 1.82 root/native tests and the dungeon example pass; native formatting/Clippy/tests,
package verification, 93 offline main-job CI commands (90 in the batch plus three separately
run audits), 23 binding CI commands and eight C# fixture suites pass. The C# suites total
458 fixture replies. There are 889 conformance cases, including the final permission-epoch
rollback regression; all 483 previously tracked `expected.json` files are byte-identical.
Local-reference/credential and Markdown-link sweeps report no matches or broken links.
The immutable native/project ABI and zero-third-party-dependency policy remain unchanged.
