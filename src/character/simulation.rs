//! Mental simulation, bounded Theory of Mind, persuasion, and bargaining
//! (pass 5, stage 8, design `S`, local kind `simulation`).
use crate::character::limits::{Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, exact_fields, Persist};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "copy_size", default: 32, ceiling: 256 },
    Limit { name: "rounds_max", default: 10, ceiling: 30 },
    Limit { name: "sim_k", default: 3, ceiling: 8 },
];

pub const PERSUASION_KINDS: &[&str] = &[
    "appeal_values",
    "authority",
    "emotion",
    "evidence",
    "reciprocity",
    "threat",
];

fn int_range(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(c::bad),
        _ => Err(c::bad()),
    }
}

#[derive(Clone, Debug)]
pub struct BeliefCopy {
    pub holder: String,
    pub target: String,
    pub carefulness: u64,
    pub trust: u64,
    pub status: u64,
    pub shared_values: BTreeSet<String>,
    pub distortions: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct SimulationManager {
    pub limits: Limits,
    pub copies: BTreeMap<(String, String), BeliefCopy>,
    pub beings: BTreeSet<String>,
}

impl Default for SimulationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationManager {
    pub fn new() -> Self {
        Self {
            limits: Limits::defaults(LIMITS),
            copies: BTreeMap::new(),
            beings: BTreeSet::new(),
        }
    }

    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    fn ensure_being(&mut self, bid: &str) -> Result<()> {
        if !self.beings.contains(bid) {
            if self.beings.len() + 1 > self.limits.get("beings") {
                return Err(c::bad());
            }
            self.beings.insert(bid.to_string());
        }
        Ok(())
    }

    pub fn op_copy(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["holder", "target"], &["carefulness", "distortions", "shared_values", "status", "trust"])?;
        let holder = ident(c::get(args, "holder")?)?.to_string();
        let target = ident(c::get(args, "target")?)?.to_string();
        if holder == target {
            return Err(c::bad());
        }
        self.ensure_being(&holder)?;
        self.ensure_being(&target)?;

        let carefulness = match c::get(args, "carefulness") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, 100)?,
            _ => 50,
        };
        let trust = match c::get(args, "trust") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, 100)?,
            _ => 50,
        };
        let status = match c::get(args, "status") {
            Ok(v) if v != &Value::Null => c::uint(v, 1, 10)?,
            _ => 5,
        };

        let max_c = self.limits.get("copy_size");

        let mut shared_values = BTreeSet::new();
        if let Ok(v) = c::get(args, "shared_values") {
            if v != &Value::Null {
                let list = c::list(v)?;
                if list.len() > max_c {
                    return Err(c::bad());
                }
                for item in list {
                    let s = c::text(item)?;
                    if s.is_empty() || s.len() > 32 {
                        return Err(c::bad());
                    }
                    shared_values.insert(s.to_ascii_lowercase());
                }
            }
        }

        let mut distortions = BTreeSet::new();
        if let Ok(v) = c::get(args, "distortions") {
            if v != &Value::Null {
                let list = c::list(v)?;
                if list.len() > max_c {
                    return Err(c::bad());
                }
                for item in list {
                    let s = c::text(item)?;
                    if s.is_empty() || s.len() > 32 {
                        return Err(c::bad());
                    }
                    distortions.insert(s.to_ascii_lowercase());
                }
            }
        }

        let sv_arr: Vec<Value> = shared_values.iter().cloned().map(Value::str).collect();
        let d_arr: Vec<Value> = distortions.iter().cloned().map(Value::str).collect();

        self.copies.insert(
            (holder.clone(), target.clone()),
            BeliefCopy {
                holder: holder.clone(),
                target: target.clone(),
                carefulness,
                trust,
                status,
                shared_values,
                distortions,
            },
        );

        Ok(obj! {
            "carefulness" => Value::from(carefulness),
            "distortions" => Value::Array(d_arr),
            "holder" => holder,
            "shared_values" => Value::Array(sv_arr),
            "status" => Value::from(status),
            "target" => target,
            "trust" => Value::from(trust),
        })
    }

    fn calc_shift(
        kind: &str,
        value: Option<&str>,
        carefulness: u64,
        trust: u64,
        speaker_st: u64,
        target_st: u64,
        shared_values: &BTreeSet<String>,
        distortions: &BTreeSet<String>,
    ) -> Result<(i64, bool)> {
        if let Some(v) = value {
            if distortions.contains(v) {
                return Ok((-10, true));
            }
        }

        let base: i64 = match kind {
            "evidence" => 15 + ((carefulness as i64 * 15) / 100),
            "authority" => {
                let ratio = (((speaker_st * 2) / (target_st + 1)).max(1)).min(3) as i64;
                10 + ratio * 8
            }
            "appeal_values" => {
                if let Some(v) = value {
                    if shared_values.contains(v) { 25 } else { 5 }
                } else {
                    5
                }
            }
            "emotion" => 10 + (((100 - carefulness as i64) * 20) / 100),
            "reciprocity" => 15 + ((trust as i64 * 15) / 100),
            "threat" => 25,
            _ => return Err(c::bad()),
        };

        let scaled = (base * (50 + (trust as i64 / 2))) / 100;
        Ok((scaled, false))
    }

    pub fn op_simulate(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["candidates", "speaker", "target", "tom"], &["speaker_status"])?;
        let speaker = ident(c::get(args, "speaker")?)?.to_string();
        let target = ident(c::get(args, "target")?)?.to_string();
        if speaker == target {
            return Err(c::bad());
        }
        self.ensure_being(&speaker)?;
        self.ensure_being(&target)?;

        let tom = c::uint(c::get(args, "tom")?, 0, 2)?;
        if tom < 2 {
            return Err(c::bad());
        }

        let speaker_st = match c::get(args, "speaker_status") {
            Ok(v) if v != &Value::Null => c::uint(v, 1, 10)?,
            _ => 5,
        };

        let raw_cands = c::list(c::get(args, "candidates")?)?;
        let max_k = self.limits.get("sim_k");
        if raw_cands.is_empty() || raw_cands.len() > max_k {
            return Err(c::bad());
        }

        let default_copy = BeliefCopy {
            holder: speaker.clone(),
            target: target.clone(),
            carefulness: 50,
            trust: 50,
            status: 5,
            shared_values: BTreeSet::new(),
            distortions: BTreeSet::new(),
        };

        let copy_entry = self.copies.get(&(speaker.clone(), target.clone())).unwrap_or(&default_copy);

        let mut predictions = Vec::new();
        let mut best_idx = 0;
        let mut best_score = -999i64;

        for (i, c_val) in raw_cands.iter().enumerate() {
            keys(c_val, &["kind"], &["value"])?;
            let k_name = c::text(c::get(c_val, "kind")?)?.to_ascii_lowercase();
            if !PERSUASION_KINDS.contains(&k_name.as_str()) {
                return Err(c::bad());
            }

            let val_opt = match c::get(c_val, "value") {
                Ok(v) if v != &Value::Null => {
                    let s = c::text(v)?;
                    if s.is_empty() || s.len() > 32 {
                        return Err(c::bad());
                    }
                    Some(s.to_ascii_lowercase())
                }
                _ => None,
            };

            let (shift, def_trig) = Self::calc_shift(
                &k_name,
                val_opt.as_deref(),
                copy_entry.carefulness,
                copy_entry.trust,
                speaker_st,
                copy_entry.status,
                &copy_entry.shared_values,
                &copy_entry.distortions,
            )?;

            let mut pred_item = vec![
                (String::from("defense_triggered"), Value::from(def_trig)),
                (String::from("kind"), Value::str(k_name.clone())),
                (String::from("predicted_shift"), Value::from(shift)),
            ];
            if let Some(ref v_str) = val_opt {
                pred_item.push((String::from("value"), Value::str(v_str.clone())));
            }
            pred_item.sort_by(|a, b| a.0.cmp(&b.0));
            predictions.push(Value::Object(pred_item));

            if shift > best_score {
                best_score = shift;
                best_idx = i;
            }
        }

        let chosen_kind = predictions[best_idx].get("kind").cloned().unwrap();

        Ok(obj! {
            "best_index" => Value::from(best_idx as u64),
            "chosen_kind" => chosen_kind,
            "predictions" => Value::Array(predictions),
            "speaker" => speaker,
            "target" => target,
        })
    }

    pub fn op_persuade(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["kind", "speaker", "target"], &["carefulness", "distortions", "shared_values", "speaker_status", "target_status", "trust", "value"])?;
        let speaker = ident(c::get(args, "speaker")?)?.to_string();
        let target = ident(c::get(args, "target")?)?.to_string();
        if speaker == target {
            return Err(c::bad());
        }
        self.ensure_being(&speaker)?;
        self.ensure_being(&target)?;

        let k_name = c::text(c::get(args, "kind")?)?.to_ascii_lowercase();
        if !PERSUASION_KINDS.contains(&k_name.as_str()) {
            return Err(c::bad());
        }

        let val_opt = match c::get(args, "value") {
            Ok(v) if v != &Value::Null => {
                let s = c::text(v)?;
                if s.is_empty() || s.len() > 32 {
                    return Err(c::bad());
                }
                Some(s.to_ascii_lowercase())
            }
            _ => None,
        };

        let carefulness = match c::get(args, "carefulness") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, 100)?,
            _ => 50,
        };
        let trust = match c::get(args, "trust") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, 100)?,
            _ => 50,
        };
        let speaker_st = match c::get(args, "speaker_status") {
            Ok(v) if v != &Value::Null => c::uint(v, 1, 10)?,
            _ => 5,
        };
        let target_st = match c::get(args, "target_status") {
            Ok(v) if v != &Value::Null => c::uint(v, 1, 10)?,
            _ => 5,
        };

        let mut shared_values = BTreeSet::new();
        if let Ok(v) = c::get(args, "shared_values") {
            if v != &Value::Null {
                for item in c::list(v)? {
                    let s = c::text(item)?;
                    if s.is_empty() || s.len() > 32 {
                        return Err(c::bad());
                    }
                    shared_values.insert(s.to_ascii_lowercase());
                }
            }
        }

        let mut distortions = BTreeSet::new();
        if let Ok(v) = c::get(args, "distortions") {
            if v != &Value::Null {
                for item in c::list(v)? {
                    let s = c::text(item)?;
                    if s.is_empty() || s.len() > 32 {
                        return Err(c::bad());
                    }
                    distortions.insert(s.to_ascii_lowercase());
                }
            }
        }

        let (shift, def_trig) = Self::calc_shift(
            &k_name,
            val_opt.as_deref(),
            carefulness,
            trust,
            speaker_st,
            target_st,
            &shared_values,
            &distortions,
        )?;

        let strain: u64 = if k_name == "threat" { 25 } else if def_trig { 15 } else { 0 };

        Ok(obj! {
            "defense_triggered" => def_trig,
            "kind" => k_name,
            "shift" => Value::from(shift),
            "speaker" => speaker,
            "strain" => Value::from(strain),
            "target" => target,
        })
    }

    pub fn op_verify(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["actual_shift", "predicted_shift", "speaker", "target"], &[])?;
        let speaker = ident(c::get(args, "speaker")?)?.to_string();
        let target = ident(c::get(args, "target")?)?.to_string();
        self.ensure_being(&speaker)?;
        self.ensure_being(&target)?;

        let pred = int_range(c::get(args, "predicted_shift")?, -100, 100)?;
        let act = int_range(c::get(args, "actual_shift")?, -100, 100)?;
        let diff = (pred - act).abs() as u64;

        let confirmed = diff <= 10;
        let lesson = if !confirmed {
            obj! {
                "error" => Value::from(diff),
                "kind" => "wrong_prediction",
                "target" => target.clone(),
            }
        } else {
            Value::Null
        };

        Ok(obj! {
            "actual_shift" => Value::from(act),
            "confirmed" => confirmed,
            "diff" => Value::from(diff),
            "lesson" => lesson,
            "predicted_shift" => Value::from(pred),
            "speaker" => speaker,
            "target" => target,
        })
    }

    pub fn op_bargain(&mut self, args: &Value) -> Result<Value> {
        keys(
            args,
            &["buyer", "buyer_offer", "buyer_reservation", "round", "seller", "seller_offer", "seller_reservation"],
            &["bluff", "bluff_detected", "buyer_patience", "seller_patience"],
        )?;
        let buyer = ident(c::get(args, "buyer")?)?.to_string();
        let seller = ident(c::get(args, "seller")?)?.to_string();
        if buyer == seller {
            return Err(c::bad());
        }
        self.ensure_being(&buyer)?;
        self.ensure_being(&seller)?;

        let rnd = c::uint(c::get(args, "round")?, 1, self.limits.get("rounds_max") as u64)?;
        let b_offer = c::uint(c::get(args, "buyer_offer")?, 0, 1_000_000)?;
        let s_offer = c::uint(c::get(args, "seller_offer")?, 0, 1_000_000)?;
        let b_res = c::uint(c::get(args, "buyer_reservation")?, 0, 1_000_000)?;
        let s_res = c::uint(c::get(args, "seller_reservation")?, 0, 1_000_000)?;

        let b_pat = match c::get(args, "buyer_patience") {
            Ok(v) if v != &Value::Null => c::uint(v, 1, 10)?,
            _ => 5,
        };
        let s_pat = match c::get(args, "seller_patience") {
            Ok(v) if v != &Value::Null => c::uint(v, 1, 10)?,
            _ => 5,
        };

        let bluff = match c::get(args, "bluff") {
            Ok(v) if v != &Value::Null => c::boolean(v)?,
            _ => false,
        };
        let bluff_det = match c::get(args, "bluff_detected") {
            Ok(v) if v != &Value::Null => c::boolean(v)?,
            _ => false,
        };

        if bluff && bluff_det {
            return Ok(obj! {
                "outcome" => "impasse",
                "reason" => "bluff_called",
                "round" => Value::from(rnd),
                "settlement" => Value::Null,
            });
        }

        if b_offer >= s_offer {
            let settle = (b_offer + s_offer) / 2;
            return Ok(obj! {
                "outcome" => "agree",
                "reason" => "deal_closed",
                "round" => Value::from(rnd),
                "settlement" => Value::from(settle),
            });
        }

        if rnd >= self.limits.get("rounds_max") as u64 {
            return Ok(obj! {
                "outcome" => "impasse",
                "reason" => "max_rounds_reached",
                "round" => Value::from(rnd),
                "settlement" => Value::Null,
            });
        }

        let b_step = ((b_res.saturating_sub(b_offer.min(b_res))) / b_pat).max(1);
        let s_step = ((s_offer.max(s_res).saturating_sub(s_res)) / s_pat).max(1);

        let next_b = (b_offer + b_step).min(b_res);
        let next_s = s_offer.saturating_sub(s_step).max(s_res);

        Ok(obj! {
            "next_buyer_offer" => Value::from(next_b),
            "next_seller_offer" => Value::from(next_s),
            "outcome" => "counter",
            "reason" => "concession_made",
            "round" => Value::from(rnd),
            "settlement" => Value::Null,
        })
    }

    pub fn settle_state(&mut self) -> Result<()> {
        if self.beings.len() > self.limits.get("beings") {
            return Err(c::bad());
        }
        let max_c = self.limits.get("copy_size");
        for c in self.copies.values() {
            if c.shared_values.len() > max_c || c.distortions.len() > max_c {
                return Err(c::bad());
            }
        }
        Ok(())
    }

    pub fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &["limits"])?;
        let supplied = c::get(request, "limits").is_ok();
        if let Ok(l) = c::get(request, "limits") {
            self.limits.apply(l)?;
        }

        let ops = c::list(c::get(request, "ops")?)?;
        if ops.is_empty() || ops.len() > 64 {
            return Err(c::bad());
        }

        let mut results = Vec::new();
        for item in ops {
            keys(item, &["op"], &["args"])?;
            let op_name = c::text(c::get(item, "op")?)?;
            let args = c::get(item, "args").unwrap_or(&Value::Null);

            let res = match op_name {
                "copy" => self.op_copy(args)?,
                "simulate" => self.op_simulate(args)?,
                "persuade" => self.op_persuade(args)?,
                "verify" => self.op_verify(args)?,
                "bargain" => self.op_bargain(args)?,
                "export" => persist::export_op(self, &self.limits, args)?,
                "import" => {
                    let limits = self.limits.clone();
                    persist::import(self, &limits, args)?
                }
                _ => return Err(c::bad()),
            };
            results.push(res);
        }

        let mut out = obj! {
            "results" => Value::Array(results),
            "version" => 1,
        };
        if supplied {
            if let Value::Object(ref mut fields) = out {
                fields.insert(0, (
                    "limits".to_string(),
                    Value::Object(vec![
                        ("beings".to_string(), Value::from(self.limits.get("beings") as u64)),
                        ("copy_size".to_string(), Value::from(self.limits.get("copy_size") as u64)),
                        ("rounds_max".to_string(), Value::from(self.limits.get("rounds_max") as u64)),
                        ("sim_k".to_string(), Value::from(self.limits.get("sim_k") as u64)),
                    ]),
                ));
            }
        }
        Ok(out)
    }
}

impl Persist for SimulationManager {
    fn save(&self) -> Value {
        let mut copies_arr = Vec::new();
        for ((_, _), c) in &self.copies {
            let d_arr: Vec<Value> = c.distortions.iter().cloned().map(Value::str).collect();
            let sv_arr: Vec<Value> = c.shared_values.iter().cloned().map(Value::str).collect();
            copies_arr.push(Value::Array(vec![
                Value::from(c.carefulness),
                Value::Array(d_arr),
                Value::str(c.holder.clone()),
                Value::Array(sv_arr),
                Value::from(c.status),
                Value::str(c.target.clone()),
                Value::from(c.trust),
            ]));
        }

        let b_arr: Vec<Value> = self.beings.iter().cloned().map(Value::str).collect();

        obj! {
            "beings" => Value::Array(b_arr),
            "copies" => Value::Array(copies_arr),
        }
    }

    fn load(v: &Value) -> Result<Self> {
        exact_fields(v, &["beings", "copies"])?;
        let mut mgr = Self::new();

        let b_list = c::list(c::get(v, "beings")?)?;
        if b_list.len() > mgr.limits.get("beings") {
            return Err(c::bad());
        }
        let mut new_beings = BTreeSet::new();
        let mut last_b: Option<&str> = None;
        for b_val in b_list {
            let bid = ident(b_val)?;
            if let Some(prev) = last_b {
                if bid <= prev {
                    return Err(c::bad());
                }
            }
            last_b = Some(bid);
            new_beings.insert(bid.to_string());
        }

        let c_list = c::list(c::get(v, "copies")?)?;
        let mut new_copies = BTreeMap::new();
        let mut last_pair: Option<(&str, &str)> = None;
        let max_c = mgr.limits.get("copy_size");

        for crow_val in c_list {
            let row = c::list(crow_val)?;
            if row.len() != 7 {
                return Err(c::bad());
            }

            let carefulness = c::uint(&row[0], 0, 100)?;

            let raw_d = c::list(&row[1])?;
            if raw_d.len() > max_c {
                return Err(c::bad());
            }
            let mut distortions = BTreeSet::new();
            let mut last_d: Option<&str> = None;
            for d_val in raw_d {
                let d_str = c::text(d_val)?;
                if d_str.is_empty() || d_str.len() > 32 {
                    return Err(c::bad());
                }
                if let Some(prev) = last_d {
                    if d_str <= prev {
                        return Err(c::bad());
                    }
                }
                last_d = Some(d_str);
                distortions.insert(d_str.to_ascii_lowercase());
            }

            let holder = ident(&row[2])?;
            if !new_beings.contains(holder) {
                return Err(c::bad());
            }

            let raw_sv = c::list(&row[3])?;
            if raw_sv.len() > max_c {
                return Err(c::bad());
            }
            let mut shared_values = BTreeSet::new();
            let mut last_sv: Option<&str> = None;
            for sv_val in raw_sv {
                let sv_str = c::text(sv_val)?;
                if sv_str.is_empty() || sv_str.len() > 32 {
                    return Err(c::bad());
                }
                if let Some(prev) = last_sv {
                    if sv_str <= prev {
                        return Err(c::bad());
                    }
                }
                last_sv = Some(sv_str);
                shared_values.insert(sv_str.to_ascii_lowercase());
            }

            let status = c::uint(&row[4], 1, 10)?;
            let target = ident(&row[5])?;
            if !new_beings.contains(target) || holder == target {
                return Err(c::bad());
            }

            let pair = (holder, target);
            if let Some(prev) = last_pair {
                if pair <= prev {
                    return Err(c::bad());
                }
            }
            last_pair = Some(pair);

            let trust = c::uint(&row[6], 0, 100)?;

            new_copies.insert(
                (holder.to_string(), target.to_string()),
                BeliefCopy {
                    holder: holder.to_string(),
                    target: target.to_string(),
                    carefulness,
                    trust,
                    status,
                    shared_values,
                    distortions,
                },
            );
        }

        mgr.beings = new_beings;
        mgr.copies = new_copies;
        mgr.settle_state()?;
        Ok(mgr)
    }
}

durable!(SimulationManager, "simulation", LIMITS);

pub fn workflow(request: &Value) -> Result<Value> {
    SimulationManager::new().workflow(request)
}
