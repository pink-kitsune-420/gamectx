//! Opt-in character continuity. Definitions, subjective attitudes and attributed
//! experiences are separate. All mutation and canonical acceptance belong to hosts.
use crate::{checked as c, obj, request::bounded, Result, Value};
use std::sync::Arc;
mod continuity;
mod storage;
pub use continuity::{Coordinator, SpeechTransport};
pub use storage::{FileStore, Storage};
pub const MAX_BYTES: usize = 524_288;
pub(crate) fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    c::get(v, k)
}
pub(crate) fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(get(v, k)?)
}
pub(crate) fn num(v: &Value, k: &str) -> Result<u64> {
    c::uint(get(v, k)?, 0, u32::MAX as u64)
}
pub(crate) fn line(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 2048 || s.chars().count() > 512 || s.chars().any(char::is_control) {
        return Err(c::bad());
    }
    Ok(())
}
pub(crate) fn names(v: &Value, cap: usize) -> Result<Vec<String>> {
    let rows = c::list(v)?;
    if rows.len() > cap {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for r in rows {
        let s = c::text(r)?;
        c::scope(s)?;
        if out.iter().any(|o| o == s) {
            return Err(c::bad());
        }
        out.push(s.into());
    }
    Ok(out)
}
pub(crate) fn signed(v: &Value, bound: i64) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| n.unsigned_abs() <= bound as u64).ok_or_else(c::bad),
        _ => Err(c::bad()),
    }
}
fn subset(a: &[String], b: &[String]) -> bool {
    a.iter().all(|x| b.contains(x))
}
/// Host's approved setting and shared past. Exact event identities prevent a draft
/// from replacing an accepted shared event. Textual implication still needs author review.
#[derive(Clone, Debug)]
pub struct Setting {
    value: Value,
    entities: Vec<String>,
}
impl Setting {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, MAX_BYTES)?;
        c::fields(v, &["id", "calendar", "entities", "history"])?;
        c::scope(text(v, "id")?)?;
        c::scope(text(v, "calendar")?)?;
        let entities = names(get(v, "entities")?, 128)?;
        let rows = c::list(get(v, "history")?)?;
        if rows.len() > 128 {
            return Err(c::bad());
        }
        for (i, h) in rows.iter().enumerate() {
            c::fields(h, &["id", "at", "participants", "text"])?;
            c::scope(text(h, "id")?)?;
            num(h, "at")?;
            line(text(h, "text")?)?;
            let p = names(get(h, "participants")?, 16)?;
            if p.is_empty() || !subset(&p, &entities) || rows[..i].iter().any(|o| o.get("id") == h.get("id")) {
                return Err(c::bad());
            }
        }
        Ok(Self { value: v.clone(), entities })
    }
}
#[derive(Clone, Debug)]
pub struct Definition {
    value: Value,
    entities: Vec<String>,
}
impl Definition {
    /// Parse a draft. Calling this does not approve it; install only after host review.
    pub fn draft(v: &Value, setting: &Setting) -> Result<Self> {
        bounded(v, 65536)?;
        c::fields(
            v,
            &[
                "version",
                "id",
                "revision",
                "setting",
                "calendar",
                "facets",
                "attitudes",
                "rules",
                "history",
                "improvisation",
            ],
        )?;
        c::uint(get(v, "version")?, 1, 1)?;
        c::scope(text(v, "id")?)?;
        num(v, "revision")?;
        if v.get("setting") != setting.value.get("id") || v.get("calendar") != setting.value.get("calendar") {
            return Err(c::bad());
        }
        line(text(v, "improvisation")?)?;
        let facets = c::list(get(v, "facets")?)?;
        if facets.len() > 64 {
            return Err(c::bad());
        }
        for (i, f) in facets.iter().enumerate() {
            c::fields(f, &["id", "kind", "text", "audience", "tags"])?;
            c::scope(text(f, "id")?)?;
            line(text(f, "text")?)?;
            if !matches!(
                text(f, "kind")?,
                "identity"
                    | "background"
                    | "value"
                    | "voice"
                    | "temperament"
                    | "boundary"
                    | "knowledge"
                    | "social"
                    | "group"
                    | "reputation"
                    | "etiquette"
                    | "lore"
                    | "private"
                    | "goal"
                    | "commitment"
                    | "unfinished"
                    | "priority"
                    | "physical"
                    | "emotion"
            ) {
                return Err(c::bad());
            }
            let a = names(get(f, "audience")?, 16)?;
            names(get(f, "tags")?, 8)?;
            if a.is_empty() || !subset(&a, &setting.entities) || facets[..i].iter().any(|o| o.get("id") == f.get("id"))
            {
                return Err(c::bad());
            }
        }
        let attitudes = c::list(get(v, "attitudes")?)?;
        if attitudes.len() > 64 {
            return Err(c::bad());
        }
        for (i, a) in attitudes.iter().enumerate() {
            c::fields(
                a,
                &["id", "kind", "subject", "text", "level", "reason", "tension", "changes_if", "audience", "tags"],
            )?;
            for k in ["id", "subject"] {
                c::scope(text(a, k)?)?;
            }
            if !setting.entities.iter().any(|x| x == text(a, "subject").unwrap_or("")) {
                return Err(c::bad());
            }
            if !matches!(
                text(a, "kind")?,
                "preference"
                    | "opinion"
                    | "familiarity"
                    | "trust"
                    | "affection"
                    | "resentment"
                    | "obligation"
                    | "disagreement"
                    | "belief"
                    | "report"
                    | "hypothesis"
                    | "misconception"
                    | "unknown"
                    | "goal"
                    | "commitment"
                    | "unfinished"
                    | "priority"
                    | "physical"
                    | "emotion"
            ) {
                return Err(c::bad());
            }
            for k in ["text", "reason", "tension", "changes_if"] {
                line(text(a, k)?)?;
            }
            signed(get(a, "level")?, 100)?;
            names(get(a, "tags")?, 8)?;
            let audience = names(get(a, "audience")?, 16)?;
            if audience.is_empty()
                || !subset(&audience, &setting.entities)
                || attitudes[..i].iter().any(|o| o.get("id") == a.get("id"))
            {
                return Err(c::bad());
            }
        }
        let rules = c::list(get(v, "rules")?)?;
        if rules.len() > 64 {
            return Err(c::bad());
        }
        for (i, r) in rules.iter().enumerate() {
            c::fields(r, &["id", "attitude", "event", "delta", "cooldown", "reason"])?;
            for k in ["id", "attitude", "event"] {
                c::scope(text(r, k)?)?;
            }
            line(text(r, "reason")?)?;
            let delta = signed(get(r, "delta")?, 20)?;
            c::uint(get(r, "cooldown")?, 1, 1000000)?;
            if delta == 0
                || !attitudes.iter().any(|a| a.get("id") == r.get("attitude"))
                || rules[..i].iter().any(|o| o.get("id") == r.get("id"))
            {
                return Err(c::bad());
            }
        }
        let history = names(get(v, "history")?, 32)?;
        if history.iter().any(|id| {
            !c::list(get(&setting.value, "history").expect("validated"))
                .expect("validated")
                .iter()
                .any(|h| h.get("id").and_then(Value::as_str) == Some(id))
        }) {
            return Err(c::bad());
        }
        Ok(Self { value: v.clone(), entities: setting.entities.clone() })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
}
/// Shared defaults plus partial overrides. No features or model tools are enabled by default.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub enabled: bool,
    pub balanced: bool,
    pub deception: bool,
    pub mistaken: bool,
    pub tools: Vec<String>,
}
impl Config {
    pub fn resolve(defaults: &Value, overrides: &Value) -> Result<Self> {
        let mut out = Self::default();
        for v in [defaults, overrides] {
            bounded(v, 4096)?;
            for (k, x) in v.as_object().ok_or_else(c::bad)? {
                match k.as_str() {
                    "enabled" => out.enabled = c::boolean(x)?,
                    "balanced" => out.balanced = c::boolean(x)?,
                    "deception" => out.deception = c::boolean(x)?,
                    "mistaken" => out.mistaken = c::boolean(x)?,
                    "tools" => {
                        out.tools = names(x, 2)?;
                        if out.tools.iter().any(|n| !matches!(n.as_str(), "life_recall" | "life_change")) {
                            return Err(c::bad());
                        }
                    }
                    _ => return Err(c::bad()),
                }
            }
        }
        if !out.enabled && (out.balanced || out.deception || out.mistaken || !out.tools.is_empty()) {
            return Err(c::bad());
        }
        Ok(out)
    }
}
/// One fixed host-authorized participant scope. Do not share private scopes with a party.
#[derive(Clone, Debug)]
pub struct Life {
    pub(crate) definition: Arc<Definition>,
    pub(crate) config: Config,
    pub(crate) scope: String,
    pub(crate) actor: String,
    pub(crate) participants: Vec<String>,
    pub(crate) revision: u64,
    pub(crate) clock: u64,
    pub(crate) watermark: u64,
    pub(crate) attitudes: Vec<Value>,
    pub(crate) events: Vec<Value>,
    pub(crate) changes: Vec<Value>,
    pub(crate) archive_ack: u64,
    pub(crate) summaries: Vec<Value>,
}
impl Life {
    /// Host acceptance of a validated definition. No model-facing constructor exists.
    pub fn accept(
        scope: &str,
        actor: &str,
        participants: &Value,
        definition: Arc<Definition>,
        config: Config,
    ) -> Result<Self> {
        c::scope(scope)?;
        c::scope(actor)?;
        let participants = names(participants, 16)?;
        if !subset(&participants, &definition.entities) || !participants.iter().any(|p| p == actor) {
            return Err(c::bad());
        }
        // Revalidate public mutable Config fields.
        let config = Config::resolve(
            &obj! {"enabled"=>config.enabled,"balanced"=>config.balanced,"deception"=>config.deception,"mistaken"=>config.mistaken,"tools"=>config.tools},
            &obj! {},
        )?;
        let attitudes=c::list(get(&definition.value,"attitudes")?)?.iter().map(|a|obj!{"definition"=>a.clone(),"level"=>a.get("level").unwrap().clone(),"supports"=>Vec::<Value>::new(),"contradicts"=>Vec::<Value>::new(),"last"=>Value::Null}).collect();
        Ok(Self {
            definition,
            config,
            scope: scope.into(),
            actor: actor.into(),
            participants,
            revision: 0,
            clock: 0,
            watermark: 0,
            attitudes,
            events: Vec::new(),
            changes: Vec::new(),
            archive_ack: 0,
            summaries: Vec::new(),
        })
    }
    pub fn identity_matches(&self, scope: &str, actor: &str, participants: &[String]) -> bool {
        self.scope == scope && self.actor == actor && self.participants == participants
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    fn active(&self) -> Result<()> {
        if !self.config.enabled {
            return Err(c::bad());
        }
        Ok(())
    }
    pub(crate) fn bump(&mut self) -> Result<()> {
        self.revision = self.revision.checked_add(1).filter(|n| *n <= u32::MAX as u64).ok_or_else(c::bad)?;
        Ok(())
    }
    /// Ingest an actual host experience. `report` retains attribution, never truth.
    /// Caller authenticates sensors, receipts, quoted speech and audience membership.
    pub fn experience(&mut self, event: &Value) -> Result<()> {
        self.active()?;
        validate_event(event, &self.actor, &self.participants)?;
        if !self.definition.entities.iter().any(|id| Some(id.as_str()) == event.get("subject").and_then(Value::as_str))
        {
            return Err(c::bad());
        }
        let token = num(event, "token")?;
        let at = num(event, "at")?;
        if token <= self.watermark || at < self.clock || self.events.len() >= 128 {
            return Err(c::bad());
        }
        let mut next = self.clone();
        next.bump()?;
        next.watermark = token;
        next.clock = at;
        next.events.push(event.clone());
        bounded(&next.inspect(), MAX_BYTES)?;
        *self = next;
        Ok(())
    }
    /// Model changes are bounded proposals. Only host `appraise` applies authored rules.
    pub fn propose(&self, input: &str) -> Result<Value> {
        self.active()?;
        let v = crate::json::parse(input, 4096)?;
        c::fields(&v, &["rule", "event", "reason"])?;
        c::scope(text(&v, "rule")?)?;
        num(&v, "event")?;
        line(text(&v, "reason")?)?;
        self.rule_event(text(&v, "rule")?, num(&v, "event")?)?;
        Ok(obj! {"status"=>"proposed","revision"=>self.revision,"proposal"=>v})
    }
    fn rule_event(&self, rule: &str, token: u64) -> Result<(&Value, &Value)> {
        let r = c::list(get(&self.definition.value, "rules")?)?
            .iter()
            .find(|r| r.get("id").and_then(Value::as_str) == Some(rule))
            .ok_or_else(c::bad)?;
        let e = self.events.iter().find(|e| num(e, "token").ok() == Some(token)).ok_or_else(c::bad)?;
        if e.get("tag") != r.get("event") || !matches!(text(e, "kind")?, "direct" | "outcome") {
            return Err(c::bad());
        }
        Ok((r, e))
    }
    pub fn appraise(&mut self, rule: &str, token: u64, expected: u64) -> Result<Value> {
        self.active()?;
        if expected != self.revision || self.changes.len() >= 128 {
            return Err(c::bad());
        }
        let (r, e) = self.rule_event(rule, token)?;
        let r = r.clone();
        let e = e.clone();
        let i = self
            .attitudes
            .iter()
            .position(|a| a.get("definition").and_then(|a| a.get("id")) == r.get("attitude"))
            .ok_or_else(c::bad)?;
        let a = &self.attitudes[i];
        let already = self
            .changes
            .iter()
            .any(|x| x.get("rule").and_then(Value::as_str) == Some(rule) && num(x, "event").ok() == Some(token));
        let last = get(a, "last")?;
        if already
            || (last != &Value::Null && num(&e, "at")? < c::uint(last, 0, u32::MAX as u64)? + num(&r, "cooldown")?)
        {
            return Err(c::bad());
        }
        let before = signed(get(a, "level")?, 100)?;
        let after = (before + signed(get(&r, "delta")?, 20)?).clamp(-100, 100);
        // Each lasting reversal must pass through neutral over multiple experiences.
        let after = if before * after < 0 { 0 } else { after };
        let change = obj! {"rule"=>rule,"event"=>token,"at"=>num(&e,"at")?,"before"=>before,"after"=>after,"reason"=>get(&r,"reason")?.clone()};
        let mut next = self.clone();
        next.bump()?;
        if let Value::Object(a) = &mut next.attitudes[i] {
            for (k, v) in a {
                match k.as_str() {
                    "level" => *v = Value::from(after),
                    "last" => *v = get(&e, "at")?.clone(),
                    "supports" | "contradicts" => {
                        let key = if signed(get(&r, "delta")?, 20)? >= 0 { "supports" } else { "contradicts" };
                        if k == key {
                            let rows = if k == "supports" {
                                get(&self.attitudes[i], "supports")?
                            } else {
                                get(&self.attitudes[i], "contradicts")?
                            };
                            let mut rows = c::list(rows)?.to_vec();
                            if rows.len() == 16 {
                                rows.remove(0);
                            }
                            rows.push(Value::from(token));
                            *v = rows.into();
                        }
                    }
                    _ => {}
                }
            }
        }
        next.changes.push(change.clone());
        bounded(&next.inspect(), MAX_BYTES)?;
        *self = next;
        Ok(change)
    }
    /// Relevant subjective context. Audience must equal the fixed scope; values outside
    /// it stay host-only. Existing relationship notes/promise tools remain reusable.
    pub fn context(&self, tags: &Value, limit: usize) -> Result<Value> {
        self.active()?;
        let tags = names(tags, 8)?;
        if limit > 16 {
            return Err(c::bad());
        }
        let visible = |v: &Value| -> bool {
            names(get(v, "audience").expect("validated"), 16).is_ok_and(|a| subset(&self.participants, &a))
        };
        let relevant = |v: &Value| -> bool {
            tags.is_empty()
                || c::list(get(v, "tags").expect("validated"))
                    .expect("validated")
                    .iter()
                    .any(|t| tags.iter().any(|x| Some(x.as_str()) == t.as_str()))
        };
        let facets = c::list(get(&self.definition.value, "facets")?)?
            .iter()
            .filter(|f| visible(f) && relevant(f))
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let attitudes = self
            .attitudes
            .iter()
            .filter(|a| {
                let d = get(a, "definition").expect("validated");
                visible(d) && relevant(d) && (self.config.mistaken || text(d, "kind").ok() != Some("misconception"))
            })
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        Ok(
            obj! {"scope"=>self.scope.clone(),"revision"=>self.revision,"facets"=>facets,"attitudes"=>attitudes,"policy"=>obj!{"balanced"=>self.config.balanced,"deception"=>self.config.deception,"mistaken"=>self.config.mistaken,"improvisation"=>get(&self.definition.value,"improvisation")?.clone(),"expression"=>"Opinions, humor and imagined hypotheticals may be expressed. Separate facts, reports, beliefs, inference and proposals. Answer known parts and relevant uncertainty; invented flavor is not lore or experience."}},
        )
    }
    /// Erase learned data while retaining replay watermarks and approved definitions.
    pub fn clear(&mut self) {
        self.summaries.clear();
        self.events.clear();
        self.changes.clear();
        for a in &mut self.attitudes {
            if let Value::Object(fields) = a {
                let definition = fields.iter().find(|(k, _)| k == "definition").unwrap().1.clone();
                for (k, v) in fields {
                    match k.as_str() {
                        "level" => *v = definition.get("level").unwrap().clone(),
                        "supports" | "contradicts" => *v = Vec::<Value>::new().into(),
                        "last" => *v = Value::Null,
                        _ => {}
                    }
                }
            }
        }
        self.revision = self.revision.saturating_add(1).min(u32::MAX as u64);
    }
    pub fn inspect(&self) -> Value {
        obj! {"revision"=>self.revision,"watermark"=>self.watermark,"clock"=>self.clock,"attitudes"=>self.attitudes.clone(),"events"=>self.events.clone(),"changes"=>self.changes.clone()}
    }
}
pub(crate) fn validate_event(e: &Value, actor: &str, participants: &[String]) -> Result<()> {
    bounded(e, 8192)?;
    c::fields(e, &["token", "at", "kind", "tag", "subject", "source", "audience", "text", "causes", "retain"])?;
    c::uint(get(e, "token")?, 1, u32::MAX as u64)?;
    num(e, "at")?;
    if !matches!(text(e, "kind")?, "direct" | "report" | "intended" | "outcome" | "correction") {
        return Err(c::bad());
    }
    for k in ["tag", "subject"] {
        c::scope(text(e, k)?)?;
    }
    line(text(e, "text")?)?;
    crate::info::sources(&vec![get(e, "source")?.clone()].into())?;
    let audience = names(get(e, "audience")?, 16)?;
    if audience != participants || !audience.iter().any(|p| p == actor) {
        return Err(c::bad());
    }
    let causes = c::list(get(e, "causes")?)?;
    if causes.len() > 8 {
        return Err(c::bad());
    }
    for (i, r) in causes.iter().enumerate() {
        let n = c::uint(r, 1, num(e, "token")?.saturating_sub(1))?;
        if causes[..i].iter().any(|x| x.as_usize() == Some(n as usize)) {
            return Err(c::bad());
        }
    }
    c::boolean(get(e, "retain")?)?;
    Ok(())
}
/// Independent prototype/conformance adapter. No model or network calls.
pub fn workflow(a: &Value) -> Result<Value> {
    c::fields(a, &["setting", "definition", "defaults", "overrides", "scope", "actor", "participants", "operations"])?;
    let setting = Setting::from_value(get(a, "setting")?)?;
    let definition = Arc::new(Definition::draft(get(a, "definition")?, &setting)?);
    let mut life = Life::accept(
        text(a, "scope")?,
        text(a, "actor")?,
        get(a, "participants")?,
        definition,
        Config::resolve(get(a, "defaults")?, get(a, "overrides")?)?,
    )?;
    let ops = c::list(get(a, "operations")?)?;
    if ops.len() > 1024 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let args = get(op, "args")?;
        let result = match text(op, "op")? {
            "experience" => life.experience(args).map(|()| obj! {"revision"=>life.revision}),
            "appraise" => {
                c::fields(args, &["rule", "event", "revision"])?;
                life.appraise(text(args, "rule")?, num(args, "event")?, num(args, "revision")?)
            }
            "propose" => {
                c::fields(args, &["text"])?;
                life.propose(text(args, "text")?)
            }
            "context" => {
                c::fields(args, &["tags", "limit"])?;
                life.context(get(args, "tags")?, c::uint(get(args, "limit")?, 0, 16)? as usize)
            }
            "inspect" => {
                c::fields(args, &[])?;
                Ok(life.inspect())
            }
            _ => Err(c::bad()),
        };
        results.push(match result {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results})
}
impl Life {
    pub fn schema(&self, name: &str) -> Result<Value> {
        self.active()?;
        if !self.config.tools.iter().any(|t| t == name) {
            return Err(c::bad());
        }
        let (description, properties, required) = match name {
            "life_recall" => (
                "Read relevant approved traits and subjective attitudes in this scope.",
                obj! {"tags"=>obj!{"type"=>"array","maxItems"=>8,"items"=>obj!{"type"=>"string","maxLength"=>64}},"limit"=>obj!{"type"=>"integer","minimum"=>0,"maximum"=>16}},
                vec!["tags", "limit"],
            ),
            "life_change" => (
                "Propose an authored appraisal using an actual experience; host acceptance is required.",
                obj! {"rule"=>obj!{"type"=>"string","maxLength"=>64},"event"=>obj!{"type"=>"integer","minimum"=>1},"reason"=>obj!{"type"=>"string","maxLength"=>512}},
                vec!["rule", "event", "reason"],
            ),
            _ => return Err(c::bad()),
        };
        Ok(
            obj! {"type"=>"function","function"=>obj!{"name"=>name,"description"=>description,"parameters"=>obj!{"type"=>"object","properties"=>properties,"required"=>required,"additionalProperties"=>false}}},
        )
    }
    pub fn call(
        &self,
        name: &str,
        input: &str,
        max_bytes: usize,
        mut allowed: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        self.schema(name)?;
        if !allowed(&obj! {"action"=>name,"target"=>self.actor.clone()}) {
            return Err(c::bad());
        }
        let result = match name {
            "life_recall" => {
                let a = crate::json::parse(input, 4096)?;
                c::fields(&a, &["tags", "limit"])?;
                self.context(get(&a, "tags")?, c::uint(get(&a, "limit")?, 0, 16)? as usize)?
            }
            "life_change" => self.propose(input)?,
            _ => return Err(c::bad()),
        };
        bounded(&result, max_bytes)?;
        Ok(result)
    }
}

/// Offline continuity adapter.
pub fn continuity_fixture(a: &Value) -> Result<Value> {
    continuity::fixture(a)
}

/// Offline episodic adapter.
pub fn episodes_fixture(a: &Value) -> Result<Value> {
    storage::fixture(a)
}
