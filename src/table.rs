//! Lossless table packing: a list of objects that share the same keys becomes one header
//! plus value rows, optionally grouped under one column's values.
//!
//! ```text
//! [{"id":"ember_lantern","kind":"tool"},{"id":"salt_pouch","kind":"reagent"}]
//!   -> {"columns":["id","kind"],"rows":[["ember_lantern","tool"],["salt_pouch","reagent"]]}
//!   -> grouped by kind:
//!      {"group_by":"kind","columns":["id"],"groups":{"tool":[["ember_lantern"]],"reagent":[["salt_pouch"]]}}
//! ```
//!
//! Values are never replaced with lookup indexes: in model tests, short indexes saved no
//! tokens over the words they replaced, and models answered with the index instead of
//! the value it pointed to. Grouping by the column a game filters on improved filtering
//! accuracy and saved a few more tokens.

use crate::json::{Error, Result, Value};

/// Packs uniform objects into `columns` + `rows`, or, with `group_by`, into `groups` keyed
/// by that column's values (first-appearance order; rows keep their order within a group).
pub fn pack(items: &[Value], group_by: Option<&str>) -> Result<Value> {
    let columns: Vec<String> = match items.first() {
        None => Vec::new(),
        Some(first) => first
            .as_object()
            .ok_or_else(|| Error::new("table rows must be objects"))?
            .iter()
            .map(|(k, _)| k.clone())
            .collect(),
    };
    let mut rows = Vec::with_capacity(items.len());
    for (n, item) in items.iter().enumerate() {
        let entries = item.as_object().ok_or_else(|| Error::new(format!("table row {n} is not an object")))?;
        if entries.len() != columns.len() {
            return Err(Error::new(format!("table row {n} has different keys from row 0")));
        }
        let mut cells = Vec::with_capacity(columns.len());
        for column in &columns {
            let cell = item.get(column).ok_or_else(|| Error::new(format!("table row {n} is missing \"{column}\"")))?;
            cells.push(cell.clone());
        }
        rows.push(cells);
    }
    let Some(group) = group_by else {
        let rows = rows.into_iter().map(Value::Array).collect();
        return Ok(Value::object(vec![("columns", Value::strings(&columns)), ("rows", Value::Array(rows))]));
    };

    if items.is_empty() {
        // Nothing nearby is a normal game state, not an error.
        return Ok(Value::object(vec![
            ("group_by", Value::str(group)),
            ("columns", Value::Array(Vec::new())),
            ("groups", Value::Object(Vec::new())),
        ]));
    }
    let at = columns
        .iter()
        .position(|c| c == group)
        .ok_or_else(|| Error::new(format!("group_by column \"{group}\" is not a table column")))?;
    let mut groups: Vec<(String, Value)> = Vec::new();
    for (n, mut cells) in rows.into_iter().enumerate() {
        let Value::String(key) = cells.remove(at) else {
            return Err(Error::new(format!("group_by column \"{group}\" must hold strings (row {n})")));
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, Value::Array(list))) => list.push(Value::Array(cells)),
            _ => groups.push((key, Value::Array(vec![Value::Array(cells)]))),
        }
    }
    let rest: Vec<&String> = columns.iter().filter(|c| *c != group).collect();
    Ok(Value::object(vec![
        ("group_by", Value::str(group)),
        ("columns", Value::strings(&rest)),
        ("groups", Value::Object(groups)),
    ]))
}

/// The row order [`unpack`] restores for a table grouped by `column`: rows sorted by the
/// first appearance of their group value, keeping their order within each group.
pub fn group_order(items: &[Value], column: &str) -> Vec<Value> {
    let mut keys: Vec<Option<&Value>> = Vec::new();
    for item in items {
        let key = item.get(column);
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    let mut ordered = Vec::with_capacity(items.len());
    for key in keys {
        ordered.extend(items.iter().filter(|i| i.get(column) == key).cloned());
    }
    ordered
}

/// Restores the list of objects from a packed table. Grouped tables come back in
/// [`group_order`], with every row and value intact.
pub fn unpack(packed: &Value) -> Result<Vec<Value>> {
    let keys: Vec<&str> = packed.as_object().map(|o| o.iter().map(|(k, _)| k.as_str()).collect()).unwrap_or_default();
    let grouped = match keys.as_slice() {
        ["columns", "rows"] => false,
        ["group_by", "columns", "groups"] => true,
        _ => {
            return Err(Error::new(
                "packed table must be {\"columns\",\"rows\"} or {\"group_by\",\"columns\",\"groups\"}",
            ))
        }
    };
    let columns = packed
        .get("columns")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().map(|v| v.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
        .ok_or_else(|| Error::new("packed table needs a string array \"columns\""))?;
    let row_object = |n: usize, row: &Value, group: Option<(&str, &str)>| -> Result<Value> {
        let cells = row.as_array().ok_or_else(|| Error::new(format!("packed row {n} is not an array")))?;
        if cells.len() != columns.len() {
            return Err(Error::new(format!("packed row {n} has {} cells for {} columns", cells.len(), columns.len())));
        }
        let mut entries = Vec::with_capacity(columns.len() + 1);
        if let Some((column, key)) = group {
            if columns.iter().any(|c| c == column) {
                return Err(Error::new(format!("group_by column \"{column}\" is also listed in columns")));
            }
            entries.push((column.to_owned(), Value::str(key)));
        }
        entries.extend(columns.iter().cloned().zip(cells.iter().cloned()));
        Ok(Value::Object(entries))
    };
    let mut items = Vec::new();
    if grouped {
        let column =
            packed.get("group_by").and_then(Value::as_str).ok_or_else(|| Error::new("group_by must be a string"))?;
        let groups =
            packed.get("groups").and_then(Value::as_object).ok_or_else(|| Error::new("groups must be an object"))?;
        for (key, rows) in groups {
            let rows =
                rows.as_array().ok_or_else(|| Error::new(format!("group \"{key}\" must be an array of rows")))?;
            for row in rows {
                items.push(row_object(items.len(), row, Some((column, key)))?);
            }
        }
    } else {
        let rows = packed.get("rows").and_then(Value::as_array).ok_or_else(|| Error::new("rows must be an array"))?;
        for (n, row) in rows.iter().enumerate() {
            items.push(row_object(n, row, None)?);
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    fn items(text: &str) -> Vec<Value> {
        parse(text, 1 << 20).unwrap().as_array().unwrap().to_vec()
    }

    #[test]
    fn packs_and_restores_rows() {
        let source = items(r#"[{"id":"a","hp":1.50},{"hp":2,"id":"b"}]"#);
        let packed = pack(&source, None).unwrap();
        assert_eq!(packed.to_json(), r#"{"columns":["id","hp"],"rows":[["a",1.50],["b",2]]}"#);
        let restored = unpack(&packed).unwrap();
        assert!(Value::Array(restored).same(&Value::Array(source)));
    }

    #[test]
    fn groups_rows_and_restores_them_in_group_order() {
        let source = items(r#"[{"id":"a","kind":"tool"},{"id":"b","kind":"food"},{"id":"c","kind":"tool"}]"#);
        let packed = pack(&source, Some("kind")).unwrap();
        assert_eq!(
            packed.to_json(),
            r#"{"group_by":"kind","columns":["id"],"groups":{"tool":[["a"],["c"]],"food":[["b"]]}}"#
        );
        let restored = Value::Array(unpack(&packed).unwrap());
        assert!(restored.same(&Value::Array(group_order(&source, "kind"))));
    }

    #[test]
    fn rejects_bad_tables_and_groupings() {
        assert!(pack(&items(r#"[{"a":1},{"b":1}]"#), None).is_err());
        assert!(pack(&items(r#"[{"a":1},{"a":1,"b":2}]"#), None).is_err());
        assert!(pack(&items(r#"[{"a":1}]"#), Some("a")).is_err(), "group values must be strings");
        assert!(pack(&items(r#"[{"a":"x"}]"#), Some("b")).is_err(), "group column must exist");
        assert_eq!(pack(&[], Some("a")).unwrap().to_json(), r#"{"group_by":"a","columns":[],"groups":{}}"#);
        assert!(unpack(&pack(&[], Some("a")).unwrap()).unwrap().is_empty());
        let extra = parse(r#"{"columns":["a"],"rows":[[0]],"symbols":["x"]}"#, 1024).unwrap();
        assert!(unpack(&extra).is_err(), "unknown keys are rejected");
        let dup = parse(r#"{"group_by":"a","columns":["a"],"groups":{"x":[[1]]}}"#, 1024).unwrap();
        assert!(unpack(&dup).is_err());
    }
}
