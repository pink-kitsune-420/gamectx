use crate::json::{Error, Result, Value};

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
/// use gamectx::{obj, query};
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
