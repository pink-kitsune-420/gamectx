//! Long-term memory for game AI: facts and events with tags, an importance and the turn
//! they happened, recalled by relevance instead of resending the whole history.
//!
//! ```
//! use gamectx::memory::Memory;
//!
//! let mut memory = Memory::new();
//! memory.remember(3, "The player gave Ilse a salt pouch.", &["player", "gift"], 6)?;
//! memory.remember(9, "An ash hound was seen near the well.", &["danger"], 8)?;
//!
//! // Later: what matters for this message?
//! let recalled = memory.recall("Is the well safe?", 2);
//! assert_eq!(recalled[0].text, "An ash hound was seen near the well.");
//! let section = Memory::table(&recalled);   // add to a turn, e.g. with Turn::var
//! # Ok::<(), gamectx::Error>(())
//! ```
//!
//! The model can write memories through [`Memory::tool`] and [`Memory::apply_call`]; the
//! call is validated like any other model output.

use crate::json::{Error, Result, Value};
use crate::view::words;
mod evidence;
mod journal;
pub use evidence::{EvidenceMemory, EvidenceSnapshot};
pub use journal::{Fact, Journal, MAX_JOURNAL_BYTES};

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub id: u64,
    /// The game's turn (or time) when this was recorded.
    pub turn: u64,
    /// 1 (trivial) to 10 (never forget first).
    pub importance: u8,
    pub text: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub records: usize,
    pub text_chars: usize,
    pub tags: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { records: 1000, text_chars: 280, tags: 8 }
    }
}

#[derive(Clone, Debug)]
pub struct Memory {
    records: Vec<Record>,
    next_id: u64,
    limits: Limits,
}

impl Default for Memory {
    fn default() -> Self {
        Memory::new()
    }
}

fn singular(w: &str) -> Option<String> {
    (w.chars().count() > 3 && w.ends_with('s')).then(|| w[..w.len() - 1].to_owned())
}

impl Memory {
    pub fn new() -> Memory {
        Memory::with_limits(Limits::default())
    }

    pub fn with_limits(limits: Limits) -> Memory {
        Memory { records: Vec::new(), next_id: 1, limits }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Stores a record and returns its id. When memory is full, the least important record
    /// is forgotten first, the oldest among equals.
    pub fn remember(&mut self, turn: u64, text: &str, tags: &[&str], importance: u8) -> Result<u64> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > self.limits.text_chars || text.contains(['\n', '\r']) {
            return Err(Error::new(format!(
                "memory text must be one line of 1 to {} characters",
                self.limits.text_chars
            )));
        }
        if !(1..=10).contains(&importance) {
            return Err(Error::new("memory importance must be 1 to 10"));
        }
        let mut clean: Vec<String> = Vec::new();
        for tag in tags {
            let tag = tag.trim().trim_start_matches('#').to_lowercase();
            if tag.is_empty() || tag.chars().count() > 32 || tag.contains(|c: char| c.is_whitespace() || c == ',') {
                return Err(Error::new("memory tags must be single words of up to 32 characters"));
            }
            if !clean.contains(&tag) {
                clean.push(tag);
            }
        }
        if clean.len() > self.limits.tags {
            return Err(Error::new(format!("at most {} memory tags", self.limits.tags)));
        }
        if self.limits.records == 0 {
            return Err(Error::new("memory holds no records"));
        }
        while self.records.len() >= self.limits.records {
            let victim = (0..self.records.len())
                .min_by_key(|&i| (self.records[i].importance, self.records[i].turn, self.records[i].id))
                .expect("memory is not empty");
            self.records.remove(victim);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.records.push(Record { id, turn, importance, text: text.to_owned(), tags: clean });
        Ok(id)
    }

    /// Removes a record; returns whether it existed.
    pub fn forget(&mut self, id: u64) -> bool {
        let before = self.records.len();
        self.records.retain(|r| r.id != id);
        self.records.len() != before
    }

    /// The `count` records most relevant to `text`. Each word of `text` found in a record's
    /// text or tags (a trailing plural `s` optional) adds `1 / n`, where `n` is how many
    /// records contain that word, so rare words count fully and words in nearly every
    /// record (such as "the") barely count, in any language. Ties go to the more
    /// important, then the more recent record. With no matching words this returns the most
    /// important recent records, so a vague message still recalls what matters.
    pub fn recall(&self, text: &str, count: usize) -> Vec<&Record> {
        recall_records(self.records.iter(), text, count)
    }

    /// Recalled records as a compact table for a packet, oldest first so events read in order.
    pub fn table(records: &[&Record]) -> Value {
        let mut sorted: Vec<&&Record> = records.iter().collect();
        sorted.sort_by_key(|r| (r.turn, r.id));
        let rows = sorted
            .iter()
            .map(|r| {
                Value::Array(vec![Value::from(r.id), Value::from(r.turn), Value::str(&r.text), Value::strings(&r.tags)])
            })
            .collect();
        Value::object(vec![("columns", Value::strings(&["id", "turn", "text", "tags"])), ("rows", Value::Array(rows))])
    }

    /// A `remember` tool for the model, in the same format as other tools (strict, so it
    /// packs as signatures). Register it with your other tools.
    pub fn tool(&self) -> Value {
        crate::obj! {
            "type" => "function",
            "function" => crate::obj! {
                "name" => "remember",
                "description" => "Save one fact or event worth recalling in later turns.",
                "strict" => true,
                "parameters" => crate::obj! {
                    "type" => "object",
                    "properties" => crate::obj! {
                        "text" => crate::obj! {"type" => "string", "maxLength" => self.limits.text_chars, "description" => "One short sentence."},
                        "importance" => crate::obj! {"type" => "integer", "minimum" => 1, "maximum" => 10, "description" => "10 = never forget first."},
                        "tags" => crate::obj! {"type" => "string", "description" => "Comma-separated single words, e.g. player,gift."},
                    },
                    "required" => vec!["text", "importance", "tags"],
                    "additionalProperties" => false,
                },
            },
        }
    }

    /// Validates the arguments of a model's `remember` call and stores the record.
    pub fn apply_call(&mut self, turn: u64, args: &Value) -> Result<u64> {
        let keys: Vec<&str> = args.as_object().map(|o| o.iter().map(|(k, _)| k.as_str()).collect()).unwrap_or_default();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        if sorted != ["importance", "tags", "text"] {
            return Err(Error::new("remember needs exactly text, importance and tags"));
        }
        let text =
            args.get("text").and_then(Value::as_str).ok_or_else(|| Error::new("remember text must be a string"))?;
        let importance = args
            .get("importance")
            .and_then(Value::as_usize)
            .filter(|i| (1..=10).contains(i))
            .ok_or_else(|| Error::new("remember importance must be an integer from 1 to 10"))?;
        let tags =
            args.get("tags").and_then(Value::as_str).ok_or_else(|| Error::new("remember tags must be a string"))?;
        let tags: Vec<&str> = tags.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
        self.remember(turn, text, &tags, importance as u8)
    }

    /// Saves memory as JSON: `{"next_id":n,"records":[{"id","turn","importance","text","tags"}]}`.
    pub fn to_value(&self) -> Value {
        let records = self
            .records
            .iter()
            .map(|r| {
                crate::obj! {
                    "id" => r.id, "turn" => r.turn, "importance" => r.importance as u32,
                    "text" => r.text.as_str(), "tags" => Value::strings(&r.tags),
                }
            })
            .collect();
        crate::obj! {"next_id" => self.next_id, "records" => Value::Array(records)}
    }

    /// Loads memory saved by [`Memory::to_value`], revalidating every record.
    pub fn from_value(value: &Value, limits: Limits) -> Result<Memory> {
        let bad = |what: &str| Error::new(format!("saved memory: {what}"));
        let next_id =
            value.get("next_id").and_then(Value::as_usize).ok_or_else(|| bad("next_id must be an integer"))?;
        let list = value.get("records").and_then(Value::as_array).ok_or_else(|| bad("records must be an array"))?;
        if list.len() > limits.records {
            return Err(bad("more records than the limit"));
        }
        let mut memory = Memory::with_limits(limits);
        for item in list {
            let num =
                |k: &str| item.get(k).and_then(Value::as_usize).ok_or_else(|| bad(&format!("{k} must be an integer")));
            let (id, turn, importance) = (num("id")? as u64, num("turn")? as u64, num("importance")?);
            if importance > 10 || id == 0 || id >= next_id as u64 || memory.records.iter().any(|r| r.id == id) {
                return Err(bad("record ids must be unique and below next_id; importance 1 to 10"));
            }
            let text = item.get("text").and_then(Value::as_str).ok_or_else(|| bad("text must be a string"))?;
            let tags: Vec<&str> = item
                .get("tags")
                .and_then(Value::as_array)
                .and_then(|a| a.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
                .ok_or_else(|| bad("tags must be strings"))?;
            memory.next_id = id;
            memory.remember(turn, text, &tags, importance as u8)?;
        }
        memory.next_id = next_id as u64;
        Ok(memory)
    }
}

fn recall_records<'a>(records: impl IntoIterator<Item = &'a Record>, text: &str, count: usize) -> Vec<&'a Record> {
    let records: Vec<_> = records.into_iter().collect();
    let wanted = words(text);
    let parts: Vec<Vec<String>> = records
        .iter()
        .copied()
        .map(|r| {
            let mut p = words(&r.text);
            p.extend(r.tags.iter().cloned());
            let extra: Vec<String> = p.iter().filter_map(|w| singular(w)).collect();
            p.extend(extra);
            p
        })
        .collect();
    let weight: Vec<f64> = wanted
        .iter()
        .map(|w| {
            let n = parts.iter().filter(|p| p.contains(w)).count();
            if n == 0 {
                0.0
            } else {
                1.0 / n as f64
            }
        })
        .collect();
    let mut scored: Vec<(f64, &Record)> = records
        .iter()
        .copied()
        .zip(&parts)
        .map(|(r, p)| (wanted.iter().zip(&weight).filter(|(w, _)| p.contains(w)).map(|(_, x)| x).sum(), r))
        .collect();
    scored.sort_by(|(sa, a), (sb, b)| {
        sb.total_cmp(sa).then((b.importance, b.turn, b.id).cmp(&(a.importance, a.turn, a.id)))
    });
    scored.into_iter().take(count).map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    fn sample() -> Memory {
        let mut m = Memory::new();
        m.remember(1, "The player gave Ilse a salt pouch.", &["player", "gift"], 6).unwrap();
        m.remember(2, "Rain all morning.", &["weather"], 2).unwrap();
        m.remember(9, "An ash hound was seen near the well.", &["#Danger"], 8).unwrap();
        m.remember(12, "The player broke a lantern in the hall.", &["player"], 5).unwrap();
        m
    }

    fn texts(records: Vec<&Record>) -> Vec<String> {
        records.into_iter().map(|r| r.text.clone()).collect()
    }

    #[test]
    fn recalls_by_words_then_importance_then_recency() {
        let m = sample();
        assert_eq!(texts(m.recall("is the well safe?", 1)), ["An ash hound was seen near the well."]);
        assert_eq!(
            texts(m.recall("what did the player do?", 2)),
            ["The player gave Ilse a salt pouch.", "The player broke a lantern in the hall."],
            "both match 'player' and 'the'; the more important comes first"
        );
        assert_eq!(
            texts(m.recall("any dangers?", 1)),
            ["An ash hound was seen near the well."],
            "tags match, plural optional"
        );
        assert_eq!(texts(m.recall("zzz", 1)), ["An ash hound was seen near the well."], "no match: most important");
        assert_eq!(texts(m.recall("the rain", 1)), ["Rain all morning."], "rare words outweigh common ones");
    }

    #[test]
    fn evicts_least_important_oldest_first() {
        let mut m = Memory::with_limits(Limits { records: 3, ..Limits::default() });
        m.remember(1, "a", &[], 5).unwrap();
        m.remember(2, "b", &[], 2).unwrap();
        m.remember(3, "c", &[], 2).unwrap();
        m.remember(4, "d", &[], 9).unwrap();
        let left: Vec<&str> = m.records().iter().map(|r| r.text.as_str()).collect();
        assert_eq!(left, ["a", "c", "d"]);
    }

    #[test]
    fn validates_model_calls_and_round_trips() {
        let mut m = sample();
        let ok = parse(r#"{"text":"Ilse owes the smith a favor.","importance":7,"tags":"smith, debt"}"#, 1024).unwrap();
        let id = m.apply_call(15, &ok).unwrap();
        assert_eq!(m.records().last().unwrap().tags, ["smith", "debt"]);
        for bad in [
            r#"{"text":"x","importance":11,"tags":""}"#,
            r#"{"text":"","importance":3,"tags":""}"#,
            r#"{"text":"x","importance":3}"#,
            r#"{"text":"x","importance":3,"tags":"","extra":1}"#,
            r#"{"text":"two\nlines","importance":3,"tags":""}"#,
            r#"{"text":"x","importance":3,"tags":"two words"}"#,
        ] {
            assert!(m.apply_call(1, &parse(bad, 1024).unwrap()).is_err(), "{bad}");
        }
        let saved = m.to_value();
        let loaded = Memory::from_value(&parse(&saved.to_json(), 1 << 16).unwrap(), Limits::default()).unwrap();
        assert_eq!(loaded.records(), m.records());
        assert_eq!(loaded.to_value(), saved);
        let mut loaded = loaded;
        assert!(loaded.forget(id) && !loaded.forget(id));
        assert!(crate::signatures::pack(&[m.tool()]).is_ok(), "the remember tool packs as signatures");
    }

    #[test]
    fn table_lists_recalled_records_in_time_order() {
        let m = sample();
        let recalled = m.recall("player", 2);
        assert_eq!(
            Memory::table(&recalled).to_json(),
            r#"{"columns":["id","turn","text","tags"],"rows":[[1,1,"The player gave Ilse a salt pouch.",["player","gift"]],[4,12,"The player broke a lantern in the hall.",["player"]]]}"#
        );
    }
}
