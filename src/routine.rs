//! Optional host-tick routines. Plans describe intent, never confirmed experience.
//! Share [`Routine`] with `Arc`; update characters through the existing work scheduler.
use crate::{checked as c, obj, request::bounded, Result, Value};
use std::sync::Arc;
mod runtime;
pub use runtime::{fixture, Runtime};
pub(crate) fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
pub(crate) fn num(v: &Value, k: &str) -> Result<u64> {
    c::uint(c::get(v, k)?, 0, u32::MAX as u64)
}
pub(crate) fn names(v: &Value, max: usize) -> Result<Vec<String>> {
    let rows = c::list(v)?;
    if rows.len() > max {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for row in rows {
        let s = c::text(row)?;
        c::scope(s)?;
        if out.iter().any(|old| old == s) {
            return Err(c::bad());
        }
        out.push(s.into());
    }
    Ok(out)
}
/// Runtime policy is independent of registration, turn exposure and live permission.
/// Resolve shared defaults plus a partial character override. All features default off.
#[derive(Clone, Debug)]
pub struct Config {
    pub(crate) routines: bool,
    pub(crate) history: bool,
    pub(crate) time: bool,
    pub(crate) tools: Vec<String>,
    pub(crate) timeline: usize,
    pub(crate) observations: usize,
    pub(crate) retention: u64,
    pub(crate) horizon: u64,
    pub(crate) interval: u64,
    pub(crate) min_active: u64,
    pub(crate) switch_margin: u64,
    pub(crate) choices: usize,
    pub(crate) context_observations: usize,
    pub(crate) model_calls: u64,
    pub(crate) time_sources: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            routines: false,
            history: false,
            time: false,
            tools: Vec::new(),
            timeline: 32,
            observations: 32,
            retention: 1000,
            horizon: 100,
            interval: 1,
            min_active: 2,
            switch_margin: 1,
            choices: 4,
            context_observations: 3,
            model_calls: 1,
            time_sources: Vec::new(),
        }
    }
}
impl Config {
    pub fn resolve(defaults: &Value, overrides: &Value) -> Result<Self> {
        let mut merged = Self::default().to_value();
        for layer in [defaults, overrides] {
            bounded(layer, 8192)?;
            let fields = layer.as_object().ok_or_else(c::bad)?;
            let Value::Object(ref mut target) = merged else { unreachable!() };
            for (k, v) in fields {
                let index = target.iter().position(|(key, _)| key == k).ok_or_else(c::bad)?;
                if fields.iter().filter(|(key, _)| key == k).count() != 1 {
                    return Err(c::bad());
                }
                target[index].1 = v.clone();
            }
        }
        let v = &merged;
        let b = |k| c::boolean(c::get(v, k)?);
        let n = |k, low, hi| c::uint(c::get(v, k)?, low, hi);
        let tools = names(c::get(v, "tools")?, 2)?;
        let time_sources = names(c::get(v, "time_sources")?, 5)?;
        if tools.iter().any(|t| !matches!(t.as_str(), "routine_status" | "activity_propose"))
            || time_sources
                .iter()
                .any(|s| !matches!(s.as_str(), "sun" | "instrument" | "report" | "inference" | "routine"))
        {
            return Err(c::bad());
        }
        let out = Self {
            routines: b("routines")?,
            history: b("history")?,
            time: b("time")?,
            tools,
            timeline: n("timeline", 1, 128)? as usize,
            observations: n("observations", 1, 128)? as usize,
            retention: n("retention", 1, 1000000)?,
            horizon: n("horizon", 1, 1000000)?,
            interval: n("interval", 1, 1000000)?,
            min_active: n("min_active", 0, 1000000)?,
            switch_margin: n("switch_margin", 0, 1000000)?,
            choices: n("choices", 1, 32)? as usize,
            context_observations: n("context_observations", 0, 32)? as usize,
            model_calls: n("model_calls", 0, 32)?,
            time_sources,
        };
        if out.time && !out.history || !out.tools.is_empty() && !out.routines {
            return Err(c::bad());
        }
        Ok(out)
    }
    pub fn to_value(&self) -> Value {
        obj! {"routines"=>self.routines,"history"=>self.history,"time"=>self.time,"tools"=>self.tools.clone(),"timeline"=>self.timeline,"observations"=>self.observations,"retention"=>self.retention,"horizon"=>self.horizon,"interval"=>self.interval,"min_active"=>self.min_active,"switch_margin"=>self.switch_margin,"choices"=>self.choices,"context_observations"=>self.context_observations,"model_calls"=>self.model_calls,"time_sources"=>self.time_sources.clone()}
    }
}
#[derive(Clone, Debug)]
struct Activity {
    id: String,
    location: String,
    start: u64,
    end: u64,
    repeat: Option<u64>,
    priority: u64,
    requirements: Vec<String>,
    interrupt: String,
    resume: String,
    missed: String,
    grace: u64,
}
impl Activity {
    fn deadline(&self, start: u64) -> u64 {
        start + self.end - self.start + if self.missed == "defer" { self.grace } else { 0 }
    }
    fn value(&self) -> Value {
        obj! {"id"=>self.id.clone(),"location"=>self.location.clone(),"start"=>self.start,"end"=>self.end,"repeat"=>self.repeat.map_or(Value::Null,Value::from),"priority"=>self.priority,"requirements"=>self.requirements.clone(),"interrupt"=>self.interrupt.clone(),"resume"=>self.resume.clone(),"missed"=>self.missed.clone(),"grace"=>self.grace}
    }
}
/// Strict version-1 definition; IDs and requirement predicates are resolved by the host.
/// `repeat` is a period in host ticks, unrelated to days or real-world calendars.
#[derive(Clone, Debug)]
pub struct Routine {
    id: String,
    revision: u64,
    calendar: String,
    activities: Vec<Activity>,
}
impl Routine {
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::json::parse(input, 131072)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 131072)?;
        c::fields(v, &["version", "id", "revision", "calendar", "activities"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        for k in ["id", "calendar"] {
            c::scope(text(v, k)?)?;
        }
        let rows = c::list(c::get(v, "activities")?)?;
        if rows.len() > 128 {
            return Err(c::bad());
        }
        let mut activities: Vec<Activity> = Vec::new();
        for a in rows {
            c::fields(
                a,
                &[
                    "id",
                    "location",
                    "start",
                    "end",
                    "repeat",
                    "priority",
                    "requirements",
                    "interrupt",
                    "resume",
                    "missed",
                    "grace",
                ],
            )?;
            for k in ["id", "location"] {
                c::scope(text(a, k)?)?;
            }
            let start = num(a, "start")?;
            let end = num(a, "end")?;
            let repeat = if c::get(a, "repeat")? == &Value::Null {
                None
            } else {
                Some(c::uint(c::get(a, "repeat")?, 1, 1000000)?)
            };
            let grace = c::uint(c::get(a, "grace")?, 0, 1000000)?;
            if start >= end
                || repeat.is_some_and(|r| r < end - start)
                || activities.iter().any(|old| old.id == text(a, "id").unwrap_or(""))
                || !["allow", "locked"].contains(&text(a, "interrupt")?)
                || !["resume", "cancel"].contains(&text(a, "resume")?)
                || !["cancel", "defer"].contains(&text(a, "missed")?)
                || text(a, "missed")? == "cancel" && grace != 0
            {
                return Err(c::bad());
            }
            activities.push(Activity {
                id: text(a, "id")?.into(),
                location: text(a, "location")?.into(),
                start,
                end,
                repeat,
                priority: c::uint(c::get(a, "priority")?, 0, 1000000)?,
                requirements: names(c::get(a, "requirements")?, 16)?,
                interrupt: text(a, "interrupt")?.into(),
                resume: text(a, "resume")?.into(),
                missed: text(a, "missed")?.into(),
                grace,
            });
        }
        Ok(Self {
            id: text(v, "id")?.into(),
            revision: num(v, "revision")?,
            calendar: text(v, "calendar")?.into(),
            activities,
        })
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1,"id"=>self.id.clone(),"revision"=>self.revision,"calendar"=>self.calendar.clone(),"activities"=>self.activities.iter().map(Activity::value).collect::<Vec<_>>()}
    }
}
/// Host snapshot of resolved conditions and activity permissions. A new revision is
/// required after relevant changes; emergency bypasses frequency/switch protection.
#[derive(Clone, Debug)]
pub struct Frame {
    revision: u64,
    now: u64,
    conditions: Vec<String>,
    allowed: Vec<String>,
    emergency: bool,
}
impl Frame {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 32768)?;
        c::fields(v, &["revision", "now", "conditions", "allowed", "emergency"])?;
        Ok(Self {
            revision: num(v, "revision")?,
            now: num(v, "now")?,
            conditions: names(c::get(v, "conditions")?, 128)?,
            allowed: names(c::get(v, "allowed")?, 128)?,
            emergency: c::boolean(c::get(v, "emergency")?)?,
        })
    }
    fn value(&self) -> Value {
        obj! {"revision"=>self.revision,"now"=>self.now,"conditions"=>self.conditions.clone(),"allowed"=>self.allowed.clone(),"emergency"=>self.emergency}
    }
}
#[derive(Clone, Debug)]
struct Entry {
    id: String,
    start: u64,
    end: u64,
    state: String,
    since: u64,
    reason: String,
}
impl Entry {
    fn live(&self) -> bool {
        matches!(self.state.as_str(), "planned" | "active" | "interrupted")
    }
    fn value(&self) -> Value {
        obj! {"id"=>self.id.clone(),"start"=>self.start,"end"=>self.end,"state"=>self.state.clone(),"since"=>self.since,"reason"=>self.reason.clone()}
    }
    fn set(&mut self, state: &str, reason: &str, now: u64) {
        self.state = state.into();
        self.reason = reason.into();
        self.since = now;
    }
}
/// Bounded deterministic intended activities. `active` means selected intent; it does
/// not certify movement, execution, completion, or any sensory experience.
#[derive(Clone, Debug)]
pub struct Timeline {
    routine: Arc<Routine>,
    config: Config,
    rows: Vec<Entry>,
    frame: Option<Frame>,
}
impl Timeline {
    pub fn new(routine: impl Into<Arc<Routine>>, config: Config) -> Result<Self> {
        if !config.routines {
            return Err(c::bad());
        }
        Ok(Self { routine: routine.into(), config, rows: Vec::new(), frame: None })
    }
    fn activity(&self, id: &str) -> &Activity {
        self.routine.activities.iter().find(|a| a.id == id).expect("validated activity")
    }
    fn eligible(&self, r: &Entry, f: &Frame) -> bool {
        let a = self.activity(&r.id);
        r.live()
            && r.start <= f.now
            && f.now < a.deadline(r.start)
            && f.allowed.contains(&r.id)
            && a.requirements.iter().all(|c| f.conditions.contains(c))
    }
    /// Local work only. Schedule this through `schedule::Scheduler`, whose work-unit
    /// budget and this runtime's interval both apply. Overflow fails atomically.
    pub fn update(&mut self, frame: Frame) -> Result<()> {
        if self.frame.as_ref().is_some_and(|old| {
            frame.revision <= old.revision
                || frame.now < old.now
                || !frame.emergency && frame.now - old.now < self.config.interval
        }) || frame.allowed.iter().any(|id| !self.routine.activities.iter().any(|a| &a.id == id))
        {
            return Err(c::bad());
        }
        let mut draft = self.clone();
        draft.update_inner(&frame)?;
        draft.frame = Some(frame);
        *self = draft;
        Ok(())
    }
    fn update_inner(&mut self, f: &Frame) -> Result<()> {
        let now = f.now;
        for r in &mut self.rows {
            let a = self.routine.activities.iter().find(|a| a.id == r.id).expect("activity");
            if r.live() && now >= a.deadline(r.start) {
                r.set("cancelled", "missed", now);
            }
        }
        self.rows.retain(|r| {
            r.live() || self.routine.activities.iter().find(|a| a.id == r.id).expect("activity").deadline(r.start) > now
        });
        for a in &self.routine.activities {
            let mut start = a.start;
            let duration = a.end - a.start;
            if let Some(period) = a.repeat {
                if now >= a.deadline(start) {
                    start += ((now - a.deadline(start)) / period + 1) * period;
                }
            }
            while start <= u64::from(u32::MAX).min(now + self.config.horizon) {
                if a.deadline(start) > now
                    && start + duration <= u32::MAX as u64
                    && !self.rows.iter().any(|r| r.id == a.id && r.start == start)
                {
                    if self.rows.len() >= self.config.timeline {
                        return Err(c::bad());
                    }
                    self.rows.push(Entry {
                        id: a.id.clone(),
                        start,
                        end: start + duration,
                        state: "planned".into(),
                        since: now,
                        reason: "window".into(),
                    });
                }
                match a.repeat {
                    Some(p) => start += p,
                    None => break,
                }
            }
        }
        self.rows.sort_by(|a, b| (a.start, &a.id).cmp(&(b.start, &b.id)));
        let active = self.rows.iter().position(|r| r.state == "active");
        let mut best = self.ranked(f).first().copied();
        if let (Some(i), Some(j)) = (active, best) {
            let a = self.activity(&self.rows[i].id);
            if self.eligible(&self.rows[i], f)
                && !f.emergency
                && (a.interrupt == "locked"
                    || now - self.rows[i].since < self.config.min_active
                    || self.activity(&self.rows[j].id).priority < a.priority + self.config.switch_margin)
            {
                best = Some(i);
            }
        }
        if active != best {
            if let Some(i) = active {
                let state =
                    if self.activity(&self.rows[i].id).resume == "resume" { "interrupted" } else { "cancelled" };
                self.rows[i].set(state, "preempted", now);
            }
            if let Some(j) = best {
                self.rows[j].set("active", "selected", now);
            }
        }
        Ok(())
    }
    fn ranked(&self, f: &Frame) -> Vec<usize> {
        let mut rows = (0..self.rows.len()).filter(|&i| self.eligible(&self.rows[i], f)).collect::<Vec<_>>();
        rows.sort_by(|&i, &j| {
            let a = &self.rows[i];
            let b = &self.rows[j];
            (std::cmp::Reverse(self.activity(&a.id).priority), a.start, &a.id).cmp(&(
                std::cmp::Reverse(self.activity(&b.id).priority),
                b.start,
                &b.id,
            ))
        });
        rows
    }
    pub fn to_value(&self) -> Value {
        self.rows.iter().map(Entry::value).collect::<Vec<_>>().into()
    }
    fn brief(&self, r: &Entry) -> Value {
        obj! {"id"=>r.id.clone(),"location"=>self.activity(&r.id).location.clone(),"state"=>r.state.clone()}
    }
    /// Compact context omits exact simulation ticks; required game facts belong in
    /// `TurnPlan::facts`, not in optional observations or the full routine dump.
    pub fn status(&self) -> Result<Value> {
        let f = self.frame.as_ref().ok_or_else(c::bad)?;
        let current = self.rows.iter().find(|r| r.state == "active").map_or(Value::Null, |r| self.brief(r));
        let next = self
            .rows
            .iter()
            .find(|r| {
                r.state == "planned"
                    && r.start > f.now
                    && f.allowed.contains(&r.id)
                    && self.activity(&r.id).requirements.iter().all(|c| f.conditions.contains(c))
            })
            .map_or(Value::Null, |r| self.brief(r));
        Ok(obj! {"current"=>current,"next"=>next})
    }
    fn phase(&self) -> Option<(&str, &str)> {
        self.rows.iter().find(|r| r.state == "active").map(|r| (self.routine.id.as_str(), r.id.as_str()))
    }
    fn complete(&mut self, id: &str, start: u64, revision: u64) -> Result<()> {
        let f = self.frame.as_ref().ok_or_else(c::bad)?;
        if f.revision != revision {
            return Err(c::bad());
        }
        let r =
            self.rows.iter_mut().find(|r| r.id == id && r.start == start && r.state == "active").ok_or_else(c::bad)?;
        r.set("completed", "host", f.now);
        Ok(())
    }
}
