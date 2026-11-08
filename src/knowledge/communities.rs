//! World communities: factions extension, home places, members, customs (norms),
//! relations with numeric trust, markets, and bartering.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::knowledge::communities::CommunityWorld;
//!
//! let mut cw = CommunityWorld::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {"op": "check_relation", "args": {"community": "ember_wardens", "target": "merchant_guild"}}
//!   ],
//!   "communities": [
//!     {
//!       "id": "ember_wardens",
//!       "label": "The Ember Wardens",
//!       "description": "Guardians of Ember Vale and the mountain pass.",
//!       "relations": [
//!         {"target": "merchant_guild", "stance": "allied", "trust": 70}
//!       ]
//!     }
//!   ]
//! }"#, 4096)?;
//! let out = cw.workflow(&req)?;
//! assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::{obj, Result, Value};

pub const MAX_COMMUNITIES: usize = 64;
pub const MAX_MEMBERS: usize = 64;
pub const MAX_CUSTOMS: usize = 32;
pub const MAX_RELATIONS: usize = 64;
pub const MAX_RULES: usize = 32;
pub const MAX_OPS: usize = 64;

pub const STANCES: [&str; 5] = ["allied", "friendly", "neutral", "wary", "hostile"];

fn as_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn get_field_mut<'a>(v: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    if let Value::Object(fields) = v {
        fields.iter_mut().find(|(k, _)| k == key).map(|(_, val)| val)
    } else {
        None
    }
}

fn as_array_mut(v: &mut Value) -> Option<&mut Vec<Value>> {
    if let Value::Array(arr) = v {
        Some(arr)
    } else {
        None
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

fn int_bound(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    let n = as_i64(v).ok_or_else(c::bad)?;
    if n < lo || n > hi {
        return Err(c::bad());
    }
    Ok(n)
}

fn set(v: &mut Value, key: &str, val: Value) {
    if let Value::Object(fields) = v {
        if let Some(pos) = fields.iter().position(|(k, _)| k == key) {
            fields[pos] = (key.to_string(), val);
        } else {
            fields.push((key.to_string(), val));
        }
    }
}

pub fn stance_from_trust(trust: i64) -> &'static str {
    if trust >= 60 {
        "allied"
    } else if trust >= 25 {
        "friendly"
    } else if trust >= -25 {
        "neutral"
    } else if trust >= -60 {
        "wary"
    } else {
        "hostile"
    }
}

pub fn multipliers_from_trust(trust: i64) -> (u64, u64) {
    let buy = (1000 + trust * 2).clamp(100, 2000) as u64;
    let sell = (1000 - trust * 2).clamp(100, 3000) as u64;
    (buy, sell)
}

pub fn stance_defaults(stance: &str) -> (i64, u64, u64) {
    match stance {
        "allied" => (75, 1100, 800),
        "friendly" => (50, 1050, 900),
        "neutral" => (0, 1000, 1000),
        "wary" => (-25, 900, 1250),
        "hostile" => (-75, 500, 2000),
        _ => (0, 1000, 1000),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketRule {
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub price_mult: Option<u64>,
}

impl MarketRule {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(v, &[], &["kind", "tag", "price_mult"])?;
        let has_kind = v.get("kind").filter(|x| **x != Value::Null).is_some();
        let has_tag = v.get("tag").filter(|x| **x != Value::Null).is_some();
        if !(has_kind ^ has_tag) {
            return Err(c::bad());
        }

        let kind = if let Some(kv) = v.get("kind").filter(|x| **x != Value::Null) {
            Some(ident_bound(kv)?.to_owned())
        } else {
            None
        };

        let tag = if let Some(tv) = v.get("tag").filter(|x| **x != Value::Null) {
            Some(ident_bound(tv)?.to_owned())
        } else {
            None
        };

        let price_mult = if let Some(pv) = v.get("price_mult").filter(|x| **x != Value::Null) {
            Some(uint_bound(pv, 1, 10000)?)
        } else {
            None
        };

        Ok(Self { kind, tag, price_mult })
    }

    pub fn to_value(&self) -> Value {
        let mut r = Value::Object(Vec::new());
        if let Some(ref k) = self.kind {
            set(&mut r, "kind", Value::String(k.clone()));
        }
        if let Some(ref t) = self.tag {
            set(&mut r, "tag", Value::String(t.clone()));
        }
        if let Some(pm) = self.price_mult {
            set(&mut r, "price_mult", Value::from(pm));
        }
        r
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketDef {
    pub buys: Option<Vec<MarketRule>>,
    pub sells: Option<Vec<MarketRule>>,
}

impl MarketDef {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(v, &[], &["buys", "sells"])?;

        let buys = if let Some(bv) = v.get("buys").filter(|x| **x != Value::Null) {
            let arr = bv.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_RULES {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for r in arr {
                list.push(MarketRule::parse(r)?);
            }
            Some(list)
        } else {
            None
        };

        let sells = if let Some(sv) = v.get("sells").filter(|x| **x != Value::Null) {
            let arr = sv.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_RULES {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for r in arr {
                list.push(MarketRule::parse(r)?);
            }
            Some(list)
        } else {
            None
        };

        Ok(Self { buys, sells })
    }

    pub fn to_value(&self) -> Value {
        let mut r = Value::Object(Vec::new());
        if let Some(ref b) = self.buys {
            let arr: Vec<Value> = b.iter().map(|x| x.to_value()).collect();
            set(&mut r, "buys", Value::Array(arr));
        }
        if let Some(ref s) = self.sells {
            let arr: Vec<Value> = s.iter().map(|x| x.to_value()).collect();
            set(&mut r, "sells", Value::Array(arr));
        }
        r
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationDef {
    pub target: String,
    pub stance: String,
    pub trust: Option<i64>,
}

impl RelationDef {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(v, &["target", "stance"], &["trust"])?;
        let target = ident_bound(c::get(v, "target")?)?.to_owned();
        let stance = text_bound(c::get(v, "stance")?, 16)?.to_owned();
        if !STANCES.contains(&stance.as_str()) {
            return Err(c::bad());
        }
        let trust = if let Some(tv) = v.get("trust").filter(|x| **x != Value::Null) {
            Some(int_bound(tv, -100, 100)?)
        } else {
            None
        };
        Ok(Self { target, stance, trust })
    }

    pub fn to_value(&self) -> Value {
        let mut r = obj! {
            "target" => self.target.clone(),
            "stance" => self.stance.clone(),
        };
        if let Some(t) = self.trust {
            set(&mut r, "trust", Value::Number(t.to_string()));
        }
        r
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityDef {
    pub id: String,
    pub label: String,
    pub description: String,
    pub home_places: Option<Vec<String>>,
    pub members: Option<Vec<String>>,
    pub customs: Option<Vec<String>>,
    pub relations: Option<Vec<RelationDef>>,
    pub market: Option<MarketDef>,
}

impl CommunityDef {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(v, &["id", "label", "description"], &["home_places", "members", "customs", "relations", "market"])?;
        let id = ident_bound(c::get(v, "id")?)?.to_owned();
        let label = text_bound(c::get(v, "label")?, 48)?.to_owned();
        let description = text_bound(c::get(v, "description")?, 240)?.to_owned();

        let home_places = if let Some(pv) = v.get("home_places").filter(|x| **x != Value::Null) {
            let arr = pv.as_array().ok_or_else(c::bad)?;
            if arr.len() > 16 {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for p in arr {
                list.push(ident_bound(p)?.to_owned());
            }
            Some(list)
        } else {
            None
        };

        let members = if let Some(mv) = v.get("members").filter(|x| **x != Value::Null) {
            let arr = mv.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_MEMBERS {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for m in arr {
                list.push(ident_bound(m)?.to_owned());
            }
            Some(list)
        } else {
            None
        };

        let customs = if let Some(cv) = v.get("customs").filter(|x| **x != Value::Null) {
            let arr = cv.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_CUSTOMS {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for cust in arr {
                list.push(text_bound(cust, 120)?.to_owned());
            }
            Some(list)
        } else {
            None
        };

        let relations = if let Some(rv) = v.get("relations").filter(|x| **x != Value::Null) {
            let arr = rv.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_RELATIONS {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for r in arr {
                let rel = RelationDef::parse(r)?;
                if rel.target == id || list.iter().any(|old: &RelationDef| old.target == rel.target) {
                    return Err(c::bad());
                }
                list.push(rel);
            }
            Some(list)
        } else {
            None
        };

        let market = if let Some(mv) = v.get("market").filter(|x| **x != Value::Null) {
            Some(MarketDef::parse(mv)?)
        } else {
            None
        };

        Ok(Self { id, label, description, home_places, members, customs, relations, market })
    }

    pub fn to_value(&self) -> Value {
        let mut r = obj! {
            "id" => self.id.clone(),
            "label" => self.label.clone(),
            "description" => self.description.clone(),
        };
        if let Some(ref hp) = self.home_places {
            let arr: Vec<Value> = hp.iter().map(|s| Value::String(s.clone())).collect();
            set(&mut r, "home_places", Value::Array(arr));
        }
        if let Some(ref m) = self.members {
            let arr: Vec<Value> = m.iter().map(|s| Value::String(s.clone())).collect();
            set(&mut r, "members", Value::Array(arr));
        }
        if let Some(ref c) = self.customs {
            let arr: Vec<Value> = c.iter().map(|s| Value::String(s.clone())).collect();
            set(&mut r, "customs", Value::Array(arr));
        }
        if let Some(ref rel) = self.relations {
            let arr: Vec<Value> = rel.iter().map(|x| x.to_value()).collect();
            set(&mut r, "relations", Value::Array(arr));
        }
        if let Some(ref mkt) = self.market {
            set(&mut r, "market", mkt.to_value());
        }
        r
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityWorld {
    pub communities: Vec<CommunityDef>,
}

impl Default for CommunityWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl CommunityWorld {
    pub fn new() -> Self {
        Self { communities: Vec::new() }
    }

    pub fn parse(val: Option<&Value>) -> Result<Self> {
        let mut communities = Vec::new();
        if let Some(cv) = val.filter(|x| **x != Value::Null) {
            let arr = cv.as_array().ok_or_else(c::bad)?;
            if arr.len() > MAX_COMMUNITIES {
                return Err(c::bad());
            }
            for item in arr {
                let comm = CommunityDef::parse(item)?;
                if communities.iter().any(|old: &CommunityDef| old.id == comm.id) {
                    return Err(c::bad());
                }
                communities.push(comm);
            }
        }
        Ok(Self { communities })
    }

    pub fn to_value(&self) -> Value {
        obj! {
            "communities" => Value::Array(self.communities.iter().map(|c| c.to_value()).collect()),
        }
    }

    pub fn from_value(v: &Value) -> Result<Self> {
        keys(v, &[], &["communities"])?;
        Self::parse(v.get("communities"))
    }

    pub fn get_community(&self, cid: &str) -> Option<&CommunityDef> {
        self.communities.iter().find(|c| c.id == cid)
    }

    pub fn get_community_mut(&mut self, cid: &str) -> Option<&mut CommunityDef> {
        self.communities.iter_mut().find(|c| c.id == cid)
    }

    pub fn get_relation_info(&self, cid: &str, target: &str) -> (String, i64, u64, u64) {
        if let Some(comm) = self.get_community(cid) {
            if let Some(ref relations) = comm.relations {
                for r in relations {
                    if r.target == target {
                        let (trust, buy_mult, sell_mult) = if let Some(t) = r.trust {
                            let (b, s) = multipliers_from_trust(t);
                            (t, b, s)
                        } else {
                            stance_defaults(&r.stance)
                        };
                        return (r.stance.clone(), trust, buy_mult, sell_mult);
                    }
                }
            }
        }
        let (t, b, s) = stance_defaults("neutral");
        ("neutral".to_string(), t, b, s)
    }

    fn match_rule<'a>(
        rules: &'a [MarketRule],
        thing_kind: &str,
        kind_tags: &[String],
        kind_category: Option<&str>,
    ) -> Option<&'a MarketRule> {
        for r in rules {
            if let Some(ref k) = r.kind {
                if k == thing_kind {
                    return Some(r);
                }
            }
            if let Some(ref t) = r.tag {
                if kind_tags.iter().any(|kt| kt == t) || kind_category == Some(t.as_str()) {
                    return Some(r);
                }
            }
        }
        None
    }

    fn calc_item_value(base_val: u64, condition: u64, quantity: u64, rule_mult: u64, rel_mult: u64) -> u64 {
        let nominal = (base_val * condition / 100) * quantity;
        if nominal == 0 {
            return 0;
        }
        let val = (nominal * rule_mult * rel_mult) / 1_000_000;
        if val == 0 && nominal > 0 {
            1
        } else {
            val
        }
    }

    pub fn op_check_relation(&self, community: &str, target: &str) -> Result<Value> {
        ident_bound(&Value::String(community.to_string()))?;
        ident_bound(&Value::String(target.to_string()))?;
        if self.get_community(community).is_none() {
            return Err(c::bad());
        }
        let (stance, trust, buy_mult, sell_mult) = self.get_relation_info(community, target);
        Ok(obj! {
            "ok" => true,
            "community" => community.to_string(),
            "target" => target.to_string(),
            "stance" => stance,
            "trust" => Value::Number(trust.to_string()),
            "buy_mult" => buy_mult,
            "sell_mult" => sell_mult,
        })
    }

    pub fn op_set_relation(
        &mut self,
        at: u64,
        community: &str,
        target: &str,
        opt_stance: Option<&str>,
        opt_trust: Option<i64>,
        opt_place: Option<&str>,
    ) -> Result<Value> {
        ident_bound(&Value::String(community.to_string()))?;
        ident_bound(&Value::String(target.to_string()))?;
        if community == target {
            return Err(c::bad());
        }
        if let Some(p) = opt_place {
            ident_bound(&Value::String(p.to_string()))?;
        }

        let (stance, trust) = match (opt_stance, opt_trust) {
            (None, None) => ("neutral".to_string(), 0i64),
            (None, Some(tr)) => {
                if !(-100..=100).contains(&tr) {
                    return Err(c::bad());
                }
                (stance_from_trust(tr).to_string(), tr)
            }
            (Some(st), None) => {
                if !STANCES.contains(&st) {
                    return Err(c::bad());
                }
                let (def_tr, _, _) = stance_defaults(st);
                (st.to_string(), def_tr)
            }
            (Some(st), Some(tr)) => {
                if !STANCES.contains(&st) || !(-100..=100).contains(&tr) {
                    return Err(c::bad());
                }
                (st.to_string(), tr)
            }
        };

        let comm = self.get_community_mut(community).ok_or_else(c::bad)?;
        if comm.relations.is_none() {
            comm.relations = Some(Vec::new());
        }
        let rels = comm.relations.as_mut().unwrap();
        let mut found = false;
        for r in rels.iter_mut() {
            if r.target == target {
                r.stance = stance.clone();
                r.trust = Some(trust);
                found = true;
                break;
            }
        }
        if !found {
            if rels.len() >= MAX_RELATIONS {
                return Err(c::bad());
            }
            rels.push(RelationDef { target: target.to_string(), stance: stance.clone(), trust: Some(trust) });
        }

        let mut actors = vec![community.to_string(), target.to_string()];
        actors.sort();

        let place_str = opt_place.unwrap_or("");
        let entry = obj! {
            "at" => at,
            "kind" => "relation_change",
            "summary" => format!("{community} relation to {target} is now {stance} (trust: {trust})."),
            "actors" => Value::Array(actors.into_iter().map(Value::String).collect()),
            "place" => place_str.to_string(),
            "things" => Value::Array(Vec::new()),
            "tags" => Value::Array(vec![
                Value::String("community".to_string()),
                Value::String("relation".to_string()),
                Value::String(stance.clone()),
            ]),
            "visibility" => "public",
            "witnesses" => Value::Array(Vec::new()),
            "phrases" => Value::Array(vec![
                Value::String(community.to_string()),
                Value::String(target.to_string()),
                Value::String(stance.clone()),
            ]),
            "facts" => obj! {
                "community" => community.to_string(),
                "target" => target.to_string(),
                "stance" => stance.clone(),
                "trust" => Value::Number(trust.to_string()),
            },
        };

        Ok(obj! {
            "ok" => true,
            "community" => community.to_string(),
            "target" => target.to_string(),
            "stance" => stance,
            "trust" => Value::Number(trust.to_string()),
            "entry" => entry,
        })
    }

    pub fn op_add_member(&mut self, at: u64, community: &str, actor: &str, opt_place: Option<&str>) -> Result<Value> {
        ident_bound(&Value::String(community.to_string()))?;
        ident_bound(&Value::String(actor.to_string()))?;
        if let Some(p) = opt_place {
            ident_bound(&Value::String(p.to_string()))?;
        }

        let comm = self.get_community_mut(community).ok_or_else(c::bad)?;
        if comm.members.is_none() {
            comm.members = Some(Vec::new());
        }
        let mems = comm.members.as_mut().unwrap();
        if !mems.iter().any(|m| m == actor) {
            if mems.len() >= MAX_MEMBERS {
                return Err(c::bad());
            }
            mems.push(actor.to_string());
        }

        let mut actors = vec![actor.to_string(), community.to_string()];
        actors.sort();

        let place_str = opt_place.unwrap_or("");
        let entry = obj! {
            "at" => at,
            "kind" => "community_member",
            "summary" => format!("{actor} joined {community}."),
            "actors" => Value::Array(actors.into_iter().map(Value::String).collect()),
            "place" => place_str.to_string(),
            "things" => Value::Array(Vec::new()),
            "tags" => Value::Array(vec![
                Value::String("community".to_string()),
                Value::String("member".to_string()),
                Value::String("joined".to_string()),
            ]),
            "visibility" => "public",
            "witnesses" => Value::Array(Vec::new()),
            "phrases" => Value::Array(vec![
                Value::String(community.to_string()),
                Value::String(actor.to_string()),
                Value::String("joined".to_string()),
            ]),
            "facts" => obj! {
                "community" => community.to_string(),
                "actor" => actor.to_string(),
                "action" => "join",
            },
        };

        Ok(obj! {
            "ok" => true,
            "community" => community.to_string(),
            "actor" => actor.to_string(),
            "entry" => entry,
        })
    }

    pub fn op_remove_member(
        &mut self,
        at: u64,
        community: &str,
        actor: &str,
        opt_place: Option<&str>,
    ) -> Result<Value> {
        ident_bound(&Value::String(community.to_string()))?;
        ident_bound(&Value::String(actor.to_string()))?;
        if let Some(p) = opt_place {
            ident_bound(&Value::String(p.to_string()))?;
        }

        let comm = self.get_community_mut(community).ok_or_else(c::bad)?;
        if let Some(ref mut mems) = comm.members {
            if let Some(pos) = mems.iter().position(|m| m == actor) {
                mems.remove(pos);
            }
        }

        let mut actors = vec![actor.to_string(), community.to_string()];
        actors.sort();

        let place_str = opt_place.unwrap_or("");
        let entry = obj! {
            "at" => at,
            "kind" => "community_member",
            "summary" => format!("{actor} left {community}."),
            "actors" => Value::Array(actors.into_iter().map(Value::String).collect()),
            "place" => place_str.to_string(),
            "things" => Value::Array(Vec::new()),
            "tags" => Value::Array(vec![
                Value::String("community".to_string()),
                Value::String("member".to_string()),
                Value::String("left".to_string()),
            ]),
            "visibility" => "public",
            "witnesses" => Value::Array(Vec::new()),
            "phrases" => Value::Array(vec![
                Value::String(community.to_string()),
                Value::String(actor.to_string()),
                Value::String("left".to_string()),
            ]),
            "facts" => obj! {
                "community" => community.to_string(),
                "actor" => actor.to_string(),
                "action" => "leave",
            },
        };

        Ok(obj! {
            "ok" => true,
            "community" => community.to_string(),
            "actor" => actor.to_string(),
            "entry" => entry,
        })
    }

    pub fn op_set_customs(
        &mut self,
        at: u64,
        community: &str,
        customs_val: &Value,
        opt_place: Option<&str>,
    ) -> Result<Value> {
        ident_bound(&Value::String(community.to_string()))?;
        if let Some(p) = opt_place {
            ident_bound(&Value::String(p.to_string()))?;
        }
        let arr = customs_val.as_array().ok_or_else(c::bad)?;
        if arr.len() > MAX_CUSTOMS {
            return Err(c::bad());
        }
        let mut list = Vec::new();
        for item in arr {
            list.push(text_bound(item, 120)?.to_owned());
        }

        let count = list.len();
        let comm = self.get_community_mut(community).ok_or_else(c::bad)?;
        comm.customs = Some(list.clone());

        let place_str = opt_place.unwrap_or("");
        let entry = obj! {
            "at" => at,
            "kind" => "community_customs",
            "summary" => format!("{community} updated its customs."),
            "actors" => Value::Array(vec![Value::String(community.to_string())]),
            "place" => place_str.to_string(),
            "things" => Value::Array(Vec::new()),
            "tags" => Value::Array(vec![
                Value::String("community".to_string()),
                Value::String("customs".to_string()),
            ]),
            "visibility" => "public",
            "witnesses" => Value::Array(Vec::new()),
            "phrases" => Value::Array(vec![
                Value::String(community.to_string()),
                Value::String("customs".to_string()),
            ]),
            "facts" => obj! {
                "community" => community.to_string(),
                "customs_count" => Value::from(count as u64),
            },
        };

        let customs_out = Value::Array(list.into_iter().map(Value::String).collect());
        Ok(obj! {
            "ok" => true,
            "community" => community.to_string(),
            "customs" => customs_out,
            "entry" => entry,
        })
    }

    #[allow(clippy::type_complexity)]
    fn evaluate_trade(
        &self,
        community: &str,
        actor: &str,
        offer_things: &[String],
        request_things: &[String],
        things_state: &Value,
        trader: Option<&str>,
    ) -> Result<(bool, Option<&'static str>, Option<String>, u64, u64, Option<Value>)> {
        ident_bound(&Value::String(community.to_string()))?;
        ident_bound(&Value::String(actor.to_string()))?;
        if let Some(tr) = trader {
            ident_bound(&Value::String(tr.to_string()))?;
        }
        let comm = self.get_community(community).ok_or_else(c::bad)?;

        if offer_things.is_empty() || offer_things.len() > 16 || request_things.is_empty() || request_things.len() > 16
        {
            return Err(c::bad());
        }

        for (i, t) in offer_things.iter().enumerate() {
            ident_bound(&Value::String(t.clone()))?;
            if offer_things[..i].iter().any(|old| old == t) {
                return Err(c::bad());
            }
        }
        for (i, t) in request_things.iter().enumerate() {
            ident_bound(&Value::String(t.clone()))?;
            if request_things[..i].iter().any(|old| old == t) {
                return Err(c::bad());
            }
        }
        if offer_things.iter().any(|ot| request_things.iter().any(|rt| rt == ot)) {
            return Err(c::bad());
        }

        let all_things = things_state.get("things").and_then(|v| v.as_array()).ok_or_else(c::bad)?;
        let all_kinds = things_state.get("kinds").and_then(|v| v.as_array()).ok_or_else(c::bad)?;

        let (_stance, trust, buy_mult, sell_mult) = self.get_relation_info(community, actor);
        if trust < -40 || _stance == "hostile" {
            return Ok((false, Some("hostile_relation"), None, 0, 0, None));
        }

        let empty_rules = Vec::new();
        let buys_rules = comm.market.as_ref().and_then(|m| m.buys.as_ref()).unwrap_or(&empty_rules);
        let sells_rules = comm.market.as_ref().and_then(|m| m.sells.as_ref()).unwrap_or(&empty_rules);

        // Evaluate offer
        let mut offer_value = 0u64;
        for ot_id in offer_things {
            let thing = all_things
                .iter()
                .find(|t| t.get("id").and_then(|v| v.as_str()) == Some(ot_id.as_str()))
                .ok_or_else(c::bad)?;
            let owner = thing.get("owner").and_then(|v| v.as_str());
            let loc_holder = thing.get("location").and_then(|l| l.get("holder")).and_then(|v| v.as_str());

            if owner != Some(actor) && loc_holder != Some(actor) {
                return Ok((false, Some("offer_thing_not_held"), Some(ot_id.clone()), 0, 0, None));
            }

            let kind_id = thing.get("kind").and_then(|v| v.as_str()).ok_or_else(c::bad)?;
            let kind =
                all_kinds.iter().find(|k| k.get("id").and_then(|v| v.as_str()) == Some(kind_id)).ok_or_else(c::bad)?;

            let base_val = kind.get("base_value").and_then(as_u64).unwrap_or(0);
            let condition = thing.get("condition").and_then(as_u64).unwrap_or(100);
            let quantity = thing.get("quantity").and_then(as_u64).unwrap_or(1);

            let empty_tags = Vec::new();
            let kind_tags: Vec<String> = kind
                .get("tags")
                .and_then(|v| v.as_array())
                .unwrap_or(&empty_tags)
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            let kind_category = kind.get("category").and_then(|v| v.as_str());

            let rule = Self::match_rule(buys_rules, kind_id, &kind_tags, kind_category);
            if !buys_rules.is_empty() && rule.is_none() {
                return Ok((false, Some("unwanted_goods"), Some(ot_id.clone()), 0, 0, None));
            }
            let rule_mult = rule.and_then(|r| r.price_mult).unwrap_or(1000);
            let val = Self::calc_item_value(base_val, condition, quantity, rule_mult, buy_mult);
            offer_value = offer_value.saturating_add(val);
        }

        // Evaluate request
        let mut request_value = 0u64;
        for rt_id in request_things {
            let thing = all_things
                .iter()
                .find(|t| t.get("id").and_then(|v| v.as_str()) == Some(rt_id.as_str()))
                .ok_or_else(c::bad)?;
            let owner = thing.get("owner").and_then(|v| v.as_str());
            let loc_holder = thing.get("location").and_then(|l| l.get("holder")).and_then(|v| v.as_str());

            let valid_owner =
                (owner == Some(community)) || (trader.is_some() && (owner == trader || loc_holder == trader));

            if !valid_owner {
                return Ok((false, Some("request_thing_not_available"), Some(rt_id.clone()), 0, 0, None));
            }

            let kind_id = thing.get("kind").and_then(|v| v.as_str()).ok_or_else(c::bad)?;
            let kind =
                all_kinds.iter().find(|k| k.get("id").and_then(|v| v.as_str()) == Some(kind_id)).ok_or_else(c::bad)?;

            let base_val = kind.get("base_value").and_then(as_u64).unwrap_or(0);
            let condition = thing.get("condition").and_then(as_u64).unwrap_or(100);
            let quantity = thing.get("quantity").and_then(as_u64).unwrap_or(1);

            let empty_tags = Vec::new();
            let kind_tags: Vec<String> = kind
                .get("tags")
                .and_then(|v| v.as_array())
                .unwrap_or(&empty_tags)
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            let kind_category = kind.get("category").and_then(|v| v.as_str());

            let rule = Self::match_rule(sells_rules, kind_id, &kind_tags, kind_category);
            let rule_mult = rule.and_then(|r| r.price_mult).unwrap_or(1000);
            let val = Self::calc_item_value(base_val, condition, quantity, rule_mult, sell_mult);
            request_value = request_value.saturating_add(val);
        }

        if offer_value >= request_value {
            return Ok((true, None, None, offer_value, request_value, None));
        }

        // Insufficient value -> build counter offer
        let deficit = request_value - offer_value;
        let mut counter_obj = obj! {
            "deficit" => deficit,
        };

        let mut cand_found: Option<String> = None;
        for t in all_things {
            let tid = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if offer_things.iter().any(|ot| ot == tid) {
                continue;
            }
            let owner = t.get("owner").and_then(|v| v.as_str());
            let loc_holder = t.get("location").and_then(|l| l.get("holder")).and_then(|v| v.as_str());

            if owner == Some(actor) || loc_holder == Some(actor) {
                let kind_id = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let kind = all_kinds.iter().find(|k| k.get("id").and_then(|v| v.as_str()) == Some(kind_id));
                if let Some(k) = kind {
                    let empty_tags = Vec::new();
                    let kind_tags: Vec<String> = k
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .unwrap_or(&empty_tags)
                        .iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect();
                    let kind_category = k.get("category").and_then(|v| v.as_str());
                    let rule = Self::match_rule(buys_rules, kind_id, &kind_tags, kind_category);
                    if !buys_rules.is_empty() && rule.is_none() {
                        continue;
                    }
                    let base_val = k.get("base_value").and_then(as_u64).unwrap_or(0);
                    let condition = t.get("condition").and_then(as_u64).unwrap_or(100);
                    let quantity = t.get("quantity").and_then(as_u64).unwrap_or(1);
                    let rule_mult = rule.and_then(|r| r.price_mult).unwrap_or(1000);
                    let val = Self::calc_item_value(base_val, condition, quantity, rule_mult, buy_mult);
                    if val >= deficit {
                        cand_found = Some(tid.to_string());
                        break;
                    }
                }
            }
        }

        if let Some(cf) = cand_found {
            set(&mut counter_obj, "add_offer", Value::Array(vec![Value::String(cf)]));
        } else if request_things.len() > 1 {
            let mut max_tid: Option<String> = None;
            let mut max_val = 0u64;
            for rt_id in request_things {
                if let Some(t) =
                    all_things.iter().find(|t| t.get("id").and_then(|v| v.as_str()) == Some(rt_id.as_str()))
                {
                    let kind_id = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(k) = all_kinds.iter().find(|k| k.get("id").and_then(|v| v.as_str()) == Some(kind_id)) {
                        let empty_tags = Vec::new();
                        let kind_tags: Vec<String> = k
                            .get("tags")
                            .and_then(|v| v.as_array())
                            .unwrap_or(&empty_tags)
                            .iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect();
                        let kind_category = k.get("category").and_then(|v| v.as_str());
                        let rule = Self::match_rule(sells_rules, kind_id, &kind_tags, kind_category);
                        let base_val = k.get("base_value").and_then(as_u64).unwrap_or(0);
                        let condition = t.get("condition").and_then(as_u64).unwrap_or(100);
                        let quantity = t.get("quantity").and_then(as_u64).unwrap_or(1);
                        let rule_mult = rule.and_then(|r| r.price_mult).unwrap_or(1000);
                        let val = Self::calc_item_value(base_val, condition, quantity, rule_mult, sell_mult);
                        if val > max_val {
                            max_val = val;
                            max_tid = Some(rt_id.clone());
                        }
                    }
                }
            }
            if let Some(mt) = max_tid {
                set(&mut counter_obj, "remove_request", Value::Array(vec![Value::String(mt)]));
            }
        }

        Ok((false, Some("insufficient_value"), None, offer_value, request_value, Some(counter_obj)))
    }

    pub fn op_appraise(
        &self,
        community: &str,
        actor: &str,
        offer_things: &[String],
        request_things: &[String],
        things_state: &Value,
        trader: Option<&str>,
    ) -> Result<Value> {
        let (ok, reason, details, ov, rv, counter) =
            self.evaluate_trade(community, actor, offer_things, request_things, things_state, trader)?;

        if ok {
            let diff = ov as i64 - rv as i64;
            Ok(obj! {
                "ok" => true,
                "acceptable" => true,
                "offer_value" => ov,
                "request_value" => rv,
                "difference" => Value::Number(diff.to_string()),
            })
        } else {
            let mut r = obj! {
                "ok" => true,
                "acceptable" => false,
                "reason" => reason.unwrap_or("unacceptable"),
                "offer_value" => ov,
                "request_value" => rv,
            };
            if let Some(c) = counter {
                set(&mut r, "counter_offer", c);
            }
            if let Some(d) = details {
                set(&mut r, "details", Value::String(d));
            }
            Ok(r)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn op_barter(
        &mut self,
        at: u64,
        community: &str,
        actor: &str,
        place: &str,
        offer_things: &[String],
        request_things: &[String],
        things_state: &mut Value,
        trader: Option<&str>,
    ) -> Result<Value> {
        ident_bound(&Value::String(place.to_string()))?;
        let (ok, reason, details, ov, rv, counter) =
            self.evaluate_trade(community, actor, offer_things, request_things, things_state, trader)?;

        if !ok {
            let mut r = obj! {
                "ok" => false,
                "reason" => reason.unwrap_or("trade_refused"),
                "offer_value" => ov,
                "request_value" => rv,
            };
            if let Some(c) = counter {
                set(&mut r, "counter_offer", c);
            }
            if let Some(d) = details {
                set(&mut r, "details", Value::String(d));
            }
            return Ok(r);
        }

        // Execute atomic transfer in things_state
        let target_owner_offer = trader.unwrap_or(community);
        let target_loc_offer = if let Some(tr) = trader {
            obj! { "holder" => tr }
        } else {
            obj! { "place" => place }
        };

        if let Some(all_things) = get_field_mut(things_state, "things").and_then(as_array_mut) {
            for t in all_things.iter_mut() {
                let tid = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if offer_things.iter().any(|ot| ot == tid) {
                    set(t, "owner", Value::String(target_owner_offer.to_string()));
                    set(t, "location", target_loc_offer.clone());
                } else if request_things.iter().any(|rt| rt == tid) {
                    set(t, "owner", Value::String(actor.to_string()));
                    set(t, "location", obj! { "holder" => actor });
                }
            }
        }

        // Generous trade trust boost
        if ov >= rv * 12 / 10 && ov - rv >= 10 {
            if let Some(comm) = self.get_community_mut(community) {
                if let Some(ref mut rels) = comm.relations {
                    for r in rels.iter_mut() {
                        if r.target == actor {
                            let cur_trust = r.trust.unwrap_or_else(|| stance_defaults(&r.stance).0);
                            let new_trust = (cur_trust + 2).min(100);
                            r.trust = Some(new_trust);
                            r.stance = stance_from_trust(new_trust).to_string();
                            break;
                        }
                    }
                }
            }
        }

        let partner = trader.unwrap_or(community);
        let mut actors = if partner == actor {
            vec![actor.to_string()]
        } else {
            let mut v = vec![actor.to_string(), partner.to_string()];
            v.sort();
            v
        };
        actors.dedup();

        let mut all_traded = offer_things.to_vec();
        all_traded.extend(request_things.to_vec());
        all_traded.sort();

        let entry = obj! {
            "at" => at,
            "kind" => "barter",
            "summary" => format!("{actor} bartered with {partner} at {place}."),
            "actors" => Value::Array(actors.into_iter().map(Value::String).collect()),
            "place" => place.to_string(),
            "things" => Value::Array(all_traded.into_iter().map(Value::String).collect()),
            "tags" => Value::Array(vec![
                Value::String("community".to_string()),
                Value::String("barter".to_string()),
                Value::String("trade".to_string()),
            ]),
            "visibility" => "public",
            "witnesses" => Value::Array(Vec::new()),
            "phrases" => Value::Array(vec![
                Value::String(community.to_string()),
                Value::String(actor.to_string()),
                Value::String("barter".to_string()),
            ]),
            "facts" => obj! {
                "community" => community.to_string(),
                "offer_value" => ov,
                "request_value" => rv,
                "offered" => Value::Array(offer_things.iter().map(|s| Value::String(s.clone())).collect()),
                "requested" => Value::Array(request_things.iter().map(|s| Value::String(s.clone())).collect()),
            },
        };

        Ok(obj! {
            "ok" => true,
            "offer_value" => ov,
            "request_value" => rv,
            "transferred_offer" => Value::Array(offer_things.iter().map(|s| Value::String(s.clone())).collect()),
            "transferred_request" => Value::Array(request_things.iter().map(|s| Value::String(s.clone())).collect()),
            "entry" => entry,
        })
    }

    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
        keys(req, &["ops"], &["communities", "things"])?;
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        if req.get("communities").is_some() {
            *self = Self::parse(req.get("communities"))?;
        }

        let mut things_state = if let Some(th) = req.get("things").filter(|x| **x != Value::Null) {
            th.clone()
        } else {
            obj! {
                "version" => 1u64,
                "materials" => Vec::<Value>::new(),
                "kinds" => Vec::<Value>::new(),
                "things" => Vec::<Value>::new(),
            }
        };

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

            let comm_snapshot = self.clone();
            let things_snapshot = things_state.clone();

            match op_name {
                "check_relation" => {
                    keys(args, &["community", "target"], &[])?;
                    let comm = ident_bound(c::get(args, "community")?)?;
                    let target = ident_bound(c::get(args, "target")?)?;
                    results.push(self.op_check_relation(comm, target)?);
                }
                "set_relation" => {
                    keys(args, &["community", "target"], &["stance", "trust", "place"])?;
                    let comm = ident_bound(c::get(args, "community")?)?;
                    let target = ident_bound(c::get(args, "target")?)?;
                    let stance =
                        args.get("stance").filter(|x| **x != Value::Null).map(|v| text_bound(v, 16)).transpose()?;
                    let trust = args
                        .get("trust")
                        .filter(|x| **x != Value::Null)
                        .map(|v| int_bound(v, -100, 100))
                        .transpose()?;
                    let place = args.get("place").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
                    results.push(self.op_set_relation(at, comm, target, stance, trust, place)?);
                }
                "add_member" => {
                    keys(args, &["community", "actor"], &["place"])?;
                    let comm = ident_bound(c::get(args, "community")?)?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let place = args.get("place").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
                    results.push(self.op_add_member(at, comm, actor, place)?);
                }
                "remove_member" => {
                    keys(args, &["community", "actor"], &["place"])?;
                    let comm = ident_bound(c::get(args, "community")?)?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let place = args.get("place").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
                    results.push(self.op_remove_member(at, comm, actor, place)?);
                }
                "set_customs" => {
                    keys(args, &["community", "customs"], &["place"])?;
                    let comm = ident_bound(c::get(args, "community")?)?;
                    let customs = c::get(args, "customs")?;
                    let place = args.get("place").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
                    results.push(self.op_set_customs(at, comm, customs, place)?);
                }
                "appraise" => {
                    keys(args, &["community", "actor", "offer_things", "request_things"], &["trader", "place"])?;
                    let comm = ident_bound(c::get(args, "community")?)?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let ot_arr = c::get(args, "offer_things")?.as_array().ok_or_else(c::bad)?;
                    let mut offer_things = Vec::new();
                    for ot in ot_arr {
                        offer_things.push(ident_bound(ot)?.to_owned());
                    }
                    let rt_arr = c::get(args, "request_things")?.as_array().ok_or_else(c::bad)?;
                    let mut request_things = Vec::new();
                    for rt in rt_arr {
                        request_things.push(ident_bound(rt)?.to_owned());
                    }
                    let trader = args.get("trader").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
                    results.push(self.op_appraise(
                        comm,
                        actor,
                        &offer_things,
                        &request_things,
                        &things_state,
                        trader,
                    )?);
                }
                "barter" => {
                    keys(args, &["community", "actor", "place", "offer_things", "request_things"], &["trader"])?;
                    let comm = ident_bound(c::get(args, "community")?)?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    let ot_arr = c::get(args, "offer_things")?.as_array().ok_or_else(c::bad)?;
                    let mut offer_things = Vec::new();
                    for ot in ot_arr {
                        offer_things.push(ident_bound(ot)?.to_owned());
                    }
                    let rt_arr = c::get(args, "request_things")?.as_array().ok_or_else(c::bad)?;
                    let mut request_things = Vec::new();
                    for rt in rt_arr {
                        request_things.push(ident_bound(rt)?.to_owned());
                    }
                    let trader = args.get("trader").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;

                    let res = self.op_barter(
                        at,
                        comm,
                        actor,
                        place,
                        &offer_things,
                        &request_things,
                        &mut things_state,
                        trader,
                    )?;
                    if res.get("ok").and_then(|v| v.as_bool()) != Some(true) {
                        *self = comm_snapshot;
                        things_state = things_snapshot;
                    }
                    results.push(res);
                }
                _ => return Err(c::bad()),
            }
        }

        let comm_val = Value::Array(self.communities.iter().map(|c| c.to_value()).collect());
        Ok(obj! {
            "results" => results,
            "communities" => comm_val,
            "things" => things_state,
        })
    }
}

pub fn workflow(a: &Value) -> Result<Value> {
    keys(a, &["ops"], &["communities", "things"])?;
    let mut cw = CommunityWorld::parse(a.get("communities"))?;
    cw.workflow(a)
}
