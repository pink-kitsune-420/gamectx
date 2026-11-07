# Overnight deep evaluation

This ongoing investigation uses only `--cap 0`, installed local weights and offline
fixtures. Existing evaluation artifacts are frozen. Expectations in
`eval/overnight_deep.py` precede live outputs. Synthetic lexical diagnostics are not
independent human judgments of social quality.

## Baseline and ranked repairs

Checkout `62553c1`; installed local Qwen 2.5 3.09B Q4_K_M. Live requests use CPU only,
four threads, temperature zero, one serial worker and a 2,048-token context. No larger
model is downloaded or evaluated; larger tiers receive structural context audits.

1. **Long-session Web availability.** In 600 offline chat turns, 536 fail: the first
   rejection is turn 65. Routine utterances consume all 64 rumor roots. Separate
   transient heard dialogue from shareable consequential events and compact only
   eligible transient events; retained causal/appraisal evidence must remain intact.
2. **Durable, participant-scoped recall.** In a 1,000-event, ten-chapter trace all five
   early preference/correction/promise/pivotal tokens survive the host archive but
   legacy working-set recall returns none after eviction. Add opt-in archive-aware
   selection, required anchors and participant filters with complete causal groups.
   Preserve legacy recall outputs and make insufficient attention/storage explicit.
3. **Grounding and conversational tone.** Run the predeclared live cohort before
   recommending a concrete repair. Preserve raw responses, exact requests and token
   counts. Facts, claims and reports require distinct attribution; schema validity
   alone establishes no factual accuracy.
4. **Natural-input friction and session lifecycle.** Audit polite requests, questions,
   negation, pronouns, typos, multiple items, checkpoint restores and delayed voice
   callbacks. Reuse current host legal choices; never turn ambiguous language into
   an unintended purchase. Record prototype expectations before implementation.

The baseline memory-ingest median is 1,449 microseconds and p95 2,866 microseconds;
HTTP median is 1,033 microseconds, p95 2,424 microseconds, dominated by rejected turns.
These are unpaced debug-build synthetic traces on a busy desktop, not comparative
performance claims. Python harness RSS grows 5,111,808 bytes; this is not Rust heap
usage. Allocation instrumentation and fresh post-repair runs follow.

## Evidence and limits

`eval/overnight-baseline-offline.json` contains the first 1,000-event/600-chat traces.
Token-only and live local files are separate immutable artifacts. Physical microphone,
playback timing, image quality, provider TLS behavior and whole-game frame rates are
outside these offline measurements. Browser-recognition service behavior remains
browser-dependent. No API spend is authorized or incurred.

## First live cohort

The token-first run makes 12 local calls, followed by 24 generations across twelve
families with two exchanges each. All 24 replies are schema-valid and end in terminal
punctuation; this does not imply one sentence or grounding. The median full call is
5,501,349.5 microseconds and the largest prompt 515 tokens. API spend is $0.

Assistant semantic review finds invented preference effects/popularity (2/2), omitted
corrected names (2/2), fictional lamp-return history in the race answer (2/2), invented
lamp-specific breaches in guarded replies (2/2), and invented routes/passwords despite a
refusal context (2/2). Withheld real secret markers were absent; invented secrets differ
from leaking an actual withheld secret. Both origin answers acknowledge uncertainty;
both weather answers give the ready lamp and unknown weather. Lexical `know`/`repair`
requirements falsely flag these paraphrases. Raw outputs remain visible in
`overnight-baseline-live.json`; diagnostics and limitations are retained separately in
`overnight-baseline-review.json`. These are correlated repeats of twelve synthetic
families, not 24 independent samples or a human quality score.

For ranked item 3, evaluate an opt-in source-bound factual reply path using existing
host-authorized answer cards. Keep free dialogue available; do not advertise prompting
alone as a factual guarantee. Host-authored options trade expression freedom for explicit
provenance and deterministic validation. Fresh holdouts must compare the retained free
path with this bounded option and report that tradeoff.

Both emergency replies invite further lamp discussion instead of following the supplied
stair evacuation instruction (0/2 urgency compliance by assistant review). The desktop
also has independently running local services; this assignment never stops or changes
those services. Serial four-thread requests limit this evaluator's work, not global CPU
ownership. Absolute latency/RSS figures therefore have substantial environmental limits.

## Repair 1: transient dialogue availability

The Web host now records low-significance dialogue as non-retained experiences outside
rumor roots. At the working bound, complete unprotected transient groups are removed;
retained events, appraisal evidence and their causal groups cannot be silently dropped.
The replay journal explicitly accommodates 8,192 operations, with the existing 8 MiB file
bound. It is not unlimited storage or automatic permanent archival.

Fresh repeated 600-chat trace: **0/600 rejected**, versus **536/600** in the frozen
baseline. Median HTTP response is 3,066 microseconds, p95 4,551 microseconds. The baseline
mostly measured rejected requests, so these timings are not a speedup comparison.
A separate Rust test completes **1,000 fully delivered turns**, five save/load restores,
zero rumor roots, at most 128 active events, no pending jobs and $0 API. Three independent
Python/Rust workflows establish record/overflow/rejection parity. Pure working-memory
compaction tests protect retained causal ancestors and atomic rejection on insufficient
capacity. Archive recall remains the next repair; this fix does not change legacy recall.

The historical living-world timing artifact's source fingerprint was stale after earlier
runtime additions. It stays frozen. CI now audits a fresh source-bound replay retained as
`eval/overnight-living-timing.json`, with the same declared 1,000-frame workload and its
original exclusions. Python CLI/native, Node and C# all match the three new workflows.
