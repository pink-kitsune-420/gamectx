# A dynamic intelligent being, pass 2: how dynamic, and what comes next

[Pass 1](INTELLIGENT_BEING.md) defined a dynamic intelligent being, audited the source and
listed 13 gaps between mind and world. This pass assumes all 13 are implemented as
proposed and asks two questions:

1. How dynamic would a character be then?
2. What would still be missing, and what new risks would those features create?

This is a design projection written in November 2026 against branch `v0.4.0a`. Nothing in
it is measured. The levels below are a way to compare designs, not scores. Any feature here
still follows the project's path: `eval/` prototype, Rust parity, conformance and docs, with
numbers before claims.

## 1. A scale for "dynamic"

| Level | Name | Meaning |
|---|---|---|
| 0 | Absent | Not represented |
| 1 | Authored | Fixed text or values written by an author |
| 2 | Reactive | Changes when the host reports an event, by a fixed rule |
| 3 | Dynamic | Also changes on its own over time, and feeds other faculties |
| 4 | Adaptive | Changes its own parameters from experience (learns) |
| 5 | Generative | Forms new goals, plans, knowledge or habits nobody authored, still host-validated |

A believable character needs most faculties at level 3. A character that feels
*intelligent* needs some at 4 and a few at 5.

## 2. Scorecard

"Now" is the source as audited in pass 1. "After pass 1" assumes all 13 gaps closed.

| Faculty | Now | After pass 1 | What gets it there / what holds it back |
|---|---|---|---|
| Identity | 1 | 1 | Authored by design; no slow drift (gap B4) |
| Body | 2 | 3 | Needs with rates, fatigue that builds up, conditions |
| Senses | 2 | 2 | The witness bridge automates delivery, but noticing is still yes/no (gap A2) |
| Episodic memory | 2 | 3 | Witnessed events and outcomes become experiences automatically |
| World knowledge | 2 | 2 | The being still uses the host's action catalog as perfect knowledge (gap A1) |
| Model of others | 1 | 3 | knows/wants/feels with sources |
| Affect | 2 | 3 | Decay toward temperament baseline; fear, respect and others added |
| Relationships | 2 | 3 | One appraisal path from outcomes |
| Needs | 1 | 3 | Rates, satisfiers, species needs |
| Goals | 1 | 2 | Still authored; goals do not arise from experience (gap A4) |
| Tasks and plans | 0 | 3–5 | Deterministic planner gives 3; validated model proposals for new situations reach 5 |
| Actions | 2 | 2 | The catalog describes existing verbs; it adds none |
| Skills | 1 | 4 | Practice, diminishing returns, teaching |
| Routines | 3 | 3 | Authored, can be interrupted; they do not reshape themselves (gap A5) |
| Initiative | 2 | 3 | Curiosity produces candidates |
| Choice | 2 | 3 | One explainable arbiter; brains already reach 4 inside it |
| Honesty / deception | 1 | 3 | A record of lies told, consistency, getting caught |
| Off-screen life | 1 | 3 | Routine windows produce effects |

**Verdict.** After pass 1, a character is a complete *dynamic* agent in the
utility-and-planning style: it gets hungry and tired, plans errands, witnesses things,
remembers them for a reason, holds grudges that fade, keeps or breaks promises, lies
consistently, improves at its trade and keeps living while the player is away. That is a
large step and already more than most game characters have.

It is still **mostly reactive to its authored setup**. It learns *how well* to do things
(skills, brain weights) but not *what is true about the world*, *what to want*, or *what
went wrong*. It does not anticipate, coordinate with others, or change who it is. Those
are the pass 2 gaps.

## 3. What becomes possible after pass 1

These are combinations no one would need to script, if the systems are wired as proposed:

- A blacksmith short of iron replans to salvage a broken cart she witnessed crashing.
- A guard who saw the theft knows the innkeeper did not, so he warns her before the thief
  returns.
- A merchant who lied about her stock avoids the topic with the one customer who might
  check.
- An apprentice gets better at bread over a week of routine baking, and the baker's
  opinion of him rises because the outcomes improve.
- A villager skips the festival because she is injured, and her absence is recorded.

If one of these does not happen in a soak test (§7), something in the wiring is wrong.

## 4. Pass 2 gaps

Grouped by area. Each says why it matters once pass 1 is done.

### A. Cognition

**A1. Subjective world knowledge.** — **DONE**. After pass 1 the planner reads the host's action
catalog, so every being knows every precondition and effect exactly. A real being believes
"the mill sells flour" and can be wrong. Needed: a per-being, sourced copy of the parts of
the catalog it knows ("I know how to bake", "I heard the mill sells flour"), learned by
doing, watching and being told. A failed action corrects the belief. This turns world
knowledge from level 2 into 4, and it is what makes asking, teaching and exploring
worth doing.

*Implemented (pass 2 stage 2):* `character::beliefs::BeliefStore` (local kind `beliefs`).
The planner still reads the host catalog; `known_actions` and `gaps` are the bridge until it
reads beliefs directly. [Evidence](../characters/LIFE.md#subjective-world-knowledge-pass-2-a1).

**A2. Attention and graded noticing.** — **DONE**. The witness bridge says yes or no. Noticing should
depend on distance, light, noise, what the being is doing, its skills and what it cares
about. It should be deterministic (seeded by the host), so replay holds. Attention should
also be limited, so a busy being misses things. Without this, everyone nearby notices
everything, and secrets have nothing to work with.
*Implemented (pass 2 stage 5):* `character::attention::AttentionModel` (local kind `attention`). [Evidence](../characters/LIFE.md#attention-and-graded-noticing-pass-2-a2).

**A3. Expectation and surprise.** — **DONE**. A being should expect things: "the player comes at noon",
"the oven takes an hour", "my friend will repay". Store these as expectations with a due
time. When one is broken, that becomes an appraisal event (surprise, worry, relief).
Without expectations, a missed meeting produces nothing, because nothing *happened*.
*Implemented (pass 2 stage 5):* `character::expectation::ExpectationBook` (local kind `expectation`). [Evidence](../characters/LIFE.md#expectations-and-surprise-pass-2-a3).

**A4. Goals that arise from experience.** — **DONE**. After pass 1 every goal is still authored. Goals
should form from motives: repeated hunger → "keep a food store"; repeated theft → "get a
lock"; admiration → "learn that skill". Use bounded templates (need, threat, admiration,
loss, opportunity → goal pattern) with host validation, and a model proposal only for
cases the templates do not cover.
*Implemented (pass 2 stage 6):* `character::aspiration::Aspirations` (local kind `aspiration`). [Evidence](../characters/LIFE.md#goals-from-experience-pass-2-a4).

**A5. Habits that form and routines that adapt.** — **DONE**. The living world counts habits for
badges, but routines are authored. Repeated choices should become routine entries, and
routine entries that keep getting skipped should weaken, within limits and with reasons.
*Implemented (pass 2 stage 6):* `character::habit::Habits` (local kind `habit`). [Evidence](../characters/LIFE.md#habits-and-adapting-routines-pass-2-a5).

**A6. Reflection and lessons.** — **DONE**. Consolidation already turns episodes into ranges. Add a
bounded *lesson* record: a pattern over confirmed outcomes ("deals with the trader went
badly 3 of 4 times") that the arbiter and appraisal can use. Lessons cite their episodes,
can be wrong, and are revised by new outcomes. This is how a being explains a change of
heart.
*Implemented (pass 2 stage 6):* `character::lesson::Lessons` (local kind `lesson`). [Evidence](../characters/LIFE.md#lessons-pass-2-a6).

**A7. Bounded rationality.** — **DONE**. If every being plans perfectly, they all look the same.
Per-being cognition settings, set from temperament: plan depth, how far ahead it looks,
impulsiveness (acting on the top motive without planning), memory span, and
error rate on unfamiliar tasks. Seeded and deterministic.

**A8. Forgetting and misremembering.** — **DONE**. Episodes are kept, archived or erased on purpose.
Add natural fading of unimportant memories, and optional, bounded errors (wrong day,
merged people) marked so the host still knows the truth. This makes conflicting
testimonies possible.

*Implemented (pass 2 A7/A8):* `character::cognition::Cognition` (local kind `cognition`).
[Evidence](../characters/LIFE.md#bounded-rationality-and-fallible-memory-pass-2-a7a8).

### B. Social

**B1. Communication as action.** — **DONE**. After pass 1, speech is still mostly an answer. With a
model of others, speaking should be a planned action: inform, ask, request, persuade,
warn, command, comfort, threaten. Each has preconditions and expected effects on the
listener's knows/wants/feels. Asking becomes the way to fill knowledge gaps (A1).

*Implemented (pass 2 stage 2):* `character::speech_act::SpeechActs` (local kind
`speech_act`). [Evidence](../characters/LIFE.md#communication-as-action-pass-2-b1).

**B2. Cooperation and shared tasks.** — **DONE**. Tasks have one owner. Add joint tasks with roles,
delegation, asking for help, hand-offs and shared reservations. Add promises between
characters, not only with the player.

**B3. Conflict over resources.** — **DONE**. Two beings can plan to use the same oven, the same
last loaf, the same seat. The host already does atomic transfers; what is missing is
reservation contention, queuing, negotiation, giving way by status or relationship, and
the grudges that follow.

*Implemented (pass 2 B2/B3):* `character::cooperation::Cooperation` (local kind
`cooperation`). [Evidence](../characters/LIFE.md#cooperation-and-conflict-pass-2-b2b3).

**B4. Slow personality change.** Identity is authored and fixed. Long experience should be
able to shift values and temperament slightly, with a hard cap, a reason chain and an
author switch to turn it off. Without it, a betrayed character can be angry but never
becomes more guarded.

**B5. Conscience and norms.** — **DONE**. Communities have customs. Add a personal side: which norms
the being holds, guilt or shame when it breaks one, indignation when others do, and
sanctions through reputation. This connects lying (pass 1 gap 13) to a cost.
*Implemented (pass 2 stage 4, with creeds):* `character::creed::CreedManager` (local kind
`creed`). [Evidence](../characters/LIFE.md#creeds-and-conscience-pass-2-stage-4-and-b5).

**B6. Character-to-character conversation off-screen.** — **DONE**. Rumors spread as records. Beings
should also hold short exchanges with each other when they meet, chosen from what each
knows and wants (B1), summarized as records with no model call by default. This is
where gossip content, alliances and quarrels come from.

*Implemented (pass 2 B6):* `character::chatter::Chatter` (local kind `chatter`).
[Evidence](../characters/LIFE.md#off-screen-conversations-pass-2-b6).

**B7. Status, roles and jobs.** — **DONE**. Community membership exists. Add roles with duties
(guard, baker, elder) that create tasks automatically, and status that affects
conflicts (B3) and persuasion (B1).

*Implemented (pass 2 B7):* `character::roles::Roles` (local kind `roles`).
[Evidence](../characters/LIFE.md#status-roles-and-jobs-pass-2-b7).

### C. World and body

**C1. Life cycle.** — **DONE**. Species have lifespans. Characters do not age, have children, retire,
die of age or leave belongings and relationships behind. Needed only for long campaigns,
but it is what makes a world feel continuous.

**C2. Threat response.** — **DONE**. With fear added (pass 1 gap 6) and injuries (gap 12), add a
danger appraisal and fight/flee/hide/freeze/call-for-help actions, with temperament
choosing between them. Make safety a need.

*Implemented (pass 2 C1/C2):* `character::survival::Survival` (local kind `survival`).
[Evidence](../characters/LIFE.md#life-cycle-and-threat-response-pass-2-c1c2).

**C3. Space inside a place.** Places have links and spots. Beings do not occupy spots,
block each other, queue, or see across a room by line of sight. Crowding at a well
or a counter is a large source of natural behavior.

**C4. Economy feedback.** Markets and bartering exist. Prices do not respond to scarcity
created by everyone's needs and off-screen production. Without it, off-screen effects
(pass 1 gap 10) never show up as a reason to act.

**C5 (added during pass 2). Diets and foods.** Hunger was one number satisfied by a flat
`eat`. Foods now differ in fill, taste, freshness and safety, and beings differ in what their
body allows, what they refuse, what makes them ill and what they like.
*Implemented (pass 2 stage 3):* `character::diet::DietManager` (local kind `diet`).
[Evidence](../characters/LIFE.md#diets-and-foods-pass-2-stage-3).

**C6 (added during pass 2). Creeds.** Communities had customs but no shared belief systems.
Religions, ideologies, philosophies and traditions now give beings tenets, timed
observances, sacred places and things, devotion, standing and stances toward rival creeds;
creed food taboos feed diet `forbids`.
*Implemented (pass 2 stage 4):* `character::creed::CreedManager` (local kind `creed`).
[Evidence](../characters/LIFE.md#creeds-and-conscience-pass-2-stage-4-and-b5).

### D. The system as a whole

**D1. Stability.** — **DONE**. Pass 1 adds many feedback loops. Expected failure modes:
- *Dithering:* switching between two close motives every tick. Needs commitment inertia
  in the arbiter (the brains already have hysteresis; the arbiter needs its own).
- *Starvation spirals:* tired → cannot work → cannot buy food → more tired. Needs safety
  valves (charity, rest defaults) or at least detection.
- *Herding:* every being picks the same action at the same time.
- *Rumor and grudge runaways:* bounded already per hop; check whole-world totals too.
- *Deadlocks:* two tasks each holding what the other needs.
Add world-level monitors that detect these in soak tests and report them.

*Implemented (pass 2 stage 1):* `character::stability::StabilityMonitor` (local kind
`stability`) detects all five modes over host-reported turns and names the motive to hold
when dithering; the arbiter's optional `commitment` input supplies the inertia. A soak test
is still open.
[Evidence](../characters/LIFE.md#stability-monitors-pass-2-d1).

**D2. Level of detail for minds.** — **DONE**. The scheduler already budgets updates. Add explicit
mind detail levels: full loop near the player, coarse needs/routine/effects far away,
statistical summaries for distant crowds. Moving between levels must not lose or invent
state.

*Implemented (pass 2 D2):* `character::detail::Detail` (local kind `detail`).
[Evidence](../characters/LIFE.md#level-of-detail-for-minds-pass-2-d2).

**D3. Save size and migration.** — **DONE**. Body, needs, tasks, lessons, expectations, other-models
and lie ledgers all grow. Each needs bounds, pruning rules and a save-format migration
path (this links to the roadmap's in-place migration follow-up).

*Implemented (pass 2 D3):* `character::retention::Retention` (local kind `retention`).
[Evidence](../characters/LIFE.md#save-growth-retention-and-migration-pass-2-d3).

**D4. Explaining behavior.** — **DONE**. A "why did she do that?" trace for any action: perception →
belief → motive scores → chosen task step → outcome. This is the main authoring and
debugging tool for everything above, and the inspection profilers are the place to start.

*Implemented (pass 2 stage 1):* `character::explain::ExplainTracer` (local kind `explain`).
[Evidence](../characters/LIFE.md#explain-trace-pass-2-d4).

**D5. Default packs.** Each faculty adds authoring work. Ship original default packs
(needs and actions per species, common tasks, norm sets, temperament presets) so a
small game gets dynamic characters with little authoring.

### E. The model's role

After pass 1 almost everything is deterministic host code, which suits 1B–3B local
models. The model is most useful at the edges:

- **Interpreting free player speech** into proposed knows/wants/feels updates,
  appraisal events and requests (B1), validated like any other model output.
- **Proposing goals and plans** for situations templates do not cover (A4, pass 1 gap 4).
- **Saying it**: wording speech from the chosen communicative act and the being's state.

What to measure: whether a small model, given only the compiled slice of state for one
decision, proposes valid and in-character options more often than the deterministic
default. If it does not, keep the default. The state is now much richer than any context
window, so **selection of what to show** becomes the main lever, which is what the
packet and budget modules already do.

## 5. Suggested order after pass 1

1. **D4 explain trace** and **D1 stability monitors.** Build these first so that every later
   step can be seen and checked. — **DONE** ([Evidence](../characters/LIFE.md#explain-trace-pass-2-d4)).
2. **A1 subjective knowledge** and **B1 communication as action.** Together these make
   asking, telling, teaching and lying matter. — **DONE** ([Evidence](../characters/LIFE.md#subjective-world-knowledge-pass-2-a1)).
3. **A3 expectations** and **A2 attention.** Cheap, deterministic, and they create many
   events from things that do *not* happen. — **DONE** (stage 5).
4. **A4 goals from experience**, **A5 habits**, **A6 lessons**. This is the step to
   level 4–5. — **DONE** (stage 6).
   *Inserted on request (stage 7):* host-set limits for every module, so a game sets how many
   characters, foods, creeds and entries it needs, with measured cost at large casts. 7a (pass 2
   modules), 7b (pass 1 character modules) and 7c (knowledge kinds: places, timeline, things,
   species, recipes, communities, actions, lore, inventory, evidence, notes, facts) — **DONE**
   ([Evidence](../characters/LIFE.md#host-set-limits)).
5. **B2/B3 cooperation and conflict** — **DONE**, **B7 roles** — **DONE**, **C4 economy**, **C3 space**.
6. **A7 bounded rationality** — **DONE**, **A8 forgetting** — **DONE**, **B4 slow change**, **B5 conscience** (done with creeds, stage 4),
   **B6 off-screen talk** — **DONE**.
7. **C1 life cycle** — **DONE**, **C2 threat** — **DONE**, **D2 detail levels** — **DONE**, **D3 save growth** — **DONE**, **D5 packs**.

## 6. Risks to watch while pass 1 is built

- **Determinism.** Every new rate, decay and choice must use host ticks and integer
  arithmetic (as brains do with signed millionths) so Python/Rust parity and replay hold.
- **One tick, many systems.** Define a fixed order for each tick (body → perception →
  appraisal → motives → choice → effects → memory) and document it, or two hosts will get
  different results from the same save.
- **Explanations must be real.** The arbiter's "reason" must be the actual winning term,
  not a text summary written afterwards.
- **Don't let numbers leak as psychology.** Levels remain design controls, as the Life docs
  already say.
- **Model output stays a proposal.** Richer state invites the model to "update" it
  directly; every write still goes through validation with bounded size and revision checks.

## 7. Measuring dynamism

All offline, zero model calls:

- **Soak test:** simulate a small town for 30 in-game days. Report D1 failure counts,
  need ranges, task completion and failure rates, and save size growth.
- **Counterfactual test:** change one event (the player keeps the promise) and replay. The
  affected being should diverge for a stated reason; unrelated beings should not.
- **Diversity test:** give two beings the same situation and different temperaments. Their
  choices should differ, and each difference should trace to a setting.
- **Consistency test:** the same being in the same state chooses the same thing (replay).
- **Legibility test:** for each choice, the explain trace (D4) names its causes.

With model calls, and only after the offline tests pass: the edge tasks in §4E, on the two
smaller default models within `--cap` limits, with expectations written down first.
Whether the result *feels* intelligent to a player needs human review, which none of these
tests replace.
