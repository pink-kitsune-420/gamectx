//! World species: animals, plants, people, hybrids, temperaments, produce, and animal interactions.
//!
//! Species declare traits, diets, habitats, lifespans, temperament defaults, produce, sounds,
//! and gestures. Hybrids inherit and blend traits from parents with explicit overrides.
//! Animals are tracked characters with tameness, trust, leading/riding status, and produce schedules.
//! Interaction ops: pet, feed, calm, lead, ride, train, harvest, and non-verbal behaviour generation.
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};

pub const MAX_SPECIES: usize = 256;
pub const MAX_ANIMALS: usize = 256;
pub const MAX_OPS: usize = 64;
const MAX_SAFE: u64 = 9_007_199_254_740_991;
/// Host-set limits (defaults are the earlier fixed bounds): most `species` and `animals`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "species", default: MAX_SPECIES, ceiling: 65_536 },
    crate::character::limits::Limit { name: "animals", default: MAX_ANIMALS, ceiling: 1_000_000 },
];
crate::character::limits::module_limits!(SpeciesLimits, LIMITS);

pub const KINDS: [&str; 4] = ["animal", "plant", "person", "hybrid"];
pub const DIETS: [&str; 6] = ["herbivore", "carnivore", "omnivore", "photosynthetic", "scavenger", "mineral"];

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

fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(s) => s.parse().ok(),
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

fn bool_bound(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(c::bad)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Temperament {
    pub boldness: u64,
    pub sociability: u64,
    pub aggression: u64,
    pub trainability: u64,
}

impl Temperament {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(v, &["boldness", "sociability", "aggression", "trainability"], &[])?;
        let boldness = uint_bound(c::get(v, "boldness")?, 0, 100)?;
        let sociability = uint_bound(c::get(v, "sociability")?, 0, 100)?;
        let aggression = uint_bound(c::get(v, "aggression")?, 0, 100)?;
        let trainability = uint_bound(c::get(v, "trainability")?, 0, 100)?;
        Ok(Self { boldness, sociability, aggression, trainability })
    }

    pub fn to_value(self) -> Value {
        obj! {
            "boldness" => self.boldness,
            "sociability" => self.sociability,
            "aggression" => self.aggression,
            "trainability" => self.trainability,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProduceDef {
    pub kind: String,
    pub amount: u64,
    pub interval: u64,
}

impl ProduceDef {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(v, &["kind", "amount", "interval"], &[])?;
        let kind = ident_bound(c::get(v, "kind")?)?.to_owned();
        let amount = uint_bound(c::get(v, "amount")?, 1, 1_000_000_000)?;
        let interval = uint_bound(c::get(v, "interval")?, 1, 1_000_000_000)?;
        Ok(Self { kind, amount, interval })
    }

    pub fn to_value(&self) -> Value {
        obj! {
            "kind" => self.kind.clone(),
            "amount" => self.amount,
            "interval" => self.interval,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeciesDef {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub parents: Option<Vec<String>>,
    pub diet: Option<String>,
    pub habitats: Option<Vec<String>>,
    pub lifespan: Option<u64>,
    pub temperament: Option<Temperament>,
    pub tameable: Option<bool>,
    pub rideable: Option<bool>,
    pub carrier_capacity: Option<u64>,
    pub produce: Option<Vec<ProduceDef>>,
    pub sounds: Option<Vec<String>>,
    pub gestures: Option<Vec<String>>,
    pub needs: Option<Vec<Value>>,
}

impl SpeciesDef {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(
            v,
            &["id", "label", "kind"],
            &[
                "parents",
                "diet",
                "habitats",
                "lifespan",
                "temperament",
                "tameable",
                "rideable",
                "carrier_capacity",
                "produce",
                "sounds",
                "gestures",
                "needs",
            ],
        )?;

        let id = ident_bound(c::get(v, "id")?)?.to_owned();
        let label = text_bound(c::get(v, "label")?, 48)?.to_owned();
        let kind = ident_bound(c::get(v, "kind")?)?.to_owned();
        if !KINDS.contains(&kind.as_str()) {
            return Err(c::bad());
        }

        let parents = if kind == "hybrid" {
            let p_arr = c::get(v, "parents")?.as_array().ok_or_else(c::bad)?;
            if p_arr.is_empty() || p_arr.len() > 2 {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for p in p_arr {
                list.push(ident_bound(p)?.to_owned());
            }
            Some(list)
        } else {
            if let Some(p) = v.get("parents").filter(|x| **x != Value::Null) {
                if p.as_array().is_some() {
                    return Err(c::bad());
                }
            }
            None
        };

        let diet = if let Some(d) = v.get("diet").filter(|x| **x != Value::Null) {
            let d_str = ident_bound(d)?;
            if !DIETS.contains(&d_str) {
                return Err(c::bad());
            }
            Some(d_str.to_owned())
        } else {
            None
        };

        let habitats = if let Some(h) = v.get("habitats").filter(|x| **x != Value::Null) {
            let arr = h.as_array().ok_or_else(c::bad)?;
            if arr.len() > 16 {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for hab in arr {
                list.push(ident_bound(hab)?.to_owned());
            }
            Some(list)
        } else {
            None
        };

        let lifespan = if let Some(ls) = v.get("lifespan").filter(|x| **x != Value::Null) {
            Some(uint_bound(ls, 1, 1_000_000)?)
        } else {
            None
        };

        let temperament = if let Some(t) = v.get("temperament").filter(|x| **x != Value::Null) {
            Some(Temperament::parse(t)?)
        } else {
            None
        };

        let tameable =
            if let Some(tb) = v.get("tameable").filter(|x| **x != Value::Null) { Some(bool_bound(tb)?) } else { None };

        let rideable =
            if let Some(rb) = v.get("rideable").filter(|x| **x != Value::Null) { Some(bool_bound(rb)?) } else { None };

        let carrier_capacity = if let Some(cc) = v.get("carrier_capacity").filter(|x| **x != Value::Null) {
            Some(uint_bound(cc, 0, 1_000_000_000)?)
        } else {
            None
        };

        let produce = if let Some(pr) = v.get("produce").filter(|x| **x != Value::Null) {
            let arr = pr.as_array().ok_or_else(c::bad)?;
            if arr.len() > 8 {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for p in arr {
                list.push(ProduceDef::parse(p)?);
            }
            Some(list)
        } else {
            None
        };

        let sounds = if let Some(snd) = v.get("sounds").filter(|x| **x != Value::Null) {
            let arr = snd.as_array().ok_or_else(c::bad)?;
            if arr.len() > 16 {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for s in arr {
                list.push(text_bound(s, 32)?.to_owned());
            }
            Some(list)
        } else {
            None
        };

        let gestures = if let Some(gst) = v.get("gestures").filter(|x| **x != Value::Null) {
            let arr = gst.as_array().ok_or_else(c::bad)?;
            if arr.len() > 16 {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for g in arr {
                list.push(text_bound(g, 48)?.to_owned());
            }
            Some(list)
        } else {
            None
        };

        let needs = if let Some(n_val) = v.get("needs").filter(|x| **x != Value::Null) {
            let arr = n_val.as_array().ok_or_else(c::bad)?;
            if arr.len() > 16 {
                return Err(c::bad());
            }
            Some(arr.to_vec())
        } else {
            None
        };

        Ok(Self {
            id,
            label,
            kind,
            parents,
            diet,
            habitats,
            lifespan,
            temperament,
            tameable,
            rideable,
            carrier_capacity,
            produce,
            sounds,
            gestures,
            needs,
        })
    }

    pub fn to_value(&self) -> Value {
        let mut r = obj! {
            "id" => self.id.clone(),
            "label" => self.label.clone(),
            "kind" => self.kind.clone(),
        };
        if let Some(ref p) = self.parents {
            let arr: Vec<Value> = p.iter().map(|s| Value::String(s.clone())).collect();
            set(&mut r, "parents", Value::Array(arr));
        }
        if let Some(ref d) = self.diet {
            set(&mut r, "diet", Value::String(d.clone()));
        }
        if let Some(ref h) = self.habitats {
            let arr: Vec<Value> = h.iter().map(|s| Value::String(s.clone())).collect();
            set(&mut r, "habitats", Value::Array(arr));
        }
        if let Some(ls) = self.lifespan {
            set(&mut r, "lifespan", Value::from(ls));
        }
        if let Some(t) = self.temperament {
            set(&mut r, "temperament", t.to_value());
        }
        if let Some(tb) = self.tameable {
            set(&mut r, "tameable", Value::Bool(tb));
        }
        if let Some(rb) = self.rideable {
            set(&mut r, "rideable", Value::Bool(rb));
        }
        if let Some(cc) = self.carrier_capacity {
            set(&mut r, "carrier_capacity", Value::from(cc));
        }
        if let Some(ref pr) = self.produce {
            let arr: Vec<Value> = pr.iter().map(|p| p.to_value()).collect();
            set(&mut r, "produce", Value::Array(arr));
        }
        if let Some(ref snds) = self.sounds {
            let arr: Vec<Value> = snds.iter().map(|s| Value::String(s.clone())).collect();
            set(&mut r, "sounds", Value::Array(arr));
        }
        if let Some(ref gsts) = self.gestures {
            let arr: Vec<Value> = gsts.iter().map(|g| Value::String(g.clone())).collect();
            set(&mut r, "gestures", Value::Array(arr));
        }
        if let Some(ref nds) = self.needs {
            set(&mut r, "needs", Value::Array(nds.clone()));
        }
        r
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimalDef {
    pub id: String,
    pub species: String,
    pub tameness: Option<u64>,
    pub trust: Option<Vec<(String, i64)>>,
    pub has_lead_by: bool,
    pub lead_by: Option<String>,
    pub has_ridden_by: bool,
    pub ridden_by: Option<String>,
    pub temperament: Option<Temperament>,
    pub last_harvest: Option<Vec<(String, u64)>>,
}

impl AnimalDef {
    pub fn parse(v: &Value) -> Result<Self> {
        keys(v, &["id", "species"], &["tameness", "trust", "lead_by", "ridden_by", "temperament", "last_harvest"])?;
        let id = ident_bound(c::get(v, "id")?)?.to_owned();
        let species = ident_bound(c::get(v, "species")?)?.to_owned();

        let tameness = if let Some(tm) = v.get("tameness").filter(|x| **x != Value::Null) {
            Some(uint_bound(tm, 0, 100)?)
        } else {
            None
        };

        let trust = if let Some(tr) = v.get("trust").filter(|x| **x != Value::Null) {
            let obj_fields = tr.as_object().ok_or_else(c::bad)?;
            if obj_fields.len() > 64 {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for (actor_id, val) in obj_fields {
                let _ = ident_bound(&Value::String(actor_id.clone()))?;
                let n = as_i64(val).ok_or_else(c::bad)?;
                if !(-100..=100).contains(&n) {
                    return Err(c::bad());
                }
                list.push((actor_id.clone(), n));
            }
            Some(list)
        } else {
            None
        };

        let has_lead_by = v.get("lead_by").is_some();
        let lead_by = if let Some(lb) = v.get("lead_by").filter(|x| **x != Value::Null) {
            Some(ident_bound(lb)?.to_owned())
        } else {
            None
        };

        let has_ridden_by = v.get("ridden_by").is_some();
        let ridden_by = if let Some(rb) = v.get("ridden_by").filter(|x| **x != Value::Null) {
            Some(ident_bound(rb)?.to_owned())
        } else {
            None
        };

        let temperament = if let Some(t) = v.get("temperament").filter(|x| **x != Value::Null) {
            Some(Temperament::parse(t)?)
        } else {
            None
        };

        let last_harvest = if let Some(lh) = v.get("last_harvest").filter(|x| **x != Value::Null) {
            let obj_fields = lh.as_object().ok_or_else(c::bad)?;
            if obj_fields.len() > 16 {
                return Err(c::bad());
            }
            let mut list = Vec::new();
            for (prod_kind, val) in obj_fields {
                let _ = ident_bound(&Value::String(prod_kind.clone()))?;
                let n = as_u64(val).ok_or_else(c::bad)?;
                list.push((prod_kind.clone(), n));
            }
            Some(list)
        } else {
            None
        };

        Ok(Self {
            id,
            species,
            tameness,
            trust,
            has_lead_by,
            lead_by,
            has_ridden_by,
            ridden_by,
            temperament,
            last_harvest,
        })
    }

    pub fn to_value(&self) -> Value {
        let mut r = obj! {
            "id" => self.id.clone(),
            "species" => self.species.clone(),
        };
        if let Some(t) = self.tameness {
            set(&mut r, "tameness", Value::from(t));
        }
        if let Some(ref tr) = self.trust {
            let fields: Vec<(&str, Value)> =
                tr.iter().map(|(k, v)| (k.as_str(), Value::Number(v.to_string()))).collect();
            set(&mut r, "trust", Value::object(fields));
        }
        if self.has_lead_by {
            let val = self.lead_by.as_ref().map_or(Value::Null, |s| Value::String(s.clone()));
            set(&mut r, "lead_by", val);
        }
        if self.has_ridden_by {
            let val = self.ridden_by.as_ref().map_or(Value::Null, |s| Value::String(s.clone()));
            set(&mut r, "ridden_by", val);
        }
        if let Some(t) = self.temperament {
            set(&mut r, "temperament", t.to_value());
        }
        if let Some(ref lh) = self.last_harvest {
            let fields: Vec<(&str, Value)> = lh.iter().map(|(k, v)| (k.as_str(), Value::from(*v))).collect();
            set(&mut r, "last_harvest", Value::object(fields));
        }
        r
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeciesWorld {
    pub species: Vec<SpeciesDef>,
    pub resolved_species: Vec<SpeciesDef>,
    pub animals: Vec<AnimalDef>,
    limits: SpeciesLimits,
}

impl Default for SpeciesWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeciesWorld {
    pub fn new() -> Self {
        Self {
            species: Vec::new(),
            resolved_species: Vec::new(),
            animals: Vec::new(),
            limits: SpeciesLimits::default(),
        }
    }

    pub fn to_value(&self) -> Value {
        obj! {
            "species" => Value::Array(self.species.iter().map(|s| s.to_value()).collect()),
            "animals" => Value::Array(self.animals.iter().map(|a| a.to_value()).collect()),
        }
    }

    pub fn from_value(v: &Value) -> Result<Self> {
        keys(v, &[], &["species", "animals"])?;
        Self::parse(v.get("species"), v.get("animals"))
    }

    pub fn parse(s_val: Option<&Value>, a_val: Option<&Value>) -> Result<Self> {
        Self::parse_within(s_val, a_val, &SpeciesLimits::default())
    }

    /// [`parse`](Self::parse) with host-set [limits](LIMITS); the result keeps them.
    pub fn parse_within(
        s_val: Option<&Value>,
        a_val: Option<&Value>,
        l: &crate::character::limits::Limits,
    ) -> Result<Self> {
        let mut raw_species = Vec::new();
        if let Some(sv) = s_val.filter(|x| **x != Value::Null) {
            let arr = sv.as_array().ok_or_else(c::bad)?;
            if arr.len() > l.get("species") {
                return Err(c::bad());
            }
            for s in arr {
                let sp = SpeciesDef::parse(s)?;
                if raw_species.iter().any(|old: &SpeciesDef| old.id == sp.id) {
                    return Err(c::bad());
                }
                raw_species.push(sp);
            }
        }

        let resolved = Self::resolve_all(&raw_species)?;

        let mut animals = Vec::new();
        if let Some(av) = a_val.filter(|x| **x != Value::Null) {
            let arr = av.as_array().ok_or_else(c::bad)?;
            if arr.len() > l.get("animals") {
                return Err(c::bad());
            }
            for a in arr {
                let an = AnimalDef::parse(a)?;
                if animals.iter().any(|old: &AnimalDef| old.id == an.id) {
                    return Err(c::bad());
                }
                animals.push(an);
            }
        }

        Ok(Self { species: raw_species, resolved_species: resolved, animals, limits: SpeciesLimits(l.clone()) })
    }

    fn resolve_all(species_list: &[SpeciesDef]) -> Result<Vec<SpeciesDef>> {
        let mut resolved = Vec::new();

        fn resolve<'a>(
            sid: &'a str,
            raw_list: &'a [SpeciesDef],
            resolved: &mut Vec<SpeciesDef>,
            stack: &mut Vec<&'a str>,
        ) -> Result<SpeciesDef> {
            if let Some(r) = resolved.iter().find(|s| s.id == sid) {
                return Ok(r.clone());
            }
            if stack.contains(&sid) {
                return Err(c::bad()); // Inheritance cycle!
            }
            let raw = raw_list.iter().find(|s| s.id == sid).ok_or_else(c::bad)?;
            if raw.kind != "hybrid" {
                resolved.push(raw.clone());
                return Ok(raw.clone());
            }

            stack.push(sid);
            let parents_ids = raw.parents.as_ref().ok_or_else(c::bad)?;
            let mut resolved_parents = Vec::new();
            for pid in parents_ids {
                let p = resolve(pid, raw_list, resolved, stack)?;
                resolved_parents.push(p);
            }
            stack.pop();

            let p0 = &resolved_parents[0];
            let mut merged = p0.clone();
            merged.id = raw.id.clone();
            merged.label = raw.label.clone();
            merged.kind = "hybrid".to_string();
            merged.parents = Some(parents_ids.clone());

            if resolved_parents.len() > 1 {
                let p1 = &resolved_parents[1];
                let mut habs = p0.habitats.clone().unwrap_or_default();
                if let Some(ref h1) = p1.habitats {
                    for h in h1 {
                        if !habs.contains(h) {
                            habs.push(h.clone());
                        }
                    }
                }
                merged.habitats = Some(habs);

                let t0 = p0.temperament.unwrap_or(Temperament {
                    boldness: 50,
                    sociability: 50,
                    aggression: 50,
                    trainability: 50,
                });
                let t1 = p1.temperament.unwrap_or(Temperament {
                    boldness: 50,
                    sociability: 50,
                    aggression: 50,
                    trainability: 50,
                });
                merged.temperament = Some(Temperament {
                    boldness: (t0.boldness + t1.boldness) / 2,
                    sociability: (t0.sociability + t1.sociability) / 2,
                    aggression: (t0.aggression + t1.aggression) / 2,
                    trainability: (t0.trainability + t1.trainability) / 2,
                });

                let mut snds = p0.sounds.clone().unwrap_or_default();
                if let Some(ref s1) = p1.sounds {
                    for s in s1 {
                        if !snds.contains(s) {
                            snds.push(s.clone());
                        }
                    }
                }
                merged.sounds = Some(snds);

                let mut gsts = p0.gestures.clone().unwrap_or_default();
                if let Some(ref g1) = p1.gestures {
                    for g in g1 {
                        if !gsts.contains(g) {
                            gsts.push(g.clone());
                        }
                    }
                }
                merged.gestures = Some(gsts);
            }

            // Apply explicit overrides from hybrid definition
            if raw.diet.is_some() {
                merged.diet = raw.diet.clone();
            }
            if raw.habitats.is_some() {
                merged.habitats = raw.habitats.clone();
            }
            if raw.lifespan.is_some() {
                merged.lifespan = raw.lifespan;
            }
            if raw.tameable.is_some() {
                merged.tameable = raw.tameable;
            }
            if raw.rideable.is_some() {
                merged.rideable = raw.rideable;
            }
            if raw.carrier_capacity.is_some() {
                merged.carrier_capacity = raw.carrier_capacity;
            }
            if raw.produce.is_some() {
                merged.produce = raw.produce.clone();
            }
            if raw.sounds.is_some() {
                merged.sounds = raw.sounds.clone();
            }
            if raw.gestures.is_some() {
                merged.gestures = raw.gestures.clone();
            }
            if raw.temperament.is_some() {
                merged.temperament = raw.temperament;
            }
            if raw.needs.is_some() {
                merged.needs = raw.needs.clone();
            }

            resolved.push(merged.clone());
            Ok(merged)
        }

        let mut stack = Vec::new();
        for s in species_list {
            resolve(&s.id, species_list, &mut resolved, &mut stack)?;
        }

        Ok(resolved)
    }

    pub fn get_resolved_species(&self, sid: &str) -> Option<&SpeciesDef> {
        self.resolved_species.iter().find(|s| s.id == sid)
    }

    pub fn get_animal_species(&self, aid: &str) -> Result<SpeciesDef> {
        let an = self.animals.iter().find(|a| a.id == aid).ok_or_else(c::bad)?;
        self.get_resolved_species(&an.species).cloned().ok_or_else(c::bad)
    }

    pub fn get_animal_mut(&mut self, aid: &str) -> Option<&mut AnimalDef> {
        self.animals.iter_mut().find(|a| a.id == aid)
    }

    pub fn op_pet(&mut self, at: u64, actor: &str, animal_id: &str, place: &str) -> Result<Value> {
        let sp = self.get_animal_species(animal_id)?;
        let an = self.get_animal_mut(animal_id).unwrap();

        let cur_trust =
            an.trust.as_ref().and_then(|tr| tr.iter().find(|(k, _)| k == actor).map(|(_, v)| *v)).unwrap_or(0);
        let tameness = an.tameness.unwrap_or(50);
        let temp = an.temperament.or(sp.temperament).unwrap_or(Temperament {
            boldness: 50,
            sociability: 50,
            aggression: 50,
            trainability: 50,
        });

        let threat = temp.aggression as i64 - cur_trust - (tameness as i64 / 2);
        if threat > 50 {
            // Rejects / snaps
            let sound = sp.sounds.as_ref().and_then(|s| s.first()).map(|s| s.as_str()).unwrap_or("growl");
            let gesture = sp.gestures.as_ref().and_then(|g| g.first()).map(|g| g.as_str()).unwrap_or("steps back");
            let new_trust = (cur_trust - 5).max(-100);

            let tr_list = an.trust.get_or_insert_with(Vec::new);
            if let Some((_, v)) = tr_list.iter_mut().find(|(k, _)| k == actor) {
                *v = new_trust;
            } else {
                tr_list.push((actor.to_string(), new_trust));
            }

            let summary = format!("{} tried to pet {}, but it snapped ({}).", actor, animal_id, sound);
            let entry = obj! {
                "at" => at,
                "kind" => "animal_interaction",
                "summary" => summary,
                "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
                "place" => place,
                "things" => Vec::<Value>::new(),
                "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("pet"), Value::str("failed")],
                "visibility" => "public",
                "witnesses" => Vec::<Value>::new(),
                "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("pet")],
                "facts" => obj! {
                    "interaction" => "pet",
                    "success" => false,
                    "trust" => Value::Number(new_trust.to_string()),
                },
            };
            return Ok(obj! {
                "ok" => false,
                "success" => false,
                "trust" => Value::Number(new_trust.to_string()),
                "sound" => sound,
                "gesture" => gesture,
                "entry" => entry,
            });
        }

        let delta = if temp.sociability >= 50 { 10 } else { 5 };
        let new_trust = (cur_trust + delta).min(100);

        let tr_list = an.trust.get_or_insert_with(Vec::new);
        if let Some((_, v)) = tr_list.iter_mut().find(|(k, _)| k == actor) {
            *v = new_trust;
        } else {
            tr_list.push((actor.to_string(), new_trust));
        }

        let sound = sp.sounds.as_ref().and_then(|s| s.first()).map(|s| s.as_str()).unwrap_or("purr");
        let gesture = sp.gestures.as_ref().and_then(|g| g.first()).map(|g| g.as_str()).unwrap_or("nudges hand");
        let summary = format!("{} petted {} ({}).", actor, animal_id, gesture);
        let entry = obj! {
            "at" => at,
            "kind" => "animal_interaction",
            "summary" => summary,
            "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
            "place" => place,
            "things" => Vec::<Value>::new(),
            "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("pet"), Value::str("success")],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("pet")],
            "facts" => obj! {
                "interaction" => "pet",
                "success" => true,
                "trust" => Value::Number(new_trust.to_string()),
            },
        };
        Ok(obj! {
            "ok" => true,
            "success" => true,
            "trust" => Value::Number(new_trust.to_string()),
            "sound" => sound,
            "gesture" => gesture,
            "entry" => entry,
        })
    }

    pub fn op_feed(
        &mut self,
        at: u64,
        actor: &str,
        animal_id: &str,
        place: &str,
        food_thing: &str,
        things_state: &mut Value,
    ) -> Result<Value> {
        let sp = self.get_animal_species(animal_id)?;

        let (food_idx, food_label, is_meat) = {
            let things_list = things_state.get("things").and_then(|v| v.as_array()).ok_or_else(c::bad)?;
            let kinds_list = things_state.get("kinds").and_then(|v| v.as_array()).ok_or_else(c::bad)?;

            let (idx, t) = things_list
                .iter()
                .enumerate()
                .find(|(_, t)| {
                    t.get("id").and_then(|v| v.as_str()) == Some(food_thing) && {
                        let loc = t.get("location");
                        let is_held = loc.and_then(|l| l.get("holder")).and_then(|h| h.as_str()) == Some(actor);
                        let is_in_place = loc.and_then(|l| l.get("place")).and_then(|p| p.as_str()) == Some(place);
                        is_held || is_in_place
                    }
                })
                .ok_or_else(c::bad)?;

            let t_kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let kd = kinds_list.iter().find(|k| k.get("id").and_then(|v| v.as_str()) == Some(t_kind));
            let tags = kd
                .and_then(|k| k.get("tags"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();
            let cat = kd.and_then(|k| k.get("category")).and_then(|v| v.as_str()).unwrap_or("");
            let is_meat = tags.contains(&"meat") || (cat == "food" && t_kind.contains("meat"));
            let label = kd.and_then(|k| k.get("label")).and_then(|v| v.as_str()).unwrap_or(food_thing).to_string();
            (idx, label, is_meat)
        };

        let diet = sp.diet.as_deref().unwrap_or("herbivore");
        if diet == "carnivore" && !is_meat {
            return Ok(obj! {
                "ok" => false,
                "reason" => "incompatible_diet",
                "diet" => diet,
            });
        }
        if diet == "herbivore" && is_meat {
            return Ok(obj! {
                "ok" => false,
                "reason" => "incompatible_diet",
                "diet" => diet,
            });
        }

        // Consume 1 food unit
        let things_list = get_field_mut(things_state, "things").and_then(as_array_mut).ok_or_else(c::bad)?;
        let cur_qty = things_list[food_idx].get("quantity").and_then(as_u64).unwrap_or(1);
        if cur_qty <= 1 {
            things_list.remove(food_idx);
        } else {
            set(&mut things_list[food_idx], "quantity", Value::from(cur_qty - 1));
        }

        let an = self.get_animal_mut(animal_id).unwrap();
        let cur_trust =
            an.trust.as_ref().and_then(|tr| tr.iter().find(|(k, _)| k == actor).map(|(_, v)| *v)).unwrap_or(0);
        let new_trust = (cur_trust + 15).min(100);
        let tr_list = an.trust.get_or_insert_with(Vec::new);
        if let Some((_, v)) = tr_list.iter_mut().find(|(k, _)| k == actor) {
            *v = new_trust;
        } else {
            tr_list.push((actor.to_string(), new_trust));
        }

        let cur_tameness = an.tameness.unwrap_or(50);
        let new_tameness = (cur_tameness + 5).min(100);
        an.tameness = Some(new_tameness);

        let sound = sp.sounds.as_ref().and_then(|s| s.first()).map(|s| s.as_str()).unwrap_or("munch");
        let gesture = sp.gestures.as_ref().and_then(|g| g.first()).map(|g| g.as_str()).unwrap_or("eats eagerly");
        let summary = format!("{} fed {} with {}.", actor, animal_id, food_label);
        let entry = obj! {
            "at" => at,
            "kind" => "animal_interaction",
            "summary" => summary,
            "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
            "place" => place,
            "things" => vec![Value::String(food_thing.to_string())],
            "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("feed")],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("feed")],
            "facts" => obj! {
                "interaction" => "feed",
                "trust" => Value::Number(new_trust.to_string()),
                "tameness" => new_tameness,
            },
        };
        Ok(obj! {
            "ok" => true,
            "success" => true,
            "trust" => Value::Number(new_trust.to_string()),
            "tameness" => new_tameness,
            "sound" => sound,
            "gesture" => gesture,
            "entry" => entry,
        })
    }

    pub fn op_calm(&mut self, at: u64, actor: &str, animal_id: &str, place: &str) -> Result<Value> {
        let sp = self.get_animal_species(animal_id)?;
        let an = self.get_animal_mut(animal_id).unwrap();

        let cur_trust =
            an.trust.as_ref().and_then(|tr| tr.iter().find(|(k, _)| k == actor).map(|(_, v)| *v)).unwrap_or(0);
        let new_trust = (cur_trust + 5).min(100);
        let tr_list = an.trust.get_or_insert_with(Vec::new);
        if let Some((_, v)) = tr_list.iter_mut().find(|(k, _)| k == actor) {
            *v = new_trust;
        } else {
            tr_list.push((actor.to_string(), new_trust));
        }

        let gesture = sp.gestures.as_ref().and_then(|g| g.first()).map(|g| g.as_str()).unwrap_or("relaxes stance");
        let summary = format!("{} calmed {} ({}).", actor, animal_id, gesture);
        let entry = obj! {
            "at" => at,
            "kind" => "animal_interaction",
            "summary" => summary,
            "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
            "place" => place,
            "things" => Vec::<Value>::new(),
            "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("calm")],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("calm")],
            "facts" => obj! {
                "interaction" => "calm",
                "trust" => Value::Number(new_trust.to_string()),
            },
        };
        Ok(obj! {
            "ok" => true,
            "success" => true,
            "trust" => Value::Number(new_trust.to_string()),
            "gesture" => gesture,
            "entry" => entry,
        })
    }

    pub fn op_lead(&mut self, at: u64, actor: &str, animal_id: &str, place: &str, stop: bool) -> Result<Value> {
        let sp = self.get_animal_species(animal_id)?;
        let an = self.get_animal_mut(animal_id).unwrap();

        if stop {
            an.has_lead_by = true;
            an.lead_by = None;
            let summary = format!("{} stopped leading {}.", actor, animal_id);
            let entry = obj! {
                "at" => at,
                "kind" => "animal_interaction",
                "summary" => summary,
                "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
                "place" => place,
                "things" => Vec::<Value>::new(),
                "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("lead"), Value::str("stop")],
                "visibility" => "public",
                "witnesses" => Vec::<Value>::new(),
                "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("lead")],
                "facts" => obj! {
                    "interaction" => "lead",
                    "leading" => false,
                },
            };
            return Ok(obj! {
                "ok" => true,
                "leading" => false,
                "entry" => entry,
            });
        }

        let cur_trust =
            an.trust.as_ref().and_then(|tr| tr.iter().find(|(k, _)| k == actor).map(|(_, v)| *v)).unwrap_or(0);
        let tameness = an.tameness.unwrap_or(50);
        if cur_trust < 10 && tameness < 30 {
            return Ok(obj! {
                "ok" => false,
                "reason" => "insufficient_trust_or_tameness",
                "trust" => Value::Number(cur_trust.to_string()),
                "tameness" => tameness,
            });
        }

        an.has_lead_by = true;
        an.lead_by = Some(actor.to_string());
        let summary = format!("{} began leading {}.", actor, animal_id);
        let entry = obj! {
            "at" => at,
            "kind" => "animal_interaction",
            "summary" => summary,
            "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
            "place" => place,
            "things" => Vec::<Value>::new(),
            "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("lead"), Value::str("start")],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("lead")],
            "facts" => obj! {
                "interaction" => "lead",
                "leading" => true,
            },
        };
        Ok(obj! {
            "ok" => true,
            "leading" => true,
            "entry" => entry,
        })
    }

    pub fn op_ride(&mut self, at: u64, actor: &str, animal_id: &str, place: &str, dismount: bool) -> Result<Value> {
        let sp = self.get_animal_species(animal_id)?;
        let an = self.get_animal_mut(animal_id).unwrap();

        if dismount {
            an.has_ridden_by = true;
            an.ridden_by = None;
            let summary = format!("{} dismounted from {}.", actor, animal_id);
            let entry = obj! {
                "at" => at,
                "kind" => "animal_interaction",
                "summary" => summary,
                "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
                "place" => place,
                "things" => Vec::<Value>::new(),
                "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("ride"), Value::str("dismount")],
                "visibility" => "public",
                "witnesses" => Vec::<Value>::new(),
                "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("ride")],
                "facts" => obj! {
                    "interaction" => "ride",
                    "riding" => false,
                },
            };
            return Ok(obj! {
                "ok" => true,
                "riding" => false,
                "entry" => entry,
            });
        }

        if !sp.rideable.unwrap_or(false) {
            return Ok(obj! {
                "ok" => false,
                "reason" => "not_rideable",
                "species" => sp.id.clone(),
            });
        }

        let cur_trust =
            an.trust.as_ref().and_then(|tr| tr.iter().find(|(k, _)| k == actor).map(|(_, v)| *v)).unwrap_or(0);
        let tameness = an.tameness.unwrap_or(50);
        if cur_trust < 20 || tameness < 40 {
            return Ok(obj! {
                "ok" => false,
                "reason" => "insufficient_trust_or_tameness",
                "trust" => Value::Number(cur_trust.to_string()),
                "tameness" => tameness,
            });
        }

        an.has_ridden_by = true;
        an.ridden_by = Some(actor.to_string());
        let summary = format!("{} mounted and rode {}.", actor, animal_id);
        let entry = obj! {
            "at" => at,
            "kind" => "animal_interaction",
            "summary" => summary,
            "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
            "place" => place,
            "things" => Vec::<Value>::new(),
            "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("ride"), Value::str("mount")],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("ride")],
            "facts" => obj! {
                "interaction" => "ride",
                "riding" => true,
            },
        };
        Ok(obj! {
            "ok" => true,
            "riding" => true,
            "entry" => entry,
        })
    }

    pub fn op_train(&mut self, at: u64, actor: &str, animal_id: &str, place: &str) -> Result<Value> {
        let sp = self.get_animal_species(animal_id)?;
        let an = self.get_animal_mut(animal_id).unwrap();

        if !sp.tameable.unwrap_or(true) {
            return Ok(obj! {
                "ok" => false,
                "reason" => "not_tameable",
                "species" => sp.id.clone(),
            });
        }

        let temp = an.temperament.or(sp.temperament).unwrap_or(Temperament {
            boldness: 50,
            sociability: 50,
            aggression: 50,
            trainability: 50,
        });
        let cur_tameness = an.tameness.unwrap_or(50);
        let gain = (temp.trainability / 10).max(1);
        let new_tameness = (cur_tameness + gain).min(100);
        an.tameness = Some(new_tameness);

        let summary = format!("{} trained {} (tameness +{}).", actor, animal_id, gain);
        let entry = obj! {
            "at" => at,
            "kind" => "animal_interaction",
            "summary" => summary,
            "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
            "place" => place,
            "things" => Vec::<Value>::new(),
            "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("train")],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::str("train")],
            "facts" => obj! {
                "interaction" => "train",
                "tameness" => new_tameness,
                "gain" => gain,
            },
        };
        Ok(obj! {
            "ok" => true,
            "tameness" => new_tameness,
            "gain" => gain,
            "entry" => entry,
        })
    }

    pub fn op_harvest(
        &mut self,
        at: u64,
        actor: &str,
        animal_id: &str,
        place: &str,
        things_state: &mut Value,
        new_id: Option<&str>,
    ) -> Result<Value> {
        let sp = self.get_animal_species(animal_id)?;
        let an = self.get_animal_mut(animal_id).unwrap();

        let produce_list = sp.produce.as_ref().filter(|p| !p.is_empty()).ok_or_else(|| {
            // Not harvestable
            c::bad()
        });
        let pr = match produce_list {
            Ok(p) => &p[0],
            Err(_) => {
                return Ok(obj! {
                    "ok" => false,
                    "reason" => "no_produce",
                    "species" => sp.id.clone(),
                });
            }
        };

        let last_at = an
            .last_harvest
            .as_ref()
            .and_then(|lh| lh.iter().find(|(k, _)| k == &pr.kind).map(|(_, v)| *v))
            .unwrap_or(0);
        if at < last_at + pr.interval {
            return Ok(obj! {
                "ok" => false,
                "reason" => "not_ready",
                "ready_at" => last_at + pr.interval,
            });
        }

        let lh_list = an.last_harvest.get_or_insert_with(Vec::new);
        if let Some((_, v)) = lh_list.iter_mut().find(|(k, _)| k == &pr.kind) {
            *v = at;
        } else {
            lh_list.push((pr.kind.clone(), at));
        }

        let out_id = if let Some(nid) = new_id { nid.to_string() } else { format!("{}_{}", pr.kind, at) };
        let new_t = obj! {
            "id" => out_id.clone(),
            "kind" => pr.kind.clone(),
            "location" => obj! { "holder" => actor },
            "condition" => 100u64,
            "owner" => actor,
            "quantity" => pr.amount,
            "passengers" => Vec::<Value>::new(),
        };

        let things_list = get_field_mut(things_state, "things").and_then(as_array_mut).ok_or_else(c::bad)?;
        things_list.push(new_t.clone());

        let summary = format!("{} harvested {} {} from {}.", actor, pr.amount, pr.kind, animal_id);
        let entry = obj! {
            "at" => at,
            "kind" => "animal_interaction",
            "summary" => summary,
            "actors" => vec![Value::String(actor.to_string()), Value::String(animal_id.to_string())],
            "place" => place,
            "things" => vec![Value::String(out_id)],
            "tags" => vec![Value::str("species"), Value::str("animal"), Value::str("harvest"), Value::String(pr.kind.clone())],
            "visibility" => "public",
            "witnesses" => Vec::<Value>::new(),
            "phrases" => vec![Value::String(sp.label.clone()), Value::String(animal_id.to_string()), Value::String(pr.kind.clone())],
            "facts" => obj! {
                "interaction" => "harvest",
                "product" => pr.kind.clone(),
                "amount" => pr.amount,
            },
        };
        Ok(obj! {
            "ok" => true,
            "product" => pr.kind.clone(),
            "amount" => pr.amount,
            "item" => new_t,
            "entry" => entry,
        })
    }

    pub fn op_behave(&self, actor: &str, animal_id: &str, _place: &str) -> Result<Value> {
        let an = self.animals.iter().find(|a| a.id == animal_id).ok_or_else(c::bad)?;
        let sp = self.get_resolved_species(&an.species).ok_or_else(c::bad)?;

        let cur_trust =
            an.trust.as_ref().and_then(|tr| tr.iter().find(|(k, _)| k == actor).map(|(_, v)| *v)).unwrap_or(0);
        let tameness = an.tameness.unwrap_or(50);
        let temp = an.temperament.or(sp.temperament).unwrap_or(Temperament {
            boldness: 50,
            sociability: 50,
            aggression: 50,
            trainability: 50,
        });

        let (attitude, sound, gesture) = if cur_trust >= 30 && tameness >= 50 {
            (
                "affectionate",
                sp.sounds.as_ref().and_then(|s| s.first()).map(|s| s.as_str()).unwrap_or("contented sound"),
                sp.gestures.as_ref().and_then(|g| g.first()).map(|g| g.as_str()).unwrap_or("leans against actor"),
            )
        } else if cur_trust < -10 || temp.aggression >= 70 {
            (
                "hostile",
                sp.sounds.as_ref().and_then(|s| s.first()).map(|s| s.as_str()).unwrap_or("threatening growl"),
                sp.gestures.as_ref().and_then(|g| g.first()).map(|g| g.as_str()).unwrap_or("bares teeth"),
            )
        } else if temp.boldness < 30 {
            (
                "skittish",
                sp.sounds.as_ref().and_then(|s| s.first()).map(|s| s.as_str()).unwrap_or("wary snort"),
                sp.gestures.as_ref().and_then(|g| g.first()).map(|g| g.as_str()).unwrap_or("keeps distance"),
            )
        } else {
            (
                "calm",
                sp.sounds.as_ref().and_then(|s| s.first()).map(|s| s.as_str()).unwrap_or("soft breath"),
                sp.gestures.as_ref().and_then(|g| g.first()).map(|g| g.as_str()).unwrap_or("observes calmly"),
            )
        };

        Ok(obj! {
            "ok" => true,
            "animal" => animal_id,
            "attitude" => attitude,
            "sound" => sound,
            "gesture" => gesture,
        })
    }

    /// Runs `{ops, species?, animals?, things?, limits?}`. `limits` (host-set [limits](LIMITS))
    /// apply first and are echoed in the output.
    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
        let stripped = crate::character::limits::take(req, &mut self.limits)?;
        let out = self.run_workflow(stripped.as_ref().unwrap_or(req))?;
        Ok(crate::character::limits::echo(out, &self.limits, stripped.is_some()))
    }

    /// A species world with host limits applied up front.
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut w = Self::new();
        w.limits.apply(v)?;
        Ok(w)
    }

    fn run_workflow(&mut self, req: &Value) -> Result<Value> {
        keys(req, &["ops"], &["species", "animals", "things"])?;
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        if req.get("species").is_some() || req.get("animals").is_some() {
            *self = Self::parse_within(req.get("species"), req.get("animals"), &self.limits)?;
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

            let backup_animals = self.animals.clone();
            let backup_things = things_state.clone();

            match op {
                "pet" => {
                    keys(args, &["actor", "animal", "place"], &[])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let animal = ident_bound(c::get(args, "animal")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    match self.op_pet(at, actor, animal, place) {
                        Ok(res) => results.push(obj! { "ok" => true, "result" => res }),
                        Err(_) => {
                            self.animals = backup_animals;
                            things_state = backup_things;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                "feed" => {
                    keys(args, &["actor", "animal", "place", "food_thing"], &[])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let animal = ident_bound(c::get(args, "animal")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    let food_thing = ident_bound(c::get(args, "food_thing")?)?;
                    match self.op_feed(at, actor, animal, place, food_thing, &mut things_state) {
                        Ok(res) => {
                            if res.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                results.push(obj! { "ok" => true, "result" => res });
                            } else {
                                self.animals = backup_animals;
                                things_state = backup_things;
                                results.push(obj! { "ok" => false, "result" => res });
                            }
                        }
                        Err(_) => {
                            self.animals = backup_animals;
                            things_state = backup_things;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                "calm" => {
                    keys(args, &["actor", "animal", "place"], &[])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let animal = ident_bound(c::get(args, "animal")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    match self.op_calm(at, actor, animal, place) {
                        Ok(res) => results.push(obj! { "ok" => true, "result" => res }),
                        Err(_) => {
                            self.animals = backup_animals;
                            things_state = backup_things;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                "lead" => {
                    keys(args, &["actor", "animal", "place"], &["stop"])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let animal = ident_bound(c::get(args, "animal")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    let stop =
                        args.get("stop").filter(|x| **x != Value::Null).map(bool_bound).transpose()?.unwrap_or(false);
                    match self.op_lead(at, actor, animal, place, stop) {
                        Ok(res) => {
                            if res.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                results.push(obj! { "ok" => true, "result" => res });
                            } else {
                                self.animals = backup_animals;
                                results.push(obj! { "ok" => false, "result" => res });
                            }
                        }
                        Err(_) => {
                            self.animals = backup_animals;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                "ride" => {
                    keys(args, &["actor", "animal", "place"], &["dismount"])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let animal = ident_bound(c::get(args, "animal")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    let dismount = args
                        .get("dismount")
                        .filter(|x| **x != Value::Null)
                        .map(bool_bound)
                        .transpose()?
                        .unwrap_or(false);
                    match self.op_ride(at, actor, animal, place, dismount) {
                        Ok(res) => {
                            if res.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                results.push(obj! { "ok" => true, "result" => res });
                            } else {
                                self.animals = backup_animals;
                                results.push(obj! { "ok" => false, "result" => res });
                            }
                        }
                        Err(_) => {
                            self.animals = backup_animals;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                "train" => {
                    keys(args, &["actor", "animal", "place"], &[])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let animal = ident_bound(c::get(args, "animal")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    match self.op_train(at, actor, animal, place) {
                        Ok(res) => {
                            if res.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                results.push(obj! { "ok" => true, "result" => res });
                            } else {
                                self.animals = backup_animals;
                                results.push(obj! { "ok" => false, "result" => res });
                            }
                        }
                        Err(_) => {
                            self.animals = backup_animals;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                "harvest" => {
                    keys(args, &["actor", "animal", "place"], &["new_id"])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let animal = ident_bound(c::get(args, "animal")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    let new_id = if let Some(nid) = args.get("new_id").filter(|x| **x != Value::Null) {
                        Some(ident_bound(nid)?)
                    } else {
                        None
                    };
                    match self.op_harvest(at, actor, animal, place, &mut things_state, new_id) {
                        Ok(res) => {
                            if res.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                                results.push(obj! { "ok" => true, "result" => res });
                            } else {
                                self.animals = backup_animals;
                                things_state = backup_things;
                                results.push(obj! { "ok" => false, "result" => res });
                            }
                        }
                        Err(_) => {
                            self.animals = backup_animals;
                            things_state = backup_things;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                "behave" => {
                    keys(args, &["actor", "animal", "place"], &[])?;
                    let actor = ident_bound(c::get(args, "actor")?)?;
                    let animal = ident_bound(c::get(args, "animal")?)?;
                    let place = ident_bound(c::get(args, "place")?)?;
                    match self.op_behave(actor, animal, place) {
                        Ok(res) => results.push(obj! { "ok" => true, "result" => res }),
                        Err(_) => {
                            self.animals = backup_animals;
                            things_state = backup_things;
                            results.push(obj! { "ok" => false });
                        }
                    }
                }
                _ => return Err(c::bad()),
            }
        }

        let sp_val = Value::Array(self.species.iter().map(|s| s.to_value()).collect());
        let an_val = Value::Array(self.animals.iter().map(|a| a.to_value()).collect());

        Ok(obj! {
            "results" => results,
            "species" => sp_val,
            "animals" => an_val,
            "things" => things_state,
        })
    }
}

/// The `species` local kind: `{ops, species?, animals?, things?, limits?}` →
/// `{limits?, results, ...}`.
pub fn workflow(a: &Value) -> Result<Value> {
    SpeciesWorld::new().workflow(a)
}
