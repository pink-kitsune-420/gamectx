//! Scene-scoped whole exchanges and protected scene hydration. Conversation text remains
//! data; old history is never silently promoted into authoritative facts or summaries.
use crate::budget::Section;
use crate::request::{bounded, BudgetedRequest, Count, RequestBudget, MAX_REQUEST_BYTES};
use crate::{checked as c, obj, Error, Result, Value};

#[derive(Clone, Debug)]
pub struct Thread {
    value: Value,
}
#[derive(Clone, Debug)]
pub struct Selected {
    pub messages: Value,
    pub kept: Vec<String>,
    pub omitted: Vec<String>,
}
impl Selected {
    pub fn to_value(&self) -> Value {
        obj! {"messages"=>self.messages.clone(),"kept"=>self.kept.clone(),"omitted"=>self.omitted.clone()}
    }
}
fn body(v: &Value, max: usize) -> Result<()> {
    let s = c::text(v)?;
    if s.is_empty() || s.len() > max {
        return Err(c::bad());
    }
    Ok(())
}
fn messages(exchanges: &[&Value]) -> Value {
    Value::Array(
        exchanges
            .iter()
            .flat_map(|e| {
                [
                    obj! {"role"=>"user","content"=>e.get("user").expect("user").clone()},
                    obj! {"role"=>"assistant","content"=>e.get("assistant").expect("assistant").clone()},
                ]
            })
            .collect(),
    )
}
impl Thread {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 262_144)?;
        c::fields(v, &["version", "scene", "conversation", "participants", "exchanges"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        c::scope(c::text(c::get(v, "scene")?)?)?;
        c::scope(c::text(c::get(v, "conversation")?)?)?;
        let people = c::list(c::get(v, "participants")?)?;
        if people.is_empty() || people.len() > 16 {
            return Err(c::bad());
        }
        for (i, p) in people.iter().enumerate() {
            c::scope(c::text(p)?)?;
            if people[..i].contains(p) {
                return Err(c::bad());
            }
        }
        let exchanges = c::list(c::get(v, "exchanges")?)?;
        if exchanges.len() > 128 {
            return Err(c::bad());
        }
        let mut previous = None;
        for (i, e) in exchanges.iter().enumerate() {
            c::fields(e, &["id", "turn", "user", "assistant"])?;
            c::scope(c::text(c::get(e, "id")?)?)?;
            let turn = c::uint(c::get(e, "turn")?, 0, u32::MAX as u64)?;
            if previous.is_some_and(|p| p >= turn) || exchanges[..i].iter().any(|p| p.get("id") == e.get("id")) {
                return Err(c::bad());
            }
            previous = Some(turn);
            body(c::get(e, "user")?, 4096)?;
            body(c::get(e, "assistant")?, 4096)?;
        }
        Ok(Self { value: v.clone() })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    /// Protect the newest `protect` whole exchanges; try up to `maximum`. The current
    /// request ID is removed from the thread so the same input is not sent twice.
    /// If protected dialogue alone does not fit, fail instead of shortening its wording.
    pub fn select(
        &self,
        current: &str,
        protect: usize,
        maximum: usize,
        tokens: usize,
        mut counter: impl FnMut(&Value) -> Result<Count>,
    ) -> Result<Selected> {
        c::scope(current)?;
        if protect > 3 || maximum > 128 || protect > maximum || tokens > crate::request::MAX_TOKENS {
            return Err(c::bad());
        }
        let all = c::list(c::get(&self.value, "exchanges")?)?;
        let eligible = all.iter().filter(|e| e.get("id").and_then(Value::as_str) != Some(current)).collect::<Vec<_>>();
        let mut selected = eligible.iter().skip(eligible.len().saturating_sub(maximum)).copied().collect::<Vec<_>>();
        loop {
            let msgs = messages(&selected);
            let count = if selected.is_empty() {
                Count { input: 0, method: crate::request::CountMethod::Estimate }
            } else {
                counter(&obj! {"messages"=>msgs.clone()})?
            };
            if count.input > crate::request::MAX_TOKENS {
                return Err(c::bad());
            }
            if count.input <= tokens {
                let kept = selected
                    .iter()
                    .map(|e| c::text(c::get(e, "id")?).map(str::to_owned))
                    .collect::<Result<Vec<_>>>()?;
                let omitted = all
                    .iter()
                    .map(|e| c::text(c::get(e, "id")?))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .filter(|id| !kept.iter().any(|k| k == id))
                    .map(str::to_owned)
                    .collect();
                return Ok(Selected { messages: msgs, kept, omitted });
            }
            if selected.len() <= protect.min(eligible.len()) {
                return Err(Error::new("protected dialogue exceeds budget; narrow the task"));
            }
            selected.remove(0);
        }
    }
    /// Oldest eligible half by measured prompt tokens, at whole exchange boundaries.
    /// Returns candidates only. The host verifies/archives events before deleting history.
    /// The protected tail and current request are excluded; no summary is generated.
    pub fn compaction_window(
        &self,
        current: &str,
        protect: usize,
        mut counter: impl FnMut(&Value) -> Result<Count>,
    ) -> Result<Value> {
        c::scope(current)?;
        if protect > 3 {
            return Err(c::bad());
        }
        let eligible = c::list(c::get(&self.value, "exchanges")?)?
            .iter()
            .filter(|e| e.get("id").and_then(Value::as_str) != Some(current))
            .collect::<Vec<_>>();
        let eligible = &eligible[..eligible.len().saturating_sub(protect)];
        if eligible.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        let target = counter(&obj! {"messages"=>messages(eligible)})?.input.div_ceil(2);
        for end in 1..=eligible.len() {
            if counter(&obj! {"messages"=>messages(&eligible[..end])})?.input >= target {
                return Ok(Value::Array(eligible[..end].iter().map(|e| (*e).clone()).collect()));
            }
        }
        Ok(Value::Array(eligible.iter().map(|e| (*e).clone()).collect()))
    }
}
#[derive(Clone, Debug)]
pub struct Scene {
    value: Value,
    thread: Thread,
}
#[derive(Clone, Debug)]
pub struct Hydrated {
    pub request: BudgetedRequest,
    pub dialogue: Selected,
}
impl Hydrated {
    pub fn to_value(&self) -> Value {
        let Value::Object(mut fields) = self.request.to_value() else { unreachable!() };
        fields.push(("dialogue".into(), self.dialogue.to_value()));
        Value::Object(fields)
    }
}
/// Native chat messages with rules once in the system role, exact prior exchanges, and
/// the current scene/outcome/input as JSON data in the final user message. No ChatML.
pub fn render_chat(packet: &Value) -> Result<Value> {
    let fields = packet.as_object().ok_or_else(c::bad)?;
    let mut msgs = vec![obj! {"role"=>"system","content"=>c::get(packet,"rules")?.clone()}];
    msgs.extend(c::list(c::get(packet, "dialogue")?)?.iter().cloned());
    let data = Value::Object(fields.iter().filter(|(k, _)| k != "rules" && k != "dialogue").cloned().collect());
    msgs.push(obj! {"role"=>"user","content"=>data.to_json()});
    Ok(obj! {"messages"=>msgs})
}
impl Scene {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, MAX_REQUEST_BYTES)?;
        c::fields(v, &["version", "id", "rules", "current", "outcome", "required_facts", "dialogue", "optional"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        c::scope(c::text(c::get(v, "id")?)?)?;
        body(c::get(v, "rules")?, 8192)?;
        let current = c::get(v, "current")?;
        c::fields(current, &["id", "text"])?;
        c::scope(c::text(c::get(current, "id")?)?)?;
        body(c::get(current, "text")?, 8192)?;
        let thread = Thread::from_value(c::get(v, "dialogue")?)?;
        if thread.value.get("scene") != v.get("id") {
            return Err(Error::new("dialogue belongs to another scene"));
        }
        if c::get(v, "required_facts")?.as_object().ok_or_else(c::bad)?.len() > 64 {
            return Err(c::bad());
        }
        let optional = c::list(c::get(v, "optional")?)?;
        if optional.len() > 32 {
            return Err(c::bad());
        }
        for (i, x) in optional.iter().enumerate() {
            c::fields(x, &["name", "priority", "value"])?;
            let n = c::text(c::get(x, "name")?)?;
            c::scope(n)?;
            if ["rules", "scene", "facts", "outcome", "dialogue", "input"].contains(&n)
                || optional[..i].iter().any(|old| old.get("name") == x.get("name"))
            {
                return Err(c::bad());
            }
            priority(c::get(x, "priority")?)?;
        }
        Ok(Self { value: v.clone(), thread })
    }
    /// All facts supplied here are required. The host resolves current authoritative
    /// revisions/privacy before hydration; this composer neither infers nor certifies truth.
    pub fn assemble(
        &self,
        budget: &RequestBudget,
        protect: usize,
        maximum: usize,
        dialogue_tokens: usize,
        mut renderer: impl FnMut(&Value) -> Result<Value>,
        mut counter: impl FnMut(&Value) -> Result<Count>,
    ) -> Result<Hydrated> {
        let current = c::get(&self.value, "current")?;
        let mut maximum = maximum;
        let mut omit_optional = false;
        loop {
            let dialogue = self.thread.select(
                c::text(c::get(current, "id")?)?,
                protect,
                maximum,
                dialogue_tokens,
                &mut counter,
            )?;
            let section = |name: &str, value: Value, priority: i64, stable: bool, required: bool| Section {
                name: name.into(),
                value,
                priority,
                stable,
                required,
            };
            let mut sections = vec![
                section("rules", c::get(&self.value, "rules")?.clone(), 1_000_001, true, true),
                section("scene", c::get(&self.value, "id")?.clone(), 1_000_001, false, true),
                section("facts", c::get(&self.value, "required_facts")?.clone(), 1_000_001, false, true),
            ];
            let mut forced_dropped = Vec::new();
            for (i, x) in c::list(c::get(&self.value, "optional")?)?.iter().enumerate() {
                let n = c::text(c::get(x, "name")?)?;
                let value = c::get(x, "value")?;
                let p = priority(c::get(x, "priority")?)?;
                if omit_optional {
                    forced_dropped.push((
                        i,
                        crate::budget::Line { name: n.into(), bytes: value.json_len(), priority: p, stable: false },
                    ));
                } else {
                    sections.push(section(n, value.clone(), p, false, false));
                }
            }
            sections.extend([
                section("outcome", c::get(&self.value, "outcome")?.clone(), 1_000_001, false, true),
                section("dialogue", dialogue.messages.clone(), 1_000_001, false, true),
                section("input", current.clone(), 1_000_001, false, true),
            ]);
            let callback_failed = std::cell::Cell::new(false);
            let built = budget.assemble(
                sections,
                |v| {
                    renderer(v).inspect_err(|_| {
                        callback_failed.set(true);
                    })
                },
                |v| {
                    counter(v).inspect_err(|_| {
                        callback_failed.set(true);
                    })
                },
            );
            match built {
                Ok(mut request) => {
                    forced_dropped.sort_by_key(|(i, l)| (l.priority, std::cmp::Reverse(*i)));
                    request.packet.receipt.dropped.extend(forced_dropped.into_iter().map(|(_, l)| l));
                    return Ok(Hydrated { request, dialogue });
                }
                Err(e) => {
                    if callback_failed.get() || dialogue.kept.len() <= protect {
                        return Err(e);
                    }
                    // All optional lore was exhausted by the first assembly. Only then
                    // remove an older unprotected exchange, never restore lore ahead of it.
                    maximum = dialogue.kept.len() - 1;
                    omit_optional = true;
                }
            }
        }
    }
}
fn priority(v: &Value) -> Result<i64> {
    let Value::Number(s) = v else { return Err(c::bad()) };
    s.parse::<i64>().ok().filter(|n| (-1_000_000..=1_000_000).contains(n)).ok_or_else(c::bad)
}
