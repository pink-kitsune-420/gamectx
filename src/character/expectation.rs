//! Expectations and surprise (pass 2, A3): what a being expects to happen, and how it feels
//! when that does not.
//!
//! A being **expects** an `event` about a subject (`about`) by a `due` time, plus a `grace`
//! period: a **hope** ("the player brings flour at noon") or a **dread** ("the tax collector
//! comes"). `relies` marks an expectation that leans on the subject's word, like a promise.
//! - `tick` advances time. Past `due` a hope is **worried** about (once, when there is a grace
//!   period); past `due + grace` it is **broken**: the being is disappointed (and resents the
//!   subject when it relied on them). A dread that never came brings **relief**. Strength is
//!   **surprise** = importance × confidence / 100.
//! - `happen` reports an event. Matching expectations resolve: a hope is **kept**, a dread is
//!   **met** (worry realized), and a hope that already broke arrives **late** (relief).
//! - Every resolution updates the subject's **reliability** (kept / broken), which sets the default
//!   confidence of the next expectation about it: 70 + 10 × kept − 15 × broken, within 10–95.
//!
//! Effects come as [affect](crate::character::affect) `set_mood` and `appraise` (`resentment`)
//! proposals in their exact argument shapes. Bounded: 32 expectations per being, 64 subjects
//! of reliability per being, 64 beings. Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::expectation::ExpectationBook;
//!
//! let mut book = ExpectationBook::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "expect", "args": {"being": "baker", "id": "flour", "about": "player", "event": "bring_flour",
//!     "due": 12, "grace": 2, "importance": 80, "relies": true}},
//!   {"op": "tick", "args": {"at": 13}},
//!   {"op": "tick", "args": {"at": 15}}
//! ]}"#, 4096)?;
//! let out = book.workflow(&req)?;
//! let broken = &out.get("results").unwrap().as_array().unwrap()[2];
//! assert!(broken.to_json().contains(r#""outcome":"disappointed""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_bool, opt_uint, run_ops, text_bound, uint_bound, MAX_AT};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const MAX_BEINGS: usize = 64;
pub const MAX_EXPECTATIONS: usize = 32;
pub const MAX_SUBJECTS: usize = 64;
pub const MAX_GRACE: u64 = 1_000_000;

/// Host-set capacity limits: `(name, default, ceiling)`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "beings", default: 64, ceiling: 1000000 },
    crate::character::limits::Limit { name: "expectations", default: 32, ceiling: 1024 },
    crate::character::limits::Limit { name: "subjects", default: 64, ceiling: 4096 },
];
crate::character::limits::module_limits!(ExpectationLimits, LIMITS);

#[derive(Clone, Debug, PartialEq)]
pub struct Expectation {
    pub about: String,
    pub event: String,
    pub due: u64,
    pub grace: u64,
    pub importance: u64,
    pub dread: bool,
    pub relies: bool,
    pub confidence: u64,
    pub label: String,
    /// `open`, `worried` or `broken`.
    pub state: &'static str,
}

impl Expectation {
    fn surprise(&self) -> u64 {
        self.importance * self.confidence / 100
    }

    fn view(&self, id: &str) -> Value {
        obj! {
            "about" => self.about.as_str(),
            "confidence" => self.confidence,
            "due" => self.due,
            "event" => self.event.as_str(),
            "grace" => self.grace,
            "id" => id,
            "importance" => self.importance,
            "kind" => if self.dread { "dread" } else { "hope" },
            "label" => self.label.as_str(),
            "relies" => self.relies,
            "state" => self.state,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mind {
    pub expectations: BTreeMap<String, Expectation>,
    /// `about` → `(kept, broken)`.
    pub reliability: BTreeMap<String, (u64, u64)>,
}

impl Mind {
    fn confidence(&self, about: &str) -> u64 {
        let (kept, broken) = self.reliability.get(about).copied().unwrap_or((0, 0));
        (70 + 10 * kept as i64 - 15 * broken as i64).clamp(10, 95) as u64
    }

    fn score(&mut self, about: &str, kept: bool, max_subjects: usize) {
        if !self.reliability.contains_key(about) && self.reliability.len() >= max_subjects {
            // Forget the least-observed subject (ties: smallest name).
            let victim =
                self.reliability.iter().min_by_key(|(k, (a, b))| (a + b, (*k).clone())).map(|(k, _)| k.clone());
            if let Some(v) = victim {
                self.reliability.remove(&v);
            }
        }
        let e = self.reliability.entry(about.to_string()).or_default();
        if kept {
            e.0 += 1;
        } else {
            e.1 += 1;
        }
    }
}

/// Per-being expectations and subject reliability.
#[derive(Clone, Debug, Default)]
pub struct ExpectationBook {
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: ExpectationLimits,
    pub beings: BTreeMap<String, Mind>,
}

fn mood(being: &str, delta: i64, reason: String) -> Value {
    obj! {
        "args" => obj! { "being" => being, "reason" => reason, "valence_delta" => delta },
        "engine" => "affect",
        "op" => "set_mood",
    }
}

fn resent(being: &str, subject: &str, delta: u64, reason: String) -> Value {
    obj! {
        "args" => obj! { "being" => being, "delta" => delta, "kind" => "resentment", "reason" => reason, "subject" => subject },
        "engine" => "affect",
        "op" => "appraise",
    }
}

impl ExpectationBook {
    pub fn new() -> Self {
        Self::default()
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        run_ops(request, |op, args| match op {
            "expect" => self.op_expect(args),
            "tick" => self.op_tick(args),
            "happen" => self.op_happen(args),
            "cancel" => self.op_cancel(args),
            "assess" => self.op_assess(args),
            _ => Err(c::bad()),
        })
    }

    fn only(&self, args: &Value) -> Result<Vec<String>> {
        let only = opt(args, "being").map(ident).transpose()?;
        if let Some(b) = only {
            if !self.beings.contains_key(b) {
                return Err(c::bad());
            }
            return Ok(vec![b.to_string()]);
        }
        Ok(self.beings.keys().cloned().collect())
    }

    fn op_expect(&mut self, args: &Value) -> Result<Value> {
        keys(
            args,
            &["being", "id", "about", "event", "due", "importance"],
            &["grace", "kind", "relies", "confidence", "label"],
        )?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let id = ident(c::get(args, "id")?)?.to_string();
        let about = ident(c::get(args, "about")?)?.to_string();
        let event = ident(c::get(args, "event")?)?.to_string();
        let due = uint_bound(c::get(args, "due")?, 0, MAX_AT)?;
        let importance = uint_bound(c::get(args, "importance")?, 1, 100)?;
        let grace = opt_uint(args, "grace", 0, MAX_GRACE, 0)?;
        let dread = match opt(args, "kind").map(|v| text_bound(v, 8)).transpose()? {
            None | Some("hope") => false,
            Some("dread") => true,
            Some(_) => return Err(c::bad()),
        };
        let relies = opt_bool(args, "relies", false)?;
        let confidence = opt(args, "confidence").map(|v| uint_bound(v, 1, 100)).transpose()?;
        let label = match opt(args, "label") {
            Some(v) => text_bound(v, 80)?.to_string(),
            None => format!("{about} {event}"),
        };
        if !self.beings.contains_key(&being) && self.beings.len() >= self.limits.get("beings") {
            return Err(c::bad());
        }
        let max_expectations = self.limits.get("expectations");
        let mind = self.beings.entry(being.clone()).or_default();
        if !mind.expectations.contains_key(&id) && mind.expectations.len() >= max_expectations {
            // Make room by dropping the broken expectation that fell due first (ties: smallest id).
            let victim = mind
                .expectations
                .iter()
                .filter(|(_, e)| e.state == "broken")
                .min_by_key(|(k, e)| (e.due, (*k).clone()))
                .map(|(k, _)| k.clone())
                .ok_or_else(c::bad)?;
            mind.expectations.remove(&victim);
        }
        let confidence = confidence.unwrap_or_else(|| mind.confidence(&about));
        mind.expectations.insert(
            id.clone(),
            Expectation { about, event, due, grace, importance, dread, relies, confidence, label, state: "open" },
        );
        Ok(obj! {
            "being" => being,
            "confidence" => confidence,
            "due_by" => due + grace,
            "id" => id,
            "open" => mind.expectations.len(),
        })
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["at"], &["being"])?;
        let at = uint_bound(c::get(args, "at")?, 0, MAX_AT)?;
        let names = self.only(args)?;
        let max_subjects = self.limits.get("subjects");
        let (mut changes, mut effects) = (Vec::new(), Vec::new());
        for name in names {
            let mind = self.beings.get_mut(&name).ok_or_else(c::bad)?;
            let mut relieved = Vec::new();
            let mut scored = Vec::new();
            for (id, e) in mind.expectations.iter_mut() {
                if e.state == "broken" || at <= e.due {
                    continue;
                }
                let surprise = e.surprise();
                let reason = format!("expected {}", e.label);
                let outcome = if at > e.due + e.grace {
                    if e.dread {
                        relieved.push(id.clone());
                        scored.push((e.about.clone(), false));
                        if surprise / 4 > 0 {
                            effects.push(mood(&name, (surprise / 4) as i64, reason));
                        }
                        "relieved"
                    } else {
                        e.state = "broken";
                        scored.push((e.about.clone(), false));
                        effects.push(mood(&name, -((surprise / 2).max(1) as i64), reason.clone()));
                        if e.relies {
                            effects.push(resent(&name, &e.about, (surprise / 2).max(1), reason));
                        }
                        "disappointed"
                    }
                } else if e.state == "open" && !e.dread {
                    e.state = "worried";
                    if surprise / 4 > 0 {
                        effects.push(mood(&name, -((surprise / 4) as i64), reason));
                    }
                    "worried"
                } else {
                    continue;
                };
                changes.push(obj! {
                    "about" => e.about.as_str(),
                    "being" => name.as_str(),
                    "event" => e.event.as_str(),
                    "id" => id.as_str(),
                    "outcome" => outcome,
                    "surprise" => surprise,
                });
            }
            for id in relieved {
                mind.expectations.remove(&id);
            }
            for (about, kept) in scored {
                mind.score(&about, kept, max_subjects);
            }
        }
        Ok(obj! { "at" => at, "changes" => Value::Array(changes), "effects" => Value::Array(effects) })
    }

    fn op_happen(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["about", "event", "at"], &["being"])?;
        let about = ident(c::get(args, "about")?)?.to_string();
        let event = ident(c::get(args, "event")?)?.to_string();
        let at = uint_bound(c::get(args, "at")?, 0, MAX_AT)?;
        let names = self.only(args)?;
        let max_subjects = self.limits.get("subjects");
        let (mut resolved, mut effects) = (Vec::new(), Vec::new());
        for name in names {
            let mind = self.beings.get_mut(&name).ok_or_else(c::bad)?;
            let hits: Vec<String> = mind
                .expectations
                .iter()
                .filter(|(_, e)| e.about == about && e.event == event)
                .map(|(k, _)| k.clone())
                .collect();
            for id in hits {
                let e = mind.expectations.remove(&id).ok_or_else(c::bad)?;
                let surprise = e.surprise();
                let reason = format!("expected {}", e.label);
                let outcome = if e.state == "broken" {
                    effects.push(mood(&name, (e.importance / 4).max(1) as i64, reason));
                    "late"
                } else if e.dread {
                    mind.score(&about, true, max_subjects);
                    effects.push(mood(&name, -((e.importance / 2).max(1) as i64), reason));
                    "dread_met"
                } else {
                    mind.score(&about, true, max_subjects);
                    effects.push(mood(&name, (e.importance / 10).max(1) as i64, reason));
                    "kept"
                };
                resolved.push(obj! {
                    "being" => name.as_str(),
                    "early" => at < e.due,
                    "id" => id,
                    "outcome" => outcome,
                    "surprise" => if outcome == "late" { surprise } else { 0 },
                });
            }
        }
        Ok(obj! {
            "about" => about,
            "at" => at,
            "effects" => Value::Array(effects),
            "event" => event,
            "resolved" => Value::Array(resolved),
        })
    }

    fn op_cancel(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "id"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let id = ident(c::get(args, "id")?)?;
        let removed = self.beings.get_mut(being).is_some_and(|m| m.expectations.remove(id).is_some());
        Ok(obj! { "being" => being, "id" => id, "removed" => removed })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let mind = self.beings.get(being).ok_or_else(c::bad)?;
        Ok(obj! {
            "being" => being,
            "expectations" => Value::Array(mind.expectations.iter().map(|(id, e)| e.view(id)).collect()),
            "reliability" => Value::Array(
                mind.reliability
                    .iter()
                    .map(|(a, (k, b))| obj! { "about" => a.as_str(), "broken" => *b, "confidence" => mind.confidence(a), "kept" => *k })
                    .collect(),
            ),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    ExpectationBook::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(b: &mut ExpectationBook, ops: &str) -> String {
        b.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16).unwrap()).unwrap().to_json()
    }

    const FLOUR: &str = r#"{"op":"expect","args":{"being":"baker","id":"flour","about":"player","event":"bring_flour","due":12,"grace":2,"importance":80,"relies":true}}"#;

    #[test]
    fn worry_then_break_then_late_relief() {
        let mut b = ExpectationBook::new();
        let out = run(
            &mut b,
            &format!(
                r#"{FLOUR},{{"op":"tick","args":{{"at":13}}}},{{"op":"tick","args":{{"at":14}}}},{{"op":"tick","args":{{"at":15}}}},
                   {{"op":"happen","args":{{"about":"player","event":"bring_flour","at":20}}}},{{"op":"assess","args":{{"being":"baker"}}}}"#
            ),
        );
        for o in ["worried", "disappointed", "late"] {
            assert!(out.contains(&format!(r#""outcome":"{o}""#)), "{o}: {out}");
        }
        // One broken promise: confidence 70 - 15 = 55.
        assert!(out.contains(r#""about":"player","broken":1,"confidence":55,"kept":0"#), "{out}");
    }

    #[test]
    fn dread_that_never_comes_is_relief() {
        let mut b = ExpectationBook::new();
        let out = run(
            &mut b,
            r#"{"op":"expect","args":{"being":"miller","id":"tax","about":"collector","event":"arrive","due":5,"importance":60,"kind":"dread"}},
               {"op":"tick","args":{"at":6}},{"op":"assess","args":{"being":"miller"}}"#,
        );
        assert!(out.contains(r#""outcome":"relieved""#) && out.contains(r#""expectations":[]"#), "{out}");
    }

    #[test]
    fn full_book_evicts_only_broken() {
        let mut b = ExpectationBook::new();
        let ex = |i: usize| {
            format!(
                r#"{{"op":"expect","args":{{"being":"b","id":"e{i}","about":"s","event":"x","due":{i},"importance":5}}}}"#
            )
        };
        let all: Vec<String> = (0..32).map(ex).collect();
        run(&mut b, &all.join(","));
        assert!(b.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, ex(40)), 4096).unwrap()).is_err());
        run(&mut b, r#"{"op":"tick","args":{"at":1}}"#);
        run(&mut b, &ex(40));
        assert!(!b.beings["b"].expectations.contains_key("e0"));
    }
}
