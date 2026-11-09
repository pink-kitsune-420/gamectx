//! Graded norms with sanction ladders, social contracts and favor balance, dyad memory, and
//! attention and pragmatic profiles (pass 3, stage 5).
//!
//! **Norms.** `norm` declares a severity (`etiquette` 10, `custom` 30, `contract` 60, `taboo`
//! 80, `crime` 100), the situations it applies in, a guilt cost and the role a severe breach is
//! reported to. `breach` climbs a sanction ladder per norm, offender and witness: a minor first
//! breach earns a verbal warning, a repeat or a contract breach refused service and a demand for
//! an apology or a fine, a taboo or crime (or a third repeat) flight or confrontation and a
//! report. Etiquette never goes past the second step. An offender with a conscience of 50 or
//! more feels the guilt cost and wants to make amends. `apologize` steps the ladder back once.
//!
//! **Contracts.** `contract` records a promise, loan, trade order, kept secret, alibi pact,
//! employment, owed apology or favor between a debtor and a creditor, with witnesses, a due
//! time, terms and a weight; `tick` marks overdue ones; `settle` closes one as fulfilled (trust
//! up a quarter of the weight), released, or broken (trust down by the weight, and the witnesses
//! learn of it); `ledger` sums what two beings owe each other and how much more readily each
//! would help the other.
//!
//! **Dyads.** `meet` records how two beings first met; `landmark` keeps up to four defining
//! moments (the same kind replaces, the oldest drops); `address` sets how one addresses the
//! other; `thread` and `resolve` keep their unfinished business; `dyad` shows it all (strangers
//! by default).
//!
//! **Profiles.** `profile` sets a being's attention (bandwidth, vigilance, per-category salience
//! biases) and pragmatic style (conflict style, baseline cooperativeness, interrupt tolerance,
//! small-talk patience, secret keeping, lying tells, coping style). `notice` ranks stimuli
//! through the attention profile; `tells` says which lying tells an observer of a given insight
//! perceives; `style` turns the pragmatic profile into reactions to interruption and small talk.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::knowledge::fabric::Fabric;
//!
//! let mut f = Fabric::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "norm", "args": {"id": "no_theft", "severity": "crime", "report_to": "town_watch"}},
//!   {"op": "breach", "args": {"norm": "no_theft", "offender": "apprentice", "witness": "mira",
//!     "conscience": 70}},
//!   {"op": "contract", "args": {"id": "firewood", "kind": "promise", "debtor": "player",
//!     "creditor": "mira", "terms": "Bring a bundle of firewood", "due_at": 10}},
//!   {"op": "tick", "args": {"now": 11}},
//!   {"op": "ledger", "args": {"a": "mira", "b": "player"}}
//! ]}"#, 4096)?;
//! let out = f.workflow(&req)?.to_json();
//! // A theft goes straight to the top of the ladder; the guilty apprentice wants to make amends.
//! assert!(out.contains(r#""sanctions":["flee_or_confront","report_to_role"],"step":3"#));
//! assert!(out.contains(r#""amends":true"#));
//! // The late firewood is overdue, and the player owes Mira a favor's weight.
//! assert!(out.contains(r#""b_owes_a":20"#) && out.contains(r#""overdue":["firewood"]"#));
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
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "norms", default: 64, ceiling: 4096 },
    Limit { name: "breaches", default: 1024, ceiling: 1_000_000 },
    Limit { name: "contracts", default: 256, ceiling: 1_000_000 },
    Limit { name: "dyads", default: 1024, ceiling: 1_000_000 },
    Limit { name: "threads", default: 4, ceiling: 32 },
];
pub const SEVERITY: [(&str, u64); 5] =
    [("etiquette", 10), ("custom", 30), ("contract", 60), ("taboo", 80), ("crime", 100)];
pub const KINDS: [&str; 8] =
    ["alibi_pact", "apology_owed", "employment", "favor", "keep_secret", "loan", "promise", "trade_order"];
pub const OUTCOMES: [&str; 3] = ["fulfilled", "released", "broken"];
pub const STYLES: [&str; 4] = ["direct", "avoidant", "diplomatic", "passive_aggressive"];
pub const COOP: [&str; 6] = ["eager", "cooperative", "guarded", "reluctant", "deflecting", "hostile_refusal"];
pub const TELL_KINDS: [&str; 4] = ["gaze", "speech", "posture", "hands"];
/// Defining moments kept per pair.
pub const LANDMARKS: usize = 4;
/// Stimuli per `notice` call.
pub const STIMULI: usize = 64;

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

fn id_list(v: Option<&Value>, hi: usize) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for x in list_bound(v, hi)? {
        let s = ident(x)?.to_string();
        need(seen.insert(s.clone()))?;
        out.push(s);
    }
    Ok(out)
}

fn text_of(args: &Value, k: &str, max: usize) -> Result<String> {
    Ok(text_bound(c::get(args, k)?, max)?.to_string())
}

fn pair(a: &str, b: &str) -> Result<(String, String)> {
    need(a != b)?;
    Ok(if a < b { (a.to_string(), b.to_string()) } else { (b.to_string(), a.to_string()) })
}

fn strings(xs: &[String]) -> Value {
    Value::Array(xs.iter().map(|s| Value::from(s.as_str())).collect())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Norm {
    pub severity: u64,
    pub applies_when: Vec<String>,
    pub guilt_cost: u64,
    pub report_to: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contract {
    pub kind: &'static str,
    pub debtor: String,
    pub creditor: String,
    pub witnesses: Vec<String>,
    pub due_at: Option<u64>,
    pub terms: String,
    pub weight: u64,
    pub overdue: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moment {
    pub at: u64,
    pub episode: String,
    pub kind: String,
    pub summary: Option<String>,
}

impl Moment {
    fn view(&self) -> Value {
        obj! {
            "at" => self.at, "episode" => self.episode.as_str(), "kind" => self.kind.as_str(),
            "summary" => self.summary.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub opened_by: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dyad {
    pub first: (u64, String, String),
    pub landmarks: Vec<Moment>,
    pub address: BTreeMap<String, String>,
    pub threads: Vec<Thread>,
}

impl Dyad {
    fn first_view(&self) -> Value {
        obj! { "at" => self.first.0, "episode" => self.first.1.as_str(), "summary" => self.first.2.as_str() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attention {
    pub bandwidth: u64,
    pub vigilance: u64,
    pub biases: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tell {
    pub kind: &'static str,
    pub cue: String,
    pub min_insight: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pragmatic {
    pub conflict_style: &'static str,
    pub cooperativeness: &'static str,
    pub interrupt_tolerance: u64,
    pub small_talk_patience: u64,
    pub secret_keeping: u64,
    pub tells: Vec<Tell>,
    pub coping: Option<String>,
}

impl Default for Attention {
    fn default() -> Self {
        Attention { bandwidth: 4, vigilance: 50, biases: BTreeMap::new() }
    }
}

impl Default for Pragmatic {
    fn default() -> Self {
        Pragmatic {
            conflict_style: "direct",
            cooperativeness: "cooperative",
            interrupt_tolerance: 50,
            small_talk_patience: 50,
            secret_keeping: 50,
            tells: Vec::new(),
            coping: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fabric {
    pub clock: u64,
    pub norms: BTreeMap<String, Norm>,
    /// (norm, offender, witness) → breaches still on the ladder.
    pub counts: BTreeMap<(String, String, String), u64>,
    pub contracts: BTreeMap<String, Contract>,
    pub dyads: BTreeMap<(String, String), Dyad>,
    pub profiles: BTreeMap<String, (Attention, Pragmatic)>,
    pub limits: Limits,
}

impl Default for Fabric {
    fn default() -> Self {
        Fabric {
            clock: 0,
            norms: BTreeMap::new(),
            counts: BTreeMap::new(),
            contracts: BTreeMap::new(),
            dyads: BTreeMap::new(),
            profiles: BTreeMap::new(),
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Fabric {
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
            "norm" => self.op_norm(args, l),
            "breach" => self.op_breach(args, l),
            "apologize" => self.op_apologize(args),
            "contract" => self.op_contract(args, l),
            "settle" => self.op_settle(args),
            "ledger" => self.op_ledger(args),
            "meet" => self.op_meet(args, l),
            "landmark" => self.op_landmark(args),
            "address" => self.op_address(args),
            "thread" => self.op_thread(args, l),
            "resolve" => self.op_resolve(args),
            "dyad" => self.op_dyad(args),
            "profile" => self.op_profile(args, l),
            "notice" => self.op_notice(args),
            "tells" => self.op_tells(args),
            "style" => self.op_style(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        let mut late = Vec::new();
        for (id, k) in self.contracts.iter_mut() {
            if !k.overdue && k.due_at.is_some_and(|d| d < now) {
                k.overdue = true;
                late.push(Value::from(id.as_str()));
            }
        }
        Ok(obj! { "now" => now, "overdue" => Value::Array(late) })
    }

    // ------------------------------------------------------------ norms

    fn op_norm(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "severity"], &["applies_when", "guilt_cost", "report_to"])?;
        let id = id_of(args, "id")?;
        let sev = pick(c::get(args, "severity")?, &SEVERITY.map(|s| s.0))?;
        let severity = SEVERITY.iter().find(|s| s.0 == sev).map_or(0, |s| s.1);
        let mut applies_when = id_list(opt(args, "applies_when"), 8)?;
        applies_when.sort();
        let guilt_cost = opt_uint(args, "guilt_cost", 0, 100, severity / 2)?;
        let report_to = opt_id(args, "report_to")?;
        need(self.norms.contains_key(&id) || self.norms.len() < l.get("norms"))?;
        self.norms.insert(id.clone(), Norm { severity, applies_when, guilt_cost, report_to });
        Ok(obj! { "id" => id, "severity" => severity })
    }

    fn op_breach(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["norm", "offender", "witness"], &["context", "conscience"])?;
        let nid = id_of(args, "norm")?;
        let off = id_of(args, "offender")?;
        let wit = id_of(args, "witness")?;
        let ctx = id_list(opt(args, "context"), 8)?;
        let conscience = opt_uint(args, "conscience", 0, 100, 0)?;
        let n = self.norms.get(&nid).ok_or_else(c::bad)?;
        need(off != wit)?;
        let key = (nid.clone(), off.clone(), wit.clone());
        let prior = self.counts.get(&key).copied().unwrap_or(0);
        let deltas = |d: i64, r: i64, t: i64| obj! { "disagreement" => d, "resentment" => r, "trust" => t };
        let view = |applies: bool, step: u64, sanctions: Vec<&str>, d: Value, guilt: u64, report: Option<String>| {
            obj! {
                "amends" => guilt > 0, "applies" => applies, "deltas" => d, "guilt" => guilt,
                "norm" => nid.as_str(), "offender" => off.as_str(), "prior" => prior, "report_to" => report,
                "sanctions" => sanctions, "step" => step, "witness" => wit.as_str(),
            }
        };
        if !n.applies_when.is_empty() && !ctx.iter().any(|x| n.applies_when.contains(x)) {
            return Ok(view(false, 0, Vec::new(), deltas(0, 0, 0), 0, None));
        }
        need(self.counts.contains_key(&key) || self.counts.len() < l.get("breaches"))?;
        let sev = n.severity;
        let base = if sev >= 80 {
            3
        } else if sev >= 60 {
            2
        } else {
            1
        };
        let mut step = (base + prior).min(3);
        if sev <= 10 {
            step = step.min(2);
        }
        let (sanctions, d) = match step {
            1 => (vec!["verbal_warning"], deltas(10, 0, 0)),
            2 => (vec!["refuse_service", "demand_apology_or_fine"], deltas(0, 0, -20)),
            _ => {
                let mut s = vec!["flee_or_confront"];
                if n.report_to.is_some() {
                    s.push("report_to_role");
                }
                (s, deltas(0, 50, 0))
            }
        };
        let guilt = if conscience >= 50 { n.guilt_cost } else { 0 };
        let report = if step == 3 { n.report_to.clone() } else { None };
        let out = view(true, step, sanctions, d, guilt, report);
        self.counts.insert(key, prior + 1);
        Ok(out)
    }

    fn op_apologize(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["norm", "offender", "witness"], &[])?;
        let key = (id_of(args, "norm")?, id_of(args, "offender")?, id_of(args, "witness")?);
        let n = self.counts.get(&key).copied().unwrap_or(0);
        need(n > 0)?;
        if n == 1 {
            self.counts.remove(&key);
        } else {
            self.counts.insert(key.clone(), n - 1);
        }
        Ok(obj! { "norm" => key.0, "offender" => key.1, "prior" => n - 1, "witness" => key.2 })
    }

    // ------------------------------------------------------------ contracts

    fn op_contract(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "kind", "debtor", "creditor", "terms"], &["witnesses", "due_at", "weight"])?;
        let id = id_of(args, "id")?;
        let kind = pick(c::get(args, "kind")?, &KINDS)?;
        let debtor = id_of(args, "debtor")?;
        let creditor = id_of(args, "creditor")?;
        let mut witnesses = id_list(opt(args, "witnesses"), 8)?;
        witnesses.sort();
        need(debtor != creditor && !witnesses.contains(&debtor) && !witnesses.contains(&creditor))?;
        let due_at = opt(args, "due_at").map(|v| uint_bound(v, self.clock, MAX_AT)).transpose()?;
        let terms = text_of(args, "terms", 160)?;
        let weight = opt_uint(args, "weight", 1, 100, 20)?;
        need(!self.contracts.contains_key(&id) && self.contracts.len() < l.get("contracts"))?;
        self.contracts
            .insert(id.clone(), Contract { kind, debtor, creditor, witnesses, due_at, terms, weight, overdue: false });
        Ok(obj! { "id" => id, "status" => "open" })
    }

    fn op_settle(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id", "outcome"], &[])?;
        let id = id_of(args, "id")?;
        let res = c::get(args, "outcome")?;
        need(self.contracts.contains_key(&id))?;
        let res = pick(res, &OUTCOMES)?;
        let k = self.contracts.remove(&id).ok_or_else(c::bad)?;
        let delta: i64 = match res {
            "fulfilled" => (k.weight / 4) as i64,
            "broken" => -(k.weight as i64),
            _ => 0,
        };
        Ok(obj! {
            "creditor" => k.creditor, "debtor" => k.debtor, "id" => id, "late" => k.overdue, "status" => res,
            "told" => if res == "broken" { strings(&k.witnesses) } else { Value::Array(Vec::new()) },
            "trust_delta" => delta,
        })
    }

    fn op_ledger(&self, args: &Value) -> Result<Value> {
        keys(args, &["a", "b"], &[])?;
        let x = id_of(args, "a")?;
        let y = id_of(args, "b")?;
        need(x != y)?;
        let (mut xy, mut yx) = (0u64, 0u64);
        let (mut open, mut late) = (Vec::new(), Vec::new());
        for (id, k) in &self.contracts {
            let forward = k.debtor == x && k.creditor == y;
            if !forward && !(k.debtor == y && k.creditor == x) {
                continue;
            }
            if forward {
                xy += k.weight;
            } else {
                yx += k.weight;
            }
            if k.overdue {
                late.push(Value::from(id.as_str()));
            } else {
                open.push(Value::from(id.as_str()));
            }
        }
        Ok(obj! {
            "a" => x, "a_owes_b" => xy, "b" => y, "b_owes_a" => yx,
            "help_bias" => obj! { "a_asks_b" => yx.min(50), "b_asks_a" => xy.min(50) },
            "open" => Value::Array(open), "overdue" => Value::Array(late),
        })
    }

    // ------------------------------------------------------------ dyads

    fn op_meet(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["a", "b", "episode", "summary"], &[])?;
        let k = pair(&id_of(args, "a")?, &id_of(args, "b")?)?;
        let episode = id_of(args, "episode")?;
        let summary = text_of(args, "summary", 160)?;
        if let Some(d) = self.dyads.get(&k) {
            return Ok(obj! { "first_encounter" => d.first_view(), "new" => false });
        }
        need(self.dyads.len() < l.get("dyads"))?;
        let d = Dyad {
            first: (self.clock, episode, summary),
            landmarks: Vec::new(),
            address: BTreeMap::new(),
            threads: Vec::new(),
        };
        let out = obj! { "first_encounter" => d.first_view(), "new" => true };
        self.dyads.insert(k, d);
        Ok(out)
    }

    fn dyad_mut(&mut self, a: &str, b: &str) -> Result<&mut Dyad> {
        let k = pair(a, b)?;
        self.dyads.get_mut(&k).ok_or_else(c::bad)
    }

    fn op_landmark(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["a", "b", "kind", "episode"], &["summary"])?;
        let (a, b) = (id_of(args, "a")?, id_of(args, "b")?);
        let now = self.clock;
        let d = self.dyad_mut(&a, &b)?;
        let kind = id_of(args, "kind")?;
        let episode = id_of(args, "episode")?;
        let summary = opt(args, "summary").map(|v| text_bound(v, 160).map(str::to_string)).transpose()?;
        let before = d.landmarks.len();
        d.landmarks.retain(|m| m.kind != kind);
        let mut evicted = None;
        if d.landmarks.len() == before && before >= LANDMARKS {
            evicted = Some(d.landmarks.remove(0).kind);
        }
        d.landmarks.push(Moment { at: now, episode, kind, summary });
        Ok(obj! {
            "evicted" => evicted,
            "landmarks" => Value::Array(d.landmarks.iter().map(|m| Value::from(m.kind.as_str())).collect()),
        })
    }

    fn op_address(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["from", "to", "term"], &[])?;
        let (f, t) = (id_of(args, "from")?, id_of(args, "to")?);
        let d = self.dyad_mut(&f, &t)?;
        let term = text_of(args, "term", 40)?;
        d.address.insert(f.clone(), term.clone());
        Ok(obj! { "from" => f, "term" => term, "to" => t })
    }

    fn op_thread(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["a", "b", "id", "text", "opened_by"], &[])?;
        let (a, b) = (id_of(args, "a")?, id_of(args, "b")?);
        let d = self.dyad_mut(&a, &b)?;
        let id = id_of(args, "id")?;
        let text = text_of(args, "text", 160)?;
        let by = id_of(args, "opened_by")?;
        need((by == a || by == b) && d.threads.iter().all(|t| t.id != id))?;
        need(d.threads.len() < l.get("threads"))?;
        d.threads.push(Thread { id: id.clone(), opened_by: by, text });
        Ok(obj! { "id" => id, "open" => d.threads.len() })
    }

    fn op_resolve(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["a", "b", "id"], &[])?;
        let (a, b) = (id_of(args, "a")?, id_of(args, "b")?);
        let d = self.dyad_mut(&a, &b)?;
        let id = id_of(args, "id")?;
        need(d.threads.iter().any(|t| t.id == id))?;
        d.threads.retain(|t| t.id != id);
        Ok(obj! { "id" => id, "open" => d.threads.len() })
    }

    fn op_dyad(&self, args: &Value) -> Result<Value> {
        keys(args, &["a", "b"], &[])?;
        let (x, y) = (id_of(args, "a")?, id_of(args, "b")?);
        let Some(d) = self.dyads.get(&pair(&x, &y)?) else {
            return Ok(obj! {
                "a" => x, "address_a_to_b" => "stranger", "address_b_to_a" => "stranger", "b" => y,
                "first_encounter" => Value::Null, "known_for" => 0, "landmarks" => Value::Array(Vec::new()),
                "met" => false, "threads" => Value::Array(Vec::new()),
            });
        };
        let term = |w: &str| d.address.get(w).map_or("acquaintance", String::as_str).to_string();
        Ok(obj! {
            "a" => x.as_str(), "address_a_to_b" => term(&x), "address_b_to_a" => term(&y), "b" => y.as_str(),
            "first_encounter" => d.first_view(), "known_for" => self.clock - d.first.0,
            "landmarks" => Value::Array(d.landmarks.iter().map(Moment::view).collect()), "met" => true,
            "threads" => Value::Array(d.threads.iter().map(|t| obj! {
                "id" => t.id.as_str(), "opened_by" => t.opened_by.as_str(), "text" => t.text.as_str(),
            }).collect()),
        })
    }

    // ------------------------------------------------------------ profiles

    fn op_profile(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being"], &["attention", "pragmatic"])?;
        let b = id_of(args, "being")?;
        let att = match opt(args, "attention") {
            None => Attention::default(),
            Some(x) => {
                keys(x, &[], &["bandwidth", "vigilance", "biases"])?;
                let mut biases = BTreeMap::new();
                if let Some(m) = opt(x, "biases") {
                    let m = m.as_object().ok_or_else(c::bad)?;
                    need(m.len() <= 16)?;
                    for (k, v) in m {
                        biases.insert(ident(&Value::from(k.as_str()))?.to_string(), uint_bound(v, 0, 300)?);
                    }
                }
                Attention {
                    bandwidth: opt_uint(x, "bandwidth", 1, 16, 4)?,
                    vigilance: opt_uint(x, "vigilance", 0, 100, 50)?,
                    biases,
                }
            }
        };
        let prag = match opt(args, "pragmatic") {
            None => Pragmatic::default(),
            Some(x) => {
                keys(
                    x,
                    &[],
                    &[
                        "conflict_style",
                        "cooperativeness",
                        "interrupt_tolerance",
                        "small_talk_patience",
                        "secret_keeping",
                        "tells",
                        "coping",
                    ],
                )?;
                let mut tells = Vec::new();
                for t in list_bound(opt(x, "tells"), 8)? {
                    keys(t, &["kind", "cue"], &["min_insight"])?;
                    let kind = pick(c::get(t, "kind")?, &TELL_KINDS)?;
                    tells.push(Tell {
                        kind,
                        cue: id_of(t, "cue")?,
                        min_insight: opt_uint(t, "min_insight", 0, 100, 50)?,
                    });
                }
                Pragmatic {
                    conflict_style: one_of(x, "conflict_style", &STYLES, "direct")?,
                    cooperativeness: one_of(x, "cooperativeness", &COOP, "cooperative")?,
                    interrupt_tolerance: opt_uint(x, "interrupt_tolerance", 0, 100, 50)?,
                    small_talk_patience: opt_uint(x, "small_talk_patience", 0, 100, 50)?,
                    secret_keeping: opt_uint(x, "secret_keeping", 0, 100, 50)?,
                    tells,
                    coping: opt_id(x, "coping")?,
                }
            }
        };
        need(self.profiles.contains_key(&b) || self.profiles.len() < l.get("beings"))?;
        let n = prag.tells.len();
        self.profiles.insert(b.clone(), (att, prag));
        Ok(obj! { "being" => b, "tells" => n })
    }

    fn op_notice(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "stimuli"], &[])?;
        let b = id_of(args, "being")?;
        let fallback = Attention::default();
        let att = self.profiles.get(&b).map_or(&fallback, |p| &p.0);
        let rows = c::get(args, "stimuli")?.as_array().ok_or_else(c::bad)?;
        need(rows.len() <= STIMULI)?;
        let mut seen = BTreeSet::new();
        let mut scored = Vec::new();
        for x in rows {
            keys(x, &["id", "category", "intensity"], &[])?;
            let id = id_of(x, "id")?;
            let cat = id_of(x, "category")?;
            need(seen.insert(id.clone()))?;
            let intensity = uint_bound(c::get(x, "intensity")?, 0, 100)?;
            scored.push((intensity * att.biases.get(&cat).copied().unwrap_or(100) / 100, id, cat));
        }
        let floor = 100 - att.vigilance;
        let mut ok: Vec<&(u64, String, String)> = scored.iter().filter(|s| s.0 >= floor).collect();
        ok.sort_by(|p, q| q.0.cmp(&p.0).then_with(|| p.1.cmp(&q.1)));
        ok.truncate(att.bandwidth as usize);
        let picked: BTreeSet<&str> = ok.iter().map(|s| s.1.as_str()).collect();
        let missed: BTreeSet<&str> = scored.iter().map(|s| s.1.as_str()).filter(|id| !picked.contains(id)).collect();
        Ok(obj! {
            "being" => b.as_str(),
            "missed" => Value::Array(missed.into_iter().map(Value::from).collect()),
            "noticed" => Value::Array(ok.iter().map(|s| obj! {
                "category" => s.2.as_str(), "id" => s.1.as_str(), "score" => s.0,
            }).collect()),
        })
    }

    fn op_tells(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "observer_insight"], &["lying"])?;
        let b = id_of(args, "being")?;
        let insight = uint_bound(c::get(args, "observer_insight")?, 0, 100)?;
        let lying = opt_bool(args, "lying", true)?;
        let fallback = Pragmatic::default();
        let p = self.profiles.get(&b).map_or(&fallback, |p| &p.1);
        let bar = p.secret_keeping / 5;
        let seen: Vec<Value> = p
            .tells
            .iter()
            .filter(|t| lying && insight >= t.min_insight + bar)
            .map(|t| obj! { "cue" => t.cue.as_str(), "kind" => t.kind })
            .collect();
        let hidden = if lying { p.tells.len() - seen.len() } else { 0 };
        let suspicion = (15 * seen.len()).min(100);
        Ok(obj! { "being" => b, "hidden" => hidden, "perceived" => Value::Array(seen), "suspicion" => suspicion })
    }

    fn op_style(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        let fallback = Pragmatic::default();
        let p = self.profiles.get(&b).map_or(&fallback, |p| &p.1);
        let (tol, pat) = (p.interrupt_tolerance, p.small_talk_patience);
        Ok(obj! {
            "being" => b, "conflict_style" => p.conflict_style, "cooperativeness" => p.cooperativeness,
            "coping" => p.coping.clone(),
            "on_interrupt" => if tol < 30 { "snaps" } else if tol < 70 { "notes_it" } else { "lets_it_go" },
            "on_small_talk" => if pat < 30 { "cuts_to_point" } else if pat < 70 { "humors_briefly" } else { "enjoys" },
            "secret_keeping" => p.secret_keeping,
        })
    }
}

persist!(Norm { applies_when, guilt_cost [0, 100], report_to, severity [0, 100] });
persist!(Contract {
    creditor, debtor, due_at [0, MAX_AT], kind in KINDS, overdue, terms, weight [1, 100], witnesses,
});
persist!(Moment { at [0, MAX_AT], episode, kind, summary });
persist!(Thread { id, opened_by, text });
persist!(Dyad { address, first, landmarks, threads });
persist!(Attention { bandwidth [1, 16], biases, vigilance [0, 100] });
persist!(Tell { cue, kind in TELL_KINDS, min_insight [0, 100] });
persist!(Pragmatic {
    conflict_style in STYLES, cooperativeness in COOP, coping, interrupt_tolerance [0, 100], secret_keeping [0, 100],
    small_talk_patience [0, 100], tells,
});
persist!(rest Fabric { clock [0, MAX_AT], contracts, counts, dyads, norms, profiles });
durable!(Fabric, "fabric", LIMITS);

fn sorted_ids(xs: &[String], max: usize) -> bool {
    xs.len() <= max && ascending(xs) && xs.iter().all(|x| is_ident(x))
}

impl Fabric {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("norms", self.norms.len())?;
        within::<Self>("breaches", self.counts.len())?;
        within::<Self>("contracts", self.contracts.len())?;
        within::<Self>("dyads", self.dyads.len())?;
        within::<Self>("beings", self.profiles.len())?;
        for (id, n) in &self.norms {
            ok(is_ident(id) && SEVERITY.iter().any(|s| s.1 == n.severity) && sorted_ids(&n.applies_when, 8))?;
            ok(n.report_to.as_deref().is_none_or(is_ident))?;
        }
        for ((n, off, wit), k) in &self.counts {
            ok(self.norms.contains_key(n) && is_ident(off) && is_ident(wit) && off != wit && (1..=MAX_AT).contains(k))?;
        }
        for (id, k) in &self.contracts {
            ok(is_ident(id) && is_ident(&k.debtor) && is_ident(&k.creditor) && k.debtor != k.creditor)?;
            ok(sorted_ids(&k.witnesses, 8) && !k.witnesses.contains(&k.debtor) && !k.witnesses.contains(&k.creditor))?;
            ok(is_text(&k.terms, 160) && k.overdue == k.due_at.is_some_and(|d| d < self.clock))?;
        }
        for ((a, b), d) in &self.dyads {
            ok(is_ident(a) && is_ident(b) && a < b)?;
            let (at, episode, summary) = &d.first;
            ok(*at <= self.clock && is_ident(episode) && is_text(summary, 160) && d.landmarks.len() <= LANDMARKS)?;
            let mut kinds = BTreeSet::new();
            let mut last = *at;
            for m in &d.landmarks {
                ok(is_ident(&m.kind) && kinds.insert(&m.kind) && is_ident(&m.episode) && m.at >= last)?;
                ok(m.at <= self.clock && m.summary.as_deref().is_none_or(|s| is_text(s, 160)))?;
                last = m.at;
            }
            ok(d.address.iter().all(|(w, t)| (w == a || w == b) && is_text(t, 40)))?;
            within::<Self>("threads", d.threads.len())?;
            let mut ids = BTreeSet::new();
            for t in &d.threads {
                ok(is_ident(&t.id) && ids.insert(&t.id) && (t.opened_by == *a || t.opened_by == *b))?;
                ok(is_text(&t.text, 160))?;
            }
        }
        for (b, (att, prag)) in &self.profiles {
            ok(is_ident(b) && att.biases.len() <= 16 && att.biases.iter().all(|(k, v)| is_ident(k) && *v <= 300))?;
            ok(prag.tells.len() <= 8 && prag.tells.iter().all(|t| is_ident(&t.cue)))?;
            ok(prag.coping.as_deref().is_none_or(is_ident))?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

/// Runs one request on a fresh [`Fabric`] (local kind `fabric`).
pub fn workflow(request: &Value) -> Result<Value> {
    Fabric::new().workflow(request)
}
