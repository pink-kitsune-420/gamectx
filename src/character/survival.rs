//! Life cycle and threat response (pass 2, C1/C2).
//!
//! **Life cycle (C1).** A being is `born` with a lifespan in ticks and is a `child` below 22.5%
//! of it, an `adult` below 75% and an `elder` after; `age` reports `came_of_age` and `grew_old`,
//! and a being whose age reaches its lifespan dies of `old_age`. Adults `pair` and have
//! children (`born` with parents); beings `own` belongings. A death (`age` or `die`) passes each
//! belonging, in order, to the next heir with room (living children by birth then id, else the
//! living partner, else living parents), and proposes grief for the bereaved (affect `emotion`:
//! partner −60, child −50, parent −40).
//!
//! **Threat response (C2).** `threat` appraises danger
//! `clamp(danger·10,000 + (threat strength − strength)·3,000 + injury·2,000, 0, 1,000,000)` and,
//! from 200,000 up, scores (millionths):
//!
//! - fight `300,000 + (strength − threat strength)·5,000 + min(allies, 5)·100,000 − injury·4,000`
//! - flee `(exit ? 400,000 : 0) + danger/2 − injury·3,000`
//! - hide `(cover ? 450,000 : 0) + danger/3`
//! - call for help `allies > 0 ? 250,000 + min(allies, 5)·50,000 : 0`
//! - freeze `danger/2 − 200,000 + injury·2,000`
//!
//! plus a temperament offset (`bold`, `steady`, `cautious`, `timid`). The highest wins (ties in
//! that order). Safety is a need: a threat lowers it to at most `100 − danger/10,000`, it recovers
//! by one per tick on `age`, and below 40 the being is `unsafe`. Fear is proposed to affect.
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::survival::Survival;
//!
//! let mut world = Survival::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "born", "args": {"being": "shepherd", "lifespan": 800, "at": 0}},
//!   {"op": "age", "args": {"now": 200}},
//!   {"op": "threat", "args": {"being": "shepherd", "threat": "wolf", "danger": 70, "temperament": "cautious",
//!                             "threat_strength": 60, "strength": 40, "cover": true, "exit": false}}
//! ]}"#, 4096)?;
//! let out = world.workflow(&req)?.to_json();
//! assert!(out.contains(r#""event":"came_of_age""#) && out.contains(r#""response":"hide""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_bool, opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "belongings", default: 64, ceiling: 4096 },
    Limit { name: "children", default: 16, ceiling: 256 },
];
pub const M: i64 = 1_000_000;
pub const ADULT_AT: u128 = 225_000;
pub const ELDER_AT: u128 = 750_000;
pub const IGNORE_BELOW: i64 = 200_000;
pub const UNSAFE_BELOW: u64 = 40;
pub const RESPONSES: [&str; 5] = ["fight", "flee", "hide", "call_for_help", "freeze"];
/// Temperament offsets in [`RESPONSES`] order.
pub const TEMPERS: [(&str, [i64; 5]); 4] = [
    ("bold", [250_000, -100_000, -100_000, 0, -200_000]),
    ("steady", [0, 0, 0, 0, 0]),
    ("cautious", [-150_000, 100_000, 150_000, 100_000, 0]),
    ("timid", [-300_000, 50_000, 100_000, 50_000, 250_000]),
];

fn grief(rel: &str) -> i64 {
    match rel {
        "partner" => 60,
        "child" => 50,
        _ => 40,
    }
}

pub fn stage(age: u64, lifespan: u64) -> &'static str {
    let f = age as u128 * M as u128 / lifespan as u128;
    if f < ADULT_AT {
        "child"
    } else if f < ELDER_AT {
        "adult"
    } else {
        "elder"
    }
}

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

fn ids(xs: &[String]) -> Value {
    Value::Array(xs.iter().map(|x| Value::from(x.as_str())).collect())
}

fn opt_s(x: &Option<String>) -> Value {
    x.as_deref().map(Value::from).unwrap_or(Value::Null)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Being {
    pub born: u64,
    pub lifespan: u64,
    pub parents: Vec<String>,
    pub partner: Option<String>,
    pub children: Vec<String>,
    pub belongings: Vec<String>,
    pub alive: bool,
    pub died: Option<u64>,
    pub cause: Option<String>,
    pub safety: u64,
    pub stage: &'static str,
    pub last: u64,
}

/// Beings' lives, families, belongings and sense of safety.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Survival {
    pub beings: BTreeMap<String, Being>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Survival {
    fn default() -> Self {
        Survival { beings: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

impl Survival {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "born" => self.op_born(args, l),
            "pair" => self.op_pair(args),
            "own" => self.op_own(args, l),
            "age" => self.op_age(args, l),
            "die" => self.op_die(args, l),
            "threat" => self.op_threat(args),
            "assess" => self.op_assess(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn living(&self, b: &str) -> Result<&Being> {
        self.beings.get(b).filter(|x| x.alive).ok_or_else(c::bad)
    }

    fn view(&self, b: &str) -> Value {
        let Some(x) = self.beings.get(b) else { return Value::Null };
        obj! {
            "alive" => x.alive, "being" => b, "belongings" => ids(&x.belongings), "born" => x.born,
            "cause" => opt_s(&x.cause), "children" => ids(&x.children),
            "died" => x.died.map(Value::from).unwrap_or(Value::Null), "lifespan" => x.lifespan,
            "parents" => ids(&x.parents), "partner" => opt_s(&x.partner), "safety" => x.safety, "stage" => x.stage,
        }
    }

    // ------------------------------------------------------------ C1

    fn op_born(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "lifespan"], &["parents", "at"])?;
        let b = id_of(args, "being")?;
        let lifespan = uint_bound(c::get(args, "lifespan")?, 1, MAX_AT)?;
        let at = opt_uint(args, "at", 0, MAX_AT, self.clock)?;
        let rows = match opt(args, "parents") {
            None => &[][..],
            Some(v) => v.as_array().ok_or_else(c::bad)?,
        };
        need(rows.len() <= 2)?;
        let parents = rows.iter().map(|v| ident(v).map(str::to_string)).collect::<Result<Vec<_>>>()?;
        need(parents.iter().collect::<BTreeSet<_>>().len() == parents.len())?;
        need(!parents.contains(&b) && !self.beings.contains_key(&b) && self.beings.len() < l.get("beings"))?;
        for p in &parents {
            let px = self.living(p)?;
            need(px.stage != "child" && px.children.len() < l.get("children"))?;
        }
        for p in &parents {
            self.beings.get_mut(p).ok_or_else(c::bad)?.children.push(b.clone());
        }
        let x = Being {
            born: at,
            lifespan,
            parents,
            partner: None,
            children: Vec::new(),
            belongings: Vec::new(),
            alive: true,
            died: None,
            cause: None,
            safety: 100,
            stage: stage(self.clock.saturating_sub(at), lifespan),
            last: at.max(self.clock),
        };
        self.beings.insert(b.clone(), x);
        Ok(self.view(&b))
    }

    fn op_pair(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["a", "b"], &[])?;
        let x = id_of(args, "a")?;
        let y = id_of(args, "b")?;
        need(x != y)?;
        let (bx, by) = (self.living(&x)?, self.living(&y)?);
        need(bx.stage != "child" && by.stage != "child" && bx.partner.is_none() && by.partner.is_none())?;
        self.beings.get_mut(&x).ok_or_else(c::bad)?.partner = Some(y.clone());
        self.beings.get_mut(&y).ok_or_else(c::bad)?.partner = Some(x.clone());
        Ok(obj! { "a" => x, "b" => y })
    }

    fn op_own(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "thing"], &[])?;
        let b = id_of(args, "being")?;
        let t = id_of(args, "thing")?;
        let x = self.living(&b)?;
        need(!x.belongings.contains(&t) && x.belongings.len() < l.get("belongings"))?;
        let x = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        x.belongings.push(t);
        Ok(obj! { "being" => b.as_str(), "belongings" => ids(&x.belongings) })
    }

    fn death(&mut self, b: &str, at: u64, cause: &str, l: &Limits) -> Result<Value> {
        let x = self.beings.get(b).ok_or_else(c::bad)?.clone();
        let alive = |n: &String| self.beings.get(n).is_some_and(|y| y.alive);
        let mut kids: Vec<String> = x.children.iter().filter(|c| alive(c)).cloned().collect();
        kids.sort_by_key(|c| (self.beings[c].born, c.clone()));
        let partner = x.partner.clone().filter(|p| alive(p));
        let mut parents: Vec<String> = x.parents.iter().filter(|p| alive(p)).cloned().collect();
        parents.sort();
        let heirs: Vec<String> = if !kids.is_empty() {
            kids.clone()
        } else if let Some(p) = &partner {
            vec![p.clone()]
        } else {
            parents.clone()
        };
        let cap = l.get("belongings");
        let mut estate = Vec::new();
        let mut i = 0usize;
        for thing in &x.belongings {
            let mut to = None;
            for k in 0..heirs.len() {
                let h = &heirs[(i + k) % heirs.len()];
                let hx = &self.beings[h];
                if !hx.belongings.contains(thing) && hx.belongings.len() < cap {
                    to = Some(h.clone());
                    i = (i + k + 1) % heirs.len();
                    break;
                }
            }
            if let Some(h) = &to {
                self.beings.get_mut(h).ok_or_else(c::bad)?.belongings.push(thing.clone());
            }
            estate.push(obj! { "thing" => thing.as_str(), "to" => opt_s(&to) });
        }
        let mut bereaved: Vec<(String, &'static str)> = Vec::new();
        if let Some(p) = &partner {
            bereaved.push((p.clone(), "partner"));
        }
        bereaved.extend(kids.iter().map(|c| (c.clone(), "child")));
        bereaved.extend(parents.iter().map(|p| (p.clone(), "parent")));
        bereaved.sort();
        let effects = bereaved
            .iter()
            .map(|(w, r)| {
                obj! {
                    "engine" => "affect", "op" => "appraise",
                    "args" => obj! { "being" => w.as_str(), "subject" => b, "kind" => "emotion", "delta" => -grief(r), "reason" => "grief" },
                }
            })
            .collect();
        if let Some(p) = &partner {
            self.beings.get_mut(p).ok_or_else(c::bad)?.partner = None;
        }
        let me = self.beings.get_mut(b).ok_or_else(c::bad)?;
        me.alive = false;
        me.died = Some(at);
        me.cause = Some(cause.to_string());
        me.belongings.clear();
        me.partner = None;
        Ok(obj! {
            "being" => b,
            "bereaved" => Value::Array(bereaved.iter().map(|(w, r)| obj! { "being" => w.as_str(), "as" => *r }).collect()),
            "cause" => cause, "died" => at, "effects" => Value::Array(effects), "estate" => Value::Array(estate),
        })
    }

    fn op_die(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "cause"], &["at"])?;
        let b = id_of(args, "being")?;
        let cause = id_of(args, "cause")?;
        let at = opt_uint(args, "at", 0, MAX_AT, self.clock)?;
        self.living(&b)?;
        self.death(&b, at, &cause, l)
    }

    fn op_age(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        let (mut events, mut dying) = (Vec::new(), Vec::new());
        for (b, x) in self.beings.iter_mut().filter(|(_, x)| x.alive) {
            if now > x.last {
                x.safety = (x.safety + (now - x.last)).min(100);
                x.last = now;
            }
            let age = now.saturating_sub(x.born);
            if age >= x.lifespan {
                dying.push(b.clone());
                continue;
            }
            let s = stage(age, x.lifespan);
            if s != x.stage {
                events.push(
                    obj! { "being" => b.as_str(), "event" => if s == "adult" { "came_of_age" } else { "grew_old" } },
                );
                x.stage = s;
            }
        }
        let mut deaths = Vec::new();
        for b in dying {
            if self.beings.get(&b).is_some_and(|x| x.alive) {
                deaths.push(self.death(&b, now, "old_age", l)?);
            }
        }
        self.clock = now;
        Ok(obj! { "deaths" => Value::Array(deaths), "events" => Value::Array(events), "now" => now })
    }

    // ------------------------------------------------------------ C2

    fn op_threat(&mut self, args: &Value) -> Result<Value> {
        keys(
            args,
            &["being", "threat", "danger"],
            &["temperament", "strength", "threat_strength", "injury", "allies", "exit", "cover"],
        )?;
        let b = id_of(args, "being")?;
        let th = id_of(args, "threat")?;
        let danger = uint_bound(c::get(args, "danger")?, 0, 100)? as i64;
        let t = opt(args, "temperament").map(ident).transpose()?.unwrap_or("steady");
        let offs = TEMPERS.iter().find(|x| x.0 == t).map(|x| x.1).ok_or_else(c::bad)?;
        let st = opt_uint(args, "strength", 0, 100, 50)? as i64;
        let ts = opt_uint(args, "threat_strength", 0, 100, 50)? as i64;
        let inj = opt_uint(args, "injury", 0, 100, 0)? as i64;
        let allies = opt_uint(args, "allies", 0, 1000, 0)? as i64;
        let exit = opt_bool(args, "exit", true)?;
        let cover = opt_bool(args, "cover", false)?;
        self.living(&b)?;
        let d = (danger * 10_000 + (ts - st) * 3_000 + inj * 2_000).clamp(0, M);
        let helpers = allies.min(5);
        let base = [
            300_000 + (st - ts) * 5_000 + helpers * 100_000 - inj * 4_000,
            if exit { 400_000 } else { 0 } + d / 2 - inj * 3_000,
            if cover { 450_000 } else { 0 } + d / 3,
            if allies > 0 { 250_000 + helpers * 50_000 } else { 0 },
            d / 2 - 200_000 + inj * 2_000,
        ];
        let scores: Vec<i64> = base.iter().zip(offs.iter()).map(|(a, o)| a + o).collect();
        let x = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        let (response, effects) = if d < IGNORE_BELOW {
            ("ignore", Vec::new())
        } else {
            // Highest score; ties keep the earlier response.
            let mut best = 0;
            for i in 1..RESPONSES.len() {
                if scores[i] > scores[best] {
                    best = i;
                }
            }
            x.safety = x.safety.min(100 - (d / 10_000) as u64);
            let effect = obj! {
                "engine" => "affect", "op" => "appraise",
                "args" => obj! { "being" => b.as_str(), "subject" => th.as_str(), "kind" => "fear", "delta" => d / 20_000, "reason" => RESPONSES[best] },
            };
            (RESPONSES[best], vec![effect])
        };
        let rows = RESPONSES.iter().zip(&scores).map(|(r, s)| obj! { "response" => *r, "score" => *s }).collect();
        Ok(obj! {
            "being" => b.as_str(), "danger" => d, "effects" => Value::Array(effects), "response" => response,
            "safety" => x.safety, "scores" => Value::Array(rows), "threat" => th, "unsafe" => x.safety < UNSAFE_BELOW,
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        need(self.beings.contains_key(&b))?;
        Ok(self.view(&b))
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Survival::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(m: &mut Survival, ops: &str) -> Result<String> {
        m.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16)?).map(|v| v.to_json())
    }

    #[test]
    fn an_elder_dies_of_age_and_the_child_inherits() {
        let mut m = Survival::new();
        let out = run(
            &mut m,
            r#"{"op":"born","args":{"being":"gran","lifespan":100,"at":0}},
               {"op":"age","args":{"now":30}},
               {"op":"born","args":{"being":"kid","lifespan":100,"parents":["gran"],"at":30}},
               {"op":"own","args":{"being":"gran","thing":"loom"}},
               {"op":"age","args":{"now":100}}"#,
        )
        .unwrap();
        assert!(out.contains(r#""cause":"old_age""#) && out.contains(r#""to":"kid""#), "{out}");
        assert_eq!(m.beings["kid"].belongings, vec!["loom".to_string()]);
    }
}
