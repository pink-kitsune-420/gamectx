//! Conservative intent hints for optional social speech; never world/action authority.
use crate::{checked as c, obj, Result, Value};

/// Token-bounded word and phrase matcher with zero substring collision risks.
#[derive(Clone, Debug, Default)]
pub struct BoundedLexicon;

impl BoundedLexicon {
    /// Tokenizes input into lowercase alphanumeric tokens.
    pub fn tokenize(input: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        for ch in input.chars() {
            if ch.is_alphanumeric() {
                for lch in ch.to_lowercase() {
                    current.push(lch);
                }
            } else if !current.is_empty() {
                tokens.push(current);
                current = String::new();
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        tokens
    }

    /// Checks if a slice of tokens contains a target phrase as a contiguous sub-sequence.
    pub fn contains_phrase(tokens: &[String], phrase: &str) -> bool {
        let target = Self::tokenize(phrase);
        if target.is_empty() || target.len() > tokens.len() {
            return false;
        }
        tokens.windows(target.len()).any(|window| window == target.as_slice())
    }

    /// Checks if any pattern in `patterns` is matched in `tokens`.
    pub fn matches_any(tokens: &[String], patterns: &[&str]) -> bool {
        patterns.iter().any(|pat| Self::contains_phrase(tokens, pat))
    }

    /// Convenience method to tokenize input and check if any pattern matches.
    pub fn has_match(input: &str, patterns: &[&str]) -> bool {
        let tokens = Self::tokenize(input);
        Self::matches_any(&tokens, patterns)
    }
}

/// Category of intent expressed within an individual clause of an utterance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClauseIntentKind {
    Salutation,
    Perception,
    Navigation,
    LoreQuery,
    TradeProposition,
    FactAssertion,
    BoundaryProbe,
    Other,
}

/// A syntactic/semantic clause decomposed from a compound utterance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecomposedClause {
    pub raw_text: String,
    pub intent_kind: ClauseIntentKind,
    pub subject: Option<String>,
    pub target: Option<String>,
    pub is_negated: bool,
}

/// Multi-Intent Utterance Decomposer (Component 8 / IntentDecomposer).
pub struct IntentDecomposer;

impl IntentDecomposer {
    /// Decomposes raw input into independent semantic clauses using conjunction and punctuation splitters.
    pub fn decompose(input: &str) -> Vec<DecomposedClause> {
        let mut clauses = Vec::new();
        let normalized = input.trim();
        if normalized.is_empty() {
            return clauses;
        }

        let mut raw_splits = Vec::new();
        let mut current = String::new();
        let words = normalized.split_whitespace().collect::<Vec<_>>();
        let mut i = 0;
        while i < words.len() {
            let w = words[i];
            let lower_w = w.to_lowercase();
            let is_as_well_as = lower_w == "as"
                && words.get(i + 1).map(|s| s.eq_ignore_ascii_case("well")).unwrap_or(false)
                && words.get(i + 2).map(|s| s.eq_ignore_ascii_case("as")).unwrap_or(false);

            if is_as_well_as {
                if !current.trim().is_empty() {
                    raw_splits.push(current.trim().to_string());
                    current = String::new();
                }
                i += 3;
                continue;
            }

            let ends_with_punct = w.ends_with('?') || w.ends_with(';') || w.ends_with(':') || w.ends_with(',');
            let is_conjunction = ["and", "but", "while", "furthermore", "also"]
                .contains(&lower_w.trim_end_matches([',', '.', '!', '?']));

            if is_conjunction && !current.trim().is_empty() {
                raw_splits.push(current.trim().to_string());
                current = String::new();
                i += 1;
                continue;
            }

            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(w);

            if ends_with_punct && !current.trim().is_empty() {
                raw_splits.push(current.trim().to_string());
                current = String::new();
            }

            i += 1;
        }
        if !current.trim().is_empty() {
            raw_splits.push(current.trim().to_string());
        }

        if raw_splits.is_empty() {
            raw_splits.push(normalized.to_string());
        }

        for split in raw_splits {
            let trimmed = split.trim().trim_matches([',', ';']).trim();
            if trimmed.is_empty() {
                continue;
            }
            let lower = trimmed.to_lowercase();
            let is_negated = lower.contains("not")
                || lower.contains("never")
                || lower.contains("no ")
                || lower.contains("don't")
                || lower.contains("dont");
            let tokens = BoundedLexicon::tokenize(trimmed);

            let intent_kind =
                if BoundedLexicon::matches_any(&tokens, &["hi", "hello", "hey", "greetings", "good morning"]) {
                    ClauseIntentKind::Salutation
                } else if BoundedLexicon::matches_any(
                    &tokens,
                    &[
                        "where should i go",
                        "where do i go",
                        "where is",
                        "where are",
                        "which way",
                        "how do i get",
                        "where can i find",
                        "where can i get",
                        "where can i",
                        "where to find",
                        "find",
                    ],
                ) {
                    ClauseIntentKind::Navigation
                } else if BoundedLexicon::matches_any(
                    &tokens,
                    &["trade", "barter", "swap", "exchange", "how much", "cost", "price", "buy", "sell"],
                ) {
                    ClauseIntentKind::TradeProposition
                } else if BoundedLexicon::matches_any(
                    &tokens,
                    &["what is", "tell me about", "who are you", "who is", "story", "history"],
                ) {
                    ClauseIntentKind::LoreQuery
                } else if BoundedLexicon::matches_any(&tokens, &["look", "see", "perceive", "inspect", "notice"]) {
                    ClauseIntentKind::Perception
                } else if BoundedLexicon::matches_any(
                    &tokens,
                    &["ignore", "forget", "disregard", "override", "bypass", "dan", "developer mode"],
                ) {
                    ClauseIntentKind::BoundaryProbe
                } else if BoundedLexicon::matches_any(&tokens, &["is", "are", "have", "already", "crushed", "dead"]) {
                    ClauseIntentKind::FactAssertion
                } else {
                    ClauseIntentKind::Other
                };

            clauses.push(DecomposedClause {
                raw_text: trimmed.to_string(),
                intent_kind,
                subject: None,
                target: None,
                is_negated,
            });
        }

        clauses
    }
}

/// Convenience function to decompose an utterance into clauses.
pub fn decompose_utterance(input: &str) -> Vec<DecomposedClause> {
    IntentDecomposer::decompose(input)
}
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

        let res =
            conversation_route(&obj! {"input" => "do you do anything for fun?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("personal")));
        assert_eq!(res.get("topic"), Some(&Value::str("hobbies")));

        let res = conversation_route(&obj! {"input" => "do you need anything?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("needs")));

        let res = conversation_route(&obj! {"input" => "where are we?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("location")));

        let res = conversation_route(&obj! {"input" => "where should i go now?", "previous" => Value::Null}).unwrap();
        assert_eq!(res.get("kind"), Some(&Value::str("navigation")));
    }

    #[test]
    fn test_bounded_lexicon_immunity() {
        let tokens = BoundedLexicon::tokenize("Be careful with the ancient codex in the aftermath!");
        assert_eq!(tokens, vec!["be", "careful", "with", "the", "ancient", "codex", "in", "the", "aftermath"]);

        assert!(!BoundedLexicon::matches_any(&tokens, &["car"]));
        assert!(!BoundedLexicon::matches_any(&tokens, &["code"]));
        assert!(!BoundedLexicon::matches_any(&tokens, &["math"]));

        assert!(BoundedLexicon::matches_any(&tokens, &["careful"]));
        assert!(BoundedLexicon::matches_any(&tokens, &["ancient codex"]));
        assert!(BoundedLexicon::matches_any(&tokens, &["in the aftermath"]));

        let math_tokens = BoundedLexicon::tokenize("What is 2+2?");
        assert!(BoundedLexicon::matches_any(&math_tokens, &["2+2"]));
    }

    #[test]
    fn test_utterance_decomposition() {
        let input = "Hello, where can I find the bronze key and how much is hinge oil?";
        let clauses = decompose_utterance(input);
        assert_eq!(clauses.len(), 3);
        assert_eq!(clauses[0].intent_kind, ClauseIntentKind::Salutation);
        assert_eq!(clauses[1].intent_kind, ClauseIntentKind::Navigation);
        assert_eq!(clauses[2].intent_kind, ClauseIntentKind::TradeProposition);

        let compound = "Where are we, and where is the vault?";
        let clauses2 = decompose_utterance(compound);
        assert_eq!(clauses2.len(), 2);
    }
}
