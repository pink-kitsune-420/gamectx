//! Off-screen conversation between characters (pass 2, B6).
//!
//! Beings hold topics they **know** (a value, a weight 0–100, maybe secret) and topics they
//! **want** to learn. When two beings `meet`, FNV-1a of `"<seed>:<a>:<b>"` picks who speaks first
//! (even: `a`), and they take turns. Each turn the speaker offers the topic with the highest
//! `2·(listener's want) + weight` (then by name) that the listener does not already hold with the
//! same value and that was not said yet in this meeting; secrets only at warmth 60 or more.
//!
//! - A topic the listener holds with another value is a **dispute**.
//! - Otherwise it is a **tell**: the listener learns it at half weight (when it has room), and a
//!   wanted topic raises warmth by 5 and proposes relationship `help`.
//! - With nothing to offer the turn is **small talk**; two in a row end the meeting.
//!
//! The meeting is summarized as a record (`quarrel` when there were disputes and warmth was below
//! 20: warmth −10 per dispute and resentment proposals for both; `alliance` when a wanted topic was
//! shared and warmth reached 60; `gossip` when anything was told; else `small_talk`). Learned
//! topics also become beliefs `learn` proposals. No model call: this is where gossip, alliances and
//! quarrels come from. Capacities are [host-set limits](crate::character::limits). Pure host
//! computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::chatter::Chatter;
//!
//! let mut talk = Chatter::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "know", "args": {"being": "baker", "topic": "mill_price", "value": "rising", "weight": 80}},
//!   {"op": "want", "args": {"being": "widow", "topic": "mill_price"}},
//!   {"op": "meet", "args": {"a": "baker", "b": "widow", "seed": 3}}
//! ]}"#, 4096)?;
//! let out = talk.workflow(&req)?.to_json();
//! assert!(out.contains(r#""kind":"tell""#) && out.contains(r#""outcome":"gossip""#));
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
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "topics", default: 64, ceiling: 4096 },
    Limit { name: "wants", default: 16, ceiling: 256 },
    Limit { name: "bonds", default: 4096, ceiling: 1_000_000 },
    Limit { name: "records", default: 256, ceiling: 1_000_000 },
    Limit { name: "turns", default: 6, ceiling: 32 },
];
pub const SECRET_WARMTH: i64 = 60;
pub const WANT_WARMTH: i64 = 5;
pub const QUARREL_BELOW: i64 = 20;
pub const QUARREL_COST: i64 = 10;
pub const ALLY_AT: i64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub value: String,
    pub weight: u64,
    pub secret: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mind {
    pub knows: BTreeMap<String, Known>,
    pub wants: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub kind: &'static str,
    pub speaker: String,
    pub topic: Option<String>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub id: u64,
    pub a: String,
    pub b: String,
    pub at: u64,
    pub first: String,
    pub outcome: &'static str,
    pub turns: Vec<Turn>,
    pub warmth_before: i64,
    pub warmth_after: i64,
}

impl Record {
    fn view(&self) -> Value {
        let opt_s = |s: &Option<String>| s.as_deref().map(Value::from).unwrap_or(Value::Null);
        let turns = self
            .turns
            .iter()
            .map(|t| obj! { "kind" => t.kind, "speaker" => t.speaker.as_str(), "topic" => opt_s(&t.topic), "value" => opt_s(&t.value) })
            .collect();
        obj! {
            "id" => self.id, "a" => self.a.as_str(), "b" => self.b.as_str(), "at" => self.at,
            "first" => self.first.as_str(), "outcome" => self.outcome, "turns" => Value::Array(turns),
            "warmth_after" => self.warmth_after, "warmth_before" => self.warmth_before,
        }
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

fn sint(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => {
            let n = s.parse::<i64>().map_err(|_| c::bad())?;
            need(lo <= n && n <= hi)?;
            Ok(n)
        }
        _ => Err(c::bad()),
    }
}

fn pair(a: &str, b: &str) -> (String, String) {
    if a < b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// What beings know and want, how warm each pair is, and recent conversation records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chatter {
    pub minds: BTreeMap<String, Mind>,
    pub bonds: BTreeMap<(String, String), i64>,
    pub records: VecDeque<Record>,
    pub seq: u64,
    pub limits: Limits,
}

impl Default for Chatter {
    fn default() -> Self {
        Chatter {
            minds: BTreeMap::new(),
            bonds: BTreeMap::new(),
            records: VecDeque::new(),
            seq: 0,
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Chatter {
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
            "know" => self.op_know(args, l),
            "want" => self.op_want(args, l),
            "bond" => self.op_bond(args, l),
            "meet" => self.op_meet(args, l),
            "log" => self.op_log(args),
            "assess" => self.op_assess(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn mind_fields(&self, b: &str) -> Vec<(String, Value)> {
        let m = self.minds.get(b).cloned().unwrap_or_default();
        let knows = m
            .knows
            .iter()
            .map(|(t, k)| obj! { "secret" => k.secret, "topic" => t.as_str(), "value" => k.value.as_str(), "weight" => k.weight })
            .collect();
        let wants = m.wants.iter().map(|(t, w)| obj! { "topic" => t.as_str(), "weight" => *w }).collect();
        vec![
            ("being".to_string(), Value::from(b)),
            ("knows".to_string(), Value::Array(knows)),
            ("wants".to_string(), Value::Array(wants)),
        ]
    }

    fn op_know(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "topic", "value"], &["weight", "secret"])?;
        let b = id_of(args, "being")?;
        let t = id_of(args, "topic")?;
        let value = id_of(args, "value")?;
        let weight = opt_uint(args, "weight", 0, 100, 50)?;
        let secret = opt_bool(args, "secret", false)?;
        let cur = self.minds.get(&b);
        need(cur.is_some() || self.minds.len() < l.get("beings"))?;
        need(cur.is_some_and(|m| m.knows.contains_key(&t)) || cur.map_or(0, |m| m.knows.len()) < l.get("topics"))?;
        let m = self.minds.entry(b.clone()).or_default();
        m.knows.insert(t.clone(), Known { value: value.clone(), weight, secret });
        m.wants.remove(&t);
        Ok(obj! {
            "being" => b, "secret" => secret, "topic" => t, "topics" => m.knows.len(), "value" => value,
            "wants" => m.wants.len(), "weight" => weight,
        })
    }

    fn op_want(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "topic"], &["weight"])?;
        let b = id_of(args, "being")?;
        let t = id_of(args, "topic")?;
        let weight = opt_uint(args, "weight", 1, 100, 50)?;
        let cur = self.minds.get(&b);
        need(cur.is_some() || self.minds.len() < l.get("beings"))?;
        let (knows, has, n) =
            cur.map_or((false, false, 0), |m| (m.knows.contains_key(&t), m.wants.contains_key(&t), m.wants.len()));
        need(!knows && (has || n < l.get("wants")))?;
        let m = self.minds.entry(b.clone()).or_default();
        m.wants.insert(t.clone(), weight);
        Ok(obj! { "being" => b, "topic" => t, "topics" => m.knows.len(), "wants" => m.wants.len(), "weight" => weight })
    }

    fn op_bond(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["a", "b", "warmth"], &[])?;
        let x = id_of(args, "a")?;
        let y = id_of(args, "b")?;
        let w = sint(c::get(args, "warmth")?, -100, 100)?;
        let k = pair(&x, &y);
        need(x != y && (self.bonds.contains_key(&k) || self.bonds.len() < l.get("bonds")))?;
        self.bonds.insert(k.clone(), w);
        Ok(obj! { "a" => k.0, "b" => k.1, "warmth" => w })
    }

    /// The speaker's best shareable topic for the listener: `(topic, value, weight, secret)`.
    fn offer(&self, speaker: &str, listener: &str, warmth: i64, said: &BTreeSet<String>) -> Option<(String, Known)> {
        let (sk, lm) = (&self.minds.get(speaker)?.knows, self.minds.get(listener)?);
        let mut best: Option<(u64, &String, &Known)> = None;
        for (t, k) in sk {
            if said.contains(t) || (k.secret && warmth < SECRET_WARMTH) {
                continue;
            }
            if lm.knows.get(t).is_some_and(|x| x.value == k.value) {
                continue;
            }
            let score = lm.wants.get(t).copied().unwrap_or(0) * 2 + k.weight;
            // Topics iterate by name, so a strictly higher score wins and ties keep the first name.
            if best.is_none_or(|(s, _, _)| score > s) {
                best = Some((score, t, k));
            }
        }
        best.map(|(_, t, k)| (t.clone(), k.clone()))
    }

    fn op_meet(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["a", "b", "seed"], &["turns", "at"])?;
        let x = id_of(args, "a")?;
        let y = id_of(args, "b")?;
        let seed = uint_bound(c::get(args, "seed")?, 0, MAX_AT)?;
        let max_turns = l.get("turns") as u64;
        let turns = opt_uint(args, "turns", 1, max_turns, max_turns)?;
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        need(x != y && self.minds.contains_key(&x) && self.minds.contains_key(&y))?;
        let k = pair(&x, &y);
        let before = self.bonds.get(&k).copied().unwrap_or(0);
        let first = if fnv1a(&format!("{seed}:{x}:{y}")) % 2 == 0 { x.clone() } else { y.clone() };
        let second = if first == x { y.clone() } else { x.clone() };
        let mut warmth = before;
        let mut said = BTreeSet::new();
        let (mut log, mut effects) = (Vec::new(), Vec::new());
        let (mut disputes, mut shared_wants, mut quiet) = (0i64, 0u64, 0u64);
        let mut learned: Vec<(String, String, Known)> = Vec::new();
        let topics = l.get("topics");
        let mut room: BTreeMap<String, usize> =
            [&x, &y].iter().map(|b| ((*b).clone(), topics.saturating_sub(self.minds[*b].knows.len()))).collect();
        for i in 0..turns {
            let (sp, li) = if i % 2 == 0 { (&first, &second) } else { (&second, &first) };
            let Some((t, known)) = self.offer(sp, li, warmth, &said) else {
                log.push(Turn { kind: "small_talk", speaker: sp.clone(), topic: None, value: None });
                quiet += 1;
                if quiet >= 2 {
                    break;
                }
                continue;
            };
            quiet = 0;
            said.insert(t.clone());
            let lm = &self.minds[li];
            if lm.knows.contains_key(&t) {
                disputes += 1;
                log.push(Turn { kind: "dispute", speaker: sp.clone(), topic: Some(t), value: Some(known.value) });
                continue;
            }
            let wanted = lm.wants.contains_key(&t);
            let slot = room.get_mut(li).ok_or_else(c::bad)?;
            let took = *slot > 0;
            if took {
                *slot -= 1;
                learned.push((
                    li.clone(),
                    t.clone(),
                    Known { value: known.value.clone(), weight: known.weight / 2, secret: known.secret },
                ));
                effects.push(obj! {
                    "engine" => "beliefs", "op" => "learn",
                    "args" => obj! { "being" => li.as_str(), "source" => "told", "about" => t.as_str(), "relation" => "is", "value" => known.value.as_str(), "from" => sp.as_str() },
                });
            }
            if wanted && took {
                shared_wants += 1;
                warmth = (warmth + WANT_WARMTH).min(100);
                effects.push(obj! {
                    "engine" => "relationship", "op" => "appraise_action",
                    "args" => obj! { "actor" => sp.as_str(), "target" => li.as_str(), "action" => "help", "success" => true },
                });
            }
            log.push(Turn { kind: "tell", speaker: sp.clone(), topic: Some(t), value: Some(known.value) });
        }
        let outcome = if disputes > 0 && before < QUARREL_BELOW {
            warmth = (warmth - QUARREL_COST * disputes).max(-100);
            for (p, q) in [(&x, &y), (&y, &x)] {
                effects.push(obj! {
                    "engine" => "affect", "op" => "appraise",
                    "args" => obj! { "being" => p.as_str(), "subject" => q.as_str(), "kind" => "resentment", "delta" => (QUARREL_COST * disputes).min(100), "reason" => "quarrel" },
                });
            }
            "quarrel"
        } else if log.iter().any(|t| t.kind == "tell") {
            if shared_wants > 0 && warmth >= ALLY_AT {
                "alliance"
            } else {
                "gossip"
            }
        } else {
            "small_talk"
        };
        need(warmth == before || self.bonds.contains_key(&k) || self.bonds.len() < l.get("bonds"))?;
        for (li, t, known) in learned {
            let m = self.minds.get_mut(&li).ok_or_else(c::bad)?;
            m.wants.remove(&t);
            m.knows.insert(t, known);
        }
        if warmth != before {
            self.bonds.insert(k, warmth);
        }
        self.seq += 1;
        let rec = Record {
            id: self.seq,
            a: x,
            b: y,
            at,
            first,
            outcome,
            turns: log,
            warmth_before: before,
            warmth_after: warmth,
        };
        let view = rec.view();
        self.records.push_back(rec);
        while self.records.len() > l.get("records") {
            self.records.pop_front();
        }
        Ok(obj! { "effects" => Value::Array(effects), "record" => view })
    }

    fn op_log(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &["being", "limit"])?;
        let b = opt(args, "being").map(ident).transpose()?;
        let n = opt_uint(args, "limit", 1, 64, 16)? as usize;
        let rows: Vec<Value> = self
            .records
            .iter()
            .rev()
            .filter(|r| b.is_none_or(|b| r.a == b || r.b == b))
            .take(n)
            .map(Record::view)
            .collect();
        Ok(obj! { "records" => Value::Array(rows) })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        need(self.minds.contains_key(&b))?;
        let bonds = self
            .bonds
            .iter()
            .filter(|((p, q), _)| *p == b || *q == b)
            .map(|((p, q), w)| obj! { "other" => if *p == b { q.as_str() } else { p.as_str() }, "warmth" => *w })
            .collect();
        let mut fields = self.mind_fields(&b);
        fields.push(("bonds".to_string(), Value::Array(bonds)));
        Ok(Value::Object(fields))
    }
}

/// What a turn of a meeting was.
pub const TURN_KINDS: [&str; 3] = ["dispute", "small_talk", "tell"];
/// How a meeting ended.
pub const OUTCOMES: [&str; 4] = ["alliance", "gossip", "quarrel", "small_talk"];

persist!(Known { secret, value, weight [0, 100] });
persist!(Mind { knows, wants });
persist!(Turn { kind in TURN_KINDS, speaker, topic, value });
persist!(Record {
    a,
    at [0, MAX_AT],
    b,
    first,
    id [1, MAX_AT],
    outcome in OUTCOMES,
    turns,
    warmth_after [-100, 100],
    warmth_before [-100, 100]
});
persist!(rest Chatter { bonds, minds, records, seq [0, MAX_AT] });
durable!(Chatter, "chatter", LIMITS);

/// A meeting record as `meet` writes it: speakers alternate from `first`, and only
/// `tell` and `dispute` turns name a topic.
fn record_ok(r: &Record) -> bool {
    let second = if r.first == r.a { &r.b } else { &r.a };
    is_ident(&r.a)
        && is_ident(&r.b)
        && r.a != r.b
        && (r.first == r.a || r.first == r.b)
        && !r.turns.is_empty()
        && r.turns.iter().enumerate().all(|(i, t)| {
            let talk = t.kind == "small_talk";
            t.speaker == *(if i % 2 == 0 { &r.first } else { second })
                && t.topic.as_ref().map_or(talk, |x| !talk && is_ident(x))
                && t.value.as_ref().map_or(talk, |x| !talk && is_ident(x))
        })
}

impl Chatter {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("beings", self.minds.len())?;
        within::<Self>("bonds", self.bonds.len())?;
        within::<Self>("records", self.records.len())?;
        for (b, m) in &self.minds {
            ok(is_ident(b) && !(m.knows.is_empty() && m.wants.is_empty()))?;
            within::<Self>("topics", m.knows.len())?;
            within::<Self>("wants", m.wants.len())?;
            ok(m.knows.iter().all(|(t, k)| is_ident(t) && is_ident(&k.value) && !m.wants.contains_key(t)))?;
            ok(m.wants.iter().all(|(t, w)| is_ident(t) && (1..=100).contains(w)))?;
        }
        ok(self.bonds.iter().all(|((a, b), w)| is_ident(a) && is_ident(b) && a < b && (-100..=100).contains(w)))?;
        for r in &self.records {
            within::<Self>("turns", r.turns.len())?;
            ok(record_ok(r) && r.id <= self.seq)?;
        }
        ok(self.records.iter().zip(self.records.iter().skip(1)).all(|(x, y)| x.id < y.id))
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Chatter::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(m: &mut Chatter, ops: &str) -> Result<String> {
        m.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16)?).map(|v| v.to_json())
    }

    #[test]
    fn secrets_wait_for_warmth_and_disputes_become_quarrels() {
        let mut m = Chatter::new();
        let know = r#"{"op":"know","args":{"being":"guard","topic":"gate_key","value":"under_stone","secret":true}},
                      {"op":"know","args":{"being":"thief","topic":"weather","value":"rain"}},
                      {"op":"know","args":{"being":"guard","topic":"weather","value":"sun"}}"#;
        let out =
            run(&mut m, &format!(r#"{know},{{"op":"meet","args":{{"a":"guard","b":"thief","seed":1}}}}"#)).unwrap();
        assert!(!out.contains("under_stone\",\"kind") && out.contains(r#""outcome":"quarrel""#), "{out}");
        assert!(!m.minds["thief"].knows.contains_key("gate_key"));
        assert_eq!(m.bonds[&("guard".to_string(), "thief".to_string())], -10);
    }
}
