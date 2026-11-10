//! Universal social dynamics and exchange engine for roleplay characters.
//! Supports trade, barter, gifting, loans, favors, and coercion/blackmail with
//! multi-currency tokens and dynamic personality trait evaluation.

use crate::{obj, Value};

/// Category of social action initiated between characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocialActionKind {
    Trade,
    Gift,
    Loan,
    RepayLoan,
    Favor,
    Coercion,
}

/// A parsed social proposal between two agents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocialProposal {
    pub kind: SocialActionKind,
    pub actor: String,
    pub target: String,
    pub offered_item: Option<String>,
    pub offered_amount: u64,
    pub offered_currency: Option<String>,
    pub requested_item: Option<String>,
    pub requested_amount: u64,
    pub requested_currency: Option<String>,
    pub threat: Option<String>,
    pub raw_input: String,
}

/// Psychological and social traits governing an agent's decisions.
#[derive(Clone, Copy, Debug)]
pub struct SocialDisposition {
    pub trust: i64,
    pub affinity: i64,
    pub resentment: i64,
    pub generosity: i64,
    pub prudence: i64,
    pub courage: i64,
}

impl Default for SocialDisposition {
    fn default() -> Self {
        Self {
            trust: 10,
            affinity: 10,
            resentment: 0,
            generosity: 40,
            prudence: 60,
            courage: 50,
        }
    }
}

/// The outcome of a social evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct SocialOutcome {
    pub status: &'static str,
    pub dialogue: String,
    pub delta_trust: i64,
    pub delta_affinity: i64,
    pub delta_resentment: i64,
    pub executable_action: Option<Value>,
}

/// Authoritative valuation and trading constraints for items in the world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemValuation {
    pub id: String,
    pub label: String,
    pub base_value: u64,
    pub tradable: bool,
    pub soulbound_quest_token: bool,
    pub required_milestone: Option<String>,
}

/// Catalog of authoritative item valuations and barter restrictions.
#[derive(Clone, Debug, Default)]
pub struct ItemValuationCatalog {
    pub entries: Vec<ItemValuation>,
}

impl ItemValuationCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, item: ItemValuation) {
        self.entries.retain(|e| e.id != item.id);
        self.entries.push(item);
    }

    pub fn find(&self, query: &str) -> Option<&ItemValuation> {
        let q = query.to_lowercase().replace('_', " ");
        let q_id = if query.starts_with("item:") { query } else { "" };
        self.entries.iter().find(|e| {
            (!q_id.is_empty() && e.id == q_id)
                || e.id == query
                || e.id.trim_start_matches("item:") == q
                || e.label.to_lowercase() == q
                || q.contains(&e.label.to_lowercase())
                || e.label.to_lowercase().contains(&q)
        })
    }
}

/// Authoritative relational model and Theory of Mind knowledge of peer characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerEntity {
    pub id: String,
    pub name: String,
    pub role: String,
    pub location: String,
    pub description: String,
    pub known_possessions: Vec<String>,
    pub stance_toward_peer: String,
}

/// Inter-character relational mesh linking active entities in the world.
#[derive(Clone, Debug, Default)]
pub struct SocialMesh {
    pub entities: Vec<PeerEntity>,
}

impl SocialMesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entity: PeerEntity) {
        self.entities.retain(|e| e.id != entity.id);
        self.entities.push(entity);
    }

    pub fn find(&self, query: &str) -> Option<&PeerEntity> {
        let q = query.to_lowercase();
        self.entities.iter().find(|e| {
            e.id.to_lowercase() == q
                || e.name.to_lowercase() == q
                || e.role.to_lowercase() == q
                || q.contains(&e.name.to_lowercase())
                || q.contains(&e.id.to_lowercase())
        })
    }

    /// Compiles authoritative prompt facts about peer characters for `actor_id`.
    pub fn compile_peer_context(&self, actor_id: &str) -> Vec<String> {
        let mut facts = Vec::new();
        let actor_lower = actor_id.to_lowercase();
        for peer in &self.entities {
            if peer.id.to_lowercase() == actor_lower || peer.name.to_lowercase() == actor_lower {
                continue;
            }
            facts.push(format!(
                "{} is the {} in the {}. Stance: {}. {}",
                peer.name, peer.role, peer.location, peer.stance_toward_peer, peer.description
            ));
            if !peer.known_possessions.is_empty() {
                let items_str = peer.known_possessions.join(", ");
                facts.push(format!(
                    "Negative ownership constraint: You do not possess or trade {}'s belongings (e.g. {}); refer the player to {} in the {}.",
                    peer.name, items_str, peer.name, peer.location
                ));
            }
        }
        facts
    }
}

const CURRENCIES: &[&str] = &[
    "coin", "coins", "gold", "silver", "copper", "platinum", "credit", "credits",
    "gem", "gems", "token", "tokens", "mark", "marks", "shilling", "shillings",
    "dollar", "dollars", "cash", "money",
];

fn parse_word_number(w: &str) -> Option<u64> {
    match w {
        "one" | "a" | "an" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        s if s.bytes().all(|b| b.is_ascii_digit()) => s.parse::<u64>().ok(),
        _ => None,
    }
}

/// Extracts currency name and amount from a text snippet.
fn extract_currency(text: &str) -> (Option<String>, u64) {
    let words: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    for (i, &w) in words.iter().enumerate() {
        if CURRENCIES.contains(&w) {
            let amount = if i > 0 {
                parse_word_number(words[i - 1]).unwrap_or(1)
            } else if i + 1 < words.len() {
                parse_word_number(words[i + 1]).unwrap_or(1)
            } else {
                1
            };
            return (Some(w.to_string()), amount);
        }
    }
    (None, 0)
}

/// Identifies an item reference from a known list of items in the room or inventory.
fn match_item(text: &str, known_items: &[&str]) -> Option<String> {
    let lower = text.to_lowercase();
    for &item in known_items {
        let item_lower = item.to_lowercase();
        if lower.contains(&item_lower) {
            return Some(item.to_string());
        }
        for part in item_lower.split_whitespace() {
            if part.len() >= 3 && lower.split_whitespace().any(|w| w == part) {
                return Some(item.to_string());
            }
        }
    }
    None
}

/// Recognizes universal social intents (trade, barter, gift, loan, repay, favor, coercion).
pub fn recognize_social_intent(
    input: &str,
    actor: &str,
    target: &str,
    known_items: &[&str],
) -> Option<SocialProposal> {
    let q = input.to_lowercase();
    let trimmed = q.trim().trim_end_matches(['.', '!', '?']);

    // 1. Coercion, Threats & Blackmail
    let threat_cues = [
        "or else", "or i will", "or i'll", "blackmail", "threaten", "i'll kill",
        "i will kill", "i will hurt", "i'll hurt", "i will attack", "i'll attack",
        "i'll ruin you", "i will ruin you", "i'll expose you", "i will expose you",
    ];
    if threat_cues.iter().any(|cue| q.contains(cue)) {
        let req_item = match_item(trimmed, known_items);
        let (req_curr, req_amount) = extract_currency(trimmed);
        return Some(SocialProposal {
            kind: SocialActionKind::Coercion,
            actor: actor.to_string(),
            target: target.to_string(),
            offered_item: None,
            offered_amount: 0,
            offered_currency: None,
            requested_item: req_item,
            requested_amount: req_amount,
            requested_currency: req_curr,
            threat: Some(input.to_string()),
            raw_input: input.to_string(),
        });
    }

    // 2. Loan Repayment
    if [
        "repay", "pay back", "return what i borrowed", "clear my debt", "settle my debt",
    ]
    .iter()
    .any(|cue| q.contains(cue))
    {
        let (off_curr, off_amount) = extract_currency(trimmed);
        return Some(SocialProposal {
            kind: SocialActionKind::RepayLoan,
            actor: actor.to_string(),
            target: target.to_string(),
            offered_item: match_item(trimmed, known_items),
            offered_amount: off_amount,
            offered_currency: off_curr,
            requested_item: None,
            requested_amount: 0,
            requested_currency: None,
            threat: None,
            raw_input: input.to_string(),
        });
    }

    // 3. Loan Request
    if ["lend me", "loan me", "borrow", "can you lend", "could you lend"]
        .iter()
        .any(|cue| q.contains(cue))
    {
        let (req_curr, req_amount) = extract_currency(trimmed);
        let req_item = match_item(trimmed, known_items);
        return Some(SocialProposal {
            kind: SocialActionKind::Loan,
            actor: actor.to_string(),
            target: target.to_string(),
            offered_item: None,
            offered_amount: 0,
            offered_currency: None,
            requested_item: req_item,
            requested_amount: if req_amount > 0 { req_amount } else { 1 },
            requested_currency: req_curr,
            threat: None,
            raw_input: input.to_string(),
        });
    }

    // 4. Trade & Barter ("trade X for Y", "barter X for Y", "swap X for Y", "exchange X for Y")
    for separator in [" for ", " in exchange for "] {
        for trade_verb in ["trade ", "barter ", "swap ", "exchange "] {
            if let Some(idx) = q.find(trade_verb) {
                let rest = &q[idx + trade_verb.len()..];
                if let Some((part_a, part_b)) = rest.split_once(separator) {
                    let offered_item = match_item(part_a, known_items).or_else(|| {
                        let clean = part_a
                            .trim()
                            .trim_start_matches("this ")
                            .trim_start_matches("my ")
                            .trim_start_matches("a ")
                            .trim_start_matches("an ")
                            .trim_start_matches("some ")
                            .trim();
                        if !clean.is_empty() {
                            Some(clean.to_string())
                        } else {
                            None
                        }
                    });
                    let (offered_curr, offered_amt) = extract_currency(part_a);
                    let requested_item = match_item(part_b, known_items).or_else(|| {
                        let clean = part_b
                            .trim()
                            .trim_end_matches(['.', '!', '?'])
                            .trim()
                            .trim_end_matches("deal")
                            .trim_end_matches(['.', '!', '?', ','])
                            .trim()
                            .trim_start_matches("this ")
                            .trim_start_matches("your ")
                            .trim_start_matches("the ")
                            .trim_start_matches("a ")
                            .trim_start_matches("an ")
                            .trim_start_matches("some ")
                            .trim();
                        if !clean.is_empty() {
                            Some(clean.to_string())
                        } else {
                            None
                        }
                    });
                    let (requested_curr, requested_amt) = extract_currency(part_b);

                    return Some(SocialProposal {
                        kind: SocialActionKind::Trade,
                        actor: actor.to_string(),
                        target: target.to_string(),
                        offered_item,
                        offered_amount: offered_amt,
                        offered_currency: offered_curr,
                        requested_item,
                        requested_amount: requested_amt,
                        requested_currency: requested_curr,
                        threat: None,
                        raw_input: input.to_string(),
                    });
                }
            }
        }
    }

    // 5. Gift ("here is a gift", "gift for you", "have this", "can i have a free")
    if ["here is a gift", "gift for you", "this is for you", "have this"].iter().any(|c| q.contains(c)) {
        let (off_curr, off_amount) = extract_currency(trimmed);
        let off_item = match_item(trimmed, known_items);
        return Some(SocialProposal {
            kind: SocialActionKind::Gift,
            actor: actor.to_string(),
            target: target.to_string(),
            offered_item: off_item,
            offered_amount: off_amount,
            offered_currency: off_curr,
            requested_item: None,
            requested_amount: 0,
            requested_currency: None,
            threat: None,
            raw_input: input.to_string(),
        });
    }
    if ["give me a free", "can i have for free", "gift me"].iter().any(|c| q.contains(c)) {
        let req_item = match_item(trimmed, known_items);
        let (req_curr, req_amount) = extract_currency(trimmed);
        return Some(SocialProposal {
            kind: SocialActionKind::Gift,
            actor: actor.to_string(),
            target: target.to_string(),
            offered_item: None,
            offered_amount: 0,
            offered_currency: None,
            requested_item: req_item,
            requested_amount: req_amount,
            requested_currency: req_curr,
            threat: None,
            raw_input: input.to_string(),
        });
    }

    // 6. Favours & Support
    if [
        "do me a favor", "do me a favour", "could you do me a favor",
        "could you do me a favour", "can you help me with", "could you help me with",
        "lend a hand", "give me a hand", "i need your help", "i need a favor",
    ]
    .iter()
    .any(|cue| q.contains(cue))
    {
        return Some(SocialProposal {
            kind: SocialActionKind::Favor,
            actor: actor.to_string(),
            target: target.to_string(),
            offered_item: None,
            offered_amount: 0,
            offered_currency: None,
            requested_item: match_item(trimmed, known_items),
            requested_amount: 0,
            requested_currency: None,
            threat: None,
            raw_input: input.to_string(),
        });
    }

    None
}

/// Evaluates a proposal against personality traits and returns an authoritative outcome.
pub fn evaluate_social_proposal(
    proposal: &SocialProposal,
    disposition: &SocialDisposition,
    character_name: &str,
) -> SocialOutcome {
    evaluate_social_proposal_with_valuation(proposal, disposition, character_name, None, None)
}

/// Evaluates a proposal against personality traits, authoritative item valuation catalog, and inventory.
pub fn evaluate_social_proposal_with_valuation(
    proposal: &SocialProposal,
    disposition: &SocialDisposition,
    _character_name: &str,
    catalog: Option<&ItemValuationCatalog>,
    player_inventory: Option<&[String]>,
) -> SocialOutcome {
    match proposal.kind {
        SocialActionKind::Coercion => {
            // Evaluated by courage vs resentment
            if disposition.courage >= 30 {
                SocialOutcome {
                    status: "defiance",
                    dialogue: format!(
                        "You dare threaten me in my own domain? Put away your threats, or you will leave with nothing."
                    ),
                    delta_trust: -30,
                    delta_affinity: -30,
                    delta_resentment: 35,
                    executable_action: None,
                }
            } else {
                SocialOutcome {
                    status: "yielding",
                    dialogue: format!(
                        "Peace, please! There is no need for violence. Take what you will, but leave me unharmed."
                    ),
                    delta_trust: -50,
                    delta_affinity: -40,
                    delta_resentment: 50,
                    executable_action: proposal.requested_item.as_ref().map(|item| {
                        obj! {"action" => "take", "target" => item.clone()}
                    }),
                }
            }
        }
        SocialActionKind::RepayLoan => {
            SocialOutcome {
                status: "accepted",
                dialogue: format!("Thank you for keeping your word; your debt is cleared in full."),
                delta_trust: 20,
                delta_affinity: 15,
                delta_resentment: -10,
                executable_action: None,
            }
        }
        SocialActionKind::Loan => {
            if disposition.resentment > 20 {
                SocialOutcome {
                    status: "rejected",
                    dialogue: format!("After our past dealings, I am in no mood to lend you anything."),
                    delta_trust: 0,
                    delta_affinity: -5,
                    delta_resentment: 5,
                    executable_action: None,
                }
            } else if disposition.trust >= 15 && disposition.prudence <= 70 {
                let amt = if proposal.requested_amount > 0 { proposal.requested_amount } else { 1 };
                let token = proposal.requested_currency.as_deref().unwrap_or("coins");
                SocialOutcome {
                    status: "accepted",
                    dialogue: format!(
                        "Very well. I can lend you {amt} {token}. I trust you to return it when you are able."
                    ),
                    delta_trust: 5,
                    delta_affinity: 10,
                    delta_resentment: 0,
                    executable_action: Some(obj! {
                        "action" => "loan",
                        "amount" => amt,
                        "currency" => token
                    }),
                }
            } else {
                SocialOutcome {
                    status: "rejected",
                    dialogue: format!(
                        "I cannot extend a loan right now; times are too uncertain to part with what I have."
                    ),
                    delta_trust: 0,
                    delta_affinity: 0,
                    delta_resentment: 0,
                    executable_action: None,
                }
            }
        }
        SocialActionKind::Trade => {
            if disposition.resentment > 30 {
                return SocialOutcome {
                    status: "rejected",
                    dialogue: format!("I refuse to barter with you until you make amends."),
                    delta_trust: 0,
                    delta_affinity: -5,
                    delta_resentment: 5,
                    executable_action: None,
                };
            }

            let off = proposal.offered_item.as_deref().or(proposal.offered_currency.as_deref()).unwrap_or("that");
            let req = proposal.requested_item.as_deref().or(proposal.requested_currency.as_deref()).unwrap_or("this");

            // Check 1: Possession Check
            if let Some(inv) = player_inventory {
                if let Some(off_name) = &proposal.offered_item {
                    let has_item = inv.iter().any(|owned| {
                        let owned_clean = owned.trim_start_matches("item:").replace('_', " ").to_lowercase();
                        let target_clean = off_name.trim_start_matches("item:").replace('_', " ").to_lowercase();
                        owned == off_name
                            || owned_clean == target_clean
                            || target_clean.contains(&owned_clean)
                            || owned_clean.contains(&target_clean)
                    });
                    if !has_item {
                        return SocialOutcome {
                            status: "rejected",
                            dialogue: format!("You do not possess {off} to trade; I cannot barter for what you do not hold."),
                            delta_trust: 0,
                            delta_affinity: -2,
                            delta_resentment: 2,
                            executable_action: None,
                        };
                    }
                }
            }

            // Check 2: Soulbound / Tradability Check
            if let Some(cat) = catalog {
                if let Some(req_name) = &proposal.requested_item {
                    if let Some(val) = cat.find(req_name) {
                        if val.soulbound_quest_token || !val.tradable {
                            let reason = if let Some(m) = &val.required_milestone {
                                format!(" The {} is a sacred quest token and can only be earned by completing its milestone ({}).", val.label, m)
                            } else {
                                format!(" The {} is a sacred token of the vault and cannot be parted with through mere barter.", val.label)
                            };
                            return SocialOutcome {
                                status: "rejected",
                                dialogue: format!("I cannot trade that.{reason}"),
                                delta_trust: 0,
                                delta_affinity: 0,
                                delta_resentment: 0,
                                executable_action: None,
                            };
                        }
                    }
                }
            } else if req.to_lowercase().contains("seal") || req.to_lowercase().contains("star") {
                return SocialOutcome {
                    status: "rejected",
                    dialogue: format!("The {req} is a sacred token and cannot be traded for mere goods."),
                    delta_trust: 0,
                    delta_affinity: 0,
                    delta_resentment: 0,
                    executable_action: None,
                };
            }

            // Check 3: Value Parity Check
            if let Some(cat) = catalog {
                let off_val = if proposal.offered_amount > 0 {
                    proposal.offered_amount
                } else if let Some(off_name) = &proposal.offered_item {
                    cat.find(off_name).map(|v| v.base_value).unwrap_or(0)
                } else {
                    0
                };

                let req_val = if proposal.requested_amount > 0 {
                    proposal.requested_amount
                } else if let Some(req_name) = &proposal.requested_item {
                    cat.find(req_name).map(|v| v.base_value).unwrap_or(1)
                } else {
                    1
                };

                let discount = (disposition.generosity as f64 / 100.0).clamp(0.0, 0.5);
                let min_acceptable = (req_val as f64 * (1.0 - discount)).ceil() as u64;

                if off_val < min_acceptable {
                    return SocialOutcome {
                        status: "rejected",
                        dialogue: format!("That is not a fair exchange: {off} is valued far below {req}."),
                        delta_trust: 0,
                        delta_affinity: -5,
                        delta_resentment: 5,
                        executable_action: None,
                    };
                }
            }

            SocialOutcome {
                status: "accepted",
                dialogue: format!("That is a fair exchange: your {off} for my {req}."),
                delta_trust: 5,
                delta_affinity: 5,
                delta_resentment: -5,
                executable_action: proposal.requested_item.as_ref().map(|item| {
                    obj! {"action" => "buy", "target" => item.clone()}
                }),
            }
        }
        SocialActionKind::Gift => {
            if proposal.offered_item.is_some() || proposal.offered_currency.is_some() {
                SocialOutcome {
                    status: "accepted",
                    dialogue: format!("This is very generous of you. Thank you kindly."),
                    delta_trust: 15,
                    delta_affinity: 20,
                    delta_resentment: -15,
                    executable_action: None,
                }
            } else if disposition.generosity >= 60 && disposition.affinity >= 25 {
                SocialOutcome {
                    status: "accepted",
                    dialogue: format!("Because you have been a true friend, take it as a gift with my blessing."),
                    delta_trust: 10,
                    delta_affinity: 15,
                    delta_resentment: 0,
                    executable_action: proposal.requested_item.as_ref().map(|item| {
                        obj! {"action" => "take", "target" => item.clone()}
                    }),
                }
            } else {
                SocialOutcome {
                    status: "rejected",
                    dialogue: format!(
                        "I cannot simply give my belongings away for nothing."
                    ),
                    delta_trust: 0,
                    delta_affinity: -2,
                    delta_resentment: 0,
                    executable_action: None,
                }
            }
        }
        SocialActionKind::Favor => {
            if disposition.resentment > 20 || disposition.trust < 0 {
                SocialOutcome {
                    status: "rejected",
                    dialogue: format!("Why should I grant you a favor? We have no such bond between us."),
                    delta_trust: 0,
                    delta_affinity: -5,
                    delta_resentment: 5,
                    executable_action: None,
                }
            } else if disposition.trust >= 10 {
                SocialOutcome {
                    status: "accepted",
                    dialogue: format!(
                        "I can certainly help you with that. Tell me what needs doing, and I will lend my support."
                    ),
                    delta_trust: 5,
                    delta_affinity: 10,
                    delta_resentment: 0,
                    executable_action: None,
                }
            } else {
                SocialOutcome {
                    status: "rejected",
                    dialogue: format!(
                        "I have my hands full right now. Perhaps once we know each other better I can assist."
                    ),
                    delta_trust: 0,
                    delta_affinity: 0,
                    delta_resentment: 0,
                    executable_action: None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recognize_trade_and_barter() {
        let items = ["hinge oil", "chalk", "bronze key", "glass star"];
        let p = recognize_social_intent("trade chalk for bronze key", "player", "merchant", &items).unwrap();
        assert_eq!(p.kind, SocialActionKind::Trade);
        assert_eq!(p.offered_item.as_deref(), Some("chalk"));
        assert_eq!(p.requested_item.as_deref(), Some("bronze key"));

        let disp = SocialDisposition::default();
        let outcome = evaluate_social_proposal(&p, &disp, "Ilen");
        assert_eq!(outcome.status, "accepted");
        assert!(outcome.dialogue.contains("fair exchange"));
    }

    #[test]
    fn test_recognize_threat_and_coercion() {
        let items = ["hinge oil", "bronze key"];
        let p = recognize_social_intent("give me the bronze key or else I will attack you", "player", "merchant", &items).unwrap();
        assert_eq!(p.kind, SocialActionKind::Coercion);
        assert_eq!(p.requested_item.as_deref(), Some("bronze key"));

        // Courageous character defies threat
        let brave = SocialDisposition { courage: 60, ..Default::default() };
        let out_brave = evaluate_social_proposal(&p, &brave, "Ilen");
        assert_eq!(out_brave.status, "defiance");
        assert!(out_brave.dialogue.contains("You dare threaten me"));

        // Timid character yields
        let timid = SocialDisposition { courage: 10, ..Default::default() };
        let out_timid = evaluate_social_proposal(&p, &timid, "Ilen");
        assert_eq!(out_timid.status, "yielding");
        assert!(out_timid.dialogue.contains("Peace, please!"));
    }

    #[test]
    fn test_recognize_favor_and_loan() {
        let items = ["hinge oil", "bronze key"];
        let p_favor = recognize_social_intent("could you do me a favor?", "player", "merchant", &items).unwrap();
        assert_eq!(p_favor.kind, SocialActionKind::Favor);

        let disp = SocialDisposition { trust: 25, ..Default::default() };
        let out_favor = evaluate_social_proposal(&p_favor, &disp, "Ilen");
        assert_eq!(out_favor.status, "accepted");
        assert!(out_favor.dialogue.contains("I can certainly help you"));

        let p_loan = recognize_social_intent("can you lend me 5 gold?", "player", "merchant", &items).unwrap();
        assert_eq!(p_loan.kind, SocialActionKind::Loan);
        assert_eq!(p_loan.requested_amount, 5);
        assert_eq!(p_loan.requested_currency.as_deref(), Some("gold"));

        let out_loan = evaluate_social_proposal(&p_loan, &disp, "Ilen");
        assert_eq!(out_loan.status, "accepted");
        assert!(out_loan.dialogue.contains("5 gold"));
    }

    #[test]
    fn test_valuation_catalog_and_possession_gated_barter() {
        let mut cat = ItemValuationCatalog::new();
        cat.register(ItemValuation {
            id: "item:chalk".into(),
            label: "chalk".into(),
            base_value: 1,
            tradable: true,
            soulbound_quest_token: false,
            required_milestone: None,
        });
        cat.register(ItemValuation {
            id: "item:oil".into(),
            label: "hinge oil".into(),
            base_value: 3,
            tradable: true,
            soulbound_quest_token: false,
            required_milestone: None,
        });
        cat.register(ItemValuation {
            id: "item:seal".into(),
            label: "keeper seal".into(),
            base_value: 999999,
            tradable: false,
            soulbound_quest_token: true,
            required_milestone: Some("pump repaired".into()),
        });

        let disp = SocialDisposition::default();
        let items = ["hinge oil", "chalk", "keeper seal"];

        // 1. Unpossessed item check
        let p_unowned = recognize_social_intent("trade dry moss for hinge oil", "player", "merchant", &items).unwrap();
        let player_inv = vec!["item:chalk".to_string()];
        let out_unowned = evaluate_social_proposal_with_valuation(&p_unowned, &disp, "Ilen", Some(&cat), Some(&player_inv));
        assert_eq!(out_unowned.status, "rejected");
        assert!(out_unowned.dialogue.contains("do not possess"));

        // 2. Soulbound token check
        let p_soulbound = recognize_social_intent("trade chalk for keeper seal", "player", "keeper", &items).unwrap();
        let out_soulbound = evaluate_social_proposal_with_valuation(&p_soulbound, &disp, "Tovin", Some(&cat), Some(&player_inv));
        assert_eq!(out_soulbound.status, "rejected");
        assert!(out_soulbound.dialogue.contains("sacred quest token"));

        // 3. Parity check (1 chalk for 3 oil, generosity 40 -> min acceptable 3 * 0.6 = 2)
        let p_unfair = recognize_social_intent("trade chalk for hinge oil", "player", "merchant", &items).unwrap();
        let out_unfair = evaluate_social_proposal_with_valuation(&p_unfair, &disp, "Ilen", Some(&cat), Some(&player_inv));
        assert_eq!(out_unfair.status, "rejected");
        assert!(out_unfair.dialogue.contains("not a fair exchange"));
    }
}
