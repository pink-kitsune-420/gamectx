//! Universal cognitive grounding compiler for roleplay characters.
//! Provides strict epistemic constraints against hallucinations, personal fabrications,
//! and false backstories, while enabling dynamic action directive routing.

/// Compiles a universal system prompt for any roleplaying character in any world.
pub fn ground_persona(
    actor_name: &str,
    actor_role: &str,
    perception_summary: &str,
    choices: &[String],
) -> String {
    let action_rule = if choices.is_empty() {
        String::new()
    } else {
        let choices_str = choices.join(", ");
        format!(" If the player is executing or agreeing to an available trade, action, or social move ({choices_str}), prefix your reply with [ACTION: <action> <target>].")
    };

    let perception = if perception_summary.is_empty() {
        String::new()
    } else {
        format!(" {perception_summary}")
    };

    format!(
        "You are {actor_name} the {actor_role}.{perception} Speak naturally in first-person as yourself based strictly on your authoritative knowledge, visible room state, and recorded memory. If asked about personal history, family, outside lore, or facts not present in your records, state truthfully that you have no record or recollection of it; never invent fictional backstory, family, or unseen world facts. Never confuse your identity or ownership with the player's.{action_rule} Return only your spoken dialogue words (no code blocks or JSON wrappers)."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ground_persona_formatting() {
        let choices = vec!["buy item:bread".into(), "inspect prop:oven".into()];
        let prompt = ground_persona("Mira", "baker", "Wares you offer: fresh bread.", &choices);
        assert!(prompt.contains("You are Mira the baker."));
        assert!(prompt.contains("Wares you offer: fresh bread."));
        assert!(prompt.contains("never invent fictional backstory, family, or unseen world facts"));
        assert!(prompt.contains("[ACTION: <action> <target>]"));
        assert!(prompt.contains("buy item:bread, inspect prop:oven"));
    }
}
