//! Views choose and shape rows before they are packed: filter, keep rows relevant to some
//! text, sort, keep the top N, keep only some columns, and round numbers. Packing is
//! lossless; a view drops information on purpose, so verification compares the packed
//! table with the viewed rows instead.
//!
//! Steps run in a fixed order: `where`, `relevant_to`, `sort_by`, `limit`, `fields`,
//! `round`. Filtering and sorting before `fields` lets a game use columns it does not send,
//! such as distance to the player.

use crate::json::{Error, Result, Value};
use std::cmp::Ordering;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    /// Keep only rows meeting every condition (column, operator, value).
    pub filters: Vec<(String, Op, Value)>,
    /// Keep only the rows that best match the words of this text; see [`relevant_rows`].
    pub relevant_to: Option<String>,
    /// With `relevant_to`: send no rows, instead of every row, when nothing matches.
    pub relevant_only: bool,
    /// Column to sort by, and whether to sort descending. Stable; nulls always last.
    pub sort_by: Option<(String, bool)>,
    /// Keep at most this many rows (after sorting).
    pub limit: Option<usize>,
    /// Keep only these columns, in this order.
    pub fields: Option<Vec<String>>,
    /// Round numbers in these columns to this many decimal places.
    pub round: Vec<(String, u32)>,
}

/// A `where` comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    In,
}

impl Op {
    pub fn parse(text: &str) -> Result<Op> {
        Ok(match text {
            "=" | "==" => Op::Eq,
            "!=" => Op::Ne,
            ">" => Op::Gt,
            ">=" => Op::Ge,
            "<" => Op::Lt,
            "<=" => Op::Le,
            "in" => Op::In,
            other => return Err(Error::new(format!("unknown where operator \"{other}\"; use = != > >= < <= in"))),
        })
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.parse().ok(),
        _ => None,
    }
}

/// Equality that treats `1` and `1.0` as the same number.
fn equal(a: &Value, b: &Value) -> bool {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// Whether a cell meets a condition. Ordering operators need numbers on both sides; a
/// non-number cell (such as `null`) simply does not match.
fn meets(cell: &Value, op: Op, value: &Value) -> bool {
    match op {
        Op::Eq => equal(cell, value),
        Op::Ne => !equal(cell, value),
        Op::In => value.as_array().is_some_and(|options| options.iter().any(|o| equal(cell, o))),
        _ => match (number(cell), number(value)) {
            (Some(x), Some(y)) => match op {
                Op::Gt => x > y,
                Op::Ge => x >= y,
                Op::Lt => x < y,
                _ => x <= y,
            },
            _ => false,
        },
    }
}

/// Words of `text` used for relevance: lowercase runs of letters, digits and `_`, without
/// all-digit words, plus each longer word without a trailing plural `s`.
pub(crate) fn words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    for word in lower.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()) {
        if word.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let singular = (word.chars().count() > 3 && word.ends_with('s')).then(|| word[..word.len() - 1].to_owned());
        for w in std::iter::once(word.to_owned()).chain(singular) {
            if !out.contains(&w) {
                out.push(w);
            }
        }
    }
    out
}

/// Keeps the rows whose text cells match the most words of `text`. A word matches a row
/// when it equals one of the row's text cells, or one `_`-separated part of a cell
/// (lowercased). Numbers in the text are ignored. If no row matches any word, every row
/// is kept, so questions the words do not anticipate still see the whole table.
pub fn relevant_rows(rows: Vec<Value>, text: &str) -> Vec<Value> {
    let scores = relevance_scores(&rows, text);
    let best = scores.iter().copied().max().unwrap_or(0);
    if best == 0 {
        return rows;
    }
    rows.into_iter().zip(scores).filter(|(_, s)| *s == best).map(|(r, _)| r).collect()
}

fn any_match(rows: &[Value], text: &str) -> bool {
    relevance_scores(rows, text).into_iter().any(|s| s > 0)
}

/// How many words of `text` each row matches; see [`relevant_rows`].
pub fn relevance_scores(rows: &[Value], text: &str) -> Vec<usize> {
    let words = words(text);
    rows.iter()
        .map(|row| {
            let mut parts: Vec<String> = Vec::new();
            for (_, v) in row.as_object().unwrap_or(&[]) {
                if let Value::String(s) = v {
                    let cell = s.to_lowercase();
                    parts.extend(cell.split('_').map(str::to_owned));
                    parts.push(cell);
                }
            }
            words.iter().filter(|w| parts.contains(w)).count()
        })
        .collect()
}

impl View {
    pub fn is_empty(&self) -> bool {
        *self == View::default()
    }

    /// Reads `sort_by`, `limit`, `fields` and `round` from an object; other keys are ignored.
    /// `round` is an object of column name to decimal places, e.g. `{"weight": 1}`.
    pub fn from_json(obj: &Value) -> Result<View> {
        let mut view = View::default();
        if let Some(v) = obj.get("where") {
            let conditions = v.as_object().ok_or_else(|| Error::new("where must be an object of column: condition"))?;
            for (column, condition) in conditions {
                match condition {
                    Value::Object(entries) if entries.len() == 1 => {
                        let (op, value) = &entries[0];
                        let op = Op::parse(op)?;
                        if op == Op::In && value.as_array().is_none() {
                            return Err(Error::new(format!("where \"{column}\": \"in\" needs an array")));
                        }
                        view.filters.push((column.clone(), op, value.clone()));
                    }
                    Value::Object(_) | Value::Array(_) => {
                        return Err(Error::new(format!(
                            "where \"{column}\": use a plain value, or one {{\"operator\": value}}"
                        )))
                    }
                    plain => view.filters.push((column.clone(), Op::Eq, plain.clone())),
                }
            }
        }
        if let Some(v) = obj.get("relevant_to") {
            view.relevant_to = Some(v.as_str().ok_or_else(|| Error::new("relevant_to must be a string"))?.to_owned());
        }
        if let Some(v) = obj.get("relevant_fallback") {
            view.relevant_only = match v.as_str() {
                Some("all") => false,
                Some("none") => true,
                _ => return Err(Error::new("relevant_fallback must be \"all\" or \"none\"")),
            };
            if view.relevant_to.is_none() {
                return Err(Error::new("relevant_fallback needs relevant_to"));
            }
        }
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
    if !view.filters.is_empty() {
        let mut kept = Vec::with_capacity(rows.len());
        for (n, row) in rows.into_iter().enumerate() {
            let mut keep = true;
            for (column, op, value) in &view.filters {
                let cell = row
                    .get(column)
                    .ok_or_else(|| Error::new(format!("row {n} has no field \"{column}\" to filter on")))?;
                keep &= meets(cell, *op, value);
            }
            if keep {
                kept.push(row);
            }
        }
        rows = kept;
    }
    if let Some(text) = &view.relevant_to {
        let total = rows.len();
        rows = relevant_rows(rows, text);
        if view.relevant_only && rows.len() == total && !any_match(&rows, text) {
            rows.clear();
        }
    }
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
            ..View::default()
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
    fn where_filters_numbers_strings_and_lists() {
        let source = items(
            r#"[{"id":"a","kind":"weapon","count":20},{"id":"b","kind":"food","count":15.0},{"id":"c","kind":"weapon","count":null},{"id":"d","kind":"key","count":3}]"#,
        );
        let ids = |v: &str| -> Vec<String> {
            let view = View::from_json(&parse(v, 1024).unwrap()).unwrap();
            apply(&source, &view).unwrap().iter().map(|r| r.get("id").unwrap().as_str().unwrap().to_owned()).collect()
        };
        assert_eq!(ids(r#"{"where":{"count":{">=":15}}}"#), ["a", "b"]);
        assert_eq!(ids(r#"{"where":{"count":15}}"#), ["b"], "15 equals 15.0");
        assert_eq!(ids(r#"{"where":{"kind":"weapon","count":{">":5}}}"#), ["a"]);
        assert_eq!(ids(r#"{"where":{"kind":{"in":["food","key"]}}}"#), ["b", "d"]);
        assert_eq!(ids(r#"{"where":{"kind":{"!=":"weapon"}}}"#), ["b", "d"]);
        assert_eq!(ids(r#"{"where":{"count":{"<":100}}}"#), ["a", "b", "d"], "null never meets an ordering");
        assert!(View::from_json(&parse(r#"{"where":{"a":{"~":1}}}"#, 64).unwrap()).is_err());
        assert!(View::from_json(&parse(r#"{"where":{"a":{"in":1}}}"#, 64).unwrap()).is_err());
        assert!(View::from_json(&parse(r#"{"where":{"a":[1]}}"#, 64).unwrap()).is_err());
        let missing = View::from_json(&parse(r#"{"where":{"zzz":1}}"#, 64).unwrap()).unwrap();
        assert!(apply(&source, &missing).is_err());
    }

    #[test]
    fn relevant_rows_keep_best_matches_or_everything() {
        let source = items(
            r#"[{"id":"salt_scroll","rarity":"rare","kind":"scroll"},{"id":"salt_pouch","rarity":"common","kind":"reagent"},{"id":"ash_knife","rarity":"rare","kind":"weapon"},{"id":"Café_Map","rarity":"common","kind":"scroll"}]"#,
        );
        let ids = |text: &str| -> Vec<String> {
            relevant_rows(source.clone(), text)
                .iter()
                .map(|r| r.get("id").unwrap().as_str().unwrap().to_owned())
                .collect()
        };
        assert_eq!(ids("How many `salt_scroll` do I have?"), ["salt_scroll"]);
        assert_eq!(ids("hand me the salt scroll"), ["salt_scroll"], "both parts beat one");
        assert_eq!(ids("any rare weapons?"), ["ash_knife"], "plural s is optional");
        assert_eq!(
            ids("count at least 15"),
            ["salt_scroll", "salt_pouch", "ash_knife", "Café_Map"],
            "no match keeps all"
        );
        assert_eq!(ids("the café map"), ["Café_Map"], "Unicode words match");
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
