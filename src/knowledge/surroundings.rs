//! Environment cycles, place rules, aftermath traces and item provenance (pass 3, stage 5).
//!
//! **Environment.** `environment` declares a day of `day_length` ticks split into contiguous
//! phases (light `dark`, `dim` or `bright`, a temperature delta) and weather kinds (exterior
//! light shift, noise, travel cost, need rates, sought place tags); `weather` sets the current
//! weather; `conditions` reports the phase, light, temperature and weather effects and, for a
//! place, its effective access, crowding, earshot and whether secrets can be discussed there.
//!
//! **Places.** `place` declares an owner, an access rule (`public`, `customers_only`,
//! `members_only`, `private`, `restricted`, optionally different outside opening hours), a
//! privacy (`public`, `semi_private`, `private`, `echoing`), a comfortable capacity, an
//! exterior flag and named stations with a user limit. `enter` admits or refuses a being by the
//! access rule (a refusal is a trespass and changes nothing), `leave` frees its stations, and
//! `reserve`/`release` share a station first come, first served with a queue.
//!
//! **Traces.** `trace_kind` declares which actions leave a trace, which clear it, whether it is
//! seen at a glance or only on inspection, how long it lasts and what fact it implies. `act`
//! leaves and clears traces at a place, `perceive` lists what an observer can see there (an
//! inferred belief, never who did it), and `tick` lets traces fade.
//!
//! **Items.** `item` records condition, legal owner, possessor, maker, who holds it dear and how
//! long it keeps; `transfer` moves it by gift, sale, loan, return, taking, finding or dropping,
//! keeping the legal owner apart from whoever holds it; `damage` lowers its condition;
//! `inspect_item` tells an observer whether they are looking at their own missing property and
//! how much a loss would weigh.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::knowledge::surroundings::Surroundings;
//!
//! let mut s = Surroundings::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "place", "args": {"id": "bakery", "owner": "mira", "privacy": "semi_private"}},
//!   {"op": "trace_kind", "args": {"id": "flour_trail", "created_by": ["drop_sack"],
//!     "cleared_by": ["sweep"], "visibility": "public", "ttl": 60, "implies": "flour_spilled"}},
//!   {"op": "act", "args": {"action": "drop_sack", "place": "bakery"}},
//!   {"op": "tick", "args": {"now": 5}},
//!   {"op": "perceive", "args": {"place": "bakery"}},
//!   {"op": "item", "args": {"id": "coin_pouch", "owner": "mira"}},
//!   {"op": "transfer", "args": {"item": "coin_pouch", "how": "take", "to": "apprentice"}},
//!   {"op": "inspect_item", "args": {"item": "coin_pouch", "observer": "mira"}}
//! ]}"#, 4096)?;
//! let out = s.workflow(&req)?.to_json();
//! // Mira sees the flour (an inference, five ticks old) and knows her own pouch in other hands.
//! assert!(out.contains(r#""age":5,"belief":"inferred","implies":"flour_spilled""#));
//! assert!(out.contains(r#""status":"stolen""#));
//! assert!(out.contains(r#""theft_appraisal":true"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_bool, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, ascending, durable, is_ident, is_text, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "places", default: 64, ceiling: 100_000 },
    Limit { name: "occupants", default: 32, ceiling: 10_000 },
    Limit { name: "trace_kinds", default: 64, ceiling: 4096 },
    Limit { name: "traces", default: 256, ceiling: 1_000_000 },
    Limit { name: "items", default: 256, ceiling: 1_000_000 },
];
pub const LIGHT: [&str; 3] = ["dark", "dim", "bright"];
pub const ACCESS: [&str; 5] = ["public", "customers_only", "members_only", "private", "restricted"];
/// Privacy with the earshot it implies.
pub const PRIVACY: [(&str, &str); 4] =
    [("public", "bystanders"), ("semi_private", "nearby"), ("private", "none"), ("echoing", "everyone")];
pub const HOW: [&str; 7] = ["gift", "sale", "loan", "return", "take", "find", "drop"];
pub const HELD_AS: [&str; 3] = ["borrowed", "stolen", "found"];

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

fn sint(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(c::bad),
        _ => Err(c::bad()),
    }
}

fn pick(v: &Value, options: &[&'static str]) -> Result<&'static str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    options.iter().copied().find(|x| *x == s).ok_or_else(c::bad)
}

fn one_of(args: &Value, k: &str, options: &[&'static str], default: &'static str) -> Result<&'static str> {
    match opt(args, k) {
        None => Ok(default),
        Some(v) => {
            ident(v)?;
            pick(v, options)
        }
    }
}

fn id_list(v: Option<&Value>, lo: usize, hi: usize) -> Result<Vec<String>> {
    let xs = list_bound(v, hi)?;
    need(xs.len() >= lo)?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for x in xs {
        let s = ident(x)?.to_string();
        need(seen.insert(s.clone()))?;
        out.push(s);
    }
    Ok(out)
}

fn label(cond: u64) -> &'static str {
    match cond {
        0 => "broken",
        1..=29 => "damaged",
        30..=69 => "worn",
        70..=94 => "intact",
        _ => "masterwork",
    }
}

fn strings(xs: &[String]) -> Value {
    Value::Array(xs.iter().map(|s| Value::from(s.as_str())).collect())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phase {
    pub id: String,
    pub start: u64,
    pub end: u64,
    pub light: usize,
    pub temp_delta: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Weather {
    pub light_delta: i64,
    pub noise: u64,
    pub travel_pct: u64,
    pub need_rates: BTreeMap<String, u64>,
    pub seek_tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    pub day_length: u64,
    pub phases: Vec<Phase>,
    pub weather: BTreeMap<String, Weather>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hours {
    pub start: u64,
    pub end: u64,
    pub outside: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Station {
    pub max_users: u64,
    pub users: Vec<String>,
    pub queue: Vec<String>,
}

impl Station {
    /// Removes a user or queued being; returns who moved up into the freed slot.
    fn free(&mut self, b: &str) -> Option<String> {
        if let Some(i) = self.queue.iter().position(|x| x == b) {
            self.queue.remove(i);
            return None;
        }
        self.users.retain(|x| x != b);
        if self.queue.is_empty() {
            return None;
        }
        let next = self.queue.remove(0);
        self.users.push(next.clone());
        Some(next)
    }

    fn holds(&self, b: &str) -> bool {
        self.users.iter().any(|x| x == b) || self.queue.iter().any(|x| x == b)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub owner: Option<String>,
    pub access: &'static str,
    pub hours: Option<Hours>,
    pub privacy: &'static str,
    pub capacity: Option<u64>,
    pub exterior: bool,
    pub stations: BTreeMap<String, Station>,
    pub occupants: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceKind {
    pub created_by: Vec<String>,
    pub cleared_by: Vec<String>,
    pub public: bool,
    pub ttl: u64,
    pub implies: String,
    pub sight: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub owner: Option<String>,
    pub possessor: Option<String>,
    pub held_as: &'static str,
    pub condition: u64,
    pub maker: Option<String>,
    pub sentimental_to: Vec<String>,
    pub perishable: Option<u64>,
    pub created: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Surroundings {
    pub clock: u64,
    pub env: Option<Environment>,
    pub weather: Option<String>,
    pub places: BTreeMap<String, Place>,
    pub kinds: BTreeMap<String, TraceKind>,
    /// (place, trace kind) → tick it was left.
    pub traces: BTreeMap<(String, String), u64>,
    pub items: BTreeMap<String, Item>,
    pub limits: Limits,
}

impl Default for Surroundings {
    fn default() -> Self {
        Surroundings {
            clock: 0,
            env: None,
            weather: None,
            places: BTreeMap::new(),
            kinds: BTreeMap::new(),
            traces: BTreeMap::new(),
            items: BTreeMap::new(),
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Surroundings {
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
            "environment" => self.op_environment(args),
            "weather" => self.op_weather(args),
            "conditions" => self.op_conditions(args),
            "place" => self.op_place(args, l),
            "enter" => self.op_enter(args, l),
            "leave" => self.op_leave(args),
            "reserve" => self.op_reserve(args),
            "release" => self.op_release(args),
            "trace_kind" => self.op_trace_kind(args, l),
            "act" => self.op_act(args, l),
            "perceive" => self.op_perceive(args),
            "item" => self.op_item(args, l),
            "transfer" => self.op_transfer(args),
            "damage" => self.op_damage(args),
            "inspect_item" => self.op_inspect_item(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    // ------------------------------------------------------------ time and environment

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        let gone: Vec<(String, String)> = self
            .traces
            .iter()
            .filter(|(k, at)| self.kinds.get(&k.1).is_some_and(|kind| now - **at >= kind.ttl))
            .map(|(k, _)| k.clone())
            .collect();
        let mut expired = Vec::new();
        for k in gone {
            self.traces.remove(&k);
            expired.push(obj! { "kind" => k.1, "place" => k.0 });
        }
        Ok(obj! { "expired" => Value::Array(expired), "now" => now })
    }

    fn op_environment(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["day_length", "phases"], &["weather"])?;
        let day = uint_bound(c::get(args, "day_length")?, 1, 1_000_000)?;
        let rows = c::get(args, "phases")?.as_array().ok_or_else(c::bad)?;
        need(!rows.is_empty() && rows.len() <= 24)?;
        let mut phases: Vec<Phase> = Vec::new();
        let mut edge = 0;
        for x in rows {
            keys(x, &["id", "start", "end", "light"], &["temp_delta"])?;
            let id = id_of(x, "id")?;
            let start = uint_bound(c::get(x, "start")?, 0, day)?;
            let end = uint_bound(c::get(x, "end")?, 1, day)?;
            let temp_delta = opt(x, "temp_delta").map(|v| sint(v, -100, 100)).transpose()?.unwrap_or(0);
            let light = pick(c::get(x, "light")?, &LIGHT)?;
            need(start == edge && end > start && phases.iter().all(|p| p.id != id))?;
            edge = end;
            let light = LIGHT.iter().position(|l| *l == light).unwrap_or(0);
            phases.push(Phase { id, start, end, light, temp_delta });
        }
        need(edge == day)?;
        let mut weather = BTreeMap::new();
        for x in list_bound(opt(args, "weather"), 16)? {
            keys(x, &["id"], &["light_delta", "noise", "travel_pct", "need_rates", "seek_tags"])?;
            let w = id_of(x, "id")?;
            need(!weather.contains_key(&w))?;
            let mut need_rates = BTreeMap::new();
            if let Some(r) = opt(x, "need_rates") {
                let r = r.as_object().ok_or_else(c::bad)?;
                need(r.len() <= 8)?;
                for (k, v) in r {
                    need_rates.insert(ident(&Value::from(k.as_str()))?.to_string(), uint_bound(v, 0, 100)?);
                }
            }
            weather.insert(
                w,
                Weather {
                    light_delta: opt(x, "light_delta").map(|v| sint(v, -2, 2)).transpose()?.unwrap_or(0),
                    noise: opt_uint(x, "noise", 0, 100, 0)?,
                    travel_pct: opt_uint(x, "travel_pct", 50, 500, 100)?,
                    need_rates,
                    seek_tags: id_list(opt(x, "seek_tags"), 0, 8)?,
                },
            );
        }
        let out = obj! {
            "day_length" => day,
            "phases" => Value::Array(phases.iter().map(|p| Value::from(p.id.as_str())).collect()),
            "weather" => Value::Array(weather.keys().map(|k| Value::from(k.as_str())).collect()),
        };
        if self.weather.as_ref().is_some_and(|w| !weather.contains_key(w)) {
            self.weather = None;
        }
        self.env = Some(Environment { day_length: day, phases, weather });
        Ok(out)
    }

    fn op_weather(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["kind"])?;
        let env = self.env.as_ref().ok_or_else(c::bad)?;
        let w = opt_id(args, "kind")?;
        need(w.as_ref().is_none_or(|w| env.weather.contains_key(w)))?;
        self.weather = w.clone();
        Ok(obj! { "weather" => w })
    }

    fn day_tick(&self) -> u64 {
        self.env.as_ref().map_or(self.clock, |e| self.clock % e.day_length)
    }

    fn access_now(&self, p: &Place) -> &'static str {
        match &p.hours {
            None => p.access,
            Some(h) => {
                let t = self.day_tick();
                let inside = if h.start < h.end { h.start <= t && t < h.end } else { t >= h.start || t < h.end };
                if inside {
                    p.access
                } else {
                    h.outside
                }
            }
        }
    }

    fn place_view(&self, pid: &str, p: &Place) -> Value {
        let n = p.occupants.len() as u64;
        let earshot = PRIVACY.iter().find(|x| x.0 == p.privacy).map_or("bystanders", |x| x.1);
        obj! {
            "access" => self.access_now(p),
            "crowded" => p.capacity.is_some_and(|cap| n > cap),
            "discuss_secrets" => p.privacy == "private" || (p.privacy == "semi_private" && n <= 2),
            "earshot" => earshot,
            "id" => pid,
            "occupants" => n,
        }
    }

    fn op_conditions(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &["place"])?;
        let env = self.env.as_ref().ok_or_else(c::bad)?;
        let pid = opt_id(args, "place")?;
        let place = match &pid {
            None => None,
            Some(p) => Some(self.places.get(p).ok_or_else(c::bad)?),
        };
        let t = self.day_tick();
        let ph = env.phases.iter().find(|p| p.start <= t && t < p.end).ok_or_else(c::bad)?;
        let exterior = place.is_none_or(|p| p.exterior);
        let w = self.weather.as_ref().and_then(|w| env.weather.get(w));
        let mut light = ph.light as i64;
        let (noise, rates, tags) = match w.filter(|_| exterior) {
            Some(w) => {
                light = (light + w.light_delta).clamp(0, 2);
                let rates = w.need_rates.iter().map(|(k, v)| (k.clone(), Value::from(*v))).collect();
                (w.noise, rates, strings(&w.seek_tags))
            }
            None => (0, Vec::new(), Value::Array(Vec::new())),
        };
        Ok(obj! {
            "day_tick" => t,
            "exterior" => exterior,
            "light" => LIGHT[light as usize],
            "need_rates" => Value::Object(rates),
            "noise" => noise,
            "phase" => ph.id.as_str(),
            "place" => match (&pid, place) {
                (Some(id), Some(p)) => self.place_view(id, p),
                _ => Value::Null,
            },
            "seek_tags" => tags,
            "temp_delta" => ph.temp_delta,
            "travel_pct" => w.map_or(100, |w| w.travel_pct),
            "weather" => self.weather.clone(),
        })
    }

    // ------------------------------------------------------------ places

    fn op_place(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id"], &["owner", "access", "hours", "privacy", "capacity", "exterior", "stations"])?;
        let pid = id_of(args, "id")?;
        let owner = opt_id(args, "owner")?;
        let access = one_of(args, "access", &ACCESS, "public")?;
        let hours = match opt(args, "hours") {
            None => None,
            Some(h) => {
                keys(h, &["start", "end", "outside"], &[])?;
                need(self.env.is_some())?;
                let start = uint_bound(c::get(h, "start")?, 0, 1_000_000)?;
                let end = uint_bound(c::get(h, "end")?, 0, 1_000_000)?;
                let outside = pick(c::get(h, "outside")?, &ACCESS)?;
                need(start != end)?;
                Some(Hours { start, end, outside })
            }
        };
        let privacy = one_of(args, "privacy", &PRIVACY.map(|p| p.0), "public")?;
        let capacity = opt(args, "capacity").map(|v| uint_bound(v, 1, 10_000)).transpose()?;
        let exterior = opt_bool(args, "exterior", false)?;
        let mut stations = BTreeMap::new();
        for x in list_bound(opt(args, "stations"), 16)? {
            keys(x, &["id"], &["max_users"])?;
            let s = id_of(x, "id")?;
            need(!stations.contains_key(&s))?;
            let max_users = opt_uint(x, "max_users", 1, 64, 1)?;
            stations.insert(s, Station { max_users, users: Vec::new(), queue: Vec::new() });
        }
        match self.places.get(&pid) {
            Some(old) => need(old.occupants.is_empty())?,
            None => need(self.places.len() < l.get("places"))?,
        }
        let out = obj! {
            "access" => access,
            "id" => pid.as_str(),
            "privacy" => privacy,
            "stations" => Value::Array(stations.keys().map(|k| Value::from(k.as_str())).collect()),
        };
        self.places
            .insert(pid, Place { owner, access, hours, privacy, capacity, exterior, stations, occupants: Vec::new() });
        Ok(out)
    }

    fn op_enter(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["place", "being"], &["customer", "member", "invited"])?;
        let pid = id_of(args, "place")?;
        let b = id_of(args, "being")?;
        let customer = opt_bool(args, "customer", false)?;
        let member = opt_bool(args, "member", false)?;
        let invited = opt_bool(args, "invited", false)?;
        let p = self.places.get(&pid).ok_or_else(c::bad)?;
        let access = self.access_now(p);
        let own = p.owner.as_deref() == Some(b.as_str());
        let allowed = match access {
            "public" => true,
            "customers_only" => customer || member || own,
            "members_only" => member || own,
            "private" => own || invited,
            _ => own,
        };
        if allowed && !p.occupants.contains(&b) {
            need(p.occupants.len() < l.get("occupants"))?;
            if let Some(p) = self.places.get_mut(&pid) {
                p.occupants.push(b.clone());
            }
        }
        let p = self.places.get(&pid).ok_or_else(c::bad)?;
        let n = p.occupants.len() as u64;
        Ok(obj! {
            "access" => access,
            "allowed" => allowed,
            "being" => b,
            "crowded" => p.capacity.is_some_and(|cap| n > cap),
            "occupants" => n,
            "place" => pid,
            "trespass" => !allowed,
        })
    }

    fn op_leave(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["place", "being"], &[])?;
        let pid = id_of(args, "place")?;
        let b = id_of(args, "being")?;
        let p = self.places.get_mut(&pid).ok_or_else(c::bad)?;
        need(p.occupants.contains(&b))?;
        p.occupants.retain(|x| *x != b);
        let mut promoted = Vec::new();
        for (sid, s) in p.stations.iter_mut() {
            if s.holds(&b) {
                if let Some(next) = s.free(&b) {
                    promoted.push(obj! { "being" => next, "station" => sid.as_str() });
                }
            }
        }
        Ok(obj! {
            "being" => b,
            "occupants" => p.occupants.len(),
            "place" => pid,
            "promoted" => Value::Array(promoted),
        })
    }

    fn op_reserve(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["place", "station", "being"], &[])?;
        let pid = id_of(args, "place")?;
        let sid = id_of(args, "station")?;
        let b = id_of(args, "being")?;
        let p = self.places.get_mut(&pid).ok_or_else(c::bad)?;
        need(p.occupants.contains(&b))?;
        let s = p.stations.get_mut(&sid).ok_or_else(c::bad)?;
        need(!s.holds(&b))?;
        let (granted, pos) = if (s.users.len() as u64) < s.max_users {
            s.users.push(b.clone());
            (true, 0)
        } else {
            s.queue.push(b.clone());
            (false, s.queue.len())
        };
        Ok(obj! { "being" => b, "granted" => granted, "queue_position" => pos, "station" => sid })
    }

    fn op_release(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["place", "station", "being"], &[])?;
        let pid = id_of(args, "place")?;
        let sid = id_of(args, "station")?;
        let b = id_of(args, "being")?;
        let p = self.places.get_mut(&pid).ok_or_else(c::bad)?;
        let s = p.stations.get_mut(&sid).ok_or_else(c::bad)?;
        need(s.holds(&b))?;
        let promoted = s.free(&b);
        Ok(obj! { "being" => b, "promoted" => promoted, "station" => sid })
    }

    // ------------------------------------------------------------ traces

    fn op_trace_kind(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "created_by", "visibility", "ttl", "implies"], &["cleared_by", "sight"])?;
        let k = id_of(args, "id")?;
        let created_by = id_list(Some(c::get(args, "created_by")?), 1, 8)?;
        let cleared_by = id_list(opt(args, "cleared_by"), 0, 8)?;
        let public = pick(c::get(args, "visibility")?, &["public", "inspect"])? == "public";
        let ttl = uint_bound(c::get(args, "ttl")?, 1, MAX_AT)?;
        let implies = id_of(args, "implies")?;
        let sight = opt(args, "sight").map(|v| text_bound(v, 160).map(str::to_string)).transpose()?;
        need(self.kinds.contains_key(&k) || self.kinds.len() < l.get("trace_kinds"))?;
        self.kinds.insert(k.clone(), TraceKind { created_by, cleared_by, public, ttl, implies, sight });
        Ok(obj! { "id" => k })
    }

    fn op_act(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["action", "place"], &[])?;
        let act = id_of(args, "action")?;
        let pid = id_of(args, "place")?;
        need(self.places.contains_key(&pid))?;
        let cleared: Vec<String> = self
            .kinds
            .iter()
            .filter(|(k, kind)| {
                kind.cleared_by.contains(&act) && self.traces.contains_key(&(pid.clone(), (*k).clone()))
            })
            .map(|(k, _)| k.clone())
            .collect();
        let created: Vec<String> =
            self.kinds.iter().filter(|(_, kind)| kind.created_by.contains(&act)).map(|(k, _)| k.clone()).collect();
        let after = self.traces.len() - cleared.len();
        let growth = created
            .iter()
            .filter(|k| !self.traces.contains_key(&(pid.clone(), (*k).clone())) || cleared.contains(k))
            .count();
        need(growth == 0 || after + growth <= l.get("traces"))?;
        for k in &cleared {
            self.traces.remove(&(pid.clone(), k.clone()));
        }
        for k in &created {
            self.traces.insert((pid.clone(), k.clone()), self.clock);
        }
        Ok(obj! { "cleared" => strings(&cleared), "created" => strings(&created), "place" => pid })
    }

    fn op_perceive(&self, args: &Value) -> Result<Value> {
        keys(args, &["place"], &["inspect"])?;
        let pid = id_of(args, "place")?;
        need(self.places.contains_key(&pid))?;
        let look = opt_bool(args, "inspect", false)?;
        let mut seen = Vec::new();
        let mut hidden = 0u64;
        for ((p, k), at) in self.traces.range((pid.clone(), String::new())..) {
            if *p != pid {
                break;
            }
            let Some(kind) = self.kinds.get(k) else { continue };
            let age = self.clock - at;
            if age >= kind.ttl {
                continue;
            }
            if kind.public || look {
                seen.push(obj! {
                    "age" => age,
                    "belief" => "inferred",
                    "implies" => kind.implies.as_str(),
                    "kind" => k.as_str(),
                    "sight" => kind.sight.clone(),
                });
            } else {
                hidden += 1;
            }
        }
        Ok(obj! { "hidden" => hidden, "place" => pid, "traces" => Value::Array(seen) })
    }

    // ------------------------------------------------------------ items

    fn item_view(&self, iid: &str) -> Result<Value> {
        let it = self.items.get(iid).ok_or_else(c::bad)?;
        let status = match (&it.owner, &it.possessor) {
            (None, _) => "unclaimed",
            (Some(o), Some(p)) if o == p => "owned",
            (Some(_), None) => "lost",
            _ => it.held_as,
        };
        Ok(obj! {
            "condition" => it.condition,
            "condition_label" => label(it.condition),
            "id" => iid,
            "maker" => it.maker.clone(),
            "owner" => it.owner.clone(),
            "possessor" => it.possessor.clone(),
            "sentimental_to" => strings(&it.sentimental_to),
            "spoiled" => it.perishable.is_some_and(|p| self.clock - it.created >= p),
            "status" => status,
        })
    }

    fn op_item(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id"], &["owner", "possessor", "held_as", "condition", "maker", "sentimental_to", "perishable"])?;
        let iid = id_of(args, "id")?;
        let owner = opt_id(args, "owner")?;
        let possessor = if args.get("possessor").is_some() { opt_id(args, "possessor")? } else { owner.clone() };
        let held_as = one_of(args, "held_as", &HELD_AS, "borrowed")?;
        let condition = opt_uint(args, "condition", 0, 100, 100)?;
        let maker = opt_id(args, "maker")?;
        let mut sentimental_to = id_list(opt(args, "sentimental_to"), 0, 8)?;
        sentimental_to.sort();
        let perishable = opt(args, "perishable").map(|v| uint_bound(v, 1, MAX_AT)).transpose()?;
        need(self.items.contains_key(&iid) || self.items.len() < l.get("items"))?;
        let it = Item { owner, possessor, held_as, condition, maker, sentimental_to, perishable, created: self.clock };
        self.items.insert(iid.clone(), it);
        self.item_view(&iid)
    }

    fn op_transfer(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["item", "how"], &["to"])?;
        let iid = id_of(args, "item")?;
        let it = self.items.get_mut(&iid).ok_or_else(c::bad)?;
        let how = pick(c::get(args, "how")?, &HOW)?;
        let to = opt_id(args, "to")?;
        let own = it.owner.clone();
        let poss = it.possessor.clone();
        if how == "drop" {
            need(to.is_none() && poss.is_some())?;
            it.possessor = None;
            return self.item_view(&iid);
        }
        let to = to.ok_or_else(c::bad)?;
        need(poss.as_ref() != Some(&to))?;
        match how {
            "gift" | "sale" => {
                need(poss.is_some())?;
                if own.is_none() || own == poss {
                    it.owner = Some(to.clone());
                }
                it.possessor = Some(to);
            }
            "loan" => {
                need(own.is_some() && own == poss)?;
                it.possessor = Some(to);
                it.held_as = "borrowed";
            }
            "return" => {
                need(own.as_ref() == Some(&to))?;
                it.possessor = Some(to);
            }
            "take" => {
                need(poss.is_some())?;
                if own.as_ref().is_some_and(|o| *o != to) {
                    it.held_as = "stolen";
                }
                it.possessor = Some(to);
            }
            _ => {
                need(poss.is_none())?;
                if own.as_ref().is_some_and(|o| *o != to) {
                    it.held_as = "found";
                }
                it.possessor = Some(to);
            }
        }
        self.item_view(&iid)
    }

    fn op_damage(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["item", "amount"], &[])?;
        let iid = id_of(args, "item")?;
        need(self.items.contains_key(&iid))?;
        let amount = uint_bound(c::get(args, "amount")?, 1, 100)?;
        if let Some(it) = self.items.get_mut(&iid) {
            it.condition = it.condition.saturating_sub(amount);
        }
        self.item_view(&iid)
    }

    fn op_inspect_item(&self, args: &Value) -> Result<Value> {
        keys(args, &["item", "observer"], &[])?;
        let iid = id_of(args, "item")?;
        let obs = id_of(args, "observer")?;
        let view = self.item_view(&iid)?;
        let it = self.items.get(&iid).ok_or_else(c::bad)?;
        let status = view.get("status").and_then(Value::as_str).unwrap_or("");
        let theft = it.owner.as_deref() == Some(obs.as_str())
            && it.possessor.as_ref().is_some_and(|p| *p != obs)
            && status != "borrowed";
        let weight: u64 = if it.sentimental_to.contains(&obs) { 300 } else { 100 };
        Ok(obj! { "item" => view, "loss_weight_pct" => weight, "observer" => obs, "theft_appraisal" => theft })
    }
}

/// Privacy words.
pub const PRIVACY_NAMES: [&str; 4] = ["public", "semi_private", "private", "echoing"];
persist!(Phase { end [1, 1_000_000], id, light [0, 2], start [0, 1_000_000], temp_delta [-100, 100] });
persist!(Weather { light_delta [-2, 2], need_rates, noise [0, 100], seek_tags, travel_pct [50, 500] });
persist!(Environment { day_length [1, 1_000_000], phases, weather });
persist!(Hours { end [0, 1_000_000], outside in ACCESS, start [0, 1_000_000] });
persist!(Station { max_users [1, 64], queue, users });
persist!(Place {
    access in ACCESS, capacity [1, 10_000], exterior, hours, occupants, owner, privacy in PRIVACY_NAMES, stations,
});
persist!(TraceKind { cleared_by, created_by, implies, public, sight, ttl [1, MAX_AT] });
persist!(Item {
    condition [0, 100], created [0, MAX_AT], held_as in HELD_AS, maker, owner, perishable [1, MAX_AT], possessor,
    sentimental_to,
});
persist!(rest Surroundings { clock [0, MAX_AT], env, items, kinds, places, traces, weather });
durable!(Surroundings, "surroundings", LIMITS);

fn distinct_ids(xs: &[String], lo: usize, hi: usize) -> bool {
    (lo..=hi).contains(&xs.len())
        && xs.iter().all(|x| is_ident(x))
        && xs.iter().collect::<BTreeSet<_>>().len() == xs.len()
}

fn opt_id_ok(x: &Option<String>) -> bool {
    x.as_deref().is_none_or(is_ident)
}

impl Surroundings {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        if let Some(env) = &self.env {
            ok((1..=24).contains(&env.phases.len()) && env.weather.len() <= 16)?;
            let mut edge = 0;
            let mut ids = BTreeSet::new();
            for p in &env.phases {
                ok(is_ident(&p.id) && ids.insert(&p.id) && p.start == edge && p.end > p.start)?;
                edge = p.end;
            }
            ok(edge == env.day_length)?;
            for (w, x) in &env.weather {
                ok(is_ident(w) && x.need_rates.len() <= 8 && distinct_ids(&x.seek_tags, 0, 8))?;
                ok(x.need_rates.iter().all(|(k, v)| is_ident(k) && *v <= 100))?;
            }
        }
        ok(self.weather.as_ref().is_none_or(|w| self.env.as_ref().is_some_and(|e| e.weather.contains_key(w))))?;
        within::<Self>("places", self.places.len())?;
        for (pid, p) in &self.places {
            ok(is_ident(pid) && opt_id_ok(&p.owner) && p.stations.len() <= 16)?;
            ok(p.hours.as_ref().is_none_or(|h| h.start != h.end && self.env.is_some()))?;
            within::<Self>("occupants", p.occupants.len())?;
            ok(distinct_ids(&p.occupants, 0, usize::MAX))?;
            for (sid, s) in &p.stations {
                ok(is_ident(sid) && s.users.len() as u64 <= s.max_users)?;
                ok(s.queue.is_empty() || s.users.len() as u64 == s.max_users)?;
                let mut all = s.users.clone();
                all.extend(s.queue.iter().cloned());
                ok(distinct_ids(&all, 0, usize::MAX) && all.iter().all(|b| p.occupants.contains(b)))?;
            }
        }
        within::<Self>("trace_kinds", self.kinds.len())?;
        for (k, x) in &self.kinds {
            ok(is_ident(k) && distinct_ids(&x.created_by, 1, 8) && distinct_ids(&x.cleared_by, 0, 8))?;
            ok(is_ident(&x.implies) && x.sight.as_deref().is_none_or(|s| is_text(s, 160)))?;
        }
        within::<Self>("traces", self.traces.len())?;
        for ((pid, k), at) in &self.traces {
            ok(self.places.contains_key(pid) && self.kinds.contains_key(k) && *at <= self.clock)?;
        }
        within::<Self>("items", self.items.len())?;
        for (iid, it) in &self.items {
            ok(is_ident(iid) && opt_id_ok(&it.owner) && opt_id_ok(&it.possessor) && opt_id_ok(&it.maker))?;
            ok(distinct_ids(&it.sentimental_to, 0, 8) && ascending(&it.sentimental_to) && it.created <= self.clock)?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

/// Runs one request on a fresh [`Surroundings`] (local kind `surroundings`).
pub fn workflow(request: &Value) -> Result<Value> {
    Surroundings::new().workflow(request)
}
