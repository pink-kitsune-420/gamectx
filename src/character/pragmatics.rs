//! Pragmatic parser, mixed-initiative agenda and conversational stance (pass 3, stage 3).
//!
//! **Parser.** `parse` splits a player turn at sentence punctuation (`.` `?` `!` `;` and the em
//! dash) into at most 4 clauses, reads each clause's words (lower-cased runs of ASCII letters and
//! digits) and classifies it with an original phrase lexicon in a fixed precedence: threat,
//! accusation, apology, offer or transfer, request for help, greeting, farewell, compliment,
//! then a question about facts or about the listener, else small talk. Authors add phrases with
//! `lexicon`. Subtext rules read urgency, sarcasm (praise for something the context says failed),
//! testing honesty (asking about something the speaker witnessed), guilt-tripping, bargaining,
//! evasion and probing; the default is sincere. Each clause says whether it needs a state update
//! (an apology to appraise, an item to receive) and a reply. `validate` checks clauses a model
//! proposed instead: known acts and subtexts, texts found in the utterance in order.
//!
//! **Agenda.** `agenda` blends a turn's obligations with the character's own. A motive scoring
//! 90 or more overrides polite routing (updates still apply, replies are deferred); a strong own
//! item is raised after the answers (answer then counter-ask); a grievance of priority 80 or more
//! withholds answers to non-urgent questions until it is addressed (conditional gate). The thread
//! stack (`open`, `interrupt`, `resume`, `close`, `threads`) keeps a few conversation threads per
//! being, so an interrupted one can be resumed with its unanswered question.
//!
//! **Stance.** `stance` gives cooperativeness from trust, affection and resentment, clamps a
//! confidential reply to a whisper or a deflection when an untrusted bystander is in earshot, and
//! shortens replies as task urgency rises.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend); model-proposed clauses are untrusted input to `validate`.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::pragmatics::Pragmatics;
//!
//! let mut p = Pragmatics::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "parse", "args": {"text": "Sorry I'm late. Here it is. Did anyone come looking for me?"}}
//! ]}"#, 4096)?;
//! let out = p.workflow(&req)?.to_json();
//! // Three clauses: an apology to appraise, an item to receive and a question to answer.
//! assert!(out.contains(r#""act":"apology","cues":["sorry"]"#));
//! assert!(out.contains(r#""update":"receive_item""#));
//! assert!(out.contains(r#""act":"question_fact""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_bool, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::frame::time_label;
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, is_text, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "threads", default: 4, ceiling: 16 },
    Limit { name: "phrases", default: 64, ceiling: 4096 },
    Limit { name: "own", default: 8, ceiling: 64 },
];
pub const MAX_CLAUSES: usize = 4;
const BOUNDARY: [char; 5] = ['.', '?', '!', ';', '\u{2014}'];
const LEADING: [&str; 8] = ["by the way", "and also", "also", "anyway", "and", "but", "so", "oh"];
/// Lexicon acts in precedence order.
pub const PRECEDENCE: [&str; 8] =
    ["threat", "accusation", "apology", "offer_or_transfer", "request_help", "greeting", "farewell", "compliment"];
pub const ACTS: [&str; 11] = [
    "threat",
    "accusation",
    "apology",
    "offer_or_transfer",
    "request_help",
    "greeting",
    "farewell",
    "compliment",
    "question_fact",
    "question_personal",
    "small_talk",
];
pub const SUBTEXTS: [&str; 8] =
    ["sincere", "probing", "evasive", "sarcastic", "urgent", "guilt_tripping", "bargaining", "testing_honesty"];
/// The built-in phrase lexicon, one row per act in precedence order.
pub const LEXICON: [&[&str]; 8] = [
    &[
        "or else",
        "you will regret",
        "you ll regret",
        "watch yourself",
        "i will hurt you",
        "i ll hurt you",
        "last warning",
    ],
    &["you stole", "you took", "you lied", "you broke", "you cheated", "liar", "thief", "your fault"],
    &["sorry", "apologize", "apologise", "apologies", "forgive me", "my fault", "my mistake"],
    &[
        "here is",
        "here s",
        "here are",
        "here it is",
        "take this",
        "i brought",
        "this is for you",
        "a gift for you",
        "as promised",
        "i have your",
    ],
    &["help me", "can you help", "could you", "would you", "please help", "i need you to", "lend me"],
    &["hello", "hi", "hey", "good morning", "good evening", "greetings", "well met"],
    &["goodbye", "farewell", "good night", "see you", "until tomorrow"],
    &[
        "well done",
        "thank you",
        "thanks",
        "fine work",
        "good work",
        "beautiful",
        "you look",
        "nicely done",
        "great job",
    ],
];
const QUESTION_START: [&str; 17] = [
    "who", "what", "where", "when", "why", "how", "is", "are", "do", "does", "did", "can", "could", "will", "would",
    "have", "has",
];
const YOU: [&str; 4] = ["you", "your", "yours", "yourself"];
const URGENT: [&str; 6] = ["now", "quick", "quickly", "hurry", "immediately", "at once"];
const GUILT: [&str; 5] = ["after all i", "after everything", "i thought you", "you never", "you always"];
const BARGAIN: [&str; 5] = ["if you", "in exchange", "for a price", "a deal", "what s in it"];
const EVASIVE: [&str; 6] = ["maybe", "perhaps", "don t know", "never mind", "whatever", "not sure"];
const PROBE: [&str; 4] = ["really", "true", "honestly", "be honest"];
const OWN_KINDS: [&str; 3] = ["ask", "followup", "grievance"];
const GATED: [&str; 4] = ["question_fact", "question_personal", "request_help", "small_talk"];

fn need(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(c::bad())
    }
}

fn id_of(args: &Value, k: &str) -> Result<String> {
    Ok(ident(c::get(args, k)?)?.to_string())
}

fn opt_id(args: &Value, k: &str) -> Result<Option<String>> {
    opt(args, k).map(|v| ident(v).map(str::to_string)).transpose()
}

fn opt_str(x: &Option<String>) -> Value {
    x.as_deref().map(Value::from).unwrap_or(Value::Null)
}

/// Lower-cased runs of ASCII letters and digits, in order.
pub fn words(s: &str) -> Vec<String> {
    s.split(|ch: char| !ch.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_ascii_lowercase()).collect()
}

fn has(ws: &[String], phrase: &str) -> bool {
    let p: Vec<&str> = phrase.split(' ').collect();
    ws.windows(p.len()).any(|w| w.iter().zip(&p).all(|(a, b)| a == b))
}

fn update_of(act: &str) -> Option<&'static str> {
    match act {
        "apology" => Some("appraise_apology"),
        "offer_or_transfer" => Some("receive_item"),
        "accusation" => Some("appraise_accusation"),
        "threat" => Some("appraise_threat"),
        "compliment" => Some("appraise_praise"),
        _ => None,
    }
}

fn act_of(v: &Value) -> Result<&'static str> {
    let a = ident(v)?;
    ACTS.iter().copied().find(|x| *x == a).ok_or_else(c::bad)
}

fn subtext_of(v: &Value) -> Result<&'static str> {
    let s = ident(v)?;
    SUBTEXTS.iter().copied().find(|x| *x == s).ok_or_else(c::bad)
}

fn word_list(args: &Value, k: &str) -> Result<BTreeSet<String>> {
    let rows = match opt(args, k) {
        None => &[][..],
        Some(v) => v.as_array().ok_or_else(c::bad)?,
    };
    need(rows.len() <= 16)?;
    let mut out = BTreeSet::new();
    for w in rows {
        let w = w.as_str().ok_or_else(c::bad)?;
        need((1..=32).contains(&w.len()) && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))?;
        need(out.insert(w.to_string()))?;
    }
    Ok(out)
}

/// `(clause text, terminator)` pairs; the terminator is `None` at the end of the text.
fn split(s: &str) -> Vec<(&str, Option<char>)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        if BOUNDARY.contains(&ch) {
            out.push((&s[start..i], Some(ch)));
            start = i + ch.len_utf8();
        }
    }
    out.push((&s[start..], None));
    out.into_iter()
        .map(|(t, term)| (t.trim_matches(|c| c == ' ' || c == '\t' || c == ','), term))
        .filter(|(t, _)| !words(t).is_empty())
        .collect()
}

fn strip_leading(ws: &[String]) -> &[String] {
    for m in LEADING {
        let n = m.split(' ').count();
        if ws.len() > n && has(&ws[..n], m) {
            return &ws[n..];
        }
    }
    ws
}

/// A reply is owed for anything but sincere small talk.
fn owes_reply(act: &str, sub: &str) -> bool {
    act != "small_talk" || sub != "sincere"
}

fn clause_view(index: usize, text: &str, act: &str, sub: &str, extra: Vec<(String, Value)>) -> Value {
    let mut f = vec![("act".to_string(), Value::from(act))];
    f.extend(extra);
    f.push(("index".to_string(), Value::from(index)));
    f.push(("reply".to_string(), Value::from(owes_reply(act, sub))));
    f.push(("subtext".to_string(), Value::from(sub)));
    f.push(("text".to_string(), Value::from(text)));
    f.push(("update".to_string(), Value::from(update_of(act))));
    Value::Object(f)
}

/// One conversation thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub topic: String,
    pub speaker: String,
    pub question: Option<String>,
    pub status: &'static str,
    pub opened_at: u64,
    pub interrupted_at: Option<u64>,
    pub cause: Option<String>,
}

impl Thread {
    fn view(&self) -> Value {
        obj! {
            "cause" => opt_str(&self.cause), "id" => self.id.as_str(), "interrupted_at" => self.interrupted_at,
            "opened_at" => self.opened_at, "question" => opt_str(&self.question), "speaker" => self.speaker.as_str(),
            "status" => self.status, "topic" => self.topic.as_str(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pragmatics {
    /// Author phrases per lexicon act.
    pub custom: BTreeMap<String, Vec<String>>,
    pub count: usize,
    /// Thread stacks, bottom first.
    pub stacks: BTreeMap<String, Vec<Thread>>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Pragmatics {
    fn default() -> Self {
        Pragmatics {
            custom: PRECEDENCE.iter().map(|a| (a.to_string(), Vec::new())).collect(),
            count: 0,
            stacks: BTreeMap::new(),
            clock: 0,
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Pragmatics {
    pub fn new() -> Self {
        Self::default()
    }

    /// A parser with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "lexicon" => self.op_lexicon(args, l),
            "parse" => self.op_parse(args),
            "validate" => self.op_validate(args),
            "agenda" => self.op_agenda(args, l),
            "tick" => self.op_tick(args),
            "open" => self.op_open(args, l),
            "interrupt" => self.op_interrupt(args),
            "resume" => self.op_resume(args),
            "close" => self.op_close(args),
            "threads" => self.op_threads(args),
            "stance" => self.op_stance(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn classify(
        &self,
        all: &[String],
        term: Option<char>,
        witnessed: &BTreeSet<String>,
        failed: &BTreeSet<String>,
    ) -> (&'static str, &'static str, Vec<(String, Value)>) {
        let ws = strip_leading(all);
        let mut act: Option<&str> = None;
        let mut cues: BTreeSet<&str> = BTreeSet::new();
        for (i, a) in PRECEDENCE.iter().enumerate() {
            let custom = self.custom[*a].iter().map(String::as_str);
            cues = LEXICON[i].iter().copied().chain(custom).filter(|p| has(ws, p)).collect();
            if !cues.is_empty() {
                act = Some(a);
                break;
            }
        }
        let question = term == Some('?') || QUESTION_START.contains(&ws[0].as_str());
        let act = act.unwrap_or(if !question {
            "small_talk"
        } else if ws.iter().take(3).any(|w| YOU.contains(&w.as_str())) {
            "question_personal"
        } else {
            "question_fact"
        });
        let m = |lst: &[&str]| lst.iter().any(|p| has(ws, p));
        let is_q = act == "question_fact" || act == "question_personal";
        let sub = if m(&URGENT) || (term == Some('!') && act == "request_help") {
            "urgent"
        } else if act == "compliment" && ws.iter().any(|w| failed.contains(w)) {
            "sarcastic"
        } else if is_q && ws.iter().any(|w| witnessed.contains(w)) {
            "testing_honesty"
        } else if m(&GUILT) {
            "guilt_tripping"
        } else if m(&BARGAIN) {
            "bargaining"
        } else if m(&EVASIVE) {
            "evasive"
        } else if act == "question_personal" || (is_q && m(&PROBE)) {
            "probing"
        } else {
            "sincere"
        };
        let extra = vec![
            ("cues".into(), Value::Array(cues.into_iter().map(Value::from).collect())),
            ("question".into(), Value::from(question)),
        ];
        (act, sub, extra)
    }

    // ------------------------------------------------------------ parser

    fn op_lexicon(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["act", "phrases"], &[])?;
        let act = ident(c::get(args, "act")?)?;
        let i = PRECEDENCE.iter().position(|a| *a == act).ok_or_else(c::bad)?;
        let rows = c::get(args, "phrases")?.as_array().ok_or_else(c::bad)?;
        need((1..=16).contains(&rows.len()))?;
        let mut ps = Vec::new();
        for p in rows {
            let s = text_bound(p, 64)?;
            let ws = words(s);
            need((1..=4).contains(&ws.len()) && ws.join(" ") == s && !ps.contains(&s))?;
            ps.push(s);
        }
        let have = &self.custom[act];
        let new: Vec<String> = ps
            .iter()
            .filter(|p| !have.iter().any(|h| h == *p) && !LEXICON[i].contains(p))
            .map(|p| p.to_string())
            .collect();
        need(self.count + new.len() <= l.get("phrases"))?;
        self.count += new.len();
        let added = new.len();
        let list = self.custom.get_mut(act).ok_or_else(c::bad)?;
        list.extend(new);
        let mut all: Vec<&str> = LEXICON[i].iter().copied().chain(list.iter().map(String::as_str)).collect();
        all.sort();
        Ok(
            obj! { "act" => act, "added" => added, "phrases" => Value::Array(all.into_iter().map(Value::from).collect()) },
        )
    }

    fn op_parse(&self, args: &Value) -> Result<Value> {
        keys(args, &["text"], &["speaker", "listener", "context"])?;
        let s = text_bound(c::get(args, "text")?, 500)?;
        let none = Value::Object(Vec::new());
        let ctx = opt(args, "context").unwrap_or(&none);
        keys(ctx, &[], &["witnessed", "failed"])?;
        let (witnessed, failed) = (word_list(ctx, "witnessed")?, word_list(ctx, "failed")?);
        let parts = split(s);
        need(!parts.is_empty())?;
        let clauses = parts
            .iter()
            .take(MAX_CLAUSES)
            .enumerate()
            .map(|(i, (t, term))| {
                let (act, sub, extra) = self.classify(&words(t), *term, &witnessed, &failed);
                clause_view(i, t, act, sub, extra)
            })
            .collect();
        Ok(obj! {
            "clauses" => Value::Array(clauses), "listener" => opt_str(&opt_id(args, "listener")?),
            "overflow" => parts.len().saturating_sub(MAX_CLAUSES), "speaker" => opt_str(&opt_id(args, "speaker")?),
        })
    }

    fn op_validate(&self, args: &Value) -> Result<Value> {
        keys(args, &["text", "clauses"], &[])?;
        let s = text_bound(c::get(args, "text")?, 500)?;
        let rows = c::get(args, "clauses")?.as_array().ok_or_else(c::bad)?;
        need((1..=MAX_CLAUSES).contains(&rows.len()))?;
        let mut pos = 0;
        let mut out = Vec::new();
        for (i, cl) in rows.iter().enumerate() {
            keys(cl, &["text", "act", "subtext"], &[])?;
            let t = text_bound(c::get(cl, "text")?, 500)?;
            let act = act_of(c::get(cl, "act")?)?;
            let sub = subtext_of(c::get(cl, "subtext")?)?;
            let at = s[pos..].find(t).ok_or_else(c::bad)?;
            pos += at + t.len();
            out.push(clause_view(i, t, act, sub, Vec::new()));
        }
        Ok(obj! { "clauses" => Value::Array(out), "valid" => true })
    }

    // ------------------------------------------------------------ agenda

    fn op_agenda(&self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["clauses"], &["motive", "own"])?;
        let rows = c::get(args, "clauses")?.as_array().ok_or_else(c::bad)?;
        need(rows.len() <= MAX_CLAUSES)?;
        let mut cls: Vec<(u64, &str, &str)> = Vec::new();
        for cl in rows {
            keys(cl, &["index", "act", "subtext"], &[])?;
            let i = uint_bound(c::get(cl, "index")?, 0, MAX_CLAUSES as u64 - 1)?;
            cls.push((i, act_of(c::get(cl, "act")?)?, subtext_of(c::get(cl, "subtext")?)?));
        }
        need(cls.iter().map(|x| x.0).collect::<BTreeSet<_>>().len() == cls.len())?;
        cls.sort();
        let motive = match opt(args, "motive") {
            None => None,
            Some(mo) => {
                keys(mo, &["id", "score"], &[])?;
                Some((id_of(mo, "id")?, uint_bound(c::get(mo, "score")?, 0, 100)?))
            }
        };
        let own_rows = match opt(args, "own") {
            None => &[][..],
            Some(v) => v.as_array().ok_or_else(c::bad)?,
        };
        need(own_rows.len() <= l.get("own"))?;
        let mut own: Vec<(String, &str, u64)> = Vec::new();
        for o in own_rows {
            keys(o, &["id", "kind", "priority"], &[])?;
            let k = ident(c::get(o, "kind")?)?;
            let k = OWN_KINDS.iter().copied().find(|x| *x == k).ok_or_else(c::bad)?;
            own.push((id_of(o, "id")?, k, uint_bound(c::get(o, "priority")?, 0, 100)?));
        }
        need(own.iter().map(|o| &o.0).collect::<BTreeSet<_>>().len() == own.len())?;

        let step = |kind: &str, what: &str, clause: Option<u64>, own: Option<&str>| {
            obj! { "clause" => clause, "kind" => kind, "own" => own, "what" => what }
        };
        let mut steps = Vec::new();
        let mut withheld: Vec<u64> = Vec::new();
        if let Some((id, score)) = &motive {
            if *score >= 90 {
                steps.push(step("own", "urgent", None, Some(id)));
                let mut deferred = Vec::new();
                for (i, act, sub) in &cls {
                    if let Some(u) = update_of(act) {
                        steps.push(step("update", u, Some(*i), None));
                    }
                    if owes_reply(act, sub) {
                        deferred.push(Value::from(*i));
                    }
                }
                return Ok(obj! {
                    "deferred" => Value::Array(deferred), "mode" => "interrupt_override",
                    "steps" => Value::Array(steps), "withheld" => Value::Array(Vec::new()),
                });
            }
        }
        let top = own.iter().filter(|o| o.2 >= 60).min_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        let gate = top.is_some_and(|t| t.1 == "grievance" && t.2 >= 80);
        let mut answered = false;
        for (i, act, sub) in &cls {
            if let Some(u) = update_of(act) {
                steps.push(step("update", u, Some(*i), None));
            }
            if !owes_reply(act, sub) {
                continue;
            }
            if gate && GATED.contains(act) && *sub != "urgent" {
                withheld.push(*i);
            } else {
                steps.push(step("answer", act, Some(*i), None));
                answered = true;
            }
        }
        if let Some(t) = top {
            steps.push(step("own", t.1, None, Some(&t.0)));
        }
        let mode = if !withheld.is_empty() {
            "conditional_gate"
        } else if top.is_some() && answered {
            "answer_then_counter_ask"
        } else if top.is_some() {
            "raise_own"
        } else {
            "respond"
        };
        Ok(obj! {
            "deferred" => Value::Array(Vec::new()), "mode" => mode, "steps" => Value::Array(steps),
            "withheld" => Value::Array(withheld.into_iter().map(Value::from).collect()),
        })
    }

    // ------------------------------------------------------------ threads

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn stack_view(&self, b: &str) -> Value {
        Value::Array(self.stacks.get(b).map(|s| s.iter().rev().map(Thread::view).collect()).unwrap_or_default())
    }

    fn op_open(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "id", "topic", "speaker"], &["question"])?;
        let b = id_of(args, "being")?;
        let id = id_of(args, "id")?;
        let topic = id_of(args, "topic")?;
        let speaker = id_of(args, "speaker")?;
        let question = opt(args, "question").map(|v| text_bound(v, 160).map(str::to_string)).transpose()?;
        need(self.stacks.contains_key(&b) || self.stacks.len() < l.get("beings"))?;
        need(self.stacks.get(&b).is_none_or(|s| s.iter().all(|t| t.id != id)))?;
        let clock = self.clock;
        let st = self.stacks.entry(b.clone()).or_default();
        let dropped = if st.len() >= l.get("threads") { Some(st.remove(0).id) } else { None };
        for t in st.iter_mut().filter(|t| t.status == "active") {
            t.status = "waiting";
        }
        st.push(Thread {
            id,
            topic,
            speaker,
            question,
            status: "active",
            opened_at: clock,
            interrupted_at: None,
            cause: None,
        });
        Ok(obj! { "being" => b.as_str(), "dropped" => opt_str(&dropped), "threads" => self.stack_view(&b) })
    }

    fn op_interrupt(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "cause"], &[])?;
        let b = id_of(args, "being")?;
        let cause = id_of(args, "cause")?;
        let clock = self.clock;
        let t = self.stacks.get_mut(&b).and_then(|s| s.last_mut()).ok_or_else(c::bad)?;
        need(t.status == "active")?;
        t.status = "interrupted";
        t.cause = Some(cause);
        t.interrupted_at = Some(clock);
        Ok(obj! { "being" => b, "thread" => t.view() })
    }

    fn op_resume(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["ticks_per_hour"])?;
        let b = id_of(args, "being")?;
        let tph = opt_uint(args, "ticks_per_hour", 1, 3600, 1)?;
        let clock = self.clock;
        let t = self.stacks.get_mut(&b).and_then(|s| s.last_mut()).ok_or_else(c::bad)?;
        need(t.status == "interrupted")?;
        let (_, since) = time_label(t.interrupted_at.unwrap_or(clock), clock, tph, false)?;
        let resumed = t.view();
        let unanswered = opt_str(&t.question);
        t.status = "active";
        t.cause = None;
        t.interrupted_at = None;
        Ok(obj! { "being" => b, "since" => since, "thread" => resumed, "unanswered" => unanswered })
    }

    fn op_close(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "id"], &[])?;
        let b = id_of(args, "being")?;
        let id = id_of(args, "id")?;
        let st = self.stacks.get_mut(&b).ok_or_else(c::bad)?;
        let at = st.iter().position(|t| t.id == id).ok_or_else(c::bad)?;
        let was_top = at + 1 == st.len();
        st.remove(at);
        if let Some(t) = st.last_mut().filter(|t| was_top && t.status == "waiting") {
            t.status = "active";
        }
        let top = st.last().map(|t| t.id.clone());
        Ok(obj! { "being" => b, "closed" => id, "top" => opt_str(&top) })
    }

    fn op_threads(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        Ok(obj! { "being" => b.as_str(), "threads" => self.stack_view(&b) })
    }

    // ------------------------------------------------------------ stance

    fn op_stance(&self, args: &Value) -> Result<Value> {
        keys(
            args,
            &["trust"],
            &["affection", "resentment", "confidential", "bystanders", "urgency", "deadline_hours"],
        )?;
        let trust = match c::get(args, "trust")? {
            Value::Number(s) => s.parse::<i64>().ok().filter(|n| (-100..=100).contains(n)).ok_or_else(c::bad)?,
            _ => return Err(c::bad()),
        };
        let aff = opt_uint(args, "affection", 0, 100, 0)? as i64;
        let res = opt_uint(args, "resentment", 0, 100, 0)? as i64;
        let conf = opt_bool(args, "confidential", false)?;
        let urg = opt_uint(args, "urgency", 0, 100, 0)?;
        let dl = opt(args, "deadline_hours").map(|v| uint_bound(v, 0, 1_000_000)).transpose()?;
        let rows = match opt(args, "bystanders") {
            None => &[][..],
            Some(v) => v.as_array().ok_or_else(c::bad)?,
        };
        need(rows.len() <= 16)?;
        let mut seen = BTreeSet::new();
        let mut untrusted = BTreeSet::new();
        for x in rows {
            keys(x, &["being"], &["trusted", "earshot"])?;
            let w = id_of(x, "being")?;
            need(seen.insert(w.clone()))?;
            if opt_bool(x, "earshot", true)? && !opt_bool(x, "trusted", false)? {
                untrusted.insert(w);
            }
        }
        let score = trust + aff / 2 - res;
        let coop = match score {
            s if s >= 70 => "eager",
            s if s >= 20 => "cooperative",
            s if s >= -10 => "guarded",
            s if s >= -40 => "reluctant",
            s if s >= -70 => "deflecting",
            _ => "hostile_refusal",
        };
        let privacy = if !conf || untrusted.is_empty() {
            "open"
        } else if trust >= 60 {
            "whispered"
        } else {
            "deflect_due_to_bystanders"
        };
        let mut n: u64 = match urg {
            u if u < 30 => 4,
            u if u < 60 => 3,
            u if u < 85 => 2,
            _ => 1,
        };
        if dl.is_some_and(|d| d <= 1) {
            n = 1;
        }
        let verbosity = match n {
            4 => "full",
            1 => "brief",
            _ => "normal",
        };
        Ok(obj! {
            "cooperativeness" => coop, "max_sentences" => n, "privacy" => privacy, "score" => score,
            "untrusted_in_earshot" => Value::Array(untrusted.into_iter().map(Value::from).collect()),
            "verbosity" => verbosity,
        })
    }
}

/// Thread status words.
pub const THREAD_STATUS: [&str; 3] = ["active", "waiting", "interrupted"];
persist!(Thread {
    cause, id, interrupted_at [0, MAX_AT], opened_at [0, MAX_AT], question, speaker, status in THREAD_STATUS, topic,
});
persist!(rest Pragmatics { clock [0, MAX_AT], custom, stacks });
durable!(Pragmatics, "pragmatics", LIMITS);

fn phrase_ok(s: &str) -> bool {
    text_bound(&Value::from(s), 64).is_ok() && {
        let ws = words(s);
        (1..=4).contains(&ws.len()) && ws.join(" ") == s
    }
}

impl Pragmatics {
    /// Invariants a save must meet (the ops never produce anything else); recounts phrases.
    fn settle_state(&mut self) -> Result<()> {
        ok(self.custom.len() == PRECEDENCE.len())?;
        let mut count = 0;
        for (i, act) in PRECEDENCE.iter().enumerate() {
            let list = self.custom.get(*act).ok_or_else(c::bad)?;
            ok(list.iter().collect::<BTreeSet<_>>().len() == list.len())?;
            ok(list.iter().all(|p| phrase_ok(p) && !LEXICON[i].contains(&p.as_str())))?;
            count += list.len();
        }
        within::<Self>("phrases", count)?;
        self.count = count;
        within::<Self>("beings", self.stacks.len())?;
        for (b, st) in &self.stacks {
            ok(is_ident(b))?;
            within::<Self>("threads", st.len())?;
            ok(st.iter().map(|t| &t.id).collect::<BTreeSet<_>>().len() == st.len())?;
            ok(st.iter().zip(st.iter().skip(1)).all(|(a, b)| a.opened_at <= b.opened_at))?;
            for (i, t) in st.iter().enumerate() {
                ok(is_ident(&t.id) && is_ident(&t.topic) && is_ident(&t.speaker) && t.opened_at <= self.clock)?;
                ok(t.question.as_deref().is_none_or(|q| is_text(q, 160)))?;
                let interrupted = t.status == "interrupted";
                ok(t.cause.is_some() == interrupted && t.interrupted_at.is_some() == interrupted)?;
                ok(t.cause.as_deref().is_none_or(is_ident) && t.interrupted_at.is_none_or(|x| x <= self.clock))?;
                ok(t.status != "active" || i + 1 == st.len())?;
            }
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Pragmatics::new().workflow(req)
}
