# Models & Services

The opt-in Lantern Vault manager selects normal or dramatic model profiles, manages local
weight residency, starts configured local services, and shows which operation is running.
The Rust library remains dependency-free; the model servers are separate applications.
Existing configurations keep their earlier behavior until model profiles are enabled.

Open **Settings → Models**, click **Scan models / services**, configure the profiles and
click **Apply model policy**. The default normal profiles are Qwen 2.5 3B and SD 1.5 Q8;
the scanner can bind the normal Qwen profile to the installed 3B alias already used by the
host. The optional dramatic defaults are Qwen 2.5 7B (8B context class) and SDXL. Larger
weights are not bundled in the crate or downloaded automatically. Missing profiles retain
normal/offline replies and approved imagery. Installed weight IDs come from the servers.

| Profile | Default | Purpose |
| --- | --- | --- |
| Everyday conversation | Qwen 2.5 3B | Focused context, one exchange, relevant recall |
| Dramatic conversation | Qwen 2.5 7B, 8B class | Expanded context and longer dialogue history |
| Everyday artwork | SD 1.5 Q8, 512px | Original normal art bible |
| Dramatic artwork | SDXL, 1024px | Separate detailed art bible, 24 generation steps |
| Voice playback | Kokoro model ID, inherited connection | Local/online voice server or browser/text alternative |
| Listening | Whisper small model ID, inherited connection | Local/online transcription server or typing alternative |

These are editable profiles, not capability or latency benchmarks. Match a model's context
class and working-memory estimate to the actual weights. On the development machine only
normal Qwen/SD weights are present. A larger local profile also has to pass the RAM
allowance and free-memory reserve; choosing its name is not installation.

## What defines a dramatic scene

**Auto** uses bounded host-confirmed signals: combat, emergency atmosphere, low health,
high event significance, large attitude change, or participant conflict. Each switch and
numeric threshold is editable. Zero disables a numeric threshold. Text and image escalation
have separate switches. **Everyday** and **Dramatic** provide explicit manual overrides.

Hold turns prevent immediate return to normal after an event. Cooldown turns prevent rapid
re-entry; a configured emergency can bypass cooldown. The status panel reports the rule
that fired, hold/cooldown state, selected profiles, and unavailable/RAM fallbacks. A player's
use of words such as “emergency” does not manufacture an authoritative scene signal.
The example supplies combat/health, confirmed event significance, actual appraisal deltas
and its LivingWorld atmosphere. General multi-party hosts supply conflict and other events
through the public policy API; the small example does not infer conflict from chat text.

The public `deploy::model_policy::ModelPolicy` validates configuration with `from_value`, evaluates
signals with `evaluate`, and returns pure routing/prewarm decisions. It performs no I/O,
inference, download or process execution. `to_value` returns configuration; host scene
hold state is reset when the example starts or restores a branch. The `model-manager`
local adapter evaluates up to 64 turns and is available through existing bindings.
See the additive conformance family for complete runnable JSON.

## Loading only when useful

**Only while needed** loads local text/image weights for the request and unloads them
once it finishes. STT can release after transcription; TTS retains its engine during the
utterance and releases on completed/interrupted delivery, or on idle expiry. External
voice services without an unload protocol own their internal residency; the manager stops
only its own voice processes. **Keep briefly for reuse** retains a bounded idle lease,
useful for KV reuse or consecutive illustrations. Idle processing runs independently of
game ticks. Applying model configuration releases the previous active lease.

**Prewarm** uses upcoming NPC/microphone hints to load weights without generating speech.
Explicit **Load now** can warm an image profile before Generate. Online and browser
profiles are excluded from prewarming. Predictions can become stale, so unused warm
leases expire. Metadata checks and lifecycle requests are separate from generated turns.
They generate no game experiences, speech, rumors or world actions.

All local model work uses the existing single worker. Before changing a controlled model,
the manager releases the previous lease and checks the profile allowance and, where
available, operating-system free memory with a reserve. This prevents overlapping
controlled loads/generations; it is not a cgroup-style hard RSS/VRAM limit or control over
other applications. Working-memory estimates are host configuration, not measured engine
peaks. Low memory keeps fallback art/replies available.

## Services and ownership

**Start / Connect** attaches to an existing loopback server or runs a trusted configured
launcher. **Unload weights** frees the selected model context. **Stop owned server** closes
only a server this manager started. An external Ollama/SD service may remain running with
its weights unloaded. Server ownership and observed model state are displayed separately.
A status scan reports external residency; a manager lease describes controlled work.

Host launch configuration is `launchers.json` in the Web state directory, with the bundled
file as the default. Browser requests supply only validated action/profile IDs; they
cannot submit executable paths, shell commands, environment maps or file-system paths.
Commands use argv arrays with no shell. `${GAMECTX_SD_SERVER}` and
`${GAMECTX_IMAGE_MODELS}` can locate the image executable/model root. TTS/STT have empty
launch commands until the host configures its installed compatible service. The optional
[native voice adapter](NATIVE_VOICE.md) instead runs separately installed Piper/whisper.cpp
CLIs per request without a persistent HTTP server. Optional
per-service `models` maps model IDs to specific argv arrays, allowing different voice or
transcription weight files without exposing their paths to the browser. Scan reloads the
trusted launcher file. API model names do not install an unconfigured service.

A stdlib supervisor watches its parent pipe and terminates the launched server on parent
exit; explicit stop uses that same cleanup path. External services are never killed.
Third-party descendant process cleanup remains the engine's responsibility. Idle timers
need a running host; default on-demand mode releases successful text/image requests
immediately. Uncertain failures retain the earlier conservative local pause behavior.

Adapters use Ollama's documented [generate/keep_alive](https://docs.ollama.com/api/generate)
and [running-model metadata](https://docs.ollama.com/api/ps). Image lifecycle uses SD.cpp's
[native model API](https://github.com/leejet/stable-diffusion.cpp/blob/master/examples/server/api.md).
Older incompatible image servers can continue through the ordinary unmanaged provider path.

## Dramatic images and approval

`art-dramatic.json` adds richer ink/material treatment, framing, palette and constraints,
while retaining the normal character anchors. It remains editable through **Dramatic art
bible** and the existing repair drawer. Both compilers receive only perceived scene facts;
escalation does not expose global/unheard facts or secret lore.

Art-bible content and generation model/backend/recipe/file/step identity affect the asset
key. Object ordering and RAM estimates do not create different art. Approved exact reuse
makes no generation request. A scoped **previous approved art** preview can stay visible
while the new style loads; it is labeled separately and is not approved under the new key.
It requires the same observer, audience, entity, scene/branch, visual facts and anchor
traits. Registry repairs do not bypass their stricter fallback constraints. New candidates
still require explicit approval; model loading never stalls the current card.

## Online APIs and spending

Each text/image profile may choose local or configured online inference, independently
for normal and dramatic scenes. TTS/STT retain their existing online alternatives.
Endpoints, credentials and per-request price ceilings remain host-owned. The existing
shared SpendingLedger reserves online ceilings before dispatch; local management never
bypasses it. With `--cap 0`, missing credentials, unpriced calls or exhausted allowance,
selection retains a permitted local/offline/cached fallback. No online lifecycle/prewarm
call is issued. Authorized cap increases still require host launch configuration.

## Evidence

The independent Python oracle has 32 policy cases with byte-identical Rust outputs.
Python CLI/native, Node and C# exercise the same fixtures. Two thousand new parser
mutations cover the pure policy without executing services. The offline host cohort
covers normal/dramatic text and images, focused/expanded requests, perceived-only art,
model-bound cache identity, approved previews, exact reuse, cap-zero fallback, predictive
warming, idle expiry and rejection of stopping external servers. Its two text and two
image generations use original mocks with maximum observed concurrency one.

A separate owned-process cohort starts three original socket-service fixtures, verifies
explicit stop and service replacement, rejects arbitrary profile commands, and checks
parent-exit cleanup. These tests prove lifecycle plumbing, not Kokoro/Whisper inference.
Synthetic positive-cap unit tests inspect online reservations and payloads without making
online requests. Larger-model quality, live image quality and general performance are
unmeasured. Retained evidence: `eval/studio/model-manager-host.json` and
`eval/studio/model-manager-owned.json`. Old conformance/evaluation artifacts remain frozen.

The new controls passed eight real Firefox workflows, including draft/apply validation,
online lifecycle disabling, art-bible persistence and mobile layout, with zero page
console errors. The nine previous setup/gameplay browser workflows also passed.
A manual installed-provider smoke completed five lifecycle operations: normal Qwen load,
unload, normal SD load, unload and restoration. Both external servers remained reachable;
no text/image was generated and API spend was $0. These acknowledgements do not measure
RAM savings or model quality. See `eval/studio/model-manager-browser.json` and
`eval/studio/model-manager-live.json`. All 23 dungeon example tests and Rust 1.82 checks pass.
