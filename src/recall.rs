//! Immutable, opt-in lexical index. Build from already privacy-filtered records and
//! rebuild after changes. Unlike legacy recall, no match returns no records.
use crate::{checked::*, memory::Record, obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct RecallIndex {
    records: Vec<Record>,
    normalize: bool,
    aliases: BTreeMap<String, String>,
    index: BTreeMap<String, Vec<usize>>,
}
impl RecallIndex {
    pub fn new(records: &[Record], config: &Value) -> Result<Self> {
        fields(config, &["normalize", "aliases"])?;
        let normalize = boolean(get(config, "normalize")?)?;
        let raw = list(get(config, "aliases")?)?;
        if raw.len() > 64 || records.len() > 4096 {
            return Err(bad());
        }
        let mut aliases = BTreeMap::new();
        for pair in raw {
            let pair = list(pair)?;
            if pair.len() != 2 {
                return Err(bad());
            }
            let (a, b) = (text(&pair[0])?, text(&pair[1])?);
            name(a)?;
            name(b)?;
            if a == b || aliases.insert(a.into(), b.into()).is_some() {
                return Err(bad());
            }
        }
        if aliases.values().any(|v| aliases.contains_key(v)) {
            return Err(bad());
        }
        let mut out = Self { records: Vec::new(), normalize, aliases, index: BTreeMap::new() };
        let mut ids = BTreeSet::new();
        for (i, r) in records.iter().enumerate() {
            if r.id == 0
                || r.id > u32::MAX as u64
                || r.turn > u32::MAX as u64
                || !ids.insert(r.id)
                || !(1..=10).contains(&r.importance)
                || r.text.chars().count() > 280
                || r.tags.len() > 8
            {
                return Err(bad());
            }
            for tag in &r.tags {
                name(tag)?
            }
            let words = out.tokens(&format!("{} {}", r.text, r.tags.join(" ")))?;
            for word in words {
                out.index.entry(word).or_default().push(i)
            }
            out.records.push(r.clone());
        }
        Ok(out)
    }
    fn tokens(&self, text: &str) -> Result<BTreeSet<String>> {
        if text.len() > 4096 {
            return Err(bad());
        }
        let lower = text.to_ascii_lowercase();
        Ok(lower
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .filter(|s| !s.is_empty() && s.len() <= 64)
            .map(|s| {
                let mut word = s;
                if self.normalize {
                    if word.len() > 5 && word.ends_with("ing") {
                        word = &word[..word.len() - 3]
                    } else if word.len() > 4 && word.ends_with("ed") {
                        word = &word[..word.len() - 2]
                    } else if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") {
                        word = &word[..word.len() - 1]
                    }
                }
                self.aliases.get(word).cloned().unwrap_or_else(|| word.into())
            })
            .collect())
    }
    pub fn search(&self, query: &str, count: usize) -> Result<Value> {
        if count > 128 {
            return Err(bad());
        }
        let mut scores = BTreeMap::<usize, usize>::new();
        let mut visits = 0;
        for word in self.tokens(query)? {
            if let Some(postings) = self.index.get(&word) {
                for i in postings {
                    *scores.entry(*i).or_default() += 1;
                    visits += 1
                }
            }
        }
        let candidates = scores.len();
        let mut ranked: Vec<_> = scores.into_iter().collect();
        ranked.sort_by_key(|(i, score)| {
            let r = &self.records[*i];
            (std::cmp::Reverse(*score), std::cmp::Reverse(r.importance), std::cmp::Reverse(r.turn), r.id)
        });
        let rows:Vec<_>=ranked.into_iter().take(count).map(|(i,_)|{let r=&self.records[i];obj!{"id"=>r.id,"turn"=>r.turn,"importance"=>r.importance as u32,"text"=>r.text.as_str(),"tags"=>Value::strings(&r.tags)}}).collect();
        Ok(obj! {"records"=>rows,"candidates"=>candidates,"postings"=>visits})
    }
}
