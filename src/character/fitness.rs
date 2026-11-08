//! Character fitness: physical attributes (strength, endurance, agility, condition),
//! carry capacity, travel speed, fatigue calculation, and physical training.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::fitness::FitnessWorld;
//!
//! let mut fw = FitnessWorld::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {"op": "assess"}
//!   ],
//!   "fitness": {
//!     "strength": 75,
//!     "endurance": 65,
//!     "agility": 60,
//!     "condition": 90
//!   }
//! }"#, 4096)?;
//! let out = fw.workflow(&req)?;
//! assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::{obj, Result, Value};

pub const MAX_OPS: usize = 64;
pub const ATTRIBUTES: [&str; 4] = ["strength", "endurance", "agility", "condition"];
pub const TRAIN_ATTRIBUTES: [&str; 3] = ["strength", "endurance", "agility"];

fn as_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

fn keys(v: &Value, required: &[&str], optional: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(c::bad)?;
    if required.iter().any(|k| v.get(k).is_none())
        || rows.iter().any(|(k, _)| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(c::bad());
    }
    Ok(())
}

fn text_bound(v: &Value, max_chars: usize) -> Result<&str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    let char_count = s.chars().count();
    if char_count == 0 || char_count > max_chars {
        return Err(c::bad());
    }
    if s.chars().any(|ch| {
        let code = ch as u32;
        code < 32 || (127..=159).contains(&code)
    }) {
        return Err(c::bad());
    }
    Ok(s)
}

fn ident_bound(v: &Value) -> Result<&str> {
    let s = text_bound(v, 64)?;
    if s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b':' || b == b'-') {
        Ok(s)
    } else {
        Err(c::bad())
    }
}

fn uint_bound(v: &Value, lo: u64, hi: u64) -> Result<u64> {
    let n = as_u64(v).ok_or_else(c::bad)?;
    if n < lo || n > hi {
        return Err(c::bad());
    }
    Ok(n)
}

pub fn validate_fitness(f: &Value) -> Result<()> {
    keys(f, &[], &["strength", "endurance", "agility", "condition"])?;
    for k in ATTRIBUTES {
        if let Some(v) = f.get(k).filter(|x| **x != Value::Null) {
            uint_bound(v, 0, 100)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fitness {
    pub strength: u64,
    pub endurance: u64,
    pub agility: u64,
    pub condition: u64,
}

impl Default for Fitness {
    fn default() -> Self {
        Self { strength: 50, endurance: 50, agility: 50, condition: 100 }
    }
}

impl Fitness {
    pub fn parse(v: &Value) -> Result<Self> {
        validate_fitness(v)?;
        let strength =
            v.get("strength").filter(|x| **x != Value::Null).map(|x| uint_bound(x, 0, 100)).transpose()?.unwrap_or(50);
        let endurance =
            v.get("endurance").filter(|x| **x != Value::Null).map(|x| uint_bound(x, 0, 100)).transpose()?.unwrap_or(50);
        let agility =
            v.get("agility").filter(|x| **x != Value::Null).map(|x| uint_bound(x, 0, 100)).transpose()?.unwrap_or(50);
        let condition = v
            .get("condition")
            .filter(|x| **x != Value::Null)
            .map(|x| uint_bound(x, 0, 100))
            .transpose()?
            .unwrap_or(100);

        Ok(Self { strength, endurance, agility, condition })
    }

    pub fn to_value(&self) -> Value {
        obj! {
            "strength" => self.strength,
            "endurance" => self.endurance,
            "agility" => self.agility,
            "condition" => self.condition,
        }
    }

    pub fn compute_effective(&self, health_status: Option<&str>, needs: Option<&Value>) -> Result<Self> {
        let mut s = self.strength as i64;
        let mut e = self.endurance as i64;
        let mut a = self.agility as i64;
        let mut c = self.condition as i64;

        match health_status {
            Some("hurt") => {
                s -= 15;
                a -= 15;
                c -= 20;
            }
            Some("sick") => {
                s -= 25;
                e -= 30;
                a -= 20;
                c -= 40;
            }
            Some("exhausted") => {
                e -= 40;
                a -= 30;
                c -= 50;
            }
            _ => {}
        }

        if let Some(nd) = needs.filter(|x| **x != Value::Null) {
            let energy = nd.get("energy").and_then(as_u64).unwrap_or(100);
            let nourishment = nd.get("nourishment").and_then(as_u64).unwrap_or(100);

            if energy < 30 {
                e -= (30 - energy) as i64;
            }
            if nourishment < 30 {
                s -= (30 - nourishment) as i64;
            }
        }

        Ok(Self {
            strength: s.clamp(0, 100) as u64,
            endurance: e.clamp(0, 100) as u64,
            agility: a.clamp(0, 100) as u64,
            condition: c.clamp(0, 100) as u64,
        })
    }

    pub fn carry_capacity(&self) -> u64 {
        let max_cap = 20 + self.strength + (self.endurance / 2);
        let eff_cap = (max_cap * (50 + self.condition / 2)) / 100;
        eff_cap.max(10)
    }

    pub fn travel_speed(&self, current_load: u64) -> u64 {
        let mut base_speed = 500 + self.agility * 6 + self.endurance * 4;
        let cap = self.carry_capacity();
        if current_load > cap {
            let overload = current_load - cap;
            let penalty = (overload * 1000) / cap;
            base_speed = base_speed.saturating_sub(penalty).max(100);
        }
        base_speed
    }

    pub fn travel_cost(&self, distance: u64, current_load: u64) -> (u64, u64, u64) {
        let speed = self.travel_speed(current_load);
        let time = if distance > 0 {
            let t = (distance * 1000) / speed;
            if t == 0 {
                1
            } else {
                t
            }
        } else {
            0
        };

        let dist_fatigue = (distance * 100) / (self.endurance + self.condition / 2 + 10);
        let load_fatigue = (current_load * 50) / (self.strength + 10);
        let total_fatigue = (dist_fatigue + load_fatigue).min(100);

        (speed, time, total_fatigue)
    }

    pub fn can_carry(&self, current_load: u64, add_weight: u64) -> (bool, u64, u64, i64) {
        let cap = self.carry_capacity();
        let total = current_load.saturating_add(add_weight);
        let remaining = cap as i64 - total as i64;
        (total <= cap, cap, total, remaining)
    }

    pub fn train(&mut self, attribute: &str, effort: u64) -> Result<(u64, u64, u64)> {
        if !TRAIN_ATTRIBUTES.contains(&attribute) {
            return Err(c::bad());
        }
        if effort == 0 || effort > 100 {
            return Err(c::bad());
        }

        let cur = match attribute {
            "strength" => self.strength,
            "endurance" => self.endurance,
            "agility" => self.agility,
            _ => return Err(c::bad()),
        };

        let gain = if cur >= 100 {
            0
        } else {
            let g = (effort * (100 - cur)) / 1000;
            if effort >= 10 && cur < 100 && g == 0 {
                1
            } else {
                g
            }
        };

        let new_level = (cur + gain).min(100);
        let fatigue = ((effort * 100) / (self.endurance + 10)).min(100);

        match attribute {
            "strength" => self.strength = new_level,
            "endurance" => self.endurance = new_level,
            "agility" => self.agility = new_level,
            _ => {}
        }

        Ok((gain, new_level, fatigue))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FitnessWorld {
    pub fitness: Fitness,
}

impl Default for FitnessWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl FitnessWorld {
    pub fn new() -> Self {
        Self { fitness: Fitness::default() }
    }

    pub fn parse(val: Option<&Value>) -> Result<Self> {
        let fitness =
            if let Some(v) = val.filter(|x| **x != Value::Null) { Fitness::parse(v)? } else { Fitness::default() };
        Ok(Self { fitness })
    }

    pub fn to_value(&self) -> Value {
        self.fitness.to_value()
    }

    pub fn from_value(v: &Value) -> Result<Self> {
        Self::parse(Some(v))
    }

    pub fn op_assess(
        &self,
        fitness_override: Option<&Value>,
        health: Option<&Value>,
        needs: Option<&Value>,
        load: u64,
    ) -> Result<Value> {
        let fit = if let Some(fv) = fitness_override.filter(|x| **x != Value::Null) {
            Fitness::parse(fv)?
        } else {
            self.fitness
        };

        let h_status = if let Some(hv) = health.filter(|x| **x != Value::Null) {
            if let Some(obj_val) = hv.as_object() {
                obj_val.iter().find(|(k, _)| k == "status").and_then(|(_, v)| v.as_str())
            } else {
                hv.as_str()
            }
        } else {
            None
        };

        let eff = fit.compute_effective(h_status, needs)?;
        let cap = eff.carry_capacity();
        let speed = eff.travel_speed(load);
        let fatigue_rate = (100 * 100) / (eff.endurance + eff.condition / 2 + 10);

        Ok(obj! {
            "ok" => true,
            "strength" => fit.strength,
            "endurance" => fit.endurance,
            "agility" => fit.agility,
            "condition" => fit.condition,
            "effective" => obj! {
                "strength" => eff.strength,
                "endurance" => eff.endurance,
                "agility" => eff.agility,
                "condition" => eff.condition,
            },
            "carry_capacity" => cap,
            "travel_speed" => speed,
            "fatigue_rate" => fatigue_rate,
        })
    }

    pub fn op_travel_cost(
        &self,
        distance: u64,
        fitness_override: Option<&Value>,
        load: u64,
        health: Option<&Value>,
        needs: Option<&Value>,
    ) -> Result<Value> {
        let fit = if let Some(fv) = fitness_override.filter(|x| **x != Value::Null) {
            Fitness::parse(fv)?
        } else {
            self.fitness
        };

        let h_status = if let Some(hv) = health.filter(|x| **x != Value::Null) {
            if let Some(obj_val) = hv.as_object() {
                obj_val.iter().find(|(k, _)| k == "status").and_then(|(_, v)| v.as_str())
            } else {
                hv.as_str()
            }
        } else {
            None
        };

        let eff = fit.compute_effective(h_status, needs)?;
        let (speed, time, fatigue) = eff.travel_cost(distance, load);

        Ok(obj! {
            "ok" => true,
            "distance" => distance,
            "current_load" => load,
            "travel_speed" => speed,
            "travel_time" => time,
            "fatigue_cost" => fatigue,
        })
    }

    pub fn op_can_carry(
        &self,
        current_load: u64,
        add_weight: u64,
        fitness_override: Option<&Value>,
        health: Option<&Value>,
        needs: Option<&Value>,
    ) -> Result<Value> {
        let fit = if let Some(fv) = fitness_override.filter(|x| **x != Value::Null) {
            Fitness::parse(fv)?
        } else {
            self.fitness
        };

        let h_status = if let Some(hv) = health.filter(|x| **x != Value::Null) {
            if let Some(obj_val) = hv.as_object() {
                obj_val.iter().find(|(k, _)| k == "status").and_then(|(_, v)| v.as_str())
            } else {
                hv.as_str()
            }
        } else {
            None
        };

        let eff = fit.compute_effective(h_status, needs)?;
        let (can_carry, cap, total, remaining) = eff.can_carry(current_load, add_weight);

        Ok(obj! {
            "ok" => true,
            "can_carry" => can_carry,
            "carry_capacity" => cap,
            "current_load" => total,
            "remaining" => Value::Number(remaining.to_string()),
        })
    }

    pub fn op_train(
        &mut self,
        at: u64,
        actor: &str,
        attribute: &str,
        effort: u64,
        fitness_override: Option<&Value>,
        opt_place: Option<&str>,
    ) -> Result<Value> {
        ident_bound(&Value::String(actor.to_string()))?;
        if let Some(p) = opt_place {
            ident_bound(&Value::String(p.to_string()))?;
        }

        let is_override = fitness_override.filter(|x| **x != Value::Null).is_some();
        let mut fit = if let Some(fv) = fitness_override.filter(|x| **x != Value::Null) {
            Fitness::parse(fv)?
        } else {
            self.fitness
        };

        let prev = match attribute {
            "strength" => fit.strength,
            "endurance" => fit.endurance,
            "agility" => fit.agility,
            _ => return Err(c::bad()),
        };

        let (gain, new_level, fatigue) = fit.train(attribute, effort)?;

        if !is_override {
            self.fitness = fit;
        }

        let place_str = opt_place.unwrap_or("");
        let entry = obj! {
            "at" => at,
            "kind" => "fitness_training",
            "summary" => format!("{actor} trained {attribute} (+{gain} to {new_level})."),
            "actors" => Value::Array(vec![Value::String(actor.to_string())]),
            "place" => place_str.to_string(),
            "things" => Value::Array(Vec::new()),
            "tags" => Value::Array(vec![
                Value::String("fitness".to_string()),
                Value::String("training".to_string()),
                Value::String(attribute.to_string()),
            ]),
            "visibility" => "public",
            "witnesses" => Value::Array(Vec::new()),
            "phrases" => Value::Array(vec![
                Value::String(actor.to_string()),
                Value::String(attribute.to_string()),
                Value::String("training".to_string()),
            ]),
            "facts" => obj! {
                "actor" => actor.to_string(),
                "attribute" => attribute.to_string(),
                "previous" => prev,
                "gain" => gain,
                "new_level" => new_level,
                "effort" => effort,
                "fatigue" => fatigue,
            },
        };

        Ok(obj! {
            "ok" => true,
            "actor" => actor.to_string(),
            "attribute" => attribute.to_string(),
            "previous" => prev,
            "gain" => gain,
            "new_level" => new_level,
            "fatigue" => fatigue,
            "fitness" => fit.to_value(),
            "entry" => entry,
        })
    }

    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
        keys(req, &["ops"], &["fitness"])?;
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        if req.get("fitness").is_some() {
            self.fitness = Fitness::parse(c::get(req, "fitness")?)?;
        }

        let mut results = Vec::new();
        for op_call in ops {
            keys(op_call, &["op"], &["at", "args"])?;
            let op_name = text_bound(c::get(op_call, "op")?, 32)?;
            let at = op_call
                .get("at")
                .filter(|x| **x != Value::Null)
                .map(|v| uint_bound(v, 0, u64::MAX))
                .transpose()?
                .unwrap_or(0);
            let empty_args = Value::Object(Vec::new());
            let args = op_call.get("args").filter(|x| **x != Value::Null).unwrap_or(&empty_args);

            match op_name {
                "assess" => {
                    keys(args, &[], &["fitness", "health", "needs", "load"])?;
                    let load = args
                        .get("load")
                        .filter(|x| **x != Value::Null)
                        .map(|v| uint_bound(v, 0, 10000))
                        .transpose()?
                        .unwrap_or(0);
                    let res = self.op_assess(args.get("fitness"), args.get("health"), args.get("needs"), load)?;
                    results.push(res);
                }
                "travel_cost" => {
                    keys(args, &["distance"], &["fitness", "load", "health", "needs"])?;
                    let dist = uint_bound(c::get(args, "distance")?, 0, 1_000_000)?;
                    let load = args
                        .get("load")
                        .filter(|x| **x != Value::Null)
                        .map(|v| uint_bound(v, 0, 10000))
                        .transpose()?
                        .unwrap_or(0);
                    let res =
                        self.op_travel_cost(dist, args.get("fitness"), load, args.get("health"), args.get("needs"))?;
                    results.push(res);
                }
                "can_carry" => {
                    keys(args, &["current_load"], &["add_weight", "fitness", "health", "needs"])?;
                    let cur = uint_bound(c::get(args, "current_load")?, 0, 10000)?;
                    let add = args
                        .get("add_weight")
                        .filter(|x| **x != Value::Null)
                        .map(|v| uint_bound(v, 0, 10000))
                        .transpose()?
                        .unwrap_or(0);
                    let res =
                        self.op_can_carry(cur, add, args.get("fitness"), args.get("health"), args.get("needs"))?;
                    results.push(res);
                }
                "train" => {
                    keys(args, &["actor", "attribute", "effort"], &["fitness", "place"])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let attr = text_bound(c::get(args, "attribute")?, 32)?;
                    let effort = uint_bound(c::get(args, "effort")?, 1, 100)?;
                    let place = args.get("place").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
                    let res = self.op_train(at, actor, attr, effort, args.get("fitness"), place)?;
                    results.push(res);
                }
                _ => return Err(c::bad()),
            }
        }

        Ok(obj! {
            "results" => results,
            "fitness" => self.to_value(),
        })
    }
}

pub fn workflow(a: &Value) -> Result<Value> {
    keys(a, &["ops"], &["fitness"])?;
    let mut fw = FitnessWorld::parse(a.get("fitness"))?;
    fw.workflow(a)
}
