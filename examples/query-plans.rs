//! Parse once; inspect, enforce a host contract and execute within explicit bounds.
use gamectx::{
    obj,
    query::{Query, Shape},
    Value,
};
pub fn main() -> gamectx::Result<()> {
    let guidance = gamectx::query::Guidance::ExplicitSelection;
    let system_prompt = format!("You answer from the supplied query tables.\n{}", guidance.text());
    assert!(system_prompt.contains("do not add LIMIT"));
    assert!(gamectx::query::Guidance::default().text().is_empty());
    let query = Query::parse("SELECT SUM(mass) FROM cargo ORDER BY mass DESC LIMIT 3")?;
    let shape = Shape::from_value(&obj! {
        "table"=>"cargo", "filter_columns"=>Vec::<String>::new(),
        "limit"=>3, "aggregate"=>true
    })?;
    query.require_shape(&shape)?;
    assert_eq!(query.plan().get("limit"), Some(&Value::from(3)));
    let rows = vec![obj! {"id"=>"grove:salt_scroll","mass"=>2}, obj! {"id"=>"quest.gate.open","mass"=>5}];
    assert_eq!(query.run_bounded(&rows, 1024, 128)?.to_json(), "7");
    let later = vec![obj! {"id"=>"grove:café_map","mass"=>8}];
    assert_eq!(query.run_bounded(&later, 1024, 128)?.to_json(), "8");
    assert!(Query::parse("SELECT SUM(mass) FROM cargo WHERE mass > 0 ORDER BY mass DESC LIMIT 3")?
        .require_shape(&shape)
        .is_err());
    println!("{}", query.plan().to_json());
    Ok(())
}
