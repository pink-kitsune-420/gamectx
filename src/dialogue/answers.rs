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

impl Answers {
    fn spoken_card(card: &Value) -> Result<String> {
        Ok(if text(card, "kind")? == "quote" {
            format!("You told me: \"{}\"", text(card, "text")?)
        } else {
            text(card, "text")?.to_owned()
        })
    }
    /// Opt-in exact authorized speech variants for focused factual/service turns.
    /// The model selects wording; host-approved cards supply every character. This
    /// deliberately trades free expression for bounded source validation.
    pub fn speech_schema(&self) -> Result<Value> {
        self.speech_schema_with_unknown(true)
    }
    /// Exclude generic deferral only when the host has supplied question-specific answers.
    /// At least one authorized sentence is then required. This is a host policy, not an
    /// assertion that arbitrary retrieved cards answer the current question.
    pub fn speech_schema_with_unknown(&self, allow_unknown: bool) -> Result<Value> {
        let mut choices = Vec::<Value>::new();
        for card in c::list(c::get(&self.value, "cards")?)? {
            choices.push(Self::spoken_card(card)?.into());
        }
        if allow_unknown {
            choices.push(c::get(&self.value, "unknown")?.clone());
        }
        if choices.is_empty() {
            return Err(c::bad());
        }
        for (i, line) in choices.iter().enumerate() {
            let s = c::text(line)?;
            if s.chars().count() > 240
                || choices[..i].contains(line)
                || crate::life::assess_sentence(s, 240)?.to_value().get("boundary_complete") != Some(&Value::Bool(true))
            {
                return Err(c::bad());
            }
        }
        Ok(
            obj! {"type"=>"object","properties"=>obj!{"speech"=>obj!{"type"=>"string","enum"=>choices}},"required"=>vec!["speech"],"additionalProperties"=>false},
        )
    }
    /// Resolve exact speech against current scope/revision and source availability.
    /// Providers that ignore enum/schema constraints still cannot introduce prose.
    pub fn resolve_speech(
        &self,
        input: &str,
        context: &Value,
        available: impl FnOnce(&Value) -> bool,
    ) -> Result<Value> {
        self.resolve_speech_with_unknown(input, context, true, available)
    }
    /// Apply the same explicit unknown policy at validation time, including sources for
    /// host-approved sentences expressing a known absence of information.
    pub fn resolve_speech_with_unknown(
        &self,
        input: &str,
        context: &Value,
        allow_unknown: bool,
        available: impl FnOnce(&Value) -> bool,
    ) -> Result<Value> {
        self.speech_schema_with_unknown(allow_unknown)?;
        let reply = crate::json::parse(input, 4096)?;
        c::fields(&reply, &["speech"])?;
        let spoken = text(&reply, "speech")?;
        let id = if allow_unknown && c::get(&self.value, "unknown")?.as_str() == Some(spoken) {
            "unknown"
        } else {
            let card = c::list(c::get(&self.value, "cards")?)?
                .iter()
                .find(|card| Self::spoken_card(card).is_ok_and(|s| s == spoken))
                .ok_or_else(c::bad)?;
            text(card, "id")?
        };
        self.resolve(&obj! {"answer"=>id}.to_json(), context, available)
    }
}
