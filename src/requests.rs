//! One-use, instance-bound asynchronous result tickets. They validate freshness;
//! they never authorize or execute an action. Host permissions must still be enforced.
use crate::{
    checked::*,
    facts::{FactRef, FactRegistry},
    Result, Value,
};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestContext {
    actor: String,
    world: String,
    schema: u64,
    permissions: u64,
    facts: Vec<FactRef>,
}
impl RequestContext {
    pub fn from_value(v: &Value) -> Result<Self> {
        fields(v, &["actor", "world", "schema", "permissions", "facts"])?;
        let actor = text(get(v, "actor")?)?;
        let world = text(get(v, "world")?)?;
        scope(actor)?;
        scope(world)?;
        let schema = uint(get(v, "schema")?, 0, u32::MAX as u64)?;
        let permissions = uint(get(v, "permissions")?, 0, u32::MAX as u64)?;
        let raw = list(get(v, "facts")?)?;
        if raw.len() > 16 {
            return Err(bad());
        }
        let mut facts = raw.iter().map(FactRef::from_value).collect::<Result<Vec<_>>>()?;
        facts.sort_by(|a, b| (&a.scope, &a.key).cmp(&(&b.scope, &b.key)));
        if facts.windows(2).any(|p| p[0].scope == p[1].scope && p[0].key == p[1].key) {
            return Err(bad());
        }
        Ok(Self { actor: actor.into(), world: world.into(), schema, permissions, facts })
    }
}
#[derive(Debug)]
pub struct RequestTicket {
    owner: Arc<()>,
    id: u64,
}
impl RequestTicket {
    pub fn id(&self) -> u64 {
        self.id
    }
}
#[derive(Debug)]
struct Pending {
    id: u64,
    context: RequestContext,
    expires: u64,
}
#[derive(Debug)]
pub struct RequestGate {
    owner: Arc<()>,
    clock: u64,
    next: u64,
    pending: Vec<Pending>,
}
impl Default for RequestGate {
    fn default() -> Self {
        Self::new()
    }
}
impl RequestGate {
    pub fn new() -> Self {
        Self { owner: Arc::new(()), clock: 0, next: 0, pending: Vec::new() }
    }
    pub fn issue(&mut self, context: RequestContext, now: u64, ttl: u64) -> Result<RequestTicket> {
        time(now, self.clock)?;
        if !(1..=1000000).contains(&ttl)
            || now + ttl > u32::MAX as u64
            || self.pending.len() >= 32
            || self.next >= u32::MAX as u64
        {
            return Err(bad());
        }
        self.next += 1;
        self.clock = now;
        self.pending.push(Pending { id: self.next, context, expires: now + ttl });
        Ok(RequestTicket { owner: self.owner.clone(), id: self.next })
    }
    fn take(&mut self, ticket: &RequestTicket, now: u64) -> Result<Pending> {
        time(now, self.clock)?;
        if !Arc::ptr_eq(&self.owner, &ticket.owner) {
            return Err(bad());
        }
        let i = self.pending.iter().position(|p| p.id == ticket.id).ok_or_else(bad)?;
        self.clock = now;
        Ok(self.pending.remove(i))
    }
    /// Freshness rejection also consumes the ticket. Validation errors before consumption
    /// (foreign ticket, invalid clock/denied labels) leave the pending set unchanged.
    pub fn accept(
        &mut self,
        ticket: &RequestTicket,
        current: &RequestContext,
        now: u64,
        facts: &FactRegistry,
        denied: &[String],
    ) -> Result<bool> {
        facts.validate_read(now, denied)?;
        let p = self.take(ticket, now)?;
        let mut valid = now < p.expires && p.context == *current;
        for reference in &current.facts {
            if facts.get(&reference.scope, &reference.key, now, denied)?.is_none_or(|f| f.reference != *reference) {
                valid = false
            }
        }
        Ok(valid)
    }
    pub fn cancel(&mut self, ticket: &RequestTicket, now: u64) -> Result<()> {
        self.take(ticket, now)?;
        Ok(())
    }
}
impl RequestGate {
    /// Durable host replay metadata. Pending capabilities are intentionally not serialized.
    pub fn watermarks(&self) -> (u64, u64) {
        (self.clock, self.next)
    }
    /// Recover with no live tickets. Authenticate branch-specific watermarks in the host.
    pub fn resume(clock: u64, next: u64) -> Result<Self> {
        time(clock, 0)?;
        if next > u32::MAX as u64 {
            return Err(bad());
        }
        let mut gate = Self::new();
        gate.clock = clock;
        gate.next = next;
        Ok(gate)
    }
}
