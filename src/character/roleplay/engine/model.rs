//! Model requests behind the `model` outcome, and validation of what a model sends back.
//!
//! The request is plain chat JSON (OpenAI / llama.cpp server shape): pack rules, a stable
//! character brief, the per-turn situation, then the player's words, constrained to a
//! `{"speech": string}` JSON schema. Only host-authored, currently known facts go in; a
//! secret's text is included only after it was revealed to this listener. Model output is
//! untrusted: [`check_speech`] accepts one exact speech object that passes bounded checks.
use super::config::{EngineConfig, Feature};
use super::lines::{render, Matcher};
use crate::data::conditions::{Scalar, Vars};
use crate::knowledge::props::normalize;
use crate::{obj, Value};

/// Everything [`compile`] reads from one turn.
pub(crate) struct Compile<'a> {
    pub config: &'a EngineConfig,
    pub input: &'a str,
    pub base: &'a Vars,
    pub scene: &'a Value,
    pub sight: Option<&'a str>,
    pub sound: Option<&'a str>,
    pub ambience: &'a dyn Fn(&str) -> Option<&'a str>,
    pub present: &'a [&'a str],
    pub exits: &'a [&'a str],
    pub listener: Option<&'a Value>,
    pub recent: &'a [String],
    pub events: &'a [(&'a str, &'a str)],
    pub on: &'a dyn Fn(Feature) -> bool,
}

type Fields = Vec<(String, Value)>;

fn put(fields: &mut Fields, k: &str, v: Value) {
    fields.push((k.to_owned(), v));
}
fn drop_key(fields: &mut Fields, k: &str) {
    fields.retain(|(old, _)| old != k);
}
fn non_empty(v: Option<&Value>) -> Option<&Value> {
    v.filter(|x| **x != Value::Null && x.as_array().is_none_or(|a| !a.is_empty()))
}
fn text<'a>(v: &'a Vars, k: &str) -> Option<&'a str> {
    match v.get(k) {
        Some(Scalar::Text(s)) => Some(s),
        _ => None,
    }
}

/// Build the chat request for a `model` outcome.
pub(crate) fn compile(t: &Compile) -> Value {
    let (config, on) = (t.config, t.on);
    let (sheet, codex) = (config.sheet(), config.codex());
    let setting = |k: &str| config.setting(Feature::Model, k).unwrap_or(0);
    let mut v = t.base.clone();
    v.int("sentences", setting("max_sentences") as i64);
    let rules = config.book.rules.iter().filter_map(|r| render(r, &v)).filter(|r| !r.is_empty()).collect::<Vec<_>>();

    let mut brief = Fields::new();
    put(&mut brief, "name", Value::str(config.character_name()));
    if on(Feature::Persona) {
        let persona = sheet.to_value().get("persona").cloned().unwrap_or(obj! {});
        for k in ["pronouns", "role", "summary", "appearance", "values", "fears", "desires"] {
            if let Some(x) = non_empty(persona.get(k)) {
                put(&mut brief, k, x.clone());
            }
        }
        let style = sheet.to_value().get("speech").cloned().unwrap_or(obj! {});
        let mut speech = Fields::new();
        for k in ["formality", "verbosity", "quirks", "catchphrases", "avoid_words"] {
            if let Some(x) = non_empty(style.get(k)) {
                put(&mut speech, k, x.clone());
            }
        }
        if !speech.is_empty() {
            put(&mut brief, "speech", Value::Object(speech));
        }
    }
    let relationships = sheet.all("relationships");
    if on(Feature::Relationships) && !relationships.is_empty() {
        let list = relationships.iter().map(|r| obj! {"name"=>r.get("name").cloned(),"kind"=>r.get("kind").cloned()});
        put(&mut brief, "relationships", Value::Array(list.collect()));
    }
    let factions = sheet.all("factions");
    if on(Feature::Factions) && !factions.is_empty() {
        let mut list = Vec::new();
        for f in factions {
            let id = f.get("faction").and_then(Value::as_str).unwrap_or_default();
            let mut x = Fields::new();
            put(&mut x, "name", codex.faction(id).and_then(|d| d.get("label")).cloned().unwrap_or(Value::Null));
            if let Some(role) = non_empty(f.get("role")) {
                put(&mut x, "role", role.clone());
            }
            list.push(Value::Object(x));
        }
        put(&mut brief, "factions", Value::Array(list));
    }
    if on(Feature::Content) {
        put(&mut brief, "content", obj! {"rating"=>config.content_rating()});
    }

    let mut scene = Fields::new();
    put(&mut scene, "name", t.scene.get("label").cloned().unwrap_or(Value::Null));
    put(&mut scene, "atmosphere", t.scene.get("atmosphere").cloned().unwrap_or(Value::Null));
    if let Some(s) = t.sight {
        put(&mut scene, "sight", Value::str(s));
    }
    if let Some(s) = t.sound {
        put(&mut scene, "sound", Value::str(s));
    }
    for k in ["smell", "light", "temperature"] {
        if let Some(a) = (t.ambience)(k) {
            put(&mut scene, k, Value::str(a));
        }
    }
    if !t.present.is_empty() {
        put(&mut scene, "props", Value::strings(t.present));
    }
    if !t.exits.is_empty() {
        put(&mut scene, "exits", Value::strings(t.exits));
    }
    let mut listener = Fields::new();
    put(&mut listener, "relationship", Value::str(text(t.base, "relationship").unwrap_or_default()));
    if let Some(l) = t.listener {
        if let Some(n) = non_empty(l.get("name")) {
            put(&mut listener, "name", n.clone());
        }
        if let Some(p) = non_empty(l.get("preferences")) {
            put(&mut listener, "preferences", p.clone());
        }
        let revealed = l.get("revealed").and_then(Value::as_array).unwrap_or(&[]);
        if on(Feature::Secrets) && !revealed.is_empty() {
            let told = sheet
                .all("secrets")
                .iter()
                .filter(|x| revealed.iter().any(|r| r.as_str().is_some() && x.get("id") == Some(r)))
                .filter_map(|x| x.get("reveal").cloned())
                .collect::<Vec<_>>();
            if !told.is_empty() {
                put(&mut listener, "already_told", Value::Array(told));
            }
        }
    }
    let mut sit = Fields::new();
    if on(Feature::Goals) {
        let mut best: Option<(&Value, u64)> = None;
        for (g, when) in sheet.all("goals").iter().zip(&sheet.goals) {
            let p = g.get("priority").and_then(Value::as_usize).unwrap_or(0) as u64;
            if when.holds(t.base) && best.is_none_or(|(_, b)| p > b) {
                best = Some((g, p));
            }
        }
        if let Some((g, _)) = best {
            put(&mut sit, "goal", g.get("description").cloned().unwrap_or(Value::Null));
        }
        if t.base.get("has_need") == Some(&Scalar::Flag(true)) {
            if let Some(n) = text(t.base, "need") {
                put(&mut sit, "need", Value::str(n));
            }
        }
    }
    if !t.events.is_empty() {
        let list = t.events.iter().map(|(name, timing)| obj! {"name"=>*name,"timing"=>*timing});
        put(&mut sit, "events", Value::Array(list.collect()));
    }
    if !t.recent.is_empty() {
        put(&mut sit, "recent_replies", Value::strings(t.recent));
    }

    // Trim optional facts in a fixed order until the brief and situation fit.
    let limit = setting("brief_chars") as usize;
    let assemble = |brief: &Fields, scene: &Fields, listener: &Fields, sit: &Fields| {
        let mut all = vec![
            ("scene".to_owned(), Value::Object(scene.clone())),
            ("listener".to_owned(), Value::Object(listener.clone())),
        ];
        all.extend(sit.iter().cloned());
        (Value::Object(brief.clone()), Value::Object(all))
    };
    let drops: [(u8, &str); 16] = [
        (3, "recent_replies"),
        (0, "appearance"),
        (0, "desires"),
        (0, "fears"),
        (0, "factions"),
        (0, "relationships"),
        (3, "events"),
        (0, "speech"),
        (3, "need"),
        (3, "goal"),
        (0, "values"),
        (0, "summary"),
        (2, "already_told"),
        (2, "preferences"),
        (1, "exits"),
        (1, "props"),
    ];
    for (target, k) in drops {
        let (b, s) = assemble(&brief, &scene, &listener, &sit);
        if b.to_json().chars().count() + s.to_json().chars().count() <= limit {
            break;
        }
        match target {
            0 => drop_key(&mut brief, k),
            1 => drop_key(&mut scene, k),
            2 => drop_key(&mut listener, k),
            _ => drop_key(&mut sit, k),
        }
    }
    let (brief, sit) = assemble(&brief, &scene, &listener, &sit);
    let mut messages = Vec::new();
    if !rules.is_empty() {
        messages.push(obj! {"role"=>"system","content"=>rules.join(" ")});
    }
    messages.push(obj! {"role"=>"system","content"=>brief.to_json()});
    messages.push(obj! {"role"=>"system","content"=>sit.to_json()});
    messages.push(obj! {"role"=>"user","content"=>t.input});
    let schema = obj! {
        "type"=>"object",
        "properties"=>obj!{"speech"=>obj!{"type"=>"string","minLength"=>1,"maxLength"=>setting("max_chars")}},
        "required"=>vec![Value::str("speech")],
        "additionalProperties"=>false
    };
    obj! {
        "messages"=>messages,
        "response_format"=>obj!{"type"=>"json_schema","json_schema"=>obj!{"name"=>"speech","strict"=>true,"schema"=>schema}},
        "max_tokens"=>setting("max_tokens")
    }
}

const WS: [char; 4] = [' ', '\t', '\n', '\r'];

/// The body inside one ```` ```json ```` fence (`None` when `raw` is not exactly one fence).
fn unfence(raw: &str) -> Option<&str> {
    let rest = raw.trim_start_matches(WS).strip_prefix("```")?;
    let rest =
        rest.strip_prefix("json").filter(|r| r.trim_start_matches([' ', '\t']).starts_with('\n')).unwrap_or(rest);
    let rest = rest.trim_start_matches([' ', '\t']).strip_prefix('\n')?;
    let body = rest.trim_end_matches(WS).strip_suffix("```")?.trim_end_matches([' ', '\t']).strip_suffix('\n')?;
    Some(body)
}

/// `{"speech": "<string>"}` with optional whitespace; returns the decoded string.
fn speech_object(body: &str) -> Option<String> {
    let rest = body.trim_start_matches(WS).strip_prefix('{')?.trim_start_matches(WS);
    let rest = rest.strip_prefix("\"speech\"")?.trim_start_matches(WS).strip_prefix(':')?.trim_start_matches(WS);
    let rest = rest.strip_prefix('"')?;
    let mut units = Vec::<u16>::new();
    let mut out = String::new();
    let mut chars = rest.char_indices();
    let flush = |units: &mut Vec<u16>, out: &mut String| -> Option<()> {
        out.push_str(&String::from_utf16(units).ok()?);
        units.clear();
        Some(())
    };
    let end = loop {
        let (i, ch) = chars.next()?;
        match ch {
            '"' => break i,
            '\\' => {
                let (_, e) = chars.next()?;
                let simple = match e {
                    '"' => Some('"'),
                    '\\' => Some('\\'),
                    '/' => Some('/'),
                    'b' => Some('\u{8}'),
                    'f' => Some('\u{c}'),
                    'n' => Some('\n'),
                    'r' => Some('\r'),
                    't' => Some('\t'),
                    'u' => None,
                    _ => return None,
                };
                match simple {
                    Some(c) => {
                        flush(&mut units, &mut out)?;
                        out.push(c);
                    }
                    None => {
                        let mut n = 0u16;
                        for _ in 0..4 {
                            n = n * 16 + chars.next()?.1.to_digit(16)? as u16;
                        }
                        units.push(n);
                    }
                }
            }
            c if (c as u32) < 0x20 => return None,
            c => {
                flush(&mut units, &mut out)?;
                out.push(c);
            }
        }
    };
    flush(&mut units, &mut out)?;
    let tail = rest[end + 1..].trim_start_matches(WS).strip_prefix('}')?;
    tail.trim_start_matches(WS).is_empty().then_some(out)
}

/// The first failed check's reason, or the accepted speech. Guardrails, not proof of truth.
pub(crate) fn check_speech(
    raw: &str,
    config: &EngineConfig,
    listener: Option<&Value>,
    who: &str,
) -> std::result::Result<String, &'static str> {
    let max = |k: &str| config.setting(Feature::Model, k).unwrap_or(0) as usize;
    if raw.len() > 16384 {
        return Err("length");
    }
    let body = unfence(raw).unwrap_or(raw);
    let decoded = speech_object(body).ok_or("format")?;
    let s = decoded.trim_matches(WS);
    if s.is_empty() {
        return Err("empty");
    }
    if s.chars().count() > max("max_chars") {
        return Err("length");
    }
    if s.chars().any(char::is_control) {
        return Err("control");
    }
    if s.contains(['*', '[', ']', '{', '}', '<', '>']) {
        return Err("stage");
    }
    let (mut runs, mut prev) = (0, false);
    for ch in s.chars() {
        let end = matches!(ch, '.' | '!' | '?');
        if end && !prev {
            runs += 1;
        }
        prev = end;
    }
    let tail = s.trim_end_matches(['"', '\'', ')', '\u{201d}', '\u{2019}', ' ']);
    if tail.chars().last().is_some_and(|c| !matches!(c, '.' | '!' | '?')) {
        runs += 1;
    }
    if runs > max("max_sentences") {
        return Err("sentences");
    }
    let q = normalize(s);
    let padded = format!(" {q} ");
    if config.book.matcher("model_reject").and_then(|m: &Matcher| m.hit(&q)).is_some() {
        return Err("out_of_character");
    }
    let content = config.enabled(Feature::Content)
        && config.avoid.iter().flat_map(|(_, ps)| ps).any(|p| padded.contains(&format!(" {} ", normalize(p))));
    if config.book.avoided(s) || content {
        return Err("avoided");
    }
    let revealed = listener.and_then(|l| l.get("revealed")).and_then(Value::as_array).unwrap_or(&[]);
    let locked = config.sheet().all("secrets").iter().filter(|x| !revealed.iter().any(|r| x.get("id") == Some(r)));
    if locked.filter_map(|x| x.get("reveal").and_then(Value::as_str)).any(|r| q.contains(&normalize(r))) {
        return Err("secret");
    }
    let lower = s.to_lowercase();
    let words = lower
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | ':' | '-')))
        .collect::<Vec<_>>();
    let ids = [who, config.character_id()];
    if ids.iter().any(|id| {
        id.contains(|c: char| c == '_' || c == ':' || c.is_ascii_digit()) && words.contains(&id.to_lowercase().as_str())
    }) {
        return Err("identifier");
    }
    let recent = listener.and_then(|l| l.get("recent")).and_then(Value::as_array).unwrap_or(&[]);
    if recent.iter().filter_map(Value::as_str).any(|r| normalize(r) == q) {
        return Err("repeat");
    }
    Ok(s.to_owned())
}
