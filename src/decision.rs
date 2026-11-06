//! One bounded scene decision from host-enumerated legal pairs. It never executes an
//! action. Current world permissions and atomic dispatch remain host responsibilities.
use crate::{checked as c, obj, request::bounded, Error, Result, Value};

mod intent;
pub use intent::IntentSession;
mod routing;
pub use routing::route;
mod codec;
pub use codec::{ChoiceCodec, ChoiceSession};

#[derive(Clone, Debug)]
pub struct Choices {
    revision: u32,
    pairs: Vec<Value>,
    fallback: Value,
}
fn pair(v: &Value) -> Result<()> {
    c::fields(v, &["action", "target"])?;
    c::name(c::text(c::get(v, "action")?)?)?;
    if c::get(v, "target")? != &Value::Null {
        c::scope(c::text(c::get(v, "target")?)?)?;
    }
    Ok(())
}
impl Choices {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 16_384)?;
        c::fields(v, &["version", "revision", "choices", "fallback"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let revision = c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)? as u32;
        let pairs = c::list(c::get(v, "choices")?)?;
        if pairs.is_empty() || pairs.len() > 32 {
            return Err(c::bad());
        }
        for (i, p) in pairs.iter().enumerate() {
            pair(p)?;
            if pairs[..i].iter().any(|x| x.same(p)) {
                return Err(c::bad());
            }
        }
        let fallback = c::get(v, "fallback")?;
        if *fallback != Value::Null && !pairs.iter().any(|p| p.same(fallback)) {
            return Err(c::bad());
        }
        Ok(Self { revision, pairs: pairs.to_vec(), fallback: fallback.clone() })
    }
    /// Standard JSON Schema, including whole-pair enum to prevent action/target mixing.
    /// Runtime support for this grammar must be checked; local validation always runs.
    pub fn schema(&self) -> Value {
        let mut actions = Vec::new();
        let mut targets = Vec::new();
        for p in &self.pairs {
            let a = p.get("action").expect("action");
            let t = p.get("target").expect("target");
            if !actions.contains(a) {
                actions.push(a.clone());
            }
            if !targets.contains(t) {
                targets.push(t.clone());
            }
        }
        obj! {"type"=>"object","properties"=>obj! {
        "action"=>obj! {"type"=>"string","maxLength"=>32,"enum"=>actions},
        "target"=>obj! {"type"=>vec!["string","null"],"maxLength"=>64,"enum"=>targets}},
        "required"=>vec!["action","target"],"additionalProperties"=>false,"enum"=>self.pairs.clone()}
    }
    pub fn validate(&self, v: &Value) -> Result<()> {
        bounded(v, 4096)?;
        pair(v)?;
        if !self.pairs.iter().any(|p| p.same(v)) {
            return Err(Error::new("decision is not a registered action/target pair"));
        }
        Ok(())
    }
    pub fn session(&self) -> Session {
        Session { choices: self.clone(), attempts: 0, finished: false }
    }
}
#[derive(Clone, Debug)]
pub struct Session {
    choices: Choices,
    attempts: u8,
    finished: bool,
}
impl Session {
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    /// Ends a request that cannot be dispatched (for example, a context-budget failure).
    /// Uses only a live-approved fallback; no model call or synthetic retry is needed.
    pub fn cancel(&mut self, mut available: impl FnMut(&Value) -> bool) -> Result<Value> {
        if self.finished {
            return Err(Error::new("decision session is finished"));
        }
        self.finished = true;
        if self.choices.fallback != Value::Null && available(&self.choices.fallback) {
            return Ok(obj! {"status"=>"fallback","decision"=>self.choices.fallback.clone()});
        }
        Ok(obj! {"status"=>"abstain","decision"=>Value::Null})
    }
    /// Strict JSON only, max 4096 bytes. One correction, then a host-approved fallback
    /// or abstention. Stale revisions skip correction. `available` checks live state;
    /// it must not execute actions. Successful proposals still require atomic host dispatch.
    pub fn apply(
        &mut self,
        text: &str,
        current_revision: u32,
        mut available: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        if self.finished {
            return Err(Error::new("decision session is finished"));
        }
        self.attempts += 1;
        let proposed = crate::json::parse(text, 4096).ok().filter(|v| self.choices.validate(v).is_ok());
        let stale = current_revision != self.choices.revision;
        if let Some(v) = proposed.filter(|v| !stale && available(v)) {
            self.finished = true;
            return Ok(obj! {"status"=>"accepted","decision"=>v});
        }
        if !stale && self.attempts == 1 {
            return Ok(obj! {"status"=>"retry","error"=>"invalid_decision","schema"=>self.choices.schema()});
        }
        self.finished = true;
        if self.choices.fallback != Value::Null && available(&self.choices.fallback) {
            return Ok(obj! {"status"=>"fallback","decision"=>self.choices.fallback.clone()});
        }
        Ok(obj! {"status"=>"abstain","decision"=>Value::Null})
    }
}
