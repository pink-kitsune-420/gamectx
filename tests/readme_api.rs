//! The "Use it in your code" example from the README, run against the Ember Keep data.

use gamectx::json::parse;
use gamectx::{obj, Context, Table, Value};

fn load(name: &str) -> Value {
    let path = format!("{}/examples/ember-keep/{name}", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap(), 1 << 20).unwrap()
}

#[test]
fn readme_example_builds_a_packet() -> gamectx::Result<()> {
    let tools = load("tools.json");
    let inventory = load("inventory.json");
    let npcs = vec![
        obj! {"name" => "Velmi", "role" => "guard", "distance" => 12.5},
        obj! {"name" => "Midor", "role" => "smith", "distance" => 3},
    ];

    let mut game = Context::new().budget_bytes(6000);
    game.tools("actions", tools)?;
    game.prompt("rules", "Ilse guards the keep. The game validates every action.")?;

    let mut turn = game.turn();
    turn.var("mood", "wary")?;
    turn.table("inventory", inventory, Table::new().group_by("kind").group_by("rarity"))?;
    turn.table("nearby_npcs", npcs, Table::new().sort_by("distance").limit(10).fields(&["name", "role"]))?;
    turn.var("event", obj! {"actor" => "player", "did" => "lit_lantern", "where" => "hall"})?;

    let packet = turn.build()?;
    let system = packet.system_prompt("You are the game logic for Warden Ilse.");

    assert!(packet.receipt.dropped.is_empty());
    assert!(packet.text.starts_with(r#"{"actions":"All arguments required; no others allowed.\nShared arguments:"#));
    assert!(packet
        .text
        .contains(r#""nearby_npcs":{"columns":["name","role"],"rows":[["Midor","smith"],["Velmi","guard"]]}"#));
    assert!(packet.text.contains(r#""inventory":{"group_by":["kind","rarity"]"#));
    assert!(system.starts_with("You are the game logic for Warden Ilse.\n\nGAME DATA:\n{"));
    Ok(())
}
