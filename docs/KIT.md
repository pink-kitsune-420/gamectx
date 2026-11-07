# Integration kit (v0.2.0a)

`gamectx::kit` connects registration, selection, packing, queries and validated tools.
It is optional: existing `Context`/`Turn` and low-level formats keep their behavior.
The core still has zero third-party dependencies and supports Rust 1.82.

Start with [`examples/registered-project.rs`](../examples/registered-project.rs), which
runs as a test. It demonstrates the JSON and Rust builders, a live-checking callback,
shared brain units/ranges, and the existing query tool over a schema-only recipe.
The complete starter lives in [`examples/kit`](../examples/kit/README.txt).

## Register once, capture each turn

`Project::parse`/`from_value` validates a version-1 definition and prepares each recipe's
stable schema/tool sections. Rust hosts can instead use `Registration`, `Dataset`,
`Field`, `Kind`, `Tool`, `Arguments`, `Parameter` and `Recipe`. Builders validate together
at `Registration::build`, including duplicate names and cross-references. Export with
`Project::to_value` to use exactly the same registration with other language adapters.

A dataset declares its ID column, scalar fields, meanings, units, nullability, visibility
and optional maximum capture age. All rows must contain exactly those fields. IDs are
unique nonempty strings, at most 256 Unicode characters; namespace/dotted IDs are retained
exactly. Fields can be string, number, integer or boolean. Definitions and property names
use lowercase ASCII identifiers, starting with a letter, at most 32 bytes.

`Project::snapshot` validates a host snapshot and physically removes host-only table
fields before retaining it. Supply only rows this actor may access; the library does not
infer entity permissions. Snapshot sections are explicitly supplied, trusted host values:
add already-filtered memories, facts or events there. Table field visibility does not
sanitize arbitrary section values or tool descriptions.

The snapshot contains host `now` and table `updated_at`, in consistent host-chosen ticks.
Future captures fail validation. A selected dataset older than `max_age` fails build/query;
null disables that age limit. Age is checked against capture-time `now`, not a live clock.
Create a new snapshot with the current clock each turn. Snapshots are sealed to their
registration; a `Project` clone can reuse one, a separately parsed definition cannot.

## Recipes and inspection

A recipe chooses datasets, tools, extra sections, and an exact UTF-8 JSON byte budget.
For each dataset choose `rows` or `schema`, a view, priority and required status. Views
support the existing `fields`, `where`, `sort_by` and `limit` operations, in existing order.
Host-only columns cannot be projected, filtered or sorted. Recipe projection restricts
query columns too, including on empty tables. Queries run on the same filtered, sorted,
limited view, rather than the full source table; aggregates therefore describe that view.

`Built.packet` is model context. `Built.inspection` is a separate host-only explanation:
source and selected row counts, zero-based original row indices, selected/omitted fields,
capture age, the view, missing optional sections and the existing budget receipt. Do not
send the inspection to the model: it includes host field names and selection diagnostics.

Stable `tools` and `schemas` sections are required and included in the budget. Rows and
extra values follow normal priority dropping; required values fail instead of dropping.
Table schemas still describe empty tables. A dropped row section does not revoke its
schema or query access: the registered recipe view is the query boundary. `schema` mode
omits packed rows entirely. It reports the selected view, not an entire source dataset.

The toolkit includes ordinary function schemas in its byte budget rather than assuming
provider-side tools are free. `Project::tools` also returns them separately for a provider's
tool field; avoid sending them twice. Hosts wanting the existing signature representation
can use `Context::tools` directly. Toolkit schemas use `strict:false`, because optional
arguments are supported. Provider-specific schema conversion remains a host decision.

`gamectx doctor` rejects invalid configurations and warns about disabled age checks or
numeric fields without units. It does not infer whether a supplied unit is physically
correct. `gamectx inspect` returns packet and inspection as JSON. `gamectx init` generates
a new directory containing a project, snapshot, scripted suite, README and runnable
`check.py`; it refuses an existing destination. Run the generated check with Python and
an optional gamectx executable argument. The starter is exercised in the Rust tests.

## Tool callbacks and brain features

`Tool` registers a name, description and bounded argument schema. `Arguments` provides
required/optional fields and always forbids unknown properties; nested objects, bounded
arrays, nullable scalars, enums and numeric limits are supported. `Tool::from_function`
imports an existing function schema, including `query::tool()`. Unsupported keywords
fail registration rather than being ignored. Full limits are in [FORMAT](FORMAT.md#integration-kit).

`Project::validate_call` checks arguments without acting. `Project::dispatcher` associates
callbacks with registered tools. Each dispatch also requires a recipe that allows the
tool. Invalid arguments and unavailable tools never invoke callbacks. Replies consistently
contain `ok` and `result` or `error`. Errors distinguish unavailable tool, invalid arguments,
missing handler, host rejection and invalid/oversized callback output. Argument errors
identify missing/unknown properties or the failing nested property; they do not establish
that a valid action is appropriate.

The callback must check current actor permissions, entity availability, resource costs
and other world constraints. Host error details are withheld from model replies. A result
limit is checked after the callback runs: rejection does not undo world mutations. The
dispatcher does not retry; hosts control idempotency and action execution.

Tool numeric bounds and enum equality compare exact decimal values. Integers are tested
mathematically, so `1.0` and `10e-1` are integers; `9007199254740992.1` is not. Original number
text is preserved. Query arithmetic/comparison and existing view numeric filtering still
use their established f64 semantics.

`Project::feature_schema` takes ordered host ranges/missing/outside policies and obtains
numeric feature names and units from the dataset. It produces the existing versioned
`brain::FeatureSchema`, preserving its limits and normalization. Units are not guessed or
converted; missing units fail. This does not automatically train a brain or expose private
features. Use the existing binding validation and native inference APIs afterwards.

## Other languages and engines

`gamectx serve project.json` keeps registration prepared in one persistent process.
Version-1 JSON-line requests contain `version`, a string `id`, `op` and `args`; replies
contain the matching ID, `ok`, and `result` or `error`. See [FORMAT](FORMAT.md#integration-kit).
Operations are build, query, validate, doctor, tools, features and evaluate. This bridge
is a host integration interface, not a tool to expose wholesale to a model. It never
invokes host callbacks; validate first, then execute in the host with current checks.

[`bindings/python/gamectx_client.py`](../bindings/python/gamectx_client.py) provides
standard-library-only persistent and native clients. Use its context-manager lifecycle.
[`bindings/typescript/gamectx.mjs`](../bindings/typescript/gamectx.mjs) is a Node.js adapter
with TypeScript declarations; it serializes concurrent calls over one persistent process.
JavaScript JSON numbers retain JavaScript's usual binary64 limitations: use string IDs
and encode precision-sensitive quantities as strings in your host format where appropriate.
The C and C# interfaces pass raw JSON text without JavaScript numeric conversion.

For in-process calls, build the separate [`bindings/native`](../bindings/native/Cargo.toml)
crate for the host platform. It depends only on this local library and exports the C ABI
in [`gamectx.h`](../bindings/native/gamectx.h). Create a project handle once, request using
UTF-8 byte buffers, free every reply with `gamectx_string_free`, and free the project after
all calls finish. Rust-owned pointers must never go to the host allocator. See the header
and safety docs for buffer/lifetime contracts. ABI failures return null with a thread-local
error; invalid protocol requests return normal JSON error envelopes. Immutable requests
can run concurrently while the handle stays alive.

[`bindings/csharp/GamectxClient.cs`](../bindings/csharp/GamectxClient.cs) wraps that ABI
with `IDisposable` and serialized requests, without depending on a particular JSON package.
It can be used by native-plugin-capable C# hosts; deploy the platform library and use the
engine's JSON serializer for request/reply envelopes. Unity and Godot engine projects and
mobile/Web exports were not exercised. The native library is a separate build artifact,
not an automatically distributed engine plug-in. No wrapper is published by this change.

## Evaluate your own game

`gamectx evaluate project.json suite.json` is offline. A suite supplies independent cases
(ID, recipe, snapshot, question, expected result) and named recorded runs. A run may override
the recipe and records a guidance profile plus one observation for every case. Observations
supply SQL or null, final answer, actual token counts/latency or null, and tool calls.

The scorer builds the selected recipe, executes SQL on its view, validates tool arguments
without dispatching, and scores final answers and executed queries separately. A correct
answer does not hide an invalid query or tool call. Missing/duplicate/unknown observations
are rejected, preventing accidental incomplete candidate comparisons. Object key order is
ignored; array order and answer/expected numeric spellings are exact. For example, convert
provider answers to your declared expected JSON representation before scoring.

Guidance is recorded configuration, not applied by this offline scorer. Generate candidate
responses with the intended profile in your model harness, then import their observations.
The evaluator cannot verify reported tokens, latency or whether a model saw that guidance.
It makes no API calls or pricing assumptions. One missing usage/latency value makes that
run's corresponding total null. Question text is retained for the host harness, not used
as a second oracle. Compare on new snapshots and questions from the actual game.

## Measurements and limits

The independent Python prototype and Rust agree byte for byte on **144** workflows:
**113** accepted and **31** rejected. The cohort includes **101** generated stock snapshots,
plus three separate scripted evaluation states. **47** new conformance cases include errors;
all **401** previous cases remain unchanged. Tests add **20,000** deterministic text
mutations, **265** Python bridge/native checks, **8** Node checks and **59** C# fixture
replies plus two C# smoke checks. C# and native were exercised on Linux, not inside engines.

In the starter, a host-selected 24-row source is **1,646 bytes**; filtering to food, keeping
the three largest counts and two public fields makes a **97-byte** packed table. The whole
packet, including schema, tools and event, is **646 bytes**. A 550-byte budget drops that
optional table and produces **540 bytes**. This is lossy host selection, not lossless
compression or a model-accuracy measurement.

Seven alternating paired release trials of 1,000 turns: rebuilding registration each
turn took median **146,130,805 ns**; reuse took **78,931,400 ns** (about **1.85×**). Both arms
validate snapshots and build identical packets; fixture parsing before the timer is
excluded. This measures one small synthetic registration workload, not engine frame rates
or model latency. Raw evidence: [workflow checks](../eval/kit-results.json),
[timing trials](../eval/kit-performance.json). New API spend: **$0**.


## Authored lore and factual answers (v0.3.0a)

`lore::Lore` loads a bounded world-info definition once and selects records only after
host-supplied actor/grant checks. Its model packet can be supplied as a recipe section
or protected scene fact; section values still require host authorization. `dialogue::Answers`
provides bounded source-ID selection and host-rendered facts/quotes with current source
validation. Existing registrations/recipes remain unchanged. [Guide](LORE.md).


Optional [information/inventory tools](INFO.md) expose bounded model calls without changing
registered recipes. Hosts bind audience scope, source evidence and visible owner IDs in
callbacks; model arguments cannot widen access. Reported notes are separate from authoritative
snapshots and `FactRegistry`. Structured lore version 2 and experimental natural phrasing
are additive; source selection remains the dungeon default after failed grounding diagnostics.


[Conversation tool adapters](INTERACTIONS.md) reuse the information notebook, reference
resolver and pending confirmation contracts. Bind actor, participants, evidence and current
revision in the host handler. Select individual schemas through `InteractionTools::tools`;
no automatic model/guidance default changes. Several players can participate in one shared
scope, or use separate private scopes. Host events and accepted offers still require engine
transactions and visibility policy; the library provides no networking or authentication.

## Optional persistent character sessions (unreleased v0.3.1a)

`kit::Session` composes an immutable `Project` with at most 64 persistent character
scopes. `gamectx characters` starts it without project-registration boilerplate;
`serve` also retains these scopes between requests. Existing project operations keep
their envelopes and outputs. New native `gamectx_session_new/request/free` symbols
leave the old immutable project ABI intact. Python/Node provide `Character` helpers;
C# retains its host-JSON-library-neutral `Request` entry point.

See [CHARACTERS.md](CHARACTERS.md) for binding views, individually enabling tools,
full-request count acknowledgements and scope lifecycle. These are host operations,
not a bundle of automatically exposed model tools. Durable persistence, perception,
current permissions, consent and engine action execution remain host responsibilities.

## Optional routines, history and time knowledge (unreleased v0.3.1a)

[ROUTINES.md](ROUTINES.md) adds a bounded per-character runtime on the existing session.
Resolve defaults/overrides once, bind the host calendar/routine, schedule local updates
with `schedule::Scheduler`, and publish real sensor records only when available. Feature
flags, requested tool registration, task exposure, data visibility and live permission
remain separate. No runtime work or model calls occur for characters without this layer.
Persistent host configure/update/observation/complete/reserve/save/restore operations are
separate from `routine_status`/`activity_propose`. Python and Node supply helpers; C#/C
use the existing session ABI. Project formats and immutable native symbols do not change.

Host-only `character_routine_context/wake/affected` operations add explicit task evidence
selection (including required tokens), timer hints and planning-event dependency checks.
They use the same persistent session and never register model functions. Complete request
counting is still required after replacing selected context. [Tested example](../examples/routine-context.rs).


Optional host read `character_routine_assess_time` categorizes existing scoped time
knowledge as unknown, approximate, exact reading, reported or conflicting. Python/Node
helpers and C#/C session transport share the same bounded revision-checked operation.
It adds no model schema or permission, and preserves the immutable project/native ABI.
[Measured limits and guards](ADVERSARIAL_EVAL.md).

## Dynamic character session operations

The same persistent `kit::Session` and additive native session ABI support life, continuity
and initiative. Python `Character.life/continuity/initiative` and Node equivalents issue
privileged host operations; never expose these generic bridge methods as model tools.
Models see only individually requested/exposed `life_recall` and `life_change` schemas.
All arguments below also include the character `id`.

| Operation suffix after `character_` | Additional arguments |
|---|---|
| life_configure | setting,definition,defaults,overrides; once per host instance |
| life_experience / appraise | event / rule,event,revision |
| life_context / recall | tags,caps / input,tags,count |
| life_forget / retention | tokens,revision / retain |
| life_ack / evict / consolidate | through / through / start,end |
| life_author_edit | setting,definition,revision; new approved definition revision |
| life_inspect / save | none / identity |
| life_restore | identity,state,fence,min_ticket,min_proposal,min_receipt,min_note,max_tick |
| continuity_configure / scene / thread | none / scene / thread |
| continuity_context | thread,selection,caps,dialogue (existing Thread format) |
| continuity_generate / validate | thread,text,dependencies / ticket |
| continuity_delivery / recognition / retire | delivery / recognition / ticket |
| initiative_configure / policy | defaults,overrides; policy edits preserve replay/call counters |
| initiative_consider / dispatch | frame,candidates / proposal,frame,allowed |
| initiative_outcome / reserve | proposal,token,success / turn |

Combined checkpoint version 2 includes life, interaction notebook/progress/consumed offer
IDs, continuity, initiative replay metadata and retention policy. Maximum combined size is
`MAX_REQUEST_BYTES - 65,536`; standalone life snapshots remain at most 524,288 bytes.
Recovery creates no live reply, offer or activity capability; actual receipts remain host
transactions. Notebook sequences survive erasure. Restore into a fresh instance for an
older save; existing live counters/state cannot silently roll backward. Persist any routine
runtime with its existing configure/save/restore operations and authenticated floors, along
with the engine world/save transaction. No model schema, visibility grant or permission
expands from a save file. Session inspection is host-only.

`life_retention(false)` disables archival election for subsequent working experiences;
it does not erase existing data. `life_forget` returns archive/cache/checkpoint invalidation
instructions and clears volatile dialogue to prevent removed text from leaking. Apply the
returned tombstones in host stores. Authored corrections use `life_author_edit`; player
corrections are attributed experiences. C#/C keep using `Request` over the unchanged ABI.

Host-only `character_life_compile_speech` takes `id,request` with the v2 request contract.
`character_continuity_speech_context` takes `id,thread,selection,caps,dialogue,turn` and returns
a v2 packet plus host inspection; scene/question facts share the attention cap. Both use
the same session/native ABI and existing generic Python/Node `life`/`continuity` helpers.
They register no model tools. The stateless `local` sentence-boundary helper is available
through the existing bridges. [V2 speech contracts](FORMAT.md#turn-aware-life-speech-v2).

The [archetype/party layer](ARCHETYPES.md) is available through the unchanged `local` bridge
operation: kinds archetype, attitude-gates, party and party-compile. Python `Client.request`,
Node `GamectxClient.request` and C#/native `Request` use the same version-1 envelopes. The
party workflow adapter replays up to 256 host operations in one request; it does not keep a
new persistent native scene between calls. Rust integrations own `PartyScene` directly;
other hosts retain authenticated replay/state and reservations. Do not expose these host
configuration or fixture operations to models. No new native entry point is required.
