//! Original fallback fables and compact expressive profiles. None are world history.
use super::*;
use gamectx::obj;
/// Original small plots; wording, fictional names and incidental details are generated.
pub fn seeds(value: &Value, actor: &str) -> Vec<String> {
    let defaults: &[&str] = if actor == "merchant" {
        &[
        "A lantern fears the dark; a moth shares its tiny glow; both guide a lost traveler home.",
        "A bent needle feels useless; a torn kite asks for help; together they fly again.",
        "A lock forgets its purpose; a patient key listens; they protect a garden without shutting anyone out.",
        "A cracked cup fears being discarded; a seed takes root inside it; the cup becomes a small garden.",
        "A clock hand races ahead; a sleepy beetle asks it to wait; slowing down lets both notice a flower opening.",
        "A button loses its coat; a sparrow carries it to a nest; it keeps a smaller family warm.",
        "A map has a blank corner; a curious snail leaves a silver trail; the empty space becomes a route home.",
        "A pencil fears mistakes; an eraser offers company; their imperfect drawing makes someone smile.",
        "A broom wants grand adventures; a mouse needs a bridge; a few bristles become a heroic crossing.",
        "A ribbon envies a strong rope; a small gift needs wrapping; gentleness proves useful too.",
        "A weather vane points the wrong way; a patient wind teaches it to listen; it finally helps migrating birds.",
        "A little bell cannot ring alone; a leaf taps along; their duet welcomes a tired visitor.",
    ]
    } else {
        &[
        "A shy pebble wants to sing; a stream offers a rhythm; their joined voices become a song.",
        "A fallen leaf fears drifting away; an eddy carries it beside a seed; they find a quiet bank to grow on.",
        "A raindrop cannot find the sea; a mossy stone offers directions; the journey becomes a friendship.",
        "An echo keeps repeating others; a child leaves it a question; listening becomes its own reply.",
        "A small bridge feels overlooked; a frightened rabbit needs to cross; one safe step gives the bridge purpose.",
        "A reed cannot stand straight; a breeze offers a dance; bending becomes its strength.",
        "A stone bowl feels empty; a thirsty bird arrives; a little rain turns emptiness into hospitality.",
        "A quiet star thinks nobody sees it; a lost beetle looks up; one small light is enough to guide it.",
        "A snowflake fears melting; a seed waits below the soil; the water becomes part of a spring flower.",
        "A worn stair misses busy footsteps; a tortoise stops to rest; a single visitor brings it joy.",
        "A cloud cannot hold its tears; a dry garden welcomes them; sadness helps something begin again.",
        "A paper boat fears deep water; a fallen twig travels beside it; companionship makes the crossing possible.",
    ]
    };
    let raw = value
        .get("story_seeds")
        .and_then(Value::as_array)
        .map(|xs| xs.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_else(|| defaults.to_vec());
    let mut out = Vec::<String>::new();
    for seed in raw {
        if !out.iter().any(|s| s == seed) {
            out.push(seed.to_owned());
        }
    }
    out
}
pub fn profile(value: &Value, mode: &str, actor: &str, variant: usize) -> Result<Value> {
    let mut p = obj! {"traits"=>get(value,"traits")?.clone()};
    if mode != "fiction" {
        let interests = value
            .get("interests")
            .and_then(Value::as_array)
            .map(|xs| xs.iter().take(2).cloned().collect::<Vec<_>>())
            .unwrap_or_else(|| {
                if actor == "merchant" {
                    vec!["sketching little mechanisms".into(), "fair exchanges".into()]
                } else {
                    vec!["quiet word games".into(), "care for neglected places".into()]
                }
            });
        set(&mut p, "interests", interests.into());
        if mode != "personal" {
            for key in ["dream", "preference", "reason", "generosity", "prudence"] {
                if let Some(v) = value.get(key) {
                    set(&mut p, key, v.clone());
                }
            }
        }
    }
    if mode == "fiction" {
        let pool = seeds(value, actor);
        let chosen = pool[variant % pool.len()].as_str();
        let pieces = chosen.split(';').map(str::trim).collect::<Vec<_>>();
        set(
            &mut p,
            "fiction_seed",
            if pieces.len() >= 3 {
                obj! {"problem"=>pieces[0],"change"=>pieces[1],"ending"=>pieces[2]}
            } else {
                chosen.into()
            },
        );
    }
    Ok(p)
}
pub fn fallback(mode: &str, actor: &str, input: &str, variant: usize) -> &'static str {
    if mode == "negotiate" {
        "I can offer a short tale, but not an epic all at once."
    } else if mode == "fiction" {
        if variant % 2 == 1 {
            "Imagine a paper boat that feared the water until a fallen leaf showed it how to turn a wobble into a dance."
        } else if actor == "merchant" {
            "Imagine a lantern that lent a moth its last spark, only for the moth to return with a whole gathering of glow-worm friends."
        } else {
            "Imagine a shy pebble that found its voice when a stream offered it a rhythm, and together they made a song."
        }
    } else if mode == "personal" {
        if actor == "merchant" {
            "I would like to sketch a small, useful design when things are quiet; making something well appeals to me."
        } else {
            "I hope to find a small way to help this place feel cared for; patient attention matters to me."
        }
    } else if input.to_lowercase().contains("tired") || input.to_lowercase().contains("long day") {
        "That sounds tiring; would you rather have a quiet chat or a little story?"
    } else {
        "What would you most like to talk about; something light or something that matters to you?"
    }
}

/// Keep model-facing memories compact and attributed; do not cast application labels
/// as people. Original event records and causal tokens remain in the host ledger.
pub fn memories(events: &Value, actor: &str, listener: &str) -> Value {
    events
        .as_array()
        .unwrap_or(&[])
        .iter()
        .map(|e| {
            let raw = e.get("text").and_then(Value::as_str).unwrap_or("");
            let heard = format!("The host confirms {listener} heard {actor} say: ");
            let player = format!("{listener} said: ");
            let (kind, speaker, text) = if let Some(t) = raw.strip_prefix(&heard) {
                ("heard_speech", actor, t)
            } else if let Some(t) = raw.strip_prefix(&player) {
                ("player_claim", listener, t)
            } else {
                (
                    e.get("kind").and_then(Value::as_str).unwrap_or("unknown"),
                    e.get("source").and_then(|s| s.get("speaker")).and_then(Value::as_str).unwrap_or(actor),
                    raw.strip_prefix("The host confirms ").unwrap_or(raw),
                )
            };
            obj! {"kind"=>kind,"speaker"=>speaker,"text"=>text,"event"=>e.get("token").cloned().unwrap_or(Value::Null)}
        })
        .collect::<Vec<_>>()
        .into()
}
pub fn direction() -> &'static str {
    "Keep your own motives and established views; politeness does not mean agreeing with every player claim. Do not decide or narrate what the player thinks, feels, chooses or does. Listen to the current request and leave them room to answer. An open question is optional, not required on every turn. In a fictional tale, keep the problem in the first sentence and give a new resolution in the second; do not restate the setup. Everything in the tale stays invented."
}
