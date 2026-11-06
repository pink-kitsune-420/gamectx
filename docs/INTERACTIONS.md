# Optional conversation tools

`dialogue::InteractionTools` adds eight individually selectable model functions over the
existing informational notebook, reference resolver and confirmation APIs. It supplies
context for adaptive dialogue; it never executes a trade, awards reputation, confirms a
promise from generated text, or retrains a neural network. The host still owns the world.
The runnable [example](../examples/conversation_tools.rs) exercises all five capabilities
with two player IDs and one character, without a model or network.

| Capability | Optional model call | Information returned or remembered |
|---|---|---|
| Witnessed changes | `events_recent({since,subject,limit})` | Recent host-confirmed events visible to this actor; `subject:null` selects all subjects in scope. |
| Promises | `promise_remember({promisor,beneficiary,task,source})` | A reported promise with original quotation; no acceptance or completion is inferred. |
| Promise progress | `promise_lookup({participant,limit})` | Reported promises plus separately host-confirmed progress; `participant:null` selects this scope. |
| Preferences/boundaries/interactions | `relationship_note({other,note,source})` | A reported relationship note with evidence, without an invented trust score. |
| Relationship recall | `relationship_lookup({other,limit})` | Relevant notes for one explicit participant ID. |
| Ambiguous references | `dialogue_resolve({kind})` | Exact matches or clarification candidates for the original host-bound utterance; `kind` may be null. |
| Available interactions | `interaction_offers({limit})` | Current host-allowed offers, recipients, decisions and exact terms. |
| Proposed interaction | `interaction_offer({id})` | One pending offer, awaiting confirmation by its specific recipient. |

Create an instance with `InteractionTools::new(scope,actor,participants)`. Register only the
schemas selected through `tools(&[names...])`. Dispatch a permitted function name and raw
argument JSON through `call(name,text,evidence,revision,max_bytes,available)`. Scope, actor,
evidence, current revision, result byte cap and policy callback come from the host, outside
model arguments. The callback checks current permissions without executing. The runner must
feed the bounded result back into context and budget the complete request, including schemas,
results, template overhead, history and reserved output. No tool is enabled automatically.

## Multiplayer and privacy

Participants are explicit stable IDs: for example `npc:merchant`, `player:fen` and
`player:mira`. One scope may represent a private conversation or an intentionally shared
party conversation. Use different instances/scopes for private conversations with different
players; a shared roster alone does not make a private quotation safe to disclose. Notes
belong only to this instance. There is no global cross-character lookup.

Host-published events are exactly `{id,turn,subject,kind,value,audience}`. Audience must be a
nonempty subset of the roster; only events including the bound actor appear in model results.
Unauthorized records, audience lists and counts are absent. For example, one character can
learn that Fen acquired a unique key by witnessing that transfer, while another character
who was elsewhere receives no event. Original ownership remains in the game's current
snapshot; historical events are not a replacement for live inventory lookup.

Construct a fresh scope when membership/permissions change, or stop dispatching to an old
scope. Networking, player authentication, host persistence and live permission checks remain
integration responsibilities. The API supports multiple players; the dungeon stays a
single-player demonstration.

## Host-confirmed progress and consent

`promise_remember` retains an extracted claim. A source ID establishes provenance, not that
the model interpreted the promise correctly. Host-only `set_progress({id,expected,state,source})`
can move `reported→accepted/cancelled` or `accepted→fulfilled/cancelled`. `expected` is the
promise's current progress revision, initially zero; each accepted transition increments it.
Completion requires acceptance first. Terminal states do not reopen. Evidence is an original
host-supplied `{id,speaker,text}` record. These transitions do not check a quest engine: the
host must publish them only after its own validation.

`set_references(context)` binds the actual utterance, visible entities, aliases, focus,
pronouns, scope and revision using the existing reference contract. A model can select a
kind, but cannot replace the utterance, focus or revision. Stale contexts fail. Ambiguous
candidate lists are returned whole or fail the output bound; they are never shortened into
a false single match. This is bounded lexical resolution, not general language understanding.

`set_offers([{recipient,offer}])` installs a host menu. Each `offer` uses the existing exact
`{id,scope,revision,decision,terms}` confirmation format. The model can propose only a current,
allowed entry. Host-only `respond(recipient,revision,confirm,terms,available)` requires the
specific recipient: another player cannot consume their offer. Confirmation rechecks that the exact offer is still present in the current menu. A valid recipient's one
attempt consumes the offer, including decline, stale terms or revoked availability. IDs
cannot be replayed or resurrected by replacing the menu. A confirmed result is still a
proposal requiring atomic engine dispatch with the same revision and terms.

## Bounds and lifecycle

The roster holds at most 16 participants and must include the bound actor. Model argument
strings are strict JSON, at most 4096 bytes; result caps are host-controlled, 64–8192 bytes.
Read tools accept limits 1–8. Events hold at most 128 records with distinct IDs, nondecreasing
u32 turns and 1024-byte values. Use the same host revision counter for event turns and query
revision; `since` is inclusive, so polling callers should deduplicate event IDs. Event and
note lookups select the most recent records that fit. They are bounded context selections,
not complete audit streams. Full stores reject new records rather than silently evicting.

Promises and relationships share the notebook's 64-note capacity, 1024-byte value bound and
16-source evidence limit. Task/note text is 1–512 non-control characters. Progress revisions
and IDs are bounded u32 integers. Offer menus hold at most 16 entries, 8192 bytes/offer;
consumed IDs are retained up to 256. Hosts can rotate scene scopes instead of growing stores
indefinitely. `clear()` erases events, notes, quotations, progress, menus, reference text and
pending offers. Note sequence and opaque consumed offer IDs remain to reject replay; drop
the instance to erase identity/replay metadata as well. `to_value()` is host inspection,
including private audiences, and must not be supplied as a model context packet.

Read-only calls avoid cloning the entire instance. Mutating model calls commit only after
validation and result fitting; failed calls leave notes, IDs and pending offers unchanged.
Existing formats and APIs remain compatible. No external dependencies are added.

## Evidence and limits of the measurement

Independent Python/Rust comparisons cover **87 workflows**, with **47 new conformance cases**;
all 700 earlier cases stay unchanged. Tests cover private audiences, two-player confirmation,
source/authority injection, promise progress, erasure, stale context, ambiguous aliases,
output-budget rollback and replay. Mutation tests add **30000 attempts**.

In a 120-event fixture, 80 events are authorized for the character. Sending every authorized
record occupies **8589 compact JSON bytes**; selecting two recent records occupies **231**.
That deliberately omits old context; it is not lossless compression or a token measurement.
All eight schemas total **3127 bytes**. Individual schemas range from **289 to 518 bytes**;
`events_recent` is **422 bytes**. Register a few relevant tools, not the full menu on every
small-model turn. Reproduce after building the example:

```bash
python3 -I eval/interaction_measure.py --cap 0 --check eval/interaction-results.json
```

All checks spend **$0 API**. These are deterministic format, privacy and byte-size checks,
not live-model tool-selection accuracy, complete-turn latency, RAM or gameplay-quality
measurements. Those evaluations remain next work before making model-specific defaults.
