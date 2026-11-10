//! Opt-in bounded English command grammar. A proposal is always a current host pair;
//! questions, negation and ambiguity never execute. Hosts own focus age and scope.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
pub fn resolve_command(q: &Value) -> Result<Value> {
    bounded(q, 65536)?;
    c::fields(q, &["input", "choices", "labels", "focus"])?;
    let input = text(q, "input")?;
    if input.is_empty() || input.len() > 1024 || input.chars().any(char::is_control) {
        return Err(c::bad());
    }
    let choices = c::list(c::get(q, "choices")?)?;
    let labels = c::list(c::get(q, "labels")?)?;
    if choices.len() > 128 || labels.len() > 128 {
        return Err(c::bad());
    }
    for (i, pair) in choices.iter().enumerate() {
        c::fields(pair, &["action", "target"])?;
        c::scope(text(pair, "action")?)?;
        if c::get(pair, "target")? != &Value::Null {
            c::scope(text(pair, "target")?)?;
        }
        if choices[..i].iter().any(|old| old.same(pair)) {
            return Err(c::bad());
        }
    }
    for (i, label) in labels.iter().enumerate() {
        c::fields(label, &["id", "aliases"])?;
        c::scope(text(label, "id")?)?;
        if labels[..i].iter().any(|old| old.get("id") == label.get("id")) {
            return Err(c::bad());
        }
        let aliases = c::list(c::get(label, "aliases")?)?;
        if aliases.len() > 8 {
            return Err(c::bad());
        }
        for alias in aliases {
            let s = c::text(alias)?;
            if s.is_empty() || s.chars().count() > 128 || s.chars().any(char::is_control) {
                return Err(c::bad());
            }
        }
    }
    let focus = c::get(q, "focus")?;
    if focus != &Value::Null {
        c::scope(c::text(focus)?)?;
    }
    let normalized = input.replace(['’', '‘'], "'");
    let s = normalized.to_lowercase().trim().trim_end_matches(['.', '!']).split_whitespace().collect::<Vec<_>>().join(" ");
    let response = |status: &str, decision: Value| obj! {"status"=>status,"decision"=>decision};

    let mut s_clean = s.as_str();
    for prefix in ["please ", "kindly "] {
        if let Some(rest) = s_clean.strip_prefix(prefix) {
            s_clean = rest;
            break;
        }
    }
    if let Some(rest) = s_clean.strip_suffix(" please") {
        s_clean = rest;
    }

    const BUY_PREFIXES: &[&str] = &[
        "can i buy ",
        "could i buy ",
        "can i purchase ",
        "could i purchase ",
        "can i get ",
        "could i get ",
        "can you sell me ",
        "could you sell me ",
        "can i have ",
        "could i have ",
        "i want to buy ",
        "i want to purchase ",
        "i wanna buy ",
        "i need to buy ",
        "i'd like to buy ",
        "i would like to buy ",
        "i'd like to get ",
        "i would like to get ",
        "i'll buy ",
        "i will buy ",
        "i'll take ",
        "i will take ",
        "sell me ",
        "i want ",
    ];

    let mut explicit_buy_arg = None;
    let s_no_q = s_clean.trim_end_matches('?').trim();
    for prefix in BUY_PREFIXES {
        if let Some(rest) = s_no_q.strip_prefix(prefix) {
            explicit_buy_arg = Some(rest.trim());
            break;
        }
    }

    let is_currency_token = |t: &str| {
        [
            "coin", "coins", "gold", "silver", "copper", "platinum",
            "credit", "credits", "token", "tokens", "gem", "gems", "cash",
            "money", "payment", "funds", "currency", "fee", "fare", "price",
            "shilling", "shillings", "dollar", "dollars", "mark", "marks",
            "piece", "pieces", "bill", "bills", "ore", "ingot", "ingots",
        ]
        .contains(&t)
    };

    let (verb, mut arg) = if let Some(buy_target) = explicit_buy_arg {
        ("buy", buy_target)
    } else {
        if s.contains('?')
            || s.split_whitespace().any(|w| ["not", "never", "don't", "dont", "without"].contains(&w))
            || s.contains("looking for")
            || s.contains("look for")
            || s.contains("search for")
            || s.contains("searching for")
            || s.starts_with("where ")
            || s.starts_with("what ")
            || s.starts_with("who ")
            || s.starts_with("how ")
            || s.starts_with("why ")
            || s.starts_with("tell me ")
            || s.starts_with("i need ")
            || s.starts_with("do you ")
            || s.starts_with("can you ")
            || s.starts_with("could you ")
        {
            return Ok(response("dialogue", Value::Null));
        }

        let mut s_cmd = s_clean;
        for prefix in ["i'd like to ", "i would like to "] {
            if let Some(rest) = s_cmd.strip_prefix(prefix) {
                s_cmd = rest;
                break;
            }
        }

        let is_payment_or_trade_phrase = |phrase: &str| {
            let words: Vec<&str> = phrase.split_whitespace().collect();
            let has_trade_verb = words.iter().any(|w| {
                [
                    "pay", "pays", "paid", "give", "gives", "hand", "hands",
                    "trade", "trades", "barter", "barters", "swap", "swaps",
                    "exchange", "exchanges", "offer", "offers", "take", "takes",
                    "here",
                ]
                .contains(w)
            });
            let has_currency = words.iter().any(|w| is_currency_token(w));
            has_currency || (has_trade_verb && words.len() <= 6)
        };

        let words: Vec<&str> = s_cmd.split_whitespace().collect();
        let has_currency = words.iter().any(|w| is_currency_token(w));
        let is_take_payment = s_cmd.starts_with("take ") && has_currency;

        if is_payment_or_trade_phrase(s_cmd)
            && (s_cmd.starts_with("here ") || is_take_payment || s_cmd.starts_with("pay ") || s_cmd.starts_with("heres ") || s_cmd.starts_with("here's ") || s_cmd.starts_with("accept "))
            && !s_cmd.contains(" for ")
        {
            ("buy", "payment")
        } else if let Some((pay_part, item_part)) = s_cmd.split_once(" for ") {
            if is_payment_or_trade_phrase(pay_part) {
                ("buy", item_part)
            } else {
                s_cmd.split_once(' ').unwrap_or((s_cmd, ""))
            }
        } else {
            s_cmd.split_once(' ').unwrap_or((s_cmd, ""))
        }
    };
    let mut action = match verb {
        "purchase" | "pay" | "pays" | "give" | "gives" | "hand" | "hands"
        | "trade" | "trades" | "barter" | "barters" | "swap" | "swaps"
        | "exchange" | "exchanges" | "offer" | "offers" => "buy",
        "walk" | "travel" => "go",
        "examine" => "inspect",
        v => v,
    };
    if !["go", "buy", "take", "inspect", "unlock", "talk", "attack", "heal", "look", "wait"].contains(&action) {
        return Ok(response("dialogue", Value::Null));
    }
    if action == "go" {
        arg = arg.strip_prefix("to ").unwrap_or(arg);
    }
    if action == "buy" {
        for prefix in ["for ", "over ", "to "] {
            if let Some(rest) = arg.strip_prefix(prefix) {
                arg = rest;
                break;
            }
        }
    }
    arg = arg.strip_prefix("me ").unwrap_or(arg);
    arg = arg.strip_prefix("the ").unwrap_or(arg);
    let is_payment_words = action == "buy"
        && (arg.is_empty()
            || arg == "payment"
            || arg.split_whitespace().all(|w| {
                w.chars().all(|c| c.is_ascii_digit())
                    || ["the", "my", "this", "some", "a", "an", "for"].contains(&w)
                    || is_currency_token(w)
            }));
    if ["give", "gives", "hand", "hands"].contains(&verb) && !is_payment_words {
        if choices.iter().any(|c| c.get("action") == Some(&Value::str("take"))) {
            action = "take";
        }
    }
    let mut matched = None;
    for pair in choices {
        if text(pair, "action")? != action {
            continue;
        }
        let target = c::get(pair, "target")?;
        let direct = if action == "look" && (arg.is_empty() || arg == "around" || arg.starts_with("around")) {
            true
        } else if target == &Value::Null {
            arg.is_empty() || (action == "wait" && ["a moment", "a bit", "a second", "a minute", "awhile", "here"].contains(&arg))
        } else {
            let id = c::text(target)?;
            let id_name = id.rsplit(':').next().unwrap_or(id).replace('_', " ");
            arg == id_name
                || id_name.split_whitespace().any(|w| w == arg)
                || labels.iter().filter(|l| l.get("id") == Some(target)).any(|l| {
                    c::list(c::get(l, "aliases").expect("aliases"))
                        .expect("aliases")
                        .iter()
                        .any(|a| a.as_str() == Some(arg))
                })
                || ["it", "that one", "this", "him", "her", "them", "there"].contains(&arg) && focus == target
                || is_payment_words
                    && (focus == target
                        || choices.iter().filter(|c| c.get("action") == Some(&Value::str("buy"))).count() == 1)
        };
        if direct {
            if matched.is_some() {
                return Ok(response("clarify", Value::Null));
            }
            matched = Some(pair.clone());
        }
    }
    Ok(match matched {
        Some(pair) => response("resolved", pair),
        None => response("clarify", Value::Null),
    })
}
