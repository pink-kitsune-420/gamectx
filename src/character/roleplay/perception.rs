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
    pub wares: Vec<(String, u64)>,
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
        let mut wares = Vec::new();
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
                wares.push((label.to_string(), price));
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
            wares,
            unowned,
            carried,
            all_items: visible_items.to_vec(),
        }
    }

    /// Formats perception into dynamic system prompt guidance for the LLM.
    pub fn prompt_summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.wares.is_empty() {
            let list: Vec<String> = self
                .wares
                .iter()
                .map(|(label, price)| {
                    if *price > 0 {
                        format!("{label} ({price} coins)")
                    } else {
                        label.clone()
                    }
                })
                .collect();
            parts.push(format!("Wares you currently offer: {}.", list.join(", ")));
        }
        if !self.unowned.is_empty() {
            parts.push(format!(
                "Unowned items visible resting in this room: {}. You do not sell these; travelers may freely pick them up.",
                self.unowned.join(", ")
            ));
        }
        if !self.carried.is_empty() {
            parts.push(format!("Items carried by the traveler: {}.", self.carried.join(", ")));
        }
        parts.join(" ")
    }

    /// Resolves ambient perception queries (e.g. surroundings, item locations) instantly.
    pub fn query(&self, input: &str) -> Option<String> {
        let q = input
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let item_prefixes = [
            "where is ", "where s ", "is there a ", "is there an ", "is there any ",
            "do you have a ", "do you have an ", "do you have any ", "do you have ",
            "do you see a ", "do you see an ", "do you see any ", "do you see ",
            "where can i find a ", "where can i find an ", "where can i find the ",
            "tell me about the ", "tell me about a ",
        ];

        let item_target = item_prefixes
            .iter()
            .find_map(|prefix| q.strip_prefix(prefix))
            .map(|rest| rest.trim_start_matches("the ").trim_start_matches("my ").trim_start_matches("our ").trim());

        if let Some(target) = item_target {
            let found = self.all_items.iter().find(|i| {
                let label = i.get("label").and_then(Value::as_str).unwrap_or("");
                let id = i.get("id").and_then(Value::as_str).unwrap_or("");
                let id_short = id.split(':').last().unwrap_or(id).replace('_', " ");
                label.eq_ignore_ascii_case(target)
                    || id_short.eq_ignore_ascii_case(target)
                    || label.split_whitespace().any(|w| w.eq_ignore_ascii_case(target))
            });

            if let Some(item) = found {
                let label = item.get("label").and_then(Value::as_str).unwrap_or(target);
                let owner = item.get("owner").and_then(Value::as_str).unwrap_or("");
                let price = item.get("price").and_then(Value::as_usize).unwrap_or(0) as u64;

                let reply = if owner == self.actor {
                    if price > 0 {
                        format!("I have the {label} right here on offer for {price} coins.")
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
            }
        }

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
            if !self.wares.is_empty() {
                let list: Vec<&str> = self.wares.iter().map(|(l, _)| l.as_str()).collect();
                parts.push(format!("I have {} on offer.", list.join(", ")));
            }
            if !self.unowned.is_empty() {
                parts.push(format!("Resting in this room: {}.", self.unowned.join(", ")));
            }
            return Some(parts.join(" "));
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
        assert!(summary.contains("Wares you currently offer: hinge oil (3 coins)."));
        assert!(summary.contains("Unowned items visible resting in this room: bronze key."));
        assert!(summary.contains("Items carried by the traveler: healing salve."));

        let q1 = p.query("where is the bronze key?").unwrap();
        assert!(q1.contains("resting right here in this room"));

        let q2 = p.query("do you have hinge oil?").unwrap();
        assert!(q2.contains("right here on offer for 3 coins"));

        let q3 = p.query("where is my healing salve?").unwrap();
        assert!(q3.contains("You already have the healing salve with you"));

        let q4 = p.query("what do you see?").unwrap();
        assert!(q4.contains("A stone chamber."));
        assert!(q4.contains("hinge oil on offer"));
        assert!(q4.contains("Resting in this room: bronze key."));
    }
}
