//! Goals that arise from experience (pass 2, A4).
//!
//! Experiences build **pressure** per `(kind, subject)`: repeated hunger, repeated theft,
//! admiration for a skill, a loss, an opportunity. When a pressure reaches 100 a bounded
//! **template** turns it into a goal; later experiences of the same kind reinforce it:
//!
//! | kind | goal id | label | action |
//! |---|---|---|---|
//! | `need` | `stock_<subject>` | Keep a store against *subject* | `stock_up` |
//! | `threat` | `guard_<subject>` | Guard against *subject* | `secure_home` |
//! | `admiration` | `learn_<subject>` | Learn *subject* | `practice` |
//! | `loss` | `replace_<subject>` | Replace the lost *subject* | `seek` |
//! | `opportunity` | `pursue_<subject>` | Pursue *subject* | `pursue` |
//!
//! Cases the templates do not cover can come from a model as a `propose`al, which the host
//! validates: it must cite recorded pressures (combined total ≥ 50), its priority is capped at
//! 60, and there must be room. `goals` lists goals in the [motive arbiter](crate::character::motive)
//! `goals` shape. Capacities are [host-set limits](crate::character::limits). Pure host
//! computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::aspiration::Aspirations;
//!
//! let mut asp = Aspirations::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "experience", "args": {"being": "baker", "kind": "threat", "subject": "thief", "weight": 60, "at": 1}},
//!   {"op": "experience", "args": {"being": "baker", "kind": "threat", "subject": "thief", "weight": 60, "at": 9}},
//!   {"op": "goals", "args": {"being": "baker"}}
//! ]}"#, 4096)?;
//! let out = asp.workflow(&req)?;
//! assert!(out.to_json().contains(r#""id":"guard_thief""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, is_text, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "pressures", default: 64, ceiling: 4096 },
    Limit { name: "goals", default: 16, ceiling: 256 },
];
pub const KINDS: [&str; 5] = ["admiration", "loss", "need", "opportunity", "threat"];
pub const THRESHOLD: u64 = 100;
pub const WINDOW: u64 = 200;
pub const MAX_TOTAL: u64 = 1000;
pub const PROPOSAL_CAP: u64 = 60;
pub const PROPOSAL_EVIDENCE: u64 = 50;

#[derive(Clone, Debug, PartialEq)]
pub struct Pressure {
    pub total: u64,
    pub count: u64,
    pub last_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Goal {
    pub label: String,
    pub action: String,
    pub target: Option<String>,
    pub priority: u64,
    pub origin: &'static str,
    pub reason: String,
    pub formed_at: u64,
}

impl Goal {
    fn view(&self, id: &str) -> Value {
        obj! {
            "action" => self.action.as_str(),
            "formed_at" => self.formed_at,
            "id" => id,
            "label" => self.label.as_str(),
            "origin" => self.origin,
            "priority" => self.priority,
            "reason" => self.reason.as_str(),
            "target" => self.target.as_deref().map(Value::str).unwrap_or(Value::Null),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drive {
    /// `"<kind>:<subject>"` → pressure.
    pub pressures: BTreeMap<String, Pressure>,
    pub goals: BTreeMap<String, Goal>,
}

/// `(goal id, label, action)` for a template.
pub fn template(kind: &str, subject: &str) -> (String, String, &'static str) {
    match kind {
        "need" => (format!("stock_{subject}"), format!("Keep a store against {subject}"), "stock_up"),
        "threat" => (format!("guard_{subject}"), format!("Guard against {subject}"), "secure_home"),
        "admiration" => (format!("learn_{subject}"), format!("Learn {subject}"), "practice"),
        "loss" => (format!("replace_{subject}"), format!("Replace the lost {subject}"), "seek"),
        _ => (format!("pursue_{subject}"), format!("Pursue {subject}"), "pursue"),
    }
}

fn kind_of(v: &Value) -> Result<&str> {
    let k = text_bound(v, 16)?;
    if !KINDS.contains(&k) {
        return Err(c::bad());
    }
    Ok(k)
}

fn subject_of(v: &Value) -> Result<&str> {
    let s = ident(v)?;
    if s.len() > 48 {
        return Err(c::bad());
    }
    Ok(s)
}

/// Per-being pressures and goals.
#[derive(Clone, Debug)]
pub struct Aspirations {
    pub beings: BTreeMap<String, Drive>,
    pub limits: Limits,
}

impl Default for Aspirations {
    fn default() -> Self {
        Aspirations { beings: BTreeMap::new(), limits: Limits::defaults(LIMITS) }
    }
}

impl Aspirations {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (`{beings?, pressures?, goals?}`).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut a = Self::new();
        a.limits.apply(v)?;
        Ok(a)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "experience" => self.op_experience(args, l),
            "propose" => self.op_propose(args, l),
            "drop" => self.op_drop(args),
            "goals" => self.op_goals(args),
            "assess" => self.op_assess(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn drive(&mut self, being: &str, l: &Limits) -> Result<&mut Drive> {
        if !self.beings.contains_key(being) && self.beings.len() >= l.get("beings") {
            return Err(c::bad());
        }
        Ok(self.beings.entry(being.to_string()).or_default())
    }

    fn op_experience(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "kind", "subject"], &["weight", "at"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let kind = kind_of(c::get(args, "kind")?)?.to_string();
        let subject = subject_of(c::get(args, "subject")?)?.to_string();
        let weight = opt_uint(args, "weight", 1, 100, 50)?;
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let (gid, label, action) = template(&kind, &subject);
        let max_p = l.get("pressures");
        let max_g = l.get("goals");
        let d = self.drive(&being, l)?;
        let reason = |n: u64| format!("{n} {kind} experiences about {subject}");

        if let Some(g) = d.goals.get_mut(&gid) {
            let before = g.priority;
            g.priority = (g.priority + weight / 10).min(90);
            return Ok(obj! {
                "being" => being,
                "blocked" => false,
                "goal" => g.view(&gid),
                "kind" => kind.as_str(),
                "pressure" => Value::Null,
                "result" => if g.priority > before { "reinforced" } else { "steady" },
                "subject" => subject.as_str(),
            });
        }

        let key = format!("{kind}:{subject}");
        if !d.pressures.contains_key(&key) && d.pressures.len() >= max_p {
            let victim = d
                .pressures
                .iter()
                .min_by_key(|(k, p)| (p.total, p.last_at, (*k).clone()))
                .map(|(k, _)| k.clone())
                .ok_or_else(c::bad)?;
            d.pressures.remove(&victim);
        }
        let p = d.pressures.entry(key.clone()).or_insert(Pressure { total: 0, count: 0, last_at: at });
        if at > p.last_at && at - p.last_at > WINDOW {
            p.total = 0;
            p.count = 0;
        }
        p.total = (p.total + weight).min(MAX_TOTAL);
        p.count += 1;
        p.last_at = p.last_at.max(at);
        let pv = obj! { "count" => p.count, "last_at" => p.last_at, "total" => p.total };
        if p.total < THRESHOLD {
            return Ok(obj! {
                "being" => being, "blocked" => false, "goal" => Value::Null, "kind" => kind.as_str(),
                "pressure" => pv, "result" => "building", "subject" => subject.as_str(),
            });
        }
        if d.goals.len() >= max_g {
            return Ok(obj! {
                "being" => being, "blocked" => true, "goal" => Value::Null, "kind" => kind.as_str(),
                "pressure" => pv, "result" => "building", "subject" => subject.as_str(),
            });
        }
        let (total, count) = (p.total, p.count);
        d.pressures.remove(&key);
        let goal = Goal {
            label,
            action: action.to_string(),
            target: Some(subject.clone()),
            priority: (40 + total / 10).min(90),
            origin: "template",
            reason: reason(count),
            formed_at: at,
        };
        let view = goal.view(&gid);
        d.goals.insert(gid, goal);
        Ok(obj! {
            "being" => being, "blocked" => false, "goal" => view, "kind" => kind.as_str(),
            "pressure" => pv, "result" => "formed", "subject" => subject.as_str(),
        })
    }

    fn op_propose(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "goal", "priority", "cites"], &["at"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let g = c::get(args, "goal")?;
        keys(g, &["id", "label", "action"], &["target"])?;
        let gid = ident(c::get(g, "id")?)?.to_string();
        let label = text_bound(c::get(g, "label")?, 80)?.to_string();
        let action = ident(c::get(g, "action")?)?.to_string();
        let target = opt(g, "target").map(ident).transpose()?.map(str::to_string);
        let priority = uint_bound(c::get(args, "priority")?, 1, 100)?;
        let raw_cites = list_bound(Some(c::get(args, "cites")?), 4)?;
        if raw_cites.is_empty() {
            return Err(c::bad());
        }
        let mut cites = Vec::new();
        for ci in raw_cites {
            keys(ci, &["kind", "subject"], &[])?;
            let key = format!("{}:{}", kind_of(c::get(ci, "kind")?)?, subject_of(c::get(ci, "subject")?)?);
            if cites.contains(&key) {
                return Err(c::bad());
            }
            cites.push(key);
        }
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let max_g = l.get("goals");
        let d = self.drive(&being, l)?;
        let evidence: u64 = cites.iter().filter_map(|k| d.pressures.get(k)).map(|p| p.total).sum();
        let verdict = if d.goals.contains_key(&gid) {
            "exists"
        } else if cites.iter().any(|k| !d.pressures.contains_key(k)) {
            "uncited"
        } else if evidence < PROPOSAL_EVIDENCE {
            "weak_evidence"
        } else if d.goals.len() >= max_g {
            "no_room"
        } else {
            "accepted"
        };
        if verdict != "accepted" {
            return Ok(
                obj! { "accepted" => false, "being" => being, "evidence" => evidence, "goal" => Value::Null, "reason" => verdict },
            );
        }
        let goal = Goal {
            label,
            action,
            target,
            priority: priority.min(PROPOSAL_CAP),
            origin: "proposal",
            reason: format!("proposed from {}", cites.join(", ")),
            formed_at: at,
        };
        let view = goal.view(&gid);
        d.goals.insert(gid, goal);
        let reason = if priority > PROPOSAL_CAP { "capped" } else { "accepted" };
        Ok(obj! { "accepted" => true, "being" => being, "evidence" => evidence, "goal" => view, "reason" => reason })
    }

    fn op_drop(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "goal"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let gid = ident(c::get(args, "goal")?)?;
        let dropped = self.beings.get_mut(being).is_some_and(|d| d.goals.remove(gid).is_some());
        Ok(obj! { "being" => being, "dropped" => dropped, "goal" => gid })
    }

    fn op_goals(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let mut list: Vec<(&String, &Goal)> =
            self.beings.get(being).map(|d| d.goals.iter().collect()).unwrap_or_default();
        list.sort_by(|a, b| b.1.priority.cmp(&a.1.priority).then_with(|| a.0.cmp(b.0)));
        let goals = list
            .into_iter()
            .map(|(id, g)| obj! { "action" => g.action.as_str(), "id" => id.as_str(), "label" => g.label.as_str(), "priority" => g.priority })
            .collect();
        Ok(obj! { "being" => being, "goals" => Value::Array(goals) })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let d = self.beings.get(being).ok_or_else(c::bad)?;
        Ok(obj! {
            "being" => being,
            "goals" => Value::Array(d.goals.iter().map(|(id, g)| g.view(id)).collect()),
            "pressures" => Value::Array(d.pressures.iter().map(|(k, p)| {
                let (kind, subject) = k.split_once(':').unwrap_or((k.as_str(), ""));
                obj! { "count" => p.count, "kind" => kind, "last_at" => p.last_at, "subject" => subject, "total" => p.total }
            }).collect()),
        })
    }
}

/// Where a goal came from.
pub const ORIGINS: [&str; 2] = ["proposal", "template"];

persist!(Pressure { count [1, MAX_AT], last_at [0, MAX_AT], total [1, MAX_TOTAL] });
persist!(Goal { action, formed_at [0, MAX_AT], label, origin in ORIGINS, priority [1, 90], reason, target });
persist!(Drive { goals, pressures });
persist!(rest Aspirations { beings });
durable!(Aspirations, "aspiration", LIMITS);

/// A `"<kind>:<subject>"` key as `experience` and `propose` build it.
fn pressure_key_ok(k: &str) -> bool {
    k.split_once(':').is_some_and(|(kind, s)| KINDS.contains(&kind) && is_ident(s) && s.len() <= 48)
}

/// A goal as `experience` (origin `template`) or `propose` (origin `proposal`) forms it.
fn goal_ok(id: &str, g: &Goal) -> bool {
    if !is_ident(id) {
        return false;
    }
    if g.origin == "template" {
        let Some(subject) = g.target.as_deref() else { return false };
        let count_ok = |n: &str| n.parse::<u64>().is_ok_and(|x| x >= 1 && x.to_string() == n);
        return is_ident(subject)
            && subject.len() <= 48
            && KINDS.iter().any(|kind| {
                let (gid, label, action) = template(kind, subject);
                let n = g.reason.strip_suffix(&format!(" {kind} experiences about {subject}"));
                gid == id && label == g.label && action == g.action && n.is_some_and(count_ok)
            });
    }
    let cites: Vec<&str> = g.reason.strip_prefix("proposed from ").map(|r| r.split(", ").collect()).unwrap_or_default();
    let distinct = cites.iter().enumerate().all(|(i, k)| !cites[..i].contains(k));
    is_text(&g.label, 80)
        && is_ident(&g.action)
        && g.target.as_deref().is_none_or(is_ident)
        && (1..=4).contains(&cites.len())
        && distinct
        && cites.iter().all(|k| pressure_key_ok(k))
}

impl Aspirations {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("beings", self.beings.len())?;
        for (b, d) in &self.beings {
            ok(is_ident(b))?;
            within::<Self>("pressures", d.pressures.len())?;
            within::<Self>("goals", d.goals.len())?;
            ok(d.pressures.iter().all(|(k, p)| pressure_key_ok(k) && p.total <= 100 * p.count))?;
            ok(d.goals.iter().all(|(id, g)| goal_ok(id, g)))?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Aspirations::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(a: &mut Aspirations, ops: &str) -> String {
        a.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16).unwrap()).unwrap().to_json()
    }

    #[test]
    fn pressure_resets_after_a_quiet_window() {
        let mut a = Aspirations::new();
        let out = run(
            &mut a,
            r#"{"op":"experience","args":{"being":"b","kind":"need","subject":"hunger","weight":60,"at":0}},
               {"op":"experience","args":{"being":"b","kind":"need","subject":"hunger","weight":60,"at":201}}"#,
        );
        assert!(!out.contains("formed"), "{out}");
    }

    #[test]
    fn proposals_are_validated() {
        let mut a = Aspirations::new();
        let exp = r#"{"op":"experience","args":{"being":"b","kind":"loss","subject":"dog","weight":60}}"#;
        let prop = |p: u64| {
            format!(
                r#"{{"op":"propose","args":{{"being":"b","goal":{{"id":"adopt_pup","label":"Adopt a pup","action":"adopt"}},"priority":{p},"cites":[{{"kind":"loss","subject":"dog"}}]}}}}"#
            )
        };
        let out = run(&mut a, &format!("{},{exp},{}", prop(90), prop(90)));
        assert!(out.contains(r#""reason":"uncited""#) && out.contains(r#""reason":"capped""#), "{out}");
        assert!(out.contains(r#""priority":60"#), "{out}");
    }

    #[test]
    fn host_limits_cap_beings() {
        let mut a = Aspirations::new();
        let req =
            r#"{"limits":{"beings":1},"ops":[{"op":"experience","args":{"being":"a","kind":"need","subject":"x"}}]}"#;
        let out = a.workflow(&parse(req, 4096).unwrap()).unwrap().to_json();
        assert!(out.starts_with(r#"{"limits":{"beings":1,"pressures":64,"goals":16},"#), "{out}");
        let more = r#"{"ops":[{"op":"experience","args":{"being":"b","kind":"need","subject":"x"}}]}"#;
        assert!(a.workflow(&parse(more, 4096).unwrap()).is_err());
    }
}
