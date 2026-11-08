# Buffered Piper speech and microphone recovery

With the optional local model profile `piper`, the host prepares the remaining approved
line (at most 240 Unicode scalars) in one native engine job. The installed CLI loads once,
processes bounded sentence/word parts of at most 80 scalars, writes separate WAVs, and
exits. The host validates and concatenates their PCM samples into one continuous buffer
before playback. This avoids restarting Piper and waiting for an HTTP round trip between
each spoken word. Preparation can still take time before the line starts. Generic unmanaged
HTTP TTS keeps its existing word-chunk behavior; browser voices retain their boundary events.
No model, engine or dependency is bundled or downloaded.

The implementation uses the documented JSON `text` and `output_file` fields in the
[Piper CLI protocol](https://raw.githubusercontent.com/rhasspy/piper/2023.11.14-2/README.md).
It invokes separately installed executables, never third-party implementation code.
Bounded phrase preparation retains the installed wrapper's memory/thread controls; it is
not a new guarantee of peak RSS. Voice IDs, rate and pitch retain their earlier behavior.

# Delivery and interruption

Each prepared WAV contributes its actual PCM frame count and ending character offset.
The joined buffer carries `unit: "buffered"`, `sample_rate`, and monotonic
`boundaries: [{frame, through}]` alongside the existing audio/start/through fields.
Rates/formats, RIFF lengths, chunks, aggregate bytes and offsets are validated before use.
A malformed/mismatched WAV or incomplete batch keeps the text/browser fallback available.
The pure `studio-speech-buffer` local adapter returns `{parts:[{text,start,through}],through}`
from an exact `{text,start}` request; no paths, processes or model calls are accepted.

The media clock advances the visible audio progress bar. Only **completed prepared parts**
advance the confirmed text prefix. A partial part is conservatively left unconfirmed;
its individual phonemes/words have no measured alignment. Interrupt samples the current
completed-part checkpoint, immediately stops playback, and sends played/interrupted/heard
receipts. It does not infer heard text from a duration ratio. For a single prepared sentence,
interrupting midway can therefore leave its confirmed prefix at zero. Normal completion
confirms the whole line. Stale callbacks and late synthesis cannot confirm a cancelled tail.

# Whisper recording

Settings → Voice offers **Find microphones** and an input selector. During capture, the
input meter shows whether the browser receives a signal; the display then distinguishes
preparing, transcribing, empty speech and failure. Input selection is browser-side and is
not sent as model context. Actual audio is normalized to the existing bounded mono 16 kHz
PCM16 WAV contract. Only a completed transcript can enter normal host action validation.
Quiet input still goes to Whisper; the level indicator does not grant execution permission.

Speech-start interim receipts finish before the final recording is dispatched, preventing
a late interim from cancelling an already running Whisper job. Async recording/codec errors
are visible, and empty/failed transcripts close the capture without executing an action.
A new Mic attempt can recover without pausing other local services.

Native worker startup uses Linux's executable handle rather than a path invalidated by
`cargo build` replacing the running file. Regression coverage unlinks the fixture host
while it runs, then launches native workers. Capture/output/model arguments are absolute
while preserving executable symlink names, so wrappers that change their own directory
still find the recording and correct engine. Other platforms retain their existing
executable-path behavior.

# Evidence and limits

`eval/studio/speech-buffer-live.json` compares the same original 61-character, 13-word reply using
the installed four-thread Piper wrapper. The retained baseline at `758dcf2` required 13
jobs and 11.0014 seconds total synthesis turnaround; buffering required one job and 1.2949
seconds (8.5× in this one sample). Audio files lasted 8.6024 versus 4.0081 seconds. These
are sequential synthesis measurements, excluding physical playback and human quality
ratings; model/file caches may have been warm. No general speed/RSS claim is implied.

A fresh 81-character holdout prepared two phrases in one host job in 2.0684 seconds, with
one 7.2034-second WAV (`eval/studio/speech-buffer-holdout.json`). The pure Python/Rust oracle has
14 cases. Mutation checks cover 2,000 plans and 2,000 WAV headers; seven player callback
workflows verify completed checkpoints, cancellation, stale events and one request per
finished line. Nine native-host workflows cover buffering, hot rebuilds, changed working
directories, empty/failed STT recovery, process cancellation and parent-exit cleanup.

`eval/studio/speech-mic-browser.json` exercises real Firefox MediaRecorder, codec decoding,
resampling, upload and the installed host Whisper using original synthetic speech. It
returned “Hello merchant, I would like some chalk.” with no page console errors. This
proves the browser/host pipeline, not the user's physical device, permissions, acoustic
noise tolerance or broad recognition quality. All runs use `--cap 0` and $0 API spend;
previous conformance and evaluation JSON remains unchanged.

A fresh browser holdout (`eval/studio/speech-mic-race-browser.json`) deliberately delays the
speech-start request until after recording ends, then checks that final upload waits for
it. The installed Whisper still returns the expected transcript. This supplements the
normal capture check; neither acquires a physical microphone.
