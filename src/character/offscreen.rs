//! Off-screen life resolution: coarse resolution of unobserved activities.
//!
//! When an activity window closes unobserved by the player, applies the Action Catalog's
//! expected effects once (resources consumed/produced, fatigue delta, skill practice),
//! records a timeline entry (`private` or `witnessed`), and creates a Life experience.
//! Bounded work per tick ($\le 16$). Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::offscreen::OffscreenResolver;
//!
//! let mut resolver = OffscreenResolver::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "resolve_window",
//!       "args": {
//!         "being": "baker",
//!         "activity": "baking_shift",
//!         "action": "cook",
//!         "observed": false,
//!         "at": 10,
//!         "place": "bakery",
//!         "inputs": [{"item": "flour", "count": 2}],
//!         "outputs": [{"item": "bread", "count": 2}],
//!         "fatigue": 5,
//!         "skill": "baking",
//!         "practice": 15
//!       }
//!     }
//!   ]
//! }"#, 4096)?;
//! let out = resolver.workflow(&req)?;
//! assert_eq!(out.get("version").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_WORK_PER_TICK: usize = 16;
pub const MAX_OPS: usize = 64;
pub const MAX_RECORDS: usize = 128;

/// Host-set limits: resolution `records` kept (oldest id evicted first; default [`MAX_RECORDS`])
/// and `work_per_tick` activities per `batch_resolve` (default [`MAX_WORK_PER_TICK`]).
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "records", default: MAX_RECORDS, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "work_per_tick", default: MAX_WORK_PER_TICK, ceiling: 4096 },
];
crate::character::limits::module_limits!(OffscreenLimits, LIMITS);

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

/// A recorded off-screen resolution outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolutionRecord {
    pub id: String,
    pub being: String,
    pub activity: String,
    pub action: String,
    pub at: u64,
    pub consumed: Vec<Value>,
    pub produced: Vec<Value>,
    pub fatigue_delta: i64,
    pub practice_gained: u64,
    pub timeline_entry: Value,
    pub life_experience: Value,
}

impl ResolutionRecord {
    pub fn to_value(&self) -> Value {
        obj! {
            "action" => Value::str(&self.action),
            "activity" => Value::str(&self.activity),
            "at" => Value::from(self.at),
            "being" => Value::str(&self.being),
            "consumed" => Value::Array(self.consumed.clone()),
            "fatigue_delta" => Value::from(self.fatigue_delta),
            "id" => Value::str(&self.id),
            "life_experience" => self.life_experience.clone(),
            "practice_gained" => Value::from(self.practice_gained),
            "produced" => Value::Array(self.produced.clone()),
            "timeline_entry" => self.timeline_entry.clone(),
        }
    }
}

/// Engine managing coarse off-screen resolution of unobserved activities.
#[derive(Clone, Debug, Default)]
pub struct OffscreenResolver {
    pub next_token: u64,
    pub records: BTreeMap<String, ResolutionRecord>,
    /// Being → record ids, kept in step with `records` by the workflow ops so `assess` for one
    /// being does not scan every record.
    pub by_being: BTreeMap<String, BTreeSet<String>>,
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: OffscreenLimits,
}

impl OffscreenResolver {
    pub fn new() -> Self {
        Self {
            next_token: 1000,
            records: BTreeMap::new(),
            by_being: BTreeMap::new(),
            limits: OffscreenLimits::default(),
        }
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &[])?;
        let ops = request.get("ops").and_then(Value::as_array).ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        let mut results = Vec::new();
        for op_val in ops {
            keys(op_val, &["op", "args"], &[])?;
            let op = text_bound(c::get(op_val, "op")?, 32)?;
            let args = c::get(op_val, "args")?;

            let res = match op {
                "init" => self.op_init(args)?,
                "resolve_window" => self.op_resolve_window(args)?,
                "batch_resolve" => self.op_batch_resolve(args)?,
                "assess" => self.op_assess(args)?,
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
        keys(args, &[], &["clear_records"])?;
        let clear =
            args.get("clear_records").filter(|x| **x != Value::Null).map(bool_bound).transpose()?.unwrap_or(false);
        if clear {
            self.records.clear();
            self.by_being.clear();
        }
        Ok(obj! {
            "ok" => Value::Bool(true),
        })
    }

    fn op_resolve_window(&mut self, args: &Value) -> Result<Value> {
        keys(
            args,
            &["being", "activity", "action", "observed"],
            &["at", "duration", "place", "inputs", "outputs", "fatigue", "skill", "practice", "visibility"],
        )?;
        let being = ident_bound(c::get(args, "being")?)?.to_string();
        let activity = text_bound(c::get(args, "activity")?, 64)?.to_string();
        let action = ident_bound(c::get(args, "action")?)?.to_string();
        let observed = bool_bound(c::get(args, "observed")?)?;

        if observed {
            return Ok(obj! {
                "being" => Value::str(&being),
                "reason" => Value::str("observed_on_screen"),
                "resolved" => Value::Bool(false),
            });
        }

        let at =
            args.get("at").filter(|x| **x != Value::Null).map(|v| uint_bound(v, 0, u64::MAX)).transpose()?.unwrap_or(0);
        let place = args
            .get("place")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 64))
            .transpose()?
            .unwrap_or("unknown");
        let visibility = args
            .get("visibility")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 32))
            .transpose()?
            .unwrap_or("private");
        if !["private", "witnessed"].contains(&visibility) {
            return Err(c::bad());
        }

        let fatigue = args
            .get("fatigue")
            .filter(|x| **x != Value::Null)
            .map(|v| i64_bound(v, -100, 100))
            .transpose()?
            .unwrap_or(5);
        let practice = args
            .get("practice")
            .filter(|x| **x != Value::Null)
            .map(|v| uint_bound(v, 0, 100))
            .transpose()?
            .unwrap_or(10);

        let consumed = args
            .get("inputs")
            .filter(|x| **x != Value::Null)
            .and_then(Value::as_array)
            .map(|a| a.to_vec())
            .unwrap_or_default();
        let produced = args
            .get("outputs")
            .filter(|x| **x != Value::Null)
            .and_then(Value::as_array)
            .map(|a| a.to_vec())
            .unwrap_or_default();

        let token = self.next_token;
        self.next_token += 1;

        let summary = format!("{being} completed off-screen {activity}.");
        let timeline_entry = obj! {
            "actors" => vec![Value::str(&being)],
            "at" => Value::from(at),
            "kind" => Value::str("offscreen_activity"),
            "noise" => Value::int(10),
            "place" => Value::str(place),
            "summary" => Value::str(&summary),
            "visibility" => Value::str(visibility),
        };

        let exp_text = format!("Completed {activity} off-screen.");
        let life_experience = obj! {
            "at" => Value::from(at),
            "audience" => vec![Value::str(&being)],
            "causes" => Vec::<Value>::new(),
            "kind" => Value::str("direct"),
            "retain" => Value::Bool(true),
            "source" => obj! {
                "id" => Value::str(format!("off_{token}")),
                "speaker" => Value::str(&being),
                "text" => Value::str(&exp_text),
            },
            "subject" => Value::str(&being),
            "tag" => Value::str(&activity),
            "text" => Value::str(&exp_text),
            "token" => Value::from(token),
        };

        let rec_id = format!("res_{token}");
        let record = ResolutionRecord {
            id: rec_id.clone(),
            being: being.clone(),
            activity: activity.clone(),
            action: action.clone(),
            at,
            consumed: consumed.clone(),
            produced: produced.clone(),
            fatigue_delta: fatigue,
            practice_gained: practice,
            timeline_entry: timeline_entry.clone(),
            life_experience: life_experience.clone(),
        };

        if self.records.len() >= self.limits.get("records") {
            if let Some(first_key) = self.records.keys().next().cloned() {
                self.drop_record(&first_key);
            }
        }
        self.drop_record(&rec_id);
        self.by_being.entry(record.being.clone()).or_default().insert(rec_id.clone());
        self.records.insert(rec_id.clone(), record);

        Ok(obj! {
            "action" => Value::str(&action),
            "activity" => Value::str(&activity),
            "being" => Value::str(&being),
            "consumed" => Value::Array(consumed),
            "fatigue_delta" => Value::from(fatigue),
            "id" => Value::str(&rec_id),
            "life_experience" => life_experience,
            "practice_gained" => Value::from(practice),
            "produced" => Value::Array(produced),
            "resolved" => Value::Bool(true),
            "timeline_entry" => timeline_entry,
        })
    }

    fn op_batch_resolve(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["activities"], &[])?;
        let list = c::get(args, "activities")?.as_array().ok_or_else(c::bad)?;
        if list.len() > self.limits.get("work_per_tick") {
            return Err(c::bad());
        }

        let mut out = Vec::new();
        for item in list {
            let res = self.op_resolve_window(item)?;
            out.push(res);
        }

        Ok(obj! {
            "count" => Value::from(out.len()),
            "ok" => Value::Bool(true),
            "resolved" => Value::Array(out),
        })
    }

    fn drop_record(&mut self, id: &str) {
        if let Some(old) = self.records.remove(id) {
            if let Some(ids) = self.by_being.get_mut(&old.being) {
                ids.remove(id);
                if ids.is_empty() {
                    self.by_being.remove(&old.being);
                }
            }
        }
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being"])?;
        let filter_being = args.get("being").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;

        let matched: Vec<Value> = match filter_being {
            Some(b) => self
                .by_being
                .get(b)
                .into_iter()
                .flatten()
                .filter_map(|id| self.records.get(id))
                .map(ResolutionRecord::to_value)
                .collect(),
            None => self.records.values().map(ResolutionRecord::to_value).collect(),
        };

        Ok(obj! {
            "records" => Value::Array(matched),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    let mut resolver = OffscreenResolver::new();
    resolver.workflow(req)
}
