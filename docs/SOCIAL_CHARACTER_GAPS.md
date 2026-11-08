# From dialogue demo to a character who knows you

The main gap is integration. The library has substantially more machinery than the
Lantern Vault characters consistently use. A plausible sentence, a stored memory and
a trust meter do not by themselves make a coherent social interaction. This is a
source-code audit of the current implementation, not a claim that every feature below
is finished or a promise of human-like understanding.

## The loop a character needs

Each turn should update what the NPC actually perceived, attribute what it heard,
retrieve the experiences relevant to this person and situation, consider its current
needs and commitments, choose an appropriate conversational or physical response,
and remember only the result the host confirms. Later events must be able to change
the character's beliefs and plans for a reason. The spoken sentence is one output of
this loop.

| Area | What is needed | What exists here; what remains |
|---|---|---|
| Identity and voice | A stable name, background, values, temperament, interests, boundaries, vocabulary and contradictions. | Life definitions, authored personality profiles and saved viewpoints exist. The demo uses a small subset; speech often follows a narrow first-person prefix. Richer voice needs testing beyond adding more biography text. |
| Embodied perception | Knowing the current place; noticing visible changes, nearby people and audible events; uncertainty, occlusion and sensory limits. | Perceived facts, location lookup, visual occlusion and host-bound scene context exist. The GUI had bypassed location lookup. Current senses are finite host cues, not autonomous seeing or hearing of the whole simulated world. |
| Person-specific knowledge | Separate names, preferences, disclosures, expectations and beliefs for each player and NPC. | Scoped explicit name/correction/preference claims and participant attitudes work. Free-form facts, nuanced personal circumstances and their relevance are not comprehensively extracted. A player's statement remains attributed, not verified. |
| Episodic memory | What happened, with whom, where, when, why it mattered and which later event corrected it. | Life events, recall, consolidation, archives, causal references and branch checkpoints exist. Most ordinary demo chatter has low significance; it does not automatically become a durable meaningful episode. |
| Implicit memory use | Remembering without the player asking a memory quiz: adapting an offer, avoiding a disliked topic, following up on unfinished business. | Selected memories and explicit preference recall reach prompts. The demo does not consistently turn them into unsolicited appropriate follow-ups or actions. This is a higher priority than story variety. |
| Relationships | Familiarity, trust, affection, resentment, respect, fear and specific reasons; different attitudes for different people. | Configurable attitudes, appraisal rules, gates, groups and bounded ripples exist. The demo chiefly exercises trust/resentment and a few authored events. Conversational meaning is not yet a general reliable source of relationship changes. |
| Emotion and needs | Temporary fatigue, worry, relief or frustration, distinct from lasting personality; gradual recovery and competing needs. | State/appraisal and atmosphere mechanisms exist. The demo has limited transient social behavior, not a complete emotional or needs simulation. Numbers are game design parameters, not measured psychology. |
| Belief and uncertainty | Distinguish observation, rumor, player claim, inference, fiction and unknown; explain a mistaken belief and update it. | These provenance boundaries are represented. Free-form wording is still checked with conservative lexical rules, which can reject a useful synonym or miss a semantic error. No prose validator here proves general truth. |
| Conversation continuity | Track the actual question, topic, referents, unanswered parts, interruptions, shared assumptions, jokes and requests to elaborate. | Threads, references, receipts and bounded history exist. The demo still relies heavily on finite English routing. An unmatched phrase can be assigned a generic unknown meaning before the model gets a fair chance to answer. |
| Social intent | Decide whether someone wants information, reassurance, advice, playful banter, help, disagreement or silence. | Some explicit social routes and personality turns exist. Subtext, indirect requests, sarcasm and ambiguous multi-part turns are limited. This needs evaluated intent selection, not an ever-growing list of stock lines. |
| Goals and commitments | Immediate goals, longer ambitions, promises, debts, priorities, obstacles and reasons to reconsider. | Promises, progress records, loans, goals and initiative candidates exist. Saying “I hope…” does not yet give every demo aspiration a persistent plan or make it compete with daily work. |
| Grounded actions | An NPC can offer help, refuse, lend, gift, move, fetch or warn only when resources, knowledge and host permissions allow it. | Reaction policies and a few concrete trade/loan/gift/support handlers work. A broader action catalog and confirmed outcomes are needed. Model narration must never manufacture the action's success. |
| Initiative and routines | NPCs notice a relevant event, resume unfinished business or seek someone out, with cooldowns and reasons. | Routine, initiative, backoff and social/call quotas exist in the core. The GUI is predominantly player-driven dialogue with arrival greetings; it does not demonstrate a full autonomous social day. |
| Multi-party society | Know who addressed whom, who heard which part, who has the floor, private versus shared knowledge, reputation and attributed rumors. | Party perspective, floor control, LAN players, groups and rumor hops exist. Rich alliances, conflicting interpretations and negotiated group conversations need more authored situations and sustained live evaluation. |
| Expressive behavior | Vary phrasing, length, gestures, humor, silence and disagreement while keeping a recognizable character. | AI wording and bounded personal/social/fiction modes exist. Strict sentence prefixes and source vocabulary currently trade expressiveness for easier checking; this contributes to repetitive or stiff replies. |
| Culture and boundaries | Personal etiquette, taboos, comfort preferences, consent and an OOC escape, without deciding the player's feelings or choices. | Authored facets/policies and explicit OOC notes exist. The demo is not a general culture or consent simulator. An in-character limit should reflect personality and circumstances. |
| Delivery and presence | Natural pacing, complete thoughts, voice identity, interruptible playback and correct heard offsets. | TTS/STT, sentence tickets, barge-in and archives exist. The current fixes add retry for audio-start failure and one playback owner across tabs. Physical audio/browser behavior still needs separate testing from text quality. |
| Persistence and continuity | Save/restore the same person without mixing players, branches or unheard replies; correct or forget information deliberately. | Scoped checkpoints and confirmed operation journals exist. Bounds are explicit. More memory capacity alone does not select the right memories or make a returning-player scene feel natural. |
| Model/runtime adaptation | Small focused turns, richer context when useful, optional supported reasoning, strict spending and call limits, cancellation and fallback. | Tier hints, manager profiles, spending, request fences and profilers exist. More thinking or a larger model does not repair missing facts, bad routing or a contradictory delivery state. |
| Evaluation and author tools | Inspect what the NPC knew, why it chose a response, what it remembers and whether behavior changes appropriately over time. | Context/execution profilers, conformance, fuzz and local raw-output cohorts exist. Current live cohorts are small. Structural acceptance must remain separate from helpfulness, emotional appropriateness and sustained character quality. |

## What to prioritize

1. **Finish the reliable scene/conversation loop.** Every reply should have the right
   person, current location, perceived facts, actual question and confirmed dialogue.
   Remove generic fallback routing where the answer is already available. Fix voice
   races so delivered and unheard text stay distinct.
2. **Make memory useful during ordinary conversation.** Preserve meaningful events and
   corrections, retrieve them for the current situation, and evaluate relevant implicit
   use. Avoid repeating a memory just to demonstrate that it was stored.
3. **Connect feelings, relationships and goals to choices.** A reserved, grateful or
   resentful NPC should choose different appropriate offers and follow-ups, with a
   durable reason and bounded change. Personality should affect behavior as well as
   adjectives in a prompt.
4. **Broaden validated social intent and action selection.** Keep one focused model
   task where possible, with typed proposals and current host permissions. Add new
   behavior through a reusable action vocabulary rather than special-casing every
   sentence a player might type.
5. **Expand the society only after the individual works.** Off-screen routines,
   independent goals, rumor, group disagreement and multi-party conversation become
   valuable when each participant's knowledge and continuity are dependable.

A useful acceptance scene is a returning player whom the NPC recognizes by a corrected
name, remembers an unfinished concern about, notices only a host-confirmed visible
condition on, and offers an actually available kind of help. A second player should
receive a different response. Interrupting midway should not make either person
remember the unspoken ending. This tests more of the character loop than asking for
another story or checking whether one sentence sounds warm.

The current patch addresses location routing, speech state and varied complete tiny
tales. It does **not** complete this whole map. The next central milestone is a tested
vertical slice of implicit memory, topic continuity and motivation-driven action.
