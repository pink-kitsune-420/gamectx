//! Engine configuration: JSON v1 with defaults, plus a small typed builder.
use super::lines::{LineBook, Matcher, Rule};
use crate::character::roleplay::RoleplayActions;
use crate::character::sheet::CharacterSheet;
use crate::knowledge::codex::Codex;
use crate::knowledge::props::{ident, line, normalize, Props};
use crate::{checked as c, obj, Result, Value};

const EN: &str = include_str!("en.json");

/// A switchable engine stage, in cascade order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// Slice long player messages into bounded chunks (`max_chars` 512–4096, default 1024).
    Chunks,
    /// `OOC:` / `((...))` notes are kept out of character memory.
    OutOfCharacter,
    /// Audience, spatial distance, facing and unfinished message pauses (`action_turns` 0–100, default 3; `conversational_distance` 1–100, default 5; `max_distance` 1–1000, default 20).
    Addressing,
    /// Host-registered `*stage actions*` (`actions`: a [`RoleplayActions`] registry).
    StageActions,
    /// Present props: descriptions and interactions (`registry`: a [`Props`] registry).
    Props,
    /// `*approaches X*` / `*enters Y*` against host-listed targets.
    Directions,
    /// Location, sight, hearing and name questions answered from the host scene.
    Perception,
    /// The bounded English command grammar over host-listed legal choices.
    Commands,
    /// Acknowledge and remember explicit name/preference claims (`max_preferences` 1–16, default 3).
    Disclosures,
    /// Answer "what is my name / what do I like" from remembered claims.
    Recall,
    /// Encounter-aware greetings and introductions (`long_gap_turns` 1–1000000, default 12).
    Greetings,
    /// Thanks, farewells, wellbeing, elaboration and the authored fallback line.
    Social,
    /// Hand anything unanswered to a model request (compiled by the host).
    Model,
    /// Steer away from listed topics (`rating`: all/teen/mature, `avoid`: `[{id, phrases}]`).
    Content,
    /// Codex entrances: exits, walking through, descriptions and sound through doorways.
    Entrances,
    /// Codex places: spots, ambience (smell, light, sound, temperature) and norm reactions.
    Scenery,
    /// Sheet persona: "what do you do", "tell me about yourself".
    Persona,
    /// Sheet services: listing them and proposing requested ones.
    Services,
    /// Sheet secrets: revealed only when their conditions hold, otherwise covered or withheld.
    Secrets,
    /// Sheet opinions on named topics.
    Opinions,
    /// Sheet relationships: "who is X?".
    Relationships,
    /// Codex factions and the character's stance toward them.
    Factions,
    /// Codex events with timing (`soon_turns` 1–1000000, default 10).
    Events,
    /// Codex quests this character gives, at the listener's current stage.
    Quests,
    /// Sheet goals ("what do you want?") and urgent needs.
    Goals,
}
impl Feature {
    pub const ALL: [Feature; 25] = [
        Feature::Chunks,
        Feature::OutOfCharacter,
        Feature::Addressing,
        Feature::StageActions,
        Feature::Props,
        Feature::Directions,
        Feature::Perception,
        Feature::Commands,
        Feature::Disclosures,
        Feature::Recall,
        Feature::Greetings,
        Feature::Social,
        Feature::Model,
        Feature::Content,
        Feature::Entrances,
        Feature::Scenery,
        Feature::Persona,
        Feature::Services,
        Feature::Secrets,
        Feature::Opinions,
        Feature::Relationships,
        Feature::Factions,
        Feature::Events,
        Feature::Quests,
        Feature::Goals,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Feature::Chunks => "chunks",
            Feature::OutOfCharacter => "out_of_character",
            Feature::Addressing => "addressing",
            Feature::StageActions => "stage_actions",
            Feature::Props => "props",
            Feature::Directions => "directions",
            Feature::Perception => "perception",
            Feature::Commands => "commands",
            Feature::Disclosures => "disclosures",
            Feature::Recall => "recall",
            Feature::Greetings => "greetings",
            Feature::Social => "social",
            Feature::Model => "model",
            Feature::Content => "content",
            Feature::Entrances => "entrances",
            Feature::Scenery => "scenery",
            Feature::Persona => "persona",
            Feature::Services => "services",
            Feature::Secrets => "secrets",
            Feature::Opinions => "opinions",
            Feature::Relationships => "relationships",
            Feature::Factions => "factions",
            Feature::Events => "events",
            Feature::Quests => "quests",
            Feature::Goals => "goals",
        }
    }
    /// Numeric settings: (name, low, high, default).
    fn params(self) -> &'static [(&'static str, u64, u64, u64)] {
        match self {
            Feature::Chunks => &[("max_chars", 512, 4096, 1024)],
            Feature::Addressing => {
                &[("action_turns", 0, 100, 3), ("conversational_distance", 1, 100, 5), ("max_distance", 1, 1000, 20)]
            }
            Feature::Disclosures => &[("max_preferences", 1, 16, 3)],
            Feature::Greetings => &[("long_gap_turns", 1, 1_000_000, 12)],
            Feature::Events => &[("soon_turns", 1, 1_000_000, 10)],
            Feature::Model => &[
                ("max_tokens", 16, 1024, 160),
                ("max_sentences", 1, 4, 3),
                ("max_chars", 40, 240, 240),
                ("brief_chars", 512, 16384, 4096),
            ],
            _ => &[],
        }
    }
}

/// Validated engine configuration. [`EngineConfig::to_value`] is the normalized JSON with
/// every default written out; [`EngineConfig::from_value`] accepts the same shape with any
/// optional part omitted.
#[derive(Clone, Debug)]
pub struct EngineConfig {
    value: Value,
    pub(crate) props: Props,
    pub(crate) actions: RoleplayActions,
    pub(crate) book: LineBook,
    pub(crate) sheet: CharacterSheet,
    pub(crate) codex: Codex,
    pub(crate) avoid: Vec<(String, Vec<String>)>,
}
/// 1–8 rule templates for the model request's first system message.
fn model_rules(v: &Value) -> Result<Vec<String>> {
    let list = c::list(v)?;
    if list.is_empty() || list.len() > 8 {
        return Err(c::bad());
    }
    list.iter().map(|t| super::lines::template(t).and_then(|()| Ok(c::text(t)?.to_owned()))).collect()
}

fn keys(v: &Value, required: &[&str], optional: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(c::bad)?;
    if required.iter().any(|k| v.get(k).is_none())
        || rows.iter().any(|(k, _)| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(c::bad());
    }
    Ok(())
}
fn lists(v: &Value) -> Result<(String, String)> {
    c::fields(v, &["separator", "final"])?;
    let part = |k: &str| -> Result<String> {
        let s = c::text(c::get(v, k)?)?;
        if s.chars().count() > 16 || s.chars().any(char::is_control) {
            return Err(c::bad());
        }
        Ok(s.to_owned())
    };
    Ok((part("separator")?, part("final")?))
}
fn articles(v: &Value) -> Result<Vec<String>> {
    let xs = c::list(v)?;
    if xs.len() > 8 {
        return Err(c::bad());
    }
    xs.iter()
        .map(|a| {
            let s = line(a, 16, usize::MAX)?;
            if normalize(s) != s {
                return Err(c::bad());
            }
            Ok(s.to_owned())
        })
        .collect()
}
impl EngineConfig {
    /// All features on, the English pack, two remembered replies per listener.
    pub fn new(id: &str, name: &str) -> Result<Self> {
        Self::from_value(&obj! {"version"=>1,"character"=>obj!{"id"=>id,"name"=>name}})
    }
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(input, 2_097_152)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        crate::packet::request::bounded(v, 2_097_152)?;
        keys(
            v,
            &["version", "character"],
            &["features", "variety", "pack", "phrases", "lines", "lists", "articles", "sheet", "codex", "model_rules"],
        )?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let character = c::get(v, "character")?;
        c::fields(character, &["id", "name"])?;
        ident(c::get(character, "id")?)?;
        line(c::get(character, "name")?, 32, usize::MAX)?;
        let empty = obj! {};
        let given = v.get("features").unwrap_or(&empty);
        let names = Feature::ALL.map(Feature::as_str);
        if given.as_object().ok_or_else(c::bad)?.iter().any(|(k, _)| !names.contains(&k.as_str())) {
            return Err(c::bad());
        }
        let (mut features, mut props, mut actions, mut avoid) = (Vec::new(), Props::default(), None, Vec::new());
        for f in Feature::ALL {
            let fv = given.get(f.as_str()).unwrap_or(&empty);
            if f == Feature::Content {
                keys(fv, &[], &["enabled", "rating", "avoid"])?;
                let enabled = fv.get("enabled").map_or(Ok(true), c::boolean)?;
                let rating = fv.get("rating").map_or(Ok("teen"), c::text)?;
                if !["all", "teen", "mature"].contains(&rating) {
                    return Err(c::bad());
                }
                let topics = crate::knowledge::codex::list(fv.get("avoid"), 0, 32)?;
                let mut seen = Vec::new();
                for a in topics {
                    keys(a, &["id", "phrases"], &[])?;
                    avoid.push((
                        c::text(c::get(a, "id")?)?.to_owned(),
                        crate::knowledge::codex::phrases(c::get(a, "phrases")?, &mut seen, 16)?,
                    ));
                }
                crate::knowledge::codex::unique_ids(topics)?;
                features
                    .push((f.as_str().to_owned(), obj! {"enabled"=>enabled,"rating"=>rating,"avoid"=>topics.to_vec()}));
                continue;
            }
            let mut optional = vec!["enabled"];
            optional.extend(f.params().iter().map(|p| p.0));
            match f {
                Feature::StageActions => optional.push("actions"),
                Feature::Props => optional.push("registry"),
                _ => {}
            }
            keys(fv, &[], &optional)?;
            let mut out = vec![("enabled".to_owned(), Value::Bool(fv.get("enabled").map_or(Ok(true), c::boolean)?))];
            for (k, lo, hi, d) in f.params() {
                let n = fv.get(k).map_or(Ok(*d), |x| c::uint(x, *lo, *hi))?;
                out.push(((*k).to_owned(), n.into()));
            }
            if f == Feature::StageActions {
                let a =
                    fv.get("actions").cloned().unwrap_or_else(|| obj! {"version"=>1,"entries"=>Vec::<Value>::new()});
                actions = Some(RoleplayActions::from_value(&a)?);
                out.push(("actions".into(), a));
            }
            if f == Feature::Props {
                if let Some(r) = fv.get("registry") {
                    props = Props::from_value(r)?;
                }
                out.push(("registry".into(), props.to_value()));
            }
            features.push((f.as_str().to_owned(), Value::Object(out)));
        }
        let variety = v.get("variety").unwrap_or(&empty);
        keys(variety, &[], &["recent"])?;
        let recent = variety.get("recent").map_or(Ok(2), |x| c::uint(x, 0, 8))?;
        let pack = v.get("pack").map_or(Ok("en"), c::text)?;
        let mut book = match pack {
            "en" => Self::pack(&crate::data::json::parse(EN, 262_144)?)?,
            "none" => LineBook::default(),
            _ => return Err(c::bad()),
        };
        let phrases = v.get("phrases").unwrap_or(&empty);
        let rows = phrases.as_object().ok_or_else(c::bad)?;
        if rows.len() > 64 {
            return Err(c::bad());
        }
        for (k, m) in rows {
            ident(&Value::str(k))?;
            let m = Matcher::from_value(m)?;
            match book.phrases.iter_mut().find(|(old, _)| old == k) {
                Some(slot) => slot.1 = m,
                None => book.phrases.push((k.clone(), m)),
            }
        }
        let no_lines = Value::Array(Vec::new());
        let lines = v.get("lines").unwrap_or(&no_lines);
        let given_lines = c::list(lines)?;
        if given_lines.len() > 128 {
            return Err(c::bad());
        }
        let mut own = Vec::<Rule>::new();
        for r in given_lines {
            let rule = Rule::from_value(r)?;
            if own.iter().any(|old| old.id == rule.id) {
                return Err(c::bad());
            }
            own.push(rule);
        }
        let mut merged = Vec::new();
        for rule in book.lines {
            match own.iter().position(|o| o.id == rule.id) {
                Some(i) => merged.push(own.remove(i)),
                None => merged.push(rule),
            }
        }
        own.extend(merged);
        if own.len() > 256 {
            return Err(c::bad());
        }
        book.lines = own;
        let given_lists = v.get("lists").filter(|x| *x != &Value::Null);
        if let Some(l) = given_lists {
            book.lists = Some(lists(l)?);
        }
        let given_articles = v.get("articles").filter(|x| *x != &Value::Null);
        if let Some(a) = given_articles {
            book.articles = articles(a)?;
        }
        let codex = match v.get("codex") {
            Some(x) => Codex::from_value(x)?,
            None => Codex::default(),
        };
        let sheet = match v.get("sheet") {
            Some(x) => CharacterSheet::from_value(x, &codex)?,
            None => CharacterSheet::default(),
        };
        book.avoid = sheet.avoid.clone();
        let given_rules = v.get("model_rules").filter(|x| *x != &Value::Null);
        if let Some(r) = given_rules {
            book.rules = model_rules(r)?;
        }
        let value = obj! {
            "version"=>1,
            "character"=>character.clone(),
            "features"=>Value::Object(features),
            "variety"=>obj!{"recent"=>recent},
            "pack"=>pack,
            "phrases"=>phrases.clone(),
            "lines"=>lines.clone(),
            "lists"=>given_lists.cloned(),
            "articles"=>given_articles.cloned(),
            "sheet"=>sheet.to_value(),
            "codex"=>codex.to_value(),
            "model_rules"=>given_rules.cloned()
        };
        Ok(Self { value, props, actions: actions.ok_or_else(c::bad)?, book, sheet, codex, avoid })
    }
    fn pack(v: &Value) -> Result<LineBook> {
        c::fields(v, &["version", "language", "phrases", "lists", "articles", "lines", "model_rules"])?;
        let mut book = LineBook::default();
        for (k, m) in v.get("phrases").and_then(Value::as_object).ok_or_else(c::bad)? {
            book.phrases.push((k.clone(), Matcher::from_value(m)?));
        }
        book.lists = Some(lists(c::get(v, "lists")?)?);
        book.articles = articles(c::get(v, "articles")?)?;
        book.lines = c::list(c::get(v, "lines")?)?.iter().map(Rule::from_value).collect::<Result<_>>()?;
        book.rules = model_rules(c::get(v, "model_rules")?)?;
        Ok(book)
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn character_id(&self) -> &str {
        self.value.get("character").and_then(|c| c.get("id")).and_then(Value::as_str).unwrap_or_default()
    }
    pub fn character_name(&self) -> &str {
        self.value.get("character").and_then(|c| c.get("name")).and_then(Value::as_str).unwrap_or_default()
    }
    fn feature(&self, f: Feature) -> &Value {
        self.value.get("features").and_then(|v| v.get(f.as_str())).unwrap_or(&Value::Null)
    }
    /// The content rating (`all`, `teen` or `mature`).
    pub fn content_rating(&self) -> &str {
        self.feature(Feature::Content).get("rating").and_then(Value::as_str).unwrap_or("teen")
    }
    pub fn enabled(&self, f: Feature) -> bool {
        self.feature(f).get("enabled") == Some(&Value::Bool(true))
    }
    /// A numeric feature setting such as `("greetings", "long_gap_turns")`.
    pub fn setting(&self, f: Feature, name: &str) -> Option<u64> {
        self.feature(f).get(name)?.as_usize().map(|n| n as u64)
    }
    pub fn recent(&self) -> usize {
        self.value.get("variety").and_then(|v| v.get("recent")).and_then(Value::as_usize).unwrap_or(0)
    }
    fn edit(&self, f: impl FnOnce(&mut Vec<(String, Value)>)) -> Result<Self> {
        let mut v = self.value.clone();
        if let Value::Object(rows) = &mut v {
            f(rows);
        }
        Self::from_value(&v)
    }
    fn edit_feature(&self, f: Feature, key: &str, value: Value) -> Result<Self> {
        self.edit(|rows| {
            if let Some((_, Value::Object(features))) = rows.iter_mut().find(|(k, _)| k == "features") {
                if let Some((_, Value::Object(fields))) = features.iter_mut().find(|(k, _)| k == f.as_str()) {
                    match fields.iter_mut().find(|(k, _)| k == key) {
                        Some(slot) => slot.1 = value,
                        None => fields.push((key.to_owned(), value)),
                    }
                }
            }
        })
    }
    fn edit_top(&self, key: &str, value: Value) -> Result<Self> {
        self.edit(|rows| match rows.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value,
            None => rows.push((key.to_owned(), value)),
        })
    }
    /// Turn one stage on or off.
    pub fn with_feature(&self, f: Feature, enabled: bool) -> Result<Self> {
        self.edit_feature(f, "enabled", enabled.into())
    }
    /// Set a numeric feature setting (validated against its documented range).
    pub fn with_setting(&self, f: Feature, name: &str, n: u64) -> Result<Self> {
        if !f.params().iter().any(|p| p.0 == name) {
            return Err(c::bad());
        }
        self.edit_feature(f, name, n.into())
    }
    pub fn with_props(&self, props: &Props) -> Result<Self> {
        self.edit_feature(Feature::Props, "registry", props.to_value())
    }
    pub fn with_actions(&self, actions: &RoleplayActions) -> Result<Self> {
        self.edit_feature(Feature::StageActions, "actions", actions.to_value())
    }
    /// How many fully heard replies per listener the variety rule avoids repeating (0–8).
    pub fn with_recent(&self, n: u64) -> Result<Self> {
        self.edit_top("variety", obj! {"recent"=>n})
    }
    /// `"en"` (the built-in English pack) or `"none"` (only your own phrases and lines).
    pub fn with_pack(&self, pack: &str) -> Result<Self> {
        self.edit_top("pack", pack.into())
    }
    /// Replace (or add) the phrase matcher for one intent.
    pub fn with_phrases(&self, intent: &str, matcher: &Value) -> Result<Self> {
        let mut phrases = self.value.get("phrases").cloned().unwrap_or_else(|| obj! {});
        if let Value::Object(rows) = &mut phrases {
            match rows.iter_mut().find(|(k, _)| k == intent) {
                Some(slot) => slot.1 = matcher.clone(),
                None => rows.push((intent.to_owned(), matcher.clone())),
            }
        }
        self.edit_top("phrases", phrases)
    }
    pub fn sheet(&self) -> &CharacterSheet {
        &self.sheet
    }
    pub fn codex(&self) -> &Codex {
        &self.codex
    }
    pub fn with_sheet(&self, sheet: &CharacterSheet) -> Result<Self> {
        self.edit_top("sheet", sheet.to_value())
    }
    pub fn with_codex(&self, codex: &Codex) -> Result<Self> {
        self.edit_top("codex", codex.to_value())
    }
    /// Add a line rule. A rule with a pack line's id replaces that line in place; a new
    /// id is tried before every pack line.
    pub fn with_line(&self, rule: &Value) -> Result<Self> {
        let mut lines = self.value.get("lines").and_then(Value::as_array).map(<[Value]>::to_vec).unwrap_or_default();
        lines.push(rule.clone());
        self.edit_top("lines", lines.into())
    }
}
