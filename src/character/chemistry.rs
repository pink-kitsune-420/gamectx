//! Chemistry, banter and pacing (pass 4, stage 6).
//!
//! A social style is a humour kind (dry, warm, absurd, cutting, none), a tempo (fast, measured,
//! slow), formality and assertiveness (0–100) and up to eight values. Chemistry between two styled
//! beings is a fixed function: 50 plus humour fit (a symmetric table), tempo fit (+10 equal, −15
//! fast with slow), minus a tenth of the formality gap, assertiveness (−10 when both are 70 or
//! more, +5 when they differ by 40 or more), +5 per shared value (at most +20) and the pair's
//! history (+4 per landmark up to +20, +3 per favor up to +15, −8 per breach down to −40),
//! clamped to 0–100. It gives cooperativeness and initiative biases of `(chemistry − 50) ÷ 2`
//! (rounded down) and the percent chance a playful move lands.
//!
//! Running jokes are shared moments of a pair with a freshness (millionths): a callback spends
//! 300,000, time restores 2,000 per tick, and a joke used five times is retired. `banter` picks
//! one move by a fixed first-match rule from the pacing advice, the speaker's mood, the pair's
//! chemistry and its fresh jokes (a seeded FNV-1a roll over `"<seed>:<speaker>:<listener>:<tick>"`
//! picks among jokes at least half fresh); a host or a model may propose a move instead, and a
//! tease below the rapport floor (chemistry 60) appraises as an insult. A model only words the
//! move.
//!
//! A scene keeps tension (millionths), the ticks of its last eight beats, recent novelty and
//! engagement, open threads and a silence budget. `pace` advises `end`, `pause`, `slow`,
//! `escalate` or `steady`, so a character can let a conversation end naturally.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend); replays are byte-identical.
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::chemistry::Chemistry;
//!
//! let mut k = Chemistry::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "style", "args": {"being": "mira", "humour": "dry", "tempo": "measured", "formality": 30, "assertiveness": 60}},
//!   {"op": "style", "args": {"being": "guard", "humour": "cutting", "tempo": "slow", "formality": 80, "assertiveness": 90}},
//!   {"op": "banter", "args": {"speaker": "mira", "listener": "guard", "mood": 10, "pacing": "steady", "move": "tease"}}
//! ]}"#, 4096)?;
//! let out = k.workflow(&req)?.to_json();
//! // Without the rapport for it, a tease is heard as an insult.
//! assert!(out.contains(r#""appraisal":"insult""#) && out.contains(r#""lands":false"#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::attention::fnv1a;
use crate::character::beliefs::{opt, opt_uint, text_bound, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, ascending, durable, is_ident, is_text, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "pairs", default: 4096, ceiling: 1_000_000 },
    Limit { name: "jokes", default: 8, ceiling: 64 },
    Limit { name: "scenes", default: 64, ceiling: 100_000 },
];
pub const FULL: u64 = 1_000_000;
pub const HUMOUR: [&str; 5] = ["dry", "warm", "absurd", "cutting", "none"];
pub const TEMPO: [&str; 3] = ["fast", "measured", "slow"];
pub const MOVES: [&str; 7] =
    ["tease", "callback", "escalate", "deflate", "disguised_compliment", "rapport_check", "exit"];
pub const ADVICE: [&str; 5] = ["end", "pause", "slow", "escalate", "steady"];
pub const BEATS: [&str; 5] = ["line", "action", "silence", "reveal", "conflict"];
/// Chemistry a tease needs to land.
pub const FLOOR: i64 = 60;
/// Freshness a callback spends.
pub const SPEND: u64 = 300_000;
/// Freshness restored per tick.
pub const RESTORE: u64 = 2_000;
/// Uses after which a joke is retired.
pub const RETIRE: u64 = 5;
/// Beats a scene remembers.
pub const WINDOW: usize = 8;

/// Humour fit of two humour kinds (symmetric).
pub fn fit(a: &str, b: &str) -> i64 {
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    match (x, y) {
        ("dry", "dry") | ("absurd", "dry") => 15,
        ("dry", "warm") | ("cutting", "cutting") => 10,
        ("cutting", "dry") | ("none", "warm") => 5,
        ("warm", "warm") | ("absurd", "absurd") => 20,
        ("absurd", "warm") => 15,
        ("cutting", "warm") => -20,
        ("absurd", "cutting") => -10,
        ("absurd", "none") => -5,
        ("cutting", "none") => -15,
        _ => 0,
    }
}

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

fn sint(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(c::bad),
        _ => Err(c::bad()),
    }
}

fn one_of(v: &Value, options: &[&'static str]) -> Result<&'static str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    options.iter().copied().find(|x| *x == s).ok_or_else(c::bad)
}

fn pair_key(a: &str, b: &str) -> (String, String) {
    if a < b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    pub humour: &'static str,
    pub tempo: &'static str,
    pub formality: u64,
    pub assertiveness: u64,
    pub values: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joke {
    pub moment: String,
    pub freshness: u64,
    pub at: u64,
    pub uses: u64,
    pub retired: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pair {
    pub landmarks: u64,
    pub favors: u64,
    pub breaches: u64,
    pub jokes: BTreeMap<String, Joke>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Beat {
    pub at: u64,
    pub novelty: u64,
    pub engagement: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    pub tension: u64,
    pub last_delta: u64,
    pub silence: u64,
    pub beats: Vec<Beat>,
    pub threads: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chemistry {
    pub styles: BTreeMap<String, Style>,
    pub pairs: BTreeMap<(String, String), Pair>,
    pub scenes: BTreeMap<String, Scene>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Chemistry {
    fn default() -> Self {
        Chemistry {
            styles: BTreeMap::new(),
            pairs: BTreeMap::new(),
            scenes: BTreeMap::new(),
            clock: 0,
            limits: Limits::defaults(LIMITS),
        }
    }
}

impl Chemistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut k = Self::new();
        k.limits.apply(v)?;
        Ok(k)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "tick" => self.op_tick(args),
            "style" => self.op_style(args, l),
            "history" => self.op_history(args, l),
            "pair" => self.op_pair(args),
            "joke" => self.op_joke(args, l),
            "banter" => self.op_banter(args),
            "scene" => self.op_scene(args, l),
            "beat" => self.op_beat(args),
            "pace" => self.op_pace(args),
            "export" => persist::export_op(self, l, args),
            "import" => persist::import(self, l, args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn style(&self, b: &str) -> Result<&Style> {
        self.styles.get(b).ok_or_else(c::bad)
    }

    /// Chemistry of two styled beings and its components.
    pub fn chemistry(&self, a: &str, b: &str) -> Result<(i64, Value)> {
        let sa = self.style(a)?;
        let sb = self.style(b)?;
        let empty = Pair::default();
        let h = self.pairs.get(&pair_key(a, b)).unwrap_or(&empty);
        let humour = fit(sa.humour, sb.humour);
        let tempo = if sa.tempo == sb.tempo {
            10
        } else if (sa.tempo == "fast" && sb.tempo == "slow") || (sa.tempo == "slow" && sb.tempo == "fast") {
            -15
        } else {
            0
        };
        let formality = -((sa.formality.abs_diff(sb.formality) / 10) as i64);
        let assertive = if sa.assertiveness >= 70 && sb.assertiveness >= 70 {
            -10
        } else if sa.assertiveness.abs_diff(sb.assertiveness) >= 40 {
            5
        } else {
            0
        };
        let shared = sa.values.iter().filter(|v| sb.values.contains(v)).count() as i64;
        let values = (5 * shared).min(20);
        let history =
            (4 * h.landmarks).min(20) as i64 + (3 * h.favors).min(15) as i64 - (8 * h.breaches).min(40) as i64;
        let chem = (50 + humour + tempo + formality + assertive + values + history).clamp(0, 100);
        Ok((
            chem,
            obj! {
                "assertiveness" => assertive, "chemistry" => chem, "formality" => formality, "history" => history,
                "humour" => humour, "tempo" => tempo, "values" => values,
            },
        ))
    }

    /// A joke's freshness now (0 once retired).
    pub fn freshness(&self, j: &Joke) -> u64 {
        if j.retired {
            0
        } else {
            (j.freshness + RESTORE.saturating_mul(self.clock - j.at)).min(FULL)
        }
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    fn op_style(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "humour", "tempo", "formality", "assertiveness"], &["values"])?;
        let b = id_of(args, "being")?;
        let humour = one_of(c::get(args, "humour")?, &HUMOUR)?;
        let tempo = one_of(c::get(args, "tempo")?, &TEMPO)?;
        let formality = uint_bound(c::get(args, "formality")?, 0, 100)?;
        let assertiveness = uint_bound(c::get(args, "assertiveness")?, 0, 100)?;
        let mut values = Vec::new();
        if let Some(v) = opt(args, "values") {
            let rows = v.as_array().ok_or_else(c::bad)?;
            need(rows.len() <= 8)?;
            for x in rows {
                values.push(ident(x)?.to_string());
            }
        }
        need(values.iter().collect::<BTreeSet<_>>().len() == values.len())?;
        values.sort();
        need(self.styles.contains_key(&b) || self.styles.len() < l.get("beings"))?;
        self.styles.insert(b.clone(), Style { humour, tempo, formality, assertiveness, values: values.clone() });
        Ok(obj! {
            "assertiveness" => assertiveness, "being" => b, "formality" => formality, "humour" => humour,
            "tempo" => tempo, "values" => values,
        })
    }

    fn op_history(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["a", "b"], &["landmarks", "favors", "breaches"])?;
        let x = id_of(args, "a")?;
        let y = id_of(args, "b")?;
        need(x != y)?;
        let landmarks = opt_uint(args, "landmarks", 0, 1000, 0)?;
        let favors = opt_uint(args, "favors", 0, 1000, 0)?;
        let breaches = opt_uint(args, "breaches", 0, 1000, 0)?;
        let k = pair_key(&x, &y);
        need(self.pairs.contains_key(&k) || self.pairs.len() < l.get("pairs"))?;
        let p = self.pairs.entry(k.clone()).or_default();
        p.landmarks = landmarks;
        p.favors = favors;
        p.breaches = breaches;
        Ok(obj! { "a" => k.0, "b" => k.1, "landmarks" => landmarks, "favors" => favors, "breaches" => breaches })
    }

    fn op_pair(&self, args: &Value) -> Result<Value> {
        keys(args, &["a", "b"], &[])?;
        let x = id_of(args, "a")?;
        let y = id_of(args, "b")?;
        need(x != y)?;
        let (chem, components) = self.chemistry(&x, &y)?;
        let k = pair_key(&x, &y);
        let jokes: Vec<Value> = self
            .pairs
            .get(&k)
            .map(|p| {
                p.jokes
                    .iter()
                    .map(|(id, j)| {
                        obj! {
                            "freshness" => self.freshness(j), "id" => id.as_str(), "retired" => j.retired,
                            "uses" => j.uses,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let bias = (chem - 50).div_euclid(2);
        Ok(obj! {
            "a" => k.0, "b" => k.1, "chemistry" => chem, "components" => components,
            "cooperativeness_bias" => bias, "initiative_bias" => bias, "jokes" => jokes,
            "playful_lands_percent" => chem,
        })
    }

    fn op_joke(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["a", "b", "id", "moment"], &[])?;
        let x = id_of(args, "a")?;
        let y = id_of(args, "b")?;
        need(x != y)?;
        let jid = id_of(args, "id")?;
        let moment = text_bound(c::get(args, "moment")?, 80)?.to_string();
        self.style(&x)?;
        self.style(&y)?;
        let k = pair_key(&x, &y);
        match self.pairs.get(&k) {
            None => need(self.pairs.len() < l.get("pairs"))?,
            Some(p) => need(!p.jokes.contains_key(&jid) && p.jokes.len() < l.get("jokes"))?,
        }
        need(l.get("jokes") > 0)?;
        let clock = self.clock;
        let p = self.pairs.entry(k.clone()).or_default();
        p.jokes
            .insert(jid.clone(), Joke { moment: moment.clone(), freshness: FULL, at: clock, uses: 0, retired: false });
        Ok(obj! { "a" => k.0, "b" => k.1, "freshness" => FULL, "id" => jid, "moment" => moment })
    }

    fn op_banter(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["speaker", "listener", "mood", "pacing"], &["seed", "move"])?;
        let s = id_of(args, "speaker")?;
        let l = id_of(args, "listener")?;
        need(s != l)?;
        let mood = sint(c::get(args, "mood")?, -100, 100)?;
        let pacing = one_of(c::get(args, "pacing")?, &ADVICE)?;
        let seed = opt_uint(args, "seed", 0, MAX_AT, 0)?;
        let proposed = opt(args, "move").map(|v| one_of(v, &MOVES)).transpose()?;
        let (chem, _) = self.chemistry(&s, &l)?;
        let k = pair_key(&s, &l);
        let fresh: Vec<String> = self
            .pairs
            .get(&k)
            .map(|p| p.jokes.iter().filter(|(_, j)| self.freshness(j) >= FULL / 2).map(|(id, _)| id.clone()).collect())
            .unwrap_or_default();
        let humour = self.style(&s)?.humour;
        let mv = match proposed {
            Some(m) => {
                need(m != "callback" || !fresh.is_empty())?;
                m
            }
            None if pacing == "end" => "exit",
            None if mood < -30 => {
                if chem >= 40 {
                    "rapport_check"
                } else {
                    "exit"
                }
            }
            None if !fresh.is_empty() && pacing != "slow" => "callback",
            None if pacing == "escalate" && chem >= FLOOR => "escalate",
            None if (humour == "dry" || humour == "cutting") && chem >= FLOOR => "tease",
            None if pacing == "slow" || pacing == "pause" => "deflate",
            None if humour == "warm" => "disguised_compliment",
            None => "rapport_check",
        };
        let mut joke = Value::Null;
        if mv == "callback" {
            let jid = fresh[(fnv1a(&format!("{seed}:{s}:{l}:{}", self.clock)) % fresh.len() as u64) as usize].clone();
            let clock = self.clock;
            let now = self.freshness(&self.pairs[&k].jokes[&jid]);
            let j = self.pairs.get_mut(&k).and_then(|p| p.jokes.get_mut(&jid)).ok_or_else(c::bad)?;
            j.freshness = now.saturating_sub(SPEND);
            j.at = clock;
            j.uses += 1;
            j.retired = j.uses >= RETIRE;
            let (moment, retired, uses) = (j.moment.clone(), j.retired, j.uses);
            let fr = self.freshness(&self.pairs[&k].jokes[&jid]);
            joke = obj! { "freshness" => fr, "id" => jid, "moment" => moment, "retired" => retired, "uses" => uses };
        }
        let (lands, appraisal) = match mv {
            "tease" => (chem >= FLOOR, if chem >= FLOOR { "playful" } else { "insult" }),
            "escalate" | "callback" => (chem >= 50, if chem >= 50 { "playful" } else { "awkward" }),
            "disguised_compliment" => (true, "warm"),
            _ => (true, "neutral"),
        };
        Ok(obj! {
            "appraisal" => appraisal, "chemistry" => chem, "joke" => joke, "lands" => lands, "listener" => l,
            "move" => mv, "proposed" => proposed.is_some(), "speaker" => s,
        })
    }

    fn advice(&self, sid: &str, sc: &Scene) -> Value {
        let recent = &sc.beats[sc.beats.len().saturating_sub(4)..];
        let novelty =
            if recent.is_empty() { 0 } else { recent.iter().map(|b| b.novelty).sum::<u64>() / recent.len() as u64 };
        let falling = recent.len() >= 2 && recent[recent.len() - 1].engagement < recent[0].engagement;
        let dense = sc.beats.iter().filter(|b| self.clock - b.at <= 10).count() as u64;
        let adv = if !recent.is_empty() && sc.threads == 0 && novelty < 30 && falling {
            "end"
        } else if sc.last_delta >= 300_000 && sc.silence > 0 {
            "pause"
        } else if sc.tension >= 800_000 || dense >= 6 {
            "slow"
        } else if sc.tension < 200_000 && !recent.is_empty() && novelty >= 50 {
            "escalate"
        } else {
            "steady"
        };
        obj! {
            "advice" => adv, "density" => dense, "falling" => falling, "novelty" => novelty, "scene" => sid,
            "silence" => sc.silence, "tension" => sc.tension, "threads_open" => sc.threads,
        }
    }

    fn op_scene(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id"], &["tension", "silence"])?;
        let sid = id_of(args, "id")?;
        let tension = opt_uint(args, "tension", 0, FULL, 0)?;
        let silence = opt_uint(args, "silence", 0, 16, 3)?;
        need(!self.scenes.contains_key(&sid) && self.scenes.len() < l.get("scenes"))?;
        let sc = Scene { tension, last_delta: 0, silence, beats: Vec::new(), threads: 0 };
        let out = self.advice(&sid, &sc);
        self.scenes.insert(sid, sc);
        Ok(out)
    }

    fn op_beat(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["scene", "kind", "tension", "novelty", "engagement", "threads_open"], &[])?;
        let sid = id_of(args, "scene")?;
        need(self.scenes.contains_key(&sid))?;
        let kind = one_of(c::get(args, "kind")?, &BEATS)?;
        let d = sint(c::get(args, "tension")?, -100, 100)?;
        let novelty = uint_bound(c::get(args, "novelty")?, 0, 100)?;
        let engagement = uint_bound(c::get(args, "engagement")?, 0, 100)?;
        let threads = uint_bound(c::get(args, "threads_open")?, 0, 16)?;
        let clock = self.clock;
        let sc = self.scenes.get_mut(&sid).ok_or_else(c::bad)?;
        need(kind != "silence" || sc.silence > 0)?;
        let before = sc.tension;
        sc.tension = (before as i64 + d * 10_000).clamp(0, FULL as i64) as u64;
        sc.last_delta = sc.tension.abs_diff(before);
        if kind == "silence" {
            sc.silence -= 1;
        }
        sc.beats.push(Beat { at: clock, novelty, engagement });
        if sc.beats.len() > WINDOW {
            sc.beats.remove(0);
        }
        sc.threads = threads;
        let sc = self.scenes[&sid].clone();
        Ok(self.advice(&sid, &sc))
    }

    fn op_pace(&self, args: &Value) -> Result<Value> {
        keys(args, &["scene"], &[])?;
        let sid = id_of(args, "scene")?;
        let sc = self.scenes.get(&sid).ok_or_else(c::bad)?;
        Ok(self.advice(&sid, sc))
    }
}

persist!(Style { assertiveness [0, 100], formality [0, 100], humour in HUMOUR, tempo in TEMPO, values });
persist!(Joke { at [0, MAX_AT], freshness [0, FULL], moment, retired, uses [0, RETIRE] });
persist!(Pair { breaches [0, 1000], favors [0, 1000], jokes, landmarks [0, 1000] });
persist!(Beat { at [0, MAX_AT], engagement [0, 100], novelty [0, 100] });
persist!(Scene { beats, last_delta [0, FULL], silence [0, 16], tension [0, FULL], threads [0, 16] });
persist!(rest Chemistry { clock [0, MAX_AT], pairs, scenes, styles });
durable!(Chemistry, "chemistry", LIMITS);

impl Chemistry {
    /// Invariants a save must meet (the ops never produce anything else).
    fn settle_state(&mut self) -> Result<()> {
        within::<Self>("beings", self.styles.len())?;
        within::<Self>("pairs", self.pairs.len())?;
        within::<Self>("scenes", self.scenes.len())?;
        for (b, s) in &self.styles {
            ok(is_ident(b) && s.values.len() <= 8 && ascending(&s.values))?;
            ok(s.values.iter().all(|v| is_ident(v)))?;
        }
        for ((a, b), p) in &self.pairs {
            ok(is_ident(a) && is_ident(b) && a < b)?;
            within::<Self>("jokes", p.jokes.len())?;
            ok(p.jokes.is_empty() || (self.styles.contains_key(a) && self.styles.contains_key(b)))?;
            for (id, j) in &p.jokes {
                ok(is_ident(id) && is_text(&j.moment, 80) && j.at <= self.clock)?;
                ok(j.retired == (j.uses >= RETIRE))?;
            }
        }
        for (id, sc) in &self.scenes {
            ok(is_ident(id) && sc.beats.len() <= WINDOW)?;
            ok(sc.beats.windows(2).all(|w| w[0].at <= w[1].at) && sc.beats.iter().all(|b| b.at <= self.clock))?;
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

/// Runs one request on a fresh [`Chemistry`] (local kind `chemistry`).
pub fn workflow(request: &Value) -> Result<Value> {
    Chemistry::new().workflow(request)
}
