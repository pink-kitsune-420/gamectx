use gamectx::{
    data::json::parse,
    obj,
    runtime::brain::{self, Brain},
    Value,
};

fn sample(x: f64, target: f64) -> Value {
    obj! {"inputs" => obj! {"x" => x}, "targets" => obj! {"act" => target}}
}

fn definition() -> Value {
    obj! {"version" => 1, "inputs" => vec!["x"], "outputs" => vec!["act"],
    "nodes" => vec![obj! {"id" => "act", "weights" => obj! {"x" => 0.5},
        "bias" => 0.1, "pinned" => false, "learning_limit" => 0.5}]}
}

fn lesson(delta: f64, id: &str, revision: usize) -> Value {
    obj! {"id" => id, "expected_revision" => revision,
    "changes" => vec![obj! {"node" => "act", "source" => "$bias", "delta" => delta}],
    "validation" => vec![sample(0.8, 0.7)]}
}

#[test]
fn lessons_validate_before_mutation_and_keep_replay_guards_after_undo() {
    let mut brain = Brain::from_value(&definition()).unwrap();
    let original = brain.to_value();
    let rejected = brain.teach(&lesson(-0.1, "bad", 0)).unwrap();
    assert_eq!(rejected.get("accepted"), Some(&Value::Bool(false)));
    assert_eq!(brain.to_value(), original);
    assert!(brain.teach(&lesson(0.1, "stale", 1)).is_err());
    assert_eq!(brain.to_value(), original);
    let accepted = brain.teach(&lesson(0.1, "good", 0)).unwrap();
    assert_eq!(accepted.get("accepted"), Some(&Value::Bool(true)));
    let saved = brain.to_value();
    assert_eq!(Brain::restore_json(&saved.to_json()).unwrap().to_value(), saved);
    brain.undo(1).unwrap();
    assert_eq!(
        brain.run(&obj! {"x" => 0.8}, &["act"]).unwrap().get("scores"),
        original_score(&original, 0.8).get("scores")
    );
    assert!(brain.teach(&lesson(0.1, "good", 2)).is_err());
    assert_eq!(brain.revision(), 2);
    assert_eq!(Brain::restore(&brain.to_value()).unwrap().to_value(), brain.to_value());
}

fn original_score(saved: &Value, x: f64) -> Value {
    Brain::restore(saved).unwrap().run(&obj! {"x" => x}, &["act"]).unwrap()
}

#[test]
fn training_rejects_leakage_bad_shapes_and_excess_work_atomically() {
    let mut brain = Brain::dense(&["x", "y"], &["act"], 4, 3).unwrap();
    let original = brain.to_value();
    let row = obj! {"inputs" => obj! {"x" => 0.2, "y" => 0.3}, "targets" => obj! {"act" => 0.7}};
    let mut request = obj! {"id" => "fit", "expected_revision" => 0, "examples" => vec![row.clone()],
    "validation" => vec![row.clone()], "epochs" => 128, "rate" => 0.1};
    assert!(brain.train(&request).is_err());
    assert_eq!(brain.to_value(), original);
    let entries = match &mut request {
        Value::Object(entries) => entries,
        _ => unreachable!(),
    };
    entries.iter_mut().find(|(key, _)| key == "examples").unwrap().1 = Value::Array(vec![row; 128]);
    entries.iter_mut().find(|(key, _)| key == "validation").unwrap().1 =
        Value::Array(vec![obj! {"inputs" => obj! {"x" => 0.4, "y" => 0.5}, "targets" => obj! {"act" => 0.8}}]);
    assert!(brain.train(&request).unwrap_err().to_string().contains("work"));
    assert_eq!(brain.to_value(), original);
}

#[test]
fn unknown_inputs_and_host_eligibility_are_separate_from_policy_scores() {
    let brain = Brain::from_value(&definition()).unwrap();
    let unknown = brain.run(&obj! {}, &["act"]).unwrap();
    assert_eq!(unknown.get("choice"), Some(&Value::Null));
    assert_eq!(unknown.get("scores").unwrap().get("act"), Some(&Value::Null));
    let zero = brain.run(&obj! {"x" => 0}, &["act"]).unwrap();
    assert_eq!(zero.get("choice").and_then(Value::as_str), Some("act"));
    let masked = brain.run(&obj! {"x" => 1}, &[]).unwrap();
    assert_eq!(masked.get("choice"), Some(&Value::Null));
    assert_eq!(masked.get("scores").unwrap().get("act").unwrap().to_json(), "0.6");
    assert!(brain.run(&obj! {"x" => 1}, &["unknown"]).is_err());
    assert!(brain.run(&obj! {"x" => 1.01}, &["act"]).is_err());
    assert!(brain.run_with(&obj! {"x" => 1}, &["act"], f64::NAN, 0.0).is_err());
    assert_eq!(brain.run_with(&obj! {"x" => 1}, &["act"], 0.6, 0.0).unwrap().get("choice"), Some(&Value::Null));
}

#[test]
fn pins_and_saved_state_are_revalidated() {
    let mut def = definition();
    if let Value::Object(root) = &mut def {
        let nodes = &mut root.iter_mut().find(|(k, _)| k == "nodes").unwrap().1;
        if let Value::Array(nodes) = nodes {
            if let Value::Object(node) = &mut nodes[0] {
                node.iter_mut().find(|(k, _)| k == "pinned").unwrap().1 = Value::Bool(true);
            }
        }
    }
    let mut brain = Brain::from_value(&def).unwrap();
    let original = brain.to_value();
    assert!(brain.teach(&lesson(0.1, "pin", 0)).is_err());
    assert_eq!(brain.to_value(), original);
    let mut forged = original;
    if let Value::Object(entries) = &mut forged {
        entries.iter_mut().find(|(k, _)| k == "weights").unwrap().1 =
            Value::Array(vec![Value::Array(vec![Value::from(0.2), Value::from(0.5)])]);
    }
    assert!(Brain::restore(&forged).is_err());
}

#[test]
fn tool_requests_are_strict_bounded_and_pack_as_signatures() {
    let tools = brain::tools();
    assert!(gamectx::packet::signatures::pack(tools.as_array().unwrap()).is_ok());
    let mut brain = brain::create_call(&obj! {"request" => definition().to_json()}).unwrap();
    let request = obj! {"inputs" => obj! {"x" => 0.8}, "allowed" => vec!["act"], "min_score" => 0, "min_margin" => 0};
    assert_eq!(
        brain
            .apply_call("run_brain", &obj! {"request" => request.to_json()})
            .unwrap()
            .get("choice")
            .and_then(Value::as_str),
        Some("act")
    );
    assert!(brain.apply_call("create_brain", &obj! {"request" => definition().to_json()}).is_err());
    assert!(brain
        .apply_call("teach_brain", &obj! {"request" => lesson(0.1,"a",0).to_json(), "extra" => true})
        .is_err());
    assert!(brain.apply_call("inspect_brain", &obj! {"request" => " ".repeat(brain::MAX_REQUEST_BYTES + 1)}).is_err());
    assert!(Brain::parse(&"[".repeat(100_000)).is_err());
    assert!(Brain::restore_json(&"[".repeat(100_000)).is_err());
    assert!(Brain::from_value(&parse(r#"{"version":1,"inputs":["x","x"],"outputs":["a"],"nodes":[]}"#, 1024).unwrap())
        .is_err());
}

#[test]
fn saturated_outputs_can_unlearn_a_previous_preference() {
    let mut brain = Brain::from_value(&obj! {"version" => 1, "inputs" => vec!["x"],
    "outputs" => vec!["act"], "nodes" => vec![obj! {"id" => "act", "weights" => obj! {},
        "bias" => 4, "pinned" => false, "learning_limit" => 4}]})
    .unwrap();
    let report = brain
        .train(&obj! {"id" => "recover", "expected_revision" => 0,
        "examples" => vec![sample(0.2, 0.2)], "validation" => vec![sample(0.8, 0.2)],
        "epochs" => 128, "rate" => 0.1})
        .unwrap();
    assert_eq!(report.get("accepted"), Some(&Value::Bool(true)));
    assert_eq!(report.get("after_loss").unwrap().to_json(), "0");
    assert_eq!(Brain::restore(&brain.to_value()).unwrap().to_value(), brain.to_value());
}

#[test]
fn accepted_update_limit_survives_undo_and_restore() {
    let mut brain = Brain::from_value(&definition()).unwrap();
    for index in 0..brain::MAX_UPDATES {
        let report = brain.teach(&lesson(0.001, &format!("step_{index}"), index)).unwrap();
        assert_eq!(report.get("accepted"), Some(&Value::Bool(true)));
    }
    let saved = brain.to_value();
    assert!(brain.teach(&lesson(0.001, "extra", brain::MAX_UPDATES)).is_err());
    assert_eq!(brain.to_value(), saved);
    brain.undo(brain::MAX_UPDATES).unwrap();
    let mut restored = Brain::restore_json(&brain.to_value().to_json()).unwrap();
    assert!(restored.teach(&lesson(0.001, "extra", restored.revision())).is_err());
}

#[test]
fn rate_selection_preserves_correct_decisions_even_when_mse_falls() {
    let request =
        parse(include_str!("../conformance/brain-select/reject-decision-regression/input.json"), 8192).unwrap();
    let mut brain = Brain::from_value(request.get("definition").unwrap()).unwrap();
    let before = brain.to_value();
    let args = request.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap();
    let report = brain.train_select(args).unwrap();
    assert_eq!(report.get("accepted"), Some(&Value::Bool(false)));
    assert_eq!(report.get("before_correct").and_then(Value::as_usize), Some(1));
    assert_eq!(report.get("trials").unwrap().as_array().unwrap()[0].get("correct").and_then(Value::as_usize), Some(0));
    assert_eq!(brain.to_value(), before);
}

#[test]
fn rate_selection_commits_one_snapshot_and_preflights_total_budget() {
    let request = parse(include_str!("../conformance/brain-select/select-earlier-tie/input.json"), 8192).unwrap();
    let mut brain = Brain::from_value(request.get("definition").unwrap()).unwrap();
    let before = brain.to_value();
    let args = request.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap();
    let report = brain.apply_call("train_select_brain", &obj! {"request" => args.to_json()}).unwrap();
    assert_eq!(report.get("selected_rate").unwrap().to_json(), "0.5");
    assert_eq!(brain.revision(), 1);
    assert_eq!(brain.to_value().get("history").unwrap().as_array().unwrap().len(), 1);
    brain.undo(1).unwrap();
    assert_eq!(brain.to_value().get("weights"), before.get("weights"));
    assert!(brain.train_select(args).is_err());
    let request = parse(include_str!("../conformance/brain-select/error-aggregate-budget/input.json"), 8192).unwrap();
    let mut brain = Brain::from_value(request.get("definition").unwrap()).unwrap();
    let before = brain.to_value();
    let args = request.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap();
    assert!(brain.train_select(args).unwrap_err().to_string().contains("work"));
    assert_eq!(brain.to_value(), before);
}
