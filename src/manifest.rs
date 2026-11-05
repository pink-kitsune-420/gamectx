//! A manifest describes one packet: which sections it has, how each is packed, and the budget.
//!
//! ```json
//! {"budget_bytes": 8192, "budget_tokens": 3000,
//!  "sections": [
//!   {"name": "actions", "pack": "schema", "file": "tools.json", "priority": 10, "stable": true, "required": true},
//!   {"name": "inventory", "pack": "table", "group_by": "kind", "file": "inventory.json", "priority": 6},
//!   {"name": "nearby_items", "pack": "catalog", "file": "items.json", "query": "lantern", "count": 4, "priority": 3},
//!   {"name": "mood", "pack": "raw", "value": "wary", "priority": 8}
//!  ]}
//! ```
//!
//! Pack modes: `raw` (as is), `table` (lossless; `group_by` regroups rows, and the view keys
//! `sort_by`, `limit`, `fields` and `round` shape rows first) `schema` and `signatures` (lossless; signatures restore tools without a provider wrapper), `outline` and `catalog`
//! (lossy by design: a summary and a search page). `file` paths are relative to the manifest.
//! Both budgets are optional; when both are set the packet must fit both.

use crate::budget::Section;
use crate::catalog::{Catalog, Limits};
use crate::json::{self, Error, Result, Value};
use crate::view::{self, View};
use crate::{schema, signatures, table};
use std::path::Path;

/// Largest manifest or data file read, in bytes.
pub const MAX_FILE_BYTES: usize = 16 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pack {
    Raw,
    Table,
    Schema,
    Signatures,
    Outline,
    Catalog,
}

impl Pack {
    pub fn name(self) -> &'static str {
        match self {
            Pack::Raw => "raw",
            Pack::Table => "table",
            Pack::Schema => "schema",
            Pack::Signatures => "signatures",
            Pack::Outline => "outline",
            Pack::Catalog => "catalog",
        }
    }

    pub fn lossless(self) -> bool {
        matches!(self, Pack::Raw | Pack::Table | Pack::Schema | Pack::Signatures)
    }
}

/// One section before and after packing.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub pack: Pack,
    pub original: Value,
    /// Table sections: the view applied before packing (empty when none).
    pub view: View,
    /// For grouped tables, the column rows were grouped by.
    pub group_by: Option<String>,
    pub section: Section,
}

/// Outcome of [`Prepared::verify`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Unpacking gives back the original exactly.
    Exact,
    /// Unpacking gives back exactly the rows the view selected; the view itself sorted,
    /// limited, dropped columns or rounded on purpose.
    ExactAfterView,
    /// An outline or catalog page: a summary by design, with nothing to restore.
    Lossy,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub budget_bytes: Option<usize>,
    /// Checked with a calibrated estimate when one is supplied, otherwise the byte bound.
    pub budget_tokens: Option<usize>,
    pub sections: Vec<Prepared>,
}

pub fn read_json_file(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::new(format!("{}: {e}", path.display())))?;
    json::parse(&text, MAX_FILE_BYTES).map_err(|e| Error::new(format!("{}: {e}", path.display())))
}

pub fn load(path: &Path) -> Result<Manifest> {
    let root = read_json_file(path)?;
    let base = path.parent().unwrap_or(Path::new("."));
    parse(&root, base)
}

/// Parses a manifest value; `base` resolves relative `file` paths.
pub fn parse(root: &Value, base: &Path) -> Result<Manifest> {
    allow_keys(root, &["budget_bytes", "budget_tokens", "sections"], "manifest")?;
    let budget = |key: &str| {
        root.get(key)
            .map(|v| v.as_usize().ok_or_else(|| Error::new(format!("{key} must be a non-negative integer"))))
            .transpose()
    };
    let (budget_bytes, budget_tokens) = (budget("budget_bytes")?, budget("budget_tokens")?);
    let list =
        root.get("sections").and_then(Value::as_array).ok_or_else(|| Error::new("manifest needs a sections array"))?;
    let mut sections = Vec::with_capacity(list.len());
    for (n, item) in list.iter().enumerate() {
        sections.push(section(item, base).map_err(|e| Error::new(format!("section {n}: {e}")))?);
    }
    Ok(Manifest { budget_bytes, budget_tokens, sections })
}

fn allow_keys(value: &Value, allowed: &[&str], what: &str) -> Result<()> {
    let entries = value.as_object().ok_or_else(|| Error::new(format!("{what} must be an object")))?;
    match entries.iter().find(|(k, _)| !allowed.contains(&k.as_str())) {
        Some((k, _)) => Err(Error::new(format!("unknown {what} key \"{k}\""))),
        None => Ok(()),
    }
}

fn section(item: &Value, base: &Path) -> Result<Prepared> {
    allow_keys(
        item,
        &[
            "name", "pack", "priority", "stable", "required", "value", "file", "group_by", "sort_by", "limit",
            "fields", "round", "query", "kind", "offset", "count",
        ],
        "section",
    )?;
    let name = item.get("name").and_then(Value::as_str).ok_or_else(|| Error::new("name must be a string"))?.to_owned();
    let pack = match item.get("pack").and_then(Value::as_str).unwrap_or("raw") {
        "raw" => Pack::Raw,
        "table" => Pack::Table,
        "schema" => Pack::Schema,
        "signatures" => Pack::Signatures,
        "outline" => Pack::Outline,
        "catalog" => Pack::Catalog,
        other => return Err(Error::new(format!("unknown pack mode \"{other}\""))),
    };
    let priority = match item.get("priority") {
        None => 0,
        Some(Value::Number(n)) => n.parse::<i64>().map_err(|_| Error::new("priority must be an integer"))?,
        Some(_) => return Err(Error::new("priority must be an integer")),
    };
    let flag = |key: &str| {
        item.get(key)
            .map_or(Ok(false), |v| v.as_bool().ok_or_else(|| Error::new(format!("{key} must be true or false"))))
    };
    let (stable, required) = (flag("stable")?, flag("required")?);
    let original = match (item.get("value"), item.get("file").and_then(Value::as_str)) {
        (Some(v), None) => v.clone(),
        (None, Some(file)) => read_json_file(&base.join(file))?,
        _ => return Err(Error::new("give exactly one of value or file")),
    };
    let only_for = |keys: &[&str], mode: Pack| -> Result<()> {
        match keys.iter().find(|k| item.get(k).is_some()) {
            Some(k) if pack != mode => Err(Error::new(format!("\"{k}\" only applies to pack \"{}\"", mode.name()))),
            _ => Ok(()),
        }
    };
    only_for(&["group_by", "sort_by", "limit", "fields", "round"], Pack::Table)?;
    let view = View::from_json(item)?;
    only_for(&["query", "kind", "offset", "count"], Pack::Catalog)?;
    let group_by = match item.get("group_by") {
        None => None,
        Some(v) => Some(v.as_str().ok_or_else(|| Error::new("group_by must be a column name"))?.to_owned()),
    };

    let list = || original.as_array().ok_or_else(|| Error::new(format!("pack \"{}\" needs an array", pack.name())));
    let value = match pack {
        Pack::Raw => original.clone(),
        Pack::Table => table::pack(&view::apply(list()?, &view)?, group_by.as_deref())?,
        Pack::Schema => schema::pack(list()?)?,
        Pack::Signatures => Value::str(signatures::pack(list()?)?),
        Pack::Outline => Value::str(schema::outline(list()?)?),
        Pack::Catalog => {
            let catalog = Catalog::from_json(&original, Limits::default())?;
            let query = item.get("query").and_then(Value::as_str).unwrap_or("");
            let kind =
                item.get("kind").map(|k| k.as_str().ok_or_else(|| Error::new("kind must be a string"))).transpose()?;
            let number = |key: &str, default: usize| {
                item.get(key).map_or(Ok(default), |v| {
                    v.as_usize().ok_or_else(|| Error::new(format!("{key} must be a non-negative integer")))
                })
            };
            catalog
                .search(query, kind, number("offset", 0)?, number("count", Limits::default().results)?, None)?
                .to_value()
        }
    };
    Ok(Prepared { pack, original, view, group_by, section: Section { name, priority, stable, required, value } })
}

impl Prepared {
    /// Unpacks lossless sections and compares them with the original: for tables, the rows
    /// after the view, in group order when grouped.
    pub fn verify(&self) -> Result<Check> {
        let restored = match self.pack {
            Pack::Raw => self.section.value.clone(),
            Pack::Table => Value::Array(table::unpack(&self.section.value)?),
            Pack::Schema => Value::Array(schema::unpack(&self.section.value)?),
            Pack::Signatures => Value::Array(signatures::unpack(self.section.value.as_str().unwrap_or_default())?),
            Pack::Outline | Pack::Catalog => return Ok(Check::Lossy),
        };
        let mut expected = self.original.clone();
        if let (Pack::Signatures, Value::Array(tools)) = (self.pack, &self.original) {
            expected = Value::Array(tools.iter().map(signatures::bare).collect());
        }
        if let Value::Array(items) = &self.original {
            if self.pack == Pack::Table {
                let viewed = view::apply(items, &self.view)?;
                expected = Value::Array(match &self.group_by {
                    Some(column) => table::group_order(&viewed, column),
                    None => viewed,
                });
            }
        }
        if restored.same(&expected) {
            Ok(if self.view.is_empty() { Check::Exact } else { Check::ExactAfterView })
        } else {
            Err(Error::new(format!("section \"{}\" did not round-trip", self.section.name)))
        }
    }
}
