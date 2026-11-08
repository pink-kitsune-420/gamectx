# Optional information tools

`info::Notes` gives a model three optional calls: `info_remember`, `info_lookup` and
`info_forget`. A host chooses one conversation/audience scope and supplies original
source quotations outside model arguments. Notes can describe ownership, locations,
preferences, relationships or quest status. Values are structured JSON, not scripted replies.

```json
{"subject":"item:oil","attribute":"owner","value":"player:fen","source":"line:12"}
```

The recorded result is `{id,subject,attribute,value,status:"reported",source:{id,speaker,text}}`.
It is a claim extracted by a model: preserving an available source does not prove that the
claim follows from it. Conflicting claims remain separate; lookup returns the most recent
16 for the exact subject/attribute. Nothing promotes a note into `FactRegistry`, inventory,
permission or an executed action. Use stable entity IDs and the existing reference resolver
when a player says “it” or uses an ambiguous name.

A notebook holds at most 64 notes. Subject/attribute/source IDs have the common 64-character,
256-byte scope bound; values are at most 1024 compact JSON bytes. Calls are strict JSON,
at most 4096 bytes. Each call accepts at most 16 host-supplied sources with unique IDs,
speaker IDs and 1–512 non-control characters of original text. Lookup arguments are exactly
`{subject,attribute}`, remember exactly `{subject,attribute,value,source}`, forget exactly
`{id}`. `clear()` removes all retained quotations without reusing IDs. Scope isolation and
physical erasure are host responsibilities; `to_value()` is host inspection, not a model packet.
The notebook is in memory and has no automatic persistence or network calls.

`Notes::tools()` returns function schemas to register with a runner. Dispatch an allowed
function name and raw argument string to `Notes::call(name,text,sources)`. The host binds
the notebook and sources; the model cannot select another notebook or supply evidence.
Expose only the useful subset of tools on each turn and include schemas and returned notes
in the full-request budget. See the runnable [example](../../examples/knowledge/info_tools.rs).

## Current inventory lookup

`inventory::Inventory` is an optional snapshot adapter and integer ledger. An existing
engine can supply a snapshot without adopting a second inventory system:

```json
{"version":1,"revision":7,"items":[{"id":"key:bronze:17","label":"bronze key","tags":["key"],"unique":true}],"holdings":[{"owner":"player:fen","item":"key:bronze:17","quantity":1}]}
```

Owner IDs may identify any player, character, container or world location. Unique instances
use different stable IDs; `unique:true` enforces total quantity at most one across all owners.
Omitted `unique` means ordinary counted stock. Tags select relevant rows; they grant no
permissions. Zero holdings are omitted. Instances have no built-in durability/equipment rules.

`lookup_tool()` exposes read-only `inventory_lookup({item})`. The host passes allowed owners
to `lookup_call(text,owners)`, so a model cannot expand visibility with extra arguments.
The result `{revision,rows}` contains uniform `{id,label,tags,owner,count}` rows, in holding
order. Empty owners return no rows. `rows(owners,tags)` requires every supplied tag and can
feed native SQL or table packing; counts and filtering stay local. There are at most 128
catalog entries and 256 positive holdings, 16 tags/item, 32 selected owners and 16 filter tags.
Snapshot/call byte limits are 262144/4096; counts and revisions are integers up to 2^32−1.

Hosts may use `transfer(revision,moves,allowed)` for a batch of 1–32 exact
`{item,from,to,quantity}` moves. All counts must stay valid, policy must accept each move,
and the revision must match. Commit increments revision once; stale, denied, insufficient,
overflowing or over-capacity batches change nothing. It neither mints nor destroys items.
The callback sees the original host state: the host must check batch-wide funds/capacity
and apply external currency/events in its own transaction. `propose(owner,text,allowed)`
accepts bounded `{revision,item,to,quantity}` JSON with the source owner bound by the host.
Neither transfer API is automatically exposed as a model tool.

Characters can learn a trade through a fresh lookup or a host-published event. Only witnesses
or informed participants should receive the event. Learning here means memory and current
context, not changing neural weights. A stale reported claim never edits current ownership.

## Verification and context cost

Independent Python/Rust output comparisons cover 39 workflows, including unique-item
trades, conflicts, deletion, source/owner injection, tag selection, atomic rejection and
replayed proposals. These add 39 conformance fixtures; all 661 prior fixtures stay unchanged.
Mutation testing adds 30000 attempts across note arguments, transfers and structured lore.

The example's four function schemas occupy **1457 compact JSON bytes** in total; the
inventory lookup schema alone is **309 bytes**. Its current-ownership result is **108 bytes**,
and its reported claim with quotation is **163 bytes**. These are byte measurements of one
small fixture, not tokenizer savings, a latency benchmark or evidence of model accuracy.
All new checks spend $0 API. Register tools selectively; four schemas need not be sent to
a small model every turn.
