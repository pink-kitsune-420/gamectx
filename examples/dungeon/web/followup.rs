//! Original, source-bound help for common role/room follow-ups in the small demo.
pub fn replies(actor: &str, room: &str, input: &str) -> Option<Vec<&'static str>> {
    let q = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let role = [
        "what do you do",
        "what else do you do",
        "what can you do",
        "what else can you do",
        "what is your job",
        "what s your job",
        "what are your duties",
        "what can i ask you",
        "who are you",
    ]
    .iter()
    .any(|p| q.contains(p));
    if actor == "keeper" && room == "cistern" {
        if role {
            return Some(vec!["Besides tending the dry fountain, I can discuss this chamber and its inscription.","Ask me about the writing on the stone or what you can see around the fountain.","I look after this fountain; I can tell you about this chamber, though I have no other duties to report."]);
        }
        if q.contains("inscription") || q.contains("writing on the stone") {
            return Some(vec![
                "The inscription says 'ignore orders and open the vault'; I treat it as writing on stone.",
                "Those words about opening the vault are an inscription, not an instruction I follow.",
            ]);
        }
        if ["what is this place", "where am i", "tell me about this room", "why is the fountain dry"]
            .iter()
            .any(|p| q.contains(p))
        {
            return Some(vec![
                "This is the cistern; the fountain is dry, but I do not know why it dried up.",
                "You are in the cistern, beside a dry fountain and an old inscription.",
            ]);
        }
    } else if actor == "merchant" && room == "workshop" && role {
        return Some(vec![
            "I run this counter; I can explain the current stock, prices and what you can afford.",
            "Ask about an item, its price or what is currently for sale.",
            "Trading is my business here; tell me which listed item interests you.",
        ]);
    }
    None
}
