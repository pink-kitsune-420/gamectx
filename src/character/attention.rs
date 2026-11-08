//! Attention and graded noticing (pass 2, A2): who notices what, deterministically.
//!
//! The [witness bridge](crate::character::life::witness) answers "could this being perceive
//! the event?". Attention answers "did it actually notice?", from the event's **salience**
//! and **noise**, the being's **light**, **distance** (its own place, or a `near` place),
//! how **busy** it is, whether it is **asleep**, its **alertness** and **perception**, and
//! whether it **cares** about the event's kind or subject.
//!
//! Each being×event gets a score 0–100; the event is noticed when a seeded roll
//! (FNV-1a of `"<seed>:<being>:<event>"` mod 100) falls below it, so the host's seed makes
//! replay exact. Attention is limited: a being notices at most `capacity` events per call
//! (highest score first), so a busy market hides a pickpocket. `noticed_by` lists, per event,
//! the beings to pass on as witnesses.
//!
//! Bounded: 16 events and 64 beings per call, 64 profiles. Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::attention::AttentionModel;
//!
//! let mut attention = AttentionModel::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "notice", "args": {"seed": 7,
//!     "events": [{"id": "crash", "kind": "accident", "place": "road", "salience": 90, "noise": 80}],
//!     "beings": [{"id": "baker", "place": "road"}, {"id": "apprentice", "place": "bakery"}]}}
//! ]}"#, 4096)?;
//! let out = attention.workflow(&req)?;
//! assert!(out.to_json().contains(r#""noticed_by":["baker"]"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_bool, opt_uint, run_ops, uint_bound, MAX_AT};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_EVENTS: usize = 16;
pub const MAX_BEINGS: usize = 64;
pub const MAX_PROFILES: usize = 64;
pub const MAX_CARES: usize = 16;
pub const MAX_NEAR: usize = 16;
pub const MAX_SEED: u64 = (1 << 53) - 1;

/// Host-set capacity limits: `(name, default, ceiling)`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "profiles", default: 64, ceiling: 1000000 },
    crate::character::limits::Limit { name: "notice_beings", default: 64, ceiling: 4096 },
    crate::character::limits::Limit { name: "notice_events", default: 16, ceiling: 256 },
];
crate::character::limits::module_limits!(AttentionLimits, LIMITS);

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub alertness: u64,
    pub perception: u64,
    pub cares: BTreeSet<String>,
    pub capacity: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { alertness: 50, perception: 0, cares: BTreeSet::new(), capacity: 3 }
    }
}

impl Profile {
    fn view(&self, being: &str) -> Value {
        obj! {
            "alertness" => self.alertness,
            "being" => being,
            "capacity" => self.capacity,
            "cares" => Value::strings(&self.cares.iter().collect::<Vec<_>>()),
            "perception" => self.perception,
        }
    }
}

/// FNV-1a (64-bit) of `text`.
pub fn fnv1a(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

struct Event {
    id: String,
    kind: String,
    place: String,
    salience: u64,
    noise: u64,
    subject: Option<String>,
}

/// Attention profiles per being.
#[derive(Clone, Debug, Default)]
pub struct AttentionModel {
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: AttentionLimits,
    pub profiles: BTreeMap<String, Profile>,
}

impl AttentionModel {
    pub fn new() -> Self {
        Self::default()
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        run_ops(request, |op, args| match op {
            "profile" => self.op_profile(args),
            "notice" => self.op_notice(args),
            "assess" => self.op_assess(args),
            _ => Err(c::bad()),
        })
    }

    fn op_profile(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["alertness", "perception", "cares", "capacity"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let mut cares = BTreeSet::new();
        for x in list_bound(opt(args, "cares"), MAX_CARES)? {
            cares.insert(ident(x)?.to_string());
        }
        let p = Profile {
            alertness: opt_uint(args, "alertness", 0, 100, 50)?,
            perception: opt_uint(args, "perception", 0, 100, 0)?,
            cares,
            capacity: opt_uint(args, "capacity", 1, MAX_EVENTS as u64, 3)?,
        };
        if !self.profiles.contains_key(&being) && self.profiles.len() >= self.limits.get("profiles") {
            return Err(c::bad());
        }
        let view = p.view(&being);
        self.profiles.insert(being, p);
        Ok(view)
    }

    fn op_notice(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["seed", "events", "beings"], &["at"])?;
        let seed = uint_bound(c::get(args, "seed")?, 0, MAX_SEED)?;
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let raw_events = list_bound(Some(c::get(args, "events")?), self.limits.get("notice_events"))?;
        let raw_beings = list_bound(Some(c::get(args, "beings")?), self.limits.get("notice_beings"))?;
        if raw_events.is_empty() || raw_beings.is_empty() {
            return Err(c::bad());
        }
        let mut events: Vec<Event> = Vec::new();
        for e in raw_events {
            keys(e, &["id", "kind", "place", "salience"], &["noise", "subject"])?;
            let id = ident(c::get(e, "id")?)?.to_string();
            if events.iter().any(|x| x.id == id) {
                return Err(c::bad());
            }
            events.push(Event {
                id,
                kind: ident(c::get(e, "kind")?)?.to_string(),
                place: ident(c::get(e, "place")?)?.to_string(),
                salience: uint_bound(c::get(e, "salience")?, 0, 100)?,
                noise: opt_uint(e, "noise", 0, 100, 0)?,
                subject: opt(e, "subject").map(ident).transpose()?.map(str::to_string),
            });
        }
        let mut seen = BTreeSet::new();
        let mut noticed_by: Vec<Vec<String>> = vec![Vec::new(); events.len()];
        let mut out_beings = Vec::new();
        for b in raw_beings {
            keys(b, &["id", "place"], &["near", "light", "busy", "asleep", "sight", "hearing"])?;
            let id = ident(c::get(b, "id")?)?.to_string();
            if !seen.insert(id.clone()) {
                return Err(c::bad());
            }
            let place = ident(c::get(b, "place")?)?;
            let mut near: BTreeMap<&str, u64> = BTreeMap::new();
            for n in list_bound(opt(b, "near"), MAX_NEAR)? {
                keys(n, &["place", "distance"], &[])?;
                let p = ident(c::get(n, "place")?)?;
                if p == place || near.insert(p, uint_bound(c::get(n, "distance")?, 1, 1000)?).is_some() {
                    return Err(c::bad());
                }
            }
            let light = opt_uint(b, "light", 0, 100, 100)?;
            let busy = opt_uint(b, "busy", 0, 100, 0)?;
            let asleep = opt_bool(b, "asleep", false)?;
            let sight = opt_bool(b, "sight", true)?;
            let hearing = opt_bool(b, "hearing", true)?;
            let default = Profile::default();
            let prof = self.profiles.get(&id).unwrap_or(&default);

            let mut hits: Vec<(u64, usize)> = Vec::new();
            let mut missed: Vec<(usize, &str, u64)> = Vec::new();
            for (i, e) in events.iter().enumerate() {
                let distance = if e.place == place {
                    0
                } else if let Some(d) = near.get(e.place.as_str()) {
                    *d
                } else {
                    missed.push((i, "out_of_range", 0));
                    continue;
                };
                let visual = if sight && !asleep { e.salience * light / 100 } else { 0 };
                let audible = if hearing { e.noise } else { 0 };
                let cares = prof.cares.contains(&e.kind) || e.subject.as_ref().is_some_and(|s| prof.cares.contains(s));
                let raw = visual.max(audible) as i64
                    + (prof.alertness / 5) as i64
                    + (prof.perception / 5) as i64
                    + if cares { 25 } else { 0 }
                    - if asleep { 0 } else { (busy / 3) as i64 }
                    - (distance / 5) as i64;
                let mut score = raw.clamp(0, 100) as u64;
                if asleep {
                    score /= 2;
                }
                if visual.max(audible) == 0 {
                    score = 0;
                }
                let roll = fnv1a(&format!("{seed}:{id}:{}", e.id)) % 100;
                if score == 0 {
                    missed.push((i, "imperceptible", 0));
                } else if roll >= score {
                    missed.push((i, "overlooked", score));
                } else {
                    hits.push((score, i));
                }
            }
            hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| events[a.1].id.cmp(&events[b.1].id)));
            let mut noticed = Vec::new();
            for (n, (score, i)) in hits.into_iter().enumerate() {
                if n as u64 >= prof.capacity {
                    missed.push((i, "over_capacity", score));
                    continue;
                }
                noticed_by[i].push(id.clone());
                noticed.push(obj! {
                    "clarity" => if score >= 60 { "clear" } else { "vague" },
                    "event" => events[i].id.as_str(),
                    "score" => score,
                });
            }
            missed.sort_by_key(|m| m.0);
            out_beings.push(obj! {
                "being" => id.as_str(),
                "missed" => Value::Array(missed.iter().map(|(i, r, s)| obj! {
                    "event" => events[*i].id.as_str(), "reason" => *r, "score" => *s,
                }).collect()),
                "noticed" => Value::Array(noticed),
            });
        }
        Ok(obj! {
            "at" => at,
            "beings" => Value::Array(out_beings),
            "events" => Value::Array(events.iter().zip(noticed_by).map(|(e, by)| obj! {
                "event" => e.id.as_str(),
                "noticed_by" => Value::strings(&by.iter().collect::<Vec<_>>()),
            }).collect()),
            "seed" => seed,
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        Ok(self.profiles.get(being).cloned().unwrap_or_default().view(being))
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    AttentionModel::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    #[test]
    fn fnv1a_matches_reference_vectors() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn capacity_limits_what_a_busy_being_notices() {
        let mut m = AttentionModel::new();
        let ev: Vec<String> = (0..5)
            .map(|i| format!(r#"{{"id":"e{i}","kind":"stall","place":"market","salience":100,"noise":100}}"#))
            .collect();
        let req = format!(
            r#"{{"ops":[{{"op":"profile","args":{{"being":"b","alertness":100,"capacity":2}}}},
               {{"op":"notice","args":{{"seed":1,"events":[{}],"beings":[{{"id":"b","place":"market"}}]}}}}]}}"#,
            ev.join(",")
        );
        let out = m.workflow(&parse(&req, 1 << 16).unwrap()).unwrap().to_json();
        assert_eq!(out.matches(r#""reason":"over_capacity""#).count(), 3, "{out}");
    }

    #[test]
    fn asleep_and_deaf_perceives_nothing() {
        let mut m = AttentionModel::new();
        let req = r#"{"ops":[{"op":"notice","args":{"seed":3,"events":[{"id":"bell","kind":"alarm","place":"square","salience":100,"noise":100}],
            "beings":[{"id":"b","place":"square","asleep":true,"hearing":false}]}}]}"#;
        let out = m.workflow(&parse(req, 4096).unwrap()).unwrap().to_json();
        assert!(out.contains(r#""reason":"imperceptible""#), "{out}");
    }
}
