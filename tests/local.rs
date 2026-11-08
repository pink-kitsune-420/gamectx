use gamectx::{
    character::dialogue,
    obj,
    packet::request::{Count, CountMethod, RequestBudget},
    Result, Value,
};
fn count(v: &Value) -> Result<Count> {
    Ok(Count { input: v.to_json().len(), method: CountMethod::Estimate })
}
fn fixture() -> Value {
    gamectx::data::json::parse(include_str!("../conformance/kit/build/input.json"), 1 << 20).unwrap()
}
#[test]
fn full_request_counts_native_tools_history_and_output() -> Result<()> {
    let mut game = gamectx::Context::new();
    game.prompt("rules", "Choose safely.")?;
    let mut turn = game.turn();
    turn.var_with("lore", "x".repeat(1000), 1, false)?;
    turn.var_with("current", "Inspect the gate", 9, true)?;
    let budget = RequestBudget::new(600, 80, 20)?;
    let built=turn.build_request(&budget,|packet|Ok(obj! {"messages"=>vec![obj! {"role"=>"system","content"=>packet.to_json()}],"tools"=>vec![gamectx::data::query::tool()]}),count);
    // Native tool descriptions alone consume this deliberately tiny fixture byte budget.
    assert!(built.is_err());
    let built=turn.build_request(&RequestBudget::new(1800,80,20)?,|packet|Ok(obj! {"messages"=>vec![obj! {"role"=>"system","content"=>packet.to_json()}],"tools"=>vec![gamectx::data::query::tool()]}),count)?;
    assert!(built.receipt.fits());
    assert!(built.packet.value.get("current").is_some());
    assert!(built.request.get("tools").is_some());
    assert_eq!(built.receipt.method, CountMethod::Estimate);
    let errors = std::cell::Cell::new(0);
    let bad = turn.build_request(
        &budget,
        |v| Ok(v.clone()),
        |_| {
            errors.set(errors.get() + 1);
            Err(gamectx::Error::new("tokenizer unavailable"))
        },
    );
    assert!(bad.unwrap_err().to_string().contains("tokenizer unavailable"));
    assert_eq!(errors.get(), 1);
    Ok(())
}
#[test]
fn decision_requires_live_permission_and_never_executes() -> Result<()> {
    let choices = gamectx::runtime::decision::Choices::from_value(&obj! {"version"=>1,"revision"=>9,
    "choices"=>vec![obj! {"action"=>"inspect","target"=>"grove:gate"},obj! {"action"=>"wait","target"=>Value::Null}],
    "fallback"=>obj! {"action"=>"wait","target"=>Value::Null}})?;
    let mut s = choices.session();
    let calls = std::cell::Cell::new(0);
    let rejected = s.apply(r#"{"action":"inspect","target":"grove:gate"}"#, 9, |_| {
        calls.set(calls.get() + 1);
        false
    })?;
    assert_eq!(rejected.get("status"), Some(&Value::str("retry")));
    assert_eq!(calls.get(), 1);
    let fallback = s.apply("{", 10, |p| p.get("action") == Some(&Value::str("wait")))?;
    assert_eq!(fallback.get("status"), Some(&Value::str("fallback")));
    assert!(s.apply("{}", 10, |_| true).is_err());
    Ok(())
}
#[test]
fn compaction_preserves_tail_and_returns_evidence_candidates_only() -> Result<()> {
    let t = obj! {"version"=>1,"scene"=>"scene","conversation"=>"conversation","participants"=>vec!["player","warden"],"exchanges"=>(0..8).map(|i|obj! {"id"=>format!("e{i}"),"turn"=>i,"user"=>"question","assistant"=>"reply"}).collect::<Vec<_>>()};
    let thread = dialogue::Thread::from_value(&t)?;
    let window = thread.compaction_window("e7", 2, count)?;
    let ids = window.as_array().unwrap().iter().map(|x| x.get("id").unwrap().as_str().unwrap()).collect::<Vec<_>>();
    assert_eq!(ids, vec!["e0", "e1", "e2"]);
    assert_eq!(thread.to_value(), t);
    let empty = thread.select("e7", 0, 0, 0, count)?;
    assert_eq!(empty.messages, Value::Array(Vec::new()));
    Ok(())
}
#[test]
fn previous_registration_still_builds_identically() -> Result<()> {
    let v = fixture();
    let p = gamectx::kit::Project::from_value(v.get("project").unwrap())?;
    let s = p.snapshot(v.get("args").unwrap().get("snapshot").unwrap())?;
    assert_eq!(p.build("conversation", &s)?.packet.receipt.total_bytes, 646);
    Ok(())
}

#[test]
fn query_stdin_supports_local_hydration_without_temp_files() {
    use std::io::Write;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_gamectx"))
        .args(["query", "-", "SELECT id FROM offers WHERE price <= 8 ORDER BY units DESC LIMIT 1"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"offers":[{"id":"grove:a","price":9,"units":10},{"id":"grove:b","price":8,"units":7}]}"#)
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"[\"grove:b\"]\n");
}

#[test]
fn size_templates_are_advisory_and_do_not_change_profile_formats() -> Result<()> {
    use gamectx::deploy::deployment::{Identity, ModelSize, Profile};
    let examples = [
        include_str!("../examples/fixtures/deployments/1b.json"),
        include_str!("../examples/fixtures/deployments/1.5b.json"),
        include_str!("../examples/fixtures/deployments/3b.json"),
        include_str!("../examples/fixtures/deployments/8b.json"),
        include_str!("../examples/fixtures/deployments/30b.json"),
        include_str!("../examples/fixtures/deployments/70b.json"),
    ];
    for text in examples {
        let value = gamectx::data::json::parse(text, 8192)?;
        let profile = value.get("profile").unwrap();
        let identity = Identity::from_value(profile.get("identity").unwrap())?;
        let size = ModelSize::parse(value.get("guideline").unwrap().get("size_class").unwrap().as_str().unwrap())?;
        assert_eq!(size.starting_profile(profile.get("name").unwrap().as_str().unwrap(), &identity)?, value);
        let parsed = Profile::from_value(profile)?;
        parsed.require_identity(&identity)?;
        assert_eq!(value.get("guideline").unwrap().get("measured"), Some(&Value::Bool(false)));
        assert_eq!(parsed.guidance(), "");
    }
    assert!(ModelSize::parse("unknown").is_err());
    assert_eq!(ModelSize::parse("1.5")?.guideline().get("size_class"), Some(&Value::str("1.5b")));
    Ok(())
}
