//! Optional integer inventory snapshots and atomic transfers. Hosts own visibility,
//! permissions, prices, persistence and item semantics; proposals grant no authority.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
use std::collections::{BTreeMap, BTreeSet};
/// Host-set limits (defaults are the earlier fixed bounds): most `items` and `holdings`, and
/// the most serialized `bytes` in a snapshot (a workflow request may carry four times that).
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "items", default: 128, ceiling: 65_536 },
    crate::character::limits::Limit { name: "holdings", default: 256, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "bytes", default: 262_144, ceiling: 67_108_864 },
];
crate::character::limits::module_limits!(InventoryLimits, LIMITS);
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    c::text(c::get(v, key)?)
}
fn strings(v: &Value, max: usize) -> Result<Vec<String>> {
    let rows = c::list(v)?;
    if rows.len() > max {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for v in rows {
        let s = c::text(v)?;
        c::scope(s)?;
        if out.iter().any(|old| old == s) {
            return Err(c::bad());
        }
        out.push(s.into());
    }
    Ok(out)
}
#[derive(Clone, Debug)]
struct Holding {
    owner: String,
    item: String,
    quantity: u32,
}
impl Holding {
    fn value(&self) -> Value {
        obj! {"owner"=>self.owner.clone(),"item"=>self.item.clone(),"quantity"=>self.quantity}
    }
}
/// A bounded, host-owned ledger. Import existing engine data with `from_value`, or
/// use `rows` as a context/query adapter. No second engine ledger is required.
#[derive(Clone, Debug)]
pub struct Inventory {
    revision: u32,
    items: Vec<Value>,
    holdings: Vec<Holding>,
    /// Item id → position in `items` (items never change after loading).
    index: BTreeMap<String, usize>,
    max_holdings: usize,
}
impl Inventory {
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(input, 262144)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        Self::from_value_within(v, &InventoryLimits::default())
    }
    /// [`from_value`](Self::from_value) with host-set [limits](LIMITS); the result keeps the
    /// `holdings` limit for later transfers.
    pub fn from_value_within(v: &Value, l: &crate::character::limits::Limits) -> Result<Self> {
        bounded(v, l.get("bytes"))?;
        c::fields(v, &["version", "revision", "items", "holdings"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let revision = c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)? as u32;
        let items = c::list(c::get(v, "items")?)?;
        let rows = c::list(c::get(v, "holdings")?)?;
        if items.len() > l.get("items") || rows.len() > l.get("holdings") {
            return Err(c::bad());
        }
        let mut index = BTreeMap::new();
        for (n, i) in items.iter().enumerate() {
            let keys = if i.get("unique").is_some() {
                vec!["id", "label", "tags", "unique"]
            } else {
                vec!["id", "label", "tags"]
            };
            c::fields(i, &keys)?;
            if let Some(v) = i.get("unique") {
                c::boolean(v)?;
            }
            c::scope(text(i, "id")?)?;
            c::scope(text(i, "label")?)?;
            strings(c::get(i, "tags")?, 16)?;
            if index.insert(text(i, "id")?.to_owned(), n).is_some() {
                return Err(c::bad());
            }
        }
        let mut holdings: Vec<Holding> = Vec::new();
        let mut pairs = BTreeSet::new();
        let mut units: BTreeMap<&str, u64> = BTreeMap::new();
        for h in rows {
            c::fields(h, &["owner", "item", "quantity"])?;
            let owner = text(h, "owner")?;
            let item = text(h, "item")?;
            c::scope(owner)?;
            let quantity = c::uint(c::get(h, "quantity")?, 1, u32::MAX as u64)? as u32;
            if !index.contains_key(item) || !pairs.insert((owner, item)) {
                return Err(c::bad());
            }
            *units.entry(item).or_default() += u64::from(quantity);
            holdings.push(Holding { owner: owner.into(), item: item.into(), quantity });
        }
        for i in items {
            if i.get("unique") == Some(&Value::Bool(true))
                && i.get("id").and_then(Value::as_str).and_then(|id| units.get(id)).is_some_and(|n| *n > 1)
            {
                return Err(c::bad());
            }
        }
        Ok(Self { revision, items: items.to_vec(), holdings, index, max_holdings: l.get("holdings") })
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1,"revision"=>self.revision,"items"=>self.items.clone(),"holdings"=>self.holdings.iter().map(Holding::value).collect::<Vec<_>>()}
    }
    pub fn revision(&self) -> u32 {
        self.revision
    }
    /// Host-selected owners, with all requested tags required. Empty owners select
    /// nothing. Returned uniform rows can be passed to `query::run` or table packing.
    pub fn rows(&self, owners: &Value, tags: &Value) -> Result<Vec<Value>> {
        bounded(owners, 16384)?;
        bounded(tags, 8192)?;
        let owners = strings(owners, 32)?;
        let tags = strings(tags, 16)?;
        let mut rows = Vec::new();
        for h in &self.holdings {
            if !owners.contains(&h.owner) {
                continue;
            }
            let i = &self.items[self.index[&h.item]];
            let available = c::list(c::get(i, "tags")?)?;
            if tags.iter().all(|t| available.iter().any(|v| v.as_str() == Some(t))) {
                rows.push(obj!{"id"=>h.item.clone(),"label"=>c::get(i,"label")?.clone(),"tags"=>c::get(i,"tags")?.clone(),"owner"=>h.owner.clone(),"count"=>h.quantity});
            }
        }
        Ok(rows)
    }
    /// Read-only model function for an exact item ID. Owners remain host-bound.
    pub fn lookup_tool(&self) -> Value {
        obj! {"type"=>"function","function"=>obj!{"name"=>"inventory_lookup","description"=>"Look up current visible owners and exact counts for an item. This executes no action.","parameters"=>obj!{"type"=>"object","properties"=>obj!{"item"=>obj!{"type"=>"string","minLength"=>1,"maxLength"=>64}},"required"=>vec!["item"],"additionalProperties"=>false}}}
    }
    pub fn lookup_call(&self, input: &str, owners: &Value) -> Result<Value> {
        let a = crate::data::json::parse(input, 4096)?;
        c::fields(&a, &["item"])?;
        let item = text(&a, "item")?;
        c::scope(item)?;
        let rows = self
            .rows(owners, &Vec::<Value>::new().into())?
            .into_iter()
            .filter(|r| r.get("id") == a.get("item"))
            .collect::<Vec<_>>();
        Ok(obj! {"revision"=>self.revision,"rows"=>rows})
    }
    /// Validate every move and host permission before committing any change. Integer
    /// counts are conserved; failed/stale/overflowing batches leave the ledger intact.
    /// The callback checks host policy against the original state; capacity and funds
    /// spanning several moves must be checked by the host for the complete batch.
    pub fn transfer<F>(&mut self, revision: u32, moves: &Value, mut allowed: F) -> Result<Value>
    where
        F: FnMut(&Value) -> bool,
    {
        bounded(moves, 32768)?;
        let moves = c::list(moves)?;
        if revision != self.revision || moves.is_empty() || moves.len() > 32 {
            return Err(c::bad());
        }
        let next = self.revision.checked_add(1).ok_or_else(c::bad)?;
        let mut draft = self.clone();
        for m in moves {
            c::fields(m, &["item", "from", "to", "quantity"])?;
            let item = text(m, "item")?;
            let from = text(m, "from")?;
            let to = text(m, "to")?;
            for s in [item, from, to] {
                c::scope(s)?;
            }
            let quantity = c::uint(c::get(m, "quantity")?, 1, u32::MAX as u64)? as u32;
            if from == to || !allowed(m) {
                return Err(c::bad());
            }
            let src = draft.holdings.iter().position(|h| h.owner == from && h.item == item).ok_or_else(c::bad)?;
            if draft.holdings[src].quantity < quantity {
                return Err(c::bad());
            }
            if let Some(dst) = draft.holdings.iter().position(|h| h.owner == to && h.item == item) {
                draft.holdings[dst].quantity = draft.holdings[dst].quantity.checked_add(quantity).ok_or_else(c::bad)?;
            } else {
                draft.holdings.push(Holding { owner: to.into(), item: item.into(), quantity });
            }
            draft.holdings[src].quantity -= quantity;
        }
        draft.holdings.retain(|h| h.quantity > 0);
        if draft.holdings.len() > self.max_holdings {
            return Err(c::bad());
        }
        draft.revision = next;
        let result = draft.to_value();
        *self = draft;
        Ok(result)
    }
    /// Bounded model proposal. `owner` is supplied by the host, never accepted from
    /// model JSON. Replay protection uses the exact current inventory revision.
    pub fn propose<F>(&mut self, owner: &str, input: &str, allowed: F) -> Result<Value>
    where
        F: FnMut(&Value) -> bool,
    {
        c::scope(owner)?;
        let a = crate::data::json::parse(input, 4096)?;
        c::fields(&a, &["revision", "item", "to", "quantity"])?;
        let revision = c::uint(c::get(&a, "revision")?, 0, u32::MAX as u64)? as u32;
        self.transfer(revision,&vec![obj!{"item"=>c::get(&a,"item")?.clone(),"from"=>owner,"to"=>c::get(&a,"to")?.clone(),"quantity"=>c::get(&a,"quantity")?.clone()}].into(),allowed)
    }
}
/// Bounded stateless CLI/conformance adapter: `{snapshot, calls ≤64, limits?}` →
/// `{limits?, snapshot, results}`. `allow` is a host-provided exact move allowlist, not part
/// of a model call. Invalid calls are isolated and leave state intact. `limits` are the
/// [host-set limits](LIMITS), echoed when supplied.
pub fn workflow(a: &Value) -> Result<Value> {
    let mut limits = InventoryLimits::default();
    let stripped = crate::character::limits::take(a, &mut limits)?;
    let out = run_workflow(stripped.as_ref().unwrap_or(a), &limits)?;
    Ok(crate::character::limits::echo(out, &limits, stripped.is_some()))
}

fn run_workflow(a: &Value, l: &crate::character::limits::Limits) -> Result<Value> {
    bounded(a, l.get("bytes").saturating_mul(4))?;
    c::fields(a, &["snapshot", "calls"])?;
    let mut inv = Inventory::from_value_within(c::get(a, "snapshot")?, l)?;
    let calls = c::list(c::get(a, "calls")?)?;
    if calls.len() > 64 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for call in calls {
        let result = (|| match text(call, "op")? {
            "rows" => {
                c::fields(call, &["op", "owners", "tags"])?;
                Ok(Value::Array(inv.rows(c::get(call, "owners")?, c::get(call, "tags")?)?))
            }
            "lookup" => {
                c::fields(call, &["op", "text", "owners"])?;
                inv.lookup_call(text(call, "text")?, c::get(call, "owners")?)
            }
            "transfer" | "propose" => {
                let allow = c::list(c::get(call, "allow")?)?;
                if allow.len() > 32 {
                    return Err(c::bad());
                }
                let allowed = |m: &Value| allow.iter().any(|v| v.same(m));
                if text(call, "op")? == "transfer" {
                    c::fields(call, &["op", "revision", "moves", "allow"])?;
                    inv.transfer(
                        c::uint(c::get(call, "revision")?, 0, u32::MAX as u64)? as u32,
                        c::get(call, "moves")?,
                        allowed,
                    )
                } else {
                    c::fields(call, &["op", "owner", "text", "allow"])?;
                    inv.propose(text(call, "owner")?, text(call, "text")?, allowed)
                }
            }
            _ => Err(c::bad()),
        })();
        results.push(match result {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"snapshot"=>inv.to_value(),"results"=>results})
}
