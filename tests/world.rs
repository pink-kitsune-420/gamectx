//! The world model guide (docs/guides/WORLD.md): its printed output must be real output.
use gamectx::data::json::parse;
use gamectx::knowledge::timeline::Timeline;

fn guide_json(after: &str) -> String {
    let guide = include_str!("../docs/guides/WORLD.md");
    let rest = &guide[guide.find(after).unwrap()..];
    let start = rest.find("```json\n").unwrap() + 8;
    rest[start..start + rest[start..].find("```").unwrap()].to_owned()
}

#[test]
fn timeline_guide_output_is_real() -> gamectx::Result<()> {
    let mut history = Timeline::new();
    for e in [
        r#"{"at":10,"kind":"fire","summary":"A fire broke out in the Ember Keep forge.","actors":["smith"],"place":"forge","tags":["disaster"],"phrases":["the fire","forge fire"]}"#,
        r#"{"at":20,"kind":"theft","summary":"Someone took the steward's ledger.","actors":["thief"],"place":"hall","things":["ledger"],"visibility":"witnessed","witnesses":["cook"],"phrases":["ledger"]}"#,
        r#"{"at":30,"kind":"pact","summary":"The miller promised his mill to the keep.","actors":["miller","steward"],"place":"mill","visibility":"private","facts":{"price":40}}"#,
    ] {
        history.record(&parse(e, 4096)?)?;
    }
    let known = history.query(&parse(r#"{"known_to":"cook","about":"What happened with the ledger?"}"#, 4096)?)?;
    assert_eq!(known.to_json(), parse(&guide_json("That query returns"), 65536)?.to_json());
    // Round trip through a save section value.
    assert_eq!(Timeline::from_value(&history.to_value())?, history);
    Ok(())
}

#[test]
fn timeline_compact_and_full() -> gamectx::Result<()> {
    let mut t = Timeline::new();
    for at in 0..gamectx::knowledge::timeline::MAX_ENTRIES as u64 {
        t.record(&parse(&format!(r#"{{"at":{at},"kind":"tick","summary":"Time passed."}}"#), 4096)?)?;
    }
    let extra = parse(r#"{"at":1,"kind":"tick","summary":"One more."}"#, 4096)?;
    assert!(t.record(&extra).is_err(), "a full timeline refuses instead of forgetting");
    assert_eq!(t.compact(100, "The first days.", "era")?, 100);
    assert_eq!(t.entries().len(), gamectx::knowledge::timeline::MAX_ENTRIES - 99);
    assert!(t.record(&extra).is_ok());
    assert!(t.retract(5000, "Never recorded.").is_err());
    Ok(())
}

#[test]
fn places_guide_output_is_real() -> gamectx::Result<()> {
    use gamectx::knowledge::codex::Codex;
    let codex = Codex::parse(&guide_json("## Places"))?;
    let query = parse(&guide_json("in `from`"), 4096)?;
    let out = codex.nearby(&query)?;
    assert_eq!(out.to_json(), parse(&guide_json("The result (real output)"), 65536)?.to_json());
    Ok(())
}

#[test]
fn place_kinds_cover_the_basics() {
    let kinds = gamectx::knowledge::places::builtin_kinds();
    for k in ["workshop", "city", "forest", "house", "castle", "cave"] {
        assert!(kinds.iter().any(|x| x.get("id").and_then(|v| v.as_str()) == Some(k)), "{k}");
    }
    assert_eq!(kinds.len(), 112);
}

#[test]
fn things_guide_output_is_real() -> gamectx::Result<()> {
    use gamectx::knowledge::things::Things;
    let mut world_things = Things::new();
    let req = parse(
        r#"{
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
    }"#,
        4096,
    )?;
    let out = world_things.workflow(&req)?;
    let first_res = out.get("results").and_then(|r| r.as_array()).and_then(|a| a.first()).unwrap();
    assert_eq!(
        first_res.to_json(),
        parse(&guide_json("That operation returns a timeline-ready entry"), 65536)?.to_json()
    );
    // Round trip through a save section value.
    assert_eq!(Things::from_value(&world_things.to_value())?, world_things);
    Ok(())
}

#[test]
fn recipes_guide_output_is_real() -> gamectx::Result<()> {
    use gamectx::knowledge::recipes::Recipes;
    let mut recipes = Recipes::new();
    let req = parse(
        r#"{
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
    }"#,
        4096,
    )?;
    let out = recipes.workflow(&req)?;
    let first_res = out.get("results").and_then(|r| r.as_array()).and_then(|a| a.first()).unwrap();
    assert_eq!(
        first_res.to_json(),
        parse(&guide_json("That recipe execution returns a timeline-ready entry"), 65536)?.to_json()
    );
    Ok(())
}

#[test]
fn species_guide_output_is_real() -> gamectx::Result<()> {
    use gamectx::knowledge::species::SpeciesWorld;
    let mut sw = SpeciesWorld::new();
    let req = parse(
        r#"{
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
    }"#,
        4096,
    )?;
    let out = sw.workflow(&req)?;
    let first_res = out.get("results").and_then(|r| r.as_array()).and_then(|a| a.first()).unwrap();
    assert_eq!(
        first_res.to_json(),
        parse(&guide_json("That animal interaction returns a timeline-ready entry"), 65536)?.to_json()
    );
    // Round trip through a save section value.
    assert_eq!(SpeciesWorld::from_value(&sw.to_value())?, sw);
    Ok(())
}

#[test]
fn communities_guide_output_is_real() -> gamectx::Result<()> {
    use gamectx::knowledge::communities::CommunityWorld;
    let mut cw = CommunityWorld::new();
    let req = parse(
        r#"{
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
    }"#,
        4096,
    )?;
    let out = cw.workflow(&req)?;
    let first_res = out.get("results").and_then(|r| r.as_array()).and_then(|a| a.first()).unwrap();
    assert_eq!(
        first_res.to_json(),
        parse(&guide_json("That barter execution returns a timeline-ready entry"), 65536)?.to_json()
    );
    // Round trip through a save section value.
    assert_eq!(CommunityWorld::from_value(&cw.to_value())?, cw);
    Ok(())
}

#[test]
fn fitness_guide_output_is_real() -> gamectx::Result<()> {
    use gamectx::character::fitness::FitnessWorld;
    let mut fw = FitnessWorld::new();
    let req = parse(
        r#"{
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
    }"#,
        4096,
    )?;
    let out = fw.workflow(&req)?;
    let first_res = out.get("results").and_then(|r| r.as_array()).and_then(|a| a.first()).unwrap();
    assert_eq!(
        first_res.to_json(),
        parse(&guide_json("That travel cost evaluation returns realistic pacing and exertion"), 65536)?.to_json()
    );
    // Round trip through a save section value.
    assert_eq!(FitnessWorld::from_value(&fw.to_value())?, fw);
    Ok(())
}
