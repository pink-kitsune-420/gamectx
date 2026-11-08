//! Register optional schemas with your model runner, then dispatch its bounded calls.
use gamectx::{knowledge::info::Notes, knowledge::inventory::Inventory, obj, Result, Value};
pub fn main() -> Result<()> {
    let mut notes = Notes::new("merchant:player")?;
    let inventory = Inventory::from_value(
        &obj! {"version"=>1,"revision"=>7,"items"=>vec![obj!{"id"=>"item:oil","label"=>"hinge oil","tags"=>vec!["repair"]}],"holdings"=>vec![obj!{"owner"=>"merchant","item"=>"item:oil","quantity"=>3}]},
    )?;
    // Expose only useful tools for this turn. Their schemas count toward the request.
    let mut tools = notes.tools().as_array().expect("tool schemas").to_vec();
    tools.push(inventory.lookup_tool());
    println!("{}", Value::Array(tools).to_json());
    // The host supplies original dialogue evidence and the visible owner scope.
    let evidence = vec![obj! {"id"=>"line:12","speaker"=>"player","text"=>"The oil belongs to me."}].into();
    let claim = notes.call(
        "info_remember",
        r#"{"subject":"item:oil","attribute":"owner","value":"player","source":"line:12"}"#,
        &evidence,
    )?;
    let remembered =
        notes.call("info_lookup", r#"{"subject":"item:oil","attribute":"owner"}"#, &Vec::<Value>::new().into())?;
    let actual = inventory.lookup_call(r#"{"item":"item:oil"}"#, &vec!["player", "merchant"].into())?;
    // These remain different: remembering the claim does not transfer the oil.
    assert_eq!(actual.get("rows").unwrap().as_array().unwrap()[0].get("owner"), Some(&Value::str("merchant")));
    println!("{}", (obj! {"recorded"=>claim,"reported"=>remembered,"current_inventory"=>actual}).to_json());
    Ok(())
}
