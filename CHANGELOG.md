# Changelog

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
  timing pilot. No semantic guarantee from compact prompts or grammar. See [ADVERSARIAL_EVAL](docs/ADVERSARIAL_EVAL.md).


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
  runs $0 API; no larger models loaded. [Report](docs/LORE_EVAL.md).

- **[internal]** Preserves 11 user-reported dungeon interaction sequences / 26 turns
  with separate manual grounding/recall/transaction annotations. Offline native/Python
  audit: 17 trade requests, 16 matching local receipts, nine supplied speech packets,
  two reproduced purchases; zero model calls. Source identity/usage remain unknown.
- **[internal]** Adds regressions proving that forgetting dialogue preserves purchased
  inventory and that invented assistant biography is never registered as player quotes
  or host facts. All 633 core cases unchanged; runtime prompts/behavior unchanged.
- **[docs]** Records remaining fabricated player history, home, weather and tonic
  purpose alongside correct transactions and name recall. Schema/citation acceptance
  is not a semantic score. New audit spend $0 API. [Report](docs/DUNGEON_FEEDBACK.md).

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
  zero model calls and spends $0 API; no new model-accuracy claim. [Guide](docs/CONVERSATION.md).

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
  controlled latency/RAM benchmark; all local runs $0 API. [Evidence](docs/WORKFLOWS.md).

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
  Both local token/gameplay runs cost $0 API; [testbed guide](examples/dungeon/README.md).

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
  retains declared dependencies. New API spend $0; [details](docs/OPTIMIZATION.md).

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
  See [the report](docs/LOCAL_MODEL_2026_11_06.md); no general model-size accuracy claim.

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
  model-latency or universal runtime speedup. New API spend $0; [integration guide](docs/KIT.md).

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


Optional dynamic character stage 1 adds bounded life definitions, attributed experiences
and host-applied appraisal rules; see [docs/LIFE.md](docs/LIFE.md). Defaults and existing formats remain unchanged.

Stage 2 adds optional scene/thread coordination, dependency-bound reply validation and
partial speech delivery acknowledgements. `eval/continuity_cases.py --cap 0` checks 41
offline workflows; generated prose quality remains unmeasured. See the dynamic character guide.
