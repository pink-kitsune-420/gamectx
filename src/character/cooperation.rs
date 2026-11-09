//! Cooperation and conflict between beings (pass 2, B2/B3).
//!
//! **Cooperation (B2).** A joint task has an organizer, a goal and named roles. The organizer
//! `assign`s roles; any member may `ask_help` for an open role, and the helper accepts when
//!
//! `300_000 + trust·4_000 + min(favors the helper owes the asker, 5)·100_000
//!  − (busy ? 300_000 : 0) − effort·2_000 − grudge(helper→asker)/2 ≥ 500_000`
//!
//! (signed millionths). Roles can be handed off, completed or left. Completing a helped role
//! repays a favor the helper owed or puts the asker in the helper's debt, and proposes a
//! relationship `help`; a helper who leaves proposes `broken_promise` and the asker holds a
//! grudge. A task is `open` while a role is unfilled, `ready` when all are filled and `done`
//! when all are completed. **Promises** between beings propose an expectation `hope` for the
//! receiver; `keep` proposes `fulfilled_promise`, the receiver may `release` one, and `tick`
//! breaks every pending promise past its due tick (`broken_promise`, and a grudge).
//!
//! **Conflict (B3).** `claim` takes a free resource or joins its queue, ordered by contest
//! turns first, then `urgency·6_000 + status·4_000` (descending), then arrival. `free` passes
//! the resource to the head of the queue. A `contest` scores
//!
//! `500_000 + Δstatus·5_000 + Δurgency·4_000 + trust·3_000 − grudge(holder→challenger)/2`
//!
//! and the holder **yields** at 650_000 or more (resenting it when trust is below 20),
//! **stands** at 350_000 or less (the challenger queues and resents it) and otherwise offers
//! **turns** (the challenger goes next).
//!
//! **Grudges** (holder → against) weigh up to 1_000_000, fade by 1_000 per tick on `tick`, and
//! weigh against helping and yielding. Each grudge increase proposes an affect `appraise`
//! resentment. Every effect is a proposal for another engine (`{engine, op, args}`); the host
//! decides. Capacities are [host-set limits](crate::character::limits). Pure host computation
//! ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::cooperation::Cooperation;
//!
//! let mut co = Cooperation::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "form", "args": {"id": "harvest", "organizer": "farmer", "goal": "Bring in the barley",
//!                           "roles": [{"role": "reap", "being": "farmer"}, {"role": "cart"}]}},
//!   {"op": "ask_help", "args": {"task": "harvest", "role": "cart", "asker": "farmer", "helper": "carter",
//!                               "trust": 60, "effort": 20}}
//! ]}"#, 4096)?;
//! let out = co.workflow(&req)?.to_json();
//! assert!(out.contains(r#""accepted":true"#) && out.contains(r#""score":500000"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_bool, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, is_text, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "tasks", default: 64, ceiling: 1_000_000 },
    Limit { name: "roles", default: 8, ceiling: 64 },
    Limit { name: "promises", default: 256, ceiling: 1_000_000 },
    Limit { name: "resources", default: 256, ceiling: 1_000_000 },
    Limit { name: "queue", default: 16, ceiling: 1024 },
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "grudges", default: 32, ceiling: 4096 },
    Limit { name: "debts", default: 4096, ceiling: 1_000_000 },
];

pub const M: i64 = 1_000_000;
pub const HELP_BASE: i64 = 300_000;
pub const HELP_TRUST: i64 = 4_000;
pub const HELP_FAVOR: i64 = 100_000;
pub const HELP_FAVOR_CAP: u64 = 5;
pub const HELP_BUSY: i64 = 300_000;
pub const HELP_EFFORT: i64 = 2_000;
pub const HELP_ACCEPT: i64 = 500_000;
pub const CONTEST_BASE: i64 = 500_000;
pub const CONTEST_STATUS: i64 = 5_000;
pub const CONTEST_URGENCY: i64 = 4_000;
pub const CONTEST_TRUST: i64 = 3_000;
pub const YIELD_AT: i64 = 650_000;
pub const STAND_AT: i64 = 350_000;
pub const QUEUE_URGENCY: u64 = 6_000;
pub const QUEUE_STATUS: u64 = 4_000;
pub const BOND_TRUST: i64 = 20;
pub const GRUDGE_LEAVE: i64 = 200_000;
pub const GRUDGE_BROKEN: i64 = 300_000;
pub const GRUDGE_STAND: i64 = 100_000;
pub const GRUDGE_STAND_URGENCY: i64 = 3_000;
pub const GRUDGE_YIELD: i64 = 100_000;
pub const GRUDGE_YIELD_URGENCY: i64 = 2_000;
pub const FADE_PER_TICK: i64 = 1_000;

fn sint(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => {
            let n = s.parse::<i64>().map_err(|_| c::bad())?;
            if n < lo || n > hi {
                return Err(c::bad());
            }
            Ok(n)
        }
        _ => Err(c::bad()),
    }
}

fn need(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(c::bad())
    }
}

fn id_of(args: &Value, k: &str) -> Result<String> {
    Ok(ident(c::get(args, k)?)?.to_string())
}

fn at_of(args: &Value) -> Result<u64> {
    opt_uint(args, "at", 0, MAX_AT, 0)
}

fn rel(actor: &str, target: &str, action: &str) -> Value {
    obj! {
        "engine" => "relationship",
        "op" => "appraise_action",
        "args" => obj! { "actor" => actor, "target" => target, "action" => action, "success" => true },
    }
}

fn resent(being: &str, subject: &str, weight: i64, reason: &str) -> Value {
    obj! {
        "engine" => "affect",
        "op" => "appraise",
        "args" => obj! {
            "being" => being, "subject" => subject, "kind" => "resentment",
            "delta" => (weight / 10_000).max(1), "reason" => reason,
        },
    }
}

fn opt_id(v: &Option<String>) -> Value {
    v.as_deref().map(Value::from).unwrap_or(Value::Null)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub being: Option<String>,
    pub done: bool,
    /// The member who asked this role's helper in, if any.
    pub via: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JointTask {
    pub organizer: String,
    pub goal: String,
    pub roles: BTreeMap<String, Role>,
}

impl JointTask {
    fn status(&self) -> &'static str {
        if self.roles.values().all(|r| r.done) {
            "done"
        } else if self.roles.values().any(|r| r.being.is_none()) {
            "open"
        } else {
            "ready"
        }
    }

    fn holds(&self, being: &str) -> bool {
        self.roles.values().any(|r| r.being.as_deref() == Some(being))
    }

    fn view(&self, id: &str) -> Value {
        let roles = self
            .roles
            .iter()
            .map(|(k, r)| obj! { "role" => k.as_str(), "being" => opt_id(&r.being), "done" => r.done, "via" => opt_id(&r.via) })
            .collect();
        obj! {
            "id" => id, "organizer" => self.organizer.as_str(), "goal" => self.goal.as_str(),
            "status" => self.status(), "roles" => Value::Array(roles),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Promise {
    pub from: String,
    pub to: String,
    pub what: String,
    pub due: u64,
    /// `pending`, `kept`, `kept_late`, `released` or `broken`.
    pub status: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
    pub being: String,
    pub urgency: u64,
    pub status: u64,
    pub at: u64,
    pub seq: u64,
    /// Promised the next turn by a contest.
    pub turn: bool,
}

impl Waiting {
    fn priority(&self) -> (u8, Reverse<u64>, u64) {
        (u8::from(!self.turn), Reverse(self.urgency * QUEUE_URGENCY + self.status * QUEUE_STATUS), self.seq)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resource {
    pub holder: Option<String>,
    pub since: u64,
    pub queue: Vec<Waiting>,
}

impl Resource {
    fn ordered(&self) -> Vec<&Waiting> {
        let mut q: Vec<&Waiting> = self.queue.iter().collect();
        q.sort_by_key(|w| w.priority());
        q
    }

    fn position(&self, being: &str) -> Option<usize> {
        self.ordered().iter().position(|w| w.being == being).map(|i| i + 1)
    }

    fn queued(&self, being: &str) -> bool {
        self.queue.iter().any(|w| w.being == being)
    }

    fn view(&self, id: &str) -> Value {
        obj! {
            "resource" => id, "holder" => opt_id(&self.holder), "since" => self.since,
            "queue" => Value::Array(self.ordered().into_iter().map(|w| Value::from(w.being.as_str())).collect()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grudge {
    pub weight: i64,
    pub last: u64,
    pub reason: String,
}

/// Joint tasks, promises, contested resources, grudges and favors owed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cooperation {
    pub tasks: BTreeMap<String, JointTask>,
    pub promises: BTreeMap<String, Promise>,
    pub resources: BTreeMap<String, Resource>,
    /// holder → against → grudge.
    pub grudges: BTreeMap<String, BTreeMap<String, Grudge>>,
    /// (debtor, creditor) → favors owed.
    pub debts: BTreeMap<(String, String), u64>,
    pub clock: u64,
    pub seq: u64,
    pub limits: Limits,
}

impl Default for Cooperation {
    fn default() -> Self {
        Cooperation {
            tasks: BTreeMap::new(),
            promises: BTreeMap::new(),
            resources: BTreeMap::new(),
            grudges: BTreeMap::new(),
            debts: BTreeMap::new(),
            clock: 0,
            seq: 0,
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Cooperation {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut co = Self::new();
        co.limits.apply(v)?;
        Ok(co)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "form" => self.op_form(args, l),
            "assign" => self.op_assign(args),
            "ask_help" => self.op_ask_help(args),
            "hand_off" => self.op_hand_off(args),
            "complete" => self.op_complete(args, l),
            "leave" => self.op_leave(args, l),
            "promise" => self.op_promise(args, l),
            "keep" => self.op_keep(args),
            "release" => self.op_release(args),
            "tick" => self.op_tick(args, l),
            "claim" => self.op_claim(args, l),
            "free" => self.op_free(args),
            "contest" => self.op_contest(args, l),
            "grudges" => self.op_grudges(args),
            "assess" => self.op_assess(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    // ------------------------------------------------------------ grudges

    /// Refuses before any change when `holders` would add more grudge holders than allowed.
    fn grudge_room(&self, holders: &[&str], l: &Limits) -> Result<()> {
        let mut new: Vec<&str> = holders.iter().copied().filter(|h| !self.grudges.contains_key(*h)).collect();
        new.sort_unstable();
        new.dedup();
        need(self.grudges.len() + new.len() <= l.get("beings"))
    }

    fn add_grudge(&mut self, holder: &str, against: &str, weight: i64, at: u64, reason: &str, l: &Limits) -> Value {
        let book = self.grudges.entry(holder.to_string()).or_default();
        if !book.contains_key(against) && book.len() >= l.get("grudges") {
            if let Some(weakest) =
                book.iter().min_by(|a, b| (a.1.weight, a.0).cmp(&(b.1.weight, b.0))).map(|(k, _)| k.clone())
            {
                book.remove(&weakest);
            }
        }
        let g = book.entry(against.to_string()).or_insert(Grudge { weight: 0, last: at, reason: reason.to_string() });
        g.weight = (g.weight + weight).min(M);
        g.last = g.last.max(at);
        g.reason = reason.to_string();
        resent(holder, against, weight, reason)
    }

    fn grudge(&self, holder: &str, against: &str) -> i64 {
        self.grudges.get(holder).and_then(|b| b.get(against)).map_or(0, |g| g.weight)
    }

    fn grudge_rows(&self, being: &str) -> Value {
        let mut rows: Vec<(&String, &Grudge)> = self.grudges.get(being).map(|b| b.iter().collect()).unwrap_or_default();
        rows.sort_by(|a, b| b.1.weight.cmp(&a.1.weight).then_with(|| a.0.cmp(b.0)));
        Value::Array(
            rows.into_iter()
                .map(|(k, g)| obj! { "against" => k.as_str(), "weight" => g.weight, "reason" => g.reason.as_str() })
                .collect(),
        )
    }

    // ------------------------------------------------------------ B2 joint tasks

    fn task_role(&self, args: &Value) -> Result<(String, String)> {
        let tid = id_of(args, "task")?;
        let t = self.tasks.get(&tid).ok_or_else(c::bad)?;
        let rid = id_of(args, "role")?;
        need(t.roles.contains_key(&rid))?;
        Ok((tid, rid))
    }

    fn view(&self, tid: &str) -> Value {
        self.tasks.get(tid).map(|t| t.view(tid)).unwrap_or(Value::Null)
    }

    fn op_form(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "organizer", "goal", "roles"], &["at"])?;
        let tid = id_of(args, "id")?;
        let organizer = id_of(args, "organizer")?;
        let goal = text_bound(c::get(args, "goal")?, 64)?.to_string();
        at_of(args)?;
        need(!self.tasks.contains_key(&tid) && self.tasks.len() < l.get("tasks"))?;
        let rows = c::get(args, "roles")?.as_array().ok_or_else(c::bad)?;
        need(!rows.is_empty() && rows.len() <= l.get("roles"))?;
        let mut roles: BTreeMap<String, Role> = BTreeMap::new();
        for r in rows {
            keys(r, &["role"], &["being"])?;
            let rid = id_of(r, "role")?;
            let being = opt(r, "being").map(|v| ident(v).map(str::to_string)).transpose()?;
            need(
                !roles.contains_key(&rid)
                    && being.as_ref().is_none_or(|b| roles.values().all(|x| x.being.as_ref() != Some(b))),
            )?;
            roles.insert(rid, Role { being, done: false, via: None });
        }
        self.tasks.insert(tid.clone(), JointTask { organizer, goal, roles });
        Ok(self.view(&tid))
    }

    fn op_assign(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["task", "role", "being", "by"], &["at"])?;
        let (tid, rid) = self.task_role(args)?;
        let being = id_of(args, "being")?;
        let by = id_of(args, "by")?;
        at_of(args)?;
        let t = self.tasks.get_mut(&tid).ok_or_else(c::bad)?;
        need(by == t.organizer && t.roles[&rid].being.is_none() && !t.holds(&being))?;
        t.roles.get_mut(&rid).ok_or_else(c::bad)?.being = Some(being);
        Ok(self.view(&tid))
    }

    fn op_ask_help(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["task", "role", "asker", "helper", "trust"], &["busy", "effort", "at"])?;
        let (tid, rid) = self.task_role(args)?;
        let asker = id_of(args, "asker")?;
        let helper = id_of(args, "helper")?;
        let trust = sint(c::get(args, "trust")?, -100, 100)?;
        let busy = opt_bool(args, "busy", false)?;
        let effort = opt_uint(args, "effort", 0, 100, 50)? as i64;
        at_of(args)?;
        let t = &self.tasks[&tid];
        need(t.roles[&rid].being.is_none() && asker != helper && (asker == t.organizer || t.holds(&asker)))?;
        need(!t.holds(&helper))?;
        let owed = self.debts.get(&(helper.clone(), asker.clone())).copied().unwrap_or(0);
        let held = self.grudge(&helper, &asker);
        let score = HELP_BASE + trust * HELP_TRUST + owed.min(HELP_FAVOR_CAP) as i64 * HELP_FAVOR
            - if busy { HELP_BUSY } else { 0 }
            - effort * HELP_EFFORT
            - held / 2;
        let mut reasons = Vec::new();
        for (on, why) in [(trust != 0, "trust"), (owed > 0, "owes_favor"), (busy, "busy"), (held > 0, "grudge")] {
            if on {
                reasons.push(Value::from(why));
            }
        }
        let accepted = score >= HELP_ACCEPT;
        if accepted {
            let r = self.tasks.get_mut(&tid).and_then(|t| t.roles.get_mut(&rid)).ok_or_else(c::bad)?;
            r.being = Some(helper.clone());
            r.via = Some(asker);
        }
        Ok(obj! {
            "accepted" => accepted, "helper" => helper, "reasons" => Value::Array(reasons),
            "role" => rid, "score" => score, "task" => tid,
        })
    }

    fn op_hand_off(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["task", "role", "from", "to"], &["at"])?;
        let (tid, rid) = self.task_role(args)?;
        let from = id_of(args, "from")?;
        let to = id_of(args, "to")?;
        at_of(args)?;
        let t = self.tasks.get_mut(&tid).ok_or_else(c::bad)?;
        let r = &t.roles[&rid];
        need(r.being.as_deref() == Some(from.as_str()) && !r.done && !t.holds(&to))?;
        let r = t.roles.get_mut(&rid).ok_or_else(c::bad)?;
        r.being = Some(to);
        r.via = None;
        Ok(self.view(&tid))
    }

    fn op_complete(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["task", "role", "being"], &["at"])?;
        let (tid, rid) = self.task_role(args)?;
        let being = id_of(args, "being")?;
        at_of(args)?;
        let r = &self.tasks[&tid].roles[&rid];
        need(r.being.as_deref() == Some(being.as_str()) && !r.done)?;
        let via = r.via.clone();
        if let Some(v) = &via {
            let repays = self.debts.get(&(being.clone(), v.clone())).copied().unwrap_or(0) > 0;
            need(repays || self.debts.contains_key(&(v.clone(), being.clone())) || self.debts.len() < l.get("debts"))?;
        }
        self.tasks.get_mut(&tid).and_then(|t| t.roles.get_mut(&rid)).ok_or_else(c::bad)?.done = true;
        let mut effects = Vec::new();
        if let Some(v) = via {
            let back = (being.clone(), v.clone());
            match self.debts.get_mut(&back) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    if *n == 0 {
                        self.debts.remove(&back);
                    }
                }
                _ => *self.debts.entry((v.clone(), being.clone())).or_insert(0) += 1,
            }
            effects.push(rel(&being, &v, "help"));
        }
        Ok(obj! { "effects" => Value::Array(effects), "task" => self.view(&tid) })
    }

    fn op_leave(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["task", "role", "being"], &["at"])?;
        let (tid, rid) = self.task_role(args)?;
        let being = id_of(args, "being")?;
        let at = at_of(args)?;
        let r = &self.tasks[&tid].roles[&rid];
        need(r.being.as_deref() == Some(being.as_str()) && !r.done)?;
        let via = r.via.clone();
        let mut effects = Vec::new();
        if let Some(v) = &via {
            self.grudge_room(&[v.as_str()], l)?;
            effects.push(rel(&being, v, "broken_promise"));
            effects.push(self.add_grudge(v, &being, GRUDGE_LEAVE, at, "left_task", l));
        }
        let r = self.tasks.get_mut(&tid).and_then(|t| t.roles.get_mut(&rid)).ok_or_else(c::bad)?;
        r.being = None;
        r.via = None;
        Ok(obj! { "effects" => Value::Array(effects), "task" => self.view(&tid) })
    }

    // ------------------------------------------------------------ B2 promises

    fn promise_view(&self, pid: &str) -> Value {
        match self.promises.get(pid) {
            Some(p) => obj! {
                "id" => pid, "from" => p.from.as_str(), "to" => p.to.as_str(), "what" => p.what.as_str(),
                "due" => p.due, "status" => p.status,
            },
            None => Value::Null,
        }
    }

    fn op_promise(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "from", "to", "what", "due"], &["at"])?;
        let pid = id_of(args, "id")?;
        let from = id_of(args, "from")?;
        let to = id_of(args, "to")?;
        let what = id_of(args, "what")?;
        let due = uint_bound(c::get(args, "due")?, 0, MAX_AT)?;
        at_of(args)?;
        need(from != to && !self.promises.contains_key(&pid) && self.promises.len() < l.get("promises"))?;
        let effect = obj! {
            "engine" => "expectation",
            "op" => "expect",
            "args" => obj! {
                "being" => to.as_str(), "id" => pid.as_str(), "about" => from.as_str(), "event" => what.as_str(),
                "due" => due, "grace" => 0u64, "importance" => 50u64, "kind" => "hope",
            },
        };
        self.promises.insert(pid.clone(), Promise { from, to, what, due, status: "pending" });
        Ok(obj! { "effects" => Value::Array(vec![effect]), "promise" => self.promise_view(&pid) })
    }

    fn op_keep(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &["at"])?;
        let pid = id_of(args, "id")?;
        let at = at_of(args)?;
        let p = self.promises.get_mut(&pid).ok_or_else(c::bad)?;
        need(p.status == "pending")?;
        p.status = if at <= p.due { "kept" } else { "kept_late" };
        let effect = rel(&p.from, &p.to, "fulfilled_promise");
        Ok(obj! { "effects" => Value::Array(vec![effect]), "promise" => self.promise_view(&pid) })
    }

    fn op_release(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id", "by"], &["at"])?;
        let pid = id_of(args, "id")?;
        let by = id_of(args, "by")?;
        at_of(args)?;
        let p = self.promises.get_mut(&pid).ok_or_else(c::bad)?;
        need(p.status == "pending" && by == p.to)?;
        p.status = "released";
        Ok(obj! { "effects" => Value::Array(Vec::new()), "promise" => self.promise_view(&pid) })
    }

    fn op_tick(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        let broken: Vec<String> = self
            .promises
            .iter()
            .filter(|(_, p)| p.status == "pending" && p.due < now)
            .map(|(k, _)| k.clone())
            .collect();
        let receivers: Vec<String> = broken.iter().map(|pid| self.promises[pid].to.clone()).collect();
        self.grudge_room(&receivers.iter().map(String::as_str).collect::<Vec<_>>(), l)?;
        let mut faded = Vec::new();
        for (holder, book) in self.grudges.iter_mut() {
            book.retain(|against, g| {
                g.weight -= now.saturating_sub(g.last) as i64 * FADE_PER_TICK;
                g.last = g.last.max(now);
                if g.weight <= 0 {
                    faded.push(obj! { "holder" => holder.as_str(), "against" => against.as_str() });
                }
                g.weight > 0
            });
        }
        self.grudges.retain(|_, book| !book.is_empty());
        let mut effects = Vec::new();
        for pid in &broken {
            let p = self.promises.get_mut(pid).ok_or_else(c::bad)?;
            p.status = "broken";
            let (from, to) = (p.from.clone(), p.to.clone());
            effects.push(rel(&from, &to, "broken_promise"));
            effects.push(self.add_grudge(&to, &from, GRUDGE_BROKEN, now, "broken_promise", l));
        }
        self.clock = now;
        Ok(obj! {
            "broken" => Value::Array(broken.into_iter().map(Value::from).collect()),
            "effects" => Value::Array(effects), "faded" => Value::Array(faded), "now" => now,
        })
    }

    // ------------------------------------------------------------ B3 resources

    fn enqueue(&mut self, rid: &str, being: &str, urgency: u64, status: u64, at: u64, turn: bool) {
        self.seq += 1;
        let seq = self.seq;
        if let Some(res) = self.resources.get_mut(rid) {
            res.queue.push(Waiting { being: being.to_string(), urgency, status, at, seq, turn });
        }
    }

    fn res_view(&self, rid: &str) -> Value {
        self.resources.get(rid).map(|r| r.view(rid)).unwrap_or(Value::Null)
    }

    fn op_claim(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["resource", "being"], &["urgency", "status", "at"])?;
        let rid = id_of(args, "resource")?;
        let being = id_of(args, "being")?;
        let urgency = opt_uint(args, "urgency", 0, 100, 50)?;
        let status = opt_uint(args, "status", 0, 100, 50)?;
        let at = at_of(args)?;
        let free = self.resources.get(&rid).is_none_or(|r| r.holder.is_none());
        if free {
            need(self.resources.contains_key(&rid) || self.resources.len() < l.get("resources"))?;
            let res = self.resources.entry(rid.clone()).or_default();
            res.holder = Some(being);
            res.since = at;
            return Ok(obj! { "granted" => true, "position" => 0u64, "resource" => self.res_view(&rid) });
        }
        let res = &self.resources[&rid];
        need(res.holder.as_deref() != Some(being.as_str()) && !res.queued(&being))?;
        need(res.queue.len() < l.get("queue"))?;
        self.enqueue(&rid, &being, urgency, status, at, false);
        let pos = self.resources[&rid].position(&being).unwrap_or(0);
        Ok(obj! { "granted" => false, "position" => pos, "resource" => self.res_view(&rid) })
    }

    fn op_free(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["resource", "being"], &["at"])?;
        let rid = id_of(args, "resource")?;
        let being = id_of(args, "being")?;
        let at = at_of(args)?;
        let res = self.resources.get_mut(&rid).ok_or_else(c::bad)?;
        if res.holder.as_deref() == Some(being.as_str()) {
            let next = res.ordered().first().map(|w| w.seq);
            match next {
                Some(seq) => {
                    let i = res.queue.iter().position(|w| w.seq == seq).ok_or_else(c::bad)?;
                    let w = res.queue.remove(i);
                    res.holder = Some(w.being);
                }
                None => res.holder = None,
            }
            res.since = at;
        } else {
            need(res.queued(&being))?;
            res.queue.retain(|w| w.being != being);
        }
        Ok(obj! { "resource" => self.res_view(&rid) })
    }

    fn op_contest(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(
            args,
            &["resource", "challenger", "trust"],
            &["urgency", "status", "holder_urgency", "holder_status", "at"],
        )?;
        let rid = id_of(args, "resource")?;
        let ch = id_of(args, "challenger")?;
        let trust = sint(c::get(args, "trust")?, -100, 100)?;
        let urg = opt_uint(args, "urgency", 0, 100, 50)?;
        let st = opt_uint(args, "status", 0, 100, 50)?;
        let h_urg = opt_uint(args, "holder_urgency", 0, 100, 50)?;
        let h_st = opt_uint(args, "holder_status", 0, 100, 50)?;
        let at = at_of(args)?;
        let res = self.resources.get(&rid).ok_or_else(c::bad)?;
        let holder = res.holder.clone().ok_or_else(c::bad)?;
        need(holder != ch)?;
        let held = self.grudge(&holder, &ch);
        let score = CONTEST_BASE
            + (st as i64 - h_st as i64) * CONTEST_STATUS
            + (urg as i64 - h_urg as i64) * CONTEST_URGENCY
            + trust * CONTEST_TRUST
            - held / 2;
        let queued = res.queued(&ch);
        let rest = res.queue.len() - usize::from(queued);
        let qmax = l.get("queue");
        let mut effects = Vec::new();
        let outcome = if score >= YIELD_AT {
            need(rest < qmax)?;
            let resented = trust < BOND_TRUST;
            if resented {
                self.grudge_room(&[holder.as_str()], l)?;
            }
            let res = self.resources.get_mut(&rid).ok_or_else(c::bad)?;
            res.queue.retain(|w| w.being != ch);
            res.holder = Some(ch.clone());
            res.since = at;
            self.enqueue(&rid, &holder, h_urg, h_st, at, false);
            if resented {
                let w = GRUDGE_YIELD + h_urg as i64 * GRUDGE_YIELD_URGENCY;
                effects.push(self.add_grudge(&holder, &ch, w, at, "gave_way", l));
            }
            "yield"
        } else if score <= STAND_AT {
            need(queued || rest < qmax)?;
            self.grudge_room(&[ch.as_str()], l)?;
            self.resources.get_mut(&rid).ok_or_else(c::bad)?.queue.retain(|w| w.being != ch);
            self.enqueue(&rid, &ch, urg, st, at, false);
            let w = GRUDGE_STAND + urg as i64 * GRUDGE_STAND_URGENCY;
            effects.push(self.add_grudge(&ch, &holder, w, at, "refused", l));
            "stand"
        } else {
            need(queued || rest < qmax)?;
            self.resources.get_mut(&rid).ok_or_else(c::bad)?.queue.retain(|w| w.being != ch);
            self.enqueue(&rid, &ch, urg, st, at, true);
            "turns"
        };
        Ok(obj! {
            "effects" => Value::Array(effects), "holder" => holder, "outcome" => outcome,
            "resource" => self.res_view(&rid), "score" => score,
        })
    }

    // ------------------------------------------------------------ views

    fn op_grudges(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = id_of(args, "being")?;
        Ok(obj! { "being" => being.as_str(), "grudges" => self.grudge_rows(&being) })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        let mut roles = Vec::new();
        for (tid, t) in &self.tasks {
            for (rid, r) in &t.roles {
                if r.being.as_deref() == Some(b.as_str()) {
                    roles.push(obj! { "task" => tid.as_str(), "role" => rid.as_str(), "done" => r.done });
                }
            }
        }
        let pending = |f: &dyn Fn(&Promise) -> bool| -> Value {
            Value::Array(
                self.promises
                    .iter()
                    .filter(|(_, p)| p.status == "pending" && f(p))
                    .map(|(k, _)| Value::from(k.as_str()))
                    .collect(),
            )
        };
        let made = pending(&|p| p.from == b);
        let received = pending(&|p| p.to == b);
        let holds: Vec<Value> = self
            .resources
            .iter()
            .filter(|(_, r)| r.holder.as_deref() == Some(b.as_str()))
            .map(|(k, _)| Value::from(k.as_str()))
            .collect();
        let queued: Vec<Value> = self
            .resources
            .iter()
            .filter_map(|(k, r)| r.position(&b).map(|p| obj! { "resource" => k.as_str(), "position" => p }))
            .collect();
        let owes: Vec<Value> = self
            .debts
            .iter()
            .filter(|((d, _), _)| *d == b)
            .map(|((_, cr), n)| obj! { "to" => cr.as_str(), "favors" => *n })
            .collect();
        let owed: Vec<Value> = self
            .debts
            .iter()
            .filter(|((_, cr), _)| *cr == b)
            .map(|((d, _), n)| obj! { "by" => d.as_str(), "favors" => *n })
            .collect();
        Ok(obj! {
            "being" => b.as_str(), "grudges" => self.grudge_rows(&b), "holds" => Value::Array(holds),
            "owed" => Value::Array(owed), "owes" => Value::Array(owes), "promises_made" => made,
            "promises_received" => received, "queued" => Value::Array(queued), "roles" => Value::Array(roles),
        })
    }
}

/// A promise's statuses.
pub const PROMISE_STATUS: [&str; 5] = ["pending", "kept", "kept_late", "released", "broken"];
/// Why a grudge was last added to.
pub const GRUDGE_REASONS: [&str; 4] = ["broken_promise", "gave_way", "left_task", "refused"];

persist!(Role { being, done, via });
persist!(JointTask { goal, organizer, roles });
persist!(Promise { due [0, MAX_AT], from, status in PROMISE_STATUS, to, what });
persist!(Waiting { at [0, MAX_AT], being, seq [1, MAX_AT], status [0, 100], turn, urgency [0, 100] });
persist!(Resource { holder, queue, since [0, MAX_AT] });
persist!(Grudge { last [0, MAX_AT], reason, weight [1, M] });
persist!(rest Cooperation { clock [0, MAX_AT], debts, grudges, promises, resources, seq [0, MAX_AT], tasks });
durable!(Cooperation, "cooperation", LIMITS);

impl Cooperation {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("tasks", self.tasks.len())?;
        within::<Self>("promises", self.promises.len())?;
        within::<Self>("resources", self.resources.len())?;
        within::<Self>("beings", self.grudges.len())?;
        within::<Self>("debts", self.debts.len())?;
        for (id, t) in &self.tasks {
            ok(is_ident(id) && is_ident(&t.organizer) && is_text(&t.goal, 64) && !t.roles.is_empty())?;
            within::<Self>("roles", t.roles.len())?;
            let mut held = BTreeSet::new();
            for (rid, r) in &t.roles {
                ok(is_ident(rid) && r.being.as_ref().is_none_or(|b| is_ident(b) && held.insert(b)))?;
                ok((!r.done || r.being.is_some())
                    && r.via.as_ref().is_none_or(|v| is_ident(v) && r.being.as_ref() != Some(v)))?;
                ok(r.via.is_none() || r.being.is_some())?;
            }
        }
        for (id, p) in &self.promises {
            ok(is_ident(id) && is_ident(&p.from) && is_ident(&p.to) && is_ident(&p.what) && p.from != p.to)?;
        }
        for (id, r) in &self.resources {
            ok(is_ident(id) && r.holder.as_deref().is_none_or(is_ident) && (r.holder.is_some() || r.queue.is_empty()))?;
            within::<Self>("queue", r.queue.len())?;
            let mut seen = BTreeSet::new();
            ok(r.queue
                .iter()
                .all(|w| is_ident(&w.being) && Some(&w.being) != r.holder.as_ref() && seen.insert(&w.being)))?;
            ok(r.queue.windows(2).all(|w| w[0].seq < w[1].seq) && r.queue.iter().all(|w| w.seq <= self.seq))?;
        }
        for (holder, book) in &self.grudges {
            ok(is_ident(holder) && !book.is_empty())?;
            within::<Self>("grudges", book.len())?;
            let reason_ok = |g: &Grudge| GRUDGE_REASONS.contains(&g.reason.as_str());
            ok(book.iter().all(|(against, g)| is_ident(against) && against != holder && reason_ok(g)))?;
        }
        for ((debtor, creditor), n) in &self.debts {
            ok(is_ident(debtor) && is_ident(creditor) && debtor != creditor && *n >= 1 && *n <= MAX_AT)?;
            ok(!self.debts.contains_key(&(creditor.clone(), debtor.clone())))?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Cooperation::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(co: &mut Cooperation, ops: &str) -> Result<String> {
        co.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16)?).map(|v| v.to_json())
    }

    #[test]
    fn broken_promises_leave_grudges_that_fade() {
        let mut co = Cooperation::new();
        let out = run(
            &mut co,
            r#"{"op":"promise","args":{"id":"p","from":"smith","to":"miller","what":"mend_wheel","due":10}},
               {"op":"tick","args":{"now":11}},
               {"op":"tick","args":{"now":311}}"#,
        )
        .unwrap();
        assert!(
            out.contains(r#""broken":["p"]"#) && out.contains(r#""faded":[{"holder":"miller","against":"smith"}]"#),
            "{out}"
        );
        assert!(co.grudges.is_empty());
        assert!(run(&mut co, r#"{"op":"tick","args":{"now":5}}"#).is_err(), "the clock never runs backwards");
    }

    #[test]
    fn contests_yield_stand_or_offer_turns() {
        let mut co = Cooperation::new();
        let claim = r#"{"op":"claim","args":{"resource":"well","being":"ann"}}"#;
        let out = run(
            &mut co,
            &format!(r#"{claim},{{"op":"contest","args":{{"resource":"well","challenger":"bo","trust":0}}}}"#),
        )
        .unwrap();
        assert!(out.contains(r#""outcome":"turns""#), "{out}");
        let out = run(&mut co, r#"{"op":"contest","args":{"resource":"well","challenger":"cy","trust":-60}}"#).unwrap();
        assert!(out.contains(r#""outcome":"stand""#) && out.contains(r#""queue":["bo","cy"]"#), "{out}");
        let out = run(&mut co, r#"{"op":"contest","args":{"resource":"well","challenger":"bo","trust":60}}"#).unwrap();
        assert!(out.contains(r#""outcome":"yield""#) && out.contains(r#""holder":"bo""#), "{out}");
    }
}
