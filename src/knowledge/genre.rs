//! Genre profiles and lexicon packs (pass 4, stage 1).
//!
//! A genre profile names the words a setting uses for each closed engine list (a *slot*) and a
//! few setting axes. Every slot keeps the engines' fixed order and arity, so mechanisms and
//! decisions stay the same and only the words change:
//!
//! | slot | engine words |
//! |---|---|
//! | `light` | dark, dim, bright |
//! | `access` | public, customers_only, members_only, private, restricted |
//! | `privacy` | public, semi_private, private, echoing |
//! | `severity` | etiquette, custom, contract, taboo, crime |
//! | `contract` | alibi_pact, apology_owed, employment, favor, keep_secret, loan, promise, trade_order |
//! | `address` | stranger, acquaintance (free text up to 40 characters) |
//!
//! Four genres are built in: `village` uses the engine words; `orbital_station`, `noir_city`
//! and `academy` are original settings. `define` adds a genre from a base with some slots
//! renamed (names unique within a slot). `render` rewrites an engine output into a genre's words
//! under the keys the host names; `normalize` rewrites genre words back to engine words and
//! refuses any word outside the genre's vocabulary, so text from a model or a player never
//! reaches an engine unchecked. Lexicon packs add phrases for the pragmatic parser's acts
//! (`station_crew`, `noir_street`, `school_casual` built in); `lexicon_requests` turns packs into
//! `pragmatics` requests.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::knowledge::genre::Genres;
//!
//! let mut g = Genres::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "render", "args": {"genre": "orbital_station", "keys": {"access": "access", "sanction_norm": "severity"},
//!     "value": {"access": "private", "sanction_norm": "crime", "allowed": false}}},
//!   {"op": "normalize", "args": {"genre": "noir_city", "keys": {"privacy": "privacy"},
//!     "value": {"privacy": "back_room"}}}
//! ]}"#, 4096)?;
//! let out = g.workflow(&req)?.to_json();
//! // The same decision in station words, and noir words mapped back for the engines.
//! assert!(out.contains(r#""value":{"access":"quarters","sanction_norm":"station_offense","allowed":false}"#));
//! assert!(out.contains(r#""value":{"privacy":"private"}"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, text_bound};
use crate::character::limits::{self, Limit, Limits};
use crate::character::pragmatics::{words, PRECEDENCE};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "genres", default: 16, ceiling: 100_000 },
    Limit { name: "packs", default: 16, ceiling: 100_000 },
    Limit { name: "nodes", default: 4096, ceiling: 1_000_000 },
];
/// Deepest nesting `render` and `normalize` walk.
pub const MAX_DEPTH: usize = 64;
/// Slots with their engine words, in engine order.
pub const SLOTS: [(&str, &[&str]); 6] = [
    ("light", &["dark", "dim", "bright"]),
    ("access", &["public", "customers_only", "members_only", "private", "restricted"]),
    ("privacy", &["public", "semi_private", "private", "echoing"]),
    ("severity", &["etiquette", "custom", "contract", "taboo", "crime"]),
    (
        "contract",
        &["alibi_pact", "apology_owed", "employment", "favor", "keep_secret", "loan", "promise", "trade_order"],
    ),
    ("address", &["stranger", "acquaintance"]),
];
pub const AXES: [&str; 4] = ["tech", "magic", "law_reach", "scarcity"];
pub const CYCLES: [&str; 3] = ["natural", "artificial", "none"];

type Vocab<'a> = &'a [(&'a str, &'a [&'a str])];

/// Built-in genres: (id, axes tech/magic/law_reach/scarcity, day cycle, renamed slots).
pub const BUILTIN_GENRES: [(&str, [u64; 4], &str, Vocab<'static>); 4] = [
    ("village", [20, 30, 40, 40], "natural", &[]),
    (
        "orbital_station",
        [90, 0, 70, 65],
        "artificial",
        &[
            ("light", &["lights_out", "night_cycle", "full_lights"]),
            ("access", &["open_deck", "crew_only", "officers_only", "quarters", "sealed"]),
            ("privacy", &["common", "alcove", "cabin", "open_channel"]),
            ("severity", &["protocol", "regulation", "contract_breach", "mutiny_taboo", "station_offense"]),
            (
                "contract",
                &[
                    "cover_story_pact",
                    "owed_apology",
                    "duty_roster",
                    "favor",
                    "sealed_secret",
                    "ration_loan",
                    "promise",
                    "supply_order",
                ],
            ),
            ("address", &["crewmate", "spacer"]),
        ],
    ),
    (
        "noir_city",
        [45, 0, 50, 55],
        "natural",
        &[
            ("light", &["dark", "neon", "daylight"]),
            ("access", &["street", "patrons_only", "club_members", "private", "police_line"]),
            ("privacy", &["street", "booth", "back_room", "echoing"]),
            ("severity", &["rude", "bad_form", "double_cross", "unforgivable", "felony"]),
            ("contract", &["alibi", "owed_apology", "on_the_payroll", "favor", "keep_mum", "loan", "promise", "job"]),
            ("address", &["pal", "friend"]),
        ],
    ),
    (
        "academy",
        [60, 10, 60, 20],
        "natural",
        &[
            ("light", &["lights_out", "dim", "bright"]),
            ("access", &["public", "visitors_only", "students_only", "private", "staff_only"]),
            ("privacy", &["hallway", "study_corner", "dorm_room", "stairwell"]),
            ("severity", &["manners", "house_rule", "honor_pledge", "forbidden", "expellable"]),
            (
                "contract",
                &[
                    "cover_story",
                    "owed_apology",
                    "part_time_job",
                    "favor",
                    "keep_secret",
                    "borrowed_notes",
                    "promise",
                    "trade",
                ],
            ),
            ("address", &["new kid", "classmate"]),
        ],
    ),
];

/// Built-in lexicon packs: (id, act → phrases).
pub const BUILTIN_PACKS: [(&str, Vocab<'static>); 3] = [
    (
        "station_crew",
        &[
            ("threat", &["out the airlock"]),
            ("accusation", &["you jacked"]),
            ("request_help", &["need a hand"]),
            ("greeting", &["morning shift", "hey crewmate"]),
            ("farewell", &["next shift", "clear skies"]),
            ("compliment", &["clean work", "solid work"]),
        ],
    ),
    (
        "noir_street",
        &[
            ("threat", &["you ll be sorry"]),
            ("accusation", &["you sold me out", "rat"]),
            ("offer_or_transfer", &["got something for you"]),
            ("request_help", &["do me a favor"]),
            ("greeting", &["evening pal"]),
            ("farewell", &["see you around"]),
            ("compliment", &["nice work"]),
        ],
    ),
    (
        "school_casual",
        &[
            ("threat", &["you ll pay"]),
            ("accusation", &["you snitched"]),
            ("apology", &["my bad"]),
            ("request_help", &["help me out", "can i borrow"]),
            ("greeting", &["yo", "sup"]),
            ("farewell", &["later", "bye"]),
            ("compliment", &["nice", "so cool"]),
        ],
    ),
];

fn need(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(c::bad())
    }
}

fn owned(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| x.to_string()).collect()
}

fn phrase_of(v: &Value) -> Result<String> {
    let s = text_bound(v, 64)?;
    let ws = words(s);
    need((1..=4).contains(&ws.len()) && ws.join(" ") == s)?;
    Ok(s.to_string())
}

/// A genre: axes, day cycle and one name list per slot (engine order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Genre {
    pub axes: [u64; 4],
    pub day_cycle: &'static str,
    pub vocab: Vec<Vec<String>>,
    pub builtin: bool,
}

impl Genre {
    fn customized(&self) -> Value {
        Value::Array(
            SLOTS
                .iter()
                .zip(&self.vocab)
                .filter(|(s, v)| v.iter().map(String::as_str).ne(s.1.iter().copied()))
                .map(|(s, _)| Value::from(s.0))
                .collect(),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Genres {
    pub genres: BTreeMap<String, Genre>,
    /// Author packs: act (in parser precedence order) → phrases.
    pub packs: BTreeMap<String, Vec<(&'static str, Vec<String>)>>,
    pub limits: Limits,
}

impl Default for Genres {
    fn default() -> Self {
        Genres { genres: BTreeMap::new(), packs: BTreeMap::new(), limits: Limits::defaults(LIMITS) }
    }
}

struct Walk<'a> {
    mapping: &'a BTreeMap<String, usize>,
    table: Vec<BTreeMap<&'a str, &'a str>>,
    strict: bool,
    nodes: usize,
    max_nodes: usize,
    unmapped: u64,
}

impl Walk<'_> {
    fn go(&mut self, v: &Value, depth: usize, slot: Option<usize>) -> Result<Value> {
        self.nodes += 1;
        need(self.nodes <= self.max_nodes && depth <= MAX_DEPTH)?;
        if let (Some(i), Value::String(s)) = (slot, v) {
            return match self.table[i].get(s.as_str()) {
                Some(t) => Ok(Value::from(*t)),
                None => {
                    need(!self.strict)?;
                    self.unmapped += 1;
                    Ok(v.clone())
                }
            };
        }
        match v {
            Value::Object(xs) => {
                let mut out = Vec::with_capacity(xs.len());
                for (k, x) in xs {
                    let child = self.mapping.get(k).copied();
                    out.push((k.clone(), self.go(x, depth + 1, child)?));
                }
                Ok(Value::Object(out))
            }
            Value::Array(xs) => {
                let mut out = Vec::with_capacity(xs.len());
                for x in xs {
                    out.push(self.go(x, depth + 1, slot)?);
                }
                Ok(Value::Array(out))
            }
            _ => Ok(v.clone()),
        }
    }
}

impl Genres {
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "list" => self.op_list(args),
            "show" => self.op_show(args),
            "define" => self.op_define(args, l),
            "pack" => self.op_pack(args, l),
            "lexicon_requests" => self.op_lexicon_requests(args),
            "render" => self.translate(args, l, false),
            "normalize" => self.translate(args, l, true),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    /// A built-in or authored genre.
    pub fn genre(&self, id: &str) -> Result<Genre> {
        if let Some((_, axes, cycle, vocab)) = BUILTIN_GENRES.iter().find(|g| g.0 == id) {
            let v = SLOTS
                .iter()
                .map(|(s, words)| owned(vocab.iter().find(|x| x.0 == *s).map_or(*words, |x| x.1)))
                .collect();
            return Ok(Genre { axes: *axes, day_cycle: cycle, vocab: v, builtin: true });
        }
        self.genres.get(id).cloned().ok_or_else(c::bad)
    }

    fn pack(&self, id: &str) -> Result<Vec<(&'static str, Vec<String>)>> {
        if let Some((_, acts)) = BUILTIN_PACKS.iter().find(|p| p.0 == id) {
            return Ok(PRECEDENCE
                .iter()
                .filter_map(|a| acts.iter().find(|x| x.0 == *a).map(|x| (*a, owned(x.1))))
                .collect());
        }
        self.packs.get(id).cloned().ok_or_else(c::bad)
    }

    fn op_list(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &[])?;
        let genres: BTreeSet<&str> =
            BUILTIN_GENRES.iter().map(|g| g.0).chain(self.genres.keys().map(String::as_str)).collect();
        let packs: BTreeSet<&str> =
            BUILTIN_PACKS.iter().map(|p| p.0).chain(self.packs.keys().map(String::as_str)).collect();
        Ok(obj! {
            "genres" => Value::Array(genres.into_iter().map(Value::from).collect()),
            "packs" => Value::Array(packs.into_iter().map(Value::from).collect()),
        })
    }

    fn op_show(&self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &[])?;
        let id = ident(c::get(args, "id")?)?;
        let g = self.genre(id)?;
        Ok(obj! {
            "axes" => Value::Object(AXES.iter().zip(g.axes).map(|(k, v)| (k.to_string(), Value::from(v))).collect()),
            "builtin" => g.builtin,
            "customized" => g.customized(),
            "day_cycle" => g.day_cycle,
            "id" => id,
            "vocab" => Value::Object(
                SLOTS.iter().zip(&g.vocab).map(|(s, v)| (s.0.to_string(), Value::from(v.clone()))).collect()
            ),
        })
    }

    fn op_define(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id"], &["base", "axes", "day_cycle", "vocab"])?;
        let id = ident(c::get(args, "id")?)?.to_string();
        need(BUILTIN_GENRES.iter().all(|g| g.0 != id))?;
        let base = match opt(args, "base") {
            None => "village",
            Some(b) => ident(b)?,
        };
        let mut g = self.genre(base)?;
        if let Some(ax) = opt(args, "axes") {
            keys(ax, &[], &AXES)?;
            for (i, k) in AXES.iter().enumerate() {
                g.axes[i] = opt_uint(ax, k, 0, 100, g.axes[i])?;
            }
        }
        if let Some(cy) = opt(args, "day_cycle") {
            let cy = ident(cy)?;
            g.day_cycle = CYCLES.iter().copied().find(|x| *x == cy).ok_or_else(c::bad)?;
        }
        if let Some(vo) = opt(args, "vocab") {
            keys(vo, &[], &SLOTS.map(|s| s.0))?;
            for (i, (slot, words)) in SLOTS.iter().enumerate() {
                let Some(rows) = opt(vo, slot) else { continue };
                let rows = rows.as_array().ok_or_else(c::bad)?;
                need(rows.len() == words.len())?;
                let mut names = Vec::new();
                for x in rows {
                    let n = if *slot == "address" { text_bound(x, 40)? } else { ident(x)? };
                    need(!names.iter().any(|m: &String| m == n))?;
                    names.push(n.to_string());
                }
                g.vocab[i] = names;
            }
        }
        need(self.genres.contains_key(&id) || self.genres.len() < l.get("genres"))?;
        g.builtin = false;
        let out = obj! { "customized" => g.customized(), "id" => id.as_str() };
        self.genres.insert(id, g);
        Ok(out)
    }

    fn op_pack(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "phrases"], &[])?;
        let id = ident(c::get(args, "id")?)?.to_string();
        need(BUILTIN_PACKS.iter().all(|p| p.0 != id))?;
        let ph = c::get(args, "phrases")?;
        need(ph.as_object().is_some_and(|o| !o.is_empty()))?;
        keys(ph, &[], &PRECEDENCE)?;
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for act in PRECEDENCE {
            let Some(rows) = ph.get(act) else { continue };
            let rows = rows.as_array().ok_or_else(c::bad)?;
            need((1..=16).contains(&rows.len()))?;
            let mut ps = Vec::new();
            for p in rows {
                let p = phrase_of(p)?;
                need(!seen.contains(&p) && !ps.contains(&p))?;
                ps.push(p);
            }
            seen.extend(ps.iter().cloned());
            out.push((act, ps));
        }
        need(self.packs.contains_key(&id) || self.packs.len() < l.get("packs"))?;
        let acts = Value::Array(out.iter().map(|(a, _)| Value::from(*a)).collect());
        self.packs.insert(id.clone(), out);
        Ok(obj! { "acts" => acts, "id" => id, "phrases" => seen.len() })
    }

    fn op_lexicon_requests(&self, args: &Value) -> Result<Value> {
        keys(args, &["packs"], &[])?;
        let rows = c::get(args, "packs")?.as_array().ok_or_else(c::bad)?;
        need((1..=8).contains(&rows.len()))?;
        let mut ids = Vec::new();
        for x in rows {
            let id = ident(x)?;
            need(!ids.contains(&id))?;
            ids.push(id);
        }
        let mut ops = Vec::new();
        for id in ids {
            for (act, phrases) in self.pack(id)? {
                ops.push(obj! { "args" => obj! { "act" => act, "phrases" => phrases }, "op" => "lexicon" });
            }
        }
        let n = ops.len();
        Ok(obj! {
            "ops" => n,
            "requests" => Value::Array(vec![obj! { "args" => obj! { "ops" => Value::Array(ops) }, "kind" => "pragmatics" }]),
        })
    }

    fn translate(&self, args: &Value, l: &Limits, strict: bool) -> Result<Value> {
        keys(args, &["genre", "value", "keys"], &[])?;
        let gid = ident(c::get(args, "genre")?)?;
        // Authored genres are borrowed; built-ins (never stored) are built on the fly.
        let built;
        let g = match self.genres.get(gid) {
            Some(g) => g,
            None => {
                built = self.genre(gid)?;
                &built
            }
        };
        let km = c::get(args, "keys")?.as_object().ok_or_else(c::bad)?;
        need((1..=16).contains(&km.len()))?;
        let mut mapping = BTreeMap::new();
        for (k, s) in km {
            ident(&Value::from(k.as_str()))?;
            let s = s.as_str().ok_or_else(c::bad)?;
            let i = SLOTS.iter().position(|x| x.0 == s).ok_or_else(c::bad)?;
            mapping.insert(k.clone(), i);
        }
        let table = SLOTS
            .iter()
            .zip(&g.vocab)
            .map(|(s, names)| {
                s.1.iter()
                    .zip(names)
                    .map(|(e, n)| if strict { (n.as_str(), *e) } else { (*e, n.as_str()) })
                    .collect::<BTreeMap<&str, &str>>()
            })
            .collect();
        let mut w = Walk { mapping: &mapping, table, strict, nodes: 0, max_nodes: l.get("nodes"), unmapped: 0 };
        let value = w.go(c::get(args, "value")?, 0, None)?;
        Ok(if strict {
            obj! { "genre" => gid, "value" => value }
        } else {
            obj! { "genre" => gid, "unmapped" => w.unmapped, "value" => value }
        })
    }
}

/// Runs one request on a fresh [`Genres`] (local kind `genre`).
pub fn workflow(request: &Value) -> Result<Value> {
    Genres::new().workflow(request)
}
