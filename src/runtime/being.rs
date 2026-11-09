//! One being, one runtime: definitions, event order, wired edges, and durable saves (pass 5 stage 2).
//!
//! A [`BeingRuntime`] owns one bundle containing the thirty-four durable engine kinds,
//! holds the authored definitions of beings, and executes turn events in the deterministic
//! twelve-stage engine order. Twelve wired edges feed outputs forward to later stages, with
//! cross-step feedback carried in durable state.
//!
//! Ops: `define`, `edges`, `step`, `show`, `export`, `import`.
//! Limits: `definitions` (default 1024, ceiling 1,000,000) and `carry` (default 256, ceiling 4096).

use crate::character::attention::fnv1a;
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, need, Persist, MAX_INT};
use crate::kit::state_bundle::{blank, Section, Sections, KINDS};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const FORMAT: &str = "gamectx-state";
pub const MAX_SEED: u64 = 1_000_000_000_000;
pub const MAX_OPS: usize = 64;

/// Limits for `being-runtime`.
pub const LIMITS: &[Limit] = &[
    Limit { name: "definitions", default: 1024, ceiling: 1_000_000 },
    Limit { name: "carry", default: 256, ceiling: 4096 },
];

pub const EDGES: [&str; 12] = [
    "chemistry_cooperation",
    "culture_retellings",
    "failure_odds",
    "genre_words",
    "minds_admit",
    "project_feeds",
    "protect_witness",
    "salience_budget",
    "threat_feed",
    "traces_beliefs",
    "voice_hints",
    "weather_needs",
];

const PARTS: [&str; 9] = ["creed", "culture", "defense", "genre", "mind", "projects", "routines", "species", "style"];

const ADMITTED: [&str; 3] = ["cueing", "pragmatics", "embodiment"];

/// The deterministic stage order for all thirty-four durable kinds plus `attempt`.
pub fn order(engine: &str) -> Option<usize> {
    match engine {
        "genre" => Some(0),
        "minds" => Some(1),
        "packs" => Some(2),
        "surroundings" => Some(3),
        "fabric" => Some(4),
        "space" => Some(5),
        "attention" => Some(6),
        "turn-frame" => Some(7),
        "pragmatics" => Some(8),
        "expectation" => Some(9),
        "economy" => Some(10),
        "defenses" => Some(11),
        "beliefs" => Some(12),
        "speech_act" => Some(13),
        "cueing" => Some(14),
        "cognition" => Some(15),
        "chatter" => Some(16),
        "culture" => Some(17),
        "survival" => Some(18),
        "detail" => Some(19),
        "diet" => Some(20),
        "creed" => Some(21),
        "habit" => Some(22),
        "roles" => Some(23),
        "projects" => Some(24),
        "cooperation" => Some(25),
        "chemistry" => Some(26),
        "embodiment" => Some(27),
        "explain" => Some(28),
        "stability" => Some(29),
        "retention" => Some(30),
        "attempt" => Some(31),
        "lesson" => Some(32),
        "drift" => Some(33),
        "aspiration" => Some(34),
        _ => None,
    }
}

fn genre_keys(kind: &str) -> Value {
    match kind {
        "fabric" => obj! { "kind" => "contract", "severity" => "severity" },
        "surroundings" => obj! { "access" => "access", "light" => "light", "privacy" => "privacy" },
        _ => obj! {},
    }
}

fn sint(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => {
            let n = s.parse::<i64>().map_err(|_| c::bad())?;
            if n < lo || n > hi {
                return Err(c::bad());
            }
            Ok(n)
        }
        _ => Err(c::bad()),
    }
}

#[derive(Clone, Debug)]
struct CheckedEvent {
    engine: String,
    op: String,
    args: Value,
    being: Option<String>,
    beings: Option<Vec<String>>,
    limits: Option<Value>,
}

#[derive(Clone, Debug)]
struct AttemptArgs {
    being: String,
    task: String,
    odds: u64,
}

#[derive(Clone, Debug)]
struct QueuedOp {
    engine: String,
    op: String,
    args: Value,
    edge: String,
    from: Option<u64>,
    why: String,
    on_ok: Option<(String, String, u64)>,
}

#[derive(Clone, Debug)]
enum QueueItem {
    Event(CheckedEvent, usize),
    Carried((String, String, Value, String, String)),
    Edge(QueuedOp),
    Attempt(AttemptArgs),
}

/// The unified being runtime.
pub struct BeingRuntime {
    pub limits: Limits,
    pub sections: Sections,
    pub definitions: BTreeMap<String, Value>,
    pub edges: BTreeMap<String, bool>,
    pub base: BTreeMap<(String, String), u64>,
    pub extra: BTreeMap<(String, String), u64>,
    pub risk: BTreeMap<String, u64>,
    pub carry: Vec<(String, String, Value, String, String)>,
    pub steps: u64,

    // Runtime state per step
    applied: Vec<Value>,
    trace: Vec<Value>,
    voice: Vec<Value>,
    odds: Vec<Value>,
    budget: Vec<Value>,
    snap: Option<BTreeMap<String, Option<Box<dyn Section>>>>,
    #[allow(clippy::type_complexity)]
    memo: Option<(
        BTreeMap<String, Value>,
        BTreeMap<String, bool>,
        BTreeMap<(String, String), u64>,
        BTreeMap<(String, String), u64>,
        BTreeMap<String, u64>,
        Vec<(String, String, Value, String, String)>,
        u64,
    )>,
}

impl Clone for BeingRuntime {
    fn clone(&self) -> Self {
        let mut sections = BTreeMap::new();
        for (k, s) in &self.sections {
            sections.insert(*k, s.copy());
        }
        BeingRuntime {
            limits: self.limits.clone(),
            sections,
            definitions: self.definitions.clone(),
            edges: self.edges.clone(),
            base: self.base.clone(),
            extra: self.extra.clone(),
            risk: self.risk.clone(),
            carry: self.carry.clone(),
            steps: self.steps,
            applied: self.applied.clone(),
            trace: self.trace.clone(),
            voice: self.voice.clone(),
            odds: self.odds.clone(),
            budget: self.budget.clone(),
            snap: None,
            memo: None,
        }
    }
}

impl Default for BeingRuntime {
    fn default() -> Self {
        let mut edges = BTreeMap::new();
        for e in EDGES {
            edges.insert(e.to_string(), true);
        }
        BeingRuntime {
            limits: Limits::defaults(LIMITS),
            sections: BTreeMap::new(),
            definitions: BTreeMap::new(),
            edges,
            base: BTreeMap::new(),
            extra: BTreeMap::new(),
            risk: BTreeMap::new(),
            carry: Vec::new(),
            steps: 0,
            applied: Vec::new(),
            trace: Vec::new(),
            voice: Vec::new(),
            odds: Vec::new(),
            budget: Vec::new(),
            snap: None,
            memo: None,
        }
    }
}

impl BeingRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    fn touch(&mut self, kind: &str) -> Result<()> {
        if let Some(snap) = self.snap.as_mut() {
            if !snap.contains_key(kind) {
                snap.insert(kind.to_string(), self.sections.get(kind).map(|s| s.copy()));
            }
        }
        if !self.sections.contains_key(kind) {
            let fresh = blank(kind).ok_or_else(c::bad)?;
            let key = fresh.kind();
            self.sections.insert(key, fresh);
            if kind == "culture" && self.edges.get("protect_witness").copied().unwrap_or(true) {
                self.run_one("culture", "protect", &obj! { "on" => true }, None)?;
                self.trace.push(obj! {
                    "edge" => "protect_witness",
                    "from" => Value::Null,
                    "skipped" => false,
                    "to" => "culture.protect",
                    "why" => "witnesses keep their own version of what they saw",
                });
            }
        }
        Ok(())
    }

    fn run_one(&mut self, kind: &str, op: &str, args: &Value, limits: Option<&Value>) -> Result<Value> {
        let section = self.sections.get_mut(kind).ok_or_else(c::bad)?;
        let req = match limits {
            Some(lim) => obj! {
                "ops" => Value::Array(vec![obj! { "op" => op, "args" => args.clone() }]),
                "limits" => lim.clone(),
            },
            None => obj! {
                "ops" => Value::Array(vec![obj! { "op" => op, "args" => args.clone() }]),
            },
        };
        let out = section.run(&req)?;
        let results = c::list(c::get(&out, "results")?)?;
        let res = results.first().ok_or_else(c::bad)?.clone();
        Ok(res)
    }

    fn query(&mut self, kind: &str, op: &str, args: &Value) -> Result<Option<Value>> {
        if !self.sections.contains_key(kind) {
            if kind != "genre" {
                return Ok(None);
            }
            self.touch(kind)?;
        }
        match self.run_one(kind, op, args, None) {
            Ok(v) => Ok(Some(v)),
            Err(_) => Ok(None),
        }
    }

    fn attempt_op(&mut self, kind: &str, op: &str, args: &Value) -> Result<Option<Value>> {
        self.touch(kind)?;
        let before = self.sections.get(kind).unwrap().copy();
        match self.run_one(kind, op, args, None) {
            Ok(res) => Ok(Some(res)),
            Err(_) => {
                self.sections.insert(before.kind(), before);
                Ok(None)
            }
        }
    }

    fn begin(&mut self) {
        self.snap = Some(BTreeMap::new());
        self.memo = Some((
            self.definitions.clone(),
            self.edges.clone(),
            self.base.clone(),
            self.extra.clone(),
            self.risk.clone(),
            self.carry.clone(),
            self.steps,
        ));
        self.applied = Vec::new();
        self.trace = Vec::new();
    }

    fn rollback(&mut self) {
        if let Some(snap) = self.snap.take() {
            for (kind, m) in snap {
                match m {
                    Some(saved) => {
                        self.sections.insert(saved.kind(), saved);
                    }
                    None => {
                        self.sections.remove(kind.as_str());
                    }
                }
            }
        }
        if let Some((defs, edges, base, extra, risk, carry, steps)) = self.memo.take() {
            self.definitions = defs;
            self.edges = edges;
            self.base = base;
            self.extra = extra;
            self.risk = risk;
            self.carry = carry;
            self.steps = steps;
        }
    }

    fn diff(&self) -> Result<Value> {
        let snap = self.snap.as_ref().ok_or_else(c::bad)?;
        let mut rows = Vec::new();
        let mut sorted_kinds: Vec<&String> = snap.keys().collect();
        sorted_kinds.sort();
        for kind in sorted_kinds {
            let old_state = match snap.get(kind).unwrap() {
                Some(s) => s.state(),
                None => blank(kind).ok_or_else(c::bad)?.state(),
            };
            let new_section = match self.sections.get(kind.as_str()) {
                Some(s) => s,
                None => continue,
            };
            let new_state = new_section.state();
            let a = old_state.as_object().ok_or_else(c::bad)?;
            let b = new_state.as_object().ok_or_else(c::bad)?;
            let mut changed = Vec::new();
            for (k, v) in b {
                let a_val = a.iter().find(|(k2, _)| k2 == k).map(|(_, v2)| v2);
                if a_val != Some(v) {
                    changed.push(Value::str(k));
                }
            }
            if !changed.is_empty() {
                rows.push(Value::Array(vec![Value::str(kind), Value::Array(changed)]));
            }
        }
        Ok(Value::Array(rows))
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "define" => self.op_define(args, l),
            "edges" => self.op_edges(args),
            "step" => self.op_step(args, l),
            "show" => self.op_show(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn op_show(&self, a: &Value) -> Result<Value> {
        keys(a, &["being"], &[])?;
        let b = c::text(c::get(a, "being")?)?;
        ident(&Value::str(b))?;
        let def = self.definitions.get(b).ok_or_else(c::bad)?;
        Ok(obj! { "being" => b, "definition" => def.clone() })
    }

    fn op_edges(&mut self, a: &Value) -> Result<Value> {
        keys(a, &["set"], &[])?;
        let s = c::get(a, "set")?;
        let s_obj = s.as_object().ok_or_else(c::bad)?;
        need(!s_obj.is_empty() && s_obj.len() <= EDGES.len())?;
        for (k, v) in s_obj {
            need(EDGES.contains(&k.as_str()) && v.as_bool().is_some())?;
        }
        self.begin();
        let was = self.edges.get("protect_witness").copied().unwrap_or(true);
        for (k, v) in s_obj {
            self.edges.insert(k.clone(), v.as_bool().unwrap());
        }
        if self.edges["protect_witness"] != was && self.sections.contains_key("culture") {
            self.touch("culture")?;
            let on = self.edges["protect_witness"];
            let res = self.run_one("culture", "protect", &obj! { "on" => on }, None)?;
            self.applied.push(obj! {
                "engine" => "culture",
                "op" => "protect",
                "result" => res,
                "source" => "edge:protect_witness",
            });
        }
        let edge_pairs: Vec<(&str, Value)> = self.edges.iter().map(|(k, v)| (k.as_str(), Value::Bool(*v))).collect();
        let out = obj! {
            "applied" => Value::Array(self.applied.clone()),
            "edges" => persist::object(edge_pairs),
        };
        self.snap = None;
        Ok(out)
    }

    fn op_define(&mut self, a: &Value, l: &Limits) -> Result<Value> {
        keys(a, &["being"], &PARTS)?;
        let b = c::text(c::get(a, "being")?)?;
        ident(&Value::str(b))?;
        need(!self.definitions.contains_key(b) && self.definitions.len() < l.get("definitions"))?;

        self.begin();
        let res = self.run_define(b, a);
        if res.is_err() {
            self.rollback();
            return res;
        }
        self.snap = None;
        res
    }

    fn run_define(&mut self, b: &str, a: &Value) -> Result<Value> {
        let mut reqs: Vec<(&'static str, &'static str, Value)> = Vec::new();
        let mut language = "full";
        let mut memory = "individual";

        if let Ok(m) = c::get(a, "mind") {
            if !matches!(m, Value::Null) {
                keys(m, &["id"], &["pool"])?;
                let mid = c::text(c::get(m, "id")?)?;
                ident(&Value::str(mid))?;
                self.touch("minds")?;
                let shown = self.run_one("minds", "show", &obj! { "id" => mid }, None)?;
                let lang_val = c::text(c::get(&shown, "language")?)?;
                language = match lang_val {
                    "none" => "none",
                    "signals" => "signals",
                    _ => "full",
                };
                let mem_val = c::text(c::get(&shown, "memory")?)?;
                memory = match mem_val {
                    "none" => "none",
                    "shared" => "shared",
                    _ => "individual",
                };
                let mut assign_pairs = vec![("being", Value::str(b)), ("mind", Value::str(mid))];
                if let Ok(pool) = c::get(m, "pool") {
                    if !matches!(pool, Value::Null) {
                        assign_pairs.push(("pool", pool.clone()));
                    }
                }
                reqs.push(("minds", "assign", persist::object(assign_pairs)));
            }
        }
        if let Ok(genre) = c::get(a, "genre") {
            if !matches!(genre, Value::Null) {
                let gid = c::text(genre)?;
                ident(&Value::str(gid))?;
                self.touch("genre")?;
                self.run_one("genre", "show", &obj! { "id" => gid }, None)?;
            }
        }
        if let Ok(species) = c::get(a, "species") {
            if !matches!(species, Value::Null) {
                let sp = c::text(species)?;
                ident(&Value::str(sp))?;
                reqs.push(("packs", "apply", obj! { "being" => b, "species" => sp }));
            }
        }
        if let Ok(st) = c::get(a, "style") {
            if !matches!(st, Value::Null) {
                need(st.as_object().is_some() && c::get(st, "being").is_err())?;
                need(language != "none")?;
                need(
                    language != "signals" || c::get(st, "humour").map(|h| h.as_str() == Some("none")).unwrap_or(false),
                )?;
                let mut st_pairs: Vec<(&str, Value)> = vec![("being", Value::str(b))];
                for (k, v) in st.as_object().unwrap() {
                    st_pairs.push((k.as_str(), v.clone()));
                }
                reqs.push(("chemistry", "style", persist::object(st_pairs)));
            }
        }
        if let Ok(cu) = c::get(a, "culture") {
            if !matches!(cu, Value::Null) {
                need(
                    cu.as_object().is_some() && c::get(cu, "being").is_err() && language != "none" && memory != "none",
                )?;
                let mut cu_pairs: Vec<(&str, Value)> = vec![("being", Value::str(b))];
                for (k, v) in cu.as_object().unwrap() {
                    cu_pairs.push((k.as_str(), v.clone()));
                }
                reqs.push(("culture", "member", persist::object(cu_pairs)));
            }
        }
        if let Ok(d) = c::get(a, "defense") {
            if !matches!(d, Value::Null) {
                need(d.as_object().is_some() && c::get(d, "being").is_err())?;
                let mut d_pairs: Vec<(&str, Value)> = vec![("being", Value::str(b))];
                for (k, v) in d.as_object().unwrap() {
                    d_pairs.push((k.as_str(), v.clone()));
                }
                reqs.push(("defenses", "profile", persist::object(d_pairs)));
            }
        }
        if let Ok(routines) = c::get(a, "routines") {
            if !matches!(routines, Value::Null) {
                let rs = c::list(routines)?;
                need(rs.len() <= 16 && memory != "none")?;
                for r in rs {
                    keys(r, &["slot", "action"], &[])?;
                    let slot = c::get(r, "slot")?;
                    let action = c::get(r, "action")?;
                    reqs.push((
                        "habit",
                        "choose",
                        obj! { "being" => b, "slot" => slot.clone(), "action" => action.clone() },
                    ));
                }
            }
        }
        if let Ok(cr) = c::get(a, "creed") {
            if !matches!(cr, Value::Null) {
                keys(cr, &["creed"], &["devotion"])?;
                let mut cr_pairs: Vec<(&str, Value)> = vec![("being", Value::str(b))];
                for (k, v) in cr.as_object().unwrap() {
                    cr_pairs.push((k.as_str(), v.clone()));
                }
                reqs.push(("creed", "adhere", persist::object(cr_pairs)));
            }
        }
        if let Ok(projects) = c::get(a, "projects") {
            if !matches!(projects, Value::Null) {
                let ps = c::list(projects)?;
                need(ps.len() <= 8)?;
                for p in ps {
                    need(p.as_object().is_some() && c::get(p, "owner").is_err())?;
                    let mut p_pairs: Vec<(&str, Value)> = vec![("owner", Value::str(b))];
                    for (k, v) in p.as_object().unwrap() {
                        p_pairs.push((k.as_str(), v.clone()));
                    }
                    reqs.push(("projects", "propose", persist::object(p_pairs)));
                }
            }
        }

        reqs.sort_by_key(|r| order(r.0).unwrap());
        let mut results = Vec::new();
        for (kind, op, r_args) in &reqs {
            self.touch(kind)?;
            results.push(self.run_one(kind, op, r_args, None)?);
        }

        let mut def_pairs: Vec<(&str, Value)> = Vec::new();
        if let Some(pairs) = a.as_object() {
            for (k, v) in pairs {
                if k != "being" && !matches!(v, Value::Null) {
                    def_pairs.push((k.as_str(), v.clone()));
                }
            }
        }
        self.definitions.insert(b.to_string(), persist::object(def_pairs));

        let requests_val: Vec<Value> =
            reqs.into_iter().map(|(k, o, x)| obj! { "args" => x, "engine" => k, "op" => o }).collect();

        Ok(obj! {
            "being" => b,
            "requests" => Value::Array(requests_val),
            "results" => Value::Array(results),
            "trace" => Value::Array(self.trace.clone()),
        })
    }

    fn op_step(&mut self, a: &Value, l: &Limits) -> Result<Value> {
        keys(a, &["events"], &["seed"])?;
        let events_raw = c::list(c::get(a, "events")?)?;
        need(events_raw.len() <= MAX_OPS)?;
        let seed = match a.get("seed") {
            Some(s) => c::uint(s, 0, MAX_SEED)?,
            None => 0,
        };

        let mut checked_events = Vec::new();
        for e in events_raw {
            checked_events.push(self.check_event(e)?);
        }

        self.begin();
        let res = self.run_step(checked_events, seed, l);
        if res.is_err() {
            self.rollback();
            return res;
        }
        self.snap = None;
        res
    }

    fn check_event(&self, e: &Value) -> Result<QueueItem> {
        need(e.as_object().is_some())?;
        let engine = c::text(c::get(e, "engine")?)?;
        need(order(engine).is_some())?;
        if engine == "attempt" {
            keys(e, &["engine", "args"], &[])?;
            let x = c::get(e, "args")?;
            keys(x, &["being", "task", "odds"], &[])?;
            let being = c::text(c::get(x, "being")?)?;
            ident(&Value::str(being))?;
            let task = c::text(c::get(x, "task")?)?;
            ident(&Value::str(task))?;
            let odds = c::uint(c::get(x, "odds")?, 0, 1000)?;
            return Ok(QueueItem::Attempt(AttemptArgs { being: being.to_string(), task: task.to_string(), odds }));
        }

        keys(e, &["engine", "op"], &["args", "being", "beings", "limits"])?;
        let op = c::text(c::get(e, "op")?)?;
        need(op.len() <= 32 && op != "export" && op != "import")?;
        let args = match e.get("args") {
            Some(a) => {
                need(a.as_object().is_some())?;
                a.clone()
            }
            None => obj! {},
        };
        let mut being = e.get("being").and_then(|v| v.as_str()).map(ToString::to_string);
        if being.is_none() {
            if let Some(b) = args.get("being").and_then(|v| v.as_str()) {
                being = Some(b.to_string());
            }
        }
        if let Some(ref b) = being {
            ident(&Value::str(b))?;
        }
        let beings = match e.get("beings") {
            Some(bs_val) => {
                let bs = c::list(bs_val)?;
                need(bs.len() <= 64)?;
                let mut v = Vec::new();
                for b in bs {
                    let b_str = c::text(b)?;
                    ident(&Value::str(b_str))?;
                    v.push(b_str.to_string());
                }
                Some(v)
            }
            None => None,
        };
        let limits = match e.get("limits") {
            Some(lim) => {
                need(lim.as_object().is_some())?;
                Some(lim.clone())
            }
            None => None,
        };

        Ok(QueueItem::Event(
            CheckedEvent { engine: engine.to_string(), op: op.to_string(), args, being, beings, limits },
            0,
        ))
    }

    fn run_step(&mut self, events: Vec<QueueItem>, seed: u64, l: &Limits) -> Result<Value> {
        self.steps += 1;
        self.voice = Vec::new();
        self.odds = Vec::new();
        self.budget = Vec::new();

        let mut queue: Vec<(usize, usize, QueueItem)> = Vec::new();
        let mut seq = 0;
        for c in &self.carry {
            let ord = order(&c.0).unwrap();
            queue.push((ord, seq, QueueItem::Carried(c.clone())));
            seq += 1;
        }
        self.carry.clear();

        for (i, e) in events.into_iter().enumerate() {
            match e {
                QueueItem::Attempt(a) => {
                    let ord = order("attempt").unwrap();
                    queue.push((ord, seq, QueueItem::Attempt(a)));
                    seq += 1;
                }
                QueueItem::Event(ce, _) => {
                    let ord = order(&ce.engine).unwrap();
                    queue.push((ord, seq, QueueItem::Event(ce, i)));
                    seq += 1;
                }
                _ => {}
            }
        }
        queue.sort_by_key(|q| (q.0, q.1));

        let mut carried_out: Vec<(String, String, Value, String, String)> = Vec::new();
        while !queue.is_empty() {
            let (_, _, item) = queue.remove(0);
            let mut new_items: Vec<QueuedOp> = Vec::new();
            let parent_order = match item {
                QueueItem::Carried(ref c) => {
                    let ord = order(&c.0).unwrap();
                    self.run_edge_op(
                        QueuedOp {
                            engine: c.0.clone(),
                            op: c.1.clone(),
                            args: c.2.clone(),
                            edge: c.3.clone(),
                            from: None,
                            why: c.4.clone(),
                            on_ok: None,
                        },
                        true,
                        &mut new_items,
                    )?;
                    ord
                }
                QueueItem::Edge(ref x) => {
                    let ord = order(&x.engine).unwrap();
                    self.run_edge_op(x.clone(), false, &mut new_items)?;
                    ord
                }
                QueueItem::Attempt(ref a) => {
                    self.run_attempt(a, seed)?;
                    order("attempt").unwrap()
                }
                QueueItem::Event(ref ce, idx) => {
                    let ord = order(&ce.engine).unwrap();
                    self.run_event(ce.clone(), idx, &mut new_items, &mut carried_out)?;
                    ord
                }
            };
            for x in new_items {
                let child_order = order(&x.engine).unwrap();
                need(child_order >= parent_order)?;
                queue.push((child_order, seq, QueueItem::Edge(x)));
                seq += 1;
            }
            queue.sort_by_key(|q| (q.0, q.1));
        }

        need(carried_out.len() <= l.get("carry"))?;
        let carried_count = carried_out.len() as u64;
        self.carry = carried_out;

        let diff = self.diff()?;
        let mut out_fields: Vec<(&str, Value)> = Vec::new();
        out_fields.push(("applied", Value::Array(self.applied.clone())));
        if self.edges.get("salience_budget").copied().unwrap_or(true) {
            out_fields.push(("budget", Value::Array(self.budget.clone())));
        }
        out_fields.push(("carried", Value::from(carried_count)));
        out_fields.push(("diff", diff));
        if self.edges.get("failure_odds").copied().unwrap_or(true) {
            out_fields.push(("odds", Value::Array(self.odds.clone())));
        }
        out_fields.push(("step", Value::from(self.steps)));
        out_fields.push(("trace", Value::Array(self.trace.clone())));
        if self.edges.get("voice_hints").copied().unwrap_or(true) {
            out_fields.push(("voice", Value::Array(self.voice.clone())));
        }

        Ok(persist::object(out_fields))
    }

    fn run_event(
        &mut self,
        e: CheckedEvent,
        idx: usize,
        new_items: &mut Vec<QueuedOp>,
        carried_out: &mut Vec<(String, String, Value, String, String)>,
    ) -> Result<()> {
        let kind = e.engine.clone();
        let op = e.op.clone();
        let mut args = e.args.clone();
        let being = e.being.clone();
        let source = format!("event:{}", idx);

        let mut genre: Option<String> = None;
        if self.edges.get("genre_words").copied().unwrap_or(true) && (kind == "fabric" || kind == "surroundings") {
            if let Some(ref b) = being {
                if let Some(def) = self.definitions.get(b) {
                    if let Some(g) = def.get("genre").and_then(|v| v.as_str()) {
                        genre = Some(g.to_string());
                    }
                }
            }
        }

        if let Some(ref g) = genre {
            self.touch("genre")?;
            let keys_obj = genre_keys(&kind);
            let out = self.run_one(
                "genre",
                "normalize",
                &obj! { "genre" => g.as_str(), "keys" => keys_obj, "value" => args.clone() },
                None,
            )?;
            let val = c::get(&out, "value")?;
            if *val != args {
                self.trace.push(obj! {
                    "edge" => "genre_words",
                    "from" => Value::Null,
                    "skipped" => false,
                    "to" => format!("{}.{}", kind, op),
                    "why" => format!("{}'s words in {} became engine words", being.as_deref().unwrap_or(""), g),
                });
            }
            args = val.clone();
        }

        if self.edges.get("minds_admit").copied().unwrap_or(true)
            && ADMITTED.contains(&kind.as_str())
            && self.sections.contains_key("minds")
        {
            if let Some(b) = args.get("being").and_then(|v| v.as_str()) {
                let adm = self.query(
                    "minds",
                    "admit",
                    &obj! { "being" => b, "kind" => kind.as_str(), "op" => op.as_str(), "args" => args.clone() },
                )?;
                if let Some(ref adm_val) = adm {
                    let allowed = c::boolean(c::get(adm_val, "allowed")?)?;
                    if !allowed {
                        self.applied.push(obj! {
                            "engine" => kind.as_str(),
                            "op" => op.as_str(),
                            "result" => obj! { "not_admitted" => adm_val.clone() },
                            "source" => source,
                        });
                        let reasons_list = c::list(c::get(adm_val, "reasons")?)?;
                        let reasons_str: Vec<&str> = reasons_list.iter().filter_map(|r| r.as_str()).collect();
                        let applied_idx = (self.applied.len() - 1) as u64;
                        self.trace.push(obj! {
                            "edge" => "minds_admit",
                            "from" => applied_idx,
                            "skipped" => false,
                            "to" => format!("{}.{}", kind, op),
                            "why" => format!("{}'s mind cannot do this: {}", b, reasons_str.join(", ")),
                        });
                        return Ok(());
                    }
                    let adm_args = c::get(adm_val, "args")?;
                    if *adm_args != args {
                        self.trace.push(obj! {
                            "edge" => "minds_admit",
                            "from" => Value::Null,
                            "skipped" => false,
                            "to" => format!("{}.{}", kind, op),
                            "why" => format!("{}'s mind changed the request", b),
                        });
                        args = adm_args.clone();
                    }
                }
            }
        }

        if self.edges.get("chemistry_cooperation").copied().unwrap_or(true)
            && kind == "cooperation"
            && op == "ask_help"
            && self.sections.contains_key("chemistry")
        {
            if let Some(trust_val) = args.get("trust") {
                if let Ok(trust_num) = sint(trust_val, -100, 100) {
                    let asker = args.get("asker").and_then(|v| v.as_str()).unwrap_or("");
                    let helper = args.get("helper").and_then(|v| v.as_str()).unwrap_or("");
                    let pair = self.query("chemistry", "pair", &obj! { "a" => asker, "b" => helper })?;
                    if let Some(ref p) = pair {
                        let bias = sint(c::get(p, "cooperativeness_bias")?, -100, 100)?;
                        let clamped = (-100).max(100.min(trust_num + bias));
                        if clamped != trust_num {
                            let chem_score = sint(c::get(p, "chemistry")?, 0, 100)?;
                            self.trace.push(obj! {
                                "edge" => "chemistry_cooperation",
                                "from" => Value::Null,
                                "skipped" => false,
                                "to" => "cooperation.ask_help",
                                "why" => format!("chemistry {} moved trust from {} to {}", chem_score, trust_num, clamped),
                            });
                            if let Value::Object(ref mut pairs) = args {
                                if let Some((_, v)) = pairs.iter_mut().find(|(k, _)| k == "trust") {
                                    *v = Value::from(clamped);
                                }
                            }
                        }
                    }
                }
            }
        }

        self.touch(&kind)?;
        let mut result = self.run_one(&kind, &op, &args, e.limits.as_ref())?;

        if let Some(ref g) = genre {
            let keys_obj = genre_keys(&kind);
            let shown = self.query(
                "genre",
                "render",
                &obj! { "genre" => g.as_str(), "keys" => keys_obj, "value" => result.clone() },
            )?;
            match shown {
                None => {
                    self.trace.push(obj! {
                        "edge" => "genre_words",
                        "from" => Value::Null,
                        "skipped" => true,
                        "to" => format!("{}.{}", kind, op),
                        "why" => format!("the result was too large to render in {}'s words", g),
                    });
                }
                Some(shown_val) => {
                    result = c::get(&shown_val, "value")?.clone();
                }
            }
        }

        self.applied.push(obj! {
            "engine" => kind.as_str(),
            "op" => op.as_str(),
            "result" => result.clone(),
            "source" => source,
        });

        self.observe(&kind, &op, &args, &result, being.as_deref(), e.beings.as_deref(), new_items, carried_out)?;
        Ok(())
    }

    fn run_edge_op(&mut self, x: QueuedOp, carried: bool, new_items: &mut Vec<QueuedOp>) -> Result<()> {
        let result = self.attempt_op(&x.engine, &x.op, &x.args)?;
        let src = format!("{}{}", if carried { "carry:" } else { "edge:" }, x.edge);
        let to = format!("{}.{}", x.engine, x.op);
        match result {
            None => {
                self.trace.push(obj! {
                    "edge" => x.edge.as_str(),
                    "from" => match x.from { Some(f) => Value::from(f), None => Value::Null },
                    "skipped" => true,
                    "to" => to,
                    "why" => x.why.as_str(),
                });
            }
            Some(res) => {
                self.applied.push(obj! {
                    "engine" => x.engine.as_str(),
                    "op" => x.op.as_str(),
                    "result" => res.clone(),
                    "source" => src,
                });
                self.trace.push(obj! {
                    "edge" => x.edge.as_str(),
                    "from" => match x.from { Some(f) => Value::from(f), None => Value::Null },
                    "skipped" => false,
                    "to" => to,
                    "why" => x.why.as_str(),
                });
                if let Some((b, n, v)) = x.on_ok {
                    if v > 0 {
                        self.extra.insert((b, n), v);
                    } else {
                        self.extra.remove(&(b, n));
                    }
                }
                self.observe(&x.engine, &x.op, &x.args, &res, None, None, new_items, &mut Vec::new())?;
            }
        }
        Ok(())
    }

    fn run_attempt(&mut self, x: &AttemptArgs, seed: u64) -> Result<()> {
        if !self.edges.get("failure_odds").copied().unwrap_or(true) {
            return Ok(());
        }
        let risk = self.risk.get(&x.being).copied().unwrap_or(0);
        let odds = x.odds * (100 - risk) / 100;
        let roll = fnv1a(&format!("{}:{}:{}:{}", seed, x.being, x.task, self.steps)) % 1000;
        self.odds.push(obj! {
            "base" => x.odds,
            "being" => x.being.as_str(),
            "odds" => odds,
            "risk" => risk,
            "roll" => roll,
            "success" => roll < odds,
            "task" => x.task.as_str(),
        });
        self.trace.push(obj! {
            "edge" => "failure_odds",
            "from" => Value::Null,
            "skipped" => false,
            "to" => "output.odds",
            "why" => format!("{}'s tension risk {}% turned odds {} into {}", x.being, risk, x.odds, odds),
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn observe(
        &mut self,
        kind: &str,
        op: &str,
        args: &Value,
        r: &Value,
        being: Option<&str>,
        beings: Option<&[String]>,
        new_items: &mut Vec<QueuedOp>,
        carried_out: &mut Vec<(String, String, Value, String, String)>,
    ) -> Result<()> {
        let at = (self.applied.len() - 1) as u64;

        if kind == "detail" && op == "track" {
            let being_str = c::text(c::get(args, "being")?)?;
            let needs = c::list(c::get(args, "needs")?)?;
            for n in needs {
                let nid = c::text(c::get(n, "id")?)?;
                let rate = c::uint(c::get(n, "rate")?, 0, 1_000_000)?;
                self.base.insert((being_str.to_string(), nid.to_string()), rate);
                self.extra.remove(&(being_str.to_string(), nid.to_string()));
            }
        }

        if kind == "detail" && op == "rate" && being.is_some() {
            let being_str = c::text(c::get(args, "being")?)?;
            let need_id = c::text(c::get(args, "need")?)?;
            let rate = c::uint(c::get(args, "rate")?, 0, 1_000_000)?;
            self.base.insert((being_str.to_string(), need_id.to_string()), rate);
            self.extra.remove(&(being_str.to_string(), need_id.to_string()));
        }

        if self.edges.get("threat_feed").copied().unwrap_or(true) && kind == "fabric" && op == "breach" {
            let step = c::uint(c::get(r, "step")?, 0, 1000)?;
            if step >= 2 {
                let offender = c::text(c::get(r, "offender")?)?;
                let norm = c::text(c::get(r, "norm")?)?;
                new_items.push(QueuedOp {
                    engine: "defenses".into(),
                    op: "threat".into(),
                    args: obj! {
                        "being" => offender,
                        "value" => "belonging",
                        "drop" => (20 * step).min(100),
                        "fact" => format!("breach_{}", norm),
                    },
                    edge: "threat_feed".into(),
                    from: Some(at),
                    why: format!("{} was sanctioned at step {} for breaching {}", offender, step, norm),
                    on_ok: None,
                });
            }
        }

        if self.edges.get("threat_feed").copied().unwrap_or(true) && kind == "lesson" && op == "outcome" {
            if let Ok(les) = c::get(r, "lesson") {
                if !matches!(les, Value::Null) {
                    let verdict = c::text(c::get(les, "verdict")?)?;
                    let change = c::text(c::get(r, "change")?)?;
                    if verdict == "avoid" && (change == "learned" || change == "revised") {
                        let b = c::text(c::get(r, "being")?)?;
                        let conf = c::uint(c::get(les, "confidence")?, 0, 100)?;
                        let about = c::text(c::get(les, "about")?)?;
                        let act = c::text(c::get(les, "activity")?)?;
                        let sum = c::text(c::get(les, "summary")?)?;
                        carried_out.push((
                            "defenses".into(),
                            "threat".into(),
                            obj! {
                                "being" => b,
                                "value" => "competence",
                                "drop" => (conf / 2).min(100),
                                "fact" => format!("lesson_{}_{}", about, act),
                            },
                            "threat_feed".into(),
                            format!("{} learned that {}", b, sum),
                        ));
                    }
                }
            }
        }

        if self.edges.get("project_feeds").copied().unwrap_or(true)
            && kind == "projects"
            && (op == "outcome" || op == "setback" || op == "offscreen")
        {
            let res = if op == "offscreen" { c::get(r, "result").ok() } else { Some(r) };
            if let Some(res_val) = res {
                if !matches!(res_val, Value::Null) {
                    if let Ok(dec) = c::get(res_val, "decision").and_then(c::text) {
                        if dec == "complete" || dec == "abandon" {
                            let proj = c::get(res_val, "project")?;
                            let pid = c::text(c::get(proj, "id")?)?;
                            let owner = c::text(c::get(proj, "owner")?)?;
                            let done = dec == "complete";
                            let why = format!(
                                "{}'s project {} was {}",
                                owner,
                                pid,
                                if done { "completed" } else { "abandoned" }
                            );
                            let feeds_list = c::list(c::get(res_val, "feeds")?)?;
                            let feeds: Vec<&str> = feeds_list.iter().filter_map(|x| x.as_str()).collect();
                            if feeds.contains(&"lesson") {
                                new_items.push(QueuedOp {
                                    engine: "lesson".into(),
                                    op: "outcome".into(),
                                    args: obj! {
                                        "being" => owner,
                                        "about" => pid,
                                        "activity" => "project",
                                        "result" => if done { "good" } else { "bad" },
                                        "episode" => format!("step_{}", self.steps),
                                    },
                                    edge: "project_feeds".into(),
                                    from: Some(at),
                                    why: why.clone(),
                                    on_ok: None,
                                });
                            }
                            if feeds.contains(&"drift") {
                                new_items.push(QueuedOp {
                                    engine: "drift".into(),
                                    op: "experience".into(),
                                    args: obj! {
                                        "being" => owner,
                                        "trait" => "resolve",
                                        "push" => if done { 40 } else { -40 },
                                        "reason" => format!("project_{}", pid),
                                    },
                                    edge: "project_feeds".into(),
                                    from: Some(at),
                                    why: why.clone(),
                                    on_ok: None,
                                });
                            }
                            if feeds.contains(&"aspiration") {
                                new_items.push(QueuedOp {
                                    engine: "aspiration".into(),
                                    op: "experience".into(),
                                    args: obj! {
                                        "being" => owner,
                                        "kind" => if done { "opportunity" } else { "loss" },
                                        "subject" => pid,
                                        "weight" => 50,
                                    },
                                    edge: "project_feeds".into(),
                                    from: Some(at),
                                    why,
                                    on_ok: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        if self.edges.get("culture_retellings").copied().unwrap_or(true) && kind == "chatter" && op == "meet" {
            let rec = c::get(r, "record")?;
            let rec_id = c::uint(c::get(rec, "id")?, 0, MAX_INT)?;
            let a = c::text(c::get(rec, "a")?)?;
            let b = c::text(c::get(rec, "b")?)?;
            let turns = c::list(c::get(rec, "turns")?)?;
            for (k, turn) in turns.iter().enumerate() {
                let tkind = c::text(c::get(turn, "kind")?)?;
                if tkind == "tell" {
                    let spk = c::text(c::get(turn, "speaker")?)?;
                    let lis = if spk == a { b } else { a };
                    let topic = c::text(c::get(turn, "topic")?)?;
                    let seed = fnv1a(&format!("{}:{}", rec_id, k)) % MAX_SEED;
                    new_items.push(QueuedOp {
                        engine: "culture".into(),
                        op: "tell".into(),
                        args: obj! {
                            "speaker" => spk,
                            "listener" => lis,
                            "topic" => topic,
                            "seed" => seed,
                        },
                        edge: "culture_retellings".into(),
                        from: Some(at),
                        why: format!("{} retold {} to {}", spk, topic, lis),
                        on_ok: None,
                    });
                }
            }
        }

        if self.edges.get("traces_beliefs").copied().unwrap_or(true) && kind == "surroundings" && op == "perceive" {
            if let Some(b) = being {
                let place = c::text(c::get(r, "place")?)?;
                let traces = c::list(c::get(r, "traces")?)?;
                for t in traces {
                    let tkind = c::text(c::get(t, "kind")?)?;
                    let imp = c::text(c::get(t, "implies")?)?;
                    new_items.push(QueuedOp {
                        engine: "beliefs".into(),
                        op: "learn".into(),
                        args: obj! {
                            "being" => b,
                            "about" => place,
                            "relation" => "shows",
                            "value" => imp,
                            "source" => "saw",
                        },
                        edge: "traces_beliefs".into(),
                        from: Some(at),
                        why: format!("{} saw {} at {}", b, tkind, place),
                        on_ok: None,
                    });
                }
            }
        }

        if self.edges.get("weather_needs").copied().unwrap_or(true) && kind == "surroundings" && op == "conditions" {
            let rates_val = c::get(r, "need_rates")?;
            need(rates_val.as_object().is_some())?;
            for ((b, n), base_rate) in &self.base {
                if let Some(who) = beings {
                    if !who.iter().any(|x| x == b) {
                        continue;
                    }
                }
                let rate_num = rates_val.get(n).and_then(|v| v.as_usize()).unwrap_or(0) as u64;
                let want = rate_num * 1000;
                let current_extra = self.extra.get(&(b.clone(), n.clone())).copied().unwrap_or(0);
                if current_extra != want {
                    let new_rate = (base_rate + want).min(1_000_000);
                    new_items.push(QueuedOp {
                        engine: "detail".into(),
                        op: "rate".into(),
                        args: obj! {
                            "being" => b.as_str(),
                            "need" => n.as_str(),
                            "rate" => new_rate,
                        },
                        edge: "weather_needs".into(),
                        from: Some(at),
                        why: format!("the weather moves {}'s {} by {} per tick", b, n, want),
                        on_ok: Some((b.clone(), n.clone(), want)),
                    });
                }
            }
        }

        if kind == "embodiment" && op == "tension" {
            let b = c::text(c::get(r, "being")?)?;
            if self.edges.get("failure_odds").copied().unwrap_or(true) {
                let eff = c::get(r, "effects")?;
                let risk = c::uint(c::get(eff, "failure_risk")?, 0, 100)?;
                self.risk.insert(b.to_string(), risk);
            }
            if self.edges.get("voice_hints").copied().unwrap_or(true) {
                let dec = c::boolean(c::get(r, "deceiving")?)?;
                let mode = c::text(c::get(r, "mode")?)?;
                let hints = self.query("embodiment", "voice", &obj! { "deceiving" => dec, "tension" => mode })?;
                if let Some(h) = hints {
                    self.voice.push(obj! { "being" => b, "hints" => h.clone() });
                    let pacing = c::text(c::get(&h, "pacing")?)?;
                    self.trace.push(obj! {
                        "edge" => "voice_hints",
                        "from" => at,
                        "skipped" => false,
                        "to" => "output.voice",
                        "why" => format!("{} sounds {} ({})", b, pacing, mode),
                    });
                }
            }
        }

        if self.edges.get("salience_budget").copied().unwrap_or(true) && kind == "turn-frame" && op == "rank" {
            let kept_raw = c::list(c::get(r, "kept")?)?;
            let mut total: u64 = 0;
            for k in kept_raw {
                total += c::uint(c::get(k, "score")?, 0, MAX_INT)?;
            }
            let mut kept = Vec::new();
            for k in kept_raw {
                let kid = c::text(c::get(k, "id")?)?;
                let score = c::uint(c::get(k, "score")?, 0, MAX_INT)?;
                let share = (score * 1000).checked_div(total).unwrap_or(0);
                kept.push(obj! { "id" => kid, "share" => share });
            }
            self.budget.push(obj! {
                "being" => being.map(Value::str).unwrap_or(Value::Null),
                "kept" => Value::Array(kept.clone()),
            });
            self.trace.push(obj! {
                "edge" => "salience_budget",
                "from" => at,
                "skipped" => false,
                "to" => "output.budget",
                "why" => format!("{} kept items share the packet budget by salience", kept.len()),
            });
        }

        Ok(())
    }

    fn settle_state(&mut self) -> Result<()> {
        persist::within::<Self>("definitions", self.definitions.len())?;
        persist::within::<Self>("carry", self.carry.len())?;
        need(self.edges.len() == EDGES.len() && EDGES.iter().all(|e| self.edges.contains_key(*e)))?;
        for b in self.definitions.keys() {
            need(persist::is_ident(b))?;
        }
        for ((b, n), rate) in &self.base {
            need(persist::is_ident(b) && persist::is_ident(n) && *rate <= 1_000_000)?;
        }
        for ((b, n), rate) in &self.extra {
            need(self.base.contains_key(&(b.clone(), n.clone())) && *rate <= 1_000_000)?;
        }
        for (b, r) in &self.risk {
            need(persist::is_ident(b) && *r <= 100)?;
        }
        for c in &self.carry {
            need(order(&c.0).is_some() && EDGES.contains(&c.3.as_str()))?;
        }
        for k in self.sections.keys() {
            need(KINDS.contains(k))?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

impl Persist for BeingRuntime {
    fn save(&self) -> Value {
        let mut sections = BTreeMap::new();
        for (k, s) in &self.sections {
            sections.insert(k.to_string(), s.save());
        }
        let carry: Vec<Value> = self
            .carry
            .iter()
            .map(|c| {
                Value::Array(vec![Value::str(&c.0), Value::str(&c.1), c.2.clone(), Value::str(&c.3), Value::str(&c.4)])
            })
            .collect();
        persist::object(vec![
            ("base", self.base.save()),
            ("carry", Value::Array(carry)),
            ("definitions", self.definitions.save()),
            ("edges", self.edges.save()),
            ("extra", self.extra.save()),
            ("risk", self.risk.save()),
            ("sections", sections.save()),
            ("steps", self.steps.save()),
        ])
    }

    fn load(v: &Value) -> Result<Self> {
        c::fields(v, &["base", "carry", "definitions", "edges", "extra", "risk", "sections", "steps"])?;
        let base = BTreeMap::<(String, String), u64>::load(c::get(v, "base")?)?;
        let carry_raw = c::list(c::get(v, "carry")?)?;
        let mut carry = Vec::new();
        for c_val in carry_raw {
            let [eng, op, args, edge, why] = c::list(c_val)? else { return Err(c::bad()) };
            carry.push((
                c::text(eng)?.to_string(),
                c::text(op)?.to_string(),
                args.clone(),
                c::text(edge)?.to_string(),
                c::text(why)?.to_string(),
            ));
        }
        let definitions = BTreeMap::<String, Value>::load(c::get(v, "definitions")?)?;
        let edges = BTreeMap::<String, bool>::load(c::get(v, "edges")?)?;
        let extra = BTreeMap::<(String, String), u64>::load(c::get(v, "extra")?)?;
        let risk = BTreeMap::<String, u64>::load(c::get(v, "risk")?)?;
        let sections_raw = BTreeMap::<String, Value>::load(c::get(v, "sections")?)?;
        let steps = u64::load(c::get(v, "steps")?)?;

        let mut sections = BTreeMap::new();
        for (kind_str, save_val) in &sections_raw {
            let spare = blank(kind_str).ok_or_else(c::bad)?;
            let (m, _ver, _saved) = spare.restore(save_val)?;
            sections.insert(m.kind(), m);
        }

        Ok(BeingRuntime {
            limits: Limits::defaults(LIMITS),
            sections,
            definitions,
            edges,
            base,
            extra,
            risk,
            carry,
            steps,
            applied: Vec::new(),
            trace: Vec::new(),
            voice: Vec::new(),
            odds: Vec::new(),
            budget: Vec::new(),
            snap: None,
            memo: None,
        })
    }
}

durable!(BeingRuntime, "being-runtime", LIMITS, 1);

/// Runs a fresh being runtime on `{ops, limits?}` (the `being-runtime` local kind).
pub fn workflow(request: &Value) -> Result<Value> {
    BeingRuntime::new().workflow(request)
}
