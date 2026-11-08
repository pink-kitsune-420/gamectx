//! Phrase matchers, line templates and the line book. Pure data; no model calls.
use crate::data::conditions::{Conditions, Scalar, Vars};
use crate::knowledge::props::{ident, line, normalize};
use crate::{checked as c, Result, Value};

/// Template and `when` variables a line may use. Unknown names are config errors;
/// conditions may also test host flags written `state.<id>`.
pub const VARS: [&str; 118] = [
    "name",
    "scene",
    "place",
    "atmosphere",
    "trust",
    "relationship",
    "meeting",
    "needs_introduction",
    "sight",
    "sound",
    "has_sight",
    "has_sound",
    "props",
    "has_props",
    "prop",
    "description",
    "note",
    "has_note",
    "slot",
    "value",
    "old",
    "corrects",
    "polarity",
    "known",
    "reason",
    "has_reason",
    "exits",
    "status",
    "destination",
    "entrance",
    "through",
    "heard",
    "smell",
    "light",
    "temperature",
    "floor",
    "has_floor",
    "ceiling",
    "has_ceiling",
    "exterior",
    "facing",
    "spot",
    "role",
    "summary",
    "services",
    "label",
    "text",
    "need",
    "has_need",
    "person",
    "kind",
    "faction",
    "stance",
    "event",
    "events",
    "timing",
    "quest",
    "stage",
    "sentences",
    "material",
    "color",
    "texture",
    "clothing",
    "eyes",
    "hair",
    "skin",
    "posture",
    "activity",
    "awareness",
    "expression",
    "scent",
    "held",
    "has_held",
    "has_posture",
    "gaze",
    "has_gaze",
    "has_clothing",
    "has_activity",
    "has_material",
    "has_color",
    "has_texture",
    "has_matter",
    "state",
    "age",
    "has_age",
    "birthday",
    "has_birthday",
    "birth_month",
    "birth_day",
    "is_birthday",
    "profession",
    "has_profession",
    "health",
    "has_health",
    "conditions",
    "has_conditions",
    "skills",
    "has_skills",
    "skill",
    "skill_level",
    "knowledge",
    "knowledge_topic",
    "knowledge_level",
    "family",
    "has_family",
    "relation",
    "member",
    "family_status",
    "species",
    "news",
    "nearby",
    "things",
    "recipe",
    "inputs",
    "community",
    "currency",
    "resource",
    "temperament",
];
const KINDS: [&str; 4] = ["exact", "contains", "prefix", "unless"];

/// A normalized phrase matcher. `unless` vetoes; then `exact`, word-boundary `prefix`
/// (yielding the remainder) and word-boundary `contains` are tried in that order.
#[derive(Clone, Debug, Default)]
pub(crate) struct Matcher {
    lists: [Vec<String>; 4],
}
impl Matcher {
    pub(crate) fn from_value(v: &Value) -> Result<Self> {
        let rows = v.as_object().ok_or_else(c::bad)?;
        if rows.is_empty() || rows.iter().any(|(k, _)| !KINDS.contains(&k.as_str())) {
            return Err(c::bad());
        }
        let mut out = Self::default();
        for (i, kind) in KINDS.iter().enumerate() {
            let Some(list) = v.get(kind) else { continue };
            let list = c::list(list)?;
            if list.is_empty() || list.len() > 32 {
                return Err(c::bad());
            }
            for p in list {
                let key = normalize(line(p, 128, 128)?);
                if key.is_empty() {
                    return Err(c::bad());
                }
                out.lists[i].push(key);
            }
        }
        Ok(out)
    }
    /// `Some(remainder)` when the normalized input `q` matches.
    pub(crate) fn hit(&self, q: &str) -> Option<String> {
        let padded = format!(" {q} ");
        let within = |p: &String| padded.contains(&format!(" {p} "));
        if self.lists[3].iter().any(within) {
            return None;
        }
        if self.lists[0].iter().any(|p| p == q) {
            return Some(String::new());
        }
        for p in &self.lists[2] {
            if q == p {
                return Some(String::new());
            }
            if let Some(rest) = q.strip_prefix(p.as_str()).and_then(|r| r.strip_prefix(' ')) {
                return Some(rest.trim().to_owned());
            }
        }
        self.lists[1].iter().any(within).then(String::new)
    }
}

/// Validate a line template: ≤240 chars, `{var}` names from [`VARS`], no stray braces.
pub(crate) fn template(v: &Value) -> Result<()> {
    let mut rest = line(v, 240, usize::MAX)?;
    while let Some(i) = rest.find(['{', '}']) {
        if !rest[i..].starts_with('{') {
            return Err(c::bad());
        }
        let j = rest[i..].find('}').ok_or_else(c::bad)? + i;
        if !VARS.contains(&&rest[i + 1..j]) {
            return Err(c::bad());
        }
        rest = &rest[j + 1..];
    }
    Ok(())
}

pub(crate) fn render(t: &str, vars: &Vars) -> Option<String> {
    let (mut out, mut rest) = (String::new(), t);
    while let Some(i) = rest.find('{') {
        let j = rest[i..].find('}')? + i;
        out.push_str(&rest[..i]);
        match vars.get(&rest[i + 1..j])? {
            Scalar::Text(s) => out.push_str(s),
            Scalar::Flag(b) => out.push_str(if *b { "true" } else { "false" }),
            Scalar::Int(n) => out.push_str(&n.to_string()),
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// One authored line rule: the first matching rule for an intent answers.
#[derive(Clone, Debug)]
pub(crate) struct Rule {
    pub(crate) id: String,
    intent: String,
    when: Conditions,
    variants: Vec<String>,
    pub(crate) introduces: bool,
}
impl Rule {
    pub(crate) fn from_value(v: &Value) -> Result<Self> {
        let rows = v.as_object().ok_or_else(c::bad)?;
        let allowed = ["id", "intent", "variants", "when", "introduces"];
        if rows.iter().any(|(k, _)| !allowed.contains(&k.as_str())) {
            return Err(c::bad());
        }
        let id = ident(c::get(v, "id")?)?.to_owned();
        let intent = ident(c::get(v, "intent")?)?.to_owned();
        let list = c::list(c::get(v, "variants")?)?;
        if list.is_empty() || list.len() > 8 {
            return Err(c::bad());
        }
        let mut variants = Vec::new();
        for t in list {
            template(t)?;
            variants.push(c::text(t)?.to_owned());
        }
        let introduces = v.get("introduces").map_or(Ok(false), c::boolean)?;
        let when = Conditions::field(v, "when", &VARS)?;
        Ok(Self { id, intent, when, variants, introduces })
    }
}

/// The merged phrases, lines and list wording an engine speaks with.
#[derive(Clone, Debug, Default)]
pub(crate) struct LineBook {
    pub(crate) phrases: Vec<(String, Matcher)>,
    pub(crate) lines: Vec<Rule>,
    pub(crate) lists: Option<(String, String)>,
    pub(crate) articles: Vec<String>,
    pub(crate) avoid: Vec<String>,
    /// Rule templates for model requests (pack `model_rules`, or the config's).
    pub(crate) rules: Vec<String>,
}
impl LineBook {
    pub(crate) fn matcher(&self, intent: &str) -> Option<&Matcher> {
        self.phrases.iter().find(|(k, _)| k == intent).map(|(_, m)| m)
    }
    /// The first applicable rule for `intent` and its first renderable variant not in `recent`.
    pub(crate) fn say(&self, intent: &str, vars: &Vars, recent: &[String]) -> Option<(&Rule, String)> {
        for rule in self.lines.iter().filter(|r| r.intent == intent && r.when.holds(vars)) {
            let options = rule
                .variants
                .iter()
                .filter_map(|t| render(t, vars))
                .filter(|s| !s.is_empty() && s.chars().count() <= 240 && !self.avoided(s))
                .collect::<Vec<_>>();
            if let Some(first) = options.first() {
                let pick = options.iter().find(|s| !recent.contains(s)).unwrap_or(first).clone();
                return Some((rule, pick));
            }
        }
        None
    }
    pub(crate) fn avoided(&self, s: &str) -> bool {
        let padded = format!(" {} ", normalize(s));
        self.avoid.iter().any(|w| padded.contains(&format!(" {w} ")))
    }
    /// Strip one leading article word (when more words follow).
    pub(crate) fn bare<'a>(&self, q: &'a str) -> &'a str {
        match q.split_once(' ') {
            Some((first, rest)) if self.articles.iter().any(|a| a == first) => rest,
            _ => q,
        }
    }
    pub(crate) fn joined(&self, labels: &[&str]) -> Option<String> {
        let (separator, last) = self.lists.as_ref()?;
        Some(match labels {
            [] => String::new(),
            [one] => (*one).to_owned(),
            [head @ .., tail] => format!("{}{last}{tail}", head.join(separator)),
        })
    }
}
