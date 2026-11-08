use gamectx::{character::dialogue::InteractionTools, obj, Value};
fn tools() -> InteractionTools {
    InteractionTools::new("shop", "npc:merchant", &vec!["npc:merchant", "player:a", "player:b"].into()).unwrap()
}
#[test]
fn scope_and_audience_are_host_bound_and_private_events_leave_no_receipt() {
    let mut t = tools();
    let mut other = tools();
    t.publish(&obj!{"id"=>"secret:1","turn"=>5,"subject"=>"seal","kind"=>"ownership","value"=>"player:a","audience"=>vec!["player:a"]}).unwrap();
    let empty = Vec::<Value>::new().into();
    let args = r#"{"since":0,"subject":null,"limit":8}"#;
    assert_eq!(
        t.call("events_recent", args, &empty, 7, 1024, |_| false).unwrap(),
        other.call("events_recent", args, &empty, 7, 1024, |_| false).unwrap()
    );
    assert!(t
        .call("events_recent", r#"{"since":0,"subject":null,"limit":8,"actor":"player:a"}"#, &empty, 7, 1024, |_| true)
        .is_err());
}
#[test]
fn failed_note_budget_preserves_ids_and_clearing_erases_evidence() {
    let mut t = tools();
    let e = vec![obj! {"id"=>"q","speaker"=>"player:a","text"=>"I prefer quiet."}].into();
    let a = r#"{"other":"player:a","note":"Prefers quiet.","source":"q"}"#;
    let before = t.to_value();
    assert!(t.call("relationship_note", a, &e, 7, 64, |_| false).is_err());
    assert_eq!(before, t.to_value());
    let note = t.call("relationship_note", a, &e, 7, 1024, |_| false).unwrap();
    assert_eq!(note.get("id"), Some(&Value::from(1u32)));
    t.clear();
    assert!(!t.to_value().to_json().contains("I prefer quiet."));
    let note = t.call("relationship_note", a, &e, 7, 1024, |_| false).unwrap();
    assert_eq!(note.get("id"), Some(&Value::from(2u32)));
    assert!(t.tools(&["events_recent", "events_recent"]).is_err());
    assert!(t.tools(&["invented"]).is_err());
    assert_eq!(t.tools(&[]).unwrap(), Vec::<Value>::new().into());
}
#[test]
fn proposal_confirmation_cannot_be_supplied_by_model_or_another_player() {
    let mut t = tools();
    let decision = obj! {"action"=>"trade","target"=>"oil"};
    let terms = obj! {"coins"=>3};
    t.set_offers(&vec![obj!{"recipient"=>"player:a","offer"=>obj!{"id"=>"trade:1","scope"=>"shop","revision"=>7,"decision"=>decision.clone(),"terms"=>terms.clone()}}].into()).unwrap();
    let empty = Vec::<Value>::new().into();
    assert!(t.call("interaction_offer", r#"{"id":"trade:1","confirm":true}"#, &empty, 7, 1024, |_| true).is_err());
    t.call("interaction_offer", r#"{"id":"trade:1"}"#, &empty, 7, 1024, |p| p.same(&decision)).unwrap();
    let pending = t.to_value();
    assert!(t.respond("player:b", 7, true, &terms, |_| true).is_err());
    assert_eq!(pending, t.to_value());
    assert!(t.respond("player:a", 8, true, &terms, |_| true).is_err()); // stale attempt is consumed
    assert!(t.respond("player:a", 7, true, &terms, |_| true).is_err());
    t.clear(); // erasure cannot resurrect consumed offer IDs
    t.set_offers(&vec![obj!{"recipient"=>"player:a","offer"=>obj!{"id"=>"trade:1","scope"=>"shop","revision"=>7,"decision"=>decision,"terms"=>terms}}].into()).unwrap();
    assert!(t.call("interaction_offer", r#"{"id":"trade:1"}"#, &empty, 7, 1024, |_| true).is_err());
}
