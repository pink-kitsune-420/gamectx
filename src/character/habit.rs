//! Habits that form and routines that adapt (pass 2, A5).
//!
//! A being's routine is a set of entries, each "do `action` in `slot`" with a **strength**
//! 0–100. Authored entries (`set_routine`, default 80) never weaken below 30. Choices shape
//! the routine:
//! - choosing an entry's action in its slot **keeps** it (+5, up to 100);
//! - choosing something else in that slot **weakens** it (authored −5, habits −10), as does an
//!   explicit `skip`; a habit that reaches 0 **fades** and is removed;
//! - choosing the same action in a slot 3 times in a row **forms** a habit entry
//!   `habit_<slot>_<action>` at strength 40.
//!
//! Every change is kept with its reason (the last 8 per entry). `routine` lists entries in the
//! [motive arbiter](crate::character::motive) `routine` shape. Capacities are
//! [host-set limits](crate::character::limits). Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::habit::Habits;
//!
//! let mut habits = Habits::new();
//! let choose = r#"{"op": "choose", "args": {"being": "baker", "slot": "dusk", "action": "walk_river", "at": 1}}"#;
//! let req = parse(&format!(r#"{{"ops": [{choose}, {choose}, {choose}]}}"#), 4096)?;
//! let out = habits.workflow(&req)?;
//! assert!(out.to_json().contains(r#""entry":"habit_dusk_walk_river""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_uint, text_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, is_text, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "entries", default: 32, ceiling: 512 },
    Limit { name: "slots", default: 32, ceiling: 512 },
];
pub const FORM_AFTER: u64 = 3;
pub const HABIT_START: u64 = 40;
pub const AUTHORED_DEFAULT: u64 = 80;
pub const AUTHORED_FLOOR: u64 = 30;
pub const KEEP: u64 = 5;
pub const HISTORY: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub slot: String,
    pub action: String,
    pub strength: u64,
    pub habit: bool,
    /// `(at, delta, reason)`, most recent last.
    pub history: Vec<(u64, i64, String)>,
}

impl Entry {
    fn view(&self, id: &str) -> Value {
        obj! {
            "action" => self.action.as_str(),
            "history" => Value::Array(self.history.iter().map(|(at, d, r)| obj! { "at" => *at, "delta" => *d, "reason" => r.as_str() }).collect()),
            "id" => id,
            "origin" => if self.habit { "habit" } else { "authored" },
            "slot" => self.slot.as_str(),
            "strength" => self.strength,
        }
    }

    fn log(&mut self, at: u64, delta: i64, reason: String) {
        self.history.push((at, delta, reason));
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Routine {
    pub entries: BTreeMap<String, Entry>,
    /// slot → (last action chosen, streak).
    pub tallies: BTreeMap<String, (String, u64)>,
}

fn slot_of(v: &Value) -> Result<&str> {
    let s = ident(v)?;
    if s.len() > 24 {
        return Err(c::bad());
    }
    Ok(s)
}

fn action_of(v: &Value) -> Result<&str> {
    let s = ident(v)?;
    if s.len() > 32 {
        return Err(c::bad());
    }
    Ok(s)
}

/// Weakens `id` by its origin's step; returns the change view (and removes a faded habit).
fn weaken(r: &mut Routine, id: &str, at: u64, reason: String) -> Option<Value> {
    let e = r.entries.get_mut(id)?;
    let (step, floor) = if e.habit { (10, 0) } else { (5, AUTHORED_FLOOR) };
    let next = e.strength.saturating_sub(step).max(floor);
    let delta = next as i64 - e.strength as i64;
    if delta == 0 {
        return Some(
            obj! { "delta" => 0i64, "entry" => id, "reason" => reason, "state" => "steady", "strength" => e.strength },
        );
    }
    e.strength = next;
    e.log(at, delta, reason.clone());
    let state = if e.habit && next == 0 { "faded" } else { "weakened" };
    if state == "faded" {
        r.entries.remove(id);
    }
    Some(obj! { "delta" => delta, "entry" => id, "reason" => reason, "state" => state, "strength" => next })
}

/// Per-being routines and slot tallies.
#[derive(Clone, Debug)]
pub struct Habits {
    pub beings: BTreeMap<String, Routine>,
    pub limits: Limits,
}

impl Default for Habits {
    fn default() -> Self {
        Habits { beings: BTreeMap::new(), limits: Limits::defaults(LIMITS) }
    }
}

impl Habits {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (`{beings?, entries?, slots?}`).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut h = Self::new();
        h.limits.apply(v)?;
        Ok(h)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "set_routine" => self.op_set_routine(args, l),
            "choose" => self.op_choose(args, l),
            "skip" => self.op_skip(args),
            "routine" => self.op_routine(args),
            "assess" => self.op_assess(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn routine_mut(&mut self, being: &str, l: &Limits) -> Result<&mut Routine> {
        if !self.beings.contains_key(being) && self.beings.len() >= l.get("beings") {
            return Err(c::bad());
        }
        Ok(self.beings.entry(being.to_string()).or_default())
    }

    fn op_set_routine(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "entries"], &[])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let max = l.get("entries");
        let mut authored: BTreeMap<String, Entry> = BTreeMap::new();
        for e in list_bound(Some(c::get(args, "entries")?), max)? {
            keys(e, &["id", "slot", "action"], &["strength"])?;
            let id = ident(c::get(e, "id")?)?;
            if id.starts_with("habit_") || authored.contains_key(id) {
                return Err(c::bad());
            }
            let entry = Entry {
                slot: slot_of(c::get(e, "slot")?)?.to_string(),
                action: action_of(c::get(e, "action")?)?.to_string(),
                strength: opt_uint(e, "strength", AUTHORED_FLOOR, 100, AUTHORED_DEFAULT)?,
                habit: false,
                history: Vec::new(),
            };
            if authored.values().any(|x| x.slot == entry.slot && x.action == entry.action) {
                return Err(c::bad());
            }
            authored.insert(id.to_string(), entry);
        }
        let r = self.routine_mut(&being, l)?;
        let habits = r.entries.values().filter(|e| e.habit).count();
        if habits + authored.len() > max {
            return Err(c::bad());
        }
        r.entries.retain(|_, e| e.habit);
        let n = authored.len();
        r.entries.extend(authored);
        Ok(obj! { "authored" => n, "being" => being, "entries" => r.entries.len() })
    }

    fn op_choose(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "slot", "action"], &["at"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let slot = slot_of(c::get(args, "slot")?)?.to_string();
        let action = action_of(c::get(args, "action")?)?.to_string();
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let (max_entries, max_slots) = (l.get("entries"), l.get("slots"));
        let r = self.routine_mut(&being, l)?;
        let mut changes = Vec::new();
        let in_slot: Vec<String> = r.entries.iter().filter(|(_, e)| e.slot == slot).map(|(k, _)| k.clone()).collect();
        for id in in_slot {
            let e = r.entries.get_mut(&id).ok_or_else(c::bad)?;
            if e.action == action {
                let next = (e.strength + KEEP).min(100);
                let delta = next as i64 - e.strength as i64;
                if delta != 0 {
                    e.strength = next;
                    e.log(at, delta, "chosen".to_string());
                }
                changes.push(obj! {
                    "delta" => delta, "entry" => id.as_str(), "reason" => "chosen",
                    "state" => if delta != 0 { "kept" } else { "steady" }, "strength" => next,
                });
            } else if let Some(v) = weaken(r, &id, at, format!("chose {action} instead")) {
                changes.push(v);
            }
        }
        if !r.tallies.contains_key(&slot) && r.tallies.len() >= max_slots {
            let victim = r
                .tallies
                .iter()
                .min_by_key(|(k, (_, n))| (*n, (*k).clone()))
                .map(|(k, _)| k.clone())
                .ok_or_else(c::bad)?;
            r.tallies.remove(&victim);
        }
        let t = r.tallies.entry(slot.clone()).or_insert((action.clone(), 0));
        if t.0 == action {
            t.1 += 1;
        } else {
            *t = (action.clone(), 1);
        }
        let streak = t.1;
        let has_entry = r.entries.values().any(|e| e.slot == slot && e.action == action);
        if streak >= FORM_AFTER && !has_entry {
            let id = format!("habit_{slot}_{action}");
            if r.entries.len() >= max_entries {
                changes.push(obj! { "delta" => 0i64, "entry" => id, "reason" => format!("chose {action} {streak} times in a row"), "state" => "blocked", "strength" => 0u64 });
            } else {
                let reason = format!("chose {action} {streak} times in a row");
                let mut e = Entry {
                    slot: slot.clone(),
                    action: action.clone(),
                    strength: HABIT_START,
                    habit: true,
                    history: Vec::new(),
                };
                e.log(at, HABIT_START as i64, reason.clone());
                r.entries.insert(id.clone(), e);
                changes.push(obj! { "delta" => HABIT_START as i64, "entry" => id, "reason" => reason, "state" => "formed", "strength" => HABIT_START });
            }
        }
        Ok(
            obj! { "action" => action, "being" => being, "changes" => Value::Array(changes), "slot" => slot, "streak" => streak },
        )
    }

    fn op_skip(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "entry"], &["at", "reason"])?;
        let being = ident(c::get(args, "being")?)?;
        let id = ident(c::get(args, "entry")?)?;
        let at = opt_uint(args, "at", 0, MAX_AT, 0)?;
        let reason = opt(args, "reason").map(|v| text_bound(v, 80)).transpose()?.unwrap_or("skipped").to_string();
        let r = self.beings.get_mut(being).ok_or_else(c::bad)?;
        weaken(r, id, at, reason).ok_or_else(c::bad)
    }

    fn op_routine(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["slot"])?;
        let being = ident(c::get(args, "being")?)?;
        let slot = opt(args, "slot").map(slot_of).transpose()?;
        let mut list: Vec<(&String, &Entry)> = self
            .beings
            .get(being)
            .map(|r| r.entries.iter().filter(|(_, e)| slot.is_none_or(|s| e.slot == s)).collect())
            .unwrap_or_default();
        list.sort_by(|a, b| b.1.strength.cmp(&a.1.strength).then_with(|| a.0.cmp(b.0)));
        let entries = list
            .into_iter()
            .map(|(id, e)| {
                obj! {
                    "action" => e.action.as_str(),
                    "id" => id.as_str(),
                    "label" => format!("{} ({})", e.action, e.slot),
                    "priority" => e.strength,
                    "slot" => e.slot.as_str(),
                }
            })
            .collect();
        Ok(obj! { "being" => being, "entries" => Value::Array(entries) })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let r = self.beings.get(being).ok_or_else(c::bad)?;
        Ok(obj! {
            "being" => being,
            "entries" => Value::Array(r.entries.iter().map(|(id, e)| e.view(id)).collect()),
            "tallies" => Value::Array(r.tallies.iter().map(|(s, (a, n))| obj! { "action" => a.as_str(), "slot" => s.as_str(), "streak" => *n }).collect()),
        })
    }
}

persist!(Entry { action, habit, history, slot, strength [0, 100] });
persist!(Routine { entries, tallies });
persist!(rest Habits { beings });
durable!(Habits, "habit", LIMITS);

/// An entry as `set_routine` (authored) or `choose` (habit) makes it, and as `weaken` leaves it.
fn entry_ok(id: &str, e: &Entry) -> bool {
    let floor = if e.habit { 1 } else { AUTHORED_FLOOR };
    is_ident(id)
        && is_ident(&e.slot)
        && e.slot.len() <= 24
        && is_ident(&e.action)
        && e.action.len() <= 32
        && e.strength >= floor
        && (if e.habit { id == format!("habit_{}_{}", e.slot, e.action) } else { !id.starts_with("habit_") })
        && e.history.len() <= HISTORY
        && e.history.iter().all(|(at, d, r)| *at <= MAX_AT && *d != 0 && (-100..=100).contains(d) && is_text(r, 80))
}

impl Habits {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("beings", self.beings.len())?;
        for (b, r) in &self.beings {
            ok(is_ident(b))?;
            within::<Self>("entries", r.entries.len())?;
            within::<Self>("slots", r.tallies.len())?;
            ok(r.entries.iter().all(|(id, e)| entry_ok(id, e)))?;
            let mut pairs = BTreeSet::new();
            ok(r.entries.values().filter(|e| !e.habit).all(|e| pairs.insert((&e.slot, &e.action))))?;
            let tally_ok = |(s, (a, n)): (&String, &(String, u64))| {
                is_ident(s) && s.len() <= 24 && is_ident(a) && a.len() <= 32 && *n >= 1 && *n <= MAX_AT
            };
            ok(r.tallies.iter().all(tally_ok))?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Habits::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(h: &mut Habits, ops: &str) -> String {
        h.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16).unwrap()).unwrap().to_json()
    }

    #[test]
    fn authored_entries_weaken_to_a_floor_and_habits_fade() {
        let mut h = Habits::new();
        let set = r#"{"op":"set_routine","args":{"being":"b","entries":[{"id":"bake","slot":"dawn","action":"bake","strength":35}]}}"#;
        let tavern = r#"{"op":"choose","args":{"being":"b","slot":"dawn","action":"tavern"}}"#;
        let fish = r#"{"op":"choose","args":{"being":"b","slot":"dawn","action":"fish"}}"#;
        let mut ops = vec![set, tavern, tavern, tavern];
        ops.extend([fish; 4]);
        let out = run(&mut h, &ops.join(","));
        assert!(out.contains(r#""state":"formed""#) && out.contains(r#""state":"faded""#), "{out}");
        assert_eq!(h.beings["b"].entries["bake"].strength, AUTHORED_FLOOR);
    }
}
