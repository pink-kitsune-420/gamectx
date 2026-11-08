//! Character body state and needs dynamics (Gap 2 and 12).
//!
//! Pure host computation over:
//! - Need definitions with tick rates, thresholds, and satisfiers (action or item -> amount).
//! - Body state record: fatigue (accumulates with actions, recovers with rest), health,
//!   injuries, conditions (`asleep`, `sick`, `unconscious`, `dead`).
//! - Impact on systems:
//!   - Action catalog: blocked verbs and extra fatigue costs.
//!   - Routines: forced rest / incapacitated overrides.
//!   - Speech: silent while asleep, unconscious, or dead.
//!
//! ```
//! use gamectx::character::body::BodyManager;
//! use gamectx::data::json::parse;
//!
//! let mut manager = BodyManager::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {"op": "init", "args": {"being": "baker", "health": 100, "fatigue": 20}},
//!     {"op": "assess", "args": {"being": "baker"}}
//!   ]
//! }"#, 4096)?;
//! let out = manager.workflow(&req)?;
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
pub const MAX_NEEDS: usize = 16;
pub const MAX_INJURIES: usize = 16;
pub const MAX_CONDITIONS: usize = 16;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedSatisfier {
    pub action: Option<String>,
    pub item: Option<String>,
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedDef {
    pub id: String,
    pub label: String,
    pub threshold: u64,
    pub rate_per_tick: u64,
    pub level: u64,
    pub satisfiers: Vec<NeedSatisfier>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InjuryDef {
    pub id: String,
    pub label: String,
    pub severity: u64,
    pub location: String,
    pub effects: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeingBody {
    pub id: String,
    pub fatigue: u64,
    pub health: u64,
    pub conditions: Vec<String>,
    pub injuries: Vec<InjuryDef>,
    pub needs: BTreeMap<String, NeedDef>,
}

impl BeingBody {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            fatigue: 0,
            health: 100,
            conditions: Vec::new(),
            injuries: Vec::new(),
            needs: BTreeMap::new(),
        }
    }

    pub fn can_speak(&self) -> (bool, Option<&'static str>) {
        if self.conditions.iter().any(|c| c == "dead") {
            return (false, Some("dead"));
        }
        if self.conditions.iter().any(|c| c == "unconscious") {
            return (false, Some("unconscious"));
        }
        if self.conditions.iter().any(|c| c == "asleep") {
            return (false, Some("asleep"));
        }
        (true, None)
    }

    pub fn routine_override(&self) -> Option<&'static str> {
        if self.conditions.iter().any(|c| c == "dead") {
            return Some("incapacitated");
        }
        if self.conditions.iter().any(|c| c == "unconscious") {
            return Some("incapacitated");
        }
        if self.conditions.iter().any(|c| c == "asleep") {
            return Some("rest");
        }
        if self.fatigue >= 90 {
            return Some("rest");
        }
        None
    }
}

#[derive(Clone, Debug, Default)]
pub struct BodyManager {
    beings: BTreeMap<String, BeingBody>,
}

impl BodyManager {
    pub fn new() -> Self {
        Self { beings: BTreeMap::new() }
    }

    pub fn get_or_create(&mut self, being_id: &str) -> Result<&mut BeingBody> {
        if !self.beings.contains_key(being_id) {
            if self.beings.len() >= MAX_BEINGS {
                return Err(c::bad());
            }
            self.beings.insert(being_id.to_string(), BeingBody::new(being_id));
        }
        Ok(self.beings.get_mut(being_id).unwrap())
    }

    pub fn can_speak(being: &BeingBody) -> (bool, Option<&'static str>) {
        being.can_speak()
    }

    pub fn routine_override(being: &BeingBody) -> Option<&'static str> {
        being.routine_override()
    }

    pub fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being", "id", "fatigue", "health", "conditions", "injuries", "needs", "species"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;

        if self.beings.len() >= MAX_BEINGS && !self.beings.contains_key(being_id) {
            return Err(c::bad());
        }

        let mut body = BeingBody::new(being_id);

        if let Some(f_val) = args.get("fatigue") {
            body.fatigue = uint_bound(f_val, 0, 100)?;
        }
        if let Some(h_val) = args.get("health") {
            body.health = uint_bound(h_val, 0, 100)?;
            if body.health == 0 && !body.conditions.iter().any(|c| c == "dead") {
                body.conditions.push("dead".to_string());
            }
        }
        if let Some(conds_val) = args.get("conditions") {
            let arr = conds_val.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_CONDITIONS {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for c_val in arr {
                let s = ident_bound(c_val)?;
                if list.iter().any(|x| x == s) {
                    return Err(c::bad());
                }
                list.push(s.to_string());
            }
            body.conditions = list;
        }

        if let Some(injs_val) = args.get("injuries") {
            let arr = injs_val.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_INJURIES {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for inj_val in arr {
                keys(inj_val, &["id"], &["label", "severity", "location", "effects"])?;
                let iid = ident_bound(c::get(inj_val, "id")?)?.to_string();
                if list.iter().any(|inj: &InjuryDef| inj.id == iid) {
                    return Err(c::bad());
                }
                let label = inj_val.get("label").and_then(Value::as_str).unwrap_or(&iid).to_string();
                let severity = inj_val.get("severity").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(10);
                let location = inj_val.get("location").and_then(Value::as_str).unwrap_or("general").to_string();
                let effects = inj_val.get("effects").cloned().unwrap_or(Value::Object(Vec::new()));
                list.push(InjuryDef { id: iid, label, severity, location, effects });
            }
            body.injuries = list;
        }

        if let Some(needs_val) = args.get("needs") {
            let arr = needs_val.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_NEEDS {
                return Err(c::bad());
            }
            let mut map = BTreeMap::new();
            for n_val in arr {
                keys(n_val, &["id"], &["label", "threshold", "rate_per_tick", "level", "satisfiers"])?;
                let nid = ident_bound(c::get(n_val, "id")?)?.to_string();
                if map.contains_key(&nid) {
                    return Err(c::bad());
                }
                let label = n_val.get("label").and_then(Value::as_str).unwrap_or(&nid).to_string();
                let threshold = n_val.get("threshold").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(70);
                let rate_per_tick = n_val.get("rate_per_tick").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(0);
                let level = n_val.get("level").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(0);
                let mut satisfiers = Vec::new();
                if let Some(sats_val) = n_val.get("satisfiers") {
                    let s_arr = sats_val.as_array().ok_or_else(c::bad)?;
                    for s_val in s_arr {
                        keys(s_val, &[], &["action", "item", "amount"])?;
                        let act = s_val.get("action").and_then(Value::as_str).map(|s| s.to_string());
                        let itm = s_val.get("item").and_then(Value::as_str).map(|s| s.to_string());
                        let amt = s_val.get("amount").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(20);
                        satisfiers.push(NeedSatisfier { action: act, item: itm, amount: amt });
                    }
                }
                map.insert(nid.clone(), NeedDef { id: nid, label, threshold, rate_per_tick, level, satisfiers });
            }
            body.needs = map;
        }

        self.beings.insert(being_id.to_string(), body);
        Ok(obj! { "ok" => true, "being" => being_id.to_string() })
    }

    pub fn op_assess(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being", "id"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        let (can_spk, spk_ref) = being.can_speak();
        let r_override = being.routine_override();

        let mut sorted_conds = being.conditions.clone();
        sorted_conds.sort();
        let conds_val = Value::Array(sorted_conds.into_iter().map(Value::String).collect());

        let injs_val = Value::Array(
            being
                .injuries
                .iter()
                .map(|inj| {
                    obj! {
                        "id" => inj.id.clone(),
                        "label" => inj.label.clone(),
                        "severity" => inj.severity,
                        "location" => inj.location.clone(),
                        "effects" => inj.effects.clone(),
                    }
                })
                .collect(),
        );

        let needs_val = Value::Array(
            being
                .needs
                .values()
                .map(|n| {
                    obj! {
                        "id" => n.id.clone(),
                        "label" => n.label.clone(),
                        "level" => n.level,
                        "threshold" => n.threshold,
                        "rate_per_tick" => n.rate_per_tick,
                        "urgent" => n.level >= n.threshold,
                    }
                })
                .collect(),
        );

        let spk_ref_val = match spk_ref {
            Some(s) => Value::String(s.to_string()),
            None => Value::Null,
        };
        let r_override_val = match r_override {
            Some(s) => Value::String(s.to_string()),
            None => Value::Null,
        };

        Ok(obj! {
            "being" => being.id.clone(),
            "fatigue" => being.fatigue,
            "health" => being.health,
            "conditions" => conds_val,
            "injuries" => injs_val,
            "needs" => needs_val,
            "can_speak" => can_spk,
            "speech_refusal" => spk_ref_val,
            "routine_override" => r_override_val,
        })
    }

    pub fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being", "id", "ticks", "resting"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        let ticks = args.get("ticks").map(|v| uint_bound(v, 1, 1000)).transpose()?.unwrap_or(1);
        let resting = args.get("resting").and_then(Value::as_bool).unwrap_or(false);

        if being.conditions.iter().any(|c| c == "dead") {
            let mut sorted_conds = being.conditions.clone();
            sorted_conds.sort();
            return Ok(obj! {
                "ok" => true,
                "being" => being.id.clone(),
                "fatigue" => being.fatigue,
                "health" => 0,
                "conditions" => Value::Array(sorted_conds.into_iter().map(Value::String).collect()),
            });
        }

        let is_asleep = being.conditions.iter().any(|c| c == "asleep");
        let is_sick = being.conditions.iter().any(|c| c == "sick");

        if is_asleep {
            let recovery = 15 * ticks;
            being.fatigue = being.fatigue.saturating_sub(recovery);
        } else if resting {
            let recovery = 5 * ticks;
            being.fatigue = being.fatigue.saturating_sub(recovery);
        }

        let mut deprived_damage = 0;
        for n in being.needs.values_mut() {
            let mut rate = n.rate_per_tick;
            if is_sick {
                rate += 1;
            }
            n.level = (n.level + rate * ticks).min(100);
            if n.level >= 100 && rate > 0 {
                deprived_damage += 5 * ticks;
            }
        }

        if deprived_damage > 0 {
            being.health = being.health.saturating_sub(deprived_damage);
            if being.health == 0 {
                if !being.conditions.iter().any(|c| c == "dead") {
                    being.conditions.push("dead".to_string());
                }
                being.conditions.retain(|c| c != "asleep");
            }
        }

        let mut sorted_conds = being.conditions.clone();
        sorted_conds.sort();
        Ok(obj! {
            "ok" => true,
            "being" => being.id.clone(),
            "fatigue" => being.fatigue,
            "health" => being.health,
            "conditions" => Value::Array(sorted_conds.into_iter().map(Value::String).collect()),
        })
    }

    pub fn op_satisfy(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being", "id", "need", "amount", "item", "count", "action"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        if being.conditions.iter().any(|c| c == "dead") {
            return Err(c::bad());
        }

        let mut satisfied = Vec::new();

        if let Some(nid_val) = args.get("need") {
            let nid = ident_bound(nid_val)?;
            let n = being.needs.get_mut(nid).ok_or_else(c::bad)?;
            let amt = args.get("amount").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(20);
            n.level = n.level.saturating_sub(amt);
            satisfied.push(obj! {
                "need" => nid.to_string(),
                "amount" => amt,
                "new_level" => n.level,
            });
        } else if let Some(itm_val) = args.get("item") {
            let item_name = text_bound(itm_val, 64)?;
            let count = args.get("count").map(|v| uint_bound(v, 1, 100)).transpose()?.unwrap_or(1);
            let mut matched = false;
            for n in being.needs.values_mut() {
                for sat in &n.satisfiers {
                    if sat.item.as_deref() == Some(item_name) {
                        matched = true;
                        let amt = sat.amount * count;
                        n.level = n.level.saturating_sub(amt);
                        satisfied.push(obj! {
                            "need" => n.id.clone(),
                            "amount" => amt,
                            "new_level" => n.level,
                        });
                    }
                }
            }
            if !matched {
                return Err(c::bad());
            }
        } else if let Some(act_val) = args.get("action") {
            let action_name = ident_bound(act_val)?;
            let mut matched = false;
            for n in being.needs.values_mut() {
                for sat in &n.satisfiers {
                    if sat.action.as_deref() == Some(action_name) {
                        matched = true;
                        let amt = sat.amount;
                        n.level = n.level.saturating_sub(amt);
                        satisfied.push(obj! {
                            "need" => n.id.clone(),
                            "amount" => amt,
                            "new_level" => n.level,
                        });
                    }
                }
            }
            if !matched {
                return Err(c::bad());
            }
        } else {
            return Err(c::bad());
        }

        Ok(obj! {
            "ok" => true,
            "being" => being.id.clone(),
            "satisfied" => Value::Array(satisfied),
        })
    }

    pub fn op_rest(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being", "id", "ticks", "asleep"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        if being.conditions.iter().any(|c| c == "dead") {
            return Err(c::bad());
        }

        let ticks = args.get("ticks").map(|v| uint_bound(v, 1, 1000)).transpose()?.unwrap_or(1);
        let asleep = args.get("asleep").and_then(Value::as_bool).unwrap_or(false);

        if asleep {
            if !being.conditions.iter().any(|c| c == "asleep") {
                being.conditions.push("asleep".to_string());
            }
        } else {
            being.conditions.retain(|c| c != "asleep");
        }

        let tick_req = obj! {
            "being" => being_id.to_string(),
            "ticks" => ticks,
            "resting" => true,
        };
        self.op_tick(&tick_req)
    }

    pub fn op_apply_action(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["action"], &["being", "id"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        let action = c::get(args, "action")?;
        let act_id = ident_bound(c::get(action, "id")?)?;

        if being.conditions.iter().any(|c| c == "dead") {
            return Ok(obj! { "ok" => false, "being" => being.id.clone(), "refusal" => "actor is dead" });
        }
        if being.conditions.iter().any(|c| c == "unconscious") {
            return Ok(obj! { "ok" => false, "being" => being.id.clone(), "refusal" => "actor is unconscious" });
        }
        if being.conditions.iter().any(|c| c == "asleep") && act_id != "wake" && act_id != "sleep" && act_id != "rest" {
            return Ok(obj! { "ok" => false, "being" => being.id.clone(), "refusal" => "actor is asleep" });
        }

        for inj in &being.injuries {
            if let Some(arr) = inj.effects.get("blocked_actions").and_then(Value::as_array) {
                if arr.iter().any(|a| a.as_str() == Some(act_id)) {
                    return Ok(obj! {
                        "ok" => false,
                        "being" => being.id.clone(),
                        "refusal" => format!("blocked by injury: {}", inj.id),
                    });
                }
            }
        }

        let actor_spec = action.get("actor");
        let body_spec = actor_spec.and_then(|a| a.get("body_state"));

        if let Some(bs) = body_spec {
            if let Some(arr) = bs.get("disallowed_conditions").and_then(Value::as_array) {
                for c in &being.conditions {
                    if arr.iter().any(|x| x.as_str() == Some(c.as_str())) {
                        return Ok(obj! {
                            "ok" => false,
                            "being" => being.id.clone(),
                            "refusal" => format!("cannot act while {c}"),
                        });
                    }
                }
            }
            if let Some(mf_val) = bs.get("max_fatigue") {
                let max_f = uint_bound(mf_val, 0, 100)?;
                if being.fatigue > max_f {
                    return Ok(obj! {
                        "ok" => false,
                        "being" => being.id.clone(),
                        "refusal" => format!("too fatigued ({} > {max_f})", being.fatigue),
                    });
                }
            }
            if let Some(mc_val) = bs.get("min_condition") {
                let min_c = uint_bound(mc_val, 0, 100)?;
                if being.health < min_c {
                    return Ok(obj! {
                        "ok" => false,
                        "being" => being.id.clone(),
                        "refusal" => format!("condition too low ({} < {min_c})", being.health),
                    });
                }
            }
        }

        let cost = action.get("cost");
        let base_fatigue = cost.and_then(|c| c.get("fatigue")).map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(0);

        if being.fatigue >= 100 && base_fatigue > 0 {
            return Ok(obj! { "ok" => false, "being" => being.id.clone(), "refusal" => "actor is exhausted" });
        }

        let mut effective_fatigue = base_fatigue;
        if being.conditions.iter().any(|c| c == "sick") && effective_fatigue > 0 {
            effective_fatigue += (effective_fatigue / 2).max(2);
        }

        for inj in &being.injuries {
            let extra_f = inj.effects.get("extra_fatigue").and_then(Value::as_usize).map(|n| n as u64).unwrap_or(0);
            effective_fatigue += extra_f;
        }

        being.fatigue = (being.fatigue + effective_fatigue).min(100);

        if act_id == "wake" {
            being.conditions.retain(|c| c != "asleep");
        }

        let mut satisfied_needs = Vec::new();
        for n in being.needs.values_mut() {
            for sat in &n.satisfiers {
                if sat.action.as_deref() == Some(act_id) {
                    let amt = sat.amount;
                    n.level = n.level.saturating_sub(amt);
                    satisfied_needs.push(obj! {
                        "need" => n.id.clone(),
                        "amount" => amt,
                        "new_level" => n.level,
                    });
                }
            }
        }

        let mut sorted_conds = being.conditions.clone();
        sorted_conds.sort();

        Ok(obj! {
            "ok" => true,
            "being" => being.id.clone(),
            "action" => act_id.to_string(),
            "effective_fatigue_cost" => effective_fatigue,
            "fatigue" => being.fatigue,
            "health" => being.health,
            "conditions" => Value::Array(sorted_conds.into_iter().map(Value::String).collect()),
            "satisfied_needs" => Value::Array(satisfied_needs),
        })
    }

    pub fn op_set_condition(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["condition"], &["being", "id"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        let cond = ident_bound(c::get(args, "condition")?)?;

        if !being.conditions.iter().any(|c| c == cond) {
            if being.conditions.len() >= MAX_CONDITIONS {
                return Err(c::bad());
            }
            being.conditions.push(cond.to_string());
        }

        if cond == "dead" {
            being.health = 0;
            being.conditions.retain(|c| c != "asleep");
        }

        let mut sorted_conds = being.conditions.clone();
        sorted_conds.sort();
        Ok(obj! {
            "ok" => true,
            "being" => being.id.clone(),
            "conditions" => Value::Array(sorted_conds.into_iter().map(Value::String).collect()),
        })
    }

    pub fn op_remove_condition(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["condition"], &["being", "id"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        let cond = ident_bound(c::get(args, "condition")?)?;
        being.conditions.retain(|c| c != cond);

        let mut sorted_conds = being.conditions.clone();
        sorted_conds.sort();
        Ok(obj! {
            "ok" => true,
            "being" => being.id.clone(),
            "conditions" => Value::Array(sorted_conds.into_iter().map(Value::String).collect()),
        })
    }

    pub fn op_add_injury(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["injury"], &["being", "id"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        let inj_val = c::get(args, "injury")?;
        keys(inj_val, &["id"], &["label", "severity", "location", "effects"])?;
        let iid = ident_bound(c::get(inj_val, "id")?)?.to_string();

        if being.injuries.iter().any(|x| x.id == iid) {
            return Err(c::bad());
        }
        if being.injuries.len() >= MAX_INJURIES {
            return Err(c::bad());
        }

        let severity = inj_val.get("severity").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(10);
        let label = inj_val.get("label").and_then(Value::as_str).unwrap_or(&iid).to_string();
        let location = inj_val.get("location").and_then(Value::as_str).unwrap_or("general").to_string();
        let effects = inj_val.get("effects").cloned().unwrap_or(Value::Object(Vec::new()));

        being.injuries.push(InjuryDef { id: iid, label, severity, location, effects });
        being.health = being.health.saturating_sub(severity);
        if being.health == 0 && !being.conditions.iter().any(|c| c == "dead") {
            being.conditions.push("dead".to_string());
        }

        let injs_val = Value::Array(
            being
                .injuries
                .iter()
                .map(|inj| {
                    obj! {
                        "id" => inj.id.clone(),
                        "label" => inj.label.clone(),
                        "severity" => inj.severity,
                        "location" => inj.location.clone(),
                        "effects" => inj.effects.clone(),
                    }
                })
                .collect(),
        );

        Ok(obj! {
            "ok" => true,
            "being" => being.id.clone(),
            "injuries" => injs_val,
            "health" => being.health,
        })
    }

    pub fn op_heal_injury(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being", "id", "injury_id", "amount", "heal_health"])?;
        let being_val = args.get("being").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let being_id = ident_bound(being_val)?;
        let being = self.get_or_create(being_id)?;

        let iid_val = args.get("injury_id").or_else(|| args.get("id")).ok_or_else(c::bad)?;
        let iid = ident_bound(iid_val)?;

        let idx = being.injuries.iter().position(|x| x.id == iid).ok_or_else(c::bad)?;
        let sev = being.injuries[idx].severity;
        let amount = args.get("amount").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(sev);
        being.injuries[idx].severity = being.injuries[idx].severity.saturating_sub(amount);

        if being.injuries[idx].severity == 0 {
            being.injuries.remove(idx);
        }

        let heal_hp = args.get("heal_health").map(|v| uint_bound(v, 0, 100)).transpose()?.unwrap_or(amount);
        being.health = (being.health + heal_hp).min(100);

        let injs_val = Value::Array(
            being
                .injuries
                .iter()
                .map(|inj| {
                    obj! {
                        "id" => inj.id.clone(),
                        "label" => inj.label.clone(),
                        "severity" => inj.severity,
                        "location" => inj.location.clone(),
                        "effects" => inj.effects.clone(),
                    }
                })
                .collect(),
        );

        Ok(obj! {
            "ok" => true,
            "being" => being.id.clone(),
            "injuries" => injs_val,
            "health" => being.health,
        })
    }

    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
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
                "init" => self.op_init(args)?,
                "assess" => self.op_assess(args)?,
                "tick" => self.op_tick(args)?,
                "satisfy" => self.op_satisfy(args)?,
                "rest" => self.op_rest(args)?,
                "apply_action" => self.op_apply_action(args)?,
                "set_condition" => self.op_set_condition(args)?,
                "remove_condition" => self.op_remove_condition(args)?,
                "add_injury" => self.op_add_injury(args)?,
                "heal_injury" => self.op_heal_injury(args)?,
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
    let mut manager = BodyManager::new();
    manager.workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    #[test]
    fn test_body_manager_lifecycle() -> Result<()> {
        let mut manager = BodyManager::new();
        let init_req = parse(
            r#"{
            "ops": [
                {
                    "op": "init",
                    "args": {
                        "being": "mira",
                        "health": 100,
                        "fatigue": 20,
                        "needs": [
                            {
                                "id": "hunger",
                                "label": "Hunger",
                                "rate_per_tick": 2,
                                "threshold": 70,
                                "level": 10,
                                "satisfiers": [{"action": "eat", "amount": 30}]
                            }
                        ]
                    }
                },
                {"op": "assess", "args": {"being": "mira"}},
                {"op": "tick", "args": {"being": "mira", "ticks": 5}},
                {"op": "assess", "args": {"being": "mira"}}
            ]
        }"#,
            4096,
        )?;
        let res = manager.workflow(&init_req)?;
        let results = res.get("results").and_then(Value::as_array).unwrap();
        assert_eq!(results.len(), 4);

        // Before tick: hunger level 10
        let before = &results[1];
        let needs_before = before.get("needs").and_then(Value::as_array).unwrap();
        assert_eq!(needs_before[0].get("level").and_then(Value::as_usize).map(|n| n as u64), Some(10));

        // After tick: hunger level 10 + 2*5 = 20
        let after = &results[3];
        let needs_after = after.get("needs").and_then(Value::as_array).unwrap();
        assert_eq!(needs_after[0].get("level").and_then(Value::as_usize).map(|n| n as u64), Some(20));

        // Satisfy hunger by eating
        let eat_req = parse(
            r#"{
            "ops": [
                {
                    "op": "apply_action",
                    "args": {
                        "being": "mira",
                        "action": {
                            "id": "eat",
                            "cost": {"fatigue": 2}
                        }
                    }
                },
                {"op": "assess", "args": {"being": "mira"}}
            ]
        }"#,
            4096,
        )?;
        let eat_res = manager.workflow(&eat_req)?;
        let eat_results = eat_res.get("results").and_then(Value::as_array).unwrap();
        let assess_after_eat = &eat_results[1];
        let needs_after_eat = assess_after_eat.get("needs").and_then(Value::as_array).unwrap();
        // 20 - 30 clamped to 0
        assert_eq!(needs_after_eat[0].get("level").and_then(Value::as_usize).map(|n| n as u64), Some(0));

        Ok(())
    }
}
