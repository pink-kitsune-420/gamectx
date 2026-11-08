//! Affect decay, temperament baselines, and mood dynamics (Gap 6).
//!
//! Separates transient background mood (valence/arousal) from targeted emotions and attitudes.
//! Decays attitudes and mood toward baselines determined by temperament and attitude kinds.
//!
//! ```
//! use gamectx::character::affect::AffectManager;
//! use gamectx::data::json::parse;
//!
//! let mut mgr = AffectManager::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "init",
//!       "args": {
//!         "being": "baker",
//!         "temperament": "calm",
//!         "mood": {"valence": 20, "arousal": 10},
//!         "attitudes": [
//!           {"id": "fear_wolf", "kind": "fear", "subject": "wolf", "level": 40, "baseline": 0, "decay_rate": 4, "reason": "howling"}
//!         ]
//!       }
//!     },
//!     {"op": "tick", "args": {"being": "baker", "ticks": 5}}
//!   ]
//! }"#, 4096)?;
//! let out = mgr.workflow(&req)?;
//! assert_eq!(out.get("version").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use std::collections::BTreeMap;

use crate::checked as c;
use crate::data::json::Value;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result};

pub const MAX_BEINGS: usize = 64;
pub const MAX_OPS: usize = 64;
pub const MAX_ATTITUDES: usize = 64;

pub const ATTITUDE_KINDS: [&str; 24] = [
    "preference",
    "opinion",
    "familiarity",
    "trust",
    "affection",
    "resentment",
    "obligation",
    "disagreement",
    "belief",
    "report",
    "hypothesis",
    "misconception",
    "unknown",
    "goal",
    "commitment",
    "unfinished",
    "priority",
    "physical",
    "emotion",
    "fear",
    "respect",
    "gratitude",
    "envy",
    "admiration",
];

pub fn mood_label(valence: i64, arousal: i64) -> &'static str {
    if valence >= 30 && arousal >= 20 {
        "excited"
    } else if valence >= 30 && arousal < -20 {
        "serene"
    } else if valence >= 30 {
        "cheerful"
    } else if valence <= -30 && arousal >= 20 {
        "agitated"
    } else if valence <= -30 && arousal < -20 {
        "depressed"
    } else if valence <= -30 {
        "irritable"
    } else if arousal >= 40 {
        "tense"
    } else if arousal <= -40 {
        "lethargic"
    } else {
        "calm"
    }
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

fn int_bound(v: &Value, lo: i64, hi: i64) -> Result<i64> {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttitudeState {
    pub id: String,
    pub kind: String,
    pub subject: String,
    pub level: i64,
    pub baseline: i64,
    pub decay_rate: u64,
    pub reason: String,
}

impl AttitudeState {
    pub fn to_value(&self) -> Value {
        obj! {
            "id" => Value::str(&self.id),
            "kind" => Value::str(&self.kind),
            "subject" => Value::str(&self.subject),
            "level" => Value::int(self.level),
            "baseline" => Value::int(self.baseline),
            "decay_rate" => Value::int(self.decay_rate as i64),
            "reason" => Value::str(&self.reason)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoodState {
    pub valence: i64,
    pub arousal: i64,
    pub baseline_valence: i64,
    pub baseline_arousal: i64,
    pub decay_rate: u64,
}

impl Default for MoodState {
    fn default() -> Self {
        Self { valence: 0, arousal: 0, baseline_valence: 0, baseline_arousal: 0, decay_rate: 5 }
    }
}

impl MoodState {
    pub fn to_value(&self) -> Value {
        obj! {
            "valence" => Value::int(self.valence),
            "arousal" => Value::int(self.arousal),
            "baseline_valence" => Value::int(self.baseline_valence),
            "baseline_arousal" => Value::int(self.baseline_arousal),
            "decay_rate" => Value::int(self.decay_rate as i64),
            "label" => Value::str(mood_label(self.valence, self.arousal))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeingAffect {
    pub being: String,
    pub temperament: String,
    pub mood: MoodState,
    pub attitudes: BTreeMap<String, AttitudeState>,
}

impl BeingAffect {
    pub fn to_value(&self) -> Value {
        let atts = self.attitudes.values().map(AttitudeState::to_value).collect::<Vec<_>>();
        obj! {
            "being" => Value::str(&self.being),
            "temperament" => Value::str(&self.temperament),
            "mood" => self.mood.to_value(),
            "attitudes" => Value::Array(atts)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AffectManager {
    beings: BTreeMap<String, BeingAffect>,
}

impl AffectManager {
    pub fn new() -> Self {
        Self { beings: BTreeMap::new() }
    }

    pub fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["temperament", "mood", "attitudes"])?;
        let being = ident_bound(c::get(args, "being")?)?.to_string();
        if self.beings.len() >= MAX_BEINGS && !self.beings.contains_key(&being) {
            return Err(c::bad());
        }

        let temp = args
            .get("temperament")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 32))
            .transpose()?
            .unwrap_or("calm")
            .to_string();

        let mut mood = MoodState::default();
        if let Some(m) = args.get("mood").filter(|x| **x != Value::Null) {
            keys(m, &[], &["valence", "arousal", "baseline_valence", "baseline_arousal", "decay_rate"])?;
            if let Some(v) = m.get("valence") {
                mood.valence = int_bound(v, -100, 100)?;
            }
            if let Some(a) = m.get("arousal") {
                mood.arousal = int_bound(a, -100, 100)?;
            }
            if let Some(bv) = m.get("baseline_valence") {
                mood.baseline_valence = int_bound(bv, -100, 100)?;
            }
            if let Some(ba) = m.get("baseline_arousal") {
                mood.baseline_arousal = int_bound(ba, -100, 100)?;
            }
            if let Some(dr) = m.get("decay_rate") {
                mood.decay_rate = uint_bound(dr, 0, 100)?;
            }
        }

        let mut attitudes = BTreeMap::new();
        if let Some(arr) = args.get("attitudes").and_then(Value::as_array) {
            if arr.len() > MAX_ATTITUDES {
                return Err(c::bad());
            }
            for item in arr {
                keys(item, &["id", "kind", "subject", "level"], &["baseline", "decay_rate", "reason"])?;
                let id = ident_bound(c::get(item, "id")?)?.to_string();
                let kind = text_bound(c::get(item, "kind")?, 32)?;
                if !ATTITUDE_KINDS.contains(&kind) {
                    return Err(c::bad());
                }
                let subject = ident_bound(c::get(item, "subject")?)?.to_string();
                let level = int_bound(c::get(item, "level")?, -100, 100)?;

                let default_base = match kind {
                    "emotion" | "fear" | "gratitude" | "envy" | "admiration" => 0,
                    _ => level,
                };
                let baseline = if let Some(b) = item.get("baseline") { int_bound(b, -100, 100)? } else { default_base };

                let default_rate: u64 = match kind {
                    "emotion" => 5,
                    "fear" => 4,
                    "admiration" | "gratitude" | "envy" => 3,
                    "resentment" => 2,
                    "affection" | "trust" => 1,
                    _ => 0,
                };
                let decay_rate =
                    if let Some(d) = item.get("decay_rate") { uint_bound(d, 0, 100)? } else { default_rate };

                let reason = item
                    .get("reason")
                    .filter(|x| **x != Value::Null)
                    .map(|v| text_bound(v, 256))
                    .transpose()?
                    .unwrap_or("")
                    .to_string();

                attitudes.insert(
                    id.clone(),
                    AttitudeState { id, kind: kind.to_string(), subject, level, baseline, decay_rate, reason },
                );
            }
        }

        let state = BeingAffect { being: being.clone(), temperament: temp, mood, attitudes };
        let val = state.to_value();
        self.beings.insert(being, state);
        Ok(val)
    }

    pub fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let state = self.beings.get(being).ok_or_else(c::bad)?;
        Ok(state.to_value())
    }

    pub fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "ticks"], &[])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let ticks = uint_bound(c::get(args, "ticks")?, 1, 100_000)?;
        let state = self.beings.get_mut(being).ok_or_else(c::bad)?;

        // 1. Mood decay toward baseline
        let m_decay = (state.mood.decay_rate.saturating_mul(ticks)) as i64;
        if state.mood.valence > state.mood.baseline_valence {
            state.mood.valence = (state.mood.valence - m_decay).max(state.mood.baseline_valence);
        } else if state.mood.valence < state.mood.baseline_valence {
            state.mood.valence = (state.mood.valence + m_decay).min(state.mood.baseline_valence);
        }
        if state.mood.arousal > state.mood.baseline_arousal {
            state.mood.arousal = (state.mood.arousal - m_decay).max(state.mood.baseline_arousal);
        } else if state.mood.arousal < state.mood.baseline_arousal {
            state.mood.arousal = (state.mood.arousal + m_decay).min(state.mood.baseline_arousal);
        }

        // 2. Attitude decay toward baseline
        let temp = &state.temperament;
        for att in state.attitudes.values_mut() {
            let mut rate = att.decay_rate;
            if (temp.contains("calm") && matches!(att.kind.as_str(), "emotion" | "fear"))
                || (temp.contains("forgiving") && att.kind == "resentment")
            {
                rate = rate.saturating_add(2);
            } else if temp.contains("volatile") && matches!(att.kind.as_str(), "emotion" | "fear") {
                rate = rate.saturating_sub(2).max(1);
            } else if temp.contains("vengeful") && att.kind == "resentment" {
                rate = rate.saturating_sub(1).max(1);
            }

            if rate > 0 {
                let amt = (rate.saturating_mul(ticks)) as i64;
                if att.level > att.baseline {
                    att.level = (att.level - amt).max(att.baseline);
                } else if att.level < att.baseline {
                    att.level = (att.level + amt).min(att.baseline);
                }
            }
        }

        Ok(state.to_value())
    }

    pub fn op_appraise(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "kind", "subject", "delta"], &["id", "reason"])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let kind = text_bound(c::get(args, "kind")?, 32)?;
        if !ATTITUDE_KINDS.contains(&kind) {
            return Err(c::bad());
        }
        let subject = ident_bound(c::get(args, "subject")?)?;
        let delta = int_bound(c::get(args, "delta")?, -100, 100)?;
        let reason = args
            .get("reason")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 256))
            .transpose()?
            .unwrap_or("")
            .to_string();

        let att_id = if let Some(i) = args.get("id").filter(|x| **x != Value::Null) {
            ident_bound(i)?.to_string()
        } else {
            format!("{kind}_{subject}")
        };

        let state = self.beings.get_mut(being).ok_or_else(c::bad)?;

        let (before, after) = if let Some(att) = state.attitudes.get_mut(&att_id) {
            let b = att.level;
            att.level = (att.level + delta).clamp(-100, 100);
            if !reason.is_empty() {
                att.reason = reason.clone();
            }
            (b, att.level)
        } else {
            if state.attitudes.len() >= MAX_ATTITUDES {
                return Err(c::bad());
            }
            let default_base = match kind {
                "emotion" | "fear" | "gratitude" | "envy" | "admiration" => 0,
                _ => 0,
            };
            let default_rate: u64 = match kind {
                "emotion" => 5,
                "fear" => 4,
                "admiration" | "gratitude" | "envy" => 3,
                "resentment" => 2,
                "affection" | "trust" => 1,
                _ => 0,
            };
            let lvl = delta.clamp(-100, 100);
            state.attitudes.insert(
                att_id.clone(),
                AttitudeState {
                    id: att_id.clone(),
                    kind: kind.to_string(),
                    subject: subject.to_string(),
                    level: lvl,
                    baseline: default_base,
                    decay_rate: default_rate,
                    reason: reason.clone(),
                },
            );
            (0, lvl)
        };

        // Shift mood based on emotional valence of appraisal
        let is_positive = matches!(kind, "gratitude" | "respect" | "admiration" | "affection" | "trust");
        let is_negative = matches!(kind, "fear" | "resentment" | "envy");

        if is_positive {
            state.mood.valence = (state.mood.valence + delta / 2).clamp(-100, 100);
            state.mood.arousal = (state.mood.arousal + (delta / 4).abs()).clamp(-100, 100);
        } else if is_negative {
            state.mood.valence = (state.mood.valence - delta.abs() / 2).clamp(-100, 100);
            state.mood.arousal = (state.mood.arousal + delta.abs() / 2).clamp(-100, 100);
        } else {
            state.mood.valence = (state.mood.valence + delta / 4).clamp(-100, 100);
        }

        Ok(obj! {
            "being" => Value::str(being),
            "attitude_id" => Value::str(&att_id),
            "kind" => Value::str(kind),
            "subject" => Value::str(subject),
            "before" => Value::int(before),
            "after" => Value::int(after),
            "delta" => Value::int(delta),
            "reason" => Value::str(&reason),
            "mood" => state.mood.to_value()
        })
    }

    pub fn op_set_mood(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["valence", "arousal", "valence_delta", "arousal_delta", "reason"])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let state = self.beings.get_mut(being).ok_or_else(c::bad)?;

        if let Some(v) = args.get("valence").filter(|x| **x != Value::Null) {
            state.mood.valence = int_bound(v, -100, 100)?;
        }
        if let Some(a) = args.get("arousal").filter(|x| **x != Value::Null) {
            state.mood.arousal = int_bound(a, -100, 100)?;
        }
        if let Some(vd) = args.get("valence_delta").filter(|x| **x != Value::Null) {
            let d = int_bound(vd, -100, 100)?;
            state.mood.valence = (state.mood.valence + d).clamp(-100, 100);
        }
        if let Some(ad) = args.get("arousal_delta").filter(|x| **x != Value::Null) {
            let d = int_bound(ad, -100, 100)?;
            state.mood.arousal = (state.mood.arousal + d).clamp(-100, 100);
        }

        Ok(obj! {
            "being" => Value::str(being),
            "mood" => state.mood.to_value()
        })
    }

    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
        keys(req, &["ops"], &[])?;
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        let mut results = Vec::new();
        for op_call in ops {
            keys(op_call, &["op"], &["args"])?;
            let op_name = text_bound(c::get(op_call, "op")?, 32)?;
            let empty_args = Value::Object(Vec::new());
            let args = op_call.get("args").filter(|x| **x != Value::Null).unwrap_or(&empty_args);

            let res = match op_name {
                "init" => self.op_init(args)?,
                "assess" => self.op_assess(args)?,
                "tick" => self.op_tick(args)?,
                "appraise" => self.op_appraise(args)?,
                "set_mood" => self.op_set_mood(args)?,
                _ => return Err(c::bad()),
            };
            results.push(res);
        }

        Ok(obj! {
            "version" => Value::int(1),
            "results" => Value::Array(results)
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    let mut mgr = AffectManager::new();
    mgr.workflow(req)
}
