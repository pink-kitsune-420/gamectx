//! Host-authored stage direction aliases. Matching proposes; it never executes.
use crate::{checked as c, obj, Result, Value};
fn line(v: &Value, max: usize) -> Result<&str> {
    let s = c::text(v)?;
    if s.is_empty() || s.len() > max || s.chars().any(char::is_control) {
        return Err(c::bad());
    }
    Ok(s)
}
fn ident(v: &Value) -> Result<()> {
    let s = line(v, 64)?;
    if !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:-".contains(&b)) {
        return Err(c::bad());
    }
    Ok(())
}
fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn decision(v: &Value) -> Result<()> {
    if v == &Value::Null {
        return Ok(());
    }
    c::fields(v, &["action", "target"])?;
    ident(c::get(v, "action")?)?;
    if c::get(v, "target")? != &Value::Null {
        ident(c::get(v, "target")?)?;
    }
    Ok(())
}
/// A bounded registry may describe custom emotes or map phrases onto existing
/// host affordances. It does not grant capabilities or record experiences.
#[derive(Clone, Debug)]
pub struct RoleplayActions {
    value: Value,
    aliases: Vec<(String, usize)>,
}
impl RoleplayActions {
    pub fn from_value(v: &Value) -> Result<Self> {
        crate::request::bounded(v, 65536)?;
        c::fields(v, &["version", "entries"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let entries = c::list(c::get(v, "entries")?)?;
        if entries.len() > 128 {
            return Err(c::bad());
        }
        let mut aliases = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            c::fields(e, &["id", "phrases", "decision", "narration"])?;
            ident(c::get(e, "id")?)?;
            if entries[..i].iter().any(|p| p.get("id") == e.get("id")) {
                return Err(c::bad());
            }
            line(c::get(e, "narration")?, 240)?;
            decision(c::get(e, "decision")?)?;
            let phrases = c::list(c::get(e, "phrases")?)?;
            if phrases.is_empty() || phrases.len() > 8 {
                return Err(c::bad());
            }
            for p in phrases {
                let key = normalize(line(p, 128)?);
                if key.is_empty() || aliases.iter().any(|(old, _)| *old == key) {
                    return Err(c::bad());
                }
                aliases.push((key, i));
            }
        }
        Ok(Self { value: v.clone(), aliases })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn resolve(&self, input: &str, choices: &Value) -> Result<Value> {
        crate::request::bounded(choices, 32768)?;
        line(&Value::str(input), 1024)?;
        let input = input.trim();
        let choices = c::list(choices)?;
        if choices.len() > 128 {
            return Err(c::bad());
        }
        for d in choices {
            if d == &Value::Null {
                return Err(c::bad());
            }
            decision(d)?;
        }
        if input.len() < 3 || !input.starts_with('*') || !input.ends_with('*') {
            return Ok(obj! {"status"=>"not_stage"});
        }
        let key = normalize(&input[1..input.len() - 1]);
        let Some((_, i)) = self.aliases.iter().find(|(p, _)| *p == key) else {
            return Ok(obj! {"status"=>"unknown"});
        };
        let e = &c::list(c::get(&self.value, "entries")?)?[*i];
        let d = c::get(e, "decision")?;
        if d != &Value::Null && !choices.contains(d) {
            return Ok(obj! {"status"=>"blocked","id"=>c::get(e,"id")?.clone(),"reason"=>"unavailable_host_action"});
        }
        Ok(
            obj! {"status"=>"proposed","id"=>c::get(e,"id")?.clone(),"narration"=>c::get(e,"narration")?.clone(),"decision"=>d.clone()},
        )
    }
    /// Authored narration is safe to use only after this decision succeeds in the host.
    pub fn narration(&self, d: &Value) -> Option<&str> {
        self.value
            .get("entries")?
            .as_array()?
            .iter()
            .find(|e| e.get("decision") == Some(d))
            .and_then(|e| e.get("narration"))
            .and_then(Value::as_str)
    }
}
pub(crate) fn fixture(v: &Value) -> Result<Value> {
    c::fields(v, &["registry", "input", "choices"])?;
    RoleplayActions::from_value(c::get(v, "registry")?)?.resolve(c::text(c::get(v, "input")?)?, c::get(v, "choices")?)
}
