# Optional dynamic characters

`life` keeps approved definitions, subjective attitudes and attributed experiences in
separate layers. Nothing is enabled by default. `Config::resolve` merges shared defaults
and per-character overrides; `enabled`, `balanced`, `deception`, `mistaken` and requested
`tools` are independent host choices. `CharacterTools` owns the optional layer alongside
its existing interaction notebook and routine runtime. Existing relationships, promises,
source answers and tool formats are unchanged.

Run `cargo run --example character-life`. Edit `examples/fixtures/characters/setting.json` and
`merchant-life.json`. `Definition::draft` validates IDs, setting/calendar, exact shared
history references and bounded fields. The host reviews and explicitly installs a draft
with `Life::accept`; no model tool can approve it. Review prose implications against the
setting too: a structural validator cannot detect every textual contradiction. Authored
history must already exist in the host setting, so a draft cannot create future choices.
Nonhuman identities and host calendar IDs work without a model.

Facets carry identity, values, voice, boundaries, practical knowledge, social conventions,
groups, reputation, etiquette, lore/private information, goals, commitments, priorities
and physical/emotional meanings. Attitudes distinguish preferences, opinions, relationship
dimensions, beliefs/reports/hypotheses/misconceptions/unknowns and temporary states. Tags
select applicable traits; audiences gate them. Numeric levels are design controls, never
psychological measurements or probabilities. Existing relationship notes remain the place
for reported quotations; these attitudes are author-selected appraisal parameters.

Host `experience` preserves kind, source, time, scope, causes and retention intent.
`appraise` requires a matching direct/outcome experience, current state revision and an
authored rule; a report or intended action cannot justify a rule application. Each step
is at most 20, a reversal passes through zero, and cooldown/replay guards prevent a single
event from reversing a lasting view. Changes retain reasons and supporting/contradicting
event tokens. Different definitions can appraise the same event differently.

`life_change` only proposes a rule/event/reason. `life_recall` returns selected traits
and attitudes, never engine truth. Register these individually in configuration, expose
through `CharacterTools::enable` or budgeted assembly, and check live `action/target`
permissions. Host mutations revoke exposure. Context tags can select opinions for dialogue,
attention or activity ranking. More context does not register additional tools.

Balanced policy is opt-in context guidance: expression, tastes, jokes, hypothetical and
permitted flavor do not require citations for every sentence. Facts, reports, inference,
beliefs and proposals stay distinct. Useful partial answers and natural uncertainty are
encouraged. This policy does not semantically validate generated prose. High-consequence
answers still use validated host data. Prior speech failures remain unchanged.

## Stage completion record

1. Definitions/appraisals: implemented bounded Rust and independent Python parity, original
   editable files, disabled/private/nonhuman/replay/stale tests and additive fixtures.
   No model calls or conversational-accuracy claim at that stage; later speech diagnostics
   remain experimental and separate from host correctness.
2. Scene/conversation continuity: implemented source-dependent one-use validation, parallel
   thread metadata and acknowledged partial delivery; 42 independent Python/Rust workflows
   including the final permission-epoch rollback guard.
   Free-form speech quality remains experimental.
3. Durable episodic memory: implemented identity/fence-checked checkpoints, archival
   backpressure, extractive ranges, causal recall/erasure and crash-safe request recovery.
   Twenty independent workflows include 1,000 events; production storage is host-owned.
4. Initiative: implemented routine-menu integration, opinions/accepted promises/event
   dependencies, per-turn call quotas, social guards, failure receipts and backoff.
   Eighty independent workflows and a 96-scope scheduler test; social quality unmeasured.
5. Controls, persistent Python/Node/C#/native bridges, focused/expanded workload hints,
   stable prefix rendering and integrated merchant demo: implemented. 27 context workflows,
   39 persistent fixtures and 24 scaled natural requests; retained 24-call local failures.
   Human social-quality evaluation remains unproven.

## Conversation and delivery coordinator

`Coordinator` is an optional helper beside `CharacterTools`, using the existing
`RequestGate`. Bind an actor-visible scene with revision, monotonic tick, permission
epoch, source-attributed facts and a small affordance list. Each fact carries visibility,
observation time and kind: global, perceived, mutual or report. Global truth is inspection
only. A location/window/weather label creates no sensory record. The host must supply
actual perception and original attributed player reports.

Bind up to eight threads with topic, open questions, existing promise IDs, unfinished
matters, listener, participants, shared fact IDs and referents from the existing resolver.
This is host metadata; no English keyword interpreter turns free-form speech into actions.
Threads survive scene changes and can resume independently. `focused` joins a selected
thread with current life context and scoped facts. Use it as protected `TurnPlan::facts`,
then the existing full provider-render count/observe handshake. Changing the frame, tools,
results, retry or template requires a new count. Required context must fit or fail.

`generate` creates a one-use ticket with exact selected source dependencies. `validate`
consumes it. Scene ID, permission epoch, dependent source payloads/revisions, that thread's
metadata and life revision must still match; unrelated revision changes or a second thread
do not invalidate it. The host increments the epoch for consequential permissions,
interruptions, emergencies or audience changes and rechecks current affordances before
engine dispatch. Queue/play additionally recheck scene dependencies after validation.
Validation checks structure/freshness, never semantic entailment of generated prose.

Speech has distinct generated, validated, queued, played, heard, interrupted and cancelled
acknowledgements. Host `delivery` uses cumulative Unicode character offsets. A heard prefix
cannot exceed the played prefix. Each recipient supplies its own acknowledgement; neither
queueing nor generating creates hearing or a world memory. Archive confirmed portions
before `retire` frees a terminal line. Bounds: eight pending requests/threads, 32 live speech
records, 32 scene facts, 16 dependencies and a 20-tick request TTL. Full bounds reject work.
`SpeechTransport` is a dependency-free queue/cancel interface for text or external audio.
`recognition` preserves interim/final status and uncertainty and always returns no actions.
Interim STT cannot dispatch an action. Real streaming/voice integrations remain host code.

## Durable episodic state

`Life::checkpoint(identity)` and `restore_checkpoint(identity,state,fence)` bind campaign,
save branch, character instance, scope, actor, participants, full definition content and
revision, schema and configuration. Host fences specify min/max revision/token and maximum
tick. Authenticate these values in storage. Live instances cannot move backward; restore
an earlier save into a fresh configured instance using its branch-specific fence. A host
explicitly forks a loaded state by checkpointing it under a new branch identity. Conflicting
same-revision payloads, missing fields, future events, identity mismatches and oversized
snapshots fail atomically. Old replay tokens remain rejected after load, eviction or clear.

The `Storage` trait leaves authentication and transactions to the host. `FileStore` is a
single-writer standard-library reference, with exact duplicate detection, bounded reads,
synced temporary writes and atomic rename. Use one host-selected directory per identity.
An interrupted complete temporary write resumes on retry; partial/corrupt temporary files
fail explicitly and need host recovery/removal. It does not lock multiple writers. `flush`
archives retained originals before acknowledging/committing a checkpoint; retrying duplicate
identity/token records is idempotent. Failure preserves working records. Call this on each
consequential confirmed event, including while a conversation is active.

Working memory holds at most 128 events and 128 appraisal changes. It rejects overflow.
`acknowledge_archive` is a host receipt, never a model assertion. `evict` cannot drop elected
retained events before acknowledgement. Consolidation groups a complete explicit range of
original events; it is extractive and does not claim compression. At most 16 nonoverlapping
ranges, 128 original events per range; repeated identical ranges are idempotent. Archive
old groups through the host before erasing them. Original attribution, kind, corrections,
uncertainty in source text and causal references survive. No model reflections are required.

`recall_episodes` reuses `memory::Memory` lexical ranking with host tags, recency and retention
significance, then includes causal neighbors/corrections. Up to 16 selected originals; a
causal closure exceeding the bound fails. Preferences and commitments are retrieved through
life context tags without requiring an explicit memory question. Hosts can retrieve bounded
original archive ranges through their storage/index; the core does not search an unlimited
archive or install embeddings. Contradictory reports remain separate events.

`forget_episodes` removes selected tokens plus working causal dependents, invalidates affected
attitudes and consolidated ranges, and returns an invalidation plan for archives, checkpoints,
recall caches and pending requests. `FileStore::erase` removes archival descendants and all
old scope checkpoints under a 4,096-file scan cap. Erase other host stores/indices before
allowing further access. Privacy changes require a newly authorized scope and invalidated
pending work; never reuse the old audience. Retention and storage authentication stay host
responsibilities. Removed data must not survive in generated text or external voice queues.

Coordinator checkpoints preserve topics, referents, partial deliveries and request high
watermarks. Recovery cancels unplayed work, marks previously playing lines interrupted and
creates no live tickets; old asynchronous results cannot execute after a crash. The persistent composite checkpoint also stores notebook sequences, promise progress and
consumed offer IDs. Save routine checkpoints separately through their existing contracts;
this does not serialize an entire game engine.

## Purposeful initiative

`Initiative` ranks a host menu of at most 16 activities, questions, promise follow-ups,
declines, gestures or silence. It creates no model request or experience. Shared defaults
and overrides separately configure `enabled` (false), `cooldown` (3), `margin` (2),
`social_limit` (2), `attention` (4), `model_calls` (0), and `replans` (2). Counts are
bounded policy controls. `reserve_call` enforces the whole-turn allowance, including
preflight and retries; same-turn polling cannot reset it. Routine initiative uses zero calls.

Candidate IDs, prerequisites, resources, intended recipients, reason, authored priority,
attitude IDs, promise IDs and actual event tokens come from the host. Attitudes (including
commitments) contribute their current signed level divided by ten; each unresolved accepted
promise adds twenty. These are explicit design weights. Promise state comes from the existing
`InteractionTools` notebook: reported, fulfilled, cancelled and another actor's promises
cannot drive a promised activity. Missing event dependencies exclude the candidate. Proposals
retain actual life revision and selected promise revisions, rechecked at dispatch. A fulfilled
promise or an erased source cannot remain executable through an old proposal.

Use `consider_routine` and `dispatch_routine` to join the existing runtime's currently
eligible activity menu, including blocked travel, deadlines and interruptions. Run these
host jobs through `schedule::Scheduler`, driven by relevant events and `next_wake` hints.
Recheck live prerequisites, resources, audience and permission before atomic engine dispatch.
The core does not lock resources or execute movement. Host resource leases remain necessary.

Cooldowns, score hysteresis, per-turn social/replan quotas and retained recipient/candidate/
event signatures suppress repeated greetings. New scene arrival tokens can distinguish a
later reunion. Social candidates at nonzero NPC response depth are suppressed, preventing
circular automatic conversations when the host carries that depth. Inflight activities
cannot be proposed twice. Failure receipts trigger bounded backoff; three repeated failures
require a relevant new candidate event dependency. A successful engine receipt clears the
failure. Host `outcome` requires a fresh actual `outcome` event with a matching activity tag;
a proposal or imagined off-screen narrative cannot record an experience. Silence uses no
model calls. Emergency can bypass cooldown, never prerequisites, work or social quotas.

Bounds include 64 social signatures/failure entries and 16 inflight actions. Full stores
reject work; rotate authorized host instances/checkpoints deliberately. The supplied workload
serves 96 independent character scopes with 16 scheduler jobs per tick over six ticks, with
zero model calls. This is deterministic host-work verification, not model latency or social
quality. Eighty independent Python/Rust workflows include opinion/commitment scores, accepted
versus reported/fulfilled promises, token filtering, duplicate greetings, circular depth,
call reservations and confirmed failure/recovery.

## Player controls, binding workflow and tiered requests

`cargo run --example dungeon -- --life-demo --size 3` exercises an original surface-annex
merchant, Ilen: a remembered quiet-wick preference, customer interruption, a partially heard
line, an intact return that changes trust, a corrected name, a postponed accepted repair
resumed in another scene, and a reunion after save/load. Host sensor fixtures show visible
sun through a window, blocked indoor/cloudy view, aging sunlight and expired/unknown time.
Two ambiguous lamps use the existing resolver for targeted clarification. Authored example
speech demonstrates host flow; it is not a live roleplay benchmark. Source-selected gameplay
and the normal underground scene remain unchanged.

Definitions are editable in `examples/fixtures/characters`; `examples/lantern-vault/life-config.json`
contains explicit demo policy. Edit and rebuild the Rust example, or load JSON at runtime
through the persistent host `life_configure`. The minimal tested `character-life` example
loads a draft, approves it in the host, binds current actor-visible data, registers requested
read tools and dispatches with a current permission predicate. New [session operations](../guides/KIT.md#dynamic-character-session-operations)
add conversation correction (attributed event), privileged author edits, scoped forgetting,
retention election, text recognition fallback, partial speech interruption and adjustable
initiative policy. Erasure preserves request/note/proposal watermarks. Only host acknowledgements
record actual hearing/outcomes. External speech queues and archives must honor tombstones.

`ContextCaps::focused()` permits four scene facts, two facets, two attitudes, two original
episodes and one whole exchange. `expanded()` explicitly permits 16/8/8/8/4. `from_hints`
uses actual dialogue recall/exchange hints: 1B/1.5B use 1/1, 3B uses 2/1, 8B uses 3/2,
30B uses 4/3, 70B uses 6/3 (episodes/exchanges). Larger sizes select expanded trait/fact
caps; any provider/frontier model may choose explicit caps independently of parameter count.
These are advisory policies, not capability measurements or automatic default changes.
The earlier `focused` helper retains its output contract; `select_context` is the versioned
bounded path used by the demonstration and persistent context operation.

Only host-selected required/optional fact IDs enter the packet. Required context fits or
fails; optional recall uses fewer episodes when a full causal/correction group will not fit.
`Life::context_capped` separates facet/attitude caps; `recall_capped` reuses existing lexical
recall. Inspection stays outside model/player context. Stable system rules, character facets
and explicitly stable scene context precede changing attitude/state and whole dialogue tails
in `render_character_chat`. The demo starts focused profiles at 2,048 context tokens and expanded profiles at 8,192,
using a disclosed heuristic count; real hosts must recount the complete runner rendering.
Counts must cover the final provider request; stable ordering is
not a measured KV-cache hit guarantee. Unknown small-model turns in opt-in dungeon
`--dialogue natural-scaled` omit distractor lore. Earlier natural/source modes retain their
recorded request bytes. No universal language parser or mandatory topic menu is introduced.

`--life-demo --size 70 --life-tools` demonstrates expanded context with explicitly requested
`life_recall`/`life_change` schemas, without loading a 70B model. A small demo exposes only
recall when asked; appraisal stays deterministic host work. Core APIs allow independent
configuration: size hints never register tools or grant data visibility/execution permission.
An exposed change still produces a proposal; only the host validates/applies an authored
rule. Real provider selection, hosted budgets and tokenizers remain external integrations.

Combined session checkpoint version 2 includes life, interaction notebook/progress/consumed
IDs, coordinator and initiative replay metadata plus retention policy. Recovery cancels live
capabilities. Persist routines and the actual game save in the same authenticated host
transaction using their existing APIs. Changing an audience requires a newly authorized
scope; do not grant private sources by merely changing a participants array. State inspection
contains host-only sources, omissions, reasons and cancelled work and is not a model packet.

## Host efficiency and experimental limits

In-memory `Value::is_valid` is allocation-free on both success and rejection, validates
number grammar, key uniqueness and parser-equivalent depth, and safely stops before walking
an over-deep tree. `request::bounded` uses it instead of allocating/serializing/reparsing a
second tree. Error construction still allocates its existing message. JSON serialization
reserves its exact output capacity and retains every byte. Life ingestion/appraisal preflight
projected sizes before mutation, without cloning history or materializing inspection. The
budget includes definition/configuration and space for permitted campaign identity fields,
so accepted hot-path updates remain checkpointable. Overflow is atomic.

[Evaluation evidence](../evidence/LIFE_EVAL.md) separates deterministic contracts, host benchmarks,
model speech and human judgment. The installed model's valid JSON still invents facts;
balanced speech, source tokens and prefixes cannot guarantee grounding. The library enforces
host authority and available actions; free-form conversational usefulness and real speech
quality remain experimental. No model-size, RAM, cache-hit or gameplay-quality improvement
is inferred from synthetic host checks.

## Turn-aware speech policy (v2)

Use `Life::context_for_turn` or `compile_speech` for new profile-aware replies; `context`
and `context_capped` remain the legacy format. `SpeechTurn::from_hints` derives focused versus
expanded policy from dialogue workload hints. The host supplies known/partial/unknown,
creative intent, speaker, question subject and individually subject-bound facts. Focused
factual turns and all unknown/partial turns emit a compact grounded directive with empty
improvisation. Authored balanced expression requires both configuration and a known expanded
or explicitly creative turn. It does not enable tools or grant world knowledge.

Run `cargo run --example character-speech` for the tested minimal integration. The dungeon
life demo also uses v2 context and compilation. The current request is a final direct user
line, with explicit subject metadata and descriptive unknown markers such as
`player_birthplace:"unknown_to_merchant"`. Full provider counting still applies.
`assess_sentence` diagnoses punctuation/length/schema-wall cutoffs without rewriting raw
replies; it is not proof of grammar, completeness of thought or grounding. Keep generated,
validated and actually delivered portions distinct.

The [fresh paired follow-up](../evidence/LIFE_SPEECH_V2_EVAL.md) preserves v1 artifacts: 48 local calls,
$0 API, 16 token-first checks and eight pairs across regressions/holdouts. Terminal endings
improve 5/8→8/8 and suspected wall cutoffs 3/8→0/8; weather and stock invention still occur.
Balanced free-form prose remains experimental; independent human quality review is unknown.

## Archetypes and multi-party scenes

[Blueprints and party contexts](ARCHETYPES.md) optionally compose existing Life definitions,
subjective appraisals, routines, lore, evidence and promise tracking. `Life::party_context`
selects listener/relevant-subject attitudes and gates private facets before attention caps.
`PartyScene::compile` joins this with gated lore/affordances, heard-only perspective history
and existing SpeechTurn policy. It preserves the fixed participant scope: an attitude gate
cannot override a facet's audience. `PartyScene::call_life` rechecks those restrictions for
exposed recall tools; changes remain proposals. Existing Life context/call contracts stay
unchanged. Authoritative hearing, storage, permissions and accepted experiences are host-owned.

## Living-world reports and group influence

[Cast packs and living worlds](LIVING_WORLD.md) optionally reuse Life experiences while
preserving origin/relay provenance and bounding hops. Learned events stay reports and cannot
pass ordinary direct/outcome appraisal. Chronological effective group levels and their
reasons live beside the base Life state; old checkpoints remain unchanged. Use the world
compiler and gate-aware dispatcher for these effective levels. Retention, save replay and
erasure must cover provenance/influence ledgers together, not restore a base checkpoint alone.
