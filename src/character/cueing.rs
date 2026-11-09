//! Associative memory cueing and the epistemic gate (pass 3, stage 2).
//!
//! **Cueing.** A being keeps episodes with structural features (actions, entities, tags,
//! places), a valence and an optional emotion. `cue` matches the current situation against them
//! by weighted feature overlap (shared actions 3, entities 2, tags 1, places 1). A match tied to
//! fear, shame or a secret goes to `avoid` (deflect, do not disclose); otherwise it is a
//! `recall` candidate, unless it was surfaced to this listener within the cooldown (`cooling`),
//! so the character does not bring up the same past event on consecutive turns. Only `surface`
//! (the host confirming a line was delivered) records that it came up.
//!
//! **Epistemic gate.** Every fact a being holds carries how it knows it (`direct_witness`,
//! `trusted_report`, `unverified_rumor`, `inference`, `corrected_belief`). `frame_facts`
//! returns each fact with the framing a line must use (state, attribute, hedge, guess,
//! correct). An active `cover` story masks a true fact for some listeners: the framing becomes
//! `cover` with the lie and the true claim is withheld. `gate` checks a proposed reply word by
//! word: a key word of a masked fact (other than words of the cover story itself), or of a
//! host-only `secret` the being does not hold, is a leak, and the reply is refused with a
//! deterministic fallback (the cover story, else the host's fallback line).
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend); model replies are untrusted input to `gate`.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::cueing::Cueing;
//!
//! let mut c = Cueing::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "fact", "args": {"being": "mira", "id": "theft", "claim": "The apprentice took the pouch",
//!     "source": "inference", "keys": ["took", "stole"]}},
//!   {"op": "cover", "args": {"being": "mira", "id": "shield", "fact": "theft",
//!     "lie": "The pouch is safe in the back", "listeners": ["gossip"]}},
//!   {"op": "gate", "args": {"being": "mira", "listener": "gossip", "reply": "I think the boy TOOK it."}}
//! ]}"#, 4096)?;
//! let out = c.workflow(&req)?.to_json();
//! // The reply would reveal the masked fact to the gossip, so it is refused for the cover story.
//! assert!(out.contains(r#""fallback":"The pouch is safe in the back","leaks":[{"id":"theft","kind":"cover","word":"took"}]"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_bool, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "episodes", default: 64, ceiling: 4096 },
    Limit { name: "facts", default: 64, ceiling: 4096 },
    Limit { name: "covers", default: 16, ceiling: 256 },
    Limit { name: "secrets", default: 64, ceiling: 4096 },
    Limit { name: "listeners", default: 8, ceiling: 64 },
];
/// Feature kinds and their overlap weights.
pub const FEATURES: [(&str, u64); 4] = [("actions", 3), ("entities", 2), ("tags", 1), ("places", 1)];
/// Emotions that turn a cued episode into something to avoid.
pub const AVOID: [&str; 2] = ["fear", "shame"];
pub const SOURCES: [(&str, &str); 5] = [
    ("corrected_belief", "correct"),
    ("direct_witness", "state"),
    ("inference", "guess"),
    ("trusted_report", "attribute"),
    ("unverified_rumor", "hedge"),
];
pub const MAX_LIST: usize = 16;
pub const MAX_KEYS: usize = 8;
pub const DEFAULT_FALLBACK: &str = "I would rather not say.";

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

fn opt_id(args: &Value, k: &str) -> Result<Option<String>> {
    opt(args, k).map(|v| ident(v).map(str::to_string)).transpose()
}

fn opt_str(x: &Option<String>) -> Value {
    x.as_deref().map(Value::from).unwrap_or(Value::Null)
}

fn strs<'a>(xs: impl IntoIterator<Item = &'a String>) -> Value {
    Value::Array(xs.into_iter().map(|x| Value::from(x.as_str())).collect())
}

fn idset(args: &Value, k: &str) -> Result<BTreeSet<String>> {
    let rows = list_bound(opt(args, k), MAX_LIST)?;
    let set = rows.iter().map(|v| ident(v).map(str::to_string)).collect::<Result<BTreeSet<_>>>()?;
    need(set.len() == rows.len())?;
    Ok(set)
}

type Features = [BTreeSet<String>; 4];

fn features(v: Option<&Value>) -> Result<Features> {
    let none = Value::Object(Vec::new());
    let v = v.unwrap_or(&none);
    keys(v, &[], &["actions", "entities", "tags", "places"])?;
    Ok([idset(v, "actions")?, idset(v, "entities")?, idset(v, "tags")?, idset(v, "places")?])
}

fn key_words(args: &Value) -> Result<BTreeSet<String>> {
    let rows = list_bound(opt(args, "keys"), MAX_KEYS)?;
    need(!rows.is_empty())?;
    let mut out = BTreeSet::new();
    for w in rows {
        let w = w.as_str().ok_or_else(c::bad)?;
        need((1..=32).contains(&w.len()) && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))?;
        need(out.insert(w.to_string()))?;
    }
    Ok(out)
}

/// Lower-cased runs of ASCII letters and digits; everything else separates.
pub fn words(s: &str) -> BTreeSet<String> {
    s.split(|ch: char| !ch.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_ascii_lowercase()).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Episode {
    pub features: Features,
    pub valence: i64,
    pub emotion: Option<String>,
    pub secret: bool,
    pub surfaced: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub claim: String,
    pub source: String,
    pub from: Option<String>,
    pub previous: Option<String>,
    pub confidence: u64,
    pub keys: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cover {
    pub fact: String,
    pub lie: String,
    pub listeners: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Being {
    pub episodes: BTreeMap<String, Episode>,
    pub facts: BTreeMap<String, Fact>,
    pub covers: BTreeMap<String, Cover>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cueing {
    pub beings: BTreeMap<String, Being>,
    pub secrets: BTreeMap<String, BTreeSet<String>>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Cueing {
    fn default() -> Self {
        Cueing { beings: BTreeMap::new(), secrets: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

impl Being {
    /// Fact id → (cover id, lie) of the first active cover (by id) masking it for `listener`.
    fn masks(&self, listener: &str) -> BTreeMap<&str, (&str, &str)> {
        let mut out = BTreeMap::new();
        for (i, c) in &self.covers {
            if c.listeners.is_empty() || c.listeners.contains(listener) {
                out.entry(c.fact.as_str()).or_insert((i.as_str(), c.lie.as_str()));
            }
        }
        out
    }
}

impl Cueing {
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
            "tick" => self.op_tick(args),
            "episode" => self.op_episode(args, l),
            "cue" => self.op_cue(args),
            "surface" => self.op_surface(args, l),
            "fact" => self.op_fact(args, l),
            "cover" => self.op_cover(args, l),
            "uncover" => self.op_uncover(args),
            "secret" => self.op_secret(args, l),
            "frame_facts" => self.op_frame_facts(args),
            "gate" => self.op_gate(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn room(&self, b: &str, l: &Limits) -> Result<()> {
        need(self.beings.contains_key(b) || self.beings.len() < l.get("beings"))
    }

    fn known(&self, b: &str) -> Result<&Being> {
        self.beings.get(b).ok_or_else(c::bad)
    }

    // ------------------------------------------------------------ cueing

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn op_episode(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "id", "features"], &["valence", "emotion", "secret"])?;
        let b = id_of(args, "being")?;
        let i = id_of(args, "id")?;
        let f = features(Some(c::get(args, "features")?))?;
        need(f.iter().any(|s| !s.is_empty()))?;
        let valence = match opt(args, "valence") {
            None => 0,
            Some(Value::Number(s)) => s.parse::<i64>().ok().filter(|n| (-100..=100).contains(n)).ok_or_else(c::bad)?,
            Some(_) => return Err(c::bad()),
        };
        let emotion = opt_id(args, "emotion")?;
        let secret = opt_bool(args, "secret", false)?;
        self.room(&b, l)?;
        if let Some(x) = self.beings.get(&b) {
            need(x.episodes.contains_key(&i) || x.episodes.len() < l.get("episodes"))?;
        }
        let e = Episode { features: f, valence, emotion: emotion.clone(), secret, surfaced: BTreeMap::new() };
        self.beings.entry(b.clone()).or_default().episodes.insert(i.clone(), e);
        Ok(obj! { "being" => b, "emotion" => opt_str(&emotion), "id" => i, "secret" => secret, "valence" => valence })
    }

    fn op_cue(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "listener", "situation"], &["cooldown", "min_score", "max"])?;
        let b = id_of(args, "being")?;
        let lis = id_of(args, "listener")?;
        let sit = features(Some(c::get(args, "situation")?))?;
        let cooldown = opt_uint(args, "cooldown", 0, MAX_AT, 24)?;
        let min_score = opt_uint(args, "min_score", 1, 1000, 2)?;
        let max = opt_uint(args, "max", 1, 16, 2)? as usize;
        let x = self.known(&b)?;
        struct Found<'a> {
            id: &'a str,
            e: &'a Episode,
            score: u64,
            shared: Value,
            mode: &'static str,
            last: Option<u64>,
        }
        let mut found: Vec<Found> = Vec::new();
        for (i, e) in &x.episodes {
            let mut score = 0;
            let mut shared = Vec::new();
            for (j, (k, w)) in FEATURES.iter().enumerate() {
                let common: Vec<&String> = e.features[j].intersection(&sit[j]).collect();
                score += w * common.len() as u64;
                shared.push((k.to_string(), strs(common)));
            }
            if score < min_score {
                continue;
            }
            let last = e.surfaced.get(&lis).copied();
            let mode = if e.secret || e.emotion.as_deref().is_some_and(|m| AVOID.contains(&m)) {
                "avoid"
            } else if last.is_some_and(|t| self.clock - t < cooldown) {
                "cooling"
            } else {
                "recall"
            };
            found.push(Found { id: i, e, score, shared: Value::Object(shared), mode, last });
        }
        found.sort_by(|a, b| {
            b.score.cmp(&a.score).then_with(|| b.e.valence.abs().cmp(&a.e.valence.abs())).then_with(|| a.id.cmp(b.id))
        });
        let view = |m: &Found| {
            obj! {
                "emotion" => opt_str(&m.e.emotion), "id" => m.id, "last_surfaced" => m.last, "mode" => m.mode,
                "score" => m.score, "shared" => m.shared.clone(), "valence" => m.e.valence,
            }
        };
        let pick = |mode: &str| found.iter().filter(|m| m.mode == mode).map(view).collect::<Vec<_>>();
        let mut recall = pick("recall");
        let unused = recall.len().saturating_sub(max);
        recall.truncate(max);
        Ok(obj! {
            "avoid" => Value::Array(pick("avoid")), "being" => b, "cooling" => Value::Array(pick("cooling")),
            "listener" => lis, "recall" => Value::Array(recall), "unused" => unused,
        })
    }

    fn op_surface(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "listener", "episode"], &[])?;
        let b = id_of(args, "being")?;
        let lis = id_of(args, "listener")?;
        let i = id_of(args, "episode")?;
        let clock = self.clock;
        let e = self.beings.get_mut(&b).and_then(|x| x.episodes.get_mut(&i)).ok_or_else(c::bad)?;
        if !e.surfaced.contains_key(&lis) && e.surfaced.len() >= l.get("listeners") {
            let old = e.surfaced.iter().min_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.cmp(b.0))).map(|(k, _)| k.clone());
            if let Some(old) = old {
                e.surfaced.remove(&old);
            }
        }
        e.surfaced.insert(lis.clone(), clock);
        Ok(obj! { "at" => clock, "being" => b, "episode" => i, "listener" => lis })
    }

    // ------------------------------------------------------------ epistemic gate

    fn op_fact(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "id", "claim", "source", "keys"], &["from", "previous", "confidence"])?;
        let b = id_of(args, "being")?;
        let i = id_of(args, "id")?;
        let claim = text_bound(c::get(args, "claim")?, 160)?.to_string();
        let src = ident(c::get(args, "source")?)?;
        let framing = SOURCES.iter().find(|(s, _)| *s == src).map(|(_, f)| *f).ok_or_else(c::bad)?;
        let from = opt_id(args, "from")?;
        need(from.is_some() == (src == "trusted_report" || src == "unverified_rumor"))?;
        let previous = opt(args, "previous").map(|v| text_bound(v, 160).map(str::to_string)).transpose()?;
        need(previous.is_some() == (src == "corrected_belief"))?;
        let confidence = opt_uint(args, "confidence", 0, 100, if src == "direct_witness" { 100 } else { 60 })?;
        let ks = key_words(args)?;
        self.room(&b, l)?;
        if let Some(x) = self.beings.get(&b) {
            need(x.facts.contains_key(&i) || x.facts.len() < l.get("facts"))?;
        }
        let f = Fact { claim, source: src.to_string(), from, previous, confidence, keys: ks };
        self.beings.entry(b.clone()).or_default().facts.insert(i.clone(), f);
        Ok(obj! { "being" => b, "framing" => framing, "id" => i, "source" => src })
    }

    fn op_cover(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "id", "fact", "lie"], &["listeners"])?;
        let b = id_of(args, "being")?;
        let i = id_of(args, "id")?;
        let f = id_of(args, "fact")?;
        let lie = text_bound(c::get(args, "lie")?, 160)?.to_string();
        let listeners = idset(args, "listeners")?;
        let x = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        need(x.facts.contains_key(&f))?;
        need(x.covers.contains_key(&i) || x.covers.len() < l.get("covers"))?;
        let out = obj! { "being" => b, "fact" => f.as_str(), "id" => i.as_str(), "listeners" => strs(&listeners) };
        x.covers.insert(i, Cover { fact: f, lie, listeners });
        Ok(out)
    }

    fn op_uncover(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "id"], &[])?;
        let b = id_of(args, "being")?;
        let i = id_of(args, "id")?;
        let x = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        need(x.covers.remove(&i).is_some())?;
        Ok(obj! { "being" => b, "id" => i, "removed" => true })
    }

    fn op_secret(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "keys"], &[])?;
        let i = id_of(args, "id")?;
        let ks = key_words(args)?;
        need(self.secrets.contains_key(&i) || self.secrets.len() < l.get("secrets"))?;
        let out = obj! { "id" => i.as_str(), "keys" => strs(&ks) };
        self.secrets.insert(i, ks);
        Ok(out)
    }

    fn op_frame_facts(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "listener", "facts"], &[])?;
        let b = id_of(args, "being")?;
        let lis = id_of(args, "listener")?;
        let ids = idset(args, "facts")?;
        let x = self.known(&b)?;
        need(ids.iter().all(|i| x.facts.contains_key(i)))?;
        let m = x.masks(&lis);
        let facts = ids
            .iter()
            .map(|i| {
                let f = &x.facts[i];
                match m.get(i.as_str()) {
                    Some((cover, lie)) => obj! {
                        "claim" => *lie, "confidence" => Value::Null, "cover" => *cover, "framing" => "cover",
                        "from" => Value::Null, "id" => i.as_str(), "previous" => Value::Null,
                    },
                    None => obj! {
                        "claim" => f.claim.as_str(), "confidence" => f.confidence, "cover" => Value::Null,
                        "framing" => SOURCES.iter().find(|(s, _)| *s == f.source).map(|(_, x)| *x).unwrap_or("state"),
                        "from" => opt_str(&f.from), "id" => i.as_str(), "previous" => opt_str(&f.previous),
                    },
                }
            })
            .collect();
        Ok(obj! { "being" => b, "facts" => Value::Array(facts), "listener" => lis })
    }

    fn op_gate(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "listener", "reply"], &["fallback"])?;
        let b = id_of(args, "being")?;
        let lis = id_of(args, "listener")?;
        let reply = text_bound(c::get(args, "reply")?, 2000)?;
        let fb = opt(args, "fallback").map(|v| text_bound(v, 160)).transpose()?;
        let x = self.known(&b)?;
        let said = words(reply);
        let m = x.masks(&lis);
        let mut leaks: Vec<(&str, &str, &str)> = Vec::new();
        for (f, (_, lie)) in &m {
            let told = words(lie);
            for w in &x.facts[*f].keys {
                if said.contains(w) && !told.contains(w) {
                    leaks.push((f, "cover", w));
                }
            }
        }
        for (s, ks) in &self.secrets {
            if x.facts.contains_key(s) {
                continue;
            }
            for w in ks {
                if said.contains(w) {
                    leaks.push((s, "unknown", w));
                }
            }
        }
        let cover = leaks.iter().find(|l| l.1 == "cover").map(|l| m[l.0].1);
        let fallback = if leaks.is_empty() { None } else { Some(cover.or(fb).unwrap_or(DEFAULT_FALLBACK)) };
        let leaks = leaks.iter().map(|(i, k, w)| obj! { "id" => *i, "kind" => *k, "word" => *w }).collect::<Vec<_>>();
        Ok(obj! {
            "being" => b, "fallback" => fallback, "leaks" => Value::Array(leaks.clone()), "listener" => lis,
            "ok" => leaks.is_empty(),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Cueing::new().workflow(req)
}
