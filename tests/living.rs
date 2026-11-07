use gamectx::{
    cast::CastPack,
    json,
    living::{LivingWorld, SpeechQueue},
    obj, Value,
};
use std::sync::Arc;
fn pack() -> Value {
    json::parse(include_str!("../examples/cast/chime-town.json"), 1_048_576).unwrap()
}
fn world() -> LivingWorld {
    LivingWorld::new(Arc::new(CastPack::from_value(&pack()).unwrap()), "town_save_a").unwrap()
}
fn fixture(name: &str) -> Value {
    let text =
        std::fs::read_to_string(format!("{}/conformance/living-world/{name}/input.json", env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    json::parse(&text, 1_572_864).unwrap()
}
fn arg(v: &Value) -> &Value {
    v.get("args").unwrap()
}
fn stamp(n: u32) -> Value {
    obj! {"actor"=>"merchant","world"=>"town_save_a","schema"=>n,"permissions"=>1,"facts"=>Vec::<Value>::new()}
}
#[test]
fn pack_checks_whole_cast_before_acceptance_and_preserves_shared_definitions() {
    let p = CastPack::from_value(&pack()).unwrap();
    assert_eq!(p.summary().get("characters").unwrap().as_array().unwrap().len(), 4);
    let one = LivingWorld::new(Arc::new(p.clone()), "save_a").unwrap();
    let two = LivingWorld::new(Arc::new(p), "save_b").unwrap();
    assert_ne!(one.speech_context("merchant", 1).unwrap(), two.speech_context("merchant", 1).unwrap());
    let bad = fixture("pack_dangling_gate");
    assert!(CastPack::from_value(arg(&bad)).is_err());
}
#[test]
fn reports_stay_reports_and_group_influence_does_not_corrupt_life_checkpoints() {
    let f = fixture("rumor_chain_and_ripple");
    let (world, results) = gamectx::living::replay(arg(&f)).unwrap();
    let report = results[2].get("result").unwrap().get("reports").unwrap().as_array().unwrap()[0].clone();
    assert_eq!(report.get("hops"), Some(&2.into()));
    assert_eq!(report.get("event").unwrap().get("kind"), Some(&Value::str("report")));
    let base = world.base_life("keeper").unwrap();
    let state = base.inspect();
    assert!(state.get("changes").unwrap().as_array().unwrap().is_empty());
    let mut copy = base.clone();
    assert!(copy.appraise("careful_return", 1, 1).is_err());
    let id = obj! {"campaign"=>"chime","branch"=>"save_a","instance"=>"keeper"};
    let checkpoint = base.checkpoint(&id).unwrap();
    let mut restored = world.base_life("keeper").unwrap().clone();
    restored
        .restore_checkpoint(
            &id,
            &checkpoint,
            &obj! {"min_revision"=>1,"max_revision"=>1,"min_token"=>1,"max_token"=>1,"max_tick"=>4},
        )
        .unwrap();
    assert_eq!(restored.inspect(), state);
    assert_eq!(world.influence_ledger().as_array().unwrap().len(), 2);
    assert_eq!(world.rumor_ledger().as_array().unwrap().len(), 3);
}
#[test]
fn effective_ripples_control_actual_secret_selection() {
    let before = gamectx::local::process(&fixture("compile_before_ripple")).unwrap();
    let after = gamectx::local::process(&fixture("compile_after_ripple")).unwrap();
    assert!(!before.get("request").unwrap().to_json().contains("copper sparrow"));
    assert!(after.get("request").unwrap().to_json().contains("copper sparrow"));
    let messages = after.get("request").unwrap().get("messages").unwrap().as_array().unwrap();
    assert!(!messages[1].to_json().contains("group_influences"));
    assert!(messages[3].to_json().contains("group_influences"));
}
#[test]
fn immediate_fallback_and_before_delivery_upgrade_do_not_confirm_hearing() {
    let mut q = SpeechQueue::new();
    let (h, line) = q.issue(&stamp(1), 1, 10, "I am listening.").unwrap();
    assert_eq!(line.get("line"), Some(&Value::str("I am listening.")));
    let result = q.complete(&h, &stamp(1), 2, r#"{"speech":"Your lamp is ready."}"#, |_| true).unwrap();
    assert_eq!(result.get("stage"), Some(&Value::str("upgraded")));
    let frozen = q.freeze(&h, &stamp(1), 3).unwrap();
    assert_eq!(frozen.get("line"), Some(&Value::str("Your lamp is ready.")));
    assert!(q.complete(&h, &stamp(1), 4, r#"{"speech":"Another line."}"#, |_| true).is_err());
    assert!(!q.inspect().to_json().contains("heard"));
    q.retire(&h, 4).unwrap();
    assert!(q.inspect().as_array().unwrap().is_empty());
}
#[test]
fn foreign_stale_replayed_and_unapproved_completions_cannot_replace_lines() {
    let mut q = SpeechQueue::new();
    let mut other = SpeechQueue::new();
    let (h, _) = q.issue(&stamp(1), 0, 5, "One moment, please.").unwrap();
    let (foreign, _) = other.issue(&stamp(1), 0, 5, "I am listening.").unwrap();
    assert!(q.freeze(&foreign, &stamp(1), 1).is_err());
    let mut called = false;
    let result = q
        .complete(&h, &stamp(2), 2, r#"{"speech":"Ready."}"#, |_| {
            called = true;
            true
        })
        .unwrap();
    assert!(!called);
    assert_eq!(result.get("reason"), Some(&Value::str("stale")));
    assert_eq!(result.get("line"), Some(&Value::str("One moment, please.")));
    assert!(q.freeze(&h, &stamp(2), 3).is_err());
    assert!(q.complete(&h, &stamp(1), 3, r#"{"speech":"Ready."}"#, |_| true).is_err());
}
#[test]
fn malformed_model_text_retains_raw_output_and_fallback() {
    let mut q = SpeechQueue::new();
    let (h, _) = q.issue(&stamp(1), 1, 10, "I am listening.").unwrap();
    let raw = r#"{"speech":"I can also show a "}"#;
    let result = q.complete(&h, &stamp(1), 2, raw, |_| true).unwrap();
    assert_eq!(result.get("raw"), Some(&Value::str(raw)));
    assert_eq!(result.get("line"), Some(&Value::str("I am listening.")));
    assert!(q.inspect().to_json().contains("I can also show a"));
}
#[test]
fn clock_only_frames_do_not_starve_pending_speech() {
    let mut w = world();
    let context = w.speech_context("merchant", 1).unwrap();
    let mut q = SpeechQueue::new();
    let (h, _) = q.issue(&context, 0, 100, "I am listening.").unwrap();
    for now in 1..20 {
        w.tick(&obj! {"now"=>now,"windows"=>Vec::<Value>::new(),"routines"=>Vec::<Value>::new()}).unwrap();
    }
    assert_eq!(context, w.speech_context("merchant", 1).unwrap());
    assert_eq!(
        q.complete(&h, &w.speech_context("merchant", 1).unwrap(), 20, r#"{"speech":"I am here."}"#, |_| true)
            .unwrap()
            .get("stage"),
        Some(&Value::str("upgraded"))
    );
}
#[test]
fn scan_budget_and_known_roots_prevent_unbounded_work_or_ripple_replay() {
    let result = gamectx::local::process(&fixture("bounded_scan_recovers")).unwrap();
    let rows = result.get("results").unwrap().as_array().unwrap();
    let mut sent = 0;
    for row in &rows[1..] {
        let r = row.get("result").unwrap();
        assert!(r.get("checked_pairs").unwrap().as_usize().unwrap() <= 1);
        sent += r.get("reports").unwrap().as_array().unwrap().len();
    }
    assert_eq!(sent, 1);
}
#[test]
fn queue_capacity_requires_retirement_and_ids_do_not_replay() {
    let mut q = SpeechQueue::new();
    let mut handles = Vec::new();
    for _ in 0..32 {
        handles.push(q.issue(&stamp(1), 0, 10, "I am listening.").unwrap().0);
    }
    assert!(q.issue(&stamp(1), 0, 10, "I am listening.").is_err());
    q.freeze(&handles[0], &stamp(1), 1).unwrap();
    q.retire(&handles[0], 1).unwrap();
    let (new, _) = q.issue(&stamp(1), 1, 10, "I am listening.").unwrap();
    assert_eq!(new.id(), 33);
    assert!(q.freeze(&handles[0], &stamp(1), 2).is_err());
}
#[test]
fn real_routines_and_initiative_run_before_any_worker() {
    let f = fixture("immediate_routine_and_initiative");
    let (w, rows) = gamectx::living::replay(arg(&f)).unwrap();
    assert_eq!(
        rows[0].get("result").unwrap().get("routines").unwrap().as_array().unwrap()[0].get("ok"),
        Some(&true.into())
    );
    assert_eq!(rows[1].get("result").unwrap().get("status"), Some(&Value::str("proposed")));
    assert!(w.routine_context("guard").unwrap().to_json().contains("work"));
    assert!(w.base_life("guard").unwrap().inspect().get("events").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn later_direct_appraisal_does_not_reapply_a_saturated_group_effect() {
    let result = gamectx::local::process(&fixture("ripple_then_direct_appraisal")).unwrap();
    let rows = result.get("results").unwrap().as_array().unwrap();
    let inspection = rows.last().unwrap().get("result").unwrap();
    assert_eq!(
        inspection.get("base").unwrap().get("attitudes").unwrap().as_array().unwrap()[0].get("level"),
        Some(&94.into())
    );
    assert_eq!(inspection.get("effective_attitudes").unwrap().as_array().unwrap()[0].get("level"), Some(&95.into()));
}
