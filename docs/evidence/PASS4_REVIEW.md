# Pass 4 human-review report

Branch `v0.4.0a`. Pass 4 ("anyone, anywhere, over time") is designed, implemented, deep-tested and
accepted. The design is [INTELLIGENT_BEING_4.md](../research/INTELLIGENT_BEING_4.md), and the
evidence for each module is in [LIFE.md](../characters/LIFE.md). Every number below was measured.
No model was called, and API spend was $0.

## Tickets

| Ticket | Work | Commit |
| --- | --- | --- |
| T-041 | Genre profiles and lexicon packs (`knowledge::genre`) | 0ba2137 |
| T-042 | Mind architectures (`character::minds`) | e58cb28 |
| T-043 | Defense mechanisms and strain (`character::defenses`) | cc9df49 |
| T-044 | Idiom and rumor mutation (`knowledge::culture`) | 2ca36a7 |
| T-045 | Life projects (`character::projects`) | 37a8e86 |
| T-046 | Chemistry, banter and pacing (`character::chemistry`) | eb5fd4a |
| T-047 | Pass 4 deep tests (`eval/character/pass4_deep_test.rs`) | 370150e |
| T-048 | Acceptance scene, "The Missing Ration" (`tests/acceptance_missing_ration.rs`) | d8d34c4 |

T-009 (Lantern Vault chat agreement) was not touched. See the [handoff](#t-009-handoff).

The last full gate covered fmt, clippy with `-D warnings`, every test, the docs build and the 295 CI
steps. It passed 507 tests with 0 failures.

## Module evidence

Each module has three kinds of evidence:
- an independent Python reference;
- conformance cases (malformed ones included);
- 2,000 randomized parity workflows at seeds 7 and 1, with 0 mismatches.

Each also has a robustness section in `tests/robustness.rs` and a row in the scale table. The table
gives the median and p95 of 1,000 queries against 10,000 beings (or genres or projects) on an AMD
Ryzen 7 3800X.

| Module | Cases (malformed) | Robustness | Query | Median | p95 | Bytes per being |
| --- | --- | --- | --- | --- | --- | --- |
| genre | 25 (8) | §42 | `render` | 3.7 µs | 4.4 µs | 1,245 |
| minds | 26 (7) | §43 | `admit` | 2.1 µs | 2.4 µs | 158 |
| defenses | 26 (7) | §44 | `threat` | 3.2 µs | 3.8 µs | 444 |
| culture | 27 (7) | §45 | `tell` | 4.8 µs | 6.2 µs | 1,273 |
| projects | 25 (7) | §46 | `weight` | 1.7 µs | 2.2 µs | 895 |
| chemistry | 27 (6) | §47 | `pair` | 1.8 µs | 2.2 µs | 201 |

Bugs that parity and scale caught and closed during pass 4:
- **minds:** redefining a mind could leave a being in a shared mind without a pool.
- **culture:** a retelling's cost grew with the teller's held topics, reaching 98 µs at 10,000. The merge pool is now bounded at 16.
- **gate script:** its exit code hid a failing test once. Every gate since then has been checked by counting FAIL lines.

## Deep tests (T-047)

[Details](../characters/LIFE.md#pass-4-deep-tests). The report `eval/character/pass4-deep.json` has
38 checks and is replayed byte for byte in CI.

- **Cross-genre theft (12 checks).**
  - The same theft was authored in village, orbital station, noir city and academy words.
  - All four normalize to the same engine words.
  - All four make identical decisions:
    - acts: greeting, accusation, threat;
    - the crate is noticed before the hum;
    - the sanction is step 3;
    - the taker rationalizes;
    - the friend makes a playful callback at chemistry 90.
  - They render as four different vocabularies.
  - A village word in a station scene is refused.
- **Breaking point (9 checks).**
  - Defenses go humour → humour → rationalization → rationalization → denial. The confession crisis fires once, at 806,000 strain.
  - Inside the cooldown, strain reaches the 1,000,000 ceiling without a second crisis.
  - With support, strain falls 1,000,000 → 596,000 → 272,000 → 68,000 → 0, and humour returns.
- **100 agents over 180 days (17 checks).** Seed 7; 54,000 idiom exposures; 11,504 retellings.
  - Host limits held: a 101st member and a 101st project were refused.
  - At least 85% of usable idioms are home-community idioms.
  - The fad rose to 21 users and fell to 0.
  - The mean truth distance of hearsay was 0.11, 1.00, 1.41 and 3.58 operators at 1, 2–3, 4–7 and 8+ hops. A correction cut it from 3.06 to 0.29.
  - Projects: 49 completed, 45 adapted and 6 abandoned.
  - The stability monitor warned only on gossip days and recovered afterwards.
  - The run took 445 ms of wall time, with about 10 KB of live state per being and a 2.1 MB peak heap (`pass4-deep-timing.json`).
- **Engine gap found.** In `culture`, gossip can overwrite a witness's own version. The test's host gives that hearsay zero trust; the engine does not protect witnesses itself yet.

## Acceptance transcript (T-048)

Both genres make the same 11 decisions with the same 7 explanations, and each replays byte for byte.
The orbital station:

> The steward intelligence logs a missing ration crate; the junior mechanic took it for a sick friend; the certification is two milestones from done.
> The junior mechanic: "it was surplus from the last resupply." (explains_too_much)
> The junior mechanic snaps at the cadet (snaps).
> The steward intelligence tells the crew only the count; the camera record goes to the deck supervisor.
> By the next shift, the fourth teller has heard that the junior mechanic took three ration crates late at night.
> The chief's "crate rat" is on the lips of two of the crew, not the third.
> With the friend beside them, the junior mechanic confesses: the ration crate was for a sick friend.
> The deck supervisor gives a verbal warning: step 1, where it would have been step 2 without the confession.
> The certification review becomes a retake; the junior mechanic keeps going.
> The friend: "Still better than their first shift on the reactor ring." (The junior mechanic laughs.)
> Scene ends.

The village runs the same lines with a granary keeper, an apprentice, a reeve, an errand boy, grain
sacks, farmhands, a tally board, a journeyman test and "sack mouse".

Two of the seven explanations:

> mechanic did displacement because mechanic snapped at cadet because crew_saw_it threatened belonging (reaction motive protect_belonging, score 20). Triggered by: the supervisor read out the log. Outcome: pending.
> steward did withhold_camera_record because the record goes to the supervisor only (promise motive camera_hold, score 80). Triggered by: the others asked what the record showed. Outcome: pending.

## Genre readiness

**Ready:**
- Closed engine word lists can be authored in any genre's words and mapped both ways. The lists are light, access, privacy, norm severity, contract kinds and address terms.
- Unknown words are refused.
- Slang packs become pragmatics lexicons.
- Decisions were identical across the four built-in genres in the deep test and across two genres in the acceptance scene.

**Still the host's job:**
- Engines do not read a genre directly; the host calls `normalize` before and `render` after.
- Free text (excuses, joke moments, idiom phrases, nouns) is authored per genre.
- `minds admit` runs in front of other engines only when the host calls it.
- Defense threats, project feeds, chemistry biases and pacing advice are reported or applied by the host.
- Culture is not yet consulted by `chatter` or the epistemic gate.

These follow-ups are listed in [ROADMAP.md](../ROADMAP.md).

**Not covered:**
- No model has worded any pass 4 output yet.
- Quality of real model speech under these decisions is unmeasured.

## T-009 handoff

T-009 is "Measure engine vs Lantern Vault chat agreement per stage": shadow-run the roleplay engine on
Lantern Vault chat test traffic and produce a per-stage agreement table. The gate on it was "pass 4
design, implementation and deep testing finished and verified". That condition is now met, so T-009
is ready to pick up. It is still OPEN and has not been started.

Suggested start, following the repository rules:
1. Write the predeclared agreement expectations first, in `eval/`.
2. Run offline and token-only passes (`--tokens-only`, `--cap 0`).
3. Use the local llama-server router for model-backed runs. Hosted runs need explicit `--cap` limits on the two smaller default models.

Pass 4 adds stages worth including in the table:
- the genre `normalize`/`render` round trip;
- `minds admit` refusals;
- defense mechanism choice;
- culture `tell` versions;
- chemistry banter moves.

## Reproduce

```bash
cargo test --locked --test acceptance_missing_ration
cargo build --release --locked
rustc --edition=2021 -O eval/character/pass4_deep_test.rs -L dependency=target/release/deps \
  --extern gamectx=target/release/libgamectx.rlib -o target/pass4-deep
target/pass4-deep --check eval/character/pass4-deep.json
```
