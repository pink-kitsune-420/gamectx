# Authored lore and source-bound answers (unreleased)

`lore::Lore` validates and prepares an authored world-info file once. `dialogue::Answers`
lets a model select a supplied source ID while the host supplies every word of the factual
answer. Existing formats, renderers and all 633 previous cases are unchanged. The Rust
core remains standard-library-only and supports Rust 1.82.

Start with the tested [`examples/lore.rs`](../examples/lore.rs) and editable
[`examples/dungeon/world.json`](../examples/dungeon/world.json). The dungeon uses these
APIs by default in AI conversations; `--dialogue legacy` retains the previous control.

## Authored world information

A world file is exactly `{version:1,entries:[...]}`. Every entry contains:

| Field | Meaning |
|---|---|
| id | Unique stable source ID |
| subject | Stable entity/world/character ID |
| topic | Host-defined field/topic, such as background, use or ownership |
| text | One authored statement; no control characters, 1..1024 Unicode characters |
| tags | Up to 16 lexical search hints |
| known_by | 1..16 authorized actor IDs, or exactly `["*"]` for public knowledge |
| requires | Up to 16 additional host-granted knowledge labels, all required |

IDs, subjects, topics, tags, audiences and grants use the existing scope bound: nonempty,
up to 64 Unicode characters / 256 UTF-8 bytes, without control characters. Arrays of
strings reject duplicates. IDs compare exactly; retrieval lowercases alphanumeric token
phrases without semantic embeddings, stemming or Unicode normalization. Tags can supply
additional useful word forms. Do not encode a mutable price or owner as immutable lore.

`Lore::parse`, `from_value` and `read(path)` validate the same format. Bounds are 512
entries and 262144 input bytes; file reads stop at the cap plus one byte. Invalid versions,
duplicate IDs, malformed audiences and oversized input fail before selection. There are
no includes, scripts, environment expansion or network fetches. `to_value()` exports the
validated definition. Existing instances own their data; editing a file affects the next
load, not an active instance. The default dungeon embeds its example file; `--world FILE`
loads an external file once at startup.

This module does not authenticate an actor. The host supplies the actual actor and
knowledge grants after checking its permissions. Models must not choose those values.

## Select authorized, relevant records

`Lore::select` takes exactly
`{actor,grants,query,subjects,required,limit,max_bytes}`:

- Grants: up to 64 exact labels; subjects: up to 32 exact IDs (empty means any subject).
- Required: up to 32 source IDs that must be in the authorized subject view and fit.
- Query: up to 2048 bytes. Limit: 0..32 records. Byte budget: 2..32768.

Audience and grant checks run **before retrieval**. Required records come first, in file
order; missing/private required IDs fail without an enumeration of hidden entries.
Optional records rank by distinct query-token overlap with ID/subject/topic/text/tags,
then file order. Zero-overlap optional entries stay omitted. Optional candidates are
tried in rank order while preserving both bounds; this greedy selection is not optimal
packing. A required-fit failure does not drop the required record.

Output is `{packet:[{id,subject,topic,text}],receipt:{bytes,selected,omitted}}`.
The receipt names only records in the authorized subject view. It does not expose private
IDs or hidden record counts. Known-by/grant/tag metadata never enters the model packet.
`max_bytes` bounds the packet array only; count the final provider request separately,
including its tools/schema/template and output/safety reservations.

The current example world is 1669 compact JSON bytes; selecting its tonic record produces
145 packet bytes. This removes metadata and unrelated records, rather than compressing
all world information losslessly. It is a byte measurement, not a tokenizer saving.

## Keep sources distinct

Build current answer cards from trusted host data:

| kind | Appropriate source |
|---|---|
| lore | An authorized authored statement |
| state | Current authoritative inventory, ownership, prices, capabilities or observations |
| quote | Exact original player words, scoped to that conversation |

Prior generated dialogue is not an accepted source kind. Player claims are reported as
claims, never silently registered as world truth. A null/missing player home or unobserved
weather needs an explicit unknown response. Authored item purpose must not be inferred
from the absence of an implemented action. Keep authored implementation descriptions in
sync with the game version.

`Answers::from_value` takes exactly
`{version:1,scope,revision,unknown,cards}`. Each card is exactly
`{id,subject,topic,kind,source,text}`. Source is an opaque host-owned source ID; card IDs
are unique, and `unknown` is reserved. Bounds: 32 input cards, 65536 definition bytes,
1024 characters per card text, 2048 per unknown response; strings are single-line.
Scope/revision identify the current host context (revision is unsigned 32-bit).

A state card suppresses lore cards with the same exact subject and topic, regardless
of input order. This protects declared runtime fields against stale authored values.
Topics are host-defined: the library cannot infer that two differently named topics
refer to the same property. Quote cards remain reported statements, with distinct provenance.

`offer()` emits `{id,subject,topic,kind,text}` records; `schema()` accepts exactly one
`{answer:<card ID or "unknown">}`. Send only the intended model request, not host receipt
or full world metadata. `resolve(text,context,available)` parses at most 4096 bytes,
checks exact context scope/revision, and requires the selected source to pass the live
predicate. Source callback input is `{kind,id}`. Extra prose, forged IDs, stale context,
unsupported kinds and revoked sources fail.

The output is `{status:"answered"|"unknown",text,sources}`. Lore/state text is copied
verbatim; player quotes render as `You told me: "<original words>"`. Unknown returns the
host's supplied response and no sources. This performs no world mutation or fact
registration. It does not prove that the host's card is true or that the selected card
answers the question. Hosts must construct cards from authoritative/authorized sources
and bind source availability to current state.

## Dungeon workflow

The original English question policy is in `examples/dungeon/knowledge.rs`. It prepares
live owner/price/use/stock/balance cards, authorized lore, and relevant original player
quotes. Explicit missing attributes return unknown locally. Open-ended conversation uses
small selections of authored character lore alongside the existing current scene.

The AI front end keeps three stages: local trade requests/confirmation, source-bound
factual questions, then free-form character dialogue. Generated speech never approves a
purchase. The default task context is still 2K for class 3B, with explicit overrides and
32 output tokens plus 256 safety for source selection. Required request fit is checked
with an explicitly estimated JSON-byte/3+64 counter, then native one-token preflight.
Native counts are post-call observations, not proof arbitrary input escaped truncation.

Factual offers use one-use conversation tickets. Completion rechecks speaker access,
world revision and the current source card, then records the original player input and
host-rendered response. Invalid/budget-failed preparation preserves earlier pending state.
A successful new speech request, forgotten conversation, moved actor or changed source
prevents stale completion. Model responses cannot append arbitrary speech to an ID.
An optional choices_limit override (1..32) changes the editable size hint without dropping cards.
One correction is allowed, followed by a current valid unknown response.

Old speech entry points retain the 240-character reply bound. New host-rendered factual
answers can be up to 512 characters, preserving a complete 240-character player quote
plus its attribution. Longer source answers fail preparation instead of truncating.
The generic library bounds remain independent of those example limits.

Example-only JSON host operations:

| op | args | Result |
|---|---|---|
| knowledge_prepare | `{speaker,input,context,size,choices_limit?}` | `{handled:false,lore}` for dialogue fallback, or `{handled:true,ticket,offer}` |
| knowledge_finish | `{ticket,text}` | `{speaker,speech,sources,status}` from a currently valid source selection |
| lore_speech_prepare | Same args as speech_prepare | Existing speech request with selected authored lore added |

The factual offer contains `definition`, `request`, `reserved_output` and selection
receipts. Send only `offer.request`. `--world FILE` and `--dialogue sources|legacy` are
available through both the Rust AI launcher and the Python front end. Live game ownership
and prices remain host-owned, regardless of what a world file says.

The question router is an English example policy, not a universal semantic parser. It
can miss paraphrases, abstain unnecessarily or select a related fact that fails to answer
the question. Multi-part questions and unresolved pronouns may need clarification. The
free-form dialogue path still permits unverified generated prose. These limitations are
explicit in the retained tests; source formatting does not establish perfect understanding.

## Formats and verification

Additive local kinds:

- `lore`: args `{world,select}`; returns the selection above.
- `answers`: args `{definition,calls}`, at most eight calls. Each call is exactly
  `{text,context:{scope,revision},available:[{kind,id}]}` (at most 32 available sources).
  Returns `{offer,schema,results:[{ok:true,result}|{ok:false}]}`; it never executes actions.

Independent Python/Rust comparisons cover **108 workflows**; 28 new conformance cases
leave all 633 earlier cases unchanged. Sixty additional native example workflows compare
complete candidate/request JSON across five seeds. Eleven real-host/fake-generation tests
cover privacy, stale replies, exact long quotes, current owner checks, source injection,
failed budgets and external file loading. Mutation tests add 30000 attempts.

[Local model report](LORE_EVAL.md) retains successful and failed pilots, twenty new
questions, their tuned rerun, eight further questions and the final local repair. Every
new token/accuracy/audit run spends $0 API; no larger model was loaded.

## Structured values and experimental natural phrasing

Lore version 2 replaces each entry's `text` with a `value` containing any JSON value, up
to 4096 compact bytes. Identity, audience, grants and selection rules stay the same. The
packet preserves `{id,subject,topic,value}` and relevance indexes the serialized value.
Version 1 and its packet bytes remain unchanged. Null represents an explicitly missing
attribute; structured data alone does not make a model respect that absence.

The dungeon provides [structured example lore](../examples/dungeon/world-structured.json)
and an opt-in natural speech path:

```bash
cargo run --example dungeon -- --ai --dialogue natural --world examples/dungeon/world-structured.json
```

This passes relevant typed facts and the exact current request to the model, which generates
its own bounded speech. It keeps original player quotations and recent whole exchanges,
uses full-request estimated checks plus the runner's native token preflight, and executes
no world action. A failed request budget does not consume a conversation ticket. Source
selection remains the default; `legacy` remains available. Selecting version-2 values in
strict source mode renders their compact JSON as text, so use version 1 for authored strict
prose. The default world file stays version 1 for compatibility.

The natural diagnostics retain 40 generated replies across two prototypes: all 40 satisfy
the speech shape, but both invent player homes, tonic effects or permissions. The second
prototype reduces aggregate structured-world request bytes **20829→9962** over the same ten
questions while changing both prompt wording and fact selection. This is a byte-size result,
not an isolated format comparison or a demonstrated accuracy improvement. It remains an
experimental option, with no semantic guarantee or automatic default promotion.
