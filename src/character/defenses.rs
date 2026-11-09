//! Defense mechanisms and strain (pass 4, stage 3).
//!
//! A being with a defense profile has a maturity (0–100), a tolerance (0–100), protected secrets,
//! a preferred style (mechanisms tried first), a strain threshold (millionths), a crisis outcome
//! and a crisis cooldown. The host reports a threat to the self (`threat`: a self-value of
//! competence, goodness or belonging lowered by `drop` points, or a protected secret exposed),
//! computed from its own appraisals, never from model text. A drop larger than the tolerance, or
//! any exposed protected secret, is ego-threatening.
//!
//! The being then picks one mechanism by a fixed first-match rule. Its effective maturity
//! (maturity minus 60 points at full strain) gives a band (high ≥ 70, mid ≥ 40, low below); its
//! style entries in that band are tried first, then the band's defaults (low: denial, projection,
//! displacement; mid: rationalization, intellectualization, withdrawal; high: humour,
//! sublimation). A mechanism that needs an input (projection a `blame`, displacement a
//! `safer_target`, rationalization an `excuse`, sublimation a `project`) is skipped without it.
//! The mechanism keeps part of the threat as strain, tags the distortion it makes (the host's
//! truth is unchanged), names the speech or behaviour effect and explains itself.
//!
//! Immature distortions (denial, projection, displacement) are cheap now and costly later: a later
//! `contradict` of the same fact adds half again as much strain, and strain decays at half speed
//! while one is held. Strain at or over the threshold breaks the being once (its crisis outcome)
//! and drops to half the threshold; within the cooldown no second crisis happens. A fact distorted
//! three or more times is a defense loop. Strain decays lazily by the clock (4,000 millionths per
//! tick, 2,000 while an immature distortion is held); `relieve` (rest, support, coping) lowers it
//! and `accept` drops a distortion.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::defenses::Defenses;
//!
//! let mut d = Defenses::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "profile", "args": {"being": "quartermaster", "maturity": 30, "tolerance": 20,
//!     "crisis": "confession", "style": ["displacement"]}},
//!   {"op": "threat", "args": {"being": "quartermaster", "value": "competence", "drop": 50,
//!     "fact": "open_lockbox", "safer_target": "apprentice"}}
//! ]}"#, 4096)?;
//! let out = d.workflow(&req)?.to_json();
//! // An immature defense: she snaps at the apprentice instead of facing the open lockbox.
//! assert!(out.contains(r#""mechanism":"displacement""#));
//! assert!(out.contains(r#""explain":"quartermaster snapped at apprentice because open_lockbox threatened competence""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "distortions", default: 16, ceiling: 256 },
];
pub const FULL: u64 = 1_000_000;
pub const VALUES: [&str; 4] = ["competence", "goodness", "belonging", "secret"];
pub const CRISES: [&str; 4] = ["confession", "flight", "collapse", "outburst"];
pub const LOW: [&str; 3] = ["denial", "projection", "displacement"];
pub const MID: [&str; 3] = ["rationalization", "intellectualization", "withdrawal"];
pub const HIGH: [&str; 2] = ["humour", "sublimation"];
pub const MECHANISMS: [&str; 8] = [
    "denial",
    "projection",
    "displacement",
    "rationalization",
    "intellectualization",
    "withdrawal",
    "humour",
    "sublimation",
];
/// Hits on one fact that make a defense loop.
pub const LOOP: u64 = 3;

fn keep(m: &str) -> u64 {
    match m {
        "denial" => 30,
        "projection" | "displacement" | "humour" => 40,
        "rationalization" | "intellectualization" => 60,
        "withdrawal" => 70,
        _ => 50,
    }
}

fn effect(m: &str) -> &'static str {
    match m {
        "denial" => "refuses_topic",
        "projection" => "accuses",
        "displacement" => "snaps",
        "rationalization" => "explains_too_much",
        "withdrawal" => "goes_quiet",
        "intellectualization" => "talks_procedure",
        "humour" => "jokes",
        _ => "works_harder",
    }
}

fn tag(m: &str) -> Option<&'static str> {
    match m {
        "denial" => Some("denied"),
        "projection" => Some("projected"),
        "displacement" => Some("displaced"),
        "rationalization" => Some("self_serving"),
        _ => None,
    }
}

fn needs(m: &str) -> Option<&'static str> {
    match m {
        "projection" => Some("blame"),
        "displacement" => Some("safer_target"),
        "rationalization" => Some("excuse"),
        "sublimation" => Some("project"),
        _ => None,
    }
}

fn relief(kind: &str) -> Option<u64> {
    match kind {
        "rest" => Some(2_000),
        "support" => Some(4_000),
        "coping" => Some(3_000),
        _ => None,
    }
}

/// The maturity band of an effective maturity.
pub fn band(eff: u64) -> &'static str {
    if eff >= 70 {
        "high"
    } else if eff >= 40 {
        "mid"
    } else {
        "low"
    }
}

fn band_list(b: &str) -> &'static [&'static str] {
    match b {
        "low" => &LOW,
        "mid" => &MID,
        _ => &HIGH,
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

fn one_of(v: &Value, options: &[&'static str]) -> Result<&'static str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    options.iter().copied().find(|x| *x == s).ok_or_else(c::bad)
}

fn id_list(v: Option<&Value>, max: usize, options: Option<&[&'static str]>) -> Result<Vec<String>> {
    let Some(v) = v else { return Ok(Vec::new()) };
    let rows = v.as_array().ok_or_else(c::bad)?;
    need(rows.len() <= max)?;
    let mut out: Vec<String> = Vec::new();
    for x in rows {
        let s = ident(x)?;
        if let Some(o) = options {
            need(o.contains(&s))?;
        }
        out.push(s.to_string());
    }
    let mut seen = out.clone();
    seen.sort();
    seen.dedup();
    need(seen.len() == out.len())?;
    Ok(out)
}

/// A held distortion: the mechanism that made it and how often the fact was defended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Distortion {
    pub mechanism: &'static str,
    pub hits: u64,
}

/// One being's defense profile and state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub maturity: u64,
    pub tolerance: u64,
    pub crisis: &'static str,
    pub threshold: u64,
    pub cooldown: u64,
    pub style: Vec<&'static str>,
    pub protects: Vec<String>,
    pub strain: u64,
    pub at: u64,
    pub distortions: BTreeMap<String, Distortion>,
    pub crisis_at: Option<u64>,
}

impl Profile {
    fn immature(&self) -> bool {
        self.distortions.values().any(|d| LOW.contains(&d.mechanism))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Defenses {
    pub beings: BTreeMap<String, Profile>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Defenses {
    fn default() -> Self {
        Defenses { beings: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

impl Defenses {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut d = Self::new();
        d.limits.apply(v)?;
        Ok(d)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "tick" => self.op_tick(args),
            "profile" => self.op_profile(args, l),
            "status" => self.op_status(args),
            "threat" => self.op_threat(args, l),
            "contradict" => self.op_contradict(args),
            "relieve" => self.op_relieve(args),
            "accept" => self.op_accept(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    /// A being's strain now, after lazy decay.
    pub fn strain_now(&self, p: &Profile) -> u64 {
        let rate: u64 = if p.immature() { 2_000 } else { 4_000 };
        p.strain.saturating_sub(rate.saturating_mul(self.clock - p.at))
    }

    fn effective(p: &Profile, strain: u64) -> u64 {
        p.maturity.saturating_sub(strain * 60 / FULL)
    }

    /// Strain after a possible break, and the crisis if one happened.
    fn crisis(&self, p: &Profile, strain: u64) -> (u64, Option<Value>) {
        if strain < p.threshold {
            return (strain, None);
        }
        if let Some(at) = p.crisis_at {
            if self.clock < at.saturating_add(p.cooldown) {
                return (strain, None);
            }
        }
        (p.threshold / 2, Some(obj! { "at" => self.clock, "outcome" => p.crisis }))
    }

    fn get(&self, b: &str) -> Result<&Profile> {
        self.beings.get(b).ok_or_else(c::bad)
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn op_profile(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "maturity", "tolerance", "crisis"], &["threshold", "cooldown", "style", "protects"])?;
        let b = id_of(args, "being")?;
        let maturity = uint_bound(c::get(args, "maturity")?, 0, 100)?;
        let tolerance = uint_bound(c::get(args, "tolerance")?, 0, 100)?;
        let crisis = one_of(c::get(args, "crisis")?, &CRISES)?;
        let threshold = opt_uint(args, "threshold", 1, FULL, 800_000)?;
        let cooldown = opt_uint(args, "cooldown", 1, MAX_AT, 72)?;
        let style: Vec<&'static str> = id_list(opt(args, "style"), MECHANISMS.len(), Some(&MECHANISMS))?
            .iter()
            .map(|s| MECHANISMS.iter().copied().find(|m| m == s).unwrap_or("denial"))
            .collect();
        let mut protects = id_list(opt(args, "protects"), 16, None)?;
        protects.sort();
        need(self.beings.contains_key(&b) || self.beings.len() < l.get("beings"))?;
        let p = match self.beings.get(&b) {
            None => Profile {
                maturity,
                tolerance,
                crisis,
                threshold,
                cooldown,
                style,
                protects,
                strain: 0,
                at: self.clock,
                distortions: BTreeMap::new(),
                crisis_at: None,
            },
            Some(old) => Profile {
                maturity,
                tolerance,
                crisis,
                threshold,
                cooldown,
                style,
                protects,
                strain: self.strain_now(old),
                at: self.clock,
                distortions: old.distortions.clone(),
                crisis_at: old.crisis_at,
            },
        };
        let out = self.view(&b, &p);
        self.beings.insert(b, p);
        Ok(out)
    }

    fn view(&self, b: &str, p: &Profile) -> Value {
        let strain = self.strain_now(p);
        let eff = Self::effective(p, strain);
        let ds: Vec<Value> = p
            .distortions
            .iter()
            .map(|(f, d)| obj! { "fact" => f.as_str(), "hits" => d.hits, "mechanism" => d.mechanism })
            .collect();
        let loops: Vec<&str> = p.distortions.iter().filter(|(_, d)| d.hits >= LOOP).map(|(f, _)| f.as_str()).collect();
        obj! {
            "band" => band(eff),
            "being" => b,
            "crisis_at" => p.crisis_at,
            "crisis_outcome" => p.crisis,
            "distortions" => ds,
            "effective_maturity" => eff,
            "loops" => loops,
            "maturity" => p.maturity,
            "strain" => strain,
            "threshold" => p.threshold,
        }
    }

    fn op_status(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        Ok(self.view(&b, self.get(&b)?))
    }

    fn op_threat(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "value", "drop", "fact"], &["blame", "safer_target", "excuse", "project"])?;
        let b = id_of(args, "being")?;
        let value = one_of(c::get(args, "value")?, &VALUES)?;
        let drop = uint_bound(c::get(args, "drop")?, 0, 100)?;
        let fact = id_of(args, "fact")?;
        let mut given: BTreeMap<&str, String> = BTreeMap::new();
        for k in ["blame", "safer_target", "project"] {
            if let Some(v) = opt(args, k) {
                let s = ident(v)?;
                need(s != b)?;
                given.insert(k, s.to_string());
            }
        }
        if let Some(v) = opt(args, "excuse") {
            given.insert("excuse", text_bound(v, 120)?.to_string());
        }
        let p = self.get(&b)?;
        let strain = self.strain_now(p);
        let threatened = if value == "secret" { p.protects.contains(&fact) } else { drop > p.tolerance };
        let eff = Self::effective(p, strain);
        let bd = band(eff);
        let base = obj! {
            "band" => bd,
            "being" => b.as_str(),
            "crisis" => Value::Null,
            "distortion" => Value::Null,
            "effect" => Value::Null,
            "effective_maturity" => eff,
            "explain" => Value::Null,
            "fact" => fact.as_str(),
            "hits" => 0u64,
            "kept" => 0u64,
            "loop" => false,
            "mechanism" => Value::Null,
            "strain" => strain,
            "threatened" => threatened,
            "value" => value,
        };
        if !threatened {
            return Ok(base);
        }
        let order = p.style.iter().copied().filter(|m| band_list(bd).contains(m)).chain(band_list(bd).iter().copied());
        let mech = order.into_iter().find(|m| needs(m).is_none_or(|k| given.contains_key(k))).ok_or_else(c::bad)?;
        let records = tag(mech).is_some();
        need(!records || p.distortions.contains_key(&fact) || p.distortions.len() < l.get("distortions"))?;
        let kept = drop * 10_000 * keep(mech) / 100;
        let raised = (strain + kept).min(FULL);
        let mut hits = 0;
        let mut dist = Value::Null;
        let mut ds = p.distortions.clone();
        if let Some(t) = tag(mech) {
            hits = ds.get(&fact).map_or(1, |d| d.hits + 1);
            let mut f = vec![
                ("fact".to_string(), Value::from(fact.as_str())),
                ("mechanism".to_string(), Value::from(mech)),
                ("tag".to_string(), Value::from(t)),
            ];
            match mech {
                "denial" => f.push(("confidence".into(), Value::from(30u64))),
                "projection" => f.push(("blame".into(), Value::from(given["blame"].as_str()))),
                "displacement" => f.push(("target".into(), Value::from(given["safer_target"].as_str()))),
                _ => f.push(("excuse".into(), Value::from(given["excuse"].as_str()))),
            }
            dist = Value::Object(f);
            ds.insert(fact.clone(), Distortion { mechanism: mech, hits });
        }
        let mut np = p.clone();
        np.distortions = ds;
        let (after, crisis) = self.crisis(&np, raised);
        if crisis.is_some() {
            np.crisis_at = Some(self.clock);
        }
        np.strain = after;
        np.at = self.clock;
        let g = |k: &str| given.get(k).cloned().unwrap_or_default();
        let verb = match mech {
            "denial" => "denied it".to_string(),
            "projection" => format!("blamed {}", g("blame")),
            "displacement" => format!("snapped at {}", g("safer_target")),
            "rationalization" => "made an excuse".to_string(),
            "withdrawal" => "withdrew".to_string(),
            "intellectualization" => "talked procedure".to_string(),
            "humour" => "joked".to_string(),
            _ => format!("threw it into {}", g("project")),
        };
        let Value::Object(mut f) = base else { return Err(c::bad()) };
        let set = |f: &mut Vec<(String, Value)>, k: &str, v: Value| {
            if let Some(slot) = f.iter_mut().find(|(x, _)| x == k) {
                slot.1 = v;
            } else {
                f.push((k.to_string(), v));
            }
        };
        set(&mut f, "crisis", crisis.unwrap_or(Value::Null));
        set(&mut f, "distortion", dist);
        set(&mut f, "effect", Value::from(effect(mech)));
        set(&mut f, "explain", Value::from(format!("{b} {verb} because {fact} threatened {value}")));
        set(&mut f, "hits", Value::from(hits));
        set(&mut f, "kept", Value::from(kept));
        set(&mut f, "loop", Value::Bool(hits >= LOOP));
        set(&mut f, "mechanism", Value::from(mech));
        set(&mut f, "strain", Value::from(after));
        if mech == "sublimation" {
            set(&mut f, "project", Value::from(g("project")));
            set(&mut f, "progress", Value::from(drop));
        }
        self.beings.insert(b, np);
        Ok(Value::Object(f))
    }

    fn op_contradict(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "fact", "drop"], &[])?;
        let b = id_of(args, "being")?;
        let fact = id_of(args, "fact")?;
        let drop = uint_bound(c::get(args, "drop")?, 0, 100)?;
        let p = self.get(&b)?;
        let strain = self.strain_now(p);
        let held = p.distortions.get(&fact).filter(|d| LOW.contains(&d.mechanism));
        let Some(d) = held else {
            return Ok(obj! {
                "being" => b, "crisis" => Value::Null, "distorted" => false, "fact" => fact, "hits" => 0u64,
                "kept" => 0u64, "loop" => false, "strain" => strain,
            });
        };
        let hits = d.hits + 1;
        let kept = drop * 15_000;
        let mut np = p.clone();
        if let Some(d) = np.distortions.get_mut(&fact) {
            d.hits = hits;
        }
        let (after, crisis) = self.crisis(&np, (strain + kept).min(FULL));
        if crisis.is_some() {
            np.crisis_at = Some(self.clock);
        }
        np.strain = after;
        np.at = self.clock;
        self.beings.insert(b.clone(), np);
        Ok(obj! {
            "being" => b, "crisis" => crisis, "distorted" => true, "fact" => fact, "hits" => hits, "kept" => kept,
            "loop" => hits >= LOOP, "strain" => after,
        })
    }

    fn op_relieve(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "kind", "amount"], &[])?;
        let b = id_of(args, "being")?;
        let kind = c::get(args, "kind")?.as_str().ok_or_else(c::bad)?;
        let rate = relief(kind).ok_or_else(c::bad)?;
        let amount = uint_bound(c::get(args, "amount")?, 0, 100)?;
        let clock = self.clock;
        let before = self.strain_now(self.get(&b)?);
        let after = before.saturating_sub(amount * rate);
        if let Some(p) = self.beings.get_mut(&b) {
            p.strain = after;
            p.at = clock;
        }
        Ok(obj! { "being" => b, "kind" => kind, "relieved" => before - after, "strain" => after })
    }

    fn op_accept(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "fact"], &[])?;
        let b = id_of(args, "being")?;
        let fact = id_of(args, "fact")?;
        let p = self.get(&b)?;
        need(p.distortions.contains_key(&fact))?;
        let strain = self.strain_now(p);
        let clock = self.clock;
        let p = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        p.strain = strain;
        p.at = clock;
        let d = p.distortions.remove(&fact).ok_or_else(c::bad)?;
        Ok(obj! { "being" => b, "fact" => fact, "mechanism" => d.mechanism, "strain" => strain })
    }
}

/// Runs one request on a fresh [`Defenses`] (local kind `defenses`).
pub fn workflow(request: &Value) -> Result<Value> {
    Defenses::new().workflow(request)
}
