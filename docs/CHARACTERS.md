# Character tools and perception (unreleased v0.3.1a)

`gamectx::character` connects existing scoped notes, inventory and conversation tools
with new read-only location/world snapshots. It is optional, provider independent,
zero dependency and compatible with the earlier formats. Run
`cargo run --example character_tools --locked` for the tested Rust example.

## What an NPC can know

The host binds an actor, audience, current location, revision, visible inventory owners,
original sources and permitted world keys. None of those are model arguments. Supply
already-redacted scene details; the location predicate gates whole places, not fields.
An NPC can only read the bound scene, never choose another room through `look_around`.
Rebind after movement or a visibility change. Pass the live revision at each call;
old revisions fail. Calling a tool does not simulate perception, move anyone or advance time.
The engine supplies its current sensor results before binding.

`look_around({detail})` and `location_lookup({detail})` use the same read-only snapshot.
Choose one name for a turn. `brief` returns ID, label and summary; `normal` adds description;
`full` adds actor-visible details such as objects, occupants, exits, sounds and observations.
The requested result must fit whole, or fail so the host can explicitly request less detail.

`world_status({keys})` returns only host-authorized keys. It can carry time, weather,
lighting, season, hazards, quest/event flags or other JSON values. Unknown is `null`;
missing and forbidden keys are both absent. Being outdoors does not create weather data.
The host must check exposure, visibility, sensors and knowledge separately from global truth.

## Time access is a character policy

Do not give every NPC the simulation clock. These are suggested host policies, not a
universal calendar or mandatory game mechanic:

| Host setting | NPC knowledge | Suggested provenance |
|---|---|---|
| none | Unknown | No clock and no visible sky |
| clock/watch | An actual reading, possibly inaccurate in the game | Instrument ID and observation tick |
| schedule | Approximate routine phase | Routine ID; precision `approximate` |
| sun | Approximate time while the sun is visible | Sun observation and uncertainty |
| report | What someone said at a past tick | Speaker, original quotation and report tick |

Store the actor's observation under the authorized `time` value, with source, precision
and age. The value can be `{"label":"morning","basis":"schedule","precision":"approximate","observed_at":42}`.
A watch can be broken or wrong. Clouds, underground rooms and unfamiliar calendars can
prevent sun estimates. A routine alone does not grant exact clock knowledge.

If exact global time is internal, omit `time` from `CharacterView::with_world`'s keys or
bind a separate actor-specific world snapshot containing only their observation. A time
report fits existing `info_remember` with a host-supplied original source; its `reported`
status establishes attribution, not truth or freshness. The host must age, replace or
forget stale observations. This library does not automatically reconcile contradictory clocks.

The dungeon demonstrates `none` for the merchant and `schedule` for the keeper. Authored
lore can set subject `npc:merchant`/`npc:keeper`, topic `timekeeping`, v1 text `clock`,
`watch`, `schedule` or `none`; v2 uses `value:{"mode":"schedule"}`. Clock/watch modes are
explicit example entitlements to a working instrument. An actual game should additionally
check possession and condition before granting them. Fourteen offline workflows cover
these policies, denied access, reported time and forgetting. Sun estimation is a host
integration pattern; the underground NPCs have no visible-sun sensor.

## Proposed next layer: routines and actual observation history

An optional host-owned routine file could seed a rolling timeline of work, travel, rest
and observation opportunities. Activities could have time windows, priorities, requirements
and interruption/resumption rules; weather, needs, customers and commitments could shift
the plan locally. Keep the plan separate from actual activity: a scheduled morning walk is
not evidence that the NPC saw the sun. Record completed activity and observed conditions
through the existing witnessed-event APIs, retaining place, tick, source and uncertainty.

A remembered sun observation before entering a building can support a later rough time
estimate. Its uncertainty should grow with elapsed time, interrupted routines and unreliable
clocks. Being indoors alone does not forbid an observation through a usable window;
visibility and occlusion remain engine checks. Do not derive time from sunny weather alone.

This routine-file layer is a proposal, not an implemented scheduler or time-estimation
algorithm. Current APIs already accept host-selected observations/events and rough time
values. A future helper should feed only current/next activity and a few relevant actual
observations to small models, leaving the full routine on the host.

## Select context for the task and measured deployment

`TurnPlan` supplies one current task, player input, required facts, task-tagged tool
candidates, prioritized memories and whole recent exchanges. `AttentionLimits::focused()`
allows one tool, two memories and one exchange. It is an explicit starting policy for
small models, not a measured universal optimum. `AttentionLimits::new` accepts up to
32 tools, 64 memories and 16 exchanges. Candidate input bounds are larger so the host
can offer relevant alternatives. This is per-turn context, not the entire memory store.

More available context can retain more relevant optional information. Spare tokens alone
do not increase attention limits. Required facts, required tools/memories and the latest
selected whole exchange are protected; overflow fails instead of silently discarding them.
Unrelated optional tools are excluded; an unrelated required tool is an error.

`CharacterTools::assemble` uses the existing `RequestBudget` and a host renderer/counter.
Count the complete provider request, including native tool formatting, chat templates,
history, corrections and reserved output/safety. Rebuild and recount after each tool
result or retry; a fitting first request does not guarantee the next round fits.
Both exact and estimated counts are
explicit host assertions. Tool permissions are enabled only after successful assembly;
a failed new assembly disables the previous selection. No model is downloaded or called.

There is no parameter-size ceiling or provider allowlist: use any model ID, including
hosted and larger models, with measured capabilities and explicit budgets. Size hints in
the older deployment API remain advisory. Two thousand tokens is a starting configuration.
The existing work bounds (1,048,576 context tokens and 1,572,864 request JSON bytes) are
library limits, not statements about model architecture. Increasing context is not a
substitute for testing correctness, latency and RAM on the target hardware.

In the synthetic one-byte counter fixture, focused selection keeps one tool/two memories/
one exchange in **1,025 bytes**, even when context capacity rises to 32,768. Explicit
expanded limits retain three tools/12 memories/four exchanges in **3,733 bytes**. Under
pressure that expanded plan keeps two tools/two memories/four exchanges in **1,632 bytes**.
These are JSON bytes under a fake counter, **not model tokens or accuracy results**.
Forty-nine independent Python/Rust workflows check the selection contract.

## Persistent bridges and language adapters

Rust hosts use `CharacterTools` directly. Python and Node helpers use persistent
`kit::Session` instances through `gamectx characters`, or `gamectx serve PROJECT`.
C#/C hosts use the additive native session ABI. Existing immutable `Project` methods and
C ABI symbols remain available. Sessions retain at most 64 characters in process; hosts
own durable storage and scheduling. Reuse immutable snapshots through `Arc` in Rust.

Python, using the repository's standard-library-only adapter:

```python
import json
from pathlib import Path
from gamectx_client import Client, Character

with Client(binary="target/debug/gamectx") as client:
    npc = Character(client, "merchant:a", "conversation:a", "npc:merchant",
                    ["npc:merchant", "player:a"])
    npc.bind(json.loads(Path("examples/characters/view.json").read_text()))
    schemas = npc.enable(["look_around"])
    observed = npc.call("look_around", {"detail": "brief"}, revision=7)
    npc.close()
```

`Character` helpers also expose `prepare`, `observe`, `event`, `progress`, `references`,
`offers`, `respond`, `clear` and `close`. Node exports `Character.create(...)` with the
same operations. C# keeps its existing JSON-library-neutral `Request` method; its
parameterless constructor creates an empty persistent session. Python/C# automatically
fall back to the old project ABI with older native libraries; new character operations
require a new library. Do not expose host bridge operations directly as model tools.

Version-1 host operations:

| Operation | Arguments (all include character `id`) |
|---|---|
| character_create | `scope,actor,participants` |
| character_bind | `view:{revision,world,world_keys,locations,current,inventory,owners,sources}` |
| character_enable | `tools:[function names]` |
| character_call | `name,text,revision,max_bytes,allowed` |
| character_prepare | `plan_id,identity,budget,turn` |
| character_observe | `observation:{sequence,candidate,count:{input,method}}` |
| character_event / progress / references / offers | `event` / `progress` / `context` / `offers` |
| character_respond | `recipient,revision,confirm,terms,allowed` |
| character_clear / drop | No additional fields |

`text` is raw model JSON arguments. The host supplies live revision, result cap and exact
currently available action pairs in `allowed`; models cannot grant those permissions.
Clear removes the character's notes, bindings and enabled tools while preserving replay
metadata. Drop removes the session slot. Use separate scopes for separate player audiences.

For full-request planning outside Rust, `character_prepare` returns a neutral `frame`
plus a candidate/sequence. Render the frame for the chosen provider and count the entire
rendered prompt. Return the exact candidate and sequence with the count through
`character_observe`. Repeat while status is `count`; send only on `ready`. `overflow`
means required context cannot fit. Rebinding invalidates pending counts and permissions.
Never reuse a count after changing provider formatting or deployment identity.

These APIs constrain access and structure. They do not prove that unrestricted generated
speech faithfully uses a tool result. Existing source-selected answers remain the safer
default; see [the retained diagnostics](CONVERSATION_PATCH_EVAL.md).
