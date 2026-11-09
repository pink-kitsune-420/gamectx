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

## Witness bridge (Gap 1)

`gamectx::character::life::witness` (`WitnessBridge`, local kind `witness`) turns world events
and module outcomes into Life experiences:
- Timeline entries and module outcomes (from recipes, species, barter, things) produce a
  `direct` Life experience for each perceiving witness.
- When an informed character shares an event with another, `tell` produces a `report` Life
  experience for the listener with attributed provenance.
- Deterministic sensory evaluation: checks physical capacity (dead/unconscious), sleep
  state (wakes only on loud noise >= 80), co-location, line-of-sight occlusion, and distant
  public sound propagation.

All produced experiences strictly conform to `validate_event` and can be immediately ingested
via `life.experience(&exp)`.

**Evidence:** 25 cases (`conformance/witness/`, 10 malformed), 2,000 differential parity
workflows (`eval/character/witness_cases.py`, seed 7, 0 mismatches), $0 API spend.

## Body and needs dynamics (Gaps 2 and 12)

`gamectx::character::body` (`BodyManager`, local kind `body`) manages physical body state and
homeostatic needs over time:
- **Need definitions and dynamics (Gap 2):** Needs have an id, display label, urgent threshold (0..100),
  rate per tick, current level, and declared satisfiers (action or item $\to$ reduction amount).
  Species and character sheets can declare their own needs (`species.needs`, `sheet.needs`).
  Ticking advances needs by `rate_per_tick * ticks` (accelerated by `sick` condition). At level 100,
  severe deprivation damages health (5 damage per tick) and can cause death.
- **Body state record (Gap 12):** Tracks fatigue (0..100), health (0..100), injuries (id, severity,
  location, effects), and conditions (`asleep`, `sick`, `unconscious`, `dead`). Fatigue accumulates
  from action catalog costs and recovers during awake rest (5 per tick) and sleep (15 per tick).
- **Condition impacts:**
  - *Action catalog:* `dead` and `unconscious` block all actions; `asleep` blocks all actions except
    `wake` or passive rest; `fatigue == 100` blocks actions with fatigue cost; `sick` incurs +50%
    fatigue penalty (min +2); injuries block declared action verbs and add extra fatigue/time costs.
  - *Routines:* overrides active activities with `rest` when `asleep` or `fatigue >= 90`, and `incapacitated`
    when `unconscious` or `dead`.
  - *Speech:* character is silent (`can_speak: false`, `speech_refusal: "asleep" | "unconscious" | "dead"`)
    while asleep, unconscious, or dead.
- **Operations:** `init`, `assess`, `tick`, `satisfy` (direct, by item, or by action), `rest`, `apply_action`,
  `set_condition`, `remove_condition`, `add_injury`, `heal_injury`.

**Evidence:** 25 cases (`conformance/body/`, 5 malformed), 2,000 differential parity
workflows (`eval/character/body_cases.py`, seed 7, 0 mismatches), $0 API spend.

## Motive arbiter (Gap 3)

`gamectx::character::motive` (`MotiveArbiter`, local kind `motive`) resolves competing motives
across all character subsystems onto one unified, explainable 0..=100 bounded scale:
- **Unified scale & categories:**
  - *Emergencies (80..100):* immediate danger, critical health ($\le 25$), or total exhaustion collapse.
  - *Needs (0..92):* homeostatic drives scaled sharply above threshold ($50..90$, capped at 92 on starvation).
  - *Promises & Deadlines (20..95):* commitments scaled by remaining ticks ($70..85$ impending, $80..95$ overdue).
  - *Routine slot (10..75):* scheduled daily shift activities ($30..75$ active window, penalized when upcoming).
  - *Tasks (30..80):* active planning and execution tasks.
  - *Goals (20..70):* authored sheet goals when condition `when` matches (0 when inactive).
  - *Reaction brains (0..65):* situational impulses treated as candidate inputs, bounded so they never override emergencies or critical needs on their own.
- **Deterministic arbitration:**
  - Sorted by score descending, then kind precedence (`emergency` > `need` > `promise` > `routine` > `task` > `goal` > `reaction`), then lexicographical id.
  - Returns `winner` naming the winning motive, full explainable `winning_reason` citing why the winner beat the runner-up, and the complete `ranked` motive list.
- **Commitment inertia (pass 2, D1):** pass `commitment {motive, inertia? 0–50 (10)}` with the motive the being is
  already pursuing. It stays the winner unless a challenger beats its score by at least `inertia`; emergencies
  always override. The result gains `commitment {held, inertia, motive, present}` and, when held, a reason like
  `routine_bake (score 55) held over need_hunger (score 58) by commitment inertia 10 because bake shift active`.
  Without `commitment` the output is unchanged.
- **Operations:** `arbitrate {body?, routine?, goals?, promises?, emergencies?, brains?, motives?, commitment?}`.

**Evidence:** 30 cases (`conformance/motive/`, 6 malformed; 5 added for commitment inertia), 2,000 differential parity
workflows (`eval/character/motive_cases.py`, seed 7, 0 mismatches), $0 API spend.

## Tasks and planning (Gap 4)

`gamectx::character::task` (`TaskManager`, local kind `task`) turns high-level character goals
and motives into concrete, committed multi-step plans executed through the Action Catalog:
- **Task object:**
  - `id`: unique task identifier.
  - `owner`: character pursuing the work.
  - `assigned_by`, `for`: delegator and beneficiary.
  - `motive_ref`: the winning motive or commitment it serves.
  - `steps`: ordered sequence of action catalog references with target bindings, preconditions, expected effects, and duration/fatigue estimates.
  - `status`: `proposed`, `accepted`, `active`, `blocked`, `suspended`, `done`, `failed`, `abandoned`.
  - `progress`: index of currently active step.
  - `deadline`, `priority`: temporal urgency and importance (0..=100).
  - `reserved_resources`: tools, stations, or items locked for this task.
  - `on_fail` / `on_block`: bounded recovery policy (`replan`, `retry`, `ask_help`, `suspend`, `abandon`).
  - `replan_count` ($\le 3$), `retry_count` ($\le 3$).
  - `history`: array of confirmed step outcomes.
- **Host-owned execution rules:**
  1. *Host validates model proposals:* AI models may propose tasks or replans. The host strictly checks verbs against the Action Catalog, verifies required target slots, ensures causal preconditions (locations, stations, ingredients), and enforces depth ($\le 8$) and step count ($\le 16$) bounds. Untrusted proposals with unknown actions or broken causal requirements are rejected.
  2. *Only confirmed outcomes advance steps:* Step completion requires host confirmation (`outcome.success == true`). Failed outcomes trigger bounded recovery policies rather than unvalidated transitions.
  3. *Deterministic planner:* When given a goal without explicit steps, backward chaining over the Action Catalog generates executable plans (e.g., traveling to source places, bartering for missing ingredients, traveling to stations, and crafting).
  4. *Bounded replans:* Replans preserve already completed steps, compute remaining steps, and fail closed if `replan_count >= 3`.

**Evidence:** 25 cases (`conformance/task/`, 5 malformed), 2,000 differential parity workflows (`eval/character/task_cases.py`, seed 7, 0 mismatches), model proposal evaluation (`eval/character/task_model_eval.py`, 100% bounds and validation accuracy), and Acceptance Scene (§8) replay test (`tests/acceptance_baker.rs`), $0 API spend.

## Affect decay and missing feelings (Gap 6)

`gamectx::character::affect` (`AffectManager`, local kind `affect`) and `character::life` manage emotional decay and mood dynamics:
- **Additive attitude kinds [format]:** Life definitions accept 5 additional attitude kinds: `fear`, `respect`, `gratitude`, `envy`, `admiration` (joining 19 earlier kinds for 24 valid kinds in total).
- **Mood separation:** Untargeted background mood (valence, arousal, label) is modeled separately from targeted attitudes. Derived mood labels: `calm`, `cheerful`, `excited`, `serene`, `agitated`, `depressed`, `irritable`, `tense`, `lethargic`.
- **Decay toward temperament baselines:** Transient emotions decay toward 0 (or configured baseline); interpersonal attitudes decay toward initial baseline. Temperament modifiers:
  - `calm`: transient emotion and fear decay accelerates (+2).
  - `forgiving`: resentment decay accelerates (+2).
  - `volatile`: emotion decay slows down (-2, min 1).
  - `vengeful`: resentment decay slows down (-1, min 1).
- **Operations:** `init`, `assess`, `tick`, `appraise`, `set_mood`. `Life::decay(ticks)` advances time and decays `life.attitudes` toward baselines.

## Skills growth with use (Gap 8)

`gamectx::character::skills` (`SkillsManager`, local kind `skills`) manages character skills, practice, and instruction:
- **Skill records:** id, label, level (0..=100), practice, threshold for next level (`100 + level * 15`).
- **Diminishing returns practice gain:** Successful Action Catalog actions grant practice scaled down as level increases: `((raw_practice * max(10, 100 - level)) / 100).max(1)`. Failed actions grant 0 practice. Level caps strictly at 100.
- **Action Catalog scaling:**
  - Duration discount: up to 50% discount at level 100 (`duration = (base_duration * (100 - (level * 50 / 100))) / 100`).
  - Risk reduction: `risk = base_risk.saturating_sub((level * base_risk) / 100)`.
  - Quality score & tier: `quality_score = min(100, level + quality_bonus)` yielding `poor`, `standard`, `fine`, `superior`, or `masterwork`.
- **Teaching between beings:** Higher-skilled character instructs lower-skilled character in the same place. Student receives practice (capped at teacher level) and develops `respect` and `gratitude` attitudes toward teacher.
- **Operations:** `init`, `assess`, `practice`, `evaluate_action`, `teach`.

**Evidence:** 25 cases (`conformance/affect_skills/`, 5 malformed), 2,000 differential parity workflows (`eval/character/affect_skills_cases.py`, seed 7, 0 mismatches), acceptance scene verification (`tests/acceptance_baker.rs`), $0 API spend.

## Theory of mind (Gap 7)

`gamectx::character::mind` (`MindManager`, local kind `mind`) provides internal models of other beings:
- **Per-other beliefs:**
  - `knows(other, fact)`: boolean status with provenance source (`seen`, `told`, `inferred`), timestamp, and confidence (0..=100).
  - `wants(other, goal)`: goal identifier, priority (0..=100), and provenance source.
  - `feels(other, emotion)`: emotion kind (from valid attitude kinds), optional target, level (-100..=100), and provenance source.
- **Witness knowledge inference:** Evaluates sensory perception over timeline events (`can_perceive`). If another being perceived the event, records `knows: true` (source: `seen`); if not perceived (different place, occluded, asleep, etc.), records `knows: false` (source: `inferred`). This enables reasoning like "she did not see the cart crash, so she does not know about it".
- **Operations:** `init`, `assess`, `record`, `infer_from_witness`, `query`.

## Deception ledger (Gap 13)

`gamectx::character::deception` (`DeceptionLedger`, local kind `deception`) tracks intentional claims and exposure:
- **Told-ledger per listener:** Records statements made by a speaker to a listener: topic, claim, true value, whether it is an intentional lie (`claim != true_value`), timestamp, and status (`active`, `retracted`, `contradicted`).
- **Pre-speech consistency checking:** Before speaking, beings query `check_consistency` to ensure proposed claims do not contradict prior statements made to that listener ("keep a lie consistent").
- **Contradiction appraisal:** When a listener observes truth contradicting an active claim on that topic, `appraise_contradiction` marks the record as `contradicted` and applies relationship penalties: `trust` (-25) and `resentment` (+15).
- **Operations:** `init`, `assess`, `record`, `check_consistency`, `appraise_contradiction`.

## Relationship reconciliation (Gap 9)

`gamectx::character::relationship` (`RelationshipManager`, local kind `relationship`) unifies the separate relationship stores:
- **Precedence rule:**
  1. *Dynamic Life attitudes:* Highest precedence for interpersonal interactions.
  2. *Sheet relationships:* Authored baseline labels (`friend`, `mentor`, `apprentice`, `ally`, `rival`, `enemy`, `stranger`) provide fallback baseline trust/affection when personal attitudes are at baseline.
  3. *Community relations:* Group-level standing applies a contextual modifier (`community_trust / 4`, halved if personal bond $|trust| \ge 50$).
  4. *Viewpoints:* Provides stylistic tone and framing.
- **Single appraisal path:** Standardized action outcomes (`appraise_action`) generate and apply attitude deltas (barter success/failure, broken/kept promises, attacks, assistance, gifts, teaching, exposed lies).
- **Community trust aggregation:** Aggregates group trust from member cohorts (`aggregate_community_trust`).
- **Operations:** `init`, `assess`, `resolve`, `appraise_action`, `aggregate_community_trust`.

**Evidence:** 25 conformance cases (`conformance/social_mind/`, 3 malformed), 2,000 differential parity workflows (`eval/character/social_mind_cases.py`, seed 7, 0 mismatches), acceptance scene replay verification (`tests/acceptance_baker.rs`), $0 API spend.

## Off-screen life resolution (Gap 10)

`gamectx::character::offscreen` (`OffscreenResolver`, local kind `offscreen`) provides coarse resolution of unobserved activities:
- **Expected effects application:** When an activity window closes unobserved by the player (`observed: false`), applies the Action Catalog's expected effects once: resources consumed (`inputs`) and produced (`outputs`), fatigue delta, and skill practice gained.
- **Timeline entry & Life experience generation:** Creates a timeline entry (`private` or `witnessed`) with place and summary, and records a direct Life experience for the actor, ensuring the world changes and characters remember activities performed while the player is away.
- **Bounded work per tick:** Batch resolution is capped at $\le 16$ activities per tick, using the existing scheduler. Pure host computation ($0 model spend).
- **Operations:** `init`, `resolve_window`, `batch_resolve`, `assess`.

## Curiosity and exploration model (Gap 11)

`gamectx::character::curiosity` (`CuriosityManager`, local kind `curiosity`) evaluates novelty and produces exploration/dialogue initiative candidates:
- **Familiarity memory:** Tracks known entities across 4 categories: `known_people`, `known_things`, `known_places`, and `known_facts`.
- **Novelty detection & scoring:** Perceived entities not in familiarity memory produce novelty signals with base scores (`new_person`: 40, `new_place`: 35, `new_thing`: 30, `unknown_fact`: 25).
- **Temperament scaling:** Novelty scores are scaled deterministically by temperament:
  - `curious`: 1.5x scaling (`score * 15 / 10`).
  - `cautious`: 0.7x scaling (`score * 7 / 10`).
  - `apathetic`: 0.4x scaling (`score * 4 / 10`).
  - `calm`: 1.0x baseline scaling.
- **Initiative candidate generation:** Generates host initiative candidates fully conforming to the 10-field initiative schema:
  - Novel people and unknown facts generate `ask` candidates with priority matching scaled score.
  - Novel things and places generate `activity` candidates (examine / explore) with priority matching scaled score.
  - Optional `auto_mark` marks generated entities as familiar so they are not repeatedly explored.
- **Operations:** `init`, `evaluate_novelty`, `generate_candidates`, `mark_familiar`, `assess`.

**Evidence:** 25 conformance cases (`conformance/offscreen_curiosity/`, 6 malformed), 2,000 differential parity workflows (`eval/character/offscreen_curiosity_cases.py`, seed 7, 0 mismatches), acceptance scene replay verification (`tests/acceptance_baker.rs`), $0 API spend.

## Explain trace (pass 2, D4)

`gamectx::character::explain` (`ExplainTracer`, local kind `explain`) answers "why did she do
that?". The host records one trace per decision with `record_trace`: what the being
perceived, what it believed (source and confidence), the motive that won arbitration (score,
rank, reason, runner-up), the task step it served, the action and its outcome. Outcomes can be
confirmed later with `confirm_outcome` (only from `pending`). `query_trace` returns traces
newest first, filtered by `id` or `action`, each with an ordered `chain` and a `why` sentence.
History is a ring of 128 traces per being across at most 64 beings by default (see [host-set limits](#host-set-limits)). Nothing is inferred: the
trace only links what the host already decided. Real output from
`conformance/explain_stability/explain_record_full_chain`:

```text
baker did barter to miller because bread is promised for the market (task motive
task_bake_bread, score 70, beating routine_bake at 50). Triggered by: the flour bin is
empty. Believed: the mill sells flour. Step 1 of task bake_bread_task. Outcome: success
(traded two loaves for a sack of flour).
```

## Stability monitors (pass 2, D1)

`gamectx::character::stability` (`StabilityMonitor`, local kind `stability`) watches
host-reported turns (`audit_turn`, strictly increasing) and reports five failure modes:
- **Dithering:** at least 3 close switches (margin ≤ 10) between exactly two motives in the
  last 8 turns. The warning names the current motive to hold; pass it to the arbiter as
  `commitment` (see [commitment inertia](#motive-arbiter-gap-3)) to damp the switching.
- **Starvation spiral:** a need or fatigue ≥ 80 that does not improve for 3 turns while the
  being makes no recovery attempt (`recovering: true` resets the streak).
- **Deadlock:** beings in a wait-for cycle (each wants a resource another holds), reported once
  per strongly connected group with the contested resources.
- **Herding:** 3 or more beings audited and ≥ 80% on the same activity.
- **Grudge and rumor runaways:** a world total that rose 3 audits in a row and is ≥ 100.

All thresholds are `init` options (see [FORMAT](../FORMAT.md#stability-monitors-v1-pass-2-d1)).
`assess_health` reports `healthy`, `recovered` or `unstable` with per-kind totals, and
`get_warnings` filters the last 256 warnings by kind, being and turn. Monitors only report;
the host decides whether to intervene. Real output from
`conformance/explain_stability/stability_dithering_detected`:

```text
baker switched 3 times between bake and eat in 4 turns; hold eat for commitment inertia
```

**Evidence:** 25 conformance cases (`conformance/explain_stability/`, 6 malformed), 2,000
differential parity workflows (`eval/character/explain_stability_cases.py`, seed 7,
0 mismatches: 1,545 accepted, 455 refused, 181 with warnings), 1,000 mutations per engine
plus 64/65-op, 128/129-trace and 64/65-being boundaries (`tests/robustness.rs`), acceptance
scene replay (`tests/acceptance_baker.rs`), $0 API spend. These are correctness checks; no
soak-test failure rates have been measured yet.

## Subjective world knowledge (pass 2, A1)

`gamectx::character::beliefs` (`BeliefStore`, local kind `beliefs`) gives each being its own,
possibly wrong, picture of the world. A belief is a triple such as `mill sells flour` or
`baker can bake`, held true or false, with a source (`did` > `saw` > `told` > `authored`), a
confidence and, for hearsay, who said it.
- **Learning:** matching evidence reinforces (highest confidence, stronger source). Contradicting
  evidence replaces a belief only when its source ranks at least as high, so a rumor cannot
  overturn what a being saw, but doing something can overturn any rumor.
- **Doing corrects belief:** `outcome` records `<being> can <action>` after a success and marks the
  beliefs a failed attempt relied on as false. Real output from
  `conformance/beliefs_speech/beliefs_outcome_failure_corrects`: the told belief
  `mill:sells:flour` (60, from `carter`) is `revised` to false, `did`, 100.
- **Planning against belief:** `known_actions` filters catalog actions to those the being knows
  how to do; `gaps` reports needed beliefs that are `unknown` or `low_confidence`, the cue to ask.

## Communication as action (pass 2, B1)

`gamectx::character::speech_act` (`SpeechActs`, local kind `speech_act`) makes speaking an act
with preconditions and effects, over the same belief store:
- `inform`, `warn` and `ask` move beliefs between beings, scaled by trust: a miller trusted at 70
  informing the baker leaves the baker believing `mill:sells:flour` at confidence 70 (`told`,
  from `miller`). `warn` also adds fear; asking someone who does not know gives `no_answer`.
- `request`, `command`, `persuade` and `threaten` try to give the listener a want; `command` needs
  authority ≥ 50, and `threaten` costs 20 trust. `comfort` eases fear and builds trust.
- Every `perform` returns its effects, including `mind_record` entries that are exact
  [theory of mind](#theory-of-mind-gap-7) `record` args, so the speaker's model of the listener
  stays in step. Blocked acts report why (`performed: false`) rather than failing.
- `plan` turns an intent into acts: `learn` (ask the most trusted), `share`/`alarm`, `get_help`
  (command, request or persuade, whichever is most likely) and `reassure`.

Acts are honest; lying stays with the [deception ledger](#deception-ledger-gap-13). Trust here is
the pair state set by `relate`, not yet reconciled with
[relationship reconciliation](#relationship-reconciliation-gap-9); the host passes it in.

**Evidence:** 25 conformance cases (`conformance/beliefs_speech/`, 4 malformed), 2,000
differential parity workflows (`eval/character/beliefs_speech_cases.py`, seed 7, 0 mismatches:
1,891 accepted, 109 refused, 849 acts performed), 1,000 mutations per engine plus 64/65-op,
128-belief and 16/17-candidate boundaries (`tests/robustness.rs`), acceptance scene replay
(`tests/acceptance_baker.rs`), $0 API spend.

## Diets and foods (pass 2, stage 3)

`gamectx::character::diet` (`DietManager`, local kind `diet`) decides what a being can, will
and likes to eat:
- **Foods** carry tags, how much hunger and thirst a serving fills, enjoyment, a shelf life (longer
  when preserved), raw risk and a cooking bonus. The original `village` pack gives a small game
  twelve foods with no authoring.
- **Diet profiles** combine what the body allows (the species diet), what the being refuses on
  principle (`forbids`, for example a vow or a creed's taboo), allergies, and likes and dislikes.
- **Meals** are judged for freshness, refusals, fill, sickness and joy. Eating the same thing again
  and again gets duller (3 joy per repeat in the last 8 meals, up to 15). `choose` weighs fill
  against enjoyment by hunger: in `conformance/diet/diet_choose_ranks_by_hunger` a starving being
  picks cooked stew (score 67 over 40) and a sated one picks honey cake (20 over 12).
- **Eating** records the meal and returns proposals in the exact argument shapes of body `satisfy`
  / `set_condition` and affect `set_mood`; the acceptance scene applies the `satisfy` proposal to
  the body engine directly. A refused meal is eaten only with `force` (a starving being eating
  spoiled porridge gets half the fill and falls `sick`).

**Evidence:** 25 conformance cases (`conformance/diet/`, 4 malformed), 2,000 differential parity
workflows (`eval/character/diet_cases.py`, seed 7, 0 mismatches: 1,399 accepted, 601 refused,
1,748 meals eaten), 1,000 mutations plus 64/65-op, 16/17-option and 256/257-food boundaries
(`tests/robustness.rs`), acceptance scene replay (`tests/acceptance_baker.rs`), $0 API spend.

## Creeds and conscience (pass 2, stage 4 and B5)

`gamectx::character::creed` (`CreedManager`, local kind `creed`) gives beings shared belief
systems and a conscience of their own:
- **Creeds** are a `religion`, `ideology`, `philosophy` or `tradition`. Each has **tenets**
  (a forbidden action, optionally only against its `sacred` places and things, or a forbidden
  food tag), timed **observances** (do an action within a window of every period) and stances
  toward **rival** creeds (`hostile`, `wary`, `tolerant`). Stances are one-sided: in
  `conformance/creed/creed_regard_stances` the Keepers of the Hearth are `wary` of the Free
  Tillers, who are `hostile` back.
- **Adherents** hold at most one creed with a **devotion** 0–100, plus up to 16 **personal norms**
  (B5) that weigh at full strength. Keeping an observance raises devotion and standing by 1.
- **`judge`** an act: the actor feels **guilt** (the largest broken norm's severity × devotion /
  100) and **shame** (half the guilt) when a witness disapproves; each disapproving witness feels
  **indignation** by its own norms; the actor loses **standing** in the witnesses' creeds. A devout
  baker (devotion 90) seen stealing by a fellow keeper feels guilt 72 and shame 36, the witness
  resents the thief by 40 and the baker's standing falls from 50 to 42
  (`creed_judge_witnessed_shame_and_indignation`); seen by a Free Tiller, who has no theft tenet,
  the baker feels guilt alone. Effects are affect `set_mood` and `appraise` (`resentment`)
  proposals in their exact argument shapes.
- **`due`** turns open observance windows into motive arbiter `goal` candidates (score severity ×
  devotion / 100, clamped to 20–70) and charges guilt and standing once for each window missed
  since joining.
- **`food_taboos`** gives the tags for diet `forbids`: creed food tenets bind from devotion 30,
  personal ones always. In the acceptance scene the baker, a keeper, eats meat stew in front of
  the apprentice (guilt 28, shame 14, applied to the affect engine), and the creed's taboo fed
  to the diet steers the next meal to bread.

**Evidence:** 25 conformance cases (`conformance/creed/`, 4 malformed), 2,000 differential parity
workflows (`eval/character/creed_cases.py`, seed 7, 0 mismatches: 1,835 accepted, 165 refused,
106 verdicts with guilt), 1,000 mutations plus 64/65-op, 16/17-witness, 32/33-creed and
128/129-being boundaries (`tests/robustness.rs`), acceptance scene replay
(`tests/acceptance_baker.rs`), $0 API spend.

## Expectations and surprise (pass 2, A3)

`gamectx::character::expectation` (`ExpectationBook`, local kind `expectation`) lets things that
do *not* happen matter:
- A being **expects** an event about a subject by a due time, with a grace period: a **hope** or a
  **dread**, with an importance and a confidence. `relies` marks a promise.
- **`tick`**: past due, a hope is **worried** about once (a quarter of the surprise as a mood drop);
  past grace it is **disappointed** (half the surprise), and a relied-on subject is resented by the
  same amount. A dread that never came brings **relief**. Surprise = importance × confidence / 100.
  In `conformance/expect_attend/exp_worry_then_disappointment_and_resentment` the baker's flour
  promise (importance 80, confidence 70, surprise 56) gives worry −14, then mood −28 and
  resentment 28 toward the player.
- **`happen`**: a hope is **kept**, a dread is **met** (half its importance as a mood drop), and a
  hope that already broke arrives **late** (relief, a quarter of its importance).
- **Reliability**: every kept or broken expectation is counted per subject, and the next
  expectation's default confidence is 70 + 10 × kept − 15 × broken (10–95): after five broken
  repayments a drifter is expected with confidence 10 (`exp_reliability_floor`).

**Evidence:** see attention below (one suite).

## Attention and graded noticing (pass 2, A2)

`gamectx::character::attention` (`AttentionModel`, local kind `attention`) decides who actually
noticed an event, after the witness bridge has said who could:
- Score 0–100 = max(salience × light / 100 if seeing, noise if hearing) + alertness/5 +
  perception/5 + 25 if the being cares about the event's kind or subject − busy/3 − distance/5
  (halved and sound-only while asleep). Events at a place the being is neither at nor `near` are
  `out_of_range`.
- The event is noticed when FNV-1a(`"<seed>:<being>:<event>"`) mod 100 falls below the score,
  so the host's seed makes replay exact and the same scene with another seed differs only at the
  margins (`att_seed_changes_rolls`). Clear at 60 or more, else vague.
- Attention is limited: a being notices at most `capacity` events (default 3), highest scores
  first; the rest are `over_capacity`. A shout heard from 400 units away scores 0
  (`att_distance_attenuates`); a dark lane hides a silent figure (`att_light_matters`).
- `noticed_by` per event is the list of witnesses to pass on to the witness bridge.

**Evidence:** 25 conformance cases (`conformance/expect_attend/`, 5 malformed), 2,000 differential
parity workflows (`eval/character/expect_attend_cases.py`, seed 7, 0 mismatches: 1,934 accepted,
66 refused, 923 expectation changes, 3,030 notices), 1,000 mutations each plus 64/65-op,
64/65-being, 16/17-event, 64/65-profile and 64/65-subject reliability boundaries
(`tests/robustness.rs`), acceptance scene replay (`tests/acceptance_baker.rs`), $0 API spend.

## Host-set limits

The game decides how much it needs. Thirty-five character modules and three world modules take a `limits` object next to
`ops` (`{"limits": {"beings": 5000}, "ops": [...]}`) or a `with_limits` constructor: the pass 1
modules (`affect`, `skills`, `body`, `mind`, `deception`, `relationship`, `offscreen`,
`curiosity`, `task`) and the pass 2 modules (`beliefs`, `speech_act`, `diet`, `creed`, `explain`,
`stability`, `expectation`, `attention`, `aspiration`, `habit`, `lesson`, `cooperation`, `cognition`, `chatter`, `survival`, `detail`, `retention`, `roles`, `economy`, `space`, `drift`, `packs`, `turn-frame`, `cueing`, `pragmatics`, `embodiment`), the pass 3 world and social schemas (`surroundings`, `fabric`) the pass 4 genre profiles (`genre`), mind architectures (`minds`) and defenses (`defenses`). Each limit is an
integer from 1 to a hard ceiling that keeps work and memory bounded (beings up to 1,000,000;
foods up to 65,536; per-being tables up to 256–65,536; per-call lists such as attention's
`notice_beings` or mind's `witnesses` up to 4,096). Limits only refuse growth: lowering one below
current usage keeps what exists. When supplied, the output echoes the effective limits. Limits
are host configuration; never pass model output into them. The names, defaults and ceilings are
in [FORMAT](../FORMAT.md#host-limits-v1); `conformance/limits/` and
`conformance/limits_character/` show modules past their old bounds (300 foods, 33 creeds, 40
skills, 20 injuries, 80 witnesses, 149 told claims, 70 sheet relations) and refused over a
ceiling.

Defaults are the earlier fixed bounds, so games that never set limits see the same output as
before, with one exception: a few paths used to grow without any bound and now stop at the
default. Skills learned through `practice` or `teach` stop at 32 per being; minds created by
`record` or `infer_from_witness` stop at 64 beings, 64 modelled others and 64 beliefs per
category; told records added by `init` stop at 128; relationship sheets stop at 64 relations per
subject and 4,096 subjects, attitudes and community pairs. Raise the matching limit if a game
needs more.

Measured host work at large casts (`eval/character/scale_measure.rs`, retained in
`eval/character/scale-timing.json`; one release-mode run on an 8-core desktop CPU, single thread).
Each module was filled with N entries by one small op each (a being, or a told claim, attitude,
off-screen record, promise, memory, known topic or task), then given 1,000 single-op queries spread over them:

| module (query) | median / p95 µs at 1k | at 10k | fill µs per op (10k) | heap bytes per entry |
|---|---|---|---|---|
| `beliefs` (`query`) | 1.5 / 1.6 | 2.1 / 2.7 | 4.3 | 2,521 |
| `speech_act` (`assess`) | 0.4 / 0.7 | 0.5 / 0.6 | 1.2 | 173 |
| `diet` (`choose`) | 1.8 / 1.9 | 2.2 / 2.7 | 1.8 | 613 |
| `creed` (`judge`) | 1.6 / 1.9 | 1.7 / 1.9 | 1.0 | 294 |
| `explain` (`query_trace`) | 3.2 / 3.3 | 4.2 / 5.3 | 3.3 | 2,283 |
| `stability` (`get_warnings`) | 0.3 / 0.3 | 0.3 / 0.3 | 1.5 | 274 |
| `expectation` (`assess`) | 1.2 / 1.4 | 1.5 / 2.1 | 1.8 | 1,856 |
| `attention` (`notice`) | 1.8 / 1.9 | 1.9 / 2.2 | 0.9 | 132 |
| `aspiration` (`goals`) | 0.8 / 0.8 | 1.0 / 1.4 | 2.5 | 685 |
| `habit` (`routine`) | 0.7 / 0.8 | 1.0 / 1.4 | 1.7 | 772 |
| `lesson` (`advice`) | 0.7 / 0.8 | 1.1 / 1.8 | 2.4 | 1,335 |
| `affect` (`assess`) | 1.1 / 1.2 | 1.7 / 2.3 | 2.6 | 1,881 |
| `skills` (`evaluate_action`) | 1.0 / 1.1 | 1.4 / 2.0 | 1.7 | 1,314 |
| `body` (`assess`) | 1.0 / 1.1 | 1.6 / 2.1 | 1.1 | 1,599 |
| `mind` (`query`) | 0.8 / 0.9 | 1.4 / 2.0 | 1.8 | 2,580 |
| `deception` (`check_consistency`) | 1.0 / 1.2 | 2.0 / 3.0 | 2.1 | 884 |
| `relationship` (`assess`) | 1.0 / 1.1 | 1.5 / 2.1 | 2.8 | 349 |
| `offscreen` (`assess` one being) | 2.0 / 3.1 | 3.7 / 4.8 | 5.6 | 2,370 |
| `curiosity` (`evaluate_novelty`) | 1.1 / 1.2 | 1.4 / 1.9 | 0.7 | 590 |
| `cooperation` (`assess`, scans every record) | 14.2 / 16.5 | 217.7 / 229.9 | 3.5 | 230 |
| `cognition` (`recall`) | 1.7 / 1.9 | 2.3 / 2.9 | 3.6 | 1,681 |
| `chatter` (`meet`, a whole meeting) | 6.3 / 9.1 | 7.2 / 8.1 | 1.9 | 864 |
| `survival` (`threat`) | 2.4 / 2.5 | 2.6 / 2.9 | 2.0 | 377 |
| `detail` (`assess`) | 1.4 / 1.5 | 1.9 / 2.5 | 3.9 | 803 |
| `retention` (`touch`) | 1.1 / 1.2 | 1.3 / 1.4 | 1.8 | 118 |
| `roles` (`assess`) | 1.1 / 1.2 | 1.6 / 2.0 | 1.9 | 529 |
| `economy` (`price`) | 0.9 / 1.0 | 0.9 / 1.0 | 2.1 | 114 |
| `space` (`see`, line to a far being) | 2.1 / 2.2 | 2.4 / 2.9 | 2.1 | 198 |
| `drift` (`experience`) | 2.1 / 2.6 | 2.4 / 3.0 | 2.6 | 1,885 |
| `packs` (`apply`, six requests) | 6.0 / 6.1 | 6.8 / 7.7 | 4.7 | 1,089 |
| `frame` (`frame`, one being) | 3.5 / 3.7 | 3.9 / 4.5 | 1.8 | 805 |
| `cueing` (`cue`, one episode) | 2.8 / 3.0 | 3.6 / 4.3 | 3.2 | 3,071 |
| `pragmatics` (`parse`, a two-clause turn) | 10.1 / 10.6 | 10.0 / 10.4 | 2.7 | 761 |
| `embodiment` (`react`, a silent turn) | 1.7 / 1.9 | 1.9 / 2.3 | 1.5 | 145 |
| `surroundings` (`inspect_item`, an owner inspecting an item) | 1.4 / 1.6 | 1.8 / 2.3 | 2.3 | 406 |
| `fabric` (`dyad`, a pair's history) | 1.2 / 1.3 | 1.4 / 1.8 | 1.7 | 338 |
| `genre` (`render`, a decision in station words) | 3.1 / 3.4 | 3.8 / 4.4 | 2.5 | 1,245 |
| `minds` (`admit`, a slow spirit's reaction) | 2.0 / 2.5 | 2.1 / 2.5 | 1.2 | 158 |
| `defenses` (`threat`, an exposed secret rationalized) | 2.9 / 3.1 | 3.3 / 3.9 | 3.2 | 444 |
| `task` (`get`) | 1.4 / 1.5 | 2.1 / 2.7 | 3.7 | 1,417 |

Going from 1,000 to 10,000 entries raises query time by at most about 2× (lookups are ordered
maps), except cooperation `assess`, which walks every task, promise, resource and debt: 14 µs at
1,000 promises, 218 µs at 10,000 (an index per being is a ROADMAP follow-up). Memory is linear:
these minimal entries take 392 MB across all forty modules at 10,000 each, about 39 KB per
character if every module holds one entry for it; richer characters use more. The measurement found four queries that scanned everything and now use an index or an
ordered range with unchanged output: speech-act `assess` (75 µs at 10,000 pairs before),
relationship `assess` (214 µs), off-screen `assess` for one being (92 µs) and deception
`check_consistency` (66 µs). Limits: JSON parsing, the `gamectx local` CLI, cross-module wiring
and model calls are excluded; one desktop run with uncontrolled scheduling, not a realtime
guarantee. Listing ops without a filter (`assess` on the whole deception ledger or off-screen
history, task `list`) still return, and so walk, every entry.

### Knowledge modules

The world's tables take limits the same way (pass 2 stage 7c): `places` (the codex: `places`,
`entrances`, `factions`, `events`, `quests` and its serialized `bytes`), `timeline` (`entries`,
`bytes`), `things` (`materials`, `kinds`, `things`, `bytes`), `species` (`species`, `animals`),
`recipes`, `communities` (`communities`, `members`, `customs`, `relations`, market `rules`),
`actions`, `lore` (`entries`, `bytes`), `inventory` (`items`, `holdings`, snapshot `bytes`),
`evidence` (`records`, `bytes`) and attributed notes, the `info` kind (`notes`, checkpoint
`bytes`). Stateless kinds take `limits` next to their other fields (`{"limits": {"places": 2000},
"codex": ..., "queries": [...]}`); `Things`, `SpeciesWorld`, `Recipes`, `CommunityWorld` and
`ActionCatalog` also have `with_limits`, and loaders have `*_within` variants (`Codex::from_value_within`,
`Timeline::within`, `Lore::from_value_within`, `Inventory::from_value_within`, `Notes::within`).
Ceilings: places, events, quests, kinds, species, recipes, communities, actions and lore entries
up to 65,536; timeline entries, things, animals and holdings up to 1,000,000; evidence records up
to 1,024 (a selected packet stays within 262,144 bytes); document bytes up to 256 MiB (inventory
64 MiB, evidence 4 MiB). Facts already had a host-set `capacity`; its ceiling rose from 256 to
4,096 (serialized state still within 4 MiB), and dependency checks walk chains on the heap, so a
4,096-link chain cannot exhaust the thread stack. `conformance/limits_knowledge/` shows 154
places, 520 thing kinds, 263 animals, 263 recipes, 70 communities, 540 lore entries, 301
holdings, 200 evidence records and 70 notes past their old bounds, and refusals at a limit.

Defaults are the earlier bounds, with one tightening: request `actions` given to the `actions`
kind may no longer grow the catalog past the `actions` limit (256, built-ins included).

Measured host work (`eval/knowledge/scale_measure.rs`, retained in
`eval/knowledge/scale-timing.json`; same machine and method as above, loading pre-built values):

| module (query) | entries (small / large) | median / p95 µs, small | median / p95 µs, large | load µs per entry (large) | heap bytes per entry |
|---|---|---|---|---|---|
| `places` (`nearby`) | 1,000 / 10,000 places | 39.5 / 49.7 | 132 / 160 | 3.4 | 747 |
| `timeline` (`query` known_to) | 1,000 / 10,000 entries | 53.4 / 57.3 | 1,195 / 1,392 | 1.5 | 1,085 |
| `lore` (`select`) | 1,000 / 10,000 entries | 107 / 111 | 1,330 / 1,464 | 1.8 | 833 |
| `inventory` (`rows` one owner) | 1,000 / 10,000 holdings | 4.2 / 4.4 | 31.3 / 32.3 | 0.5 | 105 |
| `info` (`info_lookup`) | 1,000 / 10,000 notes | 12.5 / 12.9 | 130 / 136 | 1.9 | 657 |
| `evidence` (`select`) | 256 / 1,024 records | 82.1 / 86.0 | 389 / 397 | 0.8 | 432 |
| `facts` (`check`) | 1,000 / 4,096 facts | 0.9 / 1.0 | 1.1 / 1.4 | 2,307.1 | 570 |
| `things` (`move` workflow) | 1,000 / 10,000 things | 658 / 671 | 6,349 / 7,339 | 14.4 | 261 |
| `species` (`pet` workflow) | 1,000 / 10,000 animals | 698 / 798 | 5,502 / 5,743 | 14.3 | 349 |
| `recipes` (`check` workflow) | 1,000 / 10,000 recipes | 1,151 / 1,176 | 16,343 / 23,494 | 17.3 | 1,026 |
| `communities` (`check_relation` workflow) | 1,000 / 10,000 communities | 1,044 / 1,077 | 10,895 / 12,058 | 15.4 | 746 |
| `actions` (`describe` workflow) | 1,000 / 10,000 actions | 3,729 / 4,197 | 54,233 / 57,685 | 425.6 | 3,294 |

Facts `check` answers from an index (1.1 µs at 4,096 facts). Queries that filter or rank every
entry grow linearly: inventory `rows` (31 µs at 10,000 holdings), timeline `query` (1.2 ms at
10,000 entries), lore `select` (1.3 ms at 10,000), `info_lookup` (0.13 ms at
10,000 notes), evidence `select` (0.39 ms at 1,024 records). `nearby` builds the codex's place
graph once and then grows with the places it reaches; in this test every field sits in one
valley, so each answer also counts all 10,000 siblings (0.13 ms). The measurement found
`nearby` copying the whole codex and rebuilding every edge on each query (10.7 ms at 10,000
places before) and a per-entry allocation in timeline queries; both are fixed with unchanged
output. The things, species, recipes, communities and actions workflows return their whole
table on every call (that is their format), so a call costs time in proportion to the table:
6.3 ms for one `move` among 10,000 things and 54 ms for one `describe` among 10,000 actions,
whose entries are large. Facts are registered one at a time and each registration copies and
re-measures the registry (2.3 ms each at 4,096 facts). Limits: one desktop run with uncontrolled
scheduling; JSON text parsing, the CLI and model calls are excluded; not a realtime guarantee.

## Goals from experience (pass 2, A4)

`gamectx::character::aspiration` (`Aspirations`, local kind `aspiration`) grows goals from what
happens to a being. Each `experience` (`need`, `threat`, `admiration`, `loss`, `opportunity`, about a
subject, weight 1–100) adds pressure; 200 quiet ticks reset it. At 100 a fixed template forms a goal
(`need` → "Keep a store against <subject>", `threat` → "Guard against <subject>", `admiration` →
"Learn <subject>", `loss` → "Replace the lost <subject>", `opportunity` → "Pursue <subject>") at
priority 40 + total/10 (≤ 90); later experiences reinforce it by weight/10. Three hungry days of
weight 40 form "Keep a store against hunger" at 52 (`conformance/growth/asp_need_forms_store_goal`).

For cases the templates do not cover, a model may `propose` a goal. The host accepts it only if it
cites recorded pressures totalling at least 50 and there is room, and caps its priority at 60
(`asp_model_proposals_validated`). `goals` lists goals in the motive arbiter's `goals` shape; in the
acceptance scene two thefts make "guard against thieves" win the arbiter over a fresh habit.

## Habits and adapting routines (pass 2, A5)

`gamectx::character::habit` (`Habits`, local kind `habit`) lets routines change with use. Entries
"do `action` in `slot`" have a strength: authored ones (default 80) never drop below 30; habits
start at 40. Choosing an entry's action keeps it (+5); choosing something else in its slot weakens it
(authored −5, habit −10); `skip` weakens with a stated reason; a habit at 0 fades. Three identical
choices in a row in a slot form `habit_<slot>_<action>`. In `hab_habit_fades` a tavern habit fades
after four evenings of whittling while whittling becomes the new habit. Every change keeps its reason
(last 8 per entry), and `routine` lists entries as motive arbiter `routine` candidates.

## Lessons (pass 2, A6)

`gamectx::character::lesson` (`Lessons`, local kind `lesson`) turns confirmed outcomes into lessons
a being can explain. Outcomes are grouped by topic (activity × subject); over the last `episodes`
(default 16), at least 3 outcomes with a two-thirds majority give `avoid` or `seek`, citing the
episodes behind it. New outcomes report `learned`, `confirmed`, `contradicted`, `revised` or
`doubted`: with a three-episode window, three good deals after three bad ones flip "avoid the trader"
to "seek" (`les_revised_by_new_outcomes`). `advice` gives a motive bias of ±confidence/4 and a
sentence: "deal with trader went badly 3 of 4 times" (bias −18).

**Evidence (A4–A6):** 25 conformance cases (`conformance/growth/`, 5 malformed), 2,000 differential
parity workflows (`eval/character/growth_cases.py`, seed 7, 0 mismatches: 1,926 accepted, 74 refused,
137 goals formed, 62 habits formed, 183 lessons learned or revised), 1,000 mutations of each kind with
limits, 64/65-op and limit-ceiling boundaries and a 5,000-character cast (`tests/robustness.rs`),
acceptance scene replay (`tests/acceptance_baker.rs`), $0 API spend.

## Cooperation and conflict (pass 2, B2/B3)

`gamectx::character::cooperation` (`Cooperation`, local kind `cooperation`) lets beings work
together and compete. All scores are signed millionths. Every effect is a proposal for another
engine (`relationship` `appraise_action`, `expectation` `expect`, `affect` `appraise`
resentment) that the host may apply.

**Joint tasks.** `form` a task with an organizer, a goal and named roles; the organizer
`assign`s roles. Any member may `ask_help` for an open role, and the helper accepts when

`300,000 + trust·4,000 + min(favors owed to the asker, 5)·100,000 − busy 300,000 − effort·2,000
− grudge/2 ≥ 500,000`.

Trust 60 at effort 20 is exactly 500,000 (`help_accept_and_refuse`). Completing a helped role
proposes `help` and puts the asker in the helper's debt, or repays a favor the helper owed: a
carter who owes the farmer two favors helps at trust 0 (`help_favor_owed_tips_refusal`). A helper
who `leave`s proposes `broken_promise` and the asker holds a 200,000 grudge; `hand_off` passes a
role on and clears the helper link.

**Promises.** `promise` proposes an expectation `hope` for the receiver. `keep` proposes
`fulfilled_promise` (`kept`, or `kept_late` after the due tick); the receiver may `release` it.
`tick` breaks every pending promise past due: `broken_promise` and a 300,000 grudge. Grudges
fade by 1,000 per tick, so a broken promise is forgiven 300 ticks later
(`grudge_fades_with_time`). A grudge counts against later help: the carter whose seed loan was
never repaid refuses the farmer at trust 60 (`help_grudge_weighs_against`, score 350,000).

**Contested resources.** `claim` takes a free resource or joins its queue, ordered by contest
turns, then `urgency·6,000 + status·4,000`, then arrival; `free` passes it to the head. A
`contest` scores `500,000 + Δstatus·5,000 + Δurgency·4,000 + trust·3,000 − grudge/2`. At
650,000 or more the holder yields (resenting it below trust 20). At 350,000 or less the holder
stands, and the challenger queues and resents it. Between the two, the holder offers turns and
the challenger goes next. In `contest_grudge_hardens_holder`, a smith who yielded to a lord
(700,000, +200,000 grudge) only offers turns the next time (600,000). `grudges` and `assess` show
a being's grudges, roles, promises, holdings, queue positions and favors.

**Evidence (B2/B3):** 25 conformance cases (`conformance/cooperation/`, 6 malformed), 2,000
differential parity workflows (`eval/character/cooperation_cases.py`, seed 7, 0 mismatches:
1,446 accepted, 554 refused, 77 helps accepted, 2,149 contests resolved, 1,946 grudge
proposals). Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of three samples
with limits, 64/65-op and limit-ceiling boundaries, a 64-role task, a 1,024-deep queue, and a
refused grudge that changes nothing. Scale is in the [table above](#host-set-limits). $0 API
spend.

## Bounded rationality and fallible memory (pass 2, A7/A8)

`gamectx::character::cognition` (`Cognition`, local kind `cognition`) makes beings think and
remember differently. Rates and strengths are fixed-point millionths; rolls are FNV-1a of a
host-chosen seed, so the same seed always gives the same result.

**Temperament settings.** `profile` sets a temperament, and any setting can be overridden:

| temperament | plan depth | horizon (ticks) | impulsiveness | memory span (ticks) | slip |
|---|---|---|---|---|---|
| `careful` | 6 | 240 | 10 | 2,000 | 50,000 |
| `steady` (default) | 4 | 120 | 25 | 1,000 | 100,000 |
| `impulsive` | 2 | 30 | 70 | 500 | 200,000 |
| `dreamy` | 3 | 60 | 40 | 300 | 250,000 |

`decide` takes motives and candidate steps. When the impulse roll (0..99) is below
impulsiveness, the being acts on its top motive with no plan (`cog_impulse_skips_planning`).
Otherwise it keeps at most `plan_depth` steps due within the horizon. Given the same eight
chores, the careful miller plans six and the impulsive tinker two (`cog_plan_depth_cuts_steps`).
An unfamiliar step slips when its roll (0..999,999) is below the slip rate, and the first slip is
reported (`cog_slip_rate_is_seeded`).

**Fading memories.** `remember` stores an event (about, what, day, people, importance 0–100) at
strength 1,000,000. Each `tick` takes away
`elapsed·1,000,000·(100 − importance) / (100·memory span)`. Importance 100 never fades; a
memory is `vivid` at 900,000 or more, `clear` at 600,000 or more, otherwise `hazy`, and it is
forgotten at 0. After 500 steady ticks, importance 0 has halved and importance 50 has lost a
quarter (`mem_fade_by_importance`). The same toll payment after 200 ticks is vivid for a careful
clerk and hazy for a dreamy poet (`mem_span_from_temperament`).

**Misremembering.** `recall` rehearses a memory (+100,000). A hazy memory comes back wrong with
probability `(600,000 − strength)` millionths. The error either shifts the day by 1–3 or puts a
person from another memory in place of the first person. Every reply carries `truth` beside
`told`, with `distorted` and `kind`, so the host always knows what happened. Two witnesses of the
same barn fire disagree: the careful miller recalls day 12, the dreamy tinker says day 10
(`mem_conflicting_testimony`). In `mem_merged_person` the tinker places the smith at the fire.

**Evidence (A7/A8):** 25 conformance cases (`conformance/cognition/`, 6 malformed), 2,000
differential parity workflows (`eval/character/cognition_cases.py`, seed 7, 0 mismatches: 1,560
accepted, 440 refused, 999 impulses, 214 slips, 223 distorted recalls of which 96 merged a
person, 1,178 memories forgotten). Robustness (`tests/robustness.rs`) covers 1,500 mutations of
each of two samples with limits, 64/65-op and limit-ceiling boundaries, a 256-motive and 256-step
decision, the clock and memory span at their bounds, and a refused op that changes nothing.
Scale is in the [table above](#host-set-limits). $0 API spend.

## Off-screen conversations (pass 2, B6)

`gamectx::character::chatter` (`Chatter`, local kind `chatter`) lets characters talk to each
other when they meet, with no model call. Each being `know`s topics (a value, a weight 0–100,
optionally secret) and `want`s others. `bond` sets a pair's warmth (−100..100, default 0).

When two known beings `meet`, FNV-1a of `"<seed>:<a>:<b>"` picks who speaks first, and they take
up to `turns` turns. On each turn the speaker offers its best topic by
`2·(listener's want) + weight`, then by name. It skips topics already said in the meeting,
topics the listener already holds with the same value, and secrets while warmth is below 60.

- If the listener holds that topic with another value, the turn is a **dispute**.
- Otherwise it is a **tell**. The listener learns the topic at half weight (if it has room), a
  beliefs `learn` proposal goes out, and a wanted topic adds 5 warmth and a relationship `help`.
- With nothing to offer the turn is **small talk**; two in a row end the meeting.

Each meeting becomes a record. With any dispute while warmth was below 20, the meeting is a
**quarrel**: warmth −10 per dispute, down to −100, and resentment both ways. Otherwise it is an
**alliance** when a wanted topic was shared and warmth reached 60, **gossip** when anything was
told, and small talk when nothing was.

The widow tells the baker where the fox den is because he wants it (`chat_tell_what_is_wanted`).
News of a thief reaches the guard two meetings later at a quarter of its weight
(`chat_gossip_spreads_in_steps`). The guard keeps the gate key secret at warmth 40 and tells it
at 60, and the niece who learns it keeps it secret from a stranger
(`chat_secret_travels_as_secret`). A disagreement about the bridge is a quarrel between
strangers and plain talk between old friends (`chat_dispute_becomes_quarrel`,
`chat_warm_dispute_is_not_quarrel`). `log` lists records newest first, and `assess` shows a
being's topics, wants and bonds.

**Evidence (B6):** 25 conformance cases (`conformance/chatter/`, 6 malformed), 2,000 differential
parity workflows (`eval/character/chatter_cases.py`, seed 7, 0 mismatches: 1,438 accepted, 562
refused, 4,566 tells, 1,278 disputes, 1,025 quarrels, 31 alliances). Robustness
(`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits, 64/65-op and
limit-ceiling boundaries, two beings with 4,096 topics each meeting for 32 turns, and a refused
meeting that changes nothing. Scale is in the [table above](#host-set-limits): a whole meeting
takes 7.2 µs at 10,000 beings. $0 API spend.

## Life cycle and threat response (pass 2, C1/C2)

`gamectx::character::survival` (`Survival`, local kind `survival`) gives beings a life course and
a way to meet danger. Scores are signed millionths.

**Life cycle.** A being is `born` with a lifespan in ticks (and optional parents, who must be
living adults). It is a child below 22.5% of its lifespan, an adult below 75%, then an elder;
`age` reports `came_of_age` and `grew_old` (`life_stages_by_lifespan`) and lets every being past
its lifespan die of old age. Adults `pair`, and beings `own` belongings. When a being dies
(`age` or `die` with a cause), each belonging goes in order to the next heir with room, round
robin. Heirs are living children by birth, else the living partner, else living parents.
Anything no heir can take is unclaimed. When the weaver dies, the loom, shears and spindle
go to the elder son, the daughter and the elder son (`life_children_inherit_round_robin`).
The bereaved get grief proposals (affect `emotion`: partner −60, child −50, parent −40), and a
widowed partner may pair again.

**Threat response.** `threat` appraises danger as
`danger·10,000 + (threat strength − strength)·3,000 + injury·2,000`, clamped to 0..1,000,000.
Below 200,000 the being ignores it. Otherwise five responses are scored, each plus a temperament
offset (`bold`, `steady`, `cautious`, `timid`):

- fight: `300,000 + (strength − threat strength)·5,000 + min(allies, 5)·100,000 − injury·4,000`
- flee: `(exit ? 400,000 : 0) + danger/2 − injury·3,000`
- hide: `(cover ? 450,000 : 0) + danger/3`
- call for help: `250,000 + min(allies, 5)·50,000` with allies near, else 0
- freeze: `danger/2 − 200,000 + injury·2,000`

The highest wins. A strong bold guard fights (`threat_bold_fights`, 700,000), a cautious shepherd
with cover hides, a timid hurt child freezes, and the same guard flees at injury 80
(`threat_injury_changes_choice`). Safety is a need: a threat lowers it to at most
`100 − danger/10,000`, it recovers by one per tick on `age`, and below 40 the being is `unsafe`
(`safety_need_recovers`). Fear is proposed to affect.

**Evidence (C1/C2):** 25 conformance cases (`conformance/survival/`, 6 malformed), 2,000
differential parity workflows (`eval/character/survival_cases.py`, seed 7, 0 mismatches: 1,473
accepted, 527 refused, 4,612 births, 2,791 deaths, 320 belongings inherited, 1,249 threat
responses). Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples with
limits, 64/65-op and limit-ceiling boundaries, a parent with 256 children leaving 4,096
belongings, lifespans at the clock bound, and a refused op that changes nothing. Scale is in the
[table above](#host-set-limits). $0 API spend.

## Level of detail for minds (pass 2, D2)

`gamectx::character::detail` (`Detail`, local kind `detail`) lets a large cast cost little when
it is far from the player. Each tracked being has needs that grow linearly (level and rate per
tick in millionths of full, a threshold that makes the need urgent, default 700,000). Its detail
level follows its distance from the player or camera, set by the host:

- `full` (distance up to `near`, default 20) is brought up to date on every `tick`;
- `coarse` (up to `far`, default 100) only on multiples of `stride` (default 10);
- `crowd` beings are frozen in a named group. `crowd` summarizes a group (members, the mean of
  each need now, how many needs are urgent), and a crowd member is caught up when it `move`s.

`configure` sets `near`, `far` and `stride` before anything is tracked. Because growth is linear,
catching up after any gap gives exactly the levels and urgent ticks that full updates would
have, so moving between levels never loses or invents state: three beings with the same needs at
full, coarse and crowd detail all report thirst at tick 24 and hunger at tick 30 and end with the
same levels (`lod_same_state_every_level`). What differs is when an event is delivered (a crowd
member's events arrive with the `move` that brings it near, `lod_crowd_frozen_until_promoted`)
and the reported work. Over 100 ticks with two needs, a full being costs 200 need updates, a
coarse one 20 and a whole crowd group 1 per tick call (`lod_work_by_level`). `tick` visits only
full and coarse beings, so far crowds cost nothing per being. `assess` shows a being's exact
state now and its `lag` without changing anything.

**Evidence (D2):** 25 conformance cases (`conformance/detail/`, 6 malformed), 2,000 differential
parity workflows (`eval/character/detail_cases.py`, seed 7, 0 mismatches: 1,481 accepted, 519
refused, 1,754 urgent events, 2,792 level moves, 449 promotions out of a crowd). Robustness
(`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits, 64/65-op and
limit-ceiling boundaries, 64 needs at the top rate over 10¹² ticks at every level without
overflow, index consistency, and a refused move that changes nothing. Scale is in the [table
above](#host-set-limits): `assess` takes 1.9 µs and `track` 3.9 µs per op at 10,000 beings. Work
units are counts, not timings; $0 API spend.

## Save growth: retention and migration (pass 2, D3)

`gamectx::character::retention` (`Retention`, local kind `retention`) keeps saves from growing
without end and lets old saves load after the game changes. It never reads the other modules'
state itself: the host registers what it keeps and applies the result.

**Retention.** The host lists what a save keeps (memories, lessons, expectations, ledgers,
other-models) as entries in named collections, each with an importance 0–100, a size in bytes,
when it was added and last used, and whether it is pinned. Each collection has a rule:
`max_entries`, `max_bytes`, `max_age` (ticks since last use), `min_importance` and a `grace`
period that protects anything used recently. `prune` removes entries in a fixed order and says
why: `stale` first, then `unimportant`, then the least important and least recently used until
the collection fits (`over_entries`, `over_bytes`; `keep_reasons_in_order`). Pinned and in-grace
entries are never removed, and a collection that still does not fit is reported as `over`
(`keep_grace_protects_recent`). `touch` records a use (a recalled memory is kept longer,
`keep_touch_refreshes`) and can change importance or the pin, and a `dry_run` reports without
removing anything.

**Migration.** The host declares numbered migrations (2, 3, …), each a list of changes to
section data: `rename`, `add` (only where missing), `remove` and `clamp` an integer, at a dotted
path where `*` means every element or value. `migrate` upgrades a document from any older
version in order and reports the edits: version 2 renames every character's `mood` to `feeling`
and adds `trust: 50`, version 3 removes `flour` (`migrate_rename_add_remove`, 6 edits). Missing
paths are skipped; renaming onto an existing key or clamping a non-integer refuses the request.
Each path node reached counts as work, capped at 1,000,000 per `migrate`.

**Measured save growth.** `eval/character/retention_cases.py --soak` runs the reference over a
seeded town of 50 beings for 30 days (24 ticks a day): each being adds six memories, three
expectations and two ledger records a day, a lesson every third day, and recalls two memories;
the host prunes once a day. With no rules the kept entries reach 3,367,277 bytes on day 30 and
grow by about 1.12 MB every ten days. With rules (memories: 7 days, importance 20, one day of
grace, 2,000 entries; expectations: 3 days; ledger: 14 days, importance 30; lessons: 500
entries) they reach 945,450 bytes (28.1%; 4,254 entries instead of 17,000) and grow by 75,135
bytes from day 20 to day 30. The numbers are the reference's sizes as declared by the host, not
the size of a real save file, and they depend on the rules chosen; the output is retained in
`eval/character/retention-soak.json` and checked in CI.

**Evidence (D3):** 25 conformance cases (`conformance/retention/`, 7 malformed), 2,000
differential parity workflows (`eval/character/retention_cases.py`, seed 7, 0 mismatches: 1,477
accepted, 523 refused, 994 entries pruned, 3,746 documents migrated with 608 edits). Robustness
(`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits, 64/65-op and
limit-ceiling boundaries, 4,096 entries of 16 MiB pruned to one in a single op, a migration of
256 changes over 2,001 nodes and its refusal past the work cap, and a refused migrate that
changes nothing. Scale is in the [table above](#host-set-limits): `touch` takes 1.3 µs at
10,000 entries. $0 API spend.

## Status, roles and jobs (pass 2, B7)

`gamectx::character::roles` (`Roles`, local kind `roles`) gives a community jobs that run
themselves and a pecking order that matters.

**Roles and duties.** A `role` (guard, baker, elder) has a status 0–100, optional `seats` and
duties that recur `every` ticks from tick `at`, each with an action, an optional place and a
priority. Beings are `assign`ed roles (and `vacate` them). `tick` proposes one task per duty that
came due since the holder last ran it, as a task-engine proposal (`{engine: "task", op:
"propose"}`, with the place as a `target_bindings` slot). After a quiet gap it still proposes one
task, for the latest due time, and reports the earlier ones as `missed`: after 50 ticks the
guard gets one patrol for tick 48 with 5 missed (`roles_gap_reports_missed`). A new holder starts
from the tick it was assigned (`roles_new_holder_starts_now`). Roles with fewer holders than
seats are listed as `unstaffed` on every tick until filled (`roles_unstaffed_seats`).

**Status.** A being's status is its highest role status plus earned standing, clamped to
0–100. `esteem` changes standing by at most 20 a time, within −50..50, with a reason; beings with
no role have status from standing alone. `contest` gives a status edge of 10,000 millionths per
point, for the host to add to a cooperation contest (B3), and the being 20 or more points lower
defers: a guard (60) against a baker (30) has edge 300,000 and the baker defers
(`roles_contest_lower_defers`). `persuade` adjusts a persuasion chance (B1) by 5,000 per status
point and 50,000 when speaker and listener share a role: the elder persuading the baker goes from
400,000 to 675,000 (`roles_persuade_status_and_shared_role`).

**Evidence (B7):** 25 conformance cases (`conformance/roles/`, 7 malformed), 2,000 differential
parity workflows (`eval/character/roles_cases.py`, seed 7, 0 mismatches: 1,466 accepted, 534
refused, 4,787 duty tasks, 104,469 missed duties reported, 155 contests with a deferral).
Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits,
64/65-op and limit-ceiling boundaries, 16 roles of 64 every-tick duties on one being over 10¹²
ticks (1,024 tasks), index consistency, and a refused assignment that changes nothing. Scale is
in the [table above](#host-set-limits): `assess` takes 1.6 µs at 10,000 beings. $0 API spend.

## Economy feedback (pass 2, C4)

`gamectx::character::economy` (`Economy`, local kind `economy`) lets prices answer to what
everyone needs and what gets made off-screen, so scarcity becomes a reason to act.

**Goods and markets.** A `good` has a base price and, in per-mille, an elasticity (1500), a floor
(250) and a ceiling (4000) for its price multiplier, a smoothing `rate` (500) and a `spoil` rate
per period (0). A multiplier of 1000 is the base price, the same unit as community market
`price_mult`, so the host can copy it into market rules. Each market keeps stock per good. During
a period beings `produce` (off-screen work adds stock), `want` (needs register demand) and `buy`
(stock goes down at the current price, and the buyer's want is met).

**Settling.** `settle` closes the period for every market. With supply = stock + sold, the
pressure `(demand − supply)·1000 / (demand + supply)` gives a target multiplier `1000 +
pressure·elasticity/1000` within floor and ceiling, and the multiplier moves `rate` per-mille of
the way there. Demand 30 with no supply targets 2500 and moves to 1750 (`econ_shortage_raises_price`);
stock 40 with no demand falls to 625 (`econ_glut_lowers_price`); balanced periods bring it back
by half the gap each time (`econ_price_recovers_over_periods`); low-elasticity salt rises only to
1250. At 1500 or more a good is `scarce`: beings whose wants went unmet get an aspiration `need`
proposal (weight 10 + unmet, at most 100) and its producers an `opportunity` (multiplier / 50),
so a shortage turns into goals to stock up or to sell (`econ_scarcity_proposes_aspirations`). At
700 or less it is a `glut`. Wants and production then reset, and stock spoils (fish at 500
halves each period).

**Evidence (C4):** 25 conformance cases (`conformance/economy/`, 7 malformed), 2,000
differential parity workflows (`eval/character/economy_cases.py`, seed 7, 0 mismatches: 1,549
accepted, 451 refused, 2,008 scarce and 3,110 glut settlements, 2,364 aspiration proposals).
Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits,
64/65-op and limit-ceiling boundaries, the top base price at an 11× multiplier for a million
units (cost 1.1·10¹⁶) without overflow, and a refused op that creates no market. Scale is in the
[table above](#host-set-limits): `price` takes 0.9 µs and `want` 2.1 µs per op at 10,000 traders.
$0 API spend.

## Space inside a place (pass 2, C3)

`gamectx::character::space` (`Space`, local kind `space`) gives a place a floor plan, so beings
take up room, get in each other's way, line up and lose sight of each other behind walls.

**Rooms and movement.** A `room` is a grid of up to 256 × 256 cells with wall cells. Beings
`enter` a free cell (one being per cell) and `exit`. `move` walks to a target cell by the
shortest four-way path; walls and other beings block the way, the reply gives the steps, and a
target with no path is refused. Mira crosses the yard around the wall in 9 steps
(`space_walk_around_wall`); with guards standing in both gaps she cannot get through
(`bad_guards_block_gaps`) until one steps aside (`space_beings_block_paths`).

**Spots and queues.** A `spot` (a well, a counter) sits on a cell with a capacity. `use` takes a
free place when the being stands on or next to the spot (eight-way), otherwise it joins a
first-come queue and learns its position; `release` (or leaving the room) frees the place and
serves the next in line, wherever it stands: walking over is up to the host
(`space_well_queue`, `space_served_from_afar`). `crowd` lists who is in a room, its free cells
and each spot's users and queue.

**Line of sight.** `see` draws an integer Bresenham line between two beings; any wall on the
cells between them blocks it (the reply names the first such cell), and an optional range
limits it by Chebyshev distance. Beings in different rooms never see each other. Mira at (0,2)
cannot see Tomas at (6,1): the wall cell (3,1) is in the way (`space_line_of_sight_blocked`).

**Evidence (C3):** 25 conformance cases (`conformance/space/`, 7 malformed), 2,000 differential
parity workflows (`eval/character/space_cases.py`, seed 7, 0 mismatches: 1,472 accepted, 528
refused, 4,251 steps walked, 288 queued uses, 75 blocked sight lines). Robustness
(`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits, 64/65-op and
limit-ceiling boundaries, a 256 × 256 maze at the 4,096-wall bound that forces a walk along 16
rows, a 255-cell sight line, index consistency, and a refused move that changes nothing. Scale is
in the [table above](#host-set-limits): `see` across the square takes 2.4 µs and `enter` 2.1 µs per
op with 10,000 beings in one 256 × 256 room. $0 API spend.

## Slow personality change (pass 2, B4)

`gamectx::character::drift` (`Drift`, local kind `drift`) lets long experience reshape a being a
little, within limits the author sets, so a betrayed character becomes more guarded instead of
only angry.

**Traits and pressure.** An author gives a being traits (values and temperament dimensions such
as trust or boldness) as signed millionths with a hard `cap` on how far experience may move them
from the authored base (default 150,000, at most 300,000). `experience` pushes a trait by −100..100
with a reason. Pushes add up in a pressure that fades by `fade` per tick (default 1), so isolated
events wash out (`drift_pressure_fades`). Each time the pressure reaches the `threshold` (default
300) either way, the trait moves one `step` (default 10,000) in that direction and the shift is
recorded with the reasons that built it, the newest 8: three blows of −100 (betrayed, robbed,
lied to) move Tomas's trust from 600,000 to 590,000 (`drift_betrayal_makes_guarded`), one alone
does nothing, and kindness can bring it back (`drift_kindness_undoes`). At the cap the pressure
is spent without change (`drift_hard_cap`), and no trait leaves −1,000,000..1,000,000.

**Author control.** `lock` freezes one trait or the whole being (pushes are then ignored and
reported as locked), `revert` returns a trait to its base while keeping the history, and a cap
of 0 fixes a trait outright. `assess` shows each trait's value, drift, faded pressure and
history.

**Evidence (B4):** 25 conformance cases (`conformance/drift/`, 7 malformed), 2,000 differential
parity workflows (`eval/character/drift_cases.py`, seed 7, 0 mismatches: 1,488 accepted, 512
refused, 610 shifts, 40 experiences at the cap, 307 locked experiences). Robustness
(`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits, 64/65-op and
limit-ceiling boundaries, 6,400 blows at threshold 1 that stop at the 300,000 cap with exactly
256 history entries, fading across 10¹² ticks, and a refused push that changes nothing. Scale is
in the [table above](#host-set-limits): `experience` takes 2.4 µs at 10,000 beings. $0 API spend.

## Default packs (pass 2, D5)

`gamectx::character::packs` (`Packs`, local kind `packs`) gives a small game living characters with
little authoring. Packs are original, ready-made data; `apply` turns them into ordinary requests
for the engines that already exist and changes none of them itself, so the host runs (or edits)
the requests as it would its own.

**Built-in packs.** Four species give body needs with a threshold, a rate and a satisfying action,
plus a lifespan in hour ticks: a human (hunger met by cooking, rest, company; 613,200 hours), a dog
(feeding, rest, petting; 105,120), a horse (feeding, rest, riding; 219,000) and a goat (feeding,
rest, roaming; 131,400). Five temperaments give a cognition temperament, a threat temperament and
starting boldness, trust and warmth: bold (impulsive, boldness 600,000), steady, cautious
(careful), timid (careful, boldness −600,000) and dreamy. Five roles give status, seats and a duty:
guard (patrol the gate every 6 hours), baker (bake at 05:00), farmer, elder (three seats, counsel at
18:00) and healer. Three norm sets give personal norms: village (no theft, no wrecking), guild
(answer calls for help, no undoing guild work) and wanderer (no roots, travel light).

**Apply.** `apply {being, species?, temperament?, roles?, norms?}` returns the requests in a fixed
order: `body` `init` with the species needs, `survival` `born` with the lifespan, `cognition`
`profile`, `drift` traits, `roles` (each role defined, then assigned) and `creed` `set_norms`, plus a
hint with the lifespan and the threat temperament to pass to `threat` calls. A bold human baker of
the village becomes six requests (`packs_human_bold_baker`); a temperament alone gives only the
mind (`packs_dreamy_mind_only`); two norm sets merge into one list (`packs_two_norm_sets`) but may
not repeat a tenet (`bad_duplicate_tenet`). The host sets how many roles (default 2, up to 4) and
norm sets (default 2, up to 4) one apply may name.

**Author packs.** `define` adds a species, temperament, role or norm set of the same shape, with
defaults filled in (`packs_define_species_fills_defaults`); built-in ids cannot be replaced
(`bad_replace_builtin`), and needs and duties must use known task actions (`bad_unknown_action`).
`list` and `show` read packs back.

**Evidence (D5):** 25 conformance cases (`conformance/packs/`, 7 malformed), 2,000 differential
parity workflows (`eval/character/packs_cases.py`, seed 7, 0 mismatches: 1,463 accepted, 537
refused, 6,286 applies emitting 16,534 requests, 2,724 custom defines). Robustness
(`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits, running every
request an accepted apply emits through its own engine; all twenty species and temperament
combinations with four roles and three norm sets accepted by all six engines; 64/65-op and
limit-ceiling boundaries; the 16-tenet merge cap; and a refused redefinition of a built-in that
changes nothing. Scale is in the [table above](#host-set-limits): `apply` takes 6.8 µs median with 10,000 custom packs (it builds six requests, so it is quoted apart from lookups). $0 API spend.

## Turn frame and salience (pass 3, stage 1)

`gamectx::character::frame` (`Frame`, local kind `turn-frame`) puts a character in the moment. The
earlier modules each answer one question; this one fuses their answers for a single turn and
decides what a small model gets to see.

**Trajectory sense.** `observe` keeps a bounded window of recent values per being and key, and
`trend` reads its momentum. Trust that fell 50 in the last few ticks is an `acute_drop`
(`frame_trust_acute_drop`), while trust of −45 that has not moved for 600 ticks is `settled_low`,
cold rather than shocked (`frame_trust_settled_enmity`). `outcome` counts successes and failures:
three failures in a row are a `frustration_spiral`, three successes `flow`. `time` turns a tick gap
into an anchor a line can use (`moments_ago`, `earlier_this_shift`, `yesterday`, `overdue` by so many
hours), with the ticks per hour set by the host (`frame_time_labels`).

**Turn frame.** `frame` takes the slices the host read from the other engines (body, winning
motive, active task, affect, its model of the listener, the last off-screen activity, the most
novel thing in view, and who else is present) and returns one validated, bounded struct with
derived fields: urgent body items by level, whether the motive is strong enough (score ≥ 90) to
override polite talk, whether the task is pressing (due within the hour or overdue), the three
strongest emotions, this being's trends and streak, and the untrusted bystanders in earshot. Mira,
exhausted with bread about to burn and a gossip at the counter, comes out with fatigue urgent, the
bread motive overriding talk, the task pressing and `gossip` in `untrusted_in_earshot`
(`frame_mira_turn`).

**Salience ranker.** `rank` scores each candidate context item 0–1000 as a weighted sum of six
parts: urgency, listener relevance (the listener or an earshot bystander is involved), utterance
cues (entities, tags and places the speaker named), task relevance, mood congruence (grievances
rise in a dark mood, favors in a warm one) and recency. Items in one causal `group` are packed or
dropped together, mandatory groups are always kept (an overrun is reported as `over_budget`), and
the rest go in by salience per byte until the byte budget is used. Every dropped item says why:
`budget` or `irrelevant`. Out of eight items in Mira's scene the ranker keeps the burning bread,
the firewood promise, both theft clues, the gossip's presence and the apprentice's past kindness,
and drops the shop's founding lore and a family recipe (`rank_mira_scene`); a group too large for
the budget is dropped whole, never split (`rank_budget_drops_group_whole`).

**Evidence (pass 3 stage 1):** 25 conformance cases (`conformance/turn-frame/`, 7 malformed), 2,000
differential parity workflows (`eval/character/frame_cases.py`, seed 7, 0 mismatches: 1,489
accepted, 511 refused, 1,427 frames, 2,493 rankings keeping 1,677 items and dropping 2,971, 49 acute
trends). Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples with
limits, checking for every accepted ranking that the kept bytes equal `used`, that only mandatory
items exceed the budget and that no score passes 1000; 64/65-op and limit-ceiling boundaries;
4,096 items of 65,536 bytes at full weights and the 10¹² clock without overflow; a streak clamped
at −1,000; and a refused observation that changes nothing. Scale is in the [table
above](#host-set-limits): `frame` takes 3.9 µs median at 10,000 beings. No model was called; the
ranker's effect on reply quality is not measured yet. $0 API spend.

## Memory cueing and the epistemic gate (pass 3, stage 2)

`gamectx::character::cueing` (`Cueing`, local kind `cueing`) lets memories come up when the moment
resembles them, without a memory quiz, and keeps what a character says consistent with what it
knows and what it is hiding.

**Cueing.** Episodes carry structural features: actions, entities, tags and places. `cue` scores
each episode by what it shares with the current situation (actions 3, entities 2, tags and places 1
each). The player asking to borrow a tool cues the time they returned a loan early
(`cue_loan_recalls_early_return`); talk of the oven touches a feared fire and comes back under
`avoid`, so the line deflects instead of reciting it (`cue_fear_topic_avoided`); secret episodes are
never recall candidates. A memory surfaced to a listener is `cooling` for 24 ticks by default, so it
is not repeated on the next turn (`cue_cooldown_suppresses_repeat`), but it may still be told to
someone else (`cue_cooldown_per_listener`). Only `surface`, called by the host after a line is
delivered, records that a memory came up; a proposal that was never spoken does not count.

**Epistemic framing.** Every fact records how the being knows it. `frame_facts` returns the framing
a line must use: `state` what was witnessed, `attribute` a trusted report to its source, `hedge` a
rumor, `guess` an inference, and `correct` a corrected belief with what was believed before
(`facts_framings`). A `cover` story masks a fact for chosen listeners (or everyone): for them the
framing becomes `cover`, the claim becomes the lie, and the truth is not passed on at all
(`facts_cover_for_listener`).

**Reply gate.** `gate` checks a proposed reply, usually from a model, word by word. A key word of
a masked fact is a leak unless the cover story itself uses it, and so is a key word of a host-only
`secret` the being does not hold. A leaking reply is refused with the cover story, or the host's
fallback line, in its place: "Between us, the boy STOLE it." to the gossip becomes "The pouch is
safe in the back" (`gate_blocks_cover_leak`), and Mira cannot let slip a vault code only the warden
knows (`gate_blocks_unknown_secret`). The gate matches whole key words: paraphrases and synonyms
the author did not list pass, so it is a backstop, not a proof.

**Evidence (pass 3 stage 2):** 25 conformance cases (`conformance/cueing/`, 7 malformed), 2,000
differential parity workflows (`eval/character/cueing_cases.py`, seed 7, 0 mismatches: 1,471
accepted, 529 refused, 1,086 recalled, 575 avoided and 16 cooling episodes, 262 replies blocked and
1,845 passed). Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of three samples
with limits, replaying each accepted workflow op by op and re-checking every passed reply against
the state it ran on (no masked key word outside the told cover story, no unknown secret word);
64/65-op and limit-ceiling boundaries; a 2,000-character reply against 4,096 secrets; and a refused
cover that changes nothing. Scale is in the [table above](#host-set-limits): `cue` takes 3.6 µs median at 10,000 beings. No model was called; how often real model replies trip the gate is not
measured yet. $0 API spend.

## Pragmatics (pass 3, stage 3)

`gamectx::character::pragmatics` (`Pragmatics`, local kind `pragmatics`) reads what a player turn is
doing, decides what the character answers first, and sets how forthcoming it is.

**Clauses and subtext.** `parse` splits a turn into at most four clauses and classifies each with
an original phrase lexicon: "Here is the firewood you asked for—also, is it true the apprentice
stole from you?" is a transfer to receive and a probing question (`parse_firewood_and_question`);
"Sorry I'm late with the flour. Here it is. Did anyone come looking for me?" is an apology to
appraise, an item to receive and a question (`parse_apology_transfer_question`). A threat outranks
the request it wraps (`parse_threat_beats_request`). Subtext rules use the context the host passes:
praise for bread the context says burned is sarcastic (`parse_sarcastic_praise`), and asking about a
fire the speaker witnessed is a test of honesty (`parse_testing_honesty`). Authors add phrases per
act (`lexicon_author_phrase`), and when a model proposes its own clauses, `validate` accepts them
only with known acts and texts found in the turn in order.

**Agenda.** `agenda` blends the clauses' obligations with the character's own. With the bread
burning (a motive of 92) the firewood is still received, but both replies are deferred
(`agenda_override`); a smith answers the question and then asks where the borrowed cart is
(`agenda_counter_ask`); a strong grievance holds a plain question back until it is addressed, while
an urgent plea is still answered (`agenda_grievance_gate`). The thread stack keeps up to four
conversations per being: a crash interrupts the theft talk, and ten minutes later it resumes with
its unanswered question and `moments_ago` (`threads_interrupt_and_resume`).

**Stance.** `stance` turns trust, affection and resentment into cooperativeness, keeps a
confidential reply open only without untrusted ears (whispered at trust 60 or more, otherwise
deflected while the gossip listens), and cuts replies to one sentence when the task is urgent or
due within the hour (`stance_privacy_and_brevity`).

**Limits.** The lexicon is English and phrase-based: irony with no context cue, indirect requests
and phrasings the lexicon lacks are read as small talk or a plain question. Its accuracy on real
player turns is not measured yet.

**Evidence (pass 3 stage 3):** 25 conformance cases (`conformance/pragmatics/`, 7 malformed), 2,000
differential parity workflows (`eval/character/pragmatics_cases.py`, seed 7, 0 mismatches: 1,468
accepted, 532 refused, 9,475 clauses parsed, 41 conditional gates, 535 overrides, 80 threads
resumed). Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples with
limits, checking that accepted parses keep 1–4 clauses with words and that validated clauses appear
in order; 64/65-op and limit-ceiling boundaries; a 500-character turn of 250 sentences; multi-byte
text around boundaries; 4,096 author phrases; and a refused lexicon entry that changes nothing.
Scale is in the [table above](#host-set-limits): parsing a two-clause turn takes 10.0 µs median,
the same at 1,000 and 10,000 beings (it is text work, so it is quoted apart from lookups). $0 API
spend.

## Embodiment (pass 3, stage 4)

`gamectx::character::embodiment` (`Embodiment`, local kind `embodiment`) gives a turn a body and a
voice, so a character does not deliver every line standing still in the same tone.

**Tension.** People want incompatible things at once. `tension` reads a motive near-tie (fleeing
at 70 against keeping a promise at 62 is `torn_hesitation`, `tension_motive_near_tie`; two close
hunger motives are no conflict), ambivalence toward someone (Mira's affection of 55 and fresh
resentment of 50 toward the apprentice are `bittersweet_warmth`, `tension_bittersweet`; resentment
over affection is `resentful_obligation`), a norm broken under duress (`reluctant_compliance`) and
deception, which outranks the rest (`guilty_concealment`). Each mode carries extra task failure
risk, hesitation ticks and a tone for the line.

**Micro-reactions.** `react` picks a posture, a gaze and a hand activity by fixed rules. With
bread burning and a gossip at the counter, Mira keeps working and watches the bystander
(`react_mira_bread`); hostile and shocked over a broken heirloom she holds, she turns away, stares
at it, sets down her tool and says nothing (`react_hostile_silent_turn`); a liar avoids eye contact
(`react_lying_avoids_eyes`). The same reaction twice within three ticks is reported as `repeated`,
so the host can word it differently (`react_repeat_within_window`).

**Voice.** `voice` cuts a reply to one terse sentence under fatigue (80+), pain (50+), arousal
(75+) or task urgency (80+), picks a pacing (breathless after exertion, trembling with fear, hesitant
when lying or torn, clipped when terse, warm and relaxed with affection in a safe place, else
steady) and gives speed, pitch, pause and volume hints for line selection or speech synthesis; a
whisper drops the volume (`voice_hesitant_whisper`).

**Evidence (pass 3 stage 4):** 25 conformance cases (`conformance/embodiment/`, 7 malformed), 2,000
differential parity workflows (`eval/character/embodiment_cases.py`, seed 7, 0 mismatches: 1,562
accepted, 438 refused, 2,531 tensions, 529 silent turns, 788 repeated reactions, 1,936 terse
voices). Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples with
limits, checking that accepted voices keep 1–8 sentences (1 when terse) and that a silent turn is
always hostile or shocked; 64/65-op and limit-ceiling boundaries; 64 attitudes at full strength; the
widest repeat window at the 10¹² clock; and a refused reaction that changes nothing. Scale is in the
[table above](#host-set-limits): `react` takes 1.9 µs median at 10,000 beings. The hints are not
yet passed to speech synthesis. $0 API spend.

## World and social schemas (pass 3, stage 5)

Characters react to the world they stand in and the society they belong to. Two modules give the
host declarative definitions for both, so these rules are data rather than game code.

**Surroundings** (`gamectx::knowledge::surroundings`, local kind `surroundings`). A declared day
splits into phases with a light level and a temperature shift, and weather kinds change light,
noise, travel cost and need rates outdoors (`rain_dims_the_day`; indoors only travel changes,
`indoors_ignores_rain`). Places carry an owner, an access rule that can change after hours (a
customer may walk into the bakery by day and trespasses at night while Mira may still enter,
`shop_private_after_hours`), a privacy that sets the earshot and whether secrets can be discussed
(three people in a parlor for two are crowded and can no longer talk privately,
`crowding_and_earshot`), and stations shared first come, first served with a queue
(`oven_queue_promotes`). Actions leave traces: a dropped sack leaves a flour trail anyone sees as an
inferred fact, never as who did it, until someone sweeps (`flour_seen_then_swept`); forced lock
marks are only found on inspection (`lock_marks_need_inspection`); traces fade (`trace_fades`).
Items keep their legal owner apart from whoever holds them: a lent ladle is no theft
(`loan_and_return`), but the apprentice holding Mira's coin pouch is, and because she holds it
dear a loss weighs three times as much (`stolen_heirloom_pouch`); a thief who sells a stolen knife
passes possession, not ownership (`fence_sale_keeps_owner`).

**Social fabric** (`gamectx::knowledge::fabric`, local kind `fabric`). Norms are graded from
etiquette to crime and apply in given situations (`norm_needs_its_situation`). Each breach climbs a
ladder kept per norm, offender and witness: muddy boots earn a warning, then refused service, and
never more (`etiquette_ladder_capped`); haggling escalates to a confrontation on the third time
(`custom_repeats_escalate`); a theft goes straight to confrontation and a report to the town watch,
and an apprentice with a conscience feels the guilt and wants to make amends
(`theft_reported_with_guilt`); an apology steps the ladder back (`apology_steps_back`). Contracts
record what one being owes another, with witnesses, a due time and a weight: the player's promised
firewood goes overdue (`firewood_overdue`), a broken alibi pact costs trust and the witnesses hear
of it (`settle_broken_and_fulfilled`), and favors owed make the debtor readier to help
(`favor_balance_helps`). Each pair remembers how they met, up to four defining moments, how each
addresses the other and what was left unsaid (`landmarks_replace_and_evict`,
`address_terms_directional`, `unfinished_threads`). Attention profiles decide who notices what (a
vigilant guard sees the knife first, `guard_notices_knife`), and pragmatic profiles give lying
tells that only an observer with enough insight perceives, harder to see in a practiced liar
(`tells_and_style`). Social roles with duties are the pass 2 [roles](#status-roles-and-jobs-pass-2-b7)
module; a severe breach names the role it is reported to.

**Evidence (pass 3 stage 5):** 25 conformance cases each (`conformance/surroundings/`,
`conformance/fabric/`, 7 malformed each), 2,000 differential parity workflows each, most starting
from a small valid world (seed 7, 0 mismatches; surroundings: 1,492 accepted, 508 refused, 1,160
trespasses, 82 traces seen, 9 theft appraisals, 71 queue promotions; fabric: 1,461 accepted, 539
refused, 930 top-of-ladder breaches, 168 late contracts, 47 open threads seen, 86 tells seen).
Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples per module with
limits, checking that stations never exceed their slots and only hold beings inside the place,
occupants stay unique, conditions stay within 0–100, ladder steps stay within 0–3 with a
confrontation exactly at step 3, no tells show without a lie, and pairs keep at most four
landmarks; 64/65-op and limit-ceiling boundaries; a full room, a trace lasting to the 10¹² clock,
32 open threads and 64 stimuli through a 300% bias; and refused ops that change nothing. Scale is
in the [table above](#host-set-limits): `inspect_item` takes 1.8 µs median at 10,000 items and
`dyad` takes 1.4 µs median at 10,000 pairs. Traces, theft appraisals and weather need rates are
not yet fed into beliefs, appraisals and survival automatically. $0 API spend.

## Pass 3 acceptance: the Interrupted Accusation

`tests/acceptance_interrupted_accusation.rs` plays the pass 3 scene through the real engines with no
model. Mira is exhausted and one tick from burning her bread; she has found a flour trail by the
lockbox (an inferred fact that never names who left it) and her coin pouch is gone. The player
brings the firewood she asked for and, with a gossip in earshot, asks whether the apprentice stole
from her. The parser finds two clauses (an item to receive, a probing question); the stance is a
whisper of one sentence because a trusted player and an untrusted gossip share the room; the agenda
puts the oven first, takes the firewood and defers both replies; the turn frame keeps the burning
bread, the promise, the theft suspicion, her feelings about the apprentice and the gossip, and drops
45 lines of biography and lore; embodiment gives a bittersweet tension, hands that keep working,
eyes on the gossip and a breathless whispered voice; the explain trace says she pulled the bread
because it burns in one tick. A crash interrupts the conversation; once the gossip has left, the
shop is private enough for secrets, the stance is open, and `resume` brings back the unanswered
theft question with a tender tone and room for four sentences. Running the scene twice gives
byte-identical output. $0 API spend.

## Pass 3 deep roleplay scenarios

`eval/character/roleplay_deep_test.rs` drives three harder scenes through the engines; CI replays
them against the committed report `eval/character/roleplay-deep.json` byte for byte. Model drafts
in the scenes are fixed strings standing in for untrusted model output.

1. **High-stress multitask and boundary probe.** At noon with the bread burning, two customers
   queued for one counter and a customer in earshot, the player says six things at once. The parser
   keeps four clauses (greeting, an urgent request, small talk, a probing question about the vault
   key) and counts two as overflow; the oven overrides the agenda and every reply waits; the stance
   deflects to one sentence; the frame keeps the bread, the request and the queue within 150 bytes
   and drops twenty pieces of town gossip. A model draft that invents a clause is refused, as is an
   over-long utterance. A draft reply that says where the key hangs is caught by the gate and
   replaced by Mira's cover story. The player's trespass into the back room is refused without
   changing anything; taking the coin pouch gives Mira a theft appraisal weighing three times a
   plain loss and puts the theft at the top of the ladder with a report to the town watch. Mira
   squares her shoulders, clenches a fist and answers in one clipped sentence; the explain trace
   says she pulled the bread because it burns this tick, beating the confrontation at 90.
2. **Deception under interrogation.** The guard asks where the apprentice was and whether he took
   the pouch (a probing personal question and a question that tests honesty, since the pouch was
   witnessed). With Mira in earshot he deflects. The frame shows the model only his cover story;
   the consistent lie passes the gate, a slip ("I only borrowed a few coins from the pouch") is
   replaced by the cover story; a changed story is flagged per listener, and the flour trail
   exposes the lie. A guard of insight 60 sees one tell, Mira (75) two, nobody any when he tells
   the truth. Deception outranks his awe of the guard (guilty concealment); he slumps, cannot meet
   their eyes and hesitates. His conscience brings guilt and a wish to make amends, recorded as an
   owed apology of weight 60.
3. **Interrupted conversation and resumption.** The player's question about the rye recipe is
   interrupted by a customer; once the order is closed `resume` brings it back ("earlier this
   shift"). A fire bell interrupts it again; a day later it comes back ("yesterday"), and the pair's
   dyad still lists it as unfinished after 250 ticks. Four new threads push it off a stack of four,
   reported as dropped. The promised lesson surfaces by analogy, is cooling ten ticks later instead
   of nagging, and may return after the cooldown; asked a personal question, she answers and then
   asks her own follow-up.

**Found:** the built-in lexicon reads "Nice bread, as always" as sincere small talk rather than a
compliment, so no reply is owed; authors can add such phrases with `lexicon`. 53 checks, 0
failures, $0 API spend.

## Genre profiles (pass 4, stage 1)

`gamectx::knowledge::genre` (`Genres`, local kind `genre`) lets the same engines run any setting
by data alone. The engines keep a few closed lists (light levels, access rules, privacy, norm
severities, contract kinds, address terms); a genre names its own word for each entry, in the
engines' order, so decisions and conformance outputs never change and only the words do. Four
genres are built in: `village` uses the engine words, and `orbital_station`, `noir_city` and
`academy` are original settings (`show_orbital_station`). An author defines more from a base,
renaming only what differs (`define_from_station`).

`render` shows an engine decision in a genre's words under the keys the host names: the same
refused entry, theft and contracts read `quarters`, `alcove` and `station_offense` on the station
and `booth`, `alibi` and `felony` in the noir city (`render_station_decision`,
`render_noir_decision`, `portability_four_genres`). `normalize` maps genre words back to engine
words and refuses anything outside the vocabulary, so a player's or a model's "penthouse" never
reaches an engine (`bad_normalize_unknown_word`); rendering and normalizing gives back the decision
unchanged (`round_trip_same_decision`). Lexicon packs teach the pragmatic parser a setting's
phrases ("out the airlock" as a threat, "evening pal" as a greeting) and turn into ordinary
`pragmatics` lexicon requests (`pack_and_requests`).

**Evidence (pass 4 stage 1):** 25 conformance cases (`conformance/genre/`, 8 malformed), 2,000
differential parity workflows with random nested values drawn from every genre's words
(`eval/character/genre_cases.py`, seed 7, 0 mismatches: 1,487 accepted, 513 refused, 5,401 renders,
1,887 normalizations, 1,136 definitions). Robustness (`tests/robustness.rs`) covers 1,500 mutations of
each of two samples with limits, checking that every accepted render without unmapped words
normalizes back to its input; 64/65-op and limit-ceiling boundaries; a 100,000-node value at the
node limit and one node over; 64 and 65 levels of nesting; and a refused redefinition of a built-in
genre that changes nothing. Scale is in the [table above](#host-set-limits): `render` takes 3.8 µs
median at 10,000 genres. Engines do not read a genre directly yet: hosts call `normalize` before
and `render` after. $0 API spend.

## Mind architectures (pass 4, stage 2)

`gamectx::character::minds` (`Minds`, local kind `minds`) gives a being a kind of mind so that a
hive drone, a ship's steward, a goat or a household spirit can run on the same engines as a person
without becoming a person in disguise. A mind says where memory lives (`individual`, `shared` in a
named pool, or `none`), which of the eight affect channels exist, whether the being has language
(`full`, `signals` for tone only, or `none`), whether it can deceive (`none`, `concealment`,
`full`), how deep its theory of mind goes (0, 1 or 2), how fast it lives (a time scale in
millionths) and an optional fixation. Five original minds are built in (`human`, `hive_tender`,
`steward_intelligence`, `herd_animal`, `hearth_spirit`; `show_human`, `show_hearth_spirit`), and
authors define more (`define_custom_mind`). Unassigned beings are human.

`admit` is a gatekeeper in front of the existing engines, which stay unchanged: the host (or a
model proposal) names an engine request for a being, and `admit` returns it allowed, adjusted or
refused with reasons. A hive drone's memories go to its hive's pool without an emotion the hive
does not feel (`hive_episode_to_pool`); a goat can neither tell a cover story nor model what others
believe (`herd_cover_two_reasons`), cannot lie in its body language, loses the guilt channel and
the "who is watching" check (`herd_react_lying_and_channels`, `herd_stance_no_bystanders`) and
takes in tone but not words (`herd_parse_signals_only`); a steward intelligence can conceal but not
invent (`steward_cover_cannot_lie`) and covers an hour of conversation in a tenth of the ticks
(`steward_resume_fast`); a hearth spirit's cue cooldown stretches twentyfold
(`hearth_cue_stretched`); moss without memory or language is refused both
(`mindless_memory_refused`, `mindless_speech_no_language`). A human passes through untouched
(`human_passthrough`). `fixate` keeps a fixation at the top of a motive list, scored at least as
high as anything else, until `release` (`fixate_pins_goal`, `release_then_fixate`); `hear` says
whether a being takes in words, tone or nothing (`hear_three_minds`). A mind cannot be redefined
to shared memory while a being holds it without a pool (`bad_redefine_breaks_pool`).

**Evidence (pass 4 stage 2):** 26 conformance cases (`conformance/minds/`, 7 malformed), 2,000
differential parity workflows with every built-in and two authored minds assigned
(`eval/character/minds_cases.py`, seed 7, 0 mismatches: 1,539 accepted, 461 refused, 6,358
admissions with 1,005 refused, 388 stripped inputs and 413 pool routes; seed 1 also 0 mismatches).
Parity found a redefinition hole (a mind turned shared while a being held it without a pool) that
is now refused. Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples
with limits, checking that admission is allowed exactly when no reason is given, refusals carry no
request, stripped inputs are gone, targets are the being or its pool, every being is pooled exactly
when its memory is shared, and a fixation heads its motives with the top score; 64/65-op and
limit-ceiling boundaries; a 10¹²-tick cooldown stretched a thousandfold without overflow, the
1-tick floor, an out-of-range cooldown left as sent; and a refused assignment past the limit that
changes nothing. Scale is in the [table above](#host-set-limits): `admit` takes 2.1 µs median at
10,000 beings. Engines do not consult a mind directly yet: hosts call `admit` before each request.
$0 API spend.

## Defense mechanisms and strain (pass 4, stage 3)

`gamectx::character::defenses` (`Defenses`, local kind `defenses`) lets a character protect its
picture of itself the way people do, and lets that protection wear thin. A profile gives a being a
maturity, a tolerance, the secrets it protects, a preferred style, a strain threshold, a crisis
outcome (confession, flight, collapse or outburst) and a crisis cooldown (`profile_view`). The host
reports a threat to the self from its own appraisals (a lesson failed, a sanction, an exposed lie),
never from model text: a drop in competence, goodness or belonging larger than the tolerance, or an
exposed protected secret, is ego-threatening; anything smaller passes (`below_tolerance_no_defense`,
`unprotected_secret_ignored`).

The mechanism is chosen by a fixed first-match rule, never by a model. Effective maturity
(maturity minus up to 60 points as strain rises) gives a band; the being's style is tried first,
then the band's defaults; a mechanism that needs an input it lacks is skipped
(`style_skipped_without_input`). A low-maturity quartermaster denies the open lockbox
(`secret_exposed_denial`), blames the apprentice (`projection_by_style`) or snaps at the apprentice
instead of the captain (`displacement_to_safer_target`); a middling one makes an excuse tagged
self-serving (`rationalization_self_serving`), talks procedure or goes quiet
(`mid_without_excuse`); a mature one jokes (`humour_high`) or works harder on a project
(`sublimation_project`). Every result carries the speech or behaviour effect, the tagged
distortion (the host's truth is untouched) and an explanation such as "quartermaster snapped at
apprentice because captain_rebuke threatened belonging".

Strain is the cost. Repeated threats move the same being from mature to immature defenses
(`strain_lowers_band`); at the threshold it breaks once, with its own crisis outcome, and strain
drops to half the threshold; inside the cooldown it does not break again
(`crisis_once_then_cooldown`). Immature defenses are cheap now and costly later: contradicting a
denied fact adds half again as much strain (`contradiction_costs_more`), strain decays at half speed
while a denial is held (`decay_slower_while_denying`), and the same fact defended three times is a
defense loop (`defense_loop`). Rest, support and coping relieve strain (`relieve_kinds`); accepting
a denied fact ends the distortion (`accept_drops_distortion`); a new profile, for example after
maturity drifts, keeps strain and distortions (`reprofile_keeps_strain`).

**Evidence (pass 4 stage 3):** 26 conformance cases (`conformance/defenses/`, 7 malformed), 2,000
differential parity workflows with four profiled beings across the bands
(`eval/character/defenses_cases.py`, seed 7, 0 mismatches: 1,461 accepted, 539 refused, 2,148
defenses, 521 crises, 9 defense loops and 34 costly contradictions; seed 1 also 0 mismatches).
Robustness (`tests/robustness.rs`) covers 1,500 mutations of each of two samples with limits,
checking that strain stays within 0–10⁶, each mechanism belongs to the band it reports, a
distortion is left exactly by the immature mechanisms and rationalization, and a loop is three or
more hits; 64/65-op and limit-ceiling boundaries; 63 maximal threats under the widest cooldown that
break exactly once; 256 distortions at the limit and one more refused; full strain decaying to zero
over the 10¹² clock; and a refused self-scapegoat threat that changes nothing. Scale is in the
[table above](#host-set-limits): a defense `threat` takes 3.3 µs median at 10,000 beings. The
breaking-point deep test is still to come. $0 API spend.
