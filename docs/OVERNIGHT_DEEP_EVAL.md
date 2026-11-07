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

## Repair 2: scoped original-event recall and erasure

A fresh release-mode campaign ingests **4,000 events**, across merchant, keeper, guard
and companion Life instances, **1,000 events each**, ten chapters, two player subjects
and **40 checkpoint restores**. The guard's Life is explicitly host-enabled for this
memory test; the ambient pack preset still needs no model or mandatory memory module.
Every chapter tests required preference, corrected-name, promise and pivotal anchors.
Legacy active-set recall passes **0/80** declared participant/anchor probes after
eviction; selected original-event recall passes **80/80**, with no cross-player episodes
and complete Vira→Vela correction pairs. This is deterministic retrieval, not a live
model's ability to apply every recalled event. Five player_1 anchors use five slots;
focused hosts choose fewer task-relevant anchors within their smaller budget.

The first selected implementation performed needless ranking even when required anchors
filled the packet. An early return reduces allocator calls **3,682,978→3,065,858** in
this same trace, with identical 80/80 results. Peak requested live allocation remains
**3,769,707 bytes**; this excludes allocator metadata, process mappings and model RAM.
Merchant checkpoints grow **39,432→330,455 bytes** over ten retained consolidation
ranges: bounded durable context grows intentionally, rather than exhibiting constant
memory. The 16-range/524,288-byte checkpoint limits remain explicit; disk archive recall
and unlimited campaign retention are host responsibilities. Measurements are one
uncontrolled run per variant; no general latency reduction follows.

Eleven independent selection cases, three direct compaction/erasure tests and 2,000
selection mutations cover missing/foreign/duplicate anchors, zero caps, irrelevant noise,
causal overflow and non-mutating rejection. Erasure now includes archived descendants in
tombstones, avoiding recovery of a corrected child after deleting its causal parent.
Fifteen shared Python/native/Node/C# workflows include the new transient/selection cases
and a separately authored **132-operation** causality holdout. That holdout verifies the
trust arc **0→−20→−10→0→10→20**, twelve attributed reports with at most two hops, no new
reports in **100 cyclic noise ticks**, ten emergency guard frames and **zero player_2
attitude delta**. Existing initiative, living-world and attitude-gate suites remain green.

## Repair 3: authorized factual speech and a fresh paired cohort

The fresh suite predeclares twelve different question families and host facts (Sera,
copper/violet preferences, an unresolved measuring-string promise, unknown snow, an
attributed crash, an unverified race claim, warm/guarded trust, emergency evacuation,
interruption habits and denied code invention). Each arm has two exchanges: **24 free
and 24 source-bound generations**, preceded by **24 token-only calls**, using the same
installed CPU-only Qwen 3B with four threads. The source-bound arm offers host-authored
variants; it is not an unconstrained model-intelligence improvement.

Predeclared required-word checks pass **12/24→24/24**; all 48 replies have valid shape.
Exact authorized-variant validation passes **24/24** in the bounded arm and revalidates
against Rust's current-source/revision path. Assistant semantic review identifies explicit
unsupported effects/history/unlisted stock/invented codes in **10/24 free replies versus
0/24 host-authored replies**. This is not blinded or independent human evaluation.
Both free player_2 replies choose violet correctly but promise a ready unlisted lamp;
both free corrected-name replies use Sera but invent repair/borrowing history. Free
emergency replies both include the west-stair instruction. Other task/tone failures and
all raw outputs remain visible in the retained review and generation artifacts.

The original forbidden-word heuristic passes **24/24 in both arms**, missing the invented
entry code `123456`; it must not be reported as a hallucination-rate measurement. Unknown
sky paraphrases also fail its narrow `know` requirement despite correct uncertainty.
We preserve those diagnostics rather than changing the gold after observing replies.

The first harness exited 143 after 33 recorded generations, with cause unverified.
`overnight-grounded-live.json` remains incomplete and frozen. The completed artifact
links its SHA-256, reuses those 33 responses without rerunning them, and reserves each
subsequent request before dispatch. At most **one additional in-flight generation** was
unrecorded; the completed cohort has 48 recorded responses plus that uncertainty. No
API spend occurred. This is operational recovery, not a new unseen accuracy cohort.

The actual grounded Studio provider body for 3B uses **704 prompt tokens**, reserves
128 output and 256 guard tokens, and fits the 2,048-token runner context. Actual request
bodies for 1/1.5/3B are **3,730/3,732/3,731 bytes**; 8/30/70B are
**5,113/5,114/5,114 bytes**. Byte counts are not larger-model tokenizer counts. Focused
caps are 1/1/2 episodes and one exchange; expanded caps are 3/4/6 episodes and 2/3/3
exchanges. The audit asserts stable first-two-message prefixes across consecutive turns,
actual configured model IDs, zero-call 0B routing, scoped episodes and accepted promise
tasks. The service UI does not automatically expose Life tools; existing explicit
companion/spotlight tools and gate policies retain their separate conformance checks.
No 1B, 1.5B, 8B, 30B, 70B or frontier model was evaluated live.

## Repair 4: command comfort and delayed participant scope

A predeclared **26-case** independent grammar/Rust parity suite covers polite forms,
purchase/travel/inspect aliases, recent `it`/`that one`/`him`/`her`/`there`, legacy look/wait,
questions, negation, typos, batches and malformed requests. It returns proposals from
current legal pairs; ambiguous input gives action-button clarification. The Web host
expires focus after four ticks and clears it on participant switches, travel and branch
loads. A fresh **41-request, 14-phrase** HTTP holdout completes with **zero unintended
purchases and zero interim STT actions**. It also checks price answers, a player_2 switch,
final voice routing and save/load. This measures a bounded English grammar, not general
NLU, multilingual coverage or a player accessibility study.

The audit then reproduced a separate race: a delayed player_1 `buy oil` arriving after
switching to player_2 executed a purchase for player_2 (HTTP 200). The fresh fixed holdout
returns **409, no purchase**, also rejects stale microphone opening and malformed
versions, and accepts a current-generation command. The browser now captures generation
before awaits and the host checks it atomically before mutation. Speech/session receipts
remain separate, preserving immediate barge-in. **Six fake-browser context lifecycle
checks** cover delayed headers, no automatic retry, receipt independence, invalid numeric
versions, an older in-flight poll, bounded restart recovery and recovery after a failed
fetch. Existing five audio lifecycle cohorts remain unchanged.

The first numerical social-arc fixture reused lamp-return narration while changing host
appraisal tags. It establishes arithmetic/provenance, not textual entailment. It remains
frozen. An additional semantic fixture supplies matching broken/repaired agreement text
and verifies the same 132 operations, twelve two-hop reports, 100 cyclic noise ticks and
player isolation. Host-authored event tags, rules and prose must be consistent; source
availability cannot establish that consistency automatically.
