# Recordings and floating diagnostics

Open **Settings → Voice → Voice archive** to enable recording retention and choose the
last 1–128 replies to keep. Library/example defaults are off; a host can opt in. The
backlog also has a fixed 32 MiB ceiling. Oldest entries are removed when either bound is
reached. Turning archiving off clears saved recordings. **New game** clears recordings
while retaining the chosen settings. Save/load branches retain this review cache; game
memory is separate. Audio/index data stays in the host state directory, not the repository.

The current reply has **Replay recording** once audio exists. The **Voice recordings**
list can replay earlier retained replies. This is review playback: it makes no model
request and sends no heard/played receipts to create new memories. Replaying during an
active delivery first interrupts that active turn through its normal receipt lifecycle.
Starting a new conversation, capture, reset or branch load stops review playback.

Native Piper whole-line audio is keyed by the exact character/player, approved line,
voice/profile and native configuration. An intact matching recording can satisfy a new
synthesis request without loading Piper. WAV bytes and digests are validated on retrieval.
Changed inputs produce another key; missing/damaged cache entries are removed and can be
regenerated. With archiving off, the current in-memory playback buffer remains available
until replaced, but no recording is written for the backlog. The host-only `/api/voice/KEY`
endpoint serves retained WAVs; LAN guests cannot access host recordings or configuration.

The pure `studio::VoiceArchive` helper validates `{version:1,enabled,limit}`, remembers
bounded `{key,bytes}` records, returns removed keys, and enforces count/byte limits.
It performs no file/network/model work. The `studio-voice-archive` local fixture and all
bindings expose bounded `remember`, `configure`, and `clear` workflows. Its 64-character
hex keys are opaque; callers own audio validation and storage.

# Where time goes

The old activity elapsed time included queue waiting, so a voice job waiting behind a
long reply could appear to take a minute even though speech preparation was much shorter.
Rows retain end-to-end `elapsed_ms` and now separate `queue_ms` from `run_ms`.
Provider/stage timing can include loading, transport, controlled unloading and validation;
it is not a pure CPU inference benchmark. No OS CPU/RSS/allocation sampler is implied.

Starting a new turn, microphone capture or playback of an immediate fallback cancels a
skipped local Ollama text request. A bounded socket poll closes the obsolete request.
Managed profiles acknowledge unloading before the worker starts voice, preserving the
single provider lane. The request cancellation token is captured under the scene lock,
so cancellation during preparation cannot be missed by a later transport start. A
cancelled job is reported as cancelled, preserves the fallback and does not pause other
local services. Hosted API calls keep their existing accounting and are not refunded or
represented as cancellable by this local control.

Grounded role/room follow-ups use current authoritative character duties and perceived
scene facts directly. They exclude the last two confirmed replies to that listener when
choosing a phrasing. They need no model call. This improves role/help questions; richer
subjective character opinions are a separate path, not invented stock or world history.

# Debug window

**Debug** opens a floating, movable, resizable panel. **Minimize** removes it from the
play area; the game now uses the main two-column space. Its State tab contains the earlier
inspectors. Context shows message/selected-section byte sizes, configured output cap,
actual provider token counters when reported, and optional bounded prompt capture for
subsequent requests. Prepared-body bytes are measured before lifecycle transport fields;
byte sizes are not claimed to be exact model tokens. No tokenizer/inference call is made.
Captured prompt text is off by default, bounded, host-only, and not persisted.

Execution shows pending/cancelled work, retained audio, queue/run timelines and stage
counters. **Cancel active local reply** acts only on local text. Provider counters use the
[Ollama chat response](https://docs.ollama.com/api/chat), including prompt/output counts
and nanosecond duration fields; unreported fields stay absent. Timing is observational,
not a 60-FPS, hardware scheduling or memory-limit guarantee.

# Evidence

Twelve independent Python/Rust retention cases cover FIFO, reuse, disable/resize/reset,
byte capacity and invalid identities. Two thousand retention mutations perform no I/O.
`eval/voice-history-host.json` covers nine workflows with original delayed Ollama and CLI
fixtures: cancelled text before voice, distinct queue/run timing, cached reuse, read-only
replay, FIFO pruning, off/reset clearing and listener-scoped role follow-ups. A repeated
recording needs zero additional native jobs in that mock cohort. These are plumbing checks,
not physical timing or CPU savings measurements.

`eval/voice-history-browser.json` records seven real Firefox DOM/layout workflows using
original mock services and stubbed media play: minimized debug layout, opt-in context
capture, archive controls/replay, token/timeline display, reset and mobile width. It has
zero page console errors and $0 API spend. Physical playback and live model quality are
not measured by that cohort. Earlier evaluation/conformance artifacts remain unchanged.
