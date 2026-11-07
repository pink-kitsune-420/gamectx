# Optional dynamic characters

`life` keeps approved definitions, subjective attitudes and attributed experiences in
separate layers. Nothing is enabled by default. `Config::resolve` merges shared defaults
and per-character overrides; `enabled`, `balanced`, `deception`, `mistaken` and requested
`tools` are independent host choices. `CharacterTools` owns the optional layer alongside
its existing interaction notebook and routine runtime. Existing relationships, promises,
source answers and tool formats are unchanged.

Run `cargo run --example character-life`. Edit `examples/characters/setting.json` and
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
   Conversational usefulness is unmeasured; no model call or accuracy claim.
2. Scene/conversation continuity: implemented source-dependent one-use validation, parallel
   thread metadata and acknowledged partial delivery; 41 independent Python/Rust workflows.
   Free-form speech quality remains experimental.
3. Durable episodic memory: implemented identity/fence-checked checkpoints, archival
   backpressure, extractive ranges, causal recall/erasure and crash-safe request recovery.
   Twenty independent workflows include 1,000 events; production storage is host-owned.
4. Initiative: pending.
5. Controls, bindings and integrated demonstration: pending.

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
creates no live tickets; old asynchronous results cannot execute after a crash. Existing
interaction notebooks/routine checkpoints must also be saved through their own contracts
when used; the life checkpoint does not silently serialize an entire game engine.
