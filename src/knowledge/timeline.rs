//! The world timeline: the host's authoritative, append-only history of what happened.
//!
//! Only the host records entries; models never write history. Each entry says when it
//! happened (`at`, any host clock), what kind of event it was, a one-line summary, who
//! acted, where, which things were involved, who saw it and phrases people may use to ask
//! about it. Entries may be recorded out of time order (for authored history before play
//! starts); queries sort by `(at, seq)`.
//!
//! Who knows what: `public` entries are known to everyone, `witnessed` ones to their
//! actors and witnesses, `private` ones to their actors only. A mistaken entry is
//! retracted (kept, hidden from queries by default). The timeline holds at most
//! [`MAX_ENTRIES`]; recording into a full timeline fails rather than forgetting anything,
//! and the host decides when to [`Timeline::compact`] old entries into one summary.
//!
//! ```
//! use gamectx::knowledge::timeline::Timeline;
//! use gamectx::data::json::parse;
//! let mut history = Timeline::new();
//! history.record(&parse(r#"{"at":10,"kind":"fire","summary":"A fire broke out in the forge.",
//!   "actors":["smith"],"place":"forge","phrases":["the fire"]}"#, 4096)?)?;
//! history.record(&parse(r#"{"at":20,"kind":"pact","summary":"The miller sold his mill.",
//!   "actors":["miller"],"visibility":"private"}"#, 4096)?)?;
//! let known = history.query(&parse(r#"{"known_to":"baker","about":"What about the fire?"}"#, 4096)?)?;
//! assert_eq!(known.get("total").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```
use crate::knowledge::codex::keys;
use crate::knowledge::props::{ident, line, normalize};
use crate::packet::request::bounded;
use crate::{checked as c, obj, Result, Value};

/// Most entries one timeline holds by default (the `entries` [limit](LIMITS)).
pub const MAX_ENTRIES: usize = 4096;
/// Host-set limits on a timeline (defaults are the earlier fixed bounds): most `entries`
/// held, and the most serialized `bytes` a workflow request (with its timeline) may carry.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "entries", default: MAX_ENTRIES, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "bytes", default: 16_777_216, ceiling: 268_435_456 },
];
crate::character::limits::module_limits!(TimelineLimits, LIMITS);
const MAX_SAFE: u64 = 9_007_199_254_740_991;
const VISIBILITY: [&str; 3] = ["public", "witnessed", "private"];
const ENTRY: [&str; 14] = [
    "seq",
    "at",
    "kind",
    "summary",
    "actors",
    "place",
    "things",
    "tags",
    "visibility",
    "witnesses",
    "phrases",
    "facts",
    "retracted",
    "covers",
];
const RECORD: [&str; 11] =
    ["at", "kind", "summary", "actors", "place", "things", "tags", "visibility", "witnesses", "phrases", "facts"];
const FILTER: [&str; 12] = [
    "known_to",
    "actor",
    "place",
    "thing",
    "kind",
    "tag",
    "since",
    "until",
    "about",
    "limit",
    "order",
    "include_retracted",
];

fn ids(v: Option<&Value>, hi: usize) -> Result<Value> {
    let xs = match v {
        None => &[][..],
        Some(v) => c::list(v)?,
    };
    if xs.len() > hi {
        return Err(c::bad());
    }
    for (i, x) in xs.iter().enumerate() {
        ident(x)?;
        if xs[..i].contains(x) {
            return Err(c::bad());
        }
    }
    Ok(Value::Array(xs.to_vec()))
}

fn phrases(v: Option<&Value>) -> Result<Vec<String>> {
    let xs = match v {
        None => &[][..],
        Some(v) => c::list(v)?,
    };
    if xs.len() > 8 {
        return Err(c::bad());
    }
    let mut out: Vec<String> = Vec::new();
    for p in xs {
        let q = normalize(line(p, 128, 128)?);
        if q.is_empty() || out.contains(&q) {
            return Err(c::bad());
        }
        out.push(q);
    }
    Ok(out)
}

/// A whole number in −10⁹..=10⁹, canonicalized.
fn small_int(v: &Value) -> Option<Value> {
    let Value::Number(s) = v else { return None };
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || (digits.len() > 1 && digits.starts_with('0'))
    {
        return None;
    }
    let n = s.parse::<i64>().ok().filter(|n| (-1_000_000_000..=1_000_000_000).contains(n))?;
    Some(Value::int(n))
}

fn facts(v: Option<&Value>) -> Result<Value> {
    let rows = match v {
        None => &[][..],
        Some(v) => v.as_object().ok_or_else(c::bad)?,
    };
    if rows.len() > 16 {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for (k, x) in rows {
        ident(&Value::str(k.as_str()))?;
        let x = match x {
            Value::Bool(_) => x.clone(),
            _ => match small_int(x) {
                Some(n) => n,
                None => Value::str(line(x, 64, usize::MAX)?),
            },
        };
        out.push((k.clone(), x));
    }
    Ok(Value::Object(out))
}

fn safe(v: &Value, lo: u64) -> Result<u64> {
    c::uint(v, lo, MAX_SAFE)
}

/// A canonical entry from a record request.
fn recorded(seq: u64, e: &Value) -> Result<Value> {
    keys(e, &RECORD[..3], &RECORD[3..])?;
    let vis = match e.get("visibility") {
        None => "public",
        Some(v) => c::text(v)?,
    };
    if !VISIBILITY.contains(&vis) {
        return Err(c::bad());
    }
    let place = match e.get("place") {
        None | Some(Value::Null) => Value::Null,
        Some(p) => Value::str(ident(p)?),
    };
    Ok(obj! {
        "seq"=>seq,
        "at"=>safe(c::get(e, "at")?, 0)?,
        "kind"=>ident(c::get(e, "kind")?)?,
        "summary"=>line(c::get(e, "summary")?, 240, usize::MAX)?,
        "actors"=>ids(e.get("actors"), 16)?,
        "place"=>place,
        "things"=>ids(e.get("things"), 16)?,
        "tags"=>ids(e.get("tags"), 8)?,
        "visibility"=>vis,
        "witnesses"=>ids(e.get("witnesses"), 64)?,
        "phrases"=>Value::strings(&phrases(e.get("phrases"))?),
        "facts"=>facts(e.get("facts"))?,
        "retracted"=>Value::Null,
        "covers"=>Value::Null,
    })
}

fn set(e: &mut Value, key: &str, value: Value) {
    if let Value::Object(rows) = e {
        if let Some(slot) = rows.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value;
        }
    }
}

/// Validate one stored entry and return it canonical.
fn stored(e: &Value) -> Result<Value> {
    let rows = e.as_object().ok_or_else(c::bad)?;
    if rows.len() != ENTRY.len() || rows.iter().zip(ENTRY).any(|((k, _), want)| k != want) {
        return Err(c::bad());
    }
    let seq = safe(c::get(e, "seq")?, 1)?;
    let request = Value::Object(rows.iter().filter(|(k, _)| RECORD.contains(&k.as_str())).cloned().collect());
    let mut out = recorded(seq, &request)?;
    if out.get("phrases") != e.get("phrases") {
        return Err(c::bad());
    }
    let retracted = c::get(e, "retracted")?;
    if !matches!(retracted, Value::Null) {
        set(&mut out, "retracted", Value::str(line(retracted, 120, usize::MAX)?));
    }
    let covers = c::get(e, "covers")?;
    if !matches!(covers, Value::Null) {
        let rows = covers.as_object().ok_or_else(c::bad)?;
        if rows.len() != 3 || rows.iter().zip(["from", "through", "count"]).any(|((k, _), want)| k != want) {
            return Err(c::bad());
        }
        let from = safe(c::get(covers, "from")?, 1)?;
        let through = safe(c::get(covers, "through")?, 1)?;
        let count = safe(c::get(covers, "count")?, 2)?;
        if from >= through || through != seq {
            return Err(c::bad());
        }
        set(&mut out, "covers", obj! {"from"=>from,"through"=>through,"count"=>count});
    }
    Ok(out)
}

fn num(e: &Value, k: &str) -> u64 {
    e.get(k).and_then(Value::as_usize).unwrap_or(0) as u64
}
fn strs<'a>(e: &'a Value, k: &str) -> impl Iterator<Item = &'a str> {
    e.get(k).and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_str)
}
fn words_in(needle: &str, hay: &str) -> bool {
    format!(" {hay} ").contains(&format!(" {needle} "))
}

/// The host's authoritative history: `{version:1, next, entries}`.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    next: u64,
    entries: Vec<Value>,
    max: usize,
}
impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
impl Timeline {
    /// An empty timeline.
    pub fn new() -> Self {
        Self::within(&TimelineLimits::default())
    }
    /// An empty timeline holding at most the `entries` [limit](LIMITS).
    pub fn within(l: &crate::character::limits::Limits) -> Self {
        Self { next: 1, entries: Vec::new(), max: l.get("entries") }
    }
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(input, 16_777_216)?)
    }
    /// Load and fully revalidate a saved timeline.
    pub fn from_value(v: &Value) -> Result<Self> {
        Self::from_value_within(v, &TimelineLimits::default())
    }
    /// [`from_value`](Self::from_value) with host-set [limits](LIMITS).
    pub fn from_value_within(v: &Value, l: &crate::character::limits::Limits) -> Result<Self> {
        let max = l.get("entries");
        bounded(v, l.get("bytes"))?;
        keys(v, &["version", "next", "entries"], &[])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let next = safe(c::get(v, "next")?, 1)?;
        let rows = c::list(c::get(v, "entries")?)?;
        if rows.len() > max {
            return Err(c::bad());
        }
        let entries = rows.iter().map(stored).collect::<Result<Vec<_>>>()?;
        let mut last = 0;
        for e in &entries {
            let seq = num(e, "seq");
            if seq <= last || seq >= next {
                return Err(c::bad());
            }
            last = seq;
        }
        Ok(Self { next, entries, max })
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1u64,"next"=>self.next,"entries"=>Value::Array(self.entries.clone())}
    }
    /// Entries in `seq` order (including retracted ones).
    pub fn entries(&self) -> &[Value] {
        &self.entries
    }
    /// The entry with this `seq`.
    pub fn entry(&self, seq: u64) -> Option<&Value> {
        self.position(seq).map(|i| &self.entries[i])
    }
    /// Entries are kept in strictly increasing `seq` order, so lookups bisect.
    fn position(&self, seq: u64) -> Option<usize> {
        self.entries.binary_search_by_key(&seq, |e| num(e, "seq")).ok()
    }
    /// Record one entry (`{at, kind, summary, actors?, place?, things?, tags?, visibility?,
    /// witnesses?, phrases?, facts?}`) and return its `seq`.
    pub fn record(&mut self, entry: &Value) -> Result<u64> {
        if self.entries.len() >= self.max || self.next >= MAX_SAFE {
            return Err(c::bad());
        }
        let out = recorded(self.next, entry)?;
        self.entries.push(out);
        self.next += 1;
        Ok(self.next - 1)
    }
    /// Mark an entry as a mistake. It stays in the log and is hidden from queries unless
    /// they ask for retracted entries. Retracting twice fails.
    pub fn retract(&mut self, seq: u64, reason: &str) -> Result<()> {
        let reason = line(&Value::str(reason), 120, usize::MAX)?.to_owned();
        let i = self.position(seq).ok_or_else(c::bad)?;
        let e = &mut self.entries[i];
        if !matches!(e.get("retracted"), Some(Value::Null)) {
            return Err(c::bad());
        }
        set(e, "retracted", Value::str(reason));
        Ok(())
    }
    /// Replace every entry up to and including `through` (at least two) with one public
    /// summary entry that keeps `seq = through` and the latest `at`. Returns how many
    /// original entries the summary now covers.
    pub fn compact(&mut self, through: u64, summary: &str, kind: &str) -> Result<u64> {
        let summary = line(&Value::str(summary), 240, usize::MAX)?.to_owned();
        let kind = ident(&Value::str(kind))?.to_owned();
        let n = self.entries.iter().take_while(|e| num(e, "seq") <= through).count();
        if n < 2 || num(&self.entries[n - 1], "seq") != through {
            return Err(c::bad());
        }
        let covered = &self.entries[..n];
        let span = |e: &Value, k: &str| e.get("covers").and_then(|c| c.get(k)).and_then(Value::as_usize);
        let count = covered.iter().map(|e| span(e, "count").unwrap_or(1) as u64).sum::<u64>();
        let first = span(&covered[0], "from").map_or_else(|| num(&covered[0], "seq"), |f| f as u64);
        let at = covered.iter().map(|e| num(e, "at")).max().unwrap_or(0);
        let out = obj! {
            "seq"=>through,"at"=>at,"kind"=>kind,"summary"=>summary,"actors"=>Value::Array(vec![]),
            "place"=>Value::Null,"things"=>Value::Array(vec![]),"tags"=>Value::Array(vec![]),"visibility"=>"public",
            "witnesses"=>Value::Array(vec![]),"phrases"=>Value::Array(vec![]),"facts"=>Value::Object(vec![]),
            "retracted"=>Value::Null,"covers"=>obj! {"from"=>first,"through"=>through,"count"=>count},
        };
        self.entries.splice(..n, [out]);
        Ok(count)
    }
    /// Entries matching a filter `{known_to?, actor?, place?, thing?, kind?, tag?, since?,
    /// until?, about?, limit? 1–64 = 16, order? newest|oldest = newest,
    /// include_retracted? = false}` as `{total, entries}`.
    pub fn query(&self, f: &Value) -> Result<Value> {
        keys(f, &[], &FILTER)?;
        let id = |k: &str| f.get(k).map(ident).transpose();
        let (known, actor, place, thing, kind, tag) =
            (id("known_to")?, id("actor")?, id("place")?, id("thing")?, id("kind")?, id("tag")?);
        let since = f.get("since").map(|v| safe(v, 0)).transpose()?.unwrap_or(0);
        let until = f.get("until").map(|v| safe(v, 0)).transpose()?.unwrap_or(MAX_SAFE);
        if since > until {
            return Err(c::bad());
        }
        let about = f.get("about").map(|v| line(v, 128, usize::MAX).map(normalize)).transpose()?;
        if about.as_deref() == Some("") {
            return Err(c::bad());
        }
        let limit = f.get("limit").map(|v| c::uint(v, 1, 64)).transpose()?.unwrap_or(16) as usize;
        let newest = match f.get("order").map(c::text).transpose()? {
            None | Some("newest") => true,
            Some("oldest") => false,
            _ => return Err(c::bad()),
        };
        let retracted = f.get("include_retracted").map(c::boolean).transpose()?.unwrap_or(false);
        let ok = |e: &&Value| {
            if !retracted && !matches!(e.get("retracted"), Some(Value::Null)) {
                return false;
            }
            let at = num(e, "at");
            if at < since || at > until {
                return false;
            }
            let vis = e.get("visibility").and_then(Value::as_str).unwrap_or("");
            if let Some(w) = known {
                let public = vis == "public";
                let acted = strs(e, "actors").any(|a| a == w);
                let saw = vis == "witnessed" && strs(e, "witnesses").any(|a| a == w);
                if !(public || acted || saw) {
                    return false;
                }
            }
            actor.is_none_or(|a| strs(e, "actors").any(|x| x == a))
                && place.is_none_or(|p| e.get("place").and_then(Value::as_str) == Some(p))
                && thing.is_none_or(|t| strs(e, "things").any(|x| x == t))
                && kind.is_none_or(|k| e.get("kind").and_then(Value::as_str) == Some(k))
                && tag.is_none_or(|t| strs(e, "tags").any(|x| x == t))
                && about.as_deref().is_none_or(|q| strs(e, "phrases").any(|p| words_in(p, q) || words_in(q, p)))
        };
        let mut hits = self.entries.iter().filter(ok).collect::<Vec<_>>();
        hits.sort_by_key(|e| (num(e, "at"), num(e, "seq")));
        if newest {
            hits.reverse();
        }
        let total = hits.len();
        Ok(obj! {"total"=>total,"entries"=>Value::Array(hits.into_iter().take(limit).cloned().collect())})
    }
}

/// The `timeline` local kind: `{timeline: Timeline|null, ops: [{op, args}] ≤64, limits?}` →
/// `{limits?, results, timeline}`. Ops are `record`, `retract {seq, reason}`,
/// `compact {through, summary, kind?}` and `query`. A failed op changes nothing. `limits`
/// are the timeline's [host-set limits](LIMITS), echoed when supplied.
pub fn workflow(a: &Value) -> Result<Value> {
    let mut limits = TimelineLimits::default();
    let stripped = crate::character::limits::take(a, &mut limits)?;
    let out = run_workflow(stripped.as_ref().unwrap_or(a), &limits)?;
    Ok(crate::character::limits::echo(out, &limits, stripped.is_some()))
}

fn run_workflow(a: &Value, l: &crate::character::limits::Limits) -> Result<Value> {
    bounded(a, l.get("bytes"))?;
    keys(a, &["timeline", "ops"], &[])?;
    let mut t = match c::get(a, "timeline")? {
        Value::Null => Timeline::within(l),
        v => Timeline::from_value_within(v, l)?,
    };
    let ops = c::list(c::get(a, "ops")?)?;
    if ops.len() > 64 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    // Every op validates before it changes anything, so a failed op leaves `t` as it was.
    for op in ops {
        let out = (|| {
            keys(op, &["op", "args"], &[])?;
            let v = c::get(op, "args")?;
            match c::text(c::get(op, "op")?)? {
                "record" => Ok(obj! {"seq"=>t.record(v)?}),
                "retract" => {
                    keys(v, &["seq", "reason"], &[])?;
                    let seq = safe(c::get(v, "seq")?, 1)?;
                    t.retract(seq, c::text(c::get(v, "reason")?)?)?;
                    Ok(obj! {"seq"=>seq})
                }
                "compact" => {
                    keys(v, &["through", "summary"], &["kind"])?;
                    let through = safe(c::get(v, "through")?, 1)?;
                    let kind = v.get("kind").map(c::text).transpose()?.unwrap_or("summary");
                    let count = t.compact(through, c::text(c::get(v, "summary")?)?, kind)?;
                    Ok(obj! {"seq"=>through,"count"=>count})
                }
                "query" => t.query(v),
                _ => Err(c::bad()),
            }
        })();
        results.push(match out {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>Value::Array(results),"timeline"=>t.to_value()})
}
