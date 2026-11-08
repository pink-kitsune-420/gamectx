//! Optional read-only world/time lookup. Calendars, clocks, weather, conditions and
//! rules are supplied by the game. No real-world clock or simulation is assumed.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
fn keys(v: &Value, max: usize) -> Result<Vec<String>> {
    let a = c::list(v)?;
    if a.len() > max {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for v in a {
        let k = c::text(v)?;
        c::scope(k)?;
        if out.iter().any(|old| old == k) {
            return Err(c::bad());
        }
        out.push(k.into());
    }
    Ok(out)
}
#[derive(Clone, Debug)]
pub struct WorldState {
    value: Value,
    revision: u32,
}
impl WorldState {
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(input, 524288)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 524288)?;
        c::fields(v, &["version", "id", "revision", "states"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        c::scope(text(v, "id")?)?;
        let revision = c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)? as u32;
        let states = c::list(c::get(v, "states")?)?;
        if states.len() > 64 {
            return Err(c::bad());
        }
        for (i, s) in states.iter().enumerate() {
            c::fields(s, &["key", "value"])?;
            c::scope(text(s, "key")?)?;
            bounded(c::get(s, "value")?, 4096)?;
            if states[..i].iter().any(|old| old.get("key") == s.get("key")) {
                return Err(c::bad());
            }
        }
        Ok(Self { value: v.clone(), revision })
    }
    pub fn id(&self) -> &str {
        self.value.get("id").and_then(Value::as_str).expect("validated world id")
    }
    pub fn revision(&self) -> u32 {
        self.revision
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn tool(&self) -> Value {
        obj! {"type"=>"function","function"=>obj!{"name"=>"world_status","description"=>"Read selected current world states such as time, weather, lighting or event flags. Values come from the game; null is unknown. This advances no clock and changes no state.","parameters"=>obj!{"type"=>"object","properties"=>obj!{"keys"=>obj!{"type"=>"array","minItems"=>1,"maxItems"=>8,"items"=>obj!{"type"=>"string","minLength"=>1,"maxLength"=>64}}},"required"=>vec!["keys"],"additionalProperties"=>false}}}
    }
    /// The current revision, output cap and key visibility are host-bound. Missing
    /// and unauthorized keys are both absent; no hidden-key counts or schemas leak.
    pub fn call(
        &self,
        input: &str,
        revision: u32,
        max_bytes: usize,
        mut visible: impl FnMut(&str) -> bool,
    ) -> Result<Value> {
        if revision != self.revision || !(64..=32768).contains(&max_bytes) {
            return Err(c::bad());
        }
        let a = crate::data::json::parse(input, 4096)?;
        c::fields(&a, &["keys"])?;
        let requested = keys(c::get(&a, "keys")?, 8)?;
        if requested.is_empty() {
            return Err(c::bad());
        }
        let states = c::list(c::get(&self.value, "states")?)?;
        let mut selected = Vec::new();
        for key in requested {
            if visible(&key) {
                if let Some(s) = states.iter().find(|s| s.get("key").and_then(Value::as_str) == Some(&key)) {
                    selected.push((key, c::get(s, "value")?.clone()));
                }
            }
        }
        let result =
            obj! {"id"=>c::get(&self.value,"id")?.clone(),"revision"=>revision,"states"=>Value::Object(selected)};
        bounded(&result, max_bytes)?;
        Ok(result)
    }
}
pub fn process(a: &Value) -> Result<Value> {
    bounded(a, 600000)?;
    c::fields(a, &["definition", "text", "revision", "max_bytes", "available"])?;
    let world = WorldState::from_value(c::get(a, "definition")?)?;
    let available = keys(c::get(a, "available")?, 64)?;
    world.call(
        text(a, "text")?,
        c::uint(c::get(a, "revision")?, 0, u32::MAX as u64)? as u32,
        c::uint(c::get(a, "max_bytes")?, 64, 32768)? as usize,
        |k| available.iter().any(|v| v == k),
    )
}
