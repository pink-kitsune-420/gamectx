//! Bind physical inputs, stabilize reactions, capture replay and describe query meanings.
use gamectx::{
    data::json::parse,
    obj,
    runtime::brain::{BoundBrain, Brain, FeatureSchema, Selection, SelectionPolicy, StableBrain},
    runtime::replay::Recorder,
    Value,
};
pub fn main() -> gamectx::Result<()> {
    let schema = FeatureSchema::parse(include_str!("../fixtures/brain/bindings.json"))?;
    let definition = obj! {"version"=>1,"inputs"=>vec!["x","y"],"outputs"=>vec!["a","b"],"nodes"=>vec![
    obj!{"id"=>"a","weights"=>obj!{"x"=>1},"bias"=>0,"pinned"=>false,"learning_limit"=>1},
    obj!{"id"=>"b","weights"=>obj!{"y"=>1},"bias"=>0,"pinned"=>false,"learning_limit"=>1}]};
    let bound = BoundBrain::new(Brain::from_value(&definition)?, schema.clone())?;
    let saved = bound.to_value()?;
    let restored = BoundBrain::restore(&saved, &schema)?;
    let (brain, schema) = restored.into_parts();
    let mut recorder = Recorder::new(&brain, 8)?;
    let mut stable = StableBrain::new(brain, 40_000, 4)?;
    let mut selection = Selection::default();
    let inputs = schema.normalize(&obj! {"y"=>0.5,"x"=>2})?;
    assert_eq!(inputs.as_slice(), [Some(800_000), Some(750_000)]);
    stable.run_into(
        1,
        inputs.as_slice(),
        SelectionPolicy { allowed: 3, min_score: 0, min_margin: 0, urgent: false },
        &mut selection,
    )?;
    assert_eq!(selection.choice(), Some(0));
    let event = obj! {"op"=>"run","tick"=>1,"inputs"=>inputs.as_slice().iter().map(|v|v.map_or(Value::Null,Value::from)).collect::<Vec<_>>(),"mask"=>3,"low"=>0,"gap"=>0,"expected"=>obj!{"choice"=>0},"elapsed_ns"=>Value::Null};
    let report = recorder.record(&event)?;
    assert_eq!(report.get("reason"), Some(&Value::str("chosen")));
    let replay = gamectx::runtime::replay::run(&recorder.to_value())?;
    assert_eq!(replay.get("summary").unwrap().get("correct"), Some(&Value::from(1)));
    let tables = parse(include_str!("../data/query-meaning-tables.json"), 65536)?;
    let metadata = parse(include_str!("../data/query-meanings.json"), 65536)?;
    let named: Vec<_> =
        tables.as_object().unwrap().iter().map(|(name, rows)| (name.as_str(), rows.as_array().unwrap())).collect();
    let described = gamectx::data::query::describe_with(&named, &metadata)?;
    assert!(described.get("items").unwrap().get("meaning").is_some());
    println!("{}",(obj!{"reaction"=>selection.choice(),"diagnostic"=>report,"replay"=>replay.get("summary").unwrap().clone(),"query_tables"=>described}).to_json());
    Ok(())
}
