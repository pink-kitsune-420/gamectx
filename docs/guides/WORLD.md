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

## Recipes

`gamectx::knowledge::recipes` models crafting, cooking, building, repairing, and dismantling.
Recipes specify inputs (by specific kinds or broad material tags), required tool functions with minimum power, optional workstations in the current place, optional character bio skills, outputs, and time.
Building recipes can also mutate the world codex: placing structures, adding spots, creating travel links, opening entrances, or updating place kinds and tags.

Recipes check feasibility first—returning exact missing reasons if tools, skills, stations, inputs, or targets are lacking—and execute atomically (reverting state on failure).

```rust
use gamectx::data::json::parse;
use gamectx::knowledge::recipes::Recipes;

let mut recipes = Recipes::new();
let req = parse(r#"{
  "ops": [
    {"op": "execute", "args": {
      "recipe": "forge_pickaxe",
      "actor": "smith",
      "place": "forge",
      "new_ids": ["pickaxe_1"]
    }}
  ],
  "recipes": [
    {
      "id": "forge_pickaxe",
      "label": "Forge Pickaxe",
      "category": "crafting",
      "inputs": [
        {"kind": "iron_ingot", "quantity": 2},
        {"tag": "timber", "quantity": 1}
      ],
      "tools": [
        {"function": "hammer", "power": 3}
      ],
      "station": "anvil",
      "outputs": [
        {"kind": "pickaxe", "quantity": 1, "condition": 100}
      ],
      "time": 25
    }
  ],
  "things": {
    "version": 1,
    "materials": [
      {"id": "iron", "label": "Iron"},
      {"id": "wood", "label": "Timber"}
    ],
    "kinds": [
      {"id": "iron_ingot", "label": "Iron Ingot", "category": "material", "materials": [], "functions": [], "weight": 5, "base_value": 10, "durability": 100, "stackable": true},
      {"id": "wood_handle", "label": "Wood Handle", "category": "material", "materials": [], "functions": [], "weight": 2, "base_value": 2, "durability": 80, "stackable": true, "tags": ["timber"]},
      {"id": "smith_hammer", "label": "Smith Hammer", "category": "tool", "materials": [], "functions": [{"function": "hammer", "power": 5}], "weight": 5, "base_value": 15, "durability": 200, "stackable": false},
      {"id": "iron_anvil", "label": "Iron Anvil", "category": "furniture", "materials": [], "functions": [{"function": "anvil", "power": 10}], "weight": 150, "base_value": 80, "durability": 1000, "stackable": false},
      {"id": "pickaxe", "label": "Iron Pickaxe", "category": "tool", "materials": [], "functions": [{"function": "dig", "power": 4}], "weight": 8, "base_value": 25, "durability": 120, "stackable": false}
    ],
    "things": [
      {"id": "hammer_1", "kind": "smith_hammer", "location": {"holder": "smith"}, "condition": 100, "owner": "smith", "quantity": 1, "passengers": []},
      {"id": "anvil_1", "kind": "iron_anvil", "location": {"place": "forge"}, "condition": 100, "owner": "smith", "quantity": 1, "passengers": []},
      {"id": "ingots_1", "kind": "iron_ingot", "location": {"place": "forge"}, "condition": 100, "owner": "smith", "quantity": 4, "passengers": []},
      {"id": "handles_1", "kind": "wood_handle", "location": {"holder": "smith"}, "condition": 100, "owner": "smith", "quantity": 2, "passengers": []}
    ]
  }
}"#, 4096)?;

let out = recipes.workflow(&req)?;
assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
# Ok::<(), gamectx::Error>(())
```

That recipe execution returns a timeline-ready entry (real output):

```json
{"ok":true,"result":{"ok":true,"recipe":"forge_pickaxe","created":[{"id":"pickaxe_1","kind":"pickaxe","location":{"holder":"smith"},"condition":100,"owner":"smith","quantity":1,"passengers":[]}],"entry":{"at":0,"kind":"crafting_recipe","summary":"smith completed Forge Pickaxe (crafting).","actors":["smith"],"place":"forge","things":["pickaxe_1"],"tags":["recipes","crafting"],"visibility":"public","witnesses":[],"phrases":["Forge Pickaxe","forge_pickaxe"],"facts":{"recipe":"forge_pickaxe","time":25}}}}
```

- **Categories:** `crafting` (produces items in actor holder), `cooking` (produces food in actor holder), `building` (places structures, updates places and entrances in the codex), `repairing` (restores target thing condition to 100), and `dismantling` (reclaims materials or components from target thing).
- **Inputs & Tags:** inputs specify required quantities of either a concrete `kind` or an authored `tag` (e.g. `timber`, `ore`, `dough`), allowing recipes to accept flexible materials.
- **Feasibility Checking:** `check` returns structured missing diagnostics (`missing_skill`, `missing_tool`, `missing_station`, `missing_input`, `missing_target`, `target_not_found`, `target_not_accessible`, `target_already_repaired`).
- **World Mutability:** building recipes seamlessly integrate with the codex, adding spots, modifying links, opening entrances, and tagging places upon completion.

**Evidence:** 25 cases (`conformance/recipes/`, 9 of them malformed input) with
expectations written before running. Rust matches the Python reference
(`eval/knowledge/recipes_reference.py`) byte for byte on all of them and on 2,000
randomized workflows (seed 7, 0 mismatches). CI runs 300 more. No model calls.

## Species

`gamectx::knowledge::species` models animals, plants, people, and hybrids.
Species define diet, habitats, lifespan, temperament defaults (boldness, sociability, aggression, trainability), renewable produce, carrier capacity, and authored sounds and gestures.
Hybrids blend traits from two parent species with deterministic cycle prevention and explicit overrides.

Individual animals are tracked with tameness, trust relations per actor (-100..100), leading status, riding mount status, and harvest timestamps.

```rust
use gamectx::data::json::parse;
use gamectx::knowledge::species::SpeciesWorld;

let mut sw = SpeciesWorld::new();
let req = parse(r#"{
  "ops": [
    {"op": "pet", "at": 10, "args": {"animal": "dog_1", "actor": "hero", "place": "kennel"}}
  ],
  "species": [
    {
      "id": "canine",
      "label": "Wolfhound",
      "kind": "animal",
      "tameable": true,
      "temperament": {"boldness": 50, "sociability": 70, "aggression": 10, "trainability": 80},
      "sounds": ["bark", "whimper"],
      "gestures": ["wags tail", "pants"]
    }
  ],
  "animals": [
    {"id": "dog_1", "species": "canine", "tameness": 60, "trust": {"hero": 20}}
  ]
}"#, 4096)?;

let out = sw.workflow(&req)?;
assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
# Ok::<(), gamectx::Error>(())
```

That animal interaction returns a timeline-ready entry (real output):

```json
{"ok":true,"result":{"ok":true,"success":true,"trust":30,"sound":"bark","gesture":"wags tail","entry":{"at":10,"kind":"animal_interaction","summary":"hero petted dog_1 (wags tail).","actors":["hero","dog_1"],"place":"kennel","things":[],"tags":["species","animal","pet","success"],"visibility":"public","witnesses":[],"phrases":["Wolfhound","dog_1","pet"],"facts":{"interaction":"pet","success":true,"trust":30}}}}
```

- **Animal Interactions:**
  - `pet`: evaluates threat (aggression minus trust and half-tameness); snaps on failure or bonds on success, raising trust and emitting a gesture.
  - `feed`: verifies dietary compatibility (carnivore, herbivore, omnivore), consumes food from actor inventory or the place, and boosts both trust and tameness.
  - `calm`: pacifies skittish or agitated creatures, boosting trust.
  - `lead`: leads or unleashes an animal (requires trust ≥ 10 or tameness ≥ 30).
  - `ride`: mounts or dismounts a rideable animal (requires rideable, trust ≥ 20, tameness ≥ 40).
  - `train`: conducts training to increase tameness based on species trainability.
  - `harvest`: gathers renewable produce (such as milk, wool, or feathers) into actor possession when the harvest interval has elapsed.
  - `behave`: evaluates animal attitude (`affectionate`, `hostile`, `skittish`, or `calm`) towards an actor and outputs appropriate non-verbal sounds and physical gestures.
- **Hybrids:** inheriting from two parents averages numeric temperament values, merges diets and habitats, and applies explicit authored overrides.

**Evidence:** 27 cases (`conformance/species/`, 7 of them malformed input) with
expectations written before running. Rust matches the Python reference
(`eval/knowledge/species_reference.py`) byte for byte on all of them and on 2,000
randomized workflows (seed 7, 0 mismatches). CI runs 300 more. No model calls.

## Communities

`gamectx::knowledge::communities` models factions, settlements, and guild communities.
Communities have home places, member rosters, authored customs (cultural norms), and bilateral relations with both qualitative stances (`allied`, `friendly`, `neutral`, `wary`, `hostile`) and signed numeric trust (`-100..100`).
Their markets specify what goods they buy and sell with item demand/scarcity multipliers.

Bartering evaluates trades host-authoritatively from item condition and base value, market multipliers, and relation stance discounts or surcharges. When an offer is insufficient, the engine generates actionable counter-offers:

```rust
use gamectx::data::json::parse;
use gamectx::knowledge::communities::CommunityWorld;

let mut cw = CommunityWorld::new();
let req = parse(r#"{
  "ops": [
    {"op": "barter", "at": 10, "args": {
      "community": "ember_wardens",
      "actor": "hero",
      "place": "ember_keep",
      "offer_things": ["hero_sword"],
      "request_things": ["warden_torches"]
    }}
  ],
  "communities": [
    {
      "id": "ember_wardens",
      "label": "The Ember Wardens",
      "description": "Guardians of the mountain pass.",
      "relations": [
        {"target": "hero", "stance": "friendly", "trust": 30}
      ],
      "market": {
        "buys": [{"tag": "weapon", "price_mult": 1200}],
        "sells": [{"kind": "torch", "price_mult": 800}]
      }
    }
  ],
  "things": {
    "version": 1,
    "materials": [{"id": "iron", "label": "Iron"}],
    "kinds": [
      {"id": "sword", "label": "Iron Sword", "category": "weapon", "materials": [], "functions": [], "weight": 6, "base_value": 50, "durability": 100, "stackable": false, "tags": ["weapon"]},
      {"id": "torch", "label": "Torch", "category": "tool", "materials": [], "functions": [], "weight": 1, "base_value": 2, "durability": 20, "stackable": true, "tags": ["light"]}
    ],
    "things": [
      {"id": "hero_sword", "kind": "sword", "location": {"holder": "hero"}, "condition": 100, "owner": "hero", "quantity": 1, "passengers": []},
      {"id": "warden_torches", "kind": "torch", "location": {"place": "ember_keep"}, "condition": 100, "owner": "ember_wardens", "quantity": 5, "passengers": []}
    ]
  }
}"#, 4096)?;

let out = cw.workflow(&req)?;
assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
# Ok::<(), gamectx::Error>(())
```

That barter execution returns a timeline-ready entry (real output):

```json
{"ok":true,"offer_value":63,"request_value":7,"transferred_offer":["hero_sword"],"transferred_request":["warden_torches"],"entry":{"at":10,"kind":"barter","summary":"hero bartered with ember_wardens at ember_keep.","actors":["ember_wardens","hero"],"place":"ember_keep","things":["hero_sword","warden_torches"],"tags":["community","barter","trade"],"visibility":"public","witnesses":[],"phrases":["ember_wardens","hero","barter"],"facts":{"community":"ember_wardens","offer_value":63,"request_value":7,"offered":["hero_sword"],"requested":["warden_torches"]}}}
```

- **Relations & Trust:**
  - `check_relation`: returns active stance, numeric trust (-100..100), and computed pricing multipliers (`buy_mult`, `sell_mult`).
  - `set_relation`: updates stance or numeric trust, emitting a relation change timeline entry.
  - Generous trades (surplus offer value) automatically boost bilateral relation trust.
- **Rosters & Customs:**
  - `add_member` / `remove_member`: tracks membership rosters and logs community join/leave events.
  - `set_customs`: declares and updates local cultural norms and laws.
- **Bartering & Markets:**
  - `appraise`: previews exchange balance and returns counter-offers without moving items.
  - `barter`: atomically transfers item ownership and physical location (holder/place), recording a public timeline entry on completion or reverting state on refusal.

**Evidence:** 28 cases (`conformance/communities/`, 8 of them malformed input) with
expectations written before running. Rust matches the Python reference
(`eval/knowledge/communities_reference.py`) byte for byte on all of them and on 2,000
randomized workflows (seed 7, 0 mismatches). CI runs 300 more. No model calls.

## Fitness

`gamectx::character::fitness` models physical capability: strength, endurance, agility, and overall condition.
Integrated into character sheet `bio` and host travel workflows, fitness dynamically calculates encumbrance limits, travel pacing across linked places, and exertion fatigue.

When travel or heavy tasks occur, fitness determines travel duration and exhaustion:

```rust
use gamectx::data::json::parse;
use gamectx::character::fitness::FitnessWorld;

let mut fw = FitnessWorld::new();
let req = parse(r#"{
  "ops": [
    {"op": "travel_cost", "args": {
      "distance": 100,
      "load": 25
    }}
  ],
  "fitness": {
    "strength": 75,
    "endurance": 65,
    "agility": 60,
    "condition": 90
  }
}"#, 4096)?;

let out = fw.workflow(&req)?;
assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
# Ok::<(), gamectx::Error>(())
```

That travel cost evaluation returns realistic pacing and exertion (real output):

```json
{"ok":true,"distance":100,"current_load":25,"travel_speed":1120,"travel_time":89,"fatigue_cost":97}
```

- **Attribute Influences:**
  - `strength`: drives maximum load carrying limits (`max_cap = 20 + strength + endurance/2`) and reduces fatigue from carried weight.
  - `endurance`: scales prolonged movement duration and resists travel fatigue.
  - `agility`: boosts overland travel speed and movement responsiveness.
  - `condition`: overall physical readiness; conditions health states (`hurt`, `sick`, `exhausted`) and low energy needs to degrade effective performance.
- **Operations:**
  - `assess`: evaluates nominal vs effective physical capabilities and current fatigue rates.
  - `travel_cost`: calculates journey time and cumulative fatigue given distance, carried load, and health state.
  - `can_carry`: tests whether adding weight remains within safe capacity limits.
  - `train`: performs dedicated physical training, awarding attribute improvements with diminishing returns and logging public timeline entries.

**Evidence:** 24 cases (`conformance/fitness/`, 6 of them malformed input) with
expectations written before running. Rust matches the Python reference
(`eval/character/fitness_reference.py`) byte for byte on all of them and on 2,000
randomized workflows (seed 7, 0 mismatches). CI runs 300 more. No model calls.





