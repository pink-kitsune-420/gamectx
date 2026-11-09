//! Subjective world knowledge (pass 2, A1): what a being believes about the world.
//!
//! The planner and action catalog describe the world as the host knows it. A being only
//! believes parts of it, each with a source: it `did` something, `saw` it, was `told`, or
//! the author gave it (`authored`). A belief is a triple `about relation value` that the
//! being holds true or false, e.g. `mill sells flour` or `baker can bake`.
//!
//! - **Learning:** matching evidence reinforces (confidence becomes the maximum, the stronger
//!   source wins). Contradicting evidence revises the belief only when its source ranks at
//!   least as high (`did` > `saw` > `told` > `authored`); weaker contradictions are kept out.
//! - **Doing corrects:** `outcome` records `<being> can <action>` after a success, and on a
//!   failure marks the beliefs the attempt relied on as false (source `did`).
//! - **Planning against belief:** `known_actions` splits catalog actions into known and
//!   unknown; `gaps` reports needed beliefs that are unknown or below a confidence floor,
//!   which is what makes asking (see [`crate::character::speech_act`]) worth doing.
//!
//! Bounded: 128 beliefs per being (lowest confidence, then oldest, then key evicted), 64
//! beings. Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::beliefs::BeliefStore;
//!
//! let mut store = BeliefStore::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "learn", "args": {"being": "baker", "about": "mill", "relation": "sells", "value": "flour",
//!                            "source": "told", "from": "carter"}},
//!   {"op": "outcome", "args": {"being": "baker", "action": "barter", "success": false,
//!                              "relied_on": [{"about": "mill", "relation": "sells", "value": "flour"}]}}
//! ]}"#, 4096)?;
//! let out = store.workflow(&req)?;
//! let fixed = &out.get("results").unwrap().as_array().unwrap()[1].get("updates").unwrap().as_array().unwrap()[0];
//! assert_eq!(fixed.get("result").and_then(|v| v.as_str()), Some("revised"));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::data::persist::{self, durable};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const MAX_OPS: usize = 64;
pub const MAX_BELIEFS: usize = 128;
pub const MAX_BEINGS: usize = 64;
pub const MAX_LIST: usize = 64;
pub const MAX_NEEDS: usize = 16;
pub const MAX_RELIED: usize = 8;
pub const MAX_KEY: usize = 64;
pub const MAX_AT: u64 = 1_000_000_000_000;
pub const SOURCES: [&str; 4] = ["authored", "told", "saw", "did"];

/// Rank of a source: `authored` 1 < `told` 2 < `saw` 3 < `did` 4.
pub fn source_rank(s: &str) -> u64 {
    SOURCES.iter().position(|x| *x == s).map(|i| i as u64 + 1).unwrap_or(0)
}

/// Confidence used when the host gives none.
pub fn default_confidence(s: &str) -> u64 {
    match s {
        "did" => 100,
        "saw" => 90,
        "authored" => 80,
        _ => 60,
    }
}

pub(crate) fn text_bound(v: &Value, max_len: usize) -> Result<&str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    if s.is_empty() || s.chars().count() > max_len {
        return Err(c::bad());
    }
    if s.chars().any(|ch| (ch as u32) < 32 || (127..=159).contains(&(ch as u32))) {
        return Err(c::bad());
    }
    Ok(s)
}

pub(crate) fn uint_bound(v: &Value, lo: u64, hi: u64) -> Result<u64> {
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

pub(crate) fn opt<'a>(args: &'a Value, k: &str) -> Option<&'a Value> {
    args.get(k).filter(|x| **x != Value::Null)
}

pub(crate) fn opt_uint(args: &Value, k: &str, lo: u64, hi: u64, default: u64) -> Result<u64> {
    Ok(opt(args, k).map(|v| uint_bound(v, lo, hi)).transpose()?.unwrap_or(default))
}

pub(crate) fn opt_bool(args: &Value, k: &str, default: bool) -> Result<bool> {
    match opt(args, k) {
        None => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(c::bad()),
    }
}

pub(crate) fn list_bound(v: Option<&Value>, max: usize) -> Result<&[Value]> {
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

/// `about relation value`, each an identifier, joined by `:` in at most 64 characters.
/// Host-set capacity limits: `(name, default, ceiling)`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "beings", default: 64, ceiling: 1000000 },
    crate::character::limits::Limit { name: "beliefs", default: 128, ceiling: 4096 },
];
crate::character::limits::module_limits!(BeliefLimits, LIMITS);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Triple {
    pub about: String,
    pub relation: String,
    pub value: String,
}

impl Triple {
    pub fn new(about: &str, relation: &str, value: &str) -> Result<Self> {
        let t = Triple { about: about.to_string(), relation: relation.to_string(), value: value.to_string() };
        if t.key().len() > MAX_KEY {
            return Err(c::bad());
        }
        Ok(t)
    }

    /// Reads `about`, `relation` and `value` from an object that may carry other fields.
    pub fn read(v: &Value) -> Result<Self> {
        Self::new(ident(c::get(v, "about")?)?, ident(c::get(v, "relation")?)?, ident(c::get(v, "value")?)?)
    }

    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.about, self.relation, self.value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Belief {
    pub triple: Triple,
    pub holds: bool,
    pub source: String,
    pub confidence: u64,
    pub from: Option<String>,
    pub at: u64,
}

impl Belief {
    pub fn to_value(&self) -> Value {
        obj! {
            "about" => self.triple.about.as_str(),
            "at" => self.at,
            "confidence" => self.confidence,
            "from" => self.from.as_deref().map(Value::str).unwrap_or(Value::Null),
            "holds" => self.holds,
            "key" => self.triple.key(),
            "relation" => self.triple.relation.as_str(),
            "source" => self.source.as_str(),
            "value" => self.triple.value.as_str(),
        }
    }
}

/// Per-being sourced beliefs.
#[derive(Clone, Debug, Default)]
pub struct BeliefStore {
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: BeliefLimits,
    pub beings: BTreeMap<String, BTreeMap<Triple, Belief>>,
}

impl BeliefStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, being: &str, t: &Triple) -> Option<&Belief> {
        self.beings.get(being).and_then(|m| m.get(t))
    }

    fn ensure_room(&self, being: &str) -> Result<()> {
        if !self.beings.contains_key(being) && self.beings.len() >= self.limits.get("beings") {
            return Err(c::bad());
        }
        Ok(())
    }

    /// Merges one piece of evidence; returns `new`, `reinforced`, `revised` or `kept`
    /// and the belief as it now stands.
    pub fn learn(&mut self, being: &str, incoming: Belief) -> Result<(&'static str, Belief)> {
        self.ensure_room(being)?;
        let max_beliefs = self.limits.get("beliefs");
        let beliefs = self.beings.entry(being.to_string()).or_default();
        let key = incoming.triple.clone();
        let result = match beliefs.get_mut(&key) {
            None => {
                if beliefs.len() >= max_beliefs {
                    let victim = beliefs
                        .values()
                        .min_by(|a, b| {
                            a.confidence.cmp(&b.confidence).then(a.at.cmp(&b.at)).then(a.triple.cmp(&b.triple))
                        })
                        .map(|b| b.triple.clone());
                    if let Some(v) = victim {
                        beliefs.remove(&v);
                    }
                }
                beliefs.insert(key.clone(), incoming);
                "new"
            }
            Some(old) if old.holds == incoming.holds => {
                old.confidence = old.confidence.max(incoming.confidence);
                if source_rank(&incoming.source) > source_rank(&old.source) {
                    old.source = incoming.source;
                    old.from = incoming.from;
                }
                old.at = old.at.max(incoming.at);
                "reinforced"
            }
            Some(old) if source_rank(&incoming.source) >= source_rank(&old.source) => {
                *old = incoming;
                "revised"
            }
            Some(_) => "kept",
        };
        Ok((result, beliefs[&key].clone()))
    }

    fn learn_value(&mut self, being: &str, incoming: Belief) -> Result<Value> {
        let key = incoming.triple.key();
        let (result, now) = self.learn(being, incoming)?;
        Ok(obj! { "belief" => now.to_value(), "key" => key, "result" => result })
    }

    /// Runs one belief op; `None` when the op name is not a belief op.
    pub fn dispatch(&mut self, op: &str, args: &Value) -> Result<Option<Value>> {
        Ok(Some(match op {
            "init" => self.op_init(args)?,
            "learn" => self.op_learn(args)?,
            "outcome" => self.op_outcome(args)?,
            "query" => self.op_query(args)?,
            "known_actions" => self.op_known_actions(args)?,
            "gaps" => self.op_gaps(args)?,
            "export" => persist::export_op(self, &self.limits, args)?,
            "import" => persist::import(self, &self.limits.clone(), args)?,
            _ => return Ok(None),
        }))
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        run_ops(request, |op, args| self.dispatch(op, args)?.ok_or_else(c::bad))
    }

    fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["beliefs"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let mut fresh = Vec::new();
        for b in list_bound(opt(args, "beliefs"), MAX_LIST)? {
            keys(b, &["about", "relation", "value"], &["holds", "source", "confidence"])?;
            fresh.push(read_belief(b, "authored", None, 0)?);
        }
        self.ensure_room(&being)?;
        self.beings.insert(being.clone(), BTreeMap::new());
        for b in fresh {
            self.learn(&being, b)?;
        }
        Ok(obj! { "being" => being.as_str(), "count" => self.beings[&being].len() })
    }

    fn op_learn(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "about", "relation", "value", "source"], &["holds", "confidence", "from", "at"])?;
        let being = ident(c::get(args, "being")?)?;
        let from = opt(args, "from").map(ident).transpose()?.map(str::to_string);
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let b = read_belief(args, "", from, at)?;
        let mut out = self.learn_value(being, b)?;
        if let Value::Object(f) = &mut out {
            f.insert(0, ("being".to_string(), Value::str(being)));
        }
        Ok(out)
    }

    fn op_outcome(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "action", "success"], &["relied_on", "at"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let action = ident(c::get(args, "action")?)?.to_string();
        let success = match c::get(args, "success")? {
            Value::Bool(b) => *b,
            _ => return Err(c::bad()),
        };
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let mut relied = Vec::new();
        for r in list_bound(opt(args, "relied_on"), MAX_RELIED)? {
            keys(r, &["about", "relation", "value"], &[])?;
            relied.push(Triple::read(r)?);
        }
        let can = Triple::new(&being, "can", &action)?;
        let did = |triple: Triple, holds: bool| Belief {
            triple,
            holds,
            source: "did".to_string(),
            confidence: 100,
            from: None,
            at,
        };
        let mut updates = Vec::new();
        if success {
            updates.push(self.learn_value(&being, did(can, true))?);
        }
        for t in relied {
            updates.push(self.learn_value(&being, did(t, success))?);
        }
        Ok(obj! {
            "action" => action,
            "being" => being,
            "success" => success,
            "updates" => Value::Array(updates),
        })
    }

    fn op_query(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["about", "relation", "value", "min_confidence"])?;
        let being = ident(c::get(args, "being")?)?;
        let about = opt(args, "about").map(ident).transpose()?;
        let relation = opt(args, "relation").map(ident).transpose()?;
        let value = opt(args, "value").map(ident).transpose()?;
        let floor = opt_uint(args, "min_confidence", 0, 100, 0)?;
        let out: Vec<Value> = self
            .beings
            .get(being)
            .into_iter()
            .flat_map(|m| m.values())
            .filter(|b| about.is_none_or(|x| b.triple.about == x))
            .filter(|b| relation.is_none_or(|x| b.triple.relation == x))
            .filter(|b| value.is_none_or(|x| b.triple.value == x))
            .filter(|b| b.confidence >= floor)
            .map(Belief::to_value)
            .collect();
        Ok(obj! { "being" => being, "beliefs" => Value::Array(out.clone()), "count" => out.len() })
    }

    fn op_known_actions(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "actions"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let (mut known, mut unknown) = (Vec::new(), Vec::new());
        for a in list_bound(Some(c::get(args, "actions")?), MAX_LIST)? {
            let a = ident(a)?;
            let t = Triple::new(being, "can", a)?;
            if self.get(being, &t).is_some_and(|b| b.holds) {
                known.push(Value::str(a));
            } else {
                unknown.push(Value::str(a));
            }
        }
        Ok(obj! { "being" => being, "known" => Value::Array(known), "unknown" => Value::Array(unknown) })
    }

    fn op_gaps(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "needs"], &["min_confidence"])?;
        let being = ident(c::get(args, "being")?)?;
        let floor = opt_uint(args, "min_confidence", 0, 100, 50)?;
        let mut needs = Vec::new();
        let mut gaps = 0u64;
        for n in list_bound(Some(c::get(args, "needs")?), MAX_NEEDS)? {
            keys(n, &["about", "relation", "value"], &[])?;
            let t = Triple::read(n)?;
            let (status, confidence) = match self.get(being, &t) {
                None => ("unknown", Value::Null),
                Some(b) if b.confidence < floor => ("low_confidence", Value::from(b.confidence)),
                Some(b) if b.holds => ("known_true", Value::from(b.confidence)),
                Some(b) => ("known_false", Value::from(b.confidence)),
            };
            if status == "unknown" || status == "low_confidence" {
                gaps += 1;
            }
            needs.push(obj! { "confidence" => confidence, "key" => t.key(), "status" => status });
        }
        Ok(obj! { "being" => being, "gaps" => gaps, "needs" => Value::Array(needs) })
    }
}

/// Reads `{about, relation, value, holds?, source?, confidence?}`. An empty `default_source`
/// makes `source` required.
fn read_belief(v: &Value, default_source: &str, from: Option<String>, at: u64) -> Result<Belief> {
    let triple = Triple::read(v)?;
    let source = match opt(v, "source") {
        Some(s) => text_bound(s, 16)?,
        None if !default_source.is_empty() => default_source,
        None => return Err(c::bad()),
    };
    if source_rank(source) == 0 {
        return Err(c::bad());
    }
    Ok(Belief {
        triple,
        holds: opt_bool(v, "holds", true)?,
        source: source.to_string(),
        confidence: opt_uint(v, "confidence", 0, 100, default_confidence(source))?,
        from,
        at,
    })
}

/// Shared `{ops: 1–64 [{op, args?}]}` driver.
pub(crate) fn run_ops(request: &Value, mut f: impl FnMut(&str, &Value) -> Result<Value>) -> Result<Value> {
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
        results.push(f(op, opt(op_val, "args").unwrap_or(&empty))?);
    }
    Ok(obj! { "version" => 1u64, "results" => Value::Array(results) })
}

/// Beliefs save flat: the triple's three fields beside the belief's own.
impl persist::Persist for Belief {
    fn save(&self) -> Value {
        persist::object(vec![
            ("about", self.triple.about.save()),
            ("at", self.at.save()),
            ("confidence", self.confidence.save()),
            ("from", self.from.save()),
            ("holds", self.holds.save()),
            ("relation", self.triple.relation.save()),
            ("source", self.source.save()),
            ("value", self.triple.value.save()),
        ])
    }

    fn load(v: &Value) -> Result<Self> {
        c::fields(v, &["about", "at", "confidence", "from", "holds", "relation", "source", "value"])?;
        let s = |k: &str| -> Result<String> { persist::Persist::load(c::get(v, k)?) };
        Ok(Belief {
            triple: Triple { about: s("about")?, relation: s("relation")?, value: s("value")? },
            holds: persist::Persist::load(c::get(v, "holds")?)?,
            source: s("source")?,
            confidence: persist::ranged(c::get(v, "confidence")?, 0, 100)?,
            from: persist::Persist::load(c::get(v, "from")?)?,
            at: persist::ranged(c::get(v, "at")?, 0, MAX_AT as i128)?,
        })
    }
}

/// A store saves each being's beliefs as a list ascending by triple.
impl persist::Persist for BeliefStore {
    fn save(&self) -> Value {
        let rows = self.beings.iter().map(|(b, m)| {
            Value::Array(vec![Value::from(b.as_str()), Value::Array(m.values().map(|x| x.save()).collect())])
        });
        persist::object(vec![("beings", Value::Array(rows.collect()))])
    }

    fn load(v: &Value) -> Result<Self> {
        c::fields(v, &["beings"])?;
        let mut beings = BTreeMap::new();
        for row in c::list(c::get(v, "beings")?)? {
            let [b, xs] = c::list(row)? else { return Err(c::bad()) };
            let list: Vec<Belief> = persist::Persist::load(xs)?;
            beings.insert(persist::Persist::load(b)?, list.into_iter().map(|x| (x.triple.clone(), x)).collect());
        }
        Ok(BeliefStore { beings, ..Default::default() })
    }
}
durable!(BeliefStore, "beliefs", LIMITS);

/// Checks every belief map (shared with `speech_act`, whose limits carry the same names).
pub(crate) fn check_beliefs(beings: &BTreeMap<String, BTreeMap<Triple, Belief>>) -> Result<()> {
    let ceiling = |n: &str| LIMITS.iter().find(|l| l.name == n).map_or(0, |l| l.ceiling);
    persist::need(beings.len() <= ceiling("beings"))?;
    for (b, m) in beings {
        persist::need(persist::is_ident(b) && m.len() <= ceiling("beliefs"))?;
        for (t, x) in m {
            persist::need(*t == x.triple && Triple::new(&t.about, &t.relation, &t.value).is_ok())?;
            persist::need(
                persist::is_ident(&t.about) && persist::is_ident(&t.relation) && persist::is_ident(&t.value),
            )?;
            persist::need(source_rank(&x.source) > 0 && x.from.as_deref().is_none_or(persist::is_ident))?;
        }
    }
    Ok(())
}

impl BeliefStore {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        check_beliefs(&self.beings)
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    BeliefStore::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn learn(store: &mut BeliefStore, holds: bool, source: &str) -> &'static str {
        let b = Belief {
            triple: Triple::new("mill", "sells", "flour").unwrap(),
            holds,
            source: source.to_string(),
            confidence: default_confidence(source),
            from: None,
            at: 0,
        };
        store.learn("baker", b).unwrap().0
    }

    #[test]
    fn weaker_contradiction_is_kept_out() {
        let mut s = BeliefStore::new();
        assert_eq!(learn(&mut s, true, "saw"), "new");
        assert_eq!(learn(&mut s, false, "told"), "kept");
        assert_eq!(learn(&mut s, true, "told"), "reinforced");
        assert_eq!(learn(&mut s, false, "did"), "revised");
    }

    #[test]
    fn eviction_drops_lowest_confidence() {
        let mut s = BeliefStore::new();
        for i in 0..MAX_BELIEFS + 1 {
            let conf = if i == 5 { 1 } else { 50 };
            let b = Belief {
                triple: Triple::new("x", "r", &format!("v{i}")).unwrap(),
                holds: true,
                source: "told".into(),
                confidence: conf,
                from: None,
                at: i as u64,
            };
            s.learn("baker", b).unwrap();
        }
        let m = &s.beings["baker"];
        assert_eq!(m.len(), MAX_BELIEFS);
        assert!(!m.contains_key(&Triple::new("x", "r", "v5").unwrap()));
    }

    #[test]
    fn long_keys_are_refused() {
        let long = "a".repeat(40);
        let req = format!(
            r#"{{"ops":[{{"op":"learn","args":{{"being":"b","about":"{long}","relation":"r","value":"{long}","source":"did"}}}}]}}"#
        );
        assert!(BeliefStore::new().workflow(&parse(&req, 4096).unwrap()).is_err());
    }
}
