//! Host-only greeting wording; plans consume perceived place and confirmed encounters.
use super::*;
use gamectx::{obj, studio::GreetingMemory};
pub fn new(dir: &std::path::Path) -> Result<GreetingMemory> {
    let p = dir.join("greetings.json");
    let config = if p.exists() {
        parse(&read_file(&p, 1024)?)?
    } else {
        obj! {"version"=>1,"long_gap_turns"=>12}
    };
    GreetingMemory::new(&config)
}
pub fn configure(memory: &mut GreetingMemory, dir: &std::path::Path, v: &Value) -> Result<()> {
    let mut next = memory.clone();
    next.configure(v)?;
    atomic(&dir.join("greetings.json"), v.to_json().as_bytes())?;
    *memory = next;
    Ok(())
}
pub fn name(actor: &str) -> Result<&'static str> {
    match actor {
        "merchant" => Ok("Ilen"),
        "keeper" => Ok("Tovin"),
        _ => Err(err()),
    }
}
pub fn visit(actions: &[Value]) -> Result<u64> {
    actions
        .iter()
        .rev()
        .find(|a| a.get("decision").and_then(|d| d.get("action")).and_then(Value::as_str) == Some("go"))
        .map_or(Ok(0), |a| Ok(num(a, "revision", u32::MAX as u64 - 1)? + 1))
}
/// Strip only an exact known addressee from an otherwise pure greeting, not a question.
pub fn addressed(input: &str, actor: &str) -> String {
    let q = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if let Ok(name) = name(actor) {
        for opener in ["hello", "hi", "hey", "greetings"] {
            for who in [name.to_lowercase(), actor.to_owned(), format!("the {actor}")] {
                if q == format!("{opener} {who}") {
                    return opener.into();
                }
            }
        }
    }
    input.into()
}
/// Import a conservative familiar contact from actual heard legacy replies/viewpoints.
/// No venue or previous introduction is inferred. Tokens are new host import receipts.
pub fn seed_legacy(
    memory: &mut GreetingMemory,
    receipts: &[Value],
    views: &Value,
    at: u64,
    seq: &mut u64,
) -> Result<()> {
    let mut pairs = Vec::<(String, String)>::new();
    for row in receipts {
        let actor = text(row, "speaker")?;
        if !["merchant", "keeper"].contains(&actor) {
            continue;
        }
        let heard = get(row, "heard")?.as_array().ok_or_else(err)?;
        for listener in get(row, "recipients")?.as_array().ok_or_else(err)? {
            let Some(listener) = listener.as_str() else {
                continue;
            };
            if ["player_1", "player_2"].contains(&listener)
                && heard.iter().any(|p| p.as_str() == Some(listener))
                && !text(row, "text")?.is_empty()
            {
                pairs.push((actor.into(), listener.into()));
            }
        }
    }
    for e in get(views, "entries")?.as_array().ok_or_else(err)? {
        pairs.push((text(e, "actor")?.into(), text(e, "listener")?.into()));
    }
    for (actor, listener) in pairs {
        if memory.contact(&actor, &listener).is_some() {
            continue;
        }
        let token = seq.checked_add(1).filter(|n| *n <= u32::MAX as u64).ok_or_else(err)?;
        memory.seed(&obj! {"actor"=>actor,"listener"=>listener,"at"=>at,"token"=>token,"introduced"=>false})?;
        *seq = token;
    }
    Ok(())
}
/// The caller supplies only the active perceived scene label. No lore/global fact lookup.
pub fn render(plan: &Value, name: &str, label: &str, recent: &[String]) -> Result<Value> {
    if name.is_empty()
        || name.chars().count() > 32
        || label.is_empty()
        || label.chars().count() > 48
        || name.chars().chain(label.chars()).any(char::is_control)
    {
        return Err(err());
    }
    let meeting = text(plan, "meeting")?;
    let relationship = text(plan, "relationship")?;
    let atmosphere = text(plan, "atmosphere")?;
    let place = text(plan, "place")?;
    let needs = get(plan, "needs_introduction")? == &Value::Bool(true);
    let urgent = atmosphere == "emergency" || place == "hazard";
    let options: Vec<String> = if urgent {
        vec![
            "I'm listening; we should keep this brief while things are urgent.".into(),
            "Hello; let us keep this short for now.".into(),
            "I hear you; things are urgent, so please be brief.".into(),
        ]
    } else if needs {
        if atmosphere == "busy" {
            vec![
                format!("I'm {name}; things are busy, but I can listen."),
                format!("Hello, I'm {name}; please keep it brief while things are busy."),
                format!("You can call me {name}; what is on your mind?"),
            ]
        } else if relationship == "guarded" {
            vec![
                format!("I'm {name}; I would prefer we take this slowly."),
                format!("You can call me {name}; let us keep this civil."),
                format!("I'm {name}; I can listen, though I am cautious with you."),
            ]
        } else if meeting == "first" && place == "workplace" {
            vec![
                format!("Hello, I'm {name}; welcome to {label}."),
                format!("Welcome to {label}; you can call me {name}."),
                format!("I'm {name}; it is good to meet you here in {label}."),
            ]
        } else if meeting == "first" {
            vec![
                format!("Hello, I'm {name}; it is good to meet you here."),
                format!("I'm {name}; hello there."),
                format!("Hello; you can call me {name}."),
            ]
        } else if meeting == "returning" {
            vec![
                format!("Welcome back to {label}; you can call me {name}."),
                format!("Hello again; I'm {name}, in case we never traded names."),
                format!("You are back; you can call me {name}."),
            ]
        } else {
            vec![
                format!("Hello again; you can call me {name}."),
                format!("I'm {name}; it is good to talk again."),
                format!("Hello; we can trade names now, if you like: I'm {name}."),
            ]
        }
    } else if relationship == "guarded" {
        if meeting == "continuing" {
            vec![
                "I'm listening; let us take this slowly.".into(),
                "Go on; I can hear you.".into(),
                "I can listen if we keep this civil.".into(),
            ]
        } else {
            vec![
                "Hello again; I am cautious with you, but I can listen.".into(),
                "We can talk, though I would prefer to go slowly.".into(),
                "Hello; what would you like to say?".into(),
            ]
        }
    } else if atmosphere == "busy" {
        vec![
            "I'm listening, though things are busy here.".into(),
            "I can hear you; a brief chat would help while things are busy.".into(),
            "Hello; what is on your mind while things are so busy?".into(),
        ]
    } else if atmosphere == "tense" {
        vec![
            "Hello; let us keep this calm.".into(),
            "I'm listening; calm words would be welcome.".into(),
            "We can talk, though things feel tense.".into(),
        ]
    } else if meeting == "continuing" {
        vec![
            "I'm still listening; what is on your mind?".into(),
            "Go on; I'm listening.".into(),
            "I'm here; what would you like to talk about?".into(),
        ]
    } else if meeting == "returning" && place == "workplace" {
        if relationship == "warm" {
            vec![
                format!("Good to see you again; welcome back to {label}."),
                format!("Welcome back to {label}; I'm glad to see you."),
                "It is good to see you back; what is on your mind?".into(),
            ]
        } else {
            vec![
                format!("Welcome back to {label}; what is on your mind?"),
                "Hello again; it is good to see you back.".into(),
                format!("You are back at {label}; how are you?"),
            ]
        }
    } else if meeting == "elsewhere" {
        if place == "private" {
            vec![
                format!("Hello again; we can speak quietly here in {label}."),
                "It is good to see you here; we can keep our voices low.".into(),
                "Hello again; I'm listening.".into(),
            ]
        } else {
            vec![
                format!("Good to see you here in {label}."),
                format!("Hello again; what brings you to {label}?"),
                "It is good to meet up in another place; how are you?".into(),
            ]
        }
    } else if meeting == "after_gap" {
        vec![
            "It is good to talk again; how are you?".into(),
            "Hello again; what is on your mind now?".into(),
            "We have a moment to talk again; I'm listening.".into(),
        ]
    } else {
        vec![
            "Hello again; what is on your mind?".into(),
            "Good to see you; how are you?".into(),
            "It is good to talk again; I'm listening.".into(),
        ]
    };
    let line = options.iter().find(|line| !recent.contains(line)).unwrap_or(&options[0]);
    if line.chars().count() > 240 {
        return Err(err());
    }
    Ok(obj! {"text"=>line.clone(),"introduced"=>needs&&!urgent})
}

/// An explicit roleplay action proposal. Only these finite stage directions are parsed;
/// the host must still validate proximity, travel permissions, floor and revision.
#[derive(Debug, PartialEq)]
pub enum Direction {
    Approach(&'static str),
    Enter(&'static str),
    Unsupported,
}
pub fn direction(input: &str) -> Option<Direction> {
    if input.len() > 1024 || input.chars().any(char::is_control) {
        return Some(Direction::Unsupported);
    }
    let s = input.trim();
    let body = s.strip_prefix('*')?.strip_suffix('*')?;
    if body.is_empty() {
        return None;
    }
    let q = body.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let q = q.trim_end_matches(['.', '!']);
    for prefix in ["approaches ", "approach ", "walks over to ", "walk over to "] {
        if let Some(target) = q.strip_prefix(prefix) {
            return Some(match target.strip_prefix("the ").unwrap_or(target) {
                "merchant" | "ilen" => Direction::Approach("merchant"),
                "keeper" | "tovin" => Direction::Approach("keeper"),
                _ => Direction::Unsupported,
            });
        }
    }
    for prefix in ["enters ", "enter ", "walks into ", "walk into ", "goes into ", "go into "] {
        if let Some(target) = q.strip_prefix(prefix) {
            return Some(match target.strip_prefix("the ").unwrap_or(target) {
                "entry" => Direction::Enter("entry"),
                "gallery" => Direction::Enter("gallery"),
                "workshop" => Direction::Enter("workshop"),
                "cistern" => Direction::Enter("cistern"),
                "vault" => Direction::Enter("vault"),
                "exit" => Direction::Enter("exit"),
                _ => Direction::Unsupported,
            });
        }
    }
    Some(Direction::Unsupported)
}
