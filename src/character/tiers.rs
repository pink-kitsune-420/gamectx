//! Level of detail and tiered simulation for large casts
//! (pass 5, stage 5, design `U`, local kind `tiers`).
use crate::character::limits::{Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, exact_fields, Persist};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "summaries", default: 1024, ceiling: 1_000_000 },
    Limit { name: "tiers", default: 8, ceiling: 16 },
];

pub const TIER_ORDER: &[&str] = &["focus", "near", "background", "dormant"];

#[derive(Clone, Debug)]
pub struct TierConfig {
    pub cost: u64,
    pub max_distance: u64,
    pub min_relevance: u64,
    pub keeps: BTreeSet<String>,
    pub drops: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct BeingTierData {
    pub tier: String,
    pub distance: u64,
    pub relevance: u64,
    pub state: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct SummaryData {
    pub tier: String,
    pub kept: BTreeMap<String, Value>,
    pub drops: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct TiersManager {
    pub limits: Limits,
    pub tiers: BTreeMap<String, TierConfig>,
    pub beings: BTreeMap<String, BeingTierData>,
    pub summaries: BTreeMap<String, SummaryData>,
}

impl Default for TiersManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TiersManager {
    pub fn new() -> Self {
        let mut tiers = BTreeMap::new();
        tiers.insert(
            "focus".to_string(),
            TierConfig {
                cost: 100,
                max_distance: 20,
                min_relevance: 500_000,
                keeps: [
                    "body", "dialogue", "intents", "location", "micro_reactions", "motives", "needs", "path",
                    "status", "tasks",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                drops: BTreeSet::new(),
            },
        );
        tiers.insert(
            "near".to_string(),
            TierConfig {
                cost: 25,
                max_distance: 100,
                min_relevance: 200_000,
                keeps: ["body", "location", "motives", "needs", "status", "tasks"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                drops: ["dialogue_buffer", "fine_path", "micro_reactions"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            },
        );
        tiers.insert(
            "background".to_string(),
            TierConfig {
                cost: 5,
                max_distance: 1000,
                min_relevance: 50_000,
                keeps: ["location", "needs", "status"].iter().map(|s| s.to_string()).collect(),
                drops: ["dialogue_buffer", "fine_path", "intents", "micro_reactions", "tasks"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            },
        );
        tiers.insert(
            "dormant".to_string(),
            TierConfig {
                cost: 0,
                max_distance: 1_000_000_000,
                min_relevance: 0,
                keeps: ["home", "id"].iter().map(|s| s.to_string()).collect(),
                drops: [
                    "dialogue_buffer", "fine_path", "intents", "location", "micro_reactions", "motives", "needs",
                    "status", "tasks",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            },
        );

        Self {
            limits: Limits::defaults(LIMITS),
            tiers,
            beings: BTreeMap::new(),
            summaries: BTreeMap::new(),
        }
    }

    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    fn make_summary(&self, target_tier: &str, full_state: &BTreeMap<String, Value>) -> SummaryData {
        let cfg = self.tiers.get(target_tier).expect("tier must exist");
        let mut kept = BTreeMap::new();
        for k in &cfg.keeps {
            if let Some(val) = full_state.get(k) {
                kept.insert(k.clone(), val.clone());
            }
        }
        let mut drops = BTreeSet::new();
        for d in &cfg.drops {
            if full_state.contains_key(d) {
                drops.insert(d.clone());
            }
        }
        SummaryData {
            tier: target_tier.to_string(),
            kept,
            drops,
        }
    }

    fn target_tier_for(&self, distance: u64, relevance: u64) -> &str {
        for &t in TIER_ORDER {
            if let Some(cfg) = self.tiers.get(t) {
                if distance <= cfg.max_distance || relevance >= cfg.min_relevance {
                    return t;
                }
            }
        }
        "dormant"
    }

    pub fn op_tier(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["name", "cost", "max_distance", "min_relevance", "keeps", "drops"], &[])?;
        let tname = ident(c::get(args, "name")?)?.to_string();
        let cost = c::uint(c::get(args, "cost")?, 0, 10_000)?;
        let max_dist = c::uint(c::get(args, "max_distance")?, 0, 1_000_000_000)?;
        let min_rel = c::uint(c::get(args, "min_relevance")?, 0, 1_000_000)?;

        let mut keeps = BTreeSet::new();
        for k in c::list(c::get(args, "keeps")?)? {
            let s = c::text(k)?;
            if s.is_empty() || s.len() > 64 {
                return Err(c::bad());
            }
            keeps.insert(s.to_string());
        }

        let mut drops = BTreeSet::new();
        for d in c::list(c::get(args, "drops")?)? {
            let s = c::text(d)?;
            if s.is_empty() || s.len() > 64 {
                return Err(c::bad());
            }
            drops.insert(s.to_string());
        }

        if !self.tiers.contains_key(&tname) && self.tiers.len() + 1 > self.limits.get("tiers") {
            return Err(c::bad());
        }

        let keeps_arr: Vec<Value> = keeps.iter().cloned().map(Value::str).collect();
        let drops_arr: Vec<Value> = drops.iter().cloned().map(Value::str).collect();

        self.tiers.insert(
            tname.clone(),
            TierConfig {
                cost,
                max_distance: max_dist,
                min_relevance: min_rel,
                keeps,
                drops,
            },
        );

        Ok(obj! {
            "cost" => cost,
            "drops" => Value::Array(drops_arr),
            "keeps" => Value::Array(keeps_arr),
            "max_distance" => max_dist,
            "min_relevance" => min_rel,
            "name" => tname,
        })
    }

    pub fn op_track(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["tier", "distance", "relevance", "state", "summary"])?;
        let bid = ident(c::get(args, "being")?)?.to_string();

        let tier = match c::get(args, "tier") {
            Ok(v) if v != &Value::Null => ident(v)?.to_string(),
            _ => "focus".to_string(),
        };
        if !self.tiers.contains_key(&tier) {
            return Err(c::bad());
        }

        let distance = match c::get(args, "distance") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, 1_000_000_000)?,
            _ => 0,
        };

        let relevance = match c::get(args, "relevance") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, 1_000_000)?,
            _ => 500_000,
        };

        let mut state = BTreeMap::new();
        if let Ok(st_val) = c::get(args, "state") {
            if st_val != &Value::Null {
                let obj_map = st_val.as_object().ok_or_else(c::bad)?;
                for (k, v) in obj_map {
                    state.insert(k.clone(), v.clone());
                }
            }
        }

        if !self.beings.contains_key(&bid) && self.beings.len() + 1 > self.limits.get("beings") {
            return Err(c::bad());
        }

        if let Ok(sum_val) = c::get(args, "summary") {
            if sum_val != &Value::Null {
                let s_obj = sum_val.as_object().ok_or_else(c::bad)?;
                let mut kept = BTreeMap::new();
                for (k, v) in s_obj {
                    kept.insert(k.clone(), v.clone());
                }
                if !self.summaries.contains_key(&bid) && self.summaries.len() + 1 > self.limits.get("summaries") {
                    return Err(c::bad());
                }
                self.summaries.insert(
                    bid.clone(),
                    SummaryData {
                        tier: tier.clone(),
                        kept,
                        drops: BTreeSet::new(),
                    },
                );
            }
        } else if tier != "focus" {
            let sum_data = self.make_summary(&tier, &state);
            if !self.summaries.contains_key(&bid) && self.summaries.len() + 1 > self.limits.get("summaries") {
                return Err(c::bad());
            }
            self.summaries.insert(bid.clone(), sum_data);
        }

        self.beings.insert(
            bid.clone(),
            BeingTierData {
                tier: tier.clone(),
                distance,
                relevance,
                state,
            },
        );

        Ok(obj! {
            "being" => bid,
            "distance" => distance,
            "relevance" => relevance,
            "tier" => tier,
        })
    }

    pub fn op_demote(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "to"], &[])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        let to_tier = ident(c::get(args, "to")?)?.to_string();

        let (from_tier, summary) = {
            let b_data = self.beings.get(&bid).ok_or_else(c::bad)?;
            if !self.tiers.contains_key(&to_tier) {
                return Err(c::bad());
            }
            (b_data.tier.clone(), self.make_summary(&to_tier, &b_data.state))
        };

        if !self.summaries.contains_key(&bid) && self.summaries.len() + 1 > self.limits.get("summaries") {
            return Err(c::bad());
        }

        let dropped_arr: Vec<Value> = summary.drops.iter().cloned().map(Value::str).collect();
        let kept_obj = Value::Object(summary.kept.iter().map(|(k, v)| (k.clone(), v.clone())).collect());

        self.summaries.insert(bid.clone(), summary);
        self.beings.get_mut(&bid).unwrap().tier = to_tier.clone();

        Ok(obj! {
            "being" => bid,
            "dropped" => Value::Array(dropped_arr),
            "from" => from_tier,
            "summary" => kept_obj,
            "to" => to_tier,
        })
    }

    pub fn op_promote(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "to"], &["elapsed", "seed"])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        let to_tier = ident(c::get(args, "to")?)?.to_string();
        if !self.tiers.contains_key(&to_tier) {
            return Err(c::bad());
        }

        let elapsed = match c::get(args, "elapsed") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, 1_000_000_000)?,
            _ => 0,
        };

        let _seed = match c::get(args, "seed") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, 1_000_000_000)?,
            _ => 0,
        };

        let b_data = self.beings.get_mut(&bid).ok_or_else(c::bad)?;
        let from_tier = b_data.tier.clone();

        let kept_copy = match self.summaries.get(&bid) {
            Some(s) => s.kept.clone(),
            None => BTreeMap::new(),
        };

        for (k, v) in &kept_copy {
            b_data.state.insert(k.clone(), v.clone());
        }

        // Field agreement check: any field in kept summary must agree with b_data.state
        for (k, v) in &kept_copy {
            if b_data.state.get(k) != Some(v) {
                return Err(c::bad());
            }
        }

        if elapsed > 0 {
            if let Some(needs_val) = b_data.state.get_mut("needs") {
                if let Value::Object(ref mut need_rows) = needs_val {
                    for (_nid, n_val) in need_rows.iter_mut() {
                        if let Value::Object(ref mut fields) = n_val {
                            let mut rate: Option<u64> = None;
                            let mut level: Option<u64> = None;
                            for (fk, fv) in fields.iter() {
                                if fk == "rate" {
                                    rate = c::uint(fv, 0, 1_000_000_000).ok();
                                } else if fk == "level" {
                                    level = c::uint(fv, 0, 1_000_000_000).ok();
                                }
                            }
                            if let (Some(r), Some(l)) = (rate, level) {
                                let growth = (elapsed * r) / 1000;
                                let new_l = (l + growth).min(1_000_000);
                                for (fk, fv) in fields.iter_mut() {
                                    if fk == "level" {
                                        *fv = Value::from(new_l);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        b_data.tier = to_tier.clone();

        let restored_fields: Vec<Value> = b_data.state.keys().cloned().map(Value::str).collect();

        Ok(obj! {
            "being" => bid,
            "elapsed" => elapsed,
            "from" => from_tier,
            "restored_fields" => Value::Array(restored_fields),
            "to" => to_tier,
        })
    }

    pub fn op_assess(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["budget"], &["beings"])?;
        let budget = c::uint(c::get(args, "budget")?, 0, 1_000_000_000)?;

        if let Ok(updates_val) = c::get(args, "beings") {
            if updates_val != &Value::Null {
                for u in c::list(updates_val)? {
                    keys(u, &["being"], &["distance", "relevance"])?;
                    let u_id = ident(c::get(u, "being")?)?;
                    let b = self.beings.get_mut(u_id).ok_or_else(c::bad)?;
                    if let Ok(d) = c::get(u, "distance") {
                        if d != &Value::Null {
                            b.distance = c::uint(d, 0, 1_000_000_000)?;
                        }
                    }
                    if let Ok(r) = c::get(u, "relevance") {
                        if r != &Value::Null {
                            b.relevance = c::uint(r, 0, 1_000_000)?;
                        }
                    }
                }
            }
        }

        // 1. Transition between tiers
        let mut promotions = Vec::new();
        let mut demotions = Vec::new();

        let all_beings: Vec<String> = self.beings.keys().cloned().collect();
        for bid in all_beings {
            let (dist, rel, cur_tier) = {
                let b = &self.beings[&bid];
                (b.distance, b.relevance, b.tier.clone())
            };
            let tgt_tier = self.target_tier_for(dist, rel).to_string();
            if tgt_tier != cur_tier {
                let cur_idx = TIER_ORDER.iter().position(|&x| x == cur_tier).unwrap_or(99);
                let tgt_idx = TIER_ORDER.iter().position(|&x| x == tgt_tier).unwrap_or(99);
                if tgt_idx < cur_idx {
                    // Promotion
                    self.op_promote(&obj! { "being" => bid.clone(), "to" => tgt_tier.clone() })?;
                    promotions.push(obj! {
                        "being" => bid.clone(),
                        "from" => cur_tier,
                        "to" => tgt_tier,
                    });
                } else {
                    // Demotion
                    let d_res = self.op_demote(&obj! { "being" => bid.clone(), "to" => tgt_tier.clone() })?;
                    let dropped = c::get(&d_res, "dropped")?.clone();
                    demotions.push(obj! {
                        "being" => bid.clone(),
                        "dropped" => dropped,
                        "from" => cur_tier,
                        "to" => tgt_tier,
                    });
                }
            }
        }

        // 2. Budget allocation
        let mut sorted_beings: Vec<(usize, i64, String)> = self
            .beings
            .iter()
            .map(|(bid, b)| {
                let tier_idx = TIER_ORDER.iter().position(|&x| x == b.tier).unwrap_or(99);
                (tier_idx, -(b.relevance as i64), bid.clone())
            })
            .collect();
        sorted_beings.sort_unstable();

        let mut budget_remaining = budget;
        let mut active = Vec::new();
        let mut deferred = Vec::new();

        for (_, _, bid) in sorted_beings {
            let tier = &self.beings[&bid].tier;
            let cost = self.tiers[tier].cost;
            if budget_remaining >= cost {
                budget_remaining -= cost;
                active.push(Value::str(bid));
            } else {
                deferred.push(Value::str(bid));
            }
        }

        Ok(obj! {
            "active" => Value::Array(active),
            "budget_remaining" => budget_remaining,
            "budget_used" => budget - budget_remaining,
            "deferred" => Value::Array(deferred),
            "demotions" => Value::Array(demotions),
            "promotions" => Value::Array(promotions),
        })
    }

    pub fn settle_state(&mut self) -> Result<()> {
        if self.tiers.len() > self.limits.get("tiers") {
            return Err(c::bad());
        }
        if self.beings.len() > self.limits.get("beings") {
            return Err(c::bad());
        }
        if self.summaries.len() > self.limits.get("summaries") {
            return Err(c::bad());
        }
        for (bid, s) in &self.summaries {
            if !self.beings.contains_key(bid) {
                return Err(c::bad());
            }
            if !self.tiers.contains_key(&s.tier) {
                return Err(c::bad());
            }
        }
        for b in self.beings.values() {
            if !self.tiers.contains_key(&b.tier) {
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
            let empty = obj! {};
            let args = c::get(item, "args").unwrap_or(&empty);
            let res = match op_name {
                "tier" => self.op_tier(args)?,
                "track" => self.op_track(args)?,
                "promote" => self.op_promote(args)?,
                "demote" => self.op_demote(args)?,
                "assess" => self.op_assess(args)?,
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
                        ("summaries".to_string(), Value::from(self.limits.get("summaries") as u64)),
                        ("tiers".to_string(), Value::from(self.limits.get("tiers") as u64)),
                    ]),
                ));
            }
        }
        Ok(out)
    }
}

impl Persist for TiersManager {
    fn save(&self) -> Value {
        let tiers_arr: Vec<Value> = self
            .tiers
            .iter()
            .map(|(k, v)| {
                Value::Array(vec![
                    Value::str(k.clone()),
                    Value::from(v.cost),
                    Value::from(v.max_distance),
                    Value::from(v.min_relevance),
                    Value::Array(v.keeps.iter().cloned().map(Value::str).collect()),
                    Value::Array(v.drops.iter().cloned().map(Value::str).collect()),
                ])
            })
            .collect();

        let beings_arr: Vec<Value> = self
            .beings
            .iter()
            .map(|(k, v)| {
                let st_obj = Value::Object(v.state.iter().map(|(sk, sv)| (sk.clone(), sv.clone())).collect());
                Value::Array(vec![
                    Value::str(k.clone()),
                    Value::str(v.tier.clone()),
                    Value::from(v.distance),
                    Value::from(v.relevance),
                    st_obj,
                ])
            })
            .collect();

        let summaries_arr: Vec<Value> = self
            .summaries
            .iter()
            .map(|(k, v)| {
                let kept_obj = Value::Object(v.kept.iter().map(|(sk, sv)| (sk.clone(), sv.clone())).collect());
                Value::Array(vec![
                    Value::str(k.clone()),
                    Value::str(v.tier.clone()),
                    kept_obj,
                    Value::Array(v.drops.iter().cloned().map(Value::str).collect()),
                ])
            })
            .collect();

        obj! {
            "beings" => Value::Array(beings_arr),
            "summaries" => Value::Array(summaries_arr),
            "tiers" => Value::Array(tiers_arr),
        }
    }

    fn load(v: &Value) -> Result<Self> {
        exact_fields(v, &["beings", "summaries", "tiers"])?;

        let mut mgr = Self::new();

        let t_list = c::list(c::get(v, "tiers")?)?;
        if t_list.len() > mgr.limits.get("tiers") {
            return Err(c::bad());
        }
        let mut new_tiers = BTreeMap::new();
        let mut last_t: Option<&str> = None;
        for row_val in t_list {
            let row = c::list(row_val)?;
            if row.len() != 6 {
                return Err(c::bad());
            }
            let tname = ident(&row[0])?;
            if let Some(prev) = last_t {
                if tname <= prev {
                    return Err(c::bad());
                }
            }
            last_t = Some(tname);

            let cost = c::uint(&row[1], 0, 10_000)?;
            let max_dist = c::uint(&row[2], 0, 1_000_000_000)?;
            let min_rel = c::uint(&row[3], 0, 1_000_000)?;

            let mut keeps = BTreeSet::new();
            for k in c::list(&row[4])? {
                let s = c::text(k)?;
                if s.is_empty() || s.len() > 64 {
                    return Err(c::bad());
                }
                keeps.insert(s.to_string());
            }

            let mut drops = BTreeSet::new();
            for d in c::list(&row[5])? {
                let s = c::text(d)?;
                if s.is_empty() || s.len() > 64 {
                    return Err(c::bad());
                }
                drops.insert(s.to_string());
            }

            new_tiers.insert(
                tname.to_string(),
                TierConfig {
                    cost,
                    max_distance: max_dist,
                    min_relevance: min_rel,
                    keeps,
                    drops,
                },
            );
        }

        let b_list = c::list(c::get(v, "beings")?)?;
        if b_list.len() > mgr.limits.get("beings") {
            return Err(c::bad());
        }
        let mut new_beings = BTreeMap::new();
        let mut last_b: Option<&str> = None;
        for row_val in b_list {
            let row = c::list(row_val)?;
            if row.len() != 5 {
                return Err(c::bad());
            }
            let bid = ident(&row[0])?;
            if let Some(prev) = last_b {
                if bid <= prev {
                    return Err(c::bad());
                }
            }
            last_b = Some(bid);

            let tier = ident(&row[1])?;
            if !new_tiers.contains_key(tier) {
                return Err(c::bad());
            }
            let distance = c::uint(&row[2], 0, 1_000_000_000)?;
            let relevance = c::uint(&row[3], 0, 1_000_000)?;
            let st_obj = row[4].as_object().ok_or_else(c::bad)?;
            let mut state = BTreeMap::new();
            for (k, sv) in st_obj {
                state.insert(k.clone(), sv.clone());
            }

            new_beings.insert(
                bid.to_string(),
                BeingTierData {
                    tier: tier.to_string(),
                    distance,
                    relevance,
                    state,
                },
            );
        }

        let s_list = c::list(c::get(v, "summaries")?)?;
        if s_list.len() > mgr.limits.get("summaries") {
            return Err(c::bad());
        }
        let mut new_summaries = BTreeMap::new();
        let mut last_s: Option<&str> = None;
        for row_val in s_list {
            let row = c::list(row_val)?;
            if row.len() != 4 {
                return Err(c::bad());
            }
            let bid = ident(&row[0])?;
            if let Some(prev) = last_s {
                if bid <= prev {
                    return Err(c::bad());
                }
            }
            last_s = Some(bid);

            let tier = ident(&row[1])?;
            if !new_tiers.contains_key(tier) {
                return Err(c::bad());
            }

            let kept_obj = row[2].as_object().ok_or_else(c::bad)?;
            let mut kept = BTreeMap::new();
            for (k, sv) in kept_obj {
                kept.insert(k.clone(), sv.clone());
            }

            let mut drops = BTreeSet::new();
            for d in c::list(&row[3])? {
                let s = c::text(d)?;
                if s.is_empty() || s.len() > 64 {
                    return Err(c::bad());
                }
                drops.insert(s.to_string());
            }

            new_summaries.insert(
                bid.to_string(),
                SummaryData {
                    tier: tier.to_string(),
                    kept,
                    drops,
                },
            );
        }

        mgr.tiers = new_tiers;
        mgr.beings = new_beings;
        mgr.summaries = new_summaries;
        mgr.settle_state()?;
        Ok(mgr)
    }
}

durable!(TiersManager, "tiers", LIMITS);

pub fn workflow(request: &Value) -> Result<Value> {
    TiersManager::new().workflow(request)
}
