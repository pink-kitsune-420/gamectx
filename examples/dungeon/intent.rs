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
    let verbs = ["go", "inspect", "take", "buy", "unlock", "talk", "attack", "heal"]
        .into_iter()
        .filter(|a| wanted.iter().any(|w| w == a))
        .collect::<Vec<_>>();
    let kind = if verbs.len() == 1 {
        match verbs[0] {
            "go" => Some("room"),
            "take" | "buy" | "heal" => Some("item"),
            "unlock" => Some("door"),
            "talk" | "attack" => Some("npc"),
            _ => None,
        }
    } else {
        None
    };
    let entities=targets.iter().map(|id| {
        let suffix=id.split(':').next_back().expect("target");
        let item=items.iter().find(|v|v.get("id").and_then(Value::as_str)==Some(id));
        let label=item.and_then(|v|v.get("label")).cloned().unwrap_or_else(||Value::str(suffix.replace('_'," ")));
        let mut aliases=vec![Value::str(suffix)];
        for key in ["label","kind"] {if let Some(v)=item.and_then(|v|v.get(key)) {aliases.push(v.clone());}}
        if id.starts_with("door:") {aliases.extend([Value::str("door"),Value::str("gate")]);}
        gamectx::obj! {"id"=>id.clone(),"label"=>label,"kind"=>id.split(':').next().expect("namespace"),"aliases"=>aliases}
    }).collect::<Vec<_>>();
    let referent = |id: &str| gamectx::obj! {"id"=>id,"scope"=>"current","revision"=>0};
    // The host has already checked focus room/age and selection revision. The core
    // resolver also accepts real scope/revision metadata for other integrations.
    let resolved = gamectx::dialogue::resolve_references(&gamectx::obj! {"entities"=>entities,"input"=>input,
    "kind"=>kind.map(Value::str).unwrap_or(Value::Null),"scope"=>"current","revision"=>0,"max_age"=>0,
    "focus"=>focus.map(referent).unwrap_or(Value::Null),"selection"=>hint.map(referent).unwrap_or(Value::Null),
    "pronouns"=>vec!["it","this","that","there"]})?;
    let reason = match resolved.get("reason").and_then(Value::as_str).expect("reason") {
        "explicit_target" => "explicit_target",
        "type_mismatch" => "type_mismatch",
        "recent_action" => "recent_action",
        "unresolved_reference" => "unresolved_reference",
        "phrase" => "phrase",
        "ambiguous" => "ambiguous",
        _ => "unmatched",
    };
    let candidates = resolved.get("candidates").and_then(Value::as_array).expect("candidates").to_vec();
    let matched = candidates.iter().filter_map(|v| v.get("id").and_then(Value::as_str)).collect::<Vec<_>>();
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
                p.get("target").and_then(Value::as_str) == Some(matched[0])
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
