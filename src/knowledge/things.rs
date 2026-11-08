//! Host-authoritative world things: materials, thing kinds, and thing instances.
//! Zero third-party dependencies; Rust standard library only.
use crate::data::json::Value;
use crate::knowledge::props::ident;
use crate::{checked as c, obj, Result};

pub const MAX_MATERIALS: usize = 256;
pub const MAX_KINDS: usize = 512;
pub const MAX_THINGS: usize = 4096;
pub const MAX_OPS: usize = 64;
pub const MAX_SAFE: u64 = 9_007_199_254_740_991;
/// Host-set limits (defaults are the earlier fixed bounds): most `materials`, `kinds` and
/// `things`, and the most serialized `bytes` in a loaded things document.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "materials", default: MAX_MATERIALS, ceiling: 65_536 },
    crate::character::limits::Limit { name: "kinds", default: MAX_KINDS, ceiling: 65_536 },
    crate::character::limits::Limit { name: "things", default: MAX_THINGS, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "bytes", default: 1_048_576, ceiling: 268_435_456 },
];
crate::character::limits::module_limits!(ThingsLimits, LIMITS);

pub const CATEGORIES: [&str; 11] = [
    "tool",
    "weapon",
    "furniture",
    "container",
    "food",
    "material",
    "clothing",
    "vehicle",
    "structure",
    "plant",
    "other",
];

pub const LOC_KEYS: [&str; 4] = ["place", "container", "holder", "carrier"];

fn text_bound(v: &Value, max_len: usize) -> Result<&str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    if s.is_empty() || s.chars().count() > max_len {
        return Err(c::bad());
    }
    if s.chars().any(|ch| (ch as u32) < 32 || (127..=159).contains(&(ch as u32))) {
        return Err(c::bad());
    }
    Ok(s)
}

fn ident_bound(v: &Value) -> Result<&str> {
    ident(v)
}

fn uint_bound(v: &Value, lo: u64, hi: u64) -> Result<u64> {
    c::uint(v, lo, hi)
}

fn bool_bound(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(c::bad)
}

fn parse_location(v: &Value) -> Result<(&'static str, String)> {
    let mut found = None;
    for &k in &LOC_KEYS {
        if let Some(val) = v.get(k).filter(|x| **x != Value::Null) {
            if found.is_some() {
                return Err(c::bad());
            }
            let id = ident_bound(val)?;
            found = Some((k, id.to_owned()));
        }
    }
    found.ok_or_else(c::bad)
}

fn set(e: &mut Value, key: &str, value: Value) {
    if let Value::Object(fields) = e {
        if let Some((_, v)) = fields.iter_mut().find(|(k, _)| k == key) {
            *v = value;
        } else {
            fields.push((key.to_string(), value));
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct YieldMaterial {
    pub material: String,
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThingFunction {
    pub function: String,
    pub power: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialDef {
    pub id: String,
    pub label: String,
    pub unit_weight: Option<u64>,
    pub unit_value: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KindDef {
    pub id: String,
    pub label: String,
    pub category: String,
    pub materials: Vec<YieldMaterial>,
    pub functions: Vec<ThingFunction>,
    pub weight: u64,
    pub base_value: u64,
    pub durability: u64,
    pub stackable: bool,
    pub capacity: Option<u64>,
    pub accepted_tags: Option<Vec<String>>,
    pub load_capacity: Option<u64>,
    pub seats: Option<u64>,
    pub tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThingInstance {
    pub id: String,
    pub kind: String,
    pub loc_kind: &'static str,
    pub loc_id: String,
    pub condition: u64,
    pub owner: Option<String>,
    pub quantity: u64,
    pub passengers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Things {
    materials: Vec<MaterialDef>,
    kinds: Vec<KindDef>,
    things: Vec<ThingInstance>,
    limits: ThingsLimits,
}

impl Things {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(input: &str) -> Result<Self> {
        let v = crate::data::json::parse(input, 1_048_576)?;
        Self::from_value(&v)
    }

    pub fn from_value(v: &Value) -> Result<Self> {
        Self::from_value_within(v, &ThingsLimits::default())
    }

    /// [`from_value`](Self::from_value) with host-set [limits](LIMITS); the result keeps them.
    pub fn from_value_within(v: &Value, l: &crate::character::limits::Limits) -> Result<Self> {
        crate::packet::request::bounded(v, l.get("bytes"))?;
        crate::knowledge::codex::keys(v, &["version"], &["materials", "kinds", "things"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;

        let mut materials = Vec::new();
        if let Some(mats) = v.get("materials").filter(|x| **x != Value::Null) {
            let list = mats.as_array().ok_or_else(c::bad)?;
            if list.len() > l.get("materials") {
                return Err(c::bad());
            }
            for m in list {
                crate::knowledge::codex::keys(m, &["id", "label"], &["unit_weight", "unit_value"])?;
                let id = ident_bound(c::get(m, "id")?)?.to_owned();
                let label = text_bound(c::get(m, "label")?, 48)?.to_owned();
                if materials.iter().any(|old: &MaterialDef| old.id == id) {
                    return Err(c::bad());
                }
                let unit_weight =
                    if let Some(w) = m.get("unit_weight") { Some(uint_bound(w, 0, 1_000_000_000)?) } else { None };
                let unit_value =
                    if let Some(uv) = m.get("unit_value") { Some(uint_bound(uv, 0, 1_000_000_000)?) } else { None };
                materials.push(MaterialDef { id, label, unit_weight, unit_value });
            }
        }

        let mut kinds = Vec::new();
        if let Some(k_list) = v.get("kinds").filter(|x| **x != Value::Null) {
            let list = k_list.as_array().ok_or_else(c::bad)?;
            if list.len() > l.get("kinds") {
                return Err(c::bad());
            }
            for k in list {
                crate::knowledge::codex::keys(
                    k,
                    &[
                        "id",
                        "label",
                        "category",
                        "materials",
                        "functions",
                        "weight",
                        "base_value",
                        "durability",
                        "stackable",
                    ],
                    &["capacity", "accepted_tags", "load_capacity", "seats", "tags"],
                )?;
                let id = ident_bound(c::get(k, "id")?)?.to_owned();
                let label = text_bound(c::get(k, "label")?, 48)?.to_owned();
                let cat = text_bound(c::get(k, "category")?, 32)?;
                if !CATEGORIES.contains(&cat) || kinds.iter().any(|old: &KindDef| old.id == id) {
                    return Err(c::bad());
                }

                let m_arr = c::get(k, "materials")?.as_array().ok_or_else(c::bad)?;
                if m_arr.len() > 8 {
                    return Err(c::bad());
                }
                let mut ymats = Vec::new();
                for ym in m_arr {
                    crate::knowledge::codex::keys(ym, &["material", "amount"], &[])?;
                    let mid = ident_bound(c::get(ym, "material")?)?;
                    if !materials.iter().any(|m| m.id == mid) || ymats.iter().any(|y: &YieldMaterial| y.material == mid)
                    {
                        return Err(c::bad());
                    }
                    let amount = uint_bound(c::get(ym, "amount")?, 1, 1_000_000_000)?;
                    ymats.push(YieldMaterial { material: mid.to_owned(), amount });
                }

                let f_arr = c::get(k, "functions")?.as_array().ok_or_else(c::bad)?;
                if f_arr.len() > 16 {
                    return Err(c::bad());
                }
                let mut funcs = Vec::new();
                for fn_val in f_arr {
                    crate::knowledge::codex::keys(fn_val, &["function", "power"], &[])?;
                    let fn_name = ident_bound(c::get(fn_val, "function")?)?;
                    if funcs.iter().any(|f: &ThingFunction| f.function == fn_name) {
                        return Err(c::bad());
                    }
                    let power = uint_bound(c::get(fn_val, "power")?, 1, 1_000_000_000)?;
                    funcs.push(ThingFunction { function: fn_name.to_owned(), power });
                }

                let weight = uint_bound(c::get(k, "weight")?, 0, 1_000_000_000)?;
                let base_value = uint_bound(c::get(k, "base_value")?, 0, 1_000_000_000)?;
                let durability = uint_bound(c::get(k, "durability")?, 0, 1_000_000_000)?;
                let stackable = bool_bound(c::get(k, "stackable")?)?;

                let capacity =
                    if let Some(cap) = k.get("capacity") { Some(uint_bound(cap, 0, 1_000_000_000)?) } else { None };
                let accepted_tags = if let Some(at) = k.get("accepted_tags") {
                    let arr = at.as_array().ok_or_else(c::bad)?;
                    if arr.len() > 8 {
                        return Err(c::bad());
                    }
                    let mut tags = Vec::new();
                    for tag in arr {
                        let t_str = ident_bound(tag)?.to_owned();
                        if tags.contains(&t_str) {
                            return Err(c::bad());
                        }
                        tags.push(t_str);
                    }
                    Some(tags)
                } else {
                    None
                };
                let load_capacity =
                    if let Some(lc) = k.get("load_capacity") { Some(uint_bound(lc, 0, 1_000_000_000)?) } else { None };
                let seats = if let Some(s) = k.get("seats") { Some(uint_bound(s, 0, 64)?) } else { None };
                let tags = if let Some(tg) = k.get("tags") {
                    let arr = tg.as_array().ok_or_else(c::bad)?;
                    if arr.len() > 8 {
                        return Err(c::bad());
                    }
                    let mut list = Vec::new();
                    for tag in arr {
                        let t_str = ident_bound(tag)?.to_owned();
                        if list.contains(&t_str) {
                            return Err(c::bad());
                        }
                        list.push(t_str);
                    }
                    Some(list)
                } else {
                    None
                };

                kinds.push(KindDef {
                    id,
                    label,
                    category: cat.to_owned(),
                    materials: ymats,
                    functions: funcs,
                    weight,
                    base_value,
                    durability,
                    stackable,
                    capacity,
                    accepted_tags,
                    load_capacity,
                    seats,
                    tags,
                });
            }
        }

        let mut instance = Self { materials, kinds, things: Vec::new(), limits: ThingsLimits(l.clone()) };

        if let Some(th_list) = v.get("things").filter(|x| **x != Value::Null) {
            let list = th_list.as_array().ok_or_else(c::bad)?;
            if list.len() > l.get("things") {
                return Err(c::bad());
            }
            for t in list {
                crate::knowledge::codex::keys(
                    t,
                    &["id", "kind", "location", "condition", "quantity"],
                    &["owner", "passengers"],
                )?;
                let id = ident_bound(c::get(t, "id")?)?.to_owned();
                let kind = ident_bound(c::get(t, "kind")?)?.to_owned();
                let kd = instance.find_kind(&kind).ok_or_else(c::bad)?;
                let (loc_kind, loc_id) = parse_location(c::get(t, "location")?)?;
                let condition = uint_bound(c::get(t, "condition")?, 0, 100)?;
                let quantity = uint_bound(c::get(t, "quantity")?, 1, 1_000_000_000)?;
                if !kd.stackable && quantity != 1 {
                    return Err(c::bad());
                }
                let owner = if let Some(ow) = t.get("owner").filter(|x| **x != Value::Null) {
                    Some(ident_bound(ow)?.to_owned())
                } else {
                    None
                };
                let mut passengers = Vec::new();
                if let Some(ps) = t.get("passengers").filter(|x| **x != Value::Null) {
                    let arr = ps.as_array().ok_or_else(c::bad)?;
                    for p in arr {
                        let p_str = ident_bound(p)?.to_owned();
                        if passengers.contains(&p_str) {
                            return Err(c::bad());
                        }
                        passengers.push(p_str);
                    }
                }
                if passengers.len() > kd.seats.unwrap_or(0) as usize {
                    return Err(c::bad());
                }
                if instance.things.iter().any(|old| old.id == id) {
                    return Err(c::bad());
                }
                instance.things.push(ThingInstance {
                    id,
                    kind,
                    loc_kind,
                    loc_id,
                    condition,
                    owner,
                    quantity,
                    passengers,
                });
            }
            instance.check_all_containers_and_carriers()?;
        }

        Ok(instance)
    }

    pub fn to_value(&self) -> Value {
        let mats: Vec<Value> = self
            .materials
            .iter()
            .map(|m| {
                let mut v = obj! {
                    "id" => m.id.clone(),
                    "label" => m.label.clone(),
                };
                if let Some(w) = m.unit_weight {
                    set(&mut v, "unit_weight", Value::from(w));
                }
                if let Some(u) = m.unit_value {
                    set(&mut v, "unit_value", Value::from(u));
                }
                v
            })
            .collect();

        let knds: Vec<Value> = self
            .kinds
            .iter()
            .map(|k| {
                let ym: Vec<Value> = k
                    .materials
                    .iter()
                    .map(|y| {
                        obj! {
                            "material" => y.material.clone(),
                            "amount" => y.amount,
                        }
                    })
                    .collect();
                let fnc: Vec<Value> = k
                    .functions
                    .iter()
                    .map(|f| {
                        obj! {
                            "function" => f.function.clone(),
                            "power" => f.power,
                        }
                    })
                    .collect();
                let mut v = obj! {
                    "id" => k.id.clone(),
                    "label" => k.label.clone(),
                    "category" => k.category.clone(),
                    "materials" => ym,
                    "functions" => fnc,
                    "weight" => k.weight,
                    "base_value" => k.base_value,
                    "durability" => k.durability,
                    "stackable" => k.stackable,
                };
                if let Some(cap) = k.capacity {
                    set(&mut v, "capacity", Value::from(cap));
                }
                if let Some(at) = &k.accepted_tags {
                    set(&mut v, "accepted_tags", Value::Array(at.iter().map(|t| Value::String(t.clone())).collect()));
                }
                if let Some(lc) = k.load_capacity {
                    set(&mut v, "load_capacity", Value::from(lc));
                }
                if let Some(s) = k.seats {
                    set(&mut v, "seats", Value::from(s));
                }
                if let Some(tg) = &k.tags {
                    set(&mut v, "tags", Value::Array(tg.iter().map(|t| Value::String(t.clone())).collect()));
                }
                v
            })
            .collect();

        let ths: Vec<Value> = self
            .things
            .iter()
            .map(|t| {
                let loc = obj! { t.loc_kind => t.loc_id.clone() };
                let ps: Vec<Value> = t.passengers.iter().map(|p| Value::String(p.clone())).collect();
                obj! {
                    "id" => t.id.clone(),
                    "kind" => t.kind.clone(),
                    "location" => loc,
                    "condition" => t.condition,
                    "owner" => t.owner.as_ref().map(|o| Value::String(o.clone())).unwrap_or(Value::Null),
                    "quantity" => t.quantity,
                    "passengers" => ps,
                }
            })
            .collect();

        obj! {
            "version" => 1u64,
            "materials" => mats,
            "kinds" => knds,
            "things" => ths,
        }
    }

    fn find_kind(&self, id: &str) -> Option<&KindDef> {
        self.kinds.iter().find(|k| k.id == id)
    }

    fn find_thing(&self, id: &str) -> Option<&ThingInstance> {
        self.things.iter().find(|t| t.id == id)
    }

    fn find_thing_mut(&mut self, id: &str) -> Option<&mut ThingInstance> {
        self.things.iter_mut().find(|t| t.id == id)
    }

    fn effective_place(&self, thing_id: &str) -> Option<String> {
        let mut curr = thing_id.to_owned();
        let mut seen = Vec::new();
        loop {
            if seen.contains(&curr) {
                return None;
            }
            seen.push(curr.clone());
            let t = self.find_thing(&curr)?;
            match t.loc_kind {
                "place" => return Some(t.loc_id.clone()),
                "container" | "carrier" => curr = t.loc_id.clone(),
                _ => return None,
            }
        }
    }

    fn total_weight(&self, thing_id: &str) -> u64 {
        if let Some(t) = self.find_thing(thing_id) {
            if let Some(kd) = self.find_kind(&t.kind) {
                return kd.weight.saturating_mul(t.quantity);
            }
        }
        0
    }

    fn check_container_fit(&self, container_id: &str, thing_id: &str) -> Result<()> {
        let ct = self.find_thing(container_id).ok_or_else(c::bad)?;
        let ck = self.find_kind(&ct.kind).ok_or_else(c::bad)?;
        if ck.category != "container" && ck.capacity.is_none() {
            return Err(c::bad());
        }
        // Cycle check
        let mut curr = container_id.to_owned();
        for _ in 0..16 {
            if curr == thing_id {
                return Err(c::bad());
            }
            if let Some(parent) = self.find_thing(&curr) {
                if parent.loc_kind == "container" || parent.loc_kind == "carrier" {
                    curr = parent.loc_id.clone();
                    continue;
                }
            }
            break;
        }
        // Tag check
        if let Some(accepted) = &ck.accepted_tags {
            if !accepted.is_empty() {
                let th = self.find_thing(thing_id).ok_or_else(c::bad)?;
                let tk = self.find_kind(&th.kind).ok_or_else(c::bad)?;
                let empty_vec = Vec::new();
                let item_tags = tk.tags.as_ref().unwrap_or(&empty_vec);
                if !item_tags.iter().any(|t| accepted.contains(t)) {
                    return Err(c::bad());
                }
            }
        }
        // Capacity check
        if let Some(cap) = ck.capacity {
            let cur_weight: u64 = self
                .things
                .iter()
                .filter(|x| x.loc_kind == "container" && x.loc_id == container_id && x.id != thing_id)
                .map(|x| self.total_weight(&x.id))
                .sum();
            let add_weight = self.total_weight(thing_id);
            if cur_weight.saturating_add(add_weight) > cap {
                return Err(c::bad());
            }
        }
        Ok(())
    }

    fn check_carrier_fit(&self, carrier_id: &str, thing_id: &str) -> Result<()> {
        let ct = self.find_thing(carrier_id).ok_or_else(c::bad)?;
        let ck = self.find_kind(&ct.kind).ok_or_else(c::bad)?;
        if ck.category != "vehicle" && ck.category != "other" && ck.load_capacity.is_none() {
            return Err(c::bad());
        }
        // Cycle check
        let mut curr = carrier_id.to_owned();
        for _ in 0..16 {
            if curr == thing_id {
                return Err(c::bad());
            }
            if let Some(parent) = self.find_thing(&curr) {
                if parent.loc_kind == "container" || parent.loc_kind == "carrier" {
                    curr = parent.loc_id.clone();
                    continue;
                }
            }
            break;
        }
        if let Some(cap) = ck.load_capacity {
            let cur_weight: u64 = self
                .things
                .iter()
                .filter(|x| x.loc_kind == "carrier" && x.loc_id == carrier_id && x.id != thing_id)
                .map(|x| self.total_weight(&x.id))
                .sum();
            let add_weight = self.total_weight(thing_id);
            if cur_weight.saturating_add(add_weight) > cap {
                return Err(c::bad());
            }
        }
        Ok(())
    }

    fn check_all_containers_and_carriers(&self) -> Result<()> {
        for t in &self.things {
            if t.loc_kind == "container" {
                self.check_container_fit(&t.loc_id, &t.id)?;
            } else if t.loc_kind == "carrier" {
                self.check_carrier_fit(&t.loc_id, &t.id)?;
            }
        }
        Ok(())
    }

    fn op_create(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["id", "kind", "location"], &["condition", "owner", "quantity"])?;
        if self.things.len() >= self.limits.get("things") {
            return Err(c::bad());
        }
        let tid = ident_bound(c::get(args, "id")?)?.to_owned();
        if self.find_thing(&tid).is_some() {
            return Err(c::bad());
        }
        let kind = ident_bound(c::get(args, "kind")?)?.to_owned();
        let kd = self.find_kind(&kind).ok_or_else(c::bad)?;
        let (loc_kind, loc_id) = parse_location(c::get(args, "location")?)?;
        let cond = if let Some(v) = args.get("condition") { uint_bound(v, 0, 100)? } else { 100 };
        let qty = if let Some(v) = args.get("quantity") { uint_bound(v, 1, 1_000_000_000)? } else { 1 };
        if !kd.stackable && qty != 1 {
            return Err(c::bad());
        }
        let owner = if let Some(v) = args.get("owner").filter(|x| **x != Value::Null) {
            Some(ident_bound(v)?.to_owned())
        } else {
            None
        };
        let kd_label = kd.label.clone();
        let t = ThingInstance {
            id: tid.clone(),
            kind: kind.clone(),
            loc_kind,
            loc_id: loc_id.clone(),
            condition: cond,
            owner: owner.clone(),
            quantity: qty,
            passengers: Vec::new(),
        };
        self.things.push(t);

        if loc_kind == "container" {
            self.check_container_fit(&loc_id, &tid)?;
        } else if loc_kind == "carrier" {
            self.check_carrier_fit(&loc_id, &tid)?;
        }

        let place = self.effective_place(&tid);
        let actors = if let Some(ow) = &owner { vec![Value::String(ow.clone())] } else { Vec::<Value>::new() };
        let summary = format!("Created {} ({}).", kd_label, tid);
        let entry = obj! {
            "at" => at,
            "kind" => "create_thing",
            "summary" => summary,
            "actors" => actors,
            "place" => place.map(Value::String).unwrap_or(Value::Null),
            "things" => vec![Value::String(tid.clone())],
            "tags" => vec![Value::String("things".into()), Value::String("create".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(kd_label), Value::String(tid.clone())],
            "facts" => obj! {
                "kind" => kind,
                "quantity" => qty,
                "condition" => cond,
            }
        };

        Ok(obj! { "id" => tid, "entry" => entry })
    }

    fn op_destroy(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["thing"], &["actor"])?;
        let tid = ident_bound(c::get(args, "thing")?)?;
        let idx = self.things.iter().position(|t| t.id == tid).ok_or_else(c::bad)?;
        let t = self.things[idx].clone();
        let kd = self.find_kind(&t.kind).ok_or_else(c::bad)?.clone();
        let actor = if let Some(a) = args.get("actor") { Some(ident_bound(a)?.to_owned()) } else { None };
        let place = self.effective_place(tid);
        let cond = t.condition;
        let qty = t.quantity;

        let mut yields = Vec::new();
        for ym in &kd.materials {
            let tot = ym.amount.saturating_mul(qty);
            let salvaged = (tot.saturating_mul(cond)) / 100;
            yields.push(obj! {
                "material" => ym.material.clone(),
                "amount" => salvaged,
            });
        }

        self.things.remove(idx);

        for other in &mut self.things {
            if (other.loc_kind == "container" || other.loc_kind == "carrier") && other.loc_id == tid {
                other.loc_kind = "place";
                other.loc_id = place.clone().unwrap_or_else(|| "void".into());
            }
        }

        let actors = if let Some(ac) = actor {
            vec![Value::String(ac)]
        } else if let Some(ow) = t.owner {
            vec![Value::String(ow)]
        } else {
            Vec::<Value>::new()
        };

        let summary = format!("Destroyed {} ({}).", kd.label, tid);
        let entry = obj! {
            "at" => at,
            "kind" => "destroy_thing",
            "summary" => summary,
            "actors" => actors,
            "place" => place.map(Value::String).unwrap_or(Value::Null),
            "things" => vec![Value::String(tid.to_owned())],
            "tags" => vec![Value::String("things".into()), Value::String("destroy".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(kd.label), Value::String(tid.to_owned())],
            "facts" => obj! {
                "kind" => t.kind,
                "condition" => cond,
            }
        };

        Ok(obj! { "yields" => yields, "entry" => entry })
    }

    fn op_remove(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["thing"], &["actor"])?;
        let tid = ident_bound(c::get(args, "thing")?)?;
        let idx = self.things.iter().position(|t| t.id == tid).ok_or_else(c::bad)?;
        let t = self.things[idx].clone();
        let kd = self.find_kind(&t.kind).ok_or_else(c::bad)?.clone();
        let actor = if let Some(a) = args.get("actor") { Some(ident_bound(a)?.to_owned()) } else { None };
        let place = self.effective_place(tid);
        self.things.remove(idx);

        for other in &mut self.things {
            if (other.loc_kind == "container" || other.loc_kind == "carrier") && other.loc_id == tid {
                other.loc_kind = "place";
                other.loc_id = place.clone().unwrap_or_else(|| "void".into());
            }
        }

        let actors = if let Some(ac) = actor {
            vec![Value::String(ac)]
        } else if let Some(ow) = t.owner {
            vec![Value::String(ow)]
        } else {
            Vec::<Value>::new()
        };

        let summary = format!("Removed {} ({}).", kd.label, tid);
        let entry = obj! {
            "at" => at,
            "kind" => "remove_thing",
            "summary" => summary,
            "actors" => actors,
            "place" => place.map(Value::String).unwrap_or(Value::Null),
            "things" => vec![Value::String(tid.to_owned())],
            "tags" => vec![Value::String("things".into()), Value::String("remove".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(kd.label), Value::String(tid.to_owned())],
            "facts" => obj! { "kind" => t.kind }
        };

        Ok(obj! { "entry" => entry })
    }

    fn op_move(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["thing", "to_place"], &["actor"])?;
        let tid = ident_bound(c::get(args, "thing")?)?.to_owned();
        let to_place = ident_bound(c::get(args, "to_place")?)?.to_owned();
        let actor = if let Some(a) = args.get("actor") { Some(ident_bound(a)?.to_owned()) } else { None };
        let (kind, owner) = {
            let t = self.find_thing(&tid).ok_or_else(c::bad)?;
            (t.kind.clone(), t.owner.clone())
        };
        let kd = self.find_kind(&kind).ok_or_else(c::bad)?.clone();
        let t = self.find_thing_mut(&tid).ok_or_else(c::bad)?;
        t.loc_kind = "place";
        t.loc_id = to_place.clone();

        let actors = if let Some(ac) = actor {
            vec![Value::String(ac)]
        } else if let Some(ow) = owner {
            vec![Value::String(ow)]
        } else {
            Vec::<Value>::new()
        };

        let summary = format!("Moved {} ({}) to {}.", kd.label, tid, to_place);
        let entry = obj! {
            "at" => at,
            "kind" => "move_thing",
            "summary" => summary,
            "actors" => actors,
            "place" => Value::String(to_place.clone()),
            "things" => vec![Value::String(tid.clone())],
            "tags" => vec![Value::String("things".into()), Value::String("move".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(kd.label), Value::String(tid)],
            "facts" => obj! { "to_place" => to_place }
        };

        Ok(obj! { "entry" => entry })
    }

    fn op_store(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["thing", "container"], &["actor"])?;
        let tid = ident_bound(c::get(args, "thing")?)?.to_owned();
        let cid = ident_bound(c::get(args, "container")?)?.to_owned();
        let actor = if let Some(a) = args.get("actor") { Some(ident_bound(a)?.to_owned()) } else { None };
        self.check_container_fit(&cid, &tid)?;
        let place = self.effective_place(&cid);
        let (kind, owner) = {
            let t = self.find_thing(&tid).ok_or_else(c::bad)?;
            (t.kind.clone(), t.owner.clone())
        };
        let kd = self.find_kind(&kind).ok_or_else(c::bad)?.clone();
        let t = self.find_thing_mut(&tid).ok_or_else(c::bad)?;
        t.loc_kind = "container";
        t.loc_id = cid.clone();

        let actors = if let Some(ac) = actor {
            vec![Value::String(ac)]
        } else if let Some(ow) = owner {
            vec![Value::String(ow)]
        } else {
            Vec::<Value>::new()
        };

        let summary = format!("Stored {} ({}) in {}.", kd.label, tid, cid);
        let entry = obj! {
            "at" => at,
            "kind" => "store_thing",
            "summary" => summary,
            "actors" => actors,
            "place" => place.map(Value::String).unwrap_or(Value::Null),
            "things" => vec![Value::String(tid.clone()), Value::String(cid.clone())],
            "tags" => vec![Value::String("things".into()), Value::String("store".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(kd.label), Value::String(tid)],
            "facts" => obj! { "container" => cid }
        };

        Ok(obj! { "entry" => entry })
    }

    fn op_take(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["thing", "actor"], &[])?;
        let tid = ident_bound(c::get(args, "thing")?)?.to_owned();
        let actor = ident_bound(c::get(args, "actor")?)?.to_owned();
        let place = self.effective_place(&tid);
        let kind = self.find_thing(&tid).ok_or_else(c::bad)?.kind.clone();
        let kd = self.find_kind(&kind).ok_or_else(c::bad)?.clone();
        let t = self.find_thing_mut(&tid).ok_or_else(c::bad)?;
        t.loc_kind = "holder";
        t.loc_id = actor.clone();

        let summary = format!("{} took {} ({}).", actor, kd.label, tid);
        let entry = obj! {
            "at" => at,
            "kind" => "take_thing",
            "summary" => summary,
            "actors" => vec![Value::String(actor.clone())],
            "place" => place.map(Value::String).unwrap_or(Value::Null),
            "things" => vec![Value::String(tid.clone())],
            "tags" => vec![Value::String("things".into()), Value::String("take".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(kd.label), Value::String(tid)],
            "facts" => obj! { "holder" => actor }
        };

        Ok(obj! { "entry" => entry })
    }

    fn op_load(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["carrier"], &["thing", "passenger", "actor"])?;
        let cid = ident_bound(c::get(args, "carrier")?)?.to_owned();
        let car = self.find_thing(&cid).ok_or_else(c::bad)?.clone();
        let ck = self.find_kind(&car.kind).ok_or_else(c::bad)?.clone();
        let actor = if let Some(a) = args.get("actor") { Some(ident_bound(a)?.to_owned()) } else { None };
        let tid = if let Some(v) = args.get("thing") { Some(ident_bound(v)?.to_owned()) } else { None };
        let pid = if let Some(v) = args.get("passenger") { Some(ident_bound(v)?.to_owned()) } else { None };
        if tid.is_none() && pid.is_none() {
            return Err(c::bad());
        }

        let mut things_involved = vec![Value::String(cid.clone())];
        let mut actors = if let Some(ac) = actor {
            vec![ac]
        } else if let Some(ow) = car.owner {
            vec![ow]
        } else {
            Vec::<String>::new()
        };

        if let Some(t_item) = &tid {
            self.check_carrier_fit(&cid, t_item)?;
            let th = self.find_thing_mut(t_item).ok_or_else(c::bad)?;
            th.loc_kind = "carrier";
            th.loc_id = cid.clone();
            things_involved.push(Value::String(t_item.clone()));
        }
        if let Some(p_item) = &pid {
            let max_seats = ck.seats.unwrap_or(0) as usize;
            let c_mut = self.find_thing_mut(&cid).ok_or_else(c::bad)?;
            if c_mut.passengers.len() >= max_seats || c_mut.passengers.contains(p_item) {
                return Err(c::bad());
            }
            c_mut.passengers.push(p_item.clone());
            if !actors.contains(p_item) {
                actors.push(p_item.clone());
            }
        }

        let place = self.effective_place(&cid);
        let summary = format!("Loaded into carrier {}.", cid);
        let entry = obj! {
            "at" => at,
            "kind" => "load_carrier",
            "summary" => summary,
            "actors" => actors.into_iter().map(Value::String).collect::<Vec<_>>(),
            "place" => place.map(Value::String).unwrap_or(Value::Null),
            "things" => things_involved,
            "tags" => vec![Value::String("things".into()), Value::String("load".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(cid.clone())],
            "facts" => obj! { "carrier" => cid }
        };

        Ok(obj! { "entry" => entry })
    }

    fn op_unload(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["carrier"], &["thing", "passenger", "to_place", "actor"])?;
        let cid = ident_bound(c::get(args, "carrier")?)?.to_owned();
        let car = self.find_thing(&cid).ok_or_else(c::bad)?.clone();
        let actor = if let Some(a) = args.get("actor") { Some(ident_bound(a)?.to_owned()) } else { None };
        let place = if let Some(p) = args.get("to_place") {
            Some(ident_bound(p)?.to_owned())
        } else {
            self.effective_place(&cid)
        }
        .ok_or_else(c::bad)?;

        let tid = if let Some(v) = args.get("thing") { Some(ident_bound(v)?.to_owned()) } else { None };
        let pid = if let Some(v) = args.get("passenger") { Some(ident_bound(v)?.to_owned()) } else { None };
        if tid.is_none() && pid.is_none() {
            return Err(c::bad());
        }

        let mut things_involved = vec![Value::String(cid.clone())];
        let mut actors = if let Some(ac) = actor {
            vec![ac]
        } else if let Some(ow) = car.owner {
            vec![ow]
        } else {
            Vec::<String>::new()
        };

        if let Some(t_item) = &tid {
            let th = self.find_thing_mut(t_item).ok_or_else(c::bad)?;
            if th.loc_kind != "carrier" || th.loc_id != cid {
                return Err(c::bad());
            }
            th.loc_kind = "place";
            th.loc_id = place.clone();
            things_involved.push(Value::String(t_item.clone()));
        }
        if let Some(p_item) = &pid {
            let c_mut = self.find_thing_mut(&cid).ok_or_else(c::bad)?;
            let p_idx = c_mut.passengers.iter().position(|p| p == p_item).ok_or_else(c::bad)?;
            c_mut.passengers.remove(p_idx);
            if !actors.contains(p_item) {
                actors.push(p_item.clone());
            }
        }

        let summary = format!("Unloaded from carrier {}.", cid);
        let entry = obj! {
            "at" => at,
            "kind" => "unload_carrier",
            "summary" => summary,
            "actors" => actors.into_iter().map(Value::String).collect::<Vec<_>>(),
            "place" => Value::String(place),
            "things" => things_involved,
            "tags" => vec![Value::String("things".into()), Value::String("unload".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(cid.clone())],
            "facts" => obj! { "carrier" => cid }
        };

        Ok(obj! { "entry" => entry })
    }

    fn op_travel(&mut self, at: u64, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["carrier", "to"], &["actor"])?;
        let cid = ident_bound(c::get(args, "carrier")?)?.to_owned();
        let to_place = ident_bound(c::get(args, "to")?)?.to_owned();
        let actor = if let Some(a) = args.get("actor") { Some(ident_bound(a)?.to_owned()) } else { None };
        let from_place = self.effective_place(&cid);
        let (kind, owner, passengers) = {
            let car = self.find_thing(&cid).ok_or_else(c::bad)?;
            (car.kind.clone(), car.owner.clone(), car.passengers.clone())
        };
        let kd = self.find_kind(&kind).ok_or_else(c::bad)?.clone();
        let car = self.find_thing_mut(&cid).ok_or_else(c::bad)?;
        car.loc_kind = "place";
        car.loc_id = to_place.clone();

        let mut actors = if let Some(ac) = actor {
            vec![ac]
        } else if let Some(ow) = &owner {
            vec![ow.clone()]
        } else {
            Vec::<String>::new()
        };
        for p in &passengers {
            if !actors.contains(p) {
                actors.push(p.clone());
            }
        }

        let summary = format!("Carrier {} ({}) traveled to {}.", kd.label, cid, to_place);
        let entry = obj! {
            "at" => at,
            "kind" => "travel_carrier",
            "summary" => summary,
            "actors" => actors.into_iter().map(Value::String).collect::<Vec<_>>(),
            "place" => Value::String(to_place.clone()),
            "things" => vec![Value::String(cid.clone())],
            "tags" => vec![Value::String("things".into()), Value::String("travel".into())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(kd.label), Value::String(cid)],
            "facts" => obj! {
                "from_place" => from_place.clone().map(Value::String).unwrap_or(Value::Null),
                "to_place" => to_place.clone(),
            }
        };

        Ok(obj! {
            "from_place" => from_place.map(Value::String).unwrap_or(Value::Null),
            "to_place" => to_place,
            "entry" => entry,
        })
    }

    fn op_best_tool(&self, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["holder", "function"], &[])?;
        let holder = ident_bound(c::get(args, "holder")?)?;
        let fn_name = ident_bound(c::get(args, "function")?)?;

        let mut best_id: Option<String> = None;
        let mut best_kind: Option<String> = None;
        let mut best_power: u64 = 0;
        let mut best_cond: u64 = 0;

        for t in &self.things {
            if t.loc_kind == "holder" && t.loc_id == holder {
                if let Some(kd) = self.find_kind(&t.kind) {
                    for f in &kd.functions {
                        if f.function == fn_name {
                            let pwr = f.power;
                            let cnd = t.condition;
                            let replace = match &best_id {
                                None => true,
                                Some(b_id) => {
                                    pwr > best_power
                                        || (pwr == best_power && cnd > best_cond)
                                        || (pwr == best_power && cnd == best_cond && t.id < *b_id)
                                }
                            };
                            if replace {
                                best_id = Some(t.id.clone());
                                best_kind = Some(t.kind.clone());
                                best_power = pwr;
                                best_cond = cnd;
                            }
                        }
                    }
                }
            }
        }

        Ok(obj! {
            "thing" => best_id.map(Value::String).unwrap_or(Value::Null),
            "kind" => best_kind.map(Value::String).unwrap_or(Value::Null),
            "power" => best_power,
        })
    }

    fn op_whats_here(&self, args: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(args, &["place"], &[])?;
        let place = ident_bound(c::get(args, "place")?)?;

        let mut matched: Vec<&ThingInstance> =
            self.things.iter().filter(|t| self.effective_place(&t.id).as_deref() == Some(place)).collect();
        matched.sort_by(|a, b| a.id.cmp(&b.id));

        let things_json: Vec<Value> = matched
            .into_iter()
            .map(|t| {
                let loc = obj! { t.loc_kind => t.loc_id.clone() };
                let ps: Vec<Value> = t.passengers.iter().map(|p| Value::String(p.clone())).collect();
                obj! {
                    "id" => t.id.clone(),
                    "kind" => t.kind.clone(),
                    "location" => loc,
                    "condition" => t.condition,
                    "owner" => t.owner.as_ref().map(|o| Value::String(o.clone())).unwrap_or(Value::Null),
                    "quantity" => t.quantity,
                    "passengers" => ps,
                }
            })
            .collect();

        Ok(obj! {
            "place" => place,
            "total" => things_json.len() as u64,
            "things" => things_json,
        })
    }

    /// Runs `{ops, things?, limits?}`. `limits` (host-set [limits](LIMITS)) apply first and
    /// are echoed in the output.
    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
        let stripped = crate::character::limits::take(req, &mut self.limits)?;
        let out = self.run_workflow(stripped.as_ref().unwrap_or(req))?;
        Ok(crate::character::limits::echo(out, &self.limits, stripped.is_some()))
    }

    /// A things world with host limits applied up front.
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut t = Self::new();
        t.limits.apply(v)?;
        Ok(t)
    }

    fn run_workflow(&mut self, req: &Value) -> Result<Value> {
        crate::knowledge::codex::keys(req, &["ops"], &["things"])?;
        if let Some(th) = req.get("things").filter(|x| **x != Value::Null) {
            *self = Self::from_value_within(th, &self.limits)?;
        }
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        let mut results = Vec::new();
        for o in ops {
            crate::knowledge::codex::keys(o, &["op"], &["args", "at"])?;
            let op_name = text_bound(c::get(o, "op")?, 32)?;
            let empty_args = obj! {};
            let args = o.get("args").unwrap_or(&empty_args);
            let at = if let Some(v) = o.get("at") { uint_bound(v, 0, MAX_SAFE)? } else { 0 };

            let backup = self.things.clone();
            let res = match op_name {
                "create" => self.op_create(at, args),
                "destroy" => self.op_destroy(at, args),
                "remove" => self.op_remove(at, args),
                "move" => self.op_move(at, args),
                "store" => self.op_store(at, args),
                "take" => self.op_take(at, args),
                "load" => self.op_load(at, args),
                "unload" => self.op_unload(at, args),
                "travel" => self.op_travel(at, args),
                "best_tool" => self.op_best_tool(args),
                "whats_here" => self.op_whats_here(args),
                _ => Err(c::bad()),
            };

            match res {
                Ok(val) => results.push(obj! { "ok" => true, "result" => val }),
                Err(_) => {
                    self.things = backup;
                    results.push(obj! { "ok" => false });
                }
            }
        }

        Ok(obj! {
            "results" => results,
            "things" => self.to_value(),
        })
    }
}

/// The `things` local kind: `{ops, things?, limits?}` → `{limits?, results, things}`.
pub fn workflow(a: &Value) -> Result<Value> {
    Things::new().workflow(a)
}
