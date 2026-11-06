# Reusable conversations and confirmed actions (unreleased)

The dungeon now uses shared Rust APIs for conversation retention, reply validation and
entity references. Its purchase loop also uses a shared confirmation object. These are
additive, zero-dependency APIs; old formats and all 595 earlier cases stay unchanged.
An executable, tested introduction is [`examples/conversation.rs`](../examples/conversation.rs).

## Existing inventory, tags and facts

The host owns inventory. `query::run` counts rows with `COUNT(*)`, totals stack quantities
with `SUM(count)`, and filters/sorts/does arithmetic locally. The host supplies the meaning
of fields such as count, per-item price and stack value. Do not confuse number of rows
with number of units. SQL arithmetic retains its existing f64 contract; the dungeon
uses bounded integers for currency. Catalog entries and `memory::Memory` already support tags; table
views select fields and eligible rows, and kit registrations share types, meanings,
units, IDs and visibility. The fact registry and evidence APIs retain provenance and
invalidate stale claims. These are existing capabilities, not new inventory storage.

The dungeon currently has individual items and sells one unit at a time; it is not a
stacked-inventory implementation. Its shop checks current ownership, price and coins
and calls deterministic game rules. Adapters for real stock, currencies or quantities
remain host code. No generated sentence can register inventory or waive a payment.

## Scoped conversation memory

`dialogue::Conversation` owns bounded scopes of whole exchanges and original player
quotes. Create one per actor/session (or use actor/player IDs in the scope). The host
must authorize access to a scope and supply current facts and revision. Scope strings
are exact identifiers, not an access-control service. Nothing is loaded or persisted
automatically. Call `forget(scope)` to remove that scope and its pending reply ticket.

`prepare(args)` takes exactly:

| Field | Meaning |
|---|---|
| scope | Host-authorized conversation identifier |
| speaker | Host-authorized speaker identifier |
| revision | Current host revision, unsigned 32-bit integer |
| input | Current player line; original wording is stored after a valid reply |
| facts | Host facts object, at most 32 named fields and 32768 bytes |
| recall | Number of quotes selected by existing lexical recall, 0..min(32, records) |
| history | Number of recent whole exchanges selected, 0..configured history |

It returns `{id,revision,scope,speaker,input,facts,memories,history,tone}`.
`reply_schema()` returns the pending ticket's bounded response schema;
`speech_schema(ticket,text_chars)` builds the same schema for a host-created ticket. Quote records
are `{id,turn,text}`; exchanges are `{id,turn,user,assistant}`. `turn` is the conversation's
monotonic exchange/ticket sequence, not a claim about world time. The ticket is consumed
by `finish(scope,current_revision,ticket_id,model_text)`. Preparing a new ticket replaces
the pending one. Invalid preparation or completion preserves state.

Completion parses at most 16384 bytes of strict JSON, exactly
`{speech,tone,fact_ids,memory_ids}`. Speech is a nonempty single line; tone is
neutral/warm/wary. At most eight unique fact IDs and two unique recalled quote IDs may
be cited, and they must have been supplied in that ticket. The host still checks live
speaker access and current revision. Source availability does **not** prove that prose
follows those sources. Speech and player quotes are never promoted into world facts.

`state(scope)` returns `{scope,tone,history,memories}`. `pending_scope()` helps a host
perform live access checks without guessing the speaker. Default limits and allowed
configuration ranges are:

| Limit | Default | Allowed |
|---|---:|---:|
| retained scopes | 32 | 1..64 |
| player quotes per scope | 32 | 1..256 |
| whole exchanges per scope | 8 | 1..128 |
| characters per player/speech line | 240 | 1..1024 |
| prepared tickets per instance | 65536 | 1..1000000 |

Use `Conversation::with_limits(ConversationLimits { ... })` to choose explicit bounds.
Quotes use equal importance and evict oldest first; exchange retention also removes the
oldest. Full scopes cause a controlled error until the host forgets one. Retention limits
are not model context limits: use `Scene`, task profiles and full-request budgeting to
select required dialogue/facts for each call. The dungeon keeps its earlier 256-ticket,
4096-byte reply contract and its older replay bytes unchanged.

## Entity references

`dialogue::resolve_references(args)` takes exactly:
`{entities,input,kind,scope,revision,max_age,focus,selection,pronouns}`.

Each host-visible entity is `{id,label,kind,aliases}`. IDs are opaque, unique strings;
aliases are explicit host data. `kind` is null or an exact entity kind filter. `focus`
and `selection` are null or `{id,scope,revision}`. `pronouns` is a host-supplied list of
reference phrases, allowing different languages without hardcoded English action verbs.
The host supplies the type appropriate to its action and authenticates any selection.

The resolver lowercases and matches complete alphanumeric token phrases. A contained
shorter phrase loses to a longer match. Multiple surviving entities produce ambiguity;
missing reference context returns eligible candidates for clarification. Focus requires
the same scope, a nonfuture revision, age <= max_age and a currently eligible ID. An
explicit host selection requires the exact current scope/revision and an eligible ID.
The host must also bind it to the particular question/candidate list shown to the user;
this metadata is not an authentication token. No choice is made merely by list order.

Output is `{reason,candidates:[{id,label}]}`, with reasons phrase, ambiguous, unmatched,
type_mismatch, unresolved_reference, recent_action or explicit_target. It executes no
action and infers no permissions. Bounds: 64 entities, 1..8 aliases/entity, 16 pronoun
phrases, 2048 input bytes/256 tokens, max_age 0..1024, 65536 argument bytes. Identifier,
label, kind, alias and scope strings have the existing 64-character/256-byte scope
bound; aliases need 1..32 alphanumeric tokens. This is lexical matching, not semantic
coreference, Unicode normalization or proof that a pronoun refers to a particular object.
Overlapping-span suppression uses prefix maxima, avoiding pairwise span comparisons;
a worst-shape test with 64 entities × eight repeated aliases × 256 input tokens completes
and preserves all 64 ambiguous candidates.

## Confirming an action

`decision::PendingAction::new(offer)` takes
`{id,scope,revision,decision:{action,target},terms}` with at most 8192 bytes; terms is an
opaque host object. It validates structure and stores the proposed action and terms.
This constructor does not establish affordability or permission; the host does that first.

Call `respond(scope,current_revision,confirmed,current_terms,available)` with a human
response or explicitly authorized host policy. A decline returns `{status:"cancelled"}`.
Approval requires matching scope/revision, identical host terms, and a live permission
predicate. It returns `{status:"confirmed",decision,terms}` without executing anything.
Every attempt consumes the instance, including stale/revoked attempts. Repeated approval
fails. `offer()` exposes a copy for the UI, and `is_finished()` reports consumption.
The host must atomically dispatch using the same revision and terms; cloning or returning
an object does not provide cross-process transaction guarantees. Never treat model prose,
a model-authored `confirmed` field or an old “yes” as authorization.

The dungeon's English purchase detector and its coins/items remain in the example.
Clear purchase requests generate local offers. Unaffordable tonic gives the exact result:
“The tonic costs 9 coins. You have 5, so you need 4 more.” No offer is pending afterward.
An affordable request asks for yes/no; approval buys once and reports the new balance.
Decline, leaving conversation, forgetting, a topic change, or a stale world revision
prevents a later accidental purchase. Local trade receipts are shown directly; they are
not generated speech and are not added to the quote store. The next model request uses
the current authoritative world snapshot.

## Bridge formats and evidence

Three additive `local` kinds are available through CLI and existing bridges:

- `references`: args are the resolver object above.
- `conversation`: args `{limits,operations}`; limits contains exactly the five fields
  above. At most 256 operations: `{op,args}` with prepare, finish, schema, state or forget. Schema takes `{}`.
  Finish args are `{scope,revision,ticket,text}`; state/forget take `{scope}`. Replies
  are `{results:[{ok:true,result}|{ok:false}]}`. This is a fresh replay per call;
  native Rust users retain a `Conversation` across turns.
- `confirmation`: args `{offer,calls}` with at most eight calls, each exactly
  `{scope,revision,confirm,terms,available}`. The fixture predicate uses the supplied
  available pairs (<=32). Returns `{results,finished}`. This is a validation adapter,
  not automatic action dispatch.

Independent Python prototypes agree with Rust on **78 workflows**, including retention,
stale replies, foreign scope, forgotten tickets, forged sources/actions, terms
changes, declines, replay, Unicode IDs and reference bounds. Thirty-eight new conformance
cases are additive; 30,000 additional model-text mutations cover the shared contracts.
The separate trading reference checks **66 workflows / 144 trade requests across six
seeds**, with zero model calls. Eleven real-host/fake-generation front-end tests include
the reported unaffordable loop, a successful one-use purchase, decline and stale offers.
An actual Rust-to-Python AI-launcher smoke replays the purchase path with no generation.
These are exact rule/validation tests, not new LLM accuracy or latency measurements.
The same 78 workflows also pass 156 persistent/native bridge checks.
New API spend is $0; existing token/prompt cohorts remain frozen and audited.

```sh
cargo build --locked
cargo build --example dungeon --locked
python3 -I eval/conversation_cases.py --cap 0 --check
python3 -I eval/dungeon_trade_cases.py --cap 0
python3 -I eval/test_dungeon_ai.py
```
