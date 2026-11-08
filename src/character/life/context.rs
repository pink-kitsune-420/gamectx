//! Explicit attention caps and stable-prefix rendering. Size hints are advisory;
//! every changed provider request still needs a complete rendered count.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct ContextCaps {
    pub facts: usize,
    pub facets: usize,
    pub attitudes: usize,
    pub episodes: usize,
    pub exchanges: usize,
}
impl ContextCaps {
    pub fn focused() -> Self {
        Self { facts: 4, facets: 2, attitudes: 2, episodes: 2, exchanges: 1 }
    }
    pub fn expanded() -> Self {
        Self { facts: 16, facets: 8, attitudes: 8, episodes: 8, exchanges: 4 }
    }
    pub fn from_hints(h: crate::deploy::deployment::WorkloadHints) -> Result<Self> {
        Self::new(
            if h.exchanges <= 1 { 4 } else { 16 },
            if h.exchanges <= 1 { 2 } else { 8 },
            if h.exchanges <= 1 { 2 } else { 8 },
            h.recall,
            h.exchanges,
        )
    }
    pub fn new(facts: usize, facets: usize, attitudes: usize, episodes: usize, exchanges: usize) -> Result<Self> {
        if facts > 32 || facets > 16 || attitudes > 16 || episodes > 16 || exchanges > 16 {
            return Err(c::bad());
        }
        Ok(Self { facts, facets, attitudes, episodes, exchanges })
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        c::fields(v, &["facts", "facets", "attitudes", "episodes", "exchanges"])?;
        Self::new(
            num(v, "facts")? as usize,
            num(v, "facets")? as usize,
            num(v, "attitudes")? as usize,
            num(v, "episodes")? as usize,
            num(v, "exchanges")? as usize,
        )
    }
}
impl Life {
    /// Legacy v1 caps only; use `context_for_turn` for tier/known-state policy gating.
    pub fn context_capped(&self, tags: &Value, caps: ContextCaps) -> Result<Value> {
        ContextCaps::new(caps.facts, caps.facets, caps.attitudes, caps.episodes, caps.exchanges)?;
        let mut v = self.context(tags, caps.facets.max(caps.attitudes))?;
        for (k, cap) in [("facets", caps.facets), ("attitudes", caps.attitudes)] {
            let mut rows = c::list(get(&v, k)?)?.to_vec();
            rows.truncate(cap);
            super::continuity::set(&mut v, k, rows.into());
        }
        Ok(v)
    }
}
impl Life {
    /// Profile and known-state-aware character context, separate from the frozen legacy
    /// context format. Focused and unknown/partial factual turns get no creative invitation.
    pub fn context_for_turn(&self, tags: &Value, caps: ContextCaps, turn: &SpeechTurn) -> Result<Value> {
        ContextCaps::new(caps.facts, caps.facets, caps.attitudes, caps.episodes, caps.exchanges)?;
        turn.validate_scope(self, caps)?;
        let expressive = self.config.balanced && turn.expressive()?;
        let mut value =
            self.context_with_policy(tags, caps.facets.max(caps.attitudes), self.expression_policy(!expressive)?)?;
        for (k, cap) in [("facets", caps.facets), ("attitudes", caps.attitudes)] {
            let mut rows = c::list(get(&value, k)?)?.to_vec();
            rows.truncate(cap);
            super::continuity::set(&mut value, k, rows.into());
        }
        Ok(value)
    }
    /// Compile a version-2 host request, keeping stable context first and the actual
    /// utterance outside JSON at the end. Count the entire returned provider request.
    pub fn compile_speech(&self, a: &Value) -> Result<Value> {
        self.active()?;
        bounded(a, crate::packet::request::MAX_REQUEST_BYTES)?;
        c::fields(
            a,
            &["version", "rules", "tags", "caps", "stable_scene", "state", "dialogue", "input", "tools", "turn"],
        )?;
        c::uint(get(a, "version")?, 2, 2)?;
        let caps = ContextCaps::from_value(get(a, "caps")?)?;
        let turn = SpeechTurn::from_value(get(a, "turn")?)?;
        let character = self.context_for_turn(get(a, "tags")?, caps, &turn)?;
        if c::list(get(a, "dialogue")?)?.len() > caps.exchanges || c::list(get(a, "tools")?)?.len() > 32 {
            return Err(c::bad());
        }
        let mut request = render_character_chat(
            &obj! {"version"=>1,"rules"=>get(a,"rules")?.clone(),"character"=>character,"stable_scene"=>get(a,"stable_scene")?.clone(),"state"=>obj!{"context"=>get(a,"state")?.clone(),"facts"=>turn.scoped_facts(&self.actor)?},"dialogue"=>get(a,"dialogue")?.clone(),"input"=>get(a,"input")?.clone(),"tools"=>get(a,"tools")?.clone()},
        )?;
        let subject_rule=format!("Current utterance speaker: {}. First-person I/my in the current request refers to that speaker. Question subject: {}. Answer about that subject, not your own biography.",text(&turn.value,"speaker")?,text(&turn.value,"question_subject")?);
        if let Value::Object(fields) = &mut request {
            if let Some((_, Value::Array(messages))) = fields.iter_mut().find(|(k, _)| k == "messages") {
                let at = messages.len() - 1;
                messages.insert(at, obj! {"role"=>"system","content"=>subject_rule});
                let at = messages.len() - 1;
                messages[at] = obj! {"role"=>"user","content"=>format!("Current request:\n{}",text(a,"input")?)};
            }
        }
        bounded(&request, crate::packet::request::MAX_REQUEST_BYTES)?;
        Ok(request)
    }
}
/// Stable system/character/host-marked scene prefix followed by changing state and whole
/// exchanges. Callers own actual cache behavior and the complete provider renderer/count.
/// Stable scene values must be authorized facts, not global truth or inferred perception.
pub fn render_character_chat(v: &Value) -> Result<Value> {
    bounded(v, crate::packet::request::MAX_REQUEST_BYTES)?;
    c::fields(v, &["version", "rules", "character", "stable_scene", "state", "dialogue", "input", "tools"])?;
    c::uint(get(v, "version")?, 1, 1)?;
    line(text(v, "rules")?)?;
    line(text(v, "input")?)?;
    let character = get(v, "character")?;
    let mut stable = character.clone();
    if let Value::Object(fields) = &mut stable {
        fields.retain(|(k, _)| k == "facets" || k == "policy");
    }
    let mut messages = vec![
        obj! {"role"=>"system","content"=>text(v,"rules")?},
        obj! {"role"=>"system","content"=>obj!{"character"=>stable,"scene"=>get(v,"stable_scene")?.clone()}.to_json()},
    ];
    let mut changing = character.clone();
    if let Value::Object(fields) = &mut changing {
        fields.retain(|(k, _)| k != "facets" && k != "policy");
    }
    let tail = obj! {"character"=>changing,"state"=>get(v,"state")?.clone()};
    messages.push(obj! {"role"=>"user","content"=>tail.to_json()});
    let dialogue = c::list(get(v, "dialogue")?)?;
    if dialogue.len() > 16 {
        return Err(c::bad());
    }
    for e in dialogue {
        c::fields(e, &["user", "assistant"])?;
        for (role, key) in [("user", "user"), ("assistant", "assistant")] {
            line(text(e, key)?)?;
            messages.push(obj! {"role"=>role,"content"=>get(e,key)?.clone()});
        }
    }
    messages.push(obj! {"role"=>"user","content"=>text(v,"input")?});
    Ok(obj! {"messages"=>messages,"tools"=>get(v,"tools")?.clone()})
}
pub fn fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["life", "scene", "thread", "selection", "caps", "dialogue"])?;
    let l = get(a, "life")?;
    c::fields(l, &["setting", "definition", "defaults", "overrides", "scope", "actor", "participants", "operations"])?;
    let d = Arc::new(Definition::draft(get(l, "definition")?, &Setting::from_value(get(l, "setting")?)?)?);
    let mut life = Life::accept(
        text(l, "scope")?,
        text(l, "actor")?,
        get(l, "participants")?,
        d,
        Config::resolve(get(l, "defaults")?, get(l, "overrides")?)?,
    )?;
    let ops = c::list(get(l, "operations")?)?;
    if ops.len() > 128 {
        return Err(c::bad());
    }
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let v = get(op, "args")?;
        match text(op, "op")? {
            "experience" => life.experience(v)?,
            "appraise" => {
                c::fields(v, &["rule", "event", "revision"])?;
                life.appraise(text(v, "rule")?, num(v, "event")?, num(v, "revision")?)?;
            }
            _ => return Err(c::bad()),
        }
    }
    let mut co = Coordinator::new(&life.scope, &life.actor, &Value::strings(&life.participants))?;
    co.bind_scene(get(a, "scene")?)?;
    co.thread(get(a, "thread")?)?;
    co.select_context(
        text(get(a, "thread")?, "id")?,
        &life,
        get(a, "selection")?,
        ContextCaps::from_value(get(a, "caps")?)?,
        &crate::character::dialogue::Thread::from_value(get(a, "dialogue")?)?,
    )
}

impl Life {
    /// Optional recall preserves complete causal groups under the attention cap. When
    /// a group will not fit, use fewer optional episodes instead of a misleading fragment.
    pub fn recall_capped(&self, input: &str, tags: &Value, count: usize) -> Result<Value> {
        if count > 16 {
            return Err(c::bad());
        }
        self.active()?;
        names(tags, 8)?;
        if input.len() > 2048 {
            return Err(c::bad());
        }
        for n in (1..=count).rev() {
            if let Ok(events) = self.recall_episodes(input, tags, n) {
                if c::list(&events)?.len() <= count {
                    return Ok(events);
                }
            }
        }
        Ok(Vec::<Value>::new().into())
    }
}
