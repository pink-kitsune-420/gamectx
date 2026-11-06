//! Host-confirmed observation history, independent of routines and generated dialogue.
//! Source IDs/quotes reuse the informational evidence contract. No fabricated confidence.
use crate::{
    checked as c, obj,
    request::bounded,
    routine::{names, num, text, Config},
    Result, Value,
};
#[derive(Clone, Debug)]
pub struct History {
    scope: String,
    actor: String,
    participants: Vec<String>,
    calendar: String,
    config: Config,
    rows: Vec<Value>,
    last: u64,
    clock: u64,
}
impl History {
    pub fn new(scope: &str, actor: &str, participants: &Value, calendar: &str, config: Config) -> Result<Self> {
        for s in [scope, actor, calendar] {
            c::scope(s)?;
        }
        let participants = names(participants, 16)?;
        if !config.history || !participants.iter().any(|p| p == actor) {
            return Err(c::bad());
        }
        Ok(Self {
            scope: scope.into(),
            actor: actor.into(),
            participants,
            calendar: calendar.into(),
            config,
            rows: Vec::new(),
            last: 0,
            clock: 0,
        })
    }
    fn validate(&self, o: &Value) -> Result<()> {
        bounded(o, 8192)?;
        c::fields(
            o,
            &[
                "token",
                "observer",
                "location",
                "at",
                "expires",
                "kind",
                "source",
                "audience",
                "value",
                "uncertainty",
                "time",
            ],
        )?;
        for k in ["observer", "location"] {
            c::scope(text(o, k)?)?;
        }
        let at = num(o, "at")?;
        let expires = num(o, "expires")?;
        let token = num(o, "token")?;
        let audience = names(c::get(o, "audience")?, 16)?;
        if text(o, "observer")? != self.actor
            || token == 0
            || expires <= at
            || expires - at > self.config.retention
            || audience.len() != self.participants.len()
            || audience.iter().any(|p| !self.participants.contains(p))
            || !matches!(text(o, "kind")?, "direct" | "instrument" | "report" | "inference")
        {
            return Err(c::bad());
        }
        crate::info::sources(&vec![c::get(o, "source")?.clone()].into())?;
        bounded(c::get(o, "value")?, 1024)?;
        if c::get(o, "uncertainty")? != &Value::Null {
            let u = text(o, "uncertainty")?;
            if u.is_empty() || u.len() > 256 || u.chars().any(char::is_control) {
                return Err(c::bad());
            }
        }
        let t = c::get(o, "time")?;
        if t != &Value::Null {
            c::fields(t, &["source", "calendar", "low", "high", "rate_low", "rate_high", "usable"])?;
            let basis = text(t, "source")?;
            let kind = text(o, "kind")?;
            let usable = c::boolean(c::get(t, "usable")?)?;
            let rate_low = c::uint(c::get(t, "rate_low")?, 0, 16)?;
            let rate_high = c::uint(c::get(t, "rate_high")?, 0, 16)?;
            if text(t, "calendar")? != self.calendar
                || num(t, "low")? > num(t, "high")?
                || rate_low > rate_high
                || usable && rate_low == rate_high
                || !matches!(
                    (basis, kind),
                    ("sun", "direct")
                        | ("instrument", "instrument")
                        | ("report", "report")
                        | ("inference", "inference")
                )
                || basis == "sun" && usable && c::get(o, "value")?.get("visible").and_then(Value::as_bool) != Some(true)
            {
                return Err(c::bad());
            }
        }
        Ok(())
    }
    /// Host-only ingestion. Reports preserve attribution; direct evidence must come
    /// from real sensor/event data. Sun readings require host-confirmed visibility,
    /// which can include windows/doorways. Weather or planned walks cannot supply it.
    /// Each scope has one audience: use separate histories for private participants.
    pub fn observe(&mut self, o: &Value) -> Result<()> {
        self.validate(o)?;
        let at = num(o, "at")?;
        let token = num(o, "token")?;
        if token <= self.last || at < self.clock {
            return Err(c::bad());
        }
        self.rows.retain(|r| num(r, "expires").expect("expiry") > at);
        if self.rows.len() == self.config.observations {
            self.rows.remove(0);
        }
        self.rows.push(o.clone());
        self.last = token;
        self.clock = at;
        Ok(())
    }
    pub fn advance(&mut self, now: u64) -> Result<()> {
        c::time(now, self.clock)?;
        self.rows.retain(|o| num(o, "expires").expect("expiry") > now);
        self.clock = now;
        Ok(())
    }
    pub fn recent(&self, now: u64) -> Result<Value> {
        c::time(now, self.clock)?;
        let mut rows = self
            .rows
            .iter()
            .rev()
            .filter(|o| num(o, "expires").expect("expiry") > now)
            .take(self.config.context_observations)
            .cloned()
            .collect::<Vec<_>>();
        rows.reverse();
        Ok(rows.into())
    }
    /// Return attributed ranges independently; conflicting sources stay separate.
    /// Coordinate arithmetic is host-defined, never a built-in Earth calendar.
    /// Growth rates bound how the source ages per simulation tick (0..16).
    pub fn time_knowledge(&self, now: u64, phase: Option<(&str, &str)>) -> Result<Value> {
        c::time(now, self.clock)?;
        let mut sources = Vec::new();
        if self.config.time {
            for o in self.rows.iter().rev() {
                if sources.len() >= self.config.context_observations {
                    break;
                }
                let t = c::get(o, "time")?;
                if t == &Value::Null
                    || now >= num(o, "expires")?
                    || !c::boolean(c::get(t, "usable")?)?
                    || !self
                        .config
                        .time_sources
                        .iter()
                        .any(|s| Some(s.as_str()) == t.get("source").and_then(Value::as_str))
                {
                    continue;
                }
                let age = now - num(o, "at")?;
                let low = num(t, "low")? + age * num(t, "rate_low")?;
                let high = num(t, "high")? + age * num(t, "rate_high")?;
                if high > u32::MAX as u64 {
                    continue;
                }
                sources.push(obj!{"source"=>c::get(o,"source")?.clone(),"basis"=>c::get(t,"source")?.clone(),"observed_at"=>num(o,"at")?,"age"=>age,"calendar"=>self.calendar.clone(),"low"=>low,"high"=>high,"precision"=>if low==high&&text(t,"source")?=="instrument"{"exact"}else{"range"},"uncertainty"=>c::get(o,"uncertainty")?.clone()});
            }
            if self.config.time_sources.iter().any(|s| s == "routine") {
                if let Some((routine, id)) = phase {
                    sources.push(obj! {"source"=>routine,"basis"=>"routine","phase"=>id,"precision"=>"approximate"});
                }
            }
        }
        let conflicting = sources.iter().enumerate().any(|(i, a)| {
            sources[..i].iter().any(|b| {
                a.get("calendar") == b.get("calendar")
                    && a.get("low").is_some()
                    && b.get("low").is_some()
                    && (num(a, "high").expect("range") < num(b, "low").expect("range")
                        || num(b, "high").expect("range") < num(a, "low").expect("range"))
            })
        });
        Ok(obj! {"status"=>if sources.is_empty(){"unknown"}else{"known"},"sources"=>sources,"conflicting"=>conflicting})
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1,"scope"=>self.scope.clone(),"actor"=>self.actor.clone(),"participants"=>self.participants.clone(),"calendar"=>self.calendar.clone(),"config"=>self.config.to_value(),"last"=>self.last,"clock"=>self.clock,"records"=>self.rows.clone()}
    }
    /// Trusted host persistence only. The caller supplies durable anti-rollback
    /// watermarks; a JSON snapshot is not authenticated and must never be model input.
    pub fn restore(&mut self, v: &Value, min_token: u64, min_tick: u64) -> Result<()> {
        bounded(v, 1_200_000)?;
        c::fields(v, &["version", "scope", "actor", "participants", "calendar", "config", "last", "clock", "records"])?;
        let mut expected = self.to_value();
        if let Value::Object(fields) = &mut expected {
            for (k, value) in fields {
                if !matches!(k.as_str(), "last" | "clock" | "records") && v.get(k) != Some(value) {
                    return Err(c::bad());
                }
            }
        }
        let last = num(v, "last")?;
        let clock = num(v, "clock")?;
        if last < min_token.max(self.last) || clock < min_tick.max(self.clock) {
            return Err(c::bad());
        }
        let rows = c::list(c::get(v, "records")?)?;
        if rows.len() > self.config.observations {
            return Err(c::bad());
        }
        let mut draft = self.clone();
        draft.rows.clear();
        draft.last = 0;
        draft.clock = 0;
        for o in rows {
            draft.observe(o)?;
            if num(o, "expires")? <= clock || num(o, "at")? > clock {
                return Err(c::bad());
            }
        }
        if draft.last > last {
            return Err(c::bad());
        }
        draft.last = last;
        draft.clock = clock;
        *self = draft;
        Ok(())
    }
    pub fn last_token(&self) -> u64 {
        self.last
    }
    pub fn clear(&mut self) {
        self.rows.clear();
    }
}
