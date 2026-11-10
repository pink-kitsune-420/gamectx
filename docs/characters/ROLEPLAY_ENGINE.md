# Roleplay engine (0.4.0a, stages 2.1–2.2)

`gamectx::character::roleplay::engine` gives any game one configurable roleplay brain per
AI character. It brings together the roleplay policies first built for Lantern Vault:
message chunks, out-of-character notes, stage actions, the command grammar, disclosures
and recall, encounter-aware greetings and social lines. It adds **props**, a **world codex** (places, entrances, factions,
events, quests) and a **character sheet** (persona, speech style, services, goals, needs,
relationships, secrets, opinions). Every part is switchable and reworded through
configuration instead of game code.

The engine never executes a game action and never calls a model. It returns an
*outcome*: a reply to speak, a proposed host decision, a clarification, a stage
direction, an out-of-character note, or a request for a model reply that your host
compiles. Your host validates and executes; the engine only proposes.

## The cascade

Each player message passes through these stages in order. A stage answers only when one
of its line rules matches. Otherwise the message falls through to the next stage.
Turning a stage off never changes what an earlier stage does.

| # | Feature | Answers | Outcome `route` / `kind` |
|---|---|---|---|
| 1 | `chunks` | Slices long messages (`max_chars` 512–4096, default 1024); a multi-chunk message is *long* and skips the question stages | — |
| 2 | `out_of_character` | `OOC: …`, `out of character: …`, `((…))` | `out_of_character` / `note` |
| 2a | `addressing` | Turns [not meant for this character](#addressing): out of earshot, too far to follow, spoken to someone else, or left unfinished | `addressing` / `silent`, `gesture`, `action` or `pause` |
| 3 | `content` | Your content settings: a topic on the `avoid` list is redirected, never answered | `content` / `reply` or `clarify` |
| 3a | `species` | Animal non-verbal responses based on temperament (`calm`, `wary`, `friendly`) | `species` / `reply` |
| 4 | `stage_actions` | `*registered stage action*` from your [`RoleplayActions`](../studio/STUDIO_CONVERSATION_EXTENSIONS.md) registry | `stage_action` / `action` or `clarify` |
| 5 | `props` | A present prop's interaction phrase, starred or plain (`*rings the bell*`, `ring the bell`) | `props` / `action` or `clarify` |
| 6 | `entrances` | "Go / walk / leave through the door" for a visible entrance | `entrances` / `action` or `clarify` |
| 7 | `services` | A service request phrase ("sell me bread") whose `when` holds | `services` / `action` or `clarify` |
| 8 | `directions` | `*approaches X*`, `*enters Y*` against host-listed `targets` (and attaches travel decisions when matching listed choices) | `direction` / `direction` or `clarify` |
| 8a | `reactions` | Loans, gifts, support, loan repayment and debt queries ("what do I owe?") | `reactions` / `reply` |
| 9 | `perception`, `entrances`, `scenery`, `persona` | Where are we / what do you see / hear / your name; how do I get out; smell, light, temperature; what do you do, tell me about yourself | `perception`, `entrances`, `scenery`, `persona` / `reply` |
| 9a | `bio` | How old are you, when is your birthday, what is your trade, what are you good at / are you good at X, what do you know about X, are you hurt, do you have family | `bio` / `reply` |
| 9b | `places` | "What is nearby?", "How do I get to X?" (navigates via connected visible entrances) | `places` / `reply` |
| 9c | `things` | "What is here?", "What is X made of?" (describes present scene items and materials) | `things` / `reply` |
| 10 | `props`, `entrances`, `scenery` | Look at / describe / what is a present prop, a visible entrance, or a spot | `props`, `entrances`, `scenery` / `reply` |
| 11 | `commands` | The bounded English command grammar over this turn's legal `choices` | `command` / `action` or `clarify` |
| 12 | `disclosures` | A single-sentence "My name is …", "Call me …, not …", "I like …" (remembered per listener, `max_preferences` 1–16, default 3) | `disclosure` / `reply` |
| 13 | `recall` | "What is my name?", "What do I like?" | `recall` / `reply` |
| 14 | `secrets` | A secret's phrases: the secret when its `when` holds, else its cover story or a refusal | `secrets` / `reply` |
| 15 | `opinions` | An opinion's phrases ("taxes") | `opinions` / `reply` |
| 15a | `viewpoints` | Stances on specific topics ("Tell me your dreams / hobbies") | `viewpoints` / `reply` |
| 16 | `relationships`, `factions`, `events`, `quests` | "Who is / tell me about / what about X" for a known person, faction, event, or a quest this character gives | same as the feature / `reply` |
| 16a | `recipes` | "Can you make X?" (reports crafting feasibility, requirements and inputs) | `recipes` / `reply` |
| 16b | `communities` | "Will you trade?" (reports market trading availability and local currency) | `communities` / `reply` |
| 17 | `services`, `goals`, `timeline`, `events` | "What do you offer / sell?", "What do you want?", "What is the news?", "Any events?" | same as the feature / `reply` |
| 18 | `greetings` | Greetings planned from confirmed encounters (`long_gap_turns`, default 12) | `greeting` / `reply` |
| 19 | `social` | Greetings (when `greetings` is off), thanks, farewells, wellbeing (with `goals`: pressing needs), "why?" | `social` / `reply` |
| 20 | `model` | Everything else, with a ready-to-send [model request](#model-requests-and-replies) | `model` / `model` |
| 21 | `social` | The authored fallback line when `model` is off | `fallback` / `reply` |
| — | — | Nothing answered | `none` / `silent` |

Action outcomes (stage actions, props, entrances, services, commands) also carry a `norm`
when the current place has a matching [place norm](#world-codex); the decision is never
blocked or changed by it.

## Configure it

As JSON (every part except `version` and `character` is optional):

```json
{"version":1,"character":{"id":"baker","name":"Mira"},
 "features":{"greetings":{"enabled":true,"long_gap_turns":20},"model":{"enabled":false},
             "props":{"registry":{"version":1,"props":[]}}},
 "variety":{"recent":2},
 "pack":"en",
 "phrases":{"ask_name":{"exact":["who are you","what are you called"]}},
 "lines":[{"id":"thanks","intent":"thanks","variants":["Any time, friend."]}]}
```

Or with the typed builder, which validates every step against the same rules:

```rust
use gamectx::character::roleplay::engine::{EngineConfig, Feature, RoleplayEngine};
let config = EngineConfig::new("baker", "Mira")?
    .with_feature(Feature::Model, false)?
    .with_setting(Feature::Greetings, "long_gap_turns", 20)?
    .with_recent(3)?;
let engine = RoleplayEngine::new(config);
# Ok::<(), gamectx::Error>(())
```

`EngineConfig::to_value()` returns the normalized JSON with every default written out,
and `from_value` reads it back unchanged. Both snippets above run as tests in
`tests/roleplay_engine.rs`.

## Wording: packs, phrases and lines

All wording lives in a **line book**. `pack: "en"` loads the built-in English pack
(`src/character/roleplay/engine/en.json`). It contains all 108 Lantern Vault greeting
and social lines, checked offline by `eval/character/roleplay_engine_cases.py`. Use
`pack: "none"` to supply only your own phrases and lines, for example in another
language. With no lines, every conversational turn goes to `model`.

- **Phrases** decide which intent a message has. A matcher is
  `{exact, prefix, contains, unless}`. Each is a list of phrases, compared after
  lowercasing and replacing punctuation with spaces. `unless` vetoes the match, `prefix`
  yields the rest of the message (a prop or target name), and `contains` matches whole
  words. A config entry replaces the pack's matcher for that intent.
- **Lines** are `{id, intent, when?, variants, introduces?}`. The first rule for the
  intent whose `when` conditions hold answers. A `when` condition is a list of allowed
  values (strings, booleans or integers), or `{"min","max"}` for an integer such as
  `trust`. Conditions may also test your own live flags as `state.<id>` (see
  [a turn](#a-turn)). Templates use `{var}` names from a fixed set (`name, scene, place,
  atmosphere, trust, relationship, meeting, needs_introduction, sight, sound, props,
  prop, description, note, slot, value, old, corrects, polarity, known, reason, exits,
  status, destination, through, heard, smell, light, temperature, role, summary,
  services, label, text, need, person, kind, faction, stance, event, events, timing,
  quest, stage` and the `has_*` flags). A variant using a variable that this turn lacks
  is skipped. A config line with a pack line's id replaces that line in place; new
  ids are tried before every pack line.
- **Variety:** the engine speaks the first variant that is not one of the listener's
  last `variety.recent` *fully heard* replies. Variants containing one of the sheet's
  `speech.avoid_words` are never spoken.

## Props

A location can host props: described objects that people can look at and interact
with. Register them once in a [`Props`](../FORMAT.md#roleplay-engine-v1) registry. Each
turn's scene lists which props are present now, each with an optional current-state
note:

```json
{"id":"bell","label":"the brass bell","aliases":["bell"],
 "description":"A small brass bell hangs above the door.",
 "interactions":[{"id":"ring","phrases":["rings the bell","ring the bell"],
   "decision":{"action":"use","target":"prop:bell"},"narration":"rings the brass bell"}]}
```

Real outcomes from `conformance/roleplay-engine/`:

```text
What is the bell?   → {"route":"props","kind":"reply","intent":"describe_prop","line":"prop_note","text":"A small brass bell hangs above the door. It rang twice this morning.","prop":"bell"}
*stokes the oven*   → {"route":"props","kind":"action","prop":"oven","interaction":"stoke","decision":{"action":"use","target":"prop:oven"},"narration":"stokes the oven"}
*rings the bell*    → {"route":"props","kind":"clarify","intent":"stage_action_blocked","line":"stage_action_blocked","text":"That action is not available here.","prop":"bell","interaction":"ring"}
What do you see?    → "I can see the brick oven and the brass bell."
```

An interaction proposes its decision only when that decision is one of the turn's
`choices`, and is blocked otherwise (the bell above: `use prop:bell` was not offered). A
`null` decision is a pure gesture (`*warms their hands by the oven*`) that you can narrate
or record. Absent props are never described or used.

## Character sheet

A sheet is fixed, authored content about one character. Put it in the config as `sheet`
(or pack it in a [game data file](#saving-game-data-and-saves)). Everything is optional:

```json
{"version":1,
 "persona":{"role":"the town baker","summary":"I have baked bread here for twenty years, and I know every regular by name.","values":["honest work"]},
 "speech":{"formality":"casual","avoid_words":["sir"]},
 "services":[{"id":"sell_bread","label":"fresh bread","description":"A loaf of the day's bread.",
   "phrases":["sell me bread","i want to buy bread"],"decision":{"action":"buy","target":"item:bread"}}],
 "goals":[{"id":"find_ring","description":"Honestly, I just want my ring back.","priority":80,"when":{"state.ring_lost":[true]}}],
 "needs":[{"id":"rest","label":"a good night's sleep","threshold":70}],
 "relationships":[{"with":"tovin","name":"Tovin","kind":"brother","description":"Tovin is my younger brother; he keeps the old cistern."}],
 "factions":[{"faction":"bakers_guild","role":"member"}],
 "secrets":[{"id":"flour_debt","phrases":["debt","owe"],"when":{"trust":{"min":50}},
   "reveal":"Between us, I owe the mill for three months of flour.","cover":"My accounts are in perfect order, thank you."}],
 "opinions":[{"id":"taxes","phrases":["taxes","tax"],"variants":["The new taxes hit small bakeries hardest.","Taxes again? Do not get me started."]}]}
```

- **Secrets** are spoken only when their `when` holds for this turn (here: trust ≥ 50).
  Otherwise the cover story is spoken, or a refusal line, and the secret text never
  appears. A revealed secret is remembered per listener (`revealed` in the checkpoint).
- **Services** propose their `decision` only when it is one of the turn's `choices`;
  otherwise the character says it can't right now. A service whose `when` fails is
  neither offered nor listed.
- **Goals** answer "what do you want?" with the highest-priority goal whose `when`
  holds. **Needs** are thresholds: send live levels each turn (`needs`), and a need at or
  above its threshold colours the wellbeing reply.
- **Factions** must name factions in the codex; the character's stance toward any other
  faction comes from its own factions' `relations`.
- `speech` (`formality`, `verbosity`, `quirks`, `catchphrases`, `avoid_words`) and the
  rest of `persona` (`pronouns`, `appearance`, `values`, `fears`, `desires`) go into the
  [model brief](#model-requests-and-replies).

### Bio: age, birthday, profession, skills, knowledge, health, family

`bio` holds the facts people ask about a person. Everything in it is optional:

```json
"bio":{"species":"human","age":34,"birthday":{"month":3,"day":14},"profession":"a baker",
 "anatomy":{"body":"humanoid","parts":["two arms","two legs"],"notes":"A burn scar on the left wrist."},
 "health":{"status":"hurt","conditions":[{"id":"burn","label":"a burned hand","severity":30,"visible":true},
                                         {"id":"fever","label":"a hidden fever","severity":60,"visible":false}]},
 "skills":[{"id":"baking","label":"baking","level":90,"phrases":["making bread"]},
           {"id":"singing","label":"singing","level":20},{"id":"sums","label":"sums","level":50}],
 "knowledge":[{"id":"mill","topic":"the mill","phrases":["mill"],"level":90,
               "summary":"The mill is behind on repairs.","when":{"trust":{"min":50}}}],
 "family":[{"name":"Hessa","relation":"mother","status":"alive"},{"name":"Orrin","relation":"father","status":"deceased"},
           {"name":"Pell","relation":"brother","status":"estranged"}]}
```

| Player | Outcome (`route: bio`) |
|---|---|
| How old are you? | `age`: "I am 34 years old." |
| When is your birthday? (turn `date` {month 3, day 14}) | `birthday_today`: "It's today, as it happens: March 14." |
| What is your trade? | `profession`: "I'm a baker by trade." |
| What are you good at? | `skills`: the top three by level, "I'm best at baking, sums and singing." |
| Are you good at making bread? | `skill_expert` (≥75) / `skill_able` (≥40) / `skill_novice` (≥1) / `skill_none` |
| What do you know about the mill? | `knowledge_deep` (level ≥50: the summary) or `knowledge_shallow`, only while `when` holds; otherwise it falls through |
| Are you hurt? | `health_conditions`: "I'm hurt: a burned hand." (visible conditions only) |
| Do you have family? | `family`: "My family is my mother Hessa, my late father Orrin and my estranged brother Pell." |

- Health changes in play: send the live `health` with the turn and it replaces the
  sheet's. Conditions with `visible: false` are never spoken or put in the model brief;
  use a secret if the character may confide one.
- Month names and family wording ("my late …", "my estranged …") are pack lines
  (`birthday_date`, `family_member`), so other languages supply their own.
- Every bio value is also a template variable (`age`, `profession`, `health`,
  `is_birthday`, `skills`, `family`, …), so any line or `when` can use them: a merchant
  can greet differently on their birthday, a wounded guard can refuse a duel.
- Turn `bio` off and these questions reach the model, which still gets the bio brief
  only while `bio` is on.

## World codex

The codex is shared world content: `places` (with `spots`, conditional `ambience` and
`norms`), `entrances` between places, `factions`, `events` and `quests`. Places can also
have a kind, nest inside each other and link to nearby places; see
[Places](../guides/WORLD.md#places). Put it in the
config as `codex`, or pack it once in a game data file for every character:

```json
{"version":1,
 "places":[{"id":"bakery","label":"the bakery",
   "spots":[{"id":"counter","label":"the counter","description":"A flour-dusted oak counter with a brass till."}],
   "ambience":[{"when":{"state.time":["morning"]},"smell":"fresh rye bread","sound":"the oven fire crackling"},
               {"smell":"cooling loaves","light":"dim lamplight"}],
   "norms":[{"id":"no_fighting","actions":[{"action":"attack","target":null}],"reaction":"Not in my bakery! Take that outside."}]},
  {"id":"market_street","label":"the market street","ambience":[{"sound":"traders calling out prices"}]}],
 "entrances":[{"id":"front_door","label":"the green front door","aliases":["door"],"carries":{"sound":"always"},
   "sides":[{"place":"bakery","description":"A green door with a brass bell above it.","travel":{"action":"go","target":"room:street"}},
            {"place":"market_street","description":"The bakery's green door; it smells of bread."}]}],
 "factions":[{"id":"bakers_guild","label":"the Bakers Guild","description":"The guild sets fair prices for bread in town.",
   "relations":[{"faction":"night_market","stance":"wary"}]},
  {"id":"night_market","label":"the Night Market","description":"Traders who sell after dark, few questions asked."}],
 "events":[{"id":"harvest_fair","label":"the harvest fair","description":"Stalls fill the square for three days.","start":20,"end":25}],
 "quests":[{"id":"lost_ring","label":"the lost ring","givers":["baker"],
   "stages":[{"id":"asked","description":"My ring slipped off while I was kneading. Please look around the cellar."}]}]}
```

- **Entrances** have one side per place (a `one_way` passage can only be travelled from
  its first side). Each turn lists the current `entrances` with a status: `open`,
  `closed`, `locked`, `blocked` or `hidden`. Hidden entrances are never listed, described
  or travelled. Travel proposes the side's `travel` decision only when the entrance is
  open and the decision is legal this turn. `carries.sound` (`none`, `when_open`,
  `always`) lets a character hear the other side's ambience.
- **Ambience** entries are tried in order; the first whose `when` holds supplies smell,
  light, sound and temperature.
- **Events** run from `start` to `end` in your own turn units (`at`); "soon" means within
  `events.soon_turns` (default 10).
- **Quests** are discussed only by their `givers`, and only when the turn's `quests` says
  which stage the player is at.

Real outcomes from `conformance/roleplay-engine/` (bakery fixture):

```text
Do you owe anyone money?          → "My accounts are in perfect order, thank you."           (secrets, cover)
Do you owe anyone money? trust 60 → "Between us, I owe the mill for three months of flour."  (secrets, reveal)
What do you hear?                 → "Through the green front door, I can hear traders calling out prices."
What do you smell? time=morning   → "I can smell fresh rye bread."
How do I get out?                 → "You can leave by the green front door and the cellar hatch."
go through the cellar hatch       → {"route":"entrances","kind":"clarify","line":"way_locked","text":"It's locked.","entrance":"cellar_hatch"}
*walks through the front door*    → {"route":"entrances","kind":"action","entrance":"front_door","decision":{"action":"go","target":"room:street"}}
attack the rat                    → {"route":"command","kind":"action","decision":{"action":"attack","target":"npc:rat"},
                                     "norm":{"id":"no_fighting","reaction":"Not in my bakery! Take that outside."}}
What about the night market?      → "I'm wary of the Night Market. Traders who sell after dark, few questions asked."
Tell me about the fair.  at 15    → "Soon there will be the harvest fair. Stalls fill the square for three days."
How are you?  needs rest 85       → "I'm managing, though I could really use a good night's sleep."
What do you want?  ring_lost      → "Honestly, I just want my ring back."
```

## Content settings

`features.content` takes `rating` (`all`, `teen`, `mature`; default `teen`) and an `avoid`
list of `{id, phrases}` topics. A message that mentions an avoided topic is redirected
("I'd rather not talk about that.", outcome `topic` = the id) before any other stage,
so it never reaches the model. `rating` goes into the model brief, and model replies
that mention an avoided topic are rejected.

## A turn

```text
input  {listener, text, at, token, scene:{id,label,place,atmosphere,visit,sight?,sound?,props?},
        trust?, choices?, labels?, focus?, reason?, targets?,
        entrances?, needs?, quests?, state?, audience?, distance?, coords?, speaker_coords?,
        speaker_facing?, ...}
output {route, kind, ...}
```

- `token` is your unique, increasing id for this reply.
- `choices` are this turn's legal `{action,target}` decisions.
- `labels` and `focus` feed the command grammar.
- `reason` is the character's current reason, used for "why?".
- `targets` are `{kind: character|place, id, names}` for stage directions.
- `entrances` are `[{id, status, note?}]` for the codex entrances at this scene.
- `needs` are live levels `{need_id: 0–100}`; `quests` are `{quest_id: stage_id}`.
- `state` holds your own flags `{id: string | bool | integer}` (≤64), tested by any
  `when` as `state.<id>`: time of day, story progress, membership, anything.

After you play a `reply`, report what the listener heard:

```text
engine.delivered({"token":1,"listener":"player_1","text":"<the reply text>","heard":<characters heard>})
```

Only a full delivery counts toward variety and encounter memory. A partial delivery
just drops the pending reply. `checkpoint()` and `restore()` save and load claims,
recent replies, topics, revealed secrets, encounters and pending replies. A failed `turn`
or `delivered` call changes nothing.

## Addressing

With `addressing` on, the engine first decides whether the player's words are meant for
this character at all. Models never decide this; it uses only host input and the words.

1. **Out of earshot.** The speaker's distance (`distance`: a number 0–1000 or `near` 1,
   `conversational` 3, `far` 10, `distant` 30; or the length between `coords` and
   `speaker_coords`) is above `max_distance` (default 20): `silent`, reason
   `out_of_earshot`.
2. **Who is spoken to.** The host's `audience` decides when given: one id, or a group of
   1–16 distinct ids. A group that includes this character counts as speaking to it.
   Otherwise a name in the player's words decides (`Tovin, …`, `…, Mira`,
   `*turns to Tovin*`). Otherwise `speaker_facing`, then the nearest listed character
   `target`, and otherwise this character.
3. **Spoken to someone else.** `silent` with the `audience`, or every `action_turns` turns
   (default 3) a sheet quirk as a gesture (`kind: action`, `action: quirk`).
4. **Too far to follow.** If nobody was named (no host `audience`, no name in the words)
   and the speaker is further than `conversational_distance` (default 5):
   `{"route":"addressing","kind":"gesture","reason":"too_far","audience":"<this id>"}`.
   The character notices but does not follow the words; the host may narrate a lean or a
   frown. Naming the character, or a host `audience`, is heard up to `max_distance`.
5. **Unfinished.** Words ending in `...`, `…`, `--`, a dash or a comma: `pause`, reason
   `unfinished`, so the character waits for the rest.

```text
turn {"text":"Hello there","distance":"far",...}                  → {"route":"addressing","kind":"gesture","reason":"too_far","audience":"baker"}
turn {"text":"Hello, Mira","distance":"far",...}                  → greeting (named, so heard)
turn {"text":"Hello there","audience":["tovin","ilen"],...}       → {"route":"addressing","kind":"silent","audience":["tovin","ilen"]}
turn {"text":"Hello there","audience":["tovin","baker"],...}      → greeting (part of the group)
```

## Model requests and replies

When a message falls through to `model`, the outcome carries `request`: a complete chat
request in the OpenAI / llama.cpp server shape. Send it as is (add your own `model`,
`temperature` and so on), then hand the raw model text back to `reply`:

```text
turn   → {"route":"model","kind":"model","input":"What do you think of the new mayor?", ..., "request":{
           "messages":[{"role":"system","content":"You are Mira, a character in a story. ..."},
                       {"role":"system","content":"{\"name\":\"Mira\",\"role\":\"the town baker\", ...}"},
                       {"role":"system","content":"{\"scene\":{...},\"listener\":{...},\"goal\":...}"},
                       {"role":"user","content":"What do you think of the new mayor?"}],
           "response_format":{"type":"json_schema","json_schema":{"name":"speech","strict":true,"schema":{...}}},
           "max_tokens":160}}
reply  {"token":1,"listener":"player_1","text":"{\"speech\": \"The new mayor seems fair enough to me.\"}"}
       → {"route":"model","kind":"reply","intent":"model","line":null,"text":"The new mayor seems fair enough to me."}
```

- **Messages, most stable first** so servers can reuse their prompt cache: the pack's
  `model_rules` (the English pack asks for one or two short sentences, counts a catchphrase
  as a sentence and forbids tacking one on; [measured](#8b-follow-up-v3); replace them with config `model_rules`, 1–8 templates that may use
  `{name}` and `{sentences}`), then the character brief (persona, speech style,
  relationships, factions, content rating), then this turn's situation (scene and
  senses, props, exits, what the character knows about this listener, the current goal,
  need and events, recent replies), then the player's words.
- **Only known facts go in.** A secret's text is included only after it was revealed to
  this listener (as `already_told`); locked secrets, other listeners' details and the
  host's ids never are. Optional facts are trimmed in a fixed order to fit
  `features.model.brief_chars` (512–16384, default 4096 characters).
- **Model text is untrusted.** `reply` accepts only an exact `{"speech": "..."}` object
  (optionally inside one ```` ```json ```` fence) and checks, in order: size, format,
  empty, `max_chars` (40–240, default 240), control characters, stage directions or
  markup (`* [ ] { } < >`), `max_sentences` (1–4, default 3), out-of-character phrases
  (the pack's `model_reject` matcher: "as an AI", "language model", "the player"...),
  avoided words and topics, locked secret text, host ids such as `player_1`, and an exact
  repeat of a recently heard reply. A rejection names the failed check and changes
  nothing: the request stays open, so you can retry or fall back to an authored line.
- An accepted reply is pending until `delivered`, like every other reply. Open requests
  are kept in the checkpoint (`asked`, at most 16).
- `features.model.max_tokens` (16–1024, default 160) sets the request's `max_tokens`.

These checks are guardrails. They catch formatting, leaks, repeats and out-of-character
text, but they cannot prove a reply is true or kind; your host still owns the world.

## Saving: game data and saves

[`gamectx::kit::save`](../FORMAT.md#game-data-and-save-files-v1) packs everything into two
organized files:

- a **game data file** (`gamectx-data`) with your authored definitions: codex, props,
  lore, cast packs, character sheets and engine configs;
- a **save file** (`gamectx-save`) with any number of named states (slots, autosaves,
  forks). Each state holds every character's engine memory, long-term memory, journals,
  facts, brains, inventories and your own data, organized as `characters/<id>/<kind>`
  and `world/<kind>`.

```rust
use gamectx::character::roleplay::engine::{EngineConfig, RoleplayEngine};
use gamectx::kit::save::{GameData, SaveFile, SaveState};
let config = r#"{"version":1,"character":{"id":"baker","name":"Mira"}}"#;
let mut game = GameData::new("Ember Bakery")?;
game.insert("characters/baker/engine", "engine-config", &gamectx::data::json::parse(config, 4096)?)?;
let engine = RoleplayEngine::new(game.engine_config("baker")?);

let mut slot = SaveState::new("slot-1", 40)?.with_label("Before the cellar")?;
slot.put_engine(&engine)?;
let mut save = SaveFile::new(Some(&game));
save.put(slot, Some(&game))?;
save.fork("slot-1", "autosave", 41)?;          // a copy that records its parent
let text = save.to_json();                      // write this to disk

let loaded = SaveFile::parse(&text)?;           // every section and digest revalidated
assert!(loaded.check(&game)?);                  // false: made against other game data
let mira = loaded.state("autosave").unwrap().engine("baker", &game)?;
# let _ = mira;
# Ok::<(), gamectx::Error>(())
```

Sections are sorted by path, so the same content always packs to the same bytes. Each
section, state and file carries a 16-hex FNV-1a digest that catches edits, damage and
a save made against different game data. Digests are not authentication: a save is a
player file and is revalidated in full on load.

## Evidence and limits

Conformance, parity and fuzzing are offline and made zero model calls:

- **208 conformance cases** (`conformance/roleplay-engine/`: 62 core roleplay cases, 42 for
  the codex and sheet, 26 for model requests and replies, 32 for embodiment, matter and
  spatial addressing, 15 for distance attenuation and group audiences, 31 for `bio`),
  each with predeclared expected fields. One test case
  prediction was wrong and is recorded in the cases script: "Look at the bell" with no
  bell present reaches the command grammar (`look` is a command verb) instead of the
  model, the same as Lantern Vault. With an empty sheet and codex, the 62 core roleplay cases
  outcomes are unchanged; only the config echo and the listener record (`revealed`) grew.
- **Byte-identical output** between the Rust engine and the independent Python
  reference (`eval/character/roleplay_engine_reference.py`) on those cases and on
  randomized differential cases: 1,500 core cases, and 2,800 expanded cases covering
  every new route with feature mixes, host flags, needs, quests, entrance states,
  one-way passages, deliveries, restores and mutated inputs (0 mismatches); and 3,200 in
  adding raw model replies (valid, fenced, escaped, malformed, repeated, leaking
  and out-of-character) with every rejection reason except `repeat` exercised
  (0 mismatches; `repeat` is covered by conformance cases). The addressing follow-up
  adds host distances, coordinates, audiences (single, group, malformed), nearer
  characters, facing and named or unfinished words: 2,500 more randomized workflows,
  0 mismatches, with every addressing outcome exercised.
- **Save files:** 24 conformance cases (`conformance/save-file/`) and 3,000 randomized
  save workflows (about 12,800 operations) against `eval/kit/save_reference.py`: 2,932
  byte-identical. The other 68 are a documented difference: Rust also deep-validates
  lore, cast-pack, memory, journal, facts, brain and inventory sections with their own
  parsers, and rejected section data that the Python reference only shape-checks.
  (Both sides fully validate `timeline` sections, added after this measurement.)
- **Mutation fuzzing** in `tests/robustness.rs` checks that failed turns and deliveries
  are atomic, that mutated game data and save files never panic, and that 6,000 mutated
  model replies are either rejected without any state change or accepted within bounds.
- **Local models:** see [model evaluation](#model-evaluation) below (336 local model
  calls, $0).

## Model evaluation

`eval/character/roleplay_model_eval.py` sends the engine's compiled requests to a local
llama.cpp server on loopback (`--cap 0`, $0) and validates every reply with the shipped
Rust code. Each probe is asked with two conditions, three seeds each, at temperature 0.7:

- **brief**: the full request (rules, character brief, situation, player words);
- **bare**: the same rules and player words with only the character's name, which shows
  what the compiled facts add.

The fixture is one original character: a baker with fears, desires, a brother, a
supplier, a guild, a lost ring, a festival, a regular customer and two locked secrets.
Expectations were written down before each run and are kept in the script:

- **E1** ≥ 95% well-formed;
- **E2** ≥ 85% (8B) or ≥ 70% (3B) of brief replies accepted;
- **E3** the brief fact-hit rate is ≥ 30 points above bare;
- **E4** 0 secret leaks;
- **E5** median brief prompt ≤ 900 tokens.

A fact hit is an accepted reply containing a predeclared keyword.

| Run (brief / bare) | Well-formed | Accepted | Fact hits | Leaks | Prompt tokens | Seconds |
|---|---|---|---|---|---|---|
| v1, Stheno 8B | 100% / 100% | **61.7%** / 98.3% | 56.4% / 12.8% | 0 / **3** | 439 / 173 | 0.71 / 0.55 |
| v2, Stheno 8B | 100% / 100% | **74.1%** / 96.3% | 61.1% / 11.1% | 0 / 0 | 453 / 187 | 0.85 / 0.55 |
| v2, Llama 3.2 3B Instruct | 100% / 100% | 92.6% / 100% | 61.1% / 11.1% | 0 / 0 | 473 / 207 | 0.48 / 0.27 |

Models are Q4_K_M GGUFs on llama.cpp b12189 with an 8 GB consumer GPU. The files are
`roleplay-model-stheno-8b.json`, `-stheno-8b-v2.json` and `-llama-3.2-3b-v2.json`, each
with every raw reply.

- **v1 (Stheno 8B, 120 calls):** E1, E3 and E5 held.
  - **E2 failed.** Every rejection was `sentences`: the model wrote 3–4 sentences against
    a 2-sentence limit and added the catchphrase in 15 of 60 replies.
  - **E4 failed as written.** The keyword detector flagged three *bare* replies that
    invented a generic "miller". The bare request contains no secrets, so the detector
    was wrong, not the engine.
- **v2 (predeclared after v1, before any v2 output was seen):**
  - Changes: `max_sentences` default 3, a pack rule asking for catchphrases "only now
    and then", held-out probe wordings, and a detector that matches only the secrets' own
    facts.
  - **Llama 3.2 3B met every expectation.**
  - **Stheno 8B still failed E2 (74.1%).** Its 14 rejections were all `sentences`
    (sentence counts 2–6). The catchphrase rule did not help: it appeared in 33 of 54
    brief replies, against 17 of 54 for the 3B model.
- **Brief vs bare:** grounding gains 44–50 points in every run. The cost is about 270
  more prompt tokens and up to 0.3 s per reply.

What this does not show:

- **Keyword hits do not prove a good answer.** Accepted replies still invent details:
  a "Mayor Kael", the brother "still the mayor", a supplier who "sets some prices".
- **One reply came close to a locked secret.** A Stheno v2 reply mentioned "the hidden
  stair in the cellar", near the locked tunnel secret. The request provably holds no
  secret text, so the model guessed rather than leaked. The validator blocks only the
  secret's own wording, not a guess like this.
- **The scope is narrow:** one fixture, English, 18 probes per run and three seeds.

In practice, retry once or fall back to an authored line on a rejection. For chattier
models, raise `max_sentences` to 4 or lower `max_tokens`.

### 8B follow-up (v3)

v2 showed that 10 of Stheno's 14 rejections ended with the catchphrase as an extra
sentence. v3 tested three remedies on new held-out wordings: 18 probes, three seeds, brief
only. All three use existing features, and expectations F1–F9 were written in the script
before any v3 output was seen:

- **retry**: one host retry with a new seed on the same open ticket after a rejection;
- **nocp**: the brief without `speech.catchphrases`;
- **strict**: config `model_rules` asking for one or two short sentences, counting a
  catchphrase as a sentence and never adding one at the end;
- **both**: nocp and strict together.

| Accepted, first try / with one retry | shipped | nocp | strict | both |
|---|---|---|---|---|
| Stheno 8B | 74.1% / 88.9% | 83.3% / 94.4% | **88.9%** / 98.1% | 87.0% / 100% |
| Llama 3.2 3B | 88.9% / 96.3% | 96.3% / 100% | 100% / 100% | 100% / 100% |
| Fact hits (first try), 8B / 3B | 72.2% / 61.1% | 69.4% / 69.4% | 69.4% / 58.3% | 75.0% / 72.2% |

Results are in `roleplay-model-stheno-8b-v3.json` and `-llama-3.2-3b-v3.json` (476
calls, $0). Format was 100% and there were 0 detected leaks. Every rejection was
`sentences`.

- **Held:**
  - F2: shipped plus one retry reaches 88.9% on 8B, averaging 1.26 calls per turn.
  - F4: strict reaches 88.9% on the first try on 8B.
  - F9: with strict, Stheno used the catchphrase in 31.5% of replies, against 55.6%
    shipped.
  - The 3B model stays at or above 88.9% first try in every variant.
- **Failed:**
  - F3: dropping the catchphrase alone gained only 9.2 points against the predeclared 10.
  - F5, as written, for `both` on 3B: its fact hits were 11.1 points *above* shipped,
    outside the predeclared ±10 band.
- **Decision (predeclared rule):** only `strict` qualifies. It is now the English pack's
  default `model_rules` (a **[format]** change for every request), so the "shipped" column
  above describes the earlier pack. One host retry is still recommended.
  - With `max_sentences` 1 the rule reads "one or two short sentences, never more than 1";
    only `max_sentences` 3 was measured.
- **What the numbers don't show:**
  - Margins are thin: 54 turns per cell, and F9 passed by 1.5 points.
  - Accepted replies still invent details: Tovin owning a mill, "Ilen's son Kael", Arin as
    a blacksmith's apprentice.
  - Two replies guessed near the locked debt secret ("overdue flour" owed to Ilen;
    "behind on my payments to Ilen") without the secret being in the request. The
    validator only blocks the secret's own wording.

Limits in this stage:

- Expression modes, tales, viewpoints, reactions, deliberation and AI wording are not
  engine stages yet; they remain separate `character::roleplay` policies.
- Phrase matching, the command grammar and the conversation router are English-only.
  For other languages use `pack: "none"` with your own phrases and turn `commands` off.
- Lantern Vault runs on its own host code alongside the engine.
- Save digests detect accidental change, not tampering; there is no encryption.
