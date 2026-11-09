//! Creeds and conscience (pass 2, stage 4 and B5): religions, ideologies and personal norms.
//!
//! A **creed** is any shared belief system: a `religion`, `ideology`, `philosophy` or
//! `tradition`. It has **tenets** (forbidden actions or food tags, optionally only against its
//! `sacred` places and things), timed **observances** (do `action` within `window` ticks of
//! every `every` ticks), and stances toward **rival** creeds.
//!
//! Beings adhere to at most one creed with a **devotion** 0–100 and may hold up to 16
//! **personal norms** of their own (B5). Conscience follows from both:
//! - `judge` an act: the actor feels **guilt** (severity × devotion / 100, the largest broken
//!   norm; personal norms count at full weight), **shame** (half the guilt) when any witness
//!   disapproves, each disapproving witness feels **indignation** (its own largest broken norm,
//!   by its own devotion), and the actor loses **standing** in the witnesses' creeds. Effects come as [affect](crate::character::affect) `set_mood` and
//!   `appraise` (`resentment`) proposals in their exact argument shapes.
//! - `due` lists observances whose window is open as [motive arbiter](crate::character::motive)
//!   candidates and charges guilt and standing for the most recent window missed since joining.
//! - `food_taboos` gives the tags to pass to [diet](crate::character::diet) `forbids`
//!   (creed food tenets apply from devotion 30; personal ones always).
//! - `regard` says how one being's creed sees another's: `kin`, a rival stance, or `neutral`.
//!
//! Example creeds in tests and docs are original. Bounded: 32 creeds, 128 beings. Pure host
//! computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::creed::CreedManager;
//!
//! let mut creeds = CreedManager::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "register_creed", "args": {"creed": {"id": "hearthkeepers", "label": "Keepers of the Hearth", "kind": "religion",
//!     "tenets": [{"id": "no_theft", "action": "steal", "severity": 80, "label": "Take nothing from another's hearth"}]}}},
//!   {"op": "adhere", "args": {"being": "baker", "creed": "hearthkeepers", "devotion": 90}},
//!   {"op": "adhere", "args": {"being": "miller", "creed": "hearthkeepers", "devotion": 50}},
//!   {"op": "judge", "args": {"actor": "baker", "action": "steal", "witnesses": ["miller"]}}
//! ]}"#, 4096)?;
//! let out = creeds.workflow(&req)?;
//! let verdict = &out.get("results").unwrap().as_array().unwrap()[3];
//! assert_eq!(verdict.get("guilt").and_then(|v| v.as_usize()), Some(72));
//! assert_eq!(verdict.get("shame").and_then(|v| v.as_usize()), Some(36));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_bool, opt_uint, run_ops, text_bound, uint_bound, MAX_AT};
use crate::character::diet::FOOD_TAGS;
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, is_text, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const CREED_KINDS: [&str; 4] = ["ideology", "philosophy", "religion", "tradition"];
pub const STANCES: [&str; 3] = ["hostile", "tolerant", "wary"];
pub const MAX_CREEDS: usize = 32;
pub const MAX_BEINGS: usize = 128;
pub const MAX_TENETS: usize = 16;
pub const MAX_OBSERVANCES: usize = 8;
pub const MAX_WITNESSES: usize = 16;
pub const MAX_PERIOD: u64 = 1_000_000;
/// Devotion from which a creed's food tenets become personal taboos.
pub const LAX: u64 = 30;
pub const DEFAULT_STANDING: u64 = 50;

/// Host-set capacity limits: `(name, default, ceiling)`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "creeds", default: 32, ceiling: 4096 },
    crate::character::limits::Limit { name: "beings", default: 128, ceiling: 1000000 },
];
crate::character::limits::module_limits!(CreedLimits, LIMITS);

#[derive(Clone, Debug, PartialEq)]
pub struct Tenet {
    pub id: String,
    pub action: Option<String>,
    pub food_tag: Option<String>,
    pub sacred_only: bool,
    pub severity: u64,
    pub label: String,
}

impl Tenet {
    fn read(v: &Value, allow_sacred: bool) -> Result<Self> {
        keys(v, &["id", "severity", "label"], &["action", "food_tag", "sacred_only"])?;
        let action = opt(v, "action").map(ident).transpose()?.map(str::to_string);
        let food_tag = opt(v, "food_tag").map(ident).transpose()?.map(str::to_string);
        if action.is_some() == food_tag.is_some() || food_tag.as_deref().is_some_and(|t| !FOOD_TAGS.contains(&t)) {
            return Err(c::bad());
        }
        let sacred_only = opt_bool(v, "sacred_only", false)?;
        if sacred_only && (!allow_sacred || action.is_none()) {
            return Err(c::bad());
        }
        Ok(Tenet {
            id: ident(c::get(v, "id")?)?.to_string(),
            action,
            food_tag,
            sacred_only,
            severity: uint_bound(c::get(v, "severity")?, 1, 100)?,
            label: text_bound(c::get(v, "label")?, 80)?.to_string(),
        })
    }

    fn to_value(&self) -> Value {
        let o = |x: &Option<String>| x.as_deref().map(Value::str).unwrap_or(Value::Null);
        obj! {
            "action" => o(&self.action),
            "food_tag" => o(&self.food_tag),
            "id" => self.id.as_str(),
            "label" => self.label.as_str(),
            "sacred_only" => self.sacred_only,
            "severity" => self.severity,
        }
    }

    /// Whether doing `action` (to `target`, eating `tags`) breaks this norm, given `sacred`.
    fn broken_by(
        &self,
        action: &str,
        target: Option<&str>,
        tags: &BTreeSet<String>,
        sacred: &BTreeSet<String>,
    ) -> bool {
        match (&self.action, &self.food_tag) {
            (Some(a), _) => a == action && (!self.sacred_only || target.is_some_and(|t| sacred.contains(t))),
            (None, Some(t)) => action == "eat" && tags.contains(t),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Observance {
    pub id: String,
    pub action: String,
    pub every: u64,
    pub window: u64,
    pub offset: u64,
    pub severity: u64,
    pub label: String,
}

impl Observance {
    /// `(period, open)` at `at`, or `None` before the first period starts.
    fn period(&self, at: u64) -> Option<(u64, bool)> {
        if at < self.offset {
            return None;
        }
        let rel = at - self.offset;
        Some((rel / self.every, rel % self.every < self.window))
    }

    fn start(&self, period: u64) -> u64 {
        self.offset + period * self.every
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Creed {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub tenets: Vec<Tenet>,
    pub observances: Vec<Observance>,
    pub sacred: BTreeSet<String>,
    pub rivals: BTreeMap<String, String>,
}

fn read_creed(v: &Value) -> Result<Creed> {
    keys(v, &["id", "label", "kind"], &["tenets", "observances", "sacred", "rivals"])?;
    let id = ident(c::get(v, "id")?)?.to_string();
    let kind = text_bound(c::get(v, "kind")?, 16)?;
    if !CREED_KINDS.contains(&kind) {
        return Err(c::bad());
    }
    let mut tenets: Vec<Tenet> = Vec::new();
    for t in list_bound(opt(v, "tenets"), MAX_TENETS)? {
        let t = Tenet::read(t, true)?;
        if tenets.iter().any(|x| x.id == t.id) {
            return Err(c::bad());
        }
        tenets.push(t);
    }
    let mut observances: Vec<Observance> = Vec::new();
    for o in list_bound(opt(v, "observances"), MAX_OBSERVANCES)? {
        keys(o, &["id", "action", "every", "window", "severity", "label"], &["offset"])?;
        let oid = ident(c::get(o, "id")?)?;
        if oid.len() > 56 || observances.iter().any(|x| x.id == oid) {
            return Err(c::bad());
        }
        let every = uint_bound(c::get(o, "every")?, 1, MAX_PERIOD)?;
        observances.push(Observance {
            id: oid.to_string(),
            action: ident(c::get(o, "action")?)?.to_string(),
            every,
            window: uint_bound(c::get(o, "window")?, 1, every)?,
            offset: opt_uint(o, "offset", 0, every - 1, 0)?,
            severity: uint_bound(c::get(o, "severity")?, 1, 100)?,
            label: text_bound(c::get(o, "label")?, 80)?.to_string(),
        });
    }
    let mut sacred = BTreeSet::new();
    for s in list_bound(opt(v, "sacred"), 16)? {
        sacred.insert(ident(s)?.to_string());
    }
    let mut rivals = BTreeMap::new();
    for r in list_bound(opt(v, "rivals"), 8)? {
        keys(r, &["creed", "stance"], &[])?;
        let other = ident(c::get(r, "creed")?)?.to_string();
        let stance = text_bound(c::get(r, "stance")?, 16)?;
        if other == id || !STANCES.contains(&stance) || rivals.insert(other, stance.to_string()).is_some() {
            return Err(c::bad());
        }
    }
    Ok(Creed {
        id,
        label: text_bound(c::get(v, "label")?, 80)?.to_string(),
        kind: kind.to_string(),
        tenets,
        observances,
        sacred,
        rivals,
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Adherent {
    pub creed: Option<String>,
    pub devotion: u64,
    pub joined_at: u64,
    pub norms: Vec<Tenet>,
    pub standing: BTreeMap<String, u64>,
    /// `observance id` → periods observed (most recent kept per observance).
    pub observed: BTreeMap<String, u64>,
    /// `observance id` → last closed period already charged or cleared.
    pub settled: BTreeMap<String, u64>,
}

/// Creed registry and per-being adherence, norms and standing.
#[derive(Clone, Debug, Default)]
pub struct CreedManager {
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: CreedLimits,
    pub creeds: BTreeMap<String, Creed>,
    pub beings: BTreeMap<String, Adherent>,
}

fn mood(being: &str, delta: i64, reason: String) -> Value {
    obj! {
        "args" => obj! { "being" => being, "reason" => reason, "valence_delta" => delta },
        "engine" => "affect",
        "op" => "set_mood",
    }
}

impl CreedManager {
    pub fn new() -> Self {
        Self::default()
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        run_ops(request, |op, args| match op {
            "register_creed" => self.op_register(args),
            "adhere" => self.op_adhere(args),
            "set_norms" => self.op_set_norms(args),
            "judge" => self.op_judge(args),
            "observe" => self.op_observe(args),
            "due" => self.op_due(args),
            "food_taboos" => self.op_food_taboos(args),
            "regard" => self.op_regard(args),
            "assess" => self.op_assess(args),
            "export" => persist::export_op(self, &self.limits, args),
            "import" => {
                let limits = self.limits.clone();
                let out = persist::import(self, &limits, args)?;
                Ok(out)
            }
            _ => Err(c::bad()),
        })
    }

    fn being_mut(&mut self, being: &str) -> Result<&mut Adherent> {
        if !self.beings.contains_key(being) && self.beings.len() >= self.limits.get("beings") {
            return Err(c::bad());
        }
        Ok(self.beings.entry(being.to_string()).or_default())
    }

    fn op_register(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["creed"], &[])?;
        let creed = read_creed(c::get(args, "creed")?)?;
        if !self.creeds.contains_key(&creed.id) && self.creeds.len() >= self.limits.get("creeds") {
            return Err(c::bad());
        }
        let out = obj! {
            "creed" => creed.id.as_str(),
            "observances" => creed.observances.len(),
            "tenets" => creed.tenets.len(),
            "total" => self.creeds.len() + usize::from(!self.creeds.contains_key(&creed.id)),
        };
        self.creeds.insert(creed.id.clone(), creed);
        Ok(out)
    }

    fn op_adhere(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "creed"], &["devotion", "at"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let creed = opt(args, "creed").map(ident).transpose()?.map(str::to_string);
        if creed.as_ref().is_some_and(|x| !self.creeds.contains_key(x)) {
            return Err(c::bad());
        }
        let devotion = if creed.is_some() { opt_uint(args, "devotion", 0, 100, 50)? } else { 0 };
        if creed.is_none() && opt(args, "devotion").is_some() {
            return Err(c::bad());
        }
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let a = self.being_mut(&being)?;
        let previous = a.creed.take();
        a.creed = creed.clone();
        a.devotion = devotion;
        a.joined_at = at;
        a.observed.clear();
        a.settled.clear();
        Ok(obj! {
            "being" => being,
            "creed" => creed.map(Value::str).unwrap_or(Value::Null),
            "devotion" => devotion,
            "previous" => previous.map(Value::str).unwrap_or(Value::Null),
        })
    }

    fn op_set_norms(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "norms"], &[])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let mut norms: Vec<Tenet> = Vec::new();
        for n in list_bound(Some(c::get(args, "norms")?), MAX_TENETS)? {
            let n = Tenet::read(n, false)?;
            if norms.iter().any(|x| x.id == n.id) {
                return Err(c::bad());
            }
            norms.push(n);
        }
        let a = self.being_mut(&being)?;
        a.norms = norms;
        Ok(obj! { "being" => being, "norms" => a.norms.len() })
    }

    /// Norms `being` holds that `action` breaks: `(tenet, source, weight)`.
    fn breaches(
        &self,
        being: &str,
        action: &str,
        target: Option<&str>,
        tags: &BTreeSet<String>,
    ) -> Vec<(Tenet, String, u64)> {
        let mut out = Vec::new();
        let Some(a) = self.beings.get(being) else { return out };
        if let Some(cr) = a.creed.as_ref().and_then(|x| self.creeds.get(x)) {
            for t in &cr.tenets {
                if t.broken_by(action, target, tags, &cr.sacred) {
                    out.push((t.clone(), cr.id.clone(), a.devotion));
                }
            }
        }
        let none = BTreeSet::new();
        for t in &a.norms {
            if t.broken_by(action, target, tags, &none) {
                out.push((t.clone(), "personal".to_string(), 100));
            }
        }
        out
    }

    fn op_judge(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["actor", "action"], &["target", "witnesses", "food_tags", "at"])?;
        let actor = ident(c::get(args, "actor")?)?.to_string();
        let action = ident(c::get(args, "action")?)?.to_string();
        let target = opt(args, "target").map(ident).transpose()?.map(str::to_string);
        let mut tags = BTreeSet::new();
        for t in list_bound(opt(args, "food_tags"), 8)? {
            let t = ident(t)?;
            if !FOOD_TAGS.contains(&t) {
                return Err(c::bad());
            }
            tags.insert(t.to_string());
        }
        let mut witnesses = BTreeSet::new();
        for w in list_bound(opt(args, "witnesses"), MAX_WITNESSES)? {
            let w = ident(w)?;
            if w == actor || !witnesses.insert(w.to_string()) {
                return Err(c::bad());
            }
        }
        opt_uint(args, "at", 0, MAX_AT, 0)?;

        let mine = self.breaches(&actor, &action, target.as_deref(), &tags);
        let guilt = mine.iter().map(|(t, _, w)| t.severity * w / 100).max().unwrap_or(0);

        let mut indignant = Vec::new();
        let mut by_creed: BTreeMap<String, u64> = BTreeMap::new();
        let mut effects = Vec::new();
        for w in &witnesses {
            let theirs = self.breaches(w, &action, target.as_deref(), &tags);
            let level = theirs.iter().map(|(t, _, wt)| t.severity * wt / 100).max().unwrap_or(0);
            if level == 0 {
                continue;
            }
            for (t, src, wt) in &theirs {
                if src != "personal" {
                    let e = by_creed.entry(src.clone()).or_default();
                    *e = (*e).max(t.severity * wt / 100);
                }
            }
            indignant.push(obj! { "being" => w.as_str(), "indignation" => level });
            effects.push(obj! {
                "args" => obj! {
                    "being" => w.as_str(),
                    "delta" => level,
                    "kind" => "resentment",
                    "reason" => format!("saw {actor} {action}"),
                    "subject" => actor.as_str(),
                },
                "engine" => "affect",
                "op" => "appraise",
            });
        }
        let shame = if guilt > 0 && !indignant.is_empty() { guilt / 2 } else { 0 };
        if guilt + shame > 0 {
            let ids: Vec<&str> = mine.iter().map(|(t, _, _)| t.id.as_str()).collect();
            effects.insert(0, mood(&actor, -((guilt + shame).min(100) as i64), format!("broke {}", ids.join(", "))));
        }
        let mut standing = Vec::new();
        if !by_creed.is_empty() {
            let a = self.being_mut(&actor)?;
            for (cr, level) in by_creed {
                let loss = (level / 5).max(1);
                let s = a.standing.entry(cr.clone()).or_insert(DEFAULT_STANDING);
                *s = s.saturating_sub(loss);
                standing.push(obj! { "creed" => cr.as_str(), "delta" => -(loss as i64), "value" => *s });
            }
        }
        let broken: Vec<Value> = mine
            .iter()
            .map(|(t, src, w)| obj! { "id" => t.id.as_str(), "severity" => t.severity, "source" => src.as_str(), "weight" => *w })
            .collect();
        Ok(obj! {
            "action" => action.as_str(),
            "actor" => actor.as_str(),
            "broken" => Value::Array(broken),
            "effects" => Value::Array(effects),
            "guilt" => guilt,
            "indignant" => Value::Array(indignant),
            "shame" => shame,
            "standing" => Value::Array(standing),
        })
    }

    fn op_observe(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "observance"], &["at"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let oid = ident(c::get(args, "observance")?)?.to_string();
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let a = self.beings.get(&being).ok_or_else(c::bad)?;
        let cr = a.creed.as_ref().and_then(|x| self.creeds.get(x)).ok_or_else(c::bad)?;
        let o = cr.observances.iter().find(|o| o.id == oid).ok_or_else(c::bad)?.clone();
        let creed_id = cr.id.clone();
        let result = match o.period(at) {
            Some((p, true)) if a.observed.get(&oid) == Some(&p) => "already_done",
            Some((p, true)) => {
                let a = self.beings.get_mut(&being).ok_or_else(c::bad)?;
                a.observed.insert(oid.clone(), p);
                a.devotion = (a.devotion + 1).min(100);
                let s = a.standing.entry(creed_id.clone()).or_insert(DEFAULT_STANDING);
                *s = (*s + 1).min(100);
                "done"
            }
            _ => "not_in_window",
        };
        let a = &self.beings[&being];
        Ok(obj! {
            "being" => being.as_str(),
            "devotion" => a.devotion,
            "observance" => oid,
            "result" => result,
            "standing" => a.standing.get(&creed_id).copied().unwrap_or(DEFAULT_STANDING),
        })
    }

    fn op_due(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["at"], &["being"])?;
        let at = uint_bound(c::get(args, "at")?, 0, MAX_AT)?;
        let only = opt(args, "being").map(ident).transpose()?.map(str::to_string);
        if only.as_ref().is_some_and(|b| !self.beings.contains_key(b)) {
            return Err(c::bad());
        }
        let names: Vec<String> =
            self.beings.keys().filter(|b| only.as_ref().is_none_or(|o| o == *b)).cloned().collect();
        let (mut due, mut missed, mut effects) = (Vec::new(), Vec::new(), Vec::new());
        for name in names {
            let a = &self.beings[&name];
            let Some(cr) = a.creed.as_ref().and_then(|x| self.creeds.get(x)).cloned() else { continue };
            let devotion = a.devotion;
            for o in &cr.observances {
                let Some((p, open)) = o.period(at) else { continue };
                let a = self.beings.get_mut(&name).ok_or_else(c::bad)?;
                if open && a.observed.get(&o.id) != Some(&p) {
                    let score = (o.severity * devotion / 100).clamp(20, 70);
                    due.push(obj! {
                        "being" => name.as_str(),
                        "creed" => cr.id.as_str(),
                        "motive" => obj! {
                            "action" => o.action.as_str(),
                            "id" => format!("observe_{}", o.id),
                            "kind" => "goal",
                            "label" => o.label.as_str(),
                            "reason" => format!("{} observance {} is due", cr.id, o.id),
                            "score" => score,
                        },
                        "observance" => o.id.as_str(),
                    });
                }
                // The most recent window that has closed.
                let closed = if open { p.checked_sub(1) } else { Some(p) };
                let Some(closed) = closed else { continue };
                if o.start(closed) < a.joined_at || a.settled.get(&o.id).is_some_and(|s| *s >= closed) {
                    continue;
                }
                a.settled.insert(o.id.clone(), closed);
                if a.observed.get(&o.id) == Some(&closed) {
                    continue;
                }
                let guilt = o.severity * devotion / 100;
                let loss = (o.severity / 10).max(1);
                let s = a.standing.entry(cr.id.clone()).or_insert(DEFAULT_STANDING);
                *s = s.saturating_sub(loss);
                missed.push(obj! {
                    "being" => name.as_str(),
                    "guilt" => guilt,
                    "observance" => o.id.as_str(),
                    "period" => closed,
                    "standing" => *s,
                });
                if guilt > 0 {
                    effects.push(mood(&name, -(guilt as i64), format!("missed {}", o.id)));
                }
            }
        }
        Ok(obj! {
            "at" => at,
            "due" => Value::Array(due),
            "effects" => Value::Array(effects),
            "missed" => Value::Array(missed),
        })
    }

    fn op_food_taboos(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let mut tags = BTreeSet::new();
        if let Some(a) = self.beings.get(being) {
            if let Some(cr) = a.creed.as_ref().and_then(|x| self.creeds.get(x)) {
                if a.devotion >= LAX {
                    tags.extend(cr.tenets.iter().filter_map(|t| t.food_tag.clone()));
                }
            }
            tags.extend(a.norms.iter().filter_map(|t| t.food_tag.clone()));
        }
        Ok(obj! { "being" => being, "forbids" => Value::strings(&tags.iter().collect::<Vec<_>>()) })
    }

    fn op_regard(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "other"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let other = ident(c::get(args, "other")?)?;
        if being == other {
            return Err(c::bad());
        }
        let creed_of = |b: &str| self.beings.get(b).and_then(|a| a.creed.clone());
        let (mine, theirs) = (creed_of(being), creed_of(other));
        let stance = match (&mine, &theirs) {
            (Some(m), Some(t)) if m == t => "kin".to_string(),
            (Some(m), Some(t)) => self.creeds[m].rivals.get(t).cloned().unwrap_or_else(|| "neutral".into()),
            _ => "neutral".into(),
        };
        let o = |x: Option<String>| x.map(Value::str).unwrap_or(Value::Null);
        Ok(
            obj! { "being" => being, "creed" => o(mine), "other" => other, "other_creed" => o(theirs), "stance" => stance },
        )
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let a = self.beings.get(being).ok_or_else(c::bad)?;
        Ok(obj! {
            "being" => being,
            "creed" => a.creed.as_deref().map(Value::str).unwrap_or(Value::Null),
            "devotion" => a.devotion,
            "joined_at" => a.joined_at,
            "norms" => Value::Array(a.norms.iter().map(Tenet::to_value).collect()),
            "observed" => Value::Array(a.observed.iter().map(|(o, p)| obj! { "observance" => o.as_str(), "period" => *p }).collect()),
            "standing" => Value::Array(a.standing.iter().map(|(cr, v)| obj! { "creed" => cr.as_str(), "value" => *v }).collect()),
        })
    }
}

persist!(Tenet { action, food_tag, id, label, sacred_only, severity [1, 100] });
persist!(Observance {
    action,
    every [1, MAX_PERIOD],
    id,
    label,
    offset [0, MAX_PERIOD],
    severity [1, 100],
    window [1, MAX_PERIOD]
});
persist!(Creed { id, kind, label, observances, rivals, sacred, tenets });
persist!(Adherent { creed, devotion [0, 100], joined_at [0, MAX_AT], norms, observed, settled, standing });
persist!(rest CreedManager { beings, creeds });
durable!(CreedManager, "creed", LIMITS);

/// A tenet as `Tenet::read` would accept it.
fn tenet_ok(t: &Tenet, allow_sacred: bool) -> bool {
    is_ident(&t.id)
        && is_text(&t.label, 80)
        && t.action.is_some() != t.food_tag.is_some()
        && t.action.as_deref().is_none_or(is_ident)
        && t.food_tag.as_deref().is_none_or(|x| FOOD_TAGS.contains(&x))
        && (!t.sacred_only || (allow_sacred && t.action.is_some()))
}

/// Ids are distinct.
fn distinct<'a>(ids: impl Iterator<Item = &'a String>) -> bool {
    let mut seen = BTreeSet::new();
    ids.into_iter().all(|x| seen.insert(x))
}

impl CreedManager {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("creeds", self.creeds.len())?;
        within::<Self>("beings", self.beings.len())?;
        for (id, cr) in &self.creeds {
            ok(*id == cr.id && is_ident(id) && is_text(&cr.label, 80) && CREED_KINDS.contains(&cr.kind.as_str()))?;
            ok(cr.tenets.len() <= MAX_TENETS && cr.tenets.iter().all(|t| tenet_ok(t, true)))?;
            ok(distinct(cr.tenets.iter().map(|t| &t.id)) && distinct(cr.observances.iter().map(|o| &o.id)))?;
            ok(cr.observances.len() <= MAX_OBSERVANCES)?;
            for o in &cr.observances {
                ok(is_ident(&o.id) && o.id.len() <= 56 && is_ident(&o.action) && is_text(&o.label, 80))?;
                ok(o.window <= o.every && o.offset < o.every)?;
            }
            ok(cr.sacred.len() <= 16 && cr.sacred.iter().all(|s| is_ident(s)))?;
            ok(cr.rivals.len() <= 8)?;
            ok(cr.rivals.iter().all(|(r, s)| is_ident(r) && r != id && STANCES.contains(&s.as_str())))?;
        }
        for (b, a) in &self.beings {
            ok(is_ident(b) && a.creed.as_ref().is_none_or(|x| self.creeds.contains_key(x)))?;
            ok(a.creed.is_some() || (a.devotion == 0 && a.observed.is_empty() && a.settled.is_empty()))?;
            ok(a.norms.len() <= MAX_TENETS && a.norms.iter().all(|t| tenet_ok(t, false)))?;
            ok(distinct(a.norms.iter().map(|t| &t.id)))?;
            ok(a.standing.iter().all(|(cr, v)| self.creeds.contains_key(cr) && *v <= 100))?;
            let periods = a.observed.iter().chain(&a.settled);
            ok(periods.into_iter().all(|(o, p)| is_ident(o) && o.len() <= 56 && *p <= MAX_AT))?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    CreedManager::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    const HEARTH: &str = r#"{"op":"register_creed","args":{"creed":{"id":"hearthkeepers","label":"Keepers of the Hearth","kind":"religion",
        "tenets":[{"id":"no_theft","action":"steal","severity":80,"label":"Take nothing from another's hearth"},
                  {"id":"no_ash_scatter","action":"scatter","sacred_only":true,"severity":60,"label":"Never scatter sacred ash"},
                  {"id":"no_flesh","food_tag":"meat","severity":40,"label":"Eat nothing that bled"}],
        "observances":[{"id":"tend_flame","action":"tend_hearth","every":24,"window":4,"offset":6,"severity":50,"label":"Tend the flame at dawn"}],
        "sacred":["great_hearth"]}}}"#;

    fn run(m: &mut CreedManager, ops: &str) -> String {
        m.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16).unwrap()).unwrap().to_json()
    }

    #[test]
    fn sacred_only_tenet_needs_a_sacred_target() {
        let mut m = CreedManager::new();
        let a = r#"{"op":"adhere","args":{"being":"b","creed":"hearthkeepers","devotion":100}}"#;
        let out = run(
            &mut m,
            &format!(
                r#"{HEARTH},{a},{{"op":"judge","args":{{"actor":"b","action":"scatter","target":"old_hearth"}}}},
                   {{"op":"judge","args":{{"actor":"b","action":"scatter","target":"great_hearth"}}}}"#
            ),
        );
        assert!(out.contains(r#""guilt":0"#) && out.contains(r#""guilt":60"#), "{out}");
    }

    #[test]
    fn missed_window_charged_once_and_not_before_joining() {
        let mut m = CreedManager::new();
        let a = r#"{"op":"adhere","args":{"being":"b","creed":"hearthkeepers","devotion":80,"at":20}}"#;
        let out = run(
            &mut m,
            &format!(
                r#"{HEARTH},{a},{{"op":"due","args":{{"at":29}}}},{{"op":"due","args":{{"at":40}}}},{{"op":"due","args":{{"at":41}}}}"#
            ),
        );
        // Window [6,10) of period 0 began before joining; period 1 [30,34) is missed at 40, once.
        assert_eq!(out.matches(r#""observance":"tend_flame","period":1"#).count(), 1, "{out}");
    }

    #[test]
    fn food_taboos_respect_lax_devotion() {
        let mut m = CreedManager::new();
        let out = run(
            &mut m,
            &format!(
                r#"{HEARTH},{{"op":"adhere","args":{{"being":"lax","creed":"hearthkeepers","devotion":29}}}},
                   {{"op":"adhere","args":{{"being":"firm","creed":"hearthkeepers","devotion":30}}}},
                   {{"op":"food_taboos","args":{{"being":"lax"}}}},{{"op":"food_taboos","args":{{"being":"firm"}}}}"#
            ),
        );
        assert!(
            out.contains(r#""being":"lax","forbids":[]"#) && out.contains(r#""being":"firm","forbids":["meat"]"#),
            "{out}"
        );
    }
}
