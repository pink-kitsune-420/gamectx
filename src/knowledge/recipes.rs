//! World recipes: crafting, cooking, building, repairing, dismantling, and feasibility checking.
//!
//! Recipes consume inputs (by kind or tag), require tools and stations, check character skills,
//! produce outputs or modify world structures/places/entrances, and emit timeline-ready entries.
//! All executions are atomic (reverting on failure).
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};

pub const MAX_RECIPES: usize = 256;
pub const MAX_OPS: usize = 64;
const MAX_SAFE: u64 = 9_007_199_254_740_991;
/// Host-set limits (default is the earlier fixed bound): most `recipes`.
pub const LIMITS: &[crate::character::limits::Limit] =
    &[crate::character::limits::Limit { name: "recipes", default: MAX_RECIPES, ceiling: 65_536 }];
crate::character::limits::module_limits!(RecipesLimits, LIMITS);

pub const CATEGORIES: [&str; 5] = ["crafting", "cooking", "building", "repairing", "dismantling"];
pub const DIRECTIONS: [&str; 10] =
    ["north", "northeast", "east", "southeast", "south", "southwest", "west", "northwest", "up", "down"];

fn set(e: &mut Value, key: &str, value: Value) {
    if let Value::Object(fields) = e {
        if let Some((_, v)) = fields.iter_mut().find(|(k, _)| k == key) {
            *v = value;
        } else {
            fields.push((key.to_string(), value));
        }
    }
}

fn get_field_mut<'a>(v: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    if let Value::Object(fields) = v {
        fields.iter_mut().find(|(k, _)| k == key).map(|(_, val)| val)
    } else {
        None
    }
}

fn as_array_mut(v: &mut Value) -> Option<&mut Vec<Value>> {
    if let Value::Array(arr) = v {
        Some(arr)
    } else {
        None
    }
}

fn as_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => s.parse().ok(),
        _ => None,
    }
}

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

#[derive(Debug, Clone)]
pub struct InputReq {
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub quantity: u64,
}

#[derive(Debug, Clone)]
pub struct ToolReq {
    pub function: String,
    pub power: u64,
}

#[derive(Debug, Clone)]
pub struct SkillReq {
    pub id: String,
    pub level: u64,
}

#[derive(Debug, Clone)]
pub struct OutputDef {
    pub kind: String,
    pub quantity: u64,
    pub condition: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct BuildingSpot {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct BuildingLink {
    pub place: String,
    pub distance: u64,
    pub direction: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PatchPlace {
    pub kind: Option<String>,
    pub add_tags: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct BuildingDef {
    pub structure_kind: Option<String>,
    pub add_spots: Option<Vec<BuildingSpot>>,
    pub open_entrance: Option<String>,
    pub add_link: Option<Vec<BuildingLink>>,
    pub patch_place: Option<PatchPlace>,
}

#[derive(Debug, Clone)]
pub struct RecipeDef {
    pub id: String,
    pub label: String,
    pub category: String,
    pub inputs: Vec<InputReq>,
    pub tools: Vec<ToolReq>,
    pub station: Option<String>,
    pub skill: Option<SkillReq>,
    pub outputs: Vec<OutputDef>,
    pub time: u64,
    pub building: Option<BuildingDef>,
}

impl RecipeDef {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(v, &["id", "label", "category", "inputs", "tools", "outputs", "time"], &["station", "skill", "building"])?;

        let id = ident_bound(c::get(v, "id")?)?.to_owned();
        let label = text_bound(c::get(v, "label")?, 48)?.to_owned();
        let category = ident_bound(c::get(v, "category")?)?.to_owned();
        if !CATEGORIES.contains(&category.as_str()) {
            return Err(c::bad());
        }

        let inps = c::get(v, "inputs")?.as_array().ok_or_else(c::bad)?;
        if inps.len() > 16 {
            return Err(c::bad());
        }
        let mut inputs = Vec::new();
        for inp in inps {
            keys(inp, &["quantity"], &["kind", "tag"])?;
            let has_kind = inp.get("kind").is_some();
            let has_tag = inp.get("tag").is_some();
            if has_kind == has_tag {
                return Err(c::bad());
            }
            let kind = if has_kind { Some(ident_bound(c::get(inp, "kind")?)?.to_owned()) } else { None };
            let tag = if has_tag { Some(ident_bound(c::get(inp, "tag")?)?.to_owned()) } else { None };
            let quantity = uint_bound(c::get(inp, "quantity")?, 1, 1_000_000_000)?;
            inputs.push(InputReq { kind, tag, quantity });
        }

        let tls = c::get(v, "tools")?.as_array().ok_or_else(c::bad)?;
        if tls.len() > 8 {
            return Err(c::bad());
        }
        let mut tools = Vec::new();
        let mut t_funcs = Vec::new();
        for tl in tls {
            keys(tl, &["function", "power"], &[])?;
            let function = ident_bound(c::get(tl, "function")?)?.to_owned();
            let power = uint_bound(c::get(tl, "power")?, 1, 1_000_000_000)?;
            if t_funcs.contains(&function) {
                return Err(c::bad());
            }
            t_funcs.push(function.clone());
            tools.push(ToolReq { function, power });
        }

        let station = if let Some(st) = v.get("station").filter(|x| **x != Value::Null) {
            Some(ident_bound(st)?.to_owned())
        } else {
            None
        };

        let skill = if let Some(sk) = v.get("skill").filter(|x| **x != Value::Null) {
            keys(sk, &["id", "level"], &[])?;
            let sk_id = ident_bound(c::get(sk, "id")?)?.to_owned();
            let sk_lvl = uint_bound(c::get(sk, "level")?, 0, 100)?;
            Some(SkillReq { id: sk_id, level: sk_lvl })
        } else {
            None
        };

        let outs = c::get(v, "outputs")?.as_array().ok_or_else(c::bad)?;
        if outs.len() > 16 {
            return Err(c::bad());
        }
        let mut outputs = Vec::new();
        for out in outs {
            keys(out, &["kind", "quantity"], &["condition"])?;
            let kind = ident_bound(c::get(out, "kind")?)?.to_owned();
            let quantity = uint_bound(c::get(out, "quantity")?, 1, 1_000_000_000)?;
            let condition = if let Some(cond) = out.get("condition").filter(|x| **x != Value::Null) {
                Some(uint_bound(cond, 0, 100)?)
            } else {
                None
            };
            outputs.push(OutputDef { kind, quantity, condition });
        }

        let time = uint_bound(c::get(v, "time")?, 1, 1_000_000_000)?;

        let building = if let Some(bld) = v.get("building").filter(|x| **x != Value::Null) {
            if category != "building" {
                return Err(c::bad());
            }
            keys(bld, &[], &["structure_kind", "add_spots", "open_entrance", "add_link", "patch_place"])?;
            let structure_kind =
                if let Some(sk) = bld.get("structure_kind") { Some(ident_bound(sk)?.to_owned()) } else { None };
            let add_spots = if let Some(sp) = bld.get("add_spots") {
                let arr = sp.as_array().ok_or_else(c::bad)?;
                if arr.len() > 8 {
                    return Err(c::bad());
                }
                let mut spots = Vec::new();
                for s in arr {
                    keys(s, &["id", "label", "description"], &[])?;
                    spots.push(BuildingSpot {
                        id: ident_bound(c::get(s, "id")?)?.to_owned(),
                        label: text_bound(c::get(s, "label")?, 48)?.to_owned(),
                        description: text_bound(c::get(s, "description")?, 240)?.to_owned(),
                    });
                }
                Some(spots)
            } else {
                None
            };
            let open_entrance =
                if let Some(oe) = bld.get("open_entrance") { Some(ident_bound(oe)?.to_owned()) } else { None };
            let add_link = if let Some(al) = bld.get("add_link") {
                let arr = al.as_array().ok_or_else(c::bad)?;
                if arr.len() > 4 {
                    return Err(c::bad());
                }
                let mut links = Vec::new();
                for l in arr {
                    keys(l, &["place", "distance"], &["direction"])?;
                    let place = ident_bound(c::get(l, "place")?)?.to_owned();
                    let distance = uint_bound(c::get(l, "distance")?, 0, 1_000_000_000)?;
                    let direction = if let Some(dir) = l.get("direction") {
                        let d = ident_bound(dir)?;
                        if !DIRECTIONS.contains(&d) {
                            return Err(c::bad());
                        }
                        Some(d.to_owned())
                    } else {
                        None
                    };
                    links.push(BuildingLink { place, distance, direction });
                }
                Some(links)
            } else {
                None
            };
            let patch_place = if let Some(pp) = bld.get("patch_place") {
                keys(pp, &[], &["kind", "add_tags"])?;
                let pk = if let Some(k) = pp.get("kind") { Some(ident_bound(k)?.to_owned()) } else { None };
                let pat = if let Some(at) = pp.get("add_tags") {
                    let arr = at.as_array().ok_or_else(c::bad)?;
                    if arr.len() > 8 {
                        return Err(c::bad());
                    }
                    let mut tags = Vec::new();
                    for t in arr {
                        tags.push(ident_bound(t)?.to_owned());
                    }
                    Some(tags)
                } else {
                    None
                };
                Some(PatchPlace { kind: pk, add_tags: pat })
            } else {
                None
            };
            Some(BuildingDef { structure_kind, add_spots, open_entrance, add_link, patch_place })
        } else {
            None
        };

        Ok(Self { id, label, category, inputs, tools, station, skill, outputs, time, building })
    }

    pub fn to_value(&self) -> Value {
        let mut r = obj! {
            "id" => self.id.clone(),
            "label" => self.label.clone(),
            "category" => self.category.clone(),
            "inputs" => self.inputs.iter().map(|i| {
                let mut v = obj! {};
                if let Some(ref k) = i.kind {
                    set(&mut v, "kind", Value::String(k.clone()));
                }
                if let Some(ref t) = i.tag {
                    set(&mut v, "tag", Value::String(t.clone()));
                }
                set(&mut v, "quantity", Value::from(i.quantity));
                v
            }).collect::<Vec<_>>(),
            "tools" => self.tools.iter().map(|t| {
                obj! {
                    "function" => t.function.clone(),
                    "power" => t.power,
                }
            }).collect::<Vec<_>>(),
        };
        if let Some(ref st) = self.station {
            set(&mut r, "station", Value::String(st.clone()));
        }
        if let Some(ref sk) = self.skill {
            set(
                &mut r,
                "skill",
                obj! {
                    "id" => sk.id.clone(),
                    "level" => sk.level,
                },
            );
        }
        let outs: Vec<Value> = self
            .outputs
            .iter()
            .map(|o| {
                let mut v = obj! {
                    "kind" => o.kind.clone(),
                    "quantity" => o.quantity,
                };
                if let Some(cond) = o.condition {
                    set(&mut v, "condition", Value::from(cond));
                }
                v
            })
            .collect();
        set(&mut r, "outputs", Value::Array(outs));
        set(&mut r, "time", Value::from(self.time));
        if let Some(ref b) = self.building {
            let mut bld = obj! {};
            if let Some(ref sk) = b.structure_kind {
                set(&mut bld, "structure_kind", Value::String(sk.clone()));
            }
            if let Some(ref sp) = b.add_spots {
                let arr: Vec<Value> = sp
                    .iter()
                    .map(|s| {
                        obj! {
                            "id" => s.id.clone(),
                            "label" => s.label.clone(),
                            "description" => s.description.clone(),
                        }
                    })
                    .collect();
                set(&mut bld, "add_spots", Value::Array(arr));
            }
            if let Some(ref oe) = b.open_entrance {
                set(&mut bld, "open_entrance", Value::String(oe.clone()));
            }
            if let Some(ref al) = b.add_link {
                let arr: Vec<Value> = al
                    .iter()
                    .map(|l| {
                        let mut lv = obj! {
                            "place" => l.place.clone(),
                            "distance" => l.distance,
                        };
                        if let Some(ref dir) = l.direction {
                            set(&mut lv, "direction", Value::String(dir.clone()));
                        }
                        lv
                    })
                    .collect();
                set(&mut bld, "add_link", Value::Array(arr));
            }
            if let Some(ref pp) = b.patch_place {
                let mut pv = obj! {};
                if let Some(ref k) = pp.kind {
                    set(&mut pv, "kind", Value::String(k.clone()));
                }
                if let Some(ref at) = pp.add_tags {
                    let arr: Vec<Value> = at.iter().map(|t| Value::String(t.clone())).collect();
                    set(&mut pv, "add_tags", Value::Array(arr));
                }
                set(&mut bld, "patch_place", pv);
            }
            set(&mut r, "building", bld);
        }
        r
    }
}

#[derive(Debug, Clone, Default)]
pub struct Recipes {
    pub recipes: Vec<RecipeDef>,
    limits: RecipesLimits,
}

impl Recipes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_value(v: &Value) -> Result<Self> {
        Self::from_value_within(v, &RecipesLimits::default())
    }

    /// [`from_value`](Self::from_value) with host-set [limits](LIMITS); the result keeps them.
    pub fn from_value_within(v: &Value, l: &crate::character::limits::Limits) -> Result<Self> {
        let arr = v.as_array().ok_or_else(c::bad)?;
        if arr.len() > l.get("recipes") {
            return Err(c::bad());
        }
        let mut recipes = Vec::new();
        for r_val in arr {
            let r = RecipeDef::parse(r_val)?;
            if recipes.iter().any(|old: &RecipeDef| old.id == r.id) {
                return Err(c::bad());
            }
            recipes.push(r);
        }
        Ok(Self { recipes, limits: RecipesLimits(l.clone()) })
    }

    pub fn to_value(&self) -> Value {
        Value::Array(self.recipes.iter().map(|r| r.to_value()).collect())
    }

    pub fn find(&self, id: &str) -> Option<&RecipeDef> {
        self.recipes.iter().find(|r| r.id == id)
    }

    pub fn check(
        &self,
        recipe_id: &str,
        actor: &str,
        place: &str,
        things_state: &Value,
        bio: Option<&Value>,
        target_thing: Option<&str>,
    ) -> Result<Value> {
        let r = self.find(recipe_id).ok_or_else(c::bad)?;
        let mut missing = Vec::new();

        // 1. Skill check
        if let Some(ref req_sk) = r.skill {
            let mut actual_lvl = 0u64;
            if let Some(bio_val) = bio {
                if let Some(sks) = bio_val.get("skills").and_then(|v| v.as_array()) {
                    for sk in sks {
                        if sk.get("id").and_then(|v| v.as_str()) == Some(&req_sk.id) {
                            actual_lvl = sk.get("level").and_then(as_u64).unwrap_or(0);
                            break;
                        }
                    }
                }
            }
            if actual_lvl < req_sk.level {
                missing.push(obj! {
                    "reason" => "missing_skill",
                    "details" => obj! {
                        "skill" => req_sk.id.clone(),
                        "required" => req_sk.level,
                        "actual" => actual_lvl,
                    }
                });
            }
        }

        let empty_vec = Vec::new();
        let things_list = things_state.get("things").and_then(|v| v.as_array()).unwrap_or(&empty_vec);
        let kinds_list = things_state.get("kinds").and_then(|v| v.as_array()).unwrap_or(&empty_vec);

        // 2. Tools check: actor must hold tool with function of power >= req
        for tl in &r.tools {
            let mut has_tool = false;
            for t in things_list {
                let loc = t.get("location");
                if loc.and_then(|l| l.get("holder")).and_then(|h| h.as_str()) == Some(actor) {
                    let t_kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(kd) = kinds_list.iter().find(|k| k.get("id").and_then(|v| v.as_str()) == Some(t_kind)) {
                        if let Some(funcs) = kd.get("functions").and_then(|v| v.as_array()) {
                            for f in funcs {
                                if f.get("function").and_then(|v| v.as_str()) == Some(&tl.function)
                                    && f.get("power").and_then(as_u64).unwrap_or(0) >= tl.power
                                {
                                    has_tool = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if has_tool {
                    break;
                }
            }
            if !has_tool {
                missing.push(obj! {
                    "reason" => "missing_tool",
                    "details" => obj! {
                        "function" => tl.function.clone(),
                        "required_power" => tl.power,
                    }
                });
            }
        }

        // 3. Station check
        if let Some(ref req_st) = r.station {
            let mut has_station = false;
            for t in things_list {
                let loc = t.get("location");
                if loc.and_then(|l| l.get("place")).and_then(|p| p.as_str()) == Some(place) {
                    let t_kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(kd) = kinds_list.iter().find(|k| k.get("id").and_then(|v| v.as_str()) == Some(t_kind)) {
                        if let Some(funcs) = kd.get("functions").and_then(|v| v.as_array()) {
                            for f in funcs {
                                if f.get("function").and_then(|v| v.as_str()) == Some(req_st) {
                                    has_station = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if has_station {
                    break;
                }
            }
            if !has_station {
                missing.push(obj! {
                    "reason" => "missing_station",
                    "details" => obj! {
                        "station" => req_st.clone(),
                    }
                });
            }
        }

        // 4. Inputs check
        let mut available_things = Vec::new();
        for t in things_list {
            let loc = t.get("location");
            let is_held = loc.and_then(|l| l.get("holder")).and_then(|h| h.as_str()) == Some(actor);
            let is_in_place = loc.and_then(|l| l.get("place")).and_then(|p| p.as_str()) == Some(place);
            if is_held || is_in_place {
                available_things.push(t);
            }
        }

        for inp in &r.inputs {
            let req_qty = inp.quantity;
            let mut matched_qty = 0u64;
            if let Some(ref k_target) = inp.kind {
                for t in &available_things {
                    if t.get("id").and_then(|v| v.as_str()) != target_thing
                        && t.get("kind").and_then(|v| v.as_str()) == Some(k_target)
                    {
                        matched_qty += t.get("quantity").and_then(as_u64).unwrap_or(1);
                    }
                }
                if matched_qty < req_qty {
                    missing.push(obj! {
                        "reason" => "missing_input",
                        "details" => obj! {
                            "kind" => k_target.clone(),
                            "required" => req_qty,
                            "available" => matched_qty,
                        }
                    });
                }
            } else if let Some(ref t_target) = inp.tag {
                for t in &available_things {
                    if t.get("id").and_then(|v| v.as_str()) != target_thing {
                        let t_kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                        if let Some(kd) =
                            kinds_list.iter().find(|k| k.get("id").and_then(|v| v.as_str()) == Some(t_kind))
                        {
                            if let Some(tags) = kd.get("tags").and_then(|v| v.as_array()) {
                                if tags.iter().any(|tg| tg.as_str() == Some(t_target)) {
                                    matched_qty += t.get("quantity").and_then(as_u64).unwrap_or(1);
                                }
                            }
                        }
                    }
                }
                if matched_qty < req_qty {
                    missing.push(obj! {
                        "reason" => "missing_input",
                        "details" => obj! {
                            "tag" => t_target.clone(),
                            "required" => req_qty,
                            "available" => matched_qty,
                        }
                    });
                }
            }
        }

        // 5. Target item check for repair or dismantle
        if r.category == "repairing" || r.category == "dismantling" {
            if let Some(tgt_id) = target_thing {
                let found_target = things_list.iter().find(|t| t.get("id").and_then(|v| v.as_str()) == Some(tgt_id));
                if let Some(target) = found_target {
                    let loc = target.get("location");
                    let is_held = loc.and_then(|l| l.get("holder")).and_then(|h| h.as_str()) == Some(actor);
                    let is_in_place = loc.and_then(|l| l.get("place")).and_then(|p| p.as_str()) == Some(place);
                    if !is_held && !is_in_place {
                        missing.push(obj! {
                            "reason" => "target_not_accessible",
                            "details" => obj! {
                                "thing" => tgt_id.to_string(),
                            }
                        });
                    } else if r.category == "repairing" {
                        let cond = target.get("condition").and_then(as_u64).unwrap_or(100);
                        if cond >= 100 {
                            missing.push(obj! {
                                "reason" => "target_already_repaired",
                                "details" => obj! {
                                    "condition" => cond,
                                }
                            });
                        }
                    }
                } else {
                    missing.push(obj! {
                        "reason" => "target_not_found",
                        "details" => obj! {
                            "thing" => tgt_id.to_string(),
                        }
                    });
                }
            } else {
                missing.push(obj! {
                    "reason" => "missing_target",
                    "details" => obj! {
                        "category" => r.category.clone(),
                    }
                });
            }
        }

        let feasible = missing.is_empty();
        Ok(obj! {
            "feasible" => feasible,
            "missing" => missing,
            "time" => r.time,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        at: u64,
        recipe_id: &str,
        actor: &str,
        place: &str,
        things_state: &mut Value,
        bio: Option<&Value>,
        codex: Option<&mut Value>,
        target_thing: Option<&str>,
        new_ids: Option<&[String]>,
    ) -> Result<Value> {
        let chk = self.check(recipe_id, actor, place, things_state, bio, target_thing)?;
        if !chk.get("feasible").and_then(|v| v.as_bool()).unwrap_or(false) {
            let miss = chk.get("missing").cloned().unwrap_or_else(|| Value::Array(Vec::new()));
            return Ok(obj! {
                "ok" => false,
                "missing" => miss,
            });
        }

        let r = self.find(recipe_id).ok_or_else(c::bad)?;

        // Collect kinds tags for matching
        let empty_kinds = Vec::new();
        let kinds_tags: Vec<(String, Vec<String>)> = things_state
            .get("kinds")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_kinds)
            .iter()
            .filter_map(|kd| {
                let id = kd.get("id").and_then(|v| v.as_str())?.to_string();
                let tags = kd
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();
                Some((id, tags))
            })
            .collect();

        // 1. Consume inputs from things_list
        let things_list = if let Some(th_val) = get_field_mut(things_state, "things") {
            as_array_mut(th_val).ok_or_else(c::bad)?
        } else {
            return Err(c::bad());
        };

        for inp in &r.inputs {
            let mut rem_needed = inp.quantity;
            let mut i = 0;
            while i < things_list.len() && rem_needed > 0 {
                let t = &things_list[i];
                let is_target = t.get("id").and_then(|v| v.as_str()) == target_thing;
                let loc = t.get("location");
                let is_held = loc.and_then(|l| l.get("holder")).and_then(|h| h.as_str()) == Some(actor);
                let is_in_place = loc.and_then(|l| l.get("place")).and_then(|p| p.as_str()) == Some(place);
                if !is_target && (is_held || is_in_place) {
                    let t_kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    let mut is_match = false;
                    if let Some(ref k) = inp.kind {
                        if t_kind == k {
                            is_match = true;
                        }
                    } else if let Some(ref tg) = inp.tag {
                        if let Some((_, tags)) = kinds_tags.iter().find(|(k_id, _)| k_id == t_kind) {
                            if tags.contains(tg) {
                                is_match = true;
                            }
                        }
                    }
                    if is_match {
                        let t_qty = t.get("quantity").and_then(as_u64).unwrap_or(1);
                        if t_qty <= rem_needed {
                            rem_needed -= t_qty;
                            things_list.remove(i);
                            continue;
                        } else {
                            let new_qty = t_qty - rem_needed;
                            set(&mut things_list[i], "quantity", Value::from(new_qty));
                            rem_needed = 0;
                        }
                    }
                }
                i += 1;
            }
        }

        let mut created_things = Vec::new();

        // 2. Category-specific effects
        if r.category == "repairing" {
            if let Some(tgt_id) = target_thing {
                if let Some(t) = things_list.iter_mut().find(|t| t.get("id").and_then(|v| v.as_str()) == Some(tgt_id)) {
                    set(t, "condition", Value::from(100u64));
                }
            }
        } else if r.category == "dismantling" {
            if let Some(tgt_id) = target_thing {
                if let Some(pos) = things_list.iter().position(|t| t.get("id").and_then(|v| v.as_str()) == Some(tgt_id))
                {
                    things_list.remove(pos);
                }
            }
            for (id_idx, out) in r.outputs.iter().enumerate() {
                let out_id = if let Some(ids) = new_ids {
                    if id_idx < ids.len() {
                        ids[id_idx].clone()
                    } else {
                        format!("{}_{}_{}", out.kind, at, id_idx)
                    }
                } else {
                    format!("{}_{}_{}", out.kind, at, id_idx)
                };
                let cond = out.condition.unwrap_or(100);
                let new_t = obj! {
                    "id" => out_id,
                    "kind" => out.kind.clone(),
                    "location" => obj! { "place" => place },
                    "condition" => cond,
                    "owner" => actor,
                    "quantity" => out.quantity,
                    "passengers" => Vec::<Value>::new(),
                };
                things_list.push(new_t.clone());
                created_things.push(new_t);
            }
        } else if r.category == "building" {
            if let Some(ref bld) = r.building {
                if let Some(ref sk) = bld.structure_kind {
                    let out_id = if let Some(ids) = new_ids {
                        if !ids.is_empty() {
                            ids[0].clone()
                        } else {
                            format!("{}_{}", sk, place)
                        }
                    } else {
                        format!("{}_{}", sk, place)
                    };
                    let new_t = obj! {
                        "id" => out_id,
                        "kind" => sk.clone(),
                        "location" => obj! { "place" => place },
                        "condition" => 100u64,
                        "owner" => actor,
                        "quantity" => 1u64,
                        "passengers" => Vec::<Value>::new(),
                    };
                    things_list.push(new_t.clone());
                    created_things.push(new_t);
                }

                if let Some(codex_val) = codex {
                    if let Some(places_arr) = get_field_mut(codex_val, "places").and_then(as_array_mut) {
                        for p in places_arr {
                            if p.get("id").and_then(|v| v.as_str()) == Some(place) {
                                if let Some(ref spots) = bld.add_spots {
                                    let cur_spots = if let Some(sp) = get_field_mut(p, "spots").and_then(as_array_mut) {
                                        sp
                                    } else {
                                        set(p, "spots", Value::Array(Vec::new()));
                                        get_field_mut(p, "spots").and_then(as_array_mut).unwrap()
                                    };
                                    for s in spots {
                                        cur_spots.push(obj! {
                                            "id" => s.id.clone(),
                                            "label" => s.label.clone(),
                                            "description" => s.description.clone(),
                                        });
                                    }
                                }
                                if let Some(ref links) = bld.add_link {
                                    let cur_links = if let Some(lk) = get_field_mut(p, "links").and_then(as_array_mut) {
                                        lk
                                    } else {
                                        set(p, "links", Value::Array(Vec::new()));
                                        get_field_mut(p, "links").and_then(as_array_mut).unwrap()
                                    };
                                    for l in links {
                                        let mut lv = obj! {
                                            "place" => l.place.clone(),
                                            "distance" => l.distance,
                                        };
                                        if let Some(ref dir) = l.direction {
                                            set(&mut lv, "direction", Value::String(dir.clone()));
                                        }
                                        cur_links.push(lv);
                                    }
                                }
                                if let Some(ref pp) = bld.patch_place {
                                    if let Some(ref k) = pp.kind {
                                        set(p, "kind", Value::String(k.clone()));
                                    }
                                    if let Some(ref add_tg) = pp.add_tags {
                                        let cur_tags = if let Some(tg) = get_field_mut(p, "tags").and_then(as_array_mut)
                                        {
                                            tg
                                        } else {
                                            set(p, "tags", Value::Array(Vec::new()));
                                            get_field_mut(p, "tags").and_then(as_array_mut).unwrap()
                                        };
                                        for at in add_tg {
                                            if !cur_tags.iter().any(|existing| existing.as_str() == Some(at)) {
                                                cur_tags.push(Value::String(at.clone()));
                                            }
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }

                    if let Some(ref oe) = bld.open_entrance {
                        if let Some(ents_arr) = get_field_mut(codex_val, "entrances").and_then(as_array_mut) {
                            for e in ents_arr {
                                if e.get("id").and_then(|v| v.as_str()) == Some(oe) {
                                    set(e, "status", Value::str("open"));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // crafting or cooking
            for (id_idx, out) in r.outputs.iter().enumerate() {
                let out_id = if let Some(ids) = new_ids {
                    if id_idx < ids.len() {
                        ids[id_idx].clone()
                    } else {
                        format!("{}_{}_{}", out.kind, at, id_idx)
                    }
                } else {
                    format!("{}_{}_{}", out.kind, at, id_idx)
                };
                let cond = out.condition.unwrap_or(100);
                let new_t = obj! {
                    "id" => out_id,
                    "kind" => out.kind.clone(),
                    "location" => obj! { "holder" => actor },
                    "condition" => cond,
                    "owner" => actor,
                    "quantity" => out.quantity,
                    "passengers" => Vec::<Value>::new(),
                };
                things_list.push(new_t.clone());
                created_things.push(new_t);
            }
        }

        let mut things_involved: Vec<Value> = created_things
            .iter()
            .filter_map(|t| t.get("id").and_then(|v| v.as_str()).map(|s| Value::String(s.to_string())))
            .collect();
        if let Some(tgt_id) = target_thing {
            things_involved.push(Value::String(tgt_id.to_string()));
        }

        let summary = format!("{} completed {} ({}).", actor, r.label, r.category);
        let entry = obj! {
            "at" => at,
            "kind" => format!("{}_recipe", r.category),
            "summary" => summary,
            "actors" => vec![Value::String(actor.to_string())],
            "place" => place,
            "things" => things_involved,
            "tags" => vec![Value::str("recipes"), Value::String(r.category.clone())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(r.label.clone()), Value::String(r.id.clone())],
            "facts" => obj! {
                "recipe" => r.id.clone(),
                "time" => r.time,
            },
        };

        Ok(obj! {
            "ok" => true,
            "recipe" => recipe_id,
            "created" => created_things,
            "entry" => entry,
        })
    }

    /// Runs `{ops, recipes?, things?, codex?, bio?, limits?}`. `limits` (host-set
    /// [limits](LIMITS)) apply first and are echoed in the output.
    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
        let stripped = crate::character::limits::take(req, &mut self.limits)?;
        let out = self.run_workflow(stripped.as_ref().unwrap_or(req))?;
        Ok(crate::character::limits::echo(out, &self.limits, stripped.is_some()))
    }

    /// A recipe book with host limits applied up front.
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut r = Self::new();
        r.limits.apply(v)?;
        Ok(r)
    }

    fn run_workflow(&mut self, req: &Value) -> Result<Value> {
        keys(req, &["ops"], &["recipes", "things", "codex", "bio"])?;
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        if let Some(r_val) = req.get("recipes").filter(|x| **x != Value::Null) {
            *self = Self::from_value_within(r_val, &self.limits)?;
        }

        let mut things_state = if let Some(th) = req.get("things").filter(|x| **x != Value::Null) {
            th.clone()
        } else {
            obj! {
                "version" => 1u64,
                "materials" => Vec::<Value>::new(),
                "kinds" => Vec::<Value>::new(),
                "things" => Vec::<Value>::new(),
            }
        };

        let mut codex_state = req.get("codex").cloned();
        let bio_state = req.get("bio");

        let mut results = Vec::new();
        for o in ops {
            keys(o, &["op"], &["args", "at"])?;
            let op = ident_bound(c::get(o, "op")?)?;
            let empty_args = obj! {};
            let args = o.get("args").unwrap_or(&empty_args);
            let at = if let Some(at_val) = o.get("at").filter(|x| **x != Value::Null) {
                uint_bound(at_val, 0, MAX_SAFE)?
            } else {
                0
            };

            let backup_things = things_state.clone();
            let backup_codex = codex_state.clone();

            match op {
                "check" => {
                    keys(args, &["recipe", "actor", "place"], &["target_thing"])?;
                    let recipe = ident_bound(c::get(args, "recipe")?)?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    let target_thing = if let Some(tt) = args.get("target_thing").filter(|x| **x != Value::Null) {
                        Some(ident_bound(tt)?)
                    } else {
                        None
                    };
                    match self.check(recipe, actor, place, &things_state, bio_state, target_thing) {
                        Ok(res) => results.push(obj! { "ok" => true, "result" => res }),
                        Err(_) => {
                            things_state = backup_things;
                            codex_state = backup_codex;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                "execute" => {
                    keys(args, &["recipe", "actor", "place"], &["target_thing", "new_ids"])?;
                    let recipe = ident_bound(c::get(args, "recipe")?)?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    let target_thing = if let Some(tt) = args.get("target_thing").filter(|x| **x != Value::Null) {
                        Some(ident_bound(tt)?)
                    } else {
                        None
                    };
                    let new_ids = if let Some(nids) = args.get("new_ids").filter(|x| **x != Value::Null) {
                        let arr = nids.as_array().ok_or_else(c::bad)?;
                        let mut v = Vec::new();
                        for id in arr {
                            v.push(ident_bound(id)?.to_owned());
                        }
                        Some(v)
                    } else {
                        None
                    };
                    let exec_res = self.execute(
                        at,
                        recipe,
                        actor,
                        place,
                        &mut things_state,
                        bio_state,
                        codex_state.as_mut(),
                        target_thing,
                        new_ids.as_deref(),
                    );
                    match exec_res {
                        Ok(res) => {
                            if res.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                results.push(obj! { "ok" => true, "result" => res });
                            } else {
                                things_state = backup_things;
                                codex_state = backup_codex;
                                let miss = res.get("missing").cloned().unwrap_or_else(|| Value::Array(Vec::new()));
                                results.push(obj! { "ok" => false, "missing" => miss });
                            }
                        }
                        Err(_) => {
                            things_state = backup_things;
                            codex_state = backup_codex;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                _ => return Err(c::bad()),
            }
        }

        let mut ret = obj! {
            "results" => results,
            "recipes" => self.to_value(),
            "things" => things_state,
        };
        set(&mut ret, "codex", codex_state.unwrap_or(Value::Null));
        Ok(ret)
    }
}

/// The `recipes` local kind: `{ops, recipes?, things?, codex?, bio?, limits?}` →
/// `{limits?, results, ...}`.
pub fn workflow(a: &Value) -> Result<Value> {
    Recipes::new().workflow(a)
}
