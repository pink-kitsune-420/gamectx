# Optional routines and actual time knowledge (unreleased v0.3.1a)

Current adversarial/local-model findings are in [ADVERSARIAL_EVAL](ADVERSARIAL_EVAL.md).
The implementation measurements below retain their original evaluation scope.

Start with `cargo run --example routines --locked` ([tested source](../examples/routines.rs)).
Enable `routines:true` and list the requested `tools`; defaults enable nothing. Omit the
runtime entirely when unused. No Cargo features, network, tokenizer, model, engine plug-in
or mandatory NPC schema are needed. Share an immutable `Arc<Routine>` across characters.
Resolve shared defaults and partial per-character overrides with `Config::resolve`.

The smallest integration supplies a routine definition, actor/audience identity, calendar,
and `Frame` of current revision, monotonic tick, resolved conditions and allowed activity
IDs. Schedule `Runtime::update` through the existing `schedule::Scheduler`. Mark its job
changed for customers, needs, weather, commitments, blocked paths, hazards or unavailable
resources. Costs are host work estimates; urgent work still respects scheduler budgets.

The engine owns condition meanings, calendar/display conversion, sensors/occlusion,
movement, pathfinding, activity execution, completion, multiplayer authorization and
transactions. `active` describes selected intent. Call `complete` only after the engine
confirms success, with a new event token. A scheduled or completed walk creates no sunlight
observation. Generated dialogue and player statements cannot create direct observations.

## Separate configuration from access

| Layer | Host control |
|---|---|
| Feature policy | `routines`, `history`, `time` booleans, all false by default |
| Tool registration | `tools`, initially empty; only `routine_status`, `activity_propose` |
| Turn exposure | `CharacterTools::enable` or task-relevant `assemble`/session prepare |
| Visible data | Scoped resolved conditions, permitted activities, actual sensor records, exact audience |
| Live permission | Dispatch revision plus callback / bridge `allowed` pairs |
| Execution | Engine validates/accepts proposals and performs actions; model cannot confirm them |

`routine_status({})` returns revision, current/next intended activity, selected actual
observations and time knowledge. It needs live pair
`{"action":"routine_status","target":"ACTOR_ID"}`. `activity_propose` takes an existing
`decision::Choices` pair, `{"action":"activity","target":"ACTIVITY_ID"}`; the menu contains
only currently eligible activities, at most `choices`. Dispatch checks registration,
exposure, live character/runtime revision, current prerequisites and the exact permission
pair. The result is `proposed`, never an execution/confirmation or a timeline mutation.
The engine may accept it, then supply a fresh frame restricted to its chosen activity and
execute through its own action interface. Changes invalidate exposure and pending counts.

Models cannot call host operations by guessing their names. Bind views at the same
revision as the runtime. Rebind and recount after observation, plan, movement or permission
changes. Host `routine_mut()` revokes exposed tools. `reserve_routine_call` preserves the
counted request while reserving one complete provider call, including corrections/retries.
A host using this allowance must reserve before **each** provider request; this library
does not make provider calls or intercept a host's networking.

## Planning rules

Windows are half-open tick intervals `[start,end)`, not hours or days. `repeat:null` means
one occurrence; an integer means repeat every N host ticks. Calendar ID binds the host
coordinate system; the library does not assume Earth days, jobs, sleep or humanoid needs.
Requirements and activities are IDs, never executable text. Unknown/extra fields fail.

The rolling timeline includes unexpired occurrences and future starts through `horizon`.
It fails atomically if they exceed `timeline`; it never silently omits a higher-priority
activity. Rank by descending priority, ascending occurrence start, then lexical ID.
`min_active`, `switch_margin` and `interrupt:locked` prevent needless automatic switching.
Loss of prerequisites/permission interrupts regardless of protection; `emergency` bypasses
protection and the update interval, not requirements, permissions or work limits.

`resume:resume` keeps interrupted work eligible; `resume:cancel` cancels it. `missed:cancel`
ends eligibility at `end`; `missed:defer` extends it through `end+grace`. The grace applies
to all states, including active work; it is a bounded host policy, not inferred progress.
Completion requires the host. Completed/cancelled rows remain until their eligibility
deadline, preventing regeneration of that occurrence; expired rows are retired. The
rolling timeline is not an audit log. Record actual experiences separately when needed.
A backward tick, unchanged/older revision, frequency violation or overflow leaves state
unchanged. Same-tick emergency updates still require a new revision.

## Observations and time sources

Enable `history` independently of routines using `Runtime::new(..., None)` and a calendar.
Each record preserves a monotonic token, observer, location, observation tick, expiry,
`kind` (`direct`, `instrument`, `report`, `inference`), original source ID/speaker/text,
audience, value and host uncertainty (text or null). Source validation reuses the existing
information evidence contract. This dedicated history retains distinct simultaneous
sources rather than superseding one entity/key like `memory::Journal`. It does not
silently promote reports or model hypotheses into observed facts. Hosts can publish
corresponding confirmed `InteractionTools` events or separately recall selected memories;
no existing memory/evidence format changes.

The actor must be the observer; the record audience must equal this history's bound
participant set. Use separate character scopes for private versus shared multiplayer
knowledge. Hosts supply records at the current frame tick; earlier observations survive
as stored history. For a late report, record its reception now, retain the original report
in the source, and have the host normalize its stale range before ingestion.

Enable `time:true` (requires `history:true`) and explicitly choose `time_sources`:

- `instrument`: a real reading, possibly wrong in the game; a broken/unusable reading is retained with `usable:false`.
- `sun`: a host estimate supported by direct sensor data with `value.visible:true`. A visible window/doorway can qualify; clouds, occlusion or indoor/outdoor status require real engine checks. Sunny weather alone cannot qualify.
- `routine`: only the selected activity ID and routine ID, with approximate phase; no numerical clock reading.
- `report`: the speaker's attributed range, which may conflict with other reports.
- `inference`: an explicitly host-supplied estimate, separate from direct experience.

A time record has a host calendar, `low`/`high` reading coordinates and `rate_low`/`rate_high`
aging multipliers. At age A the interval becomes `[low+A*rate_low,high+A*rate_high]`.
Usable sources require `rate_low<rate_high`, so old observations gain uncertainty. These
integer multipliers (0..16) are host assumptions, not calibrated confidence scores.
Choose tick/reading units accordingly. Only a zero-width instrument reading is `exact`;
all sun/report/inference intervals are `range`. Sources retain age and original source.
Disjoint intervals are flagged `conflicting`, without choosing a winner. No usable source
means `unknown`. Disabled or expired sources reveal no time. Observation ticks/ages are
provenance; their simulation coordinate is separate from the reading's calendar coordinate.

Reading coordinates are unwrapped integers; the host converts rollover (for example a
37-tick cycle) into display labels or normalizes reported wraparound ranges. Values beyond
the library's u32 coordinate bound become unusable/unknown. No world clock, weather, player
quote or routine is automatically converted into an actual observation.

## Bounds and persistence

All configuration fields and limits are listed in [FORMAT](FORMAT.md#routines-observations-and-time-knowledge).
Defaults are a starting policy, not a model capability classification. Retention evicts
oldest records when full and removes expired ones; replay tokens survive eviction/clear.
Save with `to_value`, restore into the same configured identity/calendar/definition using
`restore(state,min_revision,min_observation,min_completion,min_tick)`. Validation includes
states, occurrence coordinates, counts, timestamps, identity, policy, definition revision,
audience and durable high watermarks; failures are atomic. Host storage must authenticate
and persist these watermarks across restarts. A JSON snapshot cannot authenticate itself.
Never expose snapshots or restore/configure operations to models. Replacing/configuring a
runtime creates fresh host state; use update/restore for ongoing sessions, and retain
watermarks when intentionally replacing a scope. `clear` keeps replay metadata.

## Small and expanded requests

For one bounded task, put current/next activity and required game facts in protected
`TurnPlan::facts`; offer a few relevant observations as `MemoryChoice` entries. Keep
`AttentionLimits::focused()` initially. Raise attention and `context_observations` only
explicitly after testing the deployment. More context capacity alone never registers
or exposes more tools. Use `CharacterTools::assemble` or the persistent count handshake
to count the entire provider-rendered request, including tools, template, history, tool
results, retries, output reservation and safety margin. Rebuild/recount after every result
or correction. Two thousand tokens is a starting budget, not a ceiling or a parameter-size
rule. Required facts fail overflow instead of disappearing.

## Dungeon and bindings

`cargo run --example dungeon -- --routines-demo` runs a deterministic optional **surface
annex** scenario; it does not add sunlight to the normal underground rooms. Edit
[`routine.json`](../examples/dungeon/routine.json) and
[`routine-config.json`](../examples/dungeon/routine-config.json), or pass `--routine-file`
and `--routine-config`. It shows customer interruption/resumption, postponed work, blocked
travel, a real host-confirmed sun view through a window, a later blocked indoor view,
remembered widening uncertainty and eventual unknown time. No model or network is used.

Python/Node `Character` helpers add routine configure/update/observation/complete/reserve/
save/restore methods. C#/C use the existing persistent session request ABI. Each host
operation below includes character `id`; the original immutable project/native ABI and
all old envelopes are unchanged:

| Operation | Additional fields |
|---|---|
| `character_routine_configure` | `defaults,overrides,calendar,definition` (null for history-only) |
| `character_routine_update` | `frame:{revision,now,conditions,allowed,emergency}` |
| `character_routine_observation` | `observation` |
| `character_routine_complete` | `activity,start,revision,token` |
| `character_routine_reserve` | `revision` |
| `character_routine_save` | none |
| `character_routine_restore` | `state,min_revision,min_observation,min_completion,min_tick` |

## Evidence and limits

The independent Python prototype and Rust agree byte for byte on **69** offline workflows
(**63** accepted, **6** rejected); **21** additive conformance cases preserve all **797**
previous outputs. New Rust tests cover lifecycle, protections, bounded history, private
scopes, guessed/unexposed tools, live permission, stale calls, restore watermarks, rollover
and 128 NPCs sharing a 16-update-per-tick scheduler budget. Mutation tests add **20,000**
attempts against tool text, definitions, observations and saved state. Persistent transports
share **43** fixtures; Python CLI/native make **86** fixture checks, Node **43**, C# **43**.

For the initial routine delivery, these measured deterministic host behavior and byte contracts. Model correctness,
complete-turn latency, provider tokens and RAM had **not** been measured. No model was
called, loaded or downloaded. Model/API spend **$0**. Historical dialogue evaluations and
failed speech pilots remain unchanged; free-form generation can still invent facts.

Validation completed: root/native formatting and strict Clippy, root/native tests and
rustdoc, Rust 1.82, 75 offline CI commands, 17 binding CI commands, all previous C#
fixtures, package verification and the credential/local-reference sweep. No push, tag
or publication was performed.

## Optional task selection and responsive wakeups

`cargo run --example routine-context --locked` runs the
[tested host example](../examples/routine-context.rs). `observation::Selection` filters
actual experiences by kind, location and time-source type, and pins required native
tokens. Empty filters mean all values; there is no semantic guessing or unrelated
fallback. Missing, expired or filtered-out required tokens fail selection. The host
must explicitly expand `context_observations` before requesting more records.
`Runtime::context_for` returns the same shape as `context`, deriving its time view only
from selected observations and an explicitly enabled approximate routine phase. The
ordinary default context and existing tool outputs retain their byte contracts.

Use `affected(changed_conditions,changed_activities)` to avoid waking routines for
unrelated planning events. It checks authored requirements/activity IDs, not sensor or
privacy policy. Always handle perception, permission and scope changes independently.
`next_wake()` includes eligible window starts/deadlines, entries into the rolling horizon,
interruption protection ending when a higher-priority activity waits, and observation
expiry. It returns the earliest useful timer subject to `interval`, or none. The engine
marks the existing scheduler job changed at that tick; external emergencies still arrive
as host events. Scheduler overload/intervals may delay work, so validate current engine
state again before dispatch. Refresh the frame to the current tick before conversation.

Persistent host operations (not model functions) add:

| Operation | Additional fields beyond `id` |
|---|---|
| `character_routine_context` | `selection,revision,max_bytes` (64..32,768) |
| `character_routine_wake` | none; returns `revision,at` (nullable) |
| `character_routine_affected` | `conditions,activities`; returns `affected` |

Python helpers use `routine_context`, `routine_wake`, `routine_affected`; Node uses
camelCase equivalents. These reads do not grant model tools or execute actions. Recount
any request after changing its selected context, even when the runtime revision is equal.

The independent follow-up prototype adds **54** workflows (**48** accepted, **6** rejected)
and **9** additive conformance cases; all **818** earlier outputs stay unchanged. The
expanded persistent suite has **52** fixtures (the original 43 plus nine): Python
CLI/native **104** checks, Node **52**, C# **52**. Selection adds **10,000** mutation
iterations. In one deliberately noisy fixture, newest-three observations take **2,102
JSON bytes** and omit required token 1; explicit task selection retains it in **371 bytes**.
This is host-selected observation JSON, not lossless compression, full-request tokens
or a model-accuracy result.

Runtime updates now preflight history time before the existing atomic timeline update,
avoiding a full history clone. Seven alternating release pairs of **1,000** synthetic
updates with **128** retained records measured median **328,934,760 ns** with a whole-runtime
transaction clone versus **6,986,127 ns** with preflight and a bounded timeline clone.
Both arms assert identical final state. Setup/initial cloning is outside the timer;
frame construction/validation is inside both. Initial serialized state is **228,575 bytes**.
The paired process peak RSS was **4,488 KiB** for both arms together, excluding any model.
These are one single-threaded host microbenchmark, not NPC frame deadlines, per-character
RAM, controlled system-memory comparisons or complete model-turn latency. Compiler and
raw pairs: [routine-performance.json](../eval/routine-performance.json);
[reproducible benchmark](../eval/routine_bench.rs). Model/API spend remains **$0**.

Follow-up validation also passes root/native formatting, strict Clippy, tests/rustdoc,
Rust 1.82, 76 offline CI commands, 19 binding CI commands, original/expanded C#
fixtures, package verification and the reference/credential sweep. Fifteen routine
unit tests include nonhumanoid overlapping commitments on a 13-tick calendar. Total
core cases: 827. At that checkpoint, live model correctness and complete-turn metrics were unmeasured.

## Typed time answers and stricter probe guards

Call `History::assess_time(now, phase)` or `Runtime::time_assessment()` when the host
needs to categorize a time answer without model reasoning. `TimeAssessment::kind()`
returns `Unknown`, `Approximate`, `ExactReading`, `Reported` or `Conflicting`;
`knowledge()` preserves the underlying source payload. Render/localize this in the host.
An exact reading is what an instrument displays, not proof of exact simulation time.
Reports remain reports even when their text asks the model to call them exact.

This is an explicit host helper; no model tool or default exposure is added. Persistent
hosts use `character_routine_assess_time` (`id,revision,max_bytes`), Python
`routine_assess_time` or Node `routineAssessTime`. Result fields are `kind,knowledge`.
The [tested context example](../examples/routine-context.rs) checks typed assessment.

Conflict flags now inspect every usable authorized source in the supplied view even
when output sources are capped. A flag can therefore warn about an omitted contradiction;
select/pin the relevant original sources when the task needs its explanation. Zero numeric
source allowance still withholds numeric evidence; task filters still restrict the view.
Restore rejects non-planned lifecycle timestamps before occurrence start and altered
host frames under the same revision. Host authorization and authenticated storage remain
necessary. [Probe results, limits and optional count reuse](ADVERSARIAL_EVAL.md).

`Runtime::activity_menu` is a host-only current eligible menu for optional
[life initiative](LIFE.md#purposeful-initiative). `consider_routine`/`dispatch_routine`
recheck it alongside goals, accepted notebook promises, opinions and actual source tokens.
They use the existing scheduler and make no model calls unless the host explicitly reserves
an allowed escalation. Plans, proposed activities and off-screen stories remain intent;
only fresh engine outcome receipts become experiences.
