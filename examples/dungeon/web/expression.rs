//! Original fallback fables and compact expressive profiles. None are world history.
use super::*;
use gamectx::obj;
pub fn seed(actor: &str) -> &'static str {
    if actor == "merchant" {
        "A lantern gives a moth its last spark; the moth gathers glow-worm friends; together they light the path."
    } else {
        "A pebble is too shy to sing; a stream offers it a rhythm; together their small voices become a song."
    }
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
        let seeds = value.get("story_seeds").and_then(Value::as_array).filter(|s| !s.is_empty());
        let chosen = seeds.and_then(|s| s.get(variant % s.len())).and_then(Value::as_str).unwrap_or(seed(actor));
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
