//! Searchable, paged catalogs of game data (items, blocks, spells, mobs...). Instead of
//! pasting a whole registry into every prompt, send a small page and let the model ask
//! for more. Each catalog has a `generation` fingerprint, so a follow-up request can be
//! refused when the data changed between pages.
//!
//! Catalog entries are registered metadata only: they say an ID exists, not that the
//! player owns it, that it is nearby, or that an action using it is allowed.

use crate::data::json::{Error, Result, Value};
use crate::fingerprint;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: String,
    pub id: String,
    pub tags: Vec<String>,
    /// True when the entry had more tags than `Limits::tags` allowed.
    pub tags_truncated: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub entries: usize,
    pub tags: usize,
    pub results: usize,
    pub query_chars: usize,
    pub query_words: usize,
    pub offset: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { entries: 65536, tags: 8, results: 8, query_chars: 64, query_words: 4, offset: 4096 }
    }
}

#[derive(Debug)]
pub struct Catalog {
    entries: Vec<Entry>,
    generation: String,
    complete: bool,
    limits: Limits,
}

#[derive(Debug)]
pub struct Page<'a> {
    pub generation: &'a str,
    pub complete: bool,
    pub entries: Vec<&'a Entry>,
    /// Offset for the next page, when more results exist within the offset limit.
    pub next: Option<usize>,
}

fn valid_text(s: &str, max: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max && !s.chars().any(|c| c.is_control() || c.is_whitespace())
}

impl Catalog {
    /// Builds a catalog, sorting by kind then ID. `complete` says whether the source
    /// registry fit; tags beyond the limit are cut and marked truncated.
    pub fn new(mut entries: Vec<Entry>, complete: bool, limits: Limits) -> Result<Catalog> {
        if entries.len() > limits.entries {
            return Err(Error::new(format!("{} catalog entries; limit is {}", entries.len(), limits.entries)));
        }
        for e in entries.iter_mut() {
            if !valid_text(&e.kind, 64) || !valid_text(&e.id, 256) || e.tags.iter().any(|t| !valid_text(t, 256)) {
                return Err(Error::new(format!(
                    "catalog entry \"{}\": kind, id and tags must be non-empty with no spaces or control characters",
                    e.id
                )));
            }
            e.tags.sort();
            if e.tags.len() > limits.tags {
                e.tags.truncate(limits.tags);
                e.tags_truncated = true;
            }
        }
        entries.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
        if let Some(pair) = entries.windows(2).find(|w| w[0].kind == w[1].kind && w[0].id == w[1].id) {
            return Err(Error::new(format!("duplicate catalog entry {}:{}", pair[0].kind, pair[0].id)));
        }
        let generation = fingerprint(&format!("{}:{complete}", Self::rows(&entries).to_json()));
        Ok(Catalog { entries, generation, complete, limits })
    }

    /// Reads `{"complete":bool?,"entries":[{"kind","id","tags"?}]}` or a bare entry array.
    pub fn from_json(value: &Value, limits: Limits) -> Result<Catalog> {
        let (list, complete) = match value {
            Value::Array(items) => (items.as_slice(), true),
            _ => (
                value
                    .get("entries")
                    .and_then(Value::as_array)
                    .ok_or_else(|| Error::new("catalog needs an entries array"))?,
                value.get("complete").and_then(Value::as_bool).unwrap_or(true),
            ),
        };
        let mut entries = Vec::with_capacity(list.len());
        for (n, item) in list.iter().enumerate() {
            let text = |key: &str| {
                item.get(key)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| Error::new(format!("catalog entry {n}: \"{key}\" must be a string")))
            };
            let tags = match item.get("tags") {
                None => Vec::new(),
                Some(v) => v
                    .as_array()
                    .and_then(|a| a.iter().map(|t| t.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
                    .ok_or_else(|| Error::new(format!("catalog entry {n}: tags must be strings")))?,
            };
            entries.push(Entry { kind: text("kind")?, id: text("id")?, tags, tags_truncated: false });
        }
        Catalog::new(entries, complete, limits)
    }

    pub fn generation(&self) -> &str {
        &self.generation
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Finds entries whose ID or tags contain every query word (up to `query_words`).
    /// A leading `#` matches tags written with or without it; a trailing plural `s` is
    /// optional. Pass the generation from an earlier page to refuse changed data.
    pub fn search(
        &self,
        query: &str,
        kind: Option<&str>,
        offset: usize,
        count: usize,
        expected: Option<&str>,
    ) -> Result<Page<'_>> {
        if query.chars().count() > self.limits.query_chars || query.chars().any(char::is_control) {
            return Err(Error::new(format!(
                "query must be at most {} characters with no control characters",
                self.limits.query_chars
            )));
        }
        if offset > self.limits.offset || count == 0 || count > self.limits.results {
            return Err(Error::new(format!(
                "offset must be <= {} and count 1..={}",
                self.limits.offset, self.limits.results
            )));
        }
        if expected.is_some_and(|g| g != self.generation) {
            return Err(Error::new("catalog generation changed; start the search again"));
        }
        let lower = query.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        if words.len() > self.limits.query_words {
            return Err(Error::new(format!("up to {} search words", self.limits.query_words)));
        }
        let mut out = Vec::new();
        let mut skipped = 0;
        let mut more = false;
        for e in &self.entries {
            if kind.is_some_and(|k| k != e.kind) {
                continue;
            }
            let text = format!("{} {}", e.id, e.tags.join(" ")).to_lowercase();
            let hit = words.iter().all(|w| {
                let w = w.strip_prefix('#').unwrap_or(w);
                text.contains(w) || (w.len() > 3 && w.ends_with('s') && text.contains(&w[..w.len() - 1]))
            });
            if !hit {
                continue;
            }
            if skipped < offset {
                skipped += 1;
                continue;
            }
            if out.len() == count {
                more = true;
                break;
            }
            out.push(e);
        }
        let next = offset + out.len();
        Ok(Page {
            generation: &self.generation,
            complete: self.complete,
            next: if more && next <= self.limits.offset { Some(next) } else { None },
            entries: out,
        })
    }

    /// Ranks entries by how many words of `text` (such as a player's message) match their
    /// ID, its identifier components (`: . / - _`), or their tags (without `#`; a trailing plural `s` is
    /// optional on both sides), and returns the best `count`, ties in catalog order.
    /// Entries matching nothing are never returned, so an unrelated message gives an
    /// empty page. Pass the generation from an earlier page to refuse changed data.
    pub fn relevant(&self, text: &str, kind: Option<&str>, count: usize, expected: Option<&str>) -> Result<Page<'_>> {
        if count == 0 || count > self.limits.results {
            return Err(Error::new(format!("count must be 1..={}", self.limits.results)));
        }
        if expected.is_some_and(|g| g != self.generation) {
            return Err(Error::new("catalog generation changed; start the search again"));
        }
        let words = crate::data::view::words(text);
        let singular = |w: &str| -> Option<String> {
            (w.chars().count() > 3 && w.ends_with('s')).then(|| w[..w.len() - 1].to_owned())
        };
        let mut scored: Vec<(usize, &Entry)> = Vec::new();
        for e in &self.entries {
            if kind.is_some_and(|k| k != e.kind) {
                continue;
            }
            let mut parts = crate::data::view::identifier_parts(&e.id);
            parts.extend(e.tags.iter().flat_map(|t| crate::data::view::identifier_parts(t.trim_start_matches('#'))));
            let extra: Vec<String> = parts.iter().filter_map(|p| singular(p)).collect();
            parts.extend(extra);
            let score = words.iter().filter(|w| parts.contains(w)).count();
            if score > 0 {
                scored.push((score, e));
            }
        }
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score)); // stable: ties keep catalog order
        Ok(Page {
            generation: &self.generation,
            complete: self.complete,
            entries: scored.into_iter().take(count).map(|(_, e)| e).collect(),
            next: None,
        })
    }

    fn rows(entries: &[Entry]) -> Value {
        Value::Array(
            entries
                .iter()
                .map(|e| {
                    let mut tags = Value::strings(&e.tags);
                    if e.tags_truncated {
                        if let Value::Array(t) = &mut tags {
                            t.push(Value::str("..."));
                        }
                    }
                    Value::Array(vec![Value::str(&e.kind), Value::str(&e.id), tags])
                })
                .collect(),
        )
    }
}

impl Page<'_> {
    /// Compact page for a prompt: one header, one row per entry. A trailing `"..."` tag
    /// means the entry had more tags than were kept.
    pub fn to_value(&self) -> Value {
        let owned: Vec<Entry> = self.entries.iter().map(|e| (*e).clone()).collect();
        Value::object(vec![
            ("generation", Value::str(self.generation)),
            ("complete", Value::Bool(self.complete)),
            ("columns", Value::strings(&["kind", "id", "tags"])),
            ("rows", Catalog::rows(&owned)),
            ("next", self.next.map_or(Value::Null, |n| Value::int(n as i64))),
            ("meaning", Value::str("registered metadata only; not availability, ownership or permission")),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &str, id: &str, tags: &[&str]) -> Entry {
        Entry {
            kind: kind.into(),
            id: id.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            tags_truncated: false,
        }
    }

    fn sample() -> Catalog {
        Catalog::new(
            vec![
                entry("item", "salt_ward", &["#wards"]),
                entry("item", "ember_lantern", &["#lights", "#fire"]),
                entry("item", "glass_lantern", &["#lights"]),
                entry("spell", "spark", &["#fire"]),
            ],
            true,
            Limits { results: 2, ..Limits::default() },
        )
        .unwrap()
    }

    #[test]
    fn searches_ids_tags_and_plurals_with_paging() {
        let c = sample();
        let page = c.search("lanterns", None, 0, 1, None).unwrap();
        assert_eq!(page.entries[0].id, "ember_lantern");
        assert_eq!(page.next, Some(1));
        let page2 = c.search("lanterns", None, 1, 1, Some(c.generation())).unwrap();
        assert_eq!(page2.entries[0].id, "glass_lantern");
        assert_eq!(page2.next, None);
        assert_eq!(c.search("#fire", Some("spell"), 0, 2, None).unwrap().entries[0].id, "spark");
    }

    #[test]
    fn refuses_changed_generation_bad_bounds_and_duplicates() {
        let c = sample();
        assert!(c.search("x", None, 0, 1, Some("0000000000000000")).is_err());
        assert!(c.search("x", None, 0, 3, None).is_err());
        assert!(c.search("a b c d e", None, 0, 1, None).is_err());
        assert!(Catalog::new(vec![entry("item", "a", &[]), entry("item", "a", &[])], true, Limits::default()).is_err());
    }

    #[test]
    fn relevant_ranks_by_matching_words() {
        let c = sample();
        let ids = |text: &str, kind: Option<&str>| -> Vec<String> {
            c.relevant(text, kind, 2, None).unwrap().entries.iter().map(|e| e.id.clone()).collect()
        };
        assert_eq!(ids("I need a light, maybe one that uses fire", None), ["ember_lantern", "glass_lantern"]);
        assert_eq!(ids("cast something with fire", Some("spell")), ["spark"]);
        assert!(ids("nothing here matches", None).is_empty());
        assert!(c.relevant("x", None, 0, None).is_err());
    }

    #[test]
    fn truncates_tags_and_marks_them() {
        let c =
            Catalog::new(vec![entry("item", "a", &["#1", "#2", "#3"])], true, Limits { tags: 2, ..Limits::default() })
                .unwrap();
        let page = c.search("a", None, 0, 1, None).unwrap();
        assert!(page.entries[0].tags_truncated);
        assert!(page.to_value().to_json().contains(r##"["item","a",["#1","#2","..."]]"##));
    }
}
