//! Deception ledger: told-ledger per listener tracking claims, true values, and timestamps.
//!
//! Enables beings to maintain consistent deceptions ("keep a lie consistent")
//! and enables listeners to appraise contradictions when lies are exposed.
//! Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::deception::DeceptionLedger;
//!
//! let mut ledger = DeceptionLedger::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "record",
//!       "args": {
//!         "speaker": "merchant",
//!         "listener": "buyer",
//!         "topic": "item_authenticity",
//!         "claim": "genuine_relic",
//!         "true_value": "cheap_replica",
//!         "at": 10
//!       }
//!     }
//!   ]
//! }"#, 4096)?;
//! let out = ledger.workflow(&req)?;
//! assert_eq!(out.get("version").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_RECORDS: usize = 128;
pub const MAX_OPS: usize = 64;

/// Host-set limit: told `records` kept by the ledger (default [`MAX_RECORDS`]). Pre-declared and
/// `init` records respect it as well as `record`.
pub const LIMITS: &[crate::character::limits::Limit] =
    &[crate::character::limits::Limit { name: "records", default: MAX_RECORDS, ceiling: 1_000_000 }];
crate::character::limits::module_limits!(DeceptionLimits, LIMITS);

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

fn bool_bound(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(c::bad()),
    }
}

/// A single statement record in the told-ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToldRecord {
    pub id: String,
    pub speaker: String,
    pub listener: String,
    pub topic: String,
    pub claim: String,
    pub true_value: String,
    pub is_lie: bool,
    pub at: u64,
    pub status: String, // "active", "retracted", "contradicted"
}

impl ToldRecord {
    pub fn to_value(&self) -> Value {
        obj! {
            "at" => Value::from(self.at),
            "claim" => Value::str(&self.claim),
            "id" => Value::str(&self.id),
            "is_lie" => Value::Bool(self.is_lie),
            "listener" => Value::str(&self.listener),
            "speaker" => Value::str(&self.speaker),
            "status" => Value::str(&self.status),
            "topic" => Value::str(&self.topic),
            "true_value" => Value::str(&self.true_value),
        }
    }
}

/// Told-ledger managing claims, consistency checks, and contradiction appraisal.
#[derive(Clone, Debug, Default)]
pub struct DeceptionLedger {
    pub next_id: u64,
    pub records: BTreeMap<String, ToldRecord>,
    /// `(listener, topic, speaker)` → record ids, kept in step with `records` by the workflow ops
    /// so consistency checks and contradictions do not scan the whole ledger.
    pub by_listener_topic: BTreeMap<(String, String, String), BTreeSet<String>>,
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: DeceptionLimits,
}

impl DeceptionLedger {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            records: BTreeMap::new(),
            by_listener_topic: BTreeMap::new(),
            limits: DeceptionLimits::default(),
        }
    }

    crate::character::limits::limited_workflow!();

    fn put_record(&mut self, rec: ToldRecord) {
        if let Some(old) = self.records.get(&rec.id) {
            let key = (old.listener.clone(), old.topic.clone(), old.speaker.clone());
            if let Some(ids) = self.by_listener_topic.get_mut(&key) {
                ids.remove(&rec.id);
                if ids.is_empty() {
                    self.by_listener_topic.remove(&key);
                }
            }
        }
        self.by_listener_topic
            .entry((rec.listener.clone(), rec.topic.clone(), rec.speaker.clone()))
            .or_default()
            .insert(rec.id.clone());
        self.records.insert(rec.id.clone(), rec);
    }

    /// Records `speaker` told `listener` about `topic`, in id order.
    fn told_by(&self, speaker: &str, listener: &str, topic: &str) -> impl Iterator<Item = &ToldRecord> {
        self.by_listener_topic
            .get(&(listener.to_string(), topic.to_string(), speaker.to_string()))
            .into_iter()
            .flatten()
            .filter_map(|id| self.records.get(id))
    }

    /// Ids of records told to `listener` about `topic` by anyone, in id order.
    fn told_ids(&self, listener: &str, topic: &str) -> Vec<String> {
        let start = (listener.to_string(), topic.to_string(), String::new());
        let mut ids: Vec<String> = self
            .by_listener_topic
            .range(start..)
            .take_while(|((l, t, _), _)| l == listener && t == topic)
            .flat_map(|(_, ids)| ids.iter().cloned())
            .collect();
        ids.sort_unstable();
        ids
    }

    fn record_room(&self, id: &str) -> Result<()> {
        if self.records.len() >= self.limits.get("records") && !self.records.contains_key(id) {
            return Err(c::bad());
        }
        Ok(())
    }

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &["records"])?;
        let ops = request.get("ops").and_then(Value::as_array).ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        // Optional pre-declared records
        if let Some(records_val) = request.get("records").and_then(Value::as_array) {
            for r in records_val {
                keys(r, &["id", "speaker", "listener", "topic", "claim", "true_value"], &["is_lie", "at", "status"])?;
                let id = text_bound(c::get(r, "id")?, 32)?.to_string();
                let speaker = ident_bound(c::get(r, "speaker")?)?.to_string();
                let listener = ident_bound(c::get(r, "listener")?)?.to_string();
                let topic = text_bound(c::get(r, "topic")?, 64)?.to_string();
                let claim = text_bound(c::get(r, "claim")?, 128)?.to_string();
                let true_val = text_bound(c::get(r, "true_value")?, 128)?.to_string();
                let is_lie = r.get("is_lie").map(bool_bound).transpose()?.unwrap_or(claim != true_val);
                let at = r.get("at").map(|v| uint_bound(v, 0, u64::MAX)).transpose()?.unwrap_or(0);
                let status = r.get("status").map(|v| text_bound(v, 32)).transpose()?.unwrap_or("active").to_string();
                if !["active", "retracted", "contradicted"].contains(&status.as_str()) {
                    return Err(c::bad());
                }
                self.record_room(&id)?;
                self.put_record(ToldRecord {
                    id,
                    speaker,
                    listener,
                    topic,
                    claim,
                    true_value: true_val,
                    is_lie,
                    at,
                    status,
                });
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
                "check_consistency" => self.op_check_consistency(args)?,
                "appraise_contradiction" => self.op_appraise_contradiction(args)?,
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
        keys(args, &[], &["records"])?;
        self.records.clear();
        self.by_listener_topic.clear();
        self.next_id = 1;
        if let Some(records_val) = args.get("records").and_then(Value::as_array) {
            for r in records_val {
                keys(r, &["speaker", "listener", "topic", "claim"], &["id", "true_value", "is_lie", "at", "status"])?;
                let speaker = ident_bound(c::get(r, "speaker")?)?.to_string();
                let listener = ident_bound(c::get(r, "listener")?)?.to_string();
                let topic = text_bound(c::get(r, "topic")?, 64)?.to_string();
                let claim = text_bound(c::get(r, "claim")?, 128)?.to_string();
                let true_val =
                    r.get("true_value").map(|v| text_bound(v, 128)).transpose()?.unwrap_or(&claim).to_string();
                let is_lie = r.get("is_lie").map(bool_bound).transpose()?.unwrap_or(claim != true_val);
                let at = r.get("at").map(|v| uint_bound(v, 0, u64::MAX)).transpose()?.unwrap_or(0);
                let status = r.get("status").map(|v| text_bound(v, 32)).transpose()?.unwrap_or("active").to_string();
                if !["active", "retracted", "contradicted"].contains(&status.as_str()) {
                    return Err(c::bad());
                }
                let id = if let Some(custom_id) = r.get("id").map(|v| text_bound(v, 32)).transpose()? {
                    custom_id.to_string()
                } else {
                    let id_str = format!("dec_{}", self.next_id);
                    self.next_id += 1;
                    id_str
                };
                self.record_room(&id)?;
                self.put_record(ToldRecord {
                    id,
                    speaker,
                    listener,
                    topic,
                    claim,
                    true_value: true_val,
                    is_lie,
                    at,
                    status,
                });
            }
        }
        Ok(obj! {
            "count" => Value::from(self.records.len()),
            "ok" => Value::Bool(true),
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &["speaker", "listener", "topic", "status"])?;
        let filter_speaker = args.get("speaker").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
        let filter_listener = args.get("listener").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
        let filter_topic = args.get("topic").filter(|x| **x != Value::Null).map(|v| text_bound(v, 64)).transpose()?;
        let filter_status = args.get("status").filter(|x| **x != Value::Null).map(|v| text_bound(v, 32)).transpose()?;

        let mut matched = Vec::new();
        for rec in self.records.values() {
            if let Some(s) = filter_speaker {
                if rec.speaker != s {
                    continue;
                }
            }
            if let Some(l) = filter_listener {
                if rec.listener != l {
                    continue;
                }
            }
            if let Some(t) = filter_topic {
                if rec.topic != t {
                    continue;
                }
            }
            if let Some(st) = filter_status {
                if rec.status != st {
                    continue;
                }
            }
            matched.push(rec.to_value());
        }

        Ok(obj! {
            "records" => Value::Array(matched),
        })
    }

    fn op_record(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["speaker", "listener", "topic", "claim"], &["id", "true_value", "is_lie", "at", "status"])?;
        let speaker = ident_bound(c::get(args, "speaker")?)?.to_string();
        let listener = ident_bound(c::get(args, "listener")?)?.to_string();
        if speaker == listener {
            return Err(c::bad());
        }
        let topic = text_bound(c::get(args, "topic")?, 64)?.to_string();
        let claim = text_bound(c::get(args, "claim")?, 128)?.to_string();
        let true_val = args
            .get("true_value")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 128))
            .transpose()?
            .unwrap_or(&claim)
            .to_string();
        let is_lie =
            args.get("is_lie").filter(|x| **x != Value::Null).map(bool_bound).transpose()?.unwrap_or(claim != true_val);
        let at =
            args.get("at").filter(|x| **x != Value::Null).map(|v| uint_bound(v, 0, u64::MAX)).transpose()?.unwrap_or(0);
        let status = args
            .get("status")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 32))
            .transpose()?
            .unwrap_or("active")
            .to_string();
        if !["active", "retracted", "contradicted"].contains(&status.as_str()) {
            return Err(c::bad());
        }

        if self.records.len() >= self.limits.get("records") {
            return Err(c::bad());
        }

        let id = if let Some(custom_id) =
            args.get("id").filter(|x| **x != Value::Null).map(|v| text_bound(v, 32)).transpose()?
        {
            custom_id.to_string()
        } else {
            let id_str = format!("dec_{}", self.next_id);
            self.next_id += 1;
            id_str
        };

        let record = ToldRecord {
            id: id.clone(),
            speaker: speaker.clone(),
            listener: listener.clone(),
            topic: topic.clone(),
            claim: claim.clone(),
            true_value: true_val,
            is_lie,
            at,
            status,
        };
        self.put_record(record);

        Ok(obj! {
            "id" => Value::str(&id),
            "is_lie" => Value::Bool(is_lie),
            "listener" => Value::str(&listener),
            "ok" => Value::Bool(true),
            "speaker" => Value::str(&speaker),
            "topic" => Value::str(&topic),
        })
    }

    fn op_check_consistency(&self, args: &Value) -> Result<Value> {
        keys(args, &["speaker", "listener", "topic", "proposed_claim"], &[])?;
        let speaker = ident_bound(c::get(args, "speaker")?)?;
        let listener = ident_bound(c::get(args, "listener")?)?;
        let topic = text_bound(c::get(args, "topic")?, 64)?;
        let proposed = text_bound(c::get(args, "proposed_claim")?, 128)?;

        // Find the most recent active claim made by speaker to listener on topic
        let prior = self.told_by(speaker, listener, topic).filter(|r| r.status == "active").max_by_key(|r| r.at);

        match prior {
            Some(r) => {
                let consistent = r.claim == proposed;
                let warning =
                    if consistent { Value::Null } else { Value::str("contradicts_prior_claim_to_this_listener") };
                let mut out = vec![
                    ("consistent".to_string(), Value::Bool(consistent)),
                    ("is_prior_lie".to_string(), Value::Bool(r.is_lie)),
                    ("listener".to_string(), Value::str(listener)),
                    ("ok".to_string(), Value::Bool(true)),
                    ("prior_claim".to_string(), Value::str(&r.claim)),
                    ("speaker".to_string(), Value::str(speaker)),
                    ("topic".to_string(), Value::str(topic)),
                ];
                if !consistent {
                    out.push(("warning".to_string(), warning));
                }
                out.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(Value::Object(out))
            }
            None => Ok(obj! {
                "consistent" => Value::Bool(true),
                "is_prior_lie" => Value::Bool(false),
                "listener" => Value::str(listener),
                "ok" => Value::Bool(true),
                "prior_claim" => Value::Null,
                "speaker" => Value::str(speaker),
                "topic" => Value::str(topic),
            }),
        }
    }

    fn op_appraise_contradiction(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["listener", "topic", "observed_truth"], &["at"])?;
        let listener = ident_bound(c::get(args, "listener")?)?;
        let topic = text_bound(c::get(args, "topic")?, 64)?;
        let observed = text_bound(c::get(args, "observed_truth")?, 128)?;

        let mut contradictions = Vec::new();
        let mut effects = Vec::new();

        // Check all active records told to this listener on this topic that contradict observed truth
        let rec_keys: Vec<String> = self
            .told_ids(listener, topic)
            .into_iter()
            .filter(|id| self.records.get(id).is_some_and(|r| r.status == "active" && r.claim != observed))
            .collect();

        for k in rec_keys {
            if let Some(rec) = self.records.get_mut(&k) {
                rec.status = "contradicted".to_string();
                contradictions.push(obj! {
                    "claim" => Value::str(&rec.claim),
                    "id" => Value::str(&rec.id),
                    "speaker" => Value::str(&rec.speaker),
                    "topic" => Value::str(&rec.topic),
                });

                effects.push(obj! {
                    "attitude" => Value::str("trust"),
                    "delta" => Value::int(-25),
                    "reason" => Value::str(format!("contradicted_claim:{}", rec.topic)),
                    "subject" => Value::str(listener),
                    "target" => Value::str(&rec.speaker),
                });

                effects.push(obj! {
                    "attitude" => Value::str("resentment"),
                    "delta" => Value::int(15),
                    "reason" => Value::str(format!("deception_revealed:{}", rec.topic)),
                    "subject" => Value::str(listener),
                    "target" => Value::str(&rec.speaker),
                });
            }
        }

        contradictions.sort_by(|a, b| {
            let a_id = a.get("id").and_then(Value::as_str).unwrap_or("");
            let b_id = b.get("id").and_then(Value::as_str).unwrap_or("");
            a_id.cmp(b_id)
        });

        effects.sort_by(|a, b| {
            let a_tgt = a.get("target").and_then(Value::as_str).unwrap_or("");
            let b_tgt = b.get("target").and_then(Value::as_str).unwrap_or("");
            let a_att = a.get("attitude").and_then(Value::as_str).unwrap_or("");
            let b_att = b.get("attitude").and_then(Value::as_str).unwrap_or("");
            (a_tgt, a_att).cmp(&(b_tgt, b_att))
        });

        let found = !contradictions.is_empty();

        Ok(obj! {
            "contradiction_found" => Value::Bool(found),
            "contradictions" => Value::Array(contradictions),
            "listener" => Value::str(listener),
            "observed_truth" => Value::str(observed),
            "relationship_effects" => Value::Array(effects),
            "topic" => Value::str(topic),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    let mut ledger = DeceptionLedger::new();
    ledger.workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    #[test]
    fn re_recording_an_id_moves_it_between_index_buckets() {
        let req = r#"{"ops":[
            {"op":"record","args":{"id":"c1","speaker":"trader","listener":"baker","topic":"price","claim":"cheap"}},
            {"op":"record","args":{"id":"c1","speaker":"carter","listener":"baker","topic":"price","claim":"dear"}},
            {"op":"check_consistency","args":{"speaker":"trader","listener":"baker","topic":"price","proposed_claim":"x"}},
            {"op":"check_consistency","args":{"speaker":"carter","listener":"baker","topic":"price","proposed_claim":"dear"}}
        ]}"#;
        let mut ledger = DeceptionLedger::new();
        let out = ledger.workflow(&parse(req, 1 << 16).unwrap()).unwrap();
        let results = out.get("results").and_then(Value::as_array).unwrap();
        assert_eq!(results[2].get("prior_claim"), Some(&Value::Null), "the trader's claim was replaced");
        assert_eq!(results[3].get("prior_claim").and_then(Value::as_str), Some("dear"));
        assert_eq!(ledger.by_listener_topic.values().map(BTreeSet::len).sum::<usize>(), ledger.records.len());
    }
}
