use gamectx::{character::dialogue::Answers, knowledge::lore::Lore, obj, Value};
#[test]
fn lore_permissions_required_fit_and_snapshot_ownership_are_explicit() {
    let lore = Lore::parse(include_str!("../examples/lantern-vault/world.json")).unwrap();
    let query = |actor: &str, grants: Vec<&str>, required: Vec<&str>, bytes: usize| obj! {"actor"=>actor,"grants"=>grants,"query"=>"vault inscription","subjects"=>vec!["room:vault"],"required"=>required,"limit"=>4,"max_bytes"=>bytes};
    let private = lore.select(&query("npc:merchant", vec!["vault_open"], vec![], 2048)).unwrap();
    assert!(private.get("packet").unwrap().as_array().unwrap().is_empty());
    assert!(!private.to_json().contains("vault_inscription"));
    assert!(lore.select(&query("npc:merchant", vec!["vault_open"], vec!["vault_inscription"], 2048)).is_err());
    assert!(lore.select(&query("npc:keeper", vec![], vec!["vault_inscription"], 2048)).is_err());
    assert!(lore.select(&query("npc:keeper", vec!["vault_open"], vec!["vault_inscription"], 2)).is_err());
    let selected = lore.select(&query("npc:keeper", vec!["vault_open"], vec!["vault_inscription"], 2048)).unwrap();
    assert_eq!(selected.get("packet").unwrap().as_array().unwrap().len(), 1);
}
#[test]
fn live_fields_override_matching_authored_fields_and_revoke_the_old_source() {
    let definition = obj! {"version"=>1,"scope"=>"talk","revision"=>3,"unknown"=>"Unknown.","cards"=>vec![
    obj! {"id"=>"old","subject"=>"oil","topic"=>"ownership","kind"=>"lore","source"=>"old","text"=>"The merchant owns the oil."},
    obj! {"id"=>"now","subject"=>"oil","topic"=>"ownership","kind"=>"state","source"=>"oil","text"=>"You own the oil."}]};
    let answers = Answers::from_value(&definition).unwrap();
    assert_eq!(answers.offer().as_array().unwrap().len(), 1);
    let context = obj! {"scope"=>"talk","revision"=>3};
    assert!(answers.resolve(r#"{"answer":"old"}"#, &context, |_| panic!("old source never available")).is_err());
    assert!(answers.resolve(r#"{"answer":"now","speech":"Free stock!"}"#, &context, |_| true).is_err());
    assert!(answers.resolve(r#"{"answer":"now"}"#, &obj! {"scope"=>"talk","revision"=>4}, |_| true).is_err());
    let result = answers.resolve(r#"{"answer":"now"}"#, &context, |_| true).unwrap();
    assert_eq!(result.get("text"), Some(&Value::str("You own the oil.")));
}
