# Social turns in the character studio

With **Settings → Characters → Enable personality turns** enabled, greetings and
wellbeing no longer fall through to the merchant/keeper role sentence. Previously an
unrecognized personal question reached an enum containing versions of that sentence;
the model could not answer “How are you?” under that contract.

The optional `character::roleplay::conversation_route` helper classifies bounded English small talk
before the example chooses its speech contract. Greetings, thanks, farewells, wellbeing
and short explanations now use original host-authored lines with **zero model calls**.
They use current scoped Life trust and scene atmosphere, and avoid the last two confirmed
replies to this listener. Mood fallbacks distinguish guarded, neutral, warm, busy, tense
and emergency conditions. An unhandled question asks for clarification instead of
returning the counter description when personality turns are enabled.

“Why is that?”, “Tell me more” and similar follow-ups use this NPC's last confirmed reply
to this player. A mood explanation refers to current trust/atmosphere; an established
viewpoint uses the host-authored personality reason. Profiles may include an optional
`reason` first-person sentence (at most 180 characters); older profiles use original
per-character defaults. Edit it alongside traits to keep a custom character coherent.
This expresses a disposition; it does not invent a remembered historical cause.
No heard context from another player or a model draft supplies the follow-up topic.
Moods and explanations are normal heard conversation memory but **not permanent viewpoint
entries**, and they do not overwrite established stances.

Dreams, hobbies, preferences and other supported personal viewpoints still use the
existing delivery-confirmed model path and saved stance store. More aliases include
“What do you enjoy?” and favorite/favourite questions. Subjective requests explicitly
bind the reply's first-person voice/question subject to the NPC and name its listener
separately. Engine/host labels are metadata, not people. Generated subjective replies
that mention those labels as characters or exactly repeat a recent confirmed line are
rejected; the host retains the current fallback without another model call. Debug activity
explains the rejection. This is mechanical checking, not semantic truth or paraphrase proof.

Factual queries—including mixed greetings about stock, prices, birthplace, weather or
secrets—retain their source-bound path. The pure routing helper does not appraise trust,
execute actions, create world facts or grant tools. It uses finite English hints, not
unrestricted intent understanding. `previous` is a host-confirmed topic identifier.
`studio-conversation` exposes `{input,previous}` → `{kind,topic}` through the local adapter
and all bindings. Old conformance cases and default personality-off behavior are unchanged.

# Evidence

Twenty-two independent Python/Rust routing cases cover the reported greeting, wellbeing
aliases, mixed factual requests, follow-ups, personal topics and malformed input. Two
thousand mutations perform no I/O or world actions. `eval/studio/conversation-host-final.json`
records twelve production-host workflows: three distinct immediate greetings, zero-call
mood/explanation turns, saved-stance consistency, generated duplicate/metadata rejection,
player isolation and strict factual schemas. `eval/studio/social-conversation-browser.json`
records five Firefox workflows for the three-turn conversation, immediate previous-reply
hiding, reply origins and no permanent mood entries. These use original mocks, not
physical audio.

Three retained local 3B experimental cohorts (`conversation-model-*`, `-v2-*`, `-v3-*`)
exposed limitations of freely generated mood explanations: host-as-person references,
copying the previous mood, and an invented storm. Their raw outputs and imperfect earlier
lexical reviews are retained. A relevance score alone missed these failures. The final
implementation therefore uses host-authored, state-selected routine lines rather than
asking a model to invent a causal explanation.

The final token-first paired holdout (`conversation-model-final-*`) compares a fresh
three-question merchant sequence in the old role-only route against the new routine path
plus a generated hobby. Routine candidate turns issue no request; the hobby still uses
Qwen 3B. Raw provider and delivered lines are separate, and confirmed viewpoint entries
are checked after heard delivery. Local inference is serial, with four configured threads
and `--cap 0`. These tiny original cohorts are routing/acceptance evidence, not a general
social-intelligence, mood-accuracy, hallucination or latency benchmark. All prior evaluation
artifacts remain frozen.

In the final cohort, baseline relevance was 0/3 with three model calls; candidate
relevance was 3/3 with one model call (the hobby). The hobby was accepted and saved
only after heard confirmation. Its request reported 1,060 prompt tokens and 25 output
tokens. The two routine turns made zero model requests. This is a three-question lexical
check, not a broader accuracy rate. `conversation-model-final-audit.json` verifies these
counts and corrects an inherited “two-question” label in the retained review's limits
string; the raw/review files remain unchanged.

Sentence continuations, longer input, summaries and host-registered stage actions are described
in [conversation extensions](STUDIO_CONVERSATION_EXTENSIONS.md). Trade price replies now omit
private player balances and confirmed sales receive a deterministic merchant acknowledgement.
