//! Host-owned props: described objects that a location can host, each with registered
//! interactions. Props are read-only world content. Interactions only *propose* a host
//! decision (or a pure gesture with no decision); the host validates and executes.
//!
//! ```
//! use gamectx::knowledge::props::Props;
//! let props = Props::parse(r#"{"version":1,"props":[{"id":"bell","label":"the brass bell",
//!   "aliases":["bell"],"description":"A small brass bell hangs above the door.",
//!   "interactions":[{"id":"ring","phrases":["rings the bell","ring the bell"],
//!   "decision":{"action":"use","target":"prop:bell"},"narration":"rings the brass bell"}]}]}"#)?;
//! assert_eq!(props.label("bell"), Some("the brass bell"));
//! let (prop, interaction) = props.interaction(&["bell"], "Ring the bell!").unwrap();
//! assert_eq!((prop, interaction.get("id").and_then(|v| v.as_str())), ("bell", Some("ring")));
//! # Ok::<(), gamectx::Error>(())
//! ```
use crate::{checked as c, Result, Value};

pub(crate) fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
pub(crate) fn line(v: &Value, chars: usize, bytes: usize) -> Result<&str> {
    let s = c::text(v)?;
    if s.is_empty() || s.chars().count() > chars || s.len() > bytes || s.chars().any(char::is_control) {
        return Err(c::bad());
    }
    Ok(s)
}
pub(crate) fn ident(v: &Value) -> Result<&str> {
    let s = c::text(v)?;
    if s.is_empty() || s.len() > 64 || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:-".contains(&b)) {
        return Err(c::bad());
    }
    Ok(s)
}
pub(crate) fn decision(v: &Value) -> Result<()> {
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

/// A validated prop registry (`{"version":1,"props":[...]}`, at most 128 props).
///
/// Each prop has an `id`, a sentence-ready `label` (≤48 chars), up to 8 `aliases`, a
/// speakable `description` (≤240 chars) and up to 8 `interactions` of
/// `{id, phrases (1–8), decision ({action,target} or null), narration (≤240 chars)}`.
/// Normalized names and interaction phrases are unique across the whole registry.
#[derive(Clone, Debug)]
pub struct Props {
    value: Value,
    names: Vec<(String, usize)>,
    phrases: Vec<(String, usize, usize)>,
}
impl Default for Props {
    fn default() -> Self {
        Self { value: crate::obj! {"version"=>1,"props"=>Vec::<Value>::new()}, names: Vec::new(), phrases: Vec::new() }
    }
}
impl Props {
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(input, 1_048_576)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        crate::packet::request::bounded(v, 1_048_576)?;
        c::fields(v, &["version", "props"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let props = c::list(c::get(v, "props")?)?;
        if props.len() > 128 {
            return Err(c::bad());
        }
        let (mut names, mut phrases) = (Vec::<(String, usize)>::new(), Vec::<(String, usize, usize)>::new());
        for (i, p) in props.iter().enumerate() {
            crate::knowledge::codex::keys(
                p,
                &["id", "label", "aliases", "description", "interactions"],
                &["dimensions", "mass", "state", "material", "color", "texture", "tags", "attributes"],
            )?;
            ident(c::get(p, "id")?)?;
            if props[..i].iter().any(|old| old.get("id") == p.get("id")) {
                return Err(c::bad());
            }
            if let Some(d) = p.get("dimensions").filter(|x| *x != &Value::Null) {
                crate::character::sheet::dimensions_val(d)?;
            }
            crate::character::sheet::validate_matter(p)?;
            if let Some(tg) = p.get("tags").filter(|x| *x != &Value::Null) {
                crate::character::sheet::validate_tags(tg)?;
            }
            if let Some(att) = p.get("attributes").filter(|x| *x != &Value::Null) {
                crate::character::sheet::validate_attributes(att)?;
            }
            line(c::get(p, "description")?, 240, usize::MAX)?;
            let aliases = c::list(c::get(p, "aliases")?)?;
            if aliases.len() > 8 {
                return Err(c::bad());
            }
            for n in std::iter::once(c::get(p, "label")?).chain(aliases) {
                let key = normalize(line(n, 48, usize::MAX)?);
                if key.is_empty() || names.iter().any(|(old, _)| *old == key) {
                    return Err(c::bad());
                }
                names.push((key, i));
            }
            let interactions = c::list(c::get(p, "interactions")?)?;
            if interactions.len() > 8 {
                return Err(c::bad());
            }
            for (j, x) in interactions.iter().enumerate() {
                c::fields(x, &["id", "phrases", "decision", "narration"])?;
                ident(c::get(x, "id")?)?;
                if interactions[..j].iter().any(|old| old.get("id") == x.get("id")) {
                    return Err(c::bad());
                }
                decision(c::get(x, "decision")?)?;
                line(c::get(x, "narration")?, 240, usize::MAX)?;
                let list = c::list(c::get(x, "phrases")?)?;
                if list.is_empty() || list.len() > 8 {
                    return Err(c::bad());
                }
                for phrase in list {
                    let key = normalize(line(phrase, 128, 128)?);
                    if key.is_empty() || phrases.iter().any(|(old, _, _)| *old == key) {
                        return Err(c::bad());
                    }
                    phrases.push((key, i, j));
                }
            }
        }
        Ok(Self { value: v.clone(), names, phrases })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    fn all(&self) -> &[Value] {
        self.value.get("props").and_then(Value::as_array).unwrap_or(&[])
    }
    /// The prop definition with this id.
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.all().iter().find(|p| p.get("id").and_then(Value::as_str) == Some(id))
    }
    pub fn label(&self, id: &str) -> Option<&str> {
        self.get(id)?.get("label")?.as_str()
    }
    pub fn description(&self, id: &str) -> Option<&str> {
        self.get(id)?.get("description")?.as_str()
    }
    /// The normalized label and aliases of a prop.
    pub fn names(&self, id: &str) -> Vec<&str> {
        let Some(i) = self.all().iter().position(|p| p.get("id").and_then(Value::as_str) == Some(id)) else {
            return Vec::new();
        };
        self.names.iter().filter(|(_, n)| *n == i).map(|(k, _)| k.as_str()).collect()
    }
    /// The interaction whose phrase equals `input` (normalized; one surrounding `*stage*`
    /// pair is ignored) among the `present` props, as `(prop id, interaction)`.
    pub fn interaction<'a>(&'a self, present: &[&str], input: &str) -> Option<(&'a str, &'a Value)> {
        let s = input.trim();
        let s = if s.chars().count() >= 3 && s.starts_with('*') && s.ends_with('*') { &s[1..s.len() - 1] } else { s };
        let key = normalize(s);
        let (_, i, j) = self.phrases.iter().find(|(p, _, _)| *p == key)?;
        let prop = &self.all()[*i];
        let id = prop.get("id")?.as_str()?;
        if !present.contains(&id) {
            return None;
        }
        Some((id, &prop.get("interactions")?.as_array()?[*j]))
    }
}
