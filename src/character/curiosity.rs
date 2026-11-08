//! Curiosity model and novelty-driven initiative generation (Gap 11).
//!
//! Evaluates novelty of unencountered entities (people, things, places, facts),
//! computes novelty signal scaled by temperament (`curious`, `cautious`, `apathetic`, `calm`),
//! and generates host initiative candidates for investigation or dialogue.
//! Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::curiosity::CuriosityManager;
//!
//! let mut mgr = CuriosityManager::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "init",
//!       "args": {
//!         "being": "baker",
//!         "temperament": "curious",
//!         "known_people": ["alice"],
//!         "known_things": ["oven"],
//!         "known_places": ["bakery"],
//!         "known_facts": ["recipe_bread"]
//!       }
//!     },
//!     {
//!       "op": "evaluate_novelty",
//!       "args": {
//!         "being": "baker",
//!         "people": ["stranger_bob"],
//!         "things": ["oven", "mysterious_gem"]
//!       }
//!     }
//!   ]
//! }"#, 4096)?;
//! let out = mgr.workflow(&req)?;
//! assert_eq!(out.get("version").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BEINGS: usize = 64;
pub const MAX_OPS: usize = 64;
pub const MAX_ITEMS_PER_PERCEPTION: usize = 32;
pub const MAX_KNOWN_ENTITIES: usize = 256;

pub const TEMPERAMENTS: [&str; 4] = ["curious", "cautious", "apathetic", "calm"];

pub const BASE_SCORE_PERSON: i64 = 40;
pub const BASE_SCORE_PLACE: i64 = 35;
pub const BASE_SCORE_THING: i64 = 30;
pub const BASE_SCORE_FACT: i64 = 25;

fn ident_bound(v: &Value) -> Result<&str> {
    let s = ident(v)?;
    if s.is_empty() || s.len() > 64 {
        return Err(c::bad());
    }
    Ok(s)
}

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

fn bool_bound(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(c::bad()),
    }
}

fn parse_string_list(v: &Value, max_items: usize) -> Result<BTreeSet<String>> {
    let list = v.as_array().ok_or_else(c::bad)?;
    if list.len() > max_items {
        return Err(c::bad());
    }
    let mut out = BTreeSet::new();
    for item in list {
        let s = ident_bound(item)?;
        out.insert(s.to_string());
    }
    Ok(out)
}

/// Being's curiosity and familiarity memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeingCuriosity {
    pub being: String,
    pub temperament: String,
    pub known_people: BTreeSet<String>,
    pub known_things: BTreeSet<String>,
    pub known_places: BTreeSet<String>,
    pub known_facts: BTreeSet<String>,
}

impl BeingCuriosity {
    pub fn new(being: impl Into<String>) -> Self {
        Self {
            being: being.into(),
            temperament: "calm".to_string(),
            known_people: BTreeSet::new(),
            known_things: BTreeSet::new(),
            known_places: BTreeSet::new(),
            known_facts: BTreeSet::new(),
        }
    }

    pub fn to_value(&self) -> Value {
        obj! {
            "being" => Value::str(&self.being),
            "known_facts" => Value::Array(self.known_facts.iter().map(Value::str).collect()),
            "known_people" => Value::Array(self.known_people.iter().map(Value::str).collect()),
            "known_places" => Value::Array(self.known_places.iter().map(Value::str).collect()),
            "known_things" => Value::Array(self.known_things.iter().map(Value::str).collect()),
            "temperament" => Value::str(&self.temperament),
        }
    }

    /// Scale a base score by temperament multiplier (numerator, denominator):
    /// - curious: 15 / 10 (1.5x)
    /// - cautious: 7 / 10 (0.7x)
    /// - apathetic: 4 / 10 (0.4x)
    /// - calm: 10 / 10 (1.0x)
    pub fn scale_score(&self, base: i64) -> i64 {
        let (num, den) = match self.temperament.as_str() {
            "curious" => (15, 10),
            "cautious" => (7, 10),
            "apathetic" => (4, 10),
            _ => (10, 10),
        };
        (base * num) / den
    }
}

/// Engine managing curiosity and novelty detection across beings.
#[derive(Clone, Debug, Default)]
pub struct CuriosityManager {
    pub beings: BTreeMap<String, BeingCuriosity>,
}

impl CuriosityManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn being_mut(&mut self, being: &str) -> Result<&mut BeingCuriosity> {
        if self.beings.len() >= MAX_BEINGS && !self.beings.contains_key(being) {
            return Err(c::bad());
        }
        Ok(self.beings.entry(being.to_string()).or_insert_with(|| BeingCuriosity::new(being)))
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &[])?;
        let ops = request.get("ops").and_then(Value::as_array).ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        let mut results = Vec::new();
        for op_val in ops {
            keys(op_val, &["op", "args"], &[])?;
            let op = text_bound(c::get(op_val, "op")?, 32)?;
            let args = c::get(op_val, "args")?;

            let res = match op {
                "init" => self.op_init(args)?,
                "evaluate_novelty" => self.op_evaluate_novelty(args)?,
                "generate_candidates" => self.op_generate_candidates(args)?,
                "mark_familiar" => self.op_mark_familiar(args)?,
                "assess" => self.op_assess(args)?,
                _ => return Err(c::bad()),
            };
            results.push(res);
        }

        Ok(obj! {
            "version" => Value::int(1),
            "results" => Value::Array(results),
        })
    }

    fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["temperament", "known_people", "known_things", "known_places", "known_facts"])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let temperament = args
            .get("temperament")
            .filter(|x| **x != Value::Null)
            .map(|v| text_bound(v, 16))
            .transpose()?
            .unwrap_or("calm");
        if !TEMPERAMENTS.contains(&temperament) {
            return Err(c::bad());
        }

        let known_people = args
            .get("known_people")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_KNOWN_ENTITIES))
            .transpose()?
            .unwrap_or_default();
        let known_things = args
            .get("known_things")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_KNOWN_ENTITIES))
            .transpose()?
            .unwrap_or_default();
        let known_places = args
            .get("known_places")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_KNOWN_ENTITIES))
            .transpose()?
            .unwrap_or_default();
        let known_facts = args
            .get("known_facts")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_KNOWN_ENTITIES))
            .transpose()?
            .unwrap_or_default();

        if self.beings.len() >= MAX_BEINGS && !self.beings.contains_key(being) {
            return Err(c::bad());
        }

        let entry = BeingCuriosity {
            being: being.to_string(),
            temperament: temperament.to_string(),
            known_people,
            known_things,
            known_places,
            known_facts,
        };
        self.beings.insert(being.to_string(), entry);

        Ok(obj! {
            "being" => Value::str(being),
            "ok" => Value::Bool(true),
        })
    }

    fn op_evaluate_novelty(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["people", "things", "places", "facts"])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let cur = self.being_mut(being)?;

        let people = args
            .get("people")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_ITEMS_PER_PERCEPTION))
            .transpose()?
            .unwrap_or_default();
        let things = args
            .get("things")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_ITEMS_PER_PERCEPTION))
            .transpose()?
            .unwrap_or_default();
        let places = args
            .get("places")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_ITEMS_PER_PERCEPTION))
            .transpose()?
            .unwrap_or_default();
        let facts = args
            .get("facts")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_ITEMS_PER_PERCEPTION))
            .transpose()?
            .unwrap_or_default();

        let novel_people: Vec<String> = people.into_iter().filter(|p| !cur.known_people.contains(p)).collect();
        let novel_things: Vec<String> = things.into_iter().filter(|t| !cur.known_things.contains(t)).collect();
        let novel_places: Vec<String> = places.into_iter().filter(|pl| !cur.known_places.contains(pl)).collect();
        let novel_facts: Vec<String> = facts.into_iter().filter(|f| !cur.known_facts.contains(f)).collect();

        let mut total_score: i64 = 0;
        for _ in &novel_people {
            total_score += cur.scale_score(BASE_SCORE_PERSON);
        }
        for _ in &novel_things {
            total_score += cur.scale_score(BASE_SCORE_THING);
        }
        for _ in &novel_places {
            total_score += cur.scale_score(BASE_SCORE_PLACE);
        }
        for _ in &novel_facts {
            total_score += cur.scale_score(BASE_SCORE_FACT);
        }

        Ok(obj! {
            "being" => Value::str(&cur.being),
            "novel_facts" => Value::Array(novel_facts.into_iter().map(|s| Value::str(&s)).collect()),
            "novel_people" => Value::Array(novel_people.into_iter().map(|s| Value::str(&s)).collect()),
            "novel_places" => Value::Array(novel_places.into_iter().map(|s| Value::str(&s)).collect()),
            "novel_things" => Value::Array(novel_things.into_iter().map(|s| Value::str(&s)).collect()),
            "novelty_score" => Value::from(total_score),
            "temperament" => Value::str(&cur.temperament),
        })
    }

    fn op_generate_candidates(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["people", "things", "places", "facts", "limit", "auto_mark"])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let limit =
            args.get("limit").filter(|x| **x != Value::Null).map(|v| uint_bound(v, 1, 16)).transpose()?.unwrap_or(8)
                as usize;
        let auto_mark =
            args.get("auto_mark").filter(|x| **x != Value::Null).map(bool_bound).transpose()?.unwrap_or(false);

        let cur = self.being_mut(being)?;

        let people = args
            .get("people")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_ITEMS_PER_PERCEPTION))
            .transpose()?
            .unwrap_or_default();
        let things = args
            .get("things")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_ITEMS_PER_PERCEPTION))
            .transpose()?
            .unwrap_or_default();
        let places = args
            .get("places")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_ITEMS_PER_PERCEPTION))
            .transpose()?
            .unwrap_or_default();
        let facts = args
            .get("facts")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_ITEMS_PER_PERCEPTION))
            .transpose()?
            .unwrap_or_default();

        let novel_people: Vec<String> = people.into_iter().filter(|p| !cur.known_people.contains(p)).collect();
        let novel_things: Vec<String> = things.into_iter().filter(|t| !cur.known_things.contains(t)).collect();
        let novel_places: Vec<String> = places.into_iter().filter(|pl| !cur.known_places.contains(pl)).collect();
        let novel_facts: Vec<String> = facts.into_iter().filter(|f| !cur.known_facts.contains(f)).collect();

        let mut candidates = Vec::new();

        // 1. Novel people -> kind: "ask", recipient: person
        for p in &novel_people {
            if candidates.len() >= limit {
                break;
            }
            let prio = cur.scale_score(BASE_SCORE_PERSON).clamp(1, 100);
            candidates.push(obj! {
                "attitudes" => Value::Array(Vec::new()),
                "events" => Value::Array(Vec::new()),
                "id" => Value::str(format!("curious_ask_{p}")),
                "kind" => Value::str("ask"),
                "priority" => Value::from(prio),
                "promises" => Value::Array(Vec::new()),
                "reason" => Value::str(format!("Inquire about stranger {p}")),
                "recipient" => Value::str(p),
                "requires" => Value::Array(Vec::new()),
                "resource" => Value::Null,
            });
        }

        // 2. Unknown facts -> kind: "ask", recipient: Null
        for f in &novel_facts {
            if candidates.len() >= limit {
                break;
            }
            let prio = cur.scale_score(BASE_SCORE_FACT).clamp(1, 100);
            candidates.push(obj! {
                "attitudes" => Value::Array(Vec::new()),
                "events" => Value::Array(Vec::new()),
                "id" => Value::str(format!("curious_fact_{f}")),
                "kind" => Value::str("ask"),
                "priority" => Value::from(prio),
                "promises" => Value::Array(Vec::new()),
                "reason" => Value::str(format!("Ask about fact {f}")),
                "recipient" => Value::Null,
                "requires" => Value::Array(Vec::new()),
                "resource" => Value::Null,
            });
        }

        // 3. Novel places -> kind: "activity", recipient: Null
        for pl in &novel_places {
            if candidates.len() >= limit {
                break;
            }
            let prio = cur.scale_score(BASE_SCORE_PLACE).clamp(1, 100);
            candidates.push(obj! {
                "attitudes" => Value::Array(Vec::new()),
                "events" => Value::Array(Vec::new()),
                "id" => Value::str(format!("curious_place_{pl}")),
                "kind" => Value::str("activity"),
                "priority" => Value::from(prio),
                "promises" => Value::Array(Vec::new()),
                "reason" => Value::str(format!("Explore new location {pl}")),
                "recipient" => Value::Null,
                "requires" => Value::Array(Vec::new()),
                "resource" => Value::Null,
            });
        }

        // 4. Novel things -> kind: "activity", recipient: Null
        for t in &novel_things {
            if candidates.len() >= limit {
                break;
            }
            let prio = cur.scale_score(BASE_SCORE_THING).clamp(1, 100);
            candidates.push(obj! {
                "attitudes" => Value::Array(Vec::new()),
                "events" => Value::Array(Vec::new()),
                "id" => Value::str(format!("curious_thing_{t}")),
                "kind" => Value::str("activity"),
                "priority" => Value::from(prio),
                "promises" => Value::Array(Vec::new()),
                "reason" => Value::str(format!("Examine unfamiliar {t}")),
                "recipient" => Value::Null,
                "requires" => Value::Array(Vec::new()),
                "resource" => Value::Null,
            });
        }

        if auto_mark {
            for p in &novel_people {
                if cur.known_people.len() < MAX_KNOWN_ENTITIES {
                    cur.known_people.insert(p.clone());
                }
            }
            for f in &novel_facts {
                if cur.known_facts.len() < MAX_KNOWN_ENTITIES {
                    cur.known_facts.insert(f.clone());
                }
            }
            for pl in &novel_places {
                if cur.known_places.len() < MAX_KNOWN_ENTITIES {
                    cur.known_places.insert(pl.clone());
                }
            }
            for t in &novel_things {
                if cur.known_things.len() < MAX_KNOWN_ENTITIES {
                    cur.known_things.insert(t.clone());
                }
            }
        }

        Ok(obj! {
            "being" => Value::str(&cur.being),
            "candidates" => Value::Array(candidates.clone()),
            "count" => Value::from(candidates.len()),
        })
    }

    fn op_mark_familiar(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["people", "things", "places", "facts"])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let cur = self.being_mut(being)?;

        if let Some(list) = args
            .get("people")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_KNOWN_ENTITIES))
            .transpose()?
        {
            for p in list {
                if cur.known_people.len() < MAX_KNOWN_ENTITIES {
                    cur.known_people.insert(p);
                }
            }
        }

        if let Some(list) = args
            .get("things")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_KNOWN_ENTITIES))
            .transpose()?
        {
            for t in list {
                if cur.known_things.len() < MAX_KNOWN_ENTITIES {
                    cur.known_things.insert(t);
                }
            }
        }

        if let Some(list) = args
            .get("places")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_KNOWN_ENTITIES))
            .transpose()?
        {
            for pl in list {
                if cur.known_places.len() < MAX_KNOWN_ENTITIES {
                    cur.known_places.insert(pl);
                }
            }
        }

        if let Some(list) = args
            .get("facts")
            .filter(|x| **x != Value::Null)
            .map(|v| parse_string_list(v, MAX_KNOWN_ENTITIES))
            .transpose()?
        {
            for f in list {
                if cur.known_facts.len() < MAX_KNOWN_ENTITIES {
                    cur.known_facts.insert(f);
                }
            }
        }

        Ok(obj! {
            "being" => Value::str(&cur.being),
            "ok" => Value::Bool(true),
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident_bound(c::get(args, "being")?)?;
        let cur = self.beings.get(being).ok_or_else(c::bad)?;
        Ok(cur.to_value())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    let mut mgr = CuriosityManager::new();
    mgr.workflow(req)
}
