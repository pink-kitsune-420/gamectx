# Native Piper and whisper.cpp

The optional Studio adapter invokes separately installed CLI programs. It contains original
integration code only: no third-party engine, linked audio library, weights, voice cards,
training data, installer, or automatic downloads. Existing compatible HTTP adapters remain
available. Native adapters require enabled Model Manager profiles using local audio backends.

Copy `examples/dungeon/web/native-voice.example.json` into the host state directory as
`native-voice.json`. Fill in `tts.executable` and map public voice aliases in `tts.voices`
to installed `.onnx` paths. Keep each voice's matching configuration alongside it. Fill
`stt.executable` with whisper-cli and map model aliases in `stt.models` to local GGML files.
Only the trusted host edits executable/model paths; browsers send validated aliases.
The TTS model ID is `piper`; STT model IDs must match the configured aliases.

The convenience command `dungeon --configure-voice INSTALL_DIR --web-state STATE_DIR`
registers the documented local pack layout: bin/piper, bin/whisper-cli, LJSpeech/Norman
voices, and tiny/base English weights. It refuses to overwrite an existing registration.
It neither installs anything nor checks/generalizes model license permissions. Other
installations use the editable registration file directly.

In **Settings → Voice**, choose **Use installed Piper + Whisper**, then **Apply voice
profiles**. **Start / Connect** validates/re-enables the installation. **Stop** disables
that native adapter until Start or an explicit rescan. The native status says **ready on
demand**: no voice weights remain loaded between requests. Prewarm validates files only;
it does not launch inference or retain a model. Choose a configured voice alias in the
Connections voice fields. Merchant/keeper aliases and the earlier alloy/onyx defaults
are supplied by the pack-registration helper.

Piper receives text on stdin, model/output paths in argv, and inverse speed as
`--length_scale`. Whisper receives a validated mono 16 kHz PCM16 WAV, four CPU threads,
`--no-gpu`, and a bounded text-file output request. Native output is still untrusted.
The adapter has a 90-second deadline, bounded output, removes temporary input/output,
and runs in the existing single provider worker. Installed launchers may add stricter
thread/RSS controls; the integration does not bypass them. This is one-shot execution,
not a persistent HTTP server. It avoids a Whisper server retaining a shared engine lock.

Interrupted delivery, recognition, and explicit Stop cancel an active native job. On Unix,
a dedicated process group includes wrapper descendants that remain in the group; a separate parent-pipe supervisor
terminates that group when the host exits. `/bin/kill` is used without a shell for group
termination. Other platforms currently provide direct-child cleanup only. Playback stops
in the browser immediately and existing heard-character receipts remain authoritative;
a cancelled generated tail never confirms its own delivery. CLI cancellation cannot
promise instant kernel reclamation or physical microphone accuracy.

# Licensing boundary

Separate installation avoids redistributing these engines or weights inside this MIT
repository. It does **not** grant a blanket license exemption. Developers/users must check
the exact engine build, dependencies, model/voice card and intended distribution/use.
The archived [Piper project](https://github.com/rhasspy/piper) declares MIT, the maintained
[Piper project](https://github.com/OHF-Voice/piper1-gpl) declares GPL-3.0, and
[whisper.cpp](https://github.com/ggml-org/whisper.cpp) declares MIT. Voice weights have
separate licenses; phonemization/runtime dependencies can also have separate terms.
No engine-specific license is represented as applying to every installed binary or voice.
This is an integration boundary, not a legal opinion about a particular shipped product.

Protocol references are public CLI documentation/help, never implementation sources:
[Piper CLI](https://github.com/rhasspy/piper#usage) and
[whisper.cpp CLI](https://github.com/ggml-org/whisper.cpp/blob/master/examples/cli/README.md).

# Evidence

The independent Python prototype covers 13 native-audio plan cases, including malformed
IDs, controls, excessive text, rates, formats and unknown fields. Two thousand parser
mutations never execute processes. The offline host fixture checks WAV synthesis,
transcription, temporary-file cleanup, stop/start, and cancellation of an original mock
wrapper plus child process. It makes no paid calls and does not measure engine quality,
physical voice timing, memory savings or general speech accuracy.

The manual installed-engine smoke in `eval/native-voice-live.json` completed one host TTS
job and one host STT job, plus one reference synthesis. The returned WAV was 22,050 Hz;
Whisper returned “Hello merchant, I would like some chalk.” for the synthetic reference.
Temporary captures were removed and API spend was $0. This single phrase is not a broad
accuracy, physical microphone, subjective quality, timing or memory benchmark.

The fresh seven-workflow `eval/native-voice-host-v2.json` also verifies that an active mock
wrapper and child exit when the game host is terminated; the earlier six-workflow artifact
is retained unchanged. This is process plumbing, not installed-engine quality evidence.

Playback currently retains the earlier conservative word-chunk HTTP delivery protocol;
separate CLI startup per chunk can produce audible pauses. No smooth-playback or exact
physical heard-character claim is made. Browser voices remain an alternative. Larger
phrase batching needs trustworthy alignment/receipt handling before it can preserve the
same interruption semantics.
