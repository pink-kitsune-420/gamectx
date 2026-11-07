//! Purposeful, event-driven host initiative. Poll through the existing Scheduler;
//! candidate ranking is deterministic and never calls a model or executes an action.
use super::*;
#[derive(Clone, Debug)]
pub struct InitiativePolicy {
    value: Value,
}
impl InitiativePolicy {
    pub fn resolve(defaults: &Value, overrides: &Value) -> Result<Self> {
        let mut value = obj! {"enabled"=>false,"cooldown"=>3,"margin"=>2,"social_limit"=>2,"attention"=>4,"model_calls"=>0,"replans"=>2};
        for v in [defaults, overrides] {
            bounded(v, 4096)?;
            let fields = v.as_object().ok_or_else(c::bad)?;
            for (k, x) in fields {
                if fields.iter().filter(|(n, _)| n == k).count() != 1 || value.get(k).is_none() {
                    return Err(c::bad());
                }
                super::continuity::set(&mut value, k, x.clone());
            }
        }
        c::boolean(get(&value, "enabled")?)?;
        for (k, lo, hi) in [
            ("cooldown", 1, 1000000),
            ("margin", 0, 100),
            ("social_limit", 0, 16),
            ("attention", 1, 16),
            ("model_calls", 0, 8),
            ("replans", 1, 16),
        ] {
            c::uint(get(&value, k)?, lo, hi)?;
        }
        Ok(Self { value })
    }
}
#[derive(Clone, Debug)]
struct Failure {
    id: String,
    count: u64,
    until: u64,
    event: u64,
}
#[derive(Clone, Debug)]
pub struct Initiative {
    policy: InitiativePolicy,
    scope: String,
    actor: String,
    participants: Vec<String>,
    clock: u64,
    turn: Option<u64>,
    calls: u64,
    social: u64,
    replans: u64,
    last: Option<u64>,
    chosen: Option<String>,
    next: u64,
    pending: Option<Value>,
    inflight: Vec<Value>,
    failures: Vec<Failure>,
    receipt: u64,
    social_seen: Vec<Value>,
    promises: Vec<Value>,
}
fn validate_frame(f: &Value, people: &[String]) -> Result<()> {
    bounded(f, 8192)?;
    c::fields(
        f,
        &[
            "revision",
            "now",
            "turn",
            "conditions",
            "allowed",
            "resources",
            "audience",
            "emergency",
            "depth",
            "social_remaining",
        ],
    )?;
    for k in ["revision", "now", "turn"] {
        num(f, k)?;
    }
    for k in ["conditions", "allowed", "resources"] {
        names(get(f, k)?, 64)?;
    }
    if !subset(&names(get(f, "audience")?, 16)?, people) {
        return Err(c::bad());
    }
    c::boolean(get(f, "emergency")?)?;
    c::uint(get(f, "depth")?, 0, 16)?;
    c::uint(get(f, "social_remaining")?, 0, 16)?;
    Ok(())
}
fn validate_candidate(v: &Value) -> Result<()> {
    bounded(v, 8192)?;
    c::fields(
        v,
        &["id", "kind", "priority", "reason", "requires", "resource", "recipient", "attitudes", "events", "promises"],
    )?;
    c::scope(text(v, "id")?)?;
    if !matches!(text(v, "kind")?, "activity" | "ask" | "followup" | "decline" | "gesture" | "silence") {
        return Err(c::bad());
    }
    signed(get(v, "priority")?, 100)?;
    line(text(v, "reason")?)?;
    names(get(v, "requires")?, 16)?;
    names(get(v, "attitudes")?, 8)?;
    for k in ["resource", "recipient"] {
        if get(v, k)? != &Value::Null {
            c::scope(text(v, k)?)?;
        }
    }
    for k in ["promises"] {
        let ids = c::list(get(v, k)?)?;
        if ids.len() > 4 {
            return Err(c::bad());
        }
        for (i, id) in ids.iter().enumerate() {
            c::uint(id, 1, u32::MAX as u64)?;
            if ids[..i].contains(id) {
                return Err(c::bad());
            }
        }
    }
    let events = c::list(get(v, "events")?)?;
    if events.len() > 8 {
        return Err(c::bad());
    }
    for (i, e) in events.iter().enumerate() {
        c::uint(e, 1, u32::MAX as u64)?;
        if events[..i].contains(e) {
            return Err(c::bad());
        }
    }
    Ok(())
}
impl Initiative {
    pub fn new(scope: &str, actor: &str, participants: &Value, policy: InitiativePolicy) -> Result<Self> {
        c::scope(scope)?;
        c::scope(actor)?;
        let participants = names(participants, 16)?;
        if !participants.iter().any(|p| p == actor) {
            return Err(c::bad());
        }
        Ok(Self {
            policy,
            scope: scope.into(),
            actor: actor.into(),
            participants,
            clock: 0,
            turn: None,
            calls: 0,
            social: 0,
            replans: 0,
            last: None,
            chosen: None,
            next: 0,
            pending: None,
            inflight: Vec::new(),
            failures: Vec::new(),
            receipt: 0,
            social_seen: Vec::new(),
            promises: Vec::new(),
        })
    }
    fn frame(&mut self, f: &Value) -> Result<()> {
        validate_frame(f, &self.participants)?;
        let now = num(f, "now")?;
        let turn = num(f, "turn")?;
        if now < self.clock || self.turn.is_some_and(|t| turn < t) {
            return Err(c::bad());
        }
        self.clock = now;
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.calls = 0;
            self.social = 0;
            self.replans = 0;
        }
        Ok(())
    }
    fn usable(&self, v: &Value, f: &Value, life: &Life) -> Result<bool> {
        let contains =
            |k: &str, id: &str| -> bool { c::list(get(f, k).unwrap()).unwrap().iter().any(|x| x.as_str() == Some(id)) };
        if !contains("allowed", text(v, "id")?)
            || names(get(v, "requires")?, 16)?.iter().any(|id| !contains("conditions", id))
            || get(v, "resource")? != &Value::Null && !contains("resources", text(v, "resource")?)
        {
            return Ok(false);
        }
        if get(v, "recipient")? != &Value::Null
            && (!contains("audience", text(v, "recipient")?)
                || num(f, "depth")? > 0
                || self.social >= num(&self.policy.value, "social_limit")?
                || num(f, "social_remaining")? == 0)
        {
            return Ok(false);
        }
        if get(v, "recipient")? != &Value::Null
            && self.social_seen.iter().any(|x| {
                x.get("id") == v.get("id")
                    && x.get("recipient") == v.get("recipient")
                    && x.get("events") == v.get("events")
            })
        {
            return Ok(false);
        }
        if c::list(get(v, "promises")?)?.iter().any(|id| {
            !self
                .promises
                .iter()
                .any(|p| p.get("id") == Some(id) && p.get("promisor").and_then(Value::as_str) == Some(&self.actor))
        }) {
            return Ok(false);
        }
        if self.inflight.iter().any(|p| p.get("candidate").and_then(|c| c.get("id")) == v.get("id")) {
            return Ok(false);
        }
        if self.failures.iter().any(|x| {
            x.id == text(v, "id").unwrap()
                && !c::list(get(v, "events").unwrap())
                    .unwrap()
                    .iter()
                    .any(|n| n.as_usize().is_some_and(|n| n as u64 > x.event))
                && (self.clock < x.until || x.count >= 3)
        }) {
            return Ok(false);
        }
        if c::list(get(v, "events")?)?.iter().any(|token| !life.events.iter().any(|e| e.get("token") == Some(token))) {
            return Ok(false);
        }
        Ok(true)
    }
    fn score(&self, v: &Value, life: &Life) -> Result<i64> {
        let mut score = signed(get(v, "priority")?, 100)?;
        for id in names(get(v, "attitudes")?, 8)? {
            let a = life
                .attitudes
                .iter()
                .find(|a| a.get("definition").and_then(|d| d.get("id")).and_then(Value::as_str) == Some(&id))
                .ok_or_else(c::bad)?;
            if !subset(&self.participants, &names(get(get(a, "definition")?, "audience")?, 16)?) {
                return Err(c::bad());
            }
            score += signed(get(a, "level")?, 100)? / 10;
        }
        score += 20 * c::list(get(v, "promises")?)?.len() as i64;
        Ok(score)
    }
    /// Host candidates must already be relevant to current goals, promises or events.
    /// No new model work is scheduled for quiet results, routine polling or observations.
    pub fn consider(
        &mut self,
        f: &Value,
        candidates: &Value,
        life: &Life,
        interactions: &crate::dialogue::InteractionTools,
    ) -> Result<Value> {
        if !life.identity_matches(&self.scope, &self.actor, &self.participants) {
            return Err(c::bad());
        }
        let (scope, actor, people) = interactions.identity();
        if scope != self.scope || actor != self.actor || people != self.participants {
            return Err(c::bad());
        }
        let mut next = self.clone();
        next.promises = interactions.accepted_promises();
        next.frame(f)?;
        let rows = c::list(candidates)?;
        if rows.len() > 16 {
            return Err(c::bad());
        }
        for (i, v) in rows.iter().enumerate() {
            validate_candidate(v)?;
            if rows[..i].iter().any(|x| x.get("id") == v.get("id")) {
                return Err(c::bad());
            }
        }
        let quiet = |reason: &str| obj! {"status"=>"quiet","reason"=>reason};
        let result = if !c::boolean(get(&self.policy.value, "enabled")?)? {
            quiet("disabled")
        } else if next.last.is_some_and(|at| next.clock < at + num(&self.policy.value, "cooldown").unwrap())
            && !c::boolean(get(f, "emergency")?)?
        {
            quiet("cooldown")
        } else if next.replans >= num(&self.policy.value, "replans")? {
            quiet("replan_limit")
        } else {
            let mut ranked = Vec::new();
            for v in rows {
                if next.usable(v, f, life)? {
                    ranked.push((next.score(v, life)?, text(v, "id")?.to_string(), v));
                }
            }
            ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            ranked.truncate(num(&self.policy.value, "attention")? as usize);
            if let Some(best) = ranked.first() {
                let selected = ranked
                    .iter()
                    .find(|x| {
                        Some(&x.1) == next.chosen.as_ref()
                            && x.0 + num(&self.policy.value, "margin").unwrap() as i64 >= best.0
                    })
                    .unwrap_or(best);
                if text(selected.2, "kind")? == "silence" {
                    quiet(text(selected.2, "reason")?)
                } else {
                    next.next = next.next.checked_add(1).filter(|n| *n <= u32::MAX as u64).ok_or_else(c::bad)?;
                    next.replans += 1;
                    next.last = Some(next.clock);
                    next.chosen = Some(selected.1.clone());
                    if get(selected.2, "recipient")? != &Value::Null {
                        next.social += 1;
                        if next.social_seen.len() >= 64 {
                            return Err(c::bad());
                        }
                        next.social_seen.push(obj!{"id"=>get(selected.2,"id")?.clone(),"recipient"=>get(selected.2,"recipient")?.clone(),"events"=>get(selected.2,"events")?.clone()});
                    }
                    obj! {"status"=>"proposed","id"=>next.next,"revision"=>num(f,"revision")?,"candidate"=>selected.2.clone(),"score"=>selected.0,"life_revision"=>life.revision,"promises"=>next.promises.iter().filter(|p|c::list(get(selected.2,"promises").unwrap()).unwrap().iter().any(|id|Some(id)==p.get("id"))).cloned().collect::<Vec<_>>()}
                }
            } else {
                quiet("no_usable_candidate")
            }
        };
        next.pending =
            if result.get("status").and_then(Value::as_str) == Some("proposed") { Some(result.clone()) } else { None };
        *self = next;
        Ok(result)
    }
    /// One-use proposal consumption with fresh prerequisites, permissions and life state.
    /// The host atomically reserves resources and executes after this returns ready.
    pub fn dispatch(
        &mut self,
        id: u64,
        f: &Value,
        life: &Life,
        interactions: &crate::dialogue::InteractionTools,
        mut available: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        validate_frame(f, &self.participants)?;
        if !life.identity_matches(&self.scope, &self.actor, &self.participants) {
            return Err(c::bad());
        }
        let (scope, actor, people) = interactions.identity();
        if scope != self.scope || actor != self.actor || people != self.participants {
            return Err(c::bad());
        }
        self.promises = interactions.accepted_promises();
        let p = self.pending.take().ok_or_else(c::bad)?;
        if num(&p, "id")? != id
            || num(&p, "revision")? != num(f, "revision")?
            || num(&p, "life_revision")? != life.revision
            || num(f, "now")? < self.clock
            || num(f, "turn")? != self.turn.ok_or_else(c::bad)?
            || self.inflight.len() >= 16
        {
            return Err(c::bad());
        }
        if c::list(get(&p, "promises")?)?.iter().any(|x| !self.promises.contains(x)) {
            return Err(c::bad());
        }
        let v = get(&p, "candidate")?;
        // Social quota was reserved at proposal; check current host quota, not a second charge.
        let mut current = self.clone();
        if get(v, "recipient")? != &Value::Null {
            current.social = current.social.saturating_sub(1);
            current.social_seen.retain(|x| {
                x.get("id") != v.get("id")
                    || x.get("recipient") != v.get("recipient")
                    || x.get("events") != v.get("events")
            });
        }
        current.clock = num(f, "now")?;
        if !current.usable(v, f, life)? || !available(&obj! {"action"=>text(v,"kind")?,"target"=>text(v,"id")?}) {
            return Err(c::bad());
        }
        self.inflight.push(p.clone());
        Ok(obj! {"status"=>"ready","proposal"=>p})
    }
    /// Confirmed host receipt, supported by an actual outcome event in this Life scope.
    pub fn outcome(&mut self, id: u64, token: u64, success: bool, life: &Life) -> Result<()> {
        if !life.identity_matches(&self.scope, &self.actor, &self.participants) || token <= self.receipt {
            return Err(c::bad());
        }
        let i = self.inflight.iter().position(|p| num(p, "id").ok() == Some(id)).ok_or_else(c::bad)?;
        let p = &self.inflight[i];
        let candidate = get(p, "candidate")?;
        let event = life
            .events
            .iter()
            .find(|e| num(e, "token").ok() == Some(token) && text(e, "kind").ok() == Some("outcome"))
            .ok_or_else(c::bad)?;
        let ident = text(candidate, "id")?;
        if event.get("tag").and_then(Value::as_str) != Some(ident) {
            return Err(c::bad());
        }
        let mut next = self.clone();
        next.inflight.remove(i);
        next.receipt = token;
        if success {
            next.failures.retain(|x| x.id != ident);
        } else {
            let until = (num(event, "at")? + num(&self.policy.value, "cooldown")?).min(u32::MAX as u64);
            if let Some(x) = next.failures.iter_mut().find(|x| x.id == ident) {
                x.count = (x.count + 1).min(3);
                x.until = until;
                x.event = life.watermark;
            } else {
                if next.failures.len() >= 64 {
                    return Err(c::bad());
                }
                next.failures.push(Failure { id: ident.into(), count: 1, until, event: life.watermark });
            }
        }
        *self = next;
        Ok(())
    }
    /// Reserve every provider request in the whole turn, including preflight/retry.
    pub fn reserve_call(&mut self, turn: u64) -> Result<()> {
        if !c::boolean(get(&self.policy.value, "enabled")?)?
            || self.turn != Some(turn)
            || self.calls >= num(&self.policy.value, "model_calls")?
        {
            return Err(c::bad());
        }
        self.calls += 1;
        Ok(())
    }
    pub fn next_wake(&self) -> Option<u64> {
        self.last.map(|t| t.saturating_add(num(&self.policy.value, "cooldown").unwrap()).min(u32::MAX as u64))
    }
    pub fn inspect(&self) -> Value {
        obj! {"clock"=>self.clock,"turn"=>self.turn.map_or(Value::Null,Value::from),"model_calls"=>self.calls,"social"=>self.social,"replans"=>self.replans,"pending"=>self.pending.clone().unwrap_or(Value::Null),"inflight"=>self.inflight.clone(),"next_wake"=>self.next_wake().map_or(Value::Null,Value::from)}
    }
}
pub fn fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["defaults", "overrides", "frames", "candidates", "life", "promises", "reserves", "receipts"])?;
    let l = get(a, "life")?;
    c::fields(l, &["setting", "definition", "defaults", "overrides", "scope", "actor", "participants", "operations"])?;
    let setting = Setting::from_value(get(l, "setting")?)?;
    let definition = Arc::new(Definition::draft(get(l, "definition")?, &setting)?);
    let mut life = Life::accept(
        text(l, "scope")?,
        text(l, "actor")?,
        get(l, "participants")?,
        definition,
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
    let mut tools =
        crate::dialogue::InteractionTools::new(&life.scope, &life.actor, &life.participants.clone().into())?;
    let promises = c::list(get(a, "promises")?)?;
    if promises.len() > 16 {
        return Err(c::bad());
    }
    for p in promises {
        c::fields(p, &["promisor", "beneficiary", "task", "source", "state"])?;
        let source = get(p, "source")?;
        let args = obj! {"promisor"=>get(p,"promisor")?.clone(),"beneficiary"=>get(p,"beneficiary")?.clone(),"task"=>get(p,"task")?.clone(),"source"=>get(source,"id")?.clone()};
        let note = tools.call("promise_remember", &args.to_json(), &vec![source.clone()].into(), 0, 8192, |_| false)?;
        let state = text(p, "state")?;
        if state != "reported" {
            tools.set_progress(&obj!{"id"=>get(&note,"id")?.clone(),"expected"=>0,"state"=>if state=="cancelled"{"cancelled"}else{"accepted"},"source"=>source.clone()})?;
            if state == "fulfilled" {
                tools.set_progress(
                    &obj! {"id"=>get(&note,"id")?.clone(),"expected"=>1,"state"=>"fulfilled","source"=>source.clone()},
                )?;
            } else if !matches!(state, "accepted" | "cancelled") {
                return Err(c::bad());
            }
        }
    }
    let policy = InitiativePolicy::resolve(get(a, "defaults")?, get(a, "overrides")?)?;
    let mut initiative = Initiative::new(&life.scope, &life.actor, &life.participants.clone().into(), policy)?;
    let frames = c::list(get(a, "frames")?)?;
    let reserves = c::list(get(a, "reserves")?)?;
    let receipts = c::list(get(a, "receipts")?)?;
    if frames.len() > 1024 || reserves.len() != frames.len() || receipts.len() != frames.len() {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        let decision = initiative.consider(f, get(a, "candidates")?, &life, &tools)?;
        let mut reservations = Vec::new();
        for _ in 0..c::uint(&reserves[i], 0, 16)? {
            reservations.push(initiative.reserve_call(num(f, "turn")?).is_ok());
        }
        if receipts[i] != Value::Null && decision.get("status").and_then(Value::as_str) == Some("proposed") {
            let success = c::boolean(&receipts[i])?;
            let id = num(&decision, "id")?;
            initiative.dispatch(id, f, &life, &tools, |_| true)?;
            let token = life.watermark + 1;
            let event = obj! {"token"=>token,"at"=>num(f,"now")?,"kind"=>"outcome","tag"=>get(get(&decision,"candidate")?,"id")?.clone(),"subject"=>life.actor.clone(),"source"=>obj!{"id"=>format!("host:{token}"),"speaker"=>life.actor.clone(),"text"=>if success{"Host action succeeded."}else{"Host action failed."}},"audience"=>life.participants.clone(),"text"=>"A borrowed lamp was returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true};
            life.experience(&event)?;
            initiative.outcome(id, token, success, &life)?;
        }
        results.push(obj!{"decision"=>decision,"reservations"=>reservations,"model_calls"=>initiative.calls,"life_revision"=>life.revision,"events"=>life.events.len()});
    }
    Ok(obj! {"results"=>results})
}
impl Initiative {
    /// Join currently eligible deterministic routine activities without enabling tools.
    pub fn consider_routine(
        &mut self,
        f: &Value,
        candidates: &Value,
        life: &Life,
        interactions: &crate::dialogue::InteractionTools,
        routine: &crate::routine::Runtime,
    ) -> Result<Value> {
        if !routine.identity_matches(&self.scope, &self.actor, &self.participants)
            || routine.revision().map(u64::from) != Some(num(f, "revision")?)
        {
            return Err(c::bad());
        }
        let menu = routine.activity_menu()?;
        let rows = c::list(candidates)?;
        if rows.len() > 16 {
            return Err(c::bad());
        }
        let mut selected = Vec::new();
        for c in rows {
            validate_candidate(c)?;
            if text(c, "kind")? != "activity"
                || c::list(get(&menu, "activities")?)?.iter().any(|a| a.get("id") == c.get("id"))
            {
                selected.push(c.clone());
            }
        }
        self.consider(f, &selected.into(), life, interactions)
    }
    pub fn dispatch_routine(
        &mut self,
        id: u64,
        f: &Value,
        life: &Life,
        interactions: &crate::dialogue::InteractionTools,
        routine: &crate::routine::Runtime,
        mut allowed: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        if !routine.identity_matches(&self.scope, &self.actor, &self.participants)
            || routine.revision().map(u64::from) != Some(num(f, "revision")?)
        {
            return Err(c::bad());
        }
        let menu = routine.activity_menu()?;
        self.dispatch(id, f, life, interactions, |p| {
            (p.get("action").and_then(Value::as_str) != Some("activity")
                || c::list(get(&menu, "activities").unwrap()).unwrap().iter().any(|a| a.get("id") == p.get("target")))
                && allowed(p)
        })
    }
}
