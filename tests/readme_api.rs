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
    let player_said = "Do you have any rare weapons?";
    let stock = inventory.clone();
    turn.table("inventory", inventory, Table::new().group_by("kind").group_by("rarity").relevant_to(player_said))?;
    // The README uses 15; the Ember Keep inventory's largest count is 12.
    turn.table("plenty", stock, Table::new().filter("count", ">=", 5).fields(&["id", "count"]))?;
    turn.table("nearby_npcs", npcs, Table::new().sort_by("distance").limit(10).fields(&["name", "role"]))?;
    turn.var("event", obj! {"actor" => "player", "did" => "lit_lantern", "where" => "hall"})?;
    let mut memory = gamectx::memory::Memory::new();
    memory.remember(3, "The player asked about rare weapons yesterday.", &["player"], 5)?;
    turn.memory("memories", &memory, player_said, 8)?;

    let packet = turn.build()?;
    let system = packet.system_prompt("You are the game logic for Warden Ilse.");

    assert!(packet.receipt.dropped.is_empty());
    assert!(packet.text.contains(r#""memories":{"columns":["id","turn","text","tags"],"rows":[[1,3,"The player asked about rare weapons yesterday.",["player"]]]}"#));
    assert!(packet.text.starts_with(r#"{"actions":"All arguments required; no others allowed.\nShared arguments:"#));
    assert!(packet
        .text
        .contains(r#""nearby_npcs":{"columns":["name","role"],"rows":[["Midor","smith"],["Velmi","guard"]]}"#));
    assert!(packet.text.contains(r#""inventory":{"group_by":["kind","rarity"]"#));
    // "rare" matches only the quiet bell; there are no weapons, so that word matches nothing.
    assert!(packet.text.contains(r#""inventory":{"group_by":["kind","rarity"],"columns":["id","count","durability"],"groups":{"tool":{"rare":[["quiet_bell",1,1.0]]}}}"#));
    assert!(packet.text.contains(
        r#""plenty":{"columns":["id","count"],"rows":[["salt_pouch",12],["mend_moss",7],["wardstone_chip",5]]}"#
    ));
    assert!(system.starts_with("You are the game logic for Warden Ilse.\n\nGAME DATA:\n{"));
    Ok(())
}

#[test]
fn readme_query_example() -> gamectx::Result<()> {
    use gamectx::{json, query};

    let rows = json::parse(
        r#"[{"id":"moss_cake","count":2,"weight":0.25},
    {"id":"ember_cake","count":6,"weight":3.75}]"#,
        4096,
    )?;
    let tables = [("inventory", rows.as_array().unwrap())];
    let answer = query::run("SELECT SUM(count * weight) FROM inventory WHERE count >= 6", &tables)?;
    assert_eq!(answer.to_json(), "22.5");
    Ok(())
}

#[test]
fn schema_only_query_workflow_example() -> gamectx::Result<()> {
    use gamectx::{query, view};
    let stock = vec![
        obj! {"id" => "moss_cake", "kind" => "food", "count" => 2, "unit_weight" => 0.25},
        obj! {"id" => "ember_cake", "kind" => "food", "count" => 6, "unit_weight" => 3.75},
    ];
    let stock = view::apply(
        &stock,
        &view::View {
            derive: vec![("stack_weight".to_owned(), "count * unit_weight".to_owned())],
            ..Default::default()
        },
    )?;
    let tables = [("inventory", stock.as_slice())];
    let mut game = Context::new().budget_bytes(4096);
    game.tools("actions", vec![query::tool()])?;
    game.prompt("field_notes", "unit_weight is per item; stack_weight is count * unit_weight.")?;
    let mut turn = game.turn();
    turn.var("query_tables", query::describe(&tables)?)?;
    let packet = turn.build()?;
    assert!(packet.text.contains(r#"["stack_weight","number"]"#));
    assert!(!packet.text.contains("\"rows\""));

    let mut session = query::Session::new().result_bytes(512);
    let feedback = session.apply_call(&obj! {"sql" => "SELECT SUM(stack_weight) FROM food"}, &tables)?;
    assert_eq!(feedback.get("retry"), Some(&Value::Bool(true)));
    let result =
        session.apply_call(&obj! {"sql" => "SELECT SUM(stack_weight) FROM inventory WHERE kind = food"}, &tables)?;
    assert_eq!(result.to_json(), r#"{"result":23}"#);
    assert!(session.is_finished());
    Ok(())
}
