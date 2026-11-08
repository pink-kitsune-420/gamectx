# A dynamic intelligent being: definition, tasks, actions and the AI/world gap

This is a read-only source audit of branch `v0.4.0a` (November 2026). It defines what a
simulated being needs, maps each need to the modules that already exist, and lists what
is missing between the character's mind and the world model. It makes no quality or
accuracy claim and quotes no measurements. Every proposal below is a candidate that must
go through the normal path: `eval/` prototype with predeclared expectations, Rust parity,
conformance cases and documentation.

It complements the [social character gap audit](SOCIAL_CHARACTER_GAPS.md), which looks at
conversation quality. This document looks at the whole agent: body, drives, tasks and
actions in the world, as well as speech.

## 1. Definition

For this project, a **dynamic intelligent being** is a host-owned character whose behaviour
comes from its own state, not from a script, and who changes for reasons it can explain.
It must have all of the following, each as host-validated state with a bounded update rule:

| Faculty | Minimum state | What changes it |
|---|---|---|
| Identity | Name, species, values, temperament, voice, boundaries | Author edits only |
| Body | Health, fatigue, hunger/thirst or species equivalents, injuries, posture, what is held, location | Time, actions, world events |
| Senses | What it can perceive now: sight, hearing, smell, occlusion, distance | Location, light, noise, attention |
| Memory | Episodes (what, who, where, when, why it mattered), with source and corrections | Confirmed experiences only |
| Beliefs | Facts, reports, hypotheses, misconceptions and unknowns, each with a source | Perception, reports, inference, correction |
| Model of others | What it thinks each other person knows, wants, feels and intends | Observation of them, their speech |
| Affect | Short-lived emotions and moods, distinct from lasting attitudes | Appraisal of events; decays with time |
| Relationships | Per-person trust, affection, respect, fear, resentment, obligation, with reasons | Appraised events, rumors, group ripples |
| Drives | Needs with levels that rise and fall, and what satisfies each | Time, activity, consumption |
| Goals | Wants with priority, conditions, progress and reasons to give up | Drives, events, commitments, success/failure |
| Tasks | Concrete committed work: owner, steps, preconditions, progress, deadline, outcome | Planning, assignment, execution results |
| Actions | A vocabulary of things it can do, each with preconditions, cost, duration, effects | World rules (host) |
| Skills | Competence levels that affect outcomes and grow with practice | Practice, training, teaching |
| Routines | A default daily pattern that yields to needs, tasks and emergencies | Calendar, interruption, resumption |
| Initiative | Reasons to act unprompted, with cooldowns | Events, unfinished business, drives |
| Expression | Speech, gesture, silence, matching personality and situation | Every chosen action |
| Continuity | Save, restore, branch and forget without mixing identities | Host checkpoints |

The rule that ties these together: **the model proposes, the host validates and executes,
and only confirmed outcomes become experience.** A being is intelligent in this sense when
its choices are traceable to its state, and its state is traceable to confirmed events.

## 2. The loop

```
 world tick / player turn
        │
   1 perceive ── what this being can sense now (location, senses, occlusion)
        │
   2 interpret ─ attribute, appraise, update beliefs and emotions
        │
   3 want ────── drives + goals + commitments + routine slot → ranked motives
        │
   4 choose ──── pick a task step or a reaction from host-legal actions
        │
   5 act ─────── host validates preconditions and executes atomically
        │
   6 confirm ─── outcome, cost, duration, who perceived it
        │
   7 remember ── store confirmed experience; update body, skills, relationships
        │
   8 learn ───── adjust utilities, skills and beliefs from outcomes
        └──────── back to 1
```

Speech is one kind of action at step 5, not the whole loop.

## 3. What exists, by faculty

Paths are under `src/`. "Exists" means the code is present and tested; it does not mean
the demo uses it or that it is wired to the other faculties.

| Faculty | What exists | Where |
|---|---|---|
| Identity | Life facets (`identity`, `background`, `value`, `voice`, `temperament`, `boundary`, …); sheet `persona` (role, summary, appearance, values, fears, desires) and `speech` style; archetype loadouts; cast packs | `character/life.rs`, `character/sheet.rs`, `character/archetype.rs`, `character/cast.rs` |
| Body | Sheet `state` (posture, held, gaze, activity, awareness, exposure, expression), `features`, `clothing`, `sensory`; bio health status and conditions; fitness (strength, endurance, agility, condition, carry capacity, travel speed, per-action fatigue cost, training) | `character/sheet.rs`, `character/roleplay/engine/bio.rs`, `character/fitness.rs` |
| Senses | Engine `Perception` and `Addressing` (earshot, distance, facing); scene facts tagged global/perceived/mutual/report; visual occlusion; timeline `visibility` (public/witnessed/private) and `witnesses` | `character/roleplay/engine/mod.rs`, `character/life/continuity.rs`, `media/visual.rs`, `knowledge/timeline.rs` |
| Memory | Life experiences with kind, source, causes and retention; episodic checkpoints, archives, consolidation, recall and erasure; `Memory` with importance; evidence memory; observation history; recall index | `character/life/storage.rs`, `knowledge/memory.rs`, `knowledge/memory/evidence.rs`, `knowledge/observation.rs`, `knowledge/recall.rs` |
| Beliefs | Attitude kinds `belief`, `report`, `hypothesis`, `misconception`, `unknown`; fact registry with authority and expiry; `deception` and `mistaken` config switches; attributed rumors with hop paths | `character/life.rs`, `knowledge/facts.rs`, `character/living.rs` |
| Model of others | Scoped per-listener names, preferences and disclosures; viewpoints (saved stances between actor and listener); party roles and confirmed hearing | `character/roleplay/engine/mod.rs`, `character/roleplay/viewpoints.rs`, `character/party.rs` |
| Affect | Attitude and facet kind `emotion`; appraisal rules with step limit, cooldown and reasons; living-world atmosphere | `character/life.rs`, `character/living/context.rs` |
| Relationships | Attitude kinds `familiarity`, `trust`, `affection`, `resentment`, `obligation`, `disagreement`; sheet `relationships`; community trust relations; group ripples | `character/life.rs`, `character/sheet.rs`, `knowledge/communities.rs`, `character/living.rs` |
| Drives | Sheet `needs` (id, label, threshold); the host sends live levels each turn and the engine reports urgent ones | `character/sheet.rs`, `character/roleplay/engine/mod.rs` |
| Goals | Sheet `goals` (description, priority 0–100, `when` conditions); Life `goal`, `commitment`, `unfinished`, `priority` kinds; codex quests by stage | `character/sheet.rs`, `character/life.rs`, `knowledge/codex.rs` |
| Tasks | No single task object. Closest: routine activities, initiative candidates, promises/loans, playbook procedures, scheduler jobs (see §4) | — |
| Actions | Many separate action surfaces (see §5) | — |
| Skills | Bio skills and knowledge with levels; recipes check a skill requirement; fitness attributes can be trained | `character/roleplay/engine/bio.rs`, `knowledge/recipes.rs`, `character/fitness.rs` |
| Routines | Activities with location, window, repeat, priority, requirements, interrupt/resume/missed policy and grace; deterministic timeline; activity menu | `character/routine.rs`, `character/routine/runtime.rs` |
| Initiative | Candidates of kind `activity`, `ask`, `followup`, `decline`, `gesture`, `silence`, ranked with priority, attitudes, events and promises; call quotas, backoff, outcome receipts | `character/life/initiative.rs` |
| Choice | Reaction brains (small utility networks with training, validation gates and hysteresis); one bounded decision over host-legal pairs; intent narrowing; human confirmation; outcome tracker | `runtime/brain*.rs`, `runtime/decision*.rs`, `runtime/outcome.rs` |
| Expression | Roleplay engine (34 switchable features), line book, expression, wording, greetings, reactions, deliberation notes, speech tickets and delivery offsets | `character/roleplay/` |
| Continuity | Life checkpoints with fences; save files with named, forkable states; replay recorder | `character/life/storage.rs`, `kit/save.rs`, `runtime/replay.rs` |

The foundation is broad. The gap is mostly not "a missing faculty" but **missing state
dynamics and missing connections between faculties**.

## 4. Tasks

### What exists today

| Thing | What it is | What it lacks as a task |
|---|---|---|
| Routine activity | Intended activity in a time window at a location | No steps, no effects, no progress; `active` means selected, not done |
| Initiative candidate | A ranked reason to act now | One shot; no plan behind it; kinds are mostly social |
| Sheet goal | A want with priority and conditions | No progress, no plan, no success or failure state |
| Life `goal`/`commitment`/`unfinished` | Text plus a level | Same; text only |
| Promise / loan | A recorded obligation | Not linked to steps that would fulfil it |
| Codex quest | Stages for a listener | Player-facing; NPCs do not pursue quests themselves |
| Playbook procedure | A validated reusable recipe of steps | Returns drafts; no owner or progress |
| Scheduler job | Work units for the host | Engine work, not character work |

### Definition

A **task** is committed work owned by one being (or a group), with:

- `id`, `owner`, optional `assigned_by` and `for` (who benefits)
- `motive`: the goal, need, promise or order it serves, by id
- `steps`: an ordered or partially ordered list of action references (§5), each with
  preconditions and expected effects
- `status`: proposed, accepted, active, blocked (with reason), suspended, done, failed,
  abandoned
- `progress` per step, `deadline` and `priority` on the host clock
- `resources` reserved (items, tools, stations, places)
- `on_fail` / `on_block`: retry, replan, ask for help, give up, with bounded attempts
- `history`: confirmed step outcomes, which become Life experiences

### Rules that keep it host-owned

1. A model may propose a task or a replan. The host checks it against the action catalog,
   the owner's knowledge and current permissions, and accepts or rejects it.
2. Only the host advances a step, from a confirmed action outcome.
3. Tasks compete with the routine slot, urgent needs and emergencies through one ranking
   (§6, gap 3), so a being can skip lunch to finish a promise, or drop the promise when
   hurt.
4. Planning depth, step count and replans are bounded, like every other model input.

## 5. Actions

### What exists today

Actions are spread over many modules, each with its own shape:

| Surface | Actions |
|---|---|
| Things | take, store, move, load, unload, create, destroy, travel (carriers), best_tool, whats_here |
| Recipes | craft, cook, build (changes places and entrances), repair, dismantle; inputs, tools, stations, skill, time |
| Species | pet, feed, calm, lead, ride, train, harvest, behave |
| Communities | add/remove member, set customs, set/check relation, appraise, barter |
| Inventory | atomic transfers |
| Fitness | travel cost, can carry, train |
| Props | registered interactions per prop |
| Sheet services | offered services with conditions |
| Reactions | gift, loan, help, repay, debt query |
| Engine | directions (`approaches`, `enters`), entrances, stage actions, command grammar |
| Initiative | activity, ask, followup, decline, gesture, silence |

Each one validates and executes correctly on its own. What is missing is a common
description that a planner, an initiative ranker, a brain and a model can all read.

### Definition

An **action** is a host-registered verb with:

- `id`, `actor` requirements (species, skills, body state, membership)
- `targets`: typed slots (thing, person, animal, place, community)
- `preconditions`: host conditions over world, body and inventory
- `cost`: time, fatigue, resources consumed, money
- `effects`: world changes (executed by the existing module), body changes, need changes,
  skill practice, relationship appraisal events
- `perception`: who can notice it (visibility, noise, witnesses), feeding the timeline
- `risk`: possible failure outcomes and their effects
- `interruptible` and `duration`

The existing ops stay as they are and become the executors; the catalog is a read-only
description over them. This would let one query answer "what can this being do here,
now, and what would it change?", which no single module answers today.

## 6. The AI/world gap

These are the places where the world model and the mind do not reach each other. Each was
checked by reading the source; "none found" means a search of `src/` found no code for it.

### Gap 1: world events do not become experience automatically — **DONE** ([Evidence](../characters/LIFE.md#witness-bridge-gap-1))

The timeline records `actors`, `witnesses` and `visibility`, and Life accepts host
experiences, but no bridge in `character/life` or `character/living` turns a witnessed
timeline entry (or a recipe, species, barter or things outcome) into a Life experience
for each witness. Every host must write that glue. Without it, a being does not know
that the bridge was built or that its animal was fed unless the host remembers to say so.

*Needed:* a bounded `witness` step: timeline entry + witness list + each being's senses →
one `direct` experience per perceiving witness, `report` for the rest when told.

### Gap 2: needs have no dynamics

Sheet `needs` are an id, a label and a threshold. The host supplies levels each turn. No
code raises a need over time, lowers it when satisfied, or maps an activity, recipe or
item to the need it satisfies. Hunger, thirst, sleep, warmth, safety and company were not
found anywhere in `src/`. Fatigue is computed per action by `fitness` but is not kept as
state that accumulates and recovers.

*Needed:* need definitions with rate per tick, satisfiers (action or item → amount), and a
body state record (fatigue, health, injuries, hunger-like needs) that actions update and
rest restores. Species should be able to declare their own needs.

### Gap 3: no single motive ranking

Routines rank activities, initiative ranks candidates, sheet goals have priorities, needs
have thresholds, promises have deadlines and brains rank outputs. Each ranking is
separate. Nothing decides "my need for food (80) beats my routine (shop shift) but not my
promise (due now)".

*Needed:* one deterministic arbiter that takes motives from all of these with the same
bounded scale and returns a ranked list with reasons. Brains can be one input, not the
arbiter. This must stay explainable: every choice names the motive that won.

### Gap 4: goals do not become tasks

Goals and commitments are text with a priority. There is no planning step, no task object
(§4) and no progress. Initiative kinds are mostly social; "go to the mill and buy flour"
can only be expressed as an opaque `activity`.

*Needed:* the task object, a planner over the action catalog (deterministic search first,
model proposal second, host validation always), and progress fed by confirmed outcomes.

### Gap 5: no shared action catalog — DONE

See §5. Without it, initiative, brains, decisions, routines and the model each see a
different list of what a being can do.

*Implemented:* [`ActionCatalog`](../../src/knowledge/action_catalog.rs) (local kind `actions`)
provides a read-only metadata catalog over all 11 operational surfaces (things, recipes,
species, communities, inventory, fitness, props, services, reactions, engine, initiative)
with 49 built-in actions. 25 cases (9 malformed) in [`conformance/actions/`](../../conformance/actions/),
2,000 differential parity fuzzing workflows (seed 7, 0 mismatches). Zero model spend, zero
behavior changes. [Guide](../guides/WORLD.md#actions).

### Gap 6: emotions do not fade, and some feelings are missing

`emotion` is an attitude kind changed by appraisal, with step limits and cooldowns. No
time decay was found (`decay` occurs nowhere in `src/`). Attitude kinds have no `fear`,
`respect`, `gratitude`, `envy` or `admiration`; the social audit already names fear and
respect as needed. Mood (a short-lived background state) is not separate from emotion.

*Needed:* a decay rate per attitude kind toward a baseline set by temperament, and the
missing kinds as additive enum values (a format change for Life definitions).

### Gap 7: little theory of mind

A being records what a listener said (names, preferences, disclosures) and its own stance
toward them. It has no record of what it thinks someone else knows, believes, wants or
feels. It cannot reason "she did not see the theft, so she does not know" or "he wants
the key, so he may lie".

*Needed:* per-other beliefs: `knows(other, fact)`, `wants(other, goal)`, `feels(other,
emotion)`, each with a source (seen, told, inferred) and the same provenance rules as the
being's own beliefs. Timeline `witnesses` already supply the raw data for `knows`.

### Gap 8: skills do not grow with use

Bio skills and knowledge have static levels. Recipes check a skill but do not grant
practice. Only fitness attributes can be trained. Skill level does not change the outcome
quality or failure chance of an action.

*Needed:* practice gain per successful action (bounded, with diminishing returns), skill
effect on duration, quality and risk in the action catalog, and teaching between beings.

### Gap 9: three separate relationship stores

Sheet `relationships` (authored labels), Life attitudes (trust, affection, …), community
relations (group trust) and viewpoints (saved stances) each hold part of "how I feel about
you". They are not reconciled. A barter that goes badly changes none of the personal ones.

*Needed:* a documented precedence and one appraisal path: action outcomes produce
appraisal events, Life attitudes hold personal values, community trust is derived from or
combined with members' attitudes by an explicit rule.

### Gap 10: off-screen life has intent but no effects

Routines select intended activities, and the docs are clear that `active` does not mean
done. Nothing resolves what an off-screen activity produced or consumed (bread baked,
flour used, fatigue gained). The world therefore does not change while the player is away,
unless the host does it.

*Needed:* optional coarse resolution: when an activity window closes unobserved, apply the
action catalog's expected effects once, record a `private` or `witnessed` timeline entry
and a Life experience. Bounded work per tick, using the existing scheduler.

### Gap 11: no curiosity, exploration or interest model

No code makes a being notice something new and want to look at it, learn about it or ask
about it. Initiative can ask, but only from host-authored candidates.

*Needed:* a novelty signal (new person, new thing, changed place, unknown fact referenced)
that produces an initiative candidate, scaled by temperament.

### Gap 12: no mortality, injury or incapacity

`death`, `injur` and `hunger` occur nowhere in `src/`. Bio health is a status plus
conditions without effects. A being cannot be hurt, sick, asleep, unconscious or dead in a
way that the other systems respect (routine skipped, speech blocked, actions limited).

*Needed:* body conditions with effects on the action catalog (blocked verbs, extra cost),
routine (forced rest) and expression (cannot speak while asleep), all host-set.

### Gap 13: intentional deception is a switch, not a model

Life has a `deception` config flag and secrets have covers. There is no record of what a
being has told whom that it knows to be false, so it cannot keep a lie consistent or be
caught in one.

*Needed:* a told-ledger per listener (claim, true value, when), checked before speech and
appraised by the listener when contradicted.

## 7. Suggested order

Ordered by how much each unblocks the others, not by size.

1. **Action catalog** (gap 5) — **DONE** ([Evidence](../guides/WORLD.md#actions)). Read-only description over existing ops. Unblocks planning,
   motive ranking, skills and off-screen effects. No behaviour change.
2. **Witness bridge** (gap 1) — **DONE** ([Evidence](../characters/LIFE.md#witness-bridge-gap-1)). Small, deterministic, high value: the world starts reaching
   memory without per-host glue.
3. **Body and need dynamics** (gaps 2, 12). Rates, satisfiers, rest and conditions.
4. **Motive arbiter** (gap 3). One explainable ranking over needs, routine, goals, tasks,
   promises and emergencies.
5. **Tasks and planning** (gap 4). Task object, deterministic planner, model proposals
   validated by the host.
6. **Affect decay and missing kinds** (gap 6), **skill growth** (gap 8).
7. **Theory of mind** (gap 7), **deception ledger** (gap 13), **relationship
   reconciliation** (gap 9).
8. **Off-screen effects** (gap 10) and **curiosity** (gap 11).

Each step should be zero model calls first. Items 1–4 and 6 are pure host computation and
can be verified offline. Items 5, 7 and 11 involve model proposals and need `eval/`
measurements on the two default small models before any accuracy claim.

## 8. Acceptance scene

A single offline scene that exercises the whole loop:

1. A baker NPC's routine says "bake" from dawn. Hunger rises over the morning.
2. The player promised yesterday to bring flour; the baker remembers the promise.
3. The player does not come. The flour need blocks the bake task; the baker replans:
   walk to the mill (travel cost, fatigue), barter for flour (community market).
4. On the way, the baker witnesses a cart crash (timeline entry, `witnessed`). The baker
   records a direct experience; a second NPC who was indoors does not.
5. Back home the baker is hungry and tired; the arbiter picks eating before baking.
6. Baking succeeds; the skill gains practice; flour is consumed; bread exists.
7. The player arrives. The baker greets them with mild resentment (broken promise,
   decaying since morning), mentions the crash only as something it saw, and tells the
   player the second NPC does not know about it yet.

Every step must be traceable to confirmed state. The scene passes when replaying it gives
byte-identical state and every line of speech cites only what the baker actually knows.
