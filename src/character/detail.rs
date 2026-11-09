//! Level of detail for minds (pass 2, D2).
//!
//! Each tracked being has needs that grow linearly: a level and a rate per tick in millionths of
//! full, and a threshold (default 700,000) at which the need becomes urgent. Its detail level
//! follows its distance from the player or camera:
//!
//! - `full` (distance ≤ `near`, default 20) is brought up to date on every `tick`;
//! - `coarse` (≤ `far`, default 100) only on multiples of `stride` (default 10);
//! - `crowd` beings are frozen in a named group, summarized by `crowd` and caught up when they
//!   `move` closer.
//!
//! Because growth is linear, catching up after any gap gives exactly the levels and the exact
//! urgent tick that full updates would have: moving between levels never loses or invents state,
//! only the reported `work` (full: needs × ticks, coarse: needs × strides, crowd: one unit per
//! group per tick call) and when an event is delivered differ. Capacities are
//! [host-set limits](crate::character::limits). Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::detail::Detail;
//!
//! let mut lod = Detail::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "track", "args": {"being": "farmer", "distance": 500, "group": "east_fields",
//!                            "needs": [{"id": "hunger", "rate": 10000, "level": 400000}]}},
//!   {"op": "tick", "args": {"now": 50}},
//!   {"op": "move", "args": {"being": "farmer", "distance": 5}}
//! ]}"#, 4096)?;
//! let out = lod.workflow(&req)?.to_json();
//! assert!(out.contains(r#"{"at":30,"being":"farmer","need":"hunger"}"#) && out.contains(r#""to":"full""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "needs", default: 8, ceiling: 64 },
    Limit { name: "groups", default: 64, ceiling: 65_536 },
];
pub const M: u64 = 1_000_000;
pub const THRESHOLD: u64 = 700_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Need {
    pub level: u64,
    pub rate: u64,
    pub threshold: u64,
}

impl Need {
    /// First tick after `since` at which the need reaches its threshold.
    fn urgent_at(&self, since: u64) -> Option<u64> {
        if self.level >= self.threshold || self.rate == 0 {
            return None;
        }
        Some(since + (self.threshold - self.level).div_ceil(self.rate))
    }

    fn level_after(&self, ticks: u64) -> u64 {
        (self.level as u128 + self.rate as u128 * ticks as u128).min(M as u128) as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tracked {
    pub needs: BTreeMap<String, Need>,
    pub at: u64,
    pub distance: u64,
    pub detail: &'static str,
    pub group: Option<String>,
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

/// Tracked beings, their detail levels and the shared clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detail {
    pub beings: BTreeMap<String, Tracked>,
    pub clock: u64,
    pub near: u64,
    pub far: u64,
    pub stride: u64,
    pub limits: Limits,
    /// Members of each crowd group (an index kept in step with `beings`).
    pub groups: BTreeMap<String, BTreeSet<String>>,
    /// Beings at full or coarse detail, the only ones a tick visits.
    pub active: BTreeSet<String>,
}

impl Default for Detail {
    fn default() -> Self {
        Detail {
            beings: BTreeMap::new(),
            clock: 0,
            near: 20,
            far: 100,
            stride: 10,
            limits: Limits::defaults(LIMITS),
            groups: BTreeMap::new(),
            active: BTreeSet::new(),
        }
    }
}

type Event = (u64, String, String);

fn event_view(e: &Event) -> Value {
    obj! { "at" => e.0, "being" => e.1.as_str(), "need" => e.2.as_str() }
}

impl Detail {
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
            "configure" => self.op_configure(args),
            "track" => self.op_track(args, l),
            "move" => self.op_move(args, l),
            "tick" => self.op_tick(args),
            "assess" => self.op_assess(args),
            "crowd" => self.op_crowd(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn level_of(&self, distance: u64) -> &'static str {
        if distance <= self.near {
            "full"
        } else if distance <= self.far {
            "coarse"
        } else {
            "crowd"
        }
    }

    /// Brings a being's needs up to `now`; urgent events in (tick, need) order.
    fn advance(x: &mut Tracked, b: &str, now: u64) -> Vec<Event> {
        let mut events = Vec::new();
        if now <= x.at {
            return events;
        }
        for (nid, n) in x.needs.iter_mut() {
            if let Some(t) = n.urgent_at(x.at).filter(|t| *t <= now) {
                events.push((t, b.to_string(), nid.clone()));
            }
            n.level = n.level_after(now - x.at);
        }
        x.at = now;
        events.sort();
        events
    }

    fn view(b: &str, x: &Tracked) -> Vec<(String, Value)> {
        let needs = x
            .needs
            .iter()
            .map(|(k, n)| obj! { "id" => k.as_str(), "level" => n.level, "rate" => n.rate, "threshold" => n.threshold })
            .collect();
        vec![
            ("at".into(), Value::from(x.at)),
            ("being".into(), Value::from(b)),
            ("distance".into(), Value::from(x.distance)),
            ("group".into(), x.group.as_deref().map(Value::from).unwrap_or(Value::Null)),
            ("level".into(), Value::from(x.detail)),
            ("needs".into(), Value::Array(needs)),
        ]
    }

    /// Whether `g` can take one more member under the groups limit, not counting `leaving`'s
    /// current membership (a being that is its group's only member frees that group).
    fn group_room(&self, g: &str, leaving: Option<&str>, limit: usize) -> bool {
        let alone =
            |grp: &str| leaving.is_some_and(|b| self.groups.get(grp).is_some_and(|m| m.len() == 1 && m.contains(b)));
        let exists = self.groups.contains_key(g) && !alone(g);
        let mut count = self.groups.len();
        if let Some(own) = leaving.and_then(|b| self.beings.get(b)).and_then(|x| x.group.as_deref()) {
            if alone(own) {
                count -= 1;
            }
        }
        exists || count < limit
    }

    /// Keeps the group and active indexes in step with a being's placement.
    fn index(&mut self, b: &str, group: Option<&str>, add: bool) {
        match group {
            Some(g) if add => {
                self.groups.entry(g.to_string()).or_default().insert(b.to_string());
            }
            Some(g) => {
                if let Some(m) = self.groups.get_mut(g) {
                    m.remove(b);
                    if m.is_empty() {
                        self.groups.remove(g);
                    }
                }
            }
            None if add => {
                self.active.insert(b.to_string());
            }
            None => {
                self.active.remove(b);
            }
        }
    }

    fn op_configure(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["near", "far", "stride"])?;
        let near = opt_uint(args, "near", 0, MAX_AT, self.near)?;
        let far = opt_uint(args, "far", 0, MAX_AT, self.far)?;
        let stride = opt_uint(args, "stride", 1, 1_000_000, self.stride)?;
        need(near <= far && self.beings.is_empty())?;
        (self.near, self.far, self.stride) = (near, far, stride);
        Ok(obj! { "far" => far, "near" => near, "stride" => stride })
    }

    fn op_track(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "needs", "distance"], &["group"])?;
        let b = id_of(args, "being")?;
        let rows = c::get(args, "needs")?.as_array().ok_or_else(c::bad)?;
        need(!rows.is_empty() && rows.len() <= l.get("needs"))?;
        let mut needs = BTreeMap::new();
        for r in rows {
            keys(r, &["id", "rate"], &["level", "threshold"])?;
            let nid = id_of(r, "id")?;
            need(!needs.contains_key(&nid))?;
            let n = Need {
                level: opt_uint(r, "level", 0, M, 0)?,
                rate: uint_bound(c::get(r, "rate")?, 0, M)?,
                threshold: opt_uint(r, "threshold", 1, M, THRESHOLD)?,
            };
            needs.insert(nid, n);
        }
        let distance = uint_bound(c::get(args, "distance")?, 0, MAX_AT)?;
        let group = opt(args, "group").map(|v| ident(v).map(str::to_string)).transpose()?;
        need(!self.beings.contains_key(&b) && self.beings.len() < l.get("beings"))?;
        let detail = self.level_of(distance);
        if detail == "crowd" {
            let g = group.as_deref().ok_or_else(c::bad)?;
            need(self.group_room(g, None, l.get("groups")))?;
        }
        let x =
            Tracked { needs, at: self.clock, distance, detail, group: if detail == "crowd" { group } else { None } };
        let view = Self::view(&b, &x);
        self.index(&b, x.group.as_deref(), true);
        self.beings.insert(b, x);
        Ok(Value::Object(view))
    }

    fn op_move(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "distance"], &["group"])?;
        let b = id_of(args, "being")?;
        let distance = uint_bound(c::get(args, "distance")?, 0, MAX_AT)?;
        let group = opt(args, "group").map(|v| ident(v).map(str::to_string)).transpose()?;
        need(self.beings.contains_key(&b))?;
        let detail = self.level_of(distance);
        if detail == "crowd" {
            let g = group.as_deref().ok_or_else(c::bad)?;
            need(self.group_room(g, Some(&b), l.get("groups")))?;
        }
        let clock = self.clock;
        let old = self.beings.get(&b).and_then(|x| x.group.clone());
        self.index(&b, old.as_deref(), false);
        let group = if detail == "crowd" { group } else { None };
        self.index(&b, group.as_deref(), true);
        let x = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        let before = x.detail;
        let events = Self::advance(x, &b, clock);
        x.distance = distance;
        x.detail = detail;
        x.group = group;
        let view = Self::view(&b, x);
        Ok(obj! {
            "events" => Value::Array(events.iter().map(event_view).collect()), "from" => before, "to" => detail,
            "being" => Value::Object(view),
        })
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        let elapsed = now - self.clock;
        let stride = self.stride;
        let (mut full, mut coarse) = (0u128, 0u128);
        let mut events = Vec::new();
        for b in &self.active {
            let x = self.beings.get_mut(b).ok_or_else(c::bad)?;
            let n = x.needs.len() as u128;
            match x.detail {
                "full" => {
                    full += n * elapsed as u128;
                    events.extend(Self::advance(x, b, now));
                }
                "coarse" => {
                    let edge = now - now % stride;
                    if edge > x.at {
                        coarse += n * (edge - x.at).div_ceil(stride) as u128;
                        events.extend(Self::advance(x, b, edge));
                    }
                }
                _ => {}
            }
        }
        let crowd = if elapsed > 0 { self.groups.len() } else { 0 };
        self.clock = now;
        events.sort();
        Ok(obj! {
            "events" => Value::Array(events.iter().map(event_view).collect()), "now" => now,
            "work" => obj! { "coarse" => Value::Number(coarse.to_string()), "crowd" => crowd, "full" => Value::Number(full.to_string()) },
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        let x = self.beings.get(&b).ok_or_else(c::bad)?;
        let mut copy = x.clone();
        Self::advance(&mut copy, &b, self.clock);
        let mut fields = Self::view(&b, &copy);
        fields.push(("lag".into(), Value::from(self.clock - x.at)));
        Ok(Value::Object(fields))
    }

    fn op_crowd(&self, args: &Value) -> Result<Value> {
        keys(args, &["group"], &[])?;
        let g = id_of(args, "group")?;
        let mut totals: BTreeMap<&str, (u128, u64)> = BTreeMap::new();
        let (mut members, mut urgent) = (0u64, 0u64);
        for b in self.groups.get(&g).into_iter().flatten() {
            let x = self.beings.get(b).ok_or_else(c::bad)?;
            members += 1;
            for (nid, n) in &x.needs {
                let lvl = n.level_after(self.clock - x.at);
                let t = totals.entry(nid.as_str()).or_insert((0, 0));
                t.0 += lvl as u128;
                t.1 += 1;
                if lvl >= n.threshold {
                    urgent += 1;
                }
            }
        }
        need(members > 0)?;
        let needs = totals
            .iter()
            .map(
                |(k, (s, c))| obj! { "id" => *k, "mean" => Value::Number((s / *c as u128).to_string()), "count" => *c },
            )
            .collect();
        Ok(obj! { "group" => g, "members" => members, "needs" => Value::Array(needs), "urgent" => urgent })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Detail::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(m: &mut Detail, ops: &str) -> String {
        m.workflow(&parse(&format!(r#"{{"ops":[{ops}]}}"#), 1 << 16).unwrap()).unwrap().to_json()
    }

    #[test]
    fn every_level_reaches_the_same_state() {
        let mut levels = Vec::new();
        for d in [0, 50, 500] {
            let mut m = Detail::new();
            run(
                &mut m,
                &format!(
                    r#"{{"op":"track","args":{{"being":"b","distance":{d},"group":"g","needs":[{{"id":"n","rate":7,"level":3}}]}}}},
                       {{"op":"tick","args":{{"now":37}}}},{{"op":"tick","args":{{"now":95}}}},
                       {{"op":"move","args":{{"being":"b","distance":0}}}}"#
                ),
            );
            levels.push(m.beings["b"].needs["n"].level);
        }
        assert_eq!(levels, vec![3 + 7 * 95; 3]);
    }
}
