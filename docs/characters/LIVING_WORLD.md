# Living world and one-file cast packs

This opt-in layer runs entirely in zero-dependency Rust. Pack validation, routine updates,
rumor propagation, group influence, badges, atmosphere and speech tickets need **zero model
calls**. Optional foreground model work stays in the host, under existing floor/call/spending
budgets. No planner, critic, reflection model, provider client, background thread or executor
is required. Existing Life, party, native ABI and conformance contracts stay unchanged.

Run `cargo run --example living-world`. The tested example loads
[chime-town.json](../../examples/fixtures/cast/chime-town.json), an editable original cast of four NPCs
and two players, then updates a guard's real routine/initiative, shares a report across two
confirmed windows, derives a habit badge and atmosphere, and compiles a turn using the new
effective attitudes. Fixed JSON simulates a worker completion; the example makes no model
request and does not confirm speech hearing or execute its proposed activity.

## One approved file

`cast::CastPack::{parse,read,from_value}` validate one versioned file with a 1 MiB cap.
There are no includes or external lookups. `LivingWorld::new(Arc<CastPack>, scope)` is the
host's explicit acceptance operation for a save/branch scope. Definitions, lore and routines
are shared immutably; actor state stays separate.

The file contains a shared `Setting`, typed `Locations`, player IDs, groups, character
loadouts, full original Life definitions, optional routines, sidecar attitude gates, offered
affordances, authored fallback lines, lore, feature switches and propagation/ripple/habit
rules. The loader checks:

- Unique roster IDs and known location/group IDs; membership and relationship subjects.
- Setting/calendar matches and all referenced shared history IDs against the single
  approved history. It does not infer consistency of arbitrary English biographies.
- Authored audiences and lore subject/knowledge IDs against the cast scope.
- Blueprint dependencies, routine presence/calendar/location references, gate item IDs,
  and missing/ambiguous attitude kinds used by a gate.
- Group ripple references/fractions and valid habit counter names/thresholds.
- Single-line authored fallbacks with terminal punctuation, at most 240 characters.

Feature booleans `rumors,ripples,habits,atmosphere` are explicit. Set all false for a pack
without these optional effects; ripples require rumors. Accepting a pack never enables a
model tool or grants engine permissions. Ambient actors retain zero-call routine/initiative
configuration. An ambient actor can explicitly mix in the Life module to retain reports
without enabling a model; otherwise it does not participate in subjective rumor memory.

## Attributed rumors and group influence

`LivingWorld::observe` accepts a genuine host direct/outcome event, a unique global root ID,
significance, an explicit recipient allowlist and an optional approved appraisal rule.
The rule must concern the event subject. Source admission plus appraisal is atomic on
failure. A report cannot be admitted as a new direct rumor origin. Hosts should assign one
root ID to one originating event and authenticate receipts rather than derive origins from
generated speech.

`tick` receives host-confirmed scene or routine co-location windows. Merely sharing a
scheduled destination does not prove a meeting. Only active windows at the current tick
are eligible. The learner must trust the immediate sender at the configured threshold;
missing/ambiguous trust fails closed. Significance, sharing permission and a bounded hop
limit must also pass. Each actor learns a root at most once. A snapshot of knowledge at
tick entry permits at most one new hop per strictly increasing tick; reciprocal contacts
cannot spin a report around indefinitely or multiply group effects.

The admitted recipient event is always `kind:"report"`. Its source names the immediate
relay and root ID. The envelope/ledger separately retain the complete original source
speaker/quote, original observer, path and hop count. Both immediate and original attribution
remain available; hearsay never becomes a direct observation or rewrites shared history.
Legacy `Life::appraise` still rejects these reports.

With ripples enabled, a fellow member who actually learns the root can receive a fractional
change of the origin member's **actual effective appraisal delta**, rounded toward zero
and capped at five points. Multiple shared groups still produce one effect. The target is
an existing attitude toward the event's player subject, with a single unambiguous kind.
Missing attitudes, no shared group, zero fractions or zero applied change produce no ripple.
Reasons and report token dependencies are recorded. Levels stay within -100..100 and a
reversal passes through neutral.

The world maintains chronological effective levels alongside unmodified base Life state.
Later direct appraisals use the effective level for the social view, including saturation
and neutral crossing; old group effects are not reapplied as permanent buffs. The base
Life checkpoint remains valid under its existing format. `inspect_actor` distinguishes
base and effective attitudes; root, rumor and influence ledgers are host-only diagnostics.
The world compiler and `call_life` apply effective attitudes to existing party gates.
Do not dispatch a world turn's recall tool through a base-Life dispatcher, which lacks this
additional social view.

Propagation itself consumes **zero provider tokens**. Including selected reports, badges
or attribution in a later prompt uses ordinary context bytes/tokens and must be counted.
Do not copy gated secrets into otherwise ungated events; this is source-bound selection,
not semantic redaction of duplicate prose.

## Bounded frame work

`tick` updates supplied existing routine runtimes immediately, then scans rumor pairs.
`consider`/`dispatch_initiative` reuse the existing deterministic initiative runtime against
effective attitudes in the same frame; proposals still require real host execution and
outcome acknowledgements. The example performs these steps before touching a speech worker.

`propagation.checks` limits scanned window/root/sender/receiver pairs, with a rotating cursor
for recovery under a small budget. `per_tick` separately limits accepted reports. Newly
learned rumors cannot relay again within that tick, even when there is unused work budget.
There are no sleeps, network calls, worker joins or unbounded polling loops.

| Resource | Bound |
|---|---:|
| Pack / NPCs / total participants | 1 MiB / 12 / 16 |
| Places / groups / habit rules | 32 / 16 / 16 |
| Co-location windows / routine updates per tick | 16 / 12 |
| Pair checks / accepted reports per tick | 1,024 / 16 |
| Root experiences / learned holders | 64 / at most roots × NPCs |
| Group influence records | 128 |
| Retained Life events per actor | Existing 128-event/size bounds |
| Pending or retained speech entries | 32, released by retire |

Capacity/clock failures are explicit. A learner with full Life memory or influence capacity
is reported as backpressure and is not marked as having learned the root. This first layer
does not automatically archive, evict, expire or consolidate rumor roots/influences. Long
sessions need a host retention/compaction policy; the limits are not an unlimited town
simulation claim. Store/replay the approved host operation journal and social/provenance
ledgers together with base actor state. Scope erasure must remove derived reports, counters,
levels and ledgers together. Restoring base Life alone does not restore the living world.

## Habits and atmosphere

`update_counters` accepts bounded cumulative host-confirmed fulfilled/broken promise and
interrupted-delivery counts, a player, time, monotonic receipt token and source. Exact
retry is idempotent; rollback, changed replay and foreign players reject. These are host
attestations, not claims a model can award itself. Badge rules compare a counter to an
authored threshold. Conflicting badges may coexist; this is not a personality diagnosis.

Atmosphere is an explicit revisioned host tag for one approved place: calm, busy, tense or
emergency. Its urgency is the corresponding ordinal 0..3, a design parameter, not inferred
danger or execution permission. Context includes only the requested players/tags and current
place. There is no all-player or all-location dump. Focused compilation caps each selected
extra list at two; expanded compilation permits four. The stable system/character/scene
prefix stays ahead of this changing context. Full provider request counting remains required.

## Immediate fallback and optional async upgrade

`SpeechQueue::issue` returns an instance-bound opaque handle and an immediate authored line.
No worker or model is required. A host may hand work to its existing asynchronous provider
integration, while retaining the opaque capability on the game side. The library never
waits for that work and never calls a provider itself.

`complete` consumes one attempt. It checks current revision/permission context and deadline
through the existing `RequestGate`, parses bounded strict JSON with exactly `speech`, checks
terminal punctuation/length, then calls the host's fact/policy/permission validator. Shape
and punctuation alone do not prove semantic grounding. Invalid, expired, stale or rejected
results retain the fallback and bounded raw response for inspection; there are no hidden
repairs or retries. Host validation callbacks must themselves be bounded and non-blocking.

Call `freeze` **before any caption or audio starts delivery**. It seals either the fallback
or an accepted upgrade; later completions cannot replace already visible/heard text.
Freezing is not proof of playback or hearing. Use existing party/coordinator delivery
acknowledgements for actual confirmed portions. Cancel/retire release pending/retained
entries; consumed IDs never replay. A stale delivery context rejects and needs a fresh
ticket/fallback. TTL governs completion acceptance, not how long a frozen line must play.

`speech_context` binds material world revision and a host-supplied current scene/permission
revision. Clock-only routine frames leave it unchanged, so ordinary ticking does not starve
pending speech. Material updates conservatively invalidate pending world turns. The revision
queue deliberately rejects individual FactRefs; use the lower-level RequestGate for that
contract. Recreate the queue on world/save recovery, retain authenticated branch-specific
scope/revision metadata, and never reuse old handles across a restored game state.

When already-delivered fallback is the immediate UI, it remains the delivered line; a later
model result is not a retroactive edit. Hosts control the presentation/delivery window and
all rendering, simulation, model dispatch, call reservations, permissions and storage.

## Evidence and reproduction

An independent Python prototype precedes Rust. **54 workflows** match byte-for-byte,
including malformed packs, trust/hop/sharing limits, bounded scanning, original speaker
provenance, chronological ripple saturation, relevant badges, scene urgency, real routine/
initiative steps, gate changes and fallback/upgrade/replay/staleness cases. The 54 conformance
cases are additive; all prior 972 cases and all six frozen life-model artifacts remain
unchanged. Rust tests also cover foreign opaque handles, queue capacity, unchanged base
checkpoints and clock-only frames. Mutation checks exercise 9,000 altered pack/world/ticket
inputs. Python CLI/native, Node and C# checks use the same fixtures.

Run `python3 -I eval/character/living_cases.py --cap 0 --prototype-only --tokens-only` first; it reports
provider tokens as unknown because no provider is called. After building the CLI,
`--check eval/character/living-world-results.json` audits structural results and frozen artifact
hashes; `--check-bindings` checks the existing bridges. These make **0 model calls / $0 API**.

The retained [host-work timing](../../eval/character/living-world-timing.json) runs 1,000 unpaced release
frames with four real routines, initiative, 64 roots, 128 delivered reports, at most 128
scanned pairs per frame, and a complete fallback ticket lifecycle each frame. Two hundred
fixed JSON strings simulate completions; no model runs. Source hashes and hardware identity
are recorded. This measures the library workload, excluding rendering, physics, real worker
latency and authoritative game callbacks; OS/desktop scheduling is uncontrolled. It is not
a whole-game 60 FPS or realtime guarantee.

Build with `cargo build --release --lib --locked`, then compile `eval/character/living_measure.rs`
with the release gamectx rlib and run the resulting binary. `living_measure_check.py --cap 0
--check eval/character/living-world-timing.json` audits retained identity/counts/shape without rerunning
or enforcing unstable latency thresholds in CI. The standard `.github/workflows/ci.yml`
also runs the new parity, example and binding checks.

The retained trace measured **272 µs median**, **348 µs p95**, **2248 µs maximum** for the measured host frame; **1000/1,000** frames were below 16,667 µs. These numbers apply only to the workload and method above.
