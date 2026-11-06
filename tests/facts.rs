use gamectx::{
    facts::{Authority, ClaimStatus, FactOptions, FactRegistry},
    json::parse,
    obj, Context, Value,
};

#[test]
fn typed_claims_compare_decimal_values_without_floating_point_aliases() {
    let mut facts = FactRegistry::new(16).unwrap();
    let value = parse(r#"{"n":9007199254740993,"ratio":1.00e0,"zero":-0.0,"sequence":[1,2]}"#, 4096).unwrap();
    facts.register("world", "numeric", value.clone(), FactOptions::default()).unwrap();
    let same = parse(r#"{"sequence":[1,2],"zero":0,"ratio":10e-1,"n":9007199254740993}"#, 4096).unwrap();
    assert_eq!(facts.check("world", "numeric", &same, 0, &[]).unwrap().status, ClaimStatus::Supported);
    let different = parse(r#"{"n":9007199254740992,"ratio":1,"zero":0,"sequence":[1,2]}"#, 4096).unwrap();
    assert_eq!(facts.check("world", "numeric", &different, 0, &[]).unwrap().status, ClaimStatus::Conflicting);
    assert_eq!(facts.check("world", "missing", &Value::Bool(false), 0, &[]).unwrap().status, ClaimStatus::Unknown);
    assert_eq!(facts.get("world", "numeric", 0, &[]).unwrap().unwrap().value, value);
    let saved = facts.to_value();
    assert_eq!(FactRegistry::restore_json(&saved.to_json()).unwrap().to_value(), saved);
}

#[test]
fn pins_user_precedence_and_stale_updates_require_explicit_host_actions() {
    let mut facts = FactRegistry::new(8).unwrap();
    let pinned = facts
        .register(
            "rules",
            "quiet",
            true,
            FactOptions { authority: Authority::User, pinned: true, ..Default::default() },
        )
        .unwrap();
    let before = facts.to_value();
    assert!(facts
        .register(
            "rules",
            "quiet",
            false,
            FactOptions { expected_revision: Some(pinned.revision), ..Default::default() }
        )
        .is_err());
    assert!(facts.unpin(&pinned, Authority::Host).is_err());
    assert!(facts.forget(&pinned, Authority::User).is_err());
    assert_eq!(facts.to_value(), before);
    let open = facts.unpin(&pinned, Authority::User).unwrap();
    assert!(facts
        .register("rules", "quiet", false, FactOptions { expected_revision: Some(open.revision), ..Default::default() })
        .is_err());
    let updated = facts
        .register(
            "rules",
            "quiet",
            false,
            FactOptions { authority: Authority::User, expected_revision: Some(open.revision), ..Default::default() },
        )
        .unwrap();
    assert!(facts.forget(&open, Authority::User).is_err());
    facts.forget(&updated, Authority::User).unwrap();
    assert!(facts.get("rules", "quiet", 0, &[]).unwrap().is_none());
}

#[test]
fn dependencies_invalidate_transitively_and_cyclic_replacements_are_atomic() {
    let mut facts = FactRegistry::new(16).unwrap();
    let a = facts.register("map", "exit_open", true, FactOptions::default()).unwrap();
    let b = facts
        .register("plan", "route_available", true, FactOptions { dependencies: vec![a.clone()], ..Default::default() })
        .unwrap();
    facts
        .register("plan", "approach", true, FactOptions { dependencies: vec![b.clone()], ..Default::default() })
        .unwrap();
    let before = facts.to_value();
    assert!(facts
        .register(
            "map",
            "exit_open",
            false,
            FactOptions { dependencies: vec![b], expected_revision: Some(a.revision), ..Default::default() }
        )
        .is_err());
    assert_eq!(facts.to_value(), before);
    facts
        .register("map", "exit_open", false, FactOptions { expected_revision: Some(a.revision), ..Default::default() })
        .unwrap();
    assert!(facts.get("plan", "route_available", 0, &[]).unwrap().is_none());
    assert!(facts.get("plan", "approach", 0, &[]).unwrap().is_none());
    let saved = facts.to_value();
    assert_eq!(FactRegistry::restore(&saved).unwrap().to_value(), saved);
}

#[test]
fn privacy_expiry_and_real_erasure_apply_to_dependent_payloads() {
    let mut facts = FactRegistry::new(8).unwrap();
    let private = facts
        .register(
            "actor_a",
            "location",
            "hall",
            FactOptions { privacy: vec!["position".into()], expires: Some(10), ..Default::default() },
        )
        .unwrap();
    facts
        .register(
            "actor_a",
            "plan",
            "wait",
            FactOptions { dependencies: vec![private], pinned: true, ..Default::default() },
        )
        .unwrap();
    assert!(facts.get("actor_a", "plan", 1, &["position".into()]).unwrap().is_none());
    assert!(facts.get("actor_a", "plan", 10, &[]).unwrap().is_none());
    assert_eq!(
        facts.check("actor_a", "location", &Value::str("hall"), 1, &["position".into()]).unwrap().status,
        ClaimStatus::Unknown
    );
    facts.forget_privacy(&["position".into()]).unwrap();
    assert!(facts.to_value().get("facts").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn packets_obey_exact_byte_caps_and_turns_expose_only_current_facts() {
    let mut facts = FactRegistry::new(128).unwrap();
    for n in 0..128 {
        facts
            .register(
                "world",
                &format!("fact_{n:03}"),
                obj! {"enabled"=>n%2!=0,"amount"=>n,"note"=>"Native authored state."},
                FactOptions { now: 1, pinned: n < 2, ..Default::default() },
            )
            .unwrap();
    }
    let packet = facts.packet(1, 128, 1024, &[]).unwrap();
    assert_eq!(packet.json_len(), 927);
    assert_eq!(packet.get("rows").unwrap().as_array().unwrap().len(), 8);
    assert_eq!(packet.get("dropped"), Some(&Value::from(120)));
    let mut context = Context::new().budget_bytes(1200);
    context.prompt("rules", "Use registered facts and validate actions.").unwrap();
    let mut turn = context.turn();
    turn.facts("facts", &facts, 1, 128, 1024, &[]).unwrap();
    assert_eq!(turn.build().unwrap().value.get("facts"), Some(&packet));
    assert!(facts.packet(1, 128, 1, &[]).is_err());
}

#[test]
fn hostile_values_and_saved_metadata_are_rejected_without_partial_mutation() {
    let mut facts = FactRegistry::new(4).unwrap();
    let reference = facts.register("s", "v", 1, FactOptions::default()).unwrap();
    let saved = facts.to_value();
    for v in [
        Value::Number("NaN".into()),
        Value::Number("1 ".into()),
        Value::Number("00".into()),
        Value::Number("1e1000001".into()),
        Value::str("x".repeat(4097)),
        Value::Array(vec![Value::Null; 65]),
    ] {
        assert!(facts.register("s", "other", v, FactOptions::default()).is_err());
        assert_eq!(facts.to_value(), saved);
    }
    let mut deep = Value::Null;
    for _ in 0..10 {
        deep = Value::Array(vec![deep]);
    }
    assert!(facts.check("s", "v", &deep, 0, &[]).is_err());
    assert!(facts
        .register("s", "v", 2, FactOptions { expected_revision: Some(reference.revision + 1), ..Default::default() })
        .is_err());
    assert_eq!(facts.to_value(), saved);
    let mut forged = saved.clone();
    if let Value::Object(fields) = &mut forged {
        fields.iter_mut().find(|(k, _)| k == "generation").unwrap().1 = Value::from(0);
    }
    assert!(FactRegistry::restore(&forged).is_err());
}
