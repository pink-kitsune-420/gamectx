//! Save growth: retention rules and data migration (pass 2, D3).
//!
//! **Retention.** Memories, lessons, expectations, ledgers and other-models all grow. The host
//! registers what its saves keep as entries in named collections (`{id, at, used, importance
//! 0..100, bytes, pinned}`) and gives each collection a rule: `max_entries`, `max_bytes`,
//! `max_age` (ticks since last use), `min_importance` and a `grace` period that protects recently
//! used entries. `prune` removes entries in a fixed order (stale, then unimportant, then the
//! least important and least recently used until the collection fits) and reports why each one
//! went; a `dry_run` reports without removing. Pinned and in-grace entries are never removed; a
//! collection that still does not fit is reported as `over`.
//!
//! **Migration.** The host declares numbered migrations (2, 3, …), each a list of changes to
//! section data (`rename`, `add`, `remove`, `clamp` at a dotted path where `*` means every
//! element or value), and `migrate` upgrades a document from any older version in order. The
//! nodes each path reaches are counted and capped at [`MAX_WORK`].
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::retention::Retention;
//!
//! let mut keep = Retention::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "rule", "args": {"collection": "memories", "max_entries": 2, "max_age": 100}},
//!   {"op": "add", "args": {"collection": "memories", "id": "fire", "bytes": 300, "importance": 90}},
//!   {"op": "add", "args": {"collection": "memories", "id": "rain", "bytes": 120, "importance": 10}},
//!   {"op": "add", "args": {"collection": "memories", "id": "fair", "bytes": 200, "importance": 40, "now": 30}},
//!   {"op": "prune", "args": {"now": 50}},
//!   {"op": "migration", "args": {"version": 2, "changes": [{"op": "rename", "path": "*.mood", "to": "feeling"}]}},
//!   {"op": "migrate", "args": {"from": 1, "document": {"baker": {"mood": 3}}}}
//! ]}"#, 4096)?;
//! let out = keep.workflow(&req)?.to_json();
//! assert!(out.contains(r#""removed":[{"id":"rain","reason":"over_entries"}]"#));
//! assert!(out.contains(r#""document":{"baker":{"feeling":3}}"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_bool, opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "collections", default: 64, ceiling: 4096 },
    Limit { name: "entries", default: 4096, ceiling: 1_000_000 },
    Limit { name: "migrations", default: 16, ceiling: 256 },
    Limit { name: "changes", default: 32, ceiling: 256 },
];
/// Largest byte count or clamp bound (2⁵³ − 1).
pub const MAX_BYTES: u64 = 9_007_199_254_740_991;
/// Largest entry size.
pub const MAX_ENTRY_BYTES: u64 = 16_777_216;
/// Most path nodes one `migrate` may reach.
pub const MAX_WORK: u64 = 1_000_000;
/// Most segments in a migration path.
pub const MAX_SEGMENTS: usize = 8;

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

fn opt_some(args: &Value, k: &str, lo: u64, hi: u64) -> Result<Option<u64>> {
    opt(args, k).map(|v| uint_bound(v, lo, hi)).transpose()
}

fn int_of(v: &Value) -> Option<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| n.unsigned_abs() <= MAX_BYTES),
        _ => None,
    }
}

/// One kept thing in a collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub at: u64,
    pub used: u64,
    pub importance: u64,
    pub bytes: u64,
    pub pinned: bool,
}

/// What a collection keeps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rule {
    pub max_entries: Option<u64>,
    pub max_bytes: Option<u64>,
    pub max_age: Option<u64>,
    pub min_importance: u64,
    pub grace: u64,
}

/// A named collection, its rule and its entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Collection {
    pub rule: Rule,
    pub entries: BTreeMap<String, Entry>,
    pub bytes: u64,
}

/// One declared change to section data.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Rename { path: Vec<String>, to: String },
    Add { path: Vec<String>, value: Value },
    Remove { path: Vec<String> },
    Clamp { path: Vec<String>, min: i64, max: i64 },
}

impl Change {
    fn path(&self) -> &[String] {
        match self {
            Change::Rename { path, .. }
            | Change::Add { path, .. }
            | Change::Remove { path }
            | Change::Clamp { path, .. } => path,
        }
    }
}

/// Collections, migrations and the clock.
#[derive(Clone, Debug, PartialEq)]
pub struct Retention {
    pub collections: BTreeMap<String, Collection>,
    pub clock: u64,
    /// Index `i` holds migration version `i + 2`.
    pub migrations: Vec<Vec<Change>>,
    pub limits: Limits,
}

impl Default for Retention {
    fn default() -> Self {
        Retention { collections: BTreeMap::new(), clock: 0, migrations: Vec::new(), limits: Limits::defaults(LIMITS) }
    }
}

fn parse_path(v: &Value) -> Result<Vec<String>> {
    let s = c::text(v)?;
    let segs: Vec<&str> = s.split('.').collect();
    need(segs.len() <= MAX_SEGMENTS)?;
    let last = segs.len() - 1;
    for (i, seg) in segs.iter().enumerate() {
        if i == last || *seg != "*" {
            ident(&Value::from(*seg))?;
        }
    }
    Ok(segs.into_iter().map(str::to_string).collect())
}

impl Retention {
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
            "rule" => self.op_rule(args, l),
            "add" => self.op_add(args, l),
            "touch" => self.op_touch(args),
            "prune" => self.op_prune(args),
            "assess" => self.op_assess(args),
            "migration" => self.op_migration(args, l),
            "migrate" => self.op_migrate(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    // ------------------------------------------------------------ retention

    fn rule_view(&self, name: &str) -> Value {
        let r = &self.collections[name].rule;
        obj! {
            "collection" => name, "grace" => r.grace, "max_age" => Value::from(r.max_age),
            "max_bytes" => Value::from(r.max_bytes), "max_entries" => Value::from(r.max_entries),
            "min_importance" => r.min_importance,
        }
    }

    fn entry_view(&self, name: &str, id: &str) -> Value {
        let e = &self.collections[name].entries[id];
        obj! {
            "at" => e.at, "bytes" => e.bytes, "collection" => name, "id" => id, "importance" => e.importance,
            "pinned" => e.pinned, "used" => e.used,
        }
    }

    fn op_rule(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["collection"], &["max_entries", "max_bytes", "max_age", "min_importance", "grace"])?;
        let name = id_of(args, "collection")?;
        let rule = Rule {
            max_entries: opt_some(args, "max_entries", 1, 1_000_000)?,
            max_bytes: opt_some(args, "max_bytes", 1, MAX_BYTES)?,
            max_age: opt_some(args, "max_age", 0, MAX_AT)?,
            min_importance: opt_uint(args, "min_importance", 0, 100, 0)?,
            grace: opt_uint(args, "grace", 0, MAX_AT, 0)?,
        };
        need(self.collections.contains_key(&name) || self.collections.len() < l.get("collections"))?;
        self.collections.entry(name.clone()).or_default().rule = rule;
        Ok(self.rule_view(&name))
    }

    fn op_add(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["collection", "id", "bytes"], &["importance", "pinned", "now"])?;
        let name = id_of(args, "collection")?;
        let id = id_of(args, "id")?;
        let bytes = uint_bound(c::get(args, "bytes")?, 1, MAX_ENTRY_BYTES)?;
        let importance = opt_uint(args, "importance", 0, 100, 50)?;
        let pinned = opt_bool(args, "pinned", false)?;
        let now = opt_uint(args, "now", 0, MAX_AT, self.clock)?;
        need(now >= self.clock)?;
        let col = self.collections.get_mut(&name).ok_or_else(c::bad)?;
        need(!col.entries.contains_key(&id) && col.entries.len() < l.get("entries"))?;
        self.clock = now;
        col.entries.insert(id.clone(), Entry { at: now, used: now, importance, bytes, pinned });
        col.bytes += bytes;
        Ok(self.entry_view(&name, &id))
    }

    fn op_touch(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["collection", "id"], &["importance", "pinned", "now"])?;
        let name = id_of(args, "collection")?;
        let id = id_of(args, "id")?;
        let importance = opt_some(args, "importance", 0, 100)?;
        let pinned = opt(args, "pinned").map(|v| v.as_bool().ok_or_else(c::bad)).transpose()?;
        let now = opt_uint(args, "now", 0, MAX_AT, self.clock)?;
        need(now >= self.clock)?;
        let e = self.collections.get_mut(&name).and_then(|col| col.entries.get_mut(&id)).ok_or_else(c::bad)?;
        self.clock = now;
        e.used = now;
        if let Some(i) = importance {
            e.importance = i;
        }
        if let Some(p) = pinned {
            e.pinned = p;
        }
        Ok(self.entry_view(&name, &id))
    }

    /// Which entries a prune at `now` removes, the bytes left and the report.
    fn plan(&self, name: &str, now: u64) -> (Vec<String>, u64, Value) {
        let col = &self.collections[name];
        let r = &col.rule;
        let mut cands: Vec<(&String, &Entry)> =
            col.entries.iter().filter(|(_, e)| !e.pinned && now - e.used >= r.grace).collect();
        cands.sort_by_key(|(id, e)| (e.importance, e.used, e.at, *id));
        let mut removed: Vec<(String, &'static str)> = Vec::new();
        let mut gone: BTreeSet<&str> = BTreeSet::new();
        if let Some(age) = r.max_age {
            for (id, e) in &cands {
                if now - e.used > age {
                    removed.push((id.to_string(), "stale"));
                    gone.insert(id.as_str());
                }
            }
        }
        for (id, e) in &cands {
            if !gone.contains(id.as_str()) && e.importance < r.min_importance {
                removed.push((id.to_string(), "unimportant"));
                gone.insert(id.as_str());
            }
        }
        let mut count = (col.entries.len() - gone.len()) as u64;
        let mut size = col.bytes - gone.iter().map(|id| col.entries[*id].bytes).sum::<u64>();
        let over_entries = |count: u64| r.max_entries.is_some_and(|m| count > m);
        let over_bytes = |size: u64| r.max_bytes.is_some_and(|m| size > m);
        for (id, e) in &cands {
            if gone.contains(id.as_str()) {
                continue;
            }
            let reason = if over_entries(count) {
                "over_entries"
            } else if over_bytes(size) {
                "over_bytes"
            } else {
                break;
            };
            removed.push((id.to_string(), reason));
            gone.insert(id.as_str());
            count -= 1;
            size -= e.bytes;
        }
        let report = obj! {
            "bytes_after" => size, "bytes_before" => col.bytes, "collection" => name, "kept" => count,
            "over" => over_entries(count) || over_bytes(size),
            "removed" => Value::Array(removed.iter().map(|(id, why)| obj! { "id" => id.as_str(), "reason" => *why }).collect()),
        };
        (removed.into_iter().map(|(id, _)| id).collect(), size, report)
    }

    fn op_prune(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &["dry_run", "collection"])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        let dry = opt_bool(args, "dry_run", false)?;
        let only = opt(args, "collection").map(|v| ident(v).map(str::to_string)).transpose()?;
        need(now >= self.clock && only.as_ref().is_none_or(|n| self.collections.contains_key(n)))?;
        let names: Vec<String> = match only {
            Some(n) => vec![n],
            None => self.collections.keys().cloned().collect(),
        };
        let (mut reports, mut freed, mut total) = (Vec::new(), 0u64, 0u64);
        for name in names {
            let (gone, after, report) = self.plan(&name, now);
            let before = self.collections[&name].bytes;
            freed += before - after;
            total += gone.len() as u64;
            if !dry {
                let col = self.collections.get_mut(&name).ok_or_else(c::bad)?;
                for id in &gone {
                    col.entries.remove(id);
                }
                col.bytes = after;
            }
            reports.push(report);
        }
        if !dry {
            self.clock = now;
        }
        Ok(
            obj! { "collections" => Value::Array(reports), "dry_run" => dry, "freed" => freed, "now" => now, "removed" => total },
        )
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["collection"], &[])?;
        let name = id_of(args, "collection")?;
        let col = self.collections.get(&name).ok_or_else(c::bad)?;
        let oldest = col.entries.values().map(|e| e.used).min();
        let pinned = col.entries.values().filter(|e| e.pinned).count() as u64;
        Ok(obj! {
            "bytes" => col.bytes, "entries" => col.entries.len() as u64, "oldest_used" => Value::from(oldest),
            "pinned" => pinned, "rule" => self.rule_view(&name),
        })
    }

    // ------------------------------------------------------------ migration

    fn op_migration(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["version", "changes"], &[])?;
        let version = uint_bound(c::get(args, "version")?, 2, 1_000_000)?;
        let rows = c::get(args, "changes")?.as_array().ok_or_else(c::bad)?;
        need(!rows.is_empty() && rows.len() <= l.get("changes"))?;
        let mut changes = Vec::with_capacity(rows.len());
        for r in rows {
            let kind = r.get("op").and_then(Value::as_str).ok_or_else(c::bad)?;
            changes.push(match kind {
                "rename" => {
                    keys(r, &["op", "path", "to"], &[])?;
                    Change::Rename { path: parse_path(c::get(r, "path")?)?, to: id_of(r, "to")? }
                }
                "add" => {
                    keys(r, &["op", "path", "value"], &[])?;
                    Change::Add { path: parse_path(c::get(r, "path")?)?, value: c::get(r, "value")?.clone() }
                }
                "remove" => {
                    keys(r, &["op", "path"], &[])?;
                    Change::Remove { path: parse_path(c::get(r, "path")?)? }
                }
                "clamp" => {
                    keys(r, &["op", "path", "min", "max"], &[])?;
                    let min = int_of(c::get(r, "min")?).ok_or_else(c::bad)?;
                    let max = int_of(c::get(r, "max")?).ok_or_else(c::bad)?;
                    need(min <= max)?;
                    Change::Clamp { path: parse_path(c::get(r, "path")?)?, min, max }
                }
                _ => return Err(c::bad()),
            });
        }
        need(version == self.migrations.len() as u64 + 2 && self.migrations.len() < l.get("migrations"))?;
        self.migrations.push(changes);
        Ok(obj! { "changes" => self.migrations.last().map_or(0, Vec::len) as u64, "latest" => version })
    }

    fn op_migrate(&self, args: &Value) -> Result<Value> {
        keys(args, &["document", "from"], &[])?;
        let mut doc = c::get(args, "document")?.clone();
        need(matches!(doc, Value::Object(_)))?;
        let latest = self.migrations.len() as u64 + 1;
        let from = uint_bound(c::get(args, "from")?, 1, latest)?;
        let (mut work, mut edits) = (0u64, 0u64);
        for changes in &self.migrations[(from - 1) as usize..] {
            for ch in changes {
                let (head, last) = ch.path().split_at(ch.path().len() - 1);
                edits += apply(&mut doc, head, &last[0], ch, &mut work)?;
            }
        }
        Ok(obj! { "document" => doc, "edits" => edits, "from" => from, "to" => latest, "work" => work })
    }
}

/// Applies one change below `node` along `head`, counting every node a segment reaches.
fn apply(node: &mut Value, head: &[String], last: &str, ch: &Change, work: &mut u64) -> Result<u64> {
    let Some((seg, rest)) = head.split_first() else {
        return edit(node, last, ch);
    };
    let mut edits = 0;
    if seg == "*" {
        let kids: Vec<&mut Value> = match node {
            Value::Array(a) => a.iter_mut().collect(),
            Value::Object(o) => o.iter_mut().map(|(_, v)| v).collect(),
            _ => Vec::new(),
        };
        *work += kids.len() as u64;
        need(*work <= MAX_WORK)?;
        for k in kids {
            edits += apply(k, rest, last, ch, work)?;
        }
    } else if let Value::Object(o) = node {
        if let Some((_, v)) = o.iter_mut().find(|(k, _)| k == seg) {
            *work += 1;
            need(*work <= MAX_WORK)?;
            edits += apply(v, rest, last, ch, work)?;
        }
    }
    Ok(edits)
}

/// One change on one object (anything else is skipped); returns 1 when it changed something.
fn edit(node: &mut Value, last: &str, ch: &Change) -> Result<u64> {
    let Value::Object(o) = node else {
        return Ok(0);
    };
    let at = o.iter().position(|(k, _)| k == last);
    Ok(match (ch, at) {
        (Change::Rename { to, .. }, Some(i)) if to != last => {
            need(!o.iter().any(|(k, _)| k == to))?;
            o[i].0 = to.clone();
            1
        }
        (Change::Add { value, .. }, None) => {
            o.push((last.to_string(), value.clone()));
            1
        }
        (Change::Remove { .. }, Some(i)) => {
            o.remove(i);
            1
        }
        (Change::Clamp { min, max, .. }, Some(i)) => {
            let v = int_of(&o[i].1).ok_or_else(c::bad)?;
            let clamped = v.clamp(*min, *max);
            if clamped == v {
                0
            } else {
                o[i].1 = Value::from(clamped);
                1
            }
        }
        _ => 0,
    })
}

pub fn workflow(req: &Value) -> Result<Value> {
    Retention::new().workflow(req)
}
