# Lantern Vault loopback studio

Run `cargo run --example dungeon -- --web --cap 0`, then open the printed
`http://127.0.0.1:8787/` URL. `--port 0` chooses a free port. The default UI needs only
Rust and a browser: no npm, framework, Python server, model or provider account. The Rust
core and host have zero third-party dependencies and retain Rust 1.82 support.
The CLI game, JSON-lines protocol and earlier conformance outputs remain unchanged.

The center panel plays the original deterministic Lantern Vault game. Listed buttons and
exact typed commands such as `go workshop` or `buy oil` use current legal action pairs;
model responses never execute actions. Case, terminal sentence punctuation, `the` and
`go to` are normalized for final voice commands such as “Buy the oil.”; a question or
unlisted target does not become an action. `player_1` / `player_2` share the existing game's
purse and inventory but have separate NPC attitudes, promises and interaction counters.
The merchant is at the workshop and the keeper at the cistern; a character elsewhere
cannot hear a conversation. Only the addressed character responds, preventing NPC loops.

The left panel displays approved room/portrait assets or authored SVG placeholders. The
right panel shows effective Life attitudes, episodic recall, accepted promises, attributed
rumors, habit badges, provider configuration and conservative spending receipts. The
example layers the existing chime cast over the dungeon; edits to
[`examples/fixtures/cast/chime-town.json`](../../examples/fixtures/cast/chime-town.json) still validate through
CastPack. The studio adds explicit fair-purchase appraisal rules for both players. The
chalk promise button records an accepted commitment; possessing the purchased chalk at the
addressed counter lets the host confirm completion. Off-screen guild pauses permit bounded
merchant/keeper rumor propagation. These are original example rules, not inferred traits.

## Four independent provider channels

Edit [`providers.json`](../../examples/lantern-vault/web/providers.json) or pass a copy using
`--web-config FILE`. Backend selectors and voice profiles can also change in the UI.
Endpoint URLs, credentials, per-request ceilings and tier/model mappings stay host-owned.
Credentials are read from the configured environment variable and never returned to the
browser, saved in a checkpoint or placed in command arguments.

| Channel | Backends | Wire contract |
|---|---|---|
| Text | `0b_fallback`, `local_ollama`, `online_openai_compatible` | Deterministic line; Ollama `/api/chat`; compatible chat-completions JSON |
| Images | `cached_only`, `local_sd_cpp`, `online_image_api` | Approved registry reuse; SD `/sdapi/v1/txt2img`; compatible images/generations JSON with base64 PNG output |
| TTS | `off`, `browser_web_speech`, `local_tts_http`, `online_tts_api` | Browser utterance; compatible audio/speech JSON returning WAV |
| STT | `off`, `browser_speech_recognition`, `local_stt_http`, `online_stt_api` | Browser interim/final events; compatible multipart audio/transcriptions returning JSON `text` |

Local HTTP endpoints must use literal `127.0.0.1`; defaults are Ollama port 11434, SD
port 7860, TTS port 8880 and STT port 8080. There is no automatic model installation or
service startup. A local audio server with another protocol needs a compatible HTTP bridge.
Default online URLs are intentionally unconfigured. Online adapters require HTTPS; the
optional transport uses the host's `curl` for certificate-verified TLS because Rust `std`
does not provide TLS. Local HTTP and the offline GUI do not require curl. Curl's user
configuration, redirects and retries are not used; authentication travels on stdin.

`text_models` maps each text backend and tier (`1`, `1.5`, `3`, `8`, `30`, `70`) to an
actual model ID. Empty mappings select the authored fallback without contacting a service.
The supplied 3B Ollama ID is an editable example, not a claim that it is installed. A tier
switch changes the actual request model as well as `WorkloadHints`: small models receive
one exchange, 1–2 recalls and grounded policy; larger tiers allow 2–3 exchanges and more
recall. Unknown factual turns omit distractor lore/current inventory and explicitly scope
player attributes to that player. Stable system/character/scene content precedes changing
facts and the direct `Current request:` tail. Replies must be complete and at most 240
Unicode scalars; overlong/incomplete responses retain the fallback instead of being cut.
No model tool or speech content is an executor.

Per-character voices contain `voice_id`, `pitch` and `rate` (0.5–2.0). Browser speech uses
all three when supported and prefers an installed local voice if the ID does not match.
HTTP TTS receives voice and speed. Pitch is a browser playback-rate adjustment for HTTP
WAV audio; without time-stretch DSP it also changes duration. It is not a promise of
independent pitch/speed control for every provider. Browser speech engines still consume
browser/system resources; they load no model into this Rust host. Browser STT availability,
permissions and whether it uses a browser-vendor online service vary by browser. It is
optional and off by default; the host cannot meter a browser's own service.

Protocol choices were checked against official specifications, without inspecting or
porting provider implementations: [Ollama chat](https://docs.ollama.com/api/chat),
[SD server API](https://github.com/leejet/stable-diffusion.cpp/blob/master/examples/server/api.md),
[OpenAI chat](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create),
[images](https://developers.openai.com/api/reference/resources/images/methods/generate),
[speech](https://developers.openai.com/api/reference/resources/audio/subresources/speech/methods/create),
and [transcriptions](https://developers.openai.com/api/reference/resources/audio/subresources/transcriptions/methods/create).
Compatible services must support the fields used here (including `max_completion_tokens`
for online text); service/model-specific limits still apply. The default art bible is
512×512. Select an image service/model supporting that size, or edit its locked dimensions
and matching aspect ratio before rebuilding. Legacy `dall-e-*` requests explicitly request
base64; image URLs are never fetched as an extra unreserved request. Palette, anchors,
camera and framing are included in the generation prompt. SD receives the compiled seed;
the compatible online image contract does not promise seed support, so an online re-roll
requests a fresh candidate without claiming deterministic pixels.

## One cumulative spending cap

`--cap` is exact decimal USD, up to six fractional digits; the default is zero. The GUI
may lower its active cap or restore it up to the startup-authorized maximum, but cannot
raise that maximum. All four paid channels share one `media::providers::ProviderBudget` backed by
`SpendingLedger`; game ticks never expire its charges. Set a positive host
`ceiling_micros` for each online endpoint and an explicit positive startup cap to enable it.
A ceiling is an operator-supplied worst-case charge for one bounded request, not a bundled
price quote. Configure it for the chosen service, request limits and current provider
pricing. The library cannot guarantee a remote bill when that supplied upper bound is wrong.

The host reserves and synchronously saves the ledger before queueing paid work, then saves
its dispatched state before network I/O. Failed, timed-out and ambiguously completed paid
requests retain the full ceiling. There are no retries or refunds after dispatch; unused
queued reservations can be cancelled before dispatch. Actual billing is not guessed from
missing usage fields. The header therefore says **committed**, not measured provider spend.
The ledger is bounded to 256 records; exhaustion also selects a fallback.

At the cap, for zero/unconfigured ceilings, or without credentials, text keeps its 0B line,
images keep approved art/placeholders, and audio uses browser facilities when available.
`local_fallback: true` explicitly prefers the configured local backend instead. Browser
STT fallback captures a new utterance; it does not upload an earlier recording elsewhere.
If a browser facility is unavailable, text gameplay remains usable. Positive ledger amounts
in offline policy tests simulate accounting; no paid transport is dispatched.

The default state directory is ignored `target/lantern-studio`; change it with
`--web-state DIR`. `spending.json` is separate from game checkpoints and restored
conservatively after a crash. Do not roll that file back with a game branch. Starting with
`--cap 0` after prior spend disables further paid work while retaining the recorded amount.
The directory and configured endpoints belong to the host; the GUI cannot choose file paths
or arbitrary network destinations.

## Delivery and real interruption

Play explicitly starts the current line; the fallback is already visible while optional
text work is pending. A validated model reply may replace it only before playback starts
and while the speaker, scene generation and Life revision remain current. `off` leaves
text visible; a second Read click explicitly acknowledges the displayed line.

`media::voice::VoiceTurn` calls the real `Coordinator::delivery` operations: `queued`, monotonic
`played`, recipient `heard`, and terminal `interrupted`. Browser boundary `charIndex` values
are converted from UTF-16 to Unicode scalar offsets without splitting surrogate pairs.
A normal end acknowledges the complete line. Browser engines without boundary events
confirm nothing until end. HTTP TTS synthesizes one whitespace-delimited word chunk per
request, and only the audio element's completed-chunk event advances the confirmed offset.
Each online chunk has its own spending reservation. A failed/capped HTTP voice resumes
only the unheard suffix through browser speech, rather than replaying its prefix.

Interrupt immediately calls `speechSynthesis.cancel()` / pauses and detaches the current
audio element before awaiting HTTP. Recognition speech-start and local capture voice
activity do the same. Ordered receipts report the last confirmed `through` / `heard`
offsets; stale end callbacks cannot acknowledge the tail. Only the heard prefix is copied
into Life/party history, and actual interrupted deliveries update the selected player's
host counters. No timer estimates characters from elapsed/duration ratios.

These are exact **confirmed** offsets, not a measurement of what a human physically heard
inside an unfinished word or an output buffer. Without per-character acoustic alignments,
that physical offset is unknown and is conservatively withheld. Browser event granularity
and device latency remain outside the host's guarantee. See the browser's
[boundary index contract](https://developer.mozilla.org/en-US/docs/Web/API/SpeechSynthesisEvent/charIndex)
and [speech-start event](https://developer.mozilla.org/en-US/docs/Web/API/SpeechRecognition/speechstart_event).

This is a single-controller local example; reload the page after restarting its host.
Mic capture is opt-in, bounded to ten seconds / 2 MiB, and can be stopped early.
Browser-native decoding/resampling normalizes uploads to mono 16 kHz PCM16 WAV. The
server independently validates RIFF/data sizes, format and at most 160,000 actual frames;
a client duration claim cannot enlarge a paid transcription request. Programmatic HTTP
STT callers must supply that same canonical WAV format. Interim
transcripts go through `Coordinator::recognition(final:false)` and only update preview /
interrupt playback. Final text is accepted once for the current capture session, then
enters the same exact-command/chat parser as typed input. Recognition's own `actions:[]`
is preserved; the separate host dispatcher validates any final gameplay action. Speaker /
room/provider changes invalidate old capture sessions. Uncertainty notes stay visible.

## Art studio and branch checkpoints

The [visual compiler](../guides/VISUAL.md) supplies fixed art style, identity anchors, expression
slots and perceived-only room cues. Closed shutters, actual remaining counter stock and
the current gate state come from host observations. `AssetRegistry::lookup` reuses/registers
without scheduling generation; Generate calls `request` explicitly. `render` returns the
stored variant including repairs. A generated PNG is size/signature checked and stored by
byte digest, but remains a candidate. The main stage uses approved/pinned art only; candidate
preview is an explicit debug checkbox. Approve & Cache verifies the stored bytes/digest.

Re-roll derives a new key. The patch drawer accepts whole-token replacements, negative
constraints, forbidden props and seed rotation. Repair requests name the exact inspected
key, so switching context cannot silently repair another variant. An optional hand-edited
PNG is checked against locked dimensions and pinned by digest; arbitrary asset URLs or
filesystem paths are not accepted. The approved-reference index and bounded patch history
persist across restarts. Pin-only history replays once, and late worker completions cannot
revoke/replace a newer pin. Cached approvals are reactivated when their spec is encountered;
unvisited historical variants are not preloaded. Missing/corrupt files render placeholders;
the developer can re-roll or pin a reviewed replacement.

Registry/compiled variant metadata is bounded to 128 entries and eight repairs per lineage.
Capacity exhaustion returns approved fallback/placeholder and leaves gameplay usable.
Cached selection returns immediately without an image-model request; this is not a claim
of literally zero milliseconds for disk I/O, hashing or browser rendering.

Save/Load slots A–D store authoritative game action replay, cast operation journal, heard
party receipts, promise state and interaction counters. Cast digest, scope, participants,
shapes and replayed operations are checked before replacing the live branch. Loads discard
pending speech/capture and invalidate stale jobs, while leaving spending and global asset
approval intact. The original game's 256-action bound remains. World journals, transcripts
and heard-party tails are bounded; files above 8 MiB reject rather than allocating without
limit. Checkpoints are host-owned data, not portable signed proofs or a provider transcript.

## Runtime bounds and offline evidence

One bounded background worker executes every model request, including STT/TTS; local LLM,
SD and local audio requests from this studio do not overlap under the synchronous service
contract. After an ambiguous local transport failure, new local dispatch pauses rather
than starting another heavy model while the earlier server might still be computing.
The inspector's Resume Local control is a host acknowledgement that the servers are idle;
text/browser fallbacks and gameplay remain available during the pause. Eight lightweight HTTP handlers
continue to accept playback receipts, interruption and gameplay while the worker waits.
Queues contain at most eight jobs. Ollama requests use at most four configured CPU threads
(two by default), a bounded context and `keep_alive: 0`. Set diffusion/audio server thread
limits separately. Other clients and models kept resident by external servers are outside
this scheduler; serialization is not a guarantee that every model has unloaded from RAM.
HTTP request bodies, response bytes, headers, timeouts and blob reads have explicit caps.
Only `127.0.0.1` is bound; Host/Origin and the required JSON request header are checked.

Run the same offline checks as CI:

```sh
cargo build --bin gamectx --example dungeon --locked
python3 -I eval/studio/studio_check.py --cap 0 --check eval/studio/studio-web-results.json
cargo test --example dungeon --locked
```

[Retained results](../../eval/studio/studio-web-results.json) record 32 independent Python/Rust
policy workflows, 85 loopback endpoint checks and five fake-browser event scenarios.
Eleven new Rust studio tests include a blocked fake worker, real loopback mock transport,
stale-result guards, post-restart pinning, and gameplay after registry exhaustion. Another
6,000 deterministic input mutations cover the two policy adapters. The 32 additive
conformance cases retain all 1,090 previous cases and the six frozen life-model artifacts.
Real provider calls and API spend are both zero. Provider tokens, real microphone/TTS
accuracy, physical heard-character offsets, image quality, installed-model performance,
TLS-provider integration and whole-game FPS have not been measured here.

The 32 policy fixtures also run through Python CLI/native, Node and the existing C# native
bridge: `python3 -I eval/studio/studio_cases.py --cap 0 --check-bindings` emits the shared files
under `target/studio-csharp`; `node eval/studio/studio_bindings_check.mjs --cap 0` checks Node.
The C# Check driver accepts that directory's requests.txt/expected.txt files as usual.

## Focused factual speech policy

The bundled configuration now sets `speech_mode: "grounded"`. The Speech policy selector
also offers `dialogue`; configurations omitting this optional field retain legacy free
speech. Grounded mode sends a bounded enum of host-authored sentence variants and rejects
any unlisted reply even if the provider ignores its schema. Ollama receives `format`;
compatible online adapters receive strict `response_format`. Unsupported providers retain
the immediate fallback. No paid provider has been tested by this offline evaluation.

This is a deliberate expression tradeoff, not a claim that 3B free prose became reliable.
Current stock/unknown/trust and the accepted chalk promise have deterministic replies;
confirmed interruption counters shorten fallback phrasing. Model selection cannot add
new facts. Player episodes and accepted promise tasks are scoped before prompt assembly;
old heard dialogue retains explicit speaker attribution. Focused packets include only
query-relevant visible items/coins; expanded profiles may include the fuller current view.
The stable system/character/scene prefix remains ordered ahead of changing state.

## Natural commands and delayed controls

Typed/final voice commands now accept bounded English forms such as “Please buy the oil”,
“I would like to purchase chalk”, “walk to the gallery” and “examine the tonic please”.
An explicit item question/inspection establishes a recent focus for “buy it” or “that one”.
Focus expires after four ticks and clears across participant changes, travel and restores.
Typos and multi-item commands offer action-button clarification; questions and negation
never purchase anything. Item questions can return the current price and party coins.
This is a small transparent grammar, not general natural-language understanding.

The browser captures a scene generation before awaiting interruption or sending a control.
The host atomically checks `X-Studio-Generation` before mutation, rejects stale commands
with HTTP 409, and returns the new generation in successful/409 JSON replies. A delayed
player_1 command therefore cannot purchase for player_2 after selection changes. Delivery
and STT keep their independent speech/session receipts so interruption remains immediate.
Polling waits for state at least as new as the command receipt; one additional fetch
covers an older in-flight poll, and a reset cannot cause infinite retries. Failed text
submission restores the typed line for correction/retry.

The generation header is optional for compatibility with existing trusted host callers;
those callers remain responsible for scope. It is optimistic concurrency, not login or
multi-controller authentication. The UI remains a single-controller example.

Grounded Studio replies now exclude a generic “I don't know” replacement when an authored,
question-specific answer is present. An authored sentence expressing actual uncertainty
remains valid and source-checked. A sole authorized sentence uses the immediate 0B line;
multiple variants permit optional model wording selection. Generic `Answers` APIs still
allow deferral unless the caller explicitly selects this required-answer policy.


## Setup, activity and starting over

Offline play is immediately available; meeting a character never requires configuring a
model first. The header shows the applied provider state. **Model setup** separates the
actual model ID from its context tier: choosing `3` does not install `qwen2.5:3b`.
Choose Ollama, click **Check local servers / Find models**, pick an installed model and
click **Apply changes**. A visible draft indicator distinguishes selected controls from
applied settings. Settings affect the next message and preserve game progress.

The connection check uses the same single worker as generation, outside the state lock.
It fetches only Ollama `/api/tags`, validates at most 64 names/65,536 response bytes, and
checks other local ports without generating assets/audio. An open port proves
reachability; it does not establish API compatibility, installed voices or ready models.
No downloads, online calls or spending reservations occur. A missing exact model name is
shown explicitly. Local voice/transcription each require a separately running service.

**What is running?** retains at most 24 worker receipts, with the selected model/backend,
queued/running/completed/failed/stale state, elapsed time including queue wait, and a
bounded explanation. The current reply identifies offline fallback, pending generation,
validated free dialogue, or model-selected authorized wording. This exposes operational
status rather than hidden model reasoning. Clicking **Talk to merchant** enters that same
conversation flow; it no longer only displays the deterministic game's legacy talk hint.
Typed exact addressing phrases also enter conversation. Existing game/CLI action outputs
remain unchanged. Text-only **Read reply** acknowledges a line in one click.

Connection establishment failures and explicit 400/404 rejections leave other local
services usable. Uncertain failures still pause local dispatch because an external server
may still be computing; **Resume local requests** requires the host to confirm it is idle.
Settings/check commands do not silently resume it or execute an old prompt.

**New game** asks for confirmation, resets current gameplay, conversations, habits and
promises, and invalidates late speech/microphone replies. Provider settings, saved branch
files, approved/repaired art and the cumulative spending ledger are preserved. It neither
refunds dispatched spending nor assumes that a remote model has stopped. The monotonic
job/receipt IDs and scene generation survive the reset boundary. Save/load remains the
way to return to an earlier branch.

Evidence: the independent setup prototype has eight expectations; a fresh offline HTTP
holdout covers eleven workflows with one mock discovery and four mock generations,
including reset while the mock is held and rejection of its late response. Existing
85-HTTP/five-browser-audio and 41-request command checks still pass. Real Firefox layout
and interaction checks are separate from physical voice/image/model-quality claims.
See `eval/studio/studio-setup-results.json` and the evaluation instructions.

Firefox follow-up: nine real-browser workflows pass at desktop 1366×630 and mobile
390×844; the composer ends at 566.4 pixels in the recorded desktop trace. These are
layout/interaction checks, not a player study. Unsupported browser speech recognition
is labeled explicitly instead of presenting a usable-looking microphone button.

A separate manual local smoke run uses the already installed Qwen 3B and four configured
CPU threads at `--cap 0`: one generation returns a validated authorized reply in about
23.9 seconds on this busy desktop. This proves provider wiring, not accuracy, stable
latency or physical voice. The artifact retains validated speech, not the unretained raw
wire response; see `eval/studio/studio-setup-live.json`. Successful requests refresh readiness
so a recovered service does not keep a stale missing-model warning.
