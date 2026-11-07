use gamectx::{
    json,
    life::{Config, Definition, Life, Setting},
    obj,
    visual::{self, Activity, ArtBible, AssetRegistry, RegistryConfig, VisualSpec},
    Value,
};
use std::sync::Arc;
fn parse(s: &str) -> Value {
    json::parse(s, 1_500_000).unwrap()
}
fn set(v: &mut Value, k: &str, x: Value) {
    let Value::Object(xs) = v else { panic!() };
    xs.iter_mut().find(|(key, _)| key == k).unwrap().1 = x;
}
fn fixture(name: &str) -> Value {
    parse(
        &std::fs::read_to_string(format!("{}/conformance/visual/{name}/input.json", env!("CARGO_MANIFEST_DIR")))
            .unwrap(),
    )
    .get("args")
    .unwrap()
    .clone()
}
fn registry(capacity: u32, epoch: &str) -> AssetRegistry {
    AssetRegistry::new(
        RegistryConfig::from_value(&obj! {"enabled"=>true,"capacity"=>capacity,"queue_limit"=>2,"epoch"=>epoch})
            .unwrap(),
    )
}
fn patch(delta: u32) -> Value {
    obj! {"replace_tokens"=>Vec::<Value>::new(),"add_tokens"=>Vec::<Value>::new(),"negative_tokens"=>Vec::<Value>::new(),"forbid_props"=>Vec::<Value>::new(),"seed_delta"=>delta,"asset"=>Value::Null}
}
fn spec(name: &str) -> VisualSpec {
    visual::compile_fixture(&fixture(name)).unwrap()
}
#[test]
fn live_appraisal_crosses_slot_without_leaking_unobserved_text() {
    let a = fixture("neutral_portrait");
    let l = a.get("life").unwrap();
    let setting = Setting::from_value(l.get("setting").unwrap()).unwrap();
    let mut definition = l.get("definition").unwrap().clone();
    let mut attitudes = definition.get("attitudes").unwrap().as_array().unwrap().to_vec();
    set(&mut attitudes[0], "level", 10.into());
    set(&mut definition, "attitudes", attitudes.into());
    let mut life = Life::accept(
        "counter",
        "merchant",
        &vec!["merchant", "player"].into(),
        Arc::new(Definition::draft(&definition, &setting).unwrap()),
        Config::resolve(&obj! {"enabled"=>true}, &obj! {}).unwrap(),
    )
    .unwrap();
    let bible = ArtBible::from_value(a.get("bible").unwrap()).unwrap();
    let mut frame = a.get("frame").unwrap().clone();
    let before = VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle()).unwrap();
    life.experience(&obj!{"token"=>1,"at"=>1,"kind"=>"outcome","tag"=>"returned","subject"=>"player","source"=>obj!{"id"=>"receipt:1","speaker"=>"merchant","text"=>"The lamp was returned."},"audience"=>vec!["merchant","player"],"text"=>"Returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true}).unwrap();
    life.appraise("returned_lamp", 1, 1).unwrap();
    assert!(VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle()).is_err());
    set(&mut frame, "life_revision", life.revision().into());
    let after = VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle()).unwrap();
    assert_ne!(before.key(), after.key());
    assert_eq!(after.to_value().get("spec").unwrap().get("expression").unwrap().get("slot"), Some(&Value::str("warm")));
    assert_eq!(before.render().unwrap().get("seed"), after.render().unwrap().get("seed"));
    let mut registry = registry(8, "latest_state");
    registry.request(&after).unwrap();
    let repaired = registry.repair(after.key(), &patch(1)).unwrap();
    life.experience(&obj!{"token"=>2,"at"=>3,"kind"=>"outcome","tag"=>"returned","subject"=>"player","source"=>obj!{"id"=>"receipt:2","speaker"=>"merchant","text"=>"Another lamp returned."},"audience"=>vec!["merchant","player"],"text"=>"Returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true}).unwrap();
    life.appraise("returned_lamp", 2, 3).unwrap();
    set(&mut frame, "life_revision", life.revision().into());
    let latest = VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle()).unwrap();
    assert_eq!(latest.key(), after.key());
    registry.request(&latest).unwrap();
    let record = registry.inspect(repaired.get("key").unwrap().as_str().unwrap()).unwrap();
    assert_eq!(record.get("inspection").unwrap().get("state").unwrap().get("attitude_level"), Some(&20.into()));
}
#[test]
fn render_uses_only_selected_authored_cues() {
    let s = spec("closed_shutters_no_global_sun");
    let text = s.render().unwrap().to_json();
    assert!(text.contains("closed wooden shutters"));
    assert!(!text.contains("sunlight"));
    assert!(!text.contains("Raw world"));
    let empty = spec("guarded_portrait");
    let global = spec("report_omitted");
    assert_eq!(empty.key(), global.key());
    let s = spec("perceived_prop");
    let mut r = registry(8, "one");
    r.request(&s).unwrap();
    let mut p = patch(0);
    set(&mut p, "forbid_props", vec!["lamp"].into());
    let repaired = r.repair(s.key(), &p).unwrap();
    let d = r.inspect(repaired.get("key").unwrap().as_str().unwrap()).unwrap();
    let data = d.get("spec").unwrap();
    assert_eq!(data.get("environment"), Some(&Vec::<Value>::new().into()));
    assert_eq!(data.get("visible_props"), Some(&Vec::<Value>::new().into()));
    assert!(data.get("negative_tokens").unwrap().to_json().contains("no lamp"));
}
#[test]
fn candidate_requires_approval_and_ticket_is_consumed() {
    let s = spec("neutral_portrait");
    let mut r = registry(8, "one");
    let selection = r.request(&s).unwrap();
    let ticket = selection.get("generation").unwrap().get("ticket").unwrap();
    let sha = visual::content_digest(b"host image bytes").unwrap();
    r.submit(s.key(), ticket, "asset://draft.png", &sha).unwrap();
    assert!(r.submit(s.key(), ticket, "asset://draft.png", &sha).is_err());
    assert_eq!(r.request(&s).unwrap().get("asset"), Some(&Value::Null));
    assert!(r.approve(s.key(), "asset://different.png", &sha).is_err());
    r.approve(s.key(), "asset://draft.png", &sha).unwrap();
    let reused = r.request(&s).unwrap();
    assert_eq!(reused.get("exact"), Some(&true.into()));
    assert_eq!(reused.get("generation"), Some(&Value::Null));
}
#[test]
fn repair_capacity_failure_preserves_current_worker() {
    let s = spec("neutral_portrait");
    let mut r = registry(1, "one");
    let first = r.request(&s).unwrap();
    let before = r.inspect(s.key()).unwrap();
    assert!(r.repair(s.key(), &patch(1)).is_err());
    assert_eq!(before, r.inspect(s.key()).unwrap());
    r.submit(s.key(), first.get("generation").unwrap().get("ticket").unwrap(), "asset://ok.png", &"a".repeat(64))
        .unwrap();
}
#[test]
fn epoch_and_repair_cycles_reject_stale_workers() {
    let s = spec("neutral_portrait");
    let mut r = registry(8, "one");
    let initial = r.request(&s).unwrap();
    let stale = initial.get("generation").unwrap().get("ticket").unwrap();
    let b = r.repair(s.key(), &patch(1)).unwrap();
    let b = b.get("key").unwrap().as_str().unwrap();
    r.request(&s).unwrap();
    r.repair(b, &patch(u32::MAX)).unwrap();
    let latest = r.request(&s).unwrap();
    let current = latest.get("generation").unwrap().get("ticket").unwrap();
    assert_ne!(stale, current);
    assert!(r.submit(s.key(), stale, "asset://stale.png", &"a".repeat(64)).is_err());
    r.submit(s.key(), current, "asset://current.png", &"a".repeat(64)).unwrap();
    let mut reboot = registry(8, "two");
    reboot.request(&s).unwrap();
    assert!(reboot.submit(s.key(), stale, "asset://stale.png", &"a".repeat(64)).is_err());
}
#[test]
fn active_routine_and_started_receipt_are_distinct_from_proposals() {
    use gamectx::routine::{Config, Frame, Routine, Runtime};
    let a = parse(include_str!("../conformance/routines/interruption_resume/input.json"));
    let a = a.get("args").unwrap();
    let config = Config::resolve(&obj! {"routines"=>true}, &obj! {}).unwrap();
    let routine = Arc::new(Routine::from_value(a.get("definition").unwrap()).unwrap());
    let mut r =
        Runtime::new("counter", "merchant", &vec!["merchant", "player"].into(), "cycle:37", config, Some(routine))
            .unwrap();
    r.update(Frame::from_value(&obj!{"revision"=>1,"now"=>0,"conditions"=>vec!["route"],"allowed"=>vec!["work","walk"],"emergency"=>false}).unwrap()).unwrap();
    let activity = Activity::from_routine(&r).unwrap();
    assert_eq!(activity.to_value().get("id"), Some(&Value::str("work")));
    let ready = obj! {"status"=>"ready","proposal"=>obj!{"id"=>"p1","revision"=>1,"candidate"=>obj!{"id"=>"walk","kind"=>"activity"}}};
    let mut receipt = obj! {"actor"=>"merchant","proposal"=>"p1","revision"=>1,"state"=>"proposed"};
    assert!(Activity::from_initiative("counter", "merchant", &ready, &receipt).is_err());
    set(&mut receipt, "state", "started".into());
    let started = Activity::from_initiative("counter", "merchant", &ready, &receipt).unwrap();
    assert_eq!(started.to_value().get("source"), Some(&Value::str("initiative")));
    set(&mut receipt, "revision", 2.into());
    assert!(Activity::from_initiative("counter", "merchant", &ready, &receipt).is_err());
}
#[test]
fn locked_style_and_malformed_memory_values_fail_closed() {
    let mut a = fixture("neutral_portrait");
    let mut b = a.get("bible").unwrap().clone();
    set(&mut b, "width", Value::Number("1e999".into()));
    set(&mut a, "bible", b);
    assert!(visual::compile_fixture(&a).is_err());
    let s = spec("neutral_portrait");
    let mut r = registry(8, "one");
    r.request(&s).unwrap();
    let before = r.inspect(s.key()).unwrap();
    let mut p = patch(0);
    set(
        &mut p,
        "replace_tokens",
        vec![obj! {"from"=>"Original flat-color ink illustration","to"=>"different style"}].into(),
    );
    assert!(r.repair(s.key(), &p).is_err());
    assert_eq!(before, r.inspect(s.key()).unwrap());
}
#[test]
fn disabled_registry_never_queues_and_uri_is_not_loaded() {
    let mut r = AssetRegistry::new(RegistryConfig::default());
    let s = spec("neutral_portrait");
    let first = r.request(&s).unwrap();
    assert_eq!(first.get("generation"), Some(&Value::Null));
    assert_eq!(first.get("reason"), Some(&Value::str("disabled")));
    let asset = visual::AssetRef::from_bytes("asset://owned.png", b"owned bytes").unwrap();
    assert_eq!(asset.to_value().get("digest"), Some(&visual::content_digest(b"owned bytes").unwrap().into()));
}
