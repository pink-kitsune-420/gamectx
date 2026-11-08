//! Shared world definitions for roleplay: places (with spots, ambience and norms),
//! entrances, factions, events and quests. A codex is fixed authored content; live state
//! such as an entrance's status or a quest's stage arrives with each turn.
//!
//! ```
//! use gamectx::knowledge::codex::Codex;
//! let codex = Codex::parse(r#"{"version":1,
//!   "places":[{"id":"bakery","label":"the bakery"},{"id":"street","label":"the street"}],
//!   "entrances":[{"id":"front","label":"the front door","sides":[
//!     {"place":"bakery","description":"A green door.","travel":{"action":"go","target":"room:street"}},
//!     {"place":"street","description":"The bakery's door."}]}]}"#)?;
//! assert_eq!(codex.place_label("street"), Some("the street"));
//! # Ok::<(), gamectx::Error>(())
//! ```
use crate::character::roleplay::engine::VARS;
use crate::data::conditions::{Conditions, Vars};
use crate::knowledge::props::{decision, ident, line, normalize};
use crate::{checked as c, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Host-set limits on a codex (defaults are the earlier fixed bounds): `places`, `entrances`,
/// `factions`, `events`, `quests` and the document's serialized `bytes`. Per-item lists
/// (spots, norms, ambience, relations, stages, aliases) keep their fixed bounds.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "places", default: 128, ceiling: 65_536 },
    crate::character::limits::Limit { name: "entrances", default: 256, ceiling: 262_144 },
    crate::character::limits::Limit { name: "factions", default: 64, ceiling: 4096 },
    crate::character::limits::Limit { name: "events", default: 128, ceiling: 65_536 },
    crate::character::limits::Limit { name: "quests", default: 128, ceiling: 65_536 },
    crate::character::limits::Limit { name: "bytes", default: 2_097_152, ceiling: 268_435_456 },
];
crate::character::limits::module_limits!(CodexLimits, LIMITS);

pub(crate) const STATUSES: [&str; 5] = ["open", "closed", "locked", "blocked", "hidden"];
const STANCES: [&str; 5] = ["allied", "friendly", "neutral", "wary", "hostile"];
const CARRIES: [&str; 3] = ["none", "when_open", "always"];
const AMBIENCE: [&str; 6] = ["smell", "light", "sound", "temperature", "floor", "ceiling"];

pub(crate) fn keys(v: &Value, required: &[&str], optional: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(c::bad)?;
    if required.iter().any(|k| v.get(k).is_none())
        || rows.iter().any(|(k, _)| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(c::bad());
    }
    Ok(())
}
pub(crate) fn list(v: Option<&Value>, lo: usize, hi: usize) -> Result<&[Value]> {
    let xs = match v {
        None => &[][..],
        Some(v) => c::list(v)?,
    };
    if xs.len() < lo || xs.len() > hi {
        return Err(c::bad());
    }
    Ok(xs)
}
pub(crate) fn unique_ids(items: &[Value]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for x in items {
        if !seen.insert(ident(c::get(x, "id")?)?) {
            return Err(c::bad());
        }
    }
    Ok(())
}
/// [`names`] with a set of names already used, for long lists.
fn names_in(item: &Value, label_key: &str, seen: &mut BTreeSet<String>) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for x in std::iter::once(c::get(item, label_key)?).chain(list(item.get("aliases"), 0, 8)?) {
        let q = normalize(line(x, 48, usize::MAX)?);
        if q.is_empty() || !seen.insert(q.clone()) {
            return Err(c::bad());
        }
        out.push(q);
    }
    Ok(out)
}
/// Label (or `label_key`) plus up to 8 aliases: ≤48 chars, normalized, unique within `seen`.
pub(crate) fn names(item: &Value, label_key: &str, seen: &mut Vec<String>) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for x in std::iter::once(c::get(item, label_key)?).chain(list(item.get("aliases"), 0, 8)?) {
        let q = normalize(line(x, 48, usize::MAX)?);
        if q.is_empty() || seen.contains(&q) {
            return Err(c::bad());
        }
        seen.push(q.clone());
        out.push(q);
    }
    Ok(out)
}
/// 1–`hi` phrases (≤128 bytes), normalized, unique within `seen`.
pub(crate) fn phrases(v: &Value, seen: &mut Vec<String>, hi: usize) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for p in list(Some(v), 1, hi)? {
        let q = normalize(line(p, 128, 128)?);
        if q.is_empty() || seen.contains(&q) {
            return Err(c::bad());
        }
        seen.push(q.clone());
        out.push(q);
    }
    Ok(out)
}
pub(crate) fn opt_text<'a>(v: &'a Value, k: &str, chars: usize) -> Result<Option<&'a str>> {
    v.get(k).map(|x| line(x, chars, usize::MAX)).transpose()
}

/// Validated world definitions (`{"version":1, places?, entrances?, factions?, events?, quests?,
/// place_kinds?}`). See [`crate::knowledge::places`] for place kinds, nesting and links.
#[derive(Clone, Debug)]
pub struct Codex {
    value: Value,
    ambience: Vec<Vec<Conditions>>,
    names: [Vec<Vec<String>>; 5],
    /// Position of each id in places, entrances, factions, events and quests.
    index: [BTreeMap<String, usize>; 5],
    /// The place graph [`nearby`](crate::knowledge::places::nearby) walks, built on first use.
    pub(crate) graph: std::sync::OnceLock<crate::knowledge::places::Graph>,
}
impl Default for Codex {
    fn default() -> Self {
        Self::from_value(&crate::obj! {"version"=>1}).expect("empty codex")
    }
}
impl Codex {
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(input, 2_097_152)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        Self::from_value_within(v, &CodexLimits::default())
    }
    /// [`from_value`](Self::from_value) with host-set [limits](LIMITS).
    pub fn from_value_within(v: &Value, l: &crate::character::limits::Limits) -> Result<Self> {
        crate::packet::request::bounded(v, l.get("bytes"))?;
        keys(
            v,
            &["version"],
            &[
                "places",
                "entrances",
                "factions",
                "events",
                "quests",
                "place_kinds",
                "timeline",
                "things",
                "recipes",
                "communities",
                "species",
            ],
        )?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let places = list(v.get("places"), 0, l.get("places"))?;
        for p in places {
            keys(
                p,
                &["id", "label"],
                &["aliases", "spots", "ambience", "norms", "exterior", "kind", "within", "tags", "links"],
            )?;
            if let Some(e) = p.get("exterior").filter(|x| *x != &Value::Null) {
                c::boolean(e)?;
            }
        }
        unique_ids(places)?;
        let (mut seen, mut place_names, mut ambience) = (BTreeSet::new(), Vec::new(), Vec::new());
        for p in places {
            place_names.push(names_in(p, "label", &mut seen)?);
            let spots = list(p.get("spots"), 0, 16)?;
            for s in spots {
                keys(
                    s,
                    &["id", "label", "description"],
                    &[
                        "aliases",
                        "dimensions",
                        "coords",
                        "mass",
                        "state",
                        "material",
                        "color",
                        "texture",
                        "tags",
                        "attributes",
                    ],
                )?;
                if let Some(d) = s.get("dimensions").filter(|x| *x != &Value::Null) {
                    crate::character::sheet::dimensions_val(d)?;
                }
                if let Some(cr) = s.get("coords").filter(|x| *x != &Value::Null) {
                    crate::character::sheet::coords(cr)?;
                }
                crate::character::sheet::validate_matter(s)?;
                if let Some(tg) = s.get("tags").filter(|x| *x != &Value::Null) {
                    crate::character::sheet::validate_tags(tg)?;
                }
                if let Some(att) = s.get("attributes").filter(|x| *x != &Value::Null) {
                    crate::character::sheet::validate_attributes(att)?;
                }
                line(c::get(s, "description")?, 240, usize::MAX)?;
            }
            unique_ids(spots)?;
            let mut spot_names = Vec::new();
            for s in spots {
                names(s, "label", &mut spot_names)?;
            }
            let mut conds = Vec::new();
            for a in list(p.get("ambience"), 0, 8)? {
                keys(a, &[], &["when", "smell", "light", "sound", "temperature", "floor", "ceiling"])?;
                conds.push(Conditions::field(a, "when", &VARS)?);
                if !AMBIENCE.iter().any(|k| a.get(k).is_some()) {
                    return Err(c::bad());
                }
                for k in AMBIENCE {
                    opt_text(a, k, 160)?;
                }
            }
            ambience.push(conds);
            let norms = list(p.get("norms"), 0, 16)?;
            for n in norms {
                keys(n, &["id", "actions", "reaction"], &[])?;
                line(c::get(n, "reaction")?, 240, usize::MAX)?;
                for d in list(n.get("actions"), 1, 8)? {
                    if d == &Value::Null {
                        return Err(c::bad());
                    }
                    decision(d)?;
                }
            }
            unique_ids(norms)?;
        }
        let place_ids = places.iter().filter_map(|p| p.get("id").and_then(Value::as_str)).collect::<BTreeSet<_>>();
        let entrances = list(v.get("entrances"), 0, l.get("entrances"))?;
        for e in entrances {
            keys(e, &["id", "label", "sides"], &["kind", "aliases", "passage", "carries", "distance"])?;
        }
        unique_ids(entrances)?;
        let (mut seen, mut entrance_names) = (BTreeSet::new(), Vec::new());
        for e in entrances {
            entrance_names.push(names_in(e, "label", &mut seen)?);
            if let Some(k) = e.get("kind") {
                ident(k)?;
            }
            if !["both", "one_way"].contains(&e.get("passage").map_or(Ok("both"), c::text)?) {
                return Err(c::bad());
            }
            if let Some(carries) = e.get("carries") {
                keys(carries, &[], &["sound", "sight"])?;
                for (_, mode) in carries.as_object().unwrap_or(&[]) {
                    if !CARRIES.contains(&c::text(mode)?) {
                        return Err(c::bad());
                    }
                }
            }
            let sides = list(e.get("sides"), 1, 2)?;
            for s in sides {
                keys(s, &["place", "description"], &["travel"])?;
                if !c::get(s, "place")?.as_str().is_some_and(|p| place_ids.contains(p)) {
                    return Err(c::bad());
                }
                line(c::get(s, "description")?, 240, usize::MAX)?;
                decision(s.get("travel").unwrap_or(&Value::Null))?;
            }
            if sides.len() == 2 && sides[0].get("place") == sides[1].get("place") {
                return Err(c::bad());
            }
            if let Some(d) = e.get("distance") {
                c::uint(d, 0, 1_000_000_000)?;
            }
        }
        crate::knowledge::places::validate(v, places)?;
        let factions = list(v.get("factions"), 0, l.get("factions"))?;
        for f in factions {
            keys(f, &["id", "label", "description"], &["aliases", "relations"])?;
            line(c::get(f, "description")?, 240, usize::MAX)?;
        }
        unique_ids(factions)?;
        let (mut seen, mut faction_names) = (BTreeSet::new(), Vec::new());
        let fids = factions.iter().filter_map(|f| f.get("id").and_then(Value::as_str)).collect::<BTreeSet<_>>();
        for f in factions {
            faction_names.push(names_in(f, "label", &mut seen)?);
            let mut others = Vec::new();
            for r in list(f.get("relations"), 0, 32)? {
                keys(r, &["faction", "stance"], &[])?;
                let other = c::get(r, "faction")?;
                if !other.as_str().is_some_and(|o| fids.contains(o))
                    || Some(other) == f.get("id")
                    || others.contains(&other)
                    || !STANCES.contains(&c::text(c::get(r, "stance")?)?)
                {
                    return Err(c::bad());
                }
                others.push(other);
            }
        }
        let events = list(v.get("events"), 0, l.get("events"))?;
        for e in events {
            keys(e, &["id", "label", "description", "start", "end"], &["aliases", "place"])?;
            line(c::get(e, "description")?, 240, usize::MAX)?;
            let start = c::uint(c::get(e, "start")?, 0, u32::MAX as u64)?;
            if c::uint(c::get(e, "end")?, 0, u32::MAX as u64)? < start {
                return Err(c::bad());
            }
            if let Some(p) = e.get("place") {
                if !p.as_str().is_some_and(|p| place_ids.contains(p)) {
                    return Err(c::bad());
                }
            }
        }
        unique_ids(events)?;
        let (mut seen, mut event_names) = (BTreeSet::new(), Vec::new());
        for e in events {
            event_names.push(names_in(e, "label", &mut seen)?);
        }
        let quests = list(v.get("quests"), 0, l.get("quests"))?;
        for q in quests {
            keys(q, &["id", "label", "stages"], &["aliases", "givers"])?;
            for g in list(q.get("givers"), 0, 16)? {
                ident(g)?;
            }
            let stages = list(q.get("stages"), 1, 32)?;
            for s in stages {
                keys(s, &["id", "description"], &[])?;
                line(c::get(s, "description")?, 240, usize::MAX)?;
            }
            unique_ids(stages)?;
        }
        unique_ids(quests)?;
        let (mut seen, mut quest_names) = (BTreeSet::new(), Vec::new());
        for q in quests {
            quest_names.push(names_in(q, "label", &mut seen)?);
        }
        let index = [places, entrances, factions, events, quests].map(|xs| {
            xs.iter()
                .enumerate()
                .filter_map(|(i, x)| x.get("id").and_then(Value::as_str).map(|id| (id.to_string(), i)))
                .collect()
        });
        Ok(Self {
            value: v.clone(),
            ambience,
            names: [place_names, entrance_names, faction_names, event_names, quest_names],
            index,
            graph: std::sync::OnceLock::new(),
        })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    fn all(&self, k: &str) -> &[Value] {
        self.value.get(k).and_then(Value::as_array).unwrap_or(&[])
    }
    fn find<'a>(&'a self, k: &str, id: &str) -> Option<&'a Value> {
        self.all(k).get(self.position(k, id)?)
    }
    /// Index of the item with this id in `kind` (`places`, `entrances`, `factions`, `events`
    /// or `quests`).
    pub fn position(&self, kind: &str, id: &str) -> Option<usize> {
        let k = ["places", "entrances", "factions", "events", "quests"].iter().position(|x| *x == kind)?;
        self.index[k].get(id).copied()
    }
    pub fn places(&self) -> &[Value] {
        self.all("places")
    }
    /// The codex's own `place_kinds` (built-in kinds are in [`crate::knowledge::places`]).
    pub fn place_kinds(&self) -> &[Value] {
        self.all("place_kinds")
    }
    pub fn place(&self, id: &str) -> Option<&Value> {
        self.find("places", id)
    }
    /// Where a character in a place is, what they can reach and what is around; see
    /// [`crate::knowledge::places::nearby`].
    pub fn nearby(&self, query: &Value) -> Result<Value> {
        crate::knowledge::places::nearby(self, query)
    }
    pub fn place_label(&self, id: &str) -> Option<&str> {
        self.place(id)?.get("label")?.as_str()
    }
    pub fn entrances(&self) -> &[Value] {
        self.all("entrances")
    }
    pub fn entrance(&self, id: &str) -> Option<&Value> {
        self.find("entrances", id)
    }
    pub fn factions(&self) -> &[Value] {
        self.all("factions")
    }
    pub fn faction(&self, id: &str) -> Option<&Value> {
        self.find("factions", id)
    }
    pub fn events(&self) -> &[Value] {
        self.all("events")
    }
    pub fn recipes(&self) -> &[Value] {
        self.all("recipes")
    }
    pub fn communities(&self) -> &[Value] {
        self.all("communities")
    }
    pub fn timeline(&self) -> &[Value] {
        self.all("timeline")
    }
    pub fn quests(&self) -> &[Value] {
        self.all("quests")
    }
    pub fn quest(&self, id: &str) -> Option<&Value> {
        self.find("quests", id)
    }
    /// Normalized names (label then aliases) of the `i`th item of `kind`
    /// (`places`, `entrances`, `factions`, `events` or `quests`).
    pub fn names(&self, kind: &str, i: usize) -> &[String] {
        let k = ["places", "entrances", "factions", "events", "quests"].iter().position(|x| *x == kind);
        k.and_then(|k| self.names[k].get(i)).map_or(&[], Vec::as_slice)
    }
    /// The first ambience entry of a place that has `key` (`smell`, `light`, `sound`,
    /// `temperature`) and whose conditions hold.
    pub fn ambience(&self, place: &str, key: &str, vars: &Vars) -> Option<&str> {
        let i = self.position("places", place)?;
        let entries = self.places()[i].get("ambience")?.as_array()?;
        entries.iter().zip(&self.ambience[i]).find(|(a, w)| a.get(key).is_some() && w.holds(vars))?.0.get(key)?.as_str()
    }
}
