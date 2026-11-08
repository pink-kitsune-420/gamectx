//! Stability monitors (pass 2, D1): detect systemic failure modes in soak simulations.
//!
//! Pass 1 added many feedback loops (needs, fatigue, affect, rumors, grudges, tasks). This
//! module watches whole-world turns and reports five failure modes:
//!
//! - **Dithering:** a being keeps switching between the same two motives with a small score
//!   margin. The warning names the motive to hold (commitment inertia).
//! - **Starvation spiral:** a need or fatigue stays at or above the critical level, never
//!   improving, while the being makes no recovery attempt.
//! - **Deadlock:** beings that each want a resource another one holds, in a cycle.
//! - **Herding:** a large share of beings pick the same activity in the same turn.
//! - **Grudge or rumor runaway:** world totals that keep rising turn after turn.
//!
//! The host reports each turn with `audit_turn`; turns must strictly increase. Monitors only
//! report; the host decides what to do. Pure host computation ($0 model spend), bounded:
//! $\le 64$ beings per audit, $\le 256$ tracked beings, $\le 256$ logged warnings.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::stability::StabilityMonitor;
//!
//! let mut mon = StabilityMonitor::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {"op": "init", "args": {"herd_min": 2, "herd_pct": 100}},
//!     {"op": "audit_turn", "args": {"turn": 1, "beings": [
//!       {"being": "baker", "motive": "routine_market", "activity": "market"},
//!       {"being": "smith", "motive": "routine_market", "activity": "market"}
//!     ]}}
//!   ]
//! }"#, 4096)?;
//! let out = mon.workflow(&req)?;
//! let audit = &out.get("results").unwrap().as_array().unwrap()[1];
//! assert_eq!(audit.get("status").and_then(|v| v.as_str()), Some("warn"));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const MAX_OPS: usize = 64;
pub const MAX_AUDIT_BEINGS: usize = 64;
pub const MAX_TRACKED_BEINGS: usize = 256;
pub const MAX_WARNINGS: usize = 256;
pub const MAX_NEEDS: usize = 16;
pub const MAX_RESOURCES: usize = 8;
pub const MAX_TURN: u64 = 1_000_000_000_000;
pub const MAX_TOTAL: u64 = 1_000_000_000;

pub const KINDS: [&str; 6] =
    ["deadlock", "dithering", "grudge_runaway", "herding", "rumor_runaway", "starvation_spiral"];

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

fn opt<'a>(args: &'a Value, k: &str) -> Option<&'a Value> {
    args.get(k).filter(|x| **x != Value::Null)
}

fn opt_uint(args: &Value, k: &str, lo: u64, hi: u64, default: u64) -> Result<u64> {
    Ok(opt(args, k).map(|v| uint_bound(v, lo, hi)).transpose()?.unwrap_or(default))
}

fn list_bound(v: Option<&Value>, max: usize) -> Result<&[Value]> {
    match v {
        None => Ok(&[]),
        Some(v) => {
            let xs = v.as_array().ok_or_else(c::bad)?;
            if xs.len() > max {
                return Err(c::bad());
            }
            Ok(xs)
        }
    }
}

fn ident_set(v: Option<&Value>) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for x in list_bound(v, MAX_RESOURCES)? {
        out.insert(ident(x)?.to_string());
    }
    Ok(out)
}

/// Monitor thresholds. All integers; defaults are documented in `docs/characters/LIFE.md`.
/// Host-set capacity limits: `(name, default, ceiling)`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "tracked_beings", default: 256, ceiling: 1000000 },
    crate::character::limits::Limit { name: "audit_beings", default: 64, ceiling: 4096 },
    crate::character::limits::Limit { name: "warnings", default: 256, ceiling: 65536 },
];
crate::character::limits::module_limits!(StabilityLimits, LIMITS);

#[derive(Clone, Debug, PartialEq)]
pub struct StabilityConfig {
    pub window: u64,
    pub dither_switches: u64,
    pub dither_margin: u64,
    pub critical: u64,
    pub spiral_turns: u64,
    pub herd_min: u64,
    pub herd_pct: u64,
    pub runaway_turns: u64,
    pub runaway_floor: u64,
}

impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            window: 8,
            dither_switches: 3,
            dither_margin: 10,
            critical: 80,
            spiral_turns: 3,
            herd_min: 3,
            herd_pct: 80,
            runaway_turns: 3,
            runaway_floor: 100,
        }
    }
}

impl StabilityConfig {
    pub fn to_value(&self) -> Value {
        obj! {
            "critical" => self.critical,
            "dither_margin" => self.dither_margin,
            "dither_switches" => self.dither_switches,
            "herd_min" => self.herd_min,
            "herd_pct" => self.herd_pct,
            "runaway_floor" => self.runaway_floor,
            "runaway_turns" => self.runaway_turns,
            "spiral_turns" => self.spiral_turns,
            "window" => self.window,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeingWatch {
    /// Recent `(motive, margin)` pairs, oldest first, at most `window` long.
    pub history: VecDeque<(String, u64)>,
    pub spiral_streak: u64,
    pub last_severity: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TotalWatch {
    pub last: Option<u64>,
    pub streak: u64,
}

struct Report {
    being: String,
    motive: String,
    margin: u64,
    activity: String,
    severity: u64,
    worst: String,
    recovering: bool,
    holds: BTreeSet<String>,
    wants: BTreeSet<String>,
}

/// World-level stability monitor over host-reported turns.
#[derive(Clone, Debug, Default)]
pub struct StabilityMonitor {
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: StabilityLimits,
    pub config: StabilityConfig,
    pub beings: BTreeMap<String, BeingWatch>,
    pub grudges: TotalWatch,
    pub rumors: TotalWatch,
    pub last_turn: Option<u64>,
    pub turns_audited: u64,
    pub last_turn_kinds: BTreeSet<String>,
    pub totals: BTreeMap<String, u64>,
    pub warnings: VecDeque<Value>,
    pub next_warning: u64,
}

fn names(xs: &BTreeSet<String>) -> String {
    xs.iter().map(String::as_str).collect::<Vec<_>>().join(", ")
}

impl StabilityMonitor {
    pub fn new() -> Self {
        Self { next_warning: 1, ..Self::default() }
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &[])?;
        let ops = request.get("ops").and_then(Value::as_array).ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }
        let empty = Value::Object(Vec::new());
        let mut results = Vec::new();
        for op_val in ops {
            keys(op_val, &["op"], &["args"])?;
            let op = text_bound(c::get(op_val, "op")?, 32)?;
            let args = opt(op_val, "args").unwrap_or(&empty);
            let res = match op {
                "init" => self.op_init(args)?,
                "audit_turn" => self.op_audit(args)?,
                "assess_health" => self.op_health(args)?,
                "get_warnings" => self.op_warnings(args)?,
                _ => return Err(c::bad()),
            };
            results.push(res);
        }
        Ok(obj! {
            "version" => 1u64,
            "results" => Value::Array(results),
        })
    }

    fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(
            args,
            &[],
            &[
                "window",
                "dither_switches",
                "dither_margin",
                "critical",
                "spiral_turns",
                "herd_min",
                "herd_pct",
                "runaway_turns",
                "runaway_floor",
            ],
        )?;
        let d = StabilityConfig::default();
        let config = StabilityConfig {
            window: opt_uint(args, "window", 2, 32, d.window)?,
            dither_switches: opt_uint(args, "dither_switches", 1, 31, d.dither_switches)?,
            dither_margin: opt_uint(args, "dither_margin", 0, 100, d.dither_margin)?,
            critical: opt_uint(args, "critical", 1, 100, d.critical)?,
            spiral_turns: opt_uint(args, "spiral_turns", 1, 64, d.spiral_turns)?,
            herd_min: opt_uint(args, "herd_min", 2, 64, d.herd_min)?,
            herd_pct: opt_uint(args, "herd_pct", 51, 100, d.herd_pct)?,
            runaway_turns: opt_uint(args, "runaway_turns", 1, 64, d.runaway_turns)?,
            runaway_floor: opt_uint(args, "runaway_floor", 0, MAX_TOTAL, d.runaway_floor)?,
        };
        if config.dither_switches >= config.window {
            return Err(c::bad());
        }
        *self = Self { config, limits: self.limits.clone(), ..Self::new() };
        Ok(obj! { "config" => self.config.to_value(), "ok" => true })
    }

    fn parse_report(&self, b: &Value) -> Result<Report> {
        keys(
            b,
            &["being", "motive", "activity"],
            &["score", "runner_up", "needs", "fatigue", "recovering", "holds", "wants"],
        )?;
        let being = ident(c::get(b, "being")?)?.to_string();
        let motive = ident(c::get(b, "motive")?)?.to_string();
        let activity = ident(c::get(b, "activity")?)?.to_string();
        let score = opt_uint(b, "score", 0, 100, 0)?;
        let margin = match opt(b, "runner_up") {
            Some(v) => score.saturating_sub(uint_bound(v, 0, 100)?),
            None => 100,
        };
        let fatigue = opt_uint(b, "fatigue", 0, 100, 0)?;
        let (mut severity, mut worst) = (fatigue, "fatigue".to_string());
        let mut seen = BTreeSet::new();
        for n in list_bound(opt(b, "needs"), MAX_NEEDS)? {
            keys(n, &["id", "level"], &[])?;
            let id = ident(c::get(n, "id")?)?;
            if id == "fatigue" || !seen.insert(id.to_string()) {
                return Err(c::bad());
            }
            let level = uint_bound(c::get(n, "level")?, 0, 100)?;
            if level > severity || (level == severity && id < worst.as_str()) {
                severity = level;
                worst = id.to_string();
            }
        }
        let recovering = match opt(b, "recovering") {
            Some(Value::Bool(x)) => *x,
            Some(_) => return Err(c::bad()),
            None => false,
        };
        Ok(Report {
            being,
            motive,
            margin,
            activity,
            severity,
            worst,
            recovering,
            holds: ident_set(opt(b, "holds"))?,
            wants: ident_set(opt(b, "wants"))?,
        })
    }

    fn op_audit(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["turn", "beings"], &["world"])?;
        let turn = uint_bound(c::get(args, "turn")?, 0, MAX_TURN)?;
        if self.last_turn.is_some_and(|t| turn <= t) {
            return Err(c::bad());
        }
        let raw = list_bound(Some(c::get(args, "beings")?), self.limits.get("audit_beings"))?;
        let mut reports = Vec::new();
        let mut ids = BTreeSet::new();
        for b in raw {
            let r = self.parse_report(b)?;
            if !ids.insert(r.being.clone()) {
                return Err(c::bad());
            }
            reports.push(r);
        }
        let (mut grudge_total, mut rumor_total) = (None, None);
        if let Some(w) = opt(args, "world") {
            keys(w, &[], &["grudge_total", "rumor_total"])?;
            grudge_total = opt(w, "grudge_total").map(|v| uint_bound(v, 0, MAX_TOTAL)).transpose()?;
            rumor_total = opt(w, "rumor_total").map(|v| uint_bound(v, 0, MAX_TOTAL)).transpose()?;
        }
        let new_beings = ids.iter().filter(|b| !self.beings.contains_key(*b)).count();
        if self.beings.len() + new_beings > self.limits.get("tracked_beings") {
            return Err(c::bad());
        }

        // All validation is done; mutate state from here on.
        self.last_turn = Some(turn);
        self.turns_audited += 1;
        reports.sort_by(|a, b| a.being.cmp(&b.being));
        let cfg = self.config.clone();
        let mut found: Vec<Value> = Vec::new();

        for r in &reports {
            let w = self.beings.entry(r.being.clone()).or_default();
            w.history.push_back((r.motive.clone(), r.margin));
            while w.history.len() as u64 > cfg.window {
                w.history.pop_front();
            }
            let switches = w
                .history
                .iter()
                .zip(w.history.iter().skip(1))
                .filter(|(a, b)| a.0 != b.0 && b.1 <= cfg.dither_margin)
                .count() as u64;
            let motives: BTreeSet<String> = w.history.iter().map(|h| h.0.clone()).collect();
            if switches >= cfg.dither_switches && motives.len() == 2 {
                found.push(obj! {
                    "beings" => Value::strings(&[&r.being]),
                    "detail" => format!(
                        "{} switched {} times between {} in {} turns; hold {} for commitment inertia",
                        r.being, switches, names(&motives).replace(", ", " and "), w.history.len(), r.motive
                    ),
                    "kind" => "dithering",
                    "motives" => Value::strings(&motives.iter().collect::<Vec<_>>()),
                    "switches" => switches,
                });
            }

            let worsening = w.last_severity.is_none_or(|s| r.severity >= s);
            if r.severity >= cfg.critical && !r.recovering && worsening {
                w.spiral_streak += 1;
            } else {
                w.spiral_streak = 0;
            }
            w.last_severity = Some(r.severity);
            if w.spiral_streak >= cfg.spiral_turns {
                found.push(obj! {
                    "beings" => Value::strings(&[&r.being]),
                    "detail" => format!(
                        "{} stayed critical on {} at {} for {} turns without recovering",
                        r.being, r.worst, r.severity, w.spiral_streak
                    ),
                    "kind" => "starvation_spiral",
                    "need" => r.worst.as_str(),
                    "severity" => r.severity,
                    "streak" => w.spiral_streak,
                });
            }
        }

        found.extend(deadlocks(&reports));

        if reports.len() as u64 >= cfg.herd_min {
            let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
            for r in &reports {
                *counts.entry(r.activity.as_str()).or_default() += 1;
            }
            let (mut top, mut best) = ("", 0u64);
            for (a, n) in &counts {
                if *n > best {
                    top = a;
                    best = *n;
                }
            }
            let total = reports.len() as u64;
            if best * 100 >= cfg.herd_pct * total {
                let who: Vec<&str> = reports.iter().filter(|r| r.activity == top).map(|r| r.being.as_str()).collect();
                found.push(obj! {
                    "activity" => top,
                    "beings" => Value::strings(&who),
                    "count" => best,
                    "detail" => format!("{best} of {total} beings chose {top} in the same turn"),
                    "kind" => "herding",
                    "total" => total,
                });
            }
        }

        for (label, kind, value, watch) in [
            ("grudge", "grudge_runaway", grudge_total, &mut self.grudges),
            ("rumor", "rumor_runaway", rumor_total, &mut self.rumors),
        ] {
            let Some(v) = value else { continue };
            if watch.last.is_some_and(|l| v > l) {
                watch.streak += 1;
            } else {
                watch.streak = 0;
            }
            watch.last = Some(v);
            if watch.streak >= cfg.runaway_turns && v >= cfg.runaway_floor {
                found.push(obj! {
                    "beings" => Value::Array(Vec::new()),
                    "detail" => format!("{label} total rose {} turns in a row to {v}", watch.streak),
                    "kind" => kind,
                    "streak" => watch.streak,
                    "total" => v,
                });
            }
        }

        self.last_turn_kinds.clear();
        let mut out = Vec::new();
        for f in found {
            let Value::Object(mut fields) = f else { continue };
            let kind = fields.iter().find(|(k, _)| k == "kind").and_then(|(_, v)| v.as_str()).unwrap_or("").to_string();
            let id = format!("w_{}", self.next_warning);
            self.next_warning += 1;
            fields.push(("id".to_string(), Value::str(id)));
            fields.push(("turn".to_string(), Value::from(turn)));
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            let w = Value::Object(fields);
            *self.totals.entry(kind.clone()).or_default() += 1;
            self.last_turn_kinds.insert(kind);
            if self.warnings.len() >= self.limits.get("warnings") {
                self.warnings.pop_front();
            }
            self.warnings.push_back(w.clone());
            out.push(w);
        }
        Ok(obj! {
            "count" => out.len(),
            "status" => if out.is_empty() { "ok" } else { "warn" },
            "turn" => turn,
            "warnings" => Value::Array(out),
        })
    }

    fn op_health(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &[])?;
        let mut totals = Vec::new();
        let mut sum = 0;
        for k in KINDS {
            let n = self.totals.get(k).copied().unwrap_or(0);
            sum += n;
            totals.push((k.to_string(), Value::from(n)));
        }
        let status = if !self.last_turn_kinds.is_empty() {
            "unstable"
        } else if sum > 0 {
            "recovered"
        } else {
            "healthy"
        };
        Ok(obj! {
            "active" => Value::strings(&self.last_turn_kinds.iter().collect::<Vec<_>>()),
            "beings_tracked" => self.beings.len(),
            "last_turn" => self.last_turn.map(Value::from).unwrap_or(Value::Null),
            "status" => status,
            "totals" => Value::Object(totals),
            "turns_audited" => self.turns_audited,
            "warnings_logged" => self.warnings.len(),
        })
    }

    fn op_warnings(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &["kind", "being", "since_turn", "limit"])?;
        let kind = opt(args, "kind").map(|v| text_bound(v, 32)).transpose()?;
        if kind.is_some_and(|k| !KINDS.contains(&k)) {
            return Err(c::bad());
        }
        let being = opt(args, "being").map(ident).transpose()?;
        let since = opt_uint(args, "since_turn", 0, MAX_TURN, 0)?;
        let limit = opt_uint(args, "limit", 1, self.limits.get("warnings") as u64, 32)? as usize;
        let matched: Vec<&Value> = self
            .warnings
            .iter()
            .filter(|w| kind.is_none_or(|k| w.get("kind").and_then(Value::as_str) == Some(k)))
            .filter(|w| {
                being.is_none_or(|b| {
                    w.get("beings").and_then(Value::as_array).is_some_and(|xs| xs.iter().any(|x| x.as_str() == Some(b)))
                })
            })
            .filter(|w| w.get("turn").and_then(Value::as_usize).is_some_and(|t| t as u64 >= since))
            .collect();
        let skip = matched.len().saturating_sub(limit);
        let out: Vec<Value> = matched.into_iter().skip(skip).cloned().collect();
        Ok(obj! {
            "count" => out.len(),
            "warnings" => Value::Array(out),
        })
    }
}

/// Wait-for cycles: `a -> b` when `a` wants a resource `b` holds. Each strongly connected
/// group of two or more beings is one deadlock, reported once with its contested resources.
fn deadlocks(reports: &[Report]) -> Vec<Value> {
    let n = reports.len();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, a) in reports.iter().enumerate() {
        for (j, b) in reports.iter().enumerate() {
            if i != j && a.wants.iter().any(|r| b.holds.contains(r)) {
                edges[i].push(j);
            }
        }
    }
    let reach: Vec<Vec<bool>> = (0..n)
        .map(|s| {
            let mut seen = vec![false; n];
            let mut stack = vec![s];
            while let Some(x) = stack.pop() {
                for &y in &edges[x] {
                    if !seen[y] {
                        seen[y] = true;
                        stack.push(y);
                    }
                }
            }
            seen
        })
        .collect();
    let mut done = vec![false; n];
    let mut out = Vec::new();
    for i in 0..n {
        if done[i] || !reach[i][i] {
            continue;
        }
        let group: Vec<usize> = (0..n).filter(|&j| reach[i][j] && reach[j][i]).collect();
        for &j in &group {
            done[j] = true;
        }
        let members: BTreeSet<String> = group.iter().map(|&j| reports[j].being.clone()).collect();
        let mut resources = BTreeSet::new();
        for &a in &group {
            for &b in &group {
                if a != b {
                    for r in reports[a].wants.intersection(&reports[b].holds) {
                        resources.insert(r.clone());
                    }
                }
            }
        }
        out.push(obj! {
            "beings" => Value::strings(&members.iter().collect::<Vec<_>>()),
            "detail" => format!("{} wait on each other for {}", names(&members), names(&resources)),
            "kind" => "deadlock",
            "resources" => Value::strings(&resources.iter().collect::<Vec<_>>()),
        });
    }
    out
}

pub fn workflow(req: &Value) -> Result<Value> {
    StabilityMonitor::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(m: &mut StabilityMonitor, s: &str) -> Result<Value> {
        m.workflow(&parse(s, 1 << 20).unwrap())
    }

    fn audit(turn: u64, beings: &str) -> String {
        format!(r#"{{"ops":[{{"op":"audit_turn","args":{{"turn":{turn},"beings":[{beings}]}}}}]}}"#)
    }

    #[test]
    fn dithering_between_two_close_motives() {
        let mut m = StabilityMonitor::new();
        let mut last = String::new();
        for t in 1..=4 {
            let motive = if t % 2 == 0 { "eat" } else { "bake" };
            last = run(
                &mut m,
                &audit(
                    t,
                    &format!(r#"{{"being":"baker","motive":"{motive}","activity":"x","score":60,"runner_up":55}}"#),
                ),
            )
            .unwrap()
            .to_json();
        }
        assert!(last.contains(r#""kind":"dithering""#), "{last}");
        assert!(last.contains("hold eat for commitment inertia"));
    }

    #[test]
    fn deadlock_cycle_reported_once() {
        let mut m = StabilityMonitor::new();
        let out = run(
            &mut m,
            &audit(
                1,
                r#"{"being":"a","motive":"m","activity":"x","holds":["oven"],"wants":["flour"]},
                   {"being":"b","motive":"m","activity":"y","holds":["flour"],"wants":["oven"]},
                   {"being":"c","motive":"m","activity":"z","wants":["oven"]}"#,
            ),
        )
        .unwrap()
        .to_json();
        assert_eq!(out.matches(r#""kind":"deadlock""#).count(), 1);
        assert!(out.contains("a, b wait on each other for flour, oven"));
    }

    #[test]
    fn turns_must_increase_and_beings_unique() {
        let mut m = StabilityMonitor::new();
        run(&mut m, &audit(5, "")).unwrap();
        assert!(run(&mut m, &audit(5, "")).is_err());
        let dup = r#"{"being":"a","motive":"m","activity":"x"},{"being":"a","motive":"m","activity":"x"}"#;
        assert!(run(&mut m, &audit(6, dup)).is_err());
        assert_eq!(m.last_turn, Some(5));
    }
}
