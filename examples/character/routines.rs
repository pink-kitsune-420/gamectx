//! Smallest optional Rust integration. No model, engine movement or clock is assumed.
use gamectx::{
    character::routine::{Config, Frame, Routine, Runtime},
    character::tools::{CharacterTools, CharacterView},
    data::json,
    obj, Result,
};
use std::sync::Arc;
pub fn main() -> Result<()> {
    let definition = Arc::new(Routine::parse(include_str!("../lantern-vault/routine.json"))?);
    let config = Config::resolve(&obj! {"routines"=>true,"tools"=>vec!["routine_status"]}, &obj! {"horizon"=>60})?;
    let mut runtime =
        Runtime::new("shop:solo", "npc:merchant", &vec!["npc:merchant"].into(), "cycle:37", config, Some(definition))?;
    // The engine resolves these IDs, route availability and tick/calendar meanings.
    let frame = json::parse(
        r#"{"revision":1,"now":0,"conditions":["route"],"allowed":["work","customer","walk"],"emergency":false}"#,
        8192,
    )?;
    let mut scheduler = gamectx::runtime::schedule::Scheduler::new(1, 1)?;
    scheduler.register(0, 1, 1, 10, 0)?;
    for _ in scheduler.poll(0)? {
        runtime.update(Frame::from_value(&frame)?)?;
    }
    let mut character = CharacterTools::new("shop:solo", "npc:merchant", &vec!["npc:merchant"].into())?;
    character.with_runtime(runtime)?;
    character.bind(CharacterView::new(1))?;
    // Registration in config does not expose a tool until the host selects it.
    character.enable(&["routine_status"])?;
    let result = character
        .call("routine_status", "{}", 1, 2048, |p| p == &obj! {"action"=>"routine_status","target"=>"npc:merchant"})?;
    assert_eq!(result.get("current").unwrap().get("id").and_then(gamectx::Value::as_str), Some("work"));
    assert_eq!(result.get("time").unwrap().get("status").and_then(gamectx::Value::as_str), Some("unknown"));
    println!("{}", result.to_json());
    Ok(())
}
