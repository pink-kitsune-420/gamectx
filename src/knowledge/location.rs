//! Optional read-only location/scene lookup. Hosts bind current location, revision,
//! visibility and descriptions; models never set a position or create scene facts.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
fn body(s: &str, n: usize) -> Result<()> {
    if s.is_empty() || s.chars().count() > n || s.chars().any(char::is_control) {
        Err(c::bad())
    } else {
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Locations {
    revision: u32,
    places: Vec<Value>,
}
impl Locations {
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(input, 1_048_576)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 1_048_576)?;
        c::fields(v, &["version", "revision", "places"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let revision = c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)? as u32;
        let places = c::list(c::get(v, "places")?)?;
        if places.len() > 128 {
            return Err(c::bad());
        }
        for (i, p) in places.iter().enumerate() {
            c::fields(p, &["id", "label", "summary", "description", "details"])?;
            c::scope(text(p, "id")?)?;
            c::scope(text(p, "label")?)?;
            for (key, max) in [("summary", 512), ("description", 2048)] {
                if c::get(p, key)? != &Value::Null {
                    body(text(p, key)?, max)?;
                }
            }
            bounded(c::get(p, "details")?, 16384)?;
            let fields = c::get(p, "details")?.as_object().ok_or_else(c::bad)?;
            if fields.len() > 32 || places[..i].iter().any(|old| old.get("id") == p.get("id")) {
                return Err(c::bad());
            }
        }
        Ok(Self { revision, places: places.to_vec() })
    }
    pub fn tool(&self) -> Value {
        obj! {"type"=>"function","function"=>obj!{"name"=>"location_lookup","description"=>"Look up the host-bound current location, its summary, description and visible scene details. This moves nobody and invents no facts.","parameters"=>obj!{"type"=>"object","properties"=>obj!{"detail"=>obj!{"type"=>"string","enum"=>vec!["brief","normal","full"]}},"required"=>vec!["detail"],"additionalProperties"=>false}}}
    }
    /// `current`, live revision, result cap and visibility predicate are host inputs.
    /// Give each actor an already-redacted snapshot: this predicate gates places, not
    /// individual secret fields inside `details`. Missing and unauthorized places match.
    /// Requested detail must fit whole; a caller may explicitly retry a smaller detail.
    pub fn call(
        &self,
        input: &str,
        current: &str,
        revision: u32,
        max_bytes: usize,
        available: impl FnOnce(&str) -> bool,
    ) -> Result<Value> {
        c::scope(current)?;
        if revision != self.revision || !(64..=32768).contains(&max_bytes) {
            return Err(c::bad());
        }
        let a = crate::data::json::parse(input, 4096)?;
        c::fields(&a, &["detail"])?;
        let detail = text(&a, "detail")?;
        if !["brief", "normal", "full"].contains(&detail) {
            return Err(c::bad());
        }
        let p = self.places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(current));
        let location = if let Some(p) = p.filter(|_| available(current)) {
            let mut fields = ["id", "label", "summary"]
                .into_iter()
                .map(|k| (k.into(), p.get(k).expect("validated").clone()))
                .collect::<Vec<_>>();
            if detail != "brief" {
                fields.push(("description".into(), c::get(p, "description")?.clone()));
            }
            if detail == "full" {
                fields.push(("details".into(), c::get(p, "details")?.clone()));
            }
            Value::Object(fields)
        } else {
            Value::Null
        };
        let result = obj! {"revision"=>revision,"location"=>location};
        bounded(&result, max_bytes)?;
        Ok(result)
    }
    pub fn revision(&self) -> u32 {
        self.revision
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1,"revision"=>self.revision,"places"=>self.places.clone()}
    }
}
pub fn process(a: &Value) -> Result<Value> {
    bounded(a, 1_100_000)?;
    c::fields(a, &["definition", "text", "current", "revision", "max_bytes", "available"])?;
    let places = Locations::from_value(c::get(a, "definition")?)?;
    let allowed = c::list(c::get(a, "available")?)?;
    if allowed.len() > 128 {
        return Err(c::bad());
    }
    for id in allowed {
        c::scope(c::text(id)?)?;
    }
    places.call(
        text(a, "text")?,
        text(a, "current")?,
        c::uint(c::get(a, "revision")?, 0, u32::MAX as u64)? as u32,
        c::uint(c::get(a, "max_bytes")?, 64, 32768)? as usize,
        |id| allowed.iter().any(|v| v.as_str() == Some(id)),
    )
}
