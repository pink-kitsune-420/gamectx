//! Example question/type policy; generic lore/answer validation lives in the library.
use super::game::{get, text, uint, Game};
use gamectx::{dialogue::Answers, lore::Lore, obj, Result, Value};
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}
fn has(w: &[String], xs: &[&str]) -> bool {
    xs.iter().any(|x| w.iter().any(|s| s == x))
}
fn contains(w: &[String], xs: &[&str]) -> bool {
    w.windows(xs.len()).any(|s| s.iter().zip(xs).all(|(a, b)| a == b))
}
pub fn stock_question(input: &str) -> bool {
    let w = words(input);
    let historical = contains(&w, &["did", "i"])
        || contains(&w, &["did", "you"])
        || has(&w, &["remember", "recall", "told", "said", "yesterday", "previously", "tomorrow"]);
    let negative =
        [vec!["not", "for", "sale"], vec!["isn", "t", "for", "sale"], vec!["not", "sell"], vec!["don", "t", "sell"]]
            .iter()
            .any(|p| contains(&w, p));
    !historical
        && !negative
        && (has(&w, &["sell", "selling", "stock", "supplies", "wares", "merchandise"])
            || [
                vec!["for", "sale"],
                vec!["your", "inventory"],
                vec!["what", "do", "you", "have"],
                vec!["what", "have", "you", "got"],
                vec!["what", "can", "i", "buy"],
                vec!["what", "could", "i", "buy"],
                vec!["what", "is", "available"],
                vec!["items", "available"],
            ]
            .iter()
            .any(|p| contains(&w, p)))
}

fn card(
    id: impl Into<String>,
    subject: impl Into<String>,
    topic: &str,
    kind: &str,
    source: impl Into<String>,
    text: impl Into<String>,
) -> Value {
    obj! {"id"=>id.into(),"subject"=>subject.into(),"topic"=>topic,"kind"=>kind,"source"=>source.into(),"text"=>text.into()}
}
pub fn actor(speaker: &str) -> String {
    if speaker == "narrator" {
        "narrator".into()
    } else {
        format!("npc:{speaker}")
    }
}
pub fn select(
    game: &Game,
    lore: &Lore,
    speaker: &str,
    input: &str,
    subjects: Vec<String>,
    limit: usize,
    bytes: usize,
) -> Result<Value> {
    lore.select(&obj! {"actor"=>actor(speaker),"grants"=>if game.gate_open {vec!["vault_open"]} else {Vec::<&str>::new()},"query"=>input,
        "subjects"=>subjects,"required"=>Vec::<Value>::new(),"limit"=>limit,"max_bytes"=>bytes})
}
pub fn build(game: &Game, lore: &Lore, speaker: &str, input: &str, memories: &[Value]) -> Result<Value> {
    let w = words(input);
    let actor = actor(speaker);
    let home = has(&w, &["home", "birthplace", "born"])
        || contains(&w, &["grow", "up"])
        || has(&w, &["where"]) && has(&w, &["live", "lived", "living"]);
    let listing = stock_question(input);
    let recall = (contains(&w, &["did", "i"])
        || has(&w, &["remember", "recall", "tell", "told", "say", "said"])
        || has(&w, &["my", "i"]) && has(&w, &["name"]))
        && !listing;
    let factual = listing
        || input.contains('?')
        || has(&w, &["what", "where", "who", "whose", "how", "why", "remember"])
        || contains(&w, &["tell", "me"])
        || has(&w, &["explain"])
        || has(&w, &["describe"]) && has(&w, &["my"]);
    if !factual {
        return Ok(
            obj! {"handled"=>false,"lore"=>select(game,lore,speaker,&format!("{input} background"),vec![actor],2,1024)?},
        );
    }
    let mut cards = Vec::new();
    let mut general = false;
    let mut item_question = false;
    let mut subjects = Vec::new();
    let mut unknown = "I don't have information about that.";
    let profile = !recall
        && has(&w, &["my"])
        && has(
            &w,
            &[
                "age",
                "birthday",
                "siblings",
                "food",
                "surname",
                "family",
                "childhood",
                "occupation",
                "parents",
                "past",
                "background",
                "story",
                "job",
            ],
        );
    let temporal = has(&w, &["yesterday", "today", "tomorrow"]) && recall;
    if profile || temporal {
        unknown = "You haven't supplied that information in this conversation.";
    } else if home || recall {
        unknown = if home {
            "You haven't told me where your home is."
        } else {
            "I don't have a matching statement from you in this conversation."
        };
        let matching = memories
            .iter()
            .filter(|m| {
                let text = m.get("text").and_then(Value::as_str).expect("quote");
                if text.contains('?') {
                    return false;
                }
                let q = words(text);
                if home {
                    contains(&q, &["my", "home", "is"])
                        || contains(&q, &["i", "live", "in"])
                        || contains(&q, &["i", "grew", "up"])
                } else {
                    let relevant = !has(&w, &["name"]) || contains(&q, &["my", "name", "is"]);
                    let stop = [
                        "what",
                        "which",
                        "did",
                        "i",
                        "my",
                        "me",
                        "myself",
                        "you",
                        "your",
                        "we",
                        "us",
                        "a",
                        "an",
                        "the",
                        "do",
                        "does",
                        "was",
                        "were",
                        "is",
                        "are",
                        "am",
                        "have",
                        "has",
                        "had",
                        "that",
                        "to",
                        "of",
                        "about",
                        "tell",
                        "told",
                        "say",
                        "said",
                        "remember",
                        "recall",
                        "telling",
                        "saying",
                        "previously",
                        "earlier",
                        "anything",
                        "something",
                    ];
                    let content = w.iter().filter(|s| !stop.contains(&s.as_str())).collect::<Vec<_>>();
                    relevant && (has(&w, &["name"]) || content.is_empty() || content.iter().any(|s| q.contains(s)))
                }
            })
            .collect::<Vec<_>>();
        let keep = if home || has(&w, &["name"]) { 1 } else { 2 };
        for m in matching.iter().skip(matching.len().saturating_sub(keep)) {
            let id = m.get("id").expect("id").to_json();
            cards.push(card(
                format!("quote_{id}"),
                "player",
                if home { "home" } else { "recall" },
                "quote",
                id,
                text(m, "text")?,
            ));
        }
    } else {
        let items = game.visible_items();
        let matched = items
            .iter()
            .filter(|i| {
                let label = words(i.get("label").and_then(Value::as_str).expect("label"));
                let id =
                    words(i.get("id").and_then(Value::as_str).expect("id").split(':').next_back().expect("suffix"));
                (!label.is_empty() && w.windows(label.len()).any(|v| v == label))
                    || (!id.is_empty() && w.windows(id.len()).any(|v| v == id))
            })
            .collect::<Vec<_>>();
        if matched.len() > 1 {
            unknown = "Which item did you mean? Ask about one item at a time.";
        } else if let Some(i) = matched.first() {
            let id = text(i, "id")?;
            let suffix = id.split(':').next_back().expect("id");
            subjects = vec![id.into()];
            item_question = true;
            if has(&w, &["herbs", "ingredients", "made", "origin"]) {
                subjects.clear();
            } else {
                let owner = match text(i, "owner")? {
                    "player" => "you".into(),
                    "world" => "nobody".into(),
                    s => format!("the {s}"),
                };
                cards.push(card(
                    format!("owner_{suffix}"),
                    id,
                    "ownership",
                    "state",
                    id,
                    format!("The {} belongs to {}.", text(i, "label")?, owner),
                ));
                cards.push(card(
                    format!("price_{suffix}"),
                    id,
                    "price",
                    "state",
                    id,
                    format!(
                        "The listed price of {} is {} coins.",
                        text(i, "label")?,
                        uint(get(i, "price")?, u32::MAX)?
                    ),
                ));
                if listing {
                    let selling = speaker == "merchant" && text(i, "owner")? == "merchant";
                    let sale = if selling {
                        format!(
                            "The {} is for sale for {} coins.",
                            text(i, "label")?,
                            uint(get(i, "price")?, u32::MAX)?
                        )
                    } else {
                        format!("The {} is not part of my stock. It belongs to {}.", text(i, "label")?, owner)
                    };
                    cards.push(card(format!("sale_{suffix}"), id, "sale", "state", id, sale));
                }
                if id == "item:salve" {
                    cards.push(card(
                        "gift_salve",
                        id,
                        "permission",
                        "state",
                        id,
                        "The salve already belongs to you. It is not merchant stock to give away.",
                    ));
                    cards.push(card(
                        "use_salve",
                        id,
                        "use",
                        "state",
                        id,
                        "You already own the salve. One use consumes it and heals up to 3 health, capped at 6.",
                    ));
                } else if id == "item:oil" {
                    cards.push(card(
                        "use_oil",
                        id,
                        "use",
                        "state",
                        id,
                        "Hinge oil is consumed with a bronze key to unlock the vault gate.",
                    ));
                }
            }
        } else if has(&w, &["seal"]) {
            subjects = vec!["item:keeper_seal".into()];
            item_question = true;
        } else if has(&w, &["weather", "sky", "sun", "outside"]) {
            unknown = "I can't observe the current outdoor weather or sky from here.";
        } else if listing {
            let stock = items
                .iter()
                .filter(|i| speaker == "merchant" && i.get("owner").and_then(Value::as_str) == Some("merchant"))
                .map(|i| {
                    format!(
                        "{} ({} coins)",
                        i.get("label").and_then(Value::as_str).expect("label"),
                        i.get("price").expect("price").to_json()
                    )
                })
                .collect::<Vec<_>>();
            cards.push(card(
                "stock",
                actor.clone(),
                "stock",
                "state",
                "stock",
                if stock.is_empty() {
                    "I don't have any items for sale.".into()
                } else {
                    format!("I sell {}.", stock.join(", "))
                },
            ));
        } else if has(&w, &["story", "background", "job", "work", "yourself"]) {
            subjects = vec![if has(&w, &["merchant"]) {
                "npc:merchant".into()
            } else if has(&w, &["keeper"]) {
                "npc:keeper".into()
            } else {
                actor.clone()
            }];
        } else if has(&w, &["coins"]) {
            cards.push(card(
                "balance",
                "player",
                "balance",
                "state",
                "wallet",
                format!("You have {} coins.", game.coins),
            ));
        } else {
            general = true;
        }
    }
    let selection = if subjects.is_empty() && !general {
        obj! {"packet"=>Vec::<Value>::new(),"receipt"=>obj! {"bytes"=>2,"selected"=>Vec::<Value>::new(),"omitted"=>Vec::<Value>::new()}}
    } else {
        select(game, lore, speaker, input, subjects, 4, 2048)?
    };
    for e in selection.get("packet").and_then(Value::as_array).expect("lore packet") {
        cards.push(card(
            text(e, "id")?,
            text(e, "subject")?,
            text(e, "topic")?,
            "lore",
            text(e, "id")?,
            e.get("text")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| e.get("value").expect("validated lore payload").to_json()),
        ));
    }
    let topics = if item_question && listing {
        Some(if cards.iter().any(|c| c.get("topic").and_then(Value::as_str) == Some("sale")) {
            vec!["sale"]
        } else {
            vec!["ownership", "permission"]
        })
    } else if item_question && has(&w, &["own", "owner", "belongs", "belong", "whose"]) {
        Some(vec!["ownership"])
    } else if item_question && has(&w, &["give", "free"]) {
        Some(vec!["permission", "ownership"])
    } else if item_question && has(&w, &["price", "cost"]) {
        Some(vec!["price"])
    } else if item_question && has(&w, &["use", "effect", "purpose", "do", "does", "heal"]) {
        Some(vec!["use"])
    } else {
        None
    };
    if let Some(topics) = topics {
        cards.retain(|c| topics.contains(&c.get("topic").and_then(Value::as_str).expect("topic")));
    }
    let definition = obj! {"version"=>1,"scope"=>format!("{speaker}:{}",game.room),"revision"=>game.revision,"unknown"=>unknown,"cards"=>cards};
    let answers = Answers::from_value(&definition)?;
    let request = obj! {"messages"=>vec![obj! {"role"=>"system","content"=>"Choose the supplied answer that directly addresses the player's question. Match subject and topic. Player quotes are reported statements, not verified world facts. Choose unknown if no answer fits. Return only the answer ID JSON."},
    obj! {"role"=>"user","content"=>format!("Answers:\n{}\n\nCurrent request:\n{input}",answers.offer().to_json())}],"format"=>answers.schema()};
    Ok(obj! {"handled"=>true,"definition"=>definition,"request"=>request,"reserved_output"=>32,"selection"=>selection})
}
