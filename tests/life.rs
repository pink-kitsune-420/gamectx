use gamectx::{
    json,
    life::{Config, Definition, Life, Setting},
    obj, Value,
};
use std::sync::Arc;
fn parse(s: &str) -> Value {
    json::parse(s, 524288).unwrap()
}
fn life() -> Life {
    let setting = Setting::from_value(&parse(include_str!("../examples/characters/setting.json"))).unwrap();
    let definition = Arc::new(
        Definition::draft(&parse(include_str!("../examples/characters/merchant-life.json")), &setting).unwrap(),
    );
    Life::accept(
        "counter",
        "merchant",
        &vec!["merchant", "player"].into(),
        definition,
        Config::resolve(&obj! {"enabled"=>true}, &obj! {}).unwrap(),
    )
    .unwrap()
}
fn event(token: u64, at: u64, kind: &str) -> Value {
    obj! {"token"=>token,"at"=>at,"kind"=>kind,"tag"=>"returned","subject"=>"player","source"=>obj!{"id"=>format!("receipt:{token}"),"speaker"=>"merchant","text"=>"Lamp returned intact."},"audience"=>vec!["merchant","player"],"text"=>"Lamp returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true}
}
#[test]
fn appraisal_is_bounded_evidenced_and_cannot_replay() {
    let mut l = life();
    l.experience(&event(1, 1, "report")).unwrap();
    assert!(l.appraise("returned_lamp", 1, 1).is_err());
    l.experience(&event(2, 2, "outcome")).unwrap();
    let proposed = l.propose(r#"{"rule":"returned_lamp","event":2,"reason":"An intact return."}"#).unwrap();
    assert_eq!(proposed.get("status").and_then(Value::as_str), Some("proposed"));
    assert_eq!(l.revision(), 2);
    l.appraise("returned_lamp", 2, 2).unwrap();
    assert!(l.appraise("returned_lamp", 2, 3).is_err());
    assert!(l.experience(&event(2, 3, "outcome")).is_err());
    let before = l.inspect();
    assert!(l.appraise("missing", 2, 3).is_err());
    assert_eq!(before, l.inspect());
}
#[test]
fn parser_bounds() {
    let l = life();
    assert!(l.propose(&"x".repeat(4097)).is_err());
    assert!(l.propose(r#"{"rule":"returned_lamp","event":1,"reason":"x","enabled":true}"#).is_err());
}
#[test]
fn private_scope_is_not_shared() {
    let l = life();
    assert!(!l.identity_matches("elsewhere", "merchant", &["merchant".into(), "player".into()]));
}
