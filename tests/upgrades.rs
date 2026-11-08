use gamectx::{
    data::json::parse,
    knowledge::facts::{FactOptions, FactRegistry},
    knowledge::memory::Record,
    knowledge::recall::RecallIndex,
    knowledge::temporal::TemporalWindow,
    obj,
    runtime::brain::{Brain, BrainBundle, Prediction},
    runtime::outcome::OutcomeTracker,
    runtime::playbook::Playbook,
    runtime::requests::{RequestContext, RequestGate},
    runtime::spending::SpendingLedger,
    Value,
};
fn fixture(name: &str) -> Value {
    parse(
        &std::fs::read_to_string(format!("{}/conformance/upgrades/{name}/input.json", env!("CARGO_MANIFEST_DIR")))
            .unwrap(),
        1 << 20,
    )
    .unwrap()
}
fn bundle() -> BrainBundle {
    let v = fixture("bundle-reuse");
    BrainBundle::new(
        Brain::from_value(v.get("perception").unwrap()).unwrap(),
        Brain::from_value(v.get("decision").unwrap()).unwrap(),
        v.get("ports").unwrap(),
    )
    .unwrap()
}
#[test]
fn indexed_recall_is_bounded_explicit_and_does_not_fall_back_to_unrelated_records() {
    let rows = vec![
        Record { id: 1, turn: 0, importance: 5, text: "Lighting by the door.".into(), tags: vec![] },
        Record { id: 2, turn: 1, importance: 5, text: "A medic waits.".into(), tags: vec![] },
    ];
    let exact = RecallIndex::new(&rows, &obj! {"normalize"=>false,"aliases"=>Vec::<Value>::new()}).unwrap();
    assert!(exact.search("light", 1).unwrap().get("records").unwrap().as_array().unwrap().is_empty());
    let index = RecallIndex::new(&rows, &obj! {"normalize"=>true,"aliases"=>vec![vec!["healer","medic"]]}).unwrap();
    for q in ["light", "healer"] {
        assert_eq!(index.search(q, 1).unwrap().get("candidates"), Some(&Value::from(1)));
    }
    assert!(index.search(&"x".repeat(4097), 1).is_err());
    assert!(index.search("light", 129).is_err());
    assert!(RecallIndex::new(&rows, &obj! {"normalize"=>true,"aliases"=>vec![vec!["a","b"],vec!["b","c"]]}).is_err());
    let mut records = Vec::new();
    for i in 0..1000 {
        records.push(Record {
            id: i + 1,
            turn: i,
            importance: 5,
            text: format!("Archive entry marker_{i:04}."),
            tags: vec![],
        })
    }
    let index = RecallIndex::new(&records, &obj! {"normalize"=>false,"aliases"=>Vec::<Value>::new()}).unwrap();
    let mut visits = 0;
    for i in 0..100 {
        let hit = index.search(&format!("marker_{i:04}"), 1).unwrap();
        visits += hit.get("postings").unwrap().as_usize().unwrap();
        assert_eq!(hit.get("records").unwrap().as_array().unwrap()[0].get("id"), Some(&Value::from(i + 1)));
    }
    assert_eq!(visits, 100);
}
#[test]
fn outcome_labels_require_execution_exposure_and_single_consumption() {
    let mut tracker = OutcomeTracker::new(128).unwrap();
    for id in 1..=100 {
        tracker.begin(id, 0, 10, 5).unwrap();
    }
    for id in 1..=100 {
        tracker.executed(id, 1).unwrap();
    }
    // All unexposed trials remain unknown, never a zero-score training label.
    for id in 1..=100 {
        assert_eq!(tracker.report(id, 10).unwrap().get("trainable"), Some(&Value::Bool(false)));
        assert!(tracker.take_label(id, 10).is_err());
    }
    let mut tracker = OutcomeTracker::new(2).unwrap();
    tracker.begin(1, 0, 10, 5).unwrap();
    assert!(tracker.exposed(1, 1).is_err());
    tracker.executed(1, 1).unwrap();
    tracker.exposed(1, 2).unwrap();
    assert!(tracker.resolve(1, 3, 1000001).is_err());
    tracker.resolve(1, 3, 0).unwrap();
    assert_eq!(tracker.take_label(1, 4).unwrap().get("score"), Some(&Value::from(0)));
    assert!(tracker.take_label(1, 4).is_err());
    assert_eq!(tracker.report(1, 20).unwrap().get("trainable"), Some(&Value::Bool(false)));
    tracker.forget(1, 4).unwrap();
    assert!(tracker.begin(1, 4, 10, 5).is_err());
}
#[test]
fn temporal_windows_use_distinct_events_and_reset_observation_coverage() {
    let mut window = TemporalWindow::new(2, 10, 0).unwrap();
    window.observe(1, 1, 20, "visit", 8).unwrap();
    window.observe(2, 2, 20, "rest", 2).unwrap();
    assert!(window.observe(3, 3, 20, "visit", 1).is_err());
    assert_eq!(window.last_token(), 2);
    let before = window.features("visit", 3).unwrap();
    for _ in 0..10 {
        assert_eq!(window.features("visit", 3).unwrap(), before)
    }
    assert_eq!(window.sequence(&["visit".into(), "rest".into()], 3).unwrap(), Value::from(1));
    window.observe(3, 11, 20, "visit", 1).unwrap();
    assert!(window.observe(3, 12, 20, "visit", 1).is_err());
    window.availability(12, false).unwrap();
    assert_eq!(window.features("visit", 12).unwrap().get("count"), Some(&Value::Null));
    window.availability(13, true).unwrap();
    assert_eq!(window.features("visit", 13).unwrap().get("count"), Some(&Value::from(0)));
    assert!(window.observe(2, 13, 20, "visit", 1).is_err());
}
#[test]
fn playbooks_recheck_versions_privacy_expiry_and_current_argument_rules() {
    let mut facts = FactRegistry::new(8).unwrap();
    let source = facts
        .register("world", "ready", true, FactOptions { privacy: vec!["activity".into()], ..Default::default() })
        .unwrap();
    let book=Playbook::from_value(&Value::Array(vec![obj!{"id"=>"rest_when_ready","action"=>"rest","args"=>obj!{"duration"=>2},"requires"=>vec![obj!{"reference"=>source.to_value(),"value"=>true}],"expires"=>10}])).unwrap();
    assert_eq!(
        book.select(&facts, 1, &[], |name, args| name == "rest" && args.get("duration") == Some(&Value::from(2)))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(book.select(&facts, 1, &[], |_, _| false).unwrap().as_array().unwrap().is_empty());
    assert!(book.select(&facts, 1, &["activity".into()], |_, _| true).unwrap().as_array().unwrap().is_empty());
    assert!(book.select(&facts, 10, &[], |_, _| true).unwrap().as_array().unwrap().is_empty());
    // Same value under a different revision must still invalidate a validated procedure.
    facts
        .register(
            "world",
            "ready",
            true,
            FactOptions { expected_revision: Some(source.revision), ..Default::default() },
        )
        .unwrap();
    assert!(book.select(&facts, 1, &[], |_, _| true).unwrap().as_array().unwrap().is_empty());
}
#[test]
fn asynchronous_tickets_are_one_use_instance_bound_and_privacy_checked() {
    let mut facts = FactRegistry::new(4).unwrap();
    let reference = facts
        .register("world", "ready", true, FactOptions { privacy: vec!["activity".into()], ..Default::default() })
        .unwrap();
    let raw =
        obj! {"actor"=>"actor_a","world"=>"world_a","schema"=>1,"permissions"=>1,"facts"=>vec![reference.to_value()]};
    let context = RequestContext::from_value(&raw).unwrap();
    let mut gate = RequestGate::new();
    let ticket = gate.issue(context.clone(), 0, 10).unwrap();
    let mut other = RequestGate::new();
    assert!(other.accept(&ticket, &context, 1, &facts, &[]).is_err());
    assert!(!gate.accept(&ticket, &context, 1, &facts, &["activity".into()]).unwrap());
    assert!(gate.accept(&ticket, &context, 1, &facts, &[]).is_err());
    for field in ["actor", "world", "schema", "permissions"] {
        let mut changed = raw.clone();
        if let Value::Object(fields) = &mut changed {
            fields.iter_mut().find(|(key, _)| key == field).unwrap().1 =
                if field == "actor" || field == "world" { Value::str("other") } else { Value::from(2) };
        }
        let ticket = gate.issue(context.clone(), 1, 10).unwrap();
        assert!(!gate.accept(&ticket, &RequestContext::from_value(&changed).unwrap(), 1, &facts, &[]).unwrap());
        assert!(gate.accept(&ticket, &context, 1, &facts, &[]).is_err());
    }
    for _ in 0..32 {
        gate.issue(context.clone(), 1, 10).unwrap();
    }
    assert!(gate.issue(context.clone(), 1, 10).is_err());
}
#[test]
fn reservations_cover_concurrency_crash_ambiguity_retries_and_metering_overruns() {
    let mut ledger = SpendingLedger::new(100, 10).unwrap();
    let one = ledger.reserve(0, 60).unwrap();
    let two = ledger.reserve(0, 30).unwrap();
    assert!(ledger.reserve(0, 11).is_err());
    let saved = ledger.to_value();
    assert!(ledger.finish(one, 1, Some(0)).is_err());
    assert_eq!(ledger.to_value(), saved);
    ledger.dispatch(one, 1).unwrap();
    assert!(ledger.cancel_before_dispatch(one, 1).is_err());
    ledger.cancel_before_dispatch(two, 1).unwrap();
    ledger.finish(one, 2, Some(40)).unwrap();
    let retry = ledger.reserve(3, 60).unwrap();
    assert!(retry > two);
    let mut recovered = SpendingLedger::restore_json(&ledger.to_value().to_json(), 4).unwrap();
    assert_eq!(recovered.summary(4).unwrap().get("spent"), Some(&Value::from(100)));
    assert!(recovered.cancel_before_dispatch(retry, 4).is_err());
    assert_eq!(recovered.summary(14).unwrap().get("spent"), Some(&Value::from(0)));
    let id = recovered.reserve(14, 20).unwrap();
    recovered.dispatch(id, 14).unwrap();
    recovered.finish(id, 15, Some(150)).unwrap();
    assert!(recovered.reserve(100, 1).is_err());
    assert!(recovered.reserve(100, 0).is_ok());
    let saved = recovered.to_value();
    assert_eq!(
        SpendingLedger::restore(&saved, 100).unwrap().summary(100).unwrap().get("blocked"),
        Some(&Value::Bool(true))
    );
}
#[test]
fn bundle_frames_and_module_updates_preserve_isolation_and_unknowns() {
    let mut bundle = bundle();
    let frame = bundle.prepare("actor_a", 1, &[Some(800000)]).unwrap();
    let mut out = Prediction::default();
    bundle.run_into(&frame, "actor_a", 1, &[None, Some(200000)], 1, 0, 0, &mut out).unwrap();
    assert_eq!(out.scores(), [Some(800000)]);
    let before = out.clone();
    assert!(bundle.run_into(&frame, "actor_b", 1, &[None, Some(200000)], 1, 0, 0, &mut out).is_err());
    assert_eq!(out, before);
    let other = bundle.prepare("actor_a", 2, &[None]).unwrap();
    assert!(bundle.run_into(&frame, "actor_a", 1, &[None, Some(200000)], 1, 0, 0, &mut out).is_err());
    bundle.run_into(&other, "actor_a", 2, &[None, Some(200000)], 1, 0, 0, &mut out).unwrap();
    assert_eq!(out.scores(), [None]);
    assert_eq!(out.choice(), None);
    let saved_decision = bundle.decision().to_value();
    let mut perception = bundle.perception().clone();
    let report=perception.teach(&obj!{"id"=>"adjust","expected_revision"=>0,"changes"=>vec![obj!{"node"=>"alert","source"=>"$bias","delta"=>0.1}],"validation"=>vec![obj!{"inputs"=>obj!{"danger"=>0.2},"targets"=>obj!{"alert"=>0.3}}]}).unwrap();
    assert_eq!(report.get("accepted"), Some(&Value::Bool(true)));
    bundle.replace_perception(perception).unwrap();
    assert_eq!(bundle.decision().to_value(), saved_decision);
    assert!(bundle.run_into(&other, "actor_a", 2, &[None, Some(200000)], 1, 0, 0, &mut out).is_err());
    let saved = bundle.to_value();
    assert_eq!(BrainBundle::restore_json(&saved.to_json()).unwrap().to_value(), saved);
    let frame = bundle.prepare("actor_a", 3, &[Some(100000)]).unwrap();
    let before = bundle.to_value();
    assert!(bundle.replace_decision(Brain::dense(&["unbound"], &["act"], 0, 3).unwrap()).is_err());
    assert_eq!(bundle.to_value(), before);
    bundle.run_into(&frame, "actor_a", 3, &[None, Some(200000)], 1, 0, 0, &mut out).unwrap();
    let saved_perception = bundle.perception().to_value();
    let mut decision = bundle.decision().clone();
    assert_eq!(decision.teach(&obj!{"id"=>"decision_adjust","expected_revision"=>0,"changes"=>vec![obj!{"node"=>"wait","source"=>"$bias","delta"=>0.1}],"validation"=>vec![obj!{"inputs"=>obj!{"alert"=>0.2,"cost"=>0.2},"targets"=>obj!{"wait"=>0.3}}]}).unwrap().get("accepted"),Some(&Value::Bool(true)));
    bundle.replace_decision(decision).unwrap();
    assert_eq!(bundle.perception().to_value(), saved_perception);
    assert!(bundle.run_into(&frame, "actor_a", 3, &[None, Some(200000)], 1, 0, 0, &mut out).is_err());
}
#[test]
fn shared_perception_matches_duplicate_evaluation_on_three_fresh_cohorts() {
    let mut checks = 0;
    for seed in [9101, 9102, 9103] {
        let perception = Brain::dense(&["signal"], &["alert"], 4, seed).unwrap();
        let decision = Brain::dense(&["alert", "cost"], &["wait"], 4, seed + 1).unwrap();
        let mut bundle = BrainBundle::new(
            perception.clone(),
            decision.clone(),
            &Value::Array(vec![obj! {"input"=>"alert","output"=>"alert"}]),
        )
        .unwrap();
        let mut raw = Prediction::default();
        let mut expected = Prediction::default();
        let mut actual = Prediction::default();
        for frame in 0..100 {
            let value = if frame % 17 == 0 { None } else { Some((frame * 7919 % 1000001) as i64) };
            let prepared = bundle.prepare("actor_a", frame, &[value]).unwrap();
            for candidate in 0..12 {
                let cost = Some((candidate * 7919 % 1000001) as i64);
                perception.run_into(&[value], 0, 0, 0, &mut raw).unwrap();
                decision.run_into(&[raw.scores()[0], cost], 1, 0, 0, &mut expected).unwrap();
                bundle.run_into(&prepared, "actor_a", frame, &[None, cost], 1, 0, 0, &mut actual).unwrap();
                assert_eq!(actual, expected);
                checks += 1;
            }
        }
    }
    assert_eq!(checks, 3600);
}
