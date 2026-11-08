# World model

The world model holds what the game itself knows about the world: what happened, where
things are and what they are made of. The game (the host) owns all of it. Models never
write to it; they can only be shown what a character knows.

Each part ships the same way as the rest of gamectx: an independent Python reference in
`eval/`, Rust that matches it byte for byte, and `conformance/` cases.

| Part | Status |
|---|---|
| [Timeline](#timeline) | done |
| [Places](#places): kinds, nesting, nearby | done |
| [Things](#things): materials, tools, weapons, furniture, containers, carriers, stock | done |
| Recipes: crafting, cooking, building, repairing, dismantling | planned |
| Species: animals, plants, hybrids, temperaments | planned |
| Communities: relations, markets, bartering | planned |

## Places

Codex places can say what kind of place they are, which place they sit inside and how
they connect to places that aren't joined by a door. With that, a character can answer
"where am I?" and "what's nearby?" from facts the game controls.

```json
{"version":1,"places":[
 {"id":"vale","label":"Ember Vale","kind":"valley"},
 {"id":"village","label":"Emberford","kind":"village","within":"vale",
  "links":[{"place":"woods","distance":5,"direction":"north"},{"place":"keep","distance":3,"direction":"east"}]},
 {"id":"bakery","label":"the bakery","kind":"bakery","within":"village"},
 {"id":"cellar","label":"the bakery cellar","kind":"cellar","within":"bakery"},
 {"id":"square","label":"the square","kind":"square","within":"village"},
 {"id":"well","label":"the old well","kind":"well","within":"village"},
 {"id":"woods","label":"Whisper Woods","kind":"forest","within":"vale"},
 {"id":"keep","label":"Ember Keep","kind":"castle","within":"vale"}],
 "entrances":[
 {"id":"bakery_door","label":"the bakery door","sides":[{"place":"bakery","description":"A green door."},{"place":"square","description":"The bakery's door."}]},
 {"id":"cellar_hatch","label":"the cellar hatch","distance":0,"sides":[{"place":"bakery","description":"A hatch in the floor."},{"place":"cellar","description":"Steps up to the bakery."}]}]}
```

- **`kind`** is one of 112 built-in kinds (below) or one you add under `place_kinds` as
  `{id, label, scale, indoor}`. A custom kind can't reuse a built-in id.
- **`within`** names the place this one sits inside: cellar ⊂ bakery ⊂ village ⊂ valley.
  At most 8 levels, and no loops.
- **`links`** connect places that have no door between them, such as a village and a
  forest. Each has a `distance` (in your own units) and an optional `direction`. The
  other end automatically gets the opposite direction, so author each link once.
- **Entrances** may have a `distance` (default 1). A `one_way` entrance only goes from
  its first side.
- **`tags`** (up to 8) are free labels for your own game logic.

`codex.nearby(&query)` (or the `places` local kind) answers for a character standing
in `from`. Here the bakery door is passed as `closed`:

```json
{"from":"bakery","closed":["bakery_door"]}
```

The result (real output):

```json
{"here":[{"place":"bakery","label":"the bakery","kind":"bakery","scale":"building"},{"place":"village","label":"Emberford","kind":"village","scale":"settlement"},{"place":"vale","label":"Ember Vale","kind":"valley","scale":"region"}],
 "routes":[{"place":"cellar","label":"the bakery cellar","kind":"cellar","distance":0,"from":"bakery","steps":[{"to":"cellar","entrance":"cellar_hatch","direction":null}]},
  {"place":"keep","label":"Ember Keep","kind":"castle","distance":3,"from":"village","steps":[{"to":"keep","entrance":null,"direction":"east"}]},
  {"place":"woods","label":"Whisper Woods","kind":"forest","distance":5,"from":"village","steps":[{"to":"woods","entrance":null,"direction":"north"}]}],
 "total_routes":3,
 "around":[{"place":"square","label":"the square","kind":"square","within":"village"},{"place":"well","label":"the old well","kind":"well","within":"village"}],
 "total_around":2}
```

- **`here`** is the chain of places you are in, innermost first.
- **`routes`** are the shortest ways out through open entrances and links. The search
  starts from your place and from every place around it, which is why the village's
  links work from inside the bakery. `from` says which of those places the route starts
  at, and `steps` lists each entrance or direction along the way.
- **`around`** lists places inside the same parents that have no route right now. Here
  that's the square, because its only door is closed.
- `max_distance` (default 10⁹) and `limit` (1–64, default 16) bound the result.
  `total_routes` and `total_around` count every match. Ties are broken by codex order.

Built-in place kinds, by scale (from `src/knowledge/place_kinds.json`):

| Scale | Kinds |
|---|---|
| `world` | world, realm |
| `region` | region, kingdom, province, wilds, forest, mountains, desert, swamp, coast, sea, island, plains, valley |
| `settlement` | city, town, village, hamlet, camp, outpost, port |
| `district` | district, quarter, market, square, street, harbor, docks, farmland |
| `site` | woods, grove, clearing, field, meadow, garden, farm, orchard, river, lake, pond, spring, beach, hill, cliff, road, path, bridge, ford, ruins, graveyard, quarry, well, courtyard, battlefield, cave, cavern, mine, tunnel, dungeon |
| `building` | castle, keep, fort, tower, palace, manor, house, cottage, hut, tent, shop, workshop, smithy, bakery, tavern, inn, temple, shrine, library, school, mill, barn, stable, warehouse, granary, guildhall, prison, lighthouse, ship |
| `room` | room, hall, great_hall, kitchen, pantry, cellar, bedroom, attic, study, throne_room, chapel, armory, vault, chamber, corridor, stairs, gallery, forge, storeroom, cell, bathhouse, deck, hold |

Every kind is outdoors (`indoor: false`) except the cave-like sites (cave, cavern, mine,
tunnel, dungeon), the buildings and the rooms. A ship's `deck` is the one outdoor room.

**Evidence:** 30 cases (`conformance/places/`, 12 of them malformed input) with
expectations written before running. Rust matches the Python reference
(`eval/knowledge/places_reference.py`) byte for byte on all of them and on 2,000
randomized codexes and queries (seed 7: 1,932 valid codexes, 3,891 answered queries,
1,773 with a multi-step route, 0 mismatches). CI runs 300 more. No model calls.

## Timeline

`gamectx::knowledge::timeline::Timeline` is the world's history: every fact about what
happened, in one ordered list that only the game writes. Use it for the story so far, for
what a character can know and tell, and for history written before play starts.

```rust
use gamectx::data::json::parse;
use gamectx::knowledge::timeline::Timeline;

let mut history = Timeline::new();
history.record(&parse(r#"{"at":10,"kind":"fire","summary":"A fire broke out in the Ember Keep forge.",
  "actors":["smith"],"place":"forge","tags":["disaster"],"phrases":["the fire","forge fire"]}"#, 4096)?)?;
history.record(&parse(r#"{"at":20,"kind":"theft","summary":"Someone took the steward's ledger.",
  "actors":["thief"],"place":"hall","things":["ledger"],"visibility":"witnessed","witnesses":["cook"],
  "phrases":["ledger"]}"#, 4096)?)?;
history.record(&parse(r#"{"at":30,"kind":"pact","summary":"The miller promised his mill to the keep.",
  "actors":["miller","steward"],"place":"mill","visibility":"private","facts":{"price":40}}"#, 4096)?)?;

// What does the cook know about the ledger?
let known = history.query(&parse(r#"{"known_to":"cook","about":"What happened with the ledger?"}"#, 4096)?)?;
assert_eq!(known.get("total").and_then(|v| v.as_usize()), Some(1));
# Ok::<(), gamectx::Error>(())
```

That query returns (real output):

```json
{"total":1,"entries":[{"seq":2,"at":20,"kind":"theft","summary":"Someone took the steward's ledger.",
 "actors":["thief"],"place":"hall","things":["ledger"],"tags":[],"visibility":"witnessed",
 "witnesses":["cook"],"phrases":["ledger"],"facts":{},"retracted":null,"covers":null}]}
```

The cook knows about the fire (public) and the theft (they witnessed it), but not the pact
(private to the miller and the steward).

- **When:** `at` is your game's clock (a turn, a tick or a timestamp). You can record
  entries out of order, for example to add history from before the game starts.
  Queries sort by `(at, seq)`.
- **Who knows:** `public` entries are known to everyone. `witnessed` entries are known to
  their actors and witnesses. `private` entries are known to their actors only.
- **Asking about it:** `phrases` are what people may call the event. A query's `about`
  text matches a phrase on whole words, so "the mill" matches "What of the mill?" but
  "mill" does not match "miller".
- **Facts:** `facts` holds up to 16 small values (true/false, whole numbers, short text)
  for game logic to read back later.
- **Mistakes:** `retract(seq, reason)` hides an entry from queries but keeps it in the
  log. A query with `include_retracted: true` still shows it.
- **Size:** a timeline holds at most 4,096 entries. Recording into a full timeline fails
  instead of forgetting anything. `compact(through, summary, kind)` replaces the oldest
  entries, up to `through`, with one public summary that records how many entries it
  covers.
- **Saving:** store it as a `timeline` section, for example at `world/timeline`, in a
  [save state](../characters/ROLEPLAY_ENGINE.md#saving-game-data-and-saves). Loading
  revalidates every entry.

Query filters: `known_to`, `actor`, `place`, `thing`, `kind`, `tag`, `since`, `until`,
`about`, `limit` (1–64, default 16), `order` (`newest` by default, or `oldest`) and
`include_retracted`. The result is `{total, entries}`, where `total` counts every match
before `limit` is applied. The exact format is in
[FORMAT.md](../FORMAT.md#world-timeline-v1).

**Evidence:** 46 cases (`conformance/timeline/`, 6 of them malformed input) with
expectations written before running. Rust matches the Python reference
(`eval/knowledge/timeline_reference.py`) byte for byte on all of them and on 2,000
randomized workflows (seed 7, 0 mismatches). CI runs 300 more. No model calls.

## Things

`gamectx::knowledge::things::Things` tracks what exists in the physical world:
materials, thing kinds, and individual instances. It handles weights, salvage yields,
containers with capacities and tag filters, carriers (vehicles and animals) with seats
and cargo load limits, and tool function rankings.

```rust
use gamectx::data::json::parse;
use gamectx::knowledge::things::Things;

let mut world_things = Things::new();
let req = parse(r#"{
  "ops": [
    {"op": "create", "args": {
      "id": "iron_pickaxe",
      "kind": "pickaxe",
      "location": {"place": "forge"},
      "condition": 100,
      "owner": "smith"
    }}
  ],
  "things": {
    "version": 1,
    "materials": [
      {"id": "iron", "label": "Iron Ingot"},
      {"id": "wood", "label": "Timber"}
    ],
    "kinds": [
      {
        "id": "pickaxe",
        "label": "Iron Pickaxe",
        "category": "tool",
        "materials": [{"material": "iron", "amount": 2}, {"material": "wood", "amount": 1}],
        "functions": [{"function": "dig", "power": 4}],
        "weight": 8,
        "base_value": 15,
        "durability": 120,
        "stackable": false
      }
    ],
    "things": []
  }
}"#, 4096)?;

let out = world_things.workflow(&req)?;
assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
# Ok::<(), gamectx::Error>(())
```

That operation returns a timeline-ready entry (real output):

```json
{"ok":true,"result":{"id":"iron_pickaxe","entry":{"at":0,"kind":"create_thing","summary":"Created Iron Pickaxe (iron_pickaxe).","actors":["smith"],"place":"forge","things":["iron_pickaxe"],"tags":["things","create"],"visibility":"public","witnesses":[],"phrases":["Iron Pickaxe","iron_pickaxe"],"facts":{"kind":"pickaxe","quantity":1,"condition":100}}}}
```

- **Materials:** define elemental substances with base unit weights and values.
- **Categories:** tool, weapon, furniture, container, food, material, clothing, vehicle, structure, plant, and other.
- **Functions:** tools and workstations declare functional powers (e.g. `cut`, `chop`, `dig`, `light`, `cook`, `store`, `ride`, `carry`). `best_tool` finds the highest-power tool held by an actor.
- **Instances and Storage:** each instance sits in a place, container, holder or carrier. Containers reject items when exceeding `capacity` or when tag rules fail. Carriers accept passengers up to `seats` and cargo up to `load_capacity`.
- **Salvage and Removal:** `destroy` breaks an item down into its base materials scaled by current condition. `remove` deletes an instance cleanly.
- **Travel:** moving a carrier (`travel`) transports all passengers and carried items to the new destination.
- **Relation to Inventory and Props:** `knowledge::inventory` is a lightweight ledger for tokens and counters used in fast character tool calling; `knowledge::props` provides static scene descriptions and interaction prompts; `knowledge::things` manages host-authoritative spatial entities and matter.

**Evidence:** 42 cases (`conformance/things/`, 3 of them malformed input) with
expectations written before running. Rust matches the Python reference
(`eval/knowledge/things_reference.py`) byte for byte on all of them and on 2,000
randomized workflows (seed 7, 0 mismatches). CI runs 300 more. No model calls.

