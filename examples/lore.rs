//! Load authored lore once; authorize/select before rendering source-bound answers.
use gamectx::{dialogue::Answers, lore::Lore, obj, Result, Value};
pub fn main() -> Result<()> {
    let world = Lore::parse(include_str!("dungeon/world.json"))?;
    let selected = world
        .select(&obj! {"actor"=>"npc:merchant","grants"=>Vec::<Value>::new(),"query"=>"tonic purpose",
        "subjects"=>vec!["item:tonic"],"required"=>vec!["tonic_use"],"limit"=>4,"max_bytes"=>1024})?;
    let record = &selected.get("packet").unwrap().as_array().unwrap()[0];
    let answers = Answers::from_value(
        &obj! {"version"=>1,"scope"=>"player_1:merchant","revision"=>7,"unknown"=>"I don't know.",
        "cards"=>vec![obj! {"id"=>"tonic_use","subject"=>"item:tonic","topic"=>"use","kind"=>"lore","source"=>"tonic_use","text"=>record.get("text").unwrap().clone()}]},
    )?;
    // Give offer()/schema() and the current question to the model. All response
    // text comes from the host's card; recheck current source permission here.
    let reply =
        answers.resolve(r#"{"answer":"tonic_use"}"#, &obj! {"scope"=>"player_1:merchant","revision"=>7}, |s| {
            s.get("id") == Some(&Value::str("tonic_use"))
        })?;
    assert_eq!(reply.get("text"), record.get("text"));
    Ok(())
}
