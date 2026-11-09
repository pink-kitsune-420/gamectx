//! Ambivalence, embodied micro-reactions and voice modulation (pass 3, stage 4).
//!
//! **Tension.** `tension` reads three sources of inner conflict: a motive near-tie (the top two
//! motives within 12 points and from different domains), ambivalence toward someone (affection
//! and resentment both at least 40, or respect and fear both at least 50), and a norm the chosen
//! action breaks (under duress or not), plus whether the being is deceiving. It returns one mode
//! (`none`, `reluctant_compliance`, `torn_hesitation`, `bittersweet_warmth`,
//! `resentful_obligation`, `guilty_concealment`) with its effects: extra task failure risk (per
//! mille), hesitation ticks and a speech tone.
//!
//! **Micro-reactions.** `react` picks a posture, a gaze and a hand activity from fear, anger,
//! shock, guilt, lying, cooperativeness, task state and bystanders by fixed first-match rules,
//! and says whether the turn should be silent (hostile refusal or extreme shock). It remembers
//! each being's last reaction and reports the channels repeated within a window, so the host can
//! vary its wording.
//!
//! **Voice.** `voice` compresses replies to one terse sentence under fatigue, pain, arousal or
//! task urgency, picks a pacing (breathless, trembling, hesitant, clipped, warm and relaxed, or
//! steady) and gives prosody hints (speed and volume per mille, pitch shift in tenths of a
//! semitone, pause in ms) for line selection or speech synthesis; a whisper lowers the volume.
//!
//! Capacities are [host-set limits](crate::character::limits). Pure host computation ($0 model
//! spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::embodiment::Embodiment;
//!
//! let mut e = Embodiment::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "tension", "args": {"being": "mira", "attitudes": [
//!     {"toward": "apprentice", "affection": 55, "resentment": 50}]}},
//!   {"op": "react", "args": {"being": "mira", "task": "urgent", "bystander_untrusted": true}},
//!   {"op": "voice", "args": {"fatigue": 75, "task_urgency": 90}}
//! ]}"#, 4096)?;
//! let out = e.workflow(&req)?.to_json();
//! // Fond but hurt: bittersweet. She keeps working, eyes on the gossip, in one clipped sentence.
//! assert!(out.contains(r#""mode":"bittersweet_warmth""#));
//! assert!(out.contains(r#""gaze":"watches_bystander","hand":"continues_working""#));
//! assert!(out.contains(r#""max_sentences":1,"pacing":"clipped""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_bool, opt_uint, uint_bound, MAX_AT};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "attitudes", default: 8, ceiling: 64 },
];
/// Tension modes with (failure risk per mille, hesitation ticks, tone).
pub const MODES: [(&str, u64, u64, &str); 6] = [
    ("none", 0, 0, "plain"),
    ("reluctant_compliance", 50, 1, "grudging"),
    ("torn_hesitation", 80, 2, "hesitant"),
    ("bittersweet_warmth", 0, 0, "tender"),
    ("resentful_obligation", 30, 0, "curt"),
    ("guilty_concealment", 60, 1, "evasive"),
];
pub const COOP: [&str; 6] = ["eager", "cooperative", "guarded", "reluctant", "deflecting", "hostile_refusal"];
/// Pacings with (speed per mille, pitch shift in tenths of a semitone, pause ms, volume per mille).
pub const PACING: [(&str, i64, i64, i64, i64); 6] = [
    ("steady", 1000, 0, 250, 1000),
    ("clipped", 1150, 0, 120, 1050),
    ("breathless", 1200, 20, 80, 900),
    ("hesitant", 850, -10, 600, 850),
    ("trembling", 1050, 30, 350, 750),
    ("warm_relaxed", 920, -10, 400, 950),
];
pub const VERBOSITY: [&str; 3] = ["terse", "normal", "full"];
pub const PRIVACY: [&str; 3] = ["deflect_due_to_bystanders", "open", "whispered"];
pub const WHISPER_VOLUME: i64 = 400;

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

fn one_of(args: &Value, k: &str, options: &[&'static str], default: &'static str) -> Result<&'static str> {
    match opt(args, k) {
        None => Ok(default),
        Some(v) => {
            let s = ident(v)?;
            options.iter().copied().find(|x| *x == s).ok_or_else(c::bad)
        }
    }
}

fn mode_names() -> Vec<&'static str> {
    MODES.iter().map(|m| m.0).collect()
}

/// A being's last micro-reaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Last {
    pub posture: Option<&'static str>,
    pub gaze: Option<&'static str>,
    pub hand: Option<&'static str>,
    pub at: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Embodiment {
    pub last: BTreeMap<String, Last>,
    pub clock: u64,
    pub limits: Limits,
}

impl Default for Embodiment {
    fn default() -> Self {
        Embodiment { last: BTreeMap::new(), clock: 0, limits: Limits::defaults(LIMITS) }
    }
}

impl Embodiment {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "tick" => self.op_tick(args),
            "tension" => self.op_tension(args, l),
            "react" => self.op_react(args, l),
            "voice" => self.op_voice(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn op_tick(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["now"], &[])?;
        let now = uint_bound(c::get(args, "now")?, 0, MAX_AT)?;
        need(now >= self.clock)?;
        self.clock = now;
        Ok(obj! { "now" => now })
    }

    // ------------------------------------------------------------ tension

    fn op_tension(&self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being"], &["motives", "attitudes", "norm_breach", "deceiving"])?;
        let b = id_of(args, "being")?;
        let near = match opt(args, "motives") {
            None => Value::Null,
            Some(mo) => {
                keys(mo, &["top", "runner_up"], &[])?;
                let mut rows = Vec::new();
                for k in ["top", "runner_up"] {
                    let m = c::get(mo, k)?;
                    keys(m, &["id", "domain", "score"], &[])?;
                    rows.push((id_of(m, "id")?, id_of(m, "domain")?, uint_bound(c::get(m, "score")?, 0, 100)?));
                }
                let (top, run) = (&rows[0], &rows[1]);
                need(top.0 != run.0 && run.2 <= top.2)?;
                if top.2 - run.2 <= 12 && top.1 != run.1 {
                    obj! { "gap" => top.2 - run.2, "runner_up" => run.0.as_str(), "top" => top.0.as_str() }
                } else {
                    Value::Null
                }
            }
        };
        let rows = list_bound(opt(args, "attitudes"), l.get("attitudes"))?;
        let (mut warm, mut cold, mut awe) = (false, false, false);
        let mut ambivalent = BTreeSet::new();
        let mut seen = BTreeSet::new();
        for x in rows {
            keys(x, &["toward"], &["affection", "resentment", "respect", "fear"])?;
            let w = id_of(x, "toward")?;
            need(seen.insert(w.clone()))?;
            let aff = opt_uint(x, "affection", 0, 100, 0)?;
            let res = opt_uint(x, "resentment", 0, 100, 0)?;
            let rsp = opt_uint(x, "respect", 0, 100, 0)?;
            let fear = opt_uint(x, "fear", 0, 100, 0)?;
            if aff >= 40 && res >= 40 {
                if aff >= res {
                    warm = true;
                } else {
                    cold = true;
                }
                ambivalent.insert(w);
            } else if rsp >= 50 && fear >= 50 {
                awe = true;
                ambivalent.insert(w);
            }
        }
        let breach = match opt(args, "norm_breach") {
            None => None,
            Some(nb) => {
                keys(nb, &["norm", "severity"], &["duress"])?;
                Some((opt_bool(nb, "duress", false)?, id_of(nb, "norm")?, uint_bound(c::get(nb, "severity")?, 1, 100)?))
            }
        };
        let deceiving = opt_bool(args, "deceiving", false)?;
        let mode = if deceiving || breach.as_ref().is_some_and(|x| !x.0) {
            "guilty_concealment"
        } else if breach.is_some() {
            "reluctant_compliance"
        } else if warm {
            "bittersweet_warmth"
        } else if cold {
            "resentful_obligation"
        } else if awe || near != Value::Null {
            "torn_hesitation"
        } else {
            "none"
        };
        let (_, risk, wait, tone) = MODES.iter().find(|m| m.0 == mode).copied().unwrap_or(MODES[0]);
        Ok(obj! {
            "ambivalent" => Value::Array(ambivalent.into_iter().map(Value::from).collect()),
            "being" => b,
            "conscience" => breach
                .map(|(d, n, s)| obj! { "duress" => d, "norm" => n, "severity" => s })
                .unwrap_or(Value::Null),
            "deceiving" => deceiving,
            "effects" => obj! { "failure_risk" => risk, "hesitation_ticks" => wait, "tone" => tone },
            "mode" => mode,
            "near_tie" => near,
        })
    }

    // ------------------------------------------------------------ micro-reactions

    fn op_react(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(
            args,
            &["being"],
            &[
                "fear",
                "anger",
                "shock",
                "guilt",
                "lying",
                "cooperativeness",
                "tension",
                "task",
                "holding",
                "offering",
                "threat_present",
                "bystander_untrusted",
                "window",
            ],
        )?;
        let b = id_of(args, "being")?;
        let fear = opt_uint(args, "fear", 0, 100, 0)?;
        let anger = opt_uint(args, "anger", 0, 100, 0)?;
        let shock = opt_uint(args, "shock", 0, 100, 0)?;
        let guilt = opt_uint(args, "guilt", 0, 100, 0)?;
        let lying = opt_bool(args, "lying", false)?;
        let coop = one_of(args, "cooperativeness", &COOP, "cooperative")?;
        let tension = one_of(args, "tension", &mode_names(), "none")?;
        let task = one_of(args, "task", &["none", "active", "urgent"], "none")?;
        let holding = opt_id(args, "holding")?;
        let offering = opt_id(args, "offering")?;
        let threat = opt_bool(args, "threat_present", false)?;
        let watcher = opt_bool(args, "bystander_untrusted", false)?;
        let window = opt_uint(args, "window", 0, MAX_AT, 3)?;
        need(self.last.contains_key(&b) || self.last.len() < l.get("beings"))?;

        let posture = if fear >= 70 {
            Some("steps_back")
        } else if anger >= 70 {
            Some("squares_shoulders")
        } else if shock >= 80 || coop == "hostile_refusal" {
            Some("turns_away")
        } else if guilt >= 60 || tension == "torn_hesitation" {
            Some("slumps")
        } else if coop == "eager" {
            Some("leans_in")
        } else {
            None
        };
        let gaze = if lying || guilt >= 50 || tension == "guilty_concealment" {
            Some("avoids_eye_contact")
        } else if fear >= 70 {
            Some("glances_at_exit")
        } else if watcher {
            Some("watches_bystander")
        } else if shock >= 80 && holding.is_some() {
            Some("stares_at_held_item")
        } else if coop == "eager" || coop == "cooperative" {
            Some("meets_eyes")
        } else {
            None
        };
        let hand = if threat && anger >= 70 {
            Some("reaches_for_weapon")
        } else if anger >= 50 {
            Some("clenches_fist")
        } else if offering.is_some() {
            Some("offers_item")
        } else if task == "urgent" {
            Some("continues_working")
        } else if task == "active" && shock >= 50 {
            Some("sets_down_tool")
        } else if task == "active" {
            Some("pauses_work")
        } else {
            None
        };
        let silent = coop == "hostile_refusal" || shock >= 90;
        let now = Last { posture, gaze, hand, at: self.clock };
        let mut repeated = Vec::new();
        if let Some(prev) = self.last.get(&b).filter(|p| self.clock - p.at <= window) {
            for (k, x, y) in
                [("gaze", now.gaze, prev.gaze), ("hand", now.hand, prev.hand), ("posture", now.posture, prev.posture)]
            {
                if x.is_some() && x == y {
                    repeated.push(Value::from(k));
                }
            }
        }
        let item = if hand == Some("offers_item") {
            offering.clone()
        } else if gaze == Some("stares_at_held_item") {
            holding.clone()
        } else {
            None
        };
        self.last.insert(b.clone(), now);
        Ok(obj! {
            "being" => b, "gaze" => gaze, "hand" => hand, "item" => item, "posture" => posture,
            "repeated" => Value::Array(repeated), "silent" => silent,
        })
    }

    // ------------------------------------------------------------ voice

    fn op_voice(&self, args: &Value) -> Result<Value> {
        keys(
            args,
            &[],
            &[
                "fatigue",
                "pain",
                "arousal",
                "task_urgency",
                "fear",
                "affection",
                "exertion",
                "safe_place",
                "deceiving",
                "tension",
                "base_sentences",
                "base_verbosity",
                "privacy",
            ],
        )?;
        let fatigue = opt_uint(args, "fatigue", 0, 100, 0)?;
        let pain = opt_uint(args, "pain", 0, 100, 0)?;
        let arousal = opt_uint(args, "arousal", 0, 100, 0)?;
        let urg = opt_uint(args, "task_urgency", 0, 100, 0)?;
        let fear = opt_uint(args, "fear", 0, 100, 0)?;
        let aff = opt_uint(args, "affection", 0, 100, 0)?;
        let exertion = opt_uint(args, "exertion", 0, 100, 0)?;
        let safe = opt_bool(args, "safe_place", false)?;
        let deceiving = opt_bool(args, "deceiving", false)?;
        let tension = one_of(args, "tension", &mode_names(), "none")?;
        let base_n = opt_uint(args, "base_sentences", 1, 8, 4)?;
        let base_v = one_of(args, "base_verbosity", &VERBOSITY, "normal")?;
        let privacy = one_of(args, "privacy", &PRIVACY, "open")?;
        let terse = fatigue >= 80 || pain >= 50 || arousal >= 75 || urg >= 80;
        let pacing = if exertion >= 70 {
            "breathless"
        } else if fear >= 70 {
            "trembling"
        } else if deceiving || tension == "torn_hesitation" || tension == "guilty_concealment" {
            "hesitant"
        } else if terse {
            "clipped"
        } else if aff >= 60 && safe {
            "warm_relaxed"
        } else {
            "steady"
        };
        let (_, speed, pitch, pause, mut volume) = PACING.iter().find(|p| p.0 == pacing).copied().unwrap_or(PACING[0]);
        if privacy == "whispered" {
            volume = WHISPER_VOLUME;
        }
        Ok(obj! {
            "max_sentences" => if terse { 1 } else { base_n }, "pacing" => pacing, "pause_ms" => pause,
            "pitch_shift" => pitch, "speed" => speed, "terse" => terse,
            "verbosity" => if terse { "terse" } else { base_v }, "volume" => volume,
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Embodiment::new().workflow(req)
}
