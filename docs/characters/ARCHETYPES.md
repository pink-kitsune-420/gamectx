# Modular loadouts and multi-party scenes

This opt-in layer composes the existing life, routine, brain, inventory, lore, evidence,
interaction, deployment and spending APIs. It adds no dependency, background worker,
model download, model call, tool registration or automatic game action. Existing APIs,
character scopes, native ABI and historical conformance outputs stay unchanged.

Run `cargo run --example party`. Edit [chime-counter.json](../../examples/fixtures/party/chime-counter.json)
to change the four characters, two players, approved definitions, sidecar gates, stock,
routine, routes, floor limits and spotlight policy. [party.rs](../../examples/character/party.rs)
is exercised by tests and demonstrates a reserved escalation, actual zero-call guard
routine/brain, private hearing, two different player attitudes and gate removal.

## Explicit loadouts

| Preset | Default modules | Size / reserved turn calls | Context |
|---|---|---|---|
| ambient | routine, brain, initiative | 0B / 0 | No model prompt |
| service | routine, life, stock, lore | 3B / 1 | Focused, one exchange, no episodic recall |
| quest | routine, life, lore, evidence, recall, promises | 3B / 1 | Focused, one exchange, two episodes |
| companion | routine, life, lore, evidence, recall, promises, continuity, initiative | 8B / 2 | Expanded, two exchanges, three episodes |

All presets default balanced/deception/mistaken policies and model tools **off**. Constructing
a preset is the explicit opt-in operation; there is no global default character type.
`BlueprintBuilder::new(Archetype::Service).module("recall", true)?` composes modules without
copying a character framework. Set `size`, `policies`, `tools` and `model_calls` explicitly,
then `build`. The editable input has exactly version/preset/size/modules/policies/tools/
model_calls. `modules` is a partial override; all ten supported module keys appear in
the resolved result. Invalid dependencies fail instead of silently enabling modules.

Recall, evidence, promises and continuity require life. `life_recall` requires recall;
`life_change` requires continuity. Balanced/deception and `life_change` registration require
an expanded base loadout. The 0B class has zero calls, no tools and no expressive policies.
This adds no `ModelSize::B0` variant: existing exhaustive model-size matches keep compiling.
Sizes 1B/1.5B/3B focus attention; 8B/30B/70B use larger workload hints, reaching three
exchanges and six recalled episodes. These are advisory classes, not measured capabilities.
Frontier/hybrid deployments use explicit identity-pinned profiles with the chosen hint class.

Blueprint methods install an approved `life`, `routine`, `coordinator` or `initiative`,
run a configured `brain`, select authoritative `stock`, dependency-complete `evidence`,
or inspect accepted `promises` through existing modules. Disabled adapters reject use.
There is no second inventory, promise ledger or simulation. Routine/initiative configuration
always reserves **zero background model calls**; foreground calls go through the floor.
Ambient initiative can use the existing disabled-Life identity adapter if it needs an
identity-scoped candidate set; routine/brain operation itself requires no life state.

## Attitude gates before selection

`AttitudeGates` holds optional sidecar rules, leaving lore and definition formats intact:

```json
{"kind":"lore","id":"hidden_chime","clauses":[
  {"attitude":"trust","target":"listener","op":"ge","value":15},
  {"attitude":"resentment","target":"speaker","op":"le","value":10}
]}
```

Kinds are lore/facet/affordance. Every clause must pass; a listener clause applies to
**every** host-declared listener, including possible overhearers of the upcoming reply.
The addressing speaker and intended listener set are distinct. Levels come from the active
AI's current Life state, rechecked against the requested life revision. Missing attitudes,
ambiguous duplicate kind/subject attitudes, or failed thresholds deny. Inclusive endpoints
are explicit; thresholds are bounded to -100..100. They are game parameters, not calibrated
psychology or probabilities.

`PartyScene::compile` filters facets **before attention caps**, lore **before ranking**,
and offered affordances before serialization. Withheld payloads and IDs are absent from
the provider request. Host-only inspection records kind/ID and deterministic reasons
(`attitude_missing`, `attitude_ambiguous`, `attitude_threshold`). A denied required lore
card produces `request:null` and the fixed deferral “I cannot share that or offer it yet.”
Optional denied content is omitted while available context remains usable.

Gates add restrictions to existing knowledge grants and fixed-scope audiences. They do
not grant knowledge, override `known_by`/lore prerequisites, make a private facet public,
or authorize an action. `Life::party_context` selects attitudes in subject order, prioritizing
the addressing participant, listeners, then explicitly relevant scene participants.

Use `PartyScene::call_life` for tools exposed in a party request. It verifies the actual
schema, blueprint permission, current request exposure, revisions and host permission.
Its `life_recall` result reapplies facet gates and attention caps; `life_change` remains a
proposal requiring host appraisal. Calling legacy `Life::call` directly bypasses this new
scene policy, so it must not be used as the dispatcher for these requests. No tools are
exposed automatically, even on large profiles.

## Hearing, perspectives and the floor

A `PartyScene` binds one scene ID/epoch/revision and up to 16 named participants, each
player or AI. It accepts up to 128 monotonic host receipts. Each includes token, speaker,
recipients, heard, exact confirmed text and an optional earlier causal parent token.
Generated/queued/played text is insufficient; partial speech must contain only the confirmed
portion. The host supplies hearing, sensory occlusion and parentage. Recording a receipt
does not write a Life experience or confirm any quoted claim as world truth.

Rendering filters by the active AI's heard membership, then keeps the bounded tail. Only
that AI's own confirmed lines receive `assistant`; player and other NPC lines receive
`user` with `[speaker -> recipient,...]` attribution. Participant IDs use bounded ASCII
names, so attribution delimiters cannot be injected through IDs. Quoted text remains
untrusted data. The current trigger must be addressed to and heard by the AI; it appears
once, outside nested JSON, after stable rules/character/scene and changing state/history.

Floor control defaults off. An enabled floor validates all candidates before changing state,
then ranks by host priority and input order for ties. It selects only addressed AI recipients
that actually heard the trigger. `social_limit` and `model_calls` apply across **all** floor
invocations and responders in the monotonic host turn. Zero-call ambient gestures can share
the floor with one model-backed response. One player utterance cannot be consumed twice.
Causal root tracking prevents an AI from replying twice in an uninterrupted NPC chain, even across turns;
already confirmed NPC replies also populate that guard. A configurable maximum depth bounds
NPC-to-NPC exchanges. Every new player utterance starts a fresh response chain, including
a follow-up linked to an earlier NPC line; normal multi-exchange dialogue remains available.

Grants reserve worst-case calls/cost **before I/O**. `dispatch` is one-use and checks scene
revision, epoch and turn; stale grants cannot dispatch. `cancel` releases an undispatched
spending reservation, while call/social allowances and chain participation stay consumed.
This avoids retry loops. The host confirms delivery separately through a receipt. There
are at most 16 outstanding grants and 128 total grants per scene instance; all overflows
fail or defer predictably. Create a new scene instance when changing scene/save branch;
do not rebuild it each turn to reset quotas. Durable scene/floor storage is host-owned.

## Spotlight routing and host duties

Spotlight defaults off. Explicit host significance, a computed crossing of a supplied
attitude threshold (either direction), or at least two conflicting participants can
request expansion. The floor attempts the expanded route first and falls back to focused
when call or spending reservations fail. Routine turns select focused. These are host
signals, not a model's self-assessment or inferred social understanding.

`ModelRoute` contains a host profile name, hint size, worst-case calls and integer
microdollar cost. Focused candidates use 0B–3B; expanded candidates use 8B–70B hint classes.
The shared `SpendingLedger` enforces real reservations, including prior pending work;
no amount is spent or network request issued by this layer. `bind_profile` checks the
route name and the complete actual deployment identity with the existing Profile contract.
`Blueprint::for_route` projects attention caps onto the selected route without enabling
new modules or policies; focused projections suppress `life_change` exposure and the
speech compiler suppresses creative invitations on small/unknown turns. Installed life
configuration and per-turn exposure remain separate.

Hosts authenticate participants, scope/save identity, grants, receipts, listeners and
signals; keep a single floor owner for the scene; share foreground/background call budgets;
redact stable scene/rules and other host inputs; count the complete real provider request,
including output/safety/tools/retries; dispatch within reserved calls; settle spending;
validate gameplay prerequisites at execution; and acknowledge actual outcomes/hearing.
This layer does not load models, set runners, transfer stock, discover conflicts, redact
arbitrary duplicate secret prose, or prove that generated language follows its sources.
Do not copy gated payloads into unrelated history, evidence, recalled events or host fields.
Previously disclosed information requires separate host retention/erasure policy.

## Evidence

Independent Python/Rust parity covers **68 workflows**, including **12 compiled-turn
workflows**, gate boundary/missing/ambiguity cases, multi-listener denial, mixed archetypes,
six model tiers, heard-only roles, shared quotas, replay, existing confirmed replies,
failure recovery, stale grants, call/spending fallbacks and a 129th-receipt rejection.
There are 68 additive conformance cases; all 904 prior cases remain unchanged.
Python CLI/native checks cover 136 replies; Node and C# each check the same 68 fixtures.
Rust additionally tests live appraisal-to-gate transitions, gate-aware recall dispatch,
identity mismatch, hybrid projections and real routine/brain adapters. Mutation checks
exercise 12,000 altered loadout/scene/compiler inputs.

The retained [offline report](../../eval/character/archetype-party-results.json) measures the original
trusted request at **3,210 bytes** and the untrusted-listener request at **2,708 bytes**,
with **zero gated payloads** in that request and **zero ambient reserved model calls**.
These are host serialization measurements, not token counts, savings against a baseline,
model accuracy or a secret-leakage guarantee for arbitrary duplicated text. Token counts
are unknown without a selected provider/tokenizer. This work made **0 model calls / $0 API**.
No new models or live runs were required; the frozen v1/v2 speech failures remain available.

Run the prototype/token preflight first:
`python3 -I eval/character/archetype_cases.py --cap 0 --prototype-only --tokens-only`.
After building the CLI, `--check eval/character/archetype-party-results.json` audits byte parity and
measurements; `--check-bindings` also checks the existing CLI/native bridge and writes
Node/C# fixtures under target. These checks do not assess conversational quality.

For a shared setting, locations, groups and multiple archetypes in one file, use the
opt-in [CastPack/LivingWorld layer](LIVING_WORLD.md). It adds no model calls and leaves these
blueprint and party formats unchanged. World compilation/dispatch projects effective group
levels through these same gates, while immediate authored speech works without any model.
