//! Explicit relevant experiences and event/timer wakeups, without model calls.
use gamectx::{
    json, obj,
    observation::Selection,
    routine::{Config, Frame, Routine, Runtime},
    schedule::Scheduler,
    Result, Value,
};
use std::sync::Arc;
fn frame(revision: u32, now: u32, customer: bool) -> Result<Frame> {
    Frame::from_value(
        &obj! {"revision"=>revision,"now"=>now,"conditions"=>if customer{vec!["route","customer"]}else{vec!["route"]},"allowed"=>vec!["work","customer","walk"],"emergency"=>false},
    )
}
pub fn main() -> Result<()> {
    let cfg =
        Config::resolve(&obj! {"routines"=>true,"history"=>true,"time"=>true,"time_sources"=>vec!["sun"]}, &obj! {})?;
    let mut npc = Runtime::new(
        "annex:solo",
        "merchant",
        &vec!["merchant"].into(),
        "cycle:37",
        cfg,
        Some(Arc::new(Routine::parse(include_str!("dungeon/routine.json"))?)),
    )?;
    let mut scheduler = Scheduler::new(1, 1)?;
    scheduler.register(0, 1, 1, 1000, 0)?;
    for _ in scheduler.poll(0)? {
        npc.update(frame(1, 0, false)?)?;
    }
    // Substitute an actual engine sensor confirmation in a game.
    npc.observe(&obj!{"token"=>1,"observer"=>"merchant","location"=>"annex:window","at"=>0,"expires"=>50,"kind"=>"direct","source"=>obj!{"id"=>"sensor:1","speaker"=>"merchant","text"=>"Sunlight visible through the annex window."},"audience"=>vec!["merchant"],"value"=>obj!{"visible"=>true},"uncertainty"=>"Host angular bounds.","time"=>obj!{"source"=>"sun","calendar"=>"cycle:37","low"=>3,"high"=>5,"rate_low"=>0,"rate_high"=>2,"usable"=>true}})?;
    if npc.affected(&["customer"], &[])? {
        scheduler.changed(0, false)?;
    }
    for _ in scheduler.poll(1)? {
        npc.update(frame(2, 1, true)?)?;
    }
    assert_eq!(npc.next_wake(), Some(2)); // min_active protection ends here
                                          // The engine's timer queue marks the existing job dirty when this tick arrives.
    scheduler.changed(0, false)?;
    for _ in scheduler.poll(2)? {
        npc.update(frame(3, 2, true)?)?;
    }
    let query = json::parse(
        r#"{"kinds":["direct"],"locations":["annex:window"],"time_sources":["sun"],"required":[1],"limit":1}"#,
        8192,
    )?;
    let context = npc.context_for(&Selection::from_value(&query)?)?;
    assert_eq!(context.get("current").unwrap().get("id"), Some(&Value::str("customer")));
    assert_eq!(context.get("observations").unwrap().as_array().unwrap().len(), 1);
    println!("{}", context.to_json());
    Ok(())
}
