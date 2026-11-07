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

#[test]
fn adaptive_brain_readme_example() -> gamectx::Result<()> {
    use gamectx::brain::Brain;
    let sample = |danger: f64, need: f64| {
        obj! {
            "inputs" => obj! {"danger" => danger, "need" => need},
            "targets" => obj! {"withdraw" => danger, "assist" => need * (1.0 - danger)}
        }
    };
    let mut brain = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 4, 3)?;
    let report = brain.train(&obj! {
        "id" => "first_training", "expected_revision" => 0,
        "examples" => vec![sample(0.1,0.2),sample(0.2,0.8),sample(0.8,0.1),sample(0.9,0.7)],
        "validation" => vec![sample(0.15,0.75),sample(0.75,0.15)], "epochs" => 128, "rate" => 0.1
    })?;
    assert_eq!(report.get("accepted"), Some(&Value::Bool(true)));
    let prediction = brain.run(&obj! {"danger" => 0.8, "need" => 0.2}, &["withdraw", "assist"])?;
    assert_eq!(prediction.get("choice").and_then(Value::as_str), Some("withdraw"));
    assert_eq!(Brain::restore_json(&brain.to_value().to_json())?.to_value(), brain.to_value());
    Ok(())
}

#[test]
fn brain_selection_readme_example() -> gamectx::Result<()> {
    use gamectx::brain::Brain;
    let sample = |danger: f64, need: f64| {
        obj! {
            "inputs" => obj! {"danger" => danger, "need" => need},
            "targets" => obj! {"withdraw" => danger, "assist" => need * (1.0 - danger)}
        }
    };
    let mut brain = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 4, 3)?;
    let examples = vec![sample(0.1, 0.2), sample(0.2, 0.8), sample(0.8, 0.1), sample(0.9, 0.7)];
    let validation = vec![sample(0.15, 0.75), sample(0.75, 0.15)];
    let report = brain.train_select(&obj! {
        "id" => "selected_training", "expected_revision" => brain.revision(),
        "examples" => examples, "validation" => validation, "epochs" => 128,
        "rates" => vec![0.03,0.1,0.3], "max_steps" => 2_000_000
    })?;
    assert_eq!(report.get("accepted"), Some(&Value::Bool(true)));
    assert_eq!(brain.revision(), 1);
    assert_eq!(brain.to_value().get("history").unwrap().as_array().unwrap().len(), 1);
    Ok(())
}

#[test]
fn preloaded_cache_readme_example() -> gamectx::Result<()> {
    use gamectx::brain::{Brain, CachedBrain};
    let saved = include_str!("../examples/brain/preloaded.json");
    let template = Brain::restore_json(saved)?;
    let mut actor = CachedBrain::new(template.clone(), 32)?;
    let other = CachedBrain::new(template, 32)?;
    let input = obj! {"danger" => 0.8,"need" => 0.5};
    let result = actor.run(&input, &["withdraw", "assist"])?;
    assert_eq!(actor.run(&input, &["withdraw", "assist"])?, result);
    assert_eq!(actor.stats().hits, 1);
    actor.brain_mut().undo(1)?;
    assert_eq!(actor.stats().entries, 0);
    assert_ne!(actor.run(&input, &["withdraw", "assist"])?, result);
    assert_eq!(other.brain().revision(), 1);
    Ok(())
}

#[test]
fn native_inference_readme_example() -> gamectx::Result<()> {
    use gamectx::brain::{Brain, Prediction};
    let brain = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 4, 3)?;
    let mut prediction = Prediction::default();
    brain.run_into(&[Some(800_000), Some(200_000)], 0b11, 0, 0, &mut prediction)?;
    assert_eq!(prediction.scores().len(), 2);
    Ok(())
}

#[path = "../examples/runtime-loop.rs"]
mod runtime_loop;
#[test]
fn combined_runtime_documented_example() -> gamectx::Result<()> {
    runtime_loop::main()
}

#[path = "../examples/evidence-memory.rs"]
mod evidence_memory;
#[path = "../examples/integration-tools.rs"]
mod integration_tools;
#[test]
fn documented_integration_and_evidence_examples() -> gamectx::Result<()> {
    integration_tools::main()?;
    evidence_memory::main()
}

#[path = "../examples/registered-facts.rs"]
mod registered_facts;
#[test]
fn documented_registered_facts_example() -> gamectx::Result<()> {
    registered_facts::main()
}

#[path = "../examples/runtime-upgrades.rs"]
mod runtime_upgrades;
#[test]
fn documented_runtime_upgrades_example() -> gamectx::Result<()> {
    runtime_upgrades::main()
}

#[path = "../examples/query-plans.rs"]
mod query_plans;
#[test]
fn documented_query_plan_example() -> gamectx::Result<()> {
    query_plans::main()
}

#[path = "../examples/registered-project.rs"]
mod registered_project;
#[test]
fn documented_project_example() -> gamectx::Result<()> {
    registered_project::main()
}

#[path = "../examples/local-turn.rs"]
mod local_turn;
#[test]
fn documented_local_turn_example() -> gamectx::Result<()> {
    local_turn::main()
}

#[path = "../examples/optimized-turn.rs"]
mod optimized_turn;
#[test]
fn documented_optimization_example() -> gamectx::Result<()> {
    optimized_turn::main()
}

#[path = "../examples/task-workflow.rs"]
mod task_workflow;
#[test]
fn documented_size_task_workflow_example() -> gamectx::Result<()> {
    task_workflow::main()
}

#[path = "../examples/conversation.rs"]
mod conversation_example;
#[test]
fn documented_shared_conversation_example() -> gamectx::Result<()> {
    conversation_example::main()
}

#[path = "../examples/lore.rs"]
mod lore_example;
#[test]
fn documented_lore_example() -> gamectx::Result<()> {
    lore_example::main()
}

#[path = "../examples/info_tools.rs"]
mod info_tools_example;
#[test]
fn documented_information_tools_example() -> gamectx::Result<()> {
    info_tools_example::main()
}

#[path = "../examples/conversation_tools.rs"]
mod conversation_tools_example;
#[test]
fn documented_conversation_tools_example() -> gamectx::Result<()> {
    conversation_tools_example::main()
}

#[path = "../examples/routines.rs"]
mod routines_example;
#[test]
fn documented_optional_routine_example() -> gamectx::Result<()> {
    routines_example::main()
}

#[path = "../examples/routine-context.rs"]
mod routine_context_example;
#[test]
fn documented_routine_context_example() -> gamectx::Result<()> {
    routine_context_example::main()
}

#[allow(dead_code)]
#[path = "../examples/character-life.rs"]
mod character_life_example;
