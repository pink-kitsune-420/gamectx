//! Player model, subjective beliefs about the player, session time anchors, and boundaries
//! (pass 5, stage 6, design `T`, local kind `player`).
use crate::character::limits::{Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, exact_fields, Persist};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "boundaries", default: 16, ceiling: 64 },
    Limit { name: "topics", default: 32, ceiling: 1024 },
];

pub const CONTENT_CATEGORIES: &[&str] = &["cruelty", "insults", "romance", "self_harm", "violence"];
pub const BOUNDARY_MODES: &[&str] = &["line", "off", "veil"];

pub const TIME_ANCHORS: &[(u64, &str)] = &[
    (1, "just now"),
    (24, "a few hours have passed"),
    (168, "several days have passed"),
    (720, "a few weeks have passed"),
    (2160, "you have been gone all season"),
    (1_000_000_000, "you have been gone all winter and beyond"),
];

fn integer(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(c::bad),
        _ => Err(c::bad()),
    }
}

#[derive(Clone, Debug)]
pub struct PlayerBeliefs {
    pub honesty: u64,
    pub promises_kept: u64,
    pub promises_broken: u64,
    pub lies_caught: u64,
    pub lies_admitted: u64,
    pub gifts: Vec<String>,
    pub topics: BTreeMap<String, u64>,
    pub weak_treatment: u64,
    pub last_seen: u64,
    pub open_threads: BTreeSet<String>,
}

impl Default for PlayerBeliefs {
    fn default() -> Self {
        Self {
            honesty: 500_000,
            promises_kept: 0,
            promises_broken: 0,
            lies_caught: 0,
            lies_admitted: 0,
            gifts: Vec::new(),
            topics: BTreeMap::new(),
            weak_treatment: 500_000,
            last_seen: 0,
            open_threads: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerModelManager {
    pub limits: Limits,
    pub boundaries: BTreeMap<String, String>,
    pub beings: BTreeMap<String, PlayerBeliefs>,
}

impl Default for PlayerModelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerModelManager {
    pub fn new() -> Self {
        let mut boundaries = BTreeMap::new();
        for &cat in CONTENT_CATEGORIES {
            boundaries.insert(cat.to_string(), "off".to_string());
        }
        Self { limits: Limits::defaults(LIMITS), boundaries, beings: BTreeMap::new() }
    }

    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    fn ensure_being(&mut self, bid: &str) -> Result<&mut PlayerBeliefs> {
        if !self.beings.contains_key(bid) {
            if self.beings.len() + 1 > self.limits.get("beings") {
                return Err(c::bad());
            }
            self.beings.insert(bid.to_string(), PlayerBeliefs::default());
        }
        Ok(self.beings.get_mut(bid).unwrap())
    }

    pub fn op_boundary(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["category"], &["mode"])?;
        let cat_raw = c::text(c::get(args, "category")?)?;
        if cat_raw.is_empty() || cat_raw.len() > 32 {
            return Err(c::bad());
        }
        let cat = cat_raw.to_ascii_lowercase();
        if !CONTENT_CATEGORIES.contains(&cat.as_str()) {
            return Err(c::bad());
        }

        let mode = match c::get(args, "mode") {
            Ok(v) if v != &Value::Null => {
                let m_str = c::text(v)?;
                if m_str.is_empty() || m_str.len() > 16 {
                    return Err(c::bad());
                }
                m_str.to_ascii_lowercase()
            }
            _ => "line".to_string(),
        };
        if !BOUNDARY_MODES.contains(&mode.as_str()) {
            return Err(c::bad());
        }

        self.boundaries.insert(cat.clone(), mode.clone());

        Ok(obj! {
            "category" => cat,
            "mode" => mode,
        })
    }

    pub fn op_check(&self, args: &Value) -> Result<Value> {
        keys(args, &["categories"], &["target"])?;
        let cats = c::list(c::get(args, "categories")?)?;
        let target = match c::get(args, "target") {
            Ok(v) if v != &Value::Null => {
                let s = c::text(v)?;
                if s.is_empty() || s.len() > 64 {
                    return Err(c::bad());
                }
                s.to_string()
            }
            _ => "player".to_string(),
        };

        let mut blocked_mode = "allowed".to_string();
        let mut blocked_cat = Value::Null;

        for c_raw in cats {
            let s = c::text(c_raw)?;
            if s.is_empty() || s.len() > 32 {
                return Err(c::bad());
            }
            let cat = s.to_ascii_lowercase();
            if !CONTENT_CATEGORIES.contains(&cat.as_str()) {
                return Err(c::bad());
            }

            let mode = self.boundaries.get(&cat).map(String::as_str).unwrap_or("off");
            if mode == "line" {
                blocked_mode = "line".to_string();
                blocked_cat = Value::str(cat);
                break;
            } else if mode == "veil" && blocked_mode != "line" {
                blocked_mode = "veil".to_string();
                blocked_cat = Value::str(cat);
            }
        }

        let allowed = blocked_mode != "line";

        Ok(obj! {
            "allowed" => allowed,
            "blocked_category" => blocked_cat,
            "mode" => blocked_mode,
            "target" => target,
        })
    }

    pub fn op_record(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["action", "being"], &["delta", "gift", "thread", "topic"])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        let act = c::text(c::get(args, "action")?)?.to_string();
        let max_topics = self.limits.get("topics");

        let b = self.ensure_being(&bid)?;

        match act.as_str() {
            "promise_kept" => {
                b.promises_kept += 1;
                b.honesty = (b.honesty + 50_000).min(1_000_000);
                if let Ok(t_val) = c::get(args, "thread") {
                    if t_val != &Value::Null {
                        let tid = c::text(t_val)?;
                        if tid.is_empty() || tid.len() > 64 {
                            return Err(c::bad());
                        }
                        b.open_threads.remove(tid);
                    }
                }
            }
            "promise_broken" => {
                b.promises_broken += 1;
                b.honesty = b.honesty.saturating_sub(150_000);
                if let Ok(t_val) = c::get(args, "thread") {
                    if t_val != &Value::Null {
                        let tid = c::text(t_val)?;
                        if tid.is_empty() || tid.len() > 64 {
                            return Err(c::bad());
                        }
                        b.open_threads.insert(tid.to_string());
                    }
                }
            }
            "lie_caught" => {
                b.lies_caught += 1;
                b.honesty = b.honesty.saturating_sub(250_000);
            }
            "lie_admitted" => {
                b.lies_admitted += 1;
                b.honesty = (b.honesty + 75_000).min(1_000_000);
            }
            "gift" => {
                let g_val = c::get(args, "gift")?;
                if g_val == &Value::Null {
                    return Err(c::bad());
                }
                let g_item = c::text(g_val)?;
                if g_item.is_empty() || g_item.len() > 64 {
                    return Err(c::bad());
                }
                b.gifts.push(g_item.to_string());
                b.weak_treatment = (b.weak_treatment + 40_000).min(1_000_000);
            }
            "weak_treatment" => {
                let d_val = c::get(args, "delta")?;
                if d_val == &Value::Null {
                    return Err(c::bad());
                }
                let delta = integer(d_val, -500_000, 500_000)?;
                let new_wt = (b.weak_treatment as i64 + delta).max(0).min(1_000_000);
                b.weak_treatment = new_wt as u64;
            }
            "topic" => {
                let t_val = c::get(args, "topic")?;
                if t_val == &Value::Null {
                    return Err(c::bad());
                }
                let t_raw = c::text(t_val)?;
                if t_raw.is_empty() || t_raw.len() > 64 {
                    return Err(c::bad());
                }
                let t_name = t_raw.to_ascii_lowercase();
                if !b.topics.contains_key(&t_name) {
                    if b.topics.len() + 1 > max_topics {
                        return Err(c::bad());
                    }
                    b.topics.insert(t_name.clone(), 0);
                }
                let count = b.topics.get_mut(&t_name).unwrap();
                *count += 1;
            }
            _ => return Err(c::bad()),
        }

        let honesty = b.honesty;
        let pb = b.promises_broken;
        let pk = b.promises_kept;
        let wt = b.weak_treatment;

        Ok(obj! {
            "action" => act,
            "being" => bid,
            "honesty" => honesty,
            "promises_broken" => pb,
            "promises_kept" => pk,
            "weak_treatment" => wt,
        })
    }

    pub fn op_anchor(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "now"], &["last_session"])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        let now = c::uint(c::get(args, "now")?, 0, 1_000_000_000)?;

        let b = self.ensure_being(&bid)?;
        let last_sess = match c::get(args, "last_session") {
            Ok(v) if v != &Value::Null => c::uint(v, 0, now)?,
            _ => b.last_seen,
        };
        if now < last_sess {
            return Err(c::bad());
        }
        let elapsed = now - last_sess;
        b.last_seen = now;

        let mut anchor_str = "just now";
        for &(thresh, desc) in TIME_ANCHORS {
            if elapsed <= thresh {
                anchor_str = desc;
                break;
            }
        }

        let surfaced: Vec<Value> = b.open_threads.iter().cloned().map(Value::str).collect();
        b.open_threads.clear();

        Ok(obj! {
            "anchor" => anchor_str,
            "being" => bid,
            "elapsed" => elapsed,
            "surfaced_threads" => Value::Array(surfaced),
        })
    }

    pub fn op_reputation(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        let b = self.ensure_being(&bid)?;

        let mut sorted_topics: Vec<(i64, String)> = b.topics.iter().map(|(k, v)| (-(*v as i64), k.clone())).collect();
        sorted_topics.sort_unstable();

        let top_topics: Vec<Value> = sorted_topics.into_iter().take(3).map(|(_, k)| Value::str(k)).collect();
        let mut top_sorted: Vec<String> = top_topics.into_iter().map(|v| v.as_str().unwrap().to_string()).collect();
        top_sorted.sort_unstable();

        let total_promises = b.promises_kept + b.promises_broken;
        let promise_ratio = if total_promises > 0 { (b.promises_kept * 1000) / total_promises } else { 1000 };

        let honesty = b.honesty;
        let gifts_count = b.gifts.len() as u64;
        let wt = b.weak_treatment;

        Ok(obj! {
            "being" => bid,
            "frequent_topics" => Value::Array(top_sorted.into_iter().map(Value::str).collect()),
            "gifts_count" => gifts_count,
            "honesty" => honesty,
            "promise_ratio" => promise_ratio,
            "weak_treatment" => wt,
        })
    }

    pub fn settle_state(&mut self) -> Result<()> {
        if self.boundaries.len() > self.limits.get("boundaries") {
            return Err(c::bad());
        }
        if self.beings.len() > self.limits.get("beings") {
            return Err(c::bad());
        }
        for (cat, mode) in &self.boundaries {
            if !CONTENT_CATEGORIES.contains(&cat.as_str()) {
                return Err(c::bad());
            }
            if !BOUNDARY_MODES.contains(&mode.as_str()) {
                return Err(c::bad());
            }
        }
        for b in self.beings.values() {
            if b.topics.len() > self.limits.get("topics") {
                return Err(c::bad());
            }
        }
        Ok(())
    }

    pub fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &["limits"])?;
        let supplied = c::get(request, "limits").is_ok();
        if let Ok(l) = c::get(request, "limits") {
            self.limits.apply(l)?;
        }
        let ops = c::list(c::get(request, "ops")?)?;
        if ops.is_empty() || ops.len() > 64 {
            return Err(c::bad());
        }
        let mut results = Vec::new();
        for item in ops {
            keys(item, &["op"], &["args"])?;
            let op_name = c::text(c::get(item, "op")?)?;
            let empty = obj! {};
            let args = c::get(item, "args").unwrap_or(&empty);
            let res = match op_name {
                "boundary" => self.op_boundary(args)?,
                "check" => self.op_check(args)?,
                "record" => self.op_record(args)?,
                "anchor" => self.op_anchor(args)?,
                "reputation" => self.op_reputation(args)?,
                "export" => persist::export_op(self, &self.limits, args)?,
                "import" => {
                    let limits = self.limits.clone();
                    persist::import(self, &limits, args)?
                }
                _ => return Err(c::bad()),
            };
            results.push(res);
        }
        let mut out = obj! {
            "results" => Value::Array(results),
            "version" => 1,
        };
        if supplied {
            if let Value::Object(ref mut fields) = out {
                fields.insert(
                    0,
                    (
                        "limits".to_string(),
                        Value::Object(vec![
                            ("beings".to_string(), Value::from(self.limits.get("beings") as u64)),
                            ("boundaries".to_string(), Value::from(self.limits.get("boundaries") as u64)),
                            ("topics".to_string(), Value::from(self.limits.get("topics") as u64)),
                        ]),
                    ),
                );
            }
        }
        Ok(out)
    }
}

impl Persist for PlayerModelManager {
    fn save(&self) -> Value {
        let bounds_arr: Vec<Value> = self
            .boundaries
            .iter()
            .map(|(k, v)| Value::Array(vec![Value::str(k.clone()), Value::str(v.clone())]))
            .collect();

        let beings_arr: Vec<Value> = self
            .beings
            .iter()
            .map(|(k, b)| {
                let topics_arr: Vec<Value> = b
                    .topics
                    .iter()
                    .map(|(tk, tv)| Value::Array(vec![Value::str(tk.clone()), Value::from(*tv)]))
                    .collect();
                let mut gifts_sorted = b.gifts.clone();
                gifts_sorted.sort();
                Value::Array(vec![
                    Value::str(k.clone()),
                    Value::Array(gifts_sorted.into_iter().map(Value::str).collect()),
                    Value::from(b.honesty),
                    Value::from(b.last_seen),
                    Value::from(b.lies_admitted),
                    Value::from(b.lies_caught),
                    Value::Array(b.open_threads.iter().cloned().map(Value::str).collect()),
                    Value::from(b.promises_broken),
                    Value::from(b.promises_kept),
                    Value::Array(topics_arr),
                    Value::from(b.weak_treatment),
                ])
            })
            .collect();

        obj! {
            "beings" => Value::Array(beings_arr),
            "boundaries" => Value::Array(bounds_arr),
        }
    }

    fn load(v: &Value) -> Result<Self> {
        exact_fields(v, &["beings", "boundaries"])?;
        let mut mgr = Self::new();

        let b_list = c::list(c::get(v, "boundaries")?)?;
        if b_list.len() > mgr.limits.get("boundaries") {
            return Err(c::bad());
        }
        let mut new_bounds = BTreeMap::new();
        let mut last_cat: Option<&str> = None;
        for pair_val in b_list {
            let pair = c::list(pair_val)?;
            if pair.len() != 2 {
                return Err(c::bad());
            }
            let cat = c::text(&pair[0])?;
            if !CONTENT_CATEGORIES.contains(&cat) {
                return Err(c::bad());
            }
            if let Some(prev) = last_cat {
                if cat <= prev {
                    return Err(c::bad());
                }
            }
            last_cat = Some(cat);

            let mode = c::text(&pair[1])?;
            if !BOUNDARY_MODES.contains(&mode) {
                return Err(c::bad());
            }
            new_bounds.insert(cat.to_string(), mode.to_string());
        }

        let beings_list = c::list(c::get(v, "beings")?)?;
        if beings_list.len() > mgr.limits.get("beings") {
            return Err(c::bad());
        }
        let mut new_beings = BTreeMap::new();
        let mut last_b: Option<&str> = None;

        for row_val in beings_list {
            let row = c::list(row_val)?;
            if row.len() != 11 {
                return Err(c::bad());
            }
            let bid = ident(&row[0])?;
            if let Some(prev) = last_b {
                if bid <= prev {
                    return Err(c::bad());
                }
            }
            last_b = Some(bid);

            let mut gifts = Vec::new();
            for g in c::list(&row[1])? {
                let s = c::text(g)?;
                if s.is_empty() || s.len() > 64 {
                    return Err(c::bad());
                }
                gifts.push(s.to_string());
            }
            gifts.sort();

            let honesty = c::uint(&row[2], 0, 1_000_000)?;
            let last_seen = c::uint(&row[3], 0, 1_000_000_000)?;
            let lies_admitted = c::uint(&row[4], 0, 100_000)?;
            let lies_caught = c::uint(&row[5], 0, 100_000)?;

            let mut threads = BTreeSet::new();
            for t in c::list(&row[6])? {
                let s = c::text(t)?;
                if s.is_empty() || s.len() > 64 {
                    return Err(c::bad());
                }
                threads.insert(s.to_string());
            }

            let promises_broken = c::uint(&row[7], 0, 100_000)?;
            let promises_kept = c::uint(&row[8], 0, 100_000)?;

            let raw_topics = c::list(&row[9])?;
            if raw_topics.len() > mgr.limits.get("topics") {
                return Err(c::bad());
            }
            let mut topics = BTreeMap::new();
            let mut last_top: Option<&str> = None;
            for top_val in raw_topics {
                let top_pair = c::list(top_val)?;
                if top_pair.len() != 2 {
                    return Err(c::bad());
                }
                let t_name = c::text(&top_pair[0])?;
                if t_name.is_empty() || t_name.len() > 64 {
                    return Err(c::bad());
                }
                if let Some(prev_t) = last_top {
                    if t_name <= prev_t {
                        return Err(c::bad());
                    }
                }
                last_top = Some(t_name);
                let count = c::uint(&top_pair[1], 0, 1_000_000)?;
                topics.insert(t_name.to_string(), count);
            }

            let weak_treatment = c::uint(&row[10], 0, 1_000_000)?;

            new_beings.insert(
                bid.to_string(),
                PlayerBeliefs {
                    honesty,
                    promises_kept,
                    promises_broken,
                    lies_caught,
                    lies_admitted,
                    gifts,
                    topics,
                    weak_treatment,
                    last_seen,
                    open_threads: threads,
                },
            );
        }

        mgr.boundaries = new_bounds;
        mgr.beings = new_beings;
        mgr.settle_state()?;
        Ok(mgr)
    }
}

durable!(PlayerModelManager, "player", LIMITS);

pub fn workflow(request: &Value) -> Result<Value> {
    PlayerModelManager::new().workflow(request)
}
