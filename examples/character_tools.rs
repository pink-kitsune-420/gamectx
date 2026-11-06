//! One actor-visible look tool, without project registration or a model connection.
use gamectx::{
    character::{CharacterTools, CharacterView},
    json, Value,
};
fn main() -> gamectx::Result<()> {
    let view = json::parse(include_str!("characters/view.json"), 16384)?;
    let participants: Value = vec!["npc:merchant", "player:a"].into();
    let mut character = CharacterTools::new("conversation:a", "npc:merchant", &participants)?;
    character.bind(CharacterView::from_value(&view)?)?;
    let tools = character.enable(&["look_around"])?;
    let result = character.call("look_around", r#"{"detail":"full"}"#, 7, 2048, |_| false)?;
    assert!(!result.to_json().contains("08:20"));
    println!("{}", tools.to_json());
    println!("{}", result.to_json());
    Ok(())
}
#[test]
fn runs() {
    main().unwrap();
}
