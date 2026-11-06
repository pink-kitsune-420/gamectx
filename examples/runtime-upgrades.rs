//! Synthetic host integration; no world actions, network calls or real charges.
//! Run with `cargo run --example runtime-upgrades`.
use gamectx::{
    brain::{Brain, BrainBundle, Prediction},
    facts::{FactOptions, FactRegistry},
    memory::Record,
    obj,
    outcome::OutcomeTracker,
    playbook::Playbook,
    recall::RecallIndex,
    requests::{RequestContext, RequestGate},
    spending::SpendingLedger,
    temporal::TemporalWindow,
    Value,
};
pub fn main() -> gamectx::Result<()> {
    let mut facts = FactRegistry::new(8)?;
    let ready = facts.register("actor_a", "ready", true, FactOptions::default())?;
    let records = vec![Record {
        id: 1,
        turn: 0,
        importance: 5,
        text: "Lighting marks the safe entrance.".into(),
        tags: vec!["route".into()],
    }];
    let index = RecallIndex::new(&records, &obj! {"normalize"=>true,"aliases"=>vec![vec!["gateway","entrance"]]})?;
    assert_eq!(index.search("gateway", 1)?.get("candidates"), Some(&Value::from(1)));
    let book = Playbook::from_value(&Value::Array(vec![
        obj! {"id"=>"rest_ready","action"=>"rest","args"=>obj!{"duration"=>2},"requires"=>vec![obj!{"reference"=>ready.to_value(),"value"=>true}],"expires"=>20},
    ]))?;
    let drafts =
        book.select(&facts, 0, &[], |action, args| action == "rest" && args.get("duration") == Some(&Value::from(2)))?;
    assert_eq!(drafts.as_array().unwrap().len(), 1); // Host still validates before execution.
    let context = RequestContext::from_value(
        &obj! {"actor"=>"actor_a","world"=>"world_a","schema"=>1,"permissions"=>1,"facts"=>vec![ready.to_value()]},
    )?;
    let mut gate = RequestGate::new();
    let ticket = gate.issue(context.clone(), 0, 10)?;
    assert!(gate.accept(&ticket, &context, 1, &facts, &[])?);
    let mut outcomes = OutcomeTracker::new(8)?;
    outcomes.begin(1, 1, 10, 5)?;
    outcomes.executed(1, 2)?;
    outcomes.exposed(1, 3)?;
    outcomes.resolve(1, 4, 800000)?;
    let label = outcomes.take_label(1, 4)?;
    assert!(outcomes.take_label(1, 4).is_err());
    let mut temporal = TemporalWindow::new(16, 20, 0)?;
    temporal.observe(1, 1, 30, "visit", 8)?;
    temporal.observe(2, 2, 30, "rest", 3)?;
    assert_eq!(temporal.sequence(&["visit".into(), "rest".into()], 4)?, Value::from(1));
    let perception = Brain::dense(&["signal"], &["alert"], 4, 9101)?;
    let decision = Brain::dense(&["alert", "cost"], &["wait"], 4, 9102)?;
    let mut bundle =
        BrainBundle::new(perception, decision, &Value::Array(vec![obj! {"input"=>"alert","output"=>"alert"}]))?;
    let frame = bundle.prepare("actor_a", 4, &[Some(800000)])?;
    let mut prediction = Prediction::default();
    for candidate in 0..12 {
        bundle.run_into(&frame, "actor_a", 4, &[None, Some(candidate * 50000)], 1, 0, 0, &mut prediction)?;
    }
    let mut ledger = SpendingLedger::new(100, 100)?;
    let reservation = ledger.reserve(0, 50)?;
    let durable = ledger.to_value(); // Host atomically saves before dispatch.
    ledger.dispatch(reservation, 1)?;
    ledger.finish(reservation, 2, Some(20))?;
    assert_eq!(SpendingLedger::restore(&durable, 2)?.summary(2)?.get("spent"), Some(&Value::from(50))); // Ambiguous recovery keeps the full estimate.
    println!("{}",(obj!{"synthetic"=>true,"drafts"=>drafts,"label"=>label,"features"=>temporal.features("visit",4)?,"simulated_spending"=>ledger.summary(2)?}).to_json());
    Ok(())
}
