# Roadmap

Release naming: GitHub `v0.4.0a`, Cargo package `0.4.0-a` (in progress). The released
`v0.3.0a` uses Cargo package `0.3.0-a`; the earlier `v0.2.0a` uses `0.2.0-a`.

One line per item. Numbers are from `eval/` on Llama 3.2 3B and Mistral Small 3.2 24B unless
noted; "3 models" adds Llama 3.3 70B.

## In progress for v0.4.0a

- Phase 1, layout: library modules grouped into `data`, `packet`, `knowledge`, `character`
  (with `roleplay`), `runtime`, `deploy`, `media` and `kit`; `studio` split by purpose.
  Examples, docs and eval grouped to match, with indexes. No behavior/format change: all
  conformance cases and frozen artifacts byte-identical; pre-0.4 evidence verified through
  `eval/common/provenance.py`. Breaking import paths are listed in the changelog.
- Phase 2, stage 2.1 done: `character::roleplay::engine`, a configurable roleplay engine
  with 12 switchable stages, a line book (English pack plus host phrases and lines), and
  new host-owned props (`knowledge::props`) that people can look at and interact with.
  62 additive conformance cases. Rust is byte-identical to the Python reference on those
  cases and on 1,500 randomized differential cases. All 108 Lantern Vault greeting and
  social lines are in the pack. Zero model calls. [Guide](characters/ROLEPLAY_ENGINE.md).
- Phase 2, stage 2.1b done: character sheets (persona, speech, services, goals, needs,
  relationships, factions, secrets, opinions), a world codex (places with spots, ambience
  and norms; entrances; factions; events; quests), content settings and host `state`
  flags, as 12 more switchable engine features. 104 engine cases (the 62 stage 2.1
  outcomes are unchanged), byte-identical to the Python reference on 2,800 randomized
  cases. `kit::save` packs a game data file and a multi-state save file (named, forkable
  states with digests and a game link): 22 cases, 2,932 of 3,000 randomized workflows
  byte-identical, the other 68 being stricter Rust validation of nested lore/memory.
  Zero model calls. [Guide](characters/ROLEPLAY_ENGINE.md#character-sheet).
- Phase 2, stage 2.2 (first part) done: model requests and validated replies.
  - The `model` outcome compiles a full chat request with a persona/speech/content
    brief and the turn's situation. `reply` validates raw model text.
  - 130 engine cases. Byte-identical to the Python reference on 3,200 randomized cases.
  - Local evaluation ($0, 336 calls):
    - Llama 3.2 3B met every predeclared expectation.
    - Stheno 8B was 74.1% accepted against ≥ 85%, all rejections being too many
      sentences. Grounding +50 points on both.
  - [Guide](characters/ROLEPLAY_ENGINE.md#model-evaluation).
- Phase 2, next:
  - 2.2 (rest): expression, tales, viewpoints, reactions, deliberation and wording as
    engine stages; prop interactions proposed by characters, embodiment inquiries (`ask_posture`,
    `ask_held`, `ask_matter`), and native C ABI / CLI export done.
  - 8B follow-up (v3, $0, 476 calls) done. On held-out probes, Stheno 8B went from 74.1%
    accepted (shipped) to 88.9% with one host retry, and to 88.9% first try with a stricter
    brevity rule (98.1% with retry). 3B was ≥ 88.9% in every variant. Dropping
    catchphrases alone missed its bar (+9.2 against +10).
    [Guide](characters/ROLEPLAY_ENGINE.md#8b-follow-up-v3).
  - Done: the strict rule is the default pack rule ([format] for requests; 30 cases reblessed).
  - Done: `addressing`, an engine stage for turns not meant for this character (silent,
    waits or gestures), with host audiences (one id or a group), named speech, facing,
    nearest character, earshot and distance attenuation. 47 cases, 0 parity mismatches.
    [Guide](characters/ROLEPLAY_ENGINE.md#addressing).
  - Done: character `bio` (age, birthday, profession, anatomy, health, skills,
    knowledge, family) as a sheet section and engine feature; 31 cases.
  - Measured: with `GAMECTX_SHADOW=<file>`, Lantern Vault runs a fresh engine beside
    each chat turn and logs both outcomes (`eval/lantern-vault/engine_shadow_report.py`).
    Over 1,989 chat turns (dungeon tests and CI evals), the routes agreed on 1,924 (96.7%).
    Gaps: reactions (13), stock queries (7), authored persona (4), viewpoints (2),
    secret debt (2), greeting state (23), and `*enters*`/`*approaches*` routing (6).
    The line text is not comparable because the shadow engine starts fresh each turn.
  - Done: world timeline (`knowledge::timeline`, guide `docs/guides/WORLD.md`), 46 cases.
  - Done: places (`knowledge::places`): 112 built-in kinds plus custom ones, `within`
    nesting, links with distances and directions, and `Codex::nearby`; 30 cases.
  - Done: things (`knowledge::things`, guide `docs/guides/WORLD.md`): materials, kinds,
    instances, salvage, containers, carriers, best_tool, whats_here; 42 cases.
  - Done: recipes (`knowledge::recipes`, guide `docs/guides/WORLD.md`): crafting, cooking,
    building (changing the world), repairing and dismantling, tools, stations, skills; 25 cases.
  - Done: species (`knowledge::species`, guide `docs/guides/WORLD.md`): animals, plants,
    people, hybrids, temperaments, animal interactions (pet, feed, calm, lead, ride, train, harvest, behave); 27 cases.
  - Done: communities (`knowledge::communities`, guide `docs/guides/WORLD.md`): factions,
    home places, members, customs, trust relations, markets, bartering; 28 cases.
  - Done: fitness in `bio` (`character::fitness`, guide `docs/guides/WORLD.md`): strength,
    endurance, agility, condition, carry capacity, travel pacing, fatigue; 24 cases.
  - Done: combined engine change [format] for world model dialogue (timeline, places,
    things, recipes, communities, species) and shadow gap closure (reactions, viewpoints,
    stock wares, authored persona fallback, direction travel decisions); 226 engine cases,
    2,000 parity fuzzing workflows (seed 42, 0 mismatches). Existing engine conformance outputs
    reblessed once under `[format]`.
  - Done (Stage L): Lantern Vault migrated onto `RoleplayEngine` and world model (crafting, cooking,
    bartering, animal companion, pump repair, named saves with `kit::save`). 1694 / 1707 shadow routes
    agreeing (99.2%), 63 / 63 tests passing, zero model spend.
  - Done (Stage U): Clean web UI redesigned without CDNs or third-party dependencies: responsive
    desktop/mobile layout, high-contrast light/dark themes with visible focus rings, carried inventory bag,
    characters present list, timeline journal chronicle (`/api/journal`, `/api/timeline`), 87 / 87 HTTP
    web checks green, verified via headless screenshots.
  - 2.3 done: `VisualDirector` with tiered art bibles, format profiles, host-supplied visual
    fact selectors and an approval/repair/fallback workflow. 35 cases, 3,000 randomized
    workflows byte-identical to the Python reference, 4 typed tests, $0.
    [Guide](guides/VISUAL.md#direct-pictures-tiers-profiles-selectors-and-approvals).
  - Done (Stage 1 / Gap 5): Action catalog (`knowledge::action_catalog`, local kind `actions`).
    Read-only description over all 11 operational surfaces (things, recipes, species, communities,
    inventory, fitness, props, services, reactions, engine, initiative) with 49 built-in actions.
    Schema: actor requirements, typed target slots, preconditions, cost, effects, perception, risk,
    duration, interruptibility. 25 cases (9 malformed), 2,000 differential parity workflows (seed 7,
    0 mismatches), zero model spend, zero behavior changes.
  - Implemented (Stage 2 / Gap 1): Witness bridge (`character::life::witness::WitnessBridge`, local kind `witness`).
    Transforms timeline events, craft outcomes, and barter interactions into `direct` Life experiences for
    witnesses with sensory access (sight, hearing noise thresholds, sleep/unconsciousness checks) and `report`
    experiences when told. 25 conformance cases (10 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Implemented (Stage 3 / Gaps 2 & 12): Body and needs that change over time (`character::body::BodyManager`, local kind `body`).
    Need definitions with tick rates, thresholds, and action/item satisfiers; species/sheet needs; body state record
    tracking fatigue (builds up with actions, recovers with rest/sleep), health, injuries, and conditions (`asleep`, `sick`,
    `unconscious`, `dead`). Conditions affect the action catalog (blocked verbs, extra fatigue cost), routines (forced rest /
    incapacitated), and speech (silent while asleep, unconscious, or dead). 25 conformance cases (5 malformed), 2,000 differential
    parity workflows (seed 7, 0 mismatches).
  - Implemented (Stage 4 / Gap 3): Motive arbiter (`character::motive::MotiveArbiter`, local kind `motive`).
    Deterministic, explainable ranking across emergencies, needs, promises/deadlines, routine slot, tasks, sheet goals,
    and reaction brain proposals on a common 0..=100 bounded scale. Always names the winning motive and explains why it
    won over runners-up with explicit tie-breaking and kind precedence. 25 conformance cases (5 malformed), 2,000 differential
    parity workflows (seed 7, 0 mismatches).
  - Implemented (Stage 5 / Gap 4): Tasks and planning (`character::task::TaskManager`, local kind `task`).
    Task object (id, owner, assigned_by/for, motive ref, action steps, preconditions, status, progress, deadline, priority,
    reserved resources, on_fail/on_block recovery, outcome history). Deterministic planner over Action Catalog, strict host validation
    of model proposals, and step advancement solely from confirmed outcomes. Offline acceptance scene replay (§8) verified
    to byte-identical output. 25 conformance cases (5 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Implemented (Stage 6 / Gaps 6 & 8): Emotions decay & missing feelings (Gap 6) + Skills growth with use (Gap 8) (`character::affect::AffectManager` [local kind `affect`], `character::skills::SkillsManager` [local kind `skills`], and `character::life`).
    Additive attitude kinds `fear`, `respect`, `gratitude`, `envy`, `admiration` [format]; mood separation; decay toward temperament baselines.
    Skills growth with diminishing returns, duration/risk/quality scaling, and teaching. Offline acceptance scene replay (§8) verified
    to byte-identical output. 25 conformance cases (5 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Implemented (Stage 7 / Gaps 7, 9 & 13): Theory of mind (Gap 7: knows/wants/feels with provenance and witness-based inference; `character::mind::MindManager`, local kind `mind`) + Deception ledger (Gap 13: told-ledger per listener, consistency checking, contradiction appraisal; `character::deception::DeceptionLedger`, local kind `deception`) + Relationship reconciliation (Gap 9: precedence across Life attitudes, Sheet relations, and community standing with unified action appraisal and community trust aggregation; `character::relationship::RelationshipManager`, local kind `relationship`). Offline acceptance scene replay (§8) verified to byte-identical output. 25 conformance cases (3 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Implemented (Stage 8 / Gaps 10 & 11): Off-screen effects (Gap 10: coarse unobserved resolution with Action Catalog expected effects, timeline entries, and Life experiences; `character::offscreen::OffscreenResolver`, local kind `offscreen`) + Curiosity and exploration model (Gap 11: novelty signals across people/things/places/facts, temperament scaling, and 10-field initiative candidate generation; `character::curiosity::CuriosityManager`, local kind `curiosity`). Offline acceptance scene replay (§8) verified to byte-identical output. 25 conformance cases (6 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Implemented (Pass 2 Stage 1 / D4 & D1): Explain trace (D4: perception → belief → motive → task step → action → outcome chain with a `why` sentence; `character::explain::ExplainTracer`, local kind `explain`) + Stability monitors (D1: dithering, starvation spiral, deadlock, herding, grudge/rumor runaway warnings; `character::stability::StabilityMonitor`, local kind `stability`). Offline acceptance scene replay (§8) verified to byte-identical output. 25 conformance cases (6 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Implemented (D1): commitment inertia in the motive arbiter (optional `commitment`, additive; existing motive cases byte-identical).
  - Follow-up (D1): a 30-day soak test that reports monitor counts.
  - Implemented (Pass 2 Stage 2 / A1 & B1): Subjective world knowledge (A1: sourced per-being beliefs, correction by doing, `known_actions` and `gaps`; `character::beliefs::BeliefStore`, local kind `beliefs`) + Communication as action (B1: eight speech acts with preconditions and listener effects, `mind_record` proposals, act planning; `character::speech_act::SpeechActs`, local kind `speech_act`). Offline acceptance scene replay (§8) verified to byte-identical output. 25 conformance cases (4 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Follow-up (A1/B1): planner (`character::task`) reading beliefs directly instead of the host catalog; speech-act trust reconciled with `relationship`.
  - Implemented (Pass 2 Stage 3 / diets and foods, requested 2026-11-08): foods with fill, joy, spoilage, raw risk and cooking; diet profiles (species diet, taboos, allergies, tastes); meal evaluation, choice and eating with monotony and body/affect proposals; original `village` pack (`character::diet::DietManager`, local kind `diet`). Offline acceptance scene replay (§8) verified to byte-identical output. 25 conformance cases (4 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Follow-up (diet): food stock in `inventory`/`things` aging by itself, and market prices for food (C4).
  - Implemented (Pass 2 Stage 4 / creeds and B5 conscience, requested 2026-11-08): religions, ideologies, philosophies and traditions with tenets (actions, sacred-only actions, food tags), timed observances, sacred places and one-sided rival stances; adherents with devotion and standing; personal norms; `judge` (guilt, shame, indignation, standing loss as affect proposals), `due` (observance motives, missed-window charges), `food_taboos` (diet `forbids`) and `regard` (`character::creed::CreedManager`, local kind `creed`). Acceptance scene step verified byte-identical on replay. 25 conformance cases (4 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Follow-up (creed): conversion and doubt over time (B4), creed membership feeding community reputation, and the planner avoiding acts the being's own norms forbid.
  - Implemented (Pass 2 Stage 5 / A3 expectations and A2 attention): hopes and dreads with due times and grace, worry, disappointment with resentment for relied-on subjects, relief, late arrivals and per-subject reliability setting default confidence (`character::expectation::ExpectationBook`, local kind `expectation`); seeded graded noticing from salience, noise, light, distance, busyness, sleep, alertness, perception and cares with limited capacity, and `noticed_by` for the witness bridge (`character::attention::AttentionModel`, local kind `attention`). Acceptance scene step verified byte-identical on replay. 25 conformance cases (5 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Follow-up (A2/A3): expectations created automatically from promises (mind records) and routines; attention feeding the witness bridge directly instead of through the host.
  - Implemented (Pass 2 Stage 6 / A4 goals from experience, A5 habits, A6 lessons): pressure-built template goals with validated model proposals (`character::aspiration::Aspirations`, local kind `aspiration`); habits that form from repeated choices and routine entries that weaken with reasons (`character::habit::Habits`, `habit`); revisable lessons over recent outcomes with arbiter advice (`character::lesson::Lessons`, `lesson`). Host-set limits (`character::limits`; requested 2026-11-08: "the game should be able to set how much it needs for everything") with hard ceilings, used by all three. Acceptance scene step verified byte-identical on replay. 25 conformance cases (5 malformed), 2,000 differential parity workflows (seed 7, 0 mismatches).
  - Implemented (Pass 2 Stage 7a, requested 2026-11-08): host-set limits for all pass 2 modules (beliefs, speech acts, diets, creeds, explanation, stability, expectations, attention), defaults byte-identical to the old bounds; 25 conformance cases (6 malformed) in `conformance/limits/`, 2,000 parity workflows (seed 7, 0 mismatches). Scale measurement `eval/character/scale_measure.rs`: at 10,000 beings, 0.3–4.1 µs median per query and ~11 KB minimal state per being over eleven modules; found and fixed a whole-cast scan in speech-act `assess` (75 → 0.7 µs).
  - Implemented (Pass 2 Stage 7b): host limits for the pass 1 character modules (affect, skills, body, mind, deception, relationship, offscreen, curiosity, task); previously unbounded growth paths now stop at the defaults (announced in CHANGELOG). 25 conformance cases (6 malformed) in `conformance/limits_character/`, 2,000 parity workflows (seed 7, 0 mismatches). Scale measurement extended to twenty modules (0.3–4.2 µs median per query at 10,000 entries, ~24 KB minimal state per character); three more whole-table scans fixed (relationship, off-screen, deception).
  - Implemented (Pass 2 Stage 7c, T-035): host limits for the knowledge kinds (places/codex, timeline, things, species, recipes, communities, actions, lore, inventory, evidence, attributed notes `info`; facts capacity ceiling 256 → 4,096 with a heap-walked dependency check). Defaults byte-identical; request `actions` now bounded (announced in CHANGELOG). 25 conformance cases (6 malformed) in `conformance/limits_knowledge/`, 2,000 parity workflows (seed 7, 0 mismatches). Scale measurement `eval/knowledge/scale_measure.rs` at 1,000/10,000 entries; fixed `nearby` copying the codex per query (10.7 → 0.13 ms at 10,000 places).
  - Follow-up (7c): the things, species, recipes, communities and actions workflows return their whole table on every call (5–54 ms per call at 10,000 entries); an opt-in reply without the table, or delta replies, would make 10,000-entry worlds cheap per call. That is a format addition and needs its own cases. Timeline and lore queries scan every entry (about 1.2–1.3 ms at 10,000); per-actor and per-word indexes would help. Facts registration copies the registry (2.3 ms at 4,096).
  - Implemented (Pass 2 B2/B3, cooperation and conflict): joint tasks, help, hand-offs, favors, promises broken by the clock, claimed/queued/contested resources and fading grudges in signed millionths (`character::cooperation::Cooperation`, local kind `cooperation`); 25 conformance cases (6 malformed) in `conformance/cooperation/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#cooperation-and-conflict-pass-2-b2b3).
  - Follow-up (B2/B3): cooperation `assess` walks every task, promise, resource and debt (233 µs at 10,000 promises); a per-being index would keep it flat. Promises could also feed the expectation engine directly instead of through the host, and contests could read trust from the relationship engine.
  - Implemented (Pass 2 A7/A8, bounded rationality and fallible memory): temperament cognition settings, seeded impulse/plan decisions with depth, horizon and slips, memories that fade by importance and span and hazy recalls that shift a day or merge a person beside the truth (`character::cognition::Cognition`, local kind `cognition`); 25 conformance cases (6 malformed) in `conformance/cognition/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#bounded-rationality-and-fallible-memory-pass-2-a7a8).
  - Follow-up (A7/A8): the task planner and motive arbiter could read cognition settings directly; recalled (possibly distorted) memories could feed testimony in the evidence and witness modules.
  - Implemented (Pass 2 B6, off-screen conversations): beings know and want topics; seeded meetings tell, dispute or small-talk and are recorded as gossip, alliance, quarrel or small talk, with secrets kept until warmth 60 and belief/relationship/affect proposals (`character::chatter::Chatter`, local kind `chatter`); 25 conformance cases (6 malformed) in `conformance/chatter/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#off-screen-conversations-pass-2-b6).
  - Follow-up (B6): meetings could draw topics straight from the beliefs store and rumors, and choose who meets whom from places and routines; an optional model could phrase a record as dialogue when the player overhears it.
  - Implemented (Pass 2 C1/C2, life cycle and threat response): lifespans and stages, pairs, children, belongings, deaths with round-robin heirs and grief proposals; danger appraisal and fight/flee/hide/call-for-help/freeze by temperament with safety as a need (`character::survival::Survival`, local kind `survival`); 25 conformance cases (6 malformed) in `conformance/survival/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#life-cycle-and-threat-response-pass-2-c1c2).
  - Follow-up (C1/C2): species lifespans and temperaments could seed `born` and `threat` directly; belongings could move through `inventory` transfers; the chosen response could become a task for the planner.
  - Implemented (Pass 2 D2, level of detail for minds): full, coarse and crowd levels by distance with exact catch-up, per-level work accounting and crowd summaries (`character::detail::Detail`, local kind `detail`); 25 conformance cases (6 malformed) in `conformance/detail/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#level-of-detail-for-minds-pass-2-d2).
  - Follow-up (D2): body needs, routines and the scheduler could read the detail level directly so a crowd member skips their updates too; needs with nonlinear growth would need a stepwise catch-up.
  - Implemented (Pass 2 D3, save growth): retention collections with count, byte, age, importance and grace rules, reasoned pruning with dry runs, and numbered field migrations with bounded work (`character::retention::Retention`, local kind `retention`); 25 conformance cases (7 malformed) in `conformance/retention/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row, and a seeded 30-day soak (pruned saves keep 28.1% of the unbounded bytes). [Evidence](characters/LIFE.md#save-growth-retention-and-migration-pass-2-d3).
  - Follow-up (D3): modules could register their own entries (memories, lessons, expectations, ledgers) automatically, and `kit::save` could run declared migrations when it loads an older state; the soak measures declared sizes, not real save files.
  - Implemented (Pass 2 B7, status, roles and jobs): roles with status, seats and recurring duties that propose tasks with missed counts after gaps, unstaffed seats, status from roles and earned standing, contest edges and deferral, persuasion adjustment (`character::roles::Roles`, local kind `roles`); 25 conformance cases (7 malformed) in `conformance/roles/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#status-roles-and-jobs-pass-2-b7).
  - Follow-up (B7): cooperation contests and speech-act persuasion could read the status edge directly instead of through the host; communities could grant and strip roles by their customs.
  - Implemented (Pass 2 C4, economy feedback): goods and markets with stock, production, wants and purchases; per-mille price multipliers that move toward a scarcity target each settlement, scarce/glut states, spoilage, and aspiration proposals for shortages (`character::economy::Economy`, local kind `economy`); 25 conformance cases (7 malformed) in `conformance/economy/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#economy-feedback-pass-2-c4).
  - Follow-up (C4): community barter could read the multiplier directly instead of the host copying it into `price_mult`; diets and off-screen outcomes could feed `want` and `produce` without the host.
  - Implemented (Pass 2 C3, space inside a place): grid rooms with walls, one being per cell, blocked shortest paths, spots with capacity and first-come queues, Bresenham line of sight with range (`character::space::Space`, local kind `space`); 25 conformance cases (7 malformed) in `conformance/space/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#space-inside-a-place-pass-2-c3).
  - Follow-up (C3): rooms could be generated from `places` spots and links, and `see` could feed perception and attention directly; doors between rooms and diagonal movement are not modeled.
  - Implemented (Pass 2 B4, slow personality change): capped trait drift from fading experience pressure with reason chains, author locks and revert (`character::drift::Drift`, local kind `drift`); 25 conformance cases (7 malformed) in `conformance/drift/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing, scale row. [Evidence](characters/LIFE.md#slow-personality-change-pass-2-b4).
  - Follow-up (B4): lessons, grudges and grief could push traits directly, and cognition temperaments or relationship defaults could read drifted values.
  - Implemented (Pass 2 D5, default packs): original species, temperament, role and norm-set packs that `apply` turns into plain requests for the body, survival, cognition, drift, roles and creed engines, plus author-defined packs (`character::packs::Packs`, local kind `packs`); 25 conformance cases (7 malformed) in `conformance/packs/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing that runs every emitted request through its engine, scale row. [Evidence](characters/LIFE.md#default-packs-pass-2-d5).
  - Follow-up (D5): packs could also seed habits, aspirations, diet and skills, and a cast pack could apply many beings in one call.
  - Implemented (Pass 3 stage 1, T-036, turn frame and salience): trajectory sense with trend, streak and time-anchor labels, a validated turn frame fused from the other engines' slices, and a salience ranker that packs whole causal groups by salience per byte into a byte budget with reasons for every drop (`character::frame::Frame`, local kind `turn-frame`); 25 conformance cases (7 malformed) in `conformance/turn-frame/`, 2,000 parity workflows (seed 7, 0 mismatches), robustness fuzzing that checks every accepted packing, scale row. [Evidence](characters/LIFE.md#turn-frame-and-salience-pass-3-stage-1).
  - Follow-up (pass 3 stage 1): the frame takes slices the host already read; a later stage could read them from the engines directly, and the ranker's output could drive `packet::budget` sections in the speech compiler.
  - Next: pass 3 stage 2 (associative memory cueing and the epistemic gate, T-037).
  - Follow-up: save files could gain optional per-state compression and an in-place
    migration hook for game data updates; both need measurements first.
  - Follow-up: the command grammar still hard-codes a Lantern Vault alias (`gate`/`door`
    for `door:vault`); removing it is a format change for `studio-command`.

## Implemented locally for v0.3.1a

- Scene/voice follow-up: reuse current location lookup, distinguish NPC sight questions,
  twelve plot seeds per character with confirmed heard-history rotation/restore,
  structured story opening/ending, cross-tab audio ownership and retryable audio-start
  refusal. Six local 3B calls adopt all three tale packets; factual wording still has
  conservative rejection. Sixteen additive cases, ninety shared binding fixtures,
  cap0/$0. [Evidence](studio/SCENE_SOCIAL_REPAIR.md). Larger remaining priorities: implicit
  memory use, topic continuity and motivation-driven choices; [full audit](research/SOCIAL_CHARACTER_GAPS.md).

- Connected character wording + bounded expression/fiction/short-scope negotiation,
  scoped disclosed names/preferences, explicit OOC, conservative repeat pruning and
  context-matched AI wording cache. 74 additive Python/Rust cases; 600 transient-turn
  memory regression; 33-chat-turn offline/live host journey with two players/two NPCs.
  Live campaign adopted 4/5 packets but exposed duplicate/flat replies; fresh three-call
  holdout structurally passes with connected original fables. Small manual cohort,
  no universal quality claim, all cap0/$0. [Evidence](studio/STUDIO_EXPRESSION.md),
  [public research](research/ROLEPLAY_RESEARCH.md). Remaining: finite English routing, occasional
  generic/presumptive social speech, lexical guards are not semantic truth proofs.

- Optional loopback Lantern Vault studio: RPG/party controls, live cast inspection,
  approved art/repair, independent text/image/TTS/STT channels, actual model-tier maps,
  durable shared spending cap and receipt-driven interruption. Single bounded model worker;
  default std-only UI, optional system curl for HTTPS. 32 policy workflows, 85 HTTP checks,
  five fake-browser scenarios, eleven Rust studio tests; 1,090 older cases unchanged.
  Zero real provider calls / $0; physical voice/image quality and FPS unmeasured.
  [Setup, protocols and limits](studio/STUDIO.md).


- Optional visual compiler/registry: locked art contract, live expression/activity slots,
  perceived-only cues with authored occlusion, SHA-256 keys, approved/pinned reuse,
  safe approved fallbacks, inspect/repair and stale-worker fences. Python/Rust parity:
  64 workflows, 13 malformed; 1,026 previous cases unchanged. Zero model calls / $0 API;
  37 queued metadata jobs, 5 exact reuses, 2 fallbacks. Image quality and timing unmeasured.
  [Contract, editable example and limits](guides/VISUAL.md).


- Probe validation: root/native formatting, strict Clippy, tests/rustdoc, Rust 1.82,
  82 offline CI commands, 21 binding CI commands, all C# fixture groups, package
  verification and reference/credential sweep pass. Core total 842, prior 827 unchanged.
- Adversarial repairs: whole-command conversation guard, complete authorized-view time
  conflict detection, restored lifecycle timing and same-revision frame consistency.
  15/57→57/57 probes; 900 independent engine steps match. Fake-transport tests reject
  model overrides, invalid output reservations and preflights without turn allowance.
- Optional typed host time assessment: 64 additional states match its oracle; 96 conflict
  workflows and 162 command comparisons. Fifteen additive cases retain all earlier 827
  outputs; expanded session suite 61 fixtures. No new model-callable tool or default.
- Installed Qwen time-classification diagnostic: valid JSON 32/32, correct 25/32. Focused
  12/16, expanded 13/16; full-turn input 6,440 versus 19,460 tokens; median 1,847,744.5
  versus 6,303,493 microseconds. Smaller is not accepted as more accurate. Wrong sun/report
  types, quote-containing reports returned as exact, one wrong instrument classification.
- Verified count-reuse prototype then opt-in production adapter: nine turns/arm, 18→12
  calls and 3,630→2,420 input tokens; paired answers identical and only 3/9 correct each.
  Production repeated-turn medians 1,780,275→1,637,055 µs. Exact repeats only; no model
  accuracy gain. Earlier pilot timings excluded scoring/memory; retained as adapter-only.
- New model work: 159 local calls including token-first checks/preflights, API spend $0,
  no other model loaded/downloaded. Four CPU threads/GPU0/context2048; sampled runner RSS
  peak 2,558,652,416 bytes in classification. Global memory/cache state uncontrolled.
  [Evidence, raw artifacts and limits](evidence/ADVERSARIAL_EVAL.md).


- Follow-up validation: root/native formatting, strict Clippy, tests/rustdoc, Rust 1.82,
  76 offline CI commands, 19 binding CI commands, original/expanded C# fixtures and
  package verification pass. Total core cases 827; reference/credential sweep clean.
  Model/API spend $0; no model loaded, downloaded or called.
- Optional follow-up improvements: exact task observation filters with required source
  tokens, timer hints for horizon/windows/protection/expiry, and planning-event dependency
  checks. 54 independent workflows, nine additive cases (818 earlier unchanged), 10,000
  selector mutations and a 52-fixture expanded persistent suite. No new model tools.
- Whole-runtime-copy baseline versus history preflight/timeline clone: seven alternating
  1,000-update release pairs with 128 retained observations, medians 328,934,760→6,986,127
  ns; identical final state. Initial state 228,575 JSON bytes; paired process peak RSS
  4,488 KiB, no model. Synthetic CPU work, not engine/model latency or per-NPC RAM.
- Newest-only retrieval rejected for required evidence: the synthetic newest-three view
  omitted token 1 in 2,102 observation JSON bytes; pinned filtered view retained it in
  371 bytes. No semantic/model accuracy or full-request token savings claim.


- Routine-layer validation: root/native formatting, strict Clippy, tests/rustdoc,
  Rust 1.82, package verification, 75 offline CI commands and 17 binding CI commands
  pass. C# checks cover 43 new and 263 prior fixtures. All 797 prior core outputs
  unchanged; total 818. Credential/local-reference sweep clean. Model/API spend $0.
- Optional routine definitions/timelines, actual scoped observation history and source
  time ranges: 69 independent Python/Rust workflows (63 accepted, six rejected), 21
  additive cases, 20,000 text mutations, 43 persistent binding fixtures. No existing
  conformance/evaluation output rewritten. [Integration and limits](guides/ROUTINES.md).
- Host calendars/conditions and stable priority/window/ID selection; interruption,
  resumption, bounded deferral and explicit completion. Existing scheduler budgets
  128 test NPCs at 16 updates/tick; no LLM planning calls. Source history retains real
  visibility, broken instruments, stale/conflicting reports and growing uncertainty.
- Initial recurrence prototype discarded during review: seeding only the latest and
  previous occurrence could miss older still-deferred windows. Enumeration now starts
  at the first unexpired occurrence and rejects excess work at the timeline cap.
  A zero-observation cap also needed an explicit pre-selection check. No accuracy claim.


- Validation: root/native formatting, strict Clippy, tests, rustdoc, Rust 1.82,
  package verification, 74 offline CI commands and 15 binding CI commands pass.
  All 747 prior cases unchanged; total 797 core cases. C# fixtures: 30 new plus
  126 information, 59 kit and 48 local replies. Reference sweep clean; API spend $0.

- Actor-visible location/world snapshots, `look_around` and scoped character facade:
  58 + 63 + 49 independent Python/Rust workflows; 50 additive core fixtures, 20,000
  text mutations. Persistent Python CLI/native, Node and C# share 30 actor fixtures.
- Explicit task attention caps and full-request rendering/count handshake: synthetic
  focused 1,025 bytes (one tool/two memories/one exchange), expanded 3,733 bytes
  (three/12/four); spare context alone does not raise attention limits. Fake byte
  counter, no model-token or accuracy claim. Arbitrary provider/model IDs supported.
- Dialogue regression routing: exact intended sets 0/78→78/78 across six seeds and 13
  reused reported phrases; actual key receipt, local letter counts and attributed name
  phrasing tested. Time access requires an authored clock/watch entitlement, approximate
  schedule or attributed report; no-source NPCs stay uncertain. Fourteen time workflows.
- Rejected as safe speech replacements: broad guarded prompts invented weather/history;
  compact v2 (643→225 reported input tokens) truncated both replies mid-sentence;
  one-sentence v3 (241 tokens) still implied observed clear blue weather. Three variants,
  18 local calls including token-only/preflight, $0 API. Records retained and audited;
  source-selected sky response remains default. No semantic guarantee from a prefix.
  [Measurements and limits](evidence/CONVERSATION_PATCH_EVAL.md).

## Shipped (0.1.0-alpha.2)

| Item | Result |
|---|---|
| Tables, `group_by` | −38% input tokens vs compact JSON, 140/159 vs 133/159 correct (3 models) |
| Nested `group_by` | two-column filters 7/12 → 12/12, −2 to −3% tokens |
| `signatures` | −12 to −14% vs packed JSON Schema; −46% vs JSON overall, 141/159 (3 models) |
| `relevant_to` (tables) | −26% tokens, identical accuracy (100/106 both) |
| `relevant_fallback: "none"` | another −59% tokens; loses only whole-table questions |
| `where`, `derive`, formulas | direct arithmetic 0/16, model-written expression via `calc` 8/16; misses were row selection |
| `memory` recall | −84% tokens vs a 60-event history, 20/20 both |
| `round` view | about one token per dropped digit per number (−5% for one place here) |

## Shipped (0.1.0-alpha.3)

| Item | Result |
|---|---|
| Model-written `query` tool (bounded SELECT, filters, sorting, aggregates) | 29 questions × 2 small models: direct 33/58 → SQL 53/58; numeric filters 0/6 → 6/6; arithmetic 1/16 → 12/16; packed tool 52/58 in a separate run. Full tables still sent. 116 recorded queries and 3,000 generated/mutated queries byte-identical to Python; 36 new conformance cases, 30,000 fuzz mutations. |
| Query token-only check and eval cap enforcement | Direct vs tool: 1,471 → 1,611 input tokens (Llama), 1,490 → 1,626 (Mistral); all four omitted cached-token accounting. Cost $0.000756. |
| Schema-only descriptions, explicit weight fields and bounded correction sessions | On 3 held-out states (24/48/96 inventory rows, 10/20/40 NPCs), full tables 160/174 first → 171/174 after retry; schemas 169/174 → 172/174. Final answer scores identical to executed-query scores. Complete-exchange average input 4,898.632 → 1,615.874 (−67.0%); completed-exchange cost $0.107952 → $0.038804 (−64.1%). 23 new cases; 547 measured attempt envelopes byte-identical to Python. |
| Generic adaptive local reaction brains | Offline reference-policy agreement on four tasks: random 545/1,536 → trained 1,421/1,536 (fresh seeds 707/808/909); changed preference 264/384 → 384/384 (seeds 1001/1002/1003). Graphs, training, pins, validation gates, persistence/undo; 41 new cases, 30,000 text mutations, 292 byte-identical workflows including 2,304 predictions. No API calls. |
| Opt-in validation-reaction rate selection | Fresh paired seeds 2001/2002/2003: single rate 1,436/1,536 → selected 1,487/1,536; nonlinear task 315/384 → 359/384. Rates chosen only on validation data; about 3× estimated training work, identical inference. 17 new cases, 45 byte-identical selection workflows/1,536 predictions; 10,000 additional text mutations. No API calls. |
| Preloaded templates and opt-in bounded prediction caching | Local release medians, five trials: 20,000 repeated-state predictions 15.939–19.223 ms cached vs 29.161–36.524 ms uncached (1.7–2.0×); cloning 1,000 templates 1.412/8.952 ms vs JSON restore 24.238/142.615 ms (small/large). Independent owners, 0–256 FIFO entries, mutation invalidation; six new cases, 64 byte-identical cached workflows and 10,000 workflow mutations. No API calls. |
| Native frame inference and faster packet assembly | 20,000 changing-state calls: JSON 26,030,769/32,503,158 ns → native 1,968,370/6,171,925 ns (small/large), zero native allocations. 500 pressured packet builds 146,774,066 → 19,042,995 ns; prepared reuse 1,834,731 ns. Exact output/receipt parity. |
| Scheduling, sliced training and host-policy validation | Synthetic 64 actors/240 frames: 15,360 → 512 evaluations, max eight/frame. Sample-sliced train/selection preserves weights/reports/state; cancellation and stale-state rejection. Host masks/thresholds evaluated on fresh seeds 3001/3002/3003: 1,466/1,536 for both old and host-metric selection, no measured gain. |
| Acknowledged context synchronization and local prefix harness | 120 turns: 489,490 full-state bytes → 19,089 transfer bytes, exact reconstruction; explicit ACK, stream/revision checks and snapshot recovery. 35 new cases across runtime APIs, 148 byte-identical workflows/2,304 predictions, 40,000 extra text mutations. All offline, $0; actual local-model KV-cache savings unmeasured. |
| Fitting-packet serialization repair | Paired seven-trial medians: 1,000 fitting builds 9,171,479 → 5,905,360 ns, mixed 8,822,089 → 5,708,635 ns. No extra fitting allocations; peak transient bytes 8,108→8,140. Pressure 34,371,446 → 29,824,656 ns, allocations 153→172/build. |
| Versioned bindings, diagnostic replay and optional reaction stability | Full expected schema checks names/order/units/ranges/policies. Replays capture normalized observations/labels/latencies/state changes. Fresh seeds 5001/5002/5003: 355→15 switches, zero missed urgent switches/ineligible choices, total utility regret 2,281,674 millionths/720 frames. |
| Host query meanings | Three fresh 24-row states, 30 questions/model: schema 36/60 → meanings 45/60 (3B 12→18; 24B 24→27). Token-only input 205/190 → 365/352. Token check $0.0001380125 + accuracy $0.0051951 = $0.0053331125. Three descriptions and 120 recorded queries byte-identical. |
| Scoped journal, native evidence and cited hypotheses | Provenance/expiry/supersession; sealed supplied-evidence views, stale/foreign/dependency/privacy checks and subject/clock-bound restore. Six valid/nine invalid evidence workflows; repeated hypotheses keep observations 2, confidence 0.6 and expiry 30. Total 46 new integration cases, 152 differential workflows, 60,000 added text mutations. |
| Explicit typed fact registry | Host/user assertions, user precedence/pins, exact decimal claim checks, revision dependencies with transitive invalidation, inherited privacy/physical erasure and budgeted context tables. Offline 128/128 assertion checks; 1,024-byte cap retains 8 in 927 bytes, 120 dropped. 19 new cases, 119 byte-identical workflows, 20,000 text mutations. $0 API; no model-accuracy claim. |
| Indexed recall and reusable validated procedures | Selective fixture: 100 queries/1,000 records, 100,000 scan record visits→100 index postings, excluding index construction. Two normalized/alias matches 2/2; repeated procedure matches 100/100. Immutable filtered index; exact fact revisions, current action validator, expiry/privacy gates. No live call/token reduction claim. |
| Outcome labels, temporal features and async freshness | 100 observed labels, zero trainable unexposed trials and zero replay labels. Bounded execution/exposure windows; one-use labels; event counts/rates/sequences with original times/watermarks; sealed one-use actor/world/schema/permissions/fact-bound result tickets. |
| Shared perception and independent brain modules | 12 candidates × 100 frames: 2,400→1,300 module evaluations; 3,600 identical predictions on seeds 9101/9102/9103 including unknowns. Seven paired release trials/12,000 candidates: 729,906→631,514 ns (0 hidden) and 3,341,042→2,014,761 ns (16 hidden). Scope/frame/module replacement invalidation. |
| Runtime spending reservations | Integer microdollars, concurrent reservations/retries, dispatch/cancel/settle phases, ambiguous crash charges, overrun blocking and free fallback. Entire upgrade batch: 40 new cases, 283 byte-identical workflows, 54,000 added text mutations, $0 API. |

## Documentation and measurements (0.1.0-alpha.4)

| Item | Result |
|---|---|
| Fresh alpha.3 hosted-model check | Six states across cargo/teams/routes, 48 questions/model, two small models. Full JSON 27/96, packed 27/96, schema + query 91/96. Completed-exchange average input 1,786.792→1,252.125→916.490; first query 83/96, after correction/final 91/96. 105 Rust/Python feedback envelopes match. Total usage-priced spend $0.05529491875 including token check and interrupted-run cost. [Full report](evidence/MODEL_TEST_2026_11_06.md). |

## Shipped (v0.2.0a): query optimization

| Item | Result |
|---|---|
| Borrowed query execution and streaming aggregates | Seven paired release trials/five synthetic fixtures: 4.6–32.5× faster parsed execution. Wide 1,024-row total: 1,000 calls 1,277,473,628→53,773,386 ns; allocations/query 9,229→11, peak transient bytes 1,355,791→8,211. 2,024 alpha.4 result/error comparisons and 3,000 generated Python comparisons agree. |
| Query plans, explicit host shapes and row/result bounds | 16 new conformance cases; 516 byte-identical Python comparisons; 20,000 extra mutations. Shapes check structure only, not intent/values/permissions. Existing query/tool/session output unchanged. |
| Namespace and dotted-ID relevance | Ten lexical regressions 2/10→10/10; four additional catalog parity cases and one exact-ID/numeric-spelling query case. All 366 prior conformance outputs unchanged; 35 new cases in this batch. No API spend or semantic/model-accuracy claim. |
| Optional explicit-selection system guidance | Fresh paired six-state cohort: 3B 46/48→48/48, 24B 47/48→46/48; overall 93/96→94/96, input 907.865→1,107.802 (+22.0%). Exact text available per host configuration; default standard. Four new profile cases; 209 fresh feedback envelopes agree. Token/accuracy total $0.0293735125, caps $0.01/$0.08. [Report](evidence/QUERY_OPTIMIZATION_2026_11_06.md). |

## Shipped (v0.2.0a): integration kit

| Item | Result |
|---|---|
| Shared registration and Rust builders | Data types/meanings/units/IDs/visibility/age reused by packing, recipe-scoped queries, tool validation and brain bindings. 144 Rust/Python workflows agree (113 accepted, 31 rejected); 101 generated stock states plus three scripted scoring states. |
| Tool authoring and callback dispatch | Closed bounded schemas, nested/nullable/optional arguments, exact decimal limits, recipe availability before callbacks; host rechecks world permissions. 47 new conformance cases, all 401 previous cases unchanged; 20,000 added text mutations. |
| Inspector, doctor and starter | Explains selected source indices/omitted fields/age/budget; validates references and warns about missing units/unbounded age. Starter source 1,646 bytes → selected packed table 97 bytes, complete packet 646; 550-byte cap drops rows to 540 bytes. Lossy host selection, no model-accuracy claim. |
| Named reusable context recipes | Prepared stable tools/schema and host-selected rows/schema-only/extra memory or fact sections. Seven paired release trials/1,000 turns: registration each turn 146,130,805 ns → shared registration 78,931,400 ns. Synthetic registration reuse, not model latency. |
| Persistent and native host bridges | Versioned bounded JSON-lines, separate C ABI, thin Python/Node/TypeScript/C# adapters. 265 Python/native checks, 8 Node checks, 59 C# fixture replies plus two smoke checks on Linux; engine/mobile/Web deployment untested. |
| Developer-owned evaluation kit | Complete recorded cohorts, independent expected results, separate answer/query scores, invalid calls, bytes/tokens/latency with missing data kept null. Three scripted states: correct run 3/3, injected-error run 2/3, one invalid query/call detected. No hosted-model run; entire batch $0 API. [Guide and evidence](guides/KIT.md). |

## Shipped (v0.2.0a): local deployment support

| Item | Result |
|---|---|
| Full-request contracts and protected scenes | Exact/estimated host-rendered counts, output/safety reserve, required facts/current input/outcome, whole-exchange dialogue and optional-first dropping. 148 Python/Rust workflows (132 accepted/16 rejected); 48 new cases, all 448 prior cases unchanged; 20,000 extra mutations. |
| Legal-pair decisions and explicit deployment profiles | One correction, live availability/revision checks, current fallback or abstention, no action execution. Identity pins digest/quantization/template/tokenizer/runner. Guidance/transport stay opt-in. |
| Model-size workload guide | Editable 1B, 1.5B, 3B, 8B, 30B and 70B templates and task/content/choice hints; all generic tiers marked unmeasured. Size does not infer model capability, guidance or hardware fit. |
| Actual local Qwen 3B evaluation | Four physical cores / 7,996,997,632 bytes RAM, Q4_K_M, 2K starting context. 30 scenes × three modes: raw/scene/schema final pipeline 26/30, 30/30, 30/30; accepted model correctness 23/30, 27/30, 30/30. Median turn µs 19,226,226 / 8,651,346 / 5,451,699; p95 35,123,472 / 19,212,467 / 14,292,873. Preflights/retries/local hydration included; natural cache reuse and memory pressure limit extrapolation. |
| Local guidance/tool/narration pilot and telemetry | Six scenes: concise/example/native-tools decision 6/6, 6/6, 5/6; no proved guidance gain over baseline. Backend-aware sampled runner RSS up to 2,705,707,008 bytes; main-cohort missing RSS remains null. Independent raw-data/decision/budget/report audits, all local API spend $0. [Report](evidence/LOCAL_MODEL_2026_11_06.md). |

## Added in v0.3.0a: local optimizations

| Item | Result |
|---|---|
| Host intent preservation | Optional narrowed choices, current request/facts in retry feedback and intent-appropriate fallback/abstention. Scripted legal-open trace accepted by legacy session; guard requests correction then accepts inspect. No model-understanding claim. |
| Typed prepared query tools | Fixed host SQL and typed predicate values bound as AST data; recipe views/visibility still enforced, including empty tables. Example response 56→15 bytes, schema 206 bytes; not a full-request token claim. |
| Dependency-complete evidence | Bounded DAG closures, required-fit failures, deterministic optional selection. Fixture 419→49 packet bytes retains item/owner/price. Densely shared 128-record graph completes without exponential traversal. |
| External full-render counting | Host-assisted candidate/sequence/identity handshake, recount after every optional drop, no drop of required sections; exactness stays a host assertion. JSON order and number spellings preserved in acknowledgements. |
| Local-match router | Composes host rules/procedures/brains with legality, revision and live availability; ambiguity/conflict falls through. Eight scripted cases: three local drafts/five LLM paths, not measured real-game call reduction. |
| Readable decision codec | Six local Qwen token probes: 2/4/8 choices 158→118, 224→174, 344→274. Matched-order 12 scenes: pairs 8/12, named 11/12; input 3,865→3,285, output 178→111. Small synthetic tasks, opt-in. |
| Split configuration comparison | Complete independent cases/identity-checked profiles; tuning-only quality/cost Pareto frontier and exported profile/recipe, separate holdout report. Injected held-out failure stays visible at 1/2 and does not change tuning winner. |
| Verification | 179 byte-identical Python/Rust workflows (152 accepted/27 rejected), 69 added conformance cases, 30,000 new mutations. All 496 prior cases unchanged; zero third-party dependencies, Rust 1.82. All new API spend $0. [Guide/evidence](guides/OPTIMIZATION.md). |

## Playable integration testbed (v0.3.0a)

| Item | Result |
|---|---|
| Original dungeon example | Six-room human/JSON-lines host, seeded key/purse/prices/health, local shop/combat, revision-checked actions and model-visible snapshots. 103 Python/Rust rule workflows agree; 23 separate example fixtures, all 565 core cases unchanged; 10,000 mutations. |
| Replays and packet checkpoints | Six scripted seeds win; 240 mode observations. Raw/scene/named request JSON totals 275,304/261,304/206,347 bytes across 80 matching checkpoints. Raw archive drops at 55 checkpoints; byte/history differences are not isolated tokenizer savings. |
| Optional local agent | Token-first loopback-only bounded player with complete-turn traces and independent rule/native audits. First Qwen named-mode smoke repeats legal look actions without advancing the quest; validity is not success. API spend $0 per local run. [Guide](../examples/lantern-vault/README.md). |

## Human-play characters and task workflows (v0.3.0a)

| Item | Result |
|---|---|
| Size-and-task profiles | 24 editable templates for 1B/1.5B/3B/8B/30B/70B × interpret/dialogue/narration/query, all marked unmeasured. Thirty new core cases, all 565 prior outputs unchanged; old generic profiles and renderer unchanged. |
| Direct current utterance | Native chat renderer keeps current text outside nested JSON. Focused 3B cohort generation input/output 6767/308 versus standard 7468/525, both 2/4 intent; bundled task/prompt changes prevent isolating the renderer effect. |
| Human player, AI characters | Free-form keeper/merchant/narrator speech with scoped original player quotes, bounded dialogue, deletion and one-use revision checks; speech executes no action. 43 independent workflows, 507 accepted/11 rejected operations, 13 fixture sets. |
| Entity reference guard | Longer overlapping phrases, canonical action type, fresh scoped action focus and targeted revision-bound human clarification. 360 independent workflows across six seeds; 30,000 social/entity mutations; seven fake-generation front-end tests. Example English heuristics, not a universal entity parser. |
| Local pilot | Three nine-speech/four-intent cohorts, no retries, 26 calls each. Slot pilot 7/8; phrase-aware revision 8/8, four local deferrals plus 4/4 model choices. Revised cases are regression evidence, not fresh holdout. All token/accuracy runs $0 API; no other sizes tested. [Guide and raw artifact checks](guides/WORKFLOWS.md). |

## Shared dialogue-to-action support (v0.3.0a)

| Item | Result |
|---|---|
| Reusable conversations | Native `Conversation`/configurable limits: scoped whole exchanges and verbatim quotes, one-use source-validated replies, deletion, live scope/revision gates. Dungeon storage/validation delegates to core; all old social/model artifacts still audit identically. |
| Reusable references and confirmation | Host-supplied entity types/aliases/pronouns with bounded overlap matching; generic pending action verifies scope/revision/terms/current permissions and never dispatches. 78 independent Python/Rust workflows, 38 new core cases, all 595 prior cases unchanged, 30,000 added mutations. |
| Merchant loop repair | Local exact purchase receipts and yes/no offers; insufficient funds explain the shortfall without generation. 66 workflows / 144 trade requests over six seeds and 11 front-end tests; successful real launcher trade smoke, zero generation. All new API spend $0. [Shared guide](guides/CONVERSATION.md). |

## User-reported dialogue regressions (v0.3.0a)

| Item | Result |
|---|---|
| Preserved observations | 11 sequences / 26 turns; deployment identity, seed, usage and original world trace unknown. Correct original-name recall coexists with invented player disclosures/home, weather and tonic purpose. Manual claim annotations, no aggregate model score. |
| Host/storage reproduction | Seed-7 native/Python audit: 17 trade requests, 16 matching local receipts, nine supplied speech packets, two reproduced purchases. Player quotes retain original input only; forgetting preserves inventory. All 633 core cases unchanged; no prompt/runtime changes, zero new model calls, $0 API. [Report](evidence/DUNGEON_FEEDBACK.md). |

## Authored lore and factual answers (v0.3.0a)

| Item | Result |
|---|---|
| Prepared lore loader | Versioned 262144-byte/512-entry file, stable IDs, tags, actor audiences and host knowledge grants; bounded relevant selection with required-fit errors. Example compact world 1669→145 selected packet bytes; not lossless or a token claim. |
| Source-bound answers | Model selects a source or unknown; host renders exact lore/state text or attributed player quotes, rechecks scope/revision/source and never executes actions. Live fields supersede matching lore fields. 108 independent workflows, 28 new cases, all 633 prior cases unchanged; 30000 added mutations. |
| Dungeon integration | Editable world file, new source/legacy dialogue switch, scoped factual tickets and authored background in open-ended dialogue. Sixty native request workflows across five seeds and eleven front-end checks. Legacy artifacts still audit identically. |
| New local question tests | Native v3 11/12; 20 fresh questions 17/20, after routing fixes 20/20 regression; eight more questions 7/8. Final “Did I…” disclosure guard handles the miss locally, 1/1. All pilots/failures retained; no new independent generalization claim after repair. |
| Runtime evidence | Rerun median/p95 turn 563910/2265634 µs includes ten local unknown paths; unequal call mix prevents format-only speed claims. Separate post-run runner/host RSS 2180976640/4694016 bytes, not peak RAM. Every new token/accuracy/audit run $0 API. [Contracts](guides/LORE.md), [full report](evidence/LORE_EVAL.md). |

## Measured and rejected

| Idea | Why not |
|---|---|
| Unrestricted source cards as sufficient question understanding | Initial local selector 9/12 strict targets; unrelated price selected for salve permission and unnecessary abstentions. Topic narrowing helps but does not establish entailment. |
| Treating auxiliary “does” as an item-use field | Misroutes “Who does the oil belong to?” and can hide the correct owner card. Prioritize explicit subject/topic cues; retain the failed v2 artifact. |
| Treating remembered name as evidence for an unrelated disclosure | First fresh batch offers a name quote for alleged sailing history and scores 17/20. Restrict candidate player quotes to relevant content and explicit temporal availability. |
| Presenting a tuned rerun as fresh model generalization | The 20/20 rerun follows three observed failures. Keep it as regression evidence; the next eight questions score 7/8 and retain their missed disclosure question. |
| Structured facts alone as a grounding guarantee | Both 20-reply natural diagnostics accept every JSON shape but invent homes, effects, ownership or permissions. Narrower facts reduce bytes, not proven factual errors. Keep natural mode experimental. |
| More instructions as the default cure for character understanding | Standard intent 2/4, extra grounded instructions 1/4; generation input 7468→8240 and output 525→610 across nine speeches/four intents. Retain opt-in control. Focused prompts still hallucinate tonic effects and seal permission; validation cannot prove prose entailment. |
| Unmatched choice order as a format-only accuracy comparison | Initial 12-scene codec pilot scored 11/12 for both, but presented different candidate orders. Retain frozen runner/results, exclude that comparison, rerun with matched order (8/12 versus 11/12). No prompt tuning; needs fresh/live replication. |
| Model-loop timing presented as complete-turn latency | An initial local pilot began timing after hydration. Retain nine completed pilot turns, exclude its timing comparisons, and rerun with native hydration/filtering inside the clock. |
| Universal default native tools or extra guidance on the local 3B | Six-scene narration pilot: native tools 5/6 with a truncation/retry/fallback, concise/example 6/6 with extra tokens; baseline schema already 6/6. Keep explicit; no size-based winner. |
| Universal default explicit-selection guidance | Only one extra final success (93/96→94/96), first queries 86/96→84/96, 24B regresses 47/48→46/48, input +22.0%. Keep opt-in per model/workload; no size-based automatic choice. |
| Automatic exact oracle-shape validation for natural-language queries | Rejects 27/96 recorded queries, including 22 correct equivalents; keep strict shape checks only for independent host-declared structure. |
| Flattening top-N IN subqueries automatically | Tie counterexample `[10,9,9,9]`: top-three sum 28 versus IN-membership sum 37. Reject rewriting that silently changes meaning. |
| Symbol/lookup indexes | no token saving; accuracy 102 → 74 of 108 |
| Plain-text (CSV-like) tables | 0 to +1% tokens: `","` is already one token |
| Row counts in table headers | +2/54 (noise) for +3% tokens |
| Sorting rows to help filtering | no accuracy change on its own |
| All-text packet instead of JSON | only ~1% better than signatures inside JSON |
| JSON query interface alongside SQL | 52/58 vs SQL 53/58 (noise), but more input tokens on both models: 1,601/1,616 vs 1,569/1,586; invented `group_by` filters. Keep SQL as the one shipped query interface. |
| Linear-only default reaction network | On the nonlinear switch holdout, 192/384 versus 306/384 with four hidden nodes; retain linear networks as an explicit option. |
| Straight-through gradients at every brain node | Diagnostic seeds 101/202/303: 1,243/1,536 versus 1,430/1,536 with output-only straight-through gradients. Strict clamp gradients scored 1,424/1,536 but failed the saturated-output adaptation test; use output-only recovery. |
| More examples alone as a universal brain-accuracy fix | At equal 4,096 training sample-visits, switch validation MSE improved 0.012898 → 0.009566 but correct reactions fell 110/128 → 103/128 (32 → 128 training examples). More data can help other tasks; decision quality needs its own check. |
| One universal initializer/rate | Switch validation scores ranged 74/128 (initializer 1) to 114/128 (7), versus 110/128 (3). Rate 0.3 helped switch (119/128) but hurt support (116/128 vs 119/128 at 0.1); use bounded validation selection without resetting an existing brain's learned state. |
| Always-on brain prediction caching | Cycling 256 distinct states through 32 slots: 0 hits/20,000 calls; small network 29.011 ms uncached → 53.640 ms cached, large 37.100 → 67.276 ms (1.8–1.85× slower). Keep caching opt-in with counters; no approximate input bucketing added. |
| Universal one-shot packet speedup | The fitting fixture took 5,587,160 → 6,098,296 ns (about 9% slower) despite allocations falling 47,000 → 30,500. The initial always-prepared variant added allocations too; the shipped fit path moves values directly. Prefer prepared reuse for repeated builds, and measure one-shot tradeoffs. |
| Host-policy selection as an automatic accuracy boost | Same rates and 1,466/1,536 held-out masked/thresholded decisions as prior selection. Ship as opt-in metric correctness, not a measured accuracy win. |
| Intermediate fitting-packet repair using another ordered vector | Improved fitting throughput but added an allocation and slowed the tiny fixture. Shipped two stable-first borrowing passes, preserving original pressure-path declaration order without another vector. |
| Stability as free decision-quality improvement | Switches fell 355→15, but holding sacrificed 2,281,674 utility millionths/720 frames. Keep optional, measure response quality/regret and honor urgency/eligibility immediately. |

## Added in v0.3.0a: information and conversation tools

- Release-readiness transport coverage: 126 new information/inventory/conversation
  workflows × four transports = 504 checks (Python CLI/native, Node, C# native), $0 API.
  Added to CI; all 747 core conformance cases unchanged. Full offline/model-artifact
  audit, Rust 1.82, isolated package tests and publish dry run pass. Live-model
  tool-selection accuracy remains unmeasured; natural speech remains experimental.

- Optional conversation tools: 87 independent Python/Rust workflows, 47 new conformance
  cases, 30000 mutations. Explicit participant scopes, visible events, reported promises/
  relationships, host progress, bound original-utterance references and recipient consent.
  Two recent records from 80 authorized events: 8589→231 bytes; eight schemas 3127 bytes.
  Context selection, not lossless compression/model accuracy; all checks $0 API.

- Optional informational notes and inventory lookup: 39 Python/Rust byte comparisons and
  additive conformance cases, 30000 mutations. Reported claims retain original sources;
  multiple owners, tags, unique IDs and atomic host transfers remain engine-controlled.
  Four example schemas 1457 bytes; lookup schema/result 309/108 bytes, reported note 163.
- Stock routing: eight paraphrases 1/8→8/8 offline; 96 native regression workflows over
  six seeds. Installed Qwen stock pilot 10/12 (two unnecessary abstentions), $0 API.
- Structured lore v2 and opt-in natural speech: 72 native request comparisons, old lore
  bytes unchanged. Two 20-reply diagnostics have 20/20 valid shapes each but hallucinate;
  narrowed structured request bytes 20829→9962/ten questions, no accuracy-win claim.

## Next candidates

- Evaluate conversation tool selection/claim extraction on fresh local-model dialogues
  and multiplayer transcripts. Current new tools have deterministic format/privacy checks,
  not live selection accuracy or complete-turn latency/RAM results.
- Model-callable notes/ownership tools need live tool-use and integration evaluation;
  current checks establish format/safety and byte cost, not model understanding.
- Natural phrasing remains experimental: both broad and narrowed prompts hallucinate
  absent homes, effects or permissions. Do not promote it based on valid JSON or fewer bytes.

- Extend semantic question routing on independent scene/language families and multi-part
  questions; the example English policy can still abstain unnecessarily or choose a
  related valid source. Keep authored/state/player/assistant roles explicit.
- Test whether more natural source-grounded dialogue can preserve character quality;
  free-form prose is still unverified. Measure controlled full-turn latency and peak
  game/runner/system RAM; the new sample is post-run only.

- Evaluate an opt-in factual-answer workflow using current host facts, exact original
  player quotes and explicit unknown attributes; keep creative character flavor separate.
  The reported dialogue corpus contains both correct recall and false autobiographical
  claims. Current schema/source checks do not enforce semantic grounding.

- Broaden dialogue action-intent handling on fresh phrasing, including stack quantities,
  offers and nontrade interactions. Current example purchase routing is deliberately
  English and sells one item; shared reference/confirmation APIs accept host policies.

- Replicate the focused human-play workflow and entity guard on unseen semantic families
  and states; separate incorrect local bypasses, actual model correctness and clarification
  burden. New 8/8 pilot reuses regression questions and proves no broad accuracy claim.
- Measure full human-turn latency, correction cost and runner/system RAM for the new
  conversation workflow; individual call timings in these diagnostics are insufficient.
- Test authoritative fact phrasing and bounded speech claims without bloating prompts;
  current Qwen dialogue can still invent effects or contradict ownership.

- Use the dungeon to measure planning/state-change failures separately from legal output. Test explicit subgoals or validated procedures on new seeds; preserve the original failed smoke rather than silently constraining away repeated look actions.

- Validate the new readable codec, typed tools and no-call router in live integrations; measure incorrect bypasses and full-turn savings on new scenes.
- Compare configuration candidates with complete real memory/latency data; current new tuning checks are scripted, not automatic hardware tuning.

- Live local scene/dialogue workloads and held-out question templates; the current Qwen result is small synthetic decision tasks. Compare 2K/4K context and 2/4/6/8 threads under controlled RAM/thermal load before changing defaults.
- Measure 1B/1.5B and larger size-class workloads independently; their profiles are advisory and unmeasured. No larger models were loaded by this change.
- Test host intent boundaries/correction text on live requests and held-out semantic families; the new guard enforces a supplied subset but cannot infer intent.

- Exercise the integration kit and native adapters in real hosts; capture lifecycle, unit/visibility and engine deployment gaps. The current bridge reuses registration, not model context or per-request row JSON.
- Compare independently recorded recipe/guidance candidates on live questions with the evaluation kit; current kit checks are offline scripted correctness, not model accuracy.

- Catalog `relevant_to` measured with model questions (only checked by conformance cases so far).
- Validate query guidance on independent question templates and live hosts; the opt-in profile has mixed per-model results on fresh row seeds.
- Validate host meanings on other schema families and live questions; targeted meanings still miss 15/60 executed queries.
- Reaction-brain feedback from real hosts and unseen policy families; versioned numeric bindings exist, while engine sensors, host metrics and scheduler notifications/costs remain integration work.
- Measure acknowledged synchronization and periodic snapshot policies on live multi-turn model decisions.
- Provider prompt-cache discounts; the small-model query token check omitted cached-token accounting, so discounts remain unverified.
- Independent ports checked against `conformance/` if hosts need them; thin Node/TypeScript, C# and Python adapters now reuse Rust through versioned bridges.
- Feedback from the first real game integration once its gaps list arrives.
- Measure indexed-recall precision and procedure eligibility on real host questions; aliases/normalization remain opt-in and live request savings are unmeasured.
- Test host-defined semantic constraints for all-row versus eligible-row totals; explicit wording still fails some route questions. Keep both hosted cohorts frozen.

## Routine follow-up evaluation

- Test authored routines and time-source explanations on unseen real game scenes and
  question phrasing. The optional runtime is implemented; new gameplay/model-speech
  accuracy remains unmeasured. Source-classification/adapter diagnostics are recorded
  separately above; keep historical pilots frozen.

## Dynamic character stages

See [implementation and completion record](characters/LIFE.md) and [evidence](evidence/LIFE_EVAL.md).

- Stage 1: `f863b45`, approved life definitions, subjective attitudes and attributed
  experiences; 52 independent workflows, bounded host appraisal, original editable files.
- Stage 2: `65a8367`, dependency-sensitive scenes/topics and partial delivery; 41 workflows,
  one-use request tickets, concurrent threads and fake speech transport checks.
- Stage 3: `e25b9ef`, branch/identity-bound checkpoints, archive backpressure, deterministic
  ranges and causal recall/erasure; 20 workflows including 1,000 events.
- Stage 4: `fedab27`, actual attitudes/commitments, accepted notebook promises and event
  tokens drive priority; 80 workflows, zero-call routine operation, per-turn reservations,
  greeting/depth guards, failed receipts/backoff, and 96-scope scheduler checks.
- Stage 5: implemented persistent player/author controls, notebook/continuity/initiative
  recovery, 27 context-scaling workflows, 39 cross-language fixtures, 24 scaled natural
  requests and a tested original merchant demonstration. Tool registration remains explicit.
- Host optimization evidence: zero-allocation structural checks; 128 experiences/appraisals
  median 176,881,587→7,338,259 ns over seven pairs, allocator calls 2,517,745→22,025,
  final bytes identical. No full-turn latency or cache-hit claim follows.
- Retain failed local speech artifacts `life-model-{tokens,results,review}.json`: four new
  scenario families, 24 calls, $0 API. Both variants invent birthplace details; the candidate
  invents clouds and has two unfinished replies despite 8/8 valid shapes. Do not tune these
  examples and relabel a rerun as unseen accuracy. Earlier speech failures remain frozen.
- Next evidence: independent human review and fresh continuous model sessions, privacy and
  speech integrations in actual hosts, controlled cache/latency/RAM comparisons. Generic
  size profiles and stable prefix ordering establish no model capability or cache benefit.

## Known gaps

- Table/catalog word matching recognizes namespace/dotted/path components and optional plural `s`, while memory recall/index tokenization retains its prior contract. Optional `RecallIndex` adds simple English suffix normalization/host aliases, not semantic embeddings or a multilingual stemmer.
- Numeric filtering by reading rows fails in every format; use `where` or have the model request a `query`.
- Query is a SQL subset: one table, AND/IN, no joins, subqueries or GROUP BY; ORDER BY/LIMIT run before aggregates. Arithmetic uses 64-bit floats.
- Schema-only requests can confuse semantic labels (`unique` rarity became count = 1). Valid but wrong queries do not retry.
- Original query workflow eval covers four states of one synthetic game family; the new alpha.3 check adds six states across three tabular schema families. All remain synthetic; live games and different distributions may differ.
- Brain tests use synthetic labels and one initializer in final test runs, not live gameplay or model-authored-policy quality. Original single-rate switch test misses 78/384; rate selection misses 25/384 on a fresh cohort (paired baseline misses 69/384). Validation improvements do not guarantee gameplay or out-of-distribution behavior.
- Default rate selection measures decisions with every output eligible and zero thresholds. Hosts can opt into `train_select_host` for masks/thresholds, but selection still adds training work and can overfit reused validation data.

- Runtime timing fixtures use two synthetic network sizes and mostly static packet reuse; real engines, allocators and actor change rates may differ. Estimated frame-work budgets are not wall-clock deadlines.
- Sync deltas compress host transfer, not automatically LLM input/history; integrations reconstruct snapshots or implement explicit state handling. Actual local prefix-cache tokens/timings remain unmeasured without a running server.

- Evidence validation establishes source availability/provenance, not logical entailment or calibrated confidence. Host evidence collection, consent, clock progression and archival storage remain integration responsibilities.
- Indexed recall owns a filtered snapshot and must be rebuilt after edits/erasure/privacy changes. Outcome/temporal trackers are transient; hosts supply source IDs, coverage, clocks and any durable history. Runtime ledger persistence/prices and current permission checks remain host responsibilities. Reusable drafts and observed outcomes do not prove causal effectiveness.


## Turn-aware speech follow-up

- Implemented v2 profile/known-state policy gating, explicit player/merchant fact subjects,
  direct utterance rendering and bounded punctuation/schema-wall diagnostics. Legacy
  outputs and `life-model-{tokens,results,review}.json` remain byte-identical.
- Independent prototype/Rust parity: 32 compiler workflows, 16 sentence/bound checks,
  new error/conformance cases, persistent CLI/native checks and model-text mutations.
- Fresh paired local Qwen follow-up: four regression/four holdout families, 48 calls,
  $0 API. All four v1 candidate regression raw responses reproduce exactly. Terminal endings
  5/8→8/8, suspected schema-wall cutoffs 3/8→0/8, single-sentence heuristic 0/8→5/8.
- Preserve remaining failures: v2 still invents clear weather and a dark blue wick; one
  reply exceeds the instructed 120 characters. Both v2 origin cases acknowledge unknowns
  without invented origins. Do not present this as general subject resolution or grounding.
- Raw v2 token/results/review artifacts and [method/limits](evidence/LIFE_SPEECH_V2_EVAL.md) are retained;
  no tuned rerun, larger model, automatic repair or default source-answer change. Next:
  independent human review and fresh multi-turn families covering unknown scenes and stock.

## Modular archetype and multi-party layer

- Completed opt-in ambient/service/quest/companion loadouts and a composable builder with
  validated module dependencies, zero-call background policies, actual existing-runtime
  adapters, model hints and explicit tool registration controls.
- Added current-Life listener/speaker attitude gates for lore, private facets and offered
  affordances, including multi-listener all-of requirements, deterministic deferral and
  a gate-aware Life tool dispatcher. Withheld payloads/IDs stay outside compiled requests.
- Added bounded host hearing receipts, active-AI relative roles/attribution, subject-ranked
  attitudes, shared per-turn floor quotas, causal/root and confirmed-reply guards, one-use
  stale-checked grants and SpendingLedger reservations/cancellation.
- Added disabled-by-default spotlight routing for significance, actual threshold crossings
  and explicit conflict signals, with focused fallback on spending/call exhaustion and
  identity-pinned deployment binding. Selected routes project actual context caps.
- Evidence: 68 independent Python/Rust workflows, 68 additive conformance cases (904 prior
  cases unchanged), 136 Python CLI/native replies and 68 Node/C# fixture replies each.
  Editable original four-archetype/six-participant example runs without a model.
- Offline compiled requests measure 3,210 trusted / 2,708 untrusted bytes, with zero gated
  payloads in the untrusted request and zero ambient reserved calls. 0 model calls / $0 API;
  tokenizer counts, tier capability, social quality and latency remain unmeasured.
- Host work remains: truthful receipt/signal/listener inputs, private-scope/redaction policy,
  complete provider counting, actual model loading/call limits, game dispatch, persistent
  scene/floor state, spending settlement and independent live/human evaluation. Existing
  failed speech artifacts remain unchanged. [Full contract/evidence](characters/ARCHETYPES.md).

## Living world and cast pack layer

- Implemented one-file validated CastPack with shared setting/history, locations, groups,
  archetype characters, relationship/gate checks, routines, lore and authored fallback lines.
- Added zero-call rumor updates from host-confirmed co-location, with explicit significance/
  trust/sharing rules, original source/relay/path/hops, per-root learner deduplication and
  bounded rotating pair scans. Reports never become direct observations.
- Added optional fractional group effects for actual learners; effective levels preserve
  chronological saturation/neutral crossing alongside unchanged valid base Life checkpoints.
  The world compiler/tool dispatcher applies them to existing attitude gates.
- Added sourced monotonic host habit counters, relevant bounded badges, current-location
  atmosphere/urgency, and cache-prefix-preserving extra context.
- Added non-blocking opaque speech tickets: immediate authored fallback, one-use bounded
  response validation, deadline/revision checks, raw retention, freeze-before-delivery,
  cancellation and retirement. Model calls, actual delivery/hearing and game actions remain
  host-controlled; no provider, executor or reflection work is introduced.
- Evidence: 54 Python/Rust workflows and 54 additive cases; 972 prior cases and six frozen
  life-model artifacts unchanged. Same fixtures run through Python/native/Node/C# bridges.
  Includes one-file example, mutation checks and a source-bound 1,000-frame release trace
  of four routines/initiative, 64 roots, 128 reports and 200 simulated completions.
- 0 actual model calls / $0 API. No semantic accuracy or whole-game 60 FPS claim. Host work
  remains authenticated co-location/counters/signals, correct scene/dependency revisions,
  foreground budgeting, bounded validation callbacks, real execution/delivery and durable
  journal/ledger retention/compaction/erasure. Root/influence capacities are explicit; no
  automatic archival or unlimited-world claim. [Method and limits](characters/LIVING_WORLD.md).

- Retained host trace: 272 µs median, 348 µs p95, 2248 µs maximum; 1000/1,000 measured host frames below 16,667 µs, with the documented exclusions.

## Overnight deep testing

- Initial 1,000-event archival trace preserves original evidence but legacy active-set
  recall does not search it; 600-chat Web trace rejects 536 turns starting at turn 65
  because routine dialogue fills rumor capacity. Ranked additive repairs and live
  cohort methodology: [overnight evaluation](evidence/OVERNIGHT_DEEP_EVAL.md).
- First runtime repair: transient `LivingWorld::record` plus causal/appraisal-safe
  `Life::trim_transient`, used by the Web host. Fresh 600-chat trace: 0 rejected turns
  versus 536/600 in the retained baseline. A separate 1,000 fully delivered turn test
  includes five save/load restores, at most 128 active events and zero rumor roots.
- Scoped archival selection passes 80/80 declared required-anchor probes across four
  characters, 4,000 events and 40 restores (legacy working-only recall: 0/80). Complete
  correction groups and required attention bounds are explicit. Archived erasure now
  includes causal descendants in tombstones. Host disk archival remains separate.
- Source-bound factual speech is an explicit optional Answers contract and Web policy;
  the bundled Studio uses it, while older config files retain free dialogue. Focused
  Web context filters episode/promise subjects and unrelated item/choice noise. A fresh
  paired local cohort and actual provider-body tier audit are retained separately.
- Implemented the remaining ranked input repair: bounded polite forms/pronouns, explicit
  clarification, current item prices and context-limited focus. Fresh 41-request holdout
  covers 14 phrases with no unintended purchases or interim STT actions. A separate
  delayed-player-switch race reproduced a purchase for the wrong player; optional
  generation receipts and browser scope capture now reject it before mutation.
- The linked 1,000-event-per-character campaign retained accepted promises through all
  40 restores but exposed generic deferral in 9/12 authorized replies. Added an explicit
  required-answer policy and fresh paired archived-state holdout. A failed evaluation
  harness bypassed pack gates via the legacy Life renderer; raw leaks are retained,
  the corrected host uses PartyScene compilation, and the guide names that boundary.

- Completed all ranked overnight repairs and fresh verification: restored-cast paired
  coverage 3/12→12/12, generic deferrals 9→0, no gated watchwords in corrected packets
  or outputs. Optional plural preference queries reproduce all twelve live-tested
  packets without required preference anchors. Uppercase tier normalization is covered.
  Final suite: 1,184 cases (1,122 baseline unchanged), 10,000 new mutations, 17 dungeon
  tests, binding/MSRV/package checks and $0 API. Remaining bounded-storage, host-authoring,
  physical-device and unloaded-tier limits are documented, not capability claims.


## Studio setup and feedback usability

- Explain applied vs draft providers, actual model IDs vs tier hints, and no-model play;
  list installed Ollama models with asynchronous zero-generation local checks.
- Expose bounded per-request activity and reply origin, identify missing-model/connection
  failures, and retain conservative pause handling only for uncertain local failures.
- Route the Talk UI into conversation, put the current reply/input before optional event
  history, and acknowledge text with one click. Keep advanced picture/audio controls out
  of the main setup path.
- Add confirmed New game with monotonic stale-result rejection; keep settings, saved
  branches, approved assets and spending intact. Offline setup/recovery holdout passes
  eleven workflows without an installed service or API spend.

## Models, residency and dramatic scenes

- Added pure bounded ModelPolicy routing with editable dramatic signals, hold/cooldown,
  independent text/image escalation, local RAM admission and online eligibility.
- Added on-demand/idle residency, predictive preload, native Ollama/SD lifecycle adapters,
  owned launcher controls and parent-pipe cleanup. Audio engines remain separately
  installed/configured services; browser/typing alternatives retain zero-model operation.
- Added detailed dramatic art, model-bound normalized asset identity, labeled scoped
  previous-approved previews and unchanged candidate approval/repair enforcement.
- Evidence: 32 Python/Rust cases, shared bindings, 2,000 mutations, offline host routing
  and owned-process cohorts; no larger-model or image-quality claim. See MODEL_MANAGER.md.

Native voice integration and the Settings menu are documented in [Native voice](studio/NATIVE_VOICE.md). Engines/weights are separately installed; the host launches one bounded audio job at a time, removes temporary recordings, and stops owned process groups on cancellation.

Two-player LAN conversation keeps all processing on the host. See [LAN play](studio/LAN_PLAY.md) for the private listener, join code, guest permissions, delivery/floor rules and offline evidence.

- Voice/Settings/LAN verification: 13 Python/Rust audio plan fixtures, 2,000 parser mutations,
  seven offline CLI-host workflows, ten LAN authorization workflows, four host mock AI calls
  alternating player subjects, and ten real Firefox Settings/two-origin workflows. Manual
  installed Piper/Whisper smoke used one host job each plus one reference synthesis at $0.
  Physical Wi-Fi, microphone accuracy and broad live-model quality remain unmeasured.

- Implemented Piper utterance buffering: one engine session per line, bounded phrase
  preparation, strict PCM joining, measured frame checkpoints and conservative barge-in.
  One installed 61-character comparison: 13 jobs/11.0014 seconds → 1 job/1.2949 seconds
  of synthesis turnaround (8.5× in that single sample; excludes physical playback).
- Fixed native worker launch after Linux hot rebuilds and relative capture/model paths
  for wrappers that change directory. Added microphone device selection, input meter,
  recording/transcribing status and ordered interim completion before STT dispatch.
- Evidence: 14 Python/Rust buffer cases, 2,000 policy mutations, 2,000 WAV mutations,
  seven player callback workflows, nine native-host recovery workflows and a real
  Firefox synthetic-speech capture/resample/Whisper check. Physical mic accuracy remains
  unmeasured. Earlier evaluation/conformance outputs remain frozen.

Voice recording history, cancellable local replies and the floating context/execution
profilers are documented in [Studio diagnostics](studio/STUDIO_DIAGNOSTICS.md).

- Completed: optional persistent character viewpoints with scoped personality/Life context, delivery confirmation and branch/reset behavior; Thinking UI hides old replies. Twelve parity cases, nine host workflows, five Firefox workflows; fresh local 3B holdout 3/3 relevant/accepted versus 0/3 baseline. See [evidence and limits](characters/CHARACTER_VIEWPOINTS.md). Host-gated extensible reactions and transfer adapters are next.

- Completed: opt-in host reaction policy with attitude/personality/resource/reason gates, fresh authorization, physical-threat provenance and registered generic handlers. Studio coin/item gifts/loans/support conserve resources with scoped debts and interleaved checkpoint replay. 22 parity cases, 12 host workflows (20 coins conserved), six Firefox workflows, 0 extra model calls. [Limits/integration](characters/CHARACTER_REACTIONS.md).

- Fixed: personal small talk falling through to role-only speech enums. Opt-in social routing, immediate varied greetings, zero-call mood/explanation turns with NPC/player perspective and confirmed context, and duplicate/engine-metadata rejection. 22 parity cases, 12 host workflows and five Firefox workflows; paired local 3B experiments/final holdout and limits in [studio conversation](studio/STUDIO_CONVERSATION.md).

- Completed: per-NPC/listener encounter memory and zero-model contextual greeting plans, full-heard introductions, scoped legacy migration and branch/reset behavior. Arrival/Approach uses host-confirmed stage directions; typed travel is permission checked and guests cannot bypass host control. 26 parity cases, two new aliases, 12 encounter / seven action host workflows and seven Firefox workflows; [evidence/limits](characters/CONTEXTUAL_GREETINGS.md).

- Optional native reasoning controls and ephemeral hypotheses: implemented with exact-model
  capability checks; 28 independent offline policy cases and automatic-speech browser policy.
  No reasoning quality claim: the installed Qwen 2.5 model advertises no thinking controls.
  Bounded sentence continuations and longer player-message assembly are now implemented.

- Conversation extensions: 22 message cases/14 action-registry cases; 10 offline host
  and 6 Firefox workflows. Sentence continuations use one text call and discard unheard
  tails. Stock phrasing, purse privacy and sale acknowledgements corrected.
- Local 3B follow-up retained: 4/4 complete packets, but the optional summary omitted a
  corrected name and added latency (61.161s vs 52.841s on one long pair). Summaries stay
  off by default; full input and protected name excerpts are retained. Future work:
  broader semantic summary coverage, persistent whole-opinion bundles and measured
  sentence-audio lookahead; no gapless playback or intelligence gain claimed.
