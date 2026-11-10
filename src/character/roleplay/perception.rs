//! Universal dynamic room perception and object permanence for roleplay characters.
//! Inspects authoritative visible items and rooms without any hardcoded names or presets.

use crate::Value;

/// Authoritative room sensory state for an acting character.
#[derive(Clone, Debug, Default)]
pub struct RoomPerception {
    pub actor: String,
    pub actor_name: String,
    pub actor_role: String,
    pub room_desc: String,
    pub possessions: Vec<(String, u64)>,
    pub unowned: Vec<String>,
    pub carried: Vec<String>,
    pub all_items: Vec<Value>,
}

impl RoomPerception {
    /// Build a perception snapshot from visible items and room description.
    pub fn from_state(
        actor: &str,
        actor_name: &str,
        actor_role: &str,
        room_desc: &str,
        visible_items: &[Value],
    ) -> Self {
        let mut possessions = Vec::new();
        let mut unowned = Vec::new();
        let mut carried = Vec::new();

        for item in visible_items {
            let label = item.get("label").and_then(Value::as_str).unwrap_or("");
            let owner = item.get("owner").and_then(Value::as_str).unwrap_or("");
            let price = item.get("price").and_then(Value::as_usize).unwrap_or(0) as u64;

            if label.is_empty() {
                continue;
            }

            if owner == actor {
                possessions.push((label.to_string(), price));
            } else if owner == "player" {
                carried.push(label.to_string());
            } else {
                unowned.push(label.to_string());
            }
        }

        Self {
            actor: actor.to_string(),
            actor_name: actor_name.to_string(),
            actor_role: actor_role.to_string(),
            room_desc: room_desc.to_string(),
            possessions,
            unowned,
            carried,
            all_items: visible_items.to_vec(),
        }
    }

    /// Formats perception into dynamic system prompt guidance for the LLM.
    pub fn prompt_summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.possessions.is_empty() {
            let list: Vec<String> = self
                .possessions
                .iter()
                .map(|(label, price)| {
                    if *price > 0 {
                        format!("{label} ({price} coins)")
                    } else {
                        label.clone()
                    }
                })
                .collect();
            parts.push(format!("In your possession: {}.", list.join(", ")));
        }
        if !self.unowned.is_empty() {
            parts.push(format!(
                "Visible resting in this room: {}. These are unowned objects in the environment that anyone present may examine or pick up.",
                self.unowned.join(", ")
            ));
        }
        if !self.carried.is_empty() {
            parts.push(format!("Carried by others present: {}.", self.carried.join(", ")));
        }
        parts.join(" ")
    }

    /// Resolves ambient perception queries (e.g. surroundings, single or multi-item inquiries) instantly.
    pub fn query(&self, input: &str) -> Option<String> {
        let q = input
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if [
            "what do you see",
            "what can you see",
            "look around you",
            "what is around us",
            "what s around us",
            "describe your surroundings",
        ]
        .contains(&q.as_str())
        {
            let mut parts = Vec::new();
            if !self.room_desc.is_empty() {
                parts.push(self.room_desc.clone());
            }
            if !self.possessions.is_empty() {
                let list: Vec<&str> = self.possessions.iter().map(|(l, _)| l.as_str()).collect();
                parts.push(format!("In my possession: {}.", list.join(", ")));
            }
            if !self.unowned.is_empty() {
                parts.push(format!("Resting in this area: {}.", self.unowned.join(", ")));
            }
            return Some(parts.join(" "));
        }

        let is_inquiry = [
            "where", "looking for", "look for", "find", "search", "do you have", "have you",
            "do you see", "you see", "i need", "we need", "is there", "are there",
            "can i get", "can you help", "help me", "tell me about", "what do you",
        ]
        .iter()
        .any(|cue| q.contains(cue));

        if is_inquiry {
            let mut matched_items = Vec::new();
            for item in &self.all_items {
                let label = item.get("label").and_then(Value::as_str).unwrap_or("");
                let id = item.get("id").and_then(Value::as_str).unwrap_or("");
                let id_short = id.split(':').last().unwrap_or(id).replace('_', " ");
                if label.is_empty() {
                    continue;
                }
                let label_lower = label.to_lowercase();
                let id_lower = id_short.to_lowercase();

                let direct_match = q.contains(&label_lower) || q.contains(&id_lower);
                let word_match = label_lower
                    .split_whitespace()
                    .chain(id_lower.split_whitespace())
                    .any(|w| w.len() >= 3 && q.split_whitespace().any(|qw| qw == w));

                if direct_match || word_match {
                    if !matched_items.iter().any(|(l, _, _, _)| l == &label) {
                        let owner = item.get("owner").and_then(Value::as_str).unwrap_or("world");
                        let price = item.get("price").and_then(Value::as_usize).unwrap_or(0) as u64;
                        matched_items.push((label.to_string(), owner.to_string(), price, item.clone()));
                    }
                }
            }

            if matched_items.len() == 1 {
                let (label, owner, price, _) = &matched_items[0];
                let reply = if owner == &self.actor {
                    if *price > 0 {
                        format!("I have the {label} right here with me for {price} coins.")
                    } else {
                        format!("I have the {label} right here with me.")
                    }
                } else if owner == "world" {
                    format!("The {label} is resting right here in this room; you are free to pick it up.")
                } else if owner == "player" {
                    format!("You already have the {label} with you.")
                } else {
                    format!("The {label} is here.")
                };
                return Some(reply);
            } else if matched_items.len() > 1 {
                let mut possessions_parts = Vec::new();
                let mut unowned_parts = Vec::new();
                let mut carried_parts = Vec::new();

                for (label, owner, price, _) in &matched_items {
                    if owner == &self.actor {
                        if *price > 0 {
                            possessions_parts.push(format!("{label} ({price} coins)"));
                        } else {
                            possessions_parts.push(label.clone());
                        }
                    } else if owner == "world" {
                        unowned_parts.push(label.clone());
                    } else if owner == "player" {
                        carried_parts.push(label.clone());
                    }
                }

                let mut sentences = Vec::new();
                if !possessions_parts.is_empty() {
                    if possessions_parts.len() == 1 {
                        sentences.push(format!("I have the {} with me.", possessions_parts[0]));
                    } else {
                        sentences.push(format!("I have {} with me.", possessions_parts.join(" and ")));
                    }
                }
                if !unowned_parts.is_empty() {
                    if unowned_parts.len() == 1 {
                        sentences.push(format!(
                            "As for the {}, it is resting right here in this room; you are free to pick it up.",
                            unowned_parts[0]
                        ));
                    } else {
                        sentences.push(format!(
                            "As for the {}, they are resting right here in this room; you are free to pick them up.",
                            unowned_parts.join(" and ")
                        ));
                    }
                }
                if !carried_parts.is_empty() {
                    sentences.push(format!("You already carry the {}.", carried_parts.join(" and ")));
                }

                if !sentences.is_empty() {
                    return Some(sentences.join(" "));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj;

    #[test]
    fn test_dynamic_perception_partitioning_and_queries() {
        let items = vec![
            obj! {"id" => "item:oil", "label" => "hinge oil", "owner" => "merchant", "price" => 3},
            obj! {"id" => "item:bronze_key", "label" => "bronze key", "owner" => "world", "price" => 0},
            obj! {"id" => "item:salve", "label" => "healing salve", "owner" => "player", "price" => 0},
        ];

        let p = RoomPerception::from_state("merchant", "Ilen", "merchant", "A stone chamber.", &items);
        let summary = p.prompt_summary();
        assert!(summary.contains("In your possession: hinge oil (3 coins)."));
        assert!(summary.contains("Visible resting in this room: bronze key."));
        assert!(summary.contains("Carried by others present: healing salve."));

        let q1 = p.query("where is the bronze key?").unwrap();
        assert!(q1.contains("resting right here in this room"));

        let q2 = p.query("do you have hinge oil?").unwrap();
        assert!(q2.contains("right here with me for 3 coins"));

        let q3 = p.query("where is my healing salve?").unwrap();
        assert!(q3.contains("You already have the healing salve with you"));

        let q4 = p.query("what do you see?").unwrap();
        assert!(q4.contains("A stone chamber."));
        assert!(q4.contains("hinge oil"));
        assert!(q4.contains("Resting in this area: bronze key."));

        let q5 = p.query("can you help me? I'm looking for a bronze key and some oil for the hinge door on the vault").unwrap();
        assert!(q5.contains("hinge oil (3 coins)"));
        assert!(q5.contains("bronze key"));
        assert!(q5.contains("free to pick it up"));
    }
}
