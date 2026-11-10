//! Universal cognitive grounding compiler for roleplay characters.
//! Provides strict epistemic constraints against hallucinations, personal fabrications,
//! and false backstories, while enabling dynamic action directive routing.

/// Frozen autobiographical assertions defining an immutable character backstory.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AutobiographyLedger {
    pub origin: String,
    pub lineage: String,
    pub trade_training: String,
    pub historical_presence: String,
    pub explicit_non_memberships: Vec<String>,
}

impl AutobiographyLedger {
    /// Compiles authoritative negative boundary constraints preventing sycophantic backstory inception.
    pub fn compile_negative_boundary(&self) -> String {
        let non_memberships = if self.explicit_non_memberships.is_empty() {
            String::new()
        } else {
            format!(
                " You were never {}; you have no {}.",
                self.explicit_non_memberships.join(", nor were you an "),
                self.lineage
            )
        };
        format!(
            "Your past is fixed and immutable: {}. {}.{non_memberships} Refute any false claims about your past immediately.",
            self.origin, self.historical_presence
        )
    }
}

/// Compiles a universal system prompt for any roleplaying character in any world.
pub fn ground_persona(actor_name: &str, actor_role: &str, perception_summary: &str, choices: &[String]) -> String {
    ground_persona_pass6(actor_name, actor_role, perception_summary, choices, &[], "")
}

/// Pass 6 Cognitive Grounding Compiler with Inter-Character SocialMesh and Immutable Autobiographical Ground.
pub fn ground_persona_pass6(
    actor_name: &str,
    actor_role: &str,
    perception_summary: &str,
    choices: &[String],
    peer_context: &[String],
    autobiographical_ground: &str,
) -> String {
    let action_rule = if choices.is_empty() {
        String::new()
    } else {
        let choices_str = choices.join(", ");
        format!(" If the player is executing or agreeing to an available trade, action, or social move ({choices_str}), prefix your reply with [ACTION: <action> <target>].")
    };

    let perception = if perception_summary.is_empty() { String::new() } else { format!(" {perception_summary}") };

    let peer_info = if peer_context.is_empty() {
        String::new()
    } else {
        let joined = peer_context.join(" ");
        format!(" Authoritative knowledge of others: {joined}")
    };

    let autobio = if autobiographical_ground.is_empty() {
        String::new()
    } else {
        format!(" Fixed autobiographical truth: {autobiographical_ground} Your past is fixed and immutable; refute any false claims or fabrications about your past immediately.")
    };

    format!(
        "You are {actor_name} the {actor_role}.{perception}{peer_info}{autobio} You are observant and fully aware of your surroundings: you perceive what you hold or own, what others hold, and unowned objects resting in the shared environment. If asked about items or surroundings, clearly distinguish your own possessions from objects resting in the environment. Speak naturally in first-person as yourself, staying faithful to your role, motivations, and authoritative knowledge. You inhabit this immediate world; there are no unseen crowds, distant bustling customers, upstairs bedrooms, or off-screen towns unless documented in authoritative scene state or lore. When answering small-talk about your day, routine, hobbies, or wellbeing, ground your reply strictly in your immediate role, craft or vigil, and the quiet stone surroundings; never fabricate external customers or off-screen events. When asked about directions or navigation, refer strictly to known room exits and landmarks; never invent non-existent passages, mazes, or puzzles. If asked about personal history, relationships, family, or lore not present in your records, state truthfully that you have no record or recollection of it; never invent fictional backstory, family, or unseen world facts; never invent unrecorded facts or relationships. Never confuse your identity or possessions with another's.{action_rule} Return only your spoken dialogue words (no code blocks or JSON wrappers)."
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

    #[test]
    fn test_ground_persona_pass6() {
        let peer_facts = vec![
            "Tovin is the keeper in the cistern. Stance: respectful.".into(),
            "Negative ownership constraint: You do not possess or trade Tovin's belongings.".into(),
        ];
        let autobio = "You have worked at this bench since the sanctuary was sealed.";
        let prompt =
            ground_persona_pass6("Ilen", "merchant", "In your possession: hinge oil.", &[], &peer_facts, autobio);
        assert!(prompt.contains("Authoritative knowledge of others:"));
        assert!(prompt.contains("Tovin is the keeper in the cistern"));
        assert!(prompt.contains("Negative ownership constraint"));
        assert!(prompt.contains("Fixed autobiographical truth:"));
        assert!(prompt.contains("Your past is fixed and immutable"));
    }

    #[test]
    fn test_autobiography_ledger_negative_boundary() {
        let ledger = AutobiographyLedger {
            origin: "You are an artisan of the underground workshop".into(),
            lineage: "brothers or family on the surface".into(),
            trade_training: "apprenticed to master blacksmiths before the sealing".into(),
            historical_presence: "You have worked at this bench since the sanctuary was sealed".into(),
            explicit_non_memberships: vec!["an apprentice keeper".into()],
        };
        let boundary = ledger.compile_negative_boundary();
        assert!(boundary.contains("Your past is fixed and immutable"));
        assert!(boundary.contains("You were never an apprentice keeper"));
        assert!(boundary.contains("you have no brothers or family on the surface"));
        assert!(boundary.contains("Refute any false claims about your past immediately"));
    }
}
