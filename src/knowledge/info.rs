//! Optional model-callable informational notebook. Ownership, location, preferences
//! and relationships are reported claims with original evidence, never engine truth.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    c::text(c::get(v, key)?)
}
pub(crate) fn sources(v: &Value) -> Result<&[Value]> {
    bounded(v, 65536)?;
    let rows = c::list(v)?;
    if rows.len() > 16 {
        return Err(c::bad());
    }
    for (i, s) in rows.iter().enumerate() {
        c::fields(s, &["id", "speaker", "text"])?;
        for k in ["id", "speaker"] {
            c::scope(text(s, k)?)?;
        }
        let body = text(s, "text")?;
        if body.is_empty()
            || body.chars().count() > 512
            || body.chars().any(char::is_control)
            || rows[..i].iter().any(|old| old.get("id") == s.get("id"))
        {
            return Err(c::bad());
        }
    }
    Ok(rows)
}
/// One host-chosen visibility/conversation scope. Use separate notebooks for separate
/// audiences; no global search, persistence, semantic verification or automatic promotion.
#[derive(Clone, Debug)]
pub struct Notes {
    scope: String,
    sequence: u32,
    notes: Vec<Value>,
    limits: NotesLimits,
}
/// Host-set limits (defaults are the earlier fixed bounds): most `notes` in a notebook and
/// the most serialized `bytes` in a recovered checkpoint.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "notes", default: 64, ceiling: 65_536 },
    crate::character::limits::Limit { name: "bytes", default: 131_072, ceiling: 268_435_456 },
];
crate::character::limits::module_limits!(NotesLimits, LIMITS);
impl Notes {
    pub fn new(scope: &str) -> Result<Self> {
        Self::within(scope, &NotesLimits::default())
    }
    /// An empty notebook with host-set [limits](LIMITS).
    pub fn within(scope: &str, l: &crate::character::limits::Limits) -> Result<Self> {
        c::scope(scope)?;
        Ok(Self { scope: scope.into(), sequence: 0, notes: Vec::new(), limits: NotesLimits(l.clone()) })
    }
    /// Original source IDs and text come from the host, outside model arguments.
    /// Source availability establishes provenance, not entailment of the model's claim.
    pub fn call(&mut self, name: &str, input: &str, evidence: &Value) -> Result<Value> {
        let a = crate::data::json::parse(input, 4096)?;
        let evidence = sources(evidence)?;
        match name {
            "info_lookup" => {
                c::fields(&a, &["subject", "attribute"])?;
                let subject = text(&a, "subject")?;
                let attribute = text(&a, "attribute")?;
                c::scope(subject)?;
                c::scope(attribute)?;
                let matching = self
                    .notes
                    .iter()
                    .filter(|n| n.get("subject") == a.get("subject") && n.get("attribute") == a.get("attribute"))
                    .collect::<Vec<_>>();
                Ok(
                    obj! {"scope"=>self.scope.clone(),"status"=>"reported","notes"=>matching.iter().skip(matching.len().saturating_sub(16)).map(|v|(*v).clone()).collect::<Vec<_>>()},
                )
            }
            "info_remember" => {
                c::fields(&a, &["subject", "attribute", "value", "source"])?;
                for k in ["subject", "attribute", "source"] {
                    c::scope(text(&a, k)?)?;
                }
                bounded(c::get(&a, "value")?, 1024)?;
                let source = evidence.iter().find(|s| s.get("id") == a.get("source")).ok_or_else(c::bad)?;
                if self.notes.len() >= self.limits.get("notes") {
                    return Err(c::bad());
                }
                let id = self.sequence.checked_add(1).ok_or_else(c::bad)?;
                let note = obj! {"id"=>id,"subject"=>c::get(&a,"subject")?.clone(),"attribute"=>c::get(&a,"attribute")?.clone(),"value"=>c::get(&a,"value")?.clone(),"status"=>"reported","source"=>source.clone()};
                self.sequence = id;
                self.notes.push(note.clone());
                Ok(note)
            }
            "info_forget" => {
                c::fields(&a, &["id"])?;
                c::uint(c::get(&a, "id")?, 1, u32::MAX as u64)?;
                let at = self.notes.iter().position(|n| n.get("id") == a.get("id")).ok_or_else(c::bad)?;
                self.notes.remove(at);
                Ok(obj! {"forgotten"=>c::get(&a,"id")?.clone()})
            }
            _ => Err(c::bad()),
        }
    }
    /// Register these optional function schemas with the host's dispatcher. Hosts bind
    /// this notebook and the current visible source list; neither comes from the model.
    pub fn tools(&self) -> Value {
        let string = obj! {"type"=>"string","minLength"=>1,"maxLength"=>64};
        let tool = |name: &str, description: &str, properties: Value, required: Vec<&str>| obj! {"type"=>"function","function"=>obj!{"name"=>name,"description"=>description,"parameters"=>obj!{"type"=>"object","properties"=>properties,"required"=>required,"additionalProperties"=>false}}};
        vec![
            tool("info_lookup","Look up reported information about a subject and attribute. Claims are unverified.",obj!{"subject"=>string.clone(),"attribute"=>string.clone()},vec!["subject","attribute"]),
            tool("info_remember","Record a reported claim using a supplied source ID. This changes no game facts, ownership or inventory.",obj!{"subject"=>string.clone(),"attribute"=>string.clone(),"value"=>obj!{},"source"=>string},vec!["subject","attribute","value","source"]),
            tool("info_forget","Forget one notebook claim by ID. This changes no game facts.",obj!{"id"=>obj!{"type"=>"integer","minimum"=>1,"maximum"=>u32::MAX}},vec!["id"])
        ].into()
    }
    pub(crate) fn records(&self) -> &[Value] {
        &self.notes
    }
    pub fn to_value(&self) -> Value {
        obj! {"scope"=>self.scope.clone(),"notes"=>self.notes.clone()}
    }
    /// Clear this entire audience scope, including retained source quotations. IDs
    /// are not reused, so stale forget requests cannot erase newly created notes.
    pub fn clear(&mut self) {
        self.notes.clear();
    }
}
/// The `info` local kind: `{scope, calls ≤128, limits?}` → `{limits?, scope, notes, results}`.
/// `limits` are the notebook's [host-set limits](LIMITS), echoed when supplied.
pub fn workflow(a: &Value) -> Result<Value> {
    let mut limits = NotesLimits::default();
    let stripped = crate::character::limits::take(a, &mut limits)?;
    let out = run_workflow(stripped.as_ref().unwrap_or(a), &limits)?;
    Ok(crate::character::limits::echo(out, &limits, stripped.is_some()))
}
fn run_workflow(a: &Value, l: &crate::character::limits::Limits) -> Result<Value> {
    bounded(a, 1_048_576)?;
    c::fields(a, &["scope", "calls"])?;
    let mut notes = Notes::within(text(a, "scope")?, l)?;
    let calls = c::list(c::get(a, "calls")?)?;
    if calls.len() > 128 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for call in calls {
        let result = (|| {
            c::fields(call, &["name", "text", "sources"])?;
            notes.call(text(call, "name")?, text(call, "text")?, c::get(call, "sources")?)
        })();
        results.push(match result {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"scope"=>notes.scope,"notes"=>notes.notes,"results"=>results})
}
impl Notes {
    /// Host-only durable notebook; unlike inspection it preserves erased-note IDs.
    pub fn checkpoint(&self) -> Value {
        obj! {"version"=>1,"scope"=>self.scope.clone(),"sequence"=>self.sequence,"notes"=>self.notes.clone()}
    }
    pub fn recover(scope: &str, v: &Value, min_sequence: u32) -> Result<Self> {
        Self::recover_within(scope, v, min_sequence, &NotesLimits::default())
    }
    /// [`recover`](Self::recover) with host-set [limits](LIMITS).
    pub fn recover_within(
        scope: &str,
        v: &Value,
        min_sequence: u32,
        l: &crate::character::limits::Limits,
    ) -> Result<Self> {
        bounded(v, l.get("bytes"))?;
        c::fields(v, &["version", "scope", "sequence", "notes"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        if text(v, "scope")? != scope {
            return Err(c::bad());
        }
        let sequence = c::uint(c::get(v, "sequence")?, u64::from(min_sequence), u32::MAX as u64)? as u32;
        let rows = c::list(c::get(v, "notes")?)?;
        if rows.len() > l.get("notes") {
            return Err(c::bad());
        }
        let mut out = Self::within(scope, l)?;
        let mut last = 0;
        for n in rows {
            c::fields(n, &["id", "subject", "attribute", "value", "status", "source"])?;
            let id = c::uint(c::get(n, "id")?, last + 1, u64::from(sequence))?;
            last = id;
            if text(n, "status")? != "reported" {
                return Err(c::bad());
            }
            let source = c::get(n, "source")?;
            let args = obj! {"subject"=>c::get(n,"subject")?.clone(),"attribute"=>c::get(n,"attribute")?.clone(),"value"=>c::get(n,"value")?.clone(),"source"=>c::get(source,"id")?.clone()};
            out.call("info_remember", &args.to_json(), &vec![source.clone()].into())?;
            *out.notes.last_mut().unwrap() = n.clone();
            out.sequence = id as u32;
        }
        out.sequence = sequence;
        Ok(out)
    }
}
