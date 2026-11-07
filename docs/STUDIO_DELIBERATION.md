# Optional deliberation and playback

Settings → Models → Optional reasoning & hypotheses defaults to Off (provider default).
Auto requests reasoning only for explicitly classified planning/investigation or dramatic
conflict turns. Greetings and routine trade remain deterministic. A configured low/medium/high
level must match advertised values exactly; boolean thinking uses true. Routine turns request
false/none only when explicitly supported. Omitted controls leave provider defaults intact:
this does not guarantee an intrinsically reasoning model stops thinking.

The worker's Scan models/services reads Ollama `/api/show` for up to three installed,
configured models without loading/generating. Values are matched to both actual model ID and
endpoint. Unknown or unsupported metadata sends no guessed control. Online compatible
providers require explicit `{ "model":"your-model", "values":["low","none"] }` declarations;
only configure values documented by your provider. No model is installed automatically.

Reasoning shares one request, the worker lane, cancellation, revision checks, declared spending
reservation and a bounded 128–4096 combined output-token ceiling. A model that exhausts its
budget without a final answer keeps the fallback. More reasoning can increase latency;
no measured quality gain is claimed here. The installed Qwen 2.5 completion model reported
no dedicated thinking metadata. All verification used cap zero/offline transports.

Expanded planning turns may optionally attach up to three unverified hypotheses to an existing
constrained speech envelope. Each contains a claim/check of at most 180 UTF-8 bytes and 1–4
citations from a host-selected evidence allowlist. The demo exposes only the current perceived
scene description (`visible_scene`) as note evidence. Membership validation cannot prove a
claim is true. Notes remain ephemeral Debug diagnostics, never speech, memory, facts or
permissions; the original speech validator still runs independently. Empty notes are valid.
Private provider thinking text is discarded; the profiler retains only character/token counts.

Settings → Voice → Automatically speak new replies is an opt-in, per-browser preference.
Only finalized, unplayed replies with a voice backend start automatically, once per ticket.
Pending drafts, already delivered lines, text-only mode and the other LAN player's turn are
excluded. Mic capture suppresses automatic playback. A browser may require Play once to
permit audio. Interrupt/barge-in and exact delivery receipts retain their existing behavior.

`studio::deliberation_plan`, `validate_deliberation_config`, `validate_thinking_values` and
`validate_hypotheses` are pure zero-dependency contracts available through local dispatch
`studio-deliberation` and the existing bindings. Python reference: `eval/deliberation_cases.py`
(28 cases); playback policy: `eval/autospeech_check.mjs` (11 assertions). Existing conformance
outputs are unchanged. No hardware microphone/playback or supported live reasoning-model
quality improvement is established by these offline checks.

Specifications checked: [Ollama thinking controls](https://docs.ollama.com/capabilities/thinking)
and [OpenAI reasoning controls](https://developers.openai.com/api/docs/guides/reasoning).
