//! Living culture: community idioms and rumor mutation (pass 4, stage 4).
//!
//! Members have a community, a status, openness, carefulness, credulity and plausible links (beings
//! a rumor about them could be shifted to). Idioms belong to a community and have a prestige.
//! Hearing an idiom from a speaker who uses it (its community, or a strength at or above the use
//! threshold) raises the listener's strength by `status × openness × (100 + prestige) ÷ 20`
//! millionths; strength fades by 1,000 per tick since it was last raised. Idioms at or above
//! 500,000 are usable in speech, so a newcomer starts sounding local after enough contact and a
//! fad fades when nobody repeats it.
//!
//! A witness seeds a rumor: the host's truth, which gossip never rewrites, and the witness's own
//! version. Telling copies the speaker's version to the listener, perhaps mutated: a seeded FNV-1a
//! hash of `"<seed>:<speaker>:<topic>:<tick>"` decides whether to mutate (chance per mille
//! `(100 − carefulness) × credulity ÷ 10`) and which applicable operator to apply, in the fixed
//! order drop_detail, exaggerate (at most +50% per hop), shift_attribution (only to a linked
//! being), merge (one of the first 16 other topics the speaker holds, in id order), soften (only when the speaker is fond of the
//! agent). Every version is a belief with a truth distance (operators applied, plus numeric drift
//! from the truth in millionths), a hop count and the last eight links of its provenance chain. A
//! listener who already holds a version keeps it unless it trusts the teller (trust ≥ 50), so a
//! trusted witness retelling the original is a correction.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend); replays are byte-identical.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::knowledge::culture::Culture;
//!
//! let mut k = Culture::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "member", "args": {"being": "fay", "community": "hill", "status": 60, "openness": 30, "carefulness": 80, "credulity": 30}},
//!   {"op": "member", "args": {"being": "bo", "community": "dock", "status": 50, "openness": 60, "carefulness": 10, "credulity": 90}},
//!   {"op": "member", "args": {"being": "cy", "community": "dock", "status": 30, "openness": 70, "carefulness": 5, "credulity": 95}},
//!   {"op": "seed", "args": {"topic": "missing_coins", "witness": "fay", "verb": "stole", "agent": "ada", "amount": 3}},
//!   {"op": "tell", "args": {"speaker": "fay", "listener": "bo", "topic": "missing_coins", "seed": 7}},
//!   {"op": "tell", "args": {"speaker": "bo", "listener": "cy", "topic": "missing_coins", "seed": 7}},
//!   {"op": "spread", "args": {"topic": "missing_coins"}}
//! ]}"#, 4096)?;
//! let out = k.workflow(&req)?.to_json();
//! // Two hops later the three coins are four; the truth still says three.
//! assert!(out.contains(r#""mutated":"exaggerate""#));
//! assert!(out.contains(r#""max_drift":333333"#));
//! assert!(out.contains(r#""truth":{"agent":"ada","amount":3"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::attention::fnv1a;
use crate::character::beliefs::{opt, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "idioms", default: 64, ceiling: 100_000 },
    Limit { name: "repertoire", default: 32, ceiling: 1024 },
    Limit { name: "topics", default: 256, ceiling: 1_000_000 },
    Limit { name: "beliefs", default: 4096, ceiling: 1_000_000 },
];
pub const FULL: u64 = 1_000_000;
/// Strength at which an idiom is usable in speech.
pub const USE: u64 = 500_000;
/// Strength lost per tick since an idiom was last heard.
pub const FADE: u64 = 1_000;
pub const MAX_AMOUNT: u64 = 1_000_000_000_000;
/// Provenance links kept per version.
pub const CHAIN: usize = 8;
/// Other held topics (in id order) a merge may draw from, so a hop is bounded work.
pub const MERGE_POOL: usize = 16;
pub const OPERATORS: [&str; 5] = ["drop_detail", "exaggerate", "shift_attribution", "merge", "soften"];

/// The softer verb a fond teller uses.
pub fn softer(verb: &str) -> Option<&'static str> {
    Some(match verb {
        "stole" => "borrowed",
        "struck" => "pushed",
        "lied" => "misspoke",
        "broke" => "damaged",
        "threatened" => "warned",
        "cheated" => "miscounted",
        _ => return None,
    })
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

fn opt_id(args: &Value, k: &str) -> Result<Option<String>> {
    opt(args, k).map(|v| ident(v).map(str::to_string)).transpose()
}

fn id_list(v: Option<&Value>, max: usize) -> Result<Vec<String>> {
    let Some(v) = v else { return Ok(Vec::new()) };
    let rows = v.as_array().ok_or_else(c::bad)?;
    need(rows.len() <= max)?;
    let out = rows.iter().map(|x| ident(x).map(str::to_string)).collect::<Result<Vec<_>>>()?;
    need(out.iter().collect::<BTreeSet<_>>().len() == out.len())?;
    Ok(out)
}

/// An idiom strength and the tick it was last raised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exposure {
    pub strength: u64,
    pub at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub community: String,
    pub status: u64,
    pub openness: u64,
    pub carefulness: u64,
    pub credulity: u64,
    pub links: Vec<String>,
    pub repertoire: BTreeMap<String, Exposure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Idiom {
    pub community: String,
    pub phrase: String,
    pub meaning: String,
    pub prestige: u64,
}

/// The host's truth about a topic: never rewritten by gossip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Truth {
    pub agent: String,
    pub amount: Option<u64>,
    pub details: Vec<String>,
    pub place: Option<String>,
    pub verb: String,
    pub witness: String,
}

/// One being's version of a topic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub agent: String,
    pub amount: Option<u64>,
    pub chain: Vec<String>,
    pub details: Vec<String>,
    pub hops: u64,
    pub last: Option<&'static str>,
    pub merged: Vec<String>,
    pub ops: u64,
    pub place: Option<String>,
    pub verb: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Culture {
    pub members: BTreeMap<String, Member>,
    pub idioms: BTreeMap<String, Idiom>,
    pub truths: BTreeMap<String, Truth>,
    /// Being → topic → version.
    pub beliefs: BTreeMap<String, BTreeMap<String, Version>>,
    /// Topic → beings holding a version.
    pub holders: BTreeMap<String, BTreeSet<String>>,
    pub count: usize,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Culture {
    fn default() -> Self {
        Culture {
            members: BTreeMap::new(),
            idioms: BTreeMap::new(),
            truths: BTreeMap::new(),
            beliefs: BTreeMap::new(),
            holders: BTreeMap::new(),
            count: 0,
            clock: 0,
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Culture {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut k = Self::new();
        k.limits.apply(v)?;
        Ok(k)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "tick" => self.op_tick(args),
            "member" => self.op_member(args, l),
            "idiom" => self.op_idiom(args, l),
            "expose" => self.op_expose(args, l),
            "speech" => self.op_speech(args),
            "seed" => self.op_seed(args, l),
            "tell" => self.op_tell(args, l),
            "belief" => self.op_belief(args),
            "spread" => self.op_spread(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn member(&self, b: &str) -> Result<&Member> {
        self.members.get(b).ok_or_else(c::bad)
    }

    /// An idiom strength now, after fading.
    pub fn strength(&self, e: &Exposure) -> u64 {
        e.strength.saturating_sub(FADE.saturating_mul(self.clock - e.at))
    }

    /// Numeric drift of an amount from the truth, in millionths.
    pub fn drift(&self, topic: &str, amount: Option<u64>) -> u64 {
        match (self.truths.get(topic).and_then(|t| t.amount), amount) {
            (Some(t), Some(a)) => a.abs_diff(t) * FULL / t,
            _ => 0,
        }
    }

    fn view(&self, being: &str, topic: &str, v: &Version) -> Value {
        obj! {
            "agent" => v.agent.as_str(),
            "amount" => v.amount,
            "being" => being,
            "chain" => v.chain.clone(),
            "details" => v.details.clone(),
            "distance" => obj! { "drift" => self.drift(topic, v.amount), "ops" => v.ops },
            "hearsay" => v.hops > 0,
            "hops" => v.hops,
            "last_mutation" => v.last,
            "merged" => v.merged.clone(),
            "place" => v.place.clone(),
            "topic" => topic,
            "verb" => v.verb.as_str(),
        }
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn op_member(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "community", "status", "openness", "carefulness", "credulity"], &["links"])?;
        let b = id_of(args, "being")?;
        let community = id_of(args, "community")?;
        let mut p = [0u64; 4];
        for (i, k) in ["status", "openness", "carefulness", "credulity"].iter().enumerate() {
            p[i] = uint_bound(c::get(args, k)?, 0, 100)?;
        }
        let mut links = id_list(opt(args, "links"), 8)?;
        need(!links.contains(&b))?;
        need(self.members.contains_key(&b) || self.members.len() < l.get("beings"))?;
        links.sort();
        let repertoire = self.members.get(&b).map(|m| m.repertoire.clone()).unwrap_or_default();
        let m = Member {
            community: community.clone(),
            status: p[0],
            openness: p[1],
            carefulness: p[2],
            credulity: p[3],
            links: links.clone(),
            repertoire,
        };
        self.members.insert(b.clone(), m);
        Ok(obj! {
            "being" => b, "carefulness" => p[2], "community" => community, "credulity" => p[3], "links" => links,
            "openness" => p[1], "status" => p[0],
        })
    }

    fn op_idiom(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "community", "phrase", "meaning", "prestige"], &[])?;
        let id = id_of(args, "id")?;
        let community = id_of(args, "community")?;
        let phrase = text_bound(c::get(args, "phrase")?, 60)?.to_string();
        let meaning = text_bound(c::get(args, "meaning")?, 80)?.to_string();
        let prestige = uint_bound(c::get(args, "prestige")?, 0, 100)?;
        need(self.idioms.contains_key(&id) || self.idioms.len() < l.get("idioms"))?;
        self.idioms.insert(id.clone(), Idiom { community: community.clone(), phrase, meaning, prestige });
        Ok(obj! { "community" => community, "id" => id, "prestige" => prestige })
    }

    fn op_expose(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["listener", "speaker", "idiom"], &[])?;
        let lis = id_of(args, "listener")?;
        let spk = id_of(args, "speaker")?;
        let iid = id_of(args, "idiom")?;
        need(lis != spk)?;
        let idm = self.idioms.get(&iid).ok_or_else(c::bad)?;
        let lm = self.member(&lis)?;
        let sm = self.member(&spk)?;
        let cur = lm.repertoire.get(&iid).map_or(0, |e| self.strength(e));
        let uses = sm.community == idm.community || sm.repertoire.get(&iid).is_some_and(|e| self.strength(e) >= USE);
        if !uses {
            return Ok(obj! {
                "adopted" => cur >= USE, "gain" => 0u64, "heard" => false, "idiom" => iid, "listener" => lis,
                "strength" => cur,
            });
        }
        let gain = sm.status * lm.openness * (100 + idm.prestige) / 20;
        let new = (cur + gain).min(FULL);
        let mut live: BTreeMap<String, Exposure> = lm
            .repertoire
            .iter()
            .filter(|(k, e)| **k == iid || self.strength(e) > 0)
            .map(|(k, e)| (k.clone(), e.clone()))
            .collect();
        need(live.contains_key(&iid) || live.len() < l.get("repertoire"))?;
        live.insert(iid.clone(), Exposure { strength: new, at: self.clock });
        if let Some(m) = self.members.get_mut(&lis) {
            m.repertoire = live;
        }
        Ok(obj! {
            "adopted" => new >= USE, "gain" => gain, "heard" => true, "idiom" => iid, "listener" => lis,
            "strength" => new,
        })
    }

    fn op_speech(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        let m = self.member(&b)?;
        let mut rows: Vec<(u64, &String)> = m
            .repertoire
            .iter()
            .filter(|(k, _)| self.idioms.contains_key(*k))
            .map(|(k, e)| (self.strength(e), k))
            .collect();
        rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        let usable: Vec<Value> = rows
            .iter()
            .filter(|r| r.0 >= USE)
            .map(|(s, k)| obj! { "id" => k.as_str(), "phrase" => self.idioms[*k].phrase.as_str(), "strength" => *s })
            .collect();
        let fading: Vec<Value> = rows
            .iter()
            .filter(|r| r.0 > 0 && r.0 < USE)
            .map(|(s, k)| obj! { "id" => k.as_str(), "strength" => *s })
            .collect();
        Ok(obj! { "being" => b, "fading" => fading, "usable" => usable })
    }

    fn op_seed(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["topic", "witness", "verb", "agent"], &["place", "details", "amount"])?;
        let topic = id_of(args, "topic")?;
        let w = id_of(args, "witness")?;
        let verb = id_of(args, "verb")?;
        let agent = id_of(args, "agent")?;
        let place = opt_id(args, "place")?;
        let details = id_list(opt(args, "details"), 4)?;
        let amount = opt(args, "amount").map(|v| uint_bound(v, 1, 1_000_000_000)).transpose()?;
        need(!self.truths.contains_key(&topic) && self.truths.len() < l.get("topics"))?;
        self.member(&w)?;
        need(self.count < l.get("beliefs"))?;
        self.truths.insert(
            topic.clone(),
            Truth {
                agent: agent.clone(),
                amount,
                details: details.clone(),
                place: place.clone(),
                verb: verb.clone(),
                witness: w.clone(),
            },
        );
        let v = Version {
            agent,
            amount,
            chain: vec![w.clone()],
            details,
            hops: 0,
            last: None,
            merged: Vec::new(),
            ops: 0,
            place,
            verb,
        };
        let out = self.view(&w, &topic, &v);
        self.beliefs.entry(w.clone()).or_default().insert(topic.clone(), v);
        self.holders.insert(topic, BTreeSet::from([w]));
        self.count += 1;
        Ok(out)
    }

    fn op_tell(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["speaker", "listener", "topic"], &["seed", "fondness", "trust"])?;
        let spk = id_of(args, "speaker")?;
        let lis = id_of(args, "listener")?;
        let topic = id_of(args, "topic")?;
        let seed = opt_uint(args, "seed", 0, MAX_AT, 0)?;
        let fond = opt_uint(args, "fondness", 0, 100, 0)?;
        let trust = opt_uint(args, "trust", 0, 100, 50)?;
        need(spk != lis)?;
        let sm = self.member(&spk)?;
        let lm = self.member(&lis)?;
        let held = self.beliefs.get(&spk).ok_or_else(c::bad)?;
        let src = held.get(&topic).ok_or_else(c::bad)?;
        let old = self.beliefs.get(&lis).and_then(|t| t.get(&topic));
        let before = old.map(|o| obj! { "drift" => self.drift(&topic, o.amount), "ops" => o.ops });
        if let Some(o) = old.filter(|_| trust < 50) {
            return Ok(obj! {
                "before" => before, "belief" => self.view(&lis, &topic, o), "kept" => true, "mutated" => Value::Null,
            });
        }
        need(old.is_some() || self.count < l.get("beliefs"))?;
        let mut chain = src.chain.clone();
        chain.push(lis.clone());
        if chain.len() > CHAIN {
            chain.drain(..chain.len() - CHAIN);
        }
        let mut v = Version { chain, hops: src.hops + 1, ..src.clone() };
        let h = fnv1a(&format!("{seed}:{spk}:{topic}:{}", self.clock));
        let chance = (100 - sm.carefulness) * lm.credulity / 10;
        let mut mutated: Option<&'static str> = None;
        if h % 1000 < chance {
            let links: &[String] = self.members.get(&v.agent).map_or(&[], |m| &m.links);
            let others: Vec<&String> =
                held.keys().filter(|t| **t != topic && !v.merged.contains(t)).take(MERGE_POOL).collect();
            let mut cand: Vec<&'static str> = Vec::new();
            if !v.details.is_empty() {
                cand.push("drop_detail");
            }
            if v.amount.is_some() {
                cand.push("exaggerate");
            }
            if !links.is_empty() {
                cand.push("shift_attribution");
            }
            if !others.is_empty() && v.merged.len() < 4 {
                cand.push("merge");
            }
            if softer(&v.verb).is_some() && fond >= 60 {
                cand.push("soften");
            }
            if !cand.is_empty() {
                let op = cand[((h >> 10) % cand.len() as u64) as usize];
                let k = h >> 20;
                match op {
                    "drop_detail" => {
                        let i = (k % v.details.len() as u64) as usize;
                        v.details.remove(i);
                    }
                    "exaggerate" => {
                        let a = v.amount.unwrap_or(1);
                        let pct = 1 + k % 50;
                        v.amount = Some((a + (a * pct / 100).max(1)).min(MAX_AMOUNT));
                    }
                    "shift_attribution" => v.agent = links[(k % links.len() as u64) as usize].clone(),
                    "merge" => {
                        v.merged.push(others[(k % others.len() as u64) as usize].clone());
                        v.merged.sort();
                    }
                    _ => v.verb = softer(&v.verb).unwrap_or("borrowed").to_string(),
                }
                v.ops += 1;
                v.last = Some(op);
                mutated = Some(op);
            }
        }
        if old.is_none() {
            self.count += 1;
        }
        let out = obj! {
            "before" => before, "belief" => self.view(&lis, &topic, &v), "kept" => false, "mutated" => mutated,
        };
        self.beliefs.entry(lis.clone()).or_default().insert(topic.clone(), v);
        self.holders.entry(topic).or_default().insert(lis);
        Ok(out)
    }

    fn op_belief(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "topic"], &[])?;
        let b = id_of(args, "being")?;
        let topic = id_of(args, "topic")?;
        let v = self.beliefs.get(&b).and_then(|t| t.get(&topic)).ok_or_else(c::bad)?;
        Ok(self.view(&b, &topic, v))
    }

    fn op_spread(&self, args: &Value) -> Result<Value> {
        keys(args, &["topic"], &[])?;
        let topic = id_of(args, "topic")?;
        let t = self.truths.get(&topic).ok_or_else(c::bad)?;
        let vs: Vec<&Version> = self.holders[&topic].iter().filter_map(|b| self.beliefs.get(b)?.get(&topic)).collect();
        Ok(obj! {
            "holders" => vs.len() as u64,
            "max_drift" => vs.iter().map(|v| self.drift(&topic, v.amount)).max().unwrap_or(0),
            "max_ops" => vs.iter().map(|v| v.ops).max().unwrap_or(0),
            "topic" => topic.as_str(),
            "total_ops" => vs.iter().map(|v| v.ops).sum::<u64>(),
            "truth" => obj! {
                "agent" => t.agent.as_str(), "amount" => t.amount, "details" => t.details.clone(),
                "place" => t.place.clone(), "verb" => t.verb.as_str(), "witness" => t.witness.as_str(),
            },
        })
    }
}

/// Runs one request on a fresh [`Culture`] (local kind `culture`).
pub fn workflow(request: &Value) -> Result<Value> {
    Culture::new().workflow(request)
}
