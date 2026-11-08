//! Reflection and lessons (pass 2, A6).
//!
//! A being records confirmed outcomes per **topic** — an `activity` with something or someone
//! (`about`): "dealing with the trader went badly". Over the most recent episodes of a topic
//! (at least 3), a two-thirds majority becomes a **lesson**: `avoid` (mostly bad) or `seek`
//! (mostly good). Lessons cite their episodes, can be wrong, and are revised by new outcomes:
//! each outcome reports whether the lesson was `learned`, `confirmed`, `contradicted`,
//! `revised` (verdict flipped) or `doubted` (no majority any more).
//!
//! `advice` turns lessons into a motive bias (±confidence/4, negative for `avoid`) and a
//! sentence the being can give for its change of heart. Capacities are
//! [host-set limits](crate::character::limits). Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::lesson::Lessons;
//!
//! let mut lessons = Lessons::new();
//! let deal = |e: &str, r: &str| format!(r#"{{"op": "outcome", "args": {{"being": "baker", "about": "trader", "activity": "deal", "result": "{r}", "episode": "{e}"}}}}"#);
//! let req = parse(&format!(r#"{{"ops": [{}, {}, {}, {}]}}"#, deal("e1", "bad"), deal("e2", "good"), deal("e3", "bad"), deal("e4", "bad")), 4096)?;
//! let out = lessons.workflow(&req)?;
//! assert!(out.to_json().contains("deal with trader went badly 3 of 4 times"));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, text_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "topics", default: 64, ceiling: 4096 },
    Limit { name: "episodes", default: 16, ceiling: 256 },
];
pub const MIN_EVIDENCE: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct Episode {
    pub id: String,
    pub at: u64,
    pub good: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Topic {
    pub episodes: Vec<Episode>,
    pub verdict: Option<&'static str>,
    pub last_at: u64,
}

impl Topic {
    fn counts(&self) -> (usize, usize) {
        let good = self.episodes.iter().filter(|e| e.good).count();
        (good, self.episodes.len() - good)
    }

    fn judge(&self) -> Option<&'static str> {
        let (good, bad) = self.counts();
        let total = good + bad;
        if total < MIN_EVIDENCE {
            None
        } else if bad * 3 >= total * 2 {
            Some("avoid")
        } else if good * 3 >= total * 2 {
            Some("seek")
        } else {
            None
        }
    }

    fn lesson(&self, about: &str, activity: &str) -> Value {
        let Some(verdict) = self.verdict else { return Value::Null };
        let (good, bad) = self.counts();
        let total = good + bad;
        let want_good = verdict == "seek";
        let majority = if want_good { good } else { bad };
        let cites: Vec<&str> = self.episodes.iter().filter(|e| e.good == want_good).map(|e| e.id.as_str()).collect();
        let how = if want_good { "well" } else { "badly" };
        obj! {
            "about" => about,
            "activity" => activity,
            "bad" => bad,
            "cites" => Value::strings(&cites),
            "confidence" => majority * 100 / total,
            "good" => good,
            "summary" => format!("{activity} with {about} went {how} {majority} of {total} times"),
            "verdict" => verdict,
        }
    }
}

/// Per-being topics: `(about, activity)` → episodes and the current lesson.
#[derive(Clone, Debug)]
pub struct Lessons {
    pub beings: BTreeMap<String, BTreeMap<(String, String), Topic>>,
    pub limits: Limits,
}

impl Default for Lessons {
    fn default() -> Self {
        Lessons { beings: BTreeMap::new(), limits: Limits::defaults(LIMITS) }
    }
}

impl Lessons {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (`{beings?, topics?, episodes?}`).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut x = Self::new();
        x.limits.apply(v)?;
        Ok(x)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "outcome" => self.op_outcome(args, l),
            "advice" => self.op_advice(args),
            "forget" => self.op_forget(args),
            "assess" => self.op_assess(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn op_outcome(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "about", "activity", "result", "episode"], &["at"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let about = ident(c::get(args, "about")?)?.to_string();
        let activity = ident(c::get(args, "activity")?)?.to_string();
        let good = match text_bound(c::get(args, "result")?, 8)? {
            "good" => true,
            "bad" => false,
            _ => return Err(c::bad()),
        };
        let episode = ident(c::get(args, "episode")?)?.to_string();
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        if !self.beings.contains_key(&being) && self.beings.len() >= l.get("beings") {
            return Err(c::bad());
        }
        let topics = self.beings.entry(being.clone()).or_default();
        let key = (about.clone(), activity.clone());
        if !topics.contains_key(&key) && topics.len() >= l.get("topics") {
            let victim = topics
                .iter()
                .min_by_key(|(k, t)| (t.episodes.len(), t.last_at, (*k).clone()))
                .map(|(k, _)| k.clone())
                .ok_or_else(c::bad)?;
            topics.remove(&victim);
        }
        let t = topics.entry(key).or_default();
        if t.episodes.iter().any(|e| e.id == episode) {
            return Err(c::bad());
        }
        t.episodes.push(Episode { id: episode, at, good });
        while t.episodes.len() > l.get("episodes") {
            t.episodes.remove(0);
        }
        t.last_at = t.last_at.max(at);
        let previous = t.verdict;
        t.verdict = t.judge();
        let change = match (previous, t.verdict) {
            (None, None) => "noted",
            (None, Some(_)) => "learned",
            (Some(_), None) => "doubted",
            (Some(p), Some(v)) if p != v => "revised",
            (Some(v), _) => {
                if (v == "seek") == good {
                    "confirmed"
                } else {
                    "contradicted"
                }
            }
        };
        Ok(obj! {
            "about" => about.as_str(),
            "activity" => activity.as_str(),
            "being" => being,
            "change" => change,
            "lesson" => t.lesson(&about, &activity),
            "previous" => previous.map(Value::str).unwrap_or(Value::Null),
        })
    }

    fn op_advice(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["about", "activity"])?;
        let being = ident(c::get(args, "being")?)?;
        let about = opt(args, "about").map(ident).transpose()?;
        let activity = opt(args, "activity").map(ident).transpose()?;
        let mut out = Vec::new();
        for ((ab, ac), t) in self.beings.get(being).into_iter().flatten() {
            if about.is_some_and(|x| x != ab) || activity.is_some_and(|x| x != ac) {
                continue;
            }
            let lesson = t.lesson(ab, ac);
            let Some(conf) = lesson.get("confidence").and_then(Value::as_usize) else { continue };
            let bias = if t.verdict == Some("avoid") { -((conf / 4) as i64) } else { (conf / 4) as i64 };
            out.push(obj! {
                "about" => ab.as_str(),
                "activity" => ac.as_str(),
                "bias" => bias,
                "summary" => lesson.get("summary").cloned().unwrap_or(Value::Null),
                "verdict" => t.verdict.map(Value::str).unwrap_or(Value::Null),
            });
        }
        Ok(obj! { "being" => being, "lessons" => Value::Array(out) })
    }

    fn op_forget(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "about", "activity"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let key = (ident(c::get(args, "about")?)?.to_string(), ident(c::get(args, "activity")?)?.to_string());
        let forgot = self.beings.get_mut(being).is_some_and(|t| t.remove(&key).is_some());
        Ok(obj! { "about" => key.0, "activity" => key.1, "being" => being, "forgot" => forgot })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let topics = self.beings.get(being).ok_or_else(c::bad)?;
        Ok(obj! {
            "being" => being,
            "topics" => Value::Array(topics.iter().map(|((ab, ac), t)| obj! {
                "about" => ab.as_str(),
                "activity" => ac.as_str(),
                "episodes" => Value::Array(t.episodes.iter().map(|e| obj! {
                    "at" => e.at, "episode" => e.id.as_str(), "result" => if e.good { "good" } else { "bad" },
                }).collect()),
                "lesson" => t.lesson(ab, ac),
            }).collect()),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Lessons::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    #[test]
    fn a_lesson_is_revised_or_doubted_as_the_window_moves() {
        let o = |i: usize, r: &str| {
            format!(
                r#"{{"op":"outcome","args":{{"being":"b","about":"t","activity":"deal","result":"{r}","episode":"e{i}"}}}}"#
            )
        };
        let mut ops: Vec<String> = (0..3).map(|i| o(i, "bad")).collect();
        ops.extend((3..6).map(|i| o(i, "good")));
        for (window, changes) in
            [(3, ["learned", "contradicted", "revised"]), (4, ["learned", "contradicted", "doubted"])]
        {
            let req = format!(r#"{{"limits":{{"episodes":{window}}},"ops":[{}]}}"#, ops.join(","));
            let out = Lessons::new().workflow(&parse(&req, 1 << 16).unwrap()).unwrap().to_json();
            for change in changes {
                assert!(out.contains(&format!(r#""change":"{change}""#)), "{window} {change}: {out}");
            }
        }
    }
}
