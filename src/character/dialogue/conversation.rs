//! Bounded host-scoped dialogue with verbatim player quotes and one-use reply tickets.
use crate::{
    checked as c,
    knowledge::memory::{Limits, Memory},
    obj,
    packet::request::bounded,
    Error, Result, Value,
};
fn invalid() -> Error {
    Error::new("invalid conversation request")
}
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    c::get(v, k)
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(get(v, k)?)
}
fn body(s: &str, max: usize) -> Result<()> {
    if s.trim().is_empty() || s.chars().count() > max || s.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(())
}
/// Host-controlled retention/work bounds. Nothing is persisted or loaded automatically.
#[derive(Clone, Copy, Debug)]
pub struct ConversationLimits {
    pub scopes: usize,
    pub records: usize,
    pub history: usize,
    pub text_chars: usize,
    pub tickets: u32,
}
impl Default for ConversationLimits {
    fn default() -> Self {
        Self { scopes: 32, records: 32, history: 8, text_chars: 240, tickets: 65536 }
    }
}
#[derive(Clone, Debug)]
struct Slot {
    scope: String,
    tone: String,
    history: Vec<Value>,
    memory: Memory,
}
impl Slot {
    fn new(scope: &str, limits: ConversationLimits) -> Self {
        Self {
            scope: scope.into(),
            tone: "neutral".into(),
            history: Vec::new(),
            memory: Memory::with_limits(Limits { records: limits.records, text_chars: limits.text_chars, tags: 0 }),
        }
    }
    fn memories(&self) -> Value {
        Value::Array(
            self.memory.records().iter().map(|r| obj! {"id"=>r.id,"turn"=>r.turn,"text"=>r.text.clone()}).collect(),
        )
    }
}
#[derive(Clone, Debug)]
pub struct Conversation {
    limits: ConversationLimits,
    sequence: u32,
    pending: Option<Value>,
    slots: Vec<Slot>,
}
impl Default for Conversation {
    fn default() -> Self {
        Self::with_limits(ConversationLimits::default()).expect("valid defaults")
    }
}
impl Conversation {
    pub fn with_limits(limits: ConversationLimits) -> Result<Self> {
        if !(1..=64).contains(&limits.scopes)
            || !(1..=256).contains(&limits.records)
            || !(1..=128).contains(&limits.history)
            || !(1..=1024).contains(&limits.text_chars)
            || !(1..=1_000_000).contains(&limits.tickets)
        {
            return Err(invalid());
        }
        Ok(Self { limits, sequence: 0, pending: None, slots: Vec::new() })
    }
    /// Bounded response schema for the current host-created ticket.
    pub fn reply_schema(&self) -> Result<Value> {
        speech_schema(self.pending.as_ref().ok_or_else(invalid)?, self.limits.text_chars)
    }
    /// Scope of the pending reply, for live access checks in the host.
    pub fn pending_scope(&self) -> Option<&str> {
        self.pending.as_ref().and_then(|v| v.get("scope")).and_then(Value::as_str)
    }
    pub fn state(&self, scope: &str) -> Result<Value> {
        c::scope(scope)?;
        let empty = Slot::new(scope, self.limits);
        let slot = self.slots.iter().find(|s| s.scope == scope).unwrap_or(&empty);
        Ok(obj! {"scope"=>scope,"tone"=>slot.tone.clone(),"history"=>slot.history.clone(),"memories"=>slot.memories()})
    }
    /// Host-supplied scope, revision and facts. This never infers actor permissions.
    pub fn prepare(&mut self, args: &Value) -> Result<Value> {
        bounded(args, 65536)?;
        c::fields(args, &["scope", "speaker", "revision", "input", "facts", "recall", "history"])?;
        let scope = text(args, "scope")?;
        c::scope(scope)?;
        let speaker = text(args, "speaker")?;
        c::scope(speaker)?;
        let revision = c::uint(get(args, "revision")?, 0, u32::MAX as u64)?;
        let input = text(args, "input")?.trim();
        body(input, self.limits.text_chars)?;
        let recall = c::uint(get(args, "recall")?, 0, self.limits.records.min(32) as u64)? as usize;
        let history = c::uint(get(args, "history")?, 0, self.limits.history as u64)? as usize;
        let facts = get(args, "facts")?;
        bounded(facts, 32768)?;
        let fields = facts.as_object().ok_or_else(invalid)?;
        if fields.len() > 32
            || self.sequence >= self.limits.tickets
            || (!self.slots.iter().any(|s| s.scope == scope) && self.slots.len() >= self.limits.scopes)
        {
            return Err(invalid());
        }
        for (i, (key, _)) in fields.iter().enumerate() {
            c::scope(key)?;
            if fields[..i].iter().any(|(old, _)| old == key) {
                return Err(invalid());
            }
        }
        let empty = Slot::new(scope, self.limits);
        let slot = self.slots.iter().find(|s| s.scope == scope).unwrap_or(&empty);
        let mut recalled = slot.memory.recall(input, recall);
        recalled.sort_by_key(|r| r.id);
        let memories =
            recalled.iter().map(|r| obj! {"id"=>r.id,"turn"=>r.turn,"text"=>r.text.clone()}).collect::<Vec<_>>();
        self.sequence += 1;
        let ticket = obj! {"id"=>format!("chat_{}",self.sequence),"revision"=>revision,"scope"=>scope,"speaker"=>speaker,"input"=>input,
        "facts"=>facts.clone(),"memories"=>memories,"history"=>slot.history.iter().skip(slot.history.len().saturating_sub(history)).cloned().collect::<Vec<_>>(),"tone"=>slot.tone.clone()};
        self.pending = Some(ticket.clone());
        Ok(ticket)
    }
    /// Consume one matching ticket after strict schema/source validation. Failed replies
    /// preserve state; a host must recheck scope/revision before exposing or executing anything.
    pub fn finish(&mut self, scope: &str, revision: u32, id: &str, input: &str) -> Result<Value> {
        let ticket = self.pending.as_ref().ok_or_else(invalid)?;
        if text(ticket, "id")? != id
            || c::uint(get(ticket, "revision")?, 0, u32::MAX as u64)? != revision as u64
            || scope != text(ticket, "scope")?
        {
            return Err(invalid());
        }
        let reply = crate::data::json::parse(input, 16384)?;
        c::fields(&reply, &["speech", "tone", "fact_ids", "memory_ids"])?;
        let speech = text(&reply, "speech")?.trim();
        body(speech, self.limits.text_chars)?;
        let tone = text(&reply, "tone")?;
        if !["neutral", "warm", "wary"].contains(&tone) {
            return Err(invalid());
        }
        let facts = get(&reply, "fact_ids")?.as_array().ok_or_else(invalid)?;
        if facts.len() > 8
            || facts.iter().enumerate().any(|(i, f)| {
                f.as_str().is_none_or(|f| ticket.get("facts").and_then(|v| v.get(f)).is_none())
                    || facts[..i].contains(f)
            })
        {
            return Err(invalid());
        }
        let memories = get(&reply, "memory_ids")?.as_array().ok_or_else(invalid)?;
        let recalled = get(ticket, "memories")?.as_array().ok_or_else(invalid)?;
        if memories.len() > 2
            || memories.iter().enumerate().any(|(i, m)| {
                c::uint(m, 0, u32::MAX as u64).is_err()
                    || !recalled.iter().any(|r| r.get("id") == Some(m))
                    || memories[..i].contains(m)
            })
        {
            return Err(invalid());
        }
        let scope = text(ticket, "scope")?;
        let mut slot =
            self.slots.iter().find(|s| s.scope == scope).cloned().unwrap_or_else(|| Slot::new(scope, self.limits));
        slot.tone = tone.into();
        slot.history
            .push(obj! {"id"=>id,"turn"=>self.sequence,"user"=>get(ticket,"input")?.clone(),"assistant"=>speech});
        if slot.history.len() > self.limits.history {
            slot.history.remove(0);
        }
        slot.memory.remember(self.sequence as u64, text(ticket, "input")?, &[], 5)?;
        let result = obj! {"speaker"=>get(ticket,"speaker")?.clone(),"speech"=>speech,"tone"=>tone,"fact_ids"=>facts.to_vec(),"memory_ids"=>memories.to_vec()};
        self.slots.retain(|s| s.scope != scope);
        self.slots.push(slot);
        self.pending = None;
        Ok(result)
    }
    pub fn forget(&mut self, scope: &str) -> Result<Value> {
        c::scope(scope)?;
        self.slots.retain(|s| s.scope != scope);
        if self.pending.as_ref().is_some_and(|p| p.get("scope").and_then(Value::as_str) == Some(scope)) {
            self.pending = None;
        }
        Ok(obj! {"forgotten"=>scope})
    }
}

/// Bounded replay adapter used by the CLI/bridges; one fresh conversation per call.
pub fn conversation_workflow(a: &Value) -> Result<Value> {
    bounded(a, crate::packet::request::MAX_REQUEST_BYTES)?;
    c::fields(a, &["limits", "operations"])?;
    let limits = c::get(a, "limits")?;
    c::fields(limits, &["scopes", "records", "history", "text_chars", "tickets"])?;
    let n = |key, high| c::uint(c::get(limits, key)?, 1, high);
    let mut conversation = Conversation::with_limits(ConversationLimits {
        scopes: n("scopes", 64)? as usize,
        records: n("records", 256)? as usize,
        history: n("history", 128)? as usize,
        text_chars: n("text_chars", 1024)? as usize,
        tickets: n("tickets", 1_000_000)? as u32,
    })?;
    let operations = c::list(c::get(a, "operations")?)?;
    if operations.len() > 256 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for operation in operations {
        c::fields(operation, &["op", "args"])?;
        let result = (|| {
            let args = c::get(operation, "args")?;
            match text(operation, "op")? {
                "prepare" => conversation.prepare(args),
                "schema" => {
                    c::fields(args, &[])?;
                    conversation.reply_schema()
                }
                "finish" => {
                    c::fields(args, &["scope", "revision", "ticket", "text"])?;
                    conversation.finish(
                        text(args, "scope")?,
                        c::uint(c::get(args, "revision")?, 0, u32::MAX as u64)? as u32,
                        text(args, "ticket")?,
                        text(args, "text")?,
                    )
                }
                "state" | "forget" => {
                    c::fields(args, &["scope"])?;
                    if text(operation, "op")? == "state" {
                        conversation.state(text(args, "scope")?)
                    } else {
                        conversation.forget(text(args, "scope")?)
                    }
                }
                _ => Err(c::bad()),
            }
        })();
        results.push(match result {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results})
}

/// Response schema for a host-created ticket; it does not authorize the facts or prose.
pub fn speech_schema(ticket: &Value, text_chars: usize) -> Result<Value> {
    bounded(ticket, crate::packet::request::MAX_REQUEST_BYTES)?;
    if !(1..=1024).contains(&text_chars) {
        return Err(invalid());
    }
    let supplied = get(ticket, "facts")?.as_object().ok_or_else(invalid)?;
    if supplied.len() > 32 {
        return Err(invalid());
    }
    for (key, _) in supplied {
        c::scope(key)?;
    }
    let quoted = get(ticket, "memories")?.as_array().ok_or_else(invalid)?;
    if quoted.len() > 32 {
        return Err(invalid());
    }
    for (i, memory) in quoted.iter().enumerate() {
        let id = get(memory, "id")?;
        c::uint(id, 1, u32::MAX as u64)?;
        if quoted[..i].iter().any(|v| v.get("id") == Some(id)) {
            return Err(invalid());
        }
    }
    let facts =
        get(ticket, "facts")?.as_object().ok_or_else(invalid)?.iter().map(|(k, _)| Value::str(k)).collect::<Vec<_>>();
    let memories = get(ticket, "memories")?
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|m| get(m, "id").cloned())
        .collect::<Result<Vec<_>>>()?;
    let fact_count = facts.len();
    let fact_item = if fact_count == 0 {
        obj! {"type"=>"string"}
    } else {
        obj! {"type"=>"string","enum"=>facts}
    };
    let count = memories.len();
    let memory_item = if count == 0 {
        obj! {"type"=>"integer"}
    } else {
        obj! {"type"=>"integer","enum"=>memories}
    };
    Ok(obj! {"type"=>"object","properties"=>obj! {
    "speech"=>obj! {"type"=>"string","maxLength"=>text_chars},"tone"=>obj! {"type"=>"string","enum"=>vec!["neutral","warm","wary"]},
    "fact_ids"=>obj! {"type"=>"array","items"=>fact_item,"maxItems"=>if fact_count==0 {0} else {8}},
    "memory_ids"=>obj! {"type"=>"array","items"=>memory_item,"maxItems"=>count.min(2)}},
    "required"=>vec!["speech","tone","fact_ids","memory_ids"],"additionalProperties"=>false})
}
