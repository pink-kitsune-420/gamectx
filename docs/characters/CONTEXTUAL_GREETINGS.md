# Contextual greetings and stage directions

The studio can begin an encounter with an action rather than fabricated player speech.
With personality turns enabled, accepted room travel renders *enters the workshop* and
lets the single present NPC greet the active player. **Approach merchant / keeper** engages
the character without inserting a player “Hello.” If a greeting is already ready, Approach
focuses the conversation instead of generating another introduction. Speech still uses the
normal Read/Play confirmation lifecycle; presentation alone does not count as hearing it.

Typed `*approaches the merchant*` and `*enters the workshop*` are finite host action
proposals. Approaching requires an available nearby talk target; entering follows the
existing travel rules and revision checks. A locked vault stays locked. Unsupported,
compound or destination-free stage directions ask for clarification and never become
claimed world facts. Ordinary speech remains speech. Recognition interim text never
executes an action; final recognition follows the same checks as typing. LAN guests may
approach a present character, but cannot use stage text to bypass host-only travel.
Stage directions are shown in italics, separate from spoken dialogue. Only a present NPC
observes the action; no player speech receipt is fabricated. Arrival produces at most one
NPC response and does not trigger NPC-to-NPC reply loops or a language-model prewarm.

# Encounter memory

`character::roleplay::GreetingMemory` is a reusable, zero-dependency host helper. `plan` is pure;
`confirm` records complete heard NPC speech. Its keys are NPC plus listener, so another
player or NPC never inherits your introduction. A separate flag records whether a public
name was actually included in a confirmed greeting. An urgent greeting can omit names;
a calmer encounter can introduce the NPC later. Partial/skipped replies do not establish
a new mutual encounter or an unheard introduction. Replaying audio is review playback
and does not supply new delivery receipts.

| Plan | Meaning |
| --- | --- |
| `first` | No confirmed mutual exchange is recorded. |
| `continuing` | Same continuous scene visit and recent conversation. |
| `returning` | Same place, after a departure and return. |
| `elsewhere` | A different place from the last confirmed exchange. |
| `after_gap` | Same visit, with a sufficiently long quiet interval. |
| `familiar` | Trusted prior contact exists, but its venue/visit is unknown. |

“Elsewhere” does not claim this is the first ever visit to that location. A greeting
plan also supplies introduction need, relationship band and host-selected place/atmosphere.
The renderer uses a publicly disclosable name/alias and **only the current perceived
place label**. Workplace, social, private and hazardous contexts can use different wording.
A private place does not imply there are no bystanders. No weather, unseen global facts,
biography or stock is invented. The example uses original Ilen/workshop and Tovin/cistern
content; the core supports arbitrary participant/scene IDs. NPC movement between venues
is supported by the generic plan, but the dungeon's NPCs still have fixed workplaces.

Callers supply a monotonic `visit` identifier for continuous presence, an `at` clock,
current trust and authoritative atmosphere/place tags. A player-selector change is not
a new visit. A model claim such as “we have met before” cannot create history. Plans
include the prior per-pair token as `expected`; confirmation checks it, token/time/visit
ordering, Unicode heard offsets and bounds before mutation. Identical duplicate receipts
are harmless. `seed` imports trusted prior contacts without claiming venue knowledge;
it is a host method, never a model permission.

The store holds at most 256 pairs and refuses new pairs at capacity instead of evicting
old introductions. Exchanges/greeting counters reflect tracked evidence, not a guaranteed
lifetime total after a legacy import. The bounded checkpoint is validated on recovery.
`studio-greetings` exposes `{config,state,ops}` through the local adapter and all bindings,
with `plan`, `confirm`, `seed` and `configure` operations; it does no filesystem/network,
model work, appraisal or world execution. Hosts still own perception, floor control,
correct receipts and storage. The example's action grammar is a separate host adapter.

# Settings and saves

**Settings → Characters → Greeting behavior** sets the quiet interval (default 12 game
turns, per NPC/player). This is deterministic game time, not real elapsed seconds.
The policy is stored separately in `greetings.json`; changing it does not reset history.
Optional `encounters` checkpoint data rolls back with branch loads. New game clears it
and retains policy settings. Old checkpoints load: actual heard receipts and saved,
confirmed viewpoints seed familiar contacts with unknown venue and no assumed name
introduction. This conservatively avoids calling a known player a stranger after upgrade.
Original personality-off chat/conformance outputs are preserved; the new explicit Approach
control opts into contextual greeting behavior without changing the old `chat` contract.

Debug's floating State window shows the next plan and encounter evidence. The editable
`examples/fixtures/cast/greeting-memory.json` demonstrates first contact, a different venue and
another listener; run `gamectx local examples/fixtures/cast/greeting-memory.json` without models.

# Evidence and limits

Twenty-six independent Python/Rust greeting cases cover first/continuing/returning,
location/quiet-gap changes, current relationship/urgency, partial/unheard delivery,
actor/player isolation, legacy import, stale tokens/clocks/visits, duplicate confirmation,
recovery, capacity and rejection of an extra unperceived fact field. Two additional routing
cases cover repeat/familiar greeting phrases; all existing expected outputs stay unchanged.
Two thousand encounter mutations never execute actions or access models/files.

`eval/studio/greeting-memory-host.json` records 12 workflows / 660 HTTP checks, including three
distinct same-encounter replies, introductions surviving 110 further turns and bounded
dialogue-tail pruning, branch restore, legacy migration and reset. Its configured provider
endpoint rejects any generation; **zero requests reached it**. `greeting-actions-host.json`
records seven workflows / 30 HTTP checks for arrival/Approach without fake “Hello”,
locked-route and absent-NPC rejection, unsupported effects and interim/final recognition.
`greeting-memory-browser.json` records seven Firefox workflows for visible entry staging,
first/repeat/return greetings, player/NPC isolation, settings and floating diagnostics.
Unit tests cover changed venue/private/urgent wording and guest travel restrictions.
These are deterministic text/receipt/DOM checks, not physical audio, physical LAN,
frame-rate guarantees or live model-quality measurements. All runs use `--cap 0` and no
new generation calls. Prior evaluation artifacts remain frozen.
