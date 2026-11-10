//! Concept lexicons, deterministic normalization, negation scoping, and parser coverage
//! (pass 5, stage 4, design `Q`, local kind `concept`).
use crate::character::limits::{Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, exact_fields, Persist};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "concepts", default: 256, ceiling: 65_536 },
    Limit { name: "packs", default: 32, ceiling: 4096 },
    Limit { name: "words", default: 1024, ceiling: 262_144 },
];

const CONTRACTIONS: &[(&str, &str)] = &[
    ("can't", "cannot"),
    ("won't", "will not"),
    ("shan't", "shall not"),
    ("ain't", "is not"),
    ("n't", " not"),
    ("'re", " are"),
    ("'s", " is"),
    ("'d", " would"),
    ("'ll", " will"),
    ("'ve", " have"),
    ("'m", " am"),
];

const NUMBER_WORDS: &[(&str, &str)] = &[
    ("zero", "0"),
    ("one", "1"),
    ("two", "2"),
    ("three", "3"),
    ("four", "4"),
    ("five", "5"),
    ("six", "6"),
    ("seven", "7"),
    ("eight", "8"),
    ("nine", "9"),
    ("ten", "10"),
    ("eleven", "11"),
    ("twelve", "12"),
    ("thirteen", "13"),
    ("fourteen", "14"),
    ("fifteen", "15"),
    ("sixteen", "16"),
    ("seventeen", "17"),
    ("eighteen", "18"),
    ("nineteen", "19"),
    ("twenty", "20"),
    ("thirty", "30"),
    ("forty", "40"),
    ("fifty", "50"),
    ("sixty", "60"),
    ("seventy", "70"),
    ("eighty", "80"),
    ("ninety", "90"),
    ("hundred", "100"),
];

const NEGATION_MARKERS: &[&str] =
    &["not", "never", "no", "neither", "nor", "hardly", "scarcely", "barely", "cannot", "without"];

const CLAUSE_DELIMITERS: &[&str] = &[",", ".", ";", "!", "?", "but", "however", "although", "though"];

const INDIRECT_REQUEST_PATTERNS: &[(&str, &str)] = &[
    ("cold_room", "it is cold in here"),
    ("warm_room", "it is warm in here"),
    ("dark_room", "it is dark in here"),
    ("could_you", "could you"),
    ("would_you_mind", "would you mind"),
    ("would_you", "would you"),
    ("can_you", "can you"),
    ("can_someone", "can someone"),
    ("is_there_any_way", "is there any way"),
    ("if_possible", "if possible"),
    ("wondering_if", "wondering if you"),
];

const RHETORICAL_PREFIXES: &[&str] = &[
    "who knows",
    "who cares",
    "why bother",
    "isn't it",
    "is not it",
    "doesn't everybody",
    "does not everybody",
    "how could anyone",
    "how should i know",
    "what else did you expect",
    "what difference does it make",
];

const IRONY_MARKERS: &[&str] = &[
    "oh great",
    "brilliant",
    "just wonderful",
    "wonderful",
    "as if",
    "yeah right",
    "surely you must be thrilled",
    "what a surprise",
    "perfect timing",
];

pub fn fold_contractions(s: &str) -> String {
    let mut out = s.to_ascii_lowercase();
    for &(c, repl) in CONTRACTIONS {
        out = out.replace(c, repl);
    }
    out
}

pub fn replace_number_words(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut token = String::new();
    let mut is_word = false;

    for ch in s.chars() {
        let ch_word = ch.is_ascii_alphanumeric() || ch == '_';
        if token.is_empty() {
            token.push(ch);
            is_word = ch_word;
        } else if ch_word == is_word {
            token.push(ch);
        } else {
            if is_word {
                let low = token.to_ascii_lowercase();
                if let Some(&(_, num)) = NUMBER_WORDS.iter().find(|&&(w, _)| w == low) {
                    out.push_str(num);
                } else {
                    out.push_str(&token);
                }
            } else {
                out.push_str(&token);
            }
            token.clear();
            token.push(ch);
            is_word = ch_word;
        }
    }
    if !token.is_empty() {
        if is_word {
            let low = token.to_ascii_lowercase();
            if let Some(&(_, num)) = NUMBER_WORDS.iter().find(|&&(w, _)| w == low) {
                out.push_str(num);
            } else {
                out.push_str(&token);
            }
        } else {
            out.push_str(&token);
        }
    }
    out
}

pub fn stem(w: &str) -> String {
    let w = w.to_ascii_lowercase();
    if w.len() <= 3 {
        return w;
    }
    if w.ends_with("sses") {
        return w[..w.len() - 2].to_string();
    }
    if w.ends_with("ies") {
        return format!("{}y", &w[..w.len() - 3]);
    }
    if w.ends_with("ss") {
        return w;
    }
    if w.ends_with('s') && !w.ends_with("us") && !w.ends_with("is") {
        return w[..w.len() - 1].to_string();
    }
    if w.ends_with("eed") && w.len() > 4 {
        return w[..w.len() - 1].to_string();
    }
    if w.ends_with("ed") && w.len() > 4 {
        let stem_base = &w[..w.len() - 2];
        if stem_base.ends_with('i') {
            return format!("{}y", &stem_base[..stem_base.len() - 1]);
        }
        if stem_base.len() >= 2
            && stem_base.as_bytes()[stem_base.len() - 1] == stem_base.as_bytes()[stem_base.len() - 2]
        {
            return stem_base[..stem_base.len() - 1].to_string();
        }
        return stem_base.to_string();
    }
    if w.ends_with("ing") && w.len() > 5 {
        let stem_base = &w[..w.len() - 3];
        if stem_base.len() >= 2
            && stem_base.as_bytes()[stem_base.len() - 1] == stem_base.as_bytes()[stem_base.len() - 2]
        {
            return stem_base[..stem_base.len() - 1].to_string();
        }
        return stem_base.to_string();
    }
    if w.ends_with("tion") && w.len() > 6 {
        return format!("{}t", &w[..w.len() - 4]);
    }
    if w.ends_with("ment") && w.len() > 6 {
        return w[..w.len() - 4].to_string();
    }
    if w.ends_with("able") && w.len() > 6 {
        return w[..w.len() - 4].to_string();
    }
    w
}

/// Tokenizes text returning `(token, stem, is_negated)`.
pub fn tokenize_normalized(text_str: &str) -> Vec<(String, String, bool)> {
    let folded = fold_contractions(text_str);
    let nums = replace_number_words(&folded);

    let mut raw_tokens = Vec::new();
    let mut cur = String::new();
    let mut is_alnum = false;

    for ch in nums.chars() {
        if ch.is_ascii_alphanumeric() {
            if is_alnum {
                cur.push(ch);
            } else {
                if !cur.is_empty() {
                    raw_tokens.push(cur);
                    cur = String::new();
                }
                cur.push(ch);
                is_alnum = true;
            }
        } else if matches!(ch, ',' | '.' | ';' | '!' | '?') {
            if !cur.is_empty() {
                raw_tokens.push(cur);
                cur = String::new();
            }
            raw_tokens.push(ch.to_string());
            is_alnum = false;
        } else {
            if is_alnum {
                raw_tokens.push(cur);
                cur = String::new();
                is_alnum = false;
            }
        }
    }
    if !cur.is_empty() {
        raw_tokens.push(cur);
    }

    let mut out = Vec::new();
    let mut neg_countdown: usize = 0;

    for t in raw_tokens {
        let t_str = t.trim();
        if t_str.is_empty() {
            continue;
        }
        if CLAUSE_DELIMITERS.contains(&t_str) {
            neg_countdown = 0;
            continue;
        }
        if !t_str.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        let low = t_str.to_ascii_lowercase();
        if NEGATION_MARKERS.contains(&low.as_str()) {
            neg_countdown = 4;
            out.push((low.clone(), low, false));
            continue;
        }
        let is_neg = neg_countdown > 0;
        if is_neg {
            neg_countdown -= 1;
        }
        let st = stem(&low);
        out.push((low, st, is_neg));
    }

    out
}

#[derive(Clone, Debug)]
pub struct ConceptManager {
    pub limits: Limits,
    pub concepts: BTreeMap<String, BTreeSet<String>>,
    pub concept_stems: BTreeMap<String, BTreeSet<String>>,
    pub packs: BTreeMap<String, Vec<String>>,
}

impl Default for ConceptManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConceptManager {
    pub fn new() -> Self {
        Self {
            limits: Limits::defaults(LIMITS),
            concepts: BTreeMap::new(),
            concept_stems: BTreeMap::new(),
            packs: BTreeMap::new(),
        }
    }

    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn op_define(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["concept", "words"], &["pack"])?;
        let cid = ident(c::get(args, "concept")?)?.to_string();
        let ws = c::list(c::get(args, "words")?)?;
        if ws.is_empty() || ws.len() > 128 {
            return Err(c::bad());
        }

        let mut words_set = BTreeSet::new();
        let mut stems_set = BTreeSet::new();
        for w in ws {
            let w_str = c::text(w)?;
            if w_str.is_empty() || w_str.len() > 64 {
                return Err(c::bad());
            }
            let low = w_str.to_ascii_lowercase();
            let st = stem(&low);
            words_set.insert(low);
            stems_set.insert(st);
        }

        let total_concepts = self.concepts.len() + if self.concepts.contains_key(&cid) { 0 } else { 1 };
        if total_concepts > self.limits.get("concepts") {
            return Err(c::bad());
        }

        let total_words: usize =
            self.concepts.iter().filter(|(k, _)| *k != &cid).map(|(_, v)| v.len()).sum::<usize>() + words_set.len();
        if total_words > self.limits.get("words") {
            return Err(c::bad());
        }

        self.concepts.insert(cid.clone(), words_set.clone());
        self.concept_stems.insert(cid.clone(), stems_set);

        if let Ok(pack_val) = c::get(args, "pack") {
            if pack_val != &Value::Null {
                let pname = ident(pack_val)?.to_string();
                if !self.packs.contains_key(&pname) {
                    if self.packs.len() + 1 > self.limits.get("packs") {
                        return Err(c::bad());
                    }
                    self.packs.insert(pname.clone(), Vec::new());
                }
                let p_vec = self.packs.get_mut(&pname).unwrap();
                if !p_vec.contains(&cid) {
                    p_vec.push(cid.clone());
                    p_vec.sort_unstable();
                }
            }
        }

        let words_arr: Vec<Value> = words_set.into_iter().map(Value::str).collect();
        Ok(obj! { "concept" => cid, "words" => Value::Array(words_arr) })
    }

    pub fn op_pack(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["name", "concepts"], &[])?;
        let pname = ident(c::get(args, "name")?)?.to_string();
        let cids = c::list(c::get(args, "concepts")?)?;
        if cids.is_empty() || cids.len() > 128 {
            return Err(c::bad());
        }
        let mut clean_cids = BTreeSet::new();
        for c_val in cids {
            let id = ident(c_val)?;
            clean_cids.insert(id.to_string());
        }
        if !self.packs.contains_key(&pname) && self.packs.len() + 1 > self.limits.get("packs") {
            return Err(c::bad());
        }
        let sorted_cids: Vec<String> = clean_cids.into_iter().collect();
        self.packs.insert(pname.clone(), sorted_cids.clone());
        Ok(obj! {
            "name" => pname,
            "concepts" => Value::Array(sorted_cids.into_iter().map(Value::str).collect()),
        })
    }

    pub fn op_normalize(&self, args: &Value) -> Result<Value> {
        keys(args, &["text"], &[])?;
        let raw_text = c::text(c::get(args, "text")?)?;
        if raw_text.len() > 4096 {
            return Err(c::bad());
        }
        let triples = tokenize_normalized(raw_text);
        let tokens: Vec<Value> = triples.iter().map(|(t, _, _)| Value::str(t.clone())).collect();
        let stems: Vec<Value> = triples.iter().map(|(_, s, _)| Value::str(s.clone())).collect();
        let negated: Vec<Value> = triples.iter().map(|(_, _, n)| Value::from(*n)).collect();

        Ok(obj! {
            "negated" => Value::Array(negated),
            "stems" => Value::Array(stems),
            "tokens" => Value::Array(tokens),
        })
    }

    pub fn op_match(&self, args: &Value) -> Result<Value> {
        keys(args, &["text"], &["concepts", "packs", "forbidden"])?;
        let raw_text = c::text(c::get(args, "text")?)?;
        if raw_text.len() > 4096 {
            return Err(c::bad());
        }
        let triples = tokenize_normalized(raw_text);

        let mut target_concepts = BTreeSet::new();
        let has_concepts = c::get(args, "concepts").is_ok();
        let has_packs = c::get(args, "packs").is_ok();

        if let Ok(c_val) = c::get(args, "concepts") {
            if c_val != &Value::Null {
                for c_elem in c::list(c_val)? {
                    target_concepts.insert(ident(c_elem)?.to_string());
                }
            }
        }
        if let Ok(p_val) = c::get(args, "packs") {
            if p_val != &Value::Null {
                for p_elem in c::list(p_val)? {
                    let pname = ident(p_elem)?;
                    if let Some(list) = self.packs.get(pname) {
                        for cid in list {
                            target_concepts.insert(cid.clone());
                        }
                    }
                }
            }
        }
        if target_concepts.is_empty() && !has_concepts && !has_packs {
            for cid in self.concepts.keys() {
                target_concepts.insert(cid.clone());
            }
        }

        let mut matched_details = Vec::new();
        let mut matched_concept_set = BTreeSet::new();

        for (word, w_stem, is_neg) in &triples {
            for cid in &target_concepts {
                if let Some(w_set) = self.concepts.get(cid) {
                    let s_set = self.concept_stems.get(cid);
                    let matches_word = w_set.contains(word);
                    let matches_stem = s_set.map(|s| s.contains(w_stem)).unwrap_or(false);
                    if matches_word || matches_stem {
                        matched_details.push(obj! {
                            "concept" => cid.clone(),
                            "negated" => *is_neg,
                            "word" => word.clone(),
                        });
                        matched_concept_set.insert(cid.clone());
                    }
                }
            }
        }

        let mut leaks = Vec::new();
        if let Ok(forb_val) = c::get(args, "forbidden") {
            if forb_val != &Value::Null {
                for f_elem in c::list(forb_val)? {
                    let f_raw = c::text(f_elem)?;
                    if f_raw.is_empty() || f_raw.len() > 64 {
                        return Err(c::bad());
                    }
                    let f_low = f_raw.to_ascii_lowercase();
                    let f_stem = stem(&f_low);

                    for (word, w_stem, is_neg) in &triples {
                        if (*word == f_low || *w_stem == f_stem) && !*is_neg {
                            leaks.push(obj! {
                                "forbidden" => f_raw,
                                "matched" => word.clone(),
                                "negated" => false,
                            });
                        }
                    }

                    if matched_concept_set.contains(f_raw) {
                        for m in &matched_details {
                            if c::text(c::get(m, "concept")?)? == f_raw && !c::boolean(c::get(m, "negated")?)? {
                                leaks.push(obj! {
                                    "forbidden" => f_raw,
                                    "matched" => c::text(c::get(m, "word")?)?,
                                    "negated" => false,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate leaks by (forbidden, matched)
        let mut seen = BTreeSet::new();
        let mut dedup_leaks = Vec::new();
        for lk in leaks {
            let key = (c::text(c::get(&lk, "forbidden")?)?.to_string(), c::text(c::get(&lk, "matched")?)?.to_string());
            if !seen.contains(&key) {
                seen.insert(key);
                dedup_leaks.push(lk);
            }
        }

        let concepts_arr: Vec<Value> = matched_concept_set.into_iter().map(Value::str).collect();
        Ok(obj! {
            "concepts" => Value::Array(concepts_arr),
            "leaks" => Value::Array(dedup_leaks),
            "matched" => Value::Array(matched_details),
        })
    }

    pub fn op_parse(&self, args: &Value) -> Result<Value> {
        keys(args, &["text"], &["context"])?;
        let raw_text = c::text(c::get(args, "text")?)?;
        if raw_text.len() > 4096 {
            return Err(c::bad());
        }
        let raw_low = raw_text.to_ascii_lowercase().trim().to_string();

        // 1. Indirect request
        let mut indirect = Value::Null;
        for &(pattern_id, p_str) in INDIRECT_REQUEST_PATTERNS {
            if raw_low.contains(p_str) {
                indirect = obj! { "cue" => p_str, "pattern" => pattern_id };
                break;
            }
        }

        // 2. Rhetorical question
        let mut rhetorical = Value::Null;
        if raw_low.ends_with('?') {
            for &p_str in RHETORICAL_PREFIXES {
                if raw_low.starts_with(p_str) || raw_low.contains(&format!(" {p_str}")) {
                    rhetorical = obj! { "cue" => p_str, "question" => raw_text };
                    break;
                }
            }
        }

        // 3. Irony marker
        let mut irony = Value::Null;
        for &im in IRONY_MARKERS {
            if raw_low.contains(im) {
                irony = obj! { "marker" => im };
                break;
            }
        }

        let mut forms = Vec::new();
        if indirect != Value::Null {
            forms.push("indirect_request");
        }
        if rhetorical != Value::Null {
            forms.push("rhetorical_question");
        }
        if irony != Value::Null {
            forms.push("irony");
        }
        forms.sort_unstable();

        Ok(obj! {
            "forms" => Value::Array(forms.into_iter().map(Value::str).collect()),
            "indirect_request" => indirect,
            "irony" => irony,
            "rhetorical" => rhetorical,
        })
    }

    pub fn settle_state(&mut self) -> Result<()> {
        if self.concepts.len() > self.limits.get("concepts") {
            return Err(c::bad());
        }
        if self.packs.len() > self.limits.get("packs") {
            return Err(c::bad());
        }
        let total_words: usize = self.concepts.values().map(|v| v.len()).sum();
        if total_words > self.limits.get("words") {
            return Err(c::bad());
        }
        let mut stems_map = BTreeMap::new();
        for (cid, words) in &self.concepts {
            let mut stems = BTreeSet::new();
            for w in words {
                stems.insert(stem(w));
            }
            stems_map.insert(cid.clone(), stems);
        }
        self.concept_stems = stems_map;
        Ok(())
    }

    pub fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &["limits"])?;
        let supplied = c::get(request, "limits").is_ok();
        if let Ok(l) = c::get(request, "limits") {
            self.limits.apply(l)?;
        }
        let ops = c::list(c::get(request, "ops")?)?;
        if ops.is_empty() || ops.len() > 64 {
            return Err(c::bad());
        }
        let mut results = Vec::new();
        for item in ops {
            keys(item, &["op"], &["args"])?;
            let op_name = c::text(c::get(item, "op")?)?;
            let empty = obj! {};
            let args = c::get(item, "args").unwrap_or(&empty);
            let res = match op_name {
                "define" => self.op_define(args)?,
                "pack" => self.op_pack(args)?,
                "normalize" => self.op_normalize(args)?,
                "match" => self.op_match(args)?,
                "parse" => self.op_parse(args)?,
                "export" => persist::export_op(self, &self.limits, args)?,
                "import" => {
                    let limits = self.limits.clone();
                    persist::import(self, &limits, args)?
                }
                _ => return Err(c::bad()),
            };
            results.push(res);
        }
        let mut out = obj! {
            "results" => Value::Array(results),
            "version" => 1,
        };
        if supplied {
            if let Value::Object(ref mut fields) = out {
                fields.insert(
                    0,
                    (
                        "limits".to_string(),
                        Value::Object(vec![
                            ("concepts".to_string(), Value::from(self.limits.get("concepts") as u64)),
                            ("packs".to_string(), Value::from(self.limits.get("packs") as u64)),
                            ("words".to_string(), Value::from(self.limits.get("words") as u64)),
                        ]),
                    ),
                );
            }
        }
        Ok(out)
    }
}

impl Persist for ConceptManager {
    fn save(&self) -> Value {
        let concepts_arr: Vec<Value> = self
            .concepts
            .iter()
            .map(|(k, v)| {
                Value::Array(vec![Value::str(k.clone()), Value::Array(v.iter().cloned().map(Value::str).collect())])
            })
            .collect();

        let packs_arr: Vec<Value> = self
            .packs
            .iter()
            .map(|(k, v)| {
                Value::Array(vec![Value::str(k.clone()), Value::Array(v.iter().cloned().map(Value::str).collect())])
            })
            .collect();

        obj! {
            "concepts" => Value::Array(concepts_arr),
            "packs" => Value::Array(packs_arr),
        }
    }

    fn load(v: &Value) -> Result<Self> {
        exact_fields(v, &["concepts", "packs"])?;
        let c_list = c::list(c::get(v, "concepts")?)?;
        let mut concepts = BTreeMap::new();
        let mut stems_map = BTreeMap::new();
        let mut last_cid: Option<&str> = None;
        let mut total_words = 0;

        for pair in c_list {
            let pair_arr = c::list(pair)?;
            if pair_arr.len() != 2 {
                return Err(c::bad());
            }
            let cid = ident(&pair_arr[0])?;
            if let Some(prev) = last_cid {
                if cid <= prev {
                    return Err(c::bad());
                }
            }
            last_cid = Some(cid);

            let ws = c::list(&pair_arr[1])?;
            total_words += ws.len();
            let mut w_set = BTreeSet::new();
            let mut s_set = BTreeSet::new();
            let mut last_w: Option<&str> = None;
            for w in ws {
                let w_str = c::text(w)?;
                if w_str.is_empty() || w_str.len() > 64 {
                    return Err(c::bad());
                }
                if let Some(prev_w) = last_w {
                    if w_str <= prev_w {
                        return Err(c::bad());
                    }
                }
                last_w = Some(w_str);
                let low = w_str.to_ascii_lowercase();
                let st = stem(&low);
                w_set.insert(low);
                s_set.insert(st);
            }
            concepts.insert(cid.to_string(), w_set);
            stems_map.insert(cid.to_string(), s_set);
        }

        let mut mgr = Self::new();
        if total_words > mgr.limits.get("words") {
            return Err(c::bad());
        }

        let p_list = c::list(c::get(v, "packs")?)?;
        let mut packs = BTreeMap::new();
        let mut last_p: Option<&str> = None;

        for pair in p_list {
            let pair_arr = c::list(pair)?;
            if pair_arr.len() != 2 {
                return Err(c::bad());
            }
            let pname = ident(&pair_arr[0])?;
            if let Some(prev) = last_p {
                if pname <= prev {
                    return Err(c::bad());
                }
            }
            last_p = Some(pname);

            let cids = c::list(&pair_arr[1])?;
            let mut p_cids = Vec::with_capacity(cids.len());
            let mut last_c: Option<&str> = None;
            for c_val in cids {
                let id = ident(c_val)?;
                if let Some(prev_c) = last_c {
                    if id <= prev_c {
                        return Err(c::bad());
                    }
                }
                last_c = Some(id);
                p_cids.push(id.to_string());
            }
            packs.insert(pname.to_string(), p_cids);
        }

        mgr.concepts = concepts;
        mgr.concept_stems = stems_map;
        mgr.packs = packs;
        mgr.settle_state()?;
        Ok(mgr)
    }
}

durable!(ConceptManager, "concept", LIMITS);

pub fn workflow(request: &Value) -> Result<Value> {
    ConceptManager::new().workflow(request)
}
