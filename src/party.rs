//! Opt-in host-owned scenes: confirmed hearing, relative roles, bounded floor grants
//! and budgeted route selection. This module neither generates nor delivers speech.
use crate::{
    checked as c,
    life::{get, line, names, num, signed, text},
    obj,
    request::bounded,
    spending::SpendingLedger,
    Result, Value,
};
mod compile;
mod gates;
pub use compile::{compile_fixture, SceneTurn};
pub use gates::select_fixture as select_gates_fixture;
pub use gates::AttitudeGates;
#[derive(Clone, Debug)]
pub struct ModelRoute {
    value: Value,
}
impl ModelRoute {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 2048)?;
        c::fields(v, &["name", "size", "calls", "cost_micros"])?;
        c::name(text(v, "name")?)?;
        let size = text(v, "size")?;
        if !["0b", "1b", "1.5b", "3b", "8b", "30b", "70b"].contains(&size) {
            return Err(c::bad());
        }
        let calls = c::uint(get(v, "calls")?, 0, 8)?;
        let cost = c::uint(get(v, "cost_micros")?, 0, 1_000_000_000_000)?;
        if calls == 0 && cost != 0 || size == "0b" && calls != 0 {
            return Err(c::bad());
        }
        Ok(Self { value: v.clone() })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    /// Host maps a route name to an explicit identity-pinned deployment. No loading or
    /// runner mutation occurs here; recount the actual complete request before dispatch.
    pub fn bind_profile(&self, v: &Value, actual: &crate::deployment::Identity) -> Result<crate::deployment::Profile> {
        if v.get("name") != self.value.get("name") || text(&self.value, "size")? == "0b" {
            return Err(c::bad());
        }
        let profile = crate::deployment::Profile::from_value(v)?;
        profile.require_identity(actual)?;
        Ok(profile)
    }
}
#[derive(Clone, Debug)]
pub struct SpotlightPolicy {
    value: Value,
}
impl Default for SpotlightPolicy {
    fn default() -> Self {
        Self { value: obj! {"enabled"=>false,"significance"=>70} }
    }
}
impl SpotlightPolicy {
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 1024)?;
        c::fields(v, &["enabled", "significance"])?;
        c::boolean(get(v, "enabled")?)?;
        c::uint(get(v, "significance")?, 1, 100)?;
        Ok(Self { value: v.clone() })
    }
    /// Signals are host observations, never model self-reports. Crossings are computed
    /// from explicit before/after/threshold values, including downward crossings.
    pub fn reason(&self, v: &Value) -> Result<&'static str> {
        bounded(v, 8192)?;
        c::fields(v, &["significance", "changes", "conflicts"])?;
        let significance = c::uint(get(v, "significance")?, 0, 100)?;
        let conflicts = c::uint(get(v, "conflicts")?, 0, 16)?;
        let changes = c::list(get(v, "changes")?)?;
        if changes.len() > 16 {
            return Err(c::bad());
        }
        let mut crossing = false;
        for q in changes {
            c::fields(q, &["before", "after", "threshold"])?;
            let (before, after, threshold) =
                (signed(get(q, "before")?, 100)?, signed(get(q, "after")?, 100)?, signed(get(q, "threshold")?, 100)?);
            crossing |= before < threshold && threshold <= after || after < threshold && threshold <= before;
        }
        Ok(if !c::boolean(get(&self.value, "enabled")?)? {
            "routine"
        } else if significance >= num(&self.value, "significance")? {
            "significance"
        } else if crossing {
            "attitude_crossing"
        } else if conflicts >= 2 {
            "conflict"
        } else {
            "routine"
        })
    }
}
#[derive(Clone, Debug)]
pub struct FloorPolicy {
    value: Value,
}
impl Default for FloorPolicy {
    fn default() -> Self {
        Self { value: obj! {"enabled"=>false,"social_limit"=>1,"model_calls"=>0,"max_depth"=>1} }
    }
}
impl FloorPolicy {
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 1024)?;
        c::fields(v, &["enabled", "social_limit", "model_calls", "max_depth"])?;
        c::boolean(get(v, "enabled")?)?;
        c::uint(get(v, "social_limit")?, 0, 16)?;
        c::uint(get(v, "model_calls")?, 0, 8)?;
        c::uint(get(v, "max_depth")?, 0, 4)?;
        Ok(Self { value: v.clone() })
    }
}
#[derive(Debug)]
struct Grant {
    value: Value,
    status: &'static str,
}
/// One scene instance/save branch. Keep one owner for shared quotas; creating another
/// instance does not inherit history, reservations or replay protection.
#[derive(Debug)]
pub struct PartyScene {
    value: Value,
    lines: Vec<Value>,
    turn: Option<u64>,
    social: u64,
    calls: u64,
    served: Vec<(u64, Vec<String>)>,
    consumed: Vec<u64>,
    grants: Vec<Grant>,
    next: u64,
    clock: u64,
}
impl PartyScene {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 8192)?;
        c::fields(v, &["version", "id", "epoch", "revision", "participants"])?;
        c::uint(get(v, "version")?, 1, 1)?;
        c::name(text(v, "id")?)?;
        num(v, "epoch")?;
        num(v, "revision")?;
        let rows = c::list(get(v, "participants")?)?;
        if rows.is_empty() || rows.len() > 16 {
            return Err(c::bad());
        }
        for (i, p) in rows.iter().enumerate() {
            c::fields(p, &["id", "kind"])?;
            c::name(text(p, "id")?)?;
            if !matches!(text(p, "kind")?, "player" | "ai") || rows[..i].iter().any(|x| x.get("id") == p.get("id")) {
                return Err(c::bad());
            }
        }
        Ok(Self {
            value: v.clone(),
            lines: Vec::new(),
            turn: None,
            social: 0,
            calls: 0,
            served: Vec::new(),
            consumed: Vec::new(),
            grants: Vec::new(),
            next: 0,
            clock: 0,
        })
    }
    pub fn participants(&self) -> Value {
        c::list(get(&self.value, "participants").expect("scene"))
            .expect("roster")
            .iter()
            .map(|p| get(p, "id").expect("id").clone())
            .collect::<Vec<_>>()
            .into()
    }
    fn is_ai(&self, actor: &str) -> bool {
        c::list(get(&self.value, "participants").expect("scene")).expect("roster").iter().any(|p| {
            p.get("id").and_then(Value::as_str) == Some(actor) && p.get("kind").and_then(Value::as_str) == Some("ai")
        })
    }
    fn people(&self) -> Vec<String> {
        names(&self.participants(), 16).expect("roster")
    }
    /// Host receipt: text is exactly the confirmed portion, with an explicit heard set.
    /// Generated, queued or merely played text is insufficient. Overhearing is explicit.
    pub fn receipt(&mut self, r: &Value) -> Result<()> {
        bounded(r, 8192)?;
        c::fields(r, &["token", "speaker", "recipients", "heard", "text", "parent"])?;
        let token = c::uint(get(r, "token")?, 1, u32::MAX as u64)?;
        let people = self.people();
        let recipients = names(get(r, "recipients")?, 16)?;
        let heard = names(get(r, "heard")?, 16)?;
        if self.lines.len() >= 128
            || self.lines.last().is_some_and(|p| num(p, "token").expect("token") >= token)
            || !people.iter().any(|p| p == text(r, "speaker").unwrap_or(""))
            || recipients.is_empty()
            || recipients.iter().any(|p| !people.contains(p) || p == text(r, "speaker").unwrap_or(""))
            || heard.iter().any(|p| !people.contains(p))
        {
            return Err(c::bad());
        }
        line(text(r, "text")?)?;
        if get(r, "parent")? != &Value::Null {
            let parent = c::uint(get(r, "parent")?, 1, u32::MAX as u64)?;
            if !self.lines.iter().any(|p| num(p, "token").expect("token") == parent) {
                return Err(c::bad());
            }
        }
        self.lines.push(r.clone());
        if get(r, "parent")? != &Value::Null && !heard.is_empty() && self.is_ai(text(r, "speaker")?) {
            let mut root = r;
            while get(root, "parent")? != &Value::Null && self.is_ai(text(root, "speaker")?) {
                root =
                    self.lines.iter().find(|x| x.get("token") == root.get("parent")).expect("validated causal chain");
            }
            let token = num(root, "token")?;
            let speaker = text(root, "speaker")?.to_owned();
            let seen = if let Some(i) = self.served.iter().position(|(t, _)| *t == token) {
                i
            } else {
                self.served.push((token, vec![speaker]));
                self.served.len() - 1
            };
            let speaker = text(r, "speaker")?.to_owned();
            if !self.served[seen].1.contains(&speaker) {
                self.served[seen].1.push(speaker);
            }
        }
        Ok(())
    }
    pub fn render(&self, actor: &str, limit: usize) -> Result<Value> {
        self.render_before(actor, limit, u64::MAX)
    }
    fn render_before(&self, actor: &str, limit: usize, before: u64) -> Result<Value> {
        if !self.is_ai(actor) || limit > 16 {
            return Err(c::bad());
        }
        let mut rows = self
            .lines
            .iter()
            .filter(|r| {
                num(r, "token").expect("token") < before
                    && c::list(get(r, "heard").expect("heard"))
                        .expect("heard")
                        .iter()
                        .any(|p| p.as_str() == Some(actor))
            })
            .collect::<Vec<_>>();
        let skip = rows.len().saturating_sub(limit);
        rows.drain(..skip);
        Ok(rows.into_iter().map(|r|obj!{"role"=>if r.get("speaker").and_then(Value::as_str)==Some(actor){"assistant"}else{"user"},"content"=>format!("[{} -> {}] {}",text(r,"speaker").expect("speaker"),names(get(r,"recipients").expect("recipients"),16).expect("recipients").join(","),text(r,"text").expect("text"))}).collect::<Vec<_>>().into())
    }
    pub fn advance_revision(&mut self, revision: u64) -> Result<()> {
        if revision <= num(&self.value, "revision")? || revision > u32::MAX as u64 {
            return Err(c::bad());
        }
        crate::archetype::set_field(&mut self.value, "revision", revision.into());
        Ok(())
    }
    /// Select and reserve a bounded sequence before any I/O. Quotas cover all responders
    /// and repeated calls in this turn. Cancellation/failure never refunds call/social work.
    pub fn floor(&mut self, a: &Value, ledger: &mut SpendingLedger) -> Result<Value> {
        bounded(a, 65536)?;
        c::fields(a, &["turn", "now", "trigger", "policy", "spotlight", "candidates"])?;
        let (turn, now) = (num(a, "turn")?, num(a, "now")?);
        if self.turn.is_some_and(|t| turn < t) || now < self.clock {
            return Err(c::bad());
        }
        ledger.summary(now)?;
        let policy = FloorPolicy::from_value(get(a, "policy")?)?;
        let p = &policy.value;
        let spotlight = SpotlightPolicy::from_value(get(a, "spotlight")?)?;
        let trigger = self.lines.iter().find(|r| r.get("token") == a.get("trigger")).ok_or_else(c::bad)?.clone();
        let mut root = &trigger;
        let mut depth = 0;
        while get(root, "parent")? != &Value::Null && self.is_ai(text(root, "speaker")?) {
            root = self.lines.iter().find(|r| r.get("token") == root.get("parent")).ok_or_else(c::bad)?;
            depth += 1;
        }
        let root_token = num(root, "token")?;
        let root_speaker = text(root, "speaker")?.to_owned();
        let candidates = c::list(get(a, "candidates")?)?;
        if candidates.len() > 16 {
            return Err(c::bad());
        }
        for (i, v) in candidates.iter().enumerate() {
            c::fields(v, &["actor", "priority", "focused", "expanded", "signal"])?;
            if !self.is_ai(text(v, "actor")?) || candidates[..i].iter().any(|c| c.get("actor") == v.get("actor")) {
                return Err(c::bad());
            }
            signed(get(v, "priority")?, 100)?;
            let f = ModelRoute::from_value(get(v, "focused")?)?;
            if !["0b", "1b", "1.5b", "3b"].contains(&text(&f.value, "size")?) {
                return Err(c::bad());
            }
            if get(v, "expanded")? != &Value::Null {
                let e = ModelRoute::from_value(get(v, "expanded")?)?;
                if !["8b", "30b", "70b"].contains(&text(&e.value, "size")?) {
                    return Err(c::bad());
                }
            }
            spotlight.reason(get(v, "signal")?)?;
        }
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.calls = 0;
            self.social = 0;
        }
        self.clock = now;
        let quiet = |reason: &str, calls: u64, social: u64| obj! {"selected"=>Vec::<Value>::new(),"omitted"=>Vec::<Value>::new(),"reason"=>reason,"model_calls"=>calls,"social"=>social};
        if !c::boolean(get(p, "enabled")?)? {
            return Ok(quiet("disabled", self.calls, self.social));
        }
        let token = num(&trigger, "token")?;
        if self.consumed.contains(&token) {
            return Ok(quiet("replayed_trigger", self.calls, self.social));
        }
        if depth > num(p, "max_depth")? {
            return Ok(quiet("depth", self.calls, self.social));
        }
        self.consumed.push(token);
        let seen = if let Some(i) = self.served.iter().position(|(t, _)| *t == root_token) {
            i
        } else {
            self.served.push((root_token, vec![root_speaker]));
            self.served.len() - 1
        };
        let mut ranked = candidates.iter().enumerate().collect::<Vec<_>>();
        ranked.sort_by_key(|(i, c)| {
            (std::cmp::Reverse(signed(get(c, "priority").expect("priority"), 100).expect("priority")), *i)
        });
        let mut selected = Vec::new();
        let mut omitted = Vec::new();
        for (_, v) in ranked {
            let actor = text(v, "actor")?;
            let mut reason = if !names(get(&trigger, "heard")?, 16)?.iter().any(|p| p == actor)
                || !names(get(&trigger, "recipients")?, 16)?.iter().any(|p| p == actor)
            {
                Some("not_addressed_or_heard")
            } else if text(&trigger, "speaker")? == actor || self.served[seen].1.iter().any(|p| p == actor) {
                Some("circular")
            } else if self.social >= num(p, "social_limit")? {
                Some("social_limit")
            } else if self.grants.len() >= 128 || self.grants.iter().filter(|g| g.status == "reserved").count() >= 16 {
                Some("pending_limit")
            } else {
                None
            };
            let why = spotlight.reason(get(v, "signal")?)?;
            let mut choices = Vec::new();
            if why != "routine" && get(v, "expanded")? != &Value::Null {
                choices.push((get(v, "expanded")?, why));
            }
            choices.push((get(v, "focused")?, if why == "routine" { "routine" } else { "focused_fallback" }));
            let mut picked = None;
            if reason.is_none() {
                for (route, why) in choices {
                    let calls = num(route, "calls")?;
                    if self.calls + calls > num(p, "model_calls")? {
                        reason = Some("model_call_budget");
                        continue;
                    }
                    let reservation = if calls > 0 {
                        match ledger.reserve(now, c::uint(get(route, "cost_micros")?, 0, 1_000_000_000_000)?) {
                            Ok(id) => Some(id),
                            Err(_) => {
                                reason = Some("spending_budget");
                                continue;
                            }
                        }
                    } else {
                        None
                    };
                    picked = Some((route, why, reservation));
                    reason = None;
                    break;
                }
            }
            if let Some((route, why, reservation)) = picked {
                self.next += 1;
                self.calls += num(route, "calls")?;
                self.social += 1;
                self.served[seen].1.push(actor.into());
                let grant = obj! {"id"=>self.next,"actor"=>actor,"route"=>route.clone(),"reason"=>why,"reservation"=>reservation.map_or(Value::Null,Value::from),"revision"=>num(&self.value,"revision")?,"epoch"=>num(&self.value,"epoch")?,"turn"=>turn,"trigger"=>token};
                selected.push(grant.clone());
                self.grants.push(Grant { value: grant, status: "reserved" });
            } else {
                omitted.push(obj! {"actor"=>actor,"reason"=>reason.expect("omission reason")});
            }
        }
        Ok(
            obj! {"selected"=>selected.clone(),"omitted"=>omitted,"reason"=>if selected.is_empty(){"quiet"}else{"selected"},"model_calls"=>self.calls,"social"=>self.social},
        )
    }
    pub fn dispatch(&mut self, id: u64, now: u64, ledger: &mut SpendingLedger) -> Result<Value> {
        self.consume(id, now, ledger, false)
    }
    pub fn cancel(&mut self, id: u64, now: u64, ledger: &mut SpendingLedger) -> Result<Value> {
        self.consume(id, now, ledger, true)
    }
    fn consume(&mut self, id: u64, now: u64, ledger: &mut SpendingLedger, cancel: bool) -> Result<Value> {
        if now < self.clock || now > u32::MAX as u64 {
            return Err(c::bad());
        }
        ledger.summary(now)?;
        let g = self.grants.iter_mut().find(|g| g.value.get("id") == Some(&Value::from(id))).ok_or_else(c::bad)?;
        if g.status != "reserved"
            || !cancel
                && (g.value.get("revision") != self.value.get("revision")
                    || g.value.get("epoch") != self.value.get("epoch")
                    || Some(num(&g.value, "turn")?) != self.turn)
        {
            return Err(c::bad());
        }
        if get(&g.value, "reservation")? != &Value::Null {
            let reservation = num(&g.value, "reservation")?;
            if cancel {
                ledger.cancel_before_dispatch(reservation, now)?;
            } else {
                ledger.dispatch(reservation, now)?;
            }
        }
        g.status = if cancel { "cancelled" } else { "dispatched" };
        self.clock = now;
        Ok(
            obj! {"actor"=>get(&g.value,"actor")?.clone(),"route"=>get(&g.value,"route")?.clone(),"reservation"=>get(&g.value,"reservation")?.clone()},
        )
    }
}
/// Additive offline/bridge adapter. Inner operation errors are reported without stopping
/// a replay; malformed envelopes fail before creating a scene or reserving any spending.
pub fn workflow(a: &Value) -> Result<Value> {
    bounded(a, 262144)?;
    c::fields(a, &["scene", "ledger", "operations"])?;
    let mut scene = PartyScene::from_value(get(a, "scene")?)?;
    let v = get(a, "ledger")?;
    c::fields(v, &["limit", "window"])?;
    let mut ledger = SpendingLedger::new(c::uint(get(v, "limit")?, 0, 1_000_000_000_000)?, num(v, "window")?)?;
    let operations = c::list(get(a, "operations")?)?;
    if operations.len() > 256 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in operations {
        let mut run = || -> Result<Value> {
            c::fields(op, &["op", "args"])?;
            let v = get(op, "args")?;
            match text(op, "op")? {
                "receipt" => {
                    scene.receipt(v)?;
                    Ok(Value::Null)
                }
                "render" => {
                    c::fields(v, &["actor", "limit"])?;
                    scene.render(text(v, "actor")?, num(v, "limit")? as usize)
                }
                "floor" => scene.floor(v, &mut ledger),
                "dispatch" | "cancel" => {
                    c::fields(v, &["id", "now"])?;
                    if text(op, "op")? == "dispatch" {
                        scene.dispatch(num(v, "id")?, num(v, "now")?, &mut ledger)
                    } else {
                        scene.cancel(num(v, "id")?, num(v, "now")?, &mut ledger)
                    }
                }
                "revision" => {
                    c::fields(v, &["revision"])?;
                    scene.advance_revision(num(v, "revision")?)?;
                    Ok(Value::Null)
                }
                _ => Err(c::bad()),
            }
        };
        results.push(match run() {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(
        obj! {"results"=>results,"model_calls"=>scene.calls,"social"=>scene.social,"ledger"=>ledger.summary(scene.clock)?},
    )
}
