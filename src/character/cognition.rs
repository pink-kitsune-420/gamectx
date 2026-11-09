//! Bounded rationality and fallible memory (pass 2, A7/A8).
//!
//! **Bounded rationality (A7).** Each being has cognition settings from a temperament:
//!
//! | temperament | plan depth | horizon | impulsiveness | memory span | slip (millionths) |
//! |---|---|---|---|---|---|
//! | `careful` | 6 | 240 | 10 | 2,000 | 50,000 |
//! | `steady` (default) | 4 | 120 | 25 | 1,000 | 100,000 |
//! | `impulsive` | 2 | 30 | 70 | 500 | 200,000 |
//! | `dreamy` | 3 | 60 | 40 | 300 | 250,000 |
//!
//! `decide` is seeded and deterministic. If FNV-1a of `"<seed>:<being>:impulse"` mod 100 is below
//! impulsiveness, the being acts on its top motive without a plan. Otherwise the plan keeps at
//! most `plan_depth` steps due within the horizon, and an unfamiliar step slips when FNV-1a of
//! `"<seed>:<being>:<step>"` mod 1,000,000 is below the slip rate.
//!
//! **Fallible memory (A8).** Memories start at strength 1,000,000 and lose
//! `elapsed·1,000,000·(100 − importance) / (100·memory_span)` on `tick` (importance 100 never
//! fades); they are `vivid` at 900,000 or more, `clear` at 600,000 or more, else `hazy`, and are
//! forgotten at 0. `recall` rehearses a memory (+100,000). A hazy memory may come back wrong (a
//! day shifted by 1–3, or a person merged in from another memory) with probability
//! `(600,000 − strength)` millionths; the reply always carries the `truth` beside what is `told`,
//! so the host keeps the facts while beings can give conflicting testimony.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::cognition::Cognition;
//!
//! let mut mind = Cognition::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "profile", "args": {"being": "miller", "temperament": "dreamy"}},
//!   {"op": "remember", "args": {"being": "miller", "id": "fire", "about": "barn", "what": "burned",
//!                               "day": 12, "people": ["tinker"], "importance": 40}},
//!   {"op": "tick", "args": {"now": 250}}
//! ]}"#, 4096)?;
//! let out = mind.workflow(&req)?.to_json();
//! assert!(out.contains(r#""clarity":"hazy""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::attention::fnv1a;
use crate::character::beliefs::{opt, opt_bool, opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "memories", default: 256, ceiling: 65_536 },
    Limit { name: "people", default: 8, ceiling: 64 },
    Limit { name: "motives", default: 16, ceiling: 256 },
    Limit { name: "steps", default: 16, ceiling: 256 },
];
pub const M: u64 = 1_000_000;
pub const VIVID: u64 = 900_000;
pub const CLEAR: u64 = 600_000;
pub const REHEARSE: u64 = 100_000;

/// `(name, plan_depth, horizon, impulsiveness, memory_span, slip)`.
pub const TEMPERAMENTS: [(&str, u64, u64, u64, u64, u64); 4] = [
    ("careful", 6, 240, 10, 2000, 50_000),
    ("steady", 4, 120, 25, 1000, 100_000),
    ("impulsive", 2, 30, 70, 500, 200_000),
    ("dreamy", 3, 60, 40, 300, 250_000),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub temperament: String,
    pub plan_depth: u64,
    pub horizon: u64,
    pub impulsiveness: u64,
    pub memory_span: u64,
    pub slip: u64,
}

impl Profile {
    fn of(t: &str) -> Option<Self> {
        TEMPERAMENTS.iter().find(|x| x.0 == t).map(|&(name, plan_depth, horizon, impulsiveness, memory_span, slip)| {
            Profile { temperament: name.to_string(), plan_depth, horizon, impulsiveness, memory_span, slip }
        })
    }

    fn fields(&self) -> Vec<(String, Value)> {
        vec![
            ("temperament".into(), Value::from(self.temperament.as_str())),
            ("plan_depth".into(), Value::from(self.plan_depth)),
            ("horizon".into(), Value::from(self.horizon)),
            ("impulsiveness".into(), Value::from(self.impulsiveness)),
            ("memory_span".into(), Value::from(self.memory_span)),
            ("slip".into(), Value::from(self.slip)),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
    pub about: String,
    pub what: String,
    pub day: u64,
    pub people: Vec<String>,
    pub importance: u64,
    pub strength: u64,
    pub last: u64,
}

impl Memory {
    fn facts(&self) -> Value {
        obj! {
            "about" => self.about.as_str(), "day" => self.day,
            "people" => Value::Array(self.people.iter().map(|p| Value::from(p.as_str())).collect()),
            "what" => self.what.as_str(),
        }
    }

    fn view(&self, id: &str) -> Value {
        obj! {
            "id" => id, "about" => self.about.as_str(), "day" => self.day,
            "people" => Value::Array(self.people.iter().map(|p| Value::from(p.as_str())).collect()),
            "what" => self.what.as_str(), "clarity" => clarity(self.strength), "importance" => self.importance,
            "strength" => self.strength,
        }
    }
}

pub fn clarity(strength: u64) -> &'static str {
    if strength >= VIVID {
        "vivid"
    } else if strength >= CLEAR {
        "clear"
    } else {
        "hazy"
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

fn list(v: Option<&Value>, max: usize, nonempty: bool) -> Result<&[Value]> {
    let xs = match v {
        None => &[][..],
        Some(v) => v.as_array().ok_or_else(c::bad)?,
    };
    need(xs.len() <= max && (!nonempty || !xs.is_empty()))?;
    Ok(xs)
}

/// Cognition settings and memories per being.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cognition {
    pub profiles: BTreeMap<String, Profile>,
    pub memories: BTreeMap<String, BTreeMap<String, Memory>>,
    pub clock: u64,
    /// Beings with a profile or memories (the `beings` limit counts these).
    pub known: BTreeSet<String>,
    pub limits: Limits,
}

impl Default for Cognition {
    fn default() -> Self {
        Cognition {
            profiles: BTreeMap::new(),
            memories: BTreeMap::new(),
            clock: 0,
            known: BTreeSet::new(),
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Cognition {
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
            "profile" => self.op_profile(args, l),
            "decide" => self.op_decide(args, l),
            "remember" => self.op_remember(args, l),
            "tick" => self.op_tick(args),
            "recall" => self.op_recall(args),
            "forget" => self.op_forget(args),
            "assess" => self.op_assess(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    /// The being's settings (a `steady` default when it has no profile).
    pub fn settings(&self, being: &str) -> Profile {
        self.profiles.get(being).cloned().unwrap_or_else(|| {
            Profile::of("steady").unwrap_or(Profile {
                temperament: "steady".into(),
                plan_depth: 4,
                horizon: 120,
                impulsiveness: 25,
                memory_span: 1000,
                slip: 100_000,
            })
        })
    }

    fn room(&self, being: &str, l: &Limits) -> Result<()> {
        need(self.known.contains(being) || self.known.len() < l.get("beings"))
    }

    // ------------------------------------------------------------ A7

    fn op_profile(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being"], &["temperament", "plan_depth", "horizon", "impulsiveness", "memory_span", "slip"])?;
        let being = id_of(args, "being")?;
        let t = opt(args, "temperament").map(ident).transpose()?.unwrap_or("steady");
        let mut p = Profile::of(t).ok_or_else(c::bad)?;
        p.plan_depth = opt_uint(args, "plan_depth", 1, 8, p.plan_depth)?;
        p.horizon = opt_uint(args, "horizon", 1, 10_000, p.horizon)?;
        p.impulsiveness = opt_uint(args, "impulsiveness", 0, 100, p.impulsiveness)?;
        p.memory_span = opt_uint(args, "memory_span", 1, 1_000_000, p.memory_span)?;
        p.slip = opt_uint(args, "slip", 0, M, p.slip)?;
        self.room(&being, l)?;
        let mut fields = vec![("being".to_string(), Value::from(being.as_str()))];
        fields.extend(p.fields());
        self.known.insert(being.clone());
        self.profiles.insert(being, p);
        Ok(Value::Object(fields))
    }

    fn op_decide(&self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "seed", "motives"], &["steps", "at"])?;
        let being = id_of(args, "being")?;
        let seed = uint_bound(c::get(args, "seed")?, 0, MAX_AT)?;
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let mut motives: Vec<(String, u64)> = Vec::new();
        for m in list(Some(c::get(args, "motives")?), l.get("motives"), true)? {
            keys(m, &["id", "score"], &[])?;
            motives.push((id_of(m, "id")?, uint_bound(c::get(m, "score")?, 0, 100)?));
        }
        need(motives.iter().map(|m| &m.0).collect::<BTreeSet<_>>().len() == motives.len())?;
        let mut steps: Vec<(String, bool, u64)> = Vec::new();
        for s in list(opt(args, "steps"), l.get("steps"), false)? {
            keys(s, &["id"], &["familiar", "eta"])?;
            steps.push((id_of(s, "id")?, opt_bool(s, "familiar", true)?, opt_uint(s, "eta", 0, MAX_AT, at)?));
        }
        need(steps.iter().map(|s| &s.0).collect::<BTreeSet<_>>().len() == steps.len())?;
        let p = self.settings(&being);
        let top = motives.iter().min_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))).map(|m| m.0.clone());
        let top = top.ok_or_else(c::bad)?;
        let roll = fnv1a(&format!("{seed}:{being}:impulse")) % 100;
        if roll < p.impulsiveness {
            return Ok(obj! {
                "being" => being, "beyond_horizon" => Value::Array(Vec::new()), "dropped" => 0u64, "mode" => "impulse",
                "motive" => top, "plan" => Value::Array(Vec::new()), "roll" => roll, "slipped" => Value::Null,
            });
        }
        let edge = at + p.horizon;
        let near: Vec<&(String, bool, u64)> = steps.iter().filter(|s| s.2 <= edge).collect();
        let beyond: Vec<Value> = steps.iter().filter(|s| s.2 > edge).map(|s| Value::from(s.0.as_str())).collect();
        let depth = (p.plan_depth as usize).min(near.len());
        let mut plan = Vec::new();
        let mut slipped = Value::Null;
        for (sid, familiar, _) in near.iter().take(depth).map(|s| (&s.0, s.1, s.2)) {
            let slip = !familiar && fnv1a(&format!("{seed}:{being}:{sid}")) % M < p.slip;
            if slip && slipped == Value::Null {
                slipped = Value::from(sid.as_str());
            }
            plan.push(obj! { "id" => sid.as_str(), "slip" => slip });
        }
        Ok(obj! {
            "being" => being, "beyond_horizon" => Value::Array(beyond), "dropped" => near.len() - depth,
            "mode" => "plan", "motive" => top, "plan" => Value::Array(plan), "roll" => roll, "slipped" => slipped,
        })
    }

    // ------------------------------------------------------------ A8

    fn op_remember(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "id", "about", "what", "day"], &["people", "importance", "at"])?;
        let being = id_of(args, "being")?;
        let mid = id_of(args, "id")?;
        let about = id_of(args, "about")?;
        let what = id_of(args, "what")?;
        let day = uint_bound(c::get(args, "day")?, 0, MAX_AT)?;
        let people = list(opt(args, "people"), l.get("people"), false)?
            .iter()
            .map(|v| ident(v).map(str::to_string))
            .collect::<Result<Vec<_>>>()?;
        need(people.iter().collect::<BTreeSet<_>>().len() == people.len())?;
        let importance = opt_uint(args, "importance", 0, 100, 50)?;
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        need(self.memories.get(&being).is_none_or(|b| !b.contains_key(&mid)))?;
        self.room(&being, l)?;
        let last = at.max(self.clock);
        let book = self.memories.entry(being.clone()).or_default();
        let mut evicted = Value::Null;
        if book.len() >= l.get("memories") {
            let victim = book
                .iter()
                .min_by(|a, b| (a.1.strength, a.1.importance, a.0).cmp(&(b.1.strength, b.1.importance, b.0)))
                .map(|(k, _)| k.clone());
            if let Some(v) = victim {
                book.remove(&v);
                evicted = Value::from(v);
            }
        }
        let m = Memory { about, what, day, people, importance, strength: M, last };
        let view = m.view(&mid);
        book.insert(mid, m);
        self.known.insert(being.clone());
        Ok(obj! { "being" => being, "evicted" => evicted, "memory" => view })
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        let mut changed = Vec::new();
        let mut forgotten = Vec::new();
        let spans: BTreeMap<String, u64> =
            self.memories.keys().map(|b| (b.clone(), self.settings(b).memory_span)).collect();
        for (b, book) in self.memories.iter_mut() {
            let span = spans.get(b).copied().unwrap_or(1000);
            book.retain(|mid, m| {
                if m.importance >= 100 || now <= m.last {
                    return true;
                }
                let elapsed = (now - m.last).min(100 * span) as u128;
                let loss = elapsed * M as u128 * (100 - m.importance) as u128 / (100 * span) as u128;
                let before = clarity(m.strength);
                m.strength = (m.strength as u128).saturating_sub(loss) as u64;
                m.last = now;
                if m.strength == 0 {
                    forgotten.push(obj! { "being" => b.as_str(), "id" => mid.as_str() });
                    return false;
                }
                if clarity(m.strength) != before {
                    changed
                        .push(obj! { "being" => b.as_str(), "clarity" => clarity(m.strength), "id" => mid.as_str() });
                }
                true
            });
        }
        let profiles = &self.profiles;
        let known = &mut self.known;
        self.memories.retain(|b, book| {
            if book.is_empty() && !profiles.contains_key(b) {
                known.remove(b);
            }
            !book.is_empty()
        });
        self.clock = now;
        Ok(obj! { "changed" => Value::Array(changed), "forgotten" => Value::Array(forgotten), "now" => now })
    }

    fn op_recall(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "id", "seed"], &["errors"])?;
        let being = id_of(args, "being")?;
        let mid = id_of(args, "id")?;
        let seed = uint_bound(c::get(args, "seed")?, 0, MAX_AT)?;
        let errors = opt_bool(args, "errors", true)?;
        let book = self.memories.get(&being).ok_or_else(c::bad)?;
        let m = book.get(&mid).ok_or_else(c::bad)?;
        let truth = m.facts();
        let mut told = m.clone();
        let mut kind = Value::Null;
        if errors && m.strength < CLEAR {
            let roll = fnv1a(&format!("{seed}:{being}:{mid}")) % M;
            if roll < CLEAR - m.strength {
                let mine: BTreeSet<&String> = m.people.iter().collect();
                let others: Vec<&String> = book
                    .iter()
                    .filter(|(k, _)| **k != mid)
                    .flat_map(|(_, o)| o.people.iter())
                    .filter(|p| !mine.contains(p))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                if (roll / 7) % 2 == 1 && !m.people.is_empty() && !others.is_empty() {
                    kind = Value::from("merged_person");
                    told.people[0] = others[(roll % others.len() as u64) as usize].clone();
                } else {
                    kind = Value::from("wrong_day");
                    let shift = 1 + roll % 3;
                    told.day = if (roll / 3) % 2 == 0 || m.day < shift { m.day + shift } else { m.day - shift };
                }
            }
        }
        let distorted = kind != Value::Null;
        let m = self.memories.get_mut(&being).and_then(|b| b.get_mut(&mid)).ok_or_else(c::bad)?;
        m.strength = (m.strength + REHEARSE).min(M);
        Ok(obj! {
            "being" => being, "clarity" => clarity(m.strength), "distorted" => distorted, "id" => mid.as_str(),
            "kind" => kind, "strength" => m.strength, "told" => told.facts(), "truth" => truth,
        })
    }

    fn op_forget(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "id"], &[])?;
        let being = id_of(args, "being")?;
        let mid = id_of(args, "id")?;
        let book = self.memories.get_mut(&being).ok_or_else(c::bad)?;
        need(book.remove(&mid).is_some())?;
        if book.is_empty() {
            self.memories.remove(&being);
            if !self.profiles.contains_key(&being) {
                self.known.remove(&being);
            }
        }
        Ok(obj! { "being" => being, "forgot" => mid })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = id_of(args, "being")?;
        let mut mems: Vec<(&String, &Memory)> =
            self.memories.get(&being).map(|b| b.iter().collect()).unwrap_or_default();
        mems.sort_by(|a, b| b.1.strength.cmp(&a.1.strength).then_with(|| a.0.cmp(b.0)));
        Ok(obj! {
            "being" => being.as_str(),
            "memories" => Value::Array(mems.into_iter().map(|(k, m)| m.view(k)).collect()),
            "profile" => Value::Object(self.settings(&being).fields()),
        })
    }
}

persist!(Profile {
    horizon [1, 10_000],
    impulsiveness [0, 100],
    memory_span [1, 1_000_000],
    plan_depth [1, 8],
    slip [0, M],
    temperament
});
persist!(Memory { about, day [0, MAX_AT], importance [0, 100], last [0, MAX_AT], people, strength [1, M], what });
persist!(rest Cognition { clock [0, MAX_AT], memories, profiles });
durable!(Cognition, "cognition", LIMITS);

impl Cognition {
    /// Invariants a save must meet; `known` is rebuilt from profiles and memories.
    fn settle_state(&mut self) -> Result<()> {
        for (b, p) in &self.profiles {
            ok(is_ident(b) && TEMPERAMENTS.iter().any(|t| t.0 == p.temperament))?;
        }
        for (b, book) in &self.memories {
            ok(is_ident(b) && !book.is_empty())?;
            within::<Self>("memories", book.len())?;
            for (id, m) in book {
                ok(is_ident(id) && is_ident(&m.about) && is_ident(&m.what))?;
                within::<Self>("people", m.people.len())?;
                let mut seen = BTreeSet::new();
                ok(m.people.iter().all(|p| is_ident(p) && seen.insert(p)))?;
            }
        }
        self.known = self.profiles.keys().chain(self.memories.keys()).cloned().collect();
        within::<Self>("beings", self.known.len())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Cognition::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(m: &mut Cognition, ops: &str) -> Result<String> {
        m.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16)?).map(|v| v.to_json())
    }

    #[test]
    fn important_memories_never_fade_and_trivial_ones_are_forgotten() {
        let mut m = Cognition::new();
        let out = run(
            &mut m,
            r#"{"op":"remember","args":{"being":"b","id":"wed","about":"sister","what":"married","day":1,"importance":100}},
               {"op":"remember","args":{"being":"b","id":"rain","about":"sky","what":"rained","day":1,"importance":0}},
               {"op":"tick","args":{"now":1000}}"#,
        )
        .unwrap();
        assert!(out.contains(r#""forgotten":[{"being":"b","id":"rain"}]"#), "{out}");
        assert_eq!(m.memories["b"]["wed"].strength, M);
    }

    #[test]
    fn recall_without_errors_tells_the_truth() {
        let mut m = Cognition::new();
        let out = run(
            &mut m,
            r#"{"op":"remember","args":{"being":"b","id":"x","about":"a","what":"w","day":5,"importance":0}},
               {"op":"tick","args":{"now":900}},
               {"op":"recall","args":{"being":"b","id":"x","seed":3,"errors":false}}"#,
        )
        .unwrap();
        assert!(out.contains(r#""distorted":false"#) && out.contains(r#""strength":200000"#), "{out}");
    }
}
