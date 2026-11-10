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
                    let offered_item = match_item(part_a, known_items);
                    let (offered_curr, offered_amt) = extract_currency(part_a);
                    let requested_item = match_item(part_b, known_items);
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
    _character_name: &str,
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
                        "Very well. I can lend you {amt} {token}. I trust you to repay me when your venture succeeds."
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
                        "I cannot extend a loan right now; times are too uncertain to part with supplies without coin."
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
                SocialOutcome {
                    status: "rejected",
                    dialogue: format!("I refuse to barter with you until you make amends."),
                    delta_trust: 0,
                    delta_affinity: -5,
                    delta_resentment: 5,
                    executable_action: None,
                }
            } else {
                let off = proposal.offered_item.as_deref().or(proposal.offered_currency.as_deref()).unwrap_or("that");
                let req = proposal.requested_item.as_deref().or(proposal.requested_currency.as_deref()).unwrap_or("this");
                SocialOutcome {
                    status: "accepted",
                    dialogue: format!("That is a fair exchange: your {off} for my {req}. A pleasure doing business."),
                    delta_trust: 5,
                    delta_affinity: 5,
                    delta_resentment: -5,
                    executable_action: proposal.requested_item.as_ref().map(|item| {
                        obj! {"action" => "buy", "target" => item.clone()}
                    }),
                }
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
                        "I earn my living by honest trade; I cannot simply give my supplies away for free."
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
}
