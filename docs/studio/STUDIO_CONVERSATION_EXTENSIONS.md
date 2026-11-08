# Sentence replies, player messages and registered actions

Settings → Characters → Conversation length offers one, two or three reply sentences
(default one), a player-message limit of 512–4096 Unicode scalars (default 2048), and an
optional long-message summary (default off). These preferences persist in `conversation.json`
and survive New game/branch changes. Existing library APIs and conformance outputs remain.

A model returns a bounded array of complete sentences in one text call when multi-sentence
speech is enabled. Each sentence independently passes the existing grounding/subjective
validator and a 240-character ceiling. Every sentence is checked before any is adopted;
cut-off clauses, duplicate text or a failed later sentence retain the fallback. A single
sentence is allowed. Known single-fact replies, established viewpoints and deterministic
social turns stay brief. This option does not grant access to unknown lore or tools.

Sentences become individual voice tickets, showing `1/2`, `2/2`, etc. Finishing the complete
heard sentence reveals the next validated sentence without another text call. Automatic
speech plays them in order when enabled; manual Play/Read remains available. Interrupt,
new input, travel, participant/settings changes and microphone barge-in discard the remaining
tail. Only heard prefixes enter conversation/memory; unplayed sentences and summaries do not.
Each sentence uses the existing whole-line audio buffer/cache and 240-character offsets.
Audio synthesis still occurs per sentence: this is not streaming inference or a guarantee
of gapless playback. LAN guests acknowledge their own reply tickets; they cannot execute
world actions or configure providers.

The composer accepts multiple lines, displays a character counter, sends with Enter and adds
a newline with Shift+Enter. Host limits apply independently of the browser. Longer input is
preserved in the transcript and split at whitespace/punctuation into ordered receipt/context
parts of at most 384 scalars/960 UTF-8 bytes. Layout whitespace is normalized for receipts;
words, corrections and negations are retained. Long messages are conversation-only: embedded
commands cannot execute purchases or travel. Use an explicit available action for that.
Long STT interim/final transcripts pass each bounded part through Coordinator recognition;
interims still never execute actions. Physical microphone quality is not established here.

The focused local adapter uses 4096 context slots for long input (2048 for ordinary focused
turns); expanded profiles keep 8192. One configured text call shares the existing queue,
spending reservation and output-token ceiling. More input/output can increase CPU latency.

Optional summaries use the same constrained text call, with a short `player_summary` field
before speech. They describe player claims, confer no authority, remain in Debug and never
replace the original input. The host separately retains a bounded exact `My name is ...`
excerpt alongside a generated summary. This mechanical safeguard is not a complete extractor
of every important detail. The measured 3B summary omitted the corrected name despite the
prompt instruction: summaries therefore remain off by default.

## Registered roleplay actions

`character::roleplay::RoleplayActions::from_value` accepts a host-owned version-1 registry with at most
128 entries: unique `id`, 1–8 unique normalized `phrases`, `decision` (null for an emote, or
`{action,target}`), and bounded `narration`. `resolve(input, choices)` matches wrapped stage
directions against the host's current affordances. It returns `not_stage`, `unknown`,
`blocked`, or `proposed`. A proposal neither executes nor records anything. The host confirms
success before narrating a world action; emotes require explicit host acceptance. A gesture
cannot prove a threat credible, transfer resources or bypass any reaction gate.

The editable demo registry is [roleplay-actions.json](../../examples/lantern-vault/web/roleplay-actions.json).
Examples include `*nods politely*`, `*buys hinge oil*` and `*inspects the bronze key*`.
Existing approach/entry grammar remains compatible. Settings → Characters → Registered
roleplay actions edits the host registry; a state-directory `roleplay-actions.json` overrides
the bundled defaults. Unknown/compound/negated directions do not invent world effects.

[The executable fixture](../../examples/fixtures/cast/roleplay-action-fixture.json) uses local dispatch
`studio-roleplay-actions`. `studio-message-chunks` exposes pure `player` and `reply` validation
through CLI and existing Python/native, TypeScript/Node and C# bindings. The reply fixture
marks a one-call whole-packet budget (`model_calls: 1`); its validator itself performs zero
provider calls. Both contracts have
independent Python references and additive malformed-input conformance cases.

## Trade perspective and acknowledgement

Natural stock aliases such as “what do you have for sale?” now return current visible stock
with zero model calls. An item-price reply states its price without disclosing the player's
purse. Focused and expanded NPC context omit the private balance and carried player items;
the gameplay UI and host affordability checks retain them. This does not rewrite previously
heard dialogue or purport to erase information the player already disclosed.

Typed action requests stay visible in the transcript. A confirmed purchase also narrates the
player's registered action and provides a deterministic merchant acknowledgement. Failed or
blocked transactions cannot produce a successful-sale reply. The acknowledgement follows the
normal delivery/archive/autoplay lifecycle and costs no text call.

## Measured checks and limits

- 22 independent message/chunk cases, 14 roleplay-registration cases and 4000 mutation inputs.
- 72 Python CLI/native, 36 Node and 36 C# checks against shared fixtures.
- Offline host: 10 workflows/43 HTTP checks, two mocked text calls; false summaries did not
  execute actions, interruptions discarded tails, and stock queries used zero calls.
- Firefox: six workflows, one mock text call for three ordered spoken sentences, zero page
  errors. Browser audio callbacks were simulated; hardware voice was not measured.
- Fresh local Qwen 3B: two paired prompts/four calls, four complete sentence packets. The
  long candidate used two sentences; its baseline used one. Observed long-turn wall times
  were 61.161s with summary/chunks vs 52.841s baseline. The short candidate returned one
  sentence (32.224s vs 29.935s). These tiny uncontrolled samples prove neither an accuracy
  improvement nor a latency gain. Raw results and name-summary omission remain retained in
  `eval/conversation-extensions-model-*`. Actual long-pair prompt counts were
  1950 vs 2057 tokens and output counts 14 vs 50; the token-first artifact labels
  its byte/4 proxy separately. Every run used cap zero/$0 API spend.
