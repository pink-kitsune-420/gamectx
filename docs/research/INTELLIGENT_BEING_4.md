# A dynamic intelligent being, pass 4: genre-agnostic minds, defenses, living culture, life projects and chemistry

[Pass 1](INTELLIGENT_BEING.md) closed the 13 foundational gaps between the world model and the
character mind. [Pass 2](INTELLIGENT_BEING_2.md) made characters adaptive: subjective knowledge,
graded attention, goals from experience, habits, lessons, communicative acts, shared tasks,
norms, an economy, explain traces and stability monitors. Pass 3 made them *present*: one turn
frame with salience-ranked context, memory cueing with an epistemic gate, a pragmatic parser
with a mixed-initiative agenda, ambivalence with embodied micro-reactions, and declarative
world and social schemas (`knowledge::surroundings`, `knowledge::fabric`), verified by the
"Interrupted Accusation" acceptance scene and three deep roleplay scenarios.

This fourth pass asks:

> **Once a character is present in the moment, what still stops gamectx from carrying
> believable beings into any genre, any kind of mind, and any length of story—and what
> mechanisms let those beings protect themselves, share a culture that changes, pursue
> projects that take months, and click (or clash) with each other?**

The rules stay the same as in earlier passes. Every mechanism is deterministic host computation
first ($0 API spend), with bounded memory and work, host-set limits with hard ceilings, and
strict host validation of anything a model proposes. New numeric state uses **fixed-point
signed millionths** (an `i64` where 1,000,000 is 1.0); levels a host reads directly may stay
0–100 integers. Randomness is a pure function of a host seed and the inputs (FNV-1a over a
canonical string, as `character::chatter` already does), never a clock or an OS source.

---

## 1. The pass 4 problem: a present person, but only one kind of person in one kind of world

After pass 3, a baker in a village reacts like a real person. The same engines strain when the
story is not about a human in a pre-industrial village, when it lasts a season rather than an
evening, or when the interesting thing is between two characters rather than inside one.

1. **Genre is baked into vocabulary, not mechanism.** The engines are mostly neutral: ids are
   free identifiers, needs and norms are authored data. The *built-in vocabulary* is not. The
   pass 2 default packs ship `human`, `dog`, `horse` and `goat` with pastoral roles and village
   norms; the pass 3 pragmatic lexicon is a fixed English phrase list over fixed acts; the
   world schema's light levels, access kinds and contract kinds are closed lists chosen for a
   town. A space station, a noir city, a post-collapse wasteland or a school drama can be
   authored only by bending these words (a "bakery" that is really a hydroponics bay), and
   nothing checks that a genre's data is complete or consistent.
2. **Every mind is a human mind.** Each module assumes one individual with private memory,
   human affect channels, language, theory of mind and a human sense of time. A hive whose
   drones share one memory, a ship's maintenance intelligence that never feels fear but weighs
   risk exactly, a wolf that understands tone but not words, a spirit bound to one obsession,
   or a long-lived being for whom a year is short cannot be expressed without lying to the
   engines (zeroing affect, faking speech) and accepting outputs that do not fit.
3. **Appraisal is accurate even when the truth hurts.** Pass 3 makes stress visible
   (embodiment), but every appraisal reads the facts as they are. Real people protect
   themselves: they deny the evidence, blame someone else for what they did, explain away a
   failure, take their anger out on a safe target, or joke so they do not have to feel. Without
   that, characters under pressure either break in one step or never break at all, and the
   lesson and drift engines learn the "correct" lesson from every humiliation.
4. **Talk is copied, not retold.** Off-screen `chatter` passes a topic's value verbatim from
   speaker to listener. Real rumors drift: details drop, numbers grow, the wrong person gets
   blamed, two stories merge. Communities have no shared way of speaking that spreads and fades
   (slang, nicknames, sayings), so a town after a season sounds exactly like it did on day one,
   and a stranger sounds like a local.
5. **Goals are short.** Aspirations turn pressure into a goal, tasks plan a few steps, and
   routines repeat. Nothing holds a project that takes months (rebuild the mill, win the
   apprenticeship, court someone, pay off a debt): milestones, setbacks, the time and money
   already sunk, how much of the self rides on it, and when a person gives up or doubles down.
6. **Pairs have history but no chemistry, and scenes have no pace.** `fabric` remembers how two
   people met and what is unfinished between them, but not whether their humour fits, whether
   one always talks over the other, which jokes have become *theirs*, or when a back-and-forth
   has run its course. Conversations end only when the host stops them; a scene has no sense of
   rising tension, a needed pause, or a good place to stop.

---

## 2. Scorecard: from present person (pass 3) to anyone, anywhere, over time (pass 4)

| Dimension | After pass 3 | After pass 4 | What changes |
|---|---|---|---|
| Setting | Town vocabulary in built-in lists | Genre profiles with checked vocabularies (`H`) | The same engines run a fantasy village, an orbital station, a noir city or a school by data alone |
| Kinds of mind | One human template, species change only needs | Mind architectures (`I`) | Shared memory, no affect or other affect channels, no language, fixed obsessions, other time scales |
| Self-protection | Accurate appraisal; stress shows but never distorts | Defense mechanisms with a strain budget (`J`) | Denial, projection, rationalization, displacement, humour, withdrawal; maturity; a bounded breaking point |
| Shared language | Fixed lexicon; rumors copied verbatim | Community idiom and rumor mutation (`K`) | Slang spreads by contact and prestige and fades; retold stories drift measurably from the truth |
| Time horizon | Goals and short plans | Life projects (`L`) | Months-long projects with milestones, setbacks, sunk cost, identity stake, abandonment or renewal |
| Between two people | History (first meeting, landmarks, threads) | Chemistry, banter and running jokes (`M1–M2`) | Compatibility, playful moves, callbacks that stay fresh, rapport that grows or sours |
| Scene shape | Per-turn decisions only | Pacing controller (`M3`) | Beat density, a tension arc, silences, and a host signal for when a conversation should end |

---

## 3. Pass 4 gaps

### H. Genre agnosticism

**H1. Genre profiles.** A genre profile is authoring data that declares the vocabulary each
closed list in the engines may use, plus a few setting axes:

```json
{
  "genre": {
    "id": "orbital_station",
    "axes": {"tech": 85, "magic": 0, "law_reach": 70, "scarcity": 60, "day_cycle": "artificial"},
    "light_levels": ["off", "night_cycle", "work_lights", "full"],
    "access_kinds": ["public", "crew_only", "officers_only", "quarters", "sealed"],
    "contract_kinds": ["shift_swap", "ration_loan", "keep_secret", "alibi_pact", "employment"],
    "currencies": ["ration_credit"],
    "severity_names": {"etiquette": "protocol", "crime": "station_offense"},
    "address_defaults": {"stranger": "crewmate", "acquaintance": "spacer"}
  }
}
```

Each list maps onto the engines' fixed internal ordering (a station's `full` is the engine's
brightest level), so mechanism and conformance outputs stay the same and only the words
change. Profiles are validated whole: every list has the required arity, every name is an
identifier, no two names map to the same slot. The host can ship several and switch per scene.

**H2. Lexicon packs per genre and register.** The pass 3 pragmatic lexicon gains named packs
(`village_plain`, `station_crew`, `noir_street`, `school_casual`). A pack adds phrases to the
existing acts; it never adds acts, so the parser's output format is unchanged. Packs are data
and are checked (phrase length, word limits, no duplicates across acts).

**H3. Genre conformance.** A *genre portability check* compiles one abstract scene (a theft, a
witness, an accusation, a resolution) under each shipped profile and requires the same decisions
(same act sequence, sanction step, salience order) with only vocabulary differing. This is the
pass 4 guard against a mechanism quietly depending on a town word.

*Host authority:* profiles and packs are host configuration; model output never selects or
edits them.

### I. Non-human cognition

**I1. Mind architecture.** A being gains an optional architecture that the engines consult:

| field | values | effect |
|---|---|---|
| `memory` | `individual` · `shared:<pool>` · `none` | where episodes, facts and cues live; a hive's drones read and write one pool (bounded, with per-drone recency) |
| `affect` | a subset of channels, or `none` | which emotions exist; a machine may have only `alarm` and `satisfaction`; absent channels never appear in frames |
| `language` | `full` · `signals` · `none` | `signals` beings parse tone and a small signal set (calls, gestures) instead of clauses; `none` beings take only physical acts |
| `deception` | `none` · `concealment` · `full` | whether the being can lie (`none`), only withhold (`concealment`), or tell cover stories (`full`) |
| `theory_of_mind` | 0 · 1 · 2 | 0: others are objects; 1: others want and know things; 2: others model *me* (needed for bluffing) |
| `time_scale` | millionths (1,000,000 = human) | stretches subjective time anchors and cooldowns; a long-lived being's "just now" spans weeks |
| `fixation` | optional `{goal, strength}` | a spirit or obsessive whose top motive is pinned unless a release condition is met |

**I2. Architecture-aware engines.** Engines read the architecture rather than branching on
species: the epistemic gate refuses cover stories from a `deception: none` mind; the agenda
never asks a `theory_of_mind: 0` being to answer "what do you think I want?"; embodiment maps
missing channels to neutral; cueing uses the shared pool's recency per drone. Each change is a
new optional input with defaults that keep every existing conformance output byte-identical.

**I3. Reference minds (packs).** Original example architectures: a *hive tender* (shared
memory, alarm and contentment only, no lying), a *station steward intelligence* (full language,
no fear, exact risk, concealment only, theory of mind 2), a *herd animal* (signals, fear and
comfort, no deception), a *hearth spirit* (fixation on one household, slow time scale). These
are authored from scratch for gamectx.

### J. Defense mechanisms and the breaking point

**J1. Threat to self.** An appraisal becomes *ego-threatening* when it lowers a value the being
holds about itself (competence, goodness, belonging) by more than its tolerance, or exposes a
secret it protects. Threat is computed by the host from existing appraisals (lesson outcomes,
sanction steps, contradiction appraisals, exposed lies), never from model text.

**J2. Mechanisms.** A threatened being picks one mechanism, by a fixed first-match rule over its
style and current strain, each with a defined effect on what the being believes and says:

| mechanism | maturity | belief effect | speech/behaviour effect |
|---|---|---|---|
| denial | low | the threatening fact is held at reduced confidence and not repeated | refuses the topic; insists |
| projection | low | blame or motive is attributed to someone else (a bounded, explainable shift) | accuses |
| displacement | low | anger is redirected to a safer target with lower status | snaps at the apprentice, not the guard |
| rationalization | mid | the outcome is reframed with an excuse fact (tagged `self_serving`) | explains too much |
| withdrawal | mid | none | leaves, goes quiet, fewer initiatives |
| intellectualization | mid | none | talks procedure, not feeling |
| humour | high | none | jokes; tension drops |
| sublimation | high | strain feeds a project milestone (`L`) | works harder |

Maturity is a per-being profile with an authored base that drifts slowly (the pass 2 drift
engine). Mechanisms change **framing and behaviour**, never the host's truth: the world model
still holds what happened, and every distortion is tagged so the explain trace can say *"she
blamed the apprentice because admitting the open lockbox threatened her competence"*.

**J3. Strain and the breaking point.** Each being has a strain level (millionths) that defenses
consume and rest, support from trusted others and successful coping restore. Low-maturity
defenses are cheap now and costly later (strain decays slower while a denied fact keeps being
contradicted). When strain crosses an authored threshold the being *breaks*: one bounded crisis
outcome chosen by profile (confession, flight, collapse into withdrawal, an outburst) with
cooldowns so it cannot repeat every tick. The stability monitor gains a *defense loop* warning
(the same denial re-triggered too often).

### K. Living culture: idiom, slang and rumor mutation

**K1. Community idiom.** Each community holds a bounded set of idioms (`{id, phrase, meaning,
prestige}`): slang, nicknames, sayings. A being's *repertoire* records which idioms it uses
with a strength. Exposure (hearing an idiom from someone) raises strength by an amount scaled by
the speaker's status and the listener's openness; disuse fades it. A being uses an idiom in
speech plans only above a threshold, so a newcomer starts sounding local after enough contact,
and a fad from one prestigious speaker spreads and dies out.

**K2. Rumor mutation.** When `chatter` retells a topic, a seeded mutation step may apply one
operator, each bounded and recorded:

| operator | example | bound |
|---|---|---|
| drop detail | "at the mill after dark" → "at the mill" | removes one qualifier |
| exaggerate | 3 coins → 5 coins | numeric values grow by at most a set ratio per hop |
| shift attribution | "the apprentice" → "a boy from the bakery" | only to a being plausibly linked |
| merge | the theft + the fire → "the fire covered the theft" | only topics the speaker holds |
| soften | "stole" → "borrowed" | only when the speaker likes the subject |

The mutation rate depends on the speaker's carefulness and the listener's credulity; each
retelling keeps a *truth distance* (the count of applied operators, plus numeric drift in
millionths) and a provenance chain. Corrections (a witness retells the original) reduce the
distance for listeners who trust the witness. This gives the pass 2 rumor-runaway monitor a
measurable quantity instead of a count.

**K3. Host authority.** Mutated rumors are *beliefs*, never facts: the world model is never
rewritten by gossip, and the epistemic gate frames them as hearsay with their chain.

### L. Life projects

**L1. Project model.** A project is `{id, owner, aim, milestones[], horizon, stake, resources}`.
Milestones are ordered, each with a completion condition the host can check (a task outcome, a
fact, an item, a relationship level) and an expected duration. `stake` is how much of the
being's identity rides on it (0–100); `resources` tracks time and money committed so far.

**L2. Dynamics.** Progress comes only from confirmed outcomes. Setbacks (a failed milestone, a
lost resource) lower *morale*; morale and stake decide among `persist`, `adapt` (swap a
milestone for an alternative), `pause`, or `abandon`. Sunk cost is modeled honestly: a high
investment makes abandonment less likely even when the expected value has dropped, bounded so
that a being does not throw everything after a lost cause forever. Completion and abandonment
feed drift (identity), lessons and the aspiration engine (a new project can grow from an old
failure).

**L3. Scheduling.** Projects propose routine slots and tasks through the existing scheduler and
role duties; they compete with needs and duties in the motive arbiter with a weight that grows
near deadlines. Off-screen resolution advances projects coarsely when unobserved.

### M. Chemistry, banter and pacing

**M1. Pair chemistry.** Each being gets a small *social style*: humour kind (dry, warm, absurd,
cutting, none), tempo (fast, measured, slow), formality, assertiveness. Chemistry between two
beings is a fixed function of style fit (complementary or clashing), shared values, and history
(landmarks, favors, breaches). It biases cooperativeness, initiative toward each other and the
chance a playful move lands.

**M2. Banter moves and running jokes.** A banter grammar names playful moves: tease, callback,
escalate, deflate, compliment-in-disguise, rapport check, and a graceful exit. The host selects
a move from chemistry, mood and pacing, and a model words it. A *running joke* is a callback to a
shared moment that keeps a freshness value: each use spends freshness, time restores it, and a
joke used too often stops landing and is retired. Hurtful moves are bounded by rapport: a tease
only lands above a rapport floor and otherwise appraises as an insult.

**M3. Pacing controller.** A scene keeps a tension arc (millionths), beat density (beats per
in-game minute), and a silence budget. It tells the host when to slow down (a pause, a small
physical beat), when to escalate, and when the conversation has run its course (no open thread,
low novelty, falling engagement) so a character can end it naturally instead of waiting for the
player.

---

## 4. Numeric and safety conventions

- New quantities (strain, morale, chemistry, idiom strength, truth distance, tension) are signed
  millionths with explicit saturating bounds; percentages reported to hosts stay integers.
- Every per-being, per-pair and per-community table is a host-set limit with a hard ceiling.
- Every loop is bounded by its inputs (no unbounded search); retelling and mutation are
  per-hop and per-tick.
- Mutation, idiom spread and banter selection are seeded by FNV-1a over a canonical string
  `"<seed>:<being>:<topic>:<tick>"`, so replays are byte-identical.
- Model proposals (a defense justification, a banter line, a retold rumor wording) are untrusted
  text checked by existing validators and the epistemic gate; models never choose the mechanism,
  the mutation, or the move.

---

## 5. The pass 4 turn loop (additions to pass 3)

1. Resolve the scene's genre profile and lexicon packs; load each participant's mind
   architecture.
2. Build the pass 3 turn frame; drop channels the architecture lacks; stretch time anchors by
   `time_scale`.
3. Appraise; for ego-threatening appraisals run the defense step (mechanism, strain, tagged
   distortion) before the agenda.
4. Add idioms above threshold to the speech plan; add project pressure to the agenda; consult
   pair chemistry and the pacing controller for initiative and banter moves.
5. Model words the plan; the host gates it (epistemic gate, cover stories, idiom and banter
   checks) and records outcomes, which feed projects, idiom exposure, rumor provenance and
   strain.

---

## 6. Implementation order (pass 4 stages)

Each stage follows the project discipline: an independent Python reference with predeclared
expectations, a Rust port byte-identical to it, 25 conformance cases including at least 6
malformed or boundary cases, 2,000 differential parity workflows (seed 7), mutation fuzzing in
`tests/robustness.rs`, a scale row timed at 1,000 and 10,000 entries, and docs quoting only
measured numbers.

1. **P4-1 (T-041) Genre profiles and lexicon packs (`H1–H3`).** Profile validation, vocabulary
   mapping onto fixed slots, lexicon packs, and the genre portability check across at least
   four original settings.
2. **P4-2 (T-042) Mind architectures (`I1–I3`).** Architecture records, architecture-aware
   options in cueing, gate, agenda and embodiment (defaults byte-identical), shared memory
   pools, reference minds.
3. **P4-3 (T-043) Defense mechanisms and strain (`J1–J3`).** Ego-threat appraisal, mechanism
   selection, tagged distortions, strain budget, breaking point and the defense-loop monitor.
4. **P4-4 (T-044) Idiom and rumor mutation (`K1–K3`).** Community idioms, repertoire exposure
   and fading, seeded mutation operators, truth distance and corrections.
5. **P4-5 (T-045) Life projects (`L1–L3`).** Projects, milestones, morale, sunk cost,
   persist/adapt/pause/abandon, scheduling hooks.
6. **P4-6 (T-046) Chemistry, banter and pacing (`M1–M3`).** Social styles, chemistry, banter
   moves, running-joke freshness, pacing controller.

---

## 7. Pass 4 deep tests (predeclared expectations)

These are written before any implementation; they are targets to measure, not claims.

1. **Cross-genre.** One scene skeleton (a theft, a witness, an accusation, a resolution) runs
   under at least four genre profiles (village, orbital station, noir city, school) with the
   same participants' psychology. *Expect* identical decision sequences (acts, sanction steps,
   salience order, defense choices) and differing vocabulary only; any decision difference is a
   bug unless a profile axis is documented to change it.
2. **Breaking point.** One being is pushed with escalating ego threats (exposed lies, public
   sanctions, failed milestones) while rest and support are withheld. *Expect* the mechanism
   sequence to move from mature to immature defenses as strain rises, exactly one crisis outcome
   at the threshold, no crisis repeated within its cooldown, bounded strain, and an explain trace
   for each step. Then restore support and *expect* strain to fall and mature defenses to return.
3. **100 agents over 180 days.** A seeded town of 100 beings with communities, idioms, rumors
   and projects runs for 180 in-game days off-screen. *Expect* every host-set limit to hold
   (memory per being bounded and measured), idiom adoption to form clusters by community with
   fads that rise and fall, rumor truth distance to grow with hops and fall after corrections,
   at least some projects to finish, adapt and be abandoned, no stability-monitor runaway that
   is not explained by inputs, and a byte-identical replay from the same seed. Wall time and
   memory are measured and reported with hardware, not promised.

---

## 8. Pass 4 acceptance scene ("The Missing Ration")

The same scene runs in two genres from data alone.

1. **Setup.** In the *orbital station* profile, the steward intelligence (no fear, concealment
   only, theory of mind 2) logs a missing ration crate; a junior mechanic took it for a sick
   friend. In the *village* profile, the same roles are a granary keeper and an apprentice. A
   long-running project (the mechanic's certification; the apprentice's journeyman test) is two
   milestones from done.
2. **Pressure.** The supervisor questions the mechanic in front of crew. The exposed truth
   threatens the mechanic's competence and belonging: strain rises and the mechanic first
   *rationalizes* ("it was surplus"), then, contradicted, *displaces* anger onto a lower-status
   cadet; the steward conceals the camera record from the crew but not from the supervisor.
3. **Culture.** By the next shift the story has been retold four times with measured truth
   distance (the crate became three crates; the sick friend was dropped); the crew's slang for
   a ration thief has spread from one prestigious speaker.
4. **Resolution.** A trusted friend's support lowers strain; the mechanic confesses (a mature
   path), the supervisor's sanction is one step lower for the confession, the project adapts
   (one milestone delayed, not abandoned), and the friend's dry humour lands as a callback to
   their first shared shift. The pacing controller ends the scene on the callback.

Every decision traces through `ExplainTrace`, the two genres produce the same decisions with
different words, and the scene replays byte for byte.

---

## 9. Out of scope and risks

- **No clinical claims.** Defense mechanisms here are a game model inspired by public
  descriptions of common coping styles; they are not diagnosis or therapy guidance.
- **Content safety stays with the host.** Banter and defenses can produce cutting or hostile
  lines; hosts keep their content policies, and the rapport floor exists so cruelty is a choice
  the host can see, not an accident.
- **Cost.** Mutation, idiom exposure and projects add per-tick work; each stage must show its
  scale row before it ships, and the 100-agent test is the budget check for the whole pass.
- **Byte-identical defaults.** Every pass 4 option defaults to the pass 3 behavior; conformance
  outputs of earlier modules must not change unless a format change is announced.
