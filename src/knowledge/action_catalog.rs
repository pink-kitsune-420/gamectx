//! World action catalog: read-only description over existing operations.
//!
//! Covers 11 operational surfaces:
//! things, recipes, species, communities, inventory, fitness, props, services, reactions,
//! engine (directions/commands), initiative.
//!
//! Schema per action:
//! id, label, category, description, actor requirements, typed target slots,
//! preconditions, cost (time, fatigue, resources, coins), effects (world, body, needs, skills, relationships),
//! perception (visibility, noise, witnesses), risk, duration, interruptible.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::knowledge::action_catalog::ActionCatalog;
//!
//! let mut catalog = ActionCatalog::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {"op": "describe", "args": {"id": "craft"}}
//!   ]
//! }"#, 4096)?;
//! let out = catalog.workflow(&req)?;
//! assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::checked as c;
use crate::data::json::Value;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result};

pub const MAX_ACTIONS: usize = 256;
pub const MAX_OPS: usize = 64;
/// Host-set limits (default is the earlier fixed bound): most `actions` in a catalog,
/// built-ins included (a loaded document may list at most this many of its own).
pub const LIMITS: &[crate::character::limits::Limit] =
    &[crate::character::limits::Limit { name: "actions", default: MAX_ACTIONS, ceiling: 65_536 }];
crate::character::limits::module_limits!(ActionLimits, LIMITS);

pub const CATEGORIES: [&str; 11] = [
    "things",
    "recipes",
    "species",
    "communities",
    "inventory",
    "fitness",
    "props",
    "services",
    "reactions",
    "engine",
    "initiative",
];

pub const TARGET_KINDS: [&str; 9] =
    ["thing", "person", "animal", "place", "community", "prop", "recipe", "service", "direction"];

pub const VISIBILITIES: [&str; 3] = ["public", "witnessed", "private"];

fn builtin() -> &'static Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    CATALOG.get_or_init(|| {
        crate::data::json::parse(include_str!("action_catalog.json"), 524_288).expect("built-in action catalog")
    })
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

fn bool_bound(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(c::bad()),
    }
}

fn validate_target_slot(ts: &Value) -> Result<()> {
    keys(ts, &["name", "kind", "required"], &["description"])?;
    ident_bound(c::get(ts, "name")?)?;
    let kind = text_bound(c::get(ts, "kind")?, 32)?;
    if !TARGET_KINDS.contains(&kind) {
        return Err(c::bad());
    }
    bool_bound(c::get(ts, "required")?)?;
    if let Some(desc) = ts.get("description").filter(|x| **x != Value::Null) {
        text_bound(desc, 256)?;
    }
    Ok(())
}

fn validate_actor(actor: &Value) -> Result<()> {
    keys(actor, &[], &["species", "skills", "body_state", "membership"])?;
    if let Some(sp) = actor.get("species").filter(|x| **x != Value::Null) {
        let arr = sp.as_array().ok_or_else(c::bad)?;
        if arr.len() > 16 {
            return Err(c::bad());
        }
        for item in arr {
            ident_bound(item)?;
        }
    }
    if let Some(sk) = actor.get("skills").filter(|x| **x != Value::Null) {
        let arr = sk.as_array().ok_or_else(c::bad)?;
        if arr.len() > 16 {
            return Err(c::bad());
        }
        for item in arr {
            keys(item, &["skill", "level"], &[])?;
            ident_bound(c::get(item, "skill")?)?;
            uint_bound(c::get(item, "level")?, 0, 100)?;
        }
    }
    if let Some(bs) = actor.get("body_state").filter(|x| **x != Value::Null) {
        keys(bs, &[], &["max_fatigue", "min_condition", "disallowed_conditions"])?;
        if let Some(mf) = bs.get("max_fatigue").filter(|x| **x != Value::Null) {
            uint_bound(mf, 0, 100)?;
        }
        if let Some(mc) = bs.get("min_condition").filter(|x| **x != Value::Null) {
            uint_bound(mc, 0, 100)?;
        }
        if let Some(dc) = bs.get("disallowed_conditions").filter(|x| **x != Value::Null) {
            let arr = dc.as_array().ok_or_else(c::bad)?;
            if arr.len() > 16 {
                return Err(c::bad());
            }
            for item in arr {
                ident_bound(item)?;
            }
        }
    }
    if let Some(mem) = actor.get("membership").filter(|x| **x != Value::Null) {
        ident_bound(mem)?;
    }
    Ok(())
}

fn validate_preconditions(pre: &Value) -> Result<()> {
    keys(pre, &[], &["same_place", "held", "in_reach"])?;
    if let Some(v) = pre.get("same_place").filter(|x| **x != Value::Null) {
        bool_bound(v)?;
    }
    if let Some(v) = pre.get("held").filter(|x| **x != Value::Null) {
        bool_bound(v)?;
    }
    if let Some(v) = pre.get("in_reach").filter(|x| **x != Value::Null) {
        bool_bound(v)?;
    }
    Ok(())
}

fn validate_cost(cost: &Value) -> Result<()> {
    keys(cost, &["time", "fatigue"], &["resources", "coins"])?;
    uint_bound(c::get(cost, "time")?, 0, 1_000_000)?;
    uint_bound(c::get(cost, "fatigue")?, 0, 100)?;
    if let Some(res) = cost.get("resources").filter(|x| **x != Value::Null) {
        let arr = res.as_array().ok_or_else(c::bad)?;
        if arr.len() > 16 {
            return Err(c::bad());
        }
        for item in arr {
            keys(item, &["kind", "qty"], &[])?;
            ident_bound(c::get(item, "kind")?)?;
            uint_bound(c::get(item, "qty")?, 1, 1_000_000)?;
        }
    }
    if let Some(coins) = cost.get("coins").filter(|x| **x != Value::Null) {
        uint_bound(coins, 0, 1_000_000_000)?;
    }
    Ok(())
}

fn validate_effects(eff: &Value) -> Result<()> {
    keys(eff, &["world"], &["body", "needs", "skills", "relationships"])?;
    text_bound(c::get(eff, "world")?, 256)?;
    if let Some(b) = eff.get("body").filter(|x| **x != Value::Null) {
        keys(b, &[], &["fatigue_delta", "condition_delta"])?;
        if let Some(fd) = b.get("fatigue_delta").filter(|x| **x != Value::Null) {
            int_bound(fd, -100, 100)?;
        }
        if let Some(cd) = b.get("condition_delta").filter(|x| **x != Value::Null) {
            int_bound(cd, -100, 100)?;
        }
    }
    if let Some(nd) = eff.get("needs").filter(|x| **x != Value::Null) {
        let arr = nd.as_array().ok_or_else(c::bad)?;
        if arr.len() > 16 {
            return Err(c::bad());
        }
        for item in arr {
            keys(item, &["need", "delta"], &[])?;
            ident_bound(c::get(item, "need")?)?;
            int_bound(c::get(item, "delta")?, -100, 100)?;
        }
    }
    if let Some(sk) = eff.get("skills").filter(|x| **x != Value::Null) {
        let arr = sk.as_array().ok_or_else(c::bad)?;
        if arr.len() > 16 {
            return Err(c::bad());
        }
        for item in arr {
            keys(item, &["skill", "practice"], &[])?;
            ident_bound(c::get(item, "skill")?)?;
            uint_bound(c::get(item, "practice")?, 1, 100)?;
        }
    }
    if let Some(rel) = eff.get("relationships").filter(|x| **x != Value::Null) {
        let arr = rel.as_array().ok_or_else(c::bad)?;
        if arr.len() > 16 {
            return Err(c::bad());
        }
        for item in arr {
            keys(item, &["target_slot", "attitude", "delta"], &[])?;
            ident_bound(c::get(item, "target_slot")?)?;
            ident_bound(c::get(item, "attitude")?)?;
            int_bound(c::get(item, "delta")?, -100, 100)?;
        }
    }
    Ok(())
}

fn validate_perception(p: &Value) -> Result<()> {
    keys(p, &["visibility"], &["noise", "witnesses"])?;
    let vis = text_bound(c::get(p, "visibility")?, 32)?;
    if !VISIBILITIES.contains(&vis) {
        return Err(c::bad());
    }
    if let Some(noise) = p.get("noise").filter(|x| **x != Value::Null) {
        uint_bound(noise, 0, 100)?;
    }
    if let Some(wit) = p.get("witnesses").filter(|x| **x != Value::Null) {
        text_bound(wit, 64)?;
    }
    Ok(())
}

fn validate_risk(r: &Value) -> Result<()> {
    keys(r, &["failure_chance"], &["on_failure"])?;
    uint_bound(c::get(r, "failure_chance")?, 0, 100)?;
    if let Some(on_f) = r.get("on_failure").filter(|x| **x != Value::Null) {
        text_bound(on_f, 256)?;
    }
    Ok(())
}

fn validate_action(act: &Value) -> Result<()> {
    keys(
        act,
        &[
            "id",
            "label",
            "category",
            "description",
            "actor",
            "targets",
            "preconditions",
            "cost",
            "effects",
            "perception",
            "risk",
            "duration",
            "interruptible",
        ],
        &[],
    )?;
    ident_bound(c::get(act, "id")?)?;
    text_bound(c::get(act, "label")?, 64)?;
    let cat = text_bound(c::get(act, "category")?, 32)?;
    if !CATEGORIES.contains(&cat) {
        return Err(c::bad());
    }
    text_bound(c::get(act, "description")?, 256)?;
    validate_actor(c::get(act, "actor")?)?;
    let targets = c::get(act, "targets")?.as_array().ok_or_else(c::bad)?;
    if targets.len() > 8 {
        return Err(c::bad());
    }
    let mut names = Vec::new();
    for ts in targets {
        validate_target_slot(ts)?;
        let name = c::get(ts, "name")?.as_str().unwrap();
        if names.contains(&name) {
            return Err(c::bad());
        }
        names.push(name);
    }
    validate_preconditions(c::get(act, "preconditions")?)?;
    validate_cost(c::get(act, "cost")?)?;
    validate_effects(c::get(act, "effects")?)?;
    validate_perception(c::get(act, "perception")?)?;
    validate_risk(c::get(act, "risk")?)?;
    uint_bound(c::get(act, "duration")?, 0, 1_000_000)?;
    bool_bound(c::get(act, "interruptible")?)?;
    Ok(())
}

/// Host-authoritative action catalog over all 11 simulated surfaces.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionCatalog {
    actions: BTreeMap<String, Value>,
    limits: ActionLimits,
}

impl Default for ActionCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionCatalog {
    pub fn new() -> Self {
        let b = builtin();
        let mut actions = BTreeMap::new();
        if let Some(arr) = b.get("actions").and_then(Value::as_array) {
            for act in arr {
                if let Some(id) = act.get("id").and_then(Value::as_str) {
                    actions.insert(id.to_string(), act.clone());
                }
            }
        }
        ActionCatalog { actions, limits: ActionLimits::default() }
    }

    pub fn parse(input: &str) -> Result<Self> {
        let v = crate::data::json::parse(input, 524_288)?;
        Self::from_value(&v)
    }

    pub fn from_value(v: &Value) -> Result<Self> {
        Self::from_value_within(v, &ActionLimits::default())
    }

    /// [`from_value`](Self::from_value) with host-set [limits](LIMITS); the result keeps them.
    pub fn from_value_within(v: &Value, l: &crate::character::limits::Limits) -> Result<Self> {
        keys(v, &["version"], &["categories", "actions"])?;
        uint_bound(c::get(v, "version")?, 1, 1)?;
        let mut cat = ActionCatalog::new();
        cat.limits = ActionLimits(l.clone());
        if let Some(acts) = v.get("actions").filter(|x| **x != Value::Null) {
            let arr = acts.as_array().ok_or_else(c::bad)?;
            if arr.len() > l.get("actions") {
                return Err(c::bad());
            }
            for act in arr {
                validate_action(act)?;
                let id = act.get("id").and_then(Value::as_str).unwrap();
                cat.actions.insert(id.to_string(), act.clone());
            }
        }
        Ok(cat)
    }

    pub fn to_value(&self) -> Value {
        let actions_list = self.actions.values().cloned().collect::<Vec<_>>();
        obj! {
            "version" => 1,
            "actions" => actions_list,
        }
    }

    pub fn get(&self, id: &str) -> Option<&Value> {
        self.actions.get(id)
    }

    pub fn op_list(&self, category: Option<&str>, actor_species: Option<&str>) -> Value {
        let mut matched = Vec::new();
        for act in self.actions.values() {
            if let Some(cat) = category {
                if act.get("category").and_then(Value::as_str) != Some(cat) {
                    continue;
                }
            }
            if let Some(species) = actor_species {
                if let Some(arr) = act.get("actor").and_then(|a| a.get("species")).and_then(Value::as_array) {
                    if !arr.is_empty() && !arr.iter().any(|s| s.as_str() == Some(species)) {
                        continue;
                    }
                }
            }
            matched.push(act.clone());
        }
        let count = matched.len() as u64;
        obj! {
            "actions" => matched,
            "count" => count,
        }
    }

    pub fn op_describe(&self, id: &str) -> Result<Value> {
        let act = self.actions.get(id).ok_or_else(c::bad)?;
        Ok(obj! {
            "action" => act.clone(),
        })
    }

    pub fn op_register(&mut self, action: &Value) -> Result<Value> {
        validate_action(action)?;
        if self.actions.len() >= self.limits.get("actions")
            && !self.actions.contains_key(action.get("id").unwrap().as_str().unwrap())
        {
            return Err(c::bad());
        }
        let id = action.get("id").unwrap().as_str().unwrap().to_string();
        self.actions.insert(id.clone(), action.clone());
        Ok(obj! {
            "registered" => id,
        })
    }

    fn check_actor(&self, act: &Value, actor: &Value) -> (bool, String) {
        let req_actor = act.get("actor").unwrap();
        let species = actor.get("species").and_then(Value::as_str);
        if let Some(sp) = species {
            if let Some(arr) = req_actor.get("species").and_then(Value::as_array) {
                if !arr.is_empty() && !arr.iter().any(|s| s.as_str() == Some(sp)) {
                    return (false, format!("species '{sp}' not permitted"));
                }
            }
        }

        if let Some(bs) = req_actor.get("body_state") {
            if let Some(mf_val) = bs.get("max_fatigue") {
                if let Ok(mf) = uint_bound(mf_val, 0, 100) {
                    let fatigue = actor
                        .get("fitness")
                        .and_then(|f| f.get("fatigue"))
                        .or_else(|| actor.get("fatigue"))
                        .and_then(|v| uint_bound(v, 0, 100).ok())
                        .unwrap_or(0);
                    if fatigue > mf {
                        return (false, format!("too fatigued ({fatigue} > {mf})"));
                    }
                }
            }
            if let Some(mc_val) = bs.get("min_condition") {
                if let Ok(mc) = uint_bound(mc_val, 0, 100) {
                    let cond = actor
                        .get("fitness")
                        .and_then(|f| f.get("condition"))
                        .or_else(|| actor.get("condition"))
                        .and_then(|v| uint_bound(v, 0, 100).ok())
                        .unwrap_or(100);
                    if cond < mc {
                        return (false, format!("condition too low ({cond} < {mc})"));
                    }
                }
            }
            if let Some(arr) = bs.get("disallowed_conditions").and_then(Value::as_array) {
                if let Some(actor_conds) = actor.get("conditions").and_then(Value::as_array) {
                    for c in actor_conds {
                        if let Some(c_str) = c.as_str() {
                            if arr.iter().any(|x| x.as_str() == Some(c_str)) {
                                return (false, format!("cannot act while {c_str}"));
                            }
                        }
                    }
                }
            }
        }

        if let Some(req_skills) = req_actor.get("skills").and_then(Value::as_array) {
            let actor_skills = actor.get("skills");
            for req_sk in req_skills {
                let sk_id = req_sk.get("skill").and_then(Value::as_str).unwrap_or("");
                let req_lvl = req_sk.get("level").and_then(|v| uint_bound(v, 0, 100).ok()).unwrap_or(0);
                let actual_lvl =
                    actor_skills.and_then(|s| s.get(sk_id)).and_then(|v| uint_bound(v, 0, 100).ok()).unwrap_or(0);
                if actual_lvl < req_lvl {
                    return (false, format!("skill {sk_id} level {actual_lvl} < {req_lvl}"));
                }
            }
        }

        if let Some(req_mem) = req_actor.get("membership").and_then(Value::as_str) {
            let actor_mems = actor.get("communities").or_else(|| actor.get("factions")).and_then(Value::as_array);
            let has_mem = actor_mems.is_some_and(|arr| arr.iter().any(|m| m.as_str() == Some(req_mem)));
            if !has_mem {
                return (false, format!("requires membership in {req_mem}"));
            }
        }

        (true, "ok".to_string())
    }

    pub fn op_check(
        &self,
        action_id: &str,
        actor: &Value,
        bindings: Option<&Value>,
        _place: Option<&str>,
    ) -> Result<Value> {
        let act = self.actions.get(action_id).ok_or_else(c::bad)?;
        let (ok, reason) = self.check_actor(act, actor);
        if !ok {
            return Ok(obj! {
                "feasible" => false,
                "action" => action_id,
                "reason" => reason,
            });
        }

        let empty_bindings = Value::Object(Vec::new());
        let bnd = bindings.unwrap_or(&empty_bindings);

        if let Some(targets) = act.get("targets").and_then(Value::as_array) {
            for ts in targets {
                let slot_name = ts.get("name").and_then(Value::as_str).unwrap();
                let required = ts.get("required").and_then(|v| bool_bound(v).ok()).unwrap_or(false);
                if required && bnd.get(slot_name).is_none() {
                    return Ok(obj! {
                        "feasible" => false,
                        "action" => action_id,
                        "reason" => format!("missing required target slot '{slot_name}'"),
                    });
                }
            }
        }

        if let Some(pre) = act.get("preconditions") {
            let held = pre.get("held").and_then(|v| bool_bound(v).ok()).unwrap_or(false);
            if held {
                let target_thing = bnd.get("thing").or_else(|| bnd.get("resource")).and_then(Value::as_str);
                if let Some(thing) = target_thing {
                    let inv = actor.get("inventory").and_then(Value::as_array);
                    let is_held = inv.is_some_and(|arr| arr.iter().any(|item| item.as_str() == Some(thing)));
                    if !is_held {
                        return Ok(obj! {
                            "feasible" => false,
                            "action" => action_id,
                            "reason" => format!("item '{thing}' is not held in inventory"),
                        });
                    }
                }
            }
        }

        Ok(obj! {
            "feasible" => true,
            "action" => action_id,
            "cost" => act.get("cost").cloned().unwrap(),
            "effects" => act.get("effects").cloned().unwrap(),
            "perception" => act.get("perception").cloned().unwrap(),
            "duration" => act.get("duration").cloned().unwrap(),
            "interruptible" => act.get("interruptible").cloned().unwrap(),
        })
    }

    pub fn op_available(
        &self,
        actor: &Value,
        place: Option<&str>,
        targets: Option<&[Value]>,
        category: Option<&str>,
    ) -> Result<Value> {
        let mut candidates_by_kind: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Some(tgts) = targets {
            for t in tgts {
                let k = t.get("kind").and_then(Value::as_str).unwrap_or("thing");
                let id = t.get("id").and_then(Value::as_str).unwrap_or("");
                candidates_by_kind.entry(k.to_string()).or_default().push(id.to_string());
            }
        }

        let mut available = Vec::new();
        for (act_id, act) in &self.actions {
            if let Some(cat) = category {
                if act.get("category").and_then(Value::as_str) != Some(cat) {
                    continue;
                }
            }

            let (ok, _) = self.check_actor(act, actor);
            if !ok {
                continue;
            }

            let mut can_bind = true;
            let mut bindings = Vec::new();
            if let Some(ts_list) = act.get("targets").and_then(Value::as_array) {
                for ts in ts_list {
                    let slot_name = ts.get("name").and_then(Value::as_str).unwrap();
                    let slot_kind = ts.get("kind").and_then(Value::as_str).unwrap();
                    let required = ts.get("required").and_then(|v| bool_bound(v).ok()).unwrap_or(false);

                    let available_candidates = candidates_by_kind.get(slot_kind);
                    if required {
                        if let Some(cands) = available_candidates {
                            if !cands.is_empty() {
                                bindings.push((slot_name.to_string(), Value::str(&cands[0])));
                            } else {
                                can_bind = false;
                                break;
                            }
                        } else {
                            can_bind = false;
                            break;
                        }
                    } else if let Some(cands) = available_candidates {
                        if !cands.is_empty() {
                            bindings.push((slot_name.to_string(), Value::str(&cands[0])));
                        }
                    }
                }
            }

            if can_bind {
                let bindings_val = Value::Object(bindings);
                let chk = self.op_check(act_id, actor, Some(&bindings_val), place)?;
                if chk.get("feasible").and_then(|v| bool_bound(v).ok()).unwrap_or(false) {
                    available.push(obj! {
                        "action" => act_id.as_str(),
                        "category" => act.get("category").cloned().unwrap(),
                        "label" => act.get("label").cloned().unwrap(),
                        "bindings" => bindings_val,
                        "cost" => act.get("cost").cloned().unwrap(),
                        "duration" => act.get("duration").cloned().unwrap(),
                    });
                }
            }
        }

        let count = available.len() as u64;
        Ok(obj! {
            "available" => available,
            "count" => count,
        })
    }

    /// Runs `{ops, actions?, catalog?, limits?}`. `limits` (host-set [limits](LIMITS)) apply
    /// first and are echoed in the output. Request `actions` may not grow the catalog past
    /// the `actions` limit.
    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
        let stripped = crate::character::limits::take(req, &mut self.limits)?;
        let out = self.run_workflow(stripped.as_ref().unwrap_or(req))?;
        Ok(crate::character::limits::echo(out, &self.limits, stripped.is_some()))
    }

    /// A built-in catalog with host limits applied up front.
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut a = Self::new();
        a.limits.apply(v)?;
        Ok(a)
    }

    fn run_workflow(&mut self, req: &Value) -> Result<Value> {
        keys(req, &["ops"], &["catalog", "actions"])?;
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        if let Some(actions) = req.get("actions").filter(|x| **x != Value::Null) {
            let arr = actions.as_array().ok_or_else(c::bad)?;
            for act in arr {
                validate_action(act)?;
                let id = act.get("id").unwrap().as_str().unwrap().to_string();
                self.actions.insert(id, act.clone());
            }
            if self.actions.len() > self.limits.get("actions") {
                return Err(c::bad());
            }
        }

        let mut results = Vec::new();
        for op_call in ops {
            keys(op_call, &["op"], &["at", "args"])?;
            let op_name = text_bound(c::get(op_call, "op")?, 32)?;
            let empty_args = Value::Object(Vec::new());
            let args = op_call.get("args").filter(|x| **x != Value::Null).unwrap_or(&empty_args);

            match op_name {
                "list" => {
                    keys(args, &[], &["category", "actor_species"])?;
                    let cat =
                        args.get("category").filter(|x| **x != Value::Null).map(|v| text_bound(v, 32)).transpose()?;
                    if let Some(c) = cat {
                        if !CATEGORIES.contains(&c) {
                            return Err(c::bad());
                        }
                    }
                    let species =
                        args.get("actor_species").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
                    results.push(self.op_list(cat, species));
                }
                "describe" => {
                    keys(args, &["id"], &[])?;
                    let id = ident_bound(c::get(args, "id")?)?;
                    results.push(self.op_describe(id)?);
                }
                "available" => {
                    keys(args, &["actor"], &["place", "targets", "category"])?;
                    let actor = c::get(args, "actor")?;
                    let place =
                        args.get("place").filter(|x| **x != Value::Null).map(|v| text_bound(v, 64)).transpose()?;
                    let targets = args.get("targets").filter(|x| **x != Value::Null).and_then(Value::as_array);
                    let cat =
                        args.get("category").filter(|x| **x != Value::Null).map(|v| text_bound(v, 32)).transpose()?;
                    if let Some(c) = cat {
                        if !CATEGORIES.contains(&c) {
                            return Err(c::bad());
                        }
                    }
                    results.push(self.op_available(actor, place, targets, cat)?);
                }
                "check" => {
                    keys(args, &["action", "actor"], &["bindings", "place"])?;
                    let action = ident_bound(c::get(args, "action")?)?;
                    let actor = c::get(args, "actor")?;
                    let bindings = args.get("bindings").filter(|x| **x != Value::Null);
                    let place =
                        args.get("place").filter(|x| **x != Value::Null).map(|v| text_bound(v, 64)).transpose()?;
                    results.push(self.op_check(action, actor, bindings, place)?);
                }
                "register" => {
                    keys(args, &["action"], &[])?;
                    let act = c::get(args, "action")?;
                    results.push(self.op_register(act)?);
                }
                _ => return Err(c::bad()),
            }
        }

        Ok(obj! {
            "results" => results,
            "catalog" => self.to_value(),
        })
    }
}

/// The `actions` local kind: `{ops, actions?, catalog?, limits?}` → `{limits?, results, ...}`.
pub fn workflow(a: &Value) -> Result<Value> {
    ActionCatalog::new().workflow(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    #[test]
    fn builtin_catalog_loads_and_has_all_categories() {
        let cat = ActionCatalog::new();
        assert_eq!(cat.actions.len(), 49);
        for c in CATEGORIES {
            let res = cat.op_list(Some(c), None);
            let count = res.get("count").and_then(|v| uint_bound(v, 0, 100).ok()).unwrap();
            assert!(count > 0, "category {c} must have actions");
        }
    }

    #[test]
    fn workflow_list_and_describe() -> Result<()> {
        let mut cat = ActionCatalog::new();
        let req = parse(r#"{"ops":[{"op":"list","args":{"category":"recipes"}}]}"#, 4096)?;
        let out = cat.workflow(&req)?;
        let results = out.get("results").and_then(Value::as_array).unwrap();
        assert_eq!(results[0].get("count").and_then(|v| uint_bound(v, 0, 100).ok()).unwrap(), 5);

        let req_desc = parse(r#"{"ops":[{"op":"describe","args":{"id":"craft"}}]}"#, 4096)?;
        let out_desc = cat.workflow(&req_desc)?;
        let results_desc = out_desc.get("results").and_then(Value::as_array).unwrap();
        assert_eq!(results_desc[0].get("action").and_then(|a| a.get("id")).and_then(Value::as_str), Some("craft"));
        Ok(())
    }

    #[test]
    fn check_feasibility_and_refusals() -> Result<()> {
        let mut cat = ActionCatalog::new();
        // Feasible craft
        let req1 = parse(
            r#"{"ops":[{"op":"check","args":{"action":"craft","actor":{"id":"hero","species":"person"},"bindings":{"recipe":"iron_sword"}}}]}"#,
            4096,
        )?;
        let out1 = cat.workflow(&req1)?;
        let r1 = &out1.get("results").and_then(Value::as_array).unwrap()[0];
        assert_eq!(r1.get("feasible").and_then(|v| bool_bound(v).ok()), Some(true));

        // Refused: asleep
        let req2 = parse(
            r#"{"ops":[{"op":"check","args":{"action":"craft","actor":{"id":"hero","species":"person","conditions":["asleep"]},"bindings":{"recipe":"iron_sword"}}}]}"#,
            4096,
        )?;
        let out2 = cat.workflow(&req2)?;
        let r2 = &out2.get("results").and_then(Value::as_array).unwrap()[0];
        assert_eq!(r2.get("feasible").and_then(|v| bool_bound(v).ok()), Some(false));
        assert!(r2.get("reason").and_then(Value::as_str).unwrap().contains("asleep"));
        Ok(())
    }
}
