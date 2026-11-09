//! Default packs (pass 2, D5).
//!
//! A pack is ready-made authoring data that turns into plain requests for engines that already
//! exist, so a new being can start from a sensible default instead of a blank sheet:
//!
//! - species (`human`, `dog`, `horse`, `goat`): body needs with a threshold, a rate and a
//!   satisfying action, plus a lifespan in hour ticks;
//! - temperaments (`bold`, `steady`, `cautious`, `timid`, `dreamy`): a cognition temperament, a
//!   threat temperament and starting personality traits (signed millionths);
//! - roles (`guard`, `baker`, `farmer`, `elder`, `healer`): status, seats and timed duties;
//! - norm sets (`village`, `guild`, `wanderer`): personal norms an action can break.
//!
//! `apply` changes no other engine: it returns the requests (`body`, `survival`, `cognition`,
//! `drift`, `roles`, `creed`) for the host to run, plus a hint for threat responses. Authors
//! `define` their own packs of the same shapes (bounded by the `custom` limit; built-in ids
//! cannot be replaced) and read them back with `list` and `show`. Capacities are
//! [host-set limits](crate::character::limits). Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::packs::Packs;
//!
//! let mut packs = Packs::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "apply", "args": {"being": "nell", "species": "goat", "temperament": "timid"}}
//! ]}"#, 4096)?;
//! let out = packs.workflow(&req)?.to_json();
//! // A goat is born with a lifespan of 131,400 hours and a timid mind starts with low boldness.
//! assert!(out.contains(r#"{"args":{"being":"nell","lifespan":131400},"op":"born"}"#));
//! assert!(out.contains(r#"{"args":{"base":-600000,"being":"nell","trait":"boldness"},"op":"trait"}"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::character::task::KNOWN_ACTIONS;
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "custom", default: 64, ceiling: 1_000_000 },
    Limit { name: "roles", default: 2, ceiling: 4 },
    Limit { name: "norms", default: 2, ceiling: 4 },
];
pub const M: i64 = 1_000_000;
/// Pack kinds, sorted.
pub const KINDS: [&str; 4] = ["norms", "role", "species", "temperament"];
pub const COGNITION: [&str; 4] = ["careful", "dreamy", "impulsive", "steady"];
pub const SURVIVAL: [&str; 4] = ["bold", "cautious", "steady", "timid"];
pub const MAX_NEEDS: usize = 8;
pub const MAX_TRAITS: usize = 8;
pub const MAX_DUTIES: usize = 8;
pub const MAX_TENETS: usize = 16;

/// The built-in packs, in the same shape `define` takes.
pub const BUILTIN: &str = r#"{
 "species": {
  "human": {"lifespan": 613200, "needs": [
   {"id": "hunger", "action": "cook", "threshold": 70, "rate_per_tick": 2, "amount": 30, "label": "Hunger"},
   {"id": "rest", "action": "wait", "threshold": 75, "rate_per_tick": 1, "amount": 25, "label": "Rest"},
   {"id": "company", "action": "talk", "threshold": 60, "rate_per_tick": 1, "amount": 20, "label": "Company"}]},
  "dog": {"lifespan": 105120, "needs": [
   {"id": "hunger", "action": "feed", "threshold": 65, "rate_per_tick": 3, "amount": 35, "label": "Hunger"},
   {"id": "rest", "action": "wait", "threshold": 75, "rate_per_tick": 2, "amount": 30, "label": "Rest"},
   {"id": "affection", "action": "pet", "threshold": 55, "rate_per_tick": 2, "amount": 25, "label": "Affection"}]},
  "horse": {"lifespan": 219000, "needs": [
   {"id": "hunger", "action": "feed", "threshold": 70, "rate_per_tick": 2, "amount": 30, "label": "Hunger"},
   {"id": "rest", "action": "wait", "threshold": 80, "rate_per_tick": 1, "amount": 25, "label": "Rest"},
   {"id": "exercise", "action": "ride", "threshold": 60, "rate_per_tick": 2, "amount": 25, "label": "Exercise"}]},
  "goat": {"lifespan": 131400, "needs": [
   {"id": "hunger", "action": "feed", "threshold": 60, "rate_per_tick": 3, "amount": 30, "label": "Hunger"},
   {"id": "rest", "action": "wait", "threshold": 80, "rate_per_tick": 1, "amount": 25, "label": "Rest"},
   {"id": "roam", "action": "move", "threshold": 55, "rate_per_tick": 3, "amount": 20, "label": "Roaming"}]}
 },
 "temperament": {
  "bold": {"cognition": "impulsive", "survival": "bold", "traits": [
   {"trait": "boldness", "base": 600000}, {"trait": "trust", "base": 500000}, {"trait": "warmth", "base": 400000}]},
  "steady": {"cognition": "steady", "survival": "steady", "traits": [
   {"trait": "boldness", "base": 0}, {"trait": "trust", "base": 300000}, {"trait": "warmth", "base": 300000}]},
  "cautious": {"cognition": "careful", "survival": "cautious", "traits": [
   {"trait": "boldness", "base": -300000}, {"trait": "trust", "base": 0}, {"trait": "warmth", "base": 200000}]},
  "timid": {"cognition": "careful", "survival": "timid", "traits": [
   {"trait": "boldness", "base": -600000}, {"trait": "trust", "base": -100000}, {"trait": "warmth", "base": 300000}]},
  "dreamy": {"cognition": "dreamy", "survival": "steady", "traits": [
   {"trait": "boldness", "base": -100000}, {"trait": "trust", "base": 400000}, {"trait": "warmth", "base": 500000}]}
 },
 "role": {
  "guard": {"status": 40, "duties": [{"id": "patrol", "action": "move", "every": 6, "at": 0, "place": "gate", "priority": 70}]},
  "baker": {"status": 35, "duties": [{"id": "bake", "action": "cook", "every": 24, "at": 5, "place": "bakery", "priority": 60}]},
  "farmer": {"status": 30, "duties": [{"id": "tend", "action": "harvest", "every": 24, "at": 6, "place": "fields", "priority": 55}]},
  "elder": {"status": 70, "seats": 3, "duties": [{"id": "counsel", "action": "talk", "every": 24, "at": 18, "place": "hall", "priority": 40}]},
  "healer": {"status": 55, "duties": [{"id": "rounds", "action": "help", "every": 12, "at": 8, "place": "infirmary", "priority": 75}]}
 },
 "norms": {
  "village": {"tenets": [
   {"id": "village_no_theft", "action": "take", "severity": 60, "label": "Do not take what is not yours"},
   {"id": "village_no_wrecking", "action": "destroy", "severity": 50, "label": "Do not wreck a neighbour's work"}]},
  "guild": {"tenets": [
   {"id": "guild_answer_calls", "action": "decline", "severity": 45, "label": "Do not refuse a guild-mate who asks for help"},
   {"id": "guild_no_undoing", "action": "dismantle", "severity": 40, "label": "Do not take apart guild work without leave"}]},
  "wanderer": {"tenets": [
   {"id": "wanderer_no_roots", "action": "build", "severity": 20, "label": "Do not put down roots"},
   {"id": "wanderer_travel_light", "action": "store", "severity": 15, "label": "Carry only what you can use"}]}
 }
}"#;

fn need(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(c::bad())
    }
}

fn id_of(args: &Value, k: &str) -> Result<String> {
    Ok(ident(c::get(args, k)?)?.to_string())
}

fn sint(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(c::bad),
        _ => Err(c::bad()),
    }
}

fn rows<'a>(args: &'a Value, k: &str, lo: usize, hi: usize) -> Result<&'a [Value]> {
    let xs = list_bound(opt(args, k), hi)?;
    need(xs.len() >= lo)?;
    Ok(xs)
}

fn action(v: &Value) -> Result<String> {
    let a = ident(v)?;
    need(KNOWN_ACTIONS.contains(&a))?;
    Ok(a.to_string())
}

fn unique<'a>(ids: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for i in ids {
        need(seen.insert(i))?;
    }
    Ok(())
}

fn field<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&Value::Null)
}

fn text_of(v: &Value, k: &str) -> String {
    field(v, k).as_str().unwrap_or_default().to_string()
}

fn int_of(v: &Value, k: &str) -> i64 {
    match field(v, k) {
        Value::Number(s) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn list_of<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    field(v, k).as_array().unwrap_or(&[])
}

fn read_species(p: &Value) -> Result<Value> {
    keys(p, &["lifespan", "needs"], &[])?;
    let life = uint_bound(c::get(p, "lifespan")?, 1, MAX_AT)?;
    let mut needs = Vec::new();
    for n in rows(p, "needs", 1, MAX_NEEDS)? {
        keys(n, &["id", "action"], &["label", "threshold", "rate_per_tick", "amount"])?;
        let i = id_of(n, "id")?;
        let label = match opt(n, "label") {
            None => i.clone(),
            Some(v) => text_bound(v, 40)?.to_string(),
        };
        needs.push(obj! {
            "action" => action(c::get(n, "action")?)?,
            "amount" => opt_uint(n, "amount", 0, 100, 20)?,
            "id" => i,
            "label" => label,
            "rate_per_tick" => opt_uint(n, "rate_per_tick", 0, 100, 1)?,
            "threshold" => opt_uint(n, "threshold", 0, 100, 70)?,
        });
    }
    unique(needs.iter().map(|n| field(n, "id").as_str().unwrap_or_default()))?;
    Ok(obj! { "lifespan" => life, "needs" => Value::Array(needs) })
}

fn read_temperament(p: &Value) -> Result<Value> {
    keys(p, &["traits"], &["cognition", "survival"])?;
    let cog = opt(p, "cognition").map(ident).transpose()?.unwrap_or("steady");
    let sur = opt(p, "survival").map(ident).transpose()?.unwrap_or("steady");
    need(COGNITION.contains(&cog) && SURVIVAL.contains(&sur))?;
    let mut traits = Vec::new();
    for t in rows(p, "traits", 0, MAX_TRAITS)? {
        keys(t, &["trait", "base"], &[])?;
        traits.push(obj! { "base" => sint(c::get(t, "base")?, -M, M)?, "trait" => id_of(t, "trait")? });
    }
    unique(traits.iter().map(|t| field(t, "trait").as_str().unwrap_or_default()))?;
    Ok(obj! { "cognition" => cog, "survival" => sur, "traits" => Value::Array(traits) })
}

fn read_role(p: &Value) -> Result<Value> {
    keys(p, &["status"], &["seats", "duties"])?;
    let status = uint_bound(c::get(p, "status")?, 0, 100)?;
    let seats = opt(p, "seats").map(|v| uint_bound(v, 1, 1_000_000)).transpose()?;
    let mut duties = Vec::new();
    for d in rows(p, "duties", 0, MAX_DUTIES)? {
        keys(d, &["id", "action", "every"], &["at", "place", "priority"])?;
        duties.push(obj! {
            "action" => action(c::get(d, "action")?)?,
            "at" => opt_uint(d, "at", 0, MAX_AT, 0)?,
            "every" => uint_bound(c::get(d, "every")?, 1, MAX_AT)?,
            "id" => id_of(d, "id")?,
            "place" => opt(d, "place").map(|v| ident(v).map(str::to_string)).transpose()?,
            "priority" => opt_uint(d, "priority", 0, 100, 50)?,
        });
    }
    unique(duties.iter().map(|d| field(d, "id").as_str().unwrap_or_default()))?;
    Ok(obj! { "duties" => Value::Array(duties), "seats" => seats, "status" => status })
}

fn read_norms(p: &Value) -> Result<Value> {
    keys(p, &["tenets"], &[])?;
    let mut tenets = Vec::new();
    for t in rows(p, "tenets", 1, MAX_TENETS)? {
        keys(t, &["id", "severity", "label", "action"], &[])?;
        tenets.push(obj! {
            "action" => id_of(t, "action")?,
            "id" => id_of(t, "id")?,
            "label" => text_bound(c::get(t, "label")?, 80)?,
            "severity" => uint_bound(c::get(t, "severity")?, 1, 100)?,
        });
    }
    unique(tenets.iter().map(|t| field(t, "id").as_str().unwrap_or_default()))?;
    Ok(obj! { "tenets" => Value::Array(tenets) })
}

fn read(kind: &str, p: &Value) -> Result<Value> {
    match kind {
        "norms" => read_norms(p),
        "role" => read_role(p),
        "species" => read_species(p),
        "temperament" => read_temperament(p),
        _ => Err(c::bad()),
    }
}

fn request(kind: &str, ops: Vec<Value>) -> Value {
    obj! { "args" => obj! { "ops" => Value::Array(ops) }, "kind" => kind }
}

fn op(name: &str, args: Value) -> Value {
    obj! { "args" => args, "op" => name }
}

/// Built-in and author-defined packs, each kept in its normalized form.
#[derive(Clone, Debug, PartialEq)]
pub struct Packs {
    pub builtin: BTreeMap<String, BTreeMap<String, Value>>,
    pub custom: BTreeMap<String, BTreeMap<String, Value>>,
    pub count: usize,
    pub limits: Limits,
}

impl Default for Packs {
    fn default() -> Self {
        Self::new()
    }
}

impl Packs {
    pub fn new() -> Self {
        let raw = crate::data::json::parse(BUILTIN, 65_536).expect("built-in packs parse");
        let mut builtin = BTreeMap::new();
        let mut custom = BTreeMap::new();
        for k in KINDS {
            let mut m = BTreeMap::new();
            for (i, p) in field(&raw, k).as_object().unwrap_or(&[]) {
                m.insert(i.clone(), read(k, p).expect("built-in pack is valid"));
            }
            builtin.insert(k.to_string(), m);
            custom.insert(k.to_string(), BTreeMap::new());
        }
        Packs { builtin, custom, count: 0, limits: Limits::defaults(LIMITS) }
    }

    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "define" => self.op_define(args, l),
            "list" => self.op_list(args),
            "show" => self.op_show(args),
            "apply" => self.op_apply(args, l),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn kind(args: &Value) -> Result<&'static str> {
        let k = ident(c::get(args, "kind")?)?;
        KINDS.iter().copied().find(|x| *x == k).ok_or_else(c::bad)
    }

    fn find(&self, k: &str, i: &str) -> Result<&Value> {
        self.builtin[k].get(i).or_else(|| self.custom[k].get(i)).ok_or_else(c::bad)
    }

    fn op_define(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["kind", "id", "pack"], &[])?;
        let k = Self::kind(args)?;
        let i = id_of(args, "id")?;
        let p = read(k, c::get(args, "pack")?)?;
        need(!self.builtin[k].contains_key(&i))?;
        let fresh = !self.custom[k].contains_key(&i);
        need(!fresh || self.count < l.get("custom"))?;
        if fresh {
            self.count += 1;
        }
        self.custom.get_mut(k).ok_or_else(c::bad)?.insert(i.clone(), p.clone());
        Ok(obj! { "id" => i, "kind" => k, "pack" => p })
    }

    fn op_list(&self, args: &Value) -> Result<Value> {
        keys(args, &["kind"], &[])?;
        let k = Self::kind(args)?;
        let names = |m: &BTreeMap<String, Value>| Value::Array(m.keys().map(|x| Value::from(x.as_str())).collect());
        Ok(obj! { "builtin" => names(&self.builtin[k]), "custom" => names(&self.custom[k]), "kind" => k })
    }

    fn op_show(&self, args: &Value) -> Result<Value> {
        keys(args, &["kind", "id"], &[])?;
        let k = Self::kind(args)?;
        let i = id_of(args, "id")?;
        let p = self.find(k, &i)?.clone();
        Ok(obj! { "builtin" => self.builtin[k].contains_key(&i), "id" => i, "kind" => k, "pack" => p })
    }

    fn op_apply(&self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being"], &["species", "temperament", "roles", "norms"])?;
        let b = id_of(args, "being")?;
        let sp_id = opt(args, "species").map(ident).transpose()?;
        let sp = sp_id.map(|i| self.find("species", i)).transpose()?;
        let tp = opt(args, "temperament").map(|v| ident(v).and_then(|i| self.find("temperament", i))).transpose()?;
        let role_ids = rows(args, "roles", 0, l.get("roles"))?.iter().map(ident).collect::<Result<Vec<_>>>()?;
        let norm_ids = rows(args, "norms", 0, l.get("norms"))?.iter().map(ident).collect::<Result<Vec<_>>>()?;
        unique(role_ids.iter().copied())?;
        unique(norm_ids.iter().copied())?;
        let roles = role_ids.iter().map(|r| Ok((*r, self.find("role", r)?))).collect::<Result<Vec<_>>>()?;
        let mut tenets = Vec::new();
        for n in &norm_ids {
            tenets.extend(list_of(self.find("norms", n)?, "tenets").iter().cloned());
        }
        need(tenets.len() <= MAX_TENETS)?;
        unique(tenets.iter().map(|t| field(t, "id").as_str().unwrap_or_default()))?;
        need(sp.is_some() || tp.is_some() || !roles.is_empty() || !tenets.is_empty())?;

        let being = || Value::from(b.as_str());
        let mut out = Vec::new();
        if let (Some(sp), Some(sid)) = (sp, sp_id) {
            let needs = list_of(sp, "needs")
                .iter()
                .map(|n| {
                    obj! {
                        "id" => text_of(n, "id"),
                        "label" => text_of(n, "label"),
                        "rate_per_tick" => int_of(n, "rate_per_tick"),
                        "satisfiers" => Value::Array(vec![obj! {
                            "action" => text_of(n, "action"), "amount" => int_of(n, "amount"),
                        }]),
                        "threshold" => int_of(n, "threshold"),
                    }
                })
                .collect();
            let init = obj! { "being" => being(), "needs" => Value::Array(needs), "species" => sid };
            out.push(request("body", vec![op("init", init)]));
            let born = obj! { "being" => being(), "lifespan" => int_of(sp, "lifespan") };
            out.push(request("survival", vec![op("born", born)]));
        }
        if let Some(tp) = tp {
            let prof = obj! { "being" => being(), "temperament" => text_of(tp, "cognition") };
            out.push(request("cognition", vec![op("profile", prof)]));
            let traits: Vec<Value> = list_of(tp, "traits")
                .iter()
                .map(|t| {
                    let a = obj! { "base" => int_of(t, "base"), "being" => being(), "trait" => text_of(t, "trait") };
                    op("trait", a)
                })
                .collect();
            if !traits.is_empty() {
                out.push(request("drift", traits));
            }
        }
        if !roles.is_empty() {
            let mut ops = Vec::new();
            for (r, p) in &roles {
                let duties = list_of(p, "duties")
                    .iter()
                    .map(|d| {
                        let kept = d.as_object().unwrap_or(&[]).iter().filter(|(_, v)| *v != Value::Null).cloned();
                        Value::Object(kept.collect())
                    })
                    .collect();
                let mut a = vec![
                    ("duties".to_string(), Value::Array(duties)),
                    ("id".to_string(), Value::from(*r)),
                    ("status".to_string(), field(p, "status").clone()),
                ];
                if *field(p, "seats") != Value::Null {
                    a.push(("seats".to_string(), field(p, "seats").clone()));
                }
                ops.push(op("role", Value::Object(a)));
            }
            for (r, _) in &roles {
                ops.push(op("assign", obj! { "being" => being(), "role" => *r }));
            }
            out.push(request("roles", ops));
        }
        if !tenets.is_empty() {
            let set = obj! { "being" => being(), "norms" => Value::Array(tenets) };
            out.push(request("creed", vec![op("set_norms", set)]));
        }
        let hints = obj! {
            "lifespan" => sp.map(|s| int_of(s, "lifespan")),
            "survival_temperament" => tp.map(|t| text_of(t, "survival")),
        };
        Ok(obj! { "being" => b.as_str(), "hints" => hints, "requests" => Value::Array(out) })
    }
}

persist!(rest Packs { custom });
durable!(Packs, "packs", LIMITS);

impl Packs {
    /// Invariants a save must meet: every custom pack reads back to itself under its kind's
    /// rules and does not shadow a built-in; `count` is rebuilt.
    fn settle_state(&mut self) -> Result<()> {
        ok(self.custom.keys().map(String::as_str).eq(KINDS))?;
        let mut count = 0usize;
        for (k, packs) in &self.custom {
            for (i, p) in packs {
                ok(is_ident(i) && !self.builtin[k].contains_key(i) && read(k, p)? == *p)?;
                count += 1;
            }
        }
        within::<Self>("custom", count)?;
        self.count = count;
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Packs::new().workflow(req)
}
