//! Optional coordinator over existing character plans, references and RequestGate.
//! Host acknowledgements describe delivery; generated text is never evidence of hearing.
use super::*;
use crate::requests::{RequestContext, RequestGate, RequestTicket};
#[derive(Debug)]
struct Pending {
    id: u64,
    ticket: RequestTicket,
    scene: String,
    epoch: u64,
    deps: Vec<Value>,
    life: u64,
    thread: Value,
}
#[derive(Debug)]
pub struct Coordinator {
    scope: String,
    actor: String,
    participants: Vec<String>,
    scene: Option<Value>,
    threads: Vec<Value>,
    gate: RequestGate,
    pending: Vec<Pending>,
    speech: Vec<Value>,
    approved: Vec<(u64, String, u64, Vec<Value>, Value)>,
}
impl Coordinator {
    pub fn new(scope: &str, actor: &str, participants: &Value) -> Result<Self> {
        c::scope(scope)?;
        c::scope(actor)?;
        let participants = names(participants, 16)?;
        if !participants.iter().any(|p| p == actor) {
            return Err(c::bad());
        }
        Ok(Self {
            scope: scope.into(),
            actor: actor.into(),
            participants,
            scene: None,
            threads: Vec::new(),
            gate: RequestGate::new(),
            pending: Vec::new(),
            speech: Vec::new(),
            approved: Vec::new(),
        })
    }
    pub fn bind_scene(&mut self, v: &Value) -> Result<()> {
        bounded(v, 65536)?;
        c::fields(v, &["id", "revision", "epoch", "at", "participants", "facts", "affordances"])?;
        c::scope(text(v, "id")?)?;
        num(v, "epoch")?;
        num(v, "revision")?;
        num(v, "at")?;
        if names(get(v, "participants")?, 16)? != self.participants {
            return Err(c::bad());
        }
        names(get(v, "affordances")?, 16)?;
        if let Some(old) = &self.scene {
            if num(v, "revision")? < num(old, "revision")? || num(v, "at")? < num(old, "at")? {
                return Err(c::bad());
            }
        }
        let facts = c::list(get(v, "facts")?)?;
        if facts.len() > 32 {
            return Err(c::bad());
        }
        for (i, f) in facts.iter().enumerate() {
            c::fields(f, &["id", "revision", "observed_at", "source", "kind", "audience", "text"])?;
            c::scope(text(f, "id")?)?;
            num(f, "revision")?;
            if num(f, "observed_at")? > num(v, "at")? {
                return Err(c::bad());
            }
            line(text(f, "text")?)?;
            crate::info::sources(&vec![get(f, "source")?.clone()].into())?;
            if !matches!(text(f, "kind")?, "global" | "perceived" | "mutual" | "report")
                || !subset(&names(get(f, "audience")?, 16)?, &self.participants)
                || facts[..i].iter().any(|o| o.get("id") == f.get("id"))
            {
                return Err(c::bad());
            }
        }
        self.scene = Some(v.clone());
        Ok(())
    }
    /// Host topic/commitment IDs, not an English parser. Use existing reference resolution
    /// and interaction promises to supply selected referents and accepted commitments.
    pub fn thread(&mut self, v: &Value) -> Result<()> {
        bounded(v, 16384)?;
        c::fields(
            v,
            &["id", "topic", "questions", "promises", "unfinished", "listener", "participants", "shared", "referents"],
        )?;
        c::scope(text(v, "id")?)?;
        line(text(v, "topic")?)?;
        if names(get(v, "participants")?, 16)? != self.participants
            || !self.participants.iter().any(|p| p == text(v, "listener").unwrap_or(""))
        {
            return Err(c::bad());
        }
        for k in ["promises", "unfinished", "shared", "referents"] {
            names(get(v, k)?, 16)?;
        }
        let q = c::list(get(v, "questions")?)?;
        if q.len() > 8 {
            return Err(c::bad());
        }
        for x in q {
            line(c::text(x)?)?;
        }
        if let Some(i) = self.threads.iter().position(|t| t.get("id") == v.get("id")) {
            self.threads[i] = v.clone();
        } else {
            if self.threads.len() >= 8 {
                return Err(c::bad());
            }
            self.threads.push(v.clone());
        }
        Ok(())
    }
    fn context(&self, life: u64) -> Result<RequestContext> {
        let s = self.scene.as_ref().ok_or_else(c::bad)?;
        RequestContext::from_value(
            &obj! {"actor"=>self.actor.clone(),"world"=>text(s,"id")?,"schema"=>life,"permissions"=>num(s,"epoch")?,"facts"=>Vec::<Value>::new()},
        )
    }
    pub fn generate(&mut self, thread: &str, speech: &str, dependencies: &Value, life_revision: u64) -> Result<Value> {
        line(speech)?;
        let deps = names(dependencies, 16)?;
        let scene = self.scene.as_ref().ok_or_else(c::bad)?;
        let thread = self
            .threads
            .iter()
            .find(|t| t.get("id").and_then(Value::as_str) == Some(thread))
            .ok_or_else(c::bad)?
            .clone();
        if self.pending.len() >= 8 || self.speech.len() >= 32 {
            return Err(c::bad());
        }
        let mut selected = Vec::new();
        for id in deps {
            let f = c::list(get(scene, "facts")?)?
                .iter()
                .find(|f| f.get("id").and_then(Value::as_str) == Some(&id))
                .ok_or_else(c::bad)?;
            if text(f, "kind")? == "global" || !subset(&self.participants, &names(get(f, "audience")?, 16)?) {
                return Err(c::bad());
            }
            selected.push(f.clone());
        }
        let ticket = self.gate.issue(self.context(life_revision)?, num(scene, "at")?, 20)?;
        let id = ticket.id();
        let record = obj! {"id"=>id,"thread"=>get(&thread,"id")?.clone(),"text"=>speech,"stage"=>"generated","played"=>0,"heard"=>Vec::<Value>::new()};
        self.pending.push(Pending {
            id,
            ticket,
            scene: text(scene, "id")?.into(),
            epoch: num(scene, "epoch")?,
            deps: selected,
            life: life_revision,
            thread,
        });
        self.speech.push(record);
        Ok(obj! {"id"=>id,"stage"=>"generated"})
    }
    pub fn validate(&mut self, id: u64, life_revision: u64) -> Result<Value> {
        let i = self.pending.iter().position(|p| p.id == id).ok_or_else(c::bad)?;
        let current = self.context(life_revision)?;
        let scene = self.scene.as_ref().ok_or_else(c::bad)?;
        let now = num(scene, "at")?;
        let p = self.pending.remove(i);
        let valid = self.gate.accept(&p.ticket, &current, now, &crate::facts::FactRegistry::new(1)?, &[])?
            && p.scene == text(scene, "id")?
            && p.epoch == num(scene, "epoch")?
            && p.life == life_revision
            && self.threads.iter().any(|t| t == &p.thread)
            && p.deps.iter().all(|f| c::list(get(scene, "facts").expect("scene")).expect("facts").contains(f));
        if valid {
            self.approved.push((id, p.scene, p.epoch, p.deps, p.thread));
        }
        let record = self.speech.iter_mut().find(|s| num(s, "id").ok() == Some(id)).ok_or_else(c::bad)?;
        set(record, "stage", Value::str(if valid { "validated" } else { "cancelled" }));
        Ok(obj! {"id"=>id,"stage"=>get(record,"stage")?.clone()})
    }
    /// Host transport acknowledgement. Character offsets are Unicode scalar counts,
    /// cumulative and monotonic. `heard` records only the recipient-confirmed prefix.
    pub fn delivery(&mut self, v: &Value) -> Result<Value> {
        c::fields(v, &["id", "event", "through", "recipient"])?;
        let id = num(v, "id")?;
        let event = text(v, "event")?;
        let through = num(v, "through")?;
        let i = self.speech.iter().position(|s| num(s, "id").ok() == Some(id)).ok_or_else(c::bad)?;
        let mut s = self.speech[i].clone();
        if matches!(event, "queued" | "played") {
            let binding = self.approved.iter().find(|p| p.0 == id).ok_or_else(c::bad)?;
            let scene = self.scene.as_ref().ok_or_else(c::bad)?;
            if binding.1 != text(scene, "id")?
                || binding.2 != num(scene, "epoch")?
                || !binding.3.iter().all(|f| c::list(get(scene, "facts").expect("facts")).expect("facts").contains(f))
                || !self.threads.contains(&binding.4)
            {
                return Err(c::bad());
            }
        }
        let stage = text(&s, "stage")?;
        let played = num(&s, "played")?;
        if through > text(&s, "text")?.chars().count() as u64 {
            return Err(c::bad());
        }
        match event {
            "queued" if stage == "validated" && through == 0 => {}
            "played" if matches!(stage, "queued" | "played") && through >= played => {
                set(&mut s, "played", Value::from(through));
            }
            "heard" if matches!(stage, "played" | "interrupted") && through <= played => {
                let recipient = text(v, "recipient")?;
                if !self.participants.iter().any(|p| p == recipient) {
                    return Err(c::bad());
                }
                let mut heard = c::list(get(&s, "heard")?)?.to_vec();
                if let Some(h) =
                    heard.iter_mut().find(|h| h.get("recipient").and_then(Value::as_str) == Some(recipient))
                {
                    if through <= num(h, "through")? {
                        return Err(c::bad());
                    }
                    set(h, "through", Value::from(through));
                } else {
                    heard.push(obj! {"recipient"=>recipient,"through"=>through});
                }
                set(&mut s, "heard", heard.into());
            }
            "interrupted" if matches!(stage, "queued" | "played") && through == played => {}
            "cancelled" if matches!(stage, "generated" | "validated" | "queued") && through == 0 => {}
            _ => return Err(c::bad()),
        }
        if event != "heard" {
            set(&mut s, "stage", Value::str(event));
        }
        if event == "cancelled" {
            if let Some(at) = self.pending.iter().position(|p| p.id == id) {
                let p = self.pending.remove(at);
                self.gate.cancel(&p.ticket, num(self.scene.as_ref().ok_or_else(c::bad)?, "at")?)?;
            }
        }
        let result = obj! {"id"=>id,"stage"=>get(&s,"stage")?.clone(),"played"=>num(&s,"played")?,"heard"=>get(&s,"heard")?.clone()};
        self.speech[i] = s;
        Ok(result)
    }
    /// Caller archives confirmed portions before retiring a terminal line. Pending
    /// requests cannot be discarded through this operation.
    pub fn retire(&mut self, id: u64) -> Result<()> {
        let i = self.speech.iter().position(|s| num(s, "id").ok() == Some(id)).ok_or_else(c::bad)?;
        if self.pending.iter().any(|p| p.id == id)
            || !matches!(text(&self.speech[i], "stage")?, "cancelled" | "interrupted" | "played")
        {
            return Err(c::bad());
        }
        self.speech.remove(i);
        self.approved.retain(|p| p.0 != id);
        Ok(())
    }
    /// Hydrate only actor-scoped facts; host inspection remains separate. Feed this
    /// protected value into existing TurnPlan facts and recount the full request.
    pub fn focused(&self, thread: &str, life: &Life, tags: &Value) -> Result<Value> {
        if !life.identity_matches(&self.scope, &self.actor, &self.participants) {
            return Err(c::bad());
        }
        let scene = self.scene.as_ref().ok_or_else(c::bad)?;
        let t = self.threads.iter().find(|t| t.get("id").and_then(Value::as_str) == Some(thread)).ok_or_else(c::bad)?;
        let facts = c::list(get(scene, "facts")?)?
            .iter()
            .filter(|f| {
                text(f, "kind").ok() != Some("global")
                    && subset(&self.participants, &names(get(f, "audience").expect("audience"), 16).expect("audience"))
            })
            .cloned()
            .collect::<Vec<_>>();
        Ok(
            obj! {"scene"=>text(scene,"id")?,"at"=>num(scene,"at")?,"facts"=>facts,"affordances"=>get(scene,"affordances")?.clone(),"thread"=>t.clone(),"character"=>life.context(tags,4)?},
        )
    }
    pub fn inspect(&self) -> Value {
        obj! {"scene"=>self.scene.clone().unwrap_or(Value::Null),"threads"=>self.threads.clone(),"speech"=>self.speech.clone()}
    }
    /// Final STT input remains attributed text. Interim recognition cannot generate
    /// a reply or action through this helper; ambiguity stays available to the host.
    pub fn recognition(&self, v: &Value) -> Result<Value> {
        c::fields(v, &["text", "final", "uncertainty"])?;
        line(text(v, "text")?)?;
        if get(v, "uncertainty")? != &Value::Null {
            line(text(v, "uncertainty")?)?;
        }
        Ok(
            obj! {"status"=>if c::boolean(get(v,"final")?)?{"final_text"}else{"interim"},"text"=>get(v,"text")?.clone(),"uncertainty"=>get(v,"uncertainty")?.clone(),"actions"=>Vec::<Value>::new()},
        )
    }
}
pub(crate) fn set(v: &mut Value, key: &str, value: Value) {
    if let Value::Object(fields) = v {
        if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k == key) {
            *old = value;
        }
    }
}
/// Optional host transport. Implement with text UI, streaming audio or an external
/// speech service. Queue/cancel results are not hearing receipts.
pub trait SpeechTransport {
    fn queue(&mut self, id: u64, text: &str) -> Result<()>;
    fn cancel(&mut self, id: u64) -> Result<()>;
}
pub fn fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["scope", "actor", "participants", "operations"])?;
    let mut co = Coordinator::new(text(a, "scope")?, text(a, "actor")?, get(a, "participants")?)?;
    let ops = c::list(get(a, "operations")?)?;
    if ops.len() > 1024 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let v = get(op, "args")?;
        let result = match text(op, "op")? {
            "scene" => co.bind_scene(v).map(|()| obj! {"bound"=>true}),
            "thread" => co.thread(v).map(|()| obj! {"bound"=>true}),
            "generate" => {
                c::fields(v, &["thread", "text", "dependencies", "life_revision"])?;
                co.generate(text(v, "thread")?, text(v, "text")?, get(v, "dependencies")?, num(v, "life_revision")?)
            }
            "validate" => {
                c::fields(v, &["id", "life_revision"])?;
                co.validate(num(v, "id")?, num(v, "life_revision")?)
            }
            "delivery" => co.delivery(v),
            "inspect" => {
                c::fields(v, &[])?;
                Ok(co.inspect())
            }
            _ => Err(c::bad()),
        };
        results.push(match result {
            Ok(r) => obj! {"ok"=>true,"result"=>r},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results})
}
