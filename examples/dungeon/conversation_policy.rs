//! Small example English policies; language and world content remain host choices.
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}
fn phrases(s: &str, patterns: &[&str]) -> bool {
    let w = words(s);
    patterns.iter().any(|p| {
        let n = words(p);
        w.windows(n.len()).any(|v| v == n)
    })
}
fn past(s: &str) -> bool {
    phrases(s, &["did i", "did you", "told", "said", "yesterday", "previously", "tomorrow"])
}
pub fn identity(s: &str) -> bool {
    !past(s) && phrases(s, &["your name", "you called", "call you", "who are you", "introduce yourself"])
}
pub fn biography(s: &str) -> bool {
    !past(s) && phrases(s, &["story", "background", "job", "work", "yourself"])
}
pub fn opinion(s: &str) -> bool {
    phrases(s, &["what do you think", "how do you feel", "do you like", "would you like"])
}
pub fn weather_report(s: &str) -> bool {
    phrases(s, &["hot", "cold", "warm", "sunny", "rainy", "raining", "snowing"])
        && phrases(s, &["day", "today", "weather", "outside"])
}
pub fn location(s: &str) -> bool {
    phrases(s, &["where are we", "where am i", "what is this place", "where is this", "describe this place"])
}
pub fn world(s: &str) -> bool {
    phrases(
        s,
        &[
            "what do you know about this world",
            "what can you tell me about this world",
            "tell me about this world",
            "what is this world",
        ],
    )
}
pub fn overview(s: &str) -> bool {
    let w = words(s);
    let fill =
        "hello hi hey please thanks now here on about topics anything else".split_whitespace().collect::<Vec<_>>();
    [
        "what do you have information on",
        "what do you know",
        "what can you help with",
        "what can you tell me",
        "what information do you have",
        "anything else",
        "what else",
    ]
    .into_iter()
    .any(|p| {
        let n = words(p);
        w.windows(n.len())
            .enumerate()
            .any(|(i, v)| v == n && w[..i].iter().chain(w[i + n.len()..].iter()).all(|x| fill.contains(&x.as_str())))
    })
}
pub fn stock_extra(s: &str) -> bool {
    phrases(s, &["do you have anything else", "anything else for sale", "any other stock"])
}
pub fn meaningful(s: &str) -> String {
    let stop="a an the is are was were am do does did i my me you your we our it its this that what who when where why how about on in of to for with can could should would will have has had please hello hi hey know information anything else tell say".split_whitespace().collect::<Vec<_>>();
    words(s).into_iter().filter(|w| !stop.contains(&w.as_str())).collect::<Vec<_>>().join(" ")
}
pub fn unknown(speaker: &str) -> &'static str {
    match speaker {
        "merchant" => "I couldn't tell you about that. My trade is workshop supplies and repairs.",
        "keeper" => "I couldn't tell you about that. I know the cistern and the old vault.",
        _ => "Nothing in the current scene tells us that.",
    }
}

/// Bounded example syntax; counts the literal supplied text, not model arithmetic.
pub fn letter_request(input: &str) -> Option<&str> {
    let s = input.trim();
    if let Some(prefix) = ["how many letters are in ", "how many letters in "]
        .into_iter()
        .find(|p| s.get(..p.len()).is_some_and(|v| v.eq_ignore_ascii_case(p)))
    {
        let text = s.get(prefix.len()..)?.trim().trim_end_matches('?').trim().trim_matches(['"', '\'', '“', '”']);
        return (!text.is_empty() && text.chars().count() <= 80 && !text.chars().any(char::is_control)).then_some(text);
    }
    let lower = s.to_ascii_lowercase();
    let pos = lower.find(" has ")?;
    let tail = lower[pos + 5..].trim_end_matches(['.', '!']).strip_suffix(" letters")?;
    let text = s.get(..pos)?.trim();
    (!tail.is_empty()
        && tail.bytes().all(|b| b.is_ascii_digit())
        && !text.is_empty()
        && text.chars().count() <= 80
        && !text.chars().any(char::is_control))
    .then_some(text)
}

pub fn time_question(s: &str) -> bool {
    phrases(s, &["what time is it", "what day is it", "what is the time", "what day are we on", "current world time"])
}

pub fn time_report(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    ["the time is ", "my watch says ", "the clock says ", "it is noon", "it is midnight"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        && !s.contains('?')
}
