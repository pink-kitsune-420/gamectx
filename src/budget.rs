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

/// A section serialized once for repeated assembly. Fields are private so bytes cannot
/// become stale when a source value changes. Rebuild it to change content/metadata.
#[derive(Clone, Debug)]
pub struct PreparedSection {
    section: Section,
    member: String,
    value_bytes: usize,
}

impl PreparedSection {
    pub fn new(section: Section) -> Self {
        let value_bytes = section.value.json_len();
        let mut member = String::with_capacity(crate::json::string_len(&section.name) + 1 + value_bytes);
        crate::json::write_string(&section.name, &mut member);
        member.push(':');
        section.value.write(&mut member);
        Self { section, member, value_bytes }
    }
    pub fn section(&self) -> &Section {
        &self.section
    }
    fn line(&self) -> Line {
        Line {
            name: self.section.name.clone(),
            bytes: self.value_bytes,
            priority: self.section.priority,
            stable: self.section.stable,
        }
    }
}

/// Exact byte-budget fitting. Serializes each member once, counts removals, builds once.
pub fn assemble_bytes(mut sections: Vec<Section>, max_bytes: usize) -> Result<Packet> {
    for (i, section) in sections.iter().enumerate() {
        if sections[..i].iter().any(|old| old.name == section.name) {
            return Err(Error::new(format!("duplicate section \"{}\"", section.name)));
        }
    }
    let size = 2
        + sections.len().saturating_sub(1)
        + sections.iter().map(|s| crate::json::string_len(&s.name) + 1 + s.value.json_len()).sum::<usize>();
    // A fitting one-shot packet needs no retained member strings or value clones.
    if size <= max_bytes {
        sections.sort_by_key(|s| !s.stable);
        let included = sections
            .iter()
            .map(|s| Line { name: s.name.clone(), bytes: s.value.json_len(), priority: s.priority, stable: s.stable })
            .collect();
        let value = Value::Object(sections.into_iter().map(|s| (s.name, s.value)).collect());
        let text = value.to_json();
        return Ok(Packet { value, text, receipt: Receipt { included, dropped: Vec::new(), total_bytes: size } });
    }
    let prepared: Vec<_> = sections.into_iter().map(PreparedSection::new).collect();
    assemble_prepared_bytes(&prepared.iter().collect::<Vec<_>>(), max_bytes)
}

/// Reuses previously serialized sections, including stable tools/rules across turns.
pub fn assemble_prepared_bytes(sections: &[&PreparedSection], max_bytes: usize) -> Result<Packet> {
    assemble_prepared_inner(sections, Some(max_bytes), &|_| true)
}

/// Arbitrary fitting predicates still receive the complete candidate on every attempt.
/// Their result may depend on content/order and is never assumed additive or monotonic.
pub fn assemble(sections: Vec<Section>, fits: &dyn Fn(&str) -> bool) -> Result<Packet> {
    let prepared: Vec<_> = sections.into_iter().map(PreparedSection::new).collect();
    assemble_prepared(&prepared.iter().collect::<Vec<_>>(), fits)
}

pub fn assemble_prepared(sections: &[&PreparedSection], fits: &dyn Fn(&str) -> bool) -> Result<Packet> {
    assemble_prepared_inner(sections, None, fits)
}

fn assemble_prepared_inner(
    sections: &[&PreparedSection],
    max_bytes: Option<usize>,
    fits: &dyn Fn(&str) -> bool,
) -> Result<Packet> {
    for (i, s) in sections.iter().enumerate() {
        if sections[..i].iter().any(|old| old.section.name == s.section.name) {
            return Err(Error::new(format!("duplicate section \"{}\"", s.section.name)));
        }
    }
    let mut ordered: Vec<_> = sections.iter().copied().enumerate().collect();
    ordered.sort_by_key(|(i, s)| (!s.section.stable, *i));
    let mut dropped = Vec::new();
    let encode = |items: &[(usize, &PreparedSection)]| {
        let size = 2 + items.iter().map(|(_, s)| s.member.len()).sum::<usize>() + items.len().saturating_sub(1);
        let mut text = String::with_capacity(size);
        text.push('{');
        for (i, (_, s)) in items.iter().enumerate() {
            if i != 0 {
                text.push(',');
            }
            text.push_str(&s.member);
        }
        text.push('}');
        text
    };
    let mut size = 2 + ordered.iter().map(|(_, s)| s.member.len()).sum::<usize>() + ordered.len().saturating_sub(1);
    loop {
        let text = if let Some(cap) = max_bytes {
            (size <= cap).then(|| encode(&ordered))
        } else {
            let text = encode(&ordered);
            fits(&text).then_some(text)
        };
        if let Some(text) = text {
            let value =
                Value::Object(ordered.iter().map(|(_, s)| (s.section.name.clone(), s.section.value.clone())).collect());
            let included = ordered.iter().map(|(_, s)| s.line()).collect();
            return Ok(Packet { value, text, receipt: Receipt { included, dropped, total_bytes: size } });
        }
        let victim = ordered
            .iter()
            .enumerate()
            .filter(|(_, (_, s))| !s.section.required)
            .min_by_key(|(_, (i, s))| (s.section.priority, std::cmp::Reverse(*i)))
            .map(|(pos, _)| pos);
        if let Some(pos) = victim {
            let (_, s) = ordered.remove(pos);
            size -= s.member.len() + usize::from(!ordered.is_empty());
            dropped.push(s.line());
        } else {
            let names: Vec<_> =
                ordered.iter().map(|(_, s)| format!("{} ({} bytes)", s.section.name, s.value_bytes)).collect();
            return Err(Error::new(format!("required sections alone exceed the budget: {}", names.join(", "))));
        }
    }
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
