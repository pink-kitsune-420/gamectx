use gamectx::{
    data::json::parse,
    obj,
    packet::sync::{Publisher, Replica},
    runtime::brain::{Brain, Prediction},
    runtime::schedule::Scheduler,
    Value,
};
#[path = "support/runtime.rs"]
mod runtime;

#[test]
fn native_prediction_bounds_and_unknowns_preserve_output_on_error() {
    let brain = Brain::dense(&["x", "y"], &["a", "b"], 4, 3).unwrap();
    let mut out = Prediction::default();
    brain.run_into(&[Some(500_000), None], 3, 0, 0, &mut out).unwrap();
    assert_eq!(out.choice(), None);
    let before = out.clone();
    for (inputs, mask, low, gap) in [
        (vec![Some(0)], 3, 0, 0),
        (vec![Some(-1), None], 3, 0, 0),
        (vec![None, None], 4, 0, 0),
        (vec![None, None], 3, 0, 1_000_001),
    ] {
        assert!(brain.run_into(&inputs, mask, low, gap, &mut out).is_err());
        assert_eq!(out, before);
    }
    let inputs: Vec<_> = (0..16).map(|i| format!("i{i}")).collect();
    let outputs: Vec<_> = (0..16).map(|i| format!("o{i}")).collect();
    let max = Brain::dense(
        &inputs.iter().map(String::as_str).collect::<Vec<_>>(),
        &outputs.iter().map(String::as_str).collect::<Vec<_>>(),
        16,
        3,
    )
    .unwrap();
    max.run_into(&[Some(500_000); 16], u16::MAX, 0, 0, &mut out).unwrap();
    assert_eq!(out.scores().len(), 16);
    brain.run_into(&[Some(250_000), Some(750_000)], 3, 0, 0, &mut out).unwrap();
    let mut fresh = Prediction::default();
    brain.run_into(&[Some(250_000), Some(750_000)], 3, 0, 0, &mut fresh).unwrap();
    assert_eq!(out, fresh, "reuse across graph sizes cannot retain hidden scores");
}

#[test]
fn sliced_jobs_match_direct_training_and_selection_at_different_boundaries() {
    for text in [
        include_str!("../conformance/brain/train/input.json"),
        include_str!("../conformance/brain/saturated-output-recovery/input.json"),
        include_str!("../conformance/brain-select/select-earlier-tie/input.json"),
        include_str!("../conformance/brain-select/reject-decision-regression/input.json"),
    ] {
        let workflow = parse(text, 262144).unwrap();
        let mut empty = workflow.as_object().unwrap().to_vec();
        empty.retain(|(k, _)| k != "operations");
        empty.push(("operations".into(), Value::Array(vec![])));
        let state = gamectx::runtime::brain::process(&Value::Object(empty)).unwrap();
        let brain = Brain::restore(state.get("state").unwrap()).unwrap();
        let operation = &workflow.get("operations").unwrap().as_array().unwrap()[0];
        let args = operation.get("args").unwrap();
        let selecting = operation.get("op").unwrap().as_str() == Some("train_select");
        let mut direct = brain.clone();
        let report = if selecting { direct.train_select(args) } else { direct.train(args) }.unwrap();
        for factor in [1, 7, 101] {
            let mut job = if selecting { brain.start_train_select(args) } else { brain.start_train(args) }.unwrap();
            assert_eq!(job.completed_steps(), 0);
            let saved = brain.to_value();
            while !job.is_finished() {
                let cap = (job.next_steps() * factor).min(2_000_000);
                let spent = job.advance(cap).unwrap();
                assert!(spent > 0 && spent <= cap);
                assert_eq!(brain.to_value(), saved);
            }
            assert_eq!(job.completed_steps(), job.total_steps());
            let mut target = brain.clone();
            assert_eq!(job.finish(&mut target).unwrap(), report);
            assert_eq!(target.to_value(), direct.to_value());
        }
    }
}

#[test]
fn unfinished_cancelled_and_stale_jobs_never_mutate_live_learning() {
    let workflow = parse(include_str!("../conformance/brain-select/select-earlier-tie/input.json"), 262144).unwrap();
    let mut brain = Brain::from_value(workflow.get("definition").unwrap()).unwrap();
    let args = workflow.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap();
    let before = brain.to_value();
    let mut short = brain.start_train_select(args).unwrap();
    assert!(short.advance(1).is_err());
    assert_eq!(short.completed_steps(), 0);
    assert!(short.finish(&mut brain).is_err());
    assert_eq!(brain.to_value(), before);
    {
        let mut cancelled = brain.start_train_select(args).unwrap();
        cancelled.advance(4096).unwrap();
    }
    assert_eq!(brain.to_value(), before);
    let mut stale = brain.start_train_select(args).unwrap();
    while !stale.is_finished() {
        stale.advance(4096).unwrap();
    }
    let mut other = Brain::dense(&["x"], &["different"], 0, 91).unwrap();
    let other_before = other.to_value();
    assert_eq!(brain.revision(), other.revision());
    assert!(stale.finish(&mut other).is_err());
    assert_eq!(other.to_value(), other_before);
    let mut stale = brain.start_train_select(args).unwrap();
    while !stale.is_finished() {
        stale.advance(4096).unwrap();
    }
    brain.train_select(args).unwrap();
    let changed = brain.to_value();
    assert!(stale.finish(&mut brain).is_err());
    assert_eq!(brain.to_value(), changed);
}

#[test]
fn scheduler_repeated_poll_budget_urgent_refresh_and_failure_retry() {
    let mut schedule = Scheduler::new(4, 3).unwrap();
    schedule.register(1, 2, 10, 20, 0).unwrap();
    schedule.register(2, 2, 10, 20, 0).unwrap();
    assert_eq!(schedule.poll(0).unwrap(), [1]);
    assert_eq!(schedule.remaining(), 1);
    assert!(schedule.poll(0).unwrap().is_empty());
    assert_eq!(schedule.poll(1).unwrap(), [2]);
    schedule.changed(1, true).unwrap();
    assert_eq!(schedule.poll(2).unwrap(), [1]);
    assert!(schedule.poll(3).unwrap().is_empty());
    assert_eq!(schedule.poll(21).unwrap(), [2]);
    assert!(schedule.poll(20).is_err());
    schedule.changed(2, true).unwrap();
    assert_eq!(schedule.poll(22).unwrap(), [2]);
    assert!(schedule.remove(1));
    assert!(!schedule.remove(1));
    assert!(schedule.changed(1, false).is_err());
    let mut end = Scheduler::new(1, 1).unwrap();
    end.register(0, 1, 1, 2, 0).unwrap();
    assert_eq!(end.poll(u64::MAX).unwrap(), [0]);
    assert!(end.poll(u64::MAX).unwrap().is_empty());
}

#[test]
fn sync_atomic_failures_loss_reset_reordering_and_exact_number_text() {
    let mut pub1 = Publisher::new("stream").unwrap();
    let mut receiver = Replica::new("stream").unwrap();
    let first = parse(r#"{"n":1.00,"other":"stable"}"#, 1024).unwrap();
    let snapshot = pub1.publish(&first, None).unwrap();
    assert_eq!(receiver.apply(&snapshot).unwrap(), &first);
    let saved = receiver.state().unwrap().clone();
    let rev = receiver.revision();
    let malformed = [
        snapshot.clone(),
        obj! {"stream"=>"other","base"=>Value::Null,"revision"=>2,"order"=>vec!["n"],"set"=>obj!{"n"=>2},"remove"=>Vec::<Value>::new()},
        obj! {"stream"=>"stream","base"=>1,"revision"=>2,"order"=>vec!["n"],"set"=>obj!{},"remove"=>vec!["other","missing"]},
        obj! {"stream"=>"stream","base"=>1,"revision"=>2,"order"=>vec!["n","n"],"set"=>obj!{},"remove"=>Vec::<Value>::new()},
    ];
    for bad in malformed {
        assert!(receiver.apply(&bad).is_err());
        assert_eq!(receiver.state(), Some(&saved));
        assert_eq!(receiver.revision(), rev);
    }
    assert!(receiver.apply_json(r#"{"stream":"s","stream":"s"}"#).is_err());
    let second = parse(r#"{"other":"stable","n":1e0}"#, 1024).unwrap();
    let delta = pub1.publish(&second, Some(1)).unwrap();
    assert_eq!(delta.get("set").unwrap().get("n"), second.get("n"));
    assert_eq!(receiver.apply(&delta).unwrap().to_json(), second.to_json());
    pub1.publish(&obj! {"n"=>3}, Some(2)).unwrap(); // dropped message
    let reset = pub1.publish(&obj! {"n"=>4}, Some(2)).unwrap();
    assert_eq!(reset.get("base"), Some(&Value::Null));
    assert_eq!(receiver.apply(&reset).unwrap(), &obj! {"n"=>4});
    let before = pub1.revision();
    assert!(pub1.publish(&obj! {"large"=>"x".repeat(262144)}, Some(before)).is_err());
    assert_eq!(pub1.revision(), before);
    let before = receiver.state().cloned();
    assert!(receiver.apply_json(&"[".repeat(100_000)).is_err());
    assert_eq!(receiver.state().cloned(), before);
}

#[test]
fn host_empty_masks_expect_abstention_and_use_one_retained_revision() {
    let input = parse(include_str!("../conformance/runtime/host-empty-mask/input.json"), 262144).unwrap();
    let result = runtime::process(&input).unwrap();
    let report = &result.get("results").unwrap().as_array().unwrap()[0];
    assert_eq!(report.get("before_correct"), report.get("validation_count"));
    assert_eq!(report.get("after_correct"), report.get("validation_count"));
    assert_eq!(result.get("state").unwrap().get("revision"), Some(&Value::from(1)));
}

#[test]
fn host_target_ties_expect_abstention_while_default_selection_keeps_its_metric() {
    let definition = obj! {"version"=>1,"inputs"=>vec!["x"],"outputs"=>vec!["a","b"],"nodes"=>vec![
    obj!{"id"=>"a","weights"=>obj!{},"bias"=>0.6,"pinned"=>true,"learning_limit"=>0},
    obj!{"id"=>"b","weights"=>obj!{},"bias"=>0.4,"pinned"=>true,"learning_limit"=>0}]};
    let row = |x| obj! {"inputs"=>obj!{"x"=>x},"targets"=>obj!{"a"=>0.5,"b"=>0.5}};
    let validation = row(1);
    let base = obj! {"id"=>"ties","expected_revision"=>0,"examples"=>vec![row(0)],"validation"=>vec![validation.clone()],"epochs"=>1,"rates"=>vec![0.1],"max_steps"=>1000};
    let mut legacy = Brain::from_value(&definition).unwrap();
    let report = legacy.train_select(&base).unwrap();
    assert_eq!(report.get("before_correct"), Some(&Value::from(1)));
    let mut fields = base.as_object().unwrap().to_vec();
    fields.retain(|(key, _)| key != "validation");
    let mut host_row = validation.as_object().unwrap().to_vec();
    host_row.extend([
        ("allowed".into(), Value::strings(&["a", "b"])),
        ("min_score".into(), Value::from(0)),
        ("min_margin".into(), Value::from(0)),
    ]);
    fields.push(("validation".into(), Value::Array(vec![Value::Object(host_row)])));
    let mut host = Brain::from_value(&definition).unwrap();
    let before = host.to_value();
    let report = host.train_select_host(&Value::Object(fields)).unwrap();
    assert_eq!(report.get("before_correct"), Some(&Value::from(0)));
    assert_eq!(host.to_value(), before);
}

#[test]
fn arbitrary_packet_predicates_still_receive_complete_candidates() {
    use gamectx::packet::budget::{assemble, Section};
    let sections = vec![
        Section { name: "keep".into(), priority: 1, stable: true, required: true, value: Value::str("x".repeat(256)) },
        Section { name: "drop".into(), priority: 0, stable: false, required: false, value: Value::from(1) },
    ];
    let calls = std::cell::RefCell::new(Vec::new());
    let packet = assemble(sections, &|text| {
        calls.borrow_mut().push(text.to_owned());
        text.contains("keep") && !text.contains("drop")
    })
    .unwrap();
    assert_eq!(calls.borrow().len(), 2);
    assert!(calls.borrow()[0].contains("drop"));
    assert_eq!(calls.borrow()[1], packet.text);
    assert_eq!(packet.value.get("keep"), Some(&Value::str("x".repeat(256))));
    assert_eq!(packet.receipt.dropped[0].name, "drop");
    assert_eq!(packet.receipt.total_bytes, packet.text.len());
}
