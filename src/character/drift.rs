//! Slow personality change (pass 2, B4).
//!
//! An author gives a being traits (values and temperament dimensions such as trust or boldness)
//! as signed millionths, with a hard `cap` on how far experience may move each one from its
//! authored base. `experience` pushes a trait (−100..100) with a reason; pushes add up in a
//! pressure that fades by `fade` per tick toward 0. Each time the pressure reaches `threshold`
//! either way, the trait shifts one `step` in that direction (never past base ± cap; at the cap
//! the pressure is spent without change), the threshold comes off the pressure, and each shift
//! is recorded with the reasons since the last push that shifted it (the newest 8). Authors can `lock` a trait or a
//! whole being, so pushes are ignored, and `revert` a trait to its base. Capacities are
//! [host-set limits](crate::character::limits). Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::drift::Drift;
//!
//! let mut minds = Drift::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "trait", "args": {"being": "tomas", "trait": "trust", "base": 600000}},
//!   {"op": "experience", "args": {"being": "tomas", "trait": "trust", "push": -100, "reason": "betrayed"}},
//!   {"op": "experience", "args": {"being": "tomas", "trait": "trust", "push": -100, "reason": "robbed"}},
//!   {"op": "experience", "args": {"being": "tomas", "trait": "trust", "push": -100, "reason": "lied_to"}}
//! ]}"#, 4096)?;
//! let out = minds.workflow(&req)?.to_json();
//! // Three blows reach the threshold of 300: trust moves one step, from 600,000 to 590,000.
//! assert!(out.contains(r#""shifts":[{"at":0,"delta":-10000,"reasons":["betrayed","robbed","lied_to"],"value":590000}]"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, VecDeque};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "traits", default: 16, ceiling: 64 },
    Limit { name: "history", default: 16, ceiling: 256 },
];
pub const M: i64 = 1_000_000;
/// Reasons kept for the next shift.
pub const REASONS_KEPT: usize = 8;

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

/// One recorded shift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shift {
    pub at: u64,
    pub delta: i64,
    pub reasons: Vec<String>,
    pub value: i64,
}

impl Shift {
    fn view(&self) -> Value {
        obj! {
            "at" => self.at, "delta" => self.delta,
            "reasons" => Value::Array(self.reasons.iter().map(|r| Value::from(r.as_str())).collect()),
            "value" => self.value,
        }
    }
}

/// An authored trait and the experience pressing on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trait {
    pub base: i64,
    pub value: i64,
    pub cap: i64,
    pub step: i64,
    pub threshold: i64,
    pub fade: i64,
    pub pressure: i64,
    pub at: u64,
    pub reasons: VecDeque<String>,
    pub history: VecDeque<Shift>,
    pub locked: bool,
}

impl Trait {
    /// Pressure after fading toward 0 from `at` to `now`.
    fn faded(&self, now: u64) -> i64 {
        let gone = ((now - self.at) as i128 * self.fade as i128).min(i64::MAX as i128) as i64;
        if self.pressure > 0 {
            (self.pressure - gone).max(0)
        } else {
            (self.pressure + gone).min(0)
        }
    }
}

/// A being's traits and its whole-being lock.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mind {
    pub traits: BTreeMap<String, Trait>,
    pub locked: bool,
}

/// Beings, their traits and the clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drift {
    pub beings: BTreeMap<String, Mind>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Drift {
    fn default() -> Self {
        Drift { beings: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

impl Drift {
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
            "trait" => self.op_trait(args, l),
            "experience" => self.op_experience(args, l),
            "tick" => self.op_tick(args),
            "lock" => self.op_lock(args),
            "revert" => self.op_revert(args),
            "assess" => self.op_assess(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn trait_view(&self, b: &str, t: &str, x: &Trait, pressure: i64) -> Vec<(String, Value)> {
        vec![
            ("base".into(), Value::from(x.base)),
            ("being".into(), Value::from(b)),
            ("cap".into(), Value::from(x.cap)),
            ("drift".into(), Value::from(x.value - x.base)),
            ("fade".into(), Value::from(x.fade)),
            ("history".into(), Value::Array(x.history.iter().map(Shift::view).collect())),
            ("locked".into(), Value::from(x.locked || self.beings[b].locked)),
            ("pressure".into(), Value::from(pressure)),
            ("step".into(), Value::from(x.step)),
            ("threshold".into(), Value::from(x.threshold)),
            ("trait".into(), Value::from(t)),
            ("value".into(), Value::from(x.value)),
        ]
    }

    fn op_trait(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "trait", "base"], &["cap", "step", "threshold", "fade"])?;
        let b = id_of(args, "being")?;
        let t = id_of(args, "trait")?;
        let base = sint(c::get(args, "base")?, -M, M)?;
        let cap = opt_uint(args, "cap", 0, 300_000, 150_000)? as i64;
        let step = opt_uint(args, "step", 1, 100_000, 10_000)? as i64;
        let threshold = opt_uint(args, "threshold", 1, 100_000, 300)? as i64;
        let fade = opt_uint(args, "fade", 0, 100_000, 1)? as i64;
        match self.beings.get(&b) {
            Some(x) => need(x.traits.contains_key(&t) || x.traits.len() < l.get("traits"))?,
            None => need(self.beings.len() < l.get("beings"))?,
        }
        let x = Trait {
            base,
            value: base,
            cap,
            step,
            threshold,
            fade,
            pressure: 0,
            at: self.clock,
            reasons: VecDeque::new(),
            history: VecDeque::new(),
            locked: false,
        };
        self.beings.entry(b.clone()).or_default().traits.insert(t.clone(), x);
        let x = &self.beings[&b].traits[&t];
        Ok(Value::Object(self.trait_view(&b, &t, x, x.pressure)))
    }

    fn op_experience(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "trait", "push", "reason"], &[])?;
        let b = id_of(args, "being")?;
        let t = id_of(args, "trait")?;
        let push = sint(c::get(args, "push")?, -100, 100)?;
        let reason = id_of(args, "reason")?;
        let clock = self.clock;
        let keep = l.get("history");
        let mind = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        let whole = mind.locked;
        let x = mind.traits.get_mut(&t).ok_or_else(c::bad)?;
        let mut shifts = Vec::new();
        if !(x.locked || whole) {
            x.pressure = x.faded(clock);
            x.at = clock;
            x.pressure += push;
            x.reasons.push_back(reason);
            while x.reasons.len() > REASONS_KEPT {
                x.reasons.pop_front();
            }
            let mut spent = false;
            while x.pressure.abs() >= x.threshold {
                let sign = x.pressure.signum();
                x.pressure -= sign * x.threshold;
                let (lo, hi) = ((x.base - x.cap).max(-M), (x.base + x.cap).min(M));
                let new = (x.value + sign * x.step).clamp(lo, hi);
                let delta = new - x.value;
                if delta != 0 {
                    x.value = new;
                    let s = Shift { at: clock, delta, reasons: x.reasons.iter().cloned().collect(), value: new };
                    shifts.push(s.view());
                    x.history.push_back(s);
                    while x.history.len() > keep {
                        x.history.pop_front();
                    }
                }
                spent = true;
            }
            if spent {
                x.reasons.clear();
            }
        }
        let x = &self.beings[&b].traits[&t];
        let mut v = self.trait_view(&b, &t, x, x.pressure);
        v.push(("shifts".into(), Value::Array(shifts)));
        Ok(Value::Object(v))
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn op_lock(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "locked"], &["trait"])?;
        let b = id_of(args, "being")?;
        let locked = c::get(args, "locked")?.as_bool().ok_or_else(c::bad)?;
        let t = opt(args, "trait").map(|v| ident(v).map(str::to_string)).transpose()?;
        let mind = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        match &t {
            Some(t) => mind.traits.get_mut(t).ok_or_else(c::bad)?.locked = locked,
            None => mind.locked = locked,
        }
        Ok(obj! { "being" => b, "locked" => locked, "trait" => Value::from(t) })
    }

    fn op_revert(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "trait"], &[])?;
        let b = id_of(args, "being")?;
        let t = id_of(args, "trait")?;
        let clock = self.clock;
        let x = self.beings.get_mut(&b).and_then(|m| m.traits.get_mut(&t)).ok_or_else(c::bad)?;
        x.value = x.base;
        x.pressure = 0;
        x.reasons.clear();
        x.at = clock;
        let x = &self.beings[&b].traits[&t];
        Ok(Value::Object(self.trait_view(&b, &t, x, 0)))
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["trait"])?;
        let b = id_of(args, "being")?;
        let t = opt(args, "trait").map(|v| ident(v).map(str::to_string)).transpose()?;
        let mind = self.beings.get(&b).ok_or_else(c::bad)?;
        let names: Vec<&String> = match &t {
            Some(t) => vec![mind.traits.get_key_value(t).ok_or_else(c::bad)?.0],
            None => mind.traits.keys().collect(),
        };
        let views = names
            .into_iter()
            .map(|n| {
                let x = &mind.traits[n];
                Value::Object(self.trait_view(&b, n, x, x.faded(self.clock)))
            })
            .collect();
        Ok(obj! { "being" => b.as_str(), "locked" => mind.locked, "traits" => Value::Array(views) })
    }
}

persist!(Shift { at [0, MAX_AT], delta [-100_000, 100_000], reasons, value [-M, M] });
persist!(Trait {
    at [0, MAX_AT],
    base [-M, M],
    cap [0, 300_000],
    fade [0, 100_000],
    history,
    locked,
    pressure [-100_000, 100_000],
    reasons,
    step [1, 100_000],
    threshold [1, 100_000],
    value [-M, M]
});
persist!(Mind { locked, traits });
persist!(rest Drift { beings, clock [0, MAX_AT] });
durable!(Drift, "drift", LIMITS);

/// A trait as `trait`, `experience` and `revert` leave it.
fn trait_ok(x: &Trait, clock: u64) -> bool {
    let (lo, hi) = ((x.base - x.cap).max(-M), (x.base + x.cap).min(M));
    let reasons_ok = |rs: &[String]| rs.len() <= REASONS_KEPT && rs.iter().all(|r| is_ident(r));
    (lo..=hi).contains(&x.value)
        && x.pressure.abs() < x.threshold
        && x.at <= clock
        && x.reasons.len() <= REASONS_KEPT
        && x.reasons.iter().all(|r| is_ident(r))
        && x.history.iter().all(|s| {
            s.delta != 0 && s.delta.abs() <= x.step && s.at <= x.at && !s.reasons.is_empty() && reasons_ok(&s.reasons)
        })
        && x.history.iter().zip(x.history.iter().skip(1)).all(|(a, b)| a.at <= b.at)
}

impl Drift {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("beings", self.beings.len())?;
        for (b, mind) in &self.beings {
            ok(is_ident(b) && !mind.traits.is_empty())?;
            within::<Self>("traits", mind.traits.len())?;
            for (t, x) in &mind.traits {
                within::<Self>("history", x.history.len())?;
                ok(is_ident(t) && trait_ok(x, self.clock))?;
            }
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Drift::new().workflow(req)
}
