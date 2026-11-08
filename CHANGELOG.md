# Changelog

## 0.4.0a (Cargo `0.4.0-a`), unreleased

- Places (`knowledge::places`, local kind `places`): codex places gain an optional
  `kind`, `within` (the place they sit inside), `tags` and `links` (with a distance and
  a compass direction). There are 112 built-in kinds, such as workshop, city, forest,
  house, castle and cave, and the codex can add its own under `place_kinds`. Entrances
  gain an optional `distance`.
  - `Codex::nearby` returns where a character is (the chain of places they're in), the
    shortest routes to places they can reach (through open entrances and links, with
    each step) and what else is around.
  - All new keys are optional, so existing codexes and engine outputs are unchanged.
  - 30 cases. Rust matches the Python reference byte for byte on them and on 2,000
    randomized codexes.
- World timeline (`knowledge::timeline`, local kind `timeline`): the game's own record
  of what happened.
  - Entries hold when, what kind, a summary, actors, place, things, tags, visibility
    (public, witnessed or private), witnesses, phrases people may ask with, and small
    facts.
  - Queries filter by what a character knows, actor, place, thing, kind, tag, a time
    window and `about` text.
  - Mistaken entries can be retracted (kept, hidden). The game can compact old entries
    into a summary. A full timeline refuses new entries instead of forgetting.
  - Save states accept a `timeline` section.
  - 46 cases plus 2 save-file cases. Rust matches the Python reference byte for byte on
    all of them and on 2,000 randomized workflows.
  - New guide: `docs/guides/WORLD.md` (timeline and places).
- Character sheets gain `bio`: species, age, birthday, profession, anatomy, health
  (status and conditions, visible or hidden), skills with levels, knowledge topics (gated by
  `when`) and family. A new switchable engine feature `bio` answers age, birthday,
  trade, skills, knowledge, health and family questions from it, and adds the facts to
  the model brief. Turns may send live `health` and today's `date`. All wording is in pack
  lines. 31 additive cases. Rust is byte-identical to the Python reference on them and on
  3,000 randomized workflows.
  - **[format]** The engine config echo now lists `"bio":{"enabled":true}`, so 138
    `roleplay-engine` expected outputs gained exactly that text and nothing else.
  - Fixed the Python stage-action reference, which accepted a string as `phrases`; Rust
    already rejected it.
- Added `media::visual::VisualDirector` (phase 2.3) and the `visual-director` local kind:
  tiered art bibles (1–4, falling back to a lower tier that has the entity), format profiles
  (format, framing, camera angle, size), host fact selectors (`when` over typed host
  `vars`), and an approval/repair/fallback workflow over `AssetRegistry`. Saved approvals
  are re-verified through a host callback after a restore, queued repairs apply once per
  base key, and a failed `direct` changes nothing. `checkpoint()` saves approvals and
  repairs. 35 additive conformance cases. Byte-identical to the Python reference on those
  and on 3,000 randomized workflows. `AssetRegistry` is now `Clone`. All earlier visual
  cases are unchanged. [Guide](docs/guides/VISUAL.md#direct-pictures-tiers-profiles-selectors-and-approvals).
- Addressing: distance attenuation and group audiences (15 additive `e_*` cases; the
  other 162 engine cases are byte-identical).
  - A turn's `audience` may be a group of 1–16 distinct ids. A group that includes the
    character counts as speaking to it; otherwise the outcome is `silent` (or a quirk
    gesture) with the group as `audience`.
  - Behaviour change: when nobody is named and the speaker is further than
    `conversational_distance` (default 5) but within `max_distance`, the outcome is now
    `{"route":"addressing","kind":"gesture","reason":"too_far","audience":<character>}`
    instead of a normal reply. Naming the character or a host `audience` is still heard.
  - Rust is byte-identical to the Python reference on 2,500 new randomized workflows.
- **[format]** The English pack's default `model_rules` are now the measured strict brevity
  rule: "one or two short sentences, never more than {sentences}" and "A catchphrase counts
  as a sentence. Use one only when it fits, and never tack it onto the end of an answer."
  Every compiled model `request` changes (30 `roleplay-engine` conformance cases reblessed;
  only the first system message differs). On the v3 held-out probes this took Stheno 8B
  from 74.1% to 88.9% accepted first try and Llama 3.2 3B from 88.9% to 100% ($0, local).
- Added first-class living character embodiments, 3D physical matter, scenery depth, and spatial addressing:
  - Living character profiles: `features` (eyes, hair, skin, scars, build, height), `clothing`, `char_state` (posture, held items, gaze, activity, awareness, exposure, expression), `sensory` (scent, voice_tone), `habits`, `tags`, and `attributes`.
  - 3D physical matter: `dimensions` (`[width, length, height]`), `mass`, `state` (`solid`, `liquid`, `gas`), `material`, `color`, and `texture` on props and spots.
  - Scenery depth: `floor`, `ceiling`, and `exterior` on ambience, codex places, spots, and scenery queries.
  - Spatial addressing and interaction: `Feature::Addressing` (action turns, conversational distance, max distance), bystander detection, vocatives, gestures, line-of-sight occlusion, and cadence quirk processing.
  - 19 new line template variables (70 in total) and expanded matchers in English pack.
  - 32 new conformance cases (`d_*`) with 0 errors across 162 total roleplay engine workflows and 500 randomized differential cases with byte-for-byte parity against Python reference.
- Added living embodiment and physical matter schema extensions to `media::visual::ArtBible`:
  - `ArtBible` anchors optionally accept `features`, `clothing`, `material`, `color`, `texture`, `mass`, `state`, `dimensions`, `floor`, `ceiling`, `exterior`, `tags`, `attributes`, `lighting`, and `atmosphere`.
  - `ArtBible` facts optionally accept physical matter and attribute fields.
  - Optional top-level `lighting`, `atmosphere`, `tags`, and `attributes` on the art bible.
  - `ArtBible` convenience inspection and query methods: `id()`, `style()`, `format()`, `anchor()`, `anchors()`, `describe_anchor()`, and `describe_anchor_with()`.
  - Compiled `VisualSpec` data embeds anchor physical matter and living embodiment fields while preserving strict content-addressed digest stability across all legacy visual conformance cases.
- Added first-class `media::visual::VisualDescriber` for generative AI image models:
  - Compiles ready-to-run positive and negative prompts directly from `VisualSpec` or `ArtBible` anchors with full customization.
  - `DescriberMode`: Comma-separated `Tags` mode (SD 1.5, SDXL, Flux) and fluent descriptive `Prose` mode (Midjourney, DALL-E 3).
  - Target model presets: `DescriberConfig::sdxl()`, `DescriberConfig::flux()`, `DescriberConfig::midjourney()`, and `DescriberConfig::dalle()`.
  - Full customization: configurable delimiters, palette formatters, style positioning (`style_first`), prefix/suffix injection, extra positive/negative tokens, and dynamic JSON deserialization via `DescriberConfig::from_value()`.
  - `VisualSpec::render()` augmented with `"prompt"` and `"negative_prompt"`, plus convenience methods `spec.prompt()`, `spec.negative_prompt()`, and `spec.describe()`.
  - Integrated `VisualDescriber` with provider-adaptive model preset lookup into Lantern Vault web application image generation pipeline (`examples/lantern-vault/web/app.rs`).
- Added character-initiated prop interactions and physical matter perception:
  - `Props` typed getters (`interactions`, `find_interaction`, `material`, `color`, `texture`, `mass`, `state`, `dimensions`, `tags`, `attributes`) and `eligible_interactions` filtering against host `choices`.
  - Inquiries for living embodiment and matter: `ask_posture` ("what is your posture"), `ask_held` ("what are you holding"), and `ask_matter` ("what is the anvil made of", "what material is the bell") with physical matter attributes.
  - Character turns expose `has_posture`, `has_held`, `has_gaze`, `has_clothing`, `has_activity`, `has_material`, `has_color`, `has_texture`, and `has_matter` to conditions, templates, and model brief compilation.
  - Programmatic prop interaction methods: `RoleplayEngine::eligible_prop_interactions` and `RoleplayEngine::propose_interaction`.
- Added native C ABI and CLI export for visual description and specifications:
  - `gamectx_visual_describe` in native C ABI (`bindings/native/src/lib.rs`), providing memory-safe C FFI for compiling prompts from visual specs or ArtBible anchors.
  - `"visual-describe"` operation in `kit::local::process` CLI dispatcher and `"visual_describe"` in `kit::Session`.
  - Implemented `VisualSpec::from_value` and `media::visual::describe_fixture`.

- [api] **Breaking: library modules are grouped by domain.** No behavior, output or
  format changed: every conformance case and frozen evaluation artifact is byte-identical.
  Update imports with this table; `Context`, `Table`, `Turn`, `Value`, `Error`, `Result`,
  `obj!`, `fingerprint` and `fingerprint_u64` stay at the crate root.

  | 0.3 path | 0.4 path |
  |---|---|
  | `json`, `table`, `view`, `formula`, `catalog`, `query` | `data::*` |
  | `context`, `budget`, `manifest`, `request`, `estimate`, `schema`, `signatures`, `sync` | `packet::*` |
  | `memory`, `recall`, `facts`, `lore`, `info`, `inventory`, `observation`, `selection`, `location`, `world`, `temporal` | `knowledge::*` |
  | `life`, `living`, `party`, `archetype`, `cast`, `routine`, `dialogue` | `character::*` |
  | `character` (`CharacterTools`, `CharacterView`, `TurnPlan`...) | `character::tools` |
  | `brain`, `decision`, `schedule`, `playbook`, `outcome`, `replay`, `requests`, `spending` | `runtime::*` |
  | `deployment`, `tune`, `turn` | `deploy::*` |
  | `visual` | `media::visual` |
  | `local` | `kit::local` |
  | `studio` greetings, conversation routing, expression, wording, viewpoints, reactions, deliberation, roleplay actions, commands, message chunks | `character::roleplay` |
  | `studio::{VoiceTurn, VoiceArchive, speech_buffer, native_audio_plan, voice_workflow}` | `media::voice` |
  | `studio::{backends, fallback, ProviderBudget, routing_workflow}` | `media::providers` |
  | `studio::ModelPolicy` | `deploy::model_policy::ModelPolicy` |

  JSON-lines operation names such as `studio-greetings` are formats and are unchanged.
- Repository folders follow the same grouping. `examples/` has domain folders, shared
  `fixtures/` and `lantern-vault/` (formerly `dungeon/`); every `cargo run --example NAME`
  keeps its name. `docs/` has `guides/`, `characters/`, `studio/`, `evidence/` and
  `research/` with an index; `eval/` has ten topic folders (`kit/` added with save files). Frozen evidence keeps its
  recorded paths and digests: `eval/common/provenance.py` maps them to current files and
  checks digests of edited runners/sources against the last pre-0.4 commit, so CI checks
  out full history. Evaluation scripts need Python 3.12+.
- Added `character::roleplay::engine`: `RoleplayEngine`, `EngineConfig` (JSON v1 plus a
  typed builder) and `Feature`. It is one configurable roleplay brain per character that
  combines the Lantern Vault roleplay policies, including chunks, out-of-character notes,
  stage actions, directions, perception, commands, disclosures, recall, greetings and
  social lines.
  - Each of its 12 stages can be switched on or off and tuned.
  - All wording is in a line book: the English pack, which holds all 108 Lantern Vault
    greeting and social lines, plus your own phrases and lines, or `pack: "none"`.
  - It only proposes: actions must be the turn's legal choices, model hand-offs are
    returned to the host, and replies count only after a full `delivered` receipt.
- Added `knowledge::props::Props`, a registry of described props that a location can
  host. A turn's scene lists the props present, with optional state notes; players can
  look at them, see them listed, and use registered interactions that propose host
  decisions.
- [format] Additive JSON-lines `local` kind `roleplay-engine` with 62 new conformance
  cases (`conformance/roleplay-engine/`). No existing case changed. The Rust output is
  byte-identical to the independent Python reference
  (`eval/character/roleplay_engine_reference.py`) on all 62 cases and on 1,500 randomized
  differential cases; both made zero model calls. See
  [the guide](docs/characters/ROLEPLAY_ENGINE.md) and [FORMAT](docs/FORMAT.md#roleplay-engine-v1).
- Added character and world definitions for the roleplay engine (stage 2.1b):
  - `character::sheet::CharacterSheet`: persona, speech style (avoided words are never
    spoken), services, goals, needs, relationships, faction memberships, secrets (spoken
    only when their `when` holds; otherwise a cover story or a refusal) and opinions.
  - `knowledge::codex::Codex`: places with spots, conditional ambience and house-rule
    norms, entrances between places (open/closed/locked/blocked/hidden, one-way, sound
    carrying through), factions with stances, timed events and staged quests.
  - `data::conditions`: shared `when` conditions over template variables and host flags
    (`state.<id>`).
  - 12 new switchable engine features: `content`, `entrances`, `scenery`, `persona`,
    `services`, `secrets`, `opinions`, `relationships`, `factions`, `events`, `quests` and
    `goals`. Turns accept `entrances`, `needs`, `quests` and `state`.
- [format] `roleplay-engine` gains 42 conformance cases (104 in total). The 62 stage 2.1
  cases were re-blessed and reviewed by hand. Their outcomes are unchanged; only additive
  fields grew: the config echo (12 feature entries, `sheet`, `codex`) and a listener
  `revealed` list. The two restore inputs that embed a checkpoint gained that list too.
  The kind is unreleased, so no released output changed. The Rust output is
  byte-identical to the Python reference on all 104 cases and on 2,800 randomized
  differential cases covering every new route.
- Added `kit::save`: `GameData` (a `gamectx-data` file of authored definitions: codex,
  props, lore, cast packs, sheets, engine configs, host data) and `SaveFile` /
  `SaveState` (a `gamectx-save` file holding any number of named states). Each state
  records its parent, `saved_at` value and digest and is organized as
  `characters/<id>/<kind>` and `world/<kind>`. Every section is validated by its own
  parser. Packing is canonical, FNV-1a digests catch edits and a save made against other
  game data, and engine memory is restored against the matching engine config.
- [format] Additive JSON-lines `local` kind `save-file` with 22 new conformance cases
  (`conformance/save-file/`). The Rust output is byte-identical to
  `eval/kit/save_reference.py` on all 22 cases and on 2,932 of 3,000 randomized
  workflows. The other 68 are documented: Rust also deep-validates nested lore and
  memory data that the reference only shape-checks. See
  [FORMAT](docs/FORMAT.md#game-data-and-save-files-v1).
- Added model requests and replies to the roleplay engine (stage 2.2):
  - A `model` outcome now carries `request`, a complete OpenAI/llama.cpp-shaped chat
    request. It holds the pack's `model_rules`, a character brief (persona, speech
    style, relationships, factions, content rating) and the turn's situation (scene and
    senses, props, exits, what the character knows about this listener, goal, need,
    events, recent replies), trimmed to `brief_chars` in a fixed order.
  - Locked secrets never enter a request.
  - The new `reply` op validates raw model text: an exact `{"speech"}` object, length,
    markup, sentence count, out-of-character phrases, avoided words, locked secret text,
    host ids and repeats. A rejection names its reason and changes nothing. Open
    requests are checkpointed as `asked`.
  - New `features.model` settings: `max_tokens`, `max_sentences`, `max_chars` and
    `brief_chars`. Config `model_rules` replaces the pack's rules.
- [format] `roleplay-engine` gains 26 conformance cases (130 in total).
  - Existing cases were re-blessed and reviewed by hand. Their outcomes are unchanged;
    only additive fields changed: the config echo (`model` settings, `model_rules`),
    the checkpoint `asked` list and the `request` on `model` outcomes. The two restore
    inputs gained `asked`.
  - `save-file` cases were re-blessed for the `asked` list and the resulting digests.
  - Both kinds are unreleased.
  - Rust is byte-identical to the Python reference on all 130 cases and on 3,200
    randomized differential cases with raw model replies. `tests/robustness.rs` fuzzes
    6,000 mutated replies.
  - Local model evaluation, $0: the full numbers are in
    [the guide](docs/characters/ROLEPLAY_ENGINE.md#model-evaluation).
    - Llama 3.2 3B met every predeclared expectation: 92.6% accepted and fact hits 61.1%
      versus 11.1% without the brief.
    - Stheno 8B missed the acceptance bar: 74.1% accepted against ≥ 85%, every
      rejection being too many sentences. Its grounding gain was the same.
- Added a predeclared 8B follow-up run, eval only (`roleplay_model_eval.py --probes v3`).
  No library or format change. On held-out probes Stheno 8B goes from 74.1% accepted to
  88.9% with one host retry, or 88.9% first try with stricter brevity `model_rules`;
  Llama 3.2 3B was ≥ 88.9% in every variant. Two expectations failed and are recorded.
  See [the guide](docs/characters/ROLEPLAY_ENGINE.md#8b-follow-up-v3).

- [api] Add bounded least-recent tale seed selection and ordered fictional tale parts.
- [format] Add sixteen expression cases; optional structured example tale responses and
  zero-offset `blocked` delivery retries preserve legacy arrays and prior conformance.
- Studio: current-place lookup before generic fallback, explicit NPC sight questions,
  twelve heard-history-aware plot seeds per NPC, cleared Thinking fences, one live voice
  owner across tabs and recoverable unheard audio. See docs/SCENE_SOCIAL_REPAIR.md.
- Document the broader social-character feature/gap map in docs/SOCIAL_CHARACTER_GAPS.md.

- [api] Add bounded character expression, explicit disclosure parsing, conservative
  sentence novelty checks and source-bound AI wording through the generic bindings.
- [format] Add 60 expression and 14 wording conformance cases; optional `ai_wording`,
  personality interests/story seeds preserve old configs and existing expected outputs.
- Studio: connected AI wording for greetings, acknowledgements and recall; matching
  contextual cache, AI-written short-scope offers, attributed fictional tales, scoped
  corrected names/preferences, OOC notes and perceived senses. Raw local failures and
  follow-ups retained; see docs/STUDIO_EXPRESSION.md and docs/ROLEPLAY_RESEARCH.md.


- [api] Add bounded `studio::GreetingMemory` for confirmed per-NPC/listener encounters,
  introduction state and contextual first/continuing/return/location/gap greeting plans.
- [format] Add 26 greeting-memory and two social-routing cases; optional example
  encounter checkpoints preserve old saves and existing conformance outputs.
- Studio: arrival/Approach stage directions replace fabricated player greetings; typed
  roleplay travel obeys game permissions. Greetings use current perceived place, trust
  and urgency with zero model requests; guest travel remains host controlled.
  See docs/CONTEXTUAL_GREETINGS.md for receipt, migration, bounds and offline evidence.


- [api] Add bounded `studio::conversation_route` hints for optional social speech.
- [format] Add an optional bounded personality `reason` and 22 `studio-conversation` conformance cases under `social-routing`;
  preserve existing conversation/evaluation outputs.
- Fix personality-enabled greetings/wellbeing/follow-ups falling into shop speech enums.
  Use NPC/player perspective, transient moods, heard-topic context and rejection of
  repeated or engine-metadata replies; retain strict factual paths and 0B fallbacks.
  See docs/STUDIO_CONVERSATION.md for host/browser and paired local 3B evidence.


- [api] Add `studio::ReactionPolicy` for bounded generic host capability selection and
  fresh authorization under attitude/personality/resource/reason gates.
- [format] Add 22 reaction conformance cases; optional example transfer journals preserve
  old saves. Studio adapters conserve coins/items, isolate debts by borrower, reserve
  repayment capacity and replay between actions; no extra model calls.
- Studio: conditional favor controls, editable host rules, factual/subjective authority
  unchanged, pending reply hiding also covers guest browsers and stale polls.
  See docs/CHARACTER_REACTIONS.md for 12 host and six Firefox workflows and limits.


- [api] Add opt-in bounded `studio::Viewpoints` with host complete-heard confirmation,
  actor/listener/topic isolation, immutable reuse and checkpoint recovery.
- [format] Add twelve `studio-viewpoints` conformance cases; optional example viewpoint
  checkpoint extension preserves old saves and existing conformance outputs.
- Studio: editable personality turns for dreams/hobbies/preferences, atomic persistence,
  strict factual path retained, Thinking UI hides old/provisional replies, instant fallback.
  Fresh local 3B three-question holdout: 3/3 relevant/accepted versus 0/3 baseline;
  see docs/CHARACTER_VIEWPOINTS.md for raw artifacts and limits.


While the version is `0.x` or a pre-release, any release may change the API,
command-line options or packed formats. Every entry carries one label so projects
know what an upgrade costs:

| Label | Meaning | What projects using gamectx should do |
|---|---|---|
| **[format]** | Packed output or accepted input changes; one or more `conformance/` expected files changed. | Update ports, rerun conformance, and recheck model decisions that depend on the changed shape. |
| **[api]** | Rust library or command-line interface changes; output is the same. | Update calling code. |
| **[internal]** | Faster, safer or clearer with identical behavior; conformance files untouched. | Upgrade freely. |
| **[docs]** | Documentation only. | Nothing. |

## Unreleased — v0.3.1a (Cargo `0.3.1-a`)

- **[api]** Adds opt-in Lantern Vault loopback studio: RPG/party controls, live Life,
  promises/rumors/habits, approved art/debug repairs, independent text/image/TTS/STT
  providers, actual tier/model maps, bounded single-worker scheduling and Unicode
  delivery/recognition barge-in. Paid requests share durable microdollar reservations;
  cap defaults to zero. Core/default host remain std-only; optional HTTPS uses system curl.
- **[format]** Adds 32 studio policy conformance cases. All 1,090 prior cases and native
  ABI remain unchanged; passive visual lookup/render and effective Life projection are additive.
- **[docs]** Adds editable profiles/art/UI and retained offline checks: 32 Python/Rust
  workflows, 85 HTTP checks, five fake-browser scenarios, eleven new Rust studio tests and
  6,000 policy mutations. Zero real provider calls / $0 API; physical voice accuracy,
  image quality, online integration, model latency and FPS remain unmeasured.


- **[api]** Adds opt-in zero-dependency visual specs and asset registry: locked art bibles,
  live attitude/emotion and active-pose slots, perceived-only cues, original SHA-256 content
  keys, approved fallback reuse, bounded worker tickets, inspection, atomic repair and
  hand-edited pins. Persistent host operations work through existing bindings; no generator.
- **[format]** Adds 64 visual compiler/registry conformance cases, including 13 malformed
  inputs. All 1,026 earlier cases and native ABI remain unchanged.
- **[docs]** Adds a tested editable visual example and 64 independent Python/Rust workflows:
  37 queued metadata jobs, 5 exact reuses, 2 approved fallbacks, 0 model calls / $0 API.
  Image quality, provider tokens, latency and frame rate are unmeasured.


- **[api]** Adds optional CastPack validation and a deterministic LivingWorld over existing
  archetypes, Life, routines, lore and initiative: bounded attributed rumors, chronological
  fractional group effects, host-confirmed habit badges and scene atmosphere. Adds opaque
  async speech tickets with immediate authored fallback and one-use validated upgrades
  before delivery. No provider/executor dependency or additional model calls.
- **[format]** Adds 54 living-world/pack/speech conformance cases and host-only local adapters;
  all 972 prior cases and the native ABI remain unchanged.
- **[docs]** Adds an editable one-file cast, tested frame/lifecycle example, 54 Python/Rust
  workflows and a retained 1,000-frame host-work trace. 0 model calls / $0 API; no claim
  about semantic model quality or whole-game 60 FPS.

- **[api]** Adds opt-in composable ambient/service/quest/companion blueprints over existing
  modules, current-attitude gates, heard-only multi-party perspectives, shared floor quotas,
  one-use spending-backed grants and explicit spotlight routing. No automatic model/tool work.
- **[format]** Adds 68 archetype/party conformance cases, including malformed inputs;
  all 904 prior cases and native APIs remain unchanged.
- **[docs]** Adds an editable four-archetype/six-participant example and offline parity/
  byte measurements (3,210 trusted / 2,708 untrusted request bytes; 0 model calls / $0 API).
  Token counts, model-tier accuracy and social quality remain unmeasured.

- **[api]** Adds opt-in speech compiler v2: focused/unknown-state grounded policy, explicit
  speaker/subject factual bindings, direct current utterances and bounded sentence-wall
  diagnostics. Includes coordinator/session operations and a tested Rust example.
- **[format]** Adds 15 compiler/sentence conformance cases, including errors; all existing
  889 cases/expected files remain unchanged. The legacy life context and v1 artifacts stay frozen.
- **[docs]** Records a fresh v1-style/v2 local speech pair: terminal endings 5/8→8/8,
  suspected schema-wall cutoffs 3/8→0/8; weather/stock hallucinations remain. 48 local calls,
  $0 API, four regression and four new holdout families. No universal semantic guarantee.

- **[api]** Adds optional life definitions, evidence-based appraisals, topic/scene continuity,
  partial delivery acknowledgements, durable scoped state/erasure, and routine-linked
  initiative driven by attitudes, accepted promises and event tokens. Persistent host
  controls and composite checkpoints support Python, Node/TypeScript and native/C#.
- **[api]** Adds bounded focused/expanded character context from dialogue workload hints,
  stable-prefix rendering, an original dungeon `--life-demo`, explicit `--life-tools`,
  and opt-in `--dialogue natural-scaled`. Existing defaults/native ABI are preserved.
- **[internal]** Replaces serialize/parse runtime validation with allocation-free in-memory
  checks and preallocates JSON output. Life updates preflight exact size without cloning
  whole history. Seven paired 128-event/appraisal trials: 176,881,587→7,338,259 ns median;
  2,517,745→22,025 allocator calls. Same final bytes; synthetic CPU workload only.
- **[format]** Adds 47 conformance cases; all 842 prior cases remain unchanged.
  New contracts are versioned separately. No existing model request is silently rewritten.
- **[docs]** Records 221 independent workflows and a retained failed 24-call local speech
  diagnostic: 8/8 valid shapes do not establish grounding or conversational usefulness.
  New API spend $0. See `docs/LIFE.md` and `docs/LIFE_EVAL.md`.

- **[api]** Fixes conversation shortcuts executing negated/conditional/quoted commands,
  capped time summaries hiding older conflicts, and restore accepting pre-window
  lifecycle states or changed same-revision frames. Targeted probes 15/57→57/57;
  900 independent engine steps match. No existing conformance output changes.
- **[api]** Adds optional typed source time assessment and a privileged session read,
  keeping exact instrument readings and reported claims distinct from simulation truth.
- **[api]** Adds opt-in example `--count-cache 0..64` (default 0), verified request-count
  receipts, pinned transport settings and whole-turn allowance/output validation.
  Production cache test: 18→12 calls, 3,630→2,420 input tokens over nine turns, same
  paired answers; six repeated-turn median 1,780,275→1,637,055 microseconds. Small warm
  fixed-request diagnostic, not general gameplay latency or model-accuracy improvement.
- **[format]** Adds 15 time/assessment conformance cases including errors; all 827 prior
  outputs unchanged. Existing model schemas, defaults and immutable native ABI preserved.
- **[docs]** Records 159 local Qwen calls including tokens/preflights and $0 API spend.
  Time classification: 25/32 correct despite 32/32 valid JSON; focused 12/16 versus
  expanded 13/16, with lower input/latency. Retains all misses and the earlier adapter-only
  timing pilot. No semantic guarantee from compact prompts or grammar. See [ADVERSARIAL_EVAL](docs/evidence/ADVERSARIAL_EVAL.md).


- **[api]** Adds explicit task observation selection with protected source tokens,
  next-wake timers and host planning-event dependencies, including persistent helpers.
- **[internal]** Preflights history time before atomic timeline updates, avoiding full
  history copies. Seven paired 1,000-update synthetic trials: median 328,934,760→6,986,127
  ns, identical final state. This is host work, not model-turn latency or model RAM.
- **[format]** Adds nine selection/timer cases; all 818 previous outputs unchanged.
  Adds three privileged host operations; no new model tools or default exposures.
- **[docs]** Follow-up checks: 54 independent workflows, expanded 52 binding fixtures,
  10,000 selector mutation iterations. Noisy observation JSON 2,102→371 bytes retains a
  required older source; no provider-token or accuracy claim. Model/API spend $0.

- **[api]** Adds opt-in shared/character routine policy, bounded deterministic timelines,
  scoped actual history and source-based aging time knowledge; reuses work scheduling,
  evidence source validation, bounded choices and character/session dispatch. Engines
  retain sensors, calendars, completion and action execution. Defaults remain disabled.
- **[format]** Adds 21 routine/time fixtures and privileged persistent host operations;
  all 797 previous outputs remain byte-identical. No immutable project/native ABI change.
- **[api]** Adds individually registered `routine_status` and `activity_propose`, explicit
  model-call reservations, bounded save/restore and Python/Node host helpers. Adds the
  offline editable dungeon surface-annex demo and tested minimal Rust example.
- **[docs]** Records 69 independent offline Python/Rust workflows, 43 cross-language
  session fixtures and 20,000 added mutation attempts. No new model/token/latency/RAM
  results; historical evaluations remain intact. Model/API spend $0.

- **[api]** Adds actor-bound read-only location/world lookup, `look_around`, scoped
  character dispatch and task-focused full-request selection. Attention and context
  budgets are independent; provider/model IDs have no parameter-size ceiling.
- **[format]** Adds 50 conformance fixtures for location, world, attributed quote
  rendering and synthetic character plans; all 747 prior fixtures remain unchanged.
  Adds persistent `character_*` host operations using existing version-1 envelopes.
- **[api]** Adds persistent native sessions without changing the immutable project C
  ABI; Python/Node character helpers and no-project CLI startup are available.
- **[api]** Fixes example identity, stock/topic routing, current scene and attributed
  name recall; executes explicit legal player commands using actual engine receipts.
  Counts literal letters locally. Player input uses `you>`. NPC time access is explicit:
  no clock, instrument reading, approximate schedule or attributed possibly stale report.
- **[docs]** Records 78/78 exact candidate routes across six seeded states (previous
  strict route metric 0/78), 14 time workflows and 170 new independent byte comparisons.
  These are local checks, not model accuracy. Retains three unsuccessful speech pilots:
  643/225/241 input tokens per question; smaller prompts still truncate or invent facts.
  All new API spend $0. See docs/CONVERSATION_PATCH_EVAL.md.

## v0.3.0a — 2026-11-06

Cargo package version: `0.3.0-a`.

This release adds reusable dialogue/information tools, participant-scoped ownership and
interactions, authored lore and size-and-task workflows. The original 496 conformance
cases remain unchanged; 251 additional cases bring the suite to 747. Rust stays at
1.82 with zero third-party dependencies. Natural speech remains experimental; new
conversation-tool selection has deterministic tests but no live-model accuracy result.

- **[internal]** Adds cross-language release regression coverage for all 126 new
  information/inventory/conversation workflows: Python CLI/native 252, Node 126, C#
  native 126, totaling 504 checks. CI retains the new fixtures; Rust formats unchanged.

- **[api]** Adds individually selectable conversation tools for witnessed events, reported
  promises and relationship notes, original-utterance reference resolution and exact
  recipient-bound offers. Supports explicit participant IDs/private scopes; the dungeon
  remains single-player. Progress, consent and execution stay host-controlled.
- **[format]** Adds local `interactions` workflow, 47 conformance cases and 87 independent
  Python/Rust comparisons; all 700 previous cases unchanged. Adds 30000 mutation attempts
  and a tested API example. Existing format contracts are preserved.
- **[docs]** In a 120-event fixture, 80 authorized records occupy 8589 bytes; two selected
  records 231. Eight optional schemas total 3127 bytes. Byte-size evidence only; no live
  tool-selection accuracy/latency/RAM claim, no automatic defaults and $0 API spend.

- **[api]** Adds optional model-callable informational notes and read-only inventory
  lookup, source-bound reported claims, multiple owner IDs, unique instances, tag/count
  rows and atomic host-validated integer transfers. Notes never promote themselves to
  authoritative state. Adds a runnable information-tool example.
- **[format]** Adds local `info`/`inventory` workflows and lore version 2 structured values;
  39 new conformance cases, all 661 previous cases unchanged. Independent parity,
  30000 new mutation attempts, documented bounds and exact example byte costs.
- **[api]** Fixes dungeon stock paraphrases and purchase-listing routing. Adds experimental
  `--dialogue natural` with typed relevant facts and budgeted speech, preserving defaults.
- **[docs]** Stock pilot 10/12; 96 offline stock workflows and 72 natural request comparisons.
  Two natural diagnostics accept 20/20 speech shapes each but retain factual failures;
  narrowed structured requests total 20829→9962 bytes. Natural mode stays opt-in. All
  token, generation and offline runs $0 API; no larger model loaded.

- **[api]** Adds bounded authored-lore loading/selection and source-bound factual answer
  cards, actor/grant filtering, required-fit errors, live-over-lore field precedence,
  exact attributed player quotes and live source/revision validation. Rust stays
  dependency-free; old APIs/default outputs remain compatible.
- **[format]** Adds local kinds `lore` and `answers`, 28 conformance cases; all 633 prior
  cases unchanged. Independent Python/Rust: 108 workflows, plus 60 native example
  request workflows, 11 front-end regressions and 30000 added text mutations.
- **[api]** Dungeon AI conversations now load authorized lore and use factual source
  selection before free-form dialogue. Adds editable world.json, `--world`,
  `--dialogue sources|legacy` and example knowledge host operations. Existing generated
  speech stays bounded at 240 characters; new host-rendered facts/quotes allow 512.
- **[docs]** Preserves every local pilot: native 11/12, first fresh questions 17/20,
  tuned rerun 20/20, additional questions 7/8, final local disclosure repair 1/1 without
  a model call. Core byte fixture 1669→145 selected lore bytes; no lossless/token claim.
  Timing/usage and a separate warm RSS sample are recorded with their limits. All new
  runs $0 API; no larger models loaded. [Report](docs/evidence/LORE_EVAL.md).

- **[internal]** Preserves 11 user-reported dungeon interaction sequences / 26 turns
  with separate manual grounding/recall/transaction annotations. Offline native/Python
  audit: 17 trade requests, 16 matching local receipts, nine supplied speech packets,
  two reproduced purchases; zero model calls. Source identity/usage remain unknown.
- **[internal]** Adds regressions proving that forgetting dialogue preserves purchased
  inventory and that invented assistant biography is never registered as player quotes
  or host facts. All 633 core cases unchanged; runtime prompts/behavior unchanged.
- **[docs]** Records remaining fabricated player history, home, weather and tonic
  purpose alongside correct transactions and name recall. Schema/citation acceptance
  is not a semantic score. New audit spend $0 API. [Report](docs/evidence/DUNGEON_FEEDBACK.md).

- **[api]** Extracts reusable scoped conversation memory/reply validation, configurable
  lexical entity references and one-use confirmed action proposals into the library.
  The dungeon now delegates to those APIs; current facts, pricing and callbacks stay
  host-owned. Existing dungeon/model audit outputs remain identical.
- **[format]** Adds local kinds `conversation`, `references`, `confirmation` and 38
  conformance cases. All 595 prior cases unchanged. Independent Python/Rust: 78 workflows;
  30,000 new text mutations and an adversarial repeated-alias work-bound test.
- **[api]** Fixes the merchant purchase loop: exact unaffordable-price/coins/shortfall
  receipt, affordable yes/no offers, one-use confirmation, cancellation and stale checks.
  Generated prose cannot approve or create a purchase. Separate example reference:
  66 workflows / 144 trade requests across six seeds; 11 front-end regression tests.
- **[docs]** Documents existing inventory counts/sums and catalog/memory tags, shared
  conversation APIs and the host responsibilities they preserve. New testing makes
  zero model calls and spends $0 API; no new model-accuracy claim. [Guide](docs/guides/CONVERSATION.md).

- **[api]** Adds editable size-and-task workflow profiles for 1B/1.5B/3B/8B/30B/70B,
  plus opt-in direct current-request rendering. All generic tiers remain unmeasured;
  hints never authorize dropping required content or loading larger models.
- **[format]** Adds local kinds `workflow` and `render-direct`, with 30 new conformance
  cases including errors. All 565 prior expected outputs remain byte-identical.
- **[api]** Adds human-play AI characters, scoped verbatim player memory, one-use
  revision-bound speech tickets, bounded natural-language interpretation, phrase-aware
  entity matching and explicit clarification to the dungeon example. Exact rules retain
  control over all world changes; generated dialogue can still be semantically wrong.
- **[internal]** Adds 13 social fixture sets, 43 independent Python/Rust social workflows
  (507 accepted/11 rejected operations), 360 entity workflows across six seeds, seven
  fake-generation UI tests and 30,000 social/entity mutation attempts. New core parsing
  kinds also enter the existing local mutation suite.
- **[docs]** Retains three local Qwen cohorts: standard/extra-guidance/focused intent
  2/4, 1/4, 2/4; generation input 7468/8240/6767 tokens, output 525/610/308.
  Host-slot pilot 7/8, then phrase-aware regression pilot 8/8, including four local
  deferrals and four correct model choices. Not independent held-out accuracy or a
  controlled latency/RAM benchmark; all local runs $0 API. [Evidence](docs/guides/WORKFLOWS.md).

- **[api]** Adds the playable `dungeon` example: six rooms, seeded key/prices/health,
  deterministic shop/combat, atomic revision-checked actions, persistent JSON host,
  scene/codec packets, saved replays and optional local-only model-player tooling.
- **[internal]** Adds 23 separate example replay conformance fixtures; all 565 core cases unchanged.
  Independent Python/Rust: 103 workflows (101 accepted/two rejected), 10,000 added
  mutations. Six scripted seeds escape; these are rule tests, not model accuracy.
- **[docs]** Records 240 checkpoint observations: 80 each raw/scene/named, request
  JSON bytes 275,304 / 261,304 / 206,347. Raw drops optional archives at 55 checkpoints;
  these are byte measurements with different history, not isolated token savings.
  First local Qwen smoke exposes legal-action repetition without quest progress.
  Both local token/gameplay runs cost $0 API; [testbed guide](examples/lantern-vault/README.md).

- **[api]** Adds opt-in host intent boundaries and correction context, typed prepared
  query tools with AST-bound arguments and recipe-bound execution, dependency-complete
  evidence selection, a host full-render counting handshake, local rule/procedure/brain
  routing, readable decision names and split recorded-configuration comparison.
- **[format]** Adds seven local workflow kinds, 69 new conformance cases (including
  errors), all 496 prior cases unchanged. Independent Python/Rust: 179 workflows,
  152 accepted and 27 rejected; 30,000 new text mutations. Legacy defaults unchanged.
- **[docs]** Local Qwen one-token counts for 2/4/8 choices: 158→118, 224→174, 344→274.
  Matched-order 12-scene codec probe: 8/12→11/12 correct, input 3,865→3,285 and output
  178→111. Small synthetic cohort; codec stays opt-in. Offline packet 419→49 bytes
  retains declared dependencies. New API spend $0; [details](docs/guides/OPTIMIZATION.md).

## v0.2.0a — 2026-11-06

Cargo package version: `0.2.0-a`.

- **[api]** Adds full-request `RequestBudget` with renderer/tokenizer callbacks and explicit
  output/safety reservations, plus `Turn::build_request`; exact/estimated counts remain
  distinguishable and existing packet builds retain their behavior.
- **[api]** Adds scene-scoped whole-dialogue retention, required-fact hydration, bounded
  legal-pair decisions with one retry/live fallback, deployment identity/profile checks,
  and separate full-turn telemetry. Local filtering/arithmetic remain in existing tools.
- **[api]** Adds workload guidelines and editable starting profiles for 1B, 1.5B, 3B, 8B,
  30B and 70B. Generic tiers are explicitly unmeasured hints, not capability or RAM limits.
- **[format]** Adds opt-in local workflow contracts: 48 new cases, all 448 previous
  cases unchanged; 148 Python/Rust comparisons (132 accepted, 16 rejected), 20,000 new
  text mutations. CLI `local`, kit bridge `local`, and bounded `query - SQL` stdin support.
- **[docs]** Adds actual Qwen 3B Q4_K_M local evaluation on four physical cores / roughly
  8 GB: raw/scene/schema pipeline 26/30, 30/30, 30/30; median complete-turn microseconds
  19,226,226 / 8,651,346 / 5,451,699. Six-scene guidance/tool/narration pilot, sampled RAM
  and swap, strict local-only runner and offline cohort audit. Total API spend $0.
  See [the report](docs/evidence/LOCAL_MODEL_2026_11_06.md); no general model-size accuracy claim.

- **[api]** Adds optional `kit` Rust registration/builders, named recipes, visibility/freshness
  validation, recipe-scoped SQL, brain feature-unit reuse and validated host callback dispatch.
- **[format]** Adds version-1 project/snapshot/inspection/bridge/evaluation contracts: 47 new
  conformance cases, all 401 previous expected outputs unchanged. Independent Python/Rust
  comparison: 144 workflows (113 accepted, 31 rejected); 20,000 new text mutations.
- **[api]** Adds CLI `init`, `doctor`, `inspect`, `serve`, `evaluate`, `kit`, optional native
  C ABI and Python, Node/TypeScript and C# adapters. Bridge checks: 265 Python/native,
  8 Node, 59 C# fixture replies plus two smoke checks; actual engines untested.
- **[internal]** Reuses prepared recipe registration. Seven paired synthetic release trials:
  1,000 turns 146,130,805 → 78,931,400 ns versus reparsing registration each turn. Not a
  model-latency or universal runtime speedup. New API spend $0; [integration guide](docs/guides/KIT.md).

- **[format]** Table and catalog `relevant_to` recognize namespace/dotted/path identifier
  components (`: . / - _`); full cells and underscore segments still match. Lexical
  regressions 2/10 → 10/10; four catalog cases. No existing expected output changed.
- **[format]** Adds query-plan inspection and optional explicit host-shape/bounded-run
  conformance workflows, plus exact Unicode-ID/large-number query selection: 35 new
  cases total, all 366 previous cases byte-identical.
- **[api]** Adds `Query::{plan,require_shape,run_bounded}`, `query::Shape`, and CLI
  `query --explain SQL`. Shape checks are structural host requirements, not intent or
  authorization checks; default tools and sessions retain their formats.
- **[api]** Adds opt-in `query::Guidance::{Standard,ExplicitSelection}` and CLI
  `query --guidance PROFILE`. Exact measured supplemental system text; default empty.
  Paired six-state check: 3B 46/48→48/48, 24B 47/48→46/48; input +22.0%.
  Four profile conformance cases; unknown profiles fail. Token check $0.0012702 plus
  accuracy $0.0281033125 = $0.0293735125, within caps $0.01/$0.08.
- **[internal]** Query execution borrows rows, streams aggregate values and avoids
  temporary scalar-result vectors. Five synthetic paired release fixtures: 4.6–32.5×
  faster parsed execution. 2,024 baseline result/error comparisons, 3,000 generated
  Rust/Python queries and 516 plan/bounds/shape checks agree; 20,000 added mutations.

## 0.1.0-alpha.4 — 2026-11-06

- **[docs]** Packages the latest model report and install instructions. Rust runtime/API
  behavior and all 366 conformance cases remain unchanged from alpha.3.
- **[docs]** Publishes a fresh alpha.3 model check: six states/three schema families,
  48 questions/model on the two smaller default models. Full JSON 27/96, packed tables
  27/96, schema + query 91/96; completed-exchange input 1,786.792 / 1,252.125 / 916.490.
  Includes all responses, usage, five remaining errors and an interrupted-run audit.
  Total usage-priced spend $0.05529491875; released library output stays unchanged.
- **[internal]** Adds a capped evaluation runner with conservative per-request reserves,
  one transport attempt, immutable result files, resume without repeated completed cases,
  and offline native/Python replay. CI never makes paid requests.

## 0.1.0-alpha.3 — 2026-11-06

- **[docs]** Adds exact-version crates.io installation/dependency instructions for the
  library and CLI, alongside the matching Git tag.
- **[api]** Adds opt-in `RecallIndex`, `Playbook`, `OutcomeTracker`, `TemporalWindow`,
  `BrainBundle`/`PreparedFrame`, `RequestGate`/`RequestContext`/`RequestTicket` and
  `SpendingLedger`. No new default model tools or network/file I/O. Procedures require
  current native validation; outcome labels require execution/exposure and single use;
  prepared frames and request tickets are instance-bound and invalidated on change.
- **[format]** Adds strict recall configuration/reports, procedure definitions/drafts,
  outcome/temporal reports, module-bundle envelopes, request contexts and ledger state.
  Adds 40 `upgrades` conformance cases; all preceding 326 remain byte-identical.
- **[docs]** Offline recall work: 100,000 scan visits→100 postings for 100 selective
  queries/1,000 records. Shared module evaluations 2,400→1,300 per 100 frames; 3,600
  held-out predictions unchanged. Paired 12,000-candidate release medians:
  729,906→631,514 ns (0 hidden), 3,341,042→2,014,761 ns (16 hidden). 100/100 repeated
  procedure matches; 100 observed labels, zero unexposed/replayed labels. 283 reference
  workflows, 54,000 new text mutations; $0 API, no new model-accuracy claim.
- **[api]** Adds `facts::FactRegistry`, host/user authority, explicit pins, exact typed
  claim checks, versioned dependencies, privacy erasure and `Turn::facts`. Native
  registration stays separate from model proposals. Source changes invalidate derived
  facts transitively; decimal equality preserves large integers without float rounding.
- **[format]** Adds versioned fact-registry state, references, claim reports and bounded
  fact packets. Adds 19 `facts` conformance cases; all earlier 307 remain byte-identical.
- **[docs]** Offline fact fixture: 128/128 registered assertions checked; 1,024-byte cap
  retains eight in 927 bytes with 120 dropped. 119 Python/Rust differential workflows
  and 20,000 added text mutations; no new API spend or model-accuracy claim.
- **[api]** Adds versioned `FeatureSchema`, `Features`, `BoundBrain`, opt-in
  `StableBrain`/`Selection` and deterministic `Brain::diagnose` reasons. Bound restores
  require full expected semantics. Stable choices retain raw prediction diagnostics.
- **[api]** Adds `replay::Recorder`, replay execution and `gamectx replay`, with bounded
  traces, explicit snapshots, labels and recorded latency summaries.
- **[api]** Adds `query::describe_with` and CLI `query --schema ... --metadata ...` for
  validated host meanings/units/categories. Three fresh states: executed queries 36/60
  → 45/60 on the two smaller models; token-only inputs 205/190 → 365/352. Spend
  $0.0053331125 including the token check. Default tools/descriptions stay unchanged.
- **[api]** Adds optional `memory::Journal` and `EvidenceMemory`/`EvidenceSnapshot`.
  Facts track provenance, scope, expiry and supersession. Hypotheses cite supplied
  native sources, inherit privacy, expire with sources, reject stale/foreign views and
  cannot inflate confidence or renew lifetime by repeating the same evidence.
- **[format]** Adds bound-brain envelopes, feature schemas, metadata meanings, diagnostic
  replay, journal and evidence/hypothesis state formats. Adds 46 `integration` cases;
  all previous 261 cases remain byte-identical. Existing memory and brain state formats
  stay unchanged.
- **[internal]** Fitting byte-budget packets collect lengths while serializing, avoiding
  duplicate length walks. Paired fitting builds: 9,171,479 → 5,905,360 ns/1,000;
  mixed 8,822,089 → 5,708,635 ns. Pressure allocations rise 153 → 172/build; measured
  runtime improves. New benchmark reports transient peak memory separately from timing.
- **[docs]** Three fresh noisy trajectories: switches 355 → 15, zero missed urgent
  switches or ineligible choices; utility regret 2,281,674 millionths/720 frames.
  Stability is optional and trades immediate utility for consistency. Evidence tests
  retain two native observations, confidence 0.6 and expiry 30 across repeated hypotheses.

- **[api]** Adds allocation-free `Brain::run_into`, reusable `Prediction` and fixed-point
  `SCALE`; existing JSON inference uses the same kernel and keeps its output.
- **[api]** Adds disposable `TrainingJob`s for single-rate, selection and host-policy
  selection. Sample slices preserve training order/results; finishing rejects stale
  state and dropping cancels without mutation. Adds `schedule::Scheduler` for bounded
  event/urgency/refresh scheduling.
- **[api]** Adds `PreparedSection` and reusable packet assembly; `Context` retains encoded
  sections. Exact byte-budget fitting avoids repeated packet clones/serialization.
  `json_len()` counts exact serialized bytes without allocating. Generic fitting
  predicates preserve their complete-candidate behavior.
- **[format]** Adds `train_select_host` workflows with per-validation eligibility and
  thresholds, and acknowledged `sync` envelopes. Adds 35 `runtime` conformance cases;
  all previous 226 cases remain unchanged. Default model tool schemas are unchanged;
  hosts opt into the new validation operation.
- **[api]** Adds bounded `sync::Publisher` / `Replica` with explicit stream/revision
  checks, snapshots on missing ACK, exact reconstruction and atomic rejection.
- **[docs]** Free offline measurements: native inference 5.3–13.2× faster than JSON in
  two changing-state workloads, zero allocations; budget-pressure assembly about 7.7×
  faster one-shot under pressure; already-fitting one-shot builds were about 9% slower
  with fewer allocations. Synthetic scheduling 15,360 → 512 evaluations; synchronization
  489,490 → 19,089 transfer bytes. Host-policy selection ties existing selection at
  1,466/1,536 held-out decisions. Local-server prefix-cache harness added; actual KV
  cache savings remain unmeasured. Timings are local workload results, not guarantees.

- **[api]** Adds `brain::CachedBrain`, `CacheStats`, `process_cached` and CLI
  `brain WORKFLOW --cache N`. Bounded FIFO prediction caching is opt-in, clears on
  mutable access and never enters saved learning. Preload once and clone compiled
  templates for independent owners. Local release measurements: 1.7–2.0× faster for
  repeated states, 1.8–1.85× slower with no hits; template cloning 15.9–17.2× faster
  than restoring saved JSON for each actor. No API calls.
- **[format]** Adds six `brain-cache` conformance cases exercising the existing workflow
  through the optional cache, including errors. All 220 previous cases are unchanged;
  cached results and saved learning use the existing format.
- **[format]** Adds opt-in `train_select` brain workflows/reports and the
  `train_select_brain` tool; 17 new conformance cases. All 203 previous cases stay
  byte-identical, including existing brain training, inference and saved learning.
- **[api]** `Brain::train_select` compares bounded rates using validation reaction counts
  and MSE, with an explicit aggregate cap, one retained snapshot, and rejection of MSE
  improvements that worsen validation choices. Paired fresh synthetic tests: 1,436/1,536
  single-rate → 1,487/1,536 selected-rate; nonlinear task 315/384 → 359/384, at about 3×
  estimated training work. Inference cost/format unchanged; no API spend.
- **[format]** New `brain` graph, saved-learning and workflow formats, with 41 conformance
  cases. All 162 previous cases remain byte-identical. Neural arithmetic and training use
  integer millionths, including canonical decimal serialization.
- **[api]** `brain::Brain`: game-defined acyclic reaction networks, named normalized inputs,
  eligible-output masks and abstention; bounded supervised training and weight lessons on
  candidate copies, validation-loss gates, pins, learning limits, revisions/replay protection,
  persistence and eight retained undo snapshots. Model tools and `gamectx brain` workflow CLI.
- **[docs]** Offline tests across four synthetic policy tasks: random initialization
  545/1,536 → trained 1,421/1,536 on fresh held-out samples; changed trading preference
  264/384 → 384/384. Includes weaker nonlinear results and rejected training variants.
  No LLM accuracy or billed-token saving is inferred from these local tests; API spend $0.
- **[format]** Adds `query-schema` descriptions (types and complete small string domains)
  and `query-session` result/error envelopes, with 23 conformance cases. All 139 previous
  cases remain byte-identical.
- **[api]** `query::describe` and `query::Session`: validate tool arguments, cap result
  bytes and allow at most one correction after an execution error; success ends the
  session too. CLI: `query --schema` and streaming JSON-lines `query --session`.
- **[docs]** Explicit `unit_weight`/derived `stack_weight` host example and an eval that
  includes correction and final-answer turns, with separate development and holdout states.
  On 3 holdout states × 29 questions × 2 small models: full tables 171/174, schema-only
  172/174 after correction; average complete-exchange input 4,898.632 → 1,615.874
  (−67.0%), completed-exchange cost −64.1%. Development state is reported separately.
- **[format]** New `query` conformance operation: bounded, read-only model-written SELECT
  over named game tables, with filters, sorting, limits, arithmetic and aggregates.
  ORDER BY and LIMIT apply before aggregates. Adds 36 cases; all 103 previous cases stay
  byte-identical. No existing packed format changes.
- **[api]** `query::{tool, run, Query, Limits}` and `gamectx query TABLES SQL` /
  `gamectx query --tool`. Measured on the two small models: direct answers 33/58,
  SQL 53/58; numeric filters 0/6 → 6/6; arithmetic 1/16 → 12/16. A separate run with
  the tool packed as a signature scored 52/58. The full data was sent in both runs.
- **[internal]** Query replay and differential checks against the Python prototype;
  mutation fuzzing, limit checks and non-finite arithmetic coverage. Token-only evals
  enforce the spending cap and save usage and costs.

## 0.1.0-alpha.2 — 2026-11-05

Adds features without changing any alpha.1 output: all 51 alpha.1 conformance cases are
unchanged, and 52 new cases cover the additions.

- **[format]** New `signatures` pack mode: action schemas as short text lines with shared
  arguments listed once. Lossless for strict tools with string, number, integer, boolean
  and string-enum arguments; other tools are refused with a pointer to `schema`. Existing
  formats are unchanged. Measured: 12–14% fewer input tokens than `schema`, and 46% fewer
  than compact JSON overall, at equal or better accuracy.
- **[docs]** README and guide report a 53-question run including signatures.
- **[format]** `group_by` accepts a list of columns to nest groups (`["kind", "rarity"]`).
  A single column still produces the same output as before. Measured: nested grouping took
  two-column filter questions from 7/12 to 12/12 on the smaller models.
- **[api]** `Context` and `Turn`: register tools, prompts and constants once, add variables
  and tables per turn, and build a budgeted packet; `Packet::system_prompt` adds an intro.
  `Table` options cover grouping, views, priority and required.
- **[format]** Views gain `where` (exact filters: `=`, `!=`, `>`, `>=`, `<`, `<=`, `in`) and
  `relevant_to` (keep the rows that best match some text, or all rows if none match).
  Measured: `relevant_to` with each question's text used 26% fewer input tokens at
  identical accuracy. In Rust: `Table::filter` and `Table::relevant_to`.
- **[format]** `relevant_fallback: "none"` (Rust: `Table::relevant_only`) sends no rows when
  nothing matches. Measured: 59% fewer input tokens than `relevant_to` alone; only
  whole-table questions are lost.
- **[format]** Catalog sections take `relevant_to`: entries ranked by matching words of a
  message. Rust: `Catalog::relevant`, `Turn::catalog_relevant`, `Turn::catalog_search`.
- **[format]** New `memory` module and `memory` pack mode: records with turn, importance,
  text and tags; recall ranks by rare-word matches, then importance, then recency;
  `Memory::tool` and `Memory::apply_call` let the model write memories with validation;
  `Turn::memory` adds recalled records. Measured: 84% fewer input tokens than sending a
  60-event history, same accuracy.
- **[format]** New `formula` module: safe formulas (numbers and booleans, operators, `if`,
  `min`/`max`/`clamp`/`round`/..., calls to earlier formulas only) with definition-time
  checks and step limits; `derive` view columns; manifest `formulas`; `Formulas::tools`
  gives the model `define_formula` and `calc`; `gamectx calc` on the command line.
  Measured: models answered 0/16 arithmetic questions directly, 8/16 by writing an
  expression for gamectx to compute; the rest picked wrong rows.
- **[api]** `Value` converts from strings, numbers, booleans, `Option` and `Vec`, and the
  `obj!` macro builds objects.
- **[api]** `gamectx pack -` reads the manifest from standard input, for calling from any
  language.
- **[docs]** The Ember Keep example describes its actions with `signatures`; its budget is
  2000 bytes so it still shows two sections being dropped.
- **[internal]** `gamectx report` and `verify` columns fit the longer pack name.

## 0.1.0-alpha.1 — 2026-11-05

First public release.

- **[format]** `json`: strict, order-preserving parsing that keeps exact number text; compact serialization rules.
- **[format]** `table`: lossless header-plus-rows packing, optionally grouped by one column (`group_by`).
- **[format]** `view`: `sort_by`, `limit`, `fields` and `round` shape table rows before packing;
  `verify` reports viewed sections separately from exact round-trips.
- **[format]** `schema`: lossless tool-schema packing with shared `$defs`; lossy `outline` signatures.
- **[format]** `catalog`: searchable, paged registries with change-detecting generations.
- **[format]** `budget`: stable-first assembly that drops low-priority sections, with receipts.
- **[format]** `estimate`: conservative token estimates calibrated from provider usage reports.
- **[api]** `gamectx` command: `pack`, `report`, `verify`, `search`, `outline`, `calibrate`.
- **[docs]** 51 shared conformance cases defining every format, for ports in other languages.
- **[docs]** Model evaluation script (`eval/`), measured results in the README, and
  `docs/GUIDE.md` with practical rules drawn from them.

- [api] Add opt-in transient Life compaction and non-shareable LivingWorld dialogue
  recording; prevent routine Web conversations exhausting the rumor ledger at turn 65.
  Fresh 600-chat trace rejects 0 turns (previously 536); 1,000 delivered turns exercise
  five branch restores. Three additive cases preserve all 1,122 prior cases.
- [api] Add scoped original-event `Life::recall_selected` with optional checkpoint
  consolidation search, required anchors, complete causal groups and Python/Node helpers.
  Four-character 4,000-event trace: 80/80 required-anchor probes versus 0/80 legacy.
  Eleven additive cases preserve legacy outputs; archived erasure returns causal-child
  tombstones previously omitted after working-set eviction.
- [api] [format] Add authorized `speech` enum projection/resolution for existing answer
  cards and fifteen additive source-speech cases. The Studio adds an optional speech
  policy selector, current-player episode/promise context and focused visible-state
  filtering; older configurations and existing conformance outputs remain unchanged.
- [format] The overnight transient-record and selected-episode workflows add conformance
  families without rewriting prior expected outputs; their legacy formats are preserved.
- [api] [format] Add opt-in bounded Studio command proposals (26 new cases), polite
  phrases and recent scoped pronouns. Clarify typos/multiple items without executing;
  keep questions/negation as dialogue and retain legacy look/wait commands.
- [fix] Reject delayed Web commands after a player/scene switch with optional generation
  headers; the browser captures intent scope before awaits, coalesces state refreshes and
  restores rejected input. Offline reordered-request holdout changes an unintended
  player_2 purchase into HTTP 409 without a purchase; six browser context scenarios pass.
- [api] [format] Add explicit optional/required unknown policy to authorized speech;
  keep the original methods unchanged. Four additive cases close unnecessary generic
  deferrals and preserve source validation for authored uncertainty. Grounded Web replies
  use required answers and retain their immediate line when only one variant exists.

- [fix] Normalize the already accepted uppercase model-tier suffix in Web route assembly.
- [format] Add one archived plural-preference selection case and a bounded trailing-s
  lexical alias; all prior conformance outputs remain unchanged. Final restored-cast
  paired evidence reports 3/12→12/12 required replies under host-authored answer policy,
  with failed evaluation-gate integration retained and corrected using PartyScene APIs.

- [api] Add optional Studio model-ID selection, asynchronous bounded local connection
  checks, request activity and reply-origin diagnostics without changing core APIs.
- [fix] Make missing models and unavailable services visible, preserve other local work
  after definite connection/400/404 failures, and retain pause handling for ambiguous
  failures. Route Talk UI/addressing phrases through dialogue and acknowledge text once.
- [api] Add confirmed New game with stale-result fencing; keep provider settings, saved
  branches, approved art and cumulative spending. Simplify the main play/setup layout.

- [api] [format] Add optional pure ModelPolicy and 32 additive conformance cases for
  dramatic scene routing, independent text/image profiles, residency/prewarm hints,
  hold/cooldown and bounded RAM/availability fallbacks. Existing outputs are unchanged.
- [api] Add Models & Services controls, local Ollama/SD load/unload, trusted owned-service
  launchers with parent-exit cleanup, idle leases, TTS/STT controls and capped online
  profile routing. No model download or paid call occurs automatically.
- [api] Add a detailed dramatic art bible, model-bound normalized asset identity and
  scoped previous-approved previews, retaining repair and candidate-approval constraints.

- [api] [format] Add opt-in native Piper/whisper.cpp CLI audio plans (13 additive cases), host-only executable/weight registration, bounded one-shot execution and Unix process-tree cancellation. Engines and weights remain separately installed. Move model, voice, connection and art setup into an accessible Settings dialog.

- [api] Add opt-in two-player private-LAN sessions with a host-only control plane, random join credentials, fixed guest identity, bounded shared transcript, participant-scoped AI requests, delivery ownership and deterministic turn floor. Add Settings tabs and an independent browser guest view; all inference stays on the host.

- [api] [format] Add bounded speech-buffer plans (14 additive cases) and optional native
  Piper PCM frame checkpoints. Prepare complete lines in one engine session, validate/join
  phrase WAVs and play one buffer; existing unmanaged HTTP word delivery stays compatible.
- [fix] Keep native subprocess launch working across Linux hot rebuilds; pass absolute
  capture/model paths to installed wrappers. Order microphone interims before transcription,
  show input/recording status and handle empty/failed STT with safe retry.

- [api] [format] Add an optional voice recording FIFO/cache (12 additive pure policy cases),
  configurable backlog/off, read-only replay and New game clearing. Repeated native speech
  can reuse exact validated audio without a provider call or duplicate heard memory.
- [fix] Cancel skipped local Ollama replies, acknowledge controlled unload before voice,
  and display queue/run timing separately. Add scoped role/room follow-up replies.
- [api] Move inspection into a movable/resizable/minimizable Debug window with context
  byte breakdown, optional prompt capture, provider token counters and execution timelines.

- [api] [format] Add opt-in studio deliberation planning and unverified note validation
  (28 additive cases), exact-model provider controls and shared reasoning/output budgets.
- [ui] Add a per-browser automatic-speech checkbox; keep private reasoning out of speech
  and memory, with bounded diagnostic counts and unsupported-model fallback.

- [api] [format] Add `studio-message-chunks` (22 cases) and host-authored
  `RoleplayActions`/`studio-roleplay-actions` (14 cases), preserving existing cases.
- [ui] Add opt-in 1–3 sentence replies in one text request, per-sentence delivery,
  interrupted-tail cancellation, a bounded multiline player composer and optional
  unverified same-call summaries. Preserve originals and explicit name excerpts.
- [fix] Route natural stock questions deterministically, omit private purse balances
  from NPC context/price replies, retain typed action requests and acknowledge
  host-confirmed purchases. Registered stage actions still require current affordances.
