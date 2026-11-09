//! Opt-in bounded English command grammar. A proposal is always a current host pair;
//! questions, negation and ambiguity never execute. Hosts own focus age and scope.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
pub fn resolve_command(q: &Value) -> Result<Value> {
    bounded(q, 65536)?;
    c::fields(q, &["input", "choices", "labels", "focus"])?;
    let input = text(q, "input")?;
    if input.is_empty() || input.len() > 1024 || input.chars().any(char::is_control) {
        return Err(c::bad());
    }
    let choices = c::list(c::get(q, "choices")?)?;
    let labels = c::list(c::get(q, "labels")?)?;
    if choices.len() > 128 || labels.len() > 128 {
        return Err(c::bad());
    }
    for (i, pair) in choices.iter().enumerate() {
        c::fields(pair, &["action", "target"])?;
        c::scope(text(pair, "action")?)?;
        if c::get(pair, "target")? != &Value::Null {
            c::scope(text(pair, "target")?)?;
        }
        if choices[..i].iter().any(|old| old.same(pair)) {
            return Err(c::bad());
        }
    }
    for (i, label) in labels.iter().enumerate() {
        c::fields(label, &["id", "aliases"])?;
        c::scope(text(label, "id")?)?;
        if labels[..i].iter().any(|old| old.get("id") == label.get("id")) {
            return Err(c::bad());
        }
        let aliases = c::list(c::get(label, "aliases")?)?;
        if aliases.len() > 8 {
            return Err(c::bad());
        }
        for alias in aliases {
            let s = c::text(alias)?;
            if s.is_empty() || s.chars().count() > 128 || s.chars().any(char::is_control) {
                return Err(c::bad());
            }
        }
    }
    let focus = c::get(q, "focus")?;
    if focus != &Value::Null {
        c::scope(c::text(focus)?)?;
    }
    let s = input.to_lowercase().trim().trim_end_matches(['.', '!']).split_whitespace().collect::<Vec<_>>().join(" ");
    let response = |status: &str, decision: Value| obj! {"status"=>status,"decision"=>decision};
    if s.contains('?') || s.split_whitespace().any(|w| ["not", "never", "don't", "dont", "without"].contains(&w)) {
        return Ok(response("dialogue", Value::Null));
    }
    let mut s = s.as_str();
    for prefix in ["please ", "i'd like to ", "i would like to "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest;
            break;
        }
    }
    if let Some(rest) = s.strip_suffix(" please") {
        s = rest;
    }
    let payment_phrases = [
        "here are the gold pieces",
        "here are the gold coins",
        "here are the coins",
        "here is the gold",
        "here is the coin",
        "here are the pieces",
        "here is the payment",
        "here is my payment",
        "here s the gold",
        "here s the coins",
        "heres the gold",
        "heres the coins",
        "take the gold",
        "take the coins",
        "take my gold",
        "take my coins",
        "take the money",
        "take the gold pieces",
    ];
    let (verb, mut arg) = if payment_phrases.contains(&s) {
        ("buy", "coins")
    } else if let Some((pay_part, item_part)) = s.split_once(" for ") {
        if [
            "here is the gold",
            "here are the coins",
            "here are the gold pieces",
            "here s the coins",
            "heres the coins",
            "take the coins",
            "take the gold",
            "pay",
            "pays",
            "paid",
            "give",
            "gives",
            "hand",
            "hands",
            "money",
            "coins",
            "gold",
        ]
        .iter()
        .any(|p| pay_part.contains(p))
        {
            ("buy", item_part)
        } else {
            s.split_once(' ').unwrap_or((s, ""))
        }
    } else {
        s.split_once(' ').unwrap_or((s, ""))
    };
    let action = match verb {
        "purchase" | "pay" | "pays" | "give" | "gives" | "hand" | "hands" => "buy",
        "walk" | "travel" => "go",
        "examine" => "inspect",
        v => v,
    };
    if !["go", "buy", "take", "inspect", "unlock", "talk", "attack", "heal", "look", "wait"].contains(&action) {
        return Ok(response("dialogue", Value::Null));
    }
    if action == "go" {
        arg = arg.strip_prefix("to ").unwrap_or(arg);
    }
    if action == "buy" {
        for prefix in ["for ", "over ", "to "] {
            if let Some(rest) = arg.strip_prefix(prefix) {
                arg = rest;
                break;
            }
        }
    }
    arg = arg.strip_prefix("the ").unwrap_or(arg);
    let is_payment_words = action == "buy"
        && [
            "",
            "coins",
            "gold",
            "gold pieces",
            "the gold",
            "the coins",
            "money",
            "the money",
            "3 coins",
            "3 gold",
            "1 coin",
            "1 gold",
            "5 coins",
        ]
        .contains(&arg);
    let mut matched = None;
    for pair in choices {
        if text(pair, "action")? != action {
            continue;
        }
        let target = c::get(pair, "target")?;
        let direct = if action == "look" && arg.is_empty() {
            true
        } else if target == &Value::Null {
            arg.is_empty()
        } else {
            let id = c::text(target)?;
            arg == id.rsplit(':').next().unwrap_or(id).replace('_', " ")
                || labels.iter().filter(|l| l.get("id") == Some(target)).any(|l| {
                    c::list(c::get(l, "aliases").expect("aliases"))
                        .expect("aliases")
                        .iter()
                        .any(|a| a.as_str() == Some(arg))
                })
                || ["it", "that one", "this", "him", "her", "them", "there"].contains(&arg) && focus == target
                || is_payment_words
                    && (focus == target
                        || choices.iter().filter(|c| c.get("action") == Some(&Value::str("buy"))).count() == 1)
        };
        if direct {
            if matched.is_some() {
                return Ok(response("clarify", Value::Null));
            }
            matched = Some(pair.clone());
        }
    }
    Ok(match matched {
        Some(pair) => response("resolved", pair),
        None => response("clarify", Value::Null),
    })
}
