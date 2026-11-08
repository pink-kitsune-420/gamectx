//! Theory of mind: per-other beliefs (knows, wants, feels) with provenance
//! (seen, told, inferred) and witness-based knowledge inference.
//!
//! Enables beings to model other minds: "she did not see the theft, so she does not know"
//! or "he wants the key, so he may lie".
//! Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::mind::MindManager;
//!
//! let mut mind = MindManager::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "record",
//!       "args": {
//!         "subject": "baker",
//!         "other": "apprentice",
//!         "category": "knows",
//!         "fact": "cart_crash",
//!         "status": false,
//!         "source": "inferred",
//!         "at": 10
//!       }
//!     }
//!   ]
//! }"#, 4096)?;
//! let out = mind.workflow(&req)?;
//! assert_eq!(out.get("version").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::affect::ATTITUDE_KINDS;
use crate::character::life::witness::can_perceive;
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const MAX_BEINGS: usize = 64;
pub const MAX_BELIEFS: usize = 64;
pub const MAX_OPS: usize = 64;

/// Host-set limits: `beings` (subjects with a mind), `others` modelled per being, `beliefs` per
/// other and category (knows, wants, feels), and `witnesses` per `infer_from_witness` call.
/// Every path that adds an entry (pre-declared `beings`, `init`, `record`, `infer_from_witness`)
/// respects them.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "beings", default: MAX_BEINGS, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "others", default: 64, ceiling: 4096 },
    crate::character::limits::Limit { name: "beliefs", default: MAX_BELIEFS, ceiling: 4096 },
    crate::character::limits::Limit { name: "witnesses", default: MAX_BEINGS, ceiling: 4096 },
];
crate::character::limits::module_limits!(MindLimits, LIMITS);

fn room<K: Ord + ?Sized, V>(map: &BTreeMap<String, V>, key: &K, max: usize) -> Result<()>
where
    String: std::borrow::Borrow<K>,
{
    if map.len() >= max && !map.contains_key(key) {
        return Err(c::bad());
    }
    Ok(())
}

pub const PROVENANCE_SOURCES: [&str; 3] = ["seen", "told", "inferred"];

fn ident_bound(v: &Value) -> Result<&str> {
    let s = ident(v)?;
    if s.is_empty() || s.len() > 64 {
        return Err(c::bad());
    }
    Ok(s)
}

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

fn i64_bound(v: &Value, lo: i64, hi: i64) -> Result<i64> {
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

fn bool_bound(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(c::bad()),
    }
}

/// A belief about what someone else knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactBelief {
    pub fact: String,
    pub status: bool,
    pub source: String,
    pub at: u64,
    pub confidence: u64,
}

impl FactBelief {
    pub fn to_value(&self) -> Value {
        obj! {
            "fact" => Value::str(&self.fact),
            "status" => Value::Bool(self.status),
            "source" => Value::str(&self.source),
            "at" => Value::from(self.at),
            "confidence" => Value::from(self.confidence),
        }
    }
}

/// A belief about what someone else wants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WantBelief {
    pub goal: String,
    pub priority: u64,
    pub source: String,
    pub at: u64,
}

impl WantBelief {
    pub fn to_value(&self) -> Value {
        obj! {
            "goal" => Value::str(&self.goal),
            "priority" => Value::from(self.priority),
            "source" => Value::str(&self.source),
            "at" => Value::from(self.at),
        }
    }
}

/// A belief about what someone else feels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeelBelief {
    pub emotion: String,
    pub target: Option<String>,
    pub level: i64,
    pub source: String,
    pub at: u64,
}

impl FeelBelief {
    pub fn to_value(&self) -> Value {
        let mut map = vec![
            ("emotion".to_string(), Value::str(&self.emotion)),
            ("level".to_string(), Value::from(self.level)),
            ("source".to_string(), Value::str(&self.source)),
            ("at".to_string(), Value::from(self.at)),
        ];
        if let Some(ref tgt) = self.target {
            map.push(("target".to_string(), Value::str(tgt)));
        }
        map.sort_by(|a, b| a.0.cmp(&b.0));
        Value::Object(map)
    }
}

/// Model of a single other person held by a subject.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OtherModel {
    pub other: String,
    pub knows: BTreeMap<String, FactBelief>,
    pub wants: BTreeMap<String, WantBelief>,
    pub feels: BTreeMap<String, FeelBelief>,
}

impl OtherModel {
    pub fn new(other: impl Into<String>) -> Self {
        Self { other: other.into(), knows: BTreeMap::new(), wants: BTreeMap::new(), feels: BTreeMap::new() }
    }

    pub fn to_value(&self) -> Value {
        let knows_arr: Vec<Value> = self.knows.values().map(FactBelief::to_value).collect();
        let wants_arr: Vec<Value> = self.wants.values().map(WantBelief::to_value).collect();
        let feels_arr: Vec<Value> = self.feels.values().map(FeelBelief::to_value).collect();
        obj! {
            "other" => Value::str(&self.other),
            "knows" => Value::Array(knows_arr),
            "wants" => Value::Array(wants_arr),
            "feels" => Value::Array(feels_arr),
        }
    }
}

/// Subject being's mind containing models of other beings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BeingMind {
    pub subject: String,
    pub others: BTreeMap<String, OtherModel>,
}

impl BeingMind {
    pub fn new(subject: impl Into<String>) -> Self {
        Self { subject: subject.into(), others: BTreeMap::new() }
    }

    pub fn other_mut(&mut self, other: &str) -> &mut OtherModel {
        self.others.entry(other.to_string()).or_insert_with(|| OtherModel::new(other))
    }
}

/// Engine managing Theory of Mind beliefs across beings.
#[derive(Clone, Debug, Default)]
pub struct MindManager {
    pub beings: BTreeMap<String, BeingMind>,
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: MindLimits,
}

impl MindManager {
    pub fn new() -> Self {
        Self::default()
    }

    crate::character::limits::limited_workflow!();

    /// The subject's mind and its model of `other`, created within the host limits.
    fn model_in_room(&mut self, subject: &str, other: &str) -> Result<&mut OtherModel> {
        room(&self.beings, subject, self.limits.get("beings"))?;
        let max_others = self.limits.get("others");
        let mind = self.mind_mut(subject);
        room(&mind.others, other, max_others)?;
        Ok(mind.other_mut(other))
    }

    pub fn mind_mut(&mut self, subject: &str) -> &mut BeingMind {
        self.beings.entry(subject.to_string()).or_insert_with(|| BeingMind::new(subject))
    }

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &["beings"])?;
        let ops = request.get("ops").and_then(Value::as_array).ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        // Optional pre-declared beings
        if let Some(beings_val) = request.get("beings").and_then(Value::as_array) {
            for b in beings_val {
                keys(b, &["subject"], &["others"])?;
                let subj = ident_bound(c::get(b, "subject")?)?;
                room(&self.beings, subj, self.limits.get("beings"))?;
                let (max_others, max_beliefs) = (self.limits.get("others"), self.limits.get("beliefs"));
                let mind = self.mind_mut(subj);
                if let Some(others_arr) = b.get("others").and_then(Value::as_array) {
                    for other_val in others_arr {
                        keys(other_val, &["other"], &["knows", "wants", "feels"])?;
                        let other_id = ident_bound(c::get(other_val, "other")?)?;
                        room(&mind.others, other_id, max_others)?;
                        let model = mind.other_mut(other_id);
                        if let Some(k_arr) = other_val.get("knows").and_then(Value::as_array) {
                            for k in k_arr {
                                keys(k, &["fact", "status", "source"], &["at", "confidence"])?;
                                let fact = text_bound(c::get(k, "fact")?, 64)?.to_string();
                                let status = bool_bound(c::get(k, "status")?)?;
                                let src = text_bound(c::get(k, "source")?, 32)?;
                                if !PROVENANCE_SOURCES.contains(&src) {
                                    return Err(c::bad());
                                }
                                let at = k.get("at").map(|v| uint_bound(v, 0, u64::MAX)).transpose()?.unwrap_or(0);
                                let conf =
                                    k.get("confidence").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(100);
                                room(&model.knows, &fact, max_beliefs)?;
                                model.knows.insert(
                                    fact.clone(),
                                    FactBelief { fact, status, source: src.to_string(), at, confidence: conf },
                                );
                            }
                        }
                    }
                }
            }
        }

        let mut results = Vec::new();
        for op_val in ops {
            keys(op_val, &["op", "args"], &[])?;
            let op = text_bound(c::get(op_val, "op")?, 32)?;
            let args = c::get(op_val, "args")?;

            let res = match op {
                "init" => self.op_init(args)?,
                "assess" => self.op_assess(args)?,
                "record" => self.op_record(args)?,
                "infer_from_witness" => self.op_infer_from_witness(args)?,
                "query" => self.op_query(args)?,
                _ => return Err(c::bad()),
            };
            results.push(res);
        }

        Ok(obj! {
            "version" => Value::int(1),
            "results" => Value::Array(results),
        })
    }

    fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["subject"], &["others"])?;
        let subject = ident_bound(c::get(args, "subject")?)?;
        room(&self.beings, subject, self.limits.get("beings"))?;
        let (max_others, max_beliefs) = (self.limits.get("others"), self.limits.get("beliefs"));
        let mut mind = BeingMind::new(subject);
        if let Some(others_arr) = args.get("others").and_then(Value::as_array) {
            for other_val in others_arr {
                keys(other_val, &["other"], &["knows", "wants", "feels"])?;
                let other_id = ident_bound(c::get(other_val, "other")?)?;
                room(&mind.others, other_id, max_others)?;
                let model = mind.other_mut(other_id);
                if let Some(k_arr) = other_val.get("knows").and_then(Value::as_array) {
                    for k in k_arr {
                        keys(k, &["fact", "status", "source"], &["at", "confidence"])?;
                        let fact = text_bound(c::get(k, "fact")?, 64)?.to_string();
                        let status = bool_bound(c::get(k, "status")?)?;
                        let src = text_bound(c::get(k, "source")?, 32)?;
                        if !PROVENANCE_SOURCES.contains(&src) {
                            return Err(c::bad());
                        }
                        let at = k.get("at").map(|v| uint_bound(v, 0, u64::MAX)).transpose()?.unwrap_or(0);
                        let conf = k.get("confidence").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(100);
                        room(&model.knows, &fact, max_beliefs)?;
                        model.knows.insert(
                            fact.clone(),
                            FactBelief { fact, status, source: src.to_string(), at, confidence: conf },
                        );
                    }
                }
            }
        }
        self.beings.insert(subject.to_string(), mind);
        Ok(obj! {
            "ok" => Value::Bool(true),
            "subject" => Value::str(subject),
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["subject"], &["other", "category"])?;
        let subject = ident_bound(c::get(args, "subject")?)?;
        let target_other = args.get("other").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
        let category = args
            .get("category")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 32))
            .transpose()?
            .unwrap_or("all");

        let mind = self.beings.get(subject).ok_or_else(c::bad)?;
        let mut others_arr = Vec::new();

        for (other_id, model) in &mind.others {
            if let Some(target) = target_other {
                if other_id != target {
                    continue;
                }
            }
            let mut model_obj = vec![("other".to_string(), Value::str(other_id))];
            if category == "all" || category == "knows" {
                let k_arr: Vec<Value> = model.knows.values().map(FactBelief::to_value).collect();
                model_obj.push(("knows".to_string(), Value::Array(k_arr)));
            }
            if category == "all" || category == "wants" {
                let w_arr: Vec<Value> = model.wants.values().map(WantBelief::to_value).collect();
                model_obj.push(("wants".to_string(), Value::Array(w_arr)));
            }
            if category == "all" || category == "feels" {
                let f_arr: Vec<Value> = model.feels.values().map(FeelBelief::to_value).collect();
                model_obj.push(("feels".to_string(), Value::Array(f_arr)));
            }
            model_obj.sort_by(|a, b| a.0.cmp(&b.0));
            others_arr.push(Value::Object(model_obj));
        }

        Ok(obj! {
            "subject" => Value::str(subject),
            "others" => Value::Array(others_arr),
        })
    }

    fn op_record(&mut self, args: &Value) -> Result<Value> {
        keys(
            args,
            &["subject", "other", "category"],
            &["fact", "status", "goal", "priority", "emotion", "target", "level", "source", "at", "confidence"],
        )?;
        let subject = ident_bound(c::get(args, "subject")?)?;
        let other = ident_bound(c::get(args, "other")?)?;
        let category = text_bound(c::get(args, "category")?, 32)?;
        let src = args
            .get("source")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 32))
            .transpose()?
            .unwrap_or("inferred");
        if !PROVENANCE_SOURCES.contains(&src) {
            return Err(c::bad());
        }
        let at =
            args.get("at").filter(|x| **x != Value::Null).map(|v| uint_bound(v, 0, u64::MAX)).transpose()?.unwrap_or(0);

        let max_beliefs = self.limits.get("beliefs");
        let model = self.model_in_room(subject, other)?;

        match category {
            "knows" => {
                let fact = text_bound(c::get(args, "fact")?, 64)?.to_string();
                let status = bool_bound(c::get(args, "status")?)?;
                let conf = args.get("confidence").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(100);
                if model.knows.len() >= max_beliefs && !model.knows.contains_key(&fact) {
                    return Err(c::bad());
                }
                model.knows.insert(
                    fact.clone(),
                    FactBelief { fact: fact.clone(), status, source: src.to_string(), at, confidence: conf },
                );
                Ok(obj! {
                    "ok" => Value::Bool(true),
                    "subject" => Value::str(subject),
                    "other" => Value::str(other),
                    "category" => Value::str("knows"),
                    "fact" => Value::str(&fact),
                    "status" => Value::Bool(status),
                    "source" => Value::str(src),
                })
            }
            "wants" => {
                let goal = text_bound(c::get(args, "goal")?, 64)?.to_string();
                let priority = uint_bound(c::get(args, "priority")?, 0, 100)?;
                if model.wants.len() >= max_beliefs && !model.wants.contains_key(&goal) {
                    return Err(c::bad());
                }
                model
                    .wants
                    .insert(goal.clone(), WantBelief { goal: goal.clone(), priority, source: src.to_string(), at });
                Ok(obj! {
                    "ok" => Value::Bool(true),
                    "subject" => Value::str(subject),
                    "other" => Value::str(other),
                    "category" => Value::str("wants"),
                    "goal" => Value::str(&goal),
                    "priority" => Value::from(priority),
                    "source" => Value::str(src),
                })
            }
            "feels" => {
                let emotion = text_bound(c::get(args, "emotion")?, 32)?;
                if !ATTITUDE_KINDS.contains(&emotion) {
                    return Err(c::bad());
                }
                let target =
                    args.get("target").filter(|x| **x != Value::Null).map(ident_bound).transpose()?.map(str::to_string);
                let level = i64_bound(c::get(args, "level")?, -100, 100)?;
                let key = if let Some(ref tgt) = target { format!("{emotion}:{tgt}") } else { emotion.to_string() };
                if model.feels.len() >= max_beliefs && !model.feels.contains_key(&key) {
                    return Err(c::bad());
                }
                model.feels.insert(
                    key.clone(),
                    FeelBelief {
                        emotion: emotion.to_string(),
                        target: target.clone(),
                        level,
                        source: src.to_string(),
                        at,
                    },
                );
                let mut res = vec![
                    ("category".to_string(), Value::str("feels")),
                    ("emotion".to_string(), Value::str(emotion)),
                    ("level".to_string(), Value::from(level)),
                    ("ok".to_string(), Value::Bool(true)),
                    ("other".to_string(), Value::str(other)),
                    ("source".to_string(), Value::str(src)),
                    ("subject".to_string(), Value::str(subject)),
                ];
                if let Some(tgt) = target {
                    res.push(("target".to_string(), Value::str(&tgt)));
                }
                res.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(Value::Object(res))
            }
            _ => Err(c::bad()),
        }
    }

    fn op_infer_from_witness(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["subject", "entry", "beings"], &["fact"])?;
        let subject = ident_bound(c::get(args, "subject")?)?;
        let entry = c::get(args, "entry")?;
        let beings = c::get(args, "beings")?.as_array().ok_or_else(c::bad)?;
        if beings.is_empty() || beings.len() > self.limits.get("witnesses") {
            return Err(c::bad());
        }

        let at = entry
            .get("at")
            .filter(|x| **x != Value::Null)
            .map(|v| uint_bound(v, 0, u64::MAX))
            .transpose()?
            .unwrap_or(0);
        let default_fact = entry.get("kind").or_else(|| entry.get("id")).and_then(Value::as_str).unwrap_or("event");
        let fact = args
            .get("fact")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 64))
            .transpose()?
            .unwrap_or(default_fact);

        let mut inferences = Vec::new();
        room(&self.beings, subject, self.limits.get("beings"))?;
        let (max_others, max_beliefs) = (self.limits.get("others"), self.limits.get("beliefs"));
        let mind = self.mind_mut(subject);

        for being_val in beings {
            let being_id = ident_bound(c::get(being_val, "id")?)?;
            if being_id == subject {
                continue;
            }
            let (perceived, reason) = can_perceive(being_val, entry);
            let source = if perceived { "seen" } else { "inferred" };
            let confidence = if perceived { 100 } else { 90 };

            room(&mind.others, being_id, max_others)?;
            let model = mind.other_mut(being_id);
            room(&model.knows, fact, max_beliefs)?;
            model.knows.insert(
                fact.to_string(),
                FactBelief { fact: fact.to_string(), status: perceived, source: source.to_string(), at, confidence },
            );

            inferences.push(obj! {
                "confidence" => Value::from(confidence),
                "fact" => Value::str(fact),
                "knows" => Value::Bool(perceived),
                "other" => Value::str(being_id),
                "reason" => Value::str(reason),
                "source" => Value::str(source),
            });
        }

        inferences.sort_by(|a, b| {
            let a_other = a.get("other").and_then(Value::as_str).unwrap_or("");
            let b_other = b.get("other").and_then(Value::as_str).unwrap_or("");
            a_other.cmp(b_other)
        });

        Ok(obj! {
            "fact" => Value::str(fact),
            "inferences" => Value::Array(inferences),
            "ok" => Value::Bool(true),
            "subject" => Value::str(subject),
        })
    }

    fn op_query(&self, args: &Value) -> Result<Value> {
        keys(args, &["subject", "other", "category"], &["key"])?;
        let subject = ident_bound(c::get(args, "subject")?)?;
        let other = ident_bound(c::get(args, "other")?)?;
        let category = text_bound(c::get(args, "category")?, 32)?;
        let key = args.get("key").filter(|x| **x != Value::Null).map(|v| text_bound(v, 64)).transpose()?;

        let mind = match self.beings.get(subject) {
            Some(m) => m,
            None => {
                return Ok(obj! {
                    "found" => Value::Bool(false),
                    "other" => Value::str(other),
                    "subject" => Value::str(subject),
                })
            }
        };

        let model = match mind.others.get(other) {
            Some(m) => m,
            None => {
                return Ok(obj! {
                    "found" => Value::Bool(false),
                    "other" => Value::str(other),
                    "subject" => Value::str(subject),
                })
            }
        };

        match category {
            "knows" => {
                let k = key.ok_or_else(c::bad)?;
                if let Some(belief) = model.knows.get(k) {
                    Ok(obj! {
                        "belief" => belief.to_value(),
                        "found" => Value::Bool(true),
                        "other" => Value::str(other),
                        "subject" => Value::str(subject),
                    })
                } else {
                    Ok(obj! {
                        "found" => Value::Bool(false),
                        "other" => Value::str(other),
                        "subject" => Value::str(subject),
                    })
                }
            }
            "wants" => {
                let k = key.ok_or_else(c::bad)?;
                if let Some(belief) = model.wants.get(k) {
                    Ok(obj! {
                        "belief" => belief.to_value(),
                        "found" => Value::Bool(true),
                        "other" => Value::str(other),
                        "subject" => Value::str(subject),
                    })
                } else {
                    Ok(obj! {
                        "found" => Value::Bool(false),
                        "other" => Value::str(other),
                        "subject" => Value::str(subject),
                    })
                }
            }
            "feels" => {
                let k = key.ok_or_else(c::bad)?;
                if let Some(belief) = model.feels.get(k) {
                    Ok(obj! {
                        "belief" => belief.to_value(),
                        "found" => Value::Bool(true),
                        "other" => Value::str(other),
                        "subject" => Value::str(subject),
                    })
                } else {
                    Ok(obj! {
                        "found" => Value::Bool(false),
                        "other" => Value::str(other),
                        "subject" => Value::str(subject),
                    })
                }
            }
            _ => Err(c::bad()),
        }
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    let mut mgr = MindManager::new();
    mgr.workflow(req)
}
