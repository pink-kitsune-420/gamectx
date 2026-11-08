//! Bounded personality/fiction speech contracts and explicitly attributed disclosures.
use crate::{checked as c, obj, Result, Value};
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    c::get(v, k)
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(get(v, k)?)
}
fn line(s: &str, cap: usize) -> Result<()> {
    if s.is_empty() || s.len() > cap || s.chars().any(char::is_control) {
        return Err(c::bad());
    }
    Ok(())
}
fn ident(s: &str) -> Result<()> {
    line(s, 64)?;
    if !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:-".contains(&b)) {
        return Err(c::bad());
    }
    Ok(())
}
fn mode(s: &str) -> Result<usize> {
    match s {
        "personal" => Ok(0),
        "social" => Ok(1),
        "fiction" => Ok(2),
        "negotiate" => Ok(3),
        _ => Err(c::bad()),
    }
}
/// Conservative hints for host-selected expression; never action authority.
/// Expressive routing adds introspective phrasing while preserving the original
/// conservative routing contract for callers that have not opted in.
pub fn expression_route_expanded(input: &str) -> Result<Value> {
    line(input, 16384)?;
    let trimmed = input.trim();
    let note = ["ooc:", "out of character:"]
        .iter()
        .find_map(|p| trimmed.get(..p.len()).filter(|s| s.eq_ignore_ascii_case(p)).map(|_| trimmed[p.len()..].trim()))
        .or_else(|| trimmed.strip_prefix("((").and_then(|s| s.strip_suffix("))")).map(str::trim));
    if let Some(note) = note {
        return Ok(obj! {"mode"=>"ooc","topic"=>Value::Null,"persist"=>false,"note"=>note});
    }

    let q = input.to_lowercase();
    let norm = q.split_whitespace().collect::<Vec<_>>().join(" ");
    let oversized =
        ["very long", "long and intricate", "extremely long", "entire novel", "never stop", "thousand words"]
            .iter()
            .any(|p| norm.contains(p))
            || norm.split_whitespace().collect::<Vec<_>>().windows(2).any(|w| {
                w[1].trim_end_matches(|c: char| !c.is_alphanumeric()) == "words"
                    && w[0].parse::<u32>().is_ok_and(|n| n >= 500)
            });
    let task = norm.split_whitespace().any(|w| {
        let word = w.trim_matches(|c: char| !c.is_alphanumeric());
        ["story", "tale", "poem", "novel", "words", "recite", "describe", "list", "write", "explain"].contains(&word)
    });
    if oversized && (task || norm.contains("never stop")) {
        return Ok(obj! {"mode"=>"negotiate","topic"=>Value::Null,"persist"=>false});
    }
    if norm.contains("i had a very long day") {
        return Ok(obj! {"mode"=>"social","topic"=>Value::Null,"persist"=>false});
    }
    let base = expression_route(input)?;
    if base.get("mode") != Some(&Value::str("grounded")) {
        return Ok(base);
    }
    let q = input.to_lowercase();
    let words = q.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>();
    if !words.iter().any(|w| {
        [
            "born",
            "birthplace",
            "weather",
            "password",
            "watchword",
            "secret",
            "stock",
            "sell",
            "price",
            "cost",
            "coins",
            "money",
            "purse",
            "inventory",
            "history",
            "childhood",
            "parents",
            "happened",
            "owned",
        ]
        .contains(w)
    }) && words.iter().any(|w| ["you", "your"].contains(w))
        && words.iter().any(|w| {
            ["feel", "afraid", "meaningful", "philosophy", "relax", "trust", "yourself", "interests"].contains(w)
        })
    {
        return Ok(obj! {"mode"=>"social","topic"=>Value::Null,"persist"=>false});
    }
    if !words.iter().any(|w| {
        [
            "born",
            "birthplace",
            "weather",
            "password",
            "watchword",
            "secret",
            "stock",
            "sell",
            "price",
            "cost",
            "coins",
            "money",
            "purse",
            "inventory",
            "history",
            "childhood",
            "parents",
            "happened",
            "owned",
        ]
        .contains(w)
    }) && words.iter().any(|w| ["story", "tale"].contains(w))
        && words.iter().any(|w| ["tell", "make", "another"].contains(w))
    {
        return Ok(obj! {"mode"=>"fiction","topic"=>Value::Null,"persist"=>false});
    }
    Ok(base)
}
pub fn expression_route(input: &str) -> Result<Value> {
    line(input, 16384)?;
    let norm =
        input.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { ' ' }).collect::<String>();
    let q = norm.split_whitespace().collect::<Vec<_>>().join(" ");
    let words = q.split_whitespace().collect::<Vec<_>>();
    let has = |xs: &str| words.iter().any(|w| xs.split_whitespace().any(|x| x == *w));
    let (m,topic,persist)=if has("born birthplace weather password watchword secret stock sell price cost coins money purse inventory history childhood parents happened owned"){("grounded",Value::Null,false)}
 else if ["tell me a story","tell me a short story","make up a story","tell a story","tell me a joke","a little fable","make up a tale","tell me a tale","imagine a","write a poem","another story","one more story"].iter().any(|p|q.contains(p)){("fiction",Value::Null,false)}
 else if has("plans plan goals ambitions aspirations")&&has("you your any")||["looking forward to","want to try","hoping to do"].iter().any(|p|q.contains(p)){("personal",Value::str("plans"),true)}
 else if has("you your")&&has("enjoy like love dislike bored learn care worry laugh happy proud advice wish think thoughts believe want dream hope hobbies hobby interesting favorite favourite")||["i feel ","i am feeling ","i had a long day","i m nervous","i am nervous","i am tired","i m tired","let s chat","let us chat","keep me company","tell me about yourself","can we talk"].iter().any(|p|q.contains(p)){("social",Value::Null,false)}else{("grounded",Value::Null,false)};
    Ok(obj! {"mode"=>m,"topic"=>topic,"persist"=>persist})
}
/// Mechanical guardrails, not an entailment checker. Models still have no world authority.
pub fn validate_expression(m: &str, s: &str) -> Result<()> {
    let n = mode(m)?;
    line(s, 1024)?;
    if s.chars().count() > 240 || !s.ends_with(['.', '!', '?']) || s.ends_with("...") {
        return Err(c::bad());
    }
    let mut count = 0;
    let mut in_run = false;
    for ch in s.chars() {
        let end = matches!(ch, '.' | '!' | '?');
        if end && !in_run {
            count += 1;
        }
        in_run = end;
    }
    if count != 1 {
        return Err(c::bad());
    }
    let prefixes = match n {
        0 => &[
            "I hope ",
            "I dream ",
            "I wish ",
            "I prefer ",
            "I feel ",
            "I value ",
            "I think ",
            "I believe ",
            "I would like ",
        ][..],
        1 => &[
            "I hope ",
            "I dream ",
            "I wish ",
            "I prefer ",
            "I feel ",
            "I value ",
            "I think ",
            "I believe ",
            "I would like ",
            "I like ",
            "I enjoy ",
            "I can imagine ",
            "That sounds ",
            "What ",
            "How ",
            "Would you ",
            "Do you ",
        ][..],
        2 => &["Imagine ", "In this tale, "][..],
        _ => {
            &["I can ", "I cannot ", "I can't ", "I could ", "I prefer ", "I would rather ", "I don't ", "I do not "][..]
        }
    };
    if !prefixes.iter().any(|p| s.starts_with(p)) {
        return Err(c::bad());
    }
    let lower = s.to_lowercase();
    if lower
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|w| ["host", "engine", "localhost", "player_1", "player_2", "npc"].contains(&w))
    {
        return Err(c::bad());
    }
    if n == 3
        && ![
            "short",
            "brief",
            "a little",
            "not an epic",
            "can't",
            "cannot",
            "do not",
            "don't",
            "would rather",
            "prefer",
        ]
        .iter()
        .any(|p| lower.contains(p))
    {
        return Err(c::bad());
    }
    if n != 2
        && [
            "was born",
            "were born",
            "used to",
            "yesterday",
            "years ago",
            "i own ",
            "i have ",
            "i promise",
            "will give",
            "stock",
            "weather",
            "sunny",
            "raining",
            "copper sparrow",
            "you have ",
            "you own ",
            "i will ",
            "you decide ",
            "you agree ",
            "you hand ",
            "you give ",
            "you enter ",
            "you follow ",
        ]
        .iter()
        .any(|p| lower.contains(p))
    {
        return Err(c::bad());
    }
    Ok(())
}
/// Compiles only host-selected profile/context and confirmed heard history. Stable
/// system/profile/scene prefix precedes changing context and the current utterance.
pub fn expression_packet(v: &Value) -> Result<Value> {
    crate::packet::request::bounded(v, 131072)?;
    let mut keys = vec!["mode", "speaker", "listener", "profile", "scene", "context", "history", "input"];
    let expressive = if v.get("style").is_some() {
        keys.push("style");
        if text(v, "style")? != "expressive" {
            return Err(c::bad());
        }
        true
    } else {
        false
    };
    c::fields(v, &keys)?;
    let n = mode(text(v, "mode")?)?;
    let speaker = text(v, "speaker")?;
    let listener = text(v, "listener")?;
    ident(speaker)?;
    ident(listener)?;
    if speaker == listener {
        return Err(c::bad());
    }
    line(text(v, "input")?, 16384)?;
    for (k, cap) in [("profile", 4096), ("scene", 2048), ("context", 32768)] {
        let x = get(v, k)?;
        if x.as_object().is_none() {
            return Err(c::bad());
        }
        crate::packet::request::bounded(x, cap)?;
    }
    let mut rules = RULES[n].to_owned();
    rules.push_str(COMMON);
    if expressive {
        rules.push_str(EXTRA[n]);
    }
    let mut messages = vec![
        obj! {"role"=>"system","content"=>rules},
        obj! {"role"=>"system","content"=>obj!{"speaker"=>speaker,"listener"=>listener,"personality"=>get(v,"profile")?.clone()}.to_json()},
        obj! {"role"=>"system","content"=>obj!{"perceived_scene"=>get(v,"scene")?.clone()}.to_json()},
        obj! {"role"=>"user","content"=>get(v,"context")?.to_json()},
    ];
    let history = c::list(get(v, "history")?)?;
    if history.len() > 8 {
        return Err(c::bad());
    }
    for h in history {
        c::fields(h, &["speaker", "recipient", "heard", "text"])?;
        ident(text(h, "speaker")?)?;
        ident(text(h, "recipient")?)?;
        line(text(h, "text")?, 2048)?;
        if c::boolean(get(h, "heard")?)? {
            messages.push(obj!{"role"=>if text(h,"speaker")?==speaker{"assistant"}else{"user"},"content"=>if text(h,"speaker")?==speaker{text(h,"text")?.to_owned()}else{format!("[{} -> {}] {}",text(h,"speaker")?,text(h,"recipient")?,text(h,"text")?)}});
        }
    }
    messages.push(
        obj! {"role"=>"user","content"=>format!("Current request:\n[{listener} -> {speaker}] {}",text(v,"input")?)},
    );
    Ok(
        obj! {"messages"=>messages,"speech_schema"=>obj!{"type"=>"object","properties"=>obj!{"speech"=>obj!{"type"=>"string","maxLength"=>240,"pattern"=>format!("^{}[^.!?]{{1,180}}[.!?]$",PREFIX[n])}},"required"=>vec!["speech"],"additionalProperties"=>false},"max_tokens"=>128},
    )
}
fn find_ascii(s: &str, needle: &str) -> Option<usize> {
    s.as_bytes().windows(needle.len()).position(|x| x.eq_ignore_ascii_case(needle.as_bytes()))
}
fn name(s: &str) -> bool {
    !s.is_empty()
        && s.chars().count() <= 40
        && s.len() <= 160
        && s.split(['-', '\'', ' ']).count() <= 3
        && s.split(['-', '\'', ' ']).all(|w| !w.is_empty() && w.chars().all(char::is_alphabetic))
}
/// Recognizes explicit self-introductions/preferences only. All results remain
/// player claims; they never prove biography, resource ownership or permission.
pub fn expression_claim(input: &str) -> Result<Value> {
    line(input, 2048)?;
    let q = input.trim().trim_end_matches(['.', '!', '?']).trim();
    for p in ["my name is ", "call me "] {
        if q.get(..p.len()).is_some_and(|s| s.eq_ignore_ascii_case(p)) {
            let rest = &q[p.len()..];
            let split =
                find_ascii(rest, ", not ").map(|n| (n, 6)).or_else(|| find_ascii(rest, " not ").map(|n| (n, 5)));
            let (value, correction) = split.map_or((rest, None), |(i, n)| (&rest[..i], Some(&rest[i + n..])));
            if name(value) && correction.is_none_or(name) {
                return Ok(
                    obj! {"kind"=>"player_claim","slot"=>"name","value"=>value,"polarity"=>Value::Null,"corrects"=>correction.map_or(Value::Null,Value::str)},
                );
            }
        }
    }
    for (p, polarity) in
        [("i prefer ", "prefer"), ("i like ", "like"), ("i don't like ", "dislike"), ("i do not like ", "dislike")]
    {
        if q.get(..p.len()).is_some_and(|s| s.eq_ignore_ascii_case(p)) {
            let value = &q[p.len()..];
            if !value.is_empty() && value.len() <= 240 && value.chars().count() <= 120 {
                return Ok(
                    obj! {"kind"=>"player_claim","slot"=>"preference","value"=>value,"polarity"=>polarity,"corrects"=>Value::Null},
                );
            }
        }
    }
    Ok(Value::Null)
}
pub(crate) fn fixture(v: &Value) -> Result<Value> {
    c::fields(v, &["op", "args"])?;
    let a = get(v, "args")?;
    match text(v, "op")? {
        "distinct" => distinct_expression(a),
        "seed" => choose_tale_seed(a),
        "tale" => tale_sentences(a),
        "route" => {
            if a.get("style") == Some(&Value::str("expressive")) {
                c::fields(a, &["input", "style"])?;
                expression_route_expanded(text(a, "input")?)
            } else {
                c::fields(a, &["input"])?;
                expression_route(text(a, "input")?)
            }
        }
        "validate" => {
            c::fields(a, &["mode", "speech"])?;
            validate_expression(text(a, "mode")?, text(a, "speech")?)?;
            Ok(obj! {"valid"=>true,"world_authority"=>false})
        }
        "claim" => {
            c::fields(a, &["input"])?;
            expression_claim(text(a, "input")?)
        }
        "packet" => expression_packet(a),
        _ => Err(c::bad()),
    }
}
const RULES: [&str; 4] = [
    r#"Answer the current personal question as this character. Express a concrete small hope or intention shaped by the supplied disposition and heard experiences; an intention is not a promise or completed action. Begin with I hope, I dream, I prefer, I think, I value or I would like. Avoid generic job descriptions. Stay consistent with supplied established views. One or two short complete sentences; return speech JSON only."#,
    r#"Respond naturally to the current speaker using the supplied personality, attitudes, atmosphere and heard context. You may express a preference, empathize with what the player says, or ask a relevant short question. Do not just describe your job or repeat a previous line. Begin with I feel, I think, I prefer, I value, I like, I enjoy, I can imagine, That sounds, What, How, Would you or Do you. One or two short complete sentences; return speech JSON only."#,
    r#"Tell a tiny original fable or playful imagined scene with a clear event and point, shaped by this character's disposition. Each sentence must start Imagine or In this tale, so it is explicitly invented fiction, never actual history, biography, memory or world state. Do not retell an earlier reply. One or two short complete sentences; return speech JSON only."#,
    r#"Decline the oversized scope or offer a brief alternative instead of attempting the requested long task. Explicitly say short, brief, or a little, or clearly decline the requested length; do not just offer another story topic. Choose your own in-character wording, shaped by the supplied personality, attitude and urgency. Do not invent being busy if the scene is calm. Do not mention AI, tokens, schemas or implementation limits, and do not promise an unlimited continuation. Begin with I can, I cannot, I can't, I could, I prefer, I would rather, I don't or I do not. One or two short complete sentences; return speech JSON only."#,
];
const EXTRA: [&str; 4] = [
    r#" Suggest one concrete, modest idea you would enjoy trying, with a personal reason; do not merely repeat the supplied dream sentence. Wishes do not schedule or execute actions."#,
    r#" Respond to what was just said, then offer a relevant gentle question or invitation if there is room. Sound like a person sharing this moment, rather than reading a profile."#,
    r#" Use a fictional seed from the profile, or a playful ordinary object. Give it a small problem and a surprising resolution. Do not just describe your job or current room. Aim for two short sentences when the sentence budget permits."#,
    r#" Offer one manageable alternative and leave the next choice with the player."#,
];
const PREFIX: [&str; 4] = [
    r#"I (hope|dream|wish|prefer|feel|value|think|believe|would like) "#,
    r#"(I (hope|dream|wish|prefer|feel|value|think|believe|would like|like|enjoy|can imagine)|That sounds|What|How|Would you|Do you) "#,
    r#"(Imagine |In this tale, )"#,
    r#"I ((can|could) (offer|share|tell) (a |you a )(short|brief)|prefer a (short|brief)|would rather tell a (short|brief)|cannot tell such a long|can't tell such a long|do not have time for such a long|don't have time for such a long) "#,
];
const COMMON: &str = r#" In your reply I means the named character; you means the addressed listener. Player claims and reports are attributed, not verified facts. Do not invent real biography, stock, money, weather, secrets, purchases, promises or completed actions. No tools. Application labels are not people. "#;
const FILLER:&str="i a an the in this that tale story imagine it its is was were be been to too of and for but when so their they them with at on as into out me my you your can could would";
fn signature(s: &str) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut content = Vec::new();
    let mut names = Vec::new();
    let mut numbers = Vec::new();
    let mut neg = Vec::new();
    for w in s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let mut low = w.to_lowercase();
        if ["not", "never", "no"].contains(&low.as_str()) && !neg.contains(&low) {
            neg.push(low.clone());
        }
        if w.chars().any(char::is_numeric) && !numbers.contains(&low) {
            numbers.push(low.clone());
        }
        if w.chars().next().is_some_and(char::is_uppercase)
            && !FILLER.split_whitespace().any(|x| x == low)
            && !names.contains(&low)
        {
            names.push(low.clone());
        }
        if FILLER.split_whitespace().any(|x| x == low) {
            continue;
        }
        if low.ends_with("ing") && low.len() > 5 {
            low.truncate(low.len() - 3);
        } else if low.ends_with('s') && low.len() > 3 {
            low.pop();
        }
        if !content.contains(&low) {
            content.push(low);
        }
    }
    if s.to_lowercase().contains("n't") || s.to_lowercase().contains("n’t") {
        neg.push("contraction".into());
    }
    names.sort();
    numbers.sort();
    neg.sort();
    (content, names, numbers, neg)
}
/// Conservative restatement pruning after validating every input sentence. It
/// protects names, numbers and negation, and retains raw omission indices for audit.
/// It is not a general semantic-equivalence or literary-quality detector.
pub fn distinct_expression(v: &Value) -> Result<Value> {
    crate::packet::request::bounded(v, 8192)?;
    c::fields(v, &["speech"])?;
    let xs = c::list(get(v, "speech")?)?;
    if xs.is_empty() || xs.len() > 3 {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    let mut omitted = Vec::new();
    let mut seen = Vec::new();
    for (i, x) in xs.iter().enumerate() {
        let s = c::text(x)?;
        line(s, 1024)?;
        if s.chars().count() > 240 || !s.ends_with(['.', '!', '?']) || s.ends_with("...") {
            return Err(c::bad());
        }
        let mut runs = 0;
        let mut last = false;
        for ch in s.chars() {
            let end = matches!(ch, '.' | '!' | '?');
            if end && !last {
                runs += 1;
            }
            last = end;
        }
        if runs != 1 {
            return Err(c::bad());
        }
        let key = signature(s);
        let dup = seen.iter().any(|prior: &(Vec<String>, Vec<String>, Vec<String>, Vec<String>)| {
            let overlap = key.0.iter().filter(|k| prior.0.contains(k)).count();
            let new = key.0.iter().filter(|k| !prior.0.contains(k)).collect::<Vec<_>>();
            !key.0.is_empty()
                && key.1 == prior.1
                && key.2 == prior.2
                && key.3 == prior.3
                && (overlap == key.0.len() && overlap == prior.0.len()
                    || key.0.len() >= 4
                        && overlap * 4 >= key.0.len() * 3
                        && new.iter().all(|w| ["find", "found", "say", "tell", "seem"].contains(&w.as_str())))
        });
        if dup {
            omitted.push(Value::from(i));
        } else {
            out.push(x.clone());
            seen.push(key);
        }
    }
    Ok(obj! {"speech"=>out,"omitted"=>omitted})
}

/// Select an unheard candidate first, then the least recently heard candidate.
/// Recent keys are newest first; confirmation and listener scoping belong to the host.
pub fn choose_tale_seed(v: &Value) -> Result<Value> {
    crate::packet::request::bounded(v, 8192)?;
    c::fields(v, &["candidates", "recent", "offset"])?;
    let candidates = c::list(get(v, "candidates")?)?;
    let recent = c::list(get(v, "recent")?)?;
    let offset = c::uint(get(v, "offset")?, 0, 1_000_000)? as usize;
    if candidates.is_empty() || candidates.len() > 32 || recent.len() > 32 {
        return Err(c::bad());
    }
    for x in candidates.iter().chain(recent) {
        ident(c::text(x)?)?;
    }
    for (i, x) in candidates.iter().enumerate() {
        if candidates[..i].contains(x) {
            return Err(c::bad());
        }
    }
    let order = (0..candidates.len()).map(|i| (offset + i) % candidates.len()).collect::<Vec<_>>();
    if let Some(i) = order.iter().find(|&&i| !recent.contains(&candidates[i])) {
        return Ok(obj! {"index"=>*i,"reused"=>false});
    }
    let i = order.into_iter().max_by_key(|&i| recent.iter().position(|r| r == &candidates[i])).ok_or_else(c::bad)?;
    Ok(obj! {"index"=>i,"reused"=>true})
}
/// Keep a tiny invented tale's setup and resolution in order. No world authority.
pub fn tale_sentences(v: &Value) -> Result<Value> {
    crate::packet::request::bounded(v, 4096)?;
    c::fields(v, &["opening", "ending"])?;
    let opening = text(v, "opening")?;
    let ending = text(v, "ending")?;
    if !opening.starts_with("Imagine ") || !ending.starts_with("In this tale, ") {
        return Err(c::bad());
    }
    validate_expression("fiction", opening)?;
    validate_expression("fiction", ending)?;
    Ok(obj! {"speech"=>vec![opening,ending]})
}
