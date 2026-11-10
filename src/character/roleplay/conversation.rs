//! Conservative intent hints for optional social speech; never world/action authority.
use crate::{checked as c, obj, Result, Value};
/// Classify small talk and scoped follow-ups before a host chooses its speech contract.
/// Unrecognized/factual requests stay on the factual path. `previous` must come from
/// an utterance the current NPC confirmed heard by this listener, not a model draft.
pub fn conversation_route(v: &Value) -> Result<Value> {
    if !v.is_valid(4096) {
        return Err(c::bad());
    }
    c::fields(v, &["input", "previous"])?;
    let input = c::text(c::get(v, "input")?)?;
    if input.is_empty() || input.len() > 1024 || input.chars().any(char::is_control) {
        return Err(c::bad());
    }
    let previous = c::get(v, "previous")?;
    if previous != &Value::Null {
        let p = c::text(previous)?;
        if p.is_empty() || p.len() > 64 || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:-".contains(&b)) {
            return Err(c::bad());
        }
    }
    let normalized =
        input.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>();
    let q = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    let facts="born birthplace weather password watchword stock sell price prices cost costs coin coins money lend loan borrow gift promise promises owe room vault key door inscription history family parents name";
    let factual = q.split_whitespace().any(|w| facts.split_whitespace().any(|f| f == w));
    let (mut kind, mut topic) = ("other", Value::Null);
    if !factual {
        if [
            "how are you",
            "how do you feel",
            "how are things",
            "how s it going",
            "how is it going",
            "are you doing okay",
            "are you alright",
            "are you ok",
            "how is your day",
            "how is your day today",
            "how s your day",
            "how has your day been",
            "how is your day going",
            "how are you today",
            "how do you do",
            "how goes it",
        ]
        .iter()
        .any(|p| q.contains(p))
            && !["feel about", "feel on"].iter().any(|p| q.contains(p))
        {
            kind = "wellbeing";
            topic = "wellbeing".into();
        } else if [
            "hi",
            "hello",
            "hey",
            "greetings",
            "good morning",
            "good evening",
            "hello there",
            "hi there",
            "hello again",
            "hi again",
            "hey again",
            "nice to see you",
            "nice to see you again",
            "good to see you again",
        ]
        .contains(&q.as_str())
        {
            kind = "greeting";
        } else if ["thanks", "thank you", "thanks a lot", "thank you very much"].contains(&q.as_str()) {
            kind = "thanks";
        } else if ["bye", "goodbye", "see you", "see you later", "farewell"].contains(&q.as_str()) {
            kind = "farewell";
        } else if ["why", "why is that", "tell me more", "go on", "what do you mean", "can you explain", "how come"]
            .contains(&q.as_str())
        {
            kind = "elaborate";
            topic = previous.clone();
        } else if [
            "what do you like to do",
            "what do you enjoy",
            "your interests",
            "your hobbies",
            "do you do anything for fun",
            "what do you do for fun",
            "what do you do in your free time",
            "in your free time",
            "for fun",
            "how do you spend your time",
            "pass the time",
        ]
        .iter()
        .any(|p| q.contains(p))
        {
            kind = "personal";
            topic = "hobbies".into();
        } else if [
            "do you need anything",
            "what do you need",
            "need any help",
            "can i help you with anything",
            "is there anything you need",
            "need anything",
            "can i help you",
        ]
        .iter()
        .any(|p| q.contains(p))
        {
            kind = "needs";
            topic = "needs".into();
        } else if [
            "where are we",
            "where am i",
            "what is this place",
            "where are we right now",
            "tell me about this place",
        ]
        .iter()
        .any(|p| q.contains(p))
        {
            kind = "location";
            topic = "location".into();
        } else if [
            "where should i go",
            "where should i go now",
            "where do i go",
            "where do we go",
            "which way should i go",
            "where to next",
            "what should i do next",
        ]
        .iter()
        .any(|p| q.contains(p))
        {
            kind = "navigation";
            topic = "navigation".into();
        } else if ["favorite", "favourite", "your opinion", "your thoughts", "makes you happy", "makes you sad"]
            .iter()
            .any(|p| q.contains(p))
        {
            kind = "social";
            topic = "conversation".into();
        }
    }
    Ok(obj! {"kind"=>kind,"topic"=>topic})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_route_wellbeing_and_hobbies() {
        let res = conversation_route(&obj! {"input" => "how is your day today?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("wellbeing")));

        let res = conversation_route(&obj! {"input" => "do you do anything for fun?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("personal")));
        assert_eq!(res.get("topic"), Some(&Value::str("hobbies")));

        let res = conversation_route(&obj! {"input" => "do you need anything?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("needs")));

        let res = conversation_route(&obj! {"input" => "where are we?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("location")));

        let res = conversation_route(&obj! {"input" => "where should i go now?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("navigation")));
    }
}
