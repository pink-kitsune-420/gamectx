//! Human confirmation is explicit host input; generated prose never approves an action.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
#[derive(Clone, Debug)]
pub struct PendingAction {
    offer: Value,
    finished: bool,
}
impl PendingAction {
    pub fn new(offer: &Value) -> Result<Self> {
        bounded(offer, 8192)?;
        c::fields(offer, &["id", "scope", "revision", "decision", "terms"])?;
        for key in ["id", "scope"] {
            c::scope(c::text(c::get(offer, key)?)?)?;
        }
        c::uint(c::get(offer, "revision")?, 0, u32::MAX as u64)?;
        let pair = c::get(offer, "decision")?;
        super::pair(pair)?;
        c::get(offer, "terms")?.as_object().ok_or_else(c::bad)?;
        Ok(Self { offer: offer.clone(), finished: false })
    }
    pub fn offer(&self) -> Value {
        self.offer.clone()
    }
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    /// One attempt consumes the offer, including decline, stale terms, or revocation.
    /// The predicate checks permissions without executing. A confirmed proposal still
    /// needs atomic host dispatch with the same revision and terms.
    pub fn respond(
        &mut self,
        scope: &str,
        revision: u32,
        confirm: bool,
        terms: &Value,
        available: impl FnOnce(&Value) -> bool,
    ) -> Result<Value> {
        if self.finished {
            return Err(c::bad());
        }
        self.finished = true;
        if !confirm {
            return Ok(obj! {"status"=>"cancelled"});
        }
        if c::text(c::get(&self.offer, "scope")?)? != scope
            || c::uint(c::get(&self.offer, "revision")?, 0, u32::MAX as u64)? != revision as u64
            || !c::get(&self.offer, "terms")?.same(terms)
            || !available(c::get(&self.offer, "decision")?)
        {
            return Err(c::bad());
        }
        Ok(
            obj! {"status"=>"confirmed","decision"=>c::get(&self.offer,"decision")?.clone(),"terms"=>c::get(&self.offer,"terms")?.clone()},
        )
    }
}

pub fn confirmation_workflow(a: &Value) -> Result<Value> {
    bounded(a, crate::packet::request::MAX_REQUEST_BYTES)?;
    c::fields(a, &["offer", "calls"])?;
    let mut offer = PendingAction::new(c::get(a, "offer")?)?;
    let calls = c::list(c::get(a, "calls")?)?;
    if calls.len() > 8 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for call in calls {
        c::fields(call, &["scope", "revision", "confirm", "terms", "available"])?;
        let result = (|| {
            let scope = c::text(c::get(call, "scope")?)?;
            c::scope(scope)?;
            let revision = c::uint(c::get(call, "revision")?, 0, u32::MAX as u64)? as u32;
            let confirm = c::boolean(c::get(call, "confirm")?)?;
            let available = c::list(c::get(call, "available")?)?;
            if available.len() > 32 {
                return Err(c::bad());
            }
            offer.respond(scope, revision, confirm, c::get(call, "terms")?, |p| available.iter().any(|v| v.same(p)))
        })();
        results.push(match result {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results,"finished"=>offer.is_finished()})
}
