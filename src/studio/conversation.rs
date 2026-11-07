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
        } else if ["what do you like to do", "what do you enjoy", "your interests", "your hobbies"]
            .iter()
            .any(|p| q.contains(p))
        {
            kind = "personal";
            topic = "hobbies".into();
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
