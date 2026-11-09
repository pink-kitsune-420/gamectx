//! Mind architectures (pass 4, stage 2).
//!
//! A mind architecture says what kind of mind a being has: where its memory lives
//! (`individual`, `shared` in a pool, or `none`), which affect channels exist, whether it has
//! language (`full`, `signals`, `none`), whether it can deceive (`none`, `concealment`, `full`),
//! how deep its theory of mind goes (0: others are objects; 1: others want and know things; 2:
//! others model me), its subjective time scale (millionths, 1,000,000 = human) and an optional
//! fixation that pins its top motive.
//!
//! Five minds are built in (`human`, `hive_tender`, `steward_intelligence`, `herd_animal`,
//! `hearth_spirit`), all original. `assign` gives a being a mind (unassigned beings are human).
//! `admit` checks one engine request a host (or a model proposal) wants to run for a being,
//! before it reaches the engine: it refuses what the mind cannot do (a cover story from a being
//! that cannot lie or cannot model others, speech from a being without language, memory for a
//! being without memory), strips inputs for affect channels the mind lacks, routes shared memory
//! to the pool, and stretches time windows by the time scale. Existing engines are unchanged.
//! `fixate` pins a fixation at the top of a motive list until `release`; `hear` says how much of
//! an utterance a being takes in (words, tone, or nothing).
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::minds::Minds;
//!
//! let mut m = Minds::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "assign", "args": {"being": "drone_7", "mind": "hive_tender", "pool": "east_hive"}},
//!   {"op": "admit", "args": {"being": "drone_7", "kind": "cueing", "op": "episode",
//!     "args": {"being": "drone_7", "id": "wasp_raid", "features": {"tags": ["wasps"]}, "emotion": "shame"}}},
//!   {"op": "admit", "args": {"being": "steward", "kind": "embodiment", "op": "react",
//!     "args": {"being": "steward", "lying": true}}},
//!   {"op": "assign", "args": {"being": "goat", "mind": "herd_animal"}},
//!   {"op": "admit", "args": {"being": "goat", "kind": "pragmatics", "op": "parse", "args": {"text": "Hello!"}}}
//! ]}"#, 4096)?;
//! let out = m.workflow(&req)?.to_json();
//! // The drone's memory goes to its hive, without an emotion the hive does not feel; an
//! // unassigned (human) steward may lie; a goat cannot parse words.
//! assert!(out.contains(r#""args":{"being":"east_hive","id":"wasp_raid","features":{"tags":["wasps"]}}"#));
//! assert!(out.contains(r#""stripped":["emotion"],"target":"east_hive""#));
//! assert!(out.contains(r#""reasons":["signals_only"]"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::character::pragmatics::ACTS;
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const LIMITS: &[Limit] =
    &[Limit { name: "minds", default: 16, ceiling: 4096 }, Limit { name: "beings", default: 1024, ceiling: 1_000_000 }];
pub const HUMAN_SCALE: u64 = 1_000_000;
pub const CHANNELS: [&str; 8] = ["affection", "anger", "fear", "guilt", "joy", "resentment", "shame", "shock"];
pub const MEMORY: [&str; 3] = ["individual", "shared", "none"];
pub const LANGUAGE: [&str; 3] = ["full", "signals", "none"];
pub const DECEPTION: [&str; 3] = ["none", "concealment", "full"];
pub const KINDS: [&str; 3] = ["cueing", "pragmatics", "embodiment"];
const MEMORY_OPS: [&str; 8] = ["episode", "cue", "surface", "fact", "cover", "uncover", "frame_facts", "gate"];
const SPEECH_OPS: [&str; 9] =
    ["parse", "validate", "agenda", "open", "interrupt", "resume", "close", "threads", "stance"];
const WORD_OPS: [&str; 3] = ["parse", "validate", "agenda"];
const REACT_ARGS: [&str; 4] = ["fear", "anger", "shock", "guilt"];
const VOICE_ARGS: [&str; 2] = ["fear", "affection"];
const ATTITUDE_ARGS: [&str; 3] = ["affection", "resentment", "fear"];

/// A mind: memory, affect channels (sorted), language, deception, theory of mind, time scale,
/// fixation (goal, strength).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mind {
    pub memory: &'static str,
    pub affect: Vec<&'static str>,
    pub language: &'static str,
    pub deception: &'static str,
    pub theory_of_mind: u64,
    pub time_scale: u64,
    pub fixation: Option<(String, u64)>,
}

/// Built-in minds.
pub fn builtin(id: &str) -> Option<Mind> {
    let m = |memory, affect: &[&'static str], language, deception, tom, scale, fix: Option<(&str, u64)>| Mind {
        memory,
        affect: affect.to_vec(),
        language,
        deception,
        theory_of_mind: tom,
        time_scale: scale,
        fixation: fix.map(|(g, s)| (g.to_string(), s)),
    };
    Some(match id {
        "human" => m("individual", &CHANNELS, "full", "full", 2, HUMAN_SCALE, None),
        "hive_tender" => m("shared", &["fear", "joy"], "signals", "none", 1, HUMAN_SCALE, None),
        "steward_intelligence" => m("individual", &["shock"], "full", "concealment", 2, 100_000, None),
        "herd_animal" => {
            m("individual", &["affection", "anger", "fear", "shock"], "signals", "none", 0, HUMAN_SCALE, None)
        }
        "hearth_spirit" => m(
            "individual",
            &["affection", "anger", "guilt", "resentment", "shame"],
            "full",
            "concealment",
            1,
            20_000_000,
            Some(("keep_the_household", 90)),
        ),
        _ => return None,
    })
}

pub const BUILTIN_IDS: [&str; 5] = ["human", "hive_tender", "steward_intelligence", "herd_animal", "hearth_spirit"];

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

fn pick(v: &Value, options: &[&'static str]) -> Result<&'static str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    options.iter().copied().find(|x| *x == s).ok_or_else(c::bad)
}

fn tone(act: &str) -> &'static str {
    match act {
        "threat" | "accusation" => "hostile",
        "greeting" | "compliment" | "apology" | "offer_or_transfer" => "friendly",
        "request_help" => "pleading",
        "farewell" => "parting",
        _ => "neutral",
    }
}

/// Scales a tick count by a time scale in millionths, at least 1.
pub fn scale(v: u64, s: u64) -> u64 {
    ((v as u128 * s as u128 / HUMAN_SCALE as u128) as u64).max(1)
}

/// An object's fields, editable in place (insertion order kept).
struct Fields(Vec<(String, Value)>);

impl Fields {
    fn has(&self, k: &str) -> bool {
        self.0.iter().any(|(x, _)| x == k)
    }
    fn get(&self, k: &str) -> Option<&Value> {
        self.0.iter().find(|(x, _)| x == k).map(|(_, v)| v)
    }
    fn remove(&mut self, k: &str) -> bool {
        let n = self.0.len();
        self.0.retain(|(x, _)| x != k);
        self.0.len() != n
    }
    fn set(&mut self, k: &str, v: Value) {
        match self.0.iter_mut().find(|(x, _)| x == k) {
            Some(slot) => slot.1 = v,
            None => self.0.push((k.to_string(), v)),
        }
    }
}

/// A bounded tick count (0..=10¹² for cooldowns, 1..=10¹² for rates); anything else is left alone.
fn ticks(v: Option<&Value>, lo: u64) -> Option<u64> {
    match v {
        Some(Value::Number(s)) => s.parse::<u64>().ok().filter(|n| (lo..=MAX_AT).contains(n)),
        _ => None,
    }
}

/// A being's assignment: mind id, shared pool, fixation released.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub mind: String,
    pub pool: Option<String>,
    pub released: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Minds {
    pub minds: BTreeMap<String, Mind>,
    pub beings: BTreeMap<String, Assignment>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Minds {
    fn default() -> Self {
        Minds { minds: BTreeMap::new(), beings: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

impl Minds {
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "tick" => self.op_tick(args),
            "define" => self.op_define(args, l),
            "show" => self.op_show(args),
            "assign" => self.op_assign(args, l),
            "admit" => self.op_admit(args),
            "fixate" => self.op_fixate(args),
            "release" => self.op_release(args),
            "hear" => self.op_hear(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    /// A built-in or authored mind.
    pub fn mind(&self, id: &str) -> Result<Mind> {
        match builtin(id) {
            Some(m) => Ok(m),
            None => self.minds.get(id).cloned().ok_or_else(c::bad),
        }
    }

    fn assignment(&self, b: &str) -> Assignment {
        self.beings.get(b).cloned().unwrap_or(Assignment { mind: "human".into(), pool: None, released: false })
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn op_define(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(
            args,
            &["id", "memory", "affect", "language", "deception", "theory_of_mind"],
            &["time_scale", "fixation"],
        )?;
        let id = id_of(args, "id")?;
        need(builtin(&id).is_none())?;
        let memory = pick(c::get(args, "memory")?, &MEMORY)?;
        let language = pick(c::get(args, "language")?, &LANGUAGE)?;
        let deception = pick(c::get(args, "deception")?, &DECEPTION)?;
        let rows = c::get(args, "affect")?.as_array().ok_or_else(c::bad)?;
        need(rows.len() <= CHANNELS.len())?;
        let mut affect = Vec::new();
        for x in rows {
            ident(x)?;
            let ch = pick(x, &CHANNELS)?;
            need(!affect.contains(&ch))?;
            affect.push(ch);
        }
        affect.sort();
        let theory_of_mind = uint_bound(c::get(args, "theory_of_mind")?, 0, 2)?;
        let time_scale = opt_uint(args, "time_scale", 1, 1_000_000_000, HUMAN_SCALE)?;
        let fixation = match opt(args, "fixation") {
            None => None,
            Some(fx) => {
                keys(fx, &["goal", "strength"], &[])?;
                Some((id_of(fx, "goal")?, uint_bound(c::get(fx, "strength")?, 1, 100)?))
            }
        };
        need(self.minds.contains_key(&id) || self.minds.len() < l.get("minds"))?;
        // A redefinition keeps every assigned being consistent: pooled exactly when memory is shared.
        need(self.beings.values().filter(|st| st.mind == id).all(|st| st.pool.is_some() == (memory == "shared")))?;
        self.minds
            .insert(id.clone(), Mind { memory, affect, language, deception, theory_of_mind, time_scale, fixation });
        Ok(obj! { "id" => id })
    }

    fn op_show(&self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &[])?;
        let id = id_of(args, "id")?;
        let m = self.mind(&id)?;
        Ok(obj! {
            "affect" => m.affect.clone(),
            "builtin" => builtin(&id).is_some(),
            "deception" => m.deception,
            "fixation" => m.fixation.as_ref().map(|(g, s)| obj! { "goal" => g.as_str(), "strength" => *s }),
            "id" => id,
            "language" => m.language,
            "memory" => m.memory,
            "theory_of_mind" => m.theory_of_mind,
            "time_scale" => m.time_scale,
        })
    }

    fn op_assign(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "mind"], &["pool"])?;
        let b = id_of(args, "being")?;
        let mid = id_of(args, "mind")?;
        let m = self.mind(&mid)?;
        let pool = opt(args, "pool").map(|v| ident(v).map(str::to_string)).transpose()?;
        need(pool.is_some() == (m.memory == "shared"))?;
        need(self.beings.contains_key(&b) || self.beings.len() < l.get("beings"))?;
        self.beings.insert(b.clone(), Assignment { mind: mid.clone(), pool: pool.clone(), released: false });
        Ok(obj! { "being" => b, "mind" => mid, "pool" => pool })
    }

    fn op_admit(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "kind", "op", "args"], &[])?;
        let b = id_of(args, "being")?;
        let kind = ident(c::get(args, "kind")?)?;
        let op = ident(c::get(args, "op")?)?;
        need(KINDS.contains(&kind))?;
        let mut out = Fields(c::get(args, "args")?.as_object().ok_or_else(c::bad)?.to_vec());
        let st = self.assignment(&b);
        let m = self.mind(&st.mind)?;
        let has = |ch: &str| m.affect.contains(&ch);
        let mut reasons: Vec<&str> = Vec::new();
        let mut stripped: Vec<String> = Vec::new();
        let mut target = b.clone();
        match kind {
            "cueing" => {
                if MEMORY_OPS.contains(&op) && m.memory == "none" {
                    reasons.push("no_memory");
                }
                if op == "cover" {
                    if m.deception != "full" {
                        reasons.push("cannot_lie");
                    }
                    if m.theory_of_mind < 2 {
                        reasons.push("no_theory_of_mind");
                    }
                }
                if m.memory == "shared" && out.has("being") {
                    let pool = st.pool.clone().unwrap_or_default();
                    out.set("being", Value::from(pool.as_str()));
                    target = pool;
                }
                if op == "episode" {
                    if let Some(Value::String(e)) = out.get("emotion") {
                        if !has(e) {
                            out.remove("emotion");
                            stripped.push("emotion".into());
                        }
                    }
                }
                if op == "cue" {
                    let cd = match opt_field(&out, "cooldown") {
                        None => Some(24),
                        Some(v) => ticks(Some(v), 0),
                    };
                    if let Some(cd) = cd {
                        out.set("cooldown", Value::from(scale(cd, m.time_scale)));
                    }
                }
            }
            "pragmatics" => {
                if SPEECH_OPS.contains(&op) && m.language == "none" {
                    reasons.push("no_language");
                }
                if WORD_OPS.contains(&op) && m.language == "signals" {
                    reasons.push("signals_only");
                }
                if op == "resume" {
                    if let Some(tph) = ticks(out.get("ticks_per_hour"), 1) {
                        out.set("ticks_per_hour", Value::from(scale(tph, m.time_scale)));
                    }
                }
                if op == "stance" && m.theory_of_mind == 0 && opt_field(&out, "bystanders").is_some() {
                    out.remove("bystanders");
                    stripped.push("bystanders".into());
                }
            }
            _ => {
                if op == "voice" && m.language == "none" {
                    reasons.push("no_language");
                }
                let lie = (op == "react" && out.get("lying") == Some(&Value::Bool(true)))
                    || ((op == "tension" || op == "voice") && out.get("deceiving") == Some(&Value::Bool(true)));
                if m.deception == "none" && lie {
                    reasons.push("cannot_lie");
                }
                let channel_args: &[&str] = match op {
                    "react" => &REACT_ARGS,
                    "voice" => &VOICE_ARGS,
                    _ => &[],
                };
                for arg in channel_args {
                    if !has(arg) && out.remove(arg) {
                        stripped.push(arg.to_string());
                    }
                }
                if op == "react" && m.theory_of_mind == 0 && out.remove("bystander_untrusted") {
                    stripped.push("bystander_untrusted".into());
                }
                if op == "tension" {
                    if let Some(Value::Array(rows)) = out.get("attitudes").cloned() {
                        let mut kept = Vec::with_capacity(rows.len());
                        for x in rows {
                            match x {
                                Value::Object(f) => {
                                    let mut f = Fields(f);
                                    for arg in ATTITUDE_ARGS {
                                        if !has(arg) && f.remove(arg) {
                                            let name = format!("attitudes.{arg}");
                                            if !stripped.contains(&name) {
                                                stripped.push(name);
                                            }
                                        }
                                    }
                                    kept.push(Value::Object(f.0));
                                }
                                other => kept.push(other),
                            }
                        }
                        out.set("attitudes", Value::Array(kept));
                    }
                }
            }
        }
        reasons.sort();
        stripped.sort();
        let allowed = reasons.is_empty();
        Ok(obj! {
            "allowed" => allowed,
            "args" => if allowed { Value::Object(out.0) } else { Value::Null },
            "being" => b,
            "kind" => kind,
            "mind" => st.mind,
            "op" => op,
            "reasons" => reasons,
            "stripped" => stripped,
            "target" => target,
        })
    }

    fn op_fixate(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "motives"], &[])?;
        let b = id_of(args, "being")?;
        let rows = c::get(args, "motives")?.as_array().ok_or_else(c::bad)?;
        need(rows.len() <= 16)?;
        let mut ms: Vec<(String, u64)> = Vec::new();
        for x in rows {
            keys(x, &["id", "score"], &[])?;
            let id = id_of(x, "id")?;
            need(ms.iter().all(|m| m.0 != id))?;
            ms.push((id, uint_bound(c::get(x, "score")?, 0, 100)?));
        }
        let st = self.assignment(&b);
        let fix = self.mind(&st.mind)?.fixation;
        let view = |xs: &[(String, u64)]| {
            Value::Array(xs.iter().map(|(i, s)| obj! { "id" => i.as_str(), "score" => *s }).collect())
        };
        match fix.filter(|_| !st.released) {
            None => Ok(obj! { "being" => b, "fixated" => false, "motives" => view(&ms), "pinned" => Value::Null }),
            Some((goal, strength)) => {
                let top = ms.iter().map(|m| m.1).max().unwrap_or(0);
                let mut order = vec![(goal.clone(), strength.max(top))];
                order.extend(ms.into_iter().filter(|m| m.0 != goal));
                Ok(obj! { "being" => b, "fixated" => true, "motives" => view(&order), "pinned" => goal })
            }
        }
    }

    fn op_release(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        let st = self.beings.get(&b).ok_or_else(c::bad)?;
        need(self.mind(&st.mind)?.fixation.is_some())?;
        if let Some(st) = self.beings.get_mut(&b) {
            st.released = true;
        }
        Ok(obj! { "being" => b, "released" => true })
    }

    fn op_hear(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "acts"], &[])?;
        let b = id_of(args, "being")?;
        let rows = c::get(args, "acts")?.as_array().ok_or_else(c::bad)?;
        need((1..=4).contains(&rows.len()))?;
        let mut acts = Vec::new();
        for x in rows {
            ident(x)?;
            acts.push(pick(x, &ACTS)?);
        }
        let lang = self.mind(&self.assignment(&b).mind)?.language;
        let tones: Vec<&str> = acts.iter().map(|a| tone(a)).collect();
        Ok(match lang {
            "full" => obj! { "acts" => acts, "being" => b, "tones" => tones, "understood" => "words" },
            "signals" => obj! { "acts" => Vec::<&str>::new(), "being" => b, "tones" => tones, "understood" => "tone" },
            _ => obj! {
                "acts" => Vec::<&str>::new(), "being" => b, "tones" => Vec::<&str>::new(), "understood" => "nothing",
            },
        })
    }
}

/// A field that is present and not null.
fn opt_field<'a>(f: &'a Fields, k: &str) -> Option<&'a Value> {
    f.get(k).filter(|v| **v != Value::Null)
}

/// Runs one request on a fresh [`Minds`] (local kind `minds`).
pub fn workflow(request: &Value) -> Result<Value> {
    Minds::new().workflow(request)
}
