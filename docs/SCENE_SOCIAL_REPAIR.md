# Scene awareness, varied tales and reliable delivery

The Studio now uses the existing `Locations` read-only lookup for current-place
questions before falling through to a generic reply. This is ordinary NPC knowledge
of its present location, with the host checking presence and current revision. Explicit
questions such as “Look around you” are dialogue; the player's ordinary look command
remains a world action. Room identity and visible cues do not establish outdoor weather,
another person's biography or the contents of their purse.

Connected AI wording remains enabled by the existing setting. If generation fails its
source checks, the fallback still contains the correct location or perceived scene.
This patch does not remove the conservative lexical validator: in the local run it
rejected both a correct location paraphrase with an unlisted courtesy word and a scene
description that invented history. These are separate outcomes, not two grounding errors.

## Tales

Each example NPC now has twelve original small plot seeds. Custom authored `story_seeds`
remain supported under the existing bound. Seeds supply a problem, change and ending;
the model writes the actual sentences and can invent explicitly fictional details.
The default is a complete tiny tale within the configured sentence budget, not an
unbounded multi-call narration.

The additive `studio-expression` operation `seed` accepts exact
`{candidates,recent,offset}`. There are 1–32 unique bounded candidate keys, at most 32
recent keys ordered newest first, and an offset between zero and one million. It
selects an unheard candidate first, then the least recently heard candidate. This
deterministic helper does not call a model or confirm that anything was heard.

The Studio derives recent keys from host-confirmed opening receipts in the existing
operation journal, scoped to NPC, player and save branch. A generated/unheard story
does not mark a seed used. Interrupted heard openings count as used; their unspoken
endings do not become memories. Normal journal bounds still apply.

For multi-sentence tales the private model schema now asks for
`speech:{opening,ending}`. `opening` starts with `Imagine ` and `ending` with
`In this tale, `, giving the grammar distinct setup and resolution slots. The additive
`tale` operation validates these two fields and returns ordered speech. Legacy arrays
remain accepted by the example for compatibility. All sentences are validated before
novelty pruning, and raw outputs remain in the evaluation artifacts.

## Playback and UI

The Thinking indicator follows a current undelivered text request. Fulfilled receipt
generation fences are cleared; completed/queued speech is not a pending model reply.
Browser Web Locks give live delivery one tab owner, so another inspecting tab cannot
race its autoplay receipts. Browsers without Web Locks retain foreground-only eligibility.

An audio-start refusal before any characters play now sends the additive example
delivery event `blocked` with `through:0,heard:0`. The host rejects rewinds after progress,
issues a fresh validated ticket for the unheard sentence, retains the remaining tale,
and marks `playback_blocked`. Autoplay skips that ticket until the player clicks Play.
This prevents both silent loss and an automatic retry loop. No extra model request is
made. Actual interruption still discards the unplayed tail.

## Evidence

All evaluations use cap zero and no paid API calls. `story_scene_cases.py` adds sixteen
Python/Rust conformance cases; existing outputs are unchanged. Shared binding parity
now covers ninety expression/wording fixtures. New fuzz coverage adds 2,000 mutations.

`scene-story-offline.json` selects twelve different plots across twelve heard tales;
`scene-story-restore.json` also checks the same least-recent choice after restoring a
checkpoint. These are deterministic selection checks, not semantic novelty scores.
`scene-story-tokens.json` precedes six actual local Qwen 3B calls in
`scene-story-live.json`: a greeting, location, perception and three tales. All three
tale packets were adopted with both parts. Manual review finds different plots; one
opening's awkward grammar remains visible. Two factual wording packets fell back as
described above. This small cohort does not establish general storytelling quality.

`scene_voice_check.mjs` checks six wait conditions and five playback-owner conditions.
Host/browser tests cover blocked ending recovery, preservation of the next sentence
and delivery across two tabs. The browser uses simulated audio and an original mock
Piper CLI, so it does not measure physical playback quality or guarantee every browser's
autoplay policy. See `scene-voice-browser.json` for the retained result.

These repairs address the immediate scene and delivery defects. The broader
[social-character gap audit](SOCIAL_CHARACTER_GAPS.md) distinguishes existing core
mechanisms from what the demo actually demonstrates.
