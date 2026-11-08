//! Motive arbiter: deterministic explainable ranking across motives (Gap 3).
//!
//! Takes candidate motives from:
//! - emergencies (physical danger, critical health <= 25, exhaustion collapse)
//! - needs (homeostatic physical drives with threshold transitions)
//! - promises and deadlines (commitments with remaining time / overdue status)
//! - routine slot (scheduled daily work activities)
//! - sheet goals (authored character goals)
//! - tasks (active planning tasks)
//! - reaction brains (situational impulses as inputs, never sole arbiter)
//!
//! Returns a ranked list on a common bounded 0..=100 scale, naming the winning motive
//! and explaining why it won.
//!
//! ```
//! use gamectx::character::motive::MotiveArbiter;
//! use gamectx::data::json::parse;
//!
//! let arbiter = MotiveArbiter::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "arbitrate",
//!       "args": {
//!         "body": {
//!           "health": 100,
//!           "needs": [{"id": "hunger", "level": 80, "threshold": 70}]
//!         },
//!         "routine": {"id": "bake_shift", "priority": 50, "window_active": true}
//!       }
//!     }
//!   ]
//! }"#, 4096)?;
//! let out = arbiter.workflow(&req)?;
//! assert_eq!(out.get("version").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::data::json::Value;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result};

pub const MAX_MOTIVES: usize = 64;
pub const MAX_OPS: usize = 64;
pub const DEFAULT_INERTIA: u64 = 10;
pub const MAX_INERTIA: u64 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MotiveKind {
    Reaction = 1,
    Goal = 2,
    Task = 3,
    Routine = 4,
    Promise = 5,
    Need = 6,
    Emergency = 7,
}

impl MotiveKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Emergency => "emergency",
            Self::Need => "need",
            Self::Promise => "promise",
            Self::Routine => "routine",
            Self::Task => "task",
            Self::Goal => "goal",
            Self::Reaction => "reaction",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "emergency" => Ok(Self::Emergency),
            "need" => Ok(Self::Need),
            "promise" => Ok(Self::Promise),
            "routine" => Ok(Self::Routine),
            "task" => Ok(Self::Task),
            "goal" => Ok(Self::Goal),
            "reaction" => Ok(Self::Reaction),
            _ => Err(c::bad()),
        }
    }

    pub fn precedence(&self) -> u8 {
        *self as u8
    }
}

fn ident_bound(v: &Value) -> Result<&str> {
    let s = ident(v)?;
    if s.is_empty() || s.len() > 64 {
        return Err(c::bad());
    }
    Ok(s)
}

fn uint_bound(v: &Value, lo: u64, hi: u64) -> Result<u64> {
    c::uint(v, lo, hi)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Motive {
    pub id: String,
    pub kind: MotiveKind,
    pub label: String,
    pub score: u64,
    pub action: Option<String>,
    pub reason: String,
}

impl Motive {
    pub fn to_value(&self) -> Value {
        let action_val = match &self.action {
            Some(a) => Value::String(a.clone()),
            None => Value::Null,
        };
        obj! {
            "id" => self.id.clone(),
            "kind" => self.kind.as_str(),
            "label" => self.label.clone(),
            "score" => self.score,
            "action" => action_val,
            "reason" => self.reason.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MotiveArbiter;

impl MotiveArbiter {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate_motive(&self, m: &Value) -> Result<Motive> {
        keys(
            m,
            &["id"],
            &[
                "kind",
                "label",
                "action",
                "score",
                "reason",
                "severity",
                "level",
                "threshold",
                "priority",
                "remaining_ticks",
                "overdue",
                "window_active",
                "time_to_start",
                "active",
                "raw_score",
            ],
        )?;

        let mid = ident_bound(c::get(m, "id")?)?.to_string();
        let kind = match m.get("kind") {
            Some(k) => MotiveKind::parse(ident_bound(k)?)?,
            None => MotiveKind::Goal,
        };

        let label = m.get("label").and_then(Value::as_str).unwrap_or(&mid).to_string();
        let action = m.get("action").and_then(Value::as_str).map(|s| s.to_string());

        if let Some(s_val) = m.get("score") {
            let score = uint_bound(s_val, 0, 100)?;
            let reason = m
                .get("reason")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("explicit score {score}"));
            return Ok(Motive { id: mid, kind, label, score, action, reason });
        }

        let (score, reason) = match kind {
            MotiveKind::Emergency => {
                let sev = m.get("severity").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
                let score = (80 + sev / 5).clamp(80, 100);
                let reason = m
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("emergency severity {sev} (score {score})"));
                (score, reason)
            }
            MotiveKind::Need => {
                let level = m.get("level").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
                let thresh = m.get("threshold").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(70);
                if level >= 100 {
                    (92, format!("need {mid} critical at 100 (causing damage)"))
                } else if level >= thresh {
                    let span = (100 - thresh).max(1);
                    let urgency = 50 + ((level - thresh) * 40) / span;
                    let score = urgency.clamp(50, 90);
                    (score, format!("need {mid} level {level} exceeds threshold {thresh} (urgent score {score})"))
                } else {
                    let span = thresh.max(1);
                    let score = ((level * 45) / span).clamp(0, 45);
                    (score, format!("need {mid} level {level} below threshold {thresh} (score {score})"))
                }
            }
            MotiveKind::Promise => {
                let prio = m.get("priority").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(70);
                let remaining = m.get("remaining_ticks").map(|v| uint_bound(v, 0, 1000)).transpose()?.unwrap_or(10);
                let overdue = m.get("overdue").and_then(Value::as_bool).unwrap_or(false) || remaining == 0;

                if overdue {
                    let score = (80 + prio / 5).clamp(80, 95);
                    (score, format!("promise {mid} is overdue (score {score})"))
                } else if remaining <= 5 {
                    let score = (70 + (5 - remaining) * 2 + prio / 10).clamp(70, 85);
                    (score, format!("promise {mid} impending deadline in {remaining} ticks (score {score})"))
                } else {
                    let score = prio.saturating_sub(remaining / 2).clamp(20, 70);
                    (score, format!("promise {mid} deadline in {remaining} ticks (score {score})"))
                }
            }
            MotiveKind::Routine => {
                let prio = m.get("priority").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
                let active = m.get("window_active").and_then(Value::as_bool).unwrap_or(true);
                let time_to_start = m.get("time_to_start").map(|v| uint_bound(v, 0, 1000)).transpose()?.unwrap_or(0);

                if active {
                    let score = prio.clamp(30, 75);
                    (score, format!("routine {mid} in active window with priority {prio} (score {score})"))
                } else {
                    let score = prio.saturating_sub(time_to_start).clamp(10, 50);
                    (score, format!("routine {mid} upcoming in {time_to_start} ticks (score {score})"))
                }
            }
            MotiveKind::Goal => {
                let prio = m.get("priority").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
                let active = m.get("active").and_then(Value::as_bool).unwrap_or(true);
                if active {
                    let score = prio.clamp(20, 70);
                    (score, format!("goal {mid} active with priority {prio} (score {score})"))
                } else {
                    (0, format!("goal {mid} conditions not met"))
                }
            }
            MotiveKind::Task => {
                let prio = m.get("priority").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(60);
                let score = prio.clamp(30, 80);
                (score, format!("task {mid} priority {prio} (score {score})"))
            }
            MotiveKind::Reaction => {
                let raw_score = m
                    .get("raw_score")
                    .or_else(|| m.get("priority"))
                    .map(|v| uint_bound(v, 0, 100))
                    .transpose()?
                    .unwrap_or(50);
                let score = raw_score.clamp(0, 65);
                (score, format!("reaction impulse score {score}"))
            }
        };

        Ok(Motive { id: mid, kind, label, score, action, reason })
    }

    pub fn op_arbitrate(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &["body", "routine", "goals", "promises", "emergencies", "brains", "motives", "commitment"])?;
        let commitment = match args.get("commitment").filter(|x| **x != Value::Null) {
            Some(cm) => {
                keys(cm, &["motive"], &["inertia"])?;
                let motive = ident_bound(c::get(cm, "motive")?)?.to_string();
                let inertia = cm
                    .get("inertia")
                    .filter(|x| **x != Value::Null)
                    .map(|v| uint_bound(v, 0, MAX_INERTIA))
                    .transpose()?
                    .unwrap_or(DEFAULT_INERTIA);
                Some((motive, inertia))
            }
            None => None,
        };

        let mut candidates: Vec<Value> = Vec::new();

        if let Some(body) = args.get("body").filter(|x| **x != Value::Null) {
            let health = body.get("health").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(100);
            if health <= 25 && health > 0 {
                candidates.push(obj! {
                    "id" => "critical_injury",
                    "kind" => "emergency",
                    "label" => "Seek medical attention immediately",
                    "severity" => 100 - health * 2,
                    "action" => "heal",
                    "reason" => format!("health critically low at {health}"),
                });
            } else if health == 0 {
                candidates.push(obj! {
                    "id" => "death",
                    "kind" => "emergency",
                    "label" => "Character is dead",
                    "score" => 100,
                    "action" => "none",
                    "reason" => "character is dead",
                });
            }

            let fatigue = body.get("fatigue").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(0);
            if fatigue >= 100 {
                candidates.push(obj! {
                    "id" => "exhaustion_collapse",
                    "kind" => "emergency",
                    "label" => "Collapsed from total exhaustion",
                    "score" => 95,
                    "action" => "rest",
                    "reason" => "fatigue reached 100 (total exhaustion)",
                });
            }

            if let Some(needs_arr) = body.get("needs").and_then(Value::as_array) {
                for n in needs_arr {
                    let nid = ident_bound(c::get(n, "id")?)?;
                    let label = n.get("label").and_then(Value::as_str).unwrap_or(nid);
                    let level = n.get("level").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
                    let thresh = n.get("threshold").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(70);
                    let suggested_action = n
                        .get("satisfiers")
                        .and_then(Value::as_array)
                        .and_then(|a| a.first())
                        .and_then(|s| s.get("action"))
                        .and_then(Value::as_str)
                        .map(Value::str)
                        .unwrap_or(Value::Null);

                    candidates.push(obj! {
                        "id" => format!("need_{nid}"),
                        "kind" => "need",
                        "label" => label,
                        "level" => level,
                        "threshold" => thresh,
                        "action" => suggested_action,
                    });
                }
            }
        }

        if let Some(routine) = args.get("routine").filter(|x| **x != Value::Null) {
            let rid = ident_bound(c::get(routine, "id")?)?;
            let label = routine.get("label").and_then(Value::as_str).unwrap_or(rid);
            let prio = routine.get("priority").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
            let window_active = routine.get("window_active").and_then(Value::as_bool).unwrap_or(true);
            let time_to_start = routine.get("time_to_start").map(|v| uint_bound(v, 0, 1000)).transpose()?.unwrap_or(0);
            let action = routine.get("action").and_then(Value::as_str).unwrap_or(rid);

            candidates.push(obj! {
                "id" => format!("routine_{rid}"),
                "kind" => "routine",
                "label" => label,
                "priority" => prio,
                "window_active" => window_active,
                "time_to_start" => time_to_start,
                "action" => action,
            });
        }

        if let Some(goals_arr) = args.get("goals").and_then(Value::as_array) {
            for g in goals_arr {
                let gid = ident_bound(c::get(g, "id")?)?;
                let label = g.get("description").or_else(|| g.get("label")).and_then(Value::as_str).unwrap_or(gid);
                let prio = g.get("priority").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
                let active = g.get("active").and_then(Value::as_bool).unwrap_or(true);
                let action = g.get("action").cloned().unwrap_or(Value::Null);

                candidates.push(obj! {
                    "id" => format!("goal_{gid}"),
                    "kind" => "goal",
                    "label" => label,
                    "priority" => prio,
                    "active" => active,
                    "action" => action,
                });
            }
        }

        if let Some(promises_arr) = args.get("promises").and_then(Value::as_array) {
            for p in promises_arr {
                let pid = ident_bound(c::get(p, "id")?)?;
                let label = p.get("task").and_then(Value::as_str).unwrap_or(pid);
                let prio = p.get("priority").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(70);
                let remaining = p.get("remaining_ticks").map(|v| uint_bound(v, 0, 1000)).transpose()?.unwrap_or(10);
                let overdue = p.get("overdue").and_then(Value::as_bool).unwrap_or(false);
                let action = p.get("action").cloned().unwrap_or(Value::Null);

                candidates.push(obj! {
                    "id" => format!("promise_{pid}"),
                    "kind" => "promise",
                    "label" => label,
                    "priority" => prio,
                    "remaining_ticks" => remaining,
                    "overdue" => overdue,
                    "action" => action,
                });
            }
        }

        if let Some(emg_arr) = args.get("emergencies").and_then(Value::as_array) {
            for e in emg_arr {
                let eid = ident_bound(c::get(e, "id")?)?;
                let label = e.get("label").and_then(Value::as_str).unwrap_or(eid);
                let sev = e.get("severity").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
                let action = e.get("action").cloned().unwrap_or(Value::Null);

                candidates.push(obj! {
                    "id" => format!("emergency_{eid}"),
                    "kind" => "emergency",
                    "label" => label,
                    "severity" => sev,
                    "action" => action,
                });
            }
        }

        if let Some(brains_arr) = args.get("brains").and_then(Value::as_array) {
            for b in brains_arr {
                let bid = ident_bound(c::get(b, "id")?)?;
                let label = b.get("label").and_then(Value::as_str).unwrap_or(bid);
                let score = b.get("score").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(50);
                let action = b.get("action").cloned().unwrap_or(Value::Null);

                candidates.push(obj! {
                    "id" => format!("reaction_{bid}"),
                    "kind" => "reaction",
                    "label" => label,
                    "raw_score" => score,
                    "action" => action,
                });
            }
        }

        if let Some(motives_arr) = args.get("motives").and_then(Value::as_array) {
            for m in motives_arr {
                candidates.push(m.clone());
            }
        }

        if candidates.len() > MAX_MOTIVES {
            return Err(c::bad());
        }

        let mut evaluated: Vec<Motive> = Vec::new();
        for c in &candidates {
            evaluated.push(self.evaluate_motive(c)?);
        }

        // Sort: score DESC, kind precedence DESC, id ASC
        evaluated.sort_by(|a, b| {
            b.score.cmp(&a.score).then(b.kind.precedence().cmp(&a.kind.precedence())).then(a.id.cmp(&b.id))
        });

        // Commitment inertia (pass 2, D1): keep the committed motive unless a challenger beats
        // it by at least `inertia` points. Emergencies always override.
        let mut held_over: Option<(String, u64)> = None;
        let mut present = false;
        if let Some((cid, inertia)) = &commitment {
            if let Some(pos) = evaluated.iter().position(|m| &m.id == cid) {
                present = true;
                let top = &evaluated[0];
                if pos > 0 && top.kind != MotiveKind::Emergency && top.score < evaluated[pos].score + inertia {
                    held_over = Some((top.id.clone(), top.score));
                    let kept = evaluated.remove(pos);
                    evaluated.insert(0, kept);
                }
            }
        }

        let winner_val = match evaluated.first() {
            Some(w) => w.to_value(),
            None => Value::Null,
        };

        let winning_reason = if let (Some((challenger, ch_score)), Some((_, inertia))) = (&held_over, &commitment) {
            let w = &evaluated[0];
            format!(
                "{} (score {}) held over {} (score {}) by commitment inertia {} because {}",
                w.id, w.score, challenger, ch_score, inertia, w.reason
            )
        } else if evaluated.is_empty() {
            "".to_string()
        } else if evaluated.len() == 1 {
            let w = &evaluated[0];
            format!("{} (score {}) won because {}", w.id, w.score, w.reason)
        } else {
            let w = &evaluated[0];
            let runner_up = &evaluated[1];
            format!(
                "{} (score {}) won over {} (score {}) because {}",
                w.id, w.score, runner_up.id, runner_up.score, w.reason
            )
        };

        let ranked_arr: Vec<Value> = evaluated.into_iter().map(|m| m.to_value()).collect();

        let mut out = obj! {
            "winner" => winner_val,
            "winning_reason" => winning_reason,
            "ranked" => Value::Array(ranked_arr),
            "count" => candidates.len(),
        };
        if let (Some((cid, inertia)), Value::Object(fields)) = (&commitment, &mut out) {
            fields.push((
                "commitment".to_string(),
                obj! {
                    "held" => held_over.is_some(),
                    "inertia" => *inertia,
                    "motive" => cid.as_str(),
                    "present" => present,
                },
            ));
        }
        Ok(out)
    }

    pub fn workflow(&self, req: &Value) -> Result<Value> {
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        let mut results = Vec::new();
        for op_val in ops {
            keys(op_val, &["op"], &["args"])?;
            let op = ident_bound(c::get(op_val, "op")?)?;
            let empty_args = Value::Object(Vec::new());
            let args = op_val.get("args").unwrap_or(&empty_args);

            let res = match op {
                "arbitrate" => self.op_arbitrate(args)?,
                _ => return Err(c::bad()),
            };
            results.push(res);
        }

        Ok(obj! {
            "version" => 1,
            "results" => Value::Array(results),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    let arbiter = MotiveArbiter::new();
    arbiter.workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    #[test]
    fn test_baker_hunger_beats_routine_bake() -> Result<()> {
        let arbiter = MotiveArbiter::new();
        let req = parse(
            r#"{
            "ops": [
                {
                    "op": "arbitrate",
                    "args": {
                        "body": {
                            "health": 100,
                            "needs": [
                                {
                                    "id": "hunger",
                                    "level": 80,
                                    "threshold": 70,
                                    "satisfiers": [{"action": "eat", "amount": 30}]
                                }
                            ]
                        },
                        "routine": {
                            "id": "bake_shift",
                            "priority": 50,
                            "window_active": true
                        }
                    }
                }
            ]
        }"#,
            4096,
        )?;
        let res = arbiter.workflow(&req)?;
        let results = res.get("results").and_then(Value::as_array).unwrap();
        let winner = results[0].get("winner").unwrap();
        assert_eq!(winner.get("id").and_then(Value::as_str), Some("need_hunger"));
        assert_eq!(winner.get("score").and_then(Value::as_usize), Some(63));
        assert_eq!(winner.get("action").and_then(Value::as_str), Some("eat"));

        let reason = results[0].get("winning_reason").and_then(Value::as_str).unwrap();
        assert!(reason.contains("won over routine_bake_shift"));
        Ok(())
    }
}
