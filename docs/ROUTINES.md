# Optional routines and actual time knowledge (unreleased v0.3.1a)

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

These measure deterministic host behavior and byte contracts. New model correctness,
complete-turn latency, provider tokens and RAM have **not** been measured. No model was
called, loaded or downloaded. Model/API spend **$0**. Historical dialogue evaluations and
failed speech pilots remain unchanged; free-form generation can still invent facts.

Validation completed: root/native formatting and strict Clippy, root/native tests and
rustdoc, Rust 1.82, 75 offline CI commands, 17 binding CI commands, all previous C#
fixtures, package verification and the credential/local-reference sweep. No push, tag
or publication was performed.
