//! Unified turn frame and salience ranker (pass 3, stage 1).
//!
//! Three host faculties put a character in the moment:
//!
//! - **Trajectory sense.** `observe` keeps a bounded window of recent values per being and key
//!   (trust toward someone, mood); `trend` reports the change over a window and labels it
//!   (`acute_rise` / `acute_drop` past a shock size, `settled_high` / `settled_low` when a strong
//!   value has not changed for a while, `rising`, `falling`, `steady`); `outcome` and `streak`
//!   count consecutive successes or failures (`flow`, `frustration_spiral`); `time` turns a tick
//!   gap into a human anchor (`just_now` .. `long_ago`, or `due_in` / `overdue`).
//! - **Turn frame.** `frame` takes the slices the host read from the other engines (body,
//!   motive, task, affect, listener model, off-screen, novelty, bystanders) and returns one
//!   bounded, validated struct with derived fields: urgent body items, whether the motive
//!   overrides talk, task pressure, time anchors, this being's trends and the untrusted
//!   bystanders in earshot.
//! - **Salience ranker.** `rank` scores each candidate context item 0..1000 from urgency,
//!   listener relevance, utterance cues, task relevance, mood congruence and recency, then
//!   packs whole causal groups by salience per byte into a byte budget; mandatory groups are
//!   always kept and every drop says why.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::frame::Frame;
//!
//! let mut f = Frame::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "rank", "args": {"budget": 100, "listener": "player", "items": [
//!     {"id": "oven_fire", "bytes": 60, "urgency": 95, "mandatory": true},
//!     {"id": "promise", "bytes": 30, "entities": ["player"]},
//!     {"id": "shop_history", "bytes": 200}]}}
//! ]}"#, 4096)?;
//! let out = f.workflow(&req)?.to_json();
//! // The fire is kept because it is mandatory, the promise because the listener is in it, and the
//! // shop's history is dropped as irrelevant to this moment.
//! assert!(out.contains(r#""id":"shop_history","kind":null,"parts""#));
//! assert!(out.contains(r#""reason":"irrelevant","score":0}"#));
//! assert!(out.contains(r#""over_budget":false,"used":90"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_bool, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "keys", default: 16, ceiling: 256 },
    Limit { name: "samples", default: 16, ceiling: 256 },
    Limit { name: "items", default: 64, ceiling: 4096 },
];
pub const M: i64 = 1_000_000;
/// Longest per-call list in a frame (bystanders, needs, emotions, listener facts, cues).
pub const MAX_LIST: usize = 16;
/// Salience components and their default weights.
pub const WEIGHTS: [(&str, u64); 6] =
    [("urgency", 30), ("listener", 20), ("utterance", 20), ("task", 15), ("mood", 10), ("recency", 5)];
pub const POLARITY: [&str; 3] = ["favor", "grievance", "neutral"];
pub const INTERRUPT: [&str; 3] = ["any", "never", "urgent"];

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

fn opt_sint(args: &Value, k: &str, lo: i64, hi: i64, default: i64) -> Result<i64> {
    opt(args, k).map(|v| sint(v, lo, hi)).transpose().map(|x| x.unwrap_or(default))
}

fn opt_id(args: &Value, k: &str) -> Result<Option<String>> {
    opt(args, k).map(|v| ident(v).map(str::to_string)).transpose()
}

/// Unique identifiers, sorted.
fn idset(args: &Value, k: &str) -> Result<BTreeSet<String>> {
    let rows = list_bound(opt(args, k), MAX_LIST)?;
    let set = rows.iter().map(|v| ident(v).map(str::to_string)).collect::<Result<BTreeSet<_>>>()?;
    need(set.len() == rows.len())?;
    Ok(set)
}

fn strs(xs: impl IntoIterator<Item = String>) -> Value {
    Value::Array(xs.into_iter().map(Value::from).collect())
}

fn opt_str(x: &Option<String>) -> Value {
    x.as_deref().map(Value::from).unwrap_or(Value::Null)
}

/// A human anchor for a tick gap; `tph` ticks make one hour. `due` reads `at` as a deadline.
pub fn time_label(at: u64, now: u64, tph: u64, due: bool) -> Result<(u64, &'static str)> {
    if due {
        return Ok(if now <= at { ((at - now) / tph, "due_in") } else { ((now - at) / tph, "overdue") });
    }
    need(at <= now)?;
    let ago = now - at;
    let minutes = ago as u128 * 60 / tph as u128;
    let label = match minutes {
        _ if ago == 0 => "just_now",
        m if m < 15 => "moments_ago",
        m if m < 8 * 60 => "earlier_this_shift",
        m if m < 24 * 60 => "earlier_today",
        m if m < 48 * 60 => "yesterday",
        m if m < 14 * 24 * 60 => "days_ago",
        _ => "long_ago",
    };
    Ok((ago / tph, label))
}

/// One being's recent values and outcome streak.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Being {
    pub keys: BTreeMap<String, VecDeque<(u64, i64)>>,
    pub streak: i64,
}

/// Trajectories per being; frames and rankings are computed per call and not kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub beings: BTreeMap<String, Being>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Frame {
    fn default() -> Self {
        Frame { beings: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

struct Item {
    id: String,
    bytes: u64,
    group: Option<String>,
    kind: Option<String>,
    mandatory: bool,
    parts: [u64; 6],
    score: u64,
}

impl Frame {
    pub fn new() -> Self {
        Self::default()
    }

    /// A frame with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "tick" => self.op_tick(args),
            "observe" => self.op_observe(args, l),
            "trend" => self.op_trend(args),
            "outcome" => self.op_outcome(args, l),
            "streak" => self.op_streak(args),
            "time" => self.op_time(args),
            "frame" => self.op_frame(args),
            "rank" => self.op_rank(args, l),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn being_room(&self, b: &str, l: &Limits) -> Result<()> {
        need(self.beings.contains_key(b) || self.beings.len() < l.get("beings"))
    }

    fn trend_of(&self, b: &str, k: &str, window: u64, shock: u64, level: u64, settle: u64) -> Value {
        let xs = &self.beings[b].keys[k];
        let (_, value) = xs[xs.len() - 1];
        let mut base = xs[0].1;
        if self.clock >= window {
            for &(t, v) in xs {
                if t <= self.clock - window {
                    base = v;
                }
            }
        }
        let delta = value - base;
        let mut changed = xs[0].0;
        for i in (1..xs.len()).rev() {
            if xs[i - 1].1 != xs[i].1 {
                changed = xs[i].0;
                break;
            }
        }
        let still = self.clock - changed;
        let (shock, level) = (shock as i64, level as i64);
        let label = if delta >= shock {
            "acute_rise"
        } else if delta <= -shock {
            "acute_drop"
        } else if value >= level && still >= settle {
            "settled_high"
        } else if value <= -level && still >= settle {
            "settled_low"
        } else if delta > 0 {
            "rising"
        } else if delta < 0 {
            "falling"
        } else {
            "steady"
        };
        obj! {
            "being" => b, "delta" => delta, "key" => k, "label" => label, "samples" => xs.len(),
            "unchanged_for" => still, "value" => value,
        }
    }

    fn streak_view(&self, b: &str) -> Value {
        let s = self.beings[b].streak;
        let label = if s >= 3 {
            "flow"
        } else if s <= -3 {
            "frustration_spiral"
        } else {
            "mixed"
        };
        obj! { "being" => b, "label" => label, "streak" => s }
    }

    // ------------------------------------------------------------ trajectory

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn op_observe(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "key", "value"], &[])?;
        let b = id_of(args, "being")?;
        let k = id_of(args, "key")?;
        let v = sint(c::get(args, "value")?, -M, M)?;
        self.being_room(&b, l)?;
        if let Some(x) = self.beings.get(&b) {
            need(x.keys.contains_key(&k) || x.keys.len() < l.get("keys"))?;
        }
        let clock = self.clock;
        let xs = self.beings.entry(b.clone()).or_default().keys.entry(k.clone()).or_default();
        xs.push_back((clock, v));
        while xs.len() > l.get("samples") {
            xs.pop_front();
        }
        Ok(obj! { "at" => clock, "being" => b, "key" => k, "samples" => xs.len(), "value" => v })
    }

    fn op_trend(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "key"], &["window", "shock", "level", "settle"])?;
        let b = id_of(args, "being")?;
        let k = id_of(args, "key")?;
        let window = opt_uint(args, "window", 0, MAX_AT, 3)?;
        let shock = opt_uint(args, "shock", 1, 2 * M as u64, 30)?;
        let level = opt_uint(args, "level", 0, M as u64, 40)?;
        let settle = opt_uint(args, "settle", 0, MAX_AT, 500)?;
        need(self.beings.get(&b).is_some_and(|x| x.keys.contains_key(&k)))?;
        Ok(self.trend_of(&b, &k, window, shock, level, settle))
    }

    fn op_outcome(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "ok"], &[])?;
        let b = id_of(args, "being")?;
        let ok = match c::get(args, "ok")? {
            Value::Bool(x) => *x,
            _ => return Err(c::bad()),
        };
        self.being_room(&b, l)?;
        let x = self.beings.entry(b.clone()).or_default();
        let s = x.streak;
        let s = if ok {
            if s > 0 {
                s + 1
            } else {
                1
            }
        } else if s < 0 {
            s - 1
        } else {
            -1
        };
        x.streak = s.clamp(-1000, 1000);
        Ok(self.streak_view(&b))
    }

    fn op_streak(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        need(self.beings.contains_key(&b))?;
        Ok(self.streak_view(&b))
    }

    fn op_time(&self, args: &Value) -> Result<Value> {
        keys(args, &["at"], &["now", "ticks_per_hour", "due"])?;
        let at = uint_bound(c::get(args, "at")?, 0, MAX_AT)?;
        let now = opt_uint(args, "now", 0, MAX_AT, self.clock)?;
        let tph = opt_uint(args, "ticks_per_hour", 1, 3600, 1)?;
        let due = opt_bool(args, "due", false)?;
        let (hours, label) = time_label(at, now, tph, due)?;
        Ok(obj! { "at" => at, "hours" => hours, "label" => label, "now" => now })
    }

    // ------------------------------------------------------------ frame

    fn op_frame(&self, args: &Value) -> Result<Value> {
        keys(
            args,
            &["being"],
            &[
                "listener",
                "place",
                "ticks_per_hour",
                "bystanders",
                "body",
                "motive",
                "task",
                "affect",
                "listener_model",
                "offscreen",
                "novelty",
            ],
        )?;
        let b = id_of(args, "being")?;
        let listener = opt_id(args, "listener")?;
        need(listener.as_deref() != Some(b.as_str()))?;
        let place = opt_id(args, "place")?;
        let tph = opt_uint(args, "ticks_per_hour", 1, 3600, 1)?;
        let now = self.clock;

        let mut bys: BTreeMap<String, (bool, bool)> = BTreeMap::new();
        let rows = list_bound(opt(args, "bystanders"), MAX_LIST)?;
        for x in rows {
            keys(x, &["being"], &["trusted", "earshot"])?;
            let who = id_of(x, "being")?;
            need(who != b && Some(&who) != listener.as_ref())?;
            let e = opt_bool(x, "earshot", true)?;
            let t = opt_bool(x, "trusted", false)?;
            need(bys.insert(who, (e, t)).is_none())?;
        }
        let bystanders = Value::Array(
            bys.iter().map(|(w, (e, t))| obj! { "being" => w.as_str(), "earshot" => *e, "trusted" => *t }).collect(),
        );
        let untrusted = strs(bys.iter().filter(|(_, (e, t))| *e && !*t).map(|(w, _)| w.clone()));

        let body = match opt(args, "body") {
            None => Value::Null,
            Some(body) => {
                keys(body, &[], &["fatigue", "pain", "needs", "conditions", "held", "posture"])?;
                let fatigue = opt_uint(body, "fatigue", 0, 100, 0)?;
                let pain = opt_uint(body, "pain", 0, 100, 0)?;
                let mut urgent: Vec<(u64, String)> = Vec::new();
                if fatigue >= 70 {
                    urgent.push((fatigue, "fatigue".into()));
                }
                if pain >= 50 {
                    urgent.push((pain, "pain".into()));
                }
                let mut seen = BTreeSet::new();
                for n in list_bound(opt(body, "needs"), MAX_LIST)? {
                    keys(n, &["id", "level"], &["threshold"])?;
                    let i = id_of(n, "id")?;
                    need(i != "fatigue" && i != "pain" && seen.insert(i.clone()))?;
                    let lvl = uint_bound(c::get(n, "level")?, 0, 100)?;
                    if lvl >= opt_uint(n, "threshold", 0, 100, 70)? {
                        urgent.push((lvl, i));
                    }
                }
                urgent.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                obj! {
                    "conditions" => strs(idset(body, "conditions")?),
                    "fatigue" => fatigue,
                    "held" => opt_str(&opt_id(body, "held")?),
                    "pain" => pain,
                    "posture" => opt_str(&opt_id(body, "posture")?),
                    "urgent" => Value::Array(urgent.into_iter().map(|(v, i)| obj! { "item" => i, "level" => v }).collect()),
                }
            }
        };

        let motive = match opt(args, "motive") {
            None => Value::Null,
            Some(mo) => {
                keys(mo, &["id", "score"], &["reason", "runner_up", "runner_up_score"])?;
                let score = uint_bound(c::get(mo, "score")?, 0, 100)?;
                let ru = opt_id(mo, "runner_up")?;
                let rus = opt(mo, "runner_up_score").map(|v| uint_bound(v, 0, score)).transpose()?;
                need(ru.is_none() == rus.is_none())?;
                let reason = opt(mo, "reason").map(|v| text_bound(v, 80)).transpose()?;
                obj! {
                    "id" => id_of(mo, "id")?,
                    "margin" => rus.map(|r| score - r),
                    "overrides_talk" => score >= 90,
                    "reason" => reason,
                    "runner_up" => opt_str(&ru),
                    "score" => score,
                }
            }
        };

        let task = match opt(args, "task") {
            None => Value::Null,
            Some(tk) => {
                keys(tk, &["id"], &["step", "progress", "interrupt", "deadline"])?;
                let interrupt = opt(tk, "interrupt").map(ident).transpose()?.unwrap_or("any");
                need(INTERRUPT.contains(&interrupt))?;
                let deadline = opt(tk, "deadline").map(|v| uint_bound(v, 0, MAX_AT)).transpose()?;
                let due = deadline.map(|d| time_label(d, now, tph, true)).transpose()?;
                let pressing = due.is_some_and(|(h, lab)| lab == "overdue" || h <= 1);
                obj! {
                    "deadline" => deadline,
                    "due" => due.map(|(h, lab)| obj! { "hours" => h, "label" => lab }).unwrap_or(Value::Null),
                    "id" => id_of(tk, "id")?,
                    "interrupt" => interrupt,
                    "pressing" => pressing,
                    "progress" => opt_uint(tk, "progress", 0, 100, 0)?,
                    "step" => opt_str(&opt_id(tk, "step")?),
                }
            }
        };

        let affect = match opt(args, "affect") {
            None => Value::Null,
            Some(af) => {
                keys(af, &[], &["valence", "arousal", "label", "emotions"])?;
                let mut ems: Vec<(u64, String, Option<String>)> = Vec::new();
                let mut seen = BTreeSet::new();
                for e in list_bound(opt(af, "emotions"), MAX_LIST)? {
                    keys(e, &["emotion", "value"], &["toward"])?;
                    let em = id_of(e, "emotion")?;
                    let to = opt_id(e, "toward")?;
                    let v = uint_bound(c::get(e, "value")?, 0, 100)?;
                    need(seen.insert((em.clone(), to.clone())))?;
                    ems.push((v, em, to));
                }
                ems.sort_by(|a, b| {
                    b.0.cmp(&a.0)
                        .then_with(|| a.1.cmp(&b.1))
                        .then_with(|| a.2.as_deref().unwrap_or_default().cmp(b.2.as_deref().unwrap_or_default()))
                });
                let top = ems
                    .into_iter()
                    .take(3)
                    .map(|(v, em, to)| obj! { "emotion" => em, "toward" => opt_str(&to), "value" => v })
                    .collect();
                obj! {
                    "arousal" => opt_uint(af, "arousal", 0, 100, 0)?,
                    "label" => opt_str(&opt_id(af, "label")?),
                    "top" => Value::Array(top),
                    "valence" => opt_sint(af, "valence", -100, 100, 0)?,
                }
            }
        };

        let model = match opt(args, "listener_model") {
            None => Value::Null,
            Some(lm) => {
                need(listener.is_some())?;
                keys(lm, &[], &["stance", "trust", "knows", "wants", "feels", "lies", "overdue"])?;
                obj! {
                    "feels" => strs(idset(lm, "feels")?),
                    "knows" => strs(idset(lm, "knows")?),
                    "lies" => strs(idset(lm, "lies")?),
                    "overdue" => strs(idset(lm, "overdue")?),
                    "stance" => opt_str(&opt_id(lm, "stance")?),
                    "trust" => opt_sint(lm, "trust", -100, 100, 0)?,
                    "wants" => strs(idset(lm, "wants")?),
                }
            }
        };

        let offscreen = match opt(args, "offscreen") {
            None => Value::Null,
            Some(off) => {
                keys(off, &["summary", "at"], &[])?;
                let at = uint_bound(c::get(off, "at")?, 0, now)?;
                let summary = text_bound(c::get(off, "summary")?, 160)?;
                let (_, when) = time_label(at, now, tph, false)?;
                obj! { "at" => at, "summary" => summary, "when" => when }
            }
        };

        let novelty = match opt(args, "novelty") {
            None => Value::Null,
            Some(nv) => {
                keys(nv, &["id", "score"], &[])?;
                obj! { "id" => id_of(nv, "id")?, "score" => uint_bound(c::get(nv, "score")?, 0, 100)? }
            }
        };

        let (trends, streak) = match self.beings.get(&b) {
            None => (Value::Array(Vec::new()), Value::Null),
            Some(x) => (
                Value::Array(x.keys.keys().map(|k| self.trend_of(&b, k, 3, 30, 40, 500)).collect()),
                self.streak_view(&b),
            ),
        };
        Ok(obj! {
            "active_motive" => motive,
            "active_task" => task,
            "affect_snapshot" => affect,
            "being" => b,
            "body_urgent" => body,
            "bystanders" => bystanders,
            "listener" => opt_str(&listener),
            "listener_model" => model,
            "novelty_focus" => novelty,
            "now" => now,
            "place" => opt_str(&place),
            "recent_offscreen" => offscreen,
            "streak" => streak,
            "trends" => trends,
            "untrusted_in_earshot" => untrusted,
        })
    }

    // ------------------------------------------------------------ rank

    fn op_rank(&self, args: &Value, l: &Limits) -> Result<Value> {
        keys(
            args,
            &["items", "budget"],
            &["listener", "earshot", "cues", "task_needs", "mood", "weights", "recent", "floor"],
        )?;
        let budget = uint_bound(c::get(args, "budget")?, 0, 10_000_000)?;
        let listener = opt_id(args, "listener")?;
        let earshot = idset(args, "earshot")?;
        let none = Value::Object(Vec::new());
        let cues = opt(args, "cues").unwrap_or(&none);
        keys(cues, &[], &["entities", "tags", "places"])?;
        let (ce, ct, cp) = (idset(cues, "entities")?, idset(cues, "tags")?, idset(cues, "places")?);
        let task_needs = idset(args, "task_needs")?;
        let mood = opt(args, "mood").unwrap_or(&none);
        keys(mood, &[], &["valence", "resentment", "gratitude"])?;
        let valence = opt_sint(mood, "valence", -100, 100, 0)?;
        let resent = opt_uint(mood, "resentment", 0, 100, 0)?;
        let grat = opt_uint(mood, "gratitude", 0, 100, 0)?;
        let (dark, warm) = (valence < -30 || resent >= 50, valence > 30 || grat >= 50);
        let w = opt(args, "weights").unwrap_or(&none);
        let names: Vec<&str> = WEIGHTS.iter().map(|(k, _)| *k).collect();
        keys(w, &[], &names)?;
        let mut weights = [0u64; 6];
        for (i, (k, d)) in WEIGHTS.iter().enumerate() {
            weights[i] = opt_uint(w, k, 0, 100, *d)?;
        }
        let total_w: u64 = weights.iter().sum();
        need(total_w > 0)?;
        let recent = opt_uint(args, "recent", 0, 1000, 3)?;
        let floor = opt_uint(args, "floor", 0, 1000, 1)?;

        let rows = list_bound(Some(c::get(args, "items")?), l.get("items"))?;
        let mut items: Vec<Item> = Vec::new();
        let mut ids = BTreeSet::new();
        for it in rows {
            keys(
                it,
                &["id", "bytes"],
                &["kind", "at", "entities", "tags", "places", "urgency", "mandatory", "group", "polarity"],
            )?;
            let id = id_of(it, "id")?;
            let bytes = uint_bound(c::get(it, "bytes")?, 1, 65_536)?;
            let at = opt(it, "at").map(|v| uint_bound(v, 0, self.clock)).transpose()?;
            let (ents, tags, places) = (idset(it, "entities")?, idset(it, "tags")?, idset(it, "places")?);
            let pol = opt(it, "polarity").map(ident).transpose()?.unwrap_or("neutral");
            need(POLARITY.contains(&pol))?;
            let u = opt_uint(it, "urgency", 0, 100, 0)? * 10;
            let lis = if listener.as_ref().is_some_and(|x| ents.contains(x)) {
                1000
            } else if ents.iter().any(|e| earshot.contains(e)) {
                600
            } else {
                0
            };
            let hits =
                ents.intersection(&ce).count() + tags.intersection(&ct).count() + places.intersection(&cp).count();
            let q = (250 * hits as u64).min(1000);
            let t = if task_needs.contains(&id) || tags.iter().any(|x| task_needs.contains(x)) { 1000 } else { 0 };
            let m = if (pol == "grievance" && dark) || (pol == "favor" && warm) { 1000 } else { 0 };
            let r = match at {
                Some(at) if self.clock - at <= recent => (recent + 1 - (self.clock - at)) * 1000 / (recent + 1),
                _ => 0,
            };
            let parts = [u, lis, q, t, m, r];
            let score = parts.iter().zip(weights.iter()).map(|(p, w)| p * w).sum::<u64>() / total_w;
            need(ids.insert(id.clone()))?;
            items.push(Item {
                id,
                bytes,
                group: opt_id(it, "group")?,
                kind: opt_id(it, "kind")?,
                mandatory: opt_bool(it, "mandatory", false)?,
                parts,
                score,
            });
        }

        // Units: a causal group, or a lone item; in order of first appearance.
        let mut order: Vec<String> = Vec::new();
        let mut units: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, x) in items.iter().enumerate() {
            let key = match &x.group {
                Some(g) => format!("g:{g}"),
                None => format!("i:{}", x.id),
            };
            if !units.contains_key(&key) {
                order.push(key.clone());
            }
            units.entry(key).or_default().push(i);
        }
        struct Unit<'a> {
            bytes: u64,
            first: &'a str,
            mandatory: bool,
            members: Vec<usize>,
            score: u64,
        }
        let all: Vec<Unit> = order
            .iter()
            .map(|k| {
                let members = units[k].clone();
                Unit {
                    bytes: members.iter().map(|&i| items[i].bytes).sum(),
                    first: members.iter().map(|&i| items[i].id.as_str()).min().unwrap_or_default(),
                    mandatory: members.iter().any(|&i| items[i].mandatory),
                    score: members.iter().map(|&i| items[i].score).max().unwrap_or(0),
                    members,
                }
            })
            .collect();
        let mut used = 0u64;
        let mut kept: Vec<(usize, &str)> = Vec::new();
        let mut dropped: Vec<(usize, &str)> = Vec::new();
        for u in all.iter().filter(|u| u.mandatory) {
            used += u.bytes;
            kept.extend(u.members.iter().map(|&i| (i, "mandatory")));
        }
        let mut rest: Vec<&Unit> = all.iter().filter(|u| !u.mandatory).collect();
        rest.sort_by(|a, b| {
            (b.score * 1000 / b.bytes)
                .cmp(&(a.score * 1000 / a.bytes))
                .then_with(|| b.score.cmp(&a.score))
                .then_with(|| a.first.cmp(b.first))
        });
        for u in rest {
            let why = if u.score < floor {
                "irrelevant"
            } else if used + u.bytes <= budget {
                used += u.bytes;
                kept.extend(u.members.iter().map(|&i| (i, "salience")));
                continue;
            } else {
                "budget"
            };
            dropped.extend(u.members.iter().map(|&i| (i, why)));
        }
        let view = |list: &mut Vec<(usize, &str)>| {
            list.sort_by(|a, b| {
                items[b.0].score.cmp(&items[a.0].score).then_with(|| items[a.0].id.cmp(&items[b.0].id))
            });
            Value::Array(
                list.iter()
                    .map(|&(i, why)| {
                        let x = &items[i];
                        let parts = Value::Object(
                            [
                                ("listener", 1),
                                ("mood", 4),
                                ("recency", 5),
                                ("task", 3),
                                ("urgency", 0),
                                ("utterance", 2),
                            ]
                            .iter()
                            .map(|(k, j)| (k.to_string(), Value::from(x.parts[*j])))
                            .collect(),
                        );
                        obj! {
                            "bytes" => x.bytes, "group" => opt_str(&x.group), "id" => x.id.as_str(),
                            "kind" => opt_str(&x.kind), "parts" => parts, "reason" => why, "score" => x.score,
                        }
                    })
                    .collect(),
            )
        };
        let dropped = view(&mut dropped);
        let kept = view(&mut kept);
        Ok(
            obj! { "budget" => budget, "dropped" => dropped, "kept" => kept, "over_budget" => used > budget, "used" => used },
        )
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Frame::new().workflow(req)
}
