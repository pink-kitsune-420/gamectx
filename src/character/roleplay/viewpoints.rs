//! Bounded subjective stances. Delivery confirms an utterance, never a world fact.
use crate::{checked as c, obj, Result, Value};
/// Mechanical constraints for an explicitly subjective, first-person sentence.
/// This is not a semantic truth checker; hosts must keep world authority separate.
pub fn validate_viewpoint_line(line: &str) -> Result<()> {
    if line.is_empty() || line.len() > 1024 || line.chars().count() > 240 || line.chars().any(char::is_control) {
        return Err(c::bad());
    }
    let lower = line.to_lowercase();
    if ![
        "i hope ",
        "i dream ",
        "i wish ",
        "i prefer ",
        "i feel ",
        "i value ",
        "i think ",
        "i believe ",
        "i would like ",
    ]
    .iter()
    .any(|p| lower.starts_with(p))
        || !line.ends_with(['.', '!'])
        || line[..line.len() - 1].contains(['.', '!', '?'])
        || [
            "was born",
            "were born",
            "used to",
            "yesterday",
            "years ago",
            "i own ",
            "i have ",
            "i promise",
            "will give",
            "stock",
            "weather",
            "sunny",
            "raining",
            "copper sparrow",
        ]
        .iter()
        .any(|p| lower.contains(p))
    {
        return Err(c::bad());
    }
    Ok(())
}
/// Immutable stances keyed by actor, listener and topic. Hosts own topic selection,
/// provenance and delivery receipts. A new checkpoint starts a different branch.
#[derive(Clone, Debug)]
pub struct Viewpoints {
    entries: Vec<Value>,
}
impl Default for Viewpoints {
    fn default() -> Self {
        Self::new()
    }
}
impl Viewpoints {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }
    pub fn recover(v: &Value) -> Result<Self> {
        if !v.is_valid(256 * 1024) {
            return Err(c::bad());
        }
        c::fields(v, &["version", "entries"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let entries = c::list(c::get(v, "entries")?)?;
        if entries.len() > 128 {
            return Err(c::bad());
        }
        let mut out = Self::new();
        for e in entries {
            validate(e)?;
            if out
                .find(c::text(c::get(e, "actor")?)?, c::text(c::get(e, "listener")?)?, c::text(c::get(e, "topic")?)?)
                .is_some()
            {
                return Err(c::bad());
            }
            out.entries.push(e.clone());
        }
        Ok(out)
    }
    pub fn find(&self, actor: &str, listener: &str, topic: &str) -> Option<&Value> {
        self.entries.iter().find(|v| {
            v.get("actor").and_then(Value::as_str) == Some(actor)
                && v.get("listener").and_then(Value::as_str) == Some(listener)
                && v.get("topic").and_then(Value::as_str) == Some(topic)
        })
    }
    /// `heard` must come from the host's complete delivery receipt, not the model.
    pub fn confirm(&mut self, entry: &Value, heard: &str) -> Result<Value> {
        if !entry.is_valid(4096) {
            return Err(c::bad());
        }
        validate(entry)?;
        if c::text(c::get(entry, "stance")?)? != heard {
            return Err(c::bad());
        }
        if let Some(old) = self.find(
            c::text(c::get(entry, "actor")?)?,
            c::text(c::get(entry, "listener")?)?,
            c::text(c::get(entry, "topic")?)?,
        ) {
            if old != entry {
                return Err(c::bad());
            }
        } else {
            if self.entries.len() >= 128 {
                return Err(c::bad());
            }
            self.entries.push(entry.clone());
        }
        Ok(obj! {"stored"=>true,"count"=>self.entries.len()})
    }
    pub fn checkpoint(&self) -> Value {
        obj! {"version"=>1,"entries"=>self.entries.clone()}
    }
}
fn validate(v: &Value) -> Result<()> {
    c::fields(v, &["actor", "listener", "topic", "stance", "basis", "source", "at"])?;
    for k in ["actor", "listener", "topic"] {
        let s = c::text(c::get(v, k)?)?;
        if s.is_empty() || s.len() > 64 || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:-".contains(&b)) {
            return Err(c::bad());
        }
    }
    if v.get("actor") == v.get("listener") {
        return Err(c::bad());
    }
    validate_viewpoint_line(c::text(c::get(v, "stance")?)?)?;
    let basis = c::text(c::get(v, "basis")?)?;
    if basis.len() != 64
        || !basis.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !matches!(c::text(c::get(v, "source")?)?, "profile" | "model")
    {
        return Err(c::bad());
    }
    c::uint(c::get(v, "at")?, 0, u32::MAX as u64)?;
    Ok(())
}
pub(crate) fn fixture(v: &Value) -> Result<Value> {
    c::fields(v, &["state", "ops"])?;
    let mut views = Viewpoints::recover(c::get(v, "state")?)?;
    let ops = c::list(c::get(v, "ops")?)?;
    if ops.len() > 128 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        c::fields(op, &["entry", "heard"])?;
        results.push(views.confirm(c::get(op, "entry")?, c::text(c::get(op, "heard")?)?)?);
    }
    Ok(obj! {"results"=>results,"state"=>views.checkpoint()})
}
