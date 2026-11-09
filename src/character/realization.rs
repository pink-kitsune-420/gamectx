//! Intent packets and checked realization (pass 5, stage 3).
//!
//! **P1. Intent packets.** The runtime's decision becomes a compact, closed-vocabulary packet.
//! It holds act or move, mechanism and effect, addressee, allowed facts with framings
//! (`state`, `hedge`, `cover`), idioms above threshold, tone and pacing hints, sentence budget,
//! and forbidden key words from the epistemic gate. The packet is built under a token budget
//! sized for local 1B–8B models; the salience ranker determines what is dropped when the
//! budget is tight.
//!
//! **P2. Checked realization.** The model returns a line plus a self-tag from a closed list (the move
//! it believes it made). The host runs 5 checks:
//! 1. Epistemic gate: zero leaks of forbidden key words or masked facts.
//! 2. Length and sentence budget: sentence count <= budget, characters <= 300.
//! 3. Allowed facts: unallowed facts cannot be named, cover stories must not reveal the masked claim,
//!    and hedged facts require hedge markers.
//! 4. Idioms in packet: any known community idiom used must be present in the packet.
//! 5. Self-tag matches move: model's reported tag must match the intended move.
//!
//! A failing line is retried once with the reason. After that, the host falls back to an authored
//! template for the move, so a scene never stalls on a bad generation.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend); model replies are untrusted input to `check` and `realize`.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::realization::Realization;
//!
//! let mut r = Realization::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "template", "args": {"id": "t1", "move": "callback", "text": "Like we said at the docks."}},
//!   {"op": "packet", "args": {"move": "callback", "addressee": "bo", "sentence_budget": 2}},
//!   {"op": "check", "args": {
//!     "packet": {"move": "callback", "addressee": "bo", "sentence_budget": 2},
//!     "reply": {"line": "Like we said at the docks.", "tag": "callback"}
//!   }}
//! ]}"#, 4096)?;
//! let out = r.workflow(&req)?.to_json();
//! assert!(out.contains(r#""ok":true"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, text_bound, MAX_AT};
use crate::character::cueing::words;
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::json::parse;
use crate::data::persist::{self, durable, is_ident, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "templates", default: 256, ceiling: 1_000_000 },
    Limit { name: "facts", default: 64, ceiling: 4096 },
    Limit { name: "idioms", default: 32, ceiling: 1024 },
    Limit { name: "forbidden", default: 64, ceiling: 4096 },
];

pub const FRAMINGS: [&str; 3] = ["state", "hedge", "cover"];
pub const HEDGE_MARKERS: [&str; 10] =
    ["maybe", "perhaps", "might", "suppose", "think", "heard", "seem", "could", "wonder", "guess"];
pub const DEFAULT_FALLBACK: &str = "...";

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
    match opt(args, k) {
        Some(v) => Ok(Some(ident(v)?.to_string())),
        None => Ok(None),
    }
}

/// Counts sentences deterministically by punctuation boundaries.
pub fn count_sentences(s: &str) -> usize {
    let (mut runs, mut prev) = (0, false);
    for ch in s.chars() {
        let end = matches!(ch, '.' | '!' | '?');
        if end && !prev {
            runs += 1;
        }
        prev = end;
    }
    let tail = s.trim_end_matches(['"', '\'', ')', '\u{201d}', '\u{2019}', ' ']);
    if tail.chars().last().is_some_and(|c| !matches!(c, '.' | '!' | '?')) {
        runs += 1;
    }
    runs
}

/// Estimates tokens for a text string.
pub fn estimate_tokens(text: &str) -> usize {
    let ws = text.split_whitespace().count();
    ws.max(1) + text.len() / 32
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub id: String,
    pub r#move: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Realization {
    pub limits: Limits,
    pub templates: BTreeMap<String, Template>,
    pub idioms: BTreeMap<String, String>,
    pub clock: u64,
}

impl Default for Realization {
    fn default() -> Self {
        Self { limits: Limits::defaults(LIMITS), templates: BTreeMap::new(), idioms: BTreeMap::new(), clock: 0 }
    }
}

impl Realization {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "template" => self.op_template(args, l),
            "templates" => self.op_templates(args, l),
            "idiom" => self.op_idiom(args, l),
            "packet" => self.op_packet(args, l),
            "prompt" => self.op_prompt(args),
            "check" => self.op_check(args),
            "realize" => self.op_realize(args),
            "show" => self.op_show(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        })?;
        self.limits = lim;
        Ok(out)
    }

    fn op_template(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "move", "text"], &[])?;
        let tid = id_of(args, "id")?;
        let mv = id_of(args, "move")?;
        let txt = text_bound(c::get(args, "text")?, 200)?.to_string();
        need(!txt.trim().is_empty())?;
        if !self.templates.contains_key(&tid) {
            need(self.templates.len() < l.get("templates"))?;
        }
        let t = Template { id: tid.clone(), r#move: mv.clone(), text: txt.clone() };
        self.templates.insert(tid.clone(), t);
        Ok(obj! { "id" => tid, "move" => mv, "text" => txt })
    }

    fn op_templates(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["list"], &[])?;
        let list = c::list(c::get(args, "list")?)?;
        for item in list {
            self.op_template(item, l)?;
        }
        Ok(obj! { "count" => list.len() as u64 })
    }

    fn op_idiom(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "phrase"], &[])?;
        let iid = id_of(args, "id")?;
        let ph = text_bound(c::get(args, "phrase")?, 80)?.trim().to_ascii_lowercase();
        let w_count = words(&ph).len();
        need((1..=6).contains(&w_count))?;
        if !self.idioms.contains_key(&iid) {
            need(self.idioms.len() < l.get("idioms"))?;
        }
        self.idioms.insert(iid.clone(), ph.clone());
        Ok(obj! { "id" => iid, "phrase" => ph })
    }

    fn op_packet(&self, args: &Value, l: &Limits) -> Result<Value> {
        keys(
            args,
            &["move", "addressee"],
            &[
                "speaker",
                "mechanism",
                "effect",
                "tone",
                "pacing",
                "sentence_budget",
                "token_budget",
                "facts",
                "idioms",
                "forbidden",
            ],
        )?;
        let mv = id_of(args, "move")?;
        let addr = id_of(args, "addressee")?;
        let spk = opt_id(args, "speaker")?;
        let mech = opt_id(args, "mechanism")?;
        let eff = opt_id(args, "effect")?;
        let tone = opt_id(args, "tone")?;
        let pacing = opt_id(args, "pacing")?;
        let sb = opt_uint(args, "sentence_budget", 1, 8, 2)?;
        let tb = opt_uint(args, "token_budget", 20, 2000, 200)?;

        let mut idioms_set = BTreeSet::new();
        if let Some(raw_i) = opt(args, "idioms") {
            let list = c::list(raw_i)?;
            need(list.len() <= l.get("idioms"))?;
            for item in list {
                let s = text_bound(item, 80)?.trim().to_ascii_lowercase();
                idioms_set.insert(s);
            }
        }
        let idioms_list: Vec<Value> = idioms_set.iter().map(|s| Value::from(s.as_str())).collect();

        let mut forbidden_set = BTreeSet::new();
        if let Some(raw_f) = opt(args, "forbidden") {
            let list = c::list(raw_f)?;
            need(list.len() <= l.get("forbidden"))?;
            for item in list {
                let s = text_bound(item, 64)?.trim().to_ascii_lowercase();
                forbidden_set.insert(s);
            }
        }
        let forbidden_list: Vec<Value> = forbidden_set.iter().map(|s| Value::from(s.as_str())).collect();

        struct FactItem {
            id: String,
            claim: String,
            framing: String,
            keys: Vec<String>,
            lie: Option<String>,
            share: u64,
        }

        let mut facts_parsed = Vec::new();
        if let Some(raw_facts) = opt(args, "facts") {
            let list = c::list(raw_facts)?;
            need(list.len() <= l.get("facts"))?;
            for f in list {
                keys(f, &["id", "claim"], &["framing", "keys", "lie", "share"])?;
                let fid = id_of(f, "id")?;
                let claim = text_bound(c::get(f, "claim")?, 160)?.to_string();
                let framing = match opt(f, "framing") {
                    Some(v) => ident(v)?,
                    None => "state",
                };
                need(FRAMINGS.contains(&framing))?;
                let lie = if framing == "cover" { Some(text_bound(c::get(f, "lie")?, 160)?.to_string()) } else { None };
                let keys = if let Some(raw_k) = opt(f, "keys") {
                    let klist = c::list(raw_k)?;
                    need(klist.len() <= 8)?;
                    let mut ks = Vec::new();
                    for k in klist {
                        let w = text_bound(k, 40)?.trim().to_ascii_lowercase();
                        ks.push(w);
                    }
                    ks
                } else {
                    let mut ws: Vec<String> = words(&claim).into_iter().collect();
                    ws.truncate(8);
                    ws
                };
                let share = opt_uint(f, "share", 0, 1_000_000, 0)?;
                facts_parsed.push(FactItem { id: fid, claim, framing: framing.to_string(), keys, lie, share });
            }
        }

        let mut base_tokens = 25;
        if spk.is_some() {
            base_tokens += 2;
        }
        base_tokens += 3; // move + addressee
        if mech.is_some() || eff.is_some() {
            base_tokens += 4;
        }
        if tone.is_some() || pacing.is_some() {
            base_tokens += 3;
        }
        base_tokens += idioms_list.len() * 2;
        base_tokens += forbidden_list.len();

        // Sort by (-share, id)
        facts_parsed.sort_by(|a, b| b.share.cmp(&a.share).then_with(|| a.id.cmp(&b.id)));

        let mut kept_facts = Vec::new();
        let mut dropped_facts = Vec::new();
        let mut current_tokens = base_tokens;

        for f in facts_parsed {
            let cost = estimate_tokens(&f.claim) + f.lie.as_ref().map(|l| estimate_tokens(l)).unwrap_or(0) + 4;
            if current_tokens + cost <= (tb as usize) || kept_facts.is_empty() {
                current_tokens += cost;
                kept_facts.push(f);
            } else {
                dropped_facts.push(f.id);
            }
        }

        kept_facts.sort_by(|a, b| a.id.cmp(&b.id));
        dropped_facts.sort();

        let facts_val: Vec<Value> = kept_facts
            .into_iter()
            .map(|f| {
                let mut obj_map = vec![
                    ("claim", Value::from(f.claim)),
                    ("framing", Value::from(f.framing)),
                    ("id", Value::from(f.id)),
                    ("keys", Value::Array(f.keys.into_iter().map(Value::from).collect())),
                    ("share", Value::from(f.share)),
                ];
                if let Some(lie) = f.lie {
                    obj_map.push(("lie", Value::from(lie)));
                } else {
                    obj_map.push(("lie", Value::Null));
                }
                obj_map.sort_by_key(|(k, _)| *k);
                Value::Object(obj_map.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
            })
            .collect();

        let dropped_val: Vec<Value> = dropped_facts.into_iter().map(Value::from).collect();

        Ok(obj! {
            "addressee" => addr,
            "dropped_facts" => Value::Array(dropped_val),
            "effect" => eff.map(Value::from).unwrap_or(Value::Null),
            "estimated_tokens" => current_tokens as u64,
            "facts" => Value::Array(facts_val),
            "forbidden" => Value::Array(forbidden_list),
            "idioms" => Value::Array(idioms_list),
            "mechanism" => mech.map(Value::from).unwrap_or(Value::Null),
            "move" => mv,
            "pacing" => pacing.map(Value::from).unwrap_or(Value::Null),
            "sentence_budget" => sb,
            "speaker" => spk.map(Value::from).unwrap_or(Value::Null),
            "token_budget" => tb,
            "tone" => tone.map(Value::from).unwrap_or(Value::Null),
        })
    }

    fn op_prompt(&self, args: &Value) -> Result<Value> {
        keys(args, &["packet"], &[])?;
        let pkt = c::get(args, "packet")?;
        let spk = opt(pkt, "speaker").and_then(|v| c::text(v).ok()).unwrap_or("You");
        let addr = c::text(c::get(pkt, "addressee")?)?;
        let mv = c::text(c::get(pkt, "move")?)?;
        let sb = opt_uint(pkt, "sentence_budget", 1, 8, 2)?;

        let mut lines = Vec::new();
        lines.push(format!("{spk} addressing {addr}."));
        lines.push(format!("Move: {mv}."));

        let mech = opt(pkt, "mechanism").and_then(|v| c::text(v).ok());
        let eff = opt(pkt, "effect").and_then(|v| c::text(v).ok());
        if mech.is_some() || eff.is_some() {
            lines.push(format!("Mechanism: {} ({}).", mech.unwrap_or("none"), eff.unwrap_or("none")));
        }

        let tone = opt(pkt, "tone").and_then(|v| c::text(v).ok());
        let pacing = opt(pkt, "pacing").and_then(|v| c::text(v).ok());
        if tone.is_some() || pacing.is_some() {
            lines.push(format!("Tone: {}, Pacing: {}.", tone.unwrap_or("neutral"), pacing.unwrap_or("measured")));
        }

        lines.push(format!("Sentence budget: at most {sb} sentences."));

        if let Ok(raw_facts) = c::get(pkt, "facts").and_then(c::list) {
            if !raw_facts.is_empty() {
                lines.push("Allowed facts:".to_string());
                for f in raw_facts {
                    let fr = opt(f, "framing").and_then(|v| c::text(v).ok()).unwrap_or("state");
                    let claim = c::text(c::get(f, "claim")?)?;
                    let fid = c::text(c::get(f, "id")?)?;
                    if fr == "cover" {
                        let lie = opt(f, "lie").and_then(|v| c::text(v).ok()).unwrap_or("");
                        lines.push(format!("  - (cover) Say: \"{lie}\" (do NOT mention actual fact: {fid})"));
                    } else if fr == "hedge" {
                        lines.push(format!("  - (hedge) {claim} (use uncertain wording: perhaps, maybe, think)"));
                    } else {
                        lines.push(format!("  - {claim}"));
                    }
                }
            }
        }

        if let Ok(raw_idioms) = c::get(pkt, "idioms").and_then(c::list) {
            if !raw_idioms.is_empty() {
                let idm_strs: Vec<&str> = raw_idioms.iter().filter_map(|v| c::text(v).ok()).collect();
                lines.push(format!("Available idioms: {}.", idm_strs.join(", ")));
            }
        }

        if let Ok(raw_forb) = c::get(pkt, "forbidden").and_then(c::list) {
            if !raw_forb.is_empty() {
                let forb_strs: Vec<&str> = raw_forb.iter().filter_map(|v| c::text(v).ok()).collect();
                lines.push(format!("FORBIDDEN words: {}.", forb_strs.join(", ")));
            }
        }

        lines.push(format!(r#"Respond strictly in JSON: {{"line": "...", "tag": "{mv}"}}"#));

        let prompt_text = lines.join("\n");
        let sys_text = "You are an in-world character. Stay in character, respect length and epistemic boundaries, and output JSON with keys 'line' and 'tag'.";
        let schema = obj! {
            "properties" => obj! {
                "line" => obj! { "type" => "string" },
                "tag" => obj! { "type" => "string" },
            },
            "required" => Value::Array(vec![Value::from("line"), Value::from("tag")]),
            "type" => "object",
        };

        Ok(obj! {
            "prompt" => prompt_text,
            "schema" => schema,
            "system" => sys_text,
        })
    }

    fn op_check(&self, args: &Value) -> Result<Value> {
        keys(args, &["packet", "reply"], &["known_idioms", "unallowed_facts"])?;
        let pkt = c::get(args, "packet")?;
        let raw_reply = c::get(args, "reply")?;

        let reply = match raw_reply {
            Value::String(s) => match parse(s.as_str(), 4096) {
                Ok(parsed) => parsed,
                Err(_) => {
                    return Ok(obj! {
                        "line" => "",
                        "ok" => false,
                        "tag" => "",
                        "violations" => Value::Array(vec![obj! { "check" => "format", "reason" => "invalid_json" }]),
                    });
                }
            },
            Value::Object(_) => raw_reply.clone(),
            _ => {
                return Ok(obj! {
                    "line" => "",
                    "ok" => false,
                    "tag" => "",
                    "violations" => Value::Array(vec![obj! { "check" => "format", "reason" => "invalid_reply_type" }]),
                });
            }
        };

        let line_raw = match opt(&reply, "line") {
            Some(Value::String(s)) => s.trim().to_string(),
            _ => {
                return Ok(obj! {
                    "line" => opt(&reply, "line").map(|v| v.to_json()).unwrap_or_default(),
                    "ok" => false,
                    "tag" => opt(&reply, "tag").map(|v| v.to_json()).unwrap_or_default(),
                    "violations" => Value::Array(vec![obj! { "check" => "format", "reason" => "missing_line_or_tag" }]),
                });
            }
        };

        let tag_raw = match opt(&reply, "tag") {
            Some(Value::String(s)) => s.trim().to_string(),
            _ => {
                return Ok(obj! {
                    "line" => line_raw,
                    "ok" => false,
                    "tag" => opt(&reply, "tag").map(|v| v.to_json()).unwrap_or_default(),
                    "violations" => Value::Array(vec![obj! { "check" => "format", "reason" => "missing_line_or_tag" }]),
                });
            }
        };

        let line_words = words(&line_raw);
        let line_lower = line_raw.to_ascii_lowercase();
        let mut violations = Vec::new();

        // 1. Epistemic gate
        if let Ok(forb_list) = c::get(pkt, "forbidden").and_then(c::list) {
            for f in forb_list {
                if let Ok(w) = c::text(f) {
                    let w_low = w.to_ascii_lowercase();
                    if line_words.contains(&w_low) {
                        violations.push(obj! {
                            "check" => "gate",
                            "reason" => "forbidden_word_leaked",
                            "word" => w,
                        });
                    }
                }
            }
        }

        // 2. Length and sentence budget
        let s_count = count_sentences(&line_raw);
        let sb = opt_uint(pkt, "sentence_budget", 1, 8, 2)?;
        if s_count > (sb as usize) {
            violations.push(obj! {
                "budget" => sb,
                "check" => "sentences",
                "count" => s_count as u64,
                "reason" => "sentence_budget_exceeded",
            });
        }
        if line_raw.len() > 300 {
            violations.push(obj! {
                "check" => "length",
                "len" => line_raw.len() as u64,
                "reason" => "character_limit_exceeded",
            });
        }
        if line_raw.is_empty() {
            violations.push(obj! {
                "check" => "length",
                "len" => 0,
                "reason" => "empty_line",
            });
        }

        // 3. Allowed facts
        if let Ok(facts_list) = c::get(pkt, "facts").and_then(c::list) {
            for f in facts_list {
                let fid = opt(f, "id").and_then(|v| c::text(v).ok()).unwrap_or("");
                let framing = opt(f, "framing").and_then(|v| c::text(v).ok()).unwrap_or("state");
                if framing == "cover" {
                    let lie_str = opt(f, "lie").and_then(|v| c::text(v).ok()).unwrap_or("");
                    let lie_w = words(lie_str);
                    if let Ok(klist) = c::get(f, "keys").and_then(c::list) {
                        for k in klist {
                            if let Ok(kw) = c::text(k) {
                                let kw_low = kw.to_ascii_lowercase();
                                if line_words.contains(&kw_low) && !lie_w.contains(&kw_low) {
                                    violations.push(obj! {
                                        "check" => "allowed_facts",
                                        "fact" => fid,
                                        "reason" => "masked_fact_revealed",
                                        "word" => kw,
                                    });
                                }
                            }
                        }
                    }
                } else if framing == "hedge" {
                    if let Ok(klist) = c::get(f, "keys").and_then(c::list) {
                        let mut mentions = false;
                        for k in klist {
                            if let Ok(kw) = c::text(k) {
                                if line_words.contains(&kw.to_ascii_lowercase()) {
                                    mentions = true;
                                    break;
                                }
                            }
                        }
                        if mentions {
                            let has_hedge = HEDGE_MARKERS.iter().any(|h| line_words.contains(*h));
                            if !has_hedge {
                                violations.push(obj! {
                                    "check" => "allowed_facts",
                                    "fact" => fid,
                                    "reason" => "fact_not_hedged",
                                });
                            }
                        }
                    }
                }
            }
        }

        if let Some(raw_uf) = opt(args, "unallowed_facts") {
            if let Ok(uf_list) = c::list(raw_uf) {
                for uf in uf_list {
                    let ufid = opt(uf, "id").and_then(|v| c::text(v).ok()).unwrap_or("unallowed");
                    if let Ok(klist) = c::get(uf, "keys").and_then(c::list) {
                        for k in klist {
                            if let Ok(kw) = c::text(k) {
                                if line_words.contains(&kw.to_ascii_lowercase()) {
                                    violations.push(obj! {
                                        "check" => "allowed_facts",
                                        "fact" => ufid,
                                        "reason" => "unallowed_fact_named",
                                        "word" => kw,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Idioms in packet
        let mut all_known_idioms = self.idioms.values().cloned().collect::<Vec<_>>();
        if let Some(raw_ki) = opt(args, "known_idioms") {
            if let Ok(ki_list) = c::list(raw_ki) {
                for ki in ki_list {
                    if let Ok(ph) = c::text(ki) {
                        all_known_idioms.push(ph.to_ascii_lowercase());
                    }
                }
            }
        }

        let mut packet_idioms = BTreeSet::new();
        if let Ok(p_idms) = c::get(pkt, "idioms").and_then(c::list) {
            for item in p_idms {
                if let Ok(s) = c::text(item) {
                    packet_idioms.insert(s.to_ascii_lowercase());
                }
            }
        }

        for phrase in &all_known_idioms {
            if line_lower.contains(phrase) && !packet_idioms.contains(phrase) {
                violations.push(obj! {
                    "check" => "idioms",
                    "idiom" => phrase.as_str(),
                    "reason" => "idiom_not_in_packet",
                });
            }
        }

        // 5. Self-tag matches move
        let expected_mv = id_of(pkt, "move")?;
        if tag_raw != expected_mv {
            violations.push(obj! {
                "actual" => tag_raw.clone(),
                "check" => "tag",
                "expected" => expected_mv,
                "reason" => "tag_mismatch",
            });
        }

        let ok_val = violations.is_empty();
        Ok(obj! {
            "line" => line_raw,
            "ok" => ok_val,
            "tag" => tag_raw,
            "violations" => Value::Array(violations),
        })
    }

    fn op_realize(&self, args: &Value) -> Result<Value> {
        keys(args, &["packet", "candidate"], &["retry", "unallowed_facts", "known_idioms", "templates"])?;
        let pkt = c::get(args, "packet")?;
        let cand = c::get(args, "candidate")?;
        let retry_val = opt(args, "retry");
        let uf = opt(args, "unallowed_facts");
        let ki = opt(args, "known_idioms");

        let mut check_args = vec![("packet", pkt.clone()), ("reply", cand.clone())];
        if let Some(u) = uf {
            check_args.push(("unallowed_facts", u.clone()));
        }
        if let Some(k) = ki {
            check_args.push(("known_idioms", k.clone()));
        }
        let check1 =
            self.op_check(&Value::Object(check_args.into_iter().map(|(k, v)| (k.to_string(), v)).collect()))?;

        if c::boolean(c::get(&check1, "ok")?)? {
            return Ok(obj! {
                "line" => c::get(&check1, "line")?.clone(),
                "ok" => true,
                "status" => "accepted",
                "tag" => c::get(&check1, "tag")?.clone(),
                "tries" => 1,
            });
        }

        let first_v = c::list(c::get(&check1, "violations")?)?;
        let first_r = &first_v[0];
        let first_reason = c::text(c::get(first_r, "reason")?)?;
        let first_check = c::text(c::get(first_r, "check")?)?;

        if let Some(r_val) = retry_val {
            let mut retry_args = vec![("packet", pkt.clone()), ("reply", r_val.clone())];
            if let Some(u) = uf {
                retry_args.push(("unallowed_facts", u.clone()));
            }
            if let Some(k) = ki {
                retry_args.push(("known_idioms", k.clone()));
            }
            let check2 =
                self.op_check(&Value::Object(retry_args.into_iter().map(|(k, v)| (k.to_string(), v)).collect()))?;

            if c::boolean(c::get(&check2, "ok")?)? {
                return Ok(obj! {
                    "line" => c::get(&check2, "line")?.clone(),
                    "ok" => true,
                    "status" => "accepted",
                    "tag" => c::get(&check2, "tag")?.clone(),
                    "tries" => 2,
                });
            }

            let retry_v = c::list(c::get(&check2, "violations")?)?;
            let retry_r = &retry_v[0];
            let retry_reason = c::text(c::get(retry_r, "reason")?)?;

            let mv = id_of(pkt, "move")?;
            let addr = id_of(pkt, "addressee")?;
            let extra_templates = opt(args, "templates").and_then(|v| c::list(v).ok());
            let fb_line = self.find_fallback(&mv, &addr, extra_templates);

            return Ok(obj! {
                "line" => fb_line,
                "ok" => true,
                "reason" => retry_reason,
                "status" => "fallback",
                "tag" => mv,
                "tries" => 2,
                "violations" => Value::Array(retry_v.to_vec()),
            });
        }

        let mv = id_of(pkt, "move")?;
        let sb = opt_uint(pkt, "sentence_budget", 1, 8, 2)?;
        let retry_prompt = format!(
            "Your previous reply was rejected ({first_check}: {first_reason}). Please reply strictly adhering to the move '{mv}' under {sb} sentences."
        );

        Ok(obj! {
            "check" => first_check,
            "ok" => false,
            "reason" => first_reason,
            "retry_prompt" => retry_prompt,
            "status" => "retry",
            "tries" => 1,
            "violations" => Value::Array(first_v.to_vec()),
        })
    }

    fn find_fallback(&self, mv: &str, addressee: &str, extra_templates: Option<&[Value]>) -> String {
        if let Some(list) = extra_templates {
            for t in list {
                if let (Ok(m), Ok(txt)) = (c::get(t, "move").and_then(c::text), c::get(t, "text").and_then(c::text)) {
                    if m == mv && !txt.is_empty() {
                        return txt.replace("{addressee}", addressee).replace("{listener}", addressee);
                    }
                }
            }
        }

        for t in self.templates.values() {
            if t.r#move == mv && !t.text.is_empty() {
                return t.text.replace("{addressee}", addressee).replace("{listener}", addressee);
            }
        }

        DEFAULT_FALLBACK.to_string()
    }

    fn op_show(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &["id"])?;
        if let Some(tid) = opt_id(args, "id")? {
            let t = self.templates.get(&tid).ok_or_else(c::bad)?;
            return Ok(obj! { "id" => t.id.as_str(), "move" => t.r#move.as_str(), "text" => t.text.as_str() });
        }

        let mut idms: Vec<Value> = self.idioms.values().map(|s| Value::from(s.as_str())).collect();
        idms.sort_by(|a, b| c::text(a).unwrap().cmp(c::text(b).unwrap()));

        let tmpls: Vec<Value> = self
            .templates
            .values()
            .map(|t| obj! { "id" => t.id.as_str(), "move" => t.r#move.as_str(), "text" => t.text.as_str() })
            .collect();

        Ok(obj! {
            "idioms" => Value::Array(idms),
            "templates" => Value::Array(tmpls),
        })
    }
}

persist!(Template { id, r#move, text });
persist!(rest Realization { clock [0, MAX_AT], idioms, templates });
durable!(Realization, "realization", LIMITS);

impl Realization {
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("templates", self.templates.len())?;
        within::<Self>("idioms", self.idioms.len())?;
        for (id, t) in &self.templates {
            ok(is_ident(id) && is_ident(&t.r#move) && !t.text.trim().is_empty() && t.text.len() <= 200)?;
        }
        for (id, ph) in &self.idioms {
            ok(is_ident(id) && !ph.trim().is_empty() && ph.len() <= 80)?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

/// Runs a fresh [`Realization`] on `{ops, limits?}` (the `realization` local kind).
pub fn workflow(request: &Value) -> Result<Value> {
    Realization::new().workflow(request)
}
