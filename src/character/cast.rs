//! One bounded original cast pack. Loading validates references; instantiation is the
//! host's acceptance operation. No includes, downloads, model calls or automatic action.
use crate::{
    character::archetype::Blueprint,
    character::life::{get, line, names, num, text, Definition, Setting},
    character::party::AttitudeGates,
    character::routine::Routine,
    checked as c,
    knowledge::location::Locations,
    knowledge::lore::Lore,
    obj,
    packet::request::bounded,
    Result, Value,
};
use std::{io::Read, sync::Arc};
pub const MAX_BYTES: usize = 1_048_576;
#[derive(Clone, Debug)]
pub(crate) struct Character {
    pub id: String,
    pub blueprint: Blueprint,
    pub definition: Arc<Definition>,
    pub routine: Option<Arc<Routine>>,
    pub gates: AttitudeGates,
}
#[derive(Clone, Debug)]
pub struct CastPack {
    pub(crate) value: Value,
    pub(crate) characters: Vec<Character>,
    pub(crate) participants: Vec<String>,
    pub(crate) lore: Arc<Lore>,
    pub(crate) locations: Vec<String>,
}
impl CastPack {
    pub fn parse(s: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(s, MAX_BYTES)?)
    }
    pub fn read(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let mut s = String::new();
        std::fs::File::open(path)
            .map_err(|e| crate::Error::new(e.to_string()))?
            .take(MAX_BYTES as u64 + 1)
            .read_to_string(&mut s)
            .map_err(|e| crate::Error::new(e.to_string()))?;
        Self::parse(&s)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, MAX_BYTES)?;
        c::fields(
            v,
            &[
                "version",
                "id",
                "revision",
                "setting",
                "players",
                "locations",
                "groups",
                "characters",
                "lore",
                "features",
                "propagation",
                "ripple",
                "habits",
            ],
        )?;
        c::uint(get(v, "version")?, 1, 1)?;
        c::name(text(v, "id")?)?;
        c::uint(get(v, "revision")?, 1, u32::MAX as u64)?;
        let setting = Setting::from_value(get(v, "setting")?)?;
        let entities = names(get(get(v, "setting")?, "entities")?, 128)?;
        let players = names(get(v, "players")?, 8)?;
        let chars = c::list(get(v, "characters")?)?;
        if players.is_empty() || chars.is_empty() || chars.len() > 12 {
            return Err(c::bad());
        }
        let mut participants = players.clone();
        for p in &players {
            c::name(p)?;
        }
        for c in chars {
            c::fields(c, &["id", "blueprint", "definition", "routine", "gates", "affordances", "home", "fallbacks"])?;
            let id = text(c, "id")?;
            c::name(id)?;
            if participants.iter().any(|p| p == id) {
                return Err(c::bad());
            }
            participants.push(id.into());
        }
        if participants.len() > 16 || participants.iter().any(|p| !entities.contains(p)) {
            return Err(c::bad());
        }
        let locations_value = Locations::from_value(get(v, "locations")?)?.to_value();
        let locations = c::list(get(&locations_value, "places")?)?
            .iter()
            .map(|p| text(p, "id").map(str::to_owned))
            .collect::<Result<Vec<_>>>()?;
        if locations.is_empty()
            || locations.len() > 32
            || locations.iter().any(|p| !entities.contains(p) || participants.contains(p))
        {
            return Err(c::bad());
        }
        let groups = c::list(get(v, "groups")?)?;
        if groups.len() > 16 {
            return Err(c::bad());
        }
        let mut group_ids = Vec::new();
        for g in groups {
            c::fields(g, &["id", "members"])?;
            let id = text(g, "id")?;
            c::name(id)?;
            let members = names(get(g, "members")?, 16)?;
            if group_ids.iter().any(|p| p == id)
                || !entities.iter().any(|p| p == id)
                || participants.iter().chain(&locations).any(|p| p == id)
                || members.is_empty()
                || members.iter().any(|p| !participants.contains(p))
            {
                return Err(c::bad());
            }
            group_ids.push(id.to_owned());
        }
        let lore = Arc::new(Lore::from_value(get(v, "lore")?)?);
        let lore_value = lore.to_value();
        let lore_entries = c::list(get(&lore_value, "entries")?)?;
        for e in lore_entries {
            let known = names(get(e, "known_by")?, 16)?;
            if !entities.iter().any(|p| p == text(e, "subject").unwrap_or(""))
                || known != ["*"] && known.iter().any(|p| !participants.contains(p))
            {
                return Err(c::bad());
            }
        }
        let mut characters = Vec::new();
        for ch in chars {
            let blueprint = Blueprint::from_value(get(ch, "blueprint")?)?;
            let d = get(ch, "definition")?;
            let definition = Arc::new(Definition::draft(d, &setting)?);
            for f in c::list(get(d, "facets")?)?.iter().chain(c::list(get(d, "attitudes")?)?) {
                if names(get(f, "audience")?, 16)?.iter().any(|p| !participants.contains(p)) {
                    return Err(c::bad());
                }
            }
            for a in c::list(get(d, "attitudes")?)? {
                if matches!(
                    text(a, "kind")?,
                    "trust" | "resentment" | "affection" | "familiarity" | "obligation" | "disagreement"
                ) && !participants.iter().any(|p| p == text(a, "subject").unwrap_or(""))
                {
                    return Err(c::bad());
                }
            }
            if !locations.iter().any(|p| p == text(ch, "home").unwrap_or("")) {
                return Err(c::bad());
            }
            let routine = if get(ch, "routine")? == &Value::Null {
                None
            } else {
                let value = get(ch, "routine")?;
                let routine = Arc::new(Routine::from_value(value)?);
                if value.get("calendar") != get(v, "setting")?.get("calendar")
                    || c::list(get(value, "activities")?)?.iter().any(|a| {
                        !locations.iter().any(|p| Some(p.as_str()) == a.get("location").and_then(Value::as_str))
                    })
                {
                    return Err(c::bad());
                }
                Some(routine)
            };
            if blueprint.has("routine") != routine.is_some() {
                return Err(c::bad());
            }
            let affordances = c::list(get(ch, "affordances")?)?;
            if affordances.len() > 32 {
                return Err(c::bad());
            }
            for (i, a) in affordances.iter().enumerate() {
                c::fields(a, &["id", "text", "audience"])?;
                c::name(text(a, "id")?)?;
                line(text(a, "text")?)?;
                if names(get(a, "audience")?, 16)?.iter().any(|p| !participants.contains(p))
                    || affordances[..i].iter().any(|o| o.get("id") == a.get("id"))
                {
                    return Err(c::bad());
                }
            }
            let gates = AttitudeGates::from_value(get(ch, "gates")?)?;
            for g in c::list(get(ch, "gates")?)? {
                let pool = match text(g, "kind")? {
                    "lore" => lore_entries,
                    "facet" => c::list(get(d, "facets")?)?,
                    _ => affordances,
                };
                if !blueprint.has("life") || !pool.iter().any(|p| p.get("id") == g.get("id")) {
                    return Err(c::bad());
                }
                for clause in c::list(get(g, "clauses")?)? {
                    let attrs = c::list(get(d, "attitudes")?)?
                        .iter()
                        .filter(|a| a.get("kind") == clause.get("attitude"))
                        .collect::<Vec<_>>();
                    if attrs.is_empty()
                        || attrs
                            .iter()
                            .enumerate()
                            .any(|(i, a)| attrs[..i].iter().any(|old| old.get("subject") == a.get("subject")))
                    {
                        return Err(c::bad());
                    }
                }
            }
            let fallbacks = get(ch, "fallbacks")?;
            c::fields(fallbacks, &["calm", "busy", "tense", "emergency", "unknown"])?;
            for (_, s) in fallbacks.as_object().ok_or_else(c::bad)? {
                super::living::validate_line(c::text(s)?)?;
            }
            characters.push(Character { id: text(ch, "id")?.into(), blueprint, definition, routine, gates });
        }
        let features = get(v, "features")?;
        c::fields(features, &["rumors", "ripples", "habits", "atmosphere"])?;
        for (_, b) in features.as_object().ok_or_else(c::bad)? {
            c::boolean(b)?;
        }
        if features.get("ripples") == Some(&true.into()) && features.get("rumors") != Some(&true.into()) {
            return Err(c::bad());
        }
        let p = get(v, "propagation")?;
        c::fields(p, &["significance", "trust", "max_hops", "per_tick", "checks"])?;
        c::uint(get(p, "significance")?, 0, 100)?;
        crate::character::life::signed(get(p, "trust")?, 100)?;
        c::uint(get(p, "max_hops")?, 0, 4)?;
        c::uint(get(p, "per_tick")?, 0, 16)?;
        c::uint(get(p, "checks")?, 0, 1024)?;
        let r = get(v, "ripple")?;
        c::fields(r, &["numerator", "denominator", "max_delta", "kind", "groups", "reason"])?;
        let denominator = c::uint(get(r, "denominator")?, 1, 100)?;
        c::uint(get(r, "numerator")?, 0, denominator)?;
        c::uint(get(r, "max_delta")?, 0, 5)?;
        c::name(text(r, "kind")?)?;
        line(text(r, "reason")?)?;
        if names(get(r, "groups")?, 16)?.iter().any(|p| !group_ids.contains(p)) {
            return Err(c::bad());
        }
        if features.get("ripples") == Some(&true.into())
            && (!chars.iter().any(|ch| {
                c::list(get(get(ch, "definition").expect("definition"), "attitudes").expect("attitudes"))
                    .expect("attitudes")
                    .iter()
                    .any(|a| a.get("kind") == r.get("kind"))
            }) || names(get(r, "groups")?, 16)?.is_empty())
        {
            return Err(c::bad());
        }
        let habits = c::list(get(v, "habits")?)?;
        if habits.len() > 16 {
            return Err(c::bad());
        }
        for (i, h) in habits.iter().enumerate() {
            c::fields(h, &["id", "counter", "threshold", "text", "tags"])?;
            c::name(text(h, "id")?)?;
            line(text(h, "text")?)?;
            names(get(h, "tags")?, 8)?;
            c::uint(get(h, "threshold")?, 1, 1_000_000)?;
            if !matches!(text(h, "counter")?, "fulfilled" | "broken" | "interrupted")
                || habits[..i].iter().any(|o| o.get("id") == h.get("id"))
            {
                return Err(c::bad());
            }
        }
        Ok(Self { value: v.clone(), characters, participants, lore, locations })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn summary(&self) -> Value {
        let groups = c::list(get(&self.value, "groups").expect("groups")).expect("groups");
        let chars = c::list(get(&self.value, "characters").expect("characters")).expect("characters");
        obj! {"version"=>1,"id"=>get(&self.value,"id").expect("id").clone(),"revision"=>num(&self.value,"revision").expect("revision"),"participants"=>self.participants.clone(),"characters"=>self.characters.iter().zip(chars).map(|(c,v)|obj!{"id"=>c.id.clone(),"home"=>get(v,"home").expect("home").clone(),"blueprint"=>c.blueprint.to_value(),"groups"=>groups.iter().filter(|g|names(get(g,"members").expect("members"),16).expect("members").contains(&c.id)).map(|g|get(g,"id").expect("id").clone()).collect::<Vec<_>>()}).collect::<Vec<_>>(),"locations"=>self.locations.clone()}
    }
}
