//! Compare current execution with the frozen alpha.4 implementation, including errors.
#[allow(dead_code, unused_imports)]
mod query_before;
pub use gamectx::{data, data::json, data::view, obj};
use gamectx::{data::query, Value};
use std::io::{self, BufRead};
fn main() {
    let mut count = 0;
    for line in io::stdin().lock().lines() {
        let v = json::parse(&line.unwrap(), 1 << 20).unwrap();
        let sql = v.get("sql").and_then(Value::as_str).unwrap();
        let rows = v.get("rows").and_then(Value::as_array).unwrap();
        let old =
            query_before::Query::parse(sql).and_then(|q| q.run(rows)).map(|v| v.to_json()).map_err(|e| e.to_string());
        let new = query::Query::parse(sql).and_then(|q| q.run(rows)).map(|v| v.to_json()).map_err(|e| e.to_string());
        assert_eq!(old, new, "{}", v.to_json());
        count += 1;
    }
    println!("{count} baseline/current results and errors byte-identical");
}
