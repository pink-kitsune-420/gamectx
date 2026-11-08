use crate::data::json::{Error, Result, Value};

const TYPES: [&str; 6] = ["null", "boolean", "number", "string", "array", "object"];

fn kind(value: &Value) -> usize {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

fn name(value: &str) -> Result<()> {
    let mut chars = value.chars();
    if value.len() > 64
        || !chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::new("query names must be ASCII identifiers of at most 64 characters"));
    }
    Ok(())
}

/// Describes named tables without sending rows or numeric samples.
///
/// Columns retain the first row's order, with JSON types in a fixed order. String
/// columns include their complete domain only when it has at most 8 values, each at
/// most 64 Unicode characters. Larger domains are omitted entirely, never truncated.
/// Names must be SQL identifiers of at most 64 ASCII characters; at most 16 tables and
/// 64 columns per table. All rows must be objects with the same columns. Empty tables
/// have no inferred columns. The host supplies field meanings separately in its prompt.
///
/// ```
/// use gamectx::{obj, data::query};
/// let rows = vec![obj! {"id" => "moss_cake", "kind" => "food", "unit_weight" => 0.25}];
/// let tables = [("inventory", rows.as_slice())];
/// let description = query::describe(&tables)?;
/// assert!(description.to_json().contains(r#"["unit_weight","number"]"#));
/// # Ok::<(), gamectx::Error>(())
/// ```
pub fn describe(tables: &[(&str, &[Value])]) -> Result<Value> {
    if tables.len() > 16 {
        return Err(Error::new("at most 16 query tables"));
    }
    let mut out = Vec::new();
    for (table, rows) in tables {
        name(table)?;
        if out.iter().any(|(n, _)| n == table) {
            return Err(Error::new("duplicate query table name"));
        }
        if rows.iter().any(|row| row.as_object().is_none()) {
            return Err(Error::new("table rows must be objects"));
        }
        let columns = rows.first().and_then(Value::as_object).unwrap_or(&[]);
        if columns.len() > 64 {
            return Err(Error::new("at most 64 query columns"));
        }
        for row in *rows {
            let entries = row.as_object().expect("checked object");
            if entries.len() != columns.len() || columns.iter().any(|(c, _)| row.get(c).is_none()) {
                return Err(Error::new("query table rows must have the same columns"));
            }
        }
        let mut typed = Vec::new();
        let mut values = Vec::new();
        for (i, (column, _)) in columns.iter().enumerate() {
            name(column)?;
            if columns[..i].iter().any(|(previous, _)| previous == column) {
                return Err(Error::new("duplicate query column name"));
            }
            let mut present = [false; TYPES.len()];
            for row in *rows {
                present[kind(row.get(column).expect("checked column"))] = true;
            }
            let types: Vec<&str> = TYPES.iter().enumerate().filter_map(|(i, t)| present[i].then_some(*t)).collect();
            typed.push(Value::Array(vec![Value::str(column.as_str()), Value::str(types.join("|"))]));
            if types == ["string"] {
                let mut domain = Vec::new();
                let mut complete = true;
                for row in *rows {
                    let value = row.get(column).and_then(Value::as_str).expect("checked string");
                    if value.chars().take(65).count() > 64 {
                        complete = false;
                        break;
                    }
                    if !domain.contains(&Value::str(value)) {
                        domain.push(Value::str(value));
                        if domain.len() > 8 {
                            complete = false;
                            break;
                        }
                    }
                }
                if complete {
                    values.push((column.clone(), Value::Array(domain)));
                }
            }
        }
        let mut description = vec![("columns".to_owned(), Value::Array(typed))];
        if !values.is_empty() {
            description.push(("values".to_owned(), Value::Object(values)));
        }
        out.push((table.to_string(), Value::Object(description)));
    }
    Ok(Value::Object(out))
}

/// Adds opt-in host-authored meanings without changing `describe` or query execution.
/// Metadata keys are table names; each value has description and a columns object.
/// Each column has description, nullable unit, kind, and literal-value meanings.
pub fn describe_with(tables: &[(&str, &[Value])], metadata: &Value) -> Result<Value> {
    if metadata.json_len() > 65_536 {
        return Err(Error::new("query metadata exceeds 65536 bytes"));
    }
    let mut out = describe(tables)?;
    let entries = metadata.as_object().ok_or_else(|| Error::new("query metadata must be an object"))?;
    fn fields(v: &Value, names: &[&str]) -> Result<()> {
        let fields = v.as_object().ok_or_else(|| Error::new("metadata object required"))?;
        if fields.len() != names.len()
            || names.iter().any(|k| v.get(k).is_none())
            || fields
                .iter()
                .enumerate()
                .any(|(i, (k, _))| !names.contains(&k.as_str()) || fields[..i].iter().any(|(old, _)| old == k))
        {
            return Err(Error::new("unexpected or missing metadata field"));
        }
        Ok(())
    }
    fn prose(v: &Value, max: usize) -> Result<()> {
        let s = v.as_str().ok_or_else(|| Error::new("metadata text required"))?;
        if s.is_empty() || s.chars().count() > max || s.chars().any(char::is_control) {
            return Err(Error::new("invalid metadata text"));
        }
        Ok(())
    }
    for (index, (table, meaning)) in entries.iter().enumerate() {
        if entries[..index].iter().any(|(old, _)| old == table) {
            return Err(Error::new("duplicate metadata table"));
        }
        let rows = tables
            .iter()
            .find(|(name, _)| *name == table)
            .map(|(_, rows)| *rows)
            .ok_or_else(|| Error::new("unknown metadata table"))?;
        fields(meaning, &["description", "columns"])?;
        prose(meaning.get("description").unwrap(), 280)?;
        let columns =
            meaning.get("columns").and_then(Value::as_object).ok_or_else(|| Error::new("metadata columns required"))?;
        if columns.len() > 64 {
            return Err(Error::new("metadata column limit"));
        }
        for (i, (column, spec)) in columns.iter().enumerate() {
            if columns[..i].iter().any(|(old, _)| old == column)
                || !rows.first().is_some_and(|r| r.get(column).is_some())
            {
                return Err(Error::new("unknown or duplicate metadata column"));
            }
            fields(spec, &["description", "unit", "kind", "values"])?;
            prose(spec.get("description").unwrap(), 280)?;
            if spec.get("unit") != Some(&Value::Null) {
                prose(spec.get("unit").unwrap(), 32)?;
            }
            let kind = spec.get("kind").and_then(Value::as_str).ok_or_else(|| Error::new("metadata kind required"))?;
            let compatible = match kind {
                "categorical" => rows.iter().all(|r| matches!(r.get(column), Some(Value::String(_) | Value::Null))),
                "quantity" | "continuous" => {
                    rows.iter().all(|r| matches!(r.get(column), Some(Value::Number(_) | Value::Null)))
                }
                "identifier" => rows
                    .iter()
                    .all(|r| matches!(r.get(column), Some(Value::String(_) | Value::Number(_) | Value::Null))),
                _ => false,
            };
            if !compatible {
                return Err(Error::new("metadata kind disagrees with column types"));
            }
            let values = spec
                .get("values")
                .and_then(Value::as_object)
                .ok_or_else(|| Error::new("metadata values object required"))?;
            if values.len() > 16 || !values.is_empty() && kind != "categorical" {
                return Err(Error::new("invalid metadata literal meanings"));
            }
            for (j, (literal, description)) in values.iter().enumerate() {
                prose(&Value::str(literal), 64)?;
                prose(description, 280)?;
                if values[..j].iter().any(|(old, _)| old == literal)
                    || !rows.iter().any(|r| r.get(column).and_then(Value::as_str) == Some(literal))
                {
                    return Err(Error::new("unknown or duplicate metadata literal"));
                }
            }
        }
        let Value::Object(tables) = &mut out else { unreachable!() };
        let (_, Value::Object(fields)) = tables.iter_mut().find(|(name, _)| name == table).expect("described table")
        else {
            unreachable!()
        };
        fields.push(("meaning".into(), meaning.clone()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_host_names_are_rejected() {
        assert!(describe(&[("t", &[]), ("t", &[])]).is_err());
        let duplicate = Value::Object(vec![("x".to_owned(), Value::str("a")), ("x".to_owned(), Value::str("b"))]);
        assert!(describe(&[("t", &[duplicate])]).is_err());
    }
}
