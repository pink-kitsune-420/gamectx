//! Optional example host guard. Only exact visible entity aliases and unambiguous
//! English action words narrow the candidates; this is not general intent inference.
use gamectx::Value;
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).map(str::to_owned).collect()
}
fn contains(hay: &[String], needle: &[String]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}
pub fn filter(pairs: &[Value], items: &[Value], input: &str) -> Vec<Value> {
    let wanted = words(input);
    let mut targets = Vec::<String>::new();
    for id in pairs
        .iter()
        .filter_map(|p| p.get("target").and_then(Value::as_str))
        .chain(items.iter().filter_map(|p| p.get("id").and_then(Value::as_str)))
    {
        if !targets.iter().any(|s| s == id) {
            targets.push(id.into());
        }
    }
    let matched = targets
        .iter()
        .filter(|target| {
            contains(&wanted, &words(target.split(':').next_back().expect("target")))
                || items.iter().any(|item| {
                    item.get("id").and_then(Value::as_str) == Some(target.as_str())
                        && item
                            .get("label")
                            .and_then(Value::as_str)
                            .is_some_and(|label| contains(&wanted, &words(label)))
                })
        })
        .collect::<Vec<_>>();
    if matched.len() != 1 {
        return pairs.to_vec();
    }
    let actions = ["go", "look", "inspect", "wait", "take", "buy", "unlock", "talk", "attack", "heal"]
        .into_iter()
        .filter(|a| wanted.iter().any(|w| w == a))
        .collect::<Vec<_>>();
    let negative = ["not", "don", "never", "without"].iter().any(|a| wanted.iter().any(|w| w == a));
    pairs
        .iter()
        .filter(|p| {
            p.get("target").and_then(Value::as_str) == Some(matched[0].as_str())
                && (negative
                    || actions.len() != 1
                    || p.get("action").and_then(Value::as_str) == Some(actions[0])
                    || actions[0] == "look" && p.get("action").and_then(Value::as_str) == Some("inspect"))
        })
        .cloned()
        .collect()
}

#[derive(Clone, Debug)]
pub struct Resolution {
    pub pairs: Vec<Value>,
    pub reason: &'static str,
    pub candidates: Vec<Value>,
}
/// Phrase overlap is resolved by span containment, not by guessing a target. Explicit
/// multiple entities and missing pronoun context ask the human to clarify.
pub fn resolve(
    pairs: &[Value],
    items: &[Value],
    input: &str,
    focus: Option<&str>,
    hint: Option<&str>,
) -> Result<Resolution, gamectx::Error> {
    let wanted = words(input);
    let mut targets = Vec::<String>::new();
    for id in pairs
        .iter()
        .filter_map(|p| p.get("target").and_then(Value::as_str))
        .chain(items.iter().filter_map(|p| p.get("id").and_then(Value::as_str)))
    {
        if !targets.iter().any(|s| s == id) {
            targets.push(id.into());
        }
    }
    let mut spans = Vec::new();
    for (i, target) in targets.iter().enumerate() {
        let mut aliases = vec![words(target.split(':').next_back().expect("target"))];
        aliases.extend(
            items
                .iter()
                .filter(|p| p.get("id").and_then(Value::as_str) == Some(target))
                .filter_map(|p| p.get("label").and_then(Value::as_str))
                .map(words),
        );
        aliases.extend(
            items
                .iter()
                .filter(|p| p.get("id").and_then(Value::as_str) == Some(target))
                .filter_map(|p| p.get("kind").and_then(Value::as_str))
                .map(words),
        );
        if target.starts_with("door:") {
            aliases.push(words("door"));
            aliases.push(words("gate"));
        }
        for alias in aliases {
            if !alias.is_empty() {
                for (start, w) in wanted.windows(alias.len()).enumerate() {
                    if w == alias {
                        spans.push((i, start, start + alias.len()));
                    }
                }
            }
        }
    }
    let mut matched = Vec::new();
    for &(i, start, end) in &spans {
        if !spans.iter().any(|&(_, a, b)| a <= start && b >= end && b - a > end - start) && !matched.contains(&i) {
            matched.push(i);
        }
    }
    let pronoun = wanted.iter().any(|w| ["it", "this", "that", "there"].contains(&w.as_str()))
        || contains(&wanted, &words("that one"))
        || contains(&wanted, &words("this one"));
    let verbs = ["go", "inspect", "take", "buy", "unlock", "talk", "attack", "heal"]
        .into_iter()
        .filter(|a| wanted.iter().any(|w| w == a))
        .collect::<Vec<_>>();
    let prefix = if verbs.len() == 1 {
        match verbs[0] {
            "go" => Some("room:"),
            "take" | "buy" | "heal" => Some("item:"),
            "unlock" => Some("door:"),
            "talk" | "attack" => Some("npc:"),
            _ => None,
        }
    } else {
        None
    };
    let raw_matched = matched.clone();
    if let Some(prefix) = prefix {
        matched.retain(|&i| targets[i].starts_with(prefix));
    }
    let reason = if let Some(hint) = hint {
        matched = vec![targets
            .iter()
            .position(|id| id == hint)
            .ok_or_else(|| gamectx::Error::new("target hint is unavailable"))?];
        "explicit_target"
    } else if matched.is_empty() && !raw_matched.is_empty() {
        "type_mismatch"
    } else if matched.is_empty() && pronoun {
        if let Some(i) = focus.and_then(|f| targets.iter().position(|id| id == f)) {
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
    let candidates = if matched.is_empty() && reason == "unresolved_reference" {
        (0..targets.len()).filter(|&i| prefix.is_none_or(|p| targets[i].starts_with(p))).collect::<Vec<_>>()
    } else {
        matched.clone()
    };
    let candidates = candidates
        .into_iter()
        .map(|i| {
            let id = &targets[i];
            let label = items
                .iter()
                .find(|p| p.get("id").and_then(Value::as_str) == Some(id))
                .and_then(|p| p.get("label"))
                .cloned()
                .unwrap_or_else(|| Value::str(id.split(':').next_back().expect("target").replace('_', " ")));
            gamectx::obj! {"id"=>id.clone(),"label"=>label}
        })
        .collect();
    let filtered = if ["ambiguous", "unresolved_reference", "type_mismatch"].contains(&reason) {
        Vec::new()
    } else if matched.is_empty() {
        pairs.to_vec()
    } else {
        let actions = ["go", "look", "inspect", "wait", "take", "buy", "unlock", "talk", "attack", "heal"]
            .into_iter()
            .filter(|a| wanted.iter().any(|w| w == a))
            .collect::<Vec<_>>();
        let negative = ["not", "don", "never", "without"].iter().any(|a| wanted.iter().any(|w| w == a));
        pairs
            .iter()
            .filter(|p| {
                p.get("target").and_then(Value::as_str) == Some(targets[matched[0]].as_str())
                    && (negative
                        || actions.len() != 1
                        || p.get("action").and_then(Value::as_str) == Some(actions[0])
                        || actions[0] == "look" && p.get("action").and_then(Value::as_str) == Some("inspect"))
            })
            .cloned()
            .collect()
    };
    Ok(Resolution { pairs: filtered, reason, candidates })
}
