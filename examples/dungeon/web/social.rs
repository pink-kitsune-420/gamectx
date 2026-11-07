//! Original small-talk fallbacks grounded in host attitude and atmosphere.
pub fn replies(kind: &str, trust: i64, atmosphere: &str) -> Vec<&'static str> {
    match kind {
        "greeting" if trust < 0 => vec![
            "Hello; I can listen if we take this slowly.",
            "Hello; what would you like to say?",
            "I hear you; let us keep this civil.",
        ],
        "greeting" => vec![
            "Hello; it is good to have someone to talk to.",
            "Hello there; what is on your mind?",
            "Good to see you; how are you?",
        ],
        "thanks" => vec!["You are welcome.", "Glad to help.", "Of course."],
        "farewell" => vec!["Take care.", "Goodbye; I hope things go well for you.", "Until we speak again."],
        "wellbeing" if atmosphere == "emergency" => vec![
            "I feel on edge with this urgency; I would rather keep our conversation brief.",
            "I feel tense right now and would prefer a short conversation.",
            "I feel pressed for time with things so urgent.",
        ],
        "wellbeing" if trust < 0 => vec![
            "I feel wary, and would prefer to take this slowly.",
            "I feel guarded with you, though I am willing to listen.",
            "I feel uneasy and would appreciate a little patience.",
        ],
        "wellbeing" if atmosphere == "busy" => vec![
            "I feel a little distracted with things so busy, but I can listen.",
            "I feel pulled in several directions and would appreciate a brief chat.",
            "I feel busy rather than settled right now.",
        ],
        "wellbeing" if atmosphere == "tense" => vec![
            "I feel unsettled with things so tense.",
            "I feel cautious in this tense atmosphere.",
            "I feel a little on edge and would appreciate a calm conversation.",
        ],
        "wellbeing" if trust >= 25 => vec![
            "I feel at ease with you and glad for a chance to talk.",
            "I feel warmly toward you and am happy you asked.",
            "I feel comfortable in your company.",
        ],
        "wellbeing" => vec![
            "I feel fairly settled and happy to listen.",
            "I feel a little reserved, but I appreciate you asking.",
            "I feel quiet rather than troubled right now.",
        ],
        "mood_reason" if atmosphere == "emergency" => vec![
            "I feel tense because there is so much urgency here.",
            "I prefer a brief conversation because things are urgent.",
            "I feel on edge because the situation is urgent.",
        ],
        "mood_reason" if trust < 0 => vec![
            "I feel guarded because my trust in you is still low.",
            "I prefer to go slowly because I do not feel much trust yet.",
            "I value caution because trust between us is limited.",
        ],
        "mood_reason" if atmosphere == "busy" => vec![
            "I feel distracted because things are busy around us.",
            "I prefer a short chat because things are busy here.",
            "I feel pressed for time because of the busy atmosphere.",
        ],
        "mood_reason" if atmosphere == "tense" => vec![
            "I feel cautious because the atmosphere is tense.",
            "I prefer calm words because things feel tense here.",
            "I feel on edge because of the tension around us.",
        ],
        "mood_reason" if trust >= 25 => vec![
            "I feel at ease because I trust you.",
            "I value your company because I feel trust between us.",
            "I feel comfortable because I have trust in you.",
        ],
        "mood_reason" => vec![
            "I prefer to take things calmly because trust grows better with patience.",
            "I value a patient conversation because we are still getting to know one another.",
            "I feel a little reserved because trust takes time for me.",
        ],
        "elaborate" => vec![
            "I value patience because it gives a conversation room to develop.",
            "I think care matters because a thoughtful answer means more than a quick one.",
            "I prefer to explain slowly because I want to choose my words with care.",
        ],
        _ => vec![
            "I prefer to take a moment to think about that.",
            "I value a thoughtful conversation, even when I need a moment to find my words.",
            "I feel interested in the question and would like a moment to consider it.",
        ],
    }
}
