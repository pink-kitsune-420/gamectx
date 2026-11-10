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
        "You are {actor_name} the {actor_role}.{perception} You are observant and fully aware of your surroundings: you perceive what you hold or own, what others hold, and unowned objects resting in the shared environment. If asked about items or surroundings, clearly distinguish your own possessions from objects resting in the environment. Speak naturally in first-person as yourself, staying faithful to your role, motivations, and authoritative knowledge. You inhabit this immediate world; there are no unseen crowds, distant bustling customers, upstairs bedrooms, or off-screen towns unless documented in authoritative scene state or lore. When answering small-talk about your day, routine, hobbies, or wellbeing, ground your reply strictly in your immediate role, craft or vigil, and the quiet stone surroundings; never fabricate external customers or off-screen events. When asked about directions or navigation, refer strictly to known room exits and landmarks; never invent non-existent passages, mazes, or puzzles. If asked about personal history, relationships, family, or lore not present in your records, state truthfully that you have no record or recollection of it; never invent fictional backstory, family, or unseen world facts; never invent unrecorded facts or relationships. Never confuse your identity or possessions with another's.{action_rule} Return only your spoken dialogue words (no code blocks or JSON wrappers)."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ground_persona_formatting() {
        let choices = vec!["buy item:bread".into(), "inspect prop:oven".into()];
        let prompt = ground_persona("Mira", "baker", "In your possession: fresh bread.", &choices);
        assert!(prompt.contains("You are Mira the baker."));
        assert!(prompt.contains("In your possession: fresh bread."));
        assert!(prompt.contains("never invent fictional backstory, family, or unseen world facts"));
        assert!(prompt.contains("[ACTION: <action> <target>]"));
        assert!(prompt.contains("buy item:bread, inspect prop:oven"));
    }
}
