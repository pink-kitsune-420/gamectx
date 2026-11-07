# Roleplay investigation

This short investigation uses public publisher guidance and research abstracts. No
third-party implementation, character card, prompt collection, setting or artwork was
imported. The Lantern Vault characters, fables, code and test scripts are original.

## What people do

Roleplay combines describing what a character does with speaking as that character.
Players differ in how much they use either approach. Personality, intentions and the
other participant's attitude give a conversation direction; asking a specific question
usually works better than expecting a generic interaction to cover everything.
[The public Basic Rules guidance](https://www.dndbeyond.com/sources/dnd/basic-rules-2014/adventuring)
describes both descriptive and active roleplaying. This supports accepting stage
directions without manufacturing words for the player, and keeping the player's
decisions under their control.

Shared expectations also matter: people have different comfort boundaries, including
for apparently ordinary fictional subjects. The public description of
[Consent in Gaming](https://www.montecookgames.com/store/product/consent-in-gaming/)
explains that difference. We did not reproduce its forms or download its book. In the
example, explicitly marked `OOC:` and `((...))` notes stay outside NPC knowledge and
world actions. These markers are an escape from the fiction, not a complete consent
or moderation system.

## What AI adds, and where it fails

AI roleplay typically supplies a character profile, situation, selected memories and
conversation history. [Generative Agents](https://arxiv.org/abs/2304.03442) studies an
observation, memory, reflection and planning architecture. That is a useful conceptual
separation; generated reflection is still not evidence that an event happened.

Fluency alone does not establish fidelity.
[PersonaGym](https://arxiv.org/abs/2407.18416) evaluates several dimensions of persona
behavior; [Persistent Personas?](https://aclanthology.org/2026.eacl-long.246/) examines
persona drift in extended conversations. Our response is to retain confirmed stances,
scope them by character and listener, and include selected established views in the
next turn. We do not ask the model to reconstruct identity from an ever-growing chat.

Memory needs correction and abstention, not just similarity search.
[LongMemEval](https://arxiv.org/abs/2410.10813) distinguishes extraction, reasoning,
temporal reasoning, updates and abstention.
[Lost in the Middle](https://arxiv.org/abs/2307.03172) shows that retrieval from long
contexts depends on where information appears. Our engineering inference is to keep
the current request explicit, preserve exact confirmed name corrections, and select
small attributed memories. The papers do not prove that this particular compiler is
better; our separate host tests measure that.

Knowledge boundaries need separate enforcement.
[RoleFact](https://aclanthology.org/2024.findings-emnlp.846/) studies hallucination when
a roleplaying model uses information outside its character's scope. We provide only
selected perceived facts and distinguish reports, player claims, invented tales and
unknowns. Secret gates and world actions remain deterministic host decisions. A
fictional story cannot become the NPC's biography merely because it was spoken.

Agreement is another trap.
[Towards Understanding Sycophancy](https://arxiv.org/abs/2310.13548) studies models
favoring agreement with users. NPC directions therefore preserve motives and established
views, and prohibit deciding what the player thinks or does. These prompt instructions
reduce an invitation to drift; they are not a semantic safety proof.

## What changed here

The host now distinguishes factual wording, personal expression, social conversation,
explicit fiction and negotiating an oversized request. With AI wording enabled, the
connected model supplies NPC wording, including greetings and short counteroffers.
Validated cached wording requires matching character, listener, scene, profile,
attitudes, player claims, current words, recent heard replies and model. Automated
lines provide continuity when generation is unavailable or fails validation.

Stories use original small plot seeds with a problem and resolution. Every sentence is
checked before conservative repeat pruning; raw model text and omitted indices remain
inspectable. New consequences, names, quantities and negations are protected by
regression cases. Semantic paraphrases can still escape this heuristic.

[PersonaEval](https://arxiv.org/abs/2508.10014) also cautions against treating an AI
judge as a reliable substitute for role attribution. We retain raw conversations and
report mechanical acceptance separately from manual quality observations. The small
local cohorts establish specific regressions, not that a 3B model can play any character
convincingly. See [the implementation and evidence](STUDIO_EXPRESSION.md) for counts,
remaining limits and reproducible offline checks.
