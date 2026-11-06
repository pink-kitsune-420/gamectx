//! Validated authored lore, selected only after host-provided knowledge/visibility gates.
use crate::{checked as c, obj, request::bounded, Result, Value};
use std::io::Read;
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
fn strings(v: &Value, max: usize) -> Result<Vec<String>> {
    let a = c::list(v)?;
    if a.len() > max {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for x in a {
        let s = c::text(x)?;
        c::scope(s)?;
        if out.iter().any(|v| v == s) {
            return Err(c::bad());
        }
        out.push(s.to_owned());
    }
    Ok(out)
}
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}
#[derive(Clone, Debug)]
pub struct Lore {
    value: Value,
    index: Vec<Vec<String>>,
}
impl Lore {
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::json::parse(input, 262144)?)
    }
    /// Read once, with the same 262144-byte cap as parsing; no recursive includes or network.
    pub fn read(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let mut input = String::new();
        std::fs::File::open(path)
            .map_err(|e| crate::Error::new(e.to_string()))?
            .take(262145)
            .read_to_string(&mut input)
            .map_err(|e| crate::Error::new(e.to_string()))?;
        Self::parse(&input)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 262144)?;
        c::fields(v, &["version", "entries"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let rows = c::list(c::get(v, "entries")?)?;
        if rows.len() > 512 {
            return Err(c::bad());
        }
        let mut index = Vec::new();
        for (i, e) in rows.iter().enumerate() {
            c::fields(e, &["id", "subject", "topic", "text", "tags", "known_by", "requires"])?;
            for k in ["id", "subject", "topic"] {
                c::scope(text(e, k)?)?;
            }
            if rows[..i].iter().any(|v| v.get("id") == e.get("id")) {
                return Err(c::bad());
            }
            let body = text(e, "text")?;
            if body.is_empty() || body.chars().count() > 1024 || body.chars().any(char::is_control) {
                return Err(c::bad());
            }
            let tags = strings(c::get(e, "tags")?, 16)?;
            let audience = strings(c::get(e, "known_by")?, 16)?;
            strings(c::get(e, "requires")?, 16)?;
            if audience.is_empty() || audience.iter().any(|s| s == "*") && audience.len() != 1 {
                return Err(c::bad());
            }
            let mut combined = [text(e, "id")?, text(e, "subject")?, text(e, "topic")?, body].join(" ");
            for t in tags {
                combined.push(' ');
                combined.push_str(&t);
            }
            index.push(words(&combined));
        }
        Ok(Self { value: v.clone(), index })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    /// `max_bytes` bounds the returned packet array, not receipts or final model framing.
    /// Required records must be authorized, in subject scope and fit both bounds.
    pub fn select(&self, a: &Value) -> Result<Value> {
        bounded(a, 16384)?;
        c::fields(a, &["actor", "grants", "query", "subjects", "required", "limit", "max_bytes"])?;
        let actor = text(a, "actor")?;
        c::scope(actor)?;
        let grants = strings(c::get(a, "grants")?, 64)?;
        let subjects = strings(c::get(a, "subjects")?, 32)?;
        let required = strings(c::get(a, "required")?, 32)?;
        let query = text(a, "query")?;
        if query.len() > 2048 {
            return Err(c::bad());
        }
        let limit = c::uint(c::get(a, "limit")?, 0, 32)? as usize;
        let bytes = c::uint(c::get(a, "max_bytes")?, 2, 32768)? as usize;
        let entries = self.value.get("entries").and_then(Value::as_array).expect("validated entries");
        let visible = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let known = e.get("known_by").and_then(Value::as_array).expect("audience");
                (known.iter().any(|v| v.as_str() == Some("*") || v.as_str() == Some(actor)))
                    && e.get("requires")
                        .and_then(Value::as_array)
                        .expect("requires")
                        .iter()
                        .all(|v| grants.iter().any(|s| v.as_str() == Some(s)))
                    && (subjects.is_empty()
                        || subjects.iter().any(|s| e.get("subject").and_then(Value::as_str) == Some(s)))
            })
            .collect::<Vec<_>>();
        if required.iter().any(|id| !visible.iter().any(|(_, e)| e.get("id").and_then(Value::as_str) == Some(id))) {
            return Err(c::bad());
        }
        let packed = |indices: &[usize]| {
            Value::Array(indices.iter().map(|&i| {let e=&entries[i];obj! {"id"=>e.get("id").expect("id").clone(),"subject"=>e.get("subject").expect("subject").clone(),"topic"=>e.get("topic").expect("topic").clone(),"text"=>e.get("text").expect("text").clone()}}).collect())
        };
        let mut selected = visible
            .iter()
            .filter(|(_, e)| required.iter().any(|id| e.get("id").and_then(Value::as_str) == Some(id)))
            .map(|(i, _)| *i)
            .collect::<Vec<_>>();
        if selected.len() > limit || packed(&selected).json_len() > bytes {
            return Err(c::bad());
        }
        let mut wanted = words(query);
        wanted.sort();
        wanted.dedup();
        let mut scored = visible
            .iter()
            .filter(|(i, _)| !selected.contains(i))
            .map(|(i, _)| (wanted.iter().filter(|w| self.index[*i].contains(w)).count(), *i))
            .collect::<Vec<_>>();
        scored.sort_by_key(|(score, i)| (std::cmp::Reverse(*score), *i));
        for (score, i) in scored {
            if score == 0 {
                continue;
            }
            selected.push(i);
            if selected.len() > limit || packed(&selected).json_len() > bytes {
                selected.pop();
            }
        }
        let packet = packed(&selected);
        let ids = selected.iter().map(|&i| entries[i].get("id").expect("id").clone()).collect::<Vec<_>>();
        let omitted = visible
            .iter()
            .filter(|(i, _)| !selected.contains(i))
            .map(|(_, e)| e.get("id").expect("id").clone())
            .collect::<Vec<_>>();
        Ok(
            obj! {"packet"=>packet.clone(),"receipt"=>obj! {"bytes"=>packet.json_len(),"selected"=>ids,"omitted"=>omitted}},
        )
    }
}
