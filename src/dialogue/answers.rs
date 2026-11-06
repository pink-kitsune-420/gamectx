//! Source-bound factual answers: the model selects an ID; the host supplies all prose.
use crate::{checked as c, obj, request::bounded, Result, Value};
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
fn body(v: &Value, max: usize) -> Result<()> {
    let s = c::text(v)?;
    if s.is_empty() || s.chars().count() > max || s.chars().any(char::is_control) {
        return Err(c::bad());
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Answers {
    value: Value,
}
impl Answers {
    /// Cards must be prepared and authorized by the host, never taken from model prose.
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 65536)?;
        c::fields(v, &["version", "scope", "revision", "unknown", "cards"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        c::scope(text(v, "scope")?)?;
        c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)?;
        body(c::get(v, "unknown")?, 2048)?;
        let cards = c::list(c::get(v, "cards")?)?;
        if cards.len() > 32 {
            return Err(c::bad());
        }
        for (i, card) in cards.iter().enumerate() {
            c::fields(card, &["id", "subject", "topic", "kind", "source", "text"])?;
            for key in ["id", "subject", "topic", "source"] {
                c::scope(text(card, key)?)?;
            }
            if text(card, "id")? == "unknown"
                || cards[..i].iter().any(|v| v.get("id") == card.get("id"))
                || !["lore", "state", "quote"].contains(&text(card, "kind")?)
            {
                return Err(c::bad());
            }
            body(c::get(card, "text")?, 1024)?;
        }
        let mut value = v.clone();
        let keep = cards
            .iter()
            .filter(|card| {
                card.get("kind").and_then(Value::as_str) != Some("lore")
                    || !cards.iter().any(|state| {
                        state.get("kind").and_then(Value::as_str) == Some("state")
                            && state.get("subject") == card.get("subject")
                            && state.get("topic") == card.get("topic")
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Value::Object(fields) = &mut value {
            *fields.iter_mut().find(|(k, _)| k == "cards").expect("cards") = ("cards".into(), Value::Array(keep));
        }
        Ok(Self { value })
    }
    pub fn offer(&self) -> Value {
        Value::Array(self.value.get("cards").and_then(Value::as_array).expect("cards").iter().map(|c|obj! {"id"=>c.get("id").expect("id").clone(),"subject"=>c.get("subject").expect("subject").clone(),"topic"=>c.get("topic").expect("topic").clone(),"kind"=>c.get("kind").expect("kind").clone(),"text"=>c.get("text").expect("text").clone()}).collect())
    }
    pub fn schema(&self) -> Value {
        let mut ids = self
            .value
            .get("cards")
            .and_then(Value::as_array)
            .expect("cards")
            .iter()
            .map(|c| c.get("id").expect("id").clone())
            .collect::<Vec<_>>();
        ids.push(Value::str("unknown"));
        obj! {"type"=>"object","properties"=>obj! {"answer"=>obj! {"type"=>"string","enum"=>ids}},"required"=>vec!["answer"],"additionalProperties"=>false}
    }
    /// Recheck current scope/revision and source availability. No execution or fact
    /// registration occurs. Correct selection semantics remain a model/host task.
    pub fn resolve(&self, input: &str, context: &Value, available: impl FnOnce(&Value) -> bool) -> Result<Value> {
        c::fields(context, &["scope", "revision"])?;
        if text(context, "scope")? != text(&self.value, "scope")?
            || c::uint(c::get(context, "revision")?, 0, u32::MAX as u64)?
                != c::uint(c::get(&self.value, "revision")?, 0, u32::MAX as u64)?
        {
            return Err(c::bad());
        }
        let reply = crate::json::parse(input, 4096)?;
        c::fields(&reply, &["answer"])?;
        let id = text(&reply, "answer")?;
        if id == "unknown" {
            return Ok(
                obj! {"status"=>"unknown","text"=>c::get(&self.value,"unknown")?.clone(),"sources"=>Vec::<Value>::new()},
            );
        }
        let card = c::list(c::get(&self.value, "cards")?)?
            .iter()
            .find(|c| c.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(c::bad)?;
        let source = obj! {"kind"=>c::get(card,"kind")?.clone(),"id"=>c::get(card,"source")?.clone()};
        if !available(&source) {
            return Err(c::bad());
        }
        let text = if text(card, "kind")? == "quote" {
            format!("You told me: \"{}\"", text(card, "text")?)
        } else {
            text(card, "text")?.to_owned()
        };
        Ok(obj! {"status"=>"answered","text"=>text,"sources"=>vec![source]})
    }
}
