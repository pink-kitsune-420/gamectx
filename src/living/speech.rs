//! Immediate authored speech with an optional before-delivery upgrade. No executor,
//! network, sleeps, locks or joins are owned by this queue. The host runs any worker.
use super::*;
use crate::{
    facts::FactRegistry,
    requests::{RequestContext, RequestGate, RequestTicket},
};
#[derive(Debug)]
pub struct SpeechTicket {
    owner: Arc<()>,
    id: u64,
}
impl SpeechTicket {
    pub fn id(&self) -> u64 {
        self.id
    }
}
#[derive(Debug)]
struct Entry {
    ticket: RequestTicket,
    context: RequestContext,
    line: String,
    stage: &'static str,
    completed: bool,
    raw: Option<String>,
}
#[derive(Debug)]
pub struct SpeechQueue {
    owner: Arc<()>,
    gate: RequestGate,
    entries: Vec<Entry>,
    clock: u64,
    facts: FactRegistry,
}
impl Default for SpeechQueue {
    fn default() -> Self {
        Self::new()
    }
}
fn context(v: &Value) -> Result<RequestContext> {
    bounded(v, 8192)?;
    let c = RequestContext::from_value(v)?;
    // Revision-scoped queue. Hosts that need individual FactRef freshness can use
    // RequestGate directly; nonempty references must never be silently ignored here.
    if !c::list(get(v, "facts")?)?.is_empty() {
        return Err(c::bad());
    }
    Ok(c)
}
impl SpeechQueue {
    pub fn new() -> Self {
        Self {
            owner: Arc::new(()),
            gate: RequestGate::new(),
            entries: Vec::new(),
            clock: 0,
            facts: FactRegistry::new(1).expect("fixed capacity"),
        }
    }
    fn index(&self, h: &SpeechTicket) -> Result<usize> {
        if !Arc::ptr_eq(&self.owner, &h.owner) {
            return Err(c::bad());
        }
        self.entries.iter().position(|e| e.ticket.id() == h.id).ok_or_else(c::bad)
    }
    pub fn issue(&mut self, ctx: &Value, now: u64, ttl: u64, fallback: &str) -> Result<(SpeechTicket, Value)> {
        c::time(now, self.clock)?;
        validate_line(fallback)?;
        if self.entries.len() >= 32 {
            return Err(c::bad());
        }
        let ctx = context(ctx)?;
        let ticket = self.gate.issue(ctx.clone(), now, ttl)?;
        let id = ticket.id();
        self.entries.push(Entry {
            ticket,
            context: ctx,
            line: fallback.into(),
            stage: "pending",
            completed: false,
            raw: None,
        });
        self.clock = now;
        Ok((SpeechTicket { owner: self.owner.clone(), id }, obj! {"id"=>id,"stage"=>"pending","line"=>fallback}))
    }
    /// One completion attempt. Bounded JSON/terminal punctuation is necessary but not
    /// sufficient: a host validator must approve facts, policy and current permission.
    /// Invalid/stale attempts retain the fallback and raw text, consuming the capability.
    pub fn complete(
        &mut self,
        h: &SpeechTicket,
        ctx: &Value,
        now: u64,
        response: &str,
        mut approved: impl FnMut(&str) -> bool,
    ) -> Result<Value> {
        c::time(now, self.clock)?;
        let i = self.index(h)?;
        let current = context(ctx)?;
        if response.len() > 4096 || self.entries[i].stage != "pending" || self.entries[i].completed {
            return Err(c::bad());
        }
        let fresh = self.gate.accept(&self.entries[i].ticket, &current, now, &self.facts, &[])?;
        let candidate = (|| -> Result<String> {
            let v = crate::json::parse(response, 4096)?;
            c::fields(&v, &["speech"])?;
            let s = text(&v, "speech")?;
            validate_line(s)?;
            Ok(s.into())
        })();
        let e = &mut self.entries[i];
        e.completed = true;
        e.raw = Some(response.into());
        let valid = if fresh { candidate.as_ref().is_ok_and(|s| approved(s)) } else { false };
        if valid {
            e.line = candidate.expect("validated");
            e.stage = "upgraded";
        }
        self.clock = now;
        Ok(
            obj! {"id"=>h.id,"stage"=>e.stage,"line"=>e.line.clone(),"reason"=>if fresh&&valid{"upgraded"}else if !fresh{"stale"}else{"invalid_response"},"raw"=>response},
        )
    }
    /// Seal the line before queuing/playing any text or audio. This is not evidence
    /// of playback/hearing. A later completion cannot change the frozen text.
    pub fn freeze(&mut self, h: &SpeechTicket, ctx: &Value, now: u64) -> Result<Value> {
        c::time(now, self.clock)?;
        let i = self.index(h)?;
        let current = context(ctx)?;
        let e = &mut self.entries[i];
        if !matches!(e.stage, "pending" | "upgraded") || current != e.context {
            return Err(c::bad());
        }
        if !e.completed {
            self.gate.cancel(&e.ticket, now)?;
            e.completed = true;
        }
        e.stage = "frozen";
        self.clock = now;
        Ok(obj! {"id"=>h.id,"stage"=>"frozen","line"=>e.line.clone()})
    }
    pub fn cancel(&mut self, h: &SpeechTicket, now: u64) -> Result<Value> {
        c::time(now, self.clock)?;
        let i = self.index(h)?;
        let e = &mut self.entries[i];
        if !matches!(e.stage, "pending" | "upgraded") {
            return Err(c::bad());
        }
        if !e.completed {
            self.gate.cancel(&e.ticket, now)?;
            e.completed = true;
        }
        e.stage = "cancelled";
        self.clock = now;
        Ok(obj! {"id"=>h.id,"stage"=>"cancelled"})
    }
    pub fn retire(&mut self, h: &SpeechTicket, now: u64) -> Result<()> {
        c::time(now, self.clock)?;
        let i = self.index(h)?;
        if !matches!(self.entries[i].stage, "frozen" | "cancelled") {
            return Err(c::bad());
        }
        self.entries.remove(i);
        self.clock = now;
        Ok(())
    }
    pub fn inspect(&self) -> Value {
        self.entries.iter().map(|e|obj!{"id"=>e.ticket.id(),"stage"=>e.stage,"line"=>e.line.clone(),"raw"=>e.raw.as_ref().map_or(Value::Null,Value::str)}).collect::<Vec<_>>().into()
    }
}
pub fn speech_workflow(a: &Value) -> Result<Value> {
    bounded(a, 262144)?;
    c::fields(a, &["operations"])?;
    let ops = c::list(get(a, "operations")?)?;
    if ops.len() > 256 {
        return Err(c::bad());
    }
    let mut queue = SpeechQueue::new();
    let mut handles = Vec::new();
    let mut results = Vec::new();
    for op in ops {
        let mut run = || -> Result<Value> {
            c::fields(op, &["op", "args"])?;
            let v = get(op, "args")?;
            let now = num(v, "now")?;
            if text(op, "op")? == "issue" {
                c::fields(v, &["context", "now", "ttl", "fallback"])?;
                let (h, result) = queue.issue(get(v, "context")?, now, num(v, "ttl")?, text(v, "fallback")?)?;
                handles.push(h);
                return Ok(result);
            }
            let h = handles.iter().find(|h| h.id() == num(v, "id").unwrap_or(0)).ok_or_else(c::bad)?;
            match text(op, "op")? {
                "complete" => {
                    c::fields(v, &["id", "now", "context", "response", "approved"])?;
                    let approved = c::boolean(get(v, "approved")?)?;
                    queue.complete(h, get(v, "context")?, now, text(v, "response")?, |_| approved)
                }
                "freeze" => {
                    c::fields(v, &["id", "now", "context"])?;
                    queue.freeze(h, get(v, "context")?, now)
                }
                "cancel" => {
                    c::fields(v, &["id", "now"])?;
                    queue.cancel(h, now)
                }
                "retire" => {
                    c::fields(v, &["id", "now"])?;
                    queue.retire(h, now)?;
                    Ok(Value::Null)
                }
                _ => Err(c::bad()),
            }
        };
        results.push(match run() {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results,"model_calls"=>0})
}
