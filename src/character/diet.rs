//! Diets and foods (pass 2, stage 3): what a being can, will and likes to eat.
//!
//! **Foods** are registered by the host (or loaded from the original built-in `village`
//! pack): tags, how much hunger and thirst a serving fills, enjoyment (`joy`), when it spoils
//! (and how much longer preserved), how risky it is raw, and how much cooking adds.
//!
//! **Diet profiles** say what a being's body allows (a [species diet](crate::knowledge::species::DIETS)),
//! what it refuses on principle (`forbids` tags: vegetarian, a taboo, a vow), what makes it ill
//! (`allergies`: tags or food ids) and its tastes (`likes`, `dislikes`).
//!
//! A serving is `{food, cooked?, preserved?, made_at?, at?}`. `evaluate` reports freshness
//! (`fresh` until half the shelf life, then `stale`, then `spoiled`), refusals, fill, sickness and
//! joy, including a monotony penalty for repeating recent meals. `choose` ranks options for a
//! hunger level. `eat` records the meal and returns proposals in the exact argument shapes of
//! [body](crate::character::body) `satisfy` / `set_condition` and
//! [affect](crate::character::affect) `set_mood`. A refused serving is only eaten with `force`
//! (a starving being eating spoiled food). Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::diet::DietManager;
//!
//! let mut diet = DietManager::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "register_food", "args": {"pack": "village"}},
//!   {"op": "set_diet", "args": {"being": "miller", "forbids": ["meat"], "likes": ["sweet"]}},
//!   {"op": "choose", "args": {"being": "miller", "hunger_level": 80,
//!                             "options": [{"food": "hearty_stew", "cooked": true}, {"food": "honey_cake"}]}}
//! ]}"#, 4096)?;
//! let out = diet.workflow(&req)?;
//! let choice = &out.get("results").unwrap().as_array().unwrap()[2];
//! assert_eq!(choice.get("best").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_bool, opt_uint, run_ops, text_bound, uint_bound, MAX_AT};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::knowledge::species::DIETS;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const FOOD_TAGS: [&str; 13] = [
    "alcohol",
    "dairy",
    "egg",
    "fish",
    "fruit",
    "grain",
    "herb",
    "meat",
    "mineral",
    "nut",
    "sweet",
    "vegetable",
    "water",
];
pub const MAX_FOODS: usize = 256;
pub const MAX_REGISTER: usize = 64;
pub const MAX_BEINGS: usize = 64;
pub const MAX_RECENT: usize = 16;
/// Meals looked back over for the monotony penalty.
pub const MONOTONY_WINDOW: usize = 8;
pub const MAX_OPTIONS: usize = 16;
pub const MAX_SHELF: u64 = 1_000_000;
/// Sickness at or above this proposes the body condition `sick`.
pub const SICK_AT: u64 = 30;

/// The original built-in `village` food pack.
pub const VILLAGE_PACK: &str = r#"[
 {"id":"rye_bread","label":"rye bread","tags":["grain"],"hunger":35,"joy":2,"spoils_after":96,"cooked_bonus":0},
 {"id":"oat_porridge","label":"oat porridge","tags":["grain","dairy"],"hunger":30,"thirst":5,"spoils_after":24},
 {"id":"orchard_apple","label":"orchard apple","tags":["fruit"],"hunger":12,"thirst":8,"joy":3,"spoils_after":240},
 {"id":"hard_cheese","label":"hard cheese","tags":["dairy"],"hunger":25,"joy":4,"spoils_after":480},
 {"id":"river_trout","label":"river trout","tags":["fish"],"hunger":30,"spoils_after":24,"preserved_spoils_after":720,"raw_risk":35,"cooked_bonus":20},
 {"id":"hearty_stew","label":"hearty stew","tags":["meat","vegetable"],"hunger":50,"thirst":10,"joy":6,"spoils_after":36,"raw_risk":40,"cooked_bonus":10},
 {"id":"bramble_berries","label":"bramble berries","tags":["fruit"],"hunger":8,"thirst":4,"joy":4,"spoils_after":48},
 {"id":"honey_cake","label":"honey cake","tags":["grain","sweet","egg"],"hunger":20,"joy":10,"spoils_after":72},
 {"id":"barley_ale","label":"barley ale","tags":["alcohol","grain"],"hunger":5,"thirst":30,"joy":8},
 {"id":"spring_water","label":"spring water","tags":["water"],"hunger":0,"thirst":40},
 {"id":"roasted_nuts","label":"roasted nuts","tags":["nut"],"hunger":15,"joy":3,"spoils_after":720},
 {"id":"salt_lick","label":"salt lick","tags":["mineral"],"hunger":10,"thirst":0,"joy":-2}
]"#;

/// Host-set capacity limits: `(name, default, ceiling)`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "foods", default: 256, ceiling: 65536 },
    crate::character::limits::Limit { name: "beings", default: 64, ceiling: 1000000 },
    crate::character::limits::Limit { name: "recent", default: 16, ceiling: 256 },
];
crate::character::limits::module_limits!(DietLimits, LIMITS);

#[derive(Clone, Debug, PartialEq)]
pub struct Food {
    pub id: String,
    pub label: String,
    pub tags: BTreeSet<String>,
    pub hunger: u64,
    pub thirst: u64,
    pub joy: i64,
    pub spoils_after: u64,
    pub preserved_spoils_after: u64,
    pub raw_risk: u64,
    pub cooked_bonus: u64,
}

impl Food {
    pub fn from_value(v: &Value) -> Result<Self> {
        keys(
            v,
            &["id", "tags", "hunger"],
            &["label", "thirst", "joy", "spoils_after", "preserved_spoils_after", "raw_risk", "cooked_bonus"],
        )?;
        let id = ident(c::get(v, "id")?)?.to_string();
        let label = opt(v, "label").map(|l| text_bound(l, 48)).transpose()?.unwrap_or(&id).to_string();
        let raw_tags = list_bound(Some(c::get(v, "tags")?), 8)?;
        let mut tags = BTreeSet::new();
        for t in raw_tags {
            let t = ident(t)?;
            if !FOOD_TAGS.contains(&t) || !tags.insert(t.to_string()) {
                return Err(c::bad());
            }
        }
        if tags.is_empty() {
            return Err(c::bad());
        }
        let joy = match opt(v, "joy") {
            None => 0,
            Some(Value::Number(s)) => {
                let n = s.parse::<i64>().map_err(|_| c::bad())?;
                if !(-20..=20).contains(&n) {
                    return Err(c::bad());
                }
                n
            }
            Some(_) => return Err(c::bad()),
        };
        let spoils_after = opt_uint(v, "spoils_after", 0, MAX_SHELF, 0)?;
        let preserved_default = (spoils_after * 4).min(MAX_SHELF);
        Ok(Food {
            id,
            label,
            tags,
            hunger: uint_bound(c::get(v, "hunger")?, 0, 100)?,
            thirst: opt_uint(v, "thirst", 0, 100, 0)?,
            joy,
            spoils_after,
            preserved_spoils_after: opt_uint(v, "preserved_spoils_after", 0, MAX_SHELF, preserved_default)?,
            raw_risk: opt_uint(v, "raw_risk", 0, 100, 0)?,
            cooked_bonus: opt_uint(v, "cooked_bonus", 0, 100, 0)?,
        })
    }

    pub fn to_value(&self) -> Value {
        obj! {
            "cooked_bonus" => self.cooked_bonus,
            "hunger" => self.hunger,
            "id" => self.id.as_str(),
            "joy" => self.joy,
            "label" => self.label.as_str(),
            "preserved_spoils_after" => self.preserved_spoils_after,
            "raw_risk" => self.raw_risk,
            "spoils_after" => self.spoils_after,
            "tags" => Value::strings(&self.tags.iter().collect::<Vec<_>>()),
            "thirst" => self.thirst,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub diet: String,
    pub forbids: BTreeSet<String>,
    pub allergies: BTreeSet<String>,
    pub likes: BTreeSet<String>,
    pub dislikes: BTreeSet<String>,
    pub recent: VecDeque<(String, u64)>,
    pub meals: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            diet: "omnivore".into(),
            forbids: BTreeSet::new(),
            allergies: BTreeSet::new(),
            likes: BTreeSet::new(),
            dislikes: BTreeSet::new(),
            recent: VecDeque::new(),
            meals: 0,
        }
    }
}

fn strings(xs: &BTreeSet<String>) -> Value {
    Value::strings(&xs.iter().collect::<Vec<_>>())
}

struct Serving {
    food: String,
    cooked: bool,
    preserved: bool,
    made_at: u64,
    at: u64,
}

/// What eating one serving would do.
#[derive(Clone, Debug, PartialEq)]
pub struct MealEval {
    pub food: String,
    pub freshness: &'static str,
    pub refusals: Vec<&'static str>,
    pub hunger: u64,
    pub thirst: u64,
    pub joy: i64,
    pub sickness: u64,
    pub repeats: u64,
}

impl MealEval {
    fn to_value(&self, being: &str) -> Value {
        obj! {
            "being" => being,
            "edible" => self.refusals.is_empty(),
            "food" => self.food.as_str(),
            "freshness" => self.freshness,
            "hunger" => self.hunger,
            "joy" => self.joy,
            "refusals" => Value::strings(&self.refusals),
            "repeats" => self.repeats,
            "sickness" => self.sickness,
            "thirst" => self.thirst,
        }
    }
}

fn tag_set(v: Option<&Value>, max: usize) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for t in list_bound(v, max)? {
        let t = ident(t)?;
        if !FOOD_TAGS.contains(&t) {
            return Err(c::bad());
        }
        out.insert(t.to_string());
    }
    Ok(out)
}

fn read_serving(v: &Value, extra: &[&str]) -> Result<Serving> {
    let mut optional = vec!["cooked", "preserved", "made_at", "at"];
    optional.extend_from_slice(extra);
    keys(v, &["food"], &optional)?;
    let at = opt_uint(v, "at", 0, MAX_AT, 0)?;
    let made_at = opt_uint(v, "made_at", 0, MAX_AT, at)?;
    if made_at > at {
        return Err(c::bad());
    }
    Ok(Serving {
        food: ident(c::get(v, "food")?)?.to_string(),
        cooked: opt_bool(v, "cooked", false)?,
        preserved: opt_bool(v, "preserved", false)?,
        made_at,
        at,
    })
}

/// Food registry plus per-being diet profiles and meal history.
#[derive(Clone, Debug, Default)]
pub struct DietManager {
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: DietLimits,
    pub foods: BTreeMap<String, Food>,
    pub beings: BTreeMap<String, Profile>,
}

impl DietManager {
    pub fn new() -> Self {
        Self::default()
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        run_ops(request, |op, args| match op {
            "register_food" => self.op_register(args),
            "set_diet" => self.op_set_diet(args),
            "evaluate" => {
                let being = ident(c::get(args, "being")?)?.to_string();
                let s = read_serving(args, &["being"])?;
                Ok(self.evaluate(&being, &s)?.to_value(&being))
            }
            "choose" => self.op_choose(args),
            "eat" => self.op_eat(args),
            "assess" => self.op_assess(args),
            _ => Err(c::bad()),
        })
    }

    fn op_register(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["foods", "pack"])?;
        let mut fresh = Vec::new();
        if let Some(p) = opt(args, "pack") {
            if text_bound(p, 16)? != "village" {
                return Err(c::bad());
            }
            let pack = crate::data::json::parse(VILLAGE_PACK, 1 << 16)?;
            for f in pack.as_array().ok_or_else(c::bad)? {
                fresh.push(Food::from_value(f)?);
            }
        }
        let given = list_bound(opt(args, "foods"), MAX_REGISTER)?;
        if given.is_empty() && fresh.is_empty() {
            return Err(c::bad());
        }
        for f in given {
            fresh.push(Food::from_value(f)?);
        }
        let ids: BTreeSet<&String> = fresh.iter().map(|f| &f.id).collect();
        let added = ids.iter().filter(|id| !self.foods.contains_key(**id)).count();
        if self.foods.len() + added > self.limits.get("foods") {
            return Err(c::bad());
        }
        let mut registered = Vec::new();
        for f in fresh {
            registered.push(Value::str(&f.id));
            self.foods.insert(f.id.clone(), f);
        }
        Ok(obj! { "registered" => Value::Array(registered), "total" => self.foods.len() })
    }

    fn op_set_diet(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["diet", "forbids", "allergies", "likes", "dislikes"])?;
        let being = ident(c::get(args, "being")?)?.to_string();
        let diet = opt(args, "diet").map(|v| text_bound(v, 16)).transpose()?.unwrap_or("omnivore");
        if !DIETS.contains(&diet) {
            return Err(c::bad());
        }
        let mut allergies = BTreeSet::new();
        for a in list_bound(opt(args, "allergies"), 8)? {
            allergies.insert(ident(a)?.to_string());
        }
        let likes = tag_set(opt(args, "likes"), FOOD_TAGS.len())?;
        let dislikes = tag_set(opt(args, "dislikes"), FOOD_TAGS.len())?;
        if likes.intersection(&dislikes).next().is_some() {
            return Err(c::bad());
        }
        let forbids = tag_set(opt(args, "forbids"), FOOD_TAGS.len())?;
        if !self.beings.contains_key(&being) && self.beings.len() >= self.limits.get("beings") {
            return Err(c::bad());
        }
        let old = self.beings.remove(&being).unwrap_or_default();
        let p = Profile { diet: diet.to_string(), forbids, allergies, likes, dislikes, ..old };
        self.beings.insert(being.clone(), p);
        self.op_assess(&obj! { "being" => being })
    }

    /// What eating `s` would do for `being`, without changing anything.
    fn evaluate(&self, being: &str, s: &Serving) -> Result<MealEval> {
        let food = self.foods.get(&s.food).ok_or_else(c::bad)?;
        let fallback = Profile::default();
        let p = self.beings.get(being).unwrap_or(&fallback);

        let shelf = if s.preserved { food.preserved_spoils_after } else { food.spoils_after };
        let age = s.at - s.made_at;
        let freshness = if shelf == 0 || age * 2 < shelf {
            "fresh"
        } else if age < shelf {
            "stale"
        } else {
            "spoiled"
        };

        let has = |t: &str| food.tags.contains(t);
        let species_ok = match p.diet.as_str() {
            "herbivore" => !has("meat") && !has("fish") && !has("mineral"),
            "carnivore" => has("meat") || has("fish"),
            "photosynthetic" => false,
            "mineral" => has("mineral"),
            _ => !has("mineral"),
        };
        let allergic = p.allergies.contains(&food.id) || food.tags.iter().any(|t| p.allergies.contains(t));
        let raw_unsafe = !s.cooked && food.raw_risk > 0;

        let mut refusals = Vec::new();
        if !species_ok {
            refusals.push("species_diet");
        }
        if food.tags.iter().any(|t| p.forbids.contains(t)) {
            refusals.push("forbidden");
        }
        if allergic {
            refusals.push("allergy");
        }
        if freshness == "spoiled" {
            refusals.push("spoiled");
        }
        if raw_unsafe {
            refusals.push("unsafe_raw");
        }

        let mut hunger = food.hunger;
        if s.cooked {
            hunger = (hunger + hunger * food.cooked_bonus / 100).min(100);
        }
        let mut thirst = food.thirst;
        match freshness {
            "stale" => {
                hunger = hunger * 3 / 4;
                thirst = thirst * 3 / 4;
            }
            "spoiled" => {
                hunger /= 2;
                thirst /= 2;
            }
            _ => {}
        }

        let mut sickness = 0;
        if allergic {
            sickness = sickness.max(40);
        }
        if freshness == "spoiled" {
            sickness = sickness.max(30);
        }
        if raw_unsafe {
            sickness = sickness.max(food.raw_risk);
        }

        let repeats = p.recent.iter().rev().take(MONOTONY_WINDOW).filter(|(f, _)| *f == food.id).count() as u64;
        let liked = food.tags.iter().filter(|t| p.likes.contains(*t)).count() as i64;
        let disliked = food.tags.iter().filter(|t| p.dislikes.contains(*t)).count() as i64;
        let mut joy = food.joy + 5 * liked - 5 * disliked - (3 * repeats as i64).min(15);
        joy -= match freshness {
            "stale" => 3,
            "spoiled" => 10,
            _ => 0,
        };
        let joy = joy.clamp(-50, 50);

        Ok(MealEval { food: food.id.clone(), freshness, refusals, hunger, thirst, joy, sickness, repeats })
    }

    fn op_choose(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "options"], &["hunger_level"])?;
        let being = ident(c::get(args, "being")?)?;
        let level = opt_uint(args, "hunger_level", 0, 100, 50)?;
        let raw = list_bound(Some(c::get(args, "options")?), MAX_OPTIONS)?;
        if raw.is_empty() {
            return Err(c::bad());
        }
        let mut ranked = Vec::new();
        let mut refused = Vec::new();
        for (i, o) in raw.iter().enumerate() {
            let e = self.evaluate(being, &read_serving(o, &[])?)?;
            if e.refusals.is_empty() {
                let score = (e.hunger * level / 100) as i64 + 2 * e.joy;
                ranked.push((score, i, e));
            } else {
                refused
                    .push(obj! { "food" => e.food.as_str(), "index" => i, "refusals" => Value::strings(&e.refusals) });
            }
        }
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.food.cmp(&b.2.food)).then(a.1.cmp(&b.1)));
        let best = ranked.first().map(|r| Value::from(r.1)).unwrap_or(Value::Null);
        let ranked: Vec<Value> = ranked
            .into_iter()
            .map(|(score, i, e)| {
                obj! {
                    "food" => e.food.as_str(),
                    "freshness" => e.freshness,
                    "hunger" => e.hunger,
                    "index" => i,
                    "joy" => e.joy,
                    "score" => score,
                }
            })
            .collect();
        Ok(
            obj! { "being" => being, "best" => best, "ranked" => Value::Array(ranked), "refused" => Value::Array(refused) },
        )
    }

    fn op_eat(&mut self, args: &Value) -> Result<Value> {
        let being = ident(c::get(args, "being")?)?.to_string();
        let s = read_serving(args, &["being", "force"])?;
        let force = opt_bool(args, "force", false)?;
        let e = self.evaluate(&being, &s)?;
        let eaten = e.refusals.is_empty() || force;
        let mut effects = Vec::new();
        if eaten {
            if !self.beings.contains_key(&being) && self.beings.len() >= self.limits.get("beings") {
                return Err(c::bad());
            }
            let max_recent = self.limits.get("recent");
            let p = self.beings.entry(being.clone()).or_default();
            if p.recent.len() >= max_recent {
                p.recent.pop_front();
            }
            p.recent.push_back((e.food.clone(), s.at));
            p.meals += 1;
            let satisfy = |need: &str, amount: u64| {
                obj! {
                    "args" => obj! { "amount" => amount, "being" => being.as_str(), "need" => need },
                    "engine" => "body",
                    "op" => "satisfy",
                }
            };
            if e.hunger > 0 {
                effects.push(satisfy("hunger", e.hunger));
            }
            if e.thirst > 0 {
                effects.push(satisfy("thirst", e.thirst));
            }
            if e.sickness >= SICK_AT {
                effects.push(obj! {
                    "args" => obj! { "being" => being.as_str(), "condition" => "sick" },
                    "engine" => "body",
                    "op" => "set_condition",
                });
            }
            if e.joy != 0 {
                effects.push(obj! {
                    "args" => obj! {
                        "being" => being.as_str(),
                        "reason" => format!("ate {}", self.foods[&e.food].label),
                        "valence_delta" => e.joy,
                    },
                    "engine" => "affect",
                    "op" => "set_mood",
                });
            }
        }
        let mut out = e.to_value(&being);
        if let Value::Object(f) = &mut out {
            f.push(("eaten".into(), Value::Bool(eaten)));
            f.push(("effects".into(), Value::Array(effects)));
            f.push(("forced".into(), Value::Bool(eaten && !e.refusals.is_empty())));
        }
        Ok(out)
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let p = self.beings.get(being).ok_or_else(c::bad)?;
        let recent: Vec<Value> = p.recent.iter().map(|(f, at)| obj! { "at" => *at, "food" => f.as_str() }).collect();
        Ok(obj! {
            "allergies" => strings(&p.allergies),
            "being" => being,
            "diet" => p.diet.as_str(),
            "dislikes" => strings(&p.dislikes),
            "forbids" => strings(&p.forbids),
            "likes" => strings(&p.likes),
            "meals" => p.meals,
            "recent" => Value::Array(recent),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    DietManager::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(d: &mut DietManager, s: &str) -> Result<Value> {
        d.workflow(&parse(s, 1 << 16).unwrap())
    }

    #[test]
    fn village_pack_loads_and_spoils() {
        let mut d = DietManager::new();
        run(&mut d, r#"{"ops":[{"op":"register_food","args":{"pack":"village"}}]}"#).unwrap();
        assert_eq!(d.foods.len(), 12);
        let ev = |made: u64, preserved: bool| {
            let s = Serving { food: "river_trout".into(), cooked: true, preserved, made_at: made, at: 100 };
            d.evaluate("x", &s).unwrap().freshness
        };
        assert_eq!(ev(90, false), "fresh");
        assert_eq!(ev(80, false), "stale");
        assert_eq!(ev(70, false), "spoiled");
        assert_eq!(ev(0, true), "fresh");
    }

    #[test]
    fn monotony_lowers_joy_up_to_fifteen() {
        let mut d = DietManager::new();
        run(&mut d, r#"{"ops":[{"op":"register_food","args":{"pack":"village"}}]}"#).unwrap();
        let eat = r#"{"ops":[{"op":"eat","args":{"being":"b","food":"honey_cake"}}]}"#;
        let mut joys = Vec::new();
        for _ in 0..7 {
            let out = run(&mut d, eat).unwrap();
            joys.push(out.get("results").unwrap().as_array().unwrap()[0].get("joy").cloned().unwrap());
        }
        assert_eq!(joys.first(), Some(&Value::int(10)));
        assert_eq!(joys.last(), Some(&Value::int(-5)));
    }

    #[test]
    fn refusals_need_force_and_bad_profiles_are_refused() {
        let mut d = DietManager::new();
        let out = run(
            &mut d,
            r#"{"ops":[{"op":"register_food","args":{"pack":"village"}},
                {"op":"set_diet","args":{"being":"b","diet":"herbivore"}},
                {"op":"eat","args":{"being":"b","food":"hearty_stew","cooked":true}},
                {"op":"eat","args":{"being":"b","food":"hearty_stew","cooked":true,"force":true}}]}"#,
        )
        .unwrap()
        .to_json();
        assert!(out.contains(r#""eaten":false"#) && out.contains(r#""forced":true"#), "{out}");
        assert!(run(
            &mut d,
            r#"{"ops":[{"op":"set_diet","args":{"being":"b","likes":["fish"],"dislikes":["fish"]}}]}"#
        )
        .is_err());
        assert!(run(&mut d, r#"{"ops":[{"op":"eat","args":{"being":"b","food":"rye_bread","made_at":5,"at":4}}]}"#)
            .is_err());
    }
}
