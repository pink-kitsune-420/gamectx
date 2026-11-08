//! Smallest hand-authored life integration; no model or speech service.
use gamectx::{
    character::life::{Config, Definition, Life, Setting},
    character::tools::{CharacterTools, CharacterView},
    data::json,
    obj, Result, Value,
};
use std::sync::Arc;
fn main() -> Result<()> {
    let setting = Setting::from_value(&json::parse(include_str!("../fixtures/characters/setting.json"), 65536)?)?;
    let definition = Arc::new(Definition::draft(
        &json::parse(include_str!("../fixtures/characters/merchant-life.json"), 65536)?,
        &setting,
    )?);
    let config =
        Config::resolve(&obj! {"enabled"=>true,"balanced"=>true,"tools"=>vec!["life_recall","life_change"]}, &obj! {})?;
    let participants: Value = vec!["merchant", "player"].into();
    // Installing a draft is the host's explicit author approval operation.
    let life = Life::accept("counter", "merchant", &participants, definition, config)?;
    let mut character = CharacterTools::new("counter", "merchant", &participants)?;
    character.with_life(life)?;
    character.bind(CharacterView::new(1))?;
    character.enable(&["life_recall"])?;
    let context = character.call("life_recall", r#"{"tags":["trade"],"limit":2}"#, 1, 8192, |p| {
        p.get("action").and_then(Value::as_str) == Some("life_recall")
    })?;
    assert!(context.to_json().contains("Careful borrowers"));
    assert!(character.call("life_change", "{}", 1, 8192, |_| true).is_err());
    println!("{}", context.to_json());
    Ok(())
}
#[test]
fn runs() {
    main().unwrap();
}
