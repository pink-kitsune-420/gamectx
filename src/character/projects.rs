//! Life projects (pass 4, stage 5).
//!
//! A project has an owner, an aim, a stake (how much of the owner's identity rides on it, 0–100),
//! a horizon (a deadline tick) and ordered milestones, each with a condition the host can check (a
//! task outcome, a fact, an item or a relationship level), an expected duration and an optional
//! alternative. Morale starts at 700,000 millionths. Progress comes only from confirmed outcomes:
//! a success completes the current milestone (+100,000 morale); a failure (−200,000) or a setback
//! (severity × 3,000) lowers morale and makes the owner decide by a fixed rule over
//!
//! `score = morale ÷ 10,000 + stake ÷ 2 + sunk`
//!
//! where `sunk = min(30, time ÷ 50 + money ÷ 100)` of what was invested, counted only while there
//! have been fewer than four failures in a row (sunk cost is honest but bounded): 80 or more
//! persist; 55 or more adapt (swap the current milestone for its unused alternative) or else
//! persist; 35 or more pause; below 35 abandon. Completion and abandonment name what they feed
//! (drift, lesson, aspiration); an abandoned project says a new aim can grow from it.
//!
//! For scheduling, `weight` gives an active project's motive score (stake scaled by urgency: the
//! expected work left against the time left before the horizon) and the task slot for its current
//! milestone. `offscreen` resolves the current milestone coarsely from elapsed time and host luck.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::projects::Projects;
//!
//! let mut p = Projects::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "propose", "args": {"id": "brewery", "owner": "tamsin", "aim": "Open a small brewery",
//!     "stake": 60, "horizon": 200, "milestones": [
//!       {"id": "buy_hops", "condition": {"kind": "item", "ref": "hops_sack"}, "expected": 20,
//!        "alternative": {"id": "trade_for_hops", "condition": {"kind": "relationship", "ref": "farmer_wen", "level": 60}, "expected": 30}}]}},
//!   {"op": "outcome", "args": {"project": "brewery", "milestone": "buy_hops", "success": false}},
//!   {"op": "outcome", "args": {"project": "brewery", "milestone": "buy_hops", "success": false}}
//! ]}"#, 4096)?;
//! let out = p.workflow(&req)?.to_json();
//! // The first failure is shrugged off; after the second, Tamsin tries trading for hops instead.
//! assert!(out.contains(r#""decision":"persist""#));
//! assert!(out.contains(r#""decision":"adapt""#) && out.contains(r#""current":"trade_for_hops""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, is_text, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "projects", default: 256, ceiling: 1_000_000 },
    Limit { name: "milestones", default: 8, ceiling: 32 },
];
pub const FULL: u64 = 1_000_000;
/// Starting morale (millionths).
pub const START: u64 = 700_000;
pub const CONDITIONS: [&str; 4] = ["task", "fact", "item", "relationship"];
pub const FEEDS: [&str; 3] = ["drift", "lesson", "aspiration"];

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

/// A checkable milestone condition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub kind: &'static str,
    pub reference: String,
    pub level: Option<u64>,
}

impl Condition {
    fn parse(v: &Value) -> Result<Self> {
        keys(v, &["kind", "ref"], &["level"])?;
        let s = c::get(v, "kind")?.as_str().ok_or_else(c::bad)?;
        let kind = CONDITIONS.iter().copied().find(|x| *x == s).ok_or_else(c::bad)?;
        let reference = id_of(v, "ref")?;
        let level = v.get("level").filter(|x| **x != Value::Null);
        let level = if kind == "relationship" {
            Some(uint_bound(level.ok_or_else(c::bad)?, 0, 100)?)
        } else {
            need(level.is_none())?;
            None
        };
        Ok(Condition { kind, reference, level })
    }

    fn view(&self) -> Value {
        let mut f = vec![
            ("kind".to_string(), Value::from(self.kind)),
            ("ref".to_string(), Value::from(self.reference.as_str())),
        ];
        if let Some(l) = self.level {
            f.push(("level".to_string(), Value::from(l)));
        }
        Value::Object(f)
    }
}

/// A milestone: id, condition, expected ticks and an unused alternative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Milestone {
    pub id: String,
    pub condition: Condition,
    pub expected: u64,
    pub alternative: Option<Box<Milestone>>,
}

impl Milestone {
    fn parse(v: &Value, allow_alternative: bool) -> Result<Self> {
        let fields = v.as_object().ok_or_else(c::bad)?;
        let alt = fields.iter().find(|(k, _)| k == "alternative").map(|(_, x)| x).filter(|x| **x != Value::Null);
        let rest =
            Value::Object(fields.iter().filter(|(k, _)| !(allow_alternative && k == "alternative")).cloned().collect());
        keys(&rest, &["id", "condition", "expected"], &[])?;
        let id = id_of(&rest, "id")?;
        let condition = Condition::parse(c::get(&rest, "condition")?)?;
        let expected = uint_bound(c::get(&rest, "expected")?, 1, MAX_AT)?;
        let alternative = match alt.filter(|_| allow_alternative) {
            Some(a) => Some(Box::new(Milestone::parse(a, false)?)),
            None => None,
        };
        Ok(Milestone { id, condition, expected, alternative })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub owner: String,
    pub aim: String,
    pub stake: u64,
    pub horizon: u64,
    pub milestones: Vec<Milestone>,
    pub at: usize,
    pub morale: u64,
    pub time: u64,
    pub money: u64,
    pub failures: u64,
    pub adapted: u64,
    pub status: &'static str,
}

impl Project {
    /// Invested time and money counted toward persisting, bounded at 30 and dropped after four
    /// failures in a row.
    pub fn sunk(&self) -> u64 {
        if self.failures >= 4 {
            0
        } else {
            (self.time / 50 + self.money / 100).min(30)
        }
    }

    pub fn score(&self) -> u64 {
        self.morale / 10_000 + self.stake / 2 + self.sunk()
    }

    fn current(&self) -> Option<&Milestone> {
        self.milestones.get(self.at)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Projects {
    pub projects: BTreeMap<String, Project>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Projects {
    fn default() -> Self {
        Projects { projects: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

fn decision(d: Option<&str>, feeds: bool, grows: Value, replaced: Option<String>, project: Value) -> Value {
    obj! {
        "decision" => d,
        "feeds" => if feeds { FEEDS.to_vec() } else { Vec::new() },
        "grows_from" => grows,
        "project" => project,
        "replaced" => replaced,
    }
}

impl Projects {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut p = Self::new();
        p.limits.apply(v)?;
        Ok(p)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "tick" => self.op_tick(args),
            "propose" => self.op_propose(args, l),
            "show" => self.op_show(args),
            "invest" => self.op_invest(args),
            "outcome" => self.op_outcome(args),
            "setback" => self.op_setback(args),
            "encourage" => self.op_encourage(args),
            "resume" => self.op_resume(args),
            "weight" => self.op_weight(args),
            "offscreen" => self.op_offscreen(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn get(&self, id: &str) -> Result<&Project> {
        self.projects.get(id).ok_or_else(c::bad)
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut Project> {
        self.projects.get_mut(id).ok_or_else(c::bad)
    }

    fn view(id: &str, p: &Project) -> Value {
        obj! {
            "adapted" => p.adapted,
            "aim" => p.aim.as_str(),
            "current" => p.current().map(|m| m.id.as_str()),
            "done" => p.milestones[..p.at].iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            "failures" => p.failures,
            "horizon" => p.horizon,
            "id" => id,
            "money" => p.money,
            "morale" => p.morale,
            "owner" => p.owner.as_str(),
            "remaining" => p.milestones[p.at..].iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            "score" => p.score(),
            "stake" => p.stake,
            "status" => p.status,
            "sunk" => p.sunk(),
            "time" => p.time,
        }
    }

    /// The owner's choice after a failure or setback, applied to the project.
    fn decide(id: &str, p: &mut Project) -> Value {
        let s = p.score();
        let has_alt = p.current().is_some_and(|m| m.alternative.is_some());
        let d = if s >= 80 {
            "persist"
        } else if s >= 55 {
            if has_alt {
                "adapt"
            } else {
                "persist"
            }
        } else if s >= 35 {
            "pause"
        } else {
            "abandon"
        };
        let mut replaced = None;
        let mut grows = Value::Null;
        match d {
            "adapt" => {
                let at = p.at;
                let old = &mut p.milestones[at];
                if let Some(alt) = old.alternative.take() {
                    replaced = Some(old.id.clone());
                    p.milestones[at] = *alt;
                    p.adapted += 1;
                }
            }
            "pause" => p.status = "paused",
            "abandon" => {
                p.status = "abandoned";
                grows = obj! { "aim" => p.aim.as_str(), "project" => id };
            }
            _ => {}
        }
        decision(Some(d), d == "abandon", grows, replaced, Self::view(id, p))
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn op_propose(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "owner", "aim", "stake", "horizon", "milestones"], &[])?;
        let id = id_of(args, "id")?;
        let owner = id_of(args, "owner")?;
        let aim = text_bound(c::get(args, "aim")?, 80)?.to_string();
        let stake = uint_bound(c::get(args, "stake")?, 0, 100)?;
        let horizon = uint_bound(c::get(args, "horizon")?, self.clock, MAX_AT)?;
        let rows = c::get(args, "milestones")?.as_array().ok_or_else(c::bad)?;
        need(!rows.is_empty() && rows.len() <= l.get("milestones"))?;
        let ms = rows.iter().map(|r| Milestone::parse(r, true)).collect::<Result<Vec<_>>>()?;
        let ids: Vec<&String> =
            ms.iter().map(|m| &m.id).chain(ms.iter().filter_map(|m| m.alternative.as_ref().map(|a| &a.id))).collect();
        need(ids.iter().collect::<BTreeSet<_>>().len() == ids.len())?;
        need(!self.projects.contains_key(&id) && self.projects.len() < l.get("projects"))?;
        let p = Project {
            owner,
            aim,
            stake,
            horizon,
            milestones: ms,
            at: 0,
            morale: START,
            time: 0,
            money: 0,
            failures: 0,
            adapted: 0,
            status: "active",
        };
        let out = Self::view(&id, &p);
        self.projects.insert(id, p);
        Ok(out)
    }

    fn op_show(&self, args: &Value) -> Result<Value> {
        keys(args, &["project"], &[])?;
        let id = id_of(args, "project")?;
        Ok(Self::view(&id, self.get(&id)?))
    }

    fn op_invest(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["project"], &["time", "money"])?;
        let id = id_of(args, "project")?;
        let t = opt_uint(args, "time", 0, 1_000_000, 0)?;
        let m = opt_uint(args, "money", 0, 1_000_000_000, 0)?;
        let p = self.get_mut(&id)?;
        need(p.status == "active" || p.status == "paused")?;
        p.time = (p.time + t).min(MAX_AT);
        p.money = (p.money + m).min(MAX_AT);
        Ok(Self::view(&id, p))
    }

    fn outcome(&mut self, id: &str, milestone: &str, success: bool) -> Result<Value> {
        let p = self.get_mut(id)?;
        need(p.status == "active")?;
        need(p.current().is_some_and(|m| m.id == milestone))?;
        if success {
            p.morale = (p.morale + 100_000).min(FULL);
            p.failures = 0;
            p.at += 1;
            let done = p.at == p.milestones.len();
            if done {
                p.status = "completed";
            }
            Ok(decision(done.then_some("complete"), done, Value::Null, None, Self::view(id, p)))
        } else {
            p.morale = p.morale.saturating_sub(200_000);
            p.failures += 1;
            Ok(Self::decide(id, p))
        }
    }

    fn op_outcome(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["project", "milestone", "success"], &[])?;
        let id = id_of(args, "project")?;
        let mid = id_of(args, "milestone")?;
        let Value::Bool(success) = c::get(args, "success")? else { return Err(c::bad()) };
        self.outcome(&id, &mid, *success)
    }

    fn op_setback(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["project", "severity"], &[])?;
        let id = id_of(args, "project")?;
        let sev = uint_bound(c::get(args, "severity")?, 1, 100)?;
        let p = self.get_mut(&id)?;
        need(p.status == "active")?;
        p.morale = p.morale.saturating_sub(sev * 3_000);
        Ok(Self::decide(&id, p))
    }

    fn op_encourage(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["project", "amount"], &[])?;
        let id = id_of(args, "project")?;
        let amount = uint_bound(c::get(args, "amount")?, 0, 100)?;
        let p = self.get_mut(&id)?;
        need(p.status == "active" || p.status == "paused")?;
        p.morale = (p.morale + amount * 5_000).min(FULL);
        Ok(Self::view(&id, p))
    }

    fn op_resume(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["project"], &[])?;
        let id = id_of(args, "project")?;
        let p = self.get_mut(&id)?;
        need(p.status == "paused")?;
        p.status = "active";
        Ok(Self::view(&id, p))
    }

    fn op_weight(&self, args: &Value) -> Result<Value> {
        keys(args, &["project"], &[])?;
        let id = id_of(args, "project")?;
        let p = self.get(&id)?;
        let Some(cur) = p.current().filter(|_| p.status == "active") else {
            return Ok(obj! { "motive" => Value::Null, "project" => id, "slot" => Value::Null, "urgency" => 0u64 });
        };
        let need_ticks: u64 = p.milestones[p.at..].iter().map(|m| m.expected).sum();
        let left = p.horizon.saturating_sub(self.clock);
        let urgency = if need_ticks >= left { 100 } else { need_ticks * 100 / left };
        let score = p.stake * (50 + urgency / 2) / 100;
        Ok(obj! {
            "motive" => obj! { "id" => format!("project:{id}"), "score" => score },
            "project" => id.as_str(),
            "slot" => obj! {
                "condition" => cur.condition.view(), "expected" => cur.expected, "milestone" => cur.id.as_str(),
                "owner" => p.owner.as_str(),
            },
            "urgency" => urgency,
        })
    }

    fn op_offscreen(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["project", "elapsed", "luck"], &[])?;
        let id = id_of(args, "project")?;
        let elapsed = uint_bound(c::get(args, "elapsed")?, 0, MAX_AT)?;
        let luck = uint_bound(c::get(args, "luck")?, 0, 100)?;
        let p = self.get(&id)?;
        let none = || obj! { "project" => id.as_str(), "resolved" => Value::Null, "result" => Value::Null };
        let Some(cur) = p.current().filter(|_| p.status == "active") else { return Ok(none()) };
        let success = if luck < 20 {
            false
        } else if luck >= 50 && elapsed >= cur.expected {
            true
        } else {
            return Ok(none());
        };
        let mid = cur.id.clone();
        let r = self.outcome(&id, &mid, success)?;
        Ok(obj! { "project" => id, "resolved" => if success { "success" } else { "failure" }, "result" => r })
    }
}

/// Project status words.
pub const STATUS: [&str; 4] = ["active", "paused", "completed", "abandoned"];
persist!(Condition { kind in CONDITIONS, level [0, 100], reference });
persist!(Milestone { alternative, condition, expected [1, MAX_AT], id });
persist!(Project {
    adapted [0, 32], aim, at [0, 32], failures [0, MAX_AT], horizon [0, MAX_AT], milestones, money [0, MAX_AT],
    morale [0, FULL], owner, stake [0, 100], status in STATUS, time [0, MAX_AT],
});
persist!(rest Projects { clock [0, MAX_AT], projects });
durable!(Projects, "projects", LIMITS);

fn milestone_ok(m: &Milestone) -> bool {
    is_ident(&m.id)
        && is_ident(&m.condition.reference)
        && m.condition.level.is_some() == (m.condition.kind == "relationship")
}

impl Projects {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("projects", self.projects.len())?;
        for (id, p) in &self.projects {
            ok(is_ident(id) && is_ident(&p.owner) && is_text(&p.aim, 80))?;
            ok(!p.milestones.is_empty() && p.at <= p.milestones.len())?;
            within::<Self>("milestones", p.milestones.len())?;
            ok((p.at == p.milestones.len()) == (p.status == "completed"))?;
            let mut ids = BTreeSet::new();
            for m in &p.milestones {
                ok(milestone_ok(m) && ids.insert(m.id.clone()))?;
                if let Some(a) = &m.alternative {
                    ok(milestone_ok(a) && a.alternative.is_none() && ids.insert(a.id.clone()))?;
                }
            }
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

/// Runs one request on a fresh [`Projects`] (local kind `projects`).
pub fn workflow(request: &Value) -> Result<Value> {
    Projects::new().workflow(request)
}
