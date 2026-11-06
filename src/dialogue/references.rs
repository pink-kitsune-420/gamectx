//! Bounded lexical entity references. Hosts supply visibility, aliases, type and language.
use crate::{checked as c, obj, request::bounded, Result, Value};
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    c::text(c::get(v, key)?)
}
fn reference(v: &Value) -> Result<()> {
    if *v == Value::Null {
        return Ok(());
    }
    c::fields(v, &["id", "scope", "revision"])?;
    c::scope(text(v, "id")?)?;
    c::scope(text(v, "scope")?)?;
    c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)?;
    Ok(())
}
/// Whole-token phrases; contained shorter aliases lose. Ambiguity produces candidates,
/// never an arbitrary first match. Context/selection metadata must come from the host.
pub fn resolve_references(a: &Value) -> Result<Value> {
    bounded(a, 65536)?;
    c::fields(a, &["entities", "input", "kind", "scope", "revision", "max_age", "focus", "selection", "pronouns"])?;
    let scope = text(a, "scope")?;
    c::scope(scope)?;
    let revision = c::uint(c::get(a, "revision")?, 0, u32::MAX as u64)?;
    let max_age = c::uint(c::get(a, "max_age")?, 0, 1024)?;
    let input = text(a, "input")?;
    if input.is_empty() || input.len() > 2048 {
        return Err(c::bad());
    }
    let wanted = words(input);
    if wanted.len() > 256 {
        return Err(c::bad());
    }
    let entities = c::list(c::get(a, "entities")?)?;
    if entities.len() > 64 {
        return Err(c::bad());
    }
    let kind = c::get(a, "kind")?;
    if *kind != Value::Null {
        c::scope(c::text(kind)?)?;
    }
    let pronouns = c::list(c::get(a, "pronouns")?)?;
    if pronouns.len() > 16 {
        return Err(c::bad());
    }
    for p in pronouns {
        c::scope(c::text(p)?)?;
    }
    let mut spans = Vec::new();
    let mut best_end = vec![0; wanted.len()];
    for (i, e) in entities.iter().enumerate() {
        c::fields(e, &["id", "label", "kind", "aliases"])?;
        for k in ["id", "label", "kind"] {
            c::scope(text(e, k)?)?;
        }
        if entities[..i].iter().any(|v| v.get("id") == e.get("id")) {
            return Err(c::bad());
        }
        let aliases = c::list(c::get(e, "aliases")?)?;
        if aliases.is_empty() || aliases.len() > 8 {
            return Err(c::bad());
        }
        for alias in aliases {
            let alias = c::text(alias)?;
            c::scope(alias)?;
            let needle = words(alias);
            if needle.is_empty() || needle.len() > 32 {
                return Err(c::bad());
            }
            for (start, w) in wanted.windows(needle.len()).enumerate() {
                if w == needle {
                    let end = start + needle.len();
                    spans.push((i, start, end));
                    best_end[start] = best_end[start].max(end);
                }
            }
        }
    }
    let mut prior = vec![0; wanted.len()];
    let mut largest = 0;
    for (i, end) in best_end.iter().enumerate() {
        prior[i] = largest;
        largest = largest.max(*end);
    }
    let mut matched = Vec::new();
    for (i, start, end) in spans {
        if prior[start] < end && best_end[start] == end && !matched.contains(&i) {
            matched.push(i);
        }
    }
    let eligible = (0..entities.len())
        .filter(|&i| *kind == Value::Null || entities[i].get("kind") == Some(kind))
        .collect::<Vec<_>>();
    let had_matches = !matched.is_empty();
    matched.retain(|i| eligible.contains(i));
    let focus = c::get(a, "focus")?;
    let selection = c::get(a, "selection")?;
    reference(focus)?;
    reference(selection)?;
    let find = |v: &Value| entities.iter().position(|e| e.get("id") == v.get("id"));
    let pronoun = pronouns.iter().any(|p| {
        let n = words(p.as_str().expect("validated pronoun"));
        !n.is_empty() && wanted.windows(n.len()).any(|w| w == n)
    });
    let reason = if *selection != Value::Null {
        let id = find(selection).filter(|i| eligible.contains(i)).ok_or_else(c::bad)?;
        if text(selection, "scope")? != scope
            || c::uint(c::get(selection, "revision")?, 0, u32::MAX as u64)? != revision
        {
            return Err(c::bad());
        }
        matched = vec![id];
        "explicit_target"
    } else if matched.is_empty() && had_matches {
        "type_mismatch"
    } else if matched.is_empty() && pronoun {
        let valid = if *focus != Value::Null {
            let old = c::uint(c::get(focus, "revision")?, 0, u32::MAX as u64)?;
            text(focus, "scope")? == scope && old <= revision && revision - old <= max_age
        } else {
            false
        };
        if let Some(i) = find(focus).filter(|i| valid && eligible.contains(i)) {
            matched.push(i);
            "recent_action"
        } else {
            "unresolved_reference"
        }
    } else if matched.len() == 1 {
        "phrase"
    } else if matched.len() > 1 {
        "ambiguous"
    } else {
        "unmatched"
    };
    let selected = if reason == "unresolved_reference" { eligible } else { matched };
    let candidates=selected.into_iter().map(|i|obj! {"id"=>entities[i].get("id").expect("id").clone(),"label"=>entities[i].get("label").expect("label").clone()}).collect::<Vec<_>>();
    Ok(obj! {"reason"=>reason,"candidates"=>candidates})
}
