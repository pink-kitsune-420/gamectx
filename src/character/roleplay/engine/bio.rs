//! The `bio` stage: template variables and model-brief facts from a sheet's `bio` (age,
//! birthday, profession, skills, knowledge, health, family), with the host's live
//! `health` and today's `date` from the turn. Only visible health conditions are used.
use super::lines::LineBook;
use crate::character::sheet::CharacterSheet;
use crate::data::conditions::{Conditions, Vars};
use crate::knowledge::props::normalize;
use crate::{obj, Value};

fn int(v: Option<&Value>) -> Option<i64> {
    v.and_then(Value::as_usize).map(|n| n as i64)
}
fn text<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn list<'a>(v: Option<&'a Value>, k: &str) -> &'a [Value] {
    v.and_then(|b| b.get(k)).and_then(Value::as_array).unwrap_or(&[])
}

/// Live health: the turn's `health` when given, else the sheet's.
pub(crate) fn health<'a>(sheet: &'a CharacterSheet, turn: Option<&'a Value>) -> Option<&'a Value> {
    turn.or_else(|| sheet.bio().and_then(|b| b.get("health")))
}

/// Visible conditions, most severe first (ties by id).
pub(crate) fn visible(h: Option<&Value>) -> Vec<&Value> {
    let mut out =
        list(h, "conditions").iter().filter(|c| c.get("visible") == Some(&Value::Bool(true))).collect::<Vec<_>>();
    out.sort_by(|a, b| int(b.get("severity")).cmp(&int(a.get("severity"))).then(text(a, "id").cmp(&text(b, "id"))));
    out
}

/// Skills, highest level first (ties by id).
pub(crate) fn ranked(sheet: &CharacterSheet) -> Vec<&Value> {
    let mut out = list(sheet.bio(), "skills").iter().collect::<Vec<_>>();
    out.sort_by(|a, b| int(b.get("level")).cmp(&int(a.get("level"))).then(text(a, "id").cmp(&text(b, "id"))));
    out
}

/// Add bio variables to `base`.
pub(crate) fn vars(
    base: &mut Vars,
    sheet: &CharacterSheet,
    health_turn: Option<&Value>,
    date: Option<&Value>,
    book: &LineBook,
) {
    let bio = sheet.bio();
    if let Some(s) = bio.and_then(|b| text(b, "species")) {
        base.text("species", s);
    }
    let age = int(bio.and_then(|b| b.get("age")));
    base.flag("has_age", age.is_some());
    if let Some(a) = age {
        base.int("age", a);
    }
    let birthday = bio.and_then(|b| b.get("birthday"));
    base.flag("has_birthday", birthday.is_some());
    let mut today = false;
    if let Some(b) = birthday {
        let (m, d) = (int(b.get("month")).unwrap_or(1), int(b.get("day")).unwrap_or(1));
        base.int("birth_month", m).int("birth_day", d);
        if let Some((_, s)) = book.say("birthday_date", base, &[]) {
            base.text("birthday", &s);
        }
        today = date.is_some_and(|x| int(x.get("month")) == Some(m) && int(x.get("day")) == Some(d));
    }
    base.flag("is_birthday", today);
    let profession = bio.and_then(|b| text(b, "profession"));
    base.flag("has_profession", profession.is_some());
    if let Some(p) = profession {
        base.text("profession", p);
    }
    let h = health(sheet, health_turn);
    base.flag("has_health", h.is_some());
    if let Some(s) = h.and_then(|h| text(h, "status")) {
        base.text("health", s);
    }
    let shown = visible(h).into_iter().filter_map(|c| text(c, "label")).collect::<Vec<_>>();
    let joined = book.joined(&shown).filter(|j| !shown.is_empty() && j.chars().count() <= 240);
    base.flag("has_conditions", joined.is_some());
    if let Some(j) = joined {
        base.text("conditions", &j);
    }
    let top = ranked(sheet).into_iter().take(3).filter_map(|s| text(s, "label")).collect::<Vec<_>>();
    let joined = book.joined(&top).filter(|j| !top.is_empty() && j.chars().count() <= 240);
    base.flag("has_skills", joined.is_some());
    if let Some(j) = joined {
        base.text("skills", &j);
    }
    let mut members = Vec::new();
    for m in list(bio, "family") {
        let mut v = Vars::default();
        v.text("relation", text(m, "relation").unwrap_or_default())
            .text("member", text(m, "name").unwrap_or_default())
            .text("family_status", text(m, "status").unwrap_or_default());
        if let Some((_, s)) = book.say("family_member", &v, &[]) {
            members.push(s);
        }
    }
    let refs = members.iter().map(String::as_str).collect::<Vec<_>>();
    let joined = book.joined(&refs).filter(|j| !refs.is_empty() && j.chars().count() <= 240);
    base.flag("has_family", joined.is_some());
    if let Some(j) = joined {
        base.text("family", &j);
    }
}

/// A skill named by `want` (already article-stripped): `(id, label, level)`.
pub(crate) fn skill<'a>(sheet: &'a CharacterSheet, book: &LineBook, want: &str) -> Option<(&'a str, &'a str, i64)> {
    list(sheet.bio(), "skills").iter().find_map(|s| {
        let label = text(s, "label")?;
        let mut names = vec![normalize(label)];
        names.extend(list(Some(s), "phrases").iter().filter_map(Value::as_str).map(normalize));
        if !names.iter().any(|n| book.bare(n) == want) {
            return None;
        }
        Some((text(s, "id")?, label, int(s.get("level"))?))
    })
}

/// A knowledge entry named by `want` whose `when` holds: `(id, topic, level, summary)`.
pub(crate) fn knowledge<'a>(
    sheet: &'a CharacterSheet,
    book: &LineBook,
    want: &str,
    base: &Vars,
) -> Option<(&'a str, &'a str, i64, &'a str)> {
    let x = list(sheet.bio(), "knowledge").iter().find(|k| {
        let mut names = vec![normalize(text(k, "topic").unwrap_or_default())];
        names.extend(list(Some(k), "phrases").iter().filter_map(Value::as_str).map(normalize));
        names.iter().any(|n| book.bare(n) == want)
    })?;
    let when = Conditions::field(x, "when", &super::VARS).ok()?;
    when.holds(base).then_some(())?;
    Some((text(x, "id")?, text(x, "topic")?, int(x.get("level"))?, text(x, "summary")?))
}

/// The model brief's `bio` object (`None` when empty).
pub(crate) fn brief(sheet: &CharacterSheet, health_turn: Option<&Value>, base: &Vars) -> Option<Value> {
    let bio = sheet.bio();
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut put = |k: &str, v: Value| out.push((k.to_owned(), v));
    for k in ["species", "age"] {
        if let Some(x) = bio.and_then(|b| b.get(k)) {
            put(k, x.clone());
        }
    }
    if let Some(crate::data::conditions::Scalar::Text(s)) = base.get("birthday") {
        put("birthday", Value::str(s));
    }
    for k in ["profession", "anatomy"] {
        if let Some(x) = bio.and_then(|b| b.get(k)) {
            put(k, x.clone());
        }
    }
    let h = health(sheet, health_turn);
    if let Some(h) = h {
        let shown = visible(Some(h)).into_iter().filter_map(|c| c.get("label").cloned()).collect::<Vec<_>>();
        put("health", obj! {"status"=>h.get("status").cloned().unwrap_or(Value::Null),"conditions"=>shown});
    }
    let skills = ranked(sheet);
    if !skills.is_empty() {
        put(
            "skills",
            Value::Array(
                skills
                    .iter()
                    .map(|s| obj! {"name"=>s.get("label").cloned(),"level"=>s.get("level").cloned()})
                    .collect(),
            ),
        );
    }
    let known = list(bio, "knowledge")
        .iter()
        .filter(|k| Conditions::field(k, "when", &super::VARS).is_ok_and(|w| w.holds(base)))
        .map(|k| obj! {"topic"=>k.get("topic").cloned(),"level"=>k.get("level").cloned(),"summary"=>k.get("summary").cloned()})
        .collect::<Vec<_>>();
    if !known.is_empty() {
        put("knowledge", Value::Array(known));
    }
    let family = list(bio, "family")
        .iter()
        .map(|m| obj! {"name"=>m.get("name").cloned(),"relation"=>m.get("relation").cloned(),"status"=>m.get("status").cloned()})
        .collect::<Vec<_>>();
    if !family.is_empty() {
        put("family", Value::Array(family));
    }
    (!out.is_empty()).then_some(Value::Object(out))
}
