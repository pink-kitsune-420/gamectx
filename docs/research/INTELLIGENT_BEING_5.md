# A dynamic intelligent being, pass 5: one runtime, durable minds, spoken by small models, at scale

[Pass 1](INTELLIGENT_BEING.md) connected the world model to a character mind. [Pass 2](INTELLIGENT_BEING_2.md)
made characters adaptive. Pass 3 made them present in the moment. [Pass 4](INTELLIGENT_BEING_4.md) made
them portable across genres and kinds of mind. It gave them defenses, a living culture, projects that run
for months, and chemistry with pacing. The [pass 4 review](../evidence/PASS4_REVIEW.md) records the
evidence:
- six modules;
- 38 deep-test checks;
- the "Missing Ration" acceptance scene, with the same decisions in two genres;
- a seeded town of 100 beings over 180 days, run in 445 ms.

This fifth pass asks:

> **The parts of an intelligent being now exist and are each proven. What still stops a game
> developer from shipping one on a four-core, 8 GB machine with a 1B–3B local model, keeping it
> for a hundred hours, and putting a thousand of them in a town?**

The rules stay the same as in earlier passes:
- deterministic host computation first, at $0 API spend;
- bounded memory and work;
- host-set limits with hard ceilings;
- fixed-point signed millionths for new quantities;
- seeded FNV-1a for every choice that looks random;
- every model output validated by the host before it counts.

New in this pass: each stage that involves a model states its measurements in advance. A claim about
small-model quality ships only with an `eval/` run on local models.

---

## 1. The pass 5 problem: proven parts, but not yet a product

The deep tests and the acceptance scene show that the engines decide well. The same tests also show
what was left to the host to do by hand:

1. **There are forty engines, and the host is the nervous system.** The acceptance scene's `scene`
   function is about 450 lines (after `cargo fmt`), mostly glue:
   - normalize genre words;
   - ask `minds` to admit a request;
   - report a threat to `defenses`;
   - copy a confession into `fabric`;
   - pass a setback to `projects`;
   - feed beats to the pacing controller.

   The roadmap lists eleven "the host does this, not the engine" follow-ups:
   - threats are not fed from lessons or sanctions;
   - projects do not feed drift, lessons or aspirations;
   - chemistry biases are not applied to cooperation;
   - culture is not consulted by `chatter` or the epistemic gate;
   - minds are not consulted automatically;
   - genre words are not used by the engines themselves;
   - traces are not turned into beliefs;
   - weather need rates are not applied;
   - prosody hints are not sent to voice providers;
   - failure risk is not applied to task outcomes;
   - the salience ranker does not drive packet budgets.

   A developer must learn every API and call it in the right order. One wrong order silently
   changes behaviour.
2. **Pass 2–4 state cannot be saved.** `defenses`, `culture`, `projects`, `chemistry`, `minds`,
   `fabric`, `cueing`, `pragmatics` and the others keep their state in memory only. A game that
   reloads loses its strain, rumors, idioms, projects, running jokes and open threads. A hundred-hour
   campaign is impossible without a save format.
3. **No model has spoken a pass 3 or pass 4 decision.** Every transcript so far is host text. The
   mission targets 1B–3B local models. Whether such a model can word "a dry callback to their first
   shift" or "rationalize: it was surplus" faithfully, briefly and without leaking a concealed fact
   has not been measured. The pipeline that turns decisions into a prompt and back into a checked
   line does not exist yet.
4. **The checks read words, not meaning.** The epistemic gate catches a leak only when a whole
   authored key word appears. "The cameras saw it" leaks `camera_record`; "the lens caught them" does
   not. The pragmatic parser misses indirect requests and irony without a cue. Both are English-only.
5. **Scenes have one speaker at a time.** `party` grants the floor. Nothing models the rest of a
   group scene:
   - side-taking;
   - coalitions;
   - an audience changing what someone admits;
   - moods spreading through a room;
   - a group reaching a decision and remembering who dissented.
6. **Characters react; they do not think ahead about others.** A character picks a banter move or a
   defense from its own state. It does not imagine how the listener will take an argument before
   making it. Persuasion, bargaining and deliberate social manoeuvring are therefore shallow.
7. **The player is just another id.** No engine keeps a model of the player:
   - whether they lie;
   - whether they keep promises;
   - what they care about;
   - the content they do not want to see.

   Relationships do not carry across sessions in a way built for a returning player.
8. **Scale is flat.** The 100-agent town runs every being at full detail. A town of 1,000 or 10,000
   needs detail levels: full fidelity near the player and summaries far away. A being pulled into
   focus must be consistent with its summary.
9. **Stories have a scene shape, not a season shape.** The pacing controller shapes one scene.
   Nothing turns the town's live projects, rumors, debts and strain into candidate events across a
   season. Nothing remembers which threads were set up and owe a payoff. No being carries a life
   story it can tell and be consistent with.

---

## 2. Scorecard: from proven parts (pass 4) to a shippable being (pass 5)

| Dimension | After pass 4 | After pass 5 | What changes |
|---|---|---|---|
| Wiring | The host calls ~40 engines in order | Being runtime with a fixed event order (`N`) | One definition per being and one `step`; the eleven follow-ups are wired; host overrides stay possible |
| Persistence | In-memory only | Versioned saves for every manager (`O`) | Save anywhere, then load and continue byte for byte; measured size per being; migrations |
| Speech | Host text only | Intent packets for small models with checked realization (`P`) | Measured fidelity, leak rate, fallback rate and latency on local 3B and 8B models |
| Meaning | Whole key words, English phrases | Concept lexicons, stems, negation (`Q`) | Paraphrase-resistant gate and parser, with measured catch and false-alarm rates |
| Groups | Floor grants | Group dynamics (`R`) | Sides, coalitions, audience effects, bounded contagion, group decisions with dissent |
| Foresight | Rule-chosen moves | Bounded mental simulation, persuasion, bargaining (`S`) | "If I say this, she will…" through the being's own model of the other |
| The player | An id | Player model and boundaries (`T`) | Honesty and promise record, interests, cross-session continuity, lines and veils every engine honours |
| Scale | Everyone at full detail | Levels of detail (`U`) | Focus, near, background and dormant tiers with summaries and consistent promotion |
| Story | Scene pacing | Season director and narrative self (`V`, `W`) | Storylets from live state, a setup-and-payoff ledger, life stories with turning points |

---

## 3. Pass 5 gaps

### N. One being, one runtime

**N1. Being definitions.** One authored record per being names its parts:
- species and mind;
- genre;
- defense profile;
- culture membership;
- social style;
- projects;
- routines;
- creed;
- limits.

The runtime checks the whole record:
- every referenced id exists;
- every part's limits fit;
- no part contradicts another (for example, a mind without language with a talkative social style).

A being can still be assembled engine by engine. The definition is a convenience over the existing
ops and adds no new semantics.

**N2. Event order.** `Runtime::step(events)` takes host events and runs the engines in one documented
order:
- perception and traces;
- attention and salience;
- appraisal;
- defenses;
- beliefs and cueing;
- motives with project weight and fixation;
- agenda;
- chemistry and pacing;
- embodiment;
- decision.

Events include speech, witnessed acts, outcomes, ticks and confirmations. After the decision, the
runtime feeds the outcomes back:
- lessons, drift and aspirations;
- project progress;
- idiom exposure and rumor provenance;
- strain.

Each step returns the decision, its explain trace and a *diff* of what changed in each engine.

**N3. Wired follow-ups.** Each of the eleven roadmap follow-ups becomes a named, documented edge in
the order. Examples:
- a public sanction raises a competence threat;
- an abandoned project feeds a lesson;
- chemistry bias adjusts cooperation;
- a witness's own rumor version is protected.

The pass 4 deep tests showed the last one is needed. Each edge can be switched off by the host, so a
game can keep doing it by hand.

**N4. Equivalence proof.** The Missing Ration scene and the 100-agent town are re-run through the
runtime. Their decisions must equal the hand-wired runs byte for byte wherever an edge is off. Every
edge that is on must change only the fields it documents.

### O. Durable state

**O1. Save format.** Every manager gains `export` and `import`. The output is canonical JSON with
sorted keys and integers only, plus a format version and the host limits it was made under. An
import validates the whole document before replacing any state, so a bad save changes nothing.
Saves are as untrusted as model output: they are checked for bounded size, bounded nesting and every
invariant the ops enforce. The minds pool invariant is one of these.

**O2. Save anywhere.** Saving after any op and loading into a fresh manager must continue byte for
byte. The runtime saves all engines as one bundle with per-engine sections. Then one engine can be
upgraded without rewriting the others.

**O3. Migrations.** Each format version names a pure migration from the version before. Old saves are
kept in `conformance/` as fixtures, and each fixture must load and continue.

**O4. Budgets.** The save size per being is measured for each engine and reported with the scale rows.
A per-being compression option is considered only if the measurements call for it.

### P. Speaking it with a small model

**P1. Intent packets.** The runtime's decision becomes a compact, closed-vocabulary packet. It holds
only what the model must word:

| Field | Example |
|---|---|
| act or move | `callback` |
| mechanism and effect | `rationalization`, `explains_too_much` |
| addressee | the listener |
| allowed facts with framings | `state`, `hedge` or `cover` |
| idioms above threshold | the community's slang |
| tone and pacing hints | from embodiment and chemistry |
| sentence budget | from the stance |
| forbidden key words | from the gate |

The packet is built under a token budget sized for 1B–3B models. The salience ranker decides what is
dropped.

**P2. Checked realization.** The model returns a line plus a self-tag from a closed list (the move it
believes it made). The host checks:
- the epistemic gate;
- length and sentence budget;
- that every named fact is allowed;
- that idioms appear in the packet;
- the self-tag against the intended move.

A failing line is retried once with the reason. After that, the host falls back to an authored
template for the move, so a scene never stalls on a bad generation.

**P3. Fidelity measurement.** `eval/` runs a fixed set of planned turns through local models with
`--cap` limits:
- Llama 3.2 3B and an 8B through the local router;
- a 1B model if available.

It reports:
- tag agreement;
- an offline judge-free check of required elements (the callback names the joke's moment; a
  rationalization contains an excuse);
- gate leak rate before and after the gate, where after must be 0;
- retry and fallback rates;
- tokens in and out;
- p50 and p95 latency on the reference hardware.

T-009, the Lantern Vault chat agreement ticket, fits here as the first real-traffic shadow run.

### Q. Meaning, not keywords

**Q1. Concept lexicons.** Authors group words into concepts, for example `{camera, lens, recording,
footage, feed}` for `camera_record`. The gate matches concepts as well as key words. Packs can be
shared per genre.

**Q2. Normalization.** Words are matched after these steps, all deterministic and table-driven, with
no dependencies:
- a small suffix stemmer for English;
- contraction folding;
- number words to digits.

A negation scope ("I did not see the lens") is reported, not hidden: the gate still flags the concept
but marks it negated, and the host decides.

**Q3. Parser coverage.** These are added:
- indirect requests ("it is cold in here" as a request with a context cue);
- rhetorical questions;
- simple irony markers.

The list of supported forms is published, along with the forms still missed.

**Q4. Measured.** An adversarial set of paraphrased leaks and innocent near-misses is written before
the change. The change ships only with catch and false-alarm rates before and after. English-only
remains a stated limit; language packs are authoring data.

### R. Crowds and councils

**R1. Sides.** In a group scene each being holds a stance on the open question, from −100 to 100.
Stances move by:
- arguments heard (see `S`);
- chemistry with the speaker;
- debts in `fabric`;
- creed fit.

All movements are bounded per turn.

**R2. Coalitions.** Beings with close stances and positive ties form coalitions. A coalition can
speak through one member, and a member who breaks ranks pays a social cost recorded in `fabric`.

**R3. Audience effects.** What a being admits depends on who is in earshot:
- `defenses` gets an audience multiplier on threats;
- the pragmatic stance already reads bystanders.

Confessions in public and in private differ by rule.

**R4. Contagion.** Mood spreads through a room. Each turn, a being's mood moves toward the weighted
room mood by an amount that has a hard cap, with weights from status and chemistry. The cap is
proven in the deep tests to rule out runaway; the stability monitor watches the room total.

**R5. Group decisions.** Votes, consensus with veto, or the leader's call, as the genre's norms set.
The decision records who dissented. Dissent feeds grudges, lessons and later stances.

### S. Thinking ahead about others

**S1. Bounded mental simulation.** Before a social move, a being with theory of mind 2 may simulate up
to K candidate moves against its own *belief copy* of the listener. The copy holds only what the being
knows or believes about the listener, never the host's truth. Each simulation runs the listener's
appraisal and defense rules and predicts the reaction. The being picks by predicted outcome. K, the
depth of 1 and the copy size are host limits. The explain trace records the prediction and, after the
turn, whether it came true. A wrong prediction is a lesson about that person.

**S2. Persuasion.** Arguments have kinds:
- evidence;
- authority;
- appeal to shared values;
- emotion;
- reciprocity;
- threat.

How much a stance moves depends on:
- the listener's carefulness;
- their trust in the speaker;
- the stake;
- whether the argument contradicts a held distortion, which triggers `defenses`.

**S3. Bargaining.** Bargaining runs over a value in `economy`. Each side has a reservation value it
does not reveal, an opening rule and a concession curve shaped by patience and need. A bluff is a
`cover`, so the epistemic gate and `deception` apply.

### T. The player as a person

**T1. Player model.** Each being keeps what it believes about the player. These are beliefs, not
truth, and they flow through `beliefs` and `cueing` like any other:
- an honesty record of lies caught and admitted;
- promises kept and broken;
- topics the player returns to;
- gifts given;
- how the player treats the weak.

What one being believes spreads through `culture` as a reputation that can drift.

**T2. Across sessions.** On load, the runtime turns the gap since the last session into a time anchor
("you have been gone all winter"). It surfaces the dyad's open threads and broken promises once, with
the cueing cooldown.

**T3. Boundaries.** The player sets lines (never shown) and veils (happens off-screen) over content
kinds:
- violence;
- cruelty;
- romance;
- self-harm topics;
- insults aimed at the player.

Every engine that can produce such content consults them first. Examples:
- `chemistry` will not choose a cutting tease at the player under a line on insults;
- `defenses` will not displace onto the player under a veil;
- the realization checks enforce them on model text.

Boundaries are host data and are never inferred.

### U. Scale by level of detail

**U1. Tiers.** Each being is in one tier:

| Tier | What runs |
|---|---|
| Focus | every engine every tick |
| Near | a reduced engine set every tick |
| Background | coarse daily resolution, as in `offscreen` |
| Dormant | nothing, with catch-up on wake |

The runtime moves beings between tiers by distance to the player, relevance and a budget per tick.

**U2. Summaries.** Demotion writes a summary: the fields a lower tier keeps, plus a declared list of
what it drops. Promotion rebuilds full state from the summary and resolves the time missed with
seeded coarse rules. Any field present at both tiers must agree.

**U3. Budget.** The host sets a work budget per tick. The runtime fills it from the focus tier
outwards and reports what it deferred. Nothing is dropped silently.

### V. Seasons, not just scenes

**V1. Storylets.** The director proposes candidate events from live state:
- a project near its deadline;
- a rumor with large drift;
- an overdue debt;
- a being near its strain threshold;
- a coalition one vote short.

It ranks them by a season tension curve the host sets. The host or game picks; the director never
acts.

**V2. Setup and payoff ledger.** Things the player has seen and that the story implies will matter are
recorded with a due window: a sealed door, a promised visit, a rumor about the mill. The director
reports overdue setups and setups to retire, so threads do not dangle silently.

### W. A life story

**W1. Narrative identity.** Each being keeps a short autobiography: up to eight chapters bounded by
turning points from landmarks, completed or abandoned projects, crises and lessons. Each chapter has a
theme from a closed list (redemption, contamination, growth, loss, steadiness). The autobiography is
built by rule from existing records, not written by a model.

**W2. Self-consistency.** Appraisal reads the self-story: a threat to the being's main theme weighs
more in `defenses`. Reminiscence (cueing a chapter) gives the model a one-line packet field, so a
being can say who it is and why, consistently, across sessions.

---

## 4. Numeric and safety conventions

- New quantities are signed millionths with saturating bounds:
  - stance;
  - persuasion pressure;
  - contagion weight;
  - simulation scores;
  - tier budgets;
  - storylet ranks.
- Every new table is a host limit with a ceiling: definitions, saves, packets, concept lexicons,
  coalitions, simulations, tiers, storylets and chapters.
- Mental simulation is bounded by K candidates, a depth of 1 and the size of the belief copy, and
  never reads host truth.
- Saves are untrusted input, validated in full before use.
- Model lines are untrusted. The host chooses the move; the model only words it.

---

## 5. The pass 5 turn loop

1. The host sends events to `Runtime::step`. The runtime selects the tier and the work budget.
2. The engines run in the documented order with the wired edges, as listed in N2.
3. A being with foresight simulates candidate moves against its belief copy (`S`). Group scenes update
   stances, coalitions and contagion (`R`).
4. The decision becomes an intent packet within the token budget (`P1`), filtered by player boundaries
   (`T3`).
5. A local model words it. The host checks it with concept-aware gates (`P2`, `Q`), then retries once
   or falls back to a template.
6. Outcomes feed back through the wired edges. The player model, the ledger and the autobiography are
   updated. The game can save at any step (`O`).

---

## 6. Implementation order and priorities

Every stage follows the project discipline:
- an independent Python reference with predeclared expectations;
- a byte-identical Rust port;
- 25 conformance cases with at least 6 malformed;
- 2,000 parity workflows at seeds 7 and 1;
- fuzzing;
- a scale row;
- docs that quote only measured numbers.

Stages with a model also measure locally within `--cap` limits before anything is claimed.

**Tier 1, which makes it shippable (recommended as pass 5 proper):**
1. **P5-1 (T-049) Durable state (`O`).** Export, import and migrations for every pass 2–4 manager, then the
   bundle. This comes first because everything after it should be saveable from day one.
2. **P5-2 (T-050) Being runtime (`N`).** Definitions, event order, the eleven wired edges with switches, and
   the equivalence proof.
3. **P5-3 (T-051) Intent packets and checked realization (`P`), with T-009.** The packet compiler, the
   realization checks, fallbacks, and the local-model fidelity evaluation.

**Tier 2, which makes it robust and big:**
4. **P5-4 (T-052) Concept lexicons and parser coverage (`Q`).**
5. **P5-5 (T-053) Levels of detail (`U`).**
6. **P5-6 (T-054) Player model and boundaries (`T`).**

**Tier 3, which makes it deeper (a natural pass 6):**
7. **P5-7 (T-055) Group dynamics (`R`).**
8. **P5-8 (T-056) Mental simulation, persuasion and bargaining (`S`).**
9. **P5-9 (T-057) Season director and narrative identity (`V`, `W`).**

---

## 7. Pass 5 deep tests (predeclared expectations)

These are targets to measure, not claims.

1. **Runtime equivalence.**
   - *Expect* the Missing Ration scene and the 100-agent town, run through `Runtime::step` with every
     edge off, to produce the hand-wired decisions byte for byte.
   - With each edge on, *expect* changes only in the fields the edge documents, each with an explain
     trace.
2. **Save anywhere.**
   - The 100-agent town is saved after every op for one day, and at every tenth day otherwise; each
     save is loaded into fresh managers.
   - *Expect* continued runs identical to the unsaved run.
   - *Expect* every truncated, oversized or invariant-breaking save to be refused without changing
     state.
   - *Expect* every older-format fixture to migrate and continue.
   - The save size per being is measured.
3. **Small-model fidelity.**
   - 200 planned turns, covering every banter move, every defense mechanism, cover and hedge framing,
     and idioms, are realized by the local 3B and 8B models.
   - *Expect* 0 leaks after the gate.
   - *Report* the following, with no threshold promised in advance:
     - tag agreement;
     - required-element rates;
     - retry and fallback rates;
     - tokens;
     - p50 and p95 latency on a four-core, 8 GB configuration.
4. **Paraphrase gate.**
   - *Expect* the concept gate to catch more paraphrased leaks than the key-word gate on the
     predeclared set.
   - *Report* the false-alarm rate on innocent near-misses, and the forms still missed.
5. **A thousand beings for a year.**
   - 1,000 beings over 365 days with detail levels.
   - *Expect* every promoted being to agree with its summary on all shared fields, nothing to be
     deferred silently, limits to hold, and a byte-identical replay.
   - *Report* wall time, memory and save size on the reference hardware.
6. **Council stability.**
   - Twelve beings debate one question over thirty turns with contagion on.
   - *Expect* contagion to stay below its per-turn cap, no stance to move past its bound, and a
     decision with recorded dissent.
   - *Expect* the stability monitor to warn only when the inputs explain it.

---

## 8. Pass 5 acceptance scene ("The Tide Council")

This is original content. A harbor town's council of five meets on the night a storm surge threatens
the sealed salt store. Opening it saves the stock but breaks an old oath. The player returns after a
season away and lied to the harbormaster last session. The scene runs through `Runtime::step` from
being definitions alone.

1. **Return.** The harbormaster greets the player with the gap ("since the first frost"). Her player
   model holds the lie, and she raises it once, in private, before the council.
2. **Debate.** Two coalitions form. The cooper simulates three arguments against his belief copy of the
   priest and picks an appeal to shared values, which his trace predicts will move the priest; it does.
   The young netter, in front of her elders, does not admit that she wants the store opened. In
   private, to the player, she does.
3. **Scale.** A background fisherman, summoned to testify, is promoted. His testimony agrees with his
   background summary: the same debts, the same rumor version.
4. **Decision.** The vote passes by one. The dissenting priest's grudge is recorded. The director logs
   the oath as a setup that now owes a payoff.
5. **Save.** The game is saved mid-vote, reloaded, and the scene continues identically.
6. **Words.** Every line is worded by a local 3B model from an intent packet and checked by the gate.
   Any fallback is counted and shown. The player's veil on violence keeps the surge's casualties
   off-screen.

Every decision traces through `ExplainTrace`, the scene replays byte for byte, and the realization
numbers are reported as measured.

---

## 9. Out of scope and risks

- **No model chooses.** Simulation, persuasion and the director propose, and the host decides. A model
  that words a different move than planned is caught by the tag and element checks, not trusted.
- **Model quality is unknown until measured.** P5-3 may show that 1B models cannot word some moves
  reliably. The answer is then templates or a smaller move set for that tier, not a claim.
- **Boundaries are not moderation.** Lines and veils filter what engines choose and what checks allow.
  They are not a general content classifier, and hosts keep their own policies.
- **Psychology is a game model.** Persuasion routes, contagion and narrative themes draw on public
  descriptions of common patterns. They are not clinical or scientific claims.
- **Cost.** The runtime, simulation and detail levels add work per tick. Each stage shows its scale row
  first, and the thousand-being year is the budget check for the pass.
- **Byte-identical defaults.** With every edge off and no new options, existing conformance outputs
  must not change. Save formats are a new format and are announced as one.
