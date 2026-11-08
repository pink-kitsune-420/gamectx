# Subjective character viewpoints

The studio's opt-in **Settings → Characters** path answers dreams, hobbies, simple
preferences and opinions about a named topic. Edit the original profiles in
`examples/lantern-vault/web/personalities.json`, or the host state directory's
`personalities.json`. Defaults remain off. Dispositions, authored 0B aspirations,
generosity and prudence are host configuration. The player cannot edit them through chat.

An explicitly creative turn supplies the active character's disposition, scoped Life
attitudes, selected episodes, accepted promises and heard dialogue. Stable disposition
text precedes changing turn state; the actual `Current request:` stays outside JSON.
Focused turns have a 64-token output allowance. The model proposes one first-person
mental stance under a bounded speech schema and single-sentence pattern. This is an
aspiration/preference, not a new event, biography, inventory item or permission. Ordinary
factual questions retain the existing source-bound enum and unknown-status path.

The host mechanically validates the sentence and rejects known forbidden action/history
patterns. This lexical check is **not semantic grounding proof**. Free subjective speech
can still be inappropriate; games needing a closed set of speech should keep this path
off. Models never appraise their own numeric trust, create assets, transfer property or
make their mental stance an authoritative world fact. Gated lore stays withheld by the
existing host compiler. Stock/weather/birthplace questions do not enter this creative path.

A new stance is pending until the entire approved utterance is confirmed **heard**.
Partial/interrupted prefixes do not establish an unheard opinion. Complete heard stances
are atomically written to `viewpoints.json` in the host state directory and included in
branch checkpoints. Keys contain NPC, listener and normalized topic. Another player or NPC
gets a separate entry. A later matching question reuses the canonical stance with zero
additional LLM calls. Dreams/hope and hobby aliases share topics; arbitrary paraphrases
are not semantically clustered. Listener-directed feelings use guarded/cautious/warm
host trust bands, allowing a new stance after a meaningful band crossing. Existing general
stances are immutable within a branch; profile edits guide future topics. New game clears
the store, branch load restores it, and old checkpoints without viewpoints remain valid.
The 128-entry ceiling refuses new entries rather than evicting established opinions.
A recording replay does not commit a viewpoint or create another memory.

`character::roleplay::Viewpoints` is a pure bounded store with `confirm`, `find`, `recover` and
`checkpoint`; its confirmation argument must come from the host's delivery receipt.
The `studio-viewpoints` local fixture exposes the same contract through all bindings.
It performs no model, audio or filesystem work. The example host owns atomic disk storage.

# Reply presentation

Sending conversation hides the previous reply immediately and shows **Thinking…** while
the replacement is pending. It also hides the provisional fallback so an old counter
line does not look like the new answer. **Use instant reply** reveals the current 0B
fallback and invalidates the pending upgrade. Local text is cancelled; online reservations
keep their existing accounting and an already dispatched remote call may still finish.
The usual Read/Play lifecycle still controls heard memory. Old confirmed lines remain
available in Recent events and the optional voice backlog.

# Evidence and limits

Twelve Python/Rust conformance cases cover complete delivery, immutable reuse, actor and
listener isolation, partial delivery, invalid claims/actions, cutoffs and bad provenance.
Two thousand mutations exercise bounded parsing. `eval/studio/personality-host.json` covers nine
original mock workflows including branch/reset, invalid model action rejection, factual
path preservation and zero extra calls for a repeated topic. The host test also checks
atomic sidecar persistence. `eval/studio/personality-browser.json` covers five real Firefox
workflows: immediate hiding, delayed generation, new reply display, instant fallback and
character settings, with zero page errors. These mocks do not measure physical playback.

The retained first local Qwen 3B paired cohort (`personality-model-*`) scored relevant
subjective replies on 2/3 candidate questions versus 0/3 baseline questions. Its tea reply
was two sentences and fails the host validator; raw outputs are preserved. After tightening
the pattern and removing distracting authored examples from the model prefix, a **fresh
three-question holdout** (`personality-model-v2-*`) scored 3/3 relevant, mechanically
accepted candidate replies versus 0/3 baseline replies. The merchant chose tinkering,
expressed a simple tea preference, and the keeper expressed a future-oriented stance.
Candidate requests reported 799–807 prompt tokens and 16–25 output tokens. These are
small, original lexical-scoring cohorts, not general conversation or hallucination rates.
All twelve inference calls were serial local calls with four configured threads and
`--cap 0`. Wall times are retained, but background compilation means they are not a clean
latency benchmark. Existing roleplay evaluation artifacts are unchanged.
