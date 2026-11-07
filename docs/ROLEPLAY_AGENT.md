# Dynamic character implementation agent brief

Use this brief as the task for an implementation agent working in the gamectx repository.
Complete the five stages in order. Build optional character continuity that supports
free-form conversation and long gameplay sessions on modest local hardware, while
preserving host authority over the world. Deliver working APIs, bindings, an original
dungeon demonstration, evidence and documentation for every stage.

This is an implementation assignment. The five stages below are planned work, not
capabilities established by this brief. Do not stop after producing another plan.

## Establish the current state

Read `CLAUDE.md`, `README.md`, `docs/ROADMAP.md`, `docs/CHARACTERS.md`,
`docs/CONVERSATION_PATCH_EVAL.md`, `docs/KIT.md`, `docs/ADVERSARIAL_EVAL.md`,
`docs/CONVERSATION.md`, `docs/INTERACTIONS.md`, `docs/ROUTINES.md`, and the relevant
format, integration and evaluation documentation. Read
`../low-model-optimization-handbook/index.html` for deployment context.

Inspect git status, recent commits, applicable repository instructions and existing
APIs before editing. Preserve unrelated changes. The starting reference when this brief
was written was `3f5efba`, following `bf9365b`, `2561e76` and `c0a14b5`; verify the
actual checkout instead of assuming it is unchanged. Do not reimplement shipped work.

Reuse these foundations after checking their actual contracts:

| Existing area | Intended reuse |
|---|---|
| CharacterTools, sessions, bindings and scoped views | Feature integration, authorized context and dispatch |
| Conversation, Thread, Scene, lore and answers | Bounded exchanges, original quotations, protected facts and factual rendering |
| Memory, Journal, EvidenceMemory, FactRegistry and RecallIndex | Provenance, recall, hypotheses, dependencies and corrections |
| InteractionTools and decision validation | Witnessed events, promises, relationship notes, clarification and recipient-bound offers |
| Routines, timelines, observations and typed time assessment | Local activities, actual experiences and source-based time knowledge |
| RequestGate, scheduling, synchronization and outcome helpers | Async freshness, one-use tickets, work budgets and confirmed results |
| Kit inspection, evaluation and full-request budgeting | Debugging, independent checks and provider-aware accounting |

Source availability and valid JSON do not prove that generated prose follows its
sources. Existing free-form speech experiments contain retained failures. The recorded
25/32 correct source/time classifications are a narrow diagnostic, not a roleplay or
long-session benchmark. Preserve historical artifacts and unsuccessful experiments.

## Constraints that apply to every stage

- Implement in zero-dependency Rust and preserve Rust 1.82 support. Keep the immutable
  project/native ABI and existing formats intact; add versioned contracts where needed.
- All new features default off. Support shared defaults and per-character overrides.
  Keep configuration, requested tool registration, per-turn exposure, data visibility
  and live execution permission separate. Do not add a Cargo feature per tool without
  a concrete technical reason.
- Register only requested tools and expose only relevant, usable tools. Reject disabled,
  unexposed, guessed, stale, foreign-scope and replayed calls. Recheck current revision,
  prerequisites and permissions at dispatch. More context does not enable more tools.
- Models cannot grant knowledge, enable capabilities, confirm their own proposals or
  directly execute engine actions. A host may explicitly preauthorize appropriate
  actions; do not require human confirmation for every routine action.
- Bound definitions, text, histories, retrieval, tools/results, proposals, pending work,
  replanning, initiative, persistence and model calls. Disabled features should require
  minimal work and zero model calls. Reject work that exceeds a bound predictably.
- The deployment target has four physical CPU cores, approximately 8 GB RAM and an
  installed Qwen2.5-derived 3B Q4_K_M model through Ollama. Do not download or load
  additional models without authorization. Do not change runner settings silently.
- Keep the core provider independent. Parameter count establishes neither capability
  nor useful context capacity. Treat 2K as a starting configuration; measure changes.
- Prefer one resident model and one focused task per call. Reuse deterministic host
  state for background updates. Do not introduce mandatory planner, critic, reflection,
  embedding, speech or vision models.
- Use only public issue discussions, user-facing documentation, research and model
  cards for comparison. Do not copy, adapt or port other projects' implementation code,
  prompts, biographies, game content or assets. Do not inspect their implementation
  files. Use independently designed algorithms and original example content.
- Keep movement, sensors, pathfinding, calendars, physical simulation, action execution,
  completion confirmation and authoritative world rules in the host.

## Stage 1 Life definitions opinions and relationships

Add strict, bounded, versioned character definitions and mutable character state.
Share immutable definitions where practical. Provide editable original example files
and a tested minimal Rust integration.

Keep three layers distinct: approved starting definitions, evolving subjective state
and source-attributed experiences. Do not use one growing biography as all three.

Support optional author-defined data for:

- Identity, background, values, voice, temperament, personal boundaries and practical
  knowledge. Support nonhuman characters and host-defined calendars and societies.
- Preferences, opinions, reasons, tensions and conditions that could change a view.
- Relationships that distinguish familiarity, trust, affection, resentment, obligations,
  shared experiences and unresolved disagreements. Existing relationship notes remain
  reusable; do not replace them with a duplicate reporting system.
- Short and long goals, commitments, unfinished matters and competing priorities.
- Beliefs, reports, hypotheses, misconceptions and explicit unknowns, separate from
  canonical truth. Retain supporting and contradicting sources and corrections.
- Configured social conventions, group membership, reputation, public information,
  local etiquette and rules for information propagation.
- Relevant physical or emotional state, using host-defined meanings and bounded update
  rules. Separate temporary reactions from lasting attitudes. Numeric design parameters
  are not calibrated psychological measurements or probabilities.
- Canonical lore, private information and the author's permitted scope of improvisation.

Opinions must be able to influence attention, dialogue, initiative and activity proposals.
Use bounded host-controlled appraisal and change rules with reasons, event dependencies
and protection against abrupt reversals. The same experience may affect characters
differently. A model may propose changes; the host validates and applies them. Model
prose alone must not rewrite personality, permissions, relationships or world facts.

Provide an optional life-file drafting workflow for authoring or character creation.
Validate drafts against host setting constraints, entity IDs, calendars, visibility,
size limits and permitted history. Host acceptance makes selected background canonical;
the model cannot approve its own draft. Conflicting biographies must not establish
incompatible shared events. Do not generate fixed future player choices or treat
imagined events as confirmed gameplay. A hand-authored file must work without a model.

Include an opt-in balanced conversation policy. Preserve existing default behavior and
formats. Allow opinions, tastes, humor, hypotheticals, imagination and permitted flavor
without requiring factual citations for every expressive sentence. Distinguish these
from factual claims, attributed reports, tentative inference, character belief and
world-changing proposals. Deception and mistaken beliefs require explicit host policy.

For incomplete knowledge, answer the known part and express the relevant uncertainty
naturally. Ask a useful question or propose an available next step when appropriate;
do not turn every unknown into a refusal or every reply into a clarification question.
Do not use the word "perhaps" or a fixed prefix as a hallucination guarantee. Do not
promote invented flavor into lore or future memories of actual events. High-consequence
state and transaction results still come from validated host data.

Stage acceptance: Python/Rust parity, additive conformance and malformed-input cases,
deterministic state transitions, evidence-based opinion changes, conflicting beliefs,
nonhuman definitions, disabled behavior, scope isolation and original editable examples.
Evaluate actual conversational usefulness separately from these deterministic checks.

## Stage 2 Conversation and scene continuity

Add an optional coordinator that joins current character state, perception, conversation
and available actions. Extend the existing architecture instead of introducing a second
tool/session framework.

Track active topic, unanswered questions, promises, unfinished matters, intended
listener, participants, shared context and referents. Reuse existing reference resolution
and clarification. Unsupported free-form intent must not be silently converted into an
unrelated legal command. Avoid a universal English keyword parser or mandatory topic menu.

Capture current scene facts with explicit sources, visibility, age and revision.
Distinguish global truth, this NPC's perceptions and what participants mutually know.
Include only relevant affordances, objects, routes, resources, interruptions and social
expectations. Location, indoor status or sunny weather alone do not establish a sensory
observation. Player speech is an attributed statement, not proof of its contents.

Handle entering and leaving scenes, changing audiences, two concurrent conversations,
emergencies, blocked actions and delayed results. Reuse RequestGate and existing revision
checks. Define which dependencies invalidate a pending reply or action; test harmless
updates as well as consequential changes so unrelated activity cannot starve dialogue.

Represent generated, validated, queued, played, heard, interrupted and cancelled speech
as distinct lifecycle events where the host supplies the corresponding acknowledgements.
Record only the portion and audience the host confirms. A generated response is not proof
that it was delivered or heard. Provide optional text, streaming and external STT/TTS
interfaces, cancellation and barge-in handling without requiring any speech dependency.
Do not execute actions from interim recognition results. Keep perceived input uncertainty
and clarification available for names and ambiguous utterances.

Assemble focused context from the current activity, next relevant commitment, immediate
goal, relevant character traits and relationship state, required facts, unresolved topic,
a few observations and selected memories. Permit explicitly configured expanded context.
Count the entire provider-rendered request: templates, tools, history, tool results,
corrections, retries, reserved output and safety margin. Recount after changes. Preserve
required facts or fail explicitly. Do not assume a cached count survives a changed request.

Stage acceptance: fresh scene changes, addressed and overheard speech, interrupted and
undelivered lines, stale proposals, replay, pronouns, topic resumption, harmless concurrent
changes and quoted instructions. Include a continuous original merchant scenario and
test both text delivery and fake speech transports without installing voice models.

## Stage 3 Durable episodic memory

Add a host storage interface and bounded, recoverable character checkpoints using the
existing memory, evidence and observation contracts. Provide a standard-library-only
reference adapter or example; hosts retain storage authentication and transaction duties.

Bind persistence to campaign, save branch, character instance, participants, schema,
definition identity and revision. Preserve replay watermarks. Loading an earlier save
must restore its corresponding history. Reject invalid identities, future/stale records,
partial or oversized snapshots and conflicting same-revision state predictably.

Separate bounded working memory from host-owned archival storage. Define archive
acknowledgement and backpressure before evicting events that the host elected to retain.
Make retry, duplicate delivery, crash recovery and consolidation idempotent. Do not wait
until a conversation ends to preserve consequential confirmed events.

Consolidate incrementally over explicit event ranges. Retain original references,
coverage, attribution, time, corrections, uncertainty and visibility. Avoid repeatedly
summarizing summaries or treating a compressed summary as stronger evidence than its
sources. Optional model reflections are proposals with bounded work and host acceptance.
Support a deterministic path requiring no reflection calls.

Select recall using relevance, recency, significance and causal relationships under
explicit limits. Reuse existing lexical/indexed recall and host tags first; embeddings
remain optional external integration. Retrieve applicable preferences and commitments
even when the player does not explicitly ask a memory question. Preserve contradictions
and updated information. Erasure and scope changes must invalidate derived summaries,
indices, caches and pending requests as appropriate without leaking removed information.

Stage acceptance: long simulated sessions, cross-session recall, corrections, temporal
ordering, repeated consolidation, bounded retention, archive failure/retry, save forks,
crash recovery, identity mismatch, erasure and multiplayer isolation. Keep direct
observations, reported statements, intended actions and confirmed outcomes distinguishable.

## Stage 4 Purposeful initiative and background life

Connect goals, relationships and relevant events to the existing deterministic routine
runtime and work scheduler. An NPC can resume postponed work, ask for help, follow up
on a promise, decline an interruption, react nonverbally or remain quietly occupied.
Initiative should have a reason and respect scene participation and urgency.

Use configurable cooldowns, hysteresis, per-NPC attention, global scheduling quotas,
replanning bounds, social interruption limits and whole-turn model-call allowances.
Coalesce relevant events and use dependency/timer hints. Avoid a periodic model call for
every NPC. A routine or an imagined off-screen narrative does not confirm an experience.
Off-screen simulation requires host receipts just like visible action execution.

Proposals choose among a small current set of available activities or actions. Recheck
current conditions and permissions before dispatch, then ingest confirmed outcomes.
Guard against duplicate greetings, circular NPC conversations, repeated failed proposals,
resource contention, starvation and player overload. Existing interruptions, resumption,
missed windows and conflicting commitments must keep working.

Stage acceptance: many-NPC workloads, emergencies, blocked travel, conflicting promises,
unavailable resources, repeated interrupts, purposeful silence and recovery after failed
actions. Measure host work separately from model latency and social quality.

## Stage 5 Player controls developer tools and integrated evaluation

Provide tested host APIs and an original dungeon demonstration for conversational
correction, targeted clarification, scoped forgetting, optional retention controls,
text fallback, speech interruption and adjustable proactive chatter. A player's
in-character correction is an attributed report; privileged author edits use a separate
host operation. Honor existing confirmation policies and expose only available actions.

Extend inspection and replay tools to show selected and omitted context, source and
belief distinctions, reasons for state changes, stale/cancelled work, delivery events,
action proposals versus receipts, request counts and timing. Keep host-only diagnostics
out of model context and unauthorized player views.

Demonstrate a merchant interrupted by a customer, postponed work, a remembered preference,
an opinion revised after experience, a corrected misunderstanding, a promise resumed in
a later scene and a reunion after save/load. Include remembered sunlight, a window with
visible sun, a blocked/cloudy view and uncertainty indoors. Show useful conversation
despite missing knowledge. The library must remain independent of dungeon-specific rules.

Document the smallest useful integration: load an approved definition, bind current
actor-visible facts, ingest actual events, prepare a bounded turn, validate a reply or
proposal, execute through the host and acknowledge real delivery/results. Provide
editable definitions/configuration and API examples exercised by tests.

Stage acceptance: new Rust APIs and additive persistent Python, Node/TypeScript and
C#/native bindings work together. Default setup remains lightweight and requires neither
a model nor speech service. Advanced conversation and memory settings remain opt-in.

## Evaluation and implementation loop

For each stage, prototype independently in `eval/`, define host expectations before
seeing model answers, then implement Rust and prove byte-identical Python/Rust outputs.
Add additive conformance/error cases and fuzz all model-written parsers. Preserve existing
expected outputs unless an explicitly documented, reviewed format change is necessary.
Inspect actual control flow as well as test outcomes; one passing fixture is not a
general semantic guarantee.

Use offline/local tests first. Verify remaining approved budget before any paid testing;
if it cannot be established, continue offline and ask before spending. Use `--cap` where
supported and report spend after each live run. Run token-only checks before accuracy
tests. Hosted accuracy tests may use only the two smaller default models; never use
Llama 3.3 70B without explicit authorization. Additional local models also require
authorization. A lack of budget does not block deterministic implementation work.

Keep deterministic host correctness, model correctness and human judgments separate.
Evaluate matched baseline and candidate settings on fresh scenario families and holdouts.
Record tuning separately so a repaired regression is not presented as unseen accuracy.
Retain failed variants in ROADMAP and evaluation artifacts. A model failure may justify
an explicit experimental option or documented gap; never claim an unmeasured win.

Suggested evaluation progression is a 100-turn continuous merchant session followed
by 500-1,000-turn multi-session traces. These are proposed workloads, not measured
capability or mandatory live-call counts. Long offline replays and a bounded live subset
can exercise the design within the approved resources.

Cover scene travel, multiple participants, unresolved references, interruptions, promises,
betrayal and reconciliation, ambiguous speech, corrections, rumors, privacy changes,
nonstandard calendars, instrument failures, sensory occlusion, save branches and archive
failures. Include ordinary chat and questions with full, partial and absent answers.
Test implicit preference recall and appropriate silence alongside explicit factual recall.

Measure unsupported factual claims, unnecessary refusal, useful partial answers,
attribution, relationship/personality continuity, memory updates, repetition, clarification
burden, initiative appropriateness, action legality and actual task outcomes. Use engine
oracles for exact state checks and human review for conversation quality. A model judge,
valid JSON or low token count cannot establish success by itself.

For live runs retain provider/model/digest, quantization, tokenizer/template, runner,
context, threads, sampling and hardware identity. Record complete-turn latency, first
text/audio latency where applicable, retries, model calls, full-request input/output
tokens, budget failures, sampled engine/runner RSS, system memory and swap observations.
Preserve missing measurements as unknown. Compare focused/expanded contexts with required
facts preserved. Do not silently change defaults based on one short or uncontrolled run.

## Documentation checks commits and final delivery

Update README, FORMAT, GUIDE, KIT, CHANGELOG, ROADMAP, relevant feature docs and evaluation
instructions as the implementation lands. Mark experimental speech behavior clearly and
quote measurements with their workloads and limits. Keep a stage-by-stage completion
record so another agent can resume from the current state without repeating work.

Before every implementation commit run the required checks from `CLAUDE.md`:

```sh
cargo fmt --check &&
cargo clippy --all-targets --locked -- -D warnings &&
cargo test --locked &&
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

Run Rust 1.82, native/binding checks, applicable examples, independent prototype/parity
checks, historical evaluation audits and package verification as appropriate. Derive
current commands from `.github/workflows/ci.yml` and checked-in evaluation documentation;
do not assume a previously recorded check count is still current. Perform the required
reference/credential sweep without printing secret values. Verify the final diff and
preserve unrelated work. Commit locally after checks pass. Ask before pushing, tagging,
releasing or publishing to crates.io.

Complete the five stages autonomously within these constraints. Make routine reversible
implementation choices without repeated permission requests. If a particular capability
needs unavailable authorization, state it precisely, continue independent authorized work
and leave an actionable record. Do not present a partial stage as the completed assignment.

Finish with implemented features and configuration; host-owned responsibilities;
validation and measured results with limits; model/API spend; local commit IDs; and
remaining gaps. Distinguish completed deterministic functionality from experimentally
evaluated or still unproven conversational behavior.
