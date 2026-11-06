//! Explicit facts and structural claim checks: `cargo run --example registered-facts`.
use gamectx::{
    facts::{Authority, ClaimStatus, FactOptions, FactRegistry},
    obj, Context,
};
pub fn main() -> gamectx::Result<()> {
    let mut facts = FactRegistry::new(32)?;
    let rule = facts.register(
        "rules",
        "night_start_tick",
        18000,
        FactOptions { authority: Authority::User, pinned: true, ..Default::default() },
    )?;
    let exit = facts.register("map", "exit_open", true, FactOptions { expires: Some(10), ..Default::default() })?;
    facts.register(
        "plan",
        "can_leave",
        true,
        FactOptions { dependencies: vec![exit.clone()], ..Default::default() },
    )?;
    assert_eq!(
        facts.check("rules", "night_start_tick", &gamectx::Value::from(18000), 1, &[])?.status,
        ClaimStatus::Supported
    );
    assert_eq!(
        facts.check("rules", "night_start_tick", &gamectx::Value::from(17000), 1, &[])?.status,
        ClaimStatus::Conflicting
    );
    assert!(facts
        .register(
            "rules",
            "night_start_tick",
            17000,
            FactOptions { expected_revision: Some(rule.revision), ..Default::default() }
        )
        .is_err());
    facts.register(
        "map",
        "exit_open",
        false,
        FactOptions { now: 1, expected_revision: Some(exit.revision), ..Default::default() },
    )?;
    assert!(facts.get("plan", "can_leave", 1, &[])?.is_none());
    let saved = facts.to_value();
    assert_eq!(FactRegistry::restore_json(&saved.to_json())?.to_value(), saved);
    let mut context = Context::new().budget_bytes(2048);
    context.prompt("rules", "Only the host registers facts. Validate actions against current state.")?;
    let mut turn = context.turn();
    turn.facts("knowledge", &facts, 1, 16, 1024, &[])?;
    let packet = turn.build()?;
    println!("{}",(obj!{"packet"=>packet.value,"missing_claim"=>facts.check("world","weather",&gamectx::Value::str("rain"),1,&[])?.to_value()}).to_json());
    Ok(())
}
