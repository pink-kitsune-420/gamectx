//! Lossless table packing: a list of objects that share the same keys becomes one header
//! plus value rows, optionally grouped under one or more columns' values.
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
/// by those columns' values: one level per column, so `["kind", "rarity"]` nests rarity
/// groups inside kind groups. Groups keep first-appearance order; rows keep their order
/// within a group.
pub fn pack(items: &[Value], group_by: &[&str]) -> Result<Value> {
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
    if group_by.is_empty() {
        let rows = rows.into_iter().map(Value::Array).collect();
        return Ok(Value::object(vec![("columns", Value::strings(&columns)), ("rows", Value::Array(rows))]));
    }
    for (i, g) in group_by.iter().enumerate() {
        if group_by[..i].contains(g) {
            return Err(Error::new(format!("group_by lists \"{g}\" twice")));
        }
    }
    let label = if group_by.len() == 1 { Value::str(group_by[0]) } else { Value::strings(group_by) };
    if items.is_empty() {
        // Nothing nearby is a normal game state, not an error.
        return Ok(Value::object(vec![
            ("group_by", label),
            ("columns", Value::Array(Vec::new())),
            ("groups", Value::Object(Vec::new())),
        ]));
    }
    let mut at = Vec::with_capacity(group_by.len());
    for g in group_by {
        at.push(
            columns
                .iter()
                .position(|c| c == g)
                .ok_or_else(|| Error::new(format!("group_by column \"{g}\" is not a table column")))?,
        );
    }
    let mut groups: Vec<(String, Value)> = Vec::new();
    for (n, cells) in rows.into_iter().enumerate() {
        let mut keys = Vec::with_capacity(at.len());
        for (g, &i) in group_by.iter().zip(&at) {
            match &cells[i] {
                Value::String(key) => keys.push(key.clone()),
                _ => return Err(Error::new(format!("group_by column \"{g}\" must hold strings (row {n})"))),
            }
        }
        let rest: Vec<Value> = cells.into_iter().enumerate().filter(|(i, _)| !at.contains(i)).map(|(_, v)| v).collect();
        insert(&mut groups, &keys, Value::Array(rest));
    }
    let rest: Vec<&String> = columns.iter().filter(|c| !group_by.contains(&c.as_str())).collect();
    Ok(Value::object(vec![("group_by", label), ("columns", Value::strings(&rest)), ("groups", Value::Object(groups))]))
}

/// Reads a `group_by` setting: one column name, or a list of them for nested groups.
pub fn group_columns(value: &Value) -> Result<Vec<String>> {
    match value {
        Value::String(s) => Ok(vec![s.clone()]),
        Value::Array(items) if !items.is_empty() => items
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| Error::new("group_by must be a column name or a list of column names")),
        _ => Err(Error::new("group_by must be a column name or a non-empty list of column names")),
    }
}

/// Adds a row under its chain of group keys, creating groups in first-appearance order.
fn insert(groups: &mut Vec<(String, Value)>, keys: &[String], row: Value) {
    let (key, deeper) = keys.split_first().expect("at least one group level");
    let at = match groups.iter().position(|(k, _)| k == key) {
        Some(at) => at,
        None => {
            let empty = if deeper.is_empty() { Value::Array(Vec::new()) } else { Value::Object(Vec::new()) };
            groups.push((key.clone(), empty));
            groups.len() - 1
        }
    };
    match &mut groups[at].1 {
        Value::Array(list) => list.push(row),
        Value::Object(inner) => insert(inner, deeper, row),
        _ => unreachable!("groups hold arrays or objects"),
    }
}

/// The row order [`unpack`] restores for a table grouped by `columns`: rows ordered by the
/// first appearance of their first group value, then their second, and so on, keeping
/// their order within each group.
pub fn group_order(items: &[Value], columns: &[&str]) -> Vec<Value> {
    let Some((column, deeper)) = columns.split_first() else {
        return items.to_vec();
    };
    let mut keys: Vec<Option<&Value>> = Vec::new();
    for item in items {
        let key = item.get(column);
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    let mut ordered = Vec::with_capacity(items.len());
    for key in keys {
        let bucket: Vec<Value> = items.iter().filter(|i| i.get(column) == key).cloned().collect();
        ordered.extend(group_order(&bucket, deeper));
    }
    ordered
}

/// Restores the list of objects from a packed table. Grouped tables come back in
/// [`group_order`], with every row and value intact and the group columns first.
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
    let strings = |v: Option<&Value>| {
        v.and_then(Value::as_array)
            .and_then(|items| items.iter().map(|v| v.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
    };
    let columns =
        strings(packed.get("columns")).ok_or_else(|| Error::new("packed table needs a string array \"columns\""))?;
    let mut items = Vec::new();
    if !grouped {
        let rows = packed.get("rows").and_then(Value::as_array).ok_or_else(|| Error::new("rows must be an array"))?;
        for row in rows {
            items.push(row_object(items.len(), row, &columns, &[])?);
        }
        return Ok(items);
    }
    let levels: Vec<String> = match packed.get("group_by") {
        Some(Value::String(s)) => vec![s.clone()],
        other => strings(other)
            .filter(|l| !l.is_empty())
            .ok_or_else(|| Error::new("group_by must be a column name or a non-empty list of them"))?,
    };
    for (i, level) in levels.iter().enumerate() {
        if levels[..i].contains(level) || columns.contains(level) {
            return Err(Error::new(format!("group_by column \"{level}\" is repeated or also listed in columns")));
        }
    }
    let groups =
        packed.get("groups").and_then(Value::as_object).ok_or_else(|| Error::new("groups must be an object"))?;
    walk(groups, &levels, &columns, &mut Vec::new(), &mut items)?;
    Ok(items)
}

fn walk(
    groups: &[(String, Value)],
    levels: &[String],
    columns: &[String],
    path: &mut Vec<(String, Value)>,
    items: &mut Vec<Value>,
) -> Result<()> {
    let depth = path.len();
    for (key, node) in groups {
        path.push((levels[depth].clone(), Value::str(key)));
        if depth + 1 == levels.len() {
            let rows =
                node.as_array().ok_or_else(|| Error::new(format!("group \"{key}\" must be an array of rows")))?;
            for row in rows {
                items.push(row_object(items.len(), row, columns, path)?);
            }
        } else {
            let inner =
                node.as_object().ok_or_else(|| Error::new(format!("group \"{key}\" must hold nested groups")))?;
            walk(inner, levels, columns, path, items)?;
        }
        path.pop();
    }
    Ok(())
}

fn row_object(n: usize, row: &Value, columns: &[String], path: &[(String, Value)]) -> Result<Value> {
    let cells = row.as_array().ok_or_else(|| Error::new(format!("packed row {n} is not an array")))?;
    if cells.len() != columns.len() {
        return Err(Error::new(format!("packed row {n} has {} cells for {} columns", cells.len(), columns.len())));
    }
    let mut entries = path.to_vec();
    entries.extend(columns.iter().cloned().zip(cells.iter().cloned()));
    Ok(Value::Object(entries))
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
        let packed = pack(&source, &[]).unwrap();
        assert_eq!(packed.to_json(), r#"{"columns":["id","hp"],"rows":[["a",1.50],["b",2]]}"#);
        let restored = unpack(&packed).unwrap();
        assert!(Value::Array(restored).same(&Value::Array(source)));
    }

    #[test]
    fn groups_rows_and_restores_them_in_group_order() {
        let source = items(r#"[{"id":"a","kind":"tool"},{"id":"b","kind":"food"},{"id":"c","kind":"tool"}]"#);
        let packed = pack(&source, &["kind"]).unwrap();
        assert_eq!(
            packed.to_json(),
            r#"{"group_by":"kind","columns":["id"],"groups":{"tool":[["a"],["c"]],"food":[["b"]]}}"#
        );
        let restored = Value::Array(unpack(&packed).unwrap());
        assert!(restored.same(&Value::Array(group_order(&source, &["kind"]))));
    }

    #[test]
    fn nests_groups_and_restores_them() {
        let source = items(
            r#"[{"id":"a","kind":"tool","rarity":"common"},{"id":"b","kind":"food","rarity":"rare"},{"id":"c","kind":"tool","rarity":"rare"},{"id":"d","kind":"tool","rarity":"common"}]"#,
        );
        let packed = pack(&source, &["kind", "rarity"]).unwrap();
        assert_eq!(
            packed.to_json(),
            r#"{"group_by":["kind","rarity"],"columns":["id"],"groups":{"tool":{"common":[["a"],["d"]],"rare":[["c"]]},"food":{"rare":[["b"]]}}}"#
        );
        let restored = Value::Array(unpack(&packed).unwrap());
        assert!(restored.same(&Value::Array(group_order(&source, &["kind", "rarity"]))));
        assert!(pack(&source, &["kind", "kind"]).is_err());
    }

    #[test]
    fn rejects_bad_tables_and_groupings() {
        assert!(pack(&items(r#"[{"a":1},{"b":1}]"#), &[]).is_err());
        assert!(pack(&items(r#"[{"a":1},{"a":1,"b":2}]"#), &[]).is_err());
        assert!(pack(&items(r#"[{"a":1}]"#), &["a"]).is_err(), "group values must be strings");
        assert!(pack(&items(r#"[{"a":"x"}]"#), &["b"]).is_err(), "group column must exist");
        assert_eq!(pack(&[], &["a"]).unwrap().to_json(), r#"{"group_by":"a","columns":[],"groups":{}}"#);
        assert!(unpack(&pack(&[], &["a"]).unwrap()).unwrap().is_empty());
        let extra = parse(r#"{"columns":["a"],"rows":[[0]],"symbols":["x"]}"#, 1024).unwrap();
        assert!(unpack(&extra).is_err(), "unknown keys are rejected");
        let dup = parse(r#"{"group_by":"a","columns":["a"],"groups":{"x":[[1]]}}"#, 1024).unwrap();
        assert!(unpack(&dup).is_err());
    }
}
