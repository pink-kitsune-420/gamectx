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
