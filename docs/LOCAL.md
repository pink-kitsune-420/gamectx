# Bounded local-model turns (v0.2.0a)

These opt-in APIs compose with the integration kit and existing `Context`/`Turn` APIs.
They preserve legacy packet/query/tool/session formats and use only Rust's standard
library, with Rust 1.82 minimum. The tested [local turn example](../examples/local-turn.rs)
shows required scene facts, protected whole exchanges and a bounded decision.

## Full requests, not just packets

`request::RequestBudget::new(context, output, safety)` reserves generation and headroom.
Its invariant is `full rendered input + reserved output + safety <= configured context`.
Context size is explicit; 2,048 is a starting deployment setting, not a model limit.
The implementation's 1,048,576-token guard is a work bound, not a claim about model capacity.

The host counter receives the **complete provider request**: all messages, system rules,
tool definitions, response-format grounding and history. It must use the actual deployed
chat template and tokenizer. Count the complete rendering, rather than adding separately
tokenized fragments whose boundary merges can differ. HTTP options/JSON escaping are not
necessarily prompt tokens. Native tool schemas must be counted wherever the runner renders
them; do not assume their serialized HTTP size equals their model input cost.

`CountMethod::Exact` is a trusted host assertion, not a tokenizer bundled into gamectx.
`Estimate` remains visibly estimated in every receipt. Existing byte/ratio calibration
can help estimate, but does not guarantee unseen inputs or account for hidden framing by
itself. A tokenizer/rendering callback keeps the Rust core dependency-free. Include the
exact deployment identity in calibration scope and reset/recheck after template changes.

`assess` returns fit, overflow and remaining tokens without truncating. `check` rejects an
overflowing full request. `assemble`/`assemble_prepared` reuse stable ordering and normal
priority dropping; required sections cannot drop. A renderer/counter failure aborts the
build rather than silently removing context. `Turn::build_request` uses this supplied
request contract instead of the Context's packet-only byte/token limits. Existing `build`
behavior is unchanged. Receipts include packet drops separately from request reservations.

Recheck the complete request after every tool result or correction. Cap tool result size
and enforce the reserved generation limit in the runner. A syntactically valid query or
decision can still misunderstand the player's intent; byte/token bounds cannot prove that.

## Scene hydration and dialogue

`dialogue::Thread` is scoped to one scene, conversation and supplied participants. It stores
whole user/assistant exchanges, stable request IDs and increasing turn numbers. Hosts must
supply only dialogue visible to the current actor. `select` excludes the current request
ID, tries up to `maximum` recent exchanges, and protects the newest `protect` (0..3).
If that protected tail exceeds its budget, composition fails instead of changing wording.
Repeated identical text with different IDs is not deduplicated: it can be intentional.
Empty threads contribute no dialogue messages.

`dialogue::Scene` requires current input, rules, approved outcome and a map of required
facts. Those sections remain required. Optional lore/history/narration sections drop by
priority first. If the whole request still cannot fit, older unprotected exchanges can
drop; optional lore is not restored ahead of dialogue. The protected tail never drops.
Receipts retain optional drops and kept/omitted exchange IDs, including this second stage.

`render_chat` emits native role messages: rules once as system text, retained exchanges,
and the current scene/facts/outcome/input as JSON data in the final user message. It does
not manually nest ChatML. Add native tools/format to this request in the renderer before
counting it. Keep current input in one place. Other renderers remain explicit host choices.

Facts/claims/privacy/revisions remain the responsibility of existing fact/journal/evidence
APIs and host selection. The composer preserves supplied data; it does not infer truth,
merge contradictory claims, certify provenance or automatically summarize dialogue.
`compaction_window` returns the oldest eligible prefix reaching half the measured eligible
history tokens, at whole-exchange boundaries, excluding the protected tail/current ID.
It is only a candidate window. Verify extracted commitments against source events, retain
provenance, then archive/delete in the host. Do not repeatedly summarize summaries or
turn an old observation into a current world fact.

## Small decisions and local work

`decision::Choices` accepts up to 32 explicit legal `{action,target}` pairs plus a nullable
fallback and host revision. Stable readable IDs are retained; target can be null. The
standard JSON schema includes whole-pair enum constraints so individually legal actions
and targets cannot be mixed arbitrarily. Actual runner support must be checked.

`Session::apply` parses strict JSON, validates the exact pair, compares the current host
revision and calls a live availability predicate. An invalid proposal gets at most one
correction. The second failure returns a live-approved fallback or abstention. A stale
revision skips correction. `cancel` handles a request that cannot be dispatched, such as
budget or preflight failure, without inventing a model attempt. Terminal sessions reject
further calls. Nothing here executes an action; the host must recheck and commit it
atomically through its current dispatcher/rules. A legal pair is not proof of correct intent.

Prepare candidate rows with existing views/queries; derive totals or affordability locally.
Supply those resolved facts and a tiny decision surface to the model. The existing SQL
interface remains available for questions needing it; this adds no replacement query DSL.
CLI `query - SQL` now reads bounded table JSON from stdin for local hydration without a
temporary file. Files and existing query outputs retain their behavior.

## Explicit deployment profiles

`deployment::Identity` contains model digest, weight quantization, template hash, tokenizer
identity and runner/build identity. `Profile::require_identity` compares every complete
field. Its noncryptographic scope fingerprint is only an index, never a security check.
Profiles explicitly choose context/output/safety, threads, temperature, transport and
guidance. The library neither loads models nor changes service/environment settings.

Guidance choices are `standard`, `concise`, `example` and the existing `explicit-selection`.
The first adds no text. Other text is opt-in and byte-tested against the reference; there
is no size-based default or automatic winner selection. Example guidance describes a shape
and explicitly forbids copying its illustrative target. Native tools, schema-constrained
JSON, plain JSON and prompt signatures are explicit transports, not assumptions that a
particular quantized fine-tune supports all of them. Keep the model's own chat template.

Named workload guidelines and editable profiles for 1B, 1.5B, 3B, 8B, 30B and 70B are
in [MODEL_SIZES](MODEL_SIZES.md). They are proposed starting experiments, explicitly
unmeasured, and do not automatically select guidance or assert hardware fit.

## Local evaluation

`eval/ollama_eval.py` is a standard-library, HTTP-loopback-only harness requiring `--cap 0`.
It rejects remote origins/redirects, never reads API credentials, pulls no models, unloads
none and changes no services. Models must already be installed. It records the actual
model digest/template/tokenizer/runner identity and verifies the tag at completion.

Installed weights must be local GGUF metadata; cloud/nonlocal weight descriptors are
rejected before generation.

Its frozen synthetic cases cover ownership, negation, pronoun/dialogue references,
current versus old state, locally computed affordability, and hostile quoted lore. Raw
archive JSON versus scene JSON isolates selection; scene JSON versus scene schema tests
format; concise/example conditions test guidance separately. Native tools/signatures/plain
conditions and optional narration are also available. `--seed` supplies additional states.
These are simple, short decision tasks, not a general game/story quality benchmark.

Run token-only checks first, then paired candidates using fresh output paths. The runner
randomizes condition order within each scene. One request is active at a time, with four
requested threads, CPU inference requested, one resident model kept warm, and an explicit
2K starting context. Decision output defaults to 96 tokens; optional narration uses 128.
These are trial settings, not universal limits or automatic changes to the user's server.

The harness does not bundle an exact tokenizer. It obtains prompt counts with a one-token
native preflight, checks the same pinned request against the output/safety reservation,
then dispatches. A matching prior token artifact can supply that count. Unmatched
preflights are included in complete-turn latency and token totals; they warm the prompt
cache too. Native counts are post-call observations, not proof that arbitrary oversized
prompts escaped runner truncation. Production preflight should use an actual rendered
counter; this small corpus stays well below its configured limit.

Complete-turn time includes scene compilation, native filtering, budget checks, model
calls, validation, retry/fallback and optional narration. Fixtures/oracles and registration
are prepared outside it. Streaming first text/tool availability is recorded separately
from server prefill/load/decode times. Natural prefix caching remains enabled; this is
not an isolated cold-cache speed comparison. Warmup is separately recorded. Missing
cached-token metrics remain null. Completed trials are checkpointed; interruptions retain
partial artifacts rather than changing their denominators or discarding failures.

Linux memory samples include Ollama-descendant inference runner RSS (including the
`llama-server` backend), Python host-harness RSS, system available RAM
and swap-in/out deltas. Sampling occurs between chunks, so maxima are sampled values,
not allocator peaks or uniquely owned physical pages; transient native CLI child peaks
are not included. Other platforms/missing metrics stay null. A real engine should monitor
its own process. No automatic dialogue/prose quality score is fabricated; `dialogue_score`
remains null until independently reviewed. Optional narration does not mutate game state.

`turn::report` is a new strict telemetry contract, separate from the kit's unchanged v1
recorded-run evaluator. It scores independent expected/actual JSON, counts invalid
attempts/retries/fallbacks/truncation/missing required IDs, and reports median/nearest-rank
p95 complete-turn time, TTFT coverage, usage totals and memory. Preflight is a distinct
phase; it contributes time/usage but not a decision retry or intentional output truncation.
Unknown usage makes its total null. `eval/check_local_model.py` independently audits the
cohort, raw-offer oracles, native decisions, actual supplied facts, budget counts and report.

Reproduction commands and measured results are in [the local evaluation report](LOCAL_MODEL_2026_11_06.md).
The next experiments are held-out templates/live scene transitions, a 2K/4K context sweep
when required facts justify it, and a controlled 2/4/6/8-thread comparison. Measure those
one variable at a time. Introduce an additional resident 1B analyst only after it improves
complete-turn correctness/latency within actual RAM headroom.
