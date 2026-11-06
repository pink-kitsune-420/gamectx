use gamectx::{
    obj,
    query::{Query, Shape},
    Error, Result, Value,
};
pub fn process(v: &Value) -> Result<Value> {
    let query = Query::parse(v.get("sql").and_then(Value::as_str).ok_or_else(|| Error::new("sql string required"))?)?;
    if let Some(shape) = v.get("shape") {
        query.require_shape(&Shape::from_value(shape)?)?;
    }
    let plan = query.plan();
    if v.get("op").and_then(Value::as_str).unwrap_or("plan") == "plan" {
        return Ok(plan);
    }
    let rows = v.get("rows").and_then(Value::as_array).ok_or_else(|| Error::new("rows required"))?;
    let size = |k| v.get(k).and_then(Value::as_usize).ok_or_else(|| Error::new("size required"));
    let result = query.run_bounded(rows, size("max_rows")?, size("result_bytes")?)?;
    Ok(obj! {"plan"=>plan,"result"=>result})
}
