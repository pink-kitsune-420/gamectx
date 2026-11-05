//! Run with `cargo run --example query-workflow` (offline; no model/API calls).
use gamectx::{obj, query, view, Context, Value};

fn main() -> gamectx::Result<()> {
    // The host defines field meanings. Existing game fields need not be renamed globally.
    let stock = vec![
        obj! {"id" => "moss_cake", "kind" => "food", "count" => 2, "unit_weight" => 0.25},
        obj! {"id" => "ember_cake", "kind" => "food", "count" => 6, "unit_weight" => 3.75},
        obj! {"id" => "salt_pouch", "kind" => "reagent", "count" => 14, "unit_weight" => 0.5},
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
    game.prompt("field_notes", "unit_weight is the weight of one item; stack_weight is count * unit_weight.")?;
    let mut turn = game.turn();
    // A per-turn description keeps category domains current. Rows stay in the game.
    turn.var("query_tables", query::describe(&tables)?)?;
    println!("{}", turn.build()?.text);

    // Each question gets one session over the same snapshot. These stand in for model calls.
    let mut session = query::Session::new().result_bytes(512);
    let feedback = session.apply_call(&obj! {"sql" => "SELECT SUM(stack_weight) FROM food"}, &tables)?;
    assert_eq!(feedback.get("retry"), Some(&Value::Bool(true)));
    println!("{}", feedback.to_json());
    let result =
        session.apply_call(&obj! {"sql" => "SELECT SUM(stack_weight) FROM inventory WHERE kind = food"}, &tables)?;
    assert_eq!(result.to_json(), r#"{"result":23}"#);
    assert!(session.is_finished());
    println!("{}", result.to_json());
    Ok(())
}
