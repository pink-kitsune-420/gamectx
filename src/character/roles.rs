//! Status, roles and jobs (pass 2, B7).
//!
//! A role (guard, baker, elder) has a status 0..100, optional seats, and duties that recur every
//! `every` ticks from tick `at`. Beings are assigned roles. `tick` proposes one task per duty that
//! came due since the holder last ran it (reporting how many earlier times were `missed` during a
//! gap) and lists roles with fewer holders than seats. A being's status is its highest role
//! status plus earned standing (`esteem`, −50..50), clamped to 0..100. Status feeds conflicts
//! (`contest`: an edge in signed millionths and who defers at a gap of 20) and persuasion
//! (`persuade`: a chance adjusted by the status edge and a shared role). Tasks are proposals for
//! the task engine. Capacities are [host-set limits](crate::character::limits). Pure host
//! computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::roles::Roles;
//!
//! let mut town = Roles::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "role", "args": {"id": "guard", "status": 60, "seats": 2,
//!                           "duties": [{"id": "patrol", "action": "travel", "every": 8, "place": "wall"}]}},
//!   {"op": "role", "args": {"id": "baker", "status": 30}},
//!   {"op": "assign", "args": {"being": "tomas", "role": "guard"}},
//!   {"op": "assign", "args": {"being": "mira", "role": "baker"}},
//!   {"op": "tick", "args": {"now": 20}},
//!   {"op": "contest", "args": {"a": "tomas", "b": "mira"}}
//! ]}"#, 4096)?;
//! let out = town.workflow(&req)?.to_json();
//! assert!(out.contains(r#""id":"guard-patrol-16","missed":1"#));
//! assert!(out.contains(r#""defers":"mira","edge":300000"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "roles", default: 64, ceiling: 4096 },
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "duties", default: 8, ceiling: 64 },
    Limit { name: "held", default: 4, ceiling: 16 },
];
pub const M: i64 = 1_000_000;
/// Status gap at which the lower being defers in a contest.
pub const DEFER_AT: i64 = 20;
/// Contest edge per status point (millionths).
pub const EDGE_CONTEST: i64 = 10_000;
/// Persuasion edge per status point (millionths).
pub const EDGE_PERSUADE: i64 = 5_000;
/// Persuasion bonus when speaker and listener share a role (millionths).
pub const SHARED_ROLE: i64 = 50_000;

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
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(c::bad),
        _ => Err(c::bad()),
    }
}

/// Number of due times `at + k·every` that are `<= x`.
fn fired(at: u64, every: u64, x: u64) -> u64 {
    if x < at {
        0
    } else {
        (x - at) / every + 1
    }
}

/// A recurring duty of a role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Duty {
    pub action: String,
    pub every: u64,
    pub at: u64,
    pub place: Option<String>,
    pub priority: u64,
}

/// A role: status, seats and duties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub status: i64,
    pub seats: Option<u64>,
    pub duties: BTreeMap<String, Duty>,
}

/// A being's roles (with the tick each was last run) and earned standing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Holder {
    pub held: BTreeMap<String, u64>,
    pub standing: i64,
}

/// Roles, beings and the clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roles {
    pub roles: BTreeMap<String, Role>,
    pub beings: BTreeMap<String, Holder>,
    /// Holders of each role (an index kept in step with `beings`).
    pub holders: BTreeMap<String, BTreeSet<String>>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Roles {
    fn default() -> Self {
        Roles {
            roles: BTreeMap::new(),
            beings: BTreeMap::new(),
            holders: BTreeMap::new(),
            clock: 0,
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Roles {
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
            "role" => self.op_role(args, l),
            "assign" => self.op_assign(args, l),
            "vacate" => self.op_vacate(args),
            "esteem" => self.op_esteem(args, l),
            "tick" => self.op_tick(args),
            "contest" => self.op_contest(args),
            "persuade" => self.op_persuade(args),
            "assess" => self.op_assess(args),
            "staff" => self.op_staff(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    /// Highest held role status plus standing, clamped to 0..100.
    pub fn status(&self, b: &str) -> i64 {
        let Some(x) = self.beings.get(b) else {
            return 0;
        };
        let top = x.held.keys().filter_map(|r| self.roles.get(r)).map(|r| r.status).max().unwrap_or(0);
        (top + x.standing).clamp(0, 100)
    }

    fn view(&self, b: &str) -> Vec<(String, Value)> {
        let x = &self.beings[b];
        vec![
            ("being".into(), Value::from(b)),
            ("roles".into(), Value::Array(x.held.keys().map(|r| Value::from(r.as_str())).collect())),
            ("standing".into(), Value::from(x.standing)),
            ("status".into(), Value::from(self.status(b))),
        ]
    }

    fn holder_count(&self, r: &str) -> u64 {
        self.holders.get(r).map_or(0, |h| h.len() as u64)
    }

    fn op_role(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "status"], &["duties", "seats"])?;
        let id = id_of(args, "id")?;
        let status = uint_bound(c::get(args, "status")?, 0, 100)? as i64;
        let seats = opt(args, "seats").map(|v| uint_bound(v, 1, 1_000_000)).transpose()?;
        let rows = match opt(args, "duties") {
            Some(v) => v.as_array().ok_or_else(c::bad)?,
            None => &[],
        };
        need(rows.len() <= l.get("duties"))?;
        let mut duties = BTreeMap::new();
        for d in rows {
            keys(d, &["id", "action", "every"], &["at", "place", "priority"])?;
            let did = id_of(d, "id")?;
            need(!duties.contains_key(&did))?;
            let duty = Duty {
                action: id_of(d, "action")?,
                every: uint_bound(c::get(d, "every")?, 1, MAX_AT)?,
                at: opt_uint(d, "at", 0, MAX_AT, 0)?,
                place: opt(d, "place").map(|v| ident(v).map(str::to_string)).transpose()?,
                priority: opt_uint(d, "priority", 0, 100, 50)?,
            };
            duties.insert(did, duty);
        }
        need(self.roles.contains_key(&id) || self.roles.len() < l.get("roles"))?;
        let names: Vec<Value> = duties.keys().map(|k| Value::from(k.as_str())).collect();
        self.roles.insert(id.clone(), Role { status, seats, duties });
        Ok(obj! { "duties" => Value::Array(names), "id" => id, "seats" => Value::from(seats), "status" => status })
    }

    fn op_assign(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "role"], &[])?;
        let b = id_of(args, "being")?;
        let r = id_of(args, "role")?;
        let role = self.roles.get(&r).ok_or_else(c::bad)?;
        match self.beings.get(&b) {
            Some(x) => need(!x.held.contains_key(&r) && x.held.len() < l.get("held"))?,
            None => need(self.beings.len() < l.get("beings"))?,
        }
        need(role.seats.is_none_or(|s| self.holder_count(&r) < s))?;
        let clock = self.clock;
        self.beings.entry(b.clone()).or_default().held.insert(r.clone(), clock);
        self.holders.entry(r).or_default().insert(b.clone());
        Ok(Value::Object(self.view(&b)))
    }

    fn op_vacate(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "role"], &[])?;
        let b = id_of(args, "being")?;
        let r = id_of(args, "role")?;
        let x = self.beings.get_mut(&b).ok_or_else(c::bad)?;
        need(x.held.remove(&r).is_some())?;
        if let Some(h) = self.holders.get_mut(&r) {
            h.remove(&b);
            if h.is_empty() {
                self.holders.remove(&r);
            }
        }
        Ok(Value::Object(self.view(&b)))
    }

    fn op_esteem(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "delta", "reason"], &[])?;
        let b = id_of(args, "being")?;
        let delta = sint(c::get(args, "delta")?, -20, 20)?;
        let reason = id_of(args, "reason")?;
        need(self.beings.contains_key(&b) || self.beings.len() < l.get("beings"))?;
        let x = self.beings.entry(b.clone()).or_default();
        x.standing = (x.standing + delta).clamp(-50, 50);
        let mut v = self.view(&b);
        v.insert(1, ("reason".into(), Value::from(reason)));
        Ok(Value::Object(v))
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        let (mut tasks, mut effects) = (Vec::new(), Vec::new());
        for (b, x) in self.beings.iter_mut() {
            for (r, since) in x.held.iter_mut() {
                let role = self.roles.get(r).ok_or_else(c::bad)?;
                for (did, d) in &role.duties {
                    let due = fired(d.at, d.every, now);
                    let n = due.saturating_sub(fired(d.at, d.every, *since));
                    if n == 0 {
                        continue;
                    }
                    let last = d.at + (due - 1) * d.every;
                    let tid = format!("{r}-{did}-{last}");
                    let place = d.place.as_deref().map(Value::from).unwrap_or(Value::Null);
                    tasks.push(obj! {
                        "action" => d.action.as_str(), "at" => last, "being" => b.as_str(), "duty" => did.as_str(),
                        "id" => tid.as_str(), "missed" => n - 1, "place" => place, "priority" => d.priority,
                        "role" => r.as_str(),
                    });
                    let mut step = vec![
                        ("id".to_string(), Value::from(did.as_str())),
                        ("action".to_string(), Value::from(d.action.as_str())),
                    ];
                    if let Some(p) = &d.place {
                        step.push(("target_bindings".into(), obj! { "place" => p.as_str() }));
                    }
                    effects.push(obj! {
                        "engine" => "task", "op" => "propose",
                        "args" => obj! { "id" => tid.as_str(), "owner" => b.as_str(), "steps" => Value::Array(vec![Value::Object(step)]) },
                    });
                }
                *since = now;
            }
        }
        self.clock = now;
        let unstaffed: Vec<Value> = self
            .roles
            .iter()
            .filter_map(|(r, role)| {
                let h = self.holder_count(r);
                role.seats.filter(|s| h < *s).map(|s| obj! { "holders" => h, "role" => r.as_str(), "seats" => s })
            })
            .collect();
        Ok(obj! {
            "effects" => Value::Array(effects), "now" => now, "tasks" => Value::Array(tasks),
            "unstaffed" => Value::Array(unstaffed),
        })
    }

    fn op_contest(&self, args: &Value) -> Result<Value> {
        keys(args, &["a", "b"], &[])?;
        let a = id_of(args, "a")?;
        let b = id_of(args, "b")?;
        need(a != b && self.beings.contains_key(&a) && self.beings.contains_key(&b))?;
        let (sa, sb) = (self.status(&a), self.status(&b));
        let defers = if sa - sb >= DEFER_AT {
            Value::from(b.as_str())
        } else if sb - sa >= DEFER_AT {
            Value::from(a.as_str())
        } else {
            Value::Null
        };
        Ok(obj! {
            "a" => a.as_str(), "b" => b.as_str(), "defers" => defers, "edge" => (sa - sb) * EDGE_CONTEST,
            "status_a" => sa, "status_b" => sb,
        })
    }

    fn op_persuade(&self, args: &Value) -> Result<Value> {
        keys(args, &["speaker", "listener", "chance"], &[])?;
        let s = id_of(args, "speaker")?;
        let l = id_of(args, "listener")?;
        let base = uint_bound(c::get(args, "chance")?, 0, M as u64)? as i64;
        let (xs, xl) = match (self.beings.get(&s), self.beings.get(&l)) {
            (Some(xs), Some(xl)) if s != l => (xs, xl),
            _ => return Err(c::bad()),
        };
        let edge = (self.status(&s) - self.status(&l)) * EDGE_PERSUADE;
        let shared: Vec<Value> =
            xs.held.keys().filter(|r| xl.held.contains_key(*r)).map(|r| Value::from(r.as_str())).collect();
        let bonus = if shared.is_empty() { 0 } else { SHARED_ROLE };
        Ok(obj! {
            "base" => base, "chance" => (base + edge + bonus).clamp(0, M), "listener" => l.as_str(),
            "shared_roles" => Value::Array(shared), "speaker" => s.as_str(), "status_edge" => edge,
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        need(self.beings.contains_key(&b))?;
        Ok(Value::Object(self.view(&b)))
    }

    fn op_staff(&self, args: &Value) -> Result<Value> {
        keys(args, &["role"], &[])?;
        let r = id_of(args, "role")?;
        let role = self.roles.get(&r).ok_or_else(c::bad)?;
        let holders: Vec<Value> = self.holders.get(&r).into_iter().flatten().map(|b| Value::from(b.as_str())).collect();
        Ok(
            obj! { "holders" => Value::Array(holders), "role" => r.as_str(), "seats" => Value::from(role.seats), "status" => role.status },
        )
    }
}

persist!(Duty { action, at [0, MAX_AT], every [1, MAX_AT], place, priority [0, 100] });
persist!(Role { duties, seats [1, 1_000_000], status [0, 100] });
persist!(Holder { held, standing [-50, 50] });
persist!(rest Roles { beings, clock [0, MAX_AT], roles });
durable!(Roles, "roles", LIMITS);

impl Roles {
    /// Invariants a save must meet; the holders index is rebuilt.
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("roles", self.roles.len())?;
        within::<Self>("beings", self.beings.len())?;
        for (id, r) in &self.roles {
            ok(is_ident(id))?;
            within::<Self>("duties", r.duties.len())?;
            let duty_ok = |(d, x): (&String, &Duty)| {
                is_ident(d) && is_ident(&x.action) && x.place.as_deref().is_none_or(is_ident)
            };
            ok(r.duties.iter().all(duty_ok))?;
        }
        self.holders.clear();
        for (b, x) in &self.beings {
            ok(is_ident(b))?;
            within::<Self>("held", x.held.len())?;
            for (r, since) in &x.held {
                ok(self.roles.contains_key(r) && *since <= self.clock)?;
                self.holders.entry(r.clone()).or_default().insert(b.clone());
            }
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Roles::new().workflow(req)
}
