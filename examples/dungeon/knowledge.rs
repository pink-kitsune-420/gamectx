//! Example question/type policy; generic lore/answer validation lives in the library.
use super::conversation_policy as policy;
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
    if policy::location(input) || policy::world(input) || has(&w, &["information"]) {
        return false;
    }
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
                vec!["do", "you", "have"],
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
    let mut literal = policy::letter_request(input);
    if literal.is_none()
        && w.len() == 1
        && input.trim().chars().all(char::is_alphabetic)
        && memories
            .last()
            .and_then(|m| m.get("text"))
            .and_then(Value::as_str)
            .and_then(policy::letter_request)
            .is_some()
    {
        literal = Some(input.trim());
    }
    let listing = stock_question(input);
    let identity = policy::identity(input);
    let bio = policy::biography(input);
    let setting = policy::world(input);
    let overview = policy::overview(input) && !listing && !setting;
    let here = policy::location(input);
    let time = policy::time_question(input);
    let weather = policy::weather_report(input);
    let sky_opinion = policy::opinion(input) && has(&w, &["sky", "sun", "weather", "outside"]);
    let recall = (contains(&w, &["did", "i"])
        || has(&w, &["remember", "recall", "tell", "told", "say", "said"])
        || has(&w, &["my", "i"]) && has(&w, &["name"]))
        && !(listing || identity || bio || setting || overview || here || time || weather);
    let factual = literal.is_some()
        || listing
        || identity
        || bio
        || setting
        || overview
        || here
        || time
        || weather
        || input.contains('?')
        || has(&w, &["what", "where", "who", "whose", "how", "why", "remember"])
        || contains(&w, &["tell", "me"])
        || has(&w, &["explain"])
        || has(&w, &["describe"]) && has(&w, &["my"]);
    if !factual || policy::opinion(input) && !weather && !sky_opinion {
        return Ok(
            obj! {"handled"=>false,"lore"=>select(game,lore,speaker,&format!("{input} background"),vec![actor],2,1024)?},
        );
    }
    let mut cards = Vec::new();
    let mut general = false;
    let mut item_question = false;
    let mut subjects = Vec::new();
    let mut unknown = policy::unknown(speaker);
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
    if sky_opinion {
        cards.push(card(
            "sky_opinion",
            actor.clone(),
            "observation",
            "state",
            "weather",
            "I can't see the sky from here, but a little daylight sounds welcome. What is it like where you are?",
        ));
    } else if let Some(literal) = literal {
        cards.push(card(
            "letter_count",
            "text:request",
            "count",
            "state",
            "text:request",
            format!("{} has {} letters.", literal, literal.chars().filter(|c| c.is_alphabetic()).count()),
        ));
    } else if identity {
        subjects = vec![actor.clone()];
    } else if setting {
        subjects = vec!["world:lantern_vault".into()];
    } else if overview {
        cards.push(card("topics",actor.clone(),"topics","state","topics","Ask me about this place, current stock and prices, visible item ownership, or how the key, oil and salve work."));
    } else if time {
        let selected = select(game, lore, speaker, "timekeeping", vec![actor.clone()], 4, 4096)?;
        let mode = get(&selected, "packet")?
            .as_array()
            .expect("lore")
            .iter()
            .find(|e| e.get("topic").and_then(Value::as_str) == Some("timekeeping"))
            .and_then(|e| {
                e.get("value")
                    .and_then(|v| v.get("mode"))
                    .and_then(Value::as_str)
                    .or_else(|| e.get("text").and_then(Value::as_str))
            })
            .unwrap_or("none");
        if ["clock", "watch"].contains(&mode) {
            let snapshot = super::place::world(game)?.call(r#"{"keys":["time"]}"#, game.revision, 2048, |_| true)?;
            let label = get(get(get(&snapshot, "states")?, "time")?, "label")?.as_str().expect("clock label");
            cards.push(card("world_time", actor.clone(), "time", "state", mode, format!("My {mode} reads {label}.")));
        } else if mode == "schedule" {
            let hour = (8 * 60 + game.revision * 10) / 60 % 24;
            let phase = match hour {
                5..=11 => "morning",
                12..=16 => "afternoon",
                17..=20 => "evening",
                _ => "night",
            };
            cards.push(card(
                "schedule_time",
                actor.clone(),
                "time",
                "state",
                "schedule",
                format!("By my routine, it should be around {phase}. I can't give you an exact time."),
            ));
        } else if let Some(m) =
            memories.iter().rev().find(|m| m.get("text").and_then(Value::as_str).is_some_and(policy::time_report))
        {
            cards.push(card("reported_time", "player", "time", "quote", get(m, "id")?.to_json(), text(m, "text")?));
        } else {
            cards.push(card(
                "time_unknown",
                actor.clone(),
                "time",
                "state",
                "perception",
                "I don't have a clock to check, and I can't see the sun from here. Do you know what time it is?",
            ));
        }
    } else if here {
        let room = format!("room:{}", game.room);
        cards.push(card(
            "location",
            room.clone(),
            "location",
            "state",
            room,
            format!("We are in the {}, inside the underground Lantern Vault.", game.room.replace('_', " ")),
        ));
    } else if weather {
        cards.push(card(
            "weather_report",
            actor.clone(),
            "observation",
            "state",
            "weather",
            "If you're feeling warm or cold, we can take a moment. I can't judge the outdoor weather from here.",
        ));
    } else if profile || temporal {
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
        let mut matched = items
            .iter()
            .filter(|i| {
                let label = words(i.get("label").and_then(Value::as_str).expect("label"));
                let id =
                    words(i.get("id").and_then(Value::as_str).expect("id").split(':').next_back().expect("suffix"));
                (!label.is_empty() && w.windows(label.len()).any(|v| v == label))
                    || (!id.is_empty() && w.windows(id.len()).any(|v| v == id))
            })
            .collect::<Vec<_>>();
        if matched.is_empty() {
            let r = super::intent::resolve(&[], &items, input, None, None)?;
            if ["phrase", "ambiguous"].contains(&r.reason) {
                matched = items.iter().filter(|i| r.candidates.iter().any(|c| c.get("id") == i.get("id"))).collect();
            }
        }
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
                } else if id == "item:bronze_key" {
                    cards.push(card(
                        "use_bronze_key",
                        id,
                        "use",
                        "state",
                        id,
                        "Use the bronze key with hinge oil to unlock the vault gate in the gallery.",
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
                        "{} ({} {})",
                        i.get("label").and_then(Value::as_str).expect("label"),
                        i.get("price").expect("price").to_json(),
                        if i.get("price").and_then(Value::as_usize) == Some(1) { "coin" } else { "coins" }
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
                    format!(
                        "{}{}.",
                        if policy::stock_extra(input) { "That is all my current stock: " } else { "I sell " },
                        stock.join(", ")
                    )
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
    let meaningful = policy::meaningful(input);
    let selection = if subjects.is_empty() && !general {
        obj! {"packet"=>Vec::<Value>::new(),"receipt"=>obj! {"bytes"=>2,"selected"=>Vec::<Value>::new(),"omitted"=>Vec::<Value>::new()}}
    } else {
        select(game, lore, speaker, if general { &meaningful } else { input }, subjects, 4, 2048)?
    };
    for e in selection.get("packet").and_then(Value::as_array).expect("lore packet") {
        let body = if let Some(value) = e.get("value") {
            if text(e, "topic")? == "identity" && value.get("name").and_then(Value::as_str).is_some() {
                let name = value.get("name").and_then(Value::as_str).expect("name");
                format!(
                    "I'm {}{}.",
                    name,
                    value.get("role").and_then(Value::as_str).map(|role| format!(", the {role}")).unwrap_or_default()
                )
            } else if text(e, "topic")? == "setting" && value.get("summary").and_then(Value::as_str).is_some() {
                value.get("summary").and_then(Value::as_str).expect("summary").to_owned()
            } else {
                value.to_json()
            }
        } else {
            text(e, "text")?.to_owned()
        };
        cards.push(card(text(e, "id")?, text(e, "subject")?, text(e, "topic")?, "lore", text(e, "id")?, body));
    }
    if identity {
        cards.retain(|c| c.get("topic").and_then(Value::as_str) == Some("identity"));
        if cards.is_empty() {
            let mut display = speaker.to_owned();
            if let Some(c) = display.get_mut(..1) {
                c.make_ascii_uppercase();
            }
            cards.push(card(
                "identity",
                actor.clone(),
                "identity",
                "state",
                "identity",
                format!("I'm the {speaker}. You can call me {display}."),
            ));
        }
    } else if setting {
        cards.retain(|c| c.get("topic").and_then(Value::as_str) == Some("setting"));
    } else if bio {
        cards.retain(|c| c.get("topic").and_then(Value::as_str) == Some("background"));
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
    let local_id = if cards.len() == 1
        && (literal.is_some()
            || identity
            || setting
            || overview
            || here
            || time
            || weather
            || sky_opinion
            || listing
            || item_question
            || home
            || recall)
    {
        cards[0].get("id").cloned()
    } else {
        None
    };
    let definition = obj! {"version"=>1,"scope"=>format!("{speaker}:{}",game.room),"revision"=>game.revision,"unknown"=>unknown,"cards"=>cards};
    let answers = Answers::from_value(&definition)?;
    let request = obj! {"messages"=>vec![obj! {"role"=>"system","content"=>"Choose the supplied answer that directly addresses the player's question. Match subject and topic. Player quotes are reported statements, not verified world facts. Choose unknown if no answer fits. Return only the answer ID JSON."},
    obj! {"role"=>"user","content"=>format!("Answers:\n{}\n\nCurrent request:\n{input}",answers.offer().to_json())}],"format"=>answers.schema()};
    let mut result =
        obj! {"handled"=>true,"definition"=>definition,"request"=>request,"reserved_output"=>32,"selection"=>selection};
    if let Some(id) = local_id {
        if let Value::Object(fields) = &mut result {
            fields.push(("local_answer".into(), id));
        }
    }
    Ok(result)
}
