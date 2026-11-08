//! Behavior explanation trace (pass 2, D4): "why did she do that?"
//!
//! The host records one trace per decision. A trace links what the being perceived, what
//! it believed (with source and confidence), the motive that won arbitration (score, rank,
//! reason and runner-up), the task step it was serving, the action taken and the outcome
//! the host confirmed. `query_trace` renders each trace as an ordered `chain` of stages
//! and a single deterministic `why` sentence.
//!
//! Everything here is host-supplied and validated; nothing is inferred by a model. History
//! is a bounded ring per being ($\le 128$ traces, oldest evicted first) across at most 64
//! beings. Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::explain::ExplainTracer;
//!
//! let mut tracer = ExplainTracer::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {"op": "record_trace", "args": {
//!       "being": "baker", "action": "eat", "at": 15,
//!       "perceptions": [{"source": "body", "text": "stomach growling"}],
//!       "motive": {"id": "need_hunger", "kind": "need", "score": 80, "reason": "hunger 80 is past threshold 70",
//!                  "runner_up": {"id": "task_bake_bread", "score": 70}}
//!     }},
//!     {"op": "query_trace", "args": {"being": "baker", "limit": 1}}
//!   ]
//! }"#, 4096)?;
//! let out = tracer.workflow(&req)?;
//! let why = out.get("results").unwrap().as_array().unwrap()[1]
//!     .get("traces").unwrap().as_array().unwrap()[0]
//!     .get("why").unwrap().as_str().unwrap().to_string();
//! assert!(why.starts_with("baker did eat because hunger 80 is past threshold 70"));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_OPS: usize = 64;
pub const MAX_TRACES_PER_BEING: usize = 128;
pub const MAX_BEINGS: usize = 64;
pub const MAX_PERCEPTIONS: usize = 8;
pub const MAX_BELIEFS: usize = 8;
pub const MAX_AT: u64 = 1_000_000_000_000;

const MOTIVE_KINDS: [&str; 7] = ["emergency", "need", "promise", "routine", "task", "goal", "reaction"];
const BELIEF_SOURCES: [&str; 4] = ["seen", "told", "inferred", "authored"];
const RECORD_STATUSES: [&str; 4] = ["pending", "success", "failure", "interrupted"];
const CONFIRM_STATUSES: [&str; 3] = ["success", "failure", "interrupted"];

fn text_bound(v: &Value, max_len: usize) -> Result<&str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    if s.is_empty() || s.chars().count() > max_len {
        return Err(c::bad());
    }
    if s.chars().any(|ch| (ch as u32) < 32 || (127..=159).contains(&(ch as u32))) {
        return Err(c::bad());
    }
    Ok(s)
}

fn uint_bound(v: &Value, lo: u64, hi: u64) -> Result<u64> {
    match v {
        Value::Number(s) => {
            let n = s.parse::<u64>().map_err(|_| c::bad())?;
            if n < lo || n > hi {
                return Err(c::bad());
            }
            Ok(n)
        }
        _ => Err(c::bad()),
    }
}

fn opt<'a>(args: &'a Value, k: &str) -> Option<&'a Value> {
    args.get(k).filter(|x| **x != Value::Null)
}

fn one_of<'a>(v: &'a Value, allowed: &[&str]) -> Result<&'a str> {
    let s = text_bound(v, 32)?;
    if !allowed.contains(&s) {
        return Err(c::bad());
    }
    Ok(s)
}

fn list_bound(v: Option<&Value>, max: usize) -> Result<&[Value]> {
    match v {
        None => Ok(&[]),
        Some(v) => {
            let xs = v.as_array().ok_or_else(c::bad)?;
            if xs.len() > max {
                return Err(c::bad());
            }
            Ok(xs)
        }
    }
}

/// Host-set capacity limits: `(name, default, ceiling)`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "beings", default: 64, ceiling: 1000000 },
    crate::character::limits::Limit { name: "traces", default: 128, ceiling: 4096 },
];
crate::character::limits::module_limits!(ExplainLimits, LIMITS);

#[derive(Clone, Debug, PartialEq)]
pub struct Perception {
    pub source: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Belief {
    pub about: String,
    pub fact: String,
    pub source: String,
    pub confidence: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MotiveRef {
    pub id: String,
    pub kind: String,
    pub score: u64,
    pub rank: u64,
    pub reason: String,
    pub runner_up: Option<(String, u64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskStep {
    pub id: String,
    pub step: String,
    pub index: u64,
}

/// One recorded decision: perception → belief → motive → task step → action → outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub id: String,
    pub seq: u64,
    pub being: String,
    pub action: String,
    pub target: Option<String>,
    pub at: u64,
    pub perceptions: Vec<Perception>,
    pub beliefs: Vec<Belief>,
    pub motive: MotiveRef,
    pub task: Option<TaskStep>,
    pub status: String,
    pub summary: Option<String>,
}

fn opt_str(s: &Option<String>) -> Value {
    match s {
        Some(x) => Value::str(x),
        None => Value::Null,
    }
}

impl Trace {
    /// The ordered explanation chain, one entry per stage.
    pub fn chain(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let stage = |stage: &str, detail: String| obj! { "detail" => detail, "stage" => stage };
        for p in &self.perceptions {
            out.push(stage("perception", format!("{}: {}", p.source, p.text)));
        }
        for b in &self.beliefs {
            out.push(stage("belief", format!("{}: {} ({}, {}%)", b.about, b.fact, b.source, b.confidence)));
        }
        let m = &self.motive;
        let mut md = format!("{} [{}] score {} rank {}: {}", m.id, m.kind, m.score, m.rank, m.reason);
        if let Some((rid, rs)) = &m.runner_up {
            md.push_str(&format!(" (over {rid} at {rs})"));
        }
        out.push(stage("motive", md));
        if let Some(t) = &self.task {
            out.push(stage("task", format!("{} step {}: {}", t.id, t.index, t.step)));
        }
        let mut ad = self.action.clone();
        if let Some(t) = &self.target {
            ad.push_str(&format!(" -> {t}"));
        }
        out.push(stage("action", ad));
        let mut od = self.status.clone();
        if let Some(s) = &self.summary {
            od.push_str(&format!(": {s}"));
        }
        out.push(stage("outcome", od));
        out
    }

    /// A single deterministic sentence answering "why did this being do that?".
    pub fn why(&self) -> String {
        let m = &self.motive;
        let mut s = format!("{} did {}", self.being, self.action);
        if let Some(t) = &self.target {
            s.push_str(&format!(" to {t}"));
        }
        s.push_str(&format!(" because {} ({} motive {}, score {}", m.reason, m.kind, m.id, m.score));
        if let Some((rid, rs)) = &m.runner_up {
            s.push_str(&format!(", beating {rid} at {rs}"));
        }
        s.push_str(").");
        if !self.perceptions.is_empty() {
            let xs: Vec<&str> = self.perceptions.iter().map(|p| p.text.as_str()).collect();
            s.push_str(&format!(" Triggered by: {}.", xs.join("; ")));
        }
        if !self.beliefs.is_empty() {
            let xs: Vec<&str> = self.beliefs.iter().map(|b| b.fact.as_str()).collect();
            s.push_str(&format!(" Believed: {}.", xs.join("; ")));
        }
        if let Some(t) = &self.task {
            s.push_str(&format!(" Step {} of task {}.", t.index, t.id));
        }
        s.push_str(&format!(" Outcome: {}", self.status));
        if let Some(x) = &self.summary {
            s.push_str(&format!(" ({x})"));
        }
        s.push('.');
        s
    }

    pub fn to_value(&self) -> Value {
        let m = &self.motive;
        let runner_up = match &m.runner_up {
            Some((id, score)) => obj! { "id" => id.as_str(), "score" => *score },
            None => Value::Null,
        };
        let task = match &self.task {
            Some(t) => obj! { "id" => t.id.as_str(), "index" => t.index, "step" => t.step.as_str() },
            None => Value::Null,
        };
        obj! {
            "action" => self.action.as_str(),
            "at" => self.at,
            "being" => self.being.as_str(),
            "beliefs" => Value::Array(self.beliefs.iter().map(|b| obj! {
                "about" => b.about.as_str(),
                "confidence" => b.confidence,
                "fact" => b.fact.as_str(),
                "source" => b.source.as_str(),
            }).collect()),
            "chain" => Value::Array(self.chain()),
            "id" => self.id.as_str(),
            "motive" => obj! {
                "id" => m.id.as_str(),
                "kind" => m.kind.as_str(),
                "rank" => m.rank,
                "reason" => m.reason.as_str(),
                "runner_up" => runner_up,
                "score" => m.score,
            },
            "outcome" => obj! { "status" => self.status.as_str(), "summary" => opt_str(&self.summary) },
            "perceptions" => Value::Array(self.perceptions.iter().map(|p| obj! {
                "source" => p.source.as_str(),
                "text" => p.text.as_str(),
            }).collect()),
            "seq" => self.seq,
            "target" => opt_str(&self.target),
            "task" => task,
            "why" => self.why(),
        }
    }
}

/// Bounded per-being decision history with deterministic explanations.
#[derive(Clone, Debug, Default)]
pub struct ExplainTracer {
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: ExplainLimits,
    pub next_seq: u64,
    pub beings: BTreeMap<String, VecDeque<Trace>>,
}

impl ExplainTracer {
    pub fn new() -> Self {
        Self { next_seq: 1, beings: BTreeMap::new(), limits: ExplainLimits::default() }
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &[])?;
        let ops = request.get("ops").and_then(Value::as_array).ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }
        let empty = Value::Object(Vec::new());
        let mut results = Vec::new();
        for op_val in ops {
            keys(op_val, &["op"], &["args"])?;
            let op = text_bound(c::get(op_val, "op")?, 32)?;
            let args = opt(op_val, "args").unwrap_or(&empty);
            let res = match op {
                "init" => self.op_init(args)?,
                "record_trace" => self.op_record(args)?,
                "confirm_outcome" => self.op_confirm(args)?,
                "query_trace" => self.op_query(args)?,
                _ => return Err(c::bad()),
            };
            results.push(res);
        }
        Ok(obj! {
            "version" => 1u64,
            "results" => Value::Array(results),
        })
    }

    fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being"])?;
        let cleared = match opt(args, "being").map(ident).transpose()? {
            Some(b) => self.beings.remove(b).map(|h| h.len()).unwrap_or(0),
            None => {
                let n = self.beings.values().map(VecDeque::len).sum();
                self.beings.clear();
                n
            }
        };
        Ok(obj! { "cleared" => cleared, "ok" => true })
    }

    fn op_record(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "action", "motive"], &["target", "at", "perceptions", "beliefs", "task", "outcome"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let action = ident(c::get(args, "action")?)?.to_string();
        let target = opt(args, "target").map(ident).transpose()?.map(str::to_string);
        let at = opt(args, "at").map(|v| uint_bound(v, 0, MAX_AT)).transpose()?.unwrap_or(0);

        let mut perceptions = Vec::new();
        for p in list_bound(opt(args, "perceptions"), MAX_PERCEPTIONS)? {
            keys(p, &["source", "text"], &[])?;
            perceptions.push(Perception {
                source: ident(c::get(p, "source")?)?.to_string(),
                text: text_bound(c::get(p, "text")?, 160)?.to_string(),
            });
        }

        let mut beliefs = Vec::new();
        for b in list_bound(opt(args, "beliefs"), MAX_BELIEFS)? {
            keys(b, &["about", "fact", "source"], &["confidence"])?;
            beliefs.push(Belief {
                about: ident(c::get(b, "about")?)?.to_string(),
                fact: text_bound(c::get(b, "fact")?, 160)?.to_string(),
                source: one_of(c::get(b, "source")?, &BELIEF_SOURCES)?.to_string(),
                confidence: opt(b, "confidence").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(100),
            });
        }

        let mv = c::get(args, "motive")?;
        keys(mv, &["id", "kind", "score", "reason"], &["rank", "runner_up"])?;
        let runner_up = match opt(mv, "runner_up") {
            Some(r) => {
                keys(r, &["id", "score"], &[])?;
                Some((ident(c::get(r, "id")?)?.to_string(), uint_bound(c::get(r, "score")?, 0, 100)?))
            }
            None => None,
        };
        let motive = MotiveRef {
            id: ident(c::get(mv, "id")?)?.to_string(),
            kind: one_of(c::get(mv, "kind")?, &MOTIVE_KINDS)?.to_string(),
            score: uint_bound(c::get(mv, "score")?, 0, 100)?,
            rank: opt(mv, "rank").map(|v| uint_bound(v, 1, 64)).transpose()?.unwrap_or(1),
            reason: text_bound(c::get(mv, "reason")?, 200)?.to_string(),
            runner_up,
        };

        let task = match opt(args, "task") {
            Some(t) => {
                keys(t, &["id", "step", "index"], &[])?;
                Some(TaskStep {
                    id: ident(c::get(t, "id")?)?.to_string(),
                    step: ident(c::get(t, "step")?)?.to_string(),
                    index: uint_bound(c::get(t, "index")?, 0, 63)?,
                })
            }
            None => None,
        };

        let (status, summary) = match opt(args, "outcome") {
            Some(o) => {
                keys(o, &["status"], &["summary"])?;
                (
                    one_of(c::get(o, "status")?, &RECORD_STATUSES)?.to_string(),
                    opt(o, "summary").map(|v| text_bound(v, 200)).transpose()?.map(str::to_string),
                )
            }
            None => ("pending".to_string(), None),
        };

        if !self.beings.contains_key(&being) && self.beings.len() >= self.limits.get("beings") {
            return Err(c::bad());
        }

        let seq = self.next_seq;
        self.next_seq += 1;
        let id = format!("tr_{seq}");
        let trace = Trace {
            id: id.clone(),
            seq,
            being: being.clone(),
            action,
            target,
            at,
            perceptions,
            beliefs,
            motive,
            task,
            status,
            summary,
        };
        let max_traces = self.limits.get("traces");
        let history = self.beings.entry(being.clone()).or_default();
        let mut evicted = 0u64;
        if history.len() >= max_traces {
            history.pop_front();
            evicted = 1;
        }
        history.push_back(trace);
        Ok(obj! {
            "being" => being,
            "evicted" => evicted,
            "id" => id,
            "ok" => true,
            "stored" => history.len(),
        })
    }

    fn op_confirm(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "id", "status"], &["summary"])?;
        let being = ident(c::get(args, "being")?)?;
        let id = ident(c::get(args, "id")?)?;
        let status = one_of(c::get(args, "status")?, &CONFIRM_STATUSES)?.to_string();
        let summary = opt(args, "summary").map(|v| text_bound(v, 200)).transpose()?.map(str::to_string);
        let history = self.beings.get_mut(being).ok_or_else(c::bad)?;
        let trace = history.iter_mut().find(|t| t.id == id).ok_or_else(c::bad)?;
        if trace.status != "pending" {
            return Err(c::bad());
        }
        trace.status = status.clone();
        trace.summary = summary;
        Ok(obj! {
            "being" => being,
            "id" => id,
            "ok" => true,
            "status" => status,
        })
    }

    fn op_query(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["id", "action", "limit"])?;
        let being = ident(c::get(args, "being")?)?;
        let id = opt(args, "id").map(ident).transpose()?;
        let action = opt(args, "action").map(ident).transpose()?;
        let limit =
            opt(args, "limit").map(|v| uint_bound(v, 1, self.limits.get("traces") as u64)).transpose()?.unwrap_or(8);
        let history = self.beings.get(being).ok_or_else(c::bad)?;
        let traces: Vec<Value> = history
            .iter()
            .rev()
            .filter(|t| id.is_none_or(|x| t.id == x) && action.is_none_or(|x| t.action == x))
            .take(limit as usize)
            .map(Trace::to_value)
            .collect();
        Ok(obj! {
            "being" => being,
            "count" => traces.len(),
            "total" => history.len(),
            "traces" => Value::Array(traces),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    ExplainTracer::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(t: &mut ExplainTracer, s: &str) -> Result<Value> {
        t.workflow(&parse(s, 1 << 20).unwrap())
    }

    const REC: &str = r#"{"op":"record_trace","args":{"being":"baker","action":"bake","motive":{"id":"routine_bake","kind":"routine","score":50,"reason":"bake shift is active"}}}"#;

    #[test]
    fn ring_evicts_oldest_at_capacity() {
        let mut t = ExplainTracer::new();
        for _ in 0..MAX_TRACES_PER_BEING + 1 {
            run(&mut t, &format!(r#"{{"ops":[{REC}]}}"#)).unwrap();
        }
        let h = &t.beings["baker"];
        assert_eq!(h.len(), MAX_TRACES_PER_BEING);
        assert_eq!(h.front().unwrap().seq, 2);
    }

    #[test]
    fn confirm_only_moves_pending_traces() {
        let mut t = ExplainTracer::new();
        run(&mut t, &format!(r#"{{"ops":[{REC}]}}"#)).unwrap();
        let ok = r#"{"ops":[{"op":"confirm_outcome","args":{"being":"baker","id":"tr_1","status":"success","summary":"two loaves"}}]}"#;
        run(&mut t, ok).unwrap();
        assert!(run(&mut t, ok).is_err());
        let q = run(&mut t, r#"{"ops":[{"op":"query_trace","args":{"being":"baker"}}]}"#).unwrap();
        assert!(q.to_json().contains("Outcome: success (two loaves)."));
    }

    #[test]
    fn rejects_unknown_kind_and_65th_being() {
        let mut t = ExplainTracer::new();
        let bad = REC.replace("\"routine\"", "\"whim\"");
        assert!(run(&mut t, &format!(r#"{{"ops":[{bad}]}}"#)).is_err());
        for i in 0..MAX_BEINGS {
            let r = REC.replace("\"baker\"", &format!("\"b{i}\""));
            run(&mut t, &format!(r#"{{"ops":[{r}]}}"#)).unwrap();
        }
        assert!(run(&mut t, &format!(r#"{{"ops":[{REC}]}}"#)).is_err());
    }
}
