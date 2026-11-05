//! Assembles named sections into one JSON packet that fits a budget.
//!
//! Stable sections (schemas, catalogs, rules) come first, in declared order, then the
//! per-request sections. Keeping the start of every request identical lets providers that
//! cache prompt prefixes reuse it. When the packet is too large, the least important
//! optional section is dropped (ties: the one declared last), and the receipt says so.

use crate::json::{Error, Result, Value};

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    /// Higher is more important; lower priorities are dropped first.
    pub priority: i64,
    /// Rarely changes between requests; placed first so the prefix stays cacheable.
    pub stable: bool,
    /// Never dropped; assembly fails instead.
    pub required: bool,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub name: String,
    pub bytes: usize,
    pub priority: i64,
    pub stable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub included: Vec<Line>,
    pub dropped: Vec<Line>,
    pub total_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct Packet {
    pub value: Value,
    pub text: String,
    pub receipt: Receipt,
}

/// Fits sections into `max_bytes` of compact JSON.
pub fn assemble_bytes(sections: Vec<Section>, max_bytes: usize) -> Result<Packet> {
    assemble(sections, &|text: &str| text.len() <= max_bytes)
}

/// Fits sections using any measure, such as a calibrated token estimate.
pub fn assemble(sections: Vec<Section>, fits: &dyn Fn(&str) -> bool) -> Result<Packet> {
    for (i, s) in sections.iter().enumerate() {
        if sections[..i].iter().any(|o| o.name == s.name) {
            return Err(Error::new(format!("duplicate section \"{}\"", s.name)));
        }
    }
    // Stable first, keeping declared order inside each group.
    let mut ordered: Vec<(usize, Section)> = sections.into_iter().enumerate().collect();
    ordered.sort_by_key(|(i, s)| (!s.stable, *i));

    let mut dropped = Vec::new();
    loop {
        let value = Value::Object(ordered.iter().map(|(_, s)| (s.name.clone(), s.value.clone())).collect());
        let text = value.to_json();
        if fits(&text) {
            let included = ordered.iter().map(|(_, s)| line(s)).collect();
            let total_bytes = text.len();
            return Ok(Packet { value, text, receipt: Receipt { included, dropped, total_bytes } });
        }
        let victim = ordered
            .iter()
            .enumerate()
            .filter(|(_, (_, s))| !s.required)
            .min_by_key(|(_, (declared, s))| (s.priority, std::cmp::Reverse(*declared)))
            .map(|(pos, _)| pos);
        match victim {
            Some(pos) => dropped.push(line(&ordered.remove(pos).1)),
            None => {
                let names: Vec<String> =
                    ordered.iter().map(|(_, s)| format!("{} ({} bytes)", s.name, s.value.json_len())).collect();
                return Err(Error::new(format!("required sections alone exceed the budget: {}", names.join(", "))));
            }
        }
    }
}

fn line(s: &Section) -> Line {
    Line { name: s.name.clone(), bytes: s.value.json_len(), priority: s.priority, stable: s.stable }
}

impl Packet {
    /// The packet as the end of a system prompt: `intro`, a blank line, `GAME DATA:` and
    /// the packet. Keep `intro` identical every turn so the whole prefix stays cacheable.
    pub fn system_prompt(&self, intro: &str) -> String {
        format!("{intro}\n\nGAME DATA:\n{}", self.text)
    }
}

impl Receipt {
    pub fn to_value(&self) -> Value {
        let lines = |ls: &[Line]| {
            Value::Array(
                ls.iter()
                    .map(|l| {
                        Value::Array(vec![
                            Value::str(&l.name),
                            Value::int(l.bytes as i64),
                            Value::int(l.priority),
                            Value::Bool(l.stable),
                        ])
                    })
                    .collect(),
            )
        };
        Value::object(vec![
            ("columns", Value::strings(&["name", "bytes", "priority", "stable"])),
            ("included", lines(&self.included)),
            ("dropped", lines(&self.dropped)),
            ("total_bytes", Value::int(self.total_bytes as i64)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(name: &str, priority: i64, stable: bool, required: bool, size: usize) -> Section {
        Section { name: name.into(), priority, stable, required, value: Value::str("x".repeat(size)) }
    }

    #[test]
    fn stable_sections_come_first() {
        let p = assemble_bytes(vec![section("live", 5, false, false, 1), section("rules", 5, true, false, 1)], 1000)
            .unwrap();
        assert_eq!(p.text, r#"{"rules":"x","live":"x"}"#);
    }

    #[test]
    fn drops_lowest_priority_then_latest_declared() {
        let sections = vec![
            section("tools", 10, true, true, 50),
            section("lore", 1, true, false, 50),
            section("events", 5, false, false, 50),
            section("chatter", 1, false, false, 50),
        ];
        let p = assemble_bytes(sections, 140).unwrap();
        let dropped: Vec<&str> = p.receipt.dropped.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(dropped, ["chatter", "lore"]);
        assert!(p.receipt.total_bytes <= 140);
    }

    #[test]
    fn fails_when_required_sections_cannot_fit() {
        assert!(assemble_bytes(vec![section("tools", 10, true, true, 500)], 100).is_err());
        assert!(assemble_bytes(vec![section("a", 1, true, false, 1), section("a", 1, true, false, 1)], 100).is_err());
    }
}
