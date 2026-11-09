//! Economy feedback: prices that follow scarcity (pass 2, C4).
//!
//! Goods have a base price, an elasticity, a floor and a ceiling for the price multiplier, a
//! smoothing rate and a spoilage rate (all per-mille; a multiplier of 1000 is the base price, the
//! same unit as community market `price_mult`). Each market keeps stock per good. During a
//! period beings `produce` (off-screen work adds stock), `want` (needs register demand) and `buy`
//! (stock goes down, wants are met). `settle` closes the period for every market: with `supply =
//! stock + sold` (what was on offer this period), scarcity pressure `p = (demand − supply)·1000 /
//! max(1, demand + supply)` (truncated toward zero) gives a target multiplier `clamp(1000 + p·elasticity/1000, floor, ceiling)`, and the multiplier moves
//! toward it by `rate` per-mille of the gap. A good at or above 1500 is `scarce`: beings whose
//! wants went unmet get an aspiration `need` proposal and its producers an `opportunity`
//! proposal. At or below 700 it is a `glut`. Then wants and production reset and stock spoils.
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::economy::Economy;
//!
//! let mut town = Economy::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "good", "args": {"id": "flour", "base": 20, "rate": 1000}},
//!   {"op": "produce", "args": {"market": "square", "good": "flour", "amount": 10, "by": "miller"}},
//!   {"op": "want", "args": {"market": "square", "good": "flour", "amount": 30, "by": "baker"}},
//!   {"op": "buy", "args": {"market": "square", "good": "flour", "amount": 10, "by": "baker"}},
//!   {"op": "settle", "args": {"now": 24}}
//! ]}"#, 4096)?;
//! let out = town.workflow(&req)?.to_json();
//! // Demand 30 against a supply of 10: pressure 500, and with rate 1000 the multiplier jumps to 1750.
//! assert!(out.contains(r#""mult":1750"#) && out.contains(r#""state":"scarce""#));
//! assert!(out.contains(r#"{"being":"baker","kind":"need","subject":"flour","weight":30}"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "goods", default: 64, ceiling: 4096 },
    Limit { name: "markets", default: 16, ceiling: 1024 },
    Limit { name: "traders", default: 256, ceiling: 65_536 },
];
/// Largest single amount produced, wanted or bought.
pub const MAX_AMOUNT: u64 = 1_000_000;
/// A multiplier at or above this is scarce (per-mille).
pub const SCARCE_AT: i64 = 1500;
/// A multiplier at or below this is a glut (per-mille).
pub const GLUT_AT: i64 = 700;

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

/// A tradeable good.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Good {
    pub base: u64,
    pub elasticity: i64,
    pub floor: i64,
    pub ceiling: i64,
    pub rate: i64,
    pub spoil: u64,
}

/// One good in one market for the current period.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stall {
    pub stock: u64,
    pub mult: i64,
    pub demand: u64,
    pub sold: u64,
    pub wants: BTreeMap<String, u64>,
    pub made: BTreeMap<String, u64>,
    /// Beings who produced or wanted this period (`wants` ∪ `made`), for the traders limit.
    pub traders: BTreeSet<String>,
}

impl Default for Stall {
    fn default() -> Self {
        Stall {
            stock: 0,
            mult: 1000,
            demand: 0,
            sold: 0,
            wants: BTreeMap::new(),
            made: BTreeMap::new(),
            traders: BTreeSet::new(),
        }
    }
}

/// Goods, markets and the clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Economy {
    pub goods: BTreeMap<String, Good>,
    pub markets: BTreeMap<String, BTreeMap<String, Stall>>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Economy {
    fn default() -> Self {
        Economy { goods: BTreeMap::new(), markets: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

impl Economy {
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
            "good" => self.op_good(args, l),
            "produce" => self.add_flow(args, l, true),
            "want" => self.add_flow(args, l, false),
            "buy" => self.op_buy(args),
            "settle" => self.op_settle(args),
            "price" => self.op_price(args),
            "assess" => self.op_assess(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn price(&self, good: &str, s: &Stall) -> u64 {
        self.goods[good].base * s.mult as u64 / 1000
    }

    fn view(&self, market: &str, good: &str) -> Vec<(String, Value)> {
        let s = &self.markets[market][good];
        vec![
            ("demand".into(), Value::from(s.demand)),
            ("good".into(), Value::from(good)),
            ("market".into(), Value::from(market)),
            ("mult".into(), Value::from(s.mult)),
            ("price".into(), Value::from(self.price(good, s))),
            ("sold".into(), Value::from(s.sold)),
            ("stock".into(), Value::from(s.stock)),
        ]
    }

    fn op_good(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "base"], &["elasticity", "floor", "ceiling", "rate", "spoil"])?;
        let id = id_of(args, "id")?;
        let good = Good {
            base: uint_bound(c::get(args, "base")?, 1, 1_000_000_000)?,
            elasticity: opt_uint(args, "elasticity", 0, 10_000, 1500)? as i64,
            floor: opt_uint(args, "floor", 1, 1000, 250)? as i64,
            ceiling: opt_uint(args, "ceiling", 1000, 100_000, 4000)? as i64,
            rate: opt_uint(args, "rate", 1, 1000, 500)? as i64,
            spoil: opt_uint(args, "spoil", 0, 1000, 0)?,
        };
        need(self.goods.contains_key(&id) || self.goods.len() < l.get("goods"))?;
        let out = obj! {
            "id" => id.as_str(), "base" => good.base, "elasticity" => good.elasticity, "floor" => good.floor,
            "ceiling" => good.ceiling, "rate" => good.rate, "spoil" => good.spoil,
        };
        self.goods.insert(id, good);
        Ok(out)
    }

    /// `produce` (stock) or `want` (demand): validated in full before anything is created.
    fn add_flow(&mut self, args: &Value, l: &Limits, produce: bool) -> Result<Value> {
        keys(args, &["market", "good", "amount", "by"], &[])?;
        let m = id_of(args, "market")?;
        let g = id_of(args, "good")?;
        let b = id_of(args, "by")?;
        let amount = uint_bound(c::get(args, "amount")?, 1, MAX_AMOUNT)?;
        need(self.goods.contains_key(&g) && (self.markets.contains_key(&m) || self.markets.len() < l.get("markets")))?;
        if let Some(s) = self.markets.get(&m).and_then(|x| x.get(&g)) {
            need(s.traders.contains(&b) || s.traders.len() < l.get("traders"))?;
            need((if produce { s.stock } else { s.demand }) + amount <= MAX_AT)?;
        }
        let s = self.markets.entry(m.clone()).or_default().entry(g.clone()).or_default();
        s.traders.insert(b.clone());
        if produce {
            s.stock += amount;
            *s.made.entry(b).or_default() += amount;
        } else {
            s.demand += amount;
            *s.wants.entry(b).or_default() += amount;
        }
        Ok(Value::Object(self.view(&m, &g)))
    }

    fn op_buy(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["market", "good", "amount", "by"], &[])?;
        let m = id_of(args, "market")?;
        let g = id_of(args, "good")?;
        let b = id_of(args, "by")?;
        let amount = uint_bound(c::get(args, "amount")?, 1, MAX_AMOUNT)?;
        let s = self.markets.get(&m).and_then(|x| x.get(&g)).ok_or_else(c::bad)?;
        need(s.stock >= amount)?;
        let cost = amount * self.price(&g, s);
        let s = self.markets.get_mut(&m).and_then(|x| x.get_mut(&g)).ok_or_else(c::bad)?;
        if let Some(w) = s.wants.get_mut(&b) {
            *w -= amount.min(*w);
        }
        s.stock -= amount;
        s.sold += amount;
        let mut v = self.view(&m, &g);
        v.extend([
            ("amount".into(), Value::from(amount)),
            ("by".into(), Value::from(b)),
            ("cost".into(), Value::from(cost)),
        ]);
        Ok(Value::Object(v))
    }

    fn op_settle(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        let (mut goods_out, mut effects) = (Vec::new(), Vec::new());
        for (m, stalls) in self.markets.iter_mut() {
            for (g, s) in stalls.iter_mut() {
                let gd = self.goods.get(g).ok_or_else(c::bad)?;
                let supply = s.stock + s.sold;
                let p = (s.demand as i64 - supply as i64) * 1000 / (s.demand + supply).max(1) as i64;
                let target = (1000 + p * gd.elasticity / 1000).clamp(gd.floor, gd.ceiling);
                let before = s.mult;
                s.mult = before + (target - before) * gd.rate / 1000;
                let state = if s.mult >= SCARCE_AT {
                    "scarce"
                } else if s.mult <= GLUT_AT {
                    "glut"
                } else {
                    "steady"
                };
                let unmet: Vec<(&String, u64)> =
                    s.wants.iter().filter(|(_, w)| **w > 0).map(|(b, w)| (b, *w)).collect();
                if state == "scarce" {
                    for (b, w) in &unmet {
                        effects.push(experience(b, "need", g, (10 + *w).min(100)));
                    }
                    for b in s.made.keys() {
                        effects.push(experience(b, "opportunity", g, (s.mult / 50).min(100) as u64));
                    }
                }
                let price = gd.base * s.mult as u64 / 1000;
                goods_out.push(obj! {
                    "demand" => s.demand, "from" => before, "good" => g.as_str(), "market" => m.as_str(),
                    "mult" => s.mult, "pressure" => p, "price" => price, "sold" => s.sold, "state" => state,
                    "stock" => s.stock, "target" => target, "unmet" => unmet.iter().map(|(_, w)| w).sum::<u64>(),
                });
                s.stock -= s.stock * gd.spoil / 1000;
                s.demand = 0;
                s.sold = 0;
                s.wants.clear();
                s.made.clear();
                s.traders.clear();
            }
        }
        self.clock = now;
        Ok(obj! { "effects" => Value::Array(effects), "goods" => Value::Array(goods_out), "now" => now })
    }

    fn op_price(&self, args: &Value) -> Result<Value> {
        keys(args, &["market", "good"], &[])?;
        let m = id_of(args, "market")?;
        let g = id_of(args, "good")?;
        need(self.markets.get(&m).is_some_and(|x| x.contains_key(&g)))?;
        Ok(Value::Object(self.view(&m, &g)))
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["market"], &[])?;
        let m = id_of(args, "market")?;
        let stalls = self.markets.get(&m).ok_or_else(c::bad)?;
        let goods = stalls.keys().map(|g| Value::Object(self.view(&m, g))).collect();
        Ok(obj! { "goods" => Value::Array(goods), "market" => m })
    }
}

fn experience(being: &str, kind: &str, good: &str, weight: u64) -> Value {
    obj! {
        "engine" => "aspiration", "op" => "experience",
        "args" => obj! { "being" => being, "kind" => kind, "subject" => good, "weight" => weight },
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Economy::new().workflow(req)
}
