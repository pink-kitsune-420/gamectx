//! A character sheet: persona, speech style, services, goals, needs, relationships,
//! faction memberships, secrets and opinions. Fixed authored content that the roleplay
//! engine answers from; live values (need levels, host flags) arrive with each turn.
//!
//! ```
//! use gamectx::character::sheet::CharacterSheet;
//! use gamectx::knowledge::codex::Codex;
//! let sheet = CharacterSheet::parse(r#"{"version":1,
//!   "persona":{"role":"the town baker"},
//!   "secrets":[{"id":"debt","phrases":["debt","owe"],"when":{"trust":{"min":50}},
//!     "reveal":"I owe the mill for three months of flour.","cover":"My accounts are fine."}]}"#,
//!   &Codex::default())?;
//! assert_eq!(sheet.role(), Some("the town baker"));
//! # Ok::<(), gamectx::Error>(())
//! ```
use crate::character::roleplay::engine::VARS;
use crate::data::conditions::Conditions;
use crate::knowledge::codex::{keys, list, names, opt_text, phrases, unique_ids, Codex};
use crate::knowledge::props::{decision, ident, line, normalize};
use crate::{checked as c, Result, Value};

/// A validated character sheet (`{"version":1, persona?, speech?, services?, goals?, needs?,
/// relationships?, factions?, secrets?, opinions?}`).
#[derive(Clone, Debug)]
pub struct CharacterSheet {
    value: Value,
    pub(crate) services: Vec<(Vec<String>, Conditions)>,
    pub(crate) goals: Vec<Conditions>,
    pub(crate) relationship_names: Vec<Vec<String>>,
    pub(crate) secrets: Vec<(Vec<String>, Conditions)>,
    pub(crate) opinions: Vec<(Vec<String>, Conditions)>,
    pub(crate) avoid: Vec<String>,
}
impl Default for CharacterSheet {
    fn default() -> Self {
        Self::from_value(&crate::obj! {"version"=>1}, &Codex::default()).expect("empty sheet")
    }
}
impl CharacterSheet {
    pub fn parse(input: &str, codex: &Codex) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(input, 1_048_576)?, codex)
    }
    /// Validate a sheet; faction memberships must name factions in `codex`.
    pub fn from_value(v: &Value, codex: &Codex) -> Result<Self> {
        crate::packet::request::bounded(v, 1_048_576)?;
        keys(
            v,
            &["version"],
            &[
                "persona",
                "speech",
                "services",
                "goals",
                "needs",
                "relationships",
                "factions",
                "secrets",
                "opinions",
                "features",
                "clothing",
                "state",
                "sensory",
                "habits",
                "tags",
                "attributes",
                "bio",
            ],
        )?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let empty = crate::obj! {};
        let p = v.get("persona").unwrap_or(&empty);
        keys(p, &[], &["pronouns", "role", "summary", "appearance", "values", "fears", "desires"])?;
        for (k, n) in [("pronouns", 32), ("role", 80), ("summary", 240), ("appearance", 480)] {
            opt_text(p, k, n)?;
        }
        for k in ["values", "fears", "desires"] {
            for x in list(p.get(k), 0, 8)? {
                line(x, 80, usize::MAX)?;
            }
        }
        let s = v.get("speech").unwrap_or(&empty);
        keys(s, &[], &["formality", "verbosity", "quirks", "catchphrases", "avoid_words"])?;
        if !["casual", "neutral", "formal"].contains(&s.get("formality").map_or(Ok("neutral"), c::text)?)
            || !["terse", "normal", "chatty"].contains(&s.get("verbosity").map_or(Ok("normal"), c::text)?)
        {
            return Err(c::bad());
        }
        for k in ["quirks", "catchphrases"] {
            for x in list(s.get(k), 0, 8)? {
                line(x, 120, usize::MAX)?;
            }
        }
        let mut avoid = Vec::new();
        for x in list(s.get("avoid_words"), 0, 32)? {
            let w = normalize(line(x, 40, usize::MAX)?);
            if w.is_empty() {
                return Err(c::bad());
            }
            avoid.push(w);
        }
        let mut seen = Vec::new();
        let mut services = Vec::new();
        let given = list(v.get("services"), 0, 32)?;
        for x in given {
            keys(x, &["id", "label", "description"], &["phrases", "decision", "when"])?;
            line(c::get(x, "label")?, 48, usize::MAX)?;
            line(c::get(x, "description")?, 240, usize::MAX)?;
            let ps = match x.get("phrases") {
                Some(p) => phrases(p, &mut seen, 8)?,
                None => Vec::new(),
            };
            decision(x.get("decision").unwrap_or(&Value::Null))?;
            services.push((ps, Conditions::field(x, "when", &VARS)?));
        }
        unique_ids(given)?;
        let mut goals = Vec::new();
        let given = list(v.get("goals"), 0, 16)?;
        for g in given {
            keys(g, &["id", "description", "priority"], &["when"])?;
            line(c::get(g, "description")?, 240, usize::MAX)?;
            c::uint(c::get(g, "priority")?, 0, 100)?;
            goals.push(Conditions::field(g, "when", &VARS)?);
        }
        unique_ids(given)?;
        let given = list(v.get("needs"), 0, 16)?;
        for n in given {
            keys(n, &["id", "label", "threshold"], &[])?;
            line(c::get(n, "label")?, 48, usize::MAX)?;
            c::uint(c::get(n, "threshold")?, 0, 100)?;
        }
        unique_ids(given)?;
        let (mut seen, mut withs, mut relationship_names) = (Vec::new(), Vec::new(), Vec::new());
        for r in list(v.get("relationships"), 0, 64)? {
            keys(r, &["with", "name", "kind"], &["aliases", "description"])?;
            let with = ident(c::get(r, "with")?)?;
            if withs.contains(&with) {
                return Err(c::bad());
            }
            withs.push(with);
            line(c::get(r, "kind")?, 32, usize::MAX)?;
            relationship_names.push(names(r, "name", &mut seen)?);
            opt_text(r, "description", 240)?;
        }
        let mut member = Vec::new();
        for m in list(v.get("factions"), 0, 16)? {
            keys(m, &["faction"], &["role"])?;
            let f = c::text(c::get(m, "faction")?)?;
            if codex.faction(f).is_none() || member.contains(&f) {
                return Err(c::bad());
            }
            member.push(f);
            opt_text(m, "role", 48)?;
        }
        let (mut seen, mut secrets) = (Vec::new(), Vec::new());
        let given = list(v.get("secrets"), 0, 32)?;
        for x in given {
            keys(x, &["id", "phrases", "reveal", "when"], &["cover"])?;
            let ps = phrases(c::get(x, "phrases")?, &mut seen, 16)?;
            line(c::get(x, "reveal")?, 240, usize::MAX)?;
            let when = Conditions::from_value(c::get(x, "when")?, &VARS)?;
            if let Some(cover) = x.get("cover").filter(|c| *c != &Value::Null) {
                line(cover, 240, usize::MAX)?;
            }
            secrets.push((ps, when));
        }
        unique_ids(given)?;
        let (mut seen, mut opinions) = (Vec::new(), Vec::new());
        let given = list(v.get("opinions"), 0, 64)?;
        for x in given {
            keys(x, &["id", "phrases", "variants"], &["when"])?;
            let ps = phrases(c::get(x, "phrases")?, &mut seen, 16)?;
            for t in list(x.get("variants"), 1, 8)? {
                line(t, 240, usize::MAX)?;
            }
            opinions.push((ps, Conditions::field(x, "when", &VARS)?));
        }
        unique_ids(given)?;
        if let Some(f) = v.get("features").filter(|x| *x != &Value::Null) {
            validate_features(f)?;
        }
        if let Some(cl) = v.get("clothing").filter(|x| *x != &Value::Null) {
            validate_clothing(cl)?;
        }
        if let Some(st) = v.get("state").filter(|x| *x != &Value::Null) {
            validate_char_state(st)?;
        }
        if let Some(sn) = v.get("sensory").filter(|x| *x != &Value::Null) {
            validate_sensory(sn)?;
        }
        if let Some(h) = v.get("habits").filter(|x| *x != &Value::Null) {
            validate_habits(h)?;
        }
        if let Some(tg) = v.get("tags").filter(|x| *x != &Value::Null) {
            validate_tags(tg)?;
        }
        if let Some(att) = v.get("attributes").filter(|x| *x != &Value::Null) {
            validate_attributes(att)?;
        }
        if let Some(b) = v.get("bio").filter(|x| *x != &Value::Null) {
            validate_bio(b)?;
        }
        Ok(Self { value: v.clone(), services, goals, relationship_names, secrets, opinions, avoid })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub(crate) fn all(&self, k: &str) -> &[Value] {
        self.value.get(k).and_then(Value::as_array).unwrap_or(&[])
    }
    /// A persona field such as `role`, `summary`, `pronouns` or `appearance`.
    pub fn persona(&self, k: &str) -> Option<&str> {
        self.value.get("persona")?.get(k)?.as_str()
    }
    pub fn role(&self) -> Option<&str> {
        self.persona("role")
    }
    /// The factions this character belongs to, in sheet order.
    pub fn memberships(&self) -> Vec<&str> {
        self.all("factions").iter().filter_map(|m| m.get("faction")?.as_str()).collect()
    }
    /// Quirks listed in the character's speech style.
    pub fn quirks(&self) -> Vec<&str> {
        self.value
            .get("speech")
            .and_then(|s| s.get("quirks"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }
    /// Facial and physical features (eyes, hair, skin, scars, height, build).
    pub fn features(&self) -> Option<&Value> {
        self.value.get("features")
    }
    /// Clothing items worn by the character.
    pub fn clothing(&self) -> Vec<&str> {
        self.all("clothing").iter().filter_map(Value::as_str).collect()
    }
    /// Current living state (posture, held items, gaze, activity, awareness, expression).
    pub fn char_state(&self) -> Option<&Value> {
        self.value.get("state")
    }
    /// Sensory details (scent, voice tone).
    pub fn sensory(&self) -> Option<&Value> {
        self.value.get("sensory")
    }
    /// Behavioral habits and micro-fidgets.
    pub fn habits(&self) -> Vec<&str> {
        self.all("habits").iter().filter_map(Value::as_str).collect()
    }
    /// Character tags.
    pub fn tags(&self) -> Vec<&str> {
        self.all("tags").iter().filter_map(Value::as_str).collect()
    }
    /// Open-ended custom attributes.
    /// The optional `bio` section: species, age, birthday, profession, anatomy, health,
    /// skills, knowledge and family.
    pub fn bio(&self) -> Option<&Value> {
        self.value.get("bio").filter(|x| **x != Value::Null)
    }
    pub fn attributes(&self) -> Option<&Value> {
        self.value.get("attributes")
    }
}

pub(crate) fn validate_features(v: &Value) -> Result<()> {
    keys(v, &[], &["eyes", "hair", "skin", "scars", "height", "build"])?;
    for k in ["eyes", "hair", "skin", "build"] {
        opt_text(v, k, 48)?;
    }
    for s in list(v.get("scars"), 0, 8)? {
        line(s, 48, usize::MAX)?;
    }
    if let Some(h) = v.get("height").filter(|x| *x != &Value::Null) {
        match h {
            Value::Number(s) => {
                s.parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0 && *n <= 100.0).ok_or_else(c::bad)?;
            }
            _ => return Err(c::bad()),
        }
    }
    Ok(())
}

pub(crate) fn coords(v: &Value) -> Result<Vec<f64>> {
    let list = c::list(v)?;
    if list.len() < 2 || list.len() > 3 {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for x in list {
        let n = match x {
            Value::Number(s) => s.parse::<f64>().ok().filter(|n| n.is_finite()).ok_or_else(c::bad)?,
            _ => return Err(c::bad()),
        };
        if !(-1_000_000.0..=1_000_000.0).contains(&n) {
            return Err(c::bad());
        }
        out.push(n);
    }
    Ok(out)
}

pub(crate) fn dimensions_val(v: &Value) -> Result<Vec<f64>> {
    if let Some(list) = v.as_array() {
        if list.len() != 3 {
            return Err(c::bad());
        }
        let mut out = Vec::new();
        for x in list {
            let n = match x {
                Value::Number(s) => {
                    s.parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0 && *n <= 100_000.0).ok_or_else(c::bad)?
                }
                _ => return Err(c::bad()),
            };
            out.push(n);
        }
        return Ok(out);
    }
    if let Some(map) = v.as_object() {
        if map.len() != 3 {
            return Err(c::bad());
        }
        let get_dim = |k: &str| -> Result<f64> {
            match map.iter().find(|(key, _)| key.as_str() == k).map(|(_, v)| v) {
                Some(Value::Number(s)) => {
                    s.parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0 && *n <= 100_000.0).ok_or_else(c::bad)
                }
                _ => Err(c::bad()),
            }
        };
        return Ok(vec![get_dim("width")?, get_dim("length")?, get_dim("height")?]);
    }
    Err(c::bad())
}

pub(crate) fn validate_clothing(v: &Value) -> Result<()> {
    for item in list(Some(v), 0, 16)? {
        line(item, 48, usize::MAX)?;
    }
    Ok(())
}

pub(crate) fn validate_char_state(v: &Value) -> Result<()> {
    keys(v, &[], &["posture", "held", "gaze", "activity", "awareness", "exposure", "expression"])?;
    for k in ["posture", "activity", "awareness", "exposure", "expression"] {
        opt_text(v, k, 80)?;
    }
    if let Some(g) = v.get("gaze").filter(|x| *x != &Value::Null) {
        ident(g)?;
    }
    if let Some(held) = v.get("held").filter(|x| *x != &Value::Null) {
        if let Some(map) = held.as_object() {
            if map.len() > 8 {
                return Err(c::bad());
            }
            for (k, val) in map {
                line(&Value::str(k), 32, usize::MAX)?;
                line(val, 64, usize::MAX)?;
            }
        } else if let Some(arr) = held.as_array() {
            if arr.len() > 8 {
                return Err(c::bad());
            }
            for x in arr {
                line(x, 64, usize::MAX)?;
            }
        } else {
            return Err(c::bad());
        }
    }
    Ok(())
}

pub(crate) fn validate_sensory(v: &Value) -> Result<()> {
    keys(v, &[], &["scent", "voice_tone"])?;
    opt_text(v, "scent", 48)?;
    opt_text(v, "voice_tone", 48)?;
    Ok(())
}

pub(crate) fn validate_habits(v: &Value) -> Result<()> {
    for h in list(Some(v), 0, 16)? {
        line(h, 80, usize::MAX)?;
    }
    Ok(())
}

pub(crate) fn validate_tags(v: &Value) -> Result<()> {
    for tag in list(Some(v), 0, 32)? {
        line(tag, 48, usize::MAX)?;
    }
    Ok(())
}

pub(crate) fn validate_attributes(v: &Value) -> Result<()> {
    let map = v.as_object().ok_or_else(c::bad)?;
    if map.len() > 32 {
        return Err(c::bad());
    }
    for (k, val) in map {
        ident(&Value::str(k))?;
        match val {
            Value::String(s) => {
                if s.chars().count() > 120 || s.chars().any(char::is_control) {
                    return Err(c::bad());
                }
            }
            Value::Number(_) | Value::Bool(_) => {}
            _ => return Err(c::bad()),
        }
    }
    Ok(())
}

pub(crate) fn validate_matter(v: &Value) -> Result<()> {
    if let Some(m) = v.get("mass").filter(|x| *x != &Value::Null) {
        match m {
            Value::Number(s) => {
                s.parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0 && *n <= 1_000_000.0).ok_or_else(c::bad)?;
            }
            _ => return Err(c::bad()),
        }
    }
    if let Some(st) = v.get("state").filter(|x| *x != &Value::Null) {
        let s = c::text(st)?;
        if !["solid", "liquid", "gas"].contains(&s) {
            return Err(c::bad());
        }
    }
    for k in ["material", "color", "texture"] {
        if let Some(val) = v.get(k).filter(|x| *x != &Value::Null) {
            line(val, 48, usize::MAX)?;
        }
    }
    Ok(())
}

/// Health statuses a sheet or turn may give.
pub const HEALTH: [&str; 6] = ["healthy", "tired", "hurt", "ill", "wounded", "critical"];
/// Family member statuses.
pub const FAMILY_STATUS: [&str; 4] = ["alive", "deceased", "estranged", "unknown"];

/// A calendar day `{month 1–12, day}` (February may have 29 days).
pub(crate) fn validate_day(v: &Value) -> Result<(u64, u64)> {
    keys(v, &["month", "day"], &[])?;
    let m = c::uint(c::get(v, "month")?, 1, 12)?;
    let dim = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m as usize - 1];
    Ok((m, c::uint(c::get(v, "day")?, 1, dim)?))
}

/// `{status, conditions?: ≤16 [{id, label ≤48, severity 0–100, visible}]}`.
pub(crate) fn validate_health(v: &Value) -> Result<()> {
    keys(v, &["status"], &["conditions"])?;
    if !HEALTH.contains(&c::text(c::get(v, "status")?)?) {
        return Err(c::bad());
    }
    let given = list(v.get("conditions"), 0, 16)?;
    for x in given {
        keys(x, &["id", "label", "severity", "visible"], &[])?;
        line(c::get(x, "label")?, 48, usize::MAX)?;
        c::uint(c::get(x, "severity")?, 0, 100)?;
        c::boolean(c::get(x, "visible")?)?;
    }
    unique_ids(given)
}

pub(crate) fn validate_bio(v: &Value) -> Result<()> {
    keys(
        v,
        &[],
        &["species", "age", "birthday", "profession", "anatomy", "health", "skills", "knowledge", "family", "fitness"],
    )?;
    opt_text(v, "species", 48)?;
    opt_text(v, "profession", 80)?;
    if let Some(a) = v.get("age") {
        c::uint(a, 0, 100_000)?;
    }
    if let Some(b) = v.get("birthday") {
        validate_day(b)?;
    }
    if let Some(a) = v.get("anatomy") {
        keys(a, &[], &["body", "parts", "notes"])?;
        opt_text(a, "body", 48)?;
        opt_text(a, "notes", 240)?;
        for p in list(a.get("parts"), 0, 16)? {
            line(p, 48, usize::MAX)?;
        }
    }
    if let Some(h) = v.get("health") {
        validate_health(h)?;
    }
    if let Some(f) = v.get("fitness").filter(|x| **x != Value::Null) {
        crate::character::fitness::validate_fitness(f)?;
    }
    let mut seen = Vec::new();
    let skills = list(v.get("skills"), 0, 32)?;
    for x in skills {
        keys(x, &["id", "label", "level"], &["phrases"])?;
        line(c::get(x, "label")?, 48, usize::MAX)?;
        c::uint(c::get(x, "level")?, 0, 100)?;
        if let Some(p) = x.get("phrases") {
            phrases(p, &mut seen, 8)?;
        }
    }
    unique_ids(skills)?;
    let mut seen = Vec::new();
    let knowledge = list(v.get("knowledge"), 0, 32)?;
    for x in knowledge {
        keys(x, &["id", "topic", "phrases", "level", "summary"], &["when"])?;
        line(c::get(x, "topic")?, 48, usize::MAX)?;
        phrases(c::get(x, "phrases")?, &mut seen, 8)?;
        c::uint(c::get(x, "level")?, 0, 100)?;
        line(c::get(x, "summary")?, 240, usize::MAX)?;
        Conditions::field(x, "when", &VARS)?;
    }
    unique_ids(knowledge)?;
    for x in list(v.get("family"), 0, 16)? {
        keys(x, &["name", "relation", "status"], &["with"])?;
        line(c::get(x, "name")?, 32, usize::MAX)?;
        line(c::get(x, "relation")?, 32, usize::MAX)?;
        if !FAMILY_STATUS.contains(&c::text(c::get(x, "status")?)?) {
            return Err(c::bad());
        }
        if let Some(w) = x.get("with") {
            ident(w)?;
        }
    }
    Ok(())
}
