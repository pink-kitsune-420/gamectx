use super::*;
use crate::{
    decision::Choices,
    observation::{History, Selection},
};
/// Optional per-character runtime. Host methods are privileged bridge operations;
/// only `routine_status` and `activity_propose` can be registered as model tools.
#[derive(Clone, Debug)]
pub struct Runtime {
    scope: String,
    actor: String,
    participants: Value,
    calendar: String,
    config: Config,
    timeline: Option<Timeline>,
    history: Option<History>,
    frame: Option<Frame>,
    completion_token: u64,
    calls: u64,
}
impl Runtime {
    pub fn new(
        scope: &str,
        actor: &str,
        participants: &Value,
        calendar: &str,
        config: Config,
        routine: Option<Arc<Routine>>,
    ) -> Result<Self> {
        for s in [scope, actor, calendar] {
            c::scope(s)?;
        }
        let roster = names(participants, 16)?;
        if !roster.iter().any(|p| p == actor) {
            return Err(c::bad());
        }
        if config.routines != routine.is_some() || routine.as_ref().is_some_and(|r| r.calendar != calendar) {
            return Err(c::bad());
        }
        let timeline = routine.map(|r| Timeline::new(r, config.clone())).transpose()?;
        let history = if config.history {
            Some(History::new(scope, actor, participants, calendar, config.clone())?)
        } else {
            None
        };
        Ok(Self {
            scope: scope.into(),
            actor: actor.into(),
            participants: participants.clone(),
            calendar: calendar.into(),
            config,
            timeline,
            history,
            frame: None,
            completion_token: 0,
            calls: 0,
        })
    }
    pub fn identity_matches(&self, scope: &str, actor: &str, participants: &[String]) -> bool {
        self.scope == scope
            && self.actor == actor
            && self.participants.as_array().is_some_and(|p| {
                p.len() == participants.len() && p.iter().zip(participants).all(|(a, b)| a.as_str() == Some(b.as_str()))
            })
    }
    pub fn revision(&self) -> Option<u32> {
        self.frame.as_ref().map(|f| f.revision as u32)
    }
    /// Engine-resolved conditions and permissions, independent of LLM work. On an
    /// error no timeline, history or allowance changes. Budget updates with Scheduler.
    pub fn update(&mut self, frame: Frame) -> Result<()> {
        if self.frame.as_ref().is_some_and(|old| {
            frame.revision <= old.revision
                || frame.now < old.now
                || !frame.emergency && frame.now - old.now < self.config.interval
        }) {
            return Err(c::bad());
        }
        // Preflight the only fallible history check before committing the timeline.
        // Timeline::update is already atomic; retained history need not be cloned.
        if let Some(h) = &self.history {
            h.check_tick(frame.now)?;
        }
        if let Some(t) = &mut self.timeline {
            t.update(frame.clone())?;
        } else if !frame.allowed.is_empty() {
            return Err(c::bad());
        }
        if let Some(h) = &mut self.history {
            h.advance(frame.now)?;
        }
        self.frame = Some(frame);
        self.calls = 0;
        Ok(())
    }
    /// Explicit allowance for complete provider calls, including retries/corrections.
    /// This reserves a host call; the library never calls or loads a model itself.
    pub fn reserve_model_call(&mut self, revision: u32) -> Result<()> {
        if self.revision() != Some(revision) || self.calls >= self.config.model_calls {
            return Err(c::bad());
        }
        self.calls += 1;
        Ok(())
    }
    pub fn observe(&mut self, o: &Value) -> Result<()> {
        let f = self.frame.as_ref().ok_or_else(c::bad)?;
        if num(o, "at")? != f.now {
            return Err(c::bad());
        }
        self.history.as_mut().ok_or_else(c::bad)?.observe(o)
    }
    /// Completion is an explicit engine acknowledgement, with its own monotonic
    /// event token. It creates no observation and cannot be called by the model.
    pub fn complete(&mut self, id: &str, start: u64, revision: u32, token: u64) -> Result<()> {
        if token <= self.completion_token || token > u32::MAX as u64 {
            return Err(c::bad());
        }
        self.timeline.as_mut().ok_or_else(c::bad)?.complete(id, start, revision as u64)?;
        self.completion_token = token;
        Ok(())
    }
    pub fn context(&self) -> Result<Value> {
        let f = self.frame.as_ref().ok_or_else(c::bad)?;
        let phase = self.timeline.as_ref().and_then(Timeline::phase);
        let time = self
            .history
            .as_ref()
            .map(|h| h.time_knowledge(f.now, phase))
            .transpose()?
            .unwrap_or(obj! {"status"=>"unknown","sources"=>Vec::<Value>::new(),"conflicting"=>false});
        let observations =
            self.history.as_ref().map(|h| h.recent(f.now)).transpose()?.unwrap_or_else(|| Vec::<Value>::new().into());
        self.context_value(observations, time)
    }
    fn context_value(&self, observations: Value, time: Value) -> Result<Value> {
        let f = self.frame.as_ref().ok_or_else(c::bad)?;
        let status = self
            .timeline
            .as_ref()
            .map(Timeline::status)
            .transpose()?
            .unwrap_or(obj! {"current"=>Value::Null,"next"=>Value::Null});
        Ok(
            obj! {"revision"=>f.revision,"current"=>c::get(&status,"current")?.clone(),"next"=>c::get(&status,"next")?.clone(),"observations"=>observations,"time"=>time},
        )
    }
    /// Build an explicit task view, preserving required evidence and deriving time
    /// from only those selected observations (plus an enabled approximate routine).
    pub fn context_for(&self, selection: &Selection) -> Result<Value> {
        let f = self.frame.as_ref().ok_or_else(c::bad)?;
        let h = self.history.as_ref().ok_or_else(c::bad)?;
        let selected = h.select(f.now, selection)?;
        let time = h.time_from_records(f.now, self.timeline.as_ref().and_then(Timeline::phase), c::list(&selected)?)?;
        self.context_value(selected, time)
    }
    pub fn next_wake(&self) -> Option<u64> {
        let f = self.frame.as_ref()?;
        let next = [
            self.timeline.as_ref().and_then(Timeline::next_wake),
            self.history.as_ref().and_then(|h| h.next_expiry(f.now)),
        ]
        .into_iter()
        .flatten()
        .min()?
        .max(f.now + self.config.interval);
        (next <= u32::MAX as u64).then_some(next)
    }
    pub fn affected(&self, conditions: &[&str], activities: &[&str]) -> Result<bool> {
        self.timeline.as_ref().map_or(Ok(false), |t| t.affected(conditions, activities))
    }
    fn choices(&self) -> Result<(Choices, Vec<usize>)> {
        let t = self.timeline.as_ref().ok_or_else(c::bad)?;
        let f = self.frame.as_ref().ok_or_else(c::bad)?;
        let mut selected = Vec::new();
        let mut pairs = Vec::new();
        for i in t.ranked(f) {
            let p = obj! {"action"=>"activity","target"=>t.rows[i].id.clone()};
            if !pairs.contains(&p) {
                pairs.push(p);
                selected.push(i);
            }
            if selected.len() == self.config.choices {
                break;
            }
        }
        let choices =
            Choices::from_value(&obj! {"version"=>1,"revision"=>f.revision,"choices"=>pairs,"fallback"=>Value::Null})?;
        Ok((choices, selected))
    }
    pub(crate) fn schema(&self, name: &str) -> Result<Value> {
        if !self.config.tools.iter().any(|s| s == name) || self.timeline.is_none() || self.frame.is_none() {
            return Err(c::bad());
        }
        let (description,parameters)=match name {
            "routine_status"=>("Read current and next intended activity and a few actual observations. Plans do not prove experiences. Time may be unknown or contradictory.",obj!{"type"=>"object","properties"=>obj!{},"required"=>Vec::<Value>::new(),"additionalProperties"=>false}),
            "activity_propose"=>("Propose one currently permitted activity. This does not change the plan or execute anything; only the host may accept it.",self.choices()?.0.schema()),
            _=>return Err(c::bad()),
        };
        Ok(obj! {"type"=>"function","function"=>obj!{"name"=>name,"description"=>description,"parameters"=>parameters}})
    }
    pub(crate) fn call(
        &self,
        name: &str,
        input: &str,
        revision: u32,
        max_bytes: usize,
        mut available: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        self.schema(name)?;
        if self.revision() != Some(revision) {
            return Err(c::bad());
        }
        let a = crate::json::parse(input, 4096)?;
        let result = match name {
            "routine_status" => {
                c::fields(&a, &[])?;
                if !available(&obj! {"action"=>"routine_status","target"=>self.actor.clone()}) {
                    return Err(c::bad());
                }
                self.context()?
            }
            "activity_propose" => {
                let (choices, indices) = self.choices()?;
                choices.validate(&a)?;
                if !available(&a) {
                    return Err(c::bad());
                }
                let t = self.timeline.as_ref().expect("timeline");
                let row = indices
                    .into_iter()
                    .map(|i| &t.rows[i])
                    .find(|r| Some(r.id.as_str()) == a.get("target").and_then(Value::as_str))
                    .ok_or_else(c::bad)?;
                obj! {"status"=>"proposed","revision"=>revision,"activity"=>row.id.clone(),"start"=>row.start}
            }
            _ => return Err(c::bad()),
        };
        bounded(&result, max_bytes)?;
        Ok(result)
    }
    pub fn clear(&mut self) {
        if let Some(t) = &mut self.timeline {
            t.rows.clear();
        }
        if let Some(h) = &mut self.history {
            h.clear();
        }
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1,"scope"=>self.scope.clone(),"actor"=>self.actor.clone(),"participants"=>self.participants.clone(),"calendar"=>self.calendar.clone(),"config"=>self.config.to_value(),"definition"=>self.timeline.as_ref().map_or(Value::Null,|t|t.routine.to_value()),"frame"=>self.frame.as_ref().map_or(Value::Null,Frame::value),"timeline"=>self.timeline.as_ref().map_or(Value::Null,Timeline::to_value),"history"=>self.history.as_ref().map_or(Value::Null,History::to_value),"completion_token"=>self.completion_token,"calls"=>self.calls}
    }
    /// Validate bounded trusted save data against configured identity, calendar,
    /// definition and policy. Supply durable external watermarks on fresh processes.
    /// This protects against rollback, not malicious edits to a host-controlled save.
    pub fn restore(
        &mut self,
        v: &Value,
        min_revision: u32,
        min_observation: u64,
        min_completion: u64,
        min_tick: u64,
    ) -> Result<()> {
        bounded(v, 1_500_000)?;
        c::fields(
            v,
            &[
                "version",
                "scope",
                "actor",
                "participants",
                "calendar",
                "config",
                "definition",
                "frame",
                "timeline",
                "history",
                "completion_token",
                "calls",
            ],
        )?;
        let expected = self.to_value();
        for k in ["version", "scope", "actor", "participants", "calendar", "config", "definition"] {
            if v.get(k) != expected.get(k) {
                return Err(c::bad());
            }
        }
        if c::get(v, "frame")? == &Value::Null {
            if min_revision != 0 || min_observation != 0 || min_completion != 0 || min_tick != 0 || !v.same(&expected) {
                return Err(c::bad());
            }
            return Ok(());
        }
        let f = Frame::from_value(c::get(v, "frame")?)?;
        if f.revision < u64::from(min_revision.max(self.revision().unwrap_or(0)))
            || f.now < min_tick.max(self.frame.as_ref().map_or(0, |f| f.now))
        {
            return Err(c::bad());
        }
        let completion = num(v, "completion_token")?;
        let calls = num(v, "calls")?;
        if completion < min_completion.max(self.completion_token)
            || calls > self.config.model_calls
            || self.revision() == Some(f.revision as u32) && calls < self.calls
        {
            return Err(c::bad());
        }
        let mut draft = self.clone();
        if let Some(t) = &mut draft.timeline {
            if f.allowed.iter().any(|id| !t.routine.activities.iter().any(|a| &a.id == id)) {
                return Err(c::bad());
            }
            let rows = c::list(c::get(v, "timeline")?)?;
            if rows.len() > t.config.timeline {
                return Err(c::bad());
            }
            let mut entries: Vec<Entry> = Vec::new();
            for row in rows {
                c::fields(row, &["id", "start", "end", "state", "since", "reason"])?;
                let id = text(row, "id")?;
                let a = t.routine.activities.iter().find(|a| a.id == id).ok_or_else(c::bad)?;
                let start = num(row, "start")?;
                let end = num(row, "end")?;
                let since = num(row, "since")?;
                let state = text(row, "state")?;
                let reason = text(row, "reason")?;
                if start < a.start
                    || end != start + a.end - a.start
                    || start != a.start && !a.repeat.is_some_and(|p| (start - a.start) % p == 0)
                    || since > f.now
                    || a.deadline(start) <= f.now
                    || start > f.now + t.config.horizon
                    || !matches!(
                        (state, reason),
                        ("planned", "window")
                            | ("active", "selected")
                            | ("interrupted", "preempted")
                            | ("completed", "host")
                            | ("cancelled", "preempted" | "missed")
                    )
                    || entries.iter().any(|r| r.id == id && r.start == start)
                    || entries.last().is_some_and(|r| (r.start, r.id.as_str()) >= (start, id))
                {
                    return Err(c::bad());
                }
                entries.push(Entry { id: id.into(), start, end, state: state.into(), since, reason: reason.into() });
            }
            if entries.iter().filter(|r| r.state == "active").count() > 1
                || entries.iter().any(|r| r.state == "active" && !t.eligible(r, &f))
            {
                return Err(c::bad());
            }
            t.rows = entries;
            t.frame = Some(f.clone());
        } else if c::get(v, "timeline")? != &Value::Null || !f.allowed.is_empty() {
            return Err(c::bad());
        }
        if let Some(h) = &mut draft.history {
            h.restore(c::get(v, "history")?, min_observation, min_tick)?;
            if num(c::get(v, "history")?, "clock")? != f.now {
                return Err(c::bad());
            }
        } else if c::get(v, "history")? != &Value::Null || min_observation != 0 {
            return Err(c::bad());
        }
        draft.frame = Some(f);
        draft.completion_token = completion;
        draft.calls = calls;
        *self = draft;
        Ok(())
    }
}
/// Offline Python/Rust parity adapter, not a provider/model runtime. Histories and
/// timelines are evaluated separately so fixture observations may precede frames.
pub fn fixture(a: &Value) -> Result<Value> {
    bounded(a, 1_500_000)?;
    c::fields(a, &["definition", "defaults", "overrides", "frames", "observations", "now"])?;
    let config = Config::resolve(c::get(a, "defaults")?, c::get(a, "overrides")?)?;
    let routine = Arc::new(Routine::from_value(c::get(a, "definition")?)?);
    let mut timeline = Timeline::new(routine.clone(), config.clone())?;
    let frames = c::list(c::get(a, "frames")?)?;
    if frames.len() > 128 {
        return Err(c::bad());
    }
    for f in frames {
        timeline.update(Frame::from_value(f)?)?;
    }
    let mut history = History::new("fixture", "merchant", &vec!["merchant"].into(), &routine.calendar, config)?;
    let observations = c::list(c::get(a, "observations")?)?;
    if observations.len() > 128 {
        return Err(c::bad());
    }
    for o in observations {
        history.observe(o)?;
    }
    Ok(obj! {"timeline"=>timeline.to_value(),"time"=>history.time_knowledge(num(a,"now")?,timeline.phase())?})
}

/// Offline independent-oracle adapter for explicit selection and timer/dependency hints.
pub fn detail_fixture(a: &Value) -> Result<Value> {
    bounded(a, 1_500_000)?;
    c::fields(
        a,
        &[
            "definition",
            "defaults",
            "overrides",
            "frames",
            "observations",
            "now",
            "selection",
            "changed_conditions",
            "changed_activities",
        ],
    )?;
    let config = Config::resolve(c::get(a, "defaults")?, c::get(a, "overrides")?)?;
    let routine = Arc::new(Routine::from_value(c::get(a, "definition")?)?);
    let mut t = Timeline::new(routine.clone(), config.clone())?;
    let frames = c::list(c::get(a, "frames")?)?;
    if frames.len() > 128 {
        return Err(c::bad());
    }
    for f in frames {
        t.update(Frame::from_value(f)?)?;
    }
    let mut h = History::new("fixture", "merchant", &vec!["merchant"].into(), &routine.calendar, config.clone())?;
    let rows = c::list(c::get(a, "observations")?)?;
    if rows.len() > 128 {
        return Err(c::bad());
    }
    for o in rows {
        h.observe(o)?;
    }
    let now = num(a, "now")?;
    let selected = h.select(now, &Selection::from_value(c::get(a, "selection")?)?)?;
    let time = h.time_from_records(now, t.phase(), c::list(&selected)?)?;
    let f = t.frame.as_ref().ok_or_else(c::bad)?;
    let wake = [t.next_wake(), h.next_expiry(f.now)]
        .into_iter()
        .flatten()
        .min()
        .map(|at| at.max(f.now + config.interval))
        .filter(|&at| at <= u32::MAX as u64);
    let conditions = names(c::get(a, "changed_conditions")?, 128)?;
    let activities = names(c::get(a, "changed_activities")?, 128)?;
    let affected = t.affected(
        &conditions.iter().map(String::as_str).collect::<Vec<_>>(),
        &activities.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    Ok(
        obj! {"observations"=>selected,"time"=>time,"next_wake"=>wake.map_or(Value::Null,Value::from),"affected"=>affected},
    )
}
