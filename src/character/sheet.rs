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
            &["persona", "speech", "services", "goals", "needs", "relationships", "factions", "secrets", "opinions"],
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
}
