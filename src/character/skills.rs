//! Skills growth with use, practice gain, action scaling, and teaching (Gap 8).
//!
//! Provides:
//! - Practice gain per successful action with diminishing returns as level increases.
//! - Action catalog duration, risk, and quality scaling by skill level.
//! - Teaching between beings (teacher level > student level) producing skill practice and relationship respect/gratitude.
//!
//! ```
//! use gamectx::character::skills::SkillsManager;
//! use gamectx::data::json::parse;
//!
//! let mut mgr = SkillsManager::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "init",
//!       "args": {
//!         "being": "baker",
//!         "skills": [
//!           {"id": "baking", "label": "Baking", "level": 10, "practice": 20}
//!         ]
//!       }
//!     },
//!     {
//!       "op": "practice",
//!       "args": {
//!         "being": "baker",
//!         "skill": "baking",
//!         "practice": 30,
//!         "success": true
//!       }
//!     }
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
pub const MAX_SKILLS_PER_BEING: usize = 32;

/// Host-set limits: `beings` (default [`MAX_BEINGS`]) and `skills` per being (default
/// [`MAX_SKILLS_PER_BEING`], also bounding skills added by `practice` and `teach`).
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "beings", default: MAX_BEINGS, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "skills", default: MAX_SKILLS_PER_BEING, ceiling: 1024 },
];
crate::character::limits::module_limits!(SkillsLimits, LIMITS);

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

pub fn threshold_for_level(level: u64) -> u64 {
    100 + level.saturating_mul(15)
}

pub fn quality_tier(score: u64) -> &'static str {
    match score {
        0..=24 => "poor",
        25..=49 => "standard",
        50..=74 => "fine",
        75..=89 => "superior",
        _ => "masterwork",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRecord {
    pub id: String,
    pub label: String,
    pub level: u64,
    pub practice: u64,
    pub threshold: u64,
    pub total_actions: u64,
}

impl SkillRecord {
    pub fn new(id: String, label: String, level: u64, practice: u64) -> Self {
        let lvl = level.min(100);
        let thresh = threshold_for_level(lvl);
        Self {
            id,
            label,
            level: lvl,
            practice: if lvl >= 100 { 0 } else { practice },
            threshold: thresh,
            total_actions: 0,
        }
    }

    pub fn to_value(&self) -> Value {
        let pct = if self.level >= 100 { 100 } else { (self.practice * 100).checked_div(self.threshold).unwrap_or(0) };
        obj! {
            "id" => Value::str(&self.id),
            "label" => Value::str(&self.label),
            "level" => Value::int(self.level as i64),
            "practice" => Value::int(self.practice as i64),
            "threshold" => Value::int(self.threshold as i64),
            "progress_percent" => Value::int(pct as i64),
            "total_actions" => Value::int(self.total_actions as i64)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeingSkills {
    pub being: String,
    pub skills: BTreeMap<String, SkillRecord>,
}

impl BeingSkills {
    pub fn to_value(&self) -> Value {
        let list = self.skills.values().map(SkillRecord::to_value).collect::<Vec<_>>();
        obj! {
            "being" => Value::str(&self.being),
            "skills" => Value::Array(list)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillsManager {
    beings: BTreeMap<String, BeingSkills>,
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: SkillsLimits,
}

impl SkillsManager {
    pub fn new() -> Self {
        Self::default()
    }

    crate::character::limits::limited_workflow!();

    pub fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["skills"])?;
        let being = ident_bound(c::get(args, "being")?)?.to_string();
        if self.beings.len() >= self.limits.get("beings") && !self.beings.contains_key(&being) {
            return Err(c::bad());
        }

        let mut skills = BTreeMap::new();
        if let Some(arr) = args.get("skills").and_then(Value::as_array) {
            if arr.len() > self.limits.get("skills") {
                return Err(c::bad());
            }
            for item in arr {
                keys(item, &["id"], &["label", "level", "practice"])?;
                let id = ident_bound(c::get(item, "id")?)?.to_string();
                let label = item
                    .get("label")
                    .filter(|x| **x != Value::Null)
                    .map(|v| text_bound(v, 48))
                    .transpose()?
                    .unwrap_or(&id)
                    .to_string();
                let level = if let Some(lvl) = item.get("level") { uint_bound(lvl, 0, 100)? } else { 1 };
                let practice = if let Some(p) = item.get("practice") { uint_bound(p, 0, 1_000_000)? } else { 0 };

                let record = SkillRecord::new(id.clone(), label, level, practice);
                skills.insert(id, record);
            }
        }

        let state = BeingSkills { being: being.clone(), skills };
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

    pub fn op_practice(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "skill", "practice", "success"], &["label", "action"])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let skill_id = ident_bound(c::get(args, "skill")?)?;
        let raw_practice = uint_bound(c::get(args, "practice")?, 0, 100_000)?;
        let success = c::boolean(c::get(args, "success")?)?;

        let label = args
            .get("label")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 48))
            .transpose()?
            .unwrap_or(skill_id);

        let max_skills = self.limits.get("skills");
        let state = self.beings.get_mut(being).ok_or_else(c::bad)?;
        if !state.skills.contains_key(skill_id) && state.skills.len() >= max_skills {
            return Err(c::bad());
        }

        let rec = state
            .skills
            .entry(skill_id.to_string())
            .or_insert_with(|| SkillRecord::new(skill_id.to_string(), label.to_string(), 0, 0));

        if !success || raw_practice == 0 {
            return Ok(obj! {
                "being" => Value::str(being),
                "skill" => Value::str(skill_id),
                "level" => Value::int(rec.level as i64),
                "practice" => Value::int(rec.practice as i64),
                "threshold" => Value::int(rec.threshold as i64),
                "effective_gain" => Value::int(0),
                "leveled_up" => Value::Bool(false),
                "success" => Value::Bool(success)
            });
        }

        rec.total_actions += 1;
        if rec.level >= 100 {
            return Ok(obj! {
                "being" => Value::str(being),
                "skill" => Value::str(skill_id),
                "level" => Value::int(100),
                "practice" => Value::int(0),
                "threshold" => Value::int(rec.threshold as i64),
                "effective_gain" => Value::int(0),
                "leveled_up" => Value::Bool(false),
                "success" => Value::Bool(true)
            });
        }

        // Diminishing returns: as level increases, practice gained per action scales down
        let factor = (100u64.saturating_sub(rec.level)).max(10);
        let effective_gain = ((raw_practice.saturating_mul(factor)) / 100).max(1);

        rec.practice += effective_gain;
        let mut leveled_up = false;
        while rec.practice >= rec.threshold && rec.level < 100 {
            rec.practice -= rec.threshold;
            rec.level += 1;
            rec.threshold = threshold_for_level(rec.level);
            leveled_up = true;
        }

        if rec.level >= 100 {
            rec.practice = 0;
        }

        Ok(obj! {
            "being" => Value::str(being),
            "skill" => Value::str(skill_id),
            "level" => Value::int(rec.level as i64),
            "practice" => Value::int(rec.practice as i64),
            "threshold" => Value::int(rec.threshold as i64),
            "effective_gain" => Value::int(effective_gain as i64),
            "leveled_up" => Value::Bool(leveled_up),
            "success" => Value::Bool(true)
        })
    }

    pub fn op_evaluate_action(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "action", "base_duration", "base_risk"], &["skill", "quality_bonus"])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let action = ident_bound(c::get(args, "action")?)?;
        let base_duration = uint_bound(c::get(args, "base_duration")?, 0, 1_000_000)?;
        let base_risk = uint_bound(c::get(args, "base_risk")?, 0, 100)?;
        let quality_bonus = if let Some(qb) = args.get("quality_bonus") { uint_bound(qb, 0, 100)? } else { 0 };

        let skill_id = args.get("skill").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;

        let level = if let Some(sk) = skill_id {
            self.beings.get(being).and_then(|b| b.skills.get(sk)).map(|r| r.level).unwrap_or(0)
        } else {
            0
        };

        // Skill effect on duration: up to 50% discount at level 100
        let discount = (level.saturating_mul(50)) / 100;
        let duration = (base_duration.saturating_mul(100 - discount)) / 100;

        // Skill effect on risk: reduces failure chance proportionally
        let risk_discount = (level.saturating_mul(base_risk)) / 100;
        let risk = base_risk.saturating_sub(risk_discount);

        // Quality score
        let quality_score = (level + quality_bonus).min(100);
        let tier = quality_tier(quality_score);

        Ok(obj! {
            "being" => Value::str(being),
            "action" => Value::str(action),
            "skill" => skill_id.map(Value::str).unwrap_or(Value::Null),
            "level" => Value::int(level as i64),
            "base_duration" => Value::int(base_duration as i64),
            "duration" => Value::int(duration as i64),
            "duration_discount_percent" => Value::int(discount as i64),
            "base_risk" => Value::int(base_risk as i64),
            "risk" => Value::int(risk as i64),
            "quality_score" => Value::int(quality_score as i64),
            "quality_tier" => Value::str(tier)
        })
    }

    pub fn op_teach(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["teacher", "student", "skill"], &["hours", "same_place"])?;
        let teacher = ident_bound(c::get(args, "teacher")?)?;
        let student = ident_bound(c::get(args, "student")?)?;
        let skill_id = ident_bound(c::get(args, "skill")?)?;
        let hours = if let Some(h) = args.get("hours") { uint_bound(h, 1, 24)? } else { 1 };
        let same_place = if let Some(sp) = args.get("same_place") { c::boolean(sp)? } else { true };

        if !same_place {
            return Ok(obj! {
                "ok" => Value::Bool(false),
                "reason" => Value::str("teacher and student must be in the same place")
            });
        }

        let teacher_level = self.beings.get(teacher).and_then(|b| b.skills.get(skill_id)).map(|r| r.level).unwrap_or(0);

        let student_level = self.beings.get(student).and_then(|b| b.skills.get(skill_id)).map(|r| r.level).unwrap_or(0);

        if teacher_level <= student_level {
            return Ok(obj! {
                "ok" => Value::Bool(false),
                "reason" => Value::str("teacher skill level must be strictly greater than student level")
            });
        }

        // Student gains structured practice
        let gap = teacher_level - student_level;
        let base_rate = 15 + gap / 2;
        let raw_practice = base_rate.saturating_mul(hours);

        let max_skills = self.limits.get("skills");
        let student_being = self.beings.get_mut(student).ok_or_else(c::bad)?;
        if !student_being.skills.contains_key(skill_id) && student_being.skills.len() >= max_skills {
            return Err(c::bad());
        }
        let s_rec = student_being
            .skills
            .entry(skill_id.to_string())
            .or_insert_with(|| SkillRecord::new(skill_id.to_string(), skill_id.to_string(), 0, 0));

        let factor = (100u64.saturating_sub(s_rec.level)).max(10);
        let effective_gain = ((raw_practice.saturating_mul(factor)) / 100).max(1);

        s_rec.practice += effective_gain;
        s_rec.total_actions += 1;
        let mut leveled_up = false;
        while s_rec.practice >= s_rec.threshold && s_rec.level < teacher_level {
            s_rec.practice -= s_rec.threshold;
            s_rec.level += 1;
            s_rec.threshold = threshold_for_level(s_rec.level);
            leveled_up = true;
        }

        if s_rec.level >= teacher_level {
            s_rec.level = teacher_level;
            s_rec.practice = 0;
        }

        let respect_delta = (5 * hours).min(50) as i64;
        let gratitude_delta = (5 * hours).min(50) as i64;

        let rel_effects = vec![
            obj! {
                "subject" => Value::str(student),
                "target" => Value::str(teacher),
                "attitude" => Value::str("respect"),
                "delta" => Value::int(respect_delta),
                "reason" => Value::str("learned from teacher")
            },
            obj! {
                "subject" => Value::str(student),
                "target" => Value::str(teacher),
                "attitude" => Value::str("gratitude"),
                "delta" => Value::int(gratitude_delta),
                "reason" => Value::str("received dedicated instruction")
            },
        ];

        Ok(obj! {
            "ok" => Value::Bool(true),
            "teacher" => Value::str(teacher),
            "student" => Value::str(student),
            "skill" => Value::str(skill_id),
            "teacher_level" => Value::int(teacher_level as i64),
            "student_level" => Value::int(s_rec.level as i64),
            "student_practice" => Value::int(s_rec.practice as i64),
            "practice_gained" => Value::int(effective_gain as i64),
            "leveled_up" => Value::Bool(leveled_up),
            "relationship_effects" => Value::Array(rel_effects)
        })
    }

    fn run_workflow(&mut self, req: &Value) -> Result<Value> {
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
                "practice" => self.op_practice(args)?,
                "evaluate_action" => self.op_evaluate_action(args)?,
                "teach" => self.op_teach(args)?,
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
    let mut mgr = SkillsManager::new();
    mgr.workflow(req)
}
