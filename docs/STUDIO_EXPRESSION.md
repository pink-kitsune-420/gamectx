# Character expression and AI wording

Settings → Characters → Conversation length includes **AI character wording**. Applying
a text connection or enabling Models & Services also enables it. The local preview uses
this policy. A connected model writes NPC greetings, factual acknowledgements and recall
replies; personal/social turns and fictional tales use their own bounded expression
contracts. The host still controls facts, resources, permissions and delivery receipts.

The optional `ai_wording` boolean in private `conversation.json` controls this behavior.
It defaults to false when omitted so old configurations and offline/API conformance
outputs retain their behavior. New GUI configuration enables it explicitly. Factual
wording uses one model request, not an enum-selection call followed by another request.
Failure, disconnection, exhausted budget or invalid output keeps the immediate authored
fallback. No retries or paid allowance are added. The origin label distinguishes live
AI wording, cached AI wording and fallback speech.

The bounded 64-entry session cache stores only validated AI wording. Its complete
context key includes branch, character, listener, source facts, room and description,
personality, attitudes, living context, disclosed claims, current player words, recent
heard replies, provider and model. Recent identical replies are excluded. Changing any
key component prevents reuse. New game clears it. This text cache is in memory; the
separate configurable voice archive is unchanged.

## Additive library and local contracts

`studio-expression` accepts exact `{op,args}` requests:

- `route`: `{input}`; optional `style:"expressive"` adds introspection, explicit OOC,
  broader story requests and oversized-scope negotiation. Default routing is retained.
- `packet`: `{mode,speaker,listener,profile,scene,context,history,input}` with optional
  `style:"expressive"`. Modes are `personal`, `social`, `fiction`, `negotiate`.
  Profiles/scenes/context are host-selected objects, bounded to 4/2/32 KiB. Up to eight
  history entries contain `speaker,recipient,heard,text`; unheard entries are omitted.
  System, personality and perceived scene precede changing context/history. The final
  `Current request:` is outside JSON, with explicit speaker/recipient attribution.
- `validate`: `{mode,speech}` checks one complete sentence, at most 240 Unicode scalars
  and 1,024 bytes, permitted framing and selected prohibited claims/metadata.
- `claim`: `{input}` recognizes explicit names/corrections and likes/preferences. Its
  result is a `player_claim`, never verified biography or ownership.
- `distinct`: `{speech:[...]}` accepts one to three complete bounded sentences and
  returns retained `speech` and `omitted` indices. It conservatively detects repeated
  content after stemming/framing removal while protecting names, numbers and negation.

`studio-wording` accepts `{canonical,speech}`. `studio::wording_anchors` supplies
normalized required source words for a host prompt. It checks required lexical anchors,
allowed courtesy vocabulary, negation and numeric associations. This is deliberately
conservative; it can reject a reasonable synonym. It is not an entailment checker.
Rust functions are exported from `gamectx::studio`; the generic local bridge gives
Python, native C ABI, Node and C# the same additive contracts.

Try the editable [expression packet](../examples/cast/expression-fixture.json):

```sh
cargo run --bin gamectx -- local examples/cast/expression-fixture.json
```

That fixture runs in `tests/readme_api.rs`. Personalities optionally add `interests`
and `story_seeds`, each one to eight bounded strings. Existing personality files remain
valid. Turn assembly selects at most two interests and excludes unused story seeds,
so maximum-size authored arrays still fit the profile cap. Fiction seeds are supplied as a problem/change/ending rather than room lore.

## Memory, senses and player control

Explicit name corrections and preferences are recorded only for the NPC that heard
the player. Latest name and up to three preferences are selected from the bounded
confirmed world-operation journal, including after transient Life events are trimmed
or a branch is restored. They create no rumor roots and remain attributed claims.
The journal is bounded to 8,192 operations; it is not unbounded campaign storage.

The example answers sight/hearing questions from finite authoritative scene cues;
it does not simulate arbitrary perception. Fictional speech is tagged as an imagined
tale and never persisted as a personal viewpoint or actual event. Only complete heard
mental stances are saved. Partial/unplayed speech remains governed by the existing
delivery receipts. Explicit OOC notes execute no world actions and create no NPC memory.

An oversized request routes to `negotiate`: the model chooses in-character wording
with a constrained scope-setting opener that explicitly offers a short alternative or declines the requested scope. It cannot
increase token, sentence or spending caps. A confirmed short-tale offer can be accepted
with a simple affirmative; this grants no resource action or loan permission.

## Evidence and limits

All runs use `--cap 0`, local Qwen 2.5 3B or offline mocks, four model CPU threads,
and $0 API spend. Prior artifacts and existing conformance outputs are frozen.

- `expression-model-*`: three paired questions. Baseline enum replies met 0/3 expression
  contracts; candidate met 3/3 mechanical contracts, but had bland/repetitive content.
  The v2 and v3 artifacts retain subsequent failures and the improved connected tale.
- `roleplayer-campaign-offline.json`: 33 chat turns including 12 noise turns, two
  players/two NPCs, corrections, preference recall, travel, branch restore, senses and
  an illegal compound action. Seven deterministic checks pass, zero model calls.
  A separate Rust test runs 600 transient chatter turns before recall and restore.
- `roleplayer-campaign-live.json`: the same journey with five actual local calls.
  Four packets were adopted; one exact-duplicate tale fell back. One other adopted
  tale restated its plot. These are observed limitations, not an 80% quality score.
- `roleplayer-followup-results.json`: three fresh local questions after compact
  attributed memory, standing-view directions and structured story seeds. All three
  are structurally valid; manual review finds two connected tiny fables and a relevant
  company reply. Inputs were 876, 897 and 768 tokens. This is a small cohort.
- `expression-browser.json`: six real Firefox/host workflows with mocked model/audio;
  two fictional sentences autoplay in order using one text call, stage actions and
  purchase acknowledgements remain visible, and no page errors occur. Physical audio
  and latency are not measured.
- `ai-wording-live.json` retains the initial four-turn local run, including rejected
  name shorthand and an inadequate scope offer. The v2 run adopted 0/4, exposing
  missing anchors and a scope offer without a clear limit; v3 adopted 3/4, including
  the AI-written short anecdote offer. The fresh name/novel holdout also adopted 3/4:
  an empty acknowledgement instead of the required name was correctly rejected.
  This is fallback on validation failure, not an accepted AI answer. An earlier
  four-call attempt failed before writing its raw artifact; it provides no quality
  evidence. The evaluator now records rejections before assertions. See
  `roleplayer-review.json` for the complete interpretation.

Offline parity covers 60 expression and 14 wording cases. New fuzz tests exercise
5,000 expression/claim/wording/pruning mutations. Run `eval/expression_cases.py`,
`eval/wording_cases.py`, `eval/roleplayer_campaign.py` and `eval/ai_wording_host.py`
with `--cap 0`; `--tokens-only` captures host requests before a local model run.
Bindings use `expression_bindings.py` and `.mjs`. The manual browser check requires
an explicitly started isolated Firefox BiDi instance on 9223.

Remaining limits include finite English routing, occasional generic or presumptive
social wording, conservative synonym rejection, semantic repetitions missed by lexical
comparison, and finite selected memories. Grammar validity does not prove truth, literary
quality or a natural personality. No new model, artwork or third-party RP content is
downloaded by this work. [Research and design rationale](ROLEPLAY_RESEARCH.md).
