# Optional visual compiler and approved asset registry

`gamectx::media::visual` compiles authoritative state into image-generation metadata, with no
provider, executor, image decoder or additional dependency. It never generates or loads an
image. Generation is opt-in and external; selection and repair use zero model calls.
Run the tested, original example with `cargo run --example visual`. Edit
[`examples/fixtures/visual/chime-art.json`](../../examples/fixtures/visual/chime-art.json) to change its art bible.

## Lock the style, select the visible state

`ArtBible::parse` / `from_value` accepts exactly the fields in that example:
`version: 1`, `id`, positive `revision`, `style`, `format` (`portrait`, `sprite`, `scene`),
`width`, `height`, `aspect: {width,height}`, `camera_angle`, `framing`, `palette`,
`negative_tokens`, `anchors`, `expressions`, `poses`, `facts`, `max_fact_age`.
Dimensions are 16–4096 pixels; integer aspect components 1–256 must match the dimensions.
Tokens are nonempty, at most 128 Unicode characters / 512 UTF-8 bytes, without controls or
outer ASCII spaces. Palette has 1–16 unique tokens; initial negative constraints at most 32.
The contract is at most 65,536 JSON bytes; unknown fields and duplicate keys/tokens fail.

Each of 1–64 anchors defines `entity`, `kind` (`character` or `scene`), `silhouette`,
`attire`, `props` (an allowlist of at most 16), and a u32 `seed`. Character formats require a
character anchor; scene format requires the scene's own scene anchor. The entity seed is
stable across expression/pose variants. Style and identity anchors are authored constants;
keep changing weather, visibility and private knowledge in gated fact rules instead.

At most 32 expression rules define `id`, `slot`, `kind`, `subject` (`listener` or `self`),
`min`, `max`, `priority`, `tokens`. Inclusive levels and priorities are integers −100…100.
An enabled live `Life`, matching entity/scope/roster and current revision, supplies attitudes
including `kind: "emotion"`. Only attitudes covering the complete image audience can match.
A kind/subject must have exactly one visible attitude; missing or ambiguous matches use
neutral. Highest priority wins, then lexical rule ID. The example uses trust below zero for
`guarded`, trust at least 15 for `warm`, and a self emotion at least 40 for `tense`.
This is host-authored visual selection, not inference about an image or a character's mind.

At most 32 pose rules define `activity` and `tokens`. `Activity::from_routine` reads actual
active runtime state. `Activity::from_initiative` requires both a ready activity proposal and
its matching host `started` receipt; a proposal alone cannot affect pose. `Activity::idle`
uses `at rest`. `Activity::from_value` accepts a privileged host snapshot
`{scope,actor,revision,source,state,id}` with `source: routine|initiative` and `state: active`;
do not expose this or registry controls as model tools.

`VisualSpec::compile(&bible, &frame, life, &activity)` takes this exact frame:
`{scope,observer,entity,listener,scene,audience,life_revision,activity_revision}`. `scene` is
the existing coordinator scene `{id,revision,epoch,at,participants,facts,affordances}`.
An absent Life/activity requires a null expected revision; supplied state requires an exact
revision match. The observer is the viewer/perceptor and may differ from the depicted entity.
The listener selects whose relationship affects the expression. The audience is nonempty
and contained in the scene roster. Hosts capture current state before compiling each turn.

At most 64 bible fact rules define `id`, nullable conflict `group`, `priority`, `tokens`,
`props`. All props must exist in some anchor's allowlist. A scene fact contributes only if
its kind is `perceived`, its source speaker is this observer, its audience covers the image
viewers, and it is at most `max_fact_age` ticks old. Future observations/revisions fail.
Only mapped facts whose props are allowed for this entity contribute. The existing scene
contract bounds facts to 32. Global, report, mutual, unobserved, private, aged and unmapped
facts contribute no prompt tokens or key bytes. Raw fact/source text is never interpolated.
Within each non-null conflict group, highest priority wins, then lexical ID. Thus the
example's perceived `shutters_closed` defeats `visible_sun`, even when both are supplied.
Occlusion comes from authored rules; this compiler does not infer geometry or interpret prose.

`VisualSpec::to_value()` returns `{key,spec,inspection}`. `render()` returns ordered positive
tokens, negative tokens, palette, dimensions and seed for an external host adapter. Render
only the selected spec; keep developer inspection out of image-model prompts.

## Content identity and approval

Objects inside the visual contract use lexical key order. Set-like token lists, rules and
selected cues are normalized independently of input order; token text is preserved exactly
(no Unicode normalization). `key` is `visual:` plus lowercase SHA-256 of compact UTF-8
normalized spec JSON. It covers scope, observer/audience, entity/scene/epoch, expression,
pose, visible props/cues, seed, locked style/dimensions and the full normalized bible digest
and revision. Changing a bible without bumping revision still invalidates its keys. Exact
attitude levels and raw scene/life/activity revisions stay in inspection so changes within
one visual slot can reuse approved art. Asset content digest is separate from the spec key.

SHA-256 is implemented originally from the mathematical definitions in
[NIST FIPS 180-4](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf), with standard
known-answer and Python `hashlib` parity checks. This implementation is not FIPS-certified.
`content_digest(bytes)` / `AssetRef::from_bytes` hashes host-owned bytes up to 16 MiB.

Create `AssetRegistry::new(RegistryConfig::from_value(&config)?)` with
`{enabled,capacity,queue_limit,epoch}`. Capacity is 1–128 entries; queue limit 0–32 active
worker tickets. Default config disables generation. An epoch is a host run identifier;
use a fresh epoch when recreating the registry after restart, so old worker tickets cannot
match a new run. Registry persistence/restart import is not provided.

`request(&spec)` returns `{key,state,asset,exact,pending,generation,reason}` immediately:

- Exact `approved` or `pinned`: approved reference, `exact: true`, null generation.
- New enabled missing key with queue space: `candidate` and one generation job
  `{key,ticket:{epoch,sequence},spec}`. Repeated requests never duplicate it.
- Existing candidate: compatible approved fallback and null generation; `pending` also
  includes candidates awaiting approval after their worker ticket is consumed.
- Disabled, full queue, full registry or rejected key: a deterministic reason and no job.
  Rejected keys require an explicit repair before generating a new variant.

A fallback must share entity, bible, scope, viewer/audience, scene/epoch, format and anchors.
Its expression is the requested slot or neutral, and pose the requested pose or idle.
Matching expression/pose slots must also match tokens, including repairs. A fallback
cannot add invisible props or environmental cues, violate new negative constraints,
omit required repair tokens, or show forbidden props. Rank exact expression, exact pose,
then pin status; lexical key breaks ties. A previously approved sunny image therefore
cannot fill a shuttered scene. If no compatible approved asset exists, `asset` is null:
the host renders its immediate placeholder rather than an unapproved candidate. No image
reference is promised before the developer supplies reviewed art.

`submit(key,ticket,uri,digest)` validates and consumes the current worker ticket. It stores
an unapproved candidate, never the default displayed asset. `approve(key,uri,digest)` is a
privileged host/developer action; a submitted candidate must match exactly. It may also
bind an already-reviewed asset directly to a missing key. Approved/pinned refs are immutable
under approve except for exact idempotent repeats. `reject(key,reason)` revokes reuse and
invalidates pending work. URIs are bounded references (`asset://`, `https://`, `http://`,
`file://`, `urn:`); digests are 64 lowercase hex characters. The host verifies actual bytes,
image quality and dimensions; URI/digest strings alone do not verify those properties.

## Inspect, repair, and bind hand edits

`inspect(key)` includes normalized spec, current state, approved/candidate references,
ticket, retired status, and inspection. Inspection records selected/omitted fact IDs and
reasons, scene/life revisions, visible attitude levels, chosen rule, activity snapshot and
bible digest. Repair history is capped at eight patches per lineage. IDs/reasons are
privileged developer diagnostics; raw withheld fact text is never included.

`repair(key,patch)` requires all these fields (empty arrays/null are explicit):

```json
{"replace_tokens":[],"add_tokens":[],"negative_tokens":["wide-brimmed hat"],"forbid_props":["lamp"],"seed_delta":1,"asset":null}
```

Replacement rows are `{from,to}` and may modify whole silhouette, attire, expression,
pose, environment or extra tokens. Style, palette, framing, dimensions and camera remain
locked. Add/replacement/forbid lists hold at most 16 items; negative additions at most 32;
accumulated repair lists at most 64. `forbid_props` removes matching visible props **and**
associated environmental cues and adds `no <prop>` constraints. The caller must remove
other authored prose mentioning a forbidden prop through explicit token replacement.
`seed_delta` rotates the u32 seed with wraparound. An optional
`asset: {asset_uri,digest}` pins a host-reviewed hand edit, including without token changes.
A no-op patch without an asset is rejected.

The complete patch/capacity is validated before mutation. Content changes get a new key;
old pending work and unapproved candidates are invalidated, and stale submissions fail.
The result is `{key,state,invalidated}`; a flattened redirect makes subsequent requests for
the original spec select the repaired variant. Repair applies to that variant, not every
future expression of an entity. Change the bible for global identity/style changes.
Old approved refs can remain compatible fallbacks; retired candidates cannot be approved.
Returning to an earlier key requires a fresh ticket, never the earlier worker's completion.

## CLI, bindings, and evidence

`gamectx local <input.json>` supports `visual-spec` with
`{bible,frame,life,activity}` (`life` is null or an explicit host Life fixture), and
`visual-assets` with `{config,specs,operations}`. The latter replays bounded request,
submit, approve, reject, repair and inspect actions; new conformance inputs are runnable
examples. For a full debug/repair replay, run:

```sh
cargo run --bin gamectx -- local conformance/visual/repair_forbids_prop_and_cue/input.json
cargo run --example visual
```

Persistent `gamectx characters` JSON-lines and Python/Node/C#/C ABI sessions expose
`visual_create`, `visual_prepare`, `visual_submit`, `visual_approve`, `visual_reject`,
`visual_repair`, `visual_inspect`, `visual_drop`. See [KIT](KIT.md) for exact arguments.
These host operations add no image-model tool or native ABI change.

The retained [offline artifact](../../eval/media/visual-results.json) covers 64 original Python/Rust
workflows, including 13 malformed cases, 37 queued generation records, 5 exact reuses and
2 approved fallbacks. Every accepted Rust output matches independent Python JSON bytes.
All 1,026 earlier conformance cases and six life-model artifacts remain unchanged. The
checks execute zero model calls and spend $0; queued records are not dispatched. Provider
tokens, image quality, runner latency and frame rate are unmeasured. Eight typed integration
tests, SHA-256 known answers and 9,000 deterministic input mutations cover the public
selection/repair paths. Persistent binding fixtures exercise approval, stale completion,
pinning, revocation, stale live revision and drop across calls.
