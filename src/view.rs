//! Views choose and shape rows before they are packed: sort, keep the top N, keep only
//! some columns, and round numbers. Packing is lossless; a view drops information on
//! purpose, so verification compares the packed table with the viewed rows instead.
//!
//! Steps run in a fixed order: `sort_by`, then `limit`, then `fields`, then `round`.
//! Sorting before `fields` lets a game sort by a column it does not send, such as
//! distance to the player.

use crate::json::{Error, Result, Value};
use std::cmp::Ordering;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    /// Column to sort by, and whether to sort descending. Stable; nulls always last.
    pub sort_by: Option<(String, bool)>,
    /// Keep at most this many rows (after sorting).
    pub limit: Option<usize>,
    /// Keep only these columns, in this order.
    pub fields: Option<Vec<String>>,
    /// Round numbers in these columns to this many decimal places.
    pub round: Vec<(String, u32)>,
}

impl View {
    pub fn is_empty(&self) -> bool {
        *self == View::default()
    }

    /// Reads `sort_by`, `limit`, `fields` and `round` from an object; other keys are ignored.
    /// `round` is an object of column name to decimal places, e.g. `{"weight": 1}`.
    pub fn from_json(obj: &Value) -> Result<View> {
        let mut view = View::default();
        if let Some(v) = obj.get("sort_by") {
            view.sort_by = Some(View::sort_key(v.as_str().ok_or_else(|| Error::new("sort_by must be a string"))?)?);
        }
        if let Some(v) = obj.get("limit") {
            view.limit = Some(v.as_usize().ok_or_else(|| Error::new("limit must be a non-negative integer"))?);
        }
        if let Some(v) = obj.get("fields") {
            let list = v
                .as_array()
                .and_then(|a| a.iter().map(|s| s.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
                .ok_or_else(|| Error::new("fields must be an array of column names"))?;
            view.fields = Some(list);
        }
        if let Some(v) = obj.get("round") {
            let entries = v.as_object().ok_or_else(|| Error::new("round must be an object of column: places"))?;
            for (column, places) in entries {
                let places = places.as_usize().filter(|p| *p <= 12).ok_or_else(|| {
                    Error::new(format!("round places for \"{column}\" must be an integer from 0 to 12"))
                })?;
                view.round.push((column.clone(), places as u32));
            }
        }
        Ok(view)
    }

    /// Parses `sort_by` text: `"count"` ascending, `"-count"` descending.
    pub fn sort_key(text: &str) -> Result<(String, bool)> {
        let (column, descending) = match text.strip_prefix('-') {
            Some(rest) => (rest, true),
            None => (text, false),
        };
        if column.is_empty() {
            return Err(Error::new("sort_by needs a column name"));
        }
        Ok((column.to_owned(), descending))
    }
}

/// Applies a view to a list of objects.
pub fn apply(items: &[Value], view: &View) -> Result<Vec<Value>> {
    let mut rows: Vec<Value> = items.to_vec();
    if let Some((column, descending)) = &view.sort_by {
        let mut keyed = Vec::with_capacity(rows.len());
        for (n, row) in rows.into_iter().enumerate() {
            let key = sort_value(&row, column).map_err(|e| Error::new(format!("row {n}: {e}")))?;
            keyed.push((key, row));
        }
        let numeric = keyed.iter().any(|(k, _)| matches!(k, Key::Number(_)));
        let text = keyed.iter().any(|(k, _)| matches!(k, Key::Text(_)));
        if numeric && text {
            return Err(Error::new(format!("sort_by column \"{column}\" mixes numbers and strings")));
        }
        keyed.sort_by(|(a, _), (b, _)| compare(a, b, *descending));
        rows = keyed.into_iter().map(|(_, row)| row).collect();
    }
    if let Some(limit) = view.limit {
        rows.truncate(limit);
    }
    if let Some(fields) = &view.fields {
        for (i, f) in fields.iter().enumerate() {
            if fields[..i].contains(f) {
                return Err(Error::new(format!("fields lists \"{f}\" twice")));
            }
        }
        for (n, row) in rows.iter_mut().enumerate() {
            let mut kept = Vec::with_capacity(fields.len());
            for f in fields {
                let value = row.get(f).ok_or_else(|| Error::new(format!("row {n} has no field \"{f}\"")))?;
                kept.push((f.clone(), value.clone()));
            }
            *row = Value::Object(kept);
        }
    }
    for (column, places) in &view.round {
        if *places > 12 {
            return Err(Error::new("round accepts at most 12 decimal places"));
        }
        for (n, row) in rows.iter_mut().enumerate() {
            let Value::Object(entries) = row else {
                return Err(Error::new(format!("row {n} is not an object")));
            };
            let cell = entries
                .iter_mut()
                .find(|(k, _)| k == column)
                .map(|(_, v)| v)
                .ok_or_else(|| Error::new(format!("row {n} has no field \"{column}\" to round")))?;
            match cell {
                Value::Number(text) => *text = round_text(text, *places),
                Value::Null => {}
                _ => return Err(Error::new(format!("round column \"{column}\" holds a non-number (row {n})"))),
            }
        }
    }
    Ok(rows)
}

enum Key {
    Number(f64),
    Text(String),
    Null,
}

fn sort_value(row: &Value, column: &str) -> Result<Key> {
    match row.get(column) {
        None => Err(Error::new(format!("no field \"{column}\" to sort by"))),
        Some(Value::Number(n)) => Ok(Key::Number(n.parse().map_err(|_| Error::new("unreadable number"))?)),
        Some(Value::String(s)) => Ok(Key::Text(s.clone())),
        Some(Value::Null) => Ok(Key::Null),
        Some(_) => Err(Error::new(format!("sort_by column \"{column}\" must hold numbers or strings"))),
    }
}

fn compare(a: &Key, b: &Key, descending: bool) -> Ordering {
    let order = match (a, b) {
        (Key::Null, Key::Null) => return Ordering::Equal,
        (Key::Null, _) => return Ordering::Greater, // nulls last in both directions
        (_, Key::Null) => return Ordering::Less,
        (Key::Number(x), Key::Number(y)) => x.total_cmp(y),
        (Key::Text(x), Key::Text(y)) => x.cmp(y),
        _ => Ordering::Equal, // mixed kinds are rejected before sorting
    };
    if descending {
        order.reverse()
    } else {
        order
    }
}

/// Rounds number text to `places` decimals and trims trailing zeros. Plain integers are
/// returned unchanged, so integers beyond 2^53 keep their exact digits.
pub fn round_text(text: &str, places: u32) -> String {
    if !text.contains(['.', 'e', 'E']) {
        return text.to_owned();
    }
    let Ok(value) = text.parse::<f64>() else {
        return text.to_owned();
    };
    let mut out = format!("{:.*}", places as usize, value);
    if out.contains('.') {
        out = out.trim_end_matches('0').trim_end_matches('.').to_owned();
    }
    if out == "-0" {
        out = "0".to_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    fn items(text: &str) -> Vec<Value> {
        parse(text, 1 << 20).unwrap().as_array().unwrap().to_vec()
    }

    fn json(rows: Vec<Value>) -> String {
        Value::Array(rows).to_json()
    }

    #[test]
    fn sorts_limits_selects_and_rounds_in_order() {
        let source = items(
            r#"[{"id":"a","dist":12.5,"hp":0.333},{"id":"b","dist":3,"hp":1.5},{"id":"c","dist":null,"hp":2},{"id":"d","dist":7.25,"hp":-0.04}]"#,
        );
        let view = View {
            sort_by: Some(("dist".into(), false)),
            limit: Some(3),
            fields: Some(vec!["id".into(), "hp".into()]),
            round: vec![("hp".into(), 1)],
        };
        assert_eq!(
            json(apply(&source, &view).unwrap()),
            r#"[{"id":"b","hp":1.5},{"id":"d","hp":0},{"id":"a","hp":0.3}]"#
        );
    }

    #[test]
    fn descending_keeps_nulls_last_and_ties_stable() {
        let source = items(r#"[{"k":"x","n":1},{"k":"y","n":null},{"k":"z","n":2},{"k":"w","n":1}]"#);
        let view = View { sort_by: Some(View::sort_key("-n").unwrap()), ..View::default() };
        let order: Vec<String> =
            apply(&source, &view).unwrap().iter().map(|r| r.get("k").unwrap().as_str().unwrap().to_owned()).collect();
        assert_eq!(order, ["z", "x", "w", "y"]);
    }

    #[test]
    fn rounding_keeps_integers_and_trims() {
        assert_eq!(round_text("3.4567", 2), "3.46");
        assert_eq!(round_text("2.50", 1), "2.5");
        assert_eq!(round_text("4.0001", 2), "4");
        assert_eq!(round_text("1e-3", 1), "0");
        assert_eq!(round_text("12345678901234567890", 0), "12345678901234567890");
        assert_eq!(round_text("-0.04", 1), "0");
    }

    #[test]
    fn rejects_bad_views() {
        let source = items(r#"[{"a":1,"b":"x"},{"a":"y","b":"z"}]"#);
        let sort = |c: &str| View { sort_by: Some(View::sort_key(c).unwrap()), ..View::default() };
        assert!(apply(&source, &sort("a")).is_err(), "mixed numbers and strings");
        assert!(apply(&source, &sort("missing")).is_err());
        assert!(View::sort_key("-").is_err());
        let fields = |f: &[&str]| View { fields: Some(f.iter().map(|s| s.to_string()).collect()), ..View::default() };
        assert!(apply(&source, &fields(&["a", "a"])).is_err());
        assert!(apply(&source, &fields(&["c"])).is_err());
        let round = View { round: vec![("b".into(), 1)], ..View::default() };
        assert!(apply(&source, &round).is_err(), "rounding a string");
    }
}
