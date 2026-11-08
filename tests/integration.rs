use gamectx::{
    data::json::parse,
    knowledge::memory::{EvidenceMemory, Journal},
    obj,
    runtime::brain::{BoundBrain, Brain, FeatureSchema, Prediction, Selection, SelectionPolicy, StableBrain},
    runtime::replay::Recorder,
    Value,
};
fn input(name: &str) -> Value {
    let path = format!("{}/conformance/integration/{name}/input.json", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap(), 1 << 20).unwrap()
}
fn base() -> Brain {
    Brain::from_value(input("bindings-normalization").get("workflow").unwrap().get("definition").unwrap()).unwrap()
}
fn fact(turn: u64, key: &str, expires: Value) -> Value {
    obj! {"turn"=>turn,"entity"=>"actor_a","key"=>key,"text"=>"The actor approached cover.","tags"=>vec!["activity"],"importance"=>5,"source"=>"observed","expires"=>expires}
}
fn proposal() -> Value {
    obj! {"id"=>"seeking_shelter","statement"=>"The actor may be seeking shelter.","confidence"=>0.6,"supports"=>vec![1],"contradicts"=>vec![2],"ttl"=>100}
}
fn memory() -> EvidenceMemory {
    let mut m = EvidenceMemory::new("actor_a", "game_ticks", 8, 4).unwrap();
    m.observe(1, &fact(1, "cover", Value::from(30)), &["position"]).unwrap();
    m.observe(2, &fact(2, "motion", Value::from(30)), &[]).unwrap();
    m
}

#[test]
fn bindings_reject_changed_units_version_range_and_graph_order() {
    let schema = FeatureSchema::parse(include_str!("../examples/fixtures/brain/bindings.json")).unwrap();
    let mut bound = BoundBrain::new(base(), schema.clone()).unwrap();
    let saved = bound.to_value().unwrap();
    assert_eq!(BoundBrain::restore(&saved, &schema).unwrap().to_value().unwrap(), saved);
    for (field, value) in [("unit", Value::str("centimeters")), ("max", Value::from(100))] {
        let mut changed = schema.to_value();
        if let Value::Object(root) = &mut changed {
            let features = &mut root.iter_mut().find(|(k, _)| k == "features").unwrap().1;
            if let Value::Array(features) = features {
                if let Value::Object(row) = &mut features[0] {
                    row.iter_mut().find(|(k, _)| k == field).unwrap().1 = value;
                }
            }
        }
        assert!(BoundBrain::restore(&saved, &FeatureSchema::from_value(&changed).unwrap()).is_err());
    }
    let normalized = schema.normalize(&obj! {"y"=>0.5,"x"=>2}).unwrap();
    assert_eq!(normalized.as_slice(), [Some(800000), Some(750000)]);
    assert!(schema.normalize(&obj! {"x"=>2}).is_err());
    assert_eq!(schema.normalize(&obj! {"x"=>11,"y"=>2}).unwrap().as_slice(), [Some(0), None]);
    let mut prediction = Prediction::default();
    bound.run_into(&obj! {"x"=>2,"y"=>0}, 3, 0, 0, &mut prediction).unwrap();
    let before = prediction.clone();
    *bound.brain_mut() = Brain::dense(&["y", "x"], &["a", "b"], 0, 3).unwrap();
    assert!(bound.run_into(&obj! {"x"=>2,"y"=>0}, 3, 0, 0, &mut prediction).is_err());
    assert_eq!(prediction, before);
}

#[test]
fn stability_respects_unknowns_eligibility_urgency_and_failed_call_atomicity() {
    let mut stable = StableBrain::new(base(), 40_000, 10).unwrap();
    let mut out = Selection::default();
    let policy = SelectionPolicy { allowed: 3, min_score: 0, min_margin: 0, urgent: false };
    stable.run_into(0, &[Some(600000), Some(400000)], policy, &mut out).unwrap();
    assert_eq!(out.choice(), Some(0));
    stable.run_into(1, &[Some(490000), Some(510000)], policy, &mut out).unwrap();
    assert!(out.held());
    assert_eq!(out.prediction().choice(), Some(1));
    assert_eq!(out.choice(), Some(0));
    let before = out.choice();
    assert!(stable.run_into(0, &[Some(0), None], policy, &mut out).is_err());
    assert_eq!(out.choice(), before);
    stable.run_into(2, &[Some(900000), Some(100000)], SelectionPolicy { allowed: 2, ..policy }, &mut out).unwrap();
    assert_eq!(out.choice(), Some(1));
    assert!(!out.held());
    stable.run_into(3, &[Some(900000), Some(100000)], SelectionPolicy { urgent: true, ..policy }, &mut out).unwrap();
    assert_eq!(out.choice(), Some(0));
    stable.run_into(4, &[None, Some(500000)], policy, &mut out).unwrap();
    assert_eq!(out.choice(), None);
    stable.brain_mut();
    stable.run_into(5, &[Some(490000), Some(510000)], policy, &mut out).unwrap();
    assert_eq!(out.choice(), Some(1));
}

#[test]
fn diagnostic_conditions_and_replay_capture_preserve_state_and_bounds() {
    let trace = input("replay-labeled-and-latency").get("trace").unwrap().clone();
    let mut recorder = Recorder::new(&base(), 7).unwrap();
    for event in trace.get("events").unwrap().as_array().unwrap() {
        recorder.record(event).unwrap();
    }
    assert_eq!(recorder.to_value(), trace);
    let replayed = gamectx::runtime::replay::run(&trace).unwrap();
    let stats = replayed.get("summary").unwrap();
    assert_eq!(stats.get("p95_ns"), Some(&Value::from(160)));
    assert_eq!(stats.get("correct"), Some(&Value::from(7)));
    let saved = recorder.to_value();
    assert!(recorder.record(&trace.get("events").unwrap().as_array().unwrap()[0]).is_err());
    assert_eq!(recorder.to_value(), saved);
    let reports = replayed.get("reports").unwrap().as_array().unwrap();
    let reasons: Vec<_> = reports.iter().map(|r| r.get("reason").unwrap().as_str().unwrap()).collect();
    assert_eq!(
        reasons,
        ["chosen", "unknown_eligible", "all_zero", "tie", "no_eligible", "score_threshold", "margin_threshold"]
    );
}

#[test]
fn journal_proposals_do_not_hide_observations_and_expiry_never_revives_old_facts() {
    let mut journal = Journal::new(8).unwrap();
    journal.record(&fact(1, "cover", Value::Null)).unwrap();
    let args = obj! {"entity"=>"actor_a","key"=>"cover","text"=>"A proposed change.","tags"=>Vec::<Value>::new(),"importance"=>10,"ttl"=>100};
    journal.apply_proposal(2, &args).unwrap();
    assert_eq!(journal.recall(2, "cover", 8, true).unwrap().len(), 1);
    assert_eq!(journal.recall(2, "cover", 8, true).unwrap()[0].source, "observed");
    journal.record(&fact(3, "cover", Value::from(5))).unwrap();
    assert!(journal.recall(5, "cover", 8, false).unwrap().is_empty());
    let saved = journal.to_value();
    let mut bad = args.as_object().unwrap().to_vec();
    bad.push(("source".into(), Value::str("observed")));
    assert!(journal.apply_proposal(5, &Value::Object(bad)).is_err());
    assert_eq!(journal.to_value(), saved);
    let mut restored = Journal::restore_json(&saved.to_json()).unwrap();
    restored.forget(3);
    assert!(restored.recall(5, "cover", 8, false).unwrap().is_empty());
}

#[test]
fn evidence_remains_native_and_repeated_guesses_cannot_renew_or_raise_confidence() {
    let mut m = memory();
    let view = m.snapshot(2, 16, 8000, &[]).unwrap();
    m.propose(&view, 2, &proposal(), &[]).unwrap();
    let saved = m.to_value();
    assert_eq!(saved.get("journal").unwrap().get("records").unwrap().as_array().unwrap().len(), 2);
    let h = &saved.get("hypotheses").unwrap().as_array().unwrap()[0];
    assert_eq!(h.get("expires"), Some(&Value::from(30)));
    let fresh = m.snapshot(3, 16, 8000, &[]).unwrap();
    let mut request = proposal().as_object().unwrap().to_vec();
    request.iter_mut().find(|(k, _)| k == "confidence").unwrap().1 = Value::from(0.9);
    request.iter_mut().find(|(k, _)| k == "ttl").unwrap().1 = Value::from(1000);
    m.propose(&fresh, 3, &Value::Object(request), &[]).unwrap();
    let after = m.to_value();
    let later = &after.get("hypotheses").unwrap().as_array().unwrap()[0];
    assert_eq!(later.get("confidence"), Some(&Value::Number("0.6".into())));
    assert_eq!(later.get("expires"), Some(&Value::from(30)));
    assert_eq!(later.get("created"), Some(&Value::from(2)));
    let fresh = m.snapshot(4, 16, 8000, &[]).unwrap();
    let mut changed = proposal().as_object().unwrap().to_vec();
    changed.iter_mut().find(|(k, _)| k == "statement").unwrap().1 = Value::str("A different claim.");
    let before = m.to_value();
    assert!(m.propose(&fresh, 4, &Value::Object(changed), &[]).is_err());
    assert_eq!(m.to_value(), before);
}

#[test]
fn evidence_snapshot_scope_revision_privacy_and_retention_are_rechecked() {
    let mut m = memory();
    let view = m.snapshot(2, 16, 8000, &[]).unwrap();
    let before = m.to_value();
    assert!(m.propose(&view, 2, &proposal(), &["position"]).is_err());
    assert_eq!(m.to_value(), before);
    let mut restored = EvidenceMemory::restore(&before, "actor_a", "game_ticks").unwrap();
    assert!(restored.propose(&view, 2, &proposal(), &[]).is_err());
    assert!(EvidenceMemory::restore(&before, "other_actor", "game_ticks").is_err());
    assert!(EvidenceMemory::restore(&before, "actor_a", "wall_clock").is_err());
    assert!(m.observe(2, &fact(2, "motion", Value::Null), &[]).is_err());
    assert_eq!(m.to_value(), before);
    m.propose(&view, 2, &proposal(), &[]).unwrap();
    assert!(m.propose(&view, 3, &proposal(), &[]).is_err());
    assert!(m
        .snapshot(30, 16, 8000, &[])
        .unwrap()
        .to_value()
        .get("hypotheses")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    let hidden = m.snapshot(3, 16, 8000, &["position"]).unwrap().to_value();
    assert!(hidden.get("hypotheses").unwrap().as_array().unwrap().is_empty());
    assert_eq!(hidden.get("citable").unwrap().as_array().unwrap().len(), 1);
    m.forget_privacy(&["position"]).unwrap();
    let saved = m.to_value();
    assert!(saved.get("hypotheses").unwrap().as_array().unwrap().is_empty());
    assert_eq!(saved.get("journal").unwrap().get("records").unwrap().as_array().unwrap().len(), 1);
}

#[test]
fn evidence_budget_truncation_keeps_only_supplied_citations_and_dependencies() {
    let mut m = memory();
    let small = m.snapshot(2, 1, 8000, &[]).unwrap();
    let before = m.to_value();
    assert!(m.propose(&small, 2, &proposal(), &[]).is_err());
    assert_eq!(m.to_value(), before);
    let full = m.snapshot(2, 16, 8000, &[]).unwrap();
    m.propose(&full, 2, &proposal(), &[]).unwrap();
    for cap in [128, 256, 512, 1024] {
        if let Ok(view) = m.snapshot(3, 16, cap, &[]) {
            let value = view.to_value();
            assert!(value.json_len() <= cap);
            let ids: Vec<_> = value
                .get("evidence")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.get("record").unwrap().get("id").unwrap().clone())
                .collect();
            for c in value.get("citable").unwrap().as_array().unwrap() {
                assert!(ids.contains(c.get("id").unwrap()));
            }
            for h in value.get("hypotheses").unwrap().as_array().unwrap() {
                for key in ["supports", "contradicts"] {
                    for id in h.get(key).unwrap().as_array().unwrap() {
                        assert!(ids.contains(id));
                    }
                }
            }
        }
    }
    m.contradict(1, 3).unwrap();
    assert!(m.snapshot(3, 16, 8000, &[]).unwrap().to_value().get("hypotheses").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn saved_evidence_rejects_forged_counters_privacy_and_source_lifetime() {
    let mut memory = memory();
    let view = memory.snapshot(2, 16, 8000, &[]).unwrap();
    memory.propose(&view, 2, &proposal(), &[]).unwrap();
    let saved = memory.to_value();
    for field in ["revision", "last_token"] {
        let mut bad = saved.clone();
        if let Value::Object(fields) = &mut bad {
            fields.iter_mut().find(|(k, _)| k == field).unwrap().1 = Value::from(0);
        }
        assert!(EvidenceMemory::restore(&bad, "actor_a", "game_ticks").is_err());
    }
    for (field, value) in [("privacy", Value::Array(vec![])), ("expires", Value::from(40)), ("created", Value::from(0))]
    {
        let mut bad = saved.clone();
        if let Value::Object(root) = &mut bad {
            let h = &mut root.iter_mut().find(|(k, _)| k == "hypotheses").unwrap().1;
            if let Value::Array(h) = h {
                if let Value::Object(fields) = &mut h[0] {
                    fields.iter_mut().find(|(k, _)| k == field).unwrap().1 = value;
                }
            }
        }
        assert!(EvidenceMemory::restore(&bad, "actor_a", "game_ticks").is_err());
    }
    assert_eq!(memory.to_value(), saved);
}

#[test]
fn a_new_observation_can_replace_an_invalidated_hypothesis_basis() {
    let mut memory = memory();
    let view = memory.snapshot(2, 16, 8000, &[]).unwrap();
    memory.propose(&view, 2, &proposal(), &[]).unwrap();
    memory.observe(3, &fact(3, "cover", Value::from(30)), &["position"]).unwrap();
    let view = memory.snapshot(3, 16, 8000, &[]).unwrap();
    assert!(view.to_value().get("hypotheses").unwrap().as_array().unwrap().is_empty());
    let request = obj! {"id"=>"seeking_shelter","statement"=>"The actor may be seeking shelter.","confidence"=>0.7,"supports"=>vec![3],"contradicts"=>vec![2],"ttl"=>10};
    memory.propose(&view, 3, &request, &[]).unwrap();
    let next = memory.snapshot(3, 16, 8000, &[]).unwrap().to_value();
    let h = &next.get("hypotheses").unwrap().as_array().unwrap()[0];
    assert_eq!(h.get("created"), Some(&Value::from(3)));
    assert_eq!(h.get("expires"), Some(&Value::from(13)));
}
