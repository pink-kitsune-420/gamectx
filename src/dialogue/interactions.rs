//! Optional conversation tools: visible events, reported promises/relationships,
//! host-bound references, and recipient-bound offers. No tool executes a world action.
use crate::{checked as c, decision::PendingAction, info::Notes, obj, request::bounded, Result, Value};
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
fn names(v: &Value, max: usize) -> Result<Vec<String>> {
    let a = c::list(v)?;
    if a.len() > max {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for v in a {
        let s = c::text(v)?;
        c::scope(s)?;
        if out.iter().any(|old| old == s) {
            return Err(c::bad());
        }
        out.push(s.into());
    }
    Ok(out)
}
fn body(s: &str) -> Result<()> {
    if s.is_empty() || s.chars().count() > 512 || s.chars().any(char::is_control) {
        Err(c::bad())
    } else {
        Ok(())
    }
}
fn fit(rows: Vec<Value>, limit: usize, bytes: usize) -> Value {
    let mut chosen = Vec::new();
    let mut used = 14; // compact {"records":[]}
    for row in rows.into_iter().rev() {
        if chosen.len() == limit {
            break;
        }
        let cost = row.json_len() + usize::from(!chosen.is_empty());
        if used + cost <= bytes {
            used += cost;
            chosen.insert(0, row);
        }
    }
    obj! {"records"=>chosen}
}
fn limit(v: &Value) -> Result<usize> {
    Ok(c::uint(c::get(v, "limit")?, 1, 8)? as usize)
}
#[derive(Clone, Debug)]
struct Pending {
    recipient: String,
    action: PendingAction,
}
/// One actor's private conversation/audience scope, with an explicit multiplayer
/// roster. Hosts supply events, evidence, reference context, offers and confirmations.
/// All schemas are optional; register only tools needed by this turn.
#[derive(Clone, Debug)]
pub struct InteractionTools {
    scope: String,
    actor: String,
    participants: Vec<String>,
    events: Vec<Value>,
    notes: Notes,
    progress: Vec<Value>,
    offers: Vec<Value>,
    pending: Option<Pending>,
    used: Vec<String>,
    references: Option<Value>,
}
impl InteractionTools {
    pub fn new(scope: &str, actor: &str, participants: &Value) -> Result<Self> {
        c::scope(scope)?;
        c::scope(actor)?;
        bounded(participants, 8192)?;
        let participants = names(participants, 16)?;
        if !participants.iter().any(|p| p == actor) {
            return Err(c::bad());
        }
        Ok(Self {
            scope: scope.into(),
            actor: actor.into(),
            participants,
            events: Vec::new(),
            notes: Notes::new(scope)?,
            progress: Vec::new(),
            offers: Vec::new(),
            pending: None,
            used: Vec::new(),
            references: None,
        })
    }
    /// Publish a host-confirmed event. Audience is explicit, not inferred from dialogue.
    pub fn publish(&mut self, e: &Value) -> Result<Value> {
        bounded(e, 8192)?;
        c::fields(e, &["id", "turn", "subject", "kind", "value", "audience"])?;
        for k in ["id", "subject", "kind"] {
            c::scope(text(e, k)?)?;
        }
        let turn = c::uint(c::get(e, "turn")?, 0, u32::MAX as u64)?;
        let audience = names(c::get(e, "audience")?, 16)?;
        bounded(c::get(e, "value")?, 1024)?;
        if self.events.len() >= 128
            || audience.is_empty()
            || audience.iter().any(|p| !self.participants.contains(p))
            || self.events.iter().any(|old| old.get("id") == e.get("id"))
            || self
                .events
                .last()
                .is_some_and(|old| old.get("turn").and_then(Value::as_usize).expect("turn") > turn as usize)
        {
            return Err(c::bad());
        }
        self.events.push(e.clone());
        Ok(obj! {"published"=>c::get(e,"id")?.clone()})
    }
    /// Host-only progress; model-reported promises begin unconfirmed. Completion
    /// requires an accepted promise, a fresh promise revision and host evidence.
    pub fn set_progress(&mut self, a: &Value) -> Result<Value> {
        bounded(a, 8192)?;
        c::fields(a, &["id", "expected", "state", "source"])?;
        c::uint(c::get(a, "id")?, 1, u32::MAX as u64)?;
        let expected = c::uint(c::get(a, "expected")?, 0, u32::MAX as u64)?;
        crate::info::sources(&vec![c::get(a, "source")?.clone()].into())?;
        let notes = self.notes.records();
        if !notes
            .iter()
            .any(|n| n.get("id") == a.get("id") && n.get("attribute").and_then(Value::as_str) == Some("promise"))
        {
            return Err(c::bad());
        }
        let at = self.progress.iter().position(|p| p.get("id") == a.get("id"));
        let (state, revision) = if let Some(at) = at {
            (text(&self.progress[at], "state")?, c::uint(c::get(&self.progress[at], "revision")?, 0, u32::MAX as u64)?)
        } else {
            ("reported", 0)
        };
        let next = text(a, "state")?;
        if expected != revision
            || revision == u32::MAX as u64
            || !matches!(
                (state, next),
                ("reported", "accepted" | "cancelled") | ("accepted", "fulfilled" | "cancelled")
            )
        {
            return Err(c::bad());
        }
        let result = obj! {"id"=>c::get(a,"id")?.clone(),"state"=>next,"revision"=>revision+1,"source"=>c::get(a,"source")?.clone()};
        if let Some(at) = at {
            self.progress[at] = result.clone();
        } else {
            self.progress.push(result.clone());
        }
        Ok(result)
    }
    pub fn set_references(&mut self, v: &Value) -> Result<Value> {
        super::resolve_references(v)?;
        if text(v, "scope")? != self.scope {
            return Err(c::bad());
        }
        self.references = Some(v.clone());
        Ok(obj! {"ready"=>true})
    }
    /// Host-enumerated exact decisions/terms and specific recipients. Replacing the
    /// menu does not reset consumed IDs. Confirmation rechecks the exact current entry.
    pub fn set_offers(&mut self, v: &Value) -> Result<Value> {
        bounded(v, 131072)?;
        let rows = c::list(v)?;
        if rows.len() > 16 {
            return Err(c::bad());
        }
        for (i, row) in rows.iter().enumerate() {
            c::fields(row, &["recipient", "offer"])?;
            let recipient = text(row, "recipient")?;
            if !self.participants.iter().any(|p| p == recipient) || recipient == self.actor {
                return Err(c::bad());
            }
            let offer = c::get(row, "offer")?;
            PendingAction::new(offer)?;
            if text(offer, "scope")? != self.scope
                || rows[..i].iter().any(|old| old.get("offer").and_then(|v| v.get("id")) == offer.get("id"))
            {
                return Err(c::bad());
            }
        }
        self.offers = rows.to_vec();
        Ok(obj! {"offers"=>rows.len()})
    }
    /// Host dispatch of one bounded model call. Evidence and current revision/output
    /// bound come from the host. Failed calls leave state unchanged, including notes
    /// and pending offers. `available` rechecks current permissions without executing.
    pub fn call(
        &mut self,
        name: &str,
        input: &str,
        evidence: &Value,
        revision: u32,
        max_bytes: usize,
        mut available: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        if !(64..=8192).contains(&max_bytes) {
            return Err(c::bad());
        }
        let a = crate::json::parse(input, 4096)?;
        if matches!(
            name,
            "events_recent" | "promise_lookup" | "relationship_lookup" | "dialogue_resolve" | "interaction_offers"
        ) {
            let result = self.apply(name, &a, evidence, revision, max_bytes, &mut available)?;
            bounded(&result, max_bytes)?;
            return Ok(result);
        }
        let mut draft = self.clone();
        let result = draft.apply(name, &a, evidence, revision, max_bytes, &mut available)?;
        bounded(&result, max_bytes)?;
        *self = draft;
        Ok(result)
    }
    fn apply(
        &mut self,
        name: &str,
        a: &Value,
        evidence: &Value,
        revision: u32,
        bytes: usize,
        available: &mut impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        match name {
            "events_recent" => {
                c::fields(a, &["since", "subject", "limit"])?;
                let since = c::uint(c::get(a, "since")?, 0, revision as u64)?;
                let subject = c::get(a, "subject")?;
                if *subject != Value::Null {
                    c::scope(c::text(subject)?)?;
                }
                let rows=self.events.iter().filter(|e|{
                    let turn=e.get("turn").and_then(Value::as_usize).expect("turn");
                    e.get("audience").and_then(Value::as_array).expect("audience").iter().any(|v|v.as_str()==Some(&self.actor))&&since as usize<=turn&&turn<=revision as usize&&(*subject==Value::Null||e.get("subject")==Some(subject))
                }).map(|e|obj!{"id"=>e.get("id").expect("id").clone(),"turn"=>e.get("turn").expect("turn").clone(),"subject"=>e.get("subject").expect("subject").clone(),"kind"=>e.get("kind").expect("kind").clone(),"value"=>e.get("value").expect("value").clone()}).collect();
                Ok(fit(rows, limit(a)?, bytes))
            }
            "promise_remember" => {
                c::fields(a, &["promisor", "beneficiary", "task", "source"])?;
                let from = text(a, "promisor")?;
                let to = text(a, "beneficiary")?;
                if from == to
                    || !self.participants.iter().any(|p| p == from)
                    || !self.participants.iter().any(|p| p == to)
                {
                    return Err(c::bad());
                }
                body(text(a, "task")?)?;
                self.notes.call("info_remember",&(obj!{"subject"=>from,"attribute"=>"promise","value"=>obj!{"beneficiary"=>to,"task"=>c::get(a,"task")?.clone()},"source"=>c::get(a,"source")?.clone()}).to_json(),evidence)
            }
            "promise_lookup" => {
                c::fields(a, &["participant", "limit"])?;
                let p = c::get(a, "participant")?;
                if *p != Value::Null && !self.participants.iter().any(|s| p.as_str() == Some(s)) {
                    return Err(c::bad());
                }
                let mut rows = Vec::new();
                for n in self.notes.records() {
                    if n.get("attribute").and_then(Value::as_str) == Some("promise")
                        && (*p == Value::Null
                            || n.get("subject") == Some(p)
                            || n.get("value").and_then(|v| v.get("beneficiary")) == Some(p))
                    {
                        let progress=self.progress.iter().find(|v|v.get("id")==n.get("id")).cloned().unwrap_or_else(||obj!{"id"=>n.get("id").expect("id").clone(),"state"=>"reported","revision"=>0,"source"=>Value::Null});
                        let Value::Object(mut fields) = n.clone() else { unreachable!() };
                        fields.push(("progress".into(), progress));
                        rows.push(Value::Object(fields));
                    }
                }
                Ok(fit(rows, limit(a)?, bytes))
            }
            "relationship_note" => {
                c::fields(a, &["other", "note", "source"])?;
                let other = text(a, "other")?;
                if other == self.actor || !self.participants.iter().any(|p| p == other) {
                    return Err(c::bad());
                }
                body(text(a, "note")?)?;
                self.notes.call("info_remember",&(obj!{"subject"=>self.actor.clone(),"attribute"=>"relationship","value"=>obj!{"other"=>other,"note"=>c::get(a,"note")?.clone()},"source"=>c::get(a,"source")?.clone()}).to_json(),evidence)
            }
            "relationship_lookup" => {
                c::fields(a, &["other", "limit"])?;
                let other = text(a, "other")?;
                if !self.participants.iter().any(|p| p == other) {
                    return Err(c::bad());
                }
                let rows = self
                    .notes
                    .records()
                    .iter()
                    .filter(|n| {
                        n.get("attribute").and_then(Value::as_str) == Some("relationship")
                            && n.get("value").and_then(|v| v.get("other")) == a.get("other")
                    })
                    .cloned()
                    .collect();
                Ok(fit(rows, limit(a)?, bytes))
            }
            "dialogue_resolve" => {
                c::fields(a, &["kind"])?;
                let v = self.references.as_ref().ok_or_else(c::bad)?;
                if c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)? != revision as u64 {
                    return Err(c::bad());
                }
                let Value::Object(mut fields) = v.clone() else { unreachable!() };
                *fields.iter_mut().find(|(k, _)| k == "kind").expect("kind") =
                    ("kind".into(), c::get(a, "kind")?.clone());
                super::resolve_references(&Value::Object(fields))
            }
            "interaction_offers" => {
                c::fields(a, &["limit"])?;
                let mut rows = Vec::new();
                for row in &self.offers {
                    let o = c::get(row, "offer")?;
                    if c::uint(c::get(o, "revision")?, 0, u32::MAX as u64)? == revision as u64
                        && !self.used.iter().any(|s| o.get("id").and_then(Value::as_str) == Some(s))
                        && available(c::get(o, "decision")?)
                    {
                        rows.push(row.clone());
                    }
                }
                Ok(fit(rows, limit(a)?, bytes))
            }
            "interaction_offer" => {
                c::fields(a, &["id"])?;
                let id = text(a, "id")?;
                c::scope(id)?;
                if self.pending.as_ref().is_some_and(|p| !p.action.is_finished())
                    || self.used.iter().any(|s| s == id)
                    || self.used.len() >= 256
                {
                    return Err(c::bad());
                }
                let row = self
                    .offers
                    .iter()
                    .find(|row| row.get("offer").and_then(|o| o.get("id")).and_then(Value::as_str) == Some(id))
                    .ok_or_else(c::bad)?;
                let offer = c::get(row, "offer")?;
                if c::uint(c::get(offer, "revision")?, 0, u32::MAX as u64)? != revision as u64
                    || !available(c::get(offer, "decision")?)
                {
                    return Err(c::bad());
                }
                self.pending =
                    Some(Pending { recipient: text(row, "recipient")?.into(), action: PendingAction::new(offer)? });
                self.used.push(id.into());
                Ok(obj! {"status"=>"proposed","recipient"=>c::get(row,"recipient")?.clone(),"offer"=>offer.clone()})
            }
            _ => Err(c::bad()),
        }
    }
    /// Explicit recipient confirmation is a host operation, never a generated tool
    /// argument. A wrong recipient cannot consume another player's pending offer.
    /// A confirmed proposal still requires atomic host dispatch at the same revision.
    pub fn respond(
        &mut self,
        recipient: &str,
        revision: u32,
        confirm: bool,
        terms: &Value,
        available: impl FnOnce(&Value) -> bool,
    ) -> Result<Value> {
        let pending = self.pending.as_mut().ok_or_else(c::bad)?;
        if recipient != pending.recipient {
            return Err(c::bad());
        }
        let original = pending.action.offer();
        let current = self.offers.iter().any(|row| {
            row.get("recipient").and_then(Value::as_str) == Some(recipient)
                && row.get("offer").is_some_and(|v| v.same(&original))
        });
        pending.action.respond(&self.scope, revision, confirm, terms, |p| current && available(p))
    }
    /// Erase retained event values, claims, evidence, reference text and pending
    /// offers for this scope. Opaque consumed offer IDs and note sequence remain to
    /// reject replay; drop the instance to erase identity and replay metadata too.
    pub fn clear(&mut self) {
        self.events.clear();
        self.notes.clear();
        self.progress.clear();
        self.offers.clear();
        self.references = None;
        self.pending = None;
    }
    /// Host inspection, including private event audiences. Never send this as context.
    pub fn to_value(&self) -> Value {
        let pending=self.pending.as_ref().map(|p|obj!{"recipient"=>p.recipient.clone(),"offer"=>p.action.offer(),"finished"=>p.action.is_finished()}).unwrap_or(Value::Null);
        obj! {"scope"=>self.scope.clone(),"actor"=>self.actor.clone(),"participants"=>self.participants.clone(),"events"=>self.events.clone(),"notes"=>self.notes.to_value().get("notes").expect("notes").clone(),"progress"=>self.progress.clone(),"used"=>self.used.clone(),"pending"=>pending}
    }
}
impl InteractionTools {
    /// Choose individual schemas; eight tools are never enabled automatically.
    pub fn tools(&self, names: &[&str]) -> Result<Value> {
        if names.len() > 8 {
            return Err(c::bad());
        }
        let string = obj! {"type"=>"string","minLength"=>1,"maxLength"=>64};
        let count = obj! {"type"=>"integer","minimum"=>1,"maximum"=>8};
        let mut out = Vec::new();
        for (i, name) in names.iter().enumerate() {
            if names[..i].contains(name) {
                return Err(c::bad());
            }
            let (description,properties,required)=match *name {
                "events_recent"=>("Read host-confirmed events visible to this character; since is inclusive.",obj!{"since"=>obj!{"type"=>"integer","minimum"=>0,"maximum"=>u32::MAX},"subject"=>obj!{"type"=>vec!["string","null"],"maxLength"=>64},"limit"=>count.clone()},vec!["since","subject","limit"]),
                "promise_remember"=>("Record a reported promise from a supplied source; this confirms or executes nothing.",obj!{"promisor"=>string.clone(),"beneficiary"=>string.clone(),"task"=>obj!{"type"=>"string","minLength"=>1,"maxLength"=>512},"source"=>string.clone()},vec!["promisor","beneficiary","task","source"]),
                "promise_lookup"=>("Look up promises and host-confirmed progress for a participant, or null for this scope.",obj!{"participant"=>obj!{"type"=>vec!["string","null"],"maxLength"=>64},"limit"=>count.clone()},vec!["participant","limit"]),
                "relationship_note"=>("Remember a reported preference, boundary or interaction about a participant with original evidence. This changes no reputation score.",obj!{"other"=>string.clone(),"note"=>obj!{"type"=>"string","minLength"=>1,"maxLength"=>512},"source"=>string.clone()},vec!["other","note","source"]),
                "relationship_lookup"=>("Look up reported relationship notes for this participant.",obj!{"other"=>string.clone(),"limit"=>count.clone()},vec!["other","limit"]),
                "dialogue_resolve"=>("Resolve references in the host-bound current utterance. Multiple candidates require clarification; this authorizes no action.",obj!{"kind"=>obj!{"type"=>vec!["string","null"],"maxLength"=>64}},vec!["kind"]),
                "interaction_offers"=>("List current host-allowed offers with exact terms and recipients.",obj!{"limit"=>count.clone()},vec!["limit"]),
                "interaction_offer"=>("Propose one host-supplied offer by ID. The specific recipient must confirm outside model calls; no action executes.",obj!{"id"=>string.clone()},vec!["id"]),
                _=>return Err(c::bad())
            };
            out.push(obj!{"type"=>"function","function"=>obj!{"name"=>*name,"description"=>description,"parameters"=>obj!{"type"=>"object","properties"=>properties,"required"=>required,"additionalProperties"=>false}}});
        }
        Ok(out.into())
    }
}
/// Stateless conformance/CLI adapter. All operations except `call` are host inputs.
pub fn interaction_workflow(a: &Value) -> Result<Value> {
    bounded(a, 1_048_576)?;
    c::fields(a, &["scope", "actor", "participants", "operations"])?;
    let mut tools = InteractionTools::new(text(a, "scope")?, text(a, "actor")?, c::get(a, "participants")?)?;
    let ops = c::list(c::get(a, "operations")?)?;
    if ops.len() > 128 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let v = c::get(op, "args")?;
        let result = (|| match text(op, "op")? {
            "clear" => {
                c::fields(v, &[])?;
                tools.clear();
                Ok(obj! {"cleared"=>true})
            }
            "event" => tools.publish(v),
            "progress" => tools.set_progress(v),
            "offers" => tools.set_offers(v),
            "references" => tools.set_references(v),
            "call" => {
                c::fields(v, &["name", "text", "sources", "revision", "max_bytes", "allowed"])?;
                let allowed = c::list(c::get(v, "allowed")?)?;
                if allowed.len() > 32 {
                    return Err(c::bad());
                }
                tools.call(
                    text(v, "name")?,
                    text(v, "text")?,
                    c::get(v, "sources")?,
                    c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)? as u32,
                    c::uint(c::get(v, "max_bytes")?, 64, 8192)? as usize,
                    |p| allowed.iter().any(|a| a.same(p)),
                )
            }
            "respond" => {
                c::fields(v, &["recipient", "revision", "confirm", "terms", "allowed"])?;
                let allowed = c::list(c::get(v, "allowed")?)?;
                if allowed.len() > 32 {
                    return Err(c::bad());
                }
                tools.respond(
                    text(v, "recipient")?,
                    c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)? as u32,
                    c::boolean(c::get(v, "confirm")?)?,
                    c::get(v, "terms")?,
                    |p| allowed.iter().any(|a| a.same(p)),
                )
            }
            _ => Err(c::bad()),
        })();
        results.push(match result {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"state"=>tools.to_value(),"results"=>results})
}
