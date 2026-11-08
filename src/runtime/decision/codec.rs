use super::*;
/// Opt-in readable names for a fixed bijection of legal pairs. Never derive opaque
/// numeric aliases automatically. Count the offer AND schema in the full request.
#[derive(Clone, Debug)]
pub struct ChoiceCodec {
    choices: Choices,
    names: Vec<Value>,
}
impl ChoiceCodec {
    pub fn new(choices: &Choices, names: &Value) -> Result<Self> {
        bounded(names, 32_768)?;
        let rows = c::list(names)?;
        if rows.len() != choices.pairs.len() {
            return Err(c::bad());
        }
        for (i, r) in rows.iter().enumerate() {
            c::fields(r, &["name", "pair"])?;
            c::name(c::text(c::get(r, "name")?)?)?;
            choices.validate(c::get(r, "pair")?)?;
            if rows[..i].iter().any(|p| {
                p.get("name") == r.get("name") || p.get("pair").expect("pair").same(r.get("pair").expect("pair"))
            }) {
                return Err(c::bad());
            }
        }
        Ok(Self { choices: choices.clone(), names: rows.to_vec() })
    }
    pub fn offer(&self) -> Value {
        Value::Array(self.names.clone())
    }
    pub fn schema(&self) -> Value {
        let names = self.names.iter().map(|r| r.get("name").expect("name").clone()).collect::<Vec<_>>();
        obj! {"type"=>"object","properties"=>obj! {"choice"=>obj! {"type"=>"string","enum"=>names}},"required"=>vec!["choice"],"additionalProperties"=>false}
    }
    /// Strict JSON, bounded, with exactly one known choice. Returns the original pair.
    pub fn decode(&self, text: &str) -> Result<Value> {
        let v = crate::data::json::parse(text, 4096)?;
        c::fields(&v, &["choice"])?;
        let name = c::text(c::get(&v, "choice")?)?;
        self.names
            .iter()
            .find(|r| r.get("name").and_then(Value::as_str) == Some(name))
            .and_then(|r| r.get("pair"))
            .cloned()
            .ok_or_else(c::bad)
    }
    pub fn session(&self) -> ChoiceSession {
        ChoiceSession { codec: self.clone(), inner: self.choices.session() }
    }
}
#[derive(Clone, Debug)]
pub struct ChoiceSession {
    codec: ChoiceCodec,
    inner: Session,
}
impl ChoiceSession {
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }
    pub fn cancel(&mut self, available: impl FnMut(&Value) -> bool) -> Result<Value> {
        self.inner.cancel(available)
    }
    pub fn apply(&mut self, text: &str, revision: u32, available: impl FnMut(&Value) -> bool) -> Result<Value> {
        let proposal = self.codec.decode(text).map(|v| v.to_json()).unwrap_or_else(|_| "null".into());
        let mut result = self.inner.apply(&proposal, revision, available)?;
        if result.get("status").and_then(Value::as_str) == Some("retry") {
            if let Value::Object(fields) = &mut result {
                if let Some((_, v)) = fields.iter_mut().find(|(k, _)| k == "schema") {
                    *v = self.codec.schema();
                }
            }
        }
        Ok(result)
    }
}
