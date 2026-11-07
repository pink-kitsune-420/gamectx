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
fn coordinator() -> gamectx::life::Coordinator {
    let mut c = gamectx::life::Coordinator::new("counter", "merchant", &vec!["merchant", "player"].into()).unwrap();
    c.bind_scene(&scene(1, 1)).unwrap();
    c.thread(&thread("trade")).unwrap();
    c
}
fn scene(revision: u64, epoch: u64) -> Value {
    obj! {"id"=>"counter","revision"=>revision,"epoch"=>epoch,"at"=>revision,"participants"=>vec!["merchant","player"],"facts"=>vec![obj!{"id"=>"sun","revision"=>1,"observed_at"=>1,"source"=>obj!{"id"=>"sensor","speaker"=>"merchant","text"=>"Visible sun."},"kind"=>"perceived","audience"=>vec!["merchant","player"],"text"=>"Sun through a window."}],"affordances"=>vec!["serve"]}
}
fn thread(id: &str) -> Value {
    obj! {"id"=>id,"topic"=>"Lamp repairs","questions"=>vec!["Which wick?"],"promises"=>vec!["repair"],"unfinished"=>vec!["polish"],"listener"=>"player","participants"=>vec!["merchant","player"],"shared"=>vec!["sun"],"referents"=>vec!["lamp"]}
}
#[test]
fn harmless_updates_two_threads_and_one_use_validation() {
    let mut c = coordinator();
    c.thread(&thread("weather")).unwrap();
    c.generate("trade", "That wick is ready.", &vec!["sun"].into(), 0).unwrap();
    c.bind_scene(&scene(2, 1)).unwrap();
    assert_eq!(c.validate(1, 0).unwrap().get("stage").and_then(Value::as_str), Some("validated"));
    assert!(c.validate(1, 0).is_err());
    c.generate("weather", "The window is bright.", &vec!["sun"].into(), 0).unwrap();
    c.bind_scene(&scene(3, 2)).unwrap();
    assert_eq!(c.validate(2, 0).unwrap().get("stage").and_then(Value::as_str), Some("cancelled"));
}
#[test]
fn fake_speech_transport_does_not_create_hearing_or_actions() {
    use gamectx::life::SpeechTransport;
    #[derive(Default)]
    struct Fake {
        queued: Vec<u64>,
        cancelled: Vec<u64>,
    }
    impl SpeechTransport for Fake {
        fn queue(&mut self, id: u64, _: &str) -> gamectx::Result<()> {
            self.queued.push(id);
            Ok(())
        }
        fn cancel(&mut self, id: u64) -> gamectx::Result<()> {
            self.cancelled.push(id);
            Ok(())
        }
    }
    let mut c = coordinator();
    let mut transport = Fake::default();
    c.generate("trade", "I can fix it.", &vec!["sun"].into(), 0).unwrap();
    c.validate(1, 0).unwrap();
    transport.queue(1, "I can fix it.").unwrap();
    for (event, n, recipient) in [
        ("queued", 0, Value::Null),
        ("played", 5, Value::Null),
        ("interrupted", 5, Value::Null),
        ("heard", 3, Value::str("player")),
    ] {
        c.delivery(&obj! {"id"=>1,"event"=>event,"through"=>n,"recipient"=>recipient}).unwrap();
    }
    transport.cancel(1).unwrap();
    let speech = c.inspect();
    assert_eq!(speech.get("speech").unwrap().as_array().unwrap()[0].get("played"), Some(&Value::from(5)));
    assert!(c.delivery(&obj! {"id"=>1,"event"=>"heard","through"=>6,"recipient"=>"player"}).is_err());
    let interim = c.recognition(&obj! {"text"=>"buy the lamp","final"=>false,"uncertainty"=>"Name unclear"}).unwrap();
    assert_eq!(interim.get("actions").unwrap().as_array().unwrap().len(), 0);
    c.retire(1).unwrap();
    assert!(c.validate(1, 0).is_err());
}
