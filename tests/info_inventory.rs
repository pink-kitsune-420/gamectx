use gamectx::{knowledge::info::Notes, knowledge::inventory::Inventory, obj, Value};
fn snapshot() -> Value {
    obj! {"version"=>1,"revision"=>0,"items"=>vec![obj!{"id"=>"item:oil","label"=>"oil","tags"=>vec!["repair"]}],"holdings"=>vec![obj!{"owner"=>"merchant","item"=>"item:oil","quantity"=>5}]}
}
#[test]
fn model_notes_preserve_conflicts_and_original_source_without_promoting_claims() {
    let mut notes = Notes::new("merchant:player").unwrap();
    let source = vec![obj! {"id"=>"line:1","speaker"=>"player","text"=>"The oil is mine."}].into();
    let r = notes
        .call(
            "info_remember",
            r#"{"subject":"item:oil","attribute":"owner","value":"player","source":"line:1"}"#,
            &source,
        )
        .unwrap();
    assert_eq!(r.get("status"), Some(&Value::str("reported")));
    assert_eq!(r.get("source").unwrap().get("text"), Some(&Value::str("The oil is mine.")));
    let before = notes.to_value();
    assert!(notes
        .call(
            "info_remember",
            r#"{"subject":"item:oil","attribute":"owner","value":"player","source":"missing"}"#,
            &source
        )
        .is_err());
    assert_eq!(before, notes.to_value());
    notes.clear();
    let r = notes
        .call(
            "info_remember",
            r#"{"subject":"item:oil","attribute":"owner","value":"merchant","source":"line:1"}"#,
            &source,
        )
        .unwrap();
    assert_eq!(r.get("id"), Some(&Value::from(2u32)));
    assert!(notes.call("info_forget", r#"{"id":1}"#, &source).is_err());
    assert_eq!(notes.tools().as_array().unwrap().len(), 3);
    // Notebook scopes are independent, including erasure.
    let mut other = Notes::new("keeper:player").unwrap();
    let result = other
        .call("info_lookup", r#"{"subject":"item:oil","attribute":"owner"}"#, &Vec::<Value>::new().into())
        .unwrap();
    assert!(result.get("notes").unwrap().as_array().unwrap().is_empty());
}
#[test]
fn ownership_lookup_binds_visibility_and_transfers_are_atomic_replay_safe() {
    let mut inv = Inventory::from_value(&snapshot()).unwrap();
    let hidden = inv.lookup_call(r#"{"item":"item:oil"}"#, &vec!["player"].into()).unwrap();
    assert!(hidden.get("rows").unwrap().as_array().unwrap().is_empty());
    let visible = inv.lookup_call(r#"{"item":"item:oil"}"#, &vec!["merchant"].into()).unwrap();
    assert_eq!(visible.get("rows").unwrap().as_array().unwrap()[0].get("count"), Some(&Value::from(5u32)));
    assert!(inv.lookup_call(r#"{"item":"item:oil","owners":["merchant"]}"#, &vec!["player"].into()).is_err());
    let movement = obj! {"item"=>"item:oil","from"=>"merchant","to"=>"player","quantity"=>2};
    let before = inv.to_value();
    let mut n = 0;
    assert!(inv
        .transfer(0, &vec![movement.clone(), movement.clone()].into(), |_| {
            n += 1;
            n == 1
        })
        .is_err());
    assert_eq!(before, inv.to_value());
    inv.transfer(0, &vec![movement].into(), |_| true).unwrap();
    let after = inv.to_value();
    assert!(inv
        .propose("merchant", r#"{"revision":0,"item":"item:oil","to":"player","quantity":2}"#, |_| true)
        .is_err());
    assert_eq!(after, inv.to_value());
    let rows = inv.rows(&vec!["merchant", "player"].into(), &vec!["repair"].into()).unwrap();
    assert_eq!(
        gamectx::data::query::run("SELECT SUM(count) FROM items", &[("items", &rows)]).unwrap(),
        Value::from(5u32)
    );
    assert_eq!(inv.lookup_tool().get("function").unwrap().get("name"), Some(&Value::str("inventory_lookup")));
}
