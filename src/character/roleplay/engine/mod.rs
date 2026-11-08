//! A universal, configurable roleplay engine for any game's AI characters.
//!
//! One [`RoleplayEngine`] speaks for one character. Each player message goes through a
//! fixed cascade of switchable stages ([`Feature`]): message chunks, out-of-character
//! notes, registered stage actions, prop interactions, stage directions, perception and
//! prop descriptions, the command grammar, disclosures, recall, greetings, social lines,
//! then a model hand-off. A stage answers only when one of its authored line rules
//! matches, so turning a stage off, or using `pack: "none"`, simply lets turns fall
//! through. Wording comes from a line book (the built-in English pack plus your own
//! phrases and lines). See `docs/characters/ROLEPLAY_ENGINE.md`.
//!
//! Nothing here executes game actions or calls a model: action outcomes are proposals of
//! host decisions that must be one of the turn's legal `choices`, and `model` outcomes
//! tell the host to compile a request. Replies count as heard only after
//! [`RoleplayEngine::delivered`] confirms the listener heard them in full.
//!
//! ```
//! use gamectx::character::roleplay::engine::{EngineConfig, Feature, RoleplayEngine};
//! use gamectx::obj;
//! let config = EngineConfig::new("baker", "Mira")?.with_feature(Feature::Model, false)?;
//! let mut engine = RoleplayEngine::new(config);
//! let scene = obj! {"id"=>"bakery","label"=>"the bakery","place"=>"workplace","atmosphere"=>"calm","visit"=>1};
//! let out = engine.turn(&obj! {"listener"=>"player_1","text"=>"Hello!","at"=>1,"token"=>1,"scene"=>scene})?;
//! assert_eq!(out.get("text").and_then(|v| v.as_str()), Some("Hello, I'm Mira; welcome to the bakery."));
//! engine.delivered(&obj! {"token"=>1,"listener"=>"player_1","text"=>"Hello, I'm Mira; welcome to the bakery.","heard"=>39})?;
//! # Ok::<(), gamectx::Error>(())
//! ```
mod config;
mod lines;
mod model;
pub use config::{EngineConfig, Feature};
pub use lines::VARS;
use model::{check_speech, compile, Compile};

use super::{
    conversation_route, expression_claim, expression_route_expanded, message_chunks, resolve_command, GreetingMemory,
};
use crate::character::sheet::CharacterSheet;
use crate::data::conditions::{int, Scalar, Vars};
use crate::knowledge::codex::{Codex, STATUSES};
use crate::knowledge::props::{decision, ident, line, normalize};
use crate::{checked as c, obj, Result, Value};
use lines::LineBook;

const PLACES: [&str; 5] = ["workplace", "social", "private", "other", "hazard"];
const ATMOSPHERES: [&str; 4] = ["calm", "busy", "tense", "emergency"];
const PENDING: usize = 16;
const LISTENERS: usize = 256;

fn keys(v: &Value, required: &[&str], optional: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(c::bad)?;
    if required.iter().any(|k| v.get(k).is_none())
        || rows.iter().any(|(k, _)| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(c::bad());
    }
    Ok(())
}
fn uint(v: &Value, k: &str, low: u64) -> Result<u64> {
    c::uint(c::get(v, k)?, low, u32::MAX as u64)
}
fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.get(k).filter(|x| *x != &Value::Null)
}
fn set(v: &mut Value, k: &str, value: Value) {
    if let Value::Object(rows) = v {
        match rows.iter_mut().find(|(key, _)| key == k) {
            Some(slot) => slot.1 = value,
            None => rows.push((k.to_owned(), value)),
        }
    }
}
fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_str).map(str::to_owned).collect()
}

fn coords(v: &Value) -> Result<Vec<f64>> {
    let list = c::list(v)?;
    if list.len() < 2 || list.len() > 3 {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for x in list {
        let n = match x {
            Value::Number(s) => s.parse::<f64>().ok().filter(|n| n.is_finite()).ok_or_else(c::bad)?,
            _ => return Err(c::bad()),
        };
        if !(-1_000_000.0..=1_000_000.0).contains(&n) {
            return Err(c::bad());
        }
        out.push(n);
    }
    Ok(out)
}

fn dimensions_val(v: &Value) -> Result<Vec<f64>> {
    if let Some(list) = v.as_array() {
        if list.len() != 3 {
            return Err(c::bad());
        }
        let mut out = Vec::new();
        for x in list {
            let n = match x {
                Value::Number(s) => {
                    s.parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0 && *n <= 100_000.0).ok_or_else(c::bad)?
                }
                _ => return Err(c::bad()),
            };
            out.push(n);
        }
        return Ok(out);
    }
    if let Some(map) = v.as_object() {
        if map.len() != 3 {
            return Err(c::bad());
        }
        let get_dim = |k: &str| -> Result<f64> {
            match map.iter().find(|(key, _)| key.as_str() == k).map(|(_, v)| v) {
                Some(Value::Number(s)) => {
                    s.parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0 && *n <= 100_000.0).ok_or_else(c::bad)
                }
                _ => Err(c::bad()),
            }
        };
        return Ok(vec![get_dim("width")?, get_dim("length")?, get_dim("height")?]);
    }
    Err(c::bad())
}

fn dist_val(v: &Value) -> Result<Option<f64>> {
    match v {
        Value::Number(s) => {
            let f = s.parse::<f64>().ok().filter(|n| n.is_finite()).ok_or_else(c::bad)?;
            if (0.0..=1000.0).contains(&f) {
                Ok(Some(f))
            } else {
                Err(c::bad())
            }
        }
        Value::String(s) => match s.as_str() {
            "near" => Ok(Some(1.0)),
            "conversational" => Ok(Some(3.0)),
            "far" => Ok(Some(10.0)),
            "distant" => Ok(Some(30.0)),
            _ => Err(c::bad()),
        },
        _ => Err(c::bad()),
    }
}

/// A turn audience: one id, or 1–16 distinct ids (a group the speaker is talking to).
fn audience_val(v: &Value) -> Result<()> {
    if let Value::Array(list) = v {
        if list.is_empty() || list.len() > 16 {
            return Err(c::bad());
        }
        let mut seen: Vec<&str> = Vec::new();
        for x in list {
            let id = ident(x)?;
            if seen.contains(&id) {
                return Err(c::bad());
            }
            seen.push(id);
        }
        return Ok(());
    }
    ident(v).map(|_| ())
}

fn calc_dist(d: Option<f64>, c1: Option<&[f64]>, c2: Option<&[f64]>) -> Option<f64> {
    if let Some(d) = d {
        return Some(d);
    }
    if let (Some(a), Some(b)) = (c1, c2) {
        if a.len() == b.len() {
            let sum: f64 = a.iter().zip(b.iter()).map(|(x, y)| (x - y) * (x - y)).sum();
            return Some(sum.sqrt());
        }
    }
    None
}

fn parse_vocative<'a>(
    line: &'a str,
    me_id: &'a str,
    me_name: &'a str,
    targets: &'a [Value],
    sheet: &'a CharacterSheet,
) -> Option<String> {
    let mut candidates: Vec<(&str, Vec<String>)> = Vec::new();
    candidates.push((me_id, vec![normalize(me_name), normalize(me_id), format!("the {}", normalize(me_id))]));
    for g in targets {
        if g.get("kind").and_then(Value::as_str) == Some("character") {
            if let Some(id) = g.get("id").and_then(Value::as_str) {
                let mut names = vec![normalize(id)];
                if let Some(list) = g.get("names").and_then(Value::as_array) {
                    for n in list {
                        if let Some(s) = n.as_str() {
                            names.push(normalize(s));
                        }
                    }
                }
                candidates.push((id, names));
            }
        }
    }
    for r in sheet.all("relationships") {
        if let (Some(w), Some(n)) = (r.get("with").and_then(Value::as_str), r.get("name").and_then(Value::as_str)) {
            if !candidates.iter().any(|c| c.0 == w) {
                candidates.push((w, vec![normalize(w), normalize(n)]));
            }
        }
    }

    let stripped = line.trim();
    if let Some(inner) = stripped.strip_prefix('*') {
        if let Some(end) = inner.find('*') {
            let act = normalize(&inner[..end]);
            for prefix in ["turns to", "turn to", "looks at", "look at", "to"] {
                if act == prefix || act.starts_with(&format!("{prefix} ")) {
                    let target_name = act[prefix.len()..].trim();
                    for (cid, names) in &candidates {
                        if names.iter().any(|n| n == target_name) {
                            return Some((*cid).to_owned());
                        }
                    }
                }
            }
        }
    }

    for sep in [',', ':'] {
        if let Some(idx) = line.find(sep) {
            let mut prefix = normalize(&line[..idx]);
            for opener in ["hey", "hi", "hello", "greetings"] {
                if prefix.starts_with(&format!("{opener} ")) {
                    prefix = prefix[opener.len() + 1..].trim().to_owned();
                    break;
                }
            }
            for (cid, names) in &candidates {
                if names.iter().any(|n| n == &prefix) {
                    return Some((*cid).to_owned());
                }
            }
        }
    }

    if let Some(idx) = line.rfind(',') {
        let tail = normalize(line[idx + 1..].trim().trim_end_matches(['.', '!', '?']));
        for (cid, names) in &candidates {
            if names.iter().any(|n| n == &tail) {
                return Some((*cid).to_owned());
            }
        }
    }

    None
}

/// One character's roleplay state machine: a config, encounter memory, per-listener
/// claims/recent replies/topic, and up to 16 replies awaiting delivery receipts.
#[derive(Clone, Debug)]
pub struct RoleplayEngine {
    config: EngineConfig,
    greetings: GreetingMemory,
    listeners: Vec<Value>,
    pending: Vec<Value>,
    /// Open model requests awaiting [`RoleplayEngine::reply`] (at most 16).
    asked: Vec<Value>,
}

struct Turn<'a> {
    who: &'a str,
    at: u64,
    token: u64,
    scene: &'a Value,
    recent: &'a [String],
}

impl RoleplayEngine {
    pub fn new(config: EngineConfig) -> Self {
        let greetings = GreetingMemory::new(&Self::greeting_config(&config)).expect("validated gap");
        Self { config, greetings, listeners: Vec::new(), pending: Vec::new(), asked: Vec::new() }
    }
    fn greeting_config(config: &EngineConfig) -> Value {
        obj! {"version"=>1,"long_gap_turns"=>config.setting(Feature::Greetings, "long_gap_turns").unwrap_or(12)}
    }
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Return all eligible interactions for props currently present in `scene` that
    /// are legal under `choices` (or all present interactions if `choices` is empty).
    pub fn eligible_prop_interactions(&self, scene: &Value, choices: &[Value]) -> Result<Vec<Value>> {
        let sc = scene.get("scene").unwrap_or(scene);
        let present_raw = list(sc.get("props"), 0, 64)?;
        let mut ids = Vec::new();
        for p in present_raw {
            let id = ident(c::get(p, "id")?)?;
            ids.push(id);
        }
        let eligible = self.config.props.eligible_interactions(&ids, choices);
        let mut out = Vec::new();
        for (prop, inter) in eligible {
            out.push(obj! {
                "prop" => prop,
                "interaction" => c::get(inter, "id")?.clone(),
                "decision" => c::get(inter, "decision")?.clone(),
                "narration" => c::get(inter, "narration")?.clone(),
            });
        }
        Ok(out)
    }

    /// Propose a prop interaction as a character action.
    ///
    /// Verifies the prop is registered, the interaction exists on the prop,
    /// and the interaction's decision is legal against `choices` (or returns `stage_action_blocked`).
    pub fn propose_interaction(&self, prop_id: &str, interaction_id: &str, choices: &[Value]) -> Result<Value> {
        self.config.props.get(prop_id).ok_or_else(c::bad)?;
        let x = self.config.props.find_interaction(prop_id, interaction_id).ok_or_else(c::bad)?;
        let d = c::get(x, "decision")?;
        let extra = vec![("prop", Value::str(prop_id)), ("interaction", c::get(x, "id")?.clone())];
        if d != &Value::Null && !choices.is_empty() && !choices.iter().any(|ch| ch.same(d)) {
            let base = Vars::new();
            return Ok(notice(&self.config.book, "props", "stage_action_blocked", &base, extra));
        }
        let mut out = vec![("route".to_owned(), Value::str("props")), ("kind".to_owned(), Value::str("action"))];
        out.extend(extra.into_iter().map(|(k, v)| (k.to_owned(), v)));
        out.push(("decision".into(), d.clone()));
        out.push(("narration".into(), c::get(x, "narration")?.clone()));
        Ok(Value::Object(out))
    }
    /// `{version, greetings, listeners, pending, asked}`; restore it with [`RoleplayEngine::restore`].
    pub fn checkpoint(&self) -> Value {
        obj! {"version"=>1,"greetings"=>self.greetings.checkpoint(),"listeners"=>self.listeners.clone(),"pending"=>self.pending.clone(),"asked"=>self.asked.clone()}
    }
    pub fn restore(config: EngineConfig, state: &Value) -> Result<Self> {
        crate::packet::request::bounded(state, 4_194_304)?;
        c::fields(state, &["version", "greetings", "listeners", "pending", "asked"])?;
        c::uint(c::get(state, "version")?, 1, 1)?;
        let greetings = GreetingMemory::recover(&Self::greeting_config(&config), c::get(state, "greetings")?)?;
        let me = config.character_id();
        let entries = c::get(state, "greetings")?.get("entries").and_then(Value::as_array).unwrap_or(&[]);
        if entries.iter().any(|e| e.get("actor").and_then(Value::as_str) != Some(me)) {
            return Err(c::bad());
        }
        let listeners = c::list(c::get(state, "listeners")?)?;
        if listeners.len() > LISTENERS {
            return Err(c::bad());
        }
        let max = config.setting(Feature::Disclosures, "max_preferences").unwrap_or(3) as usize;
        for (i, l) in listeners.iter().enumerate() {
            c::fields(l, &["id", "name", "preferences", "recent", "topic", "revealed"])?;
            let id = ident(c::get(l, "id")?)?;
            if id == me || listeners[..i].iter().any(|old| old.get("id") == l.get("id")) {
                return Err(c::bad());
            }
            if let Some(n) = opt(l, "name") {
                line(n, 40, 160)?;
            }
            let prefs = c::list(c::get(l, "preferences")?)?;
            if prefs.len() > max {
                return Err(c::bad());
            }
            let mut seen = Vec::new();
            for p in prefs {
                c::fields(p, &["value", "polarity"])?;
                let value = line(c::get(p, "value")?, 120, 240)?.to_lowercase();
                if !["like", "dislike", "prefer"].contains(&c::text(c::get(p, "polarity")?)?) || seen.contains(&value) {
                    return Err(c::bad());
                }
                seen.push(value);
            }
            let recent = c::list(c::get(l, "recent")?)?;
            if recent.len() > config.recent() {
                return Err(c::bad());
            }
            for r in recent {
                line(r, 240, 1024)?;
            }
            if let Some(t) = opt(l, "topic") {
                ident(t)?;
            }
            let revealed = c::list(c::get(l, "revealed")?)?;
            if revealed.len() > 32 {
                return Err(c::bad());
            }
            for (j, r) in revealed.iter().enumerate() {
                ident(r)?;
                if revealed[..j].contains(r) {
                    return Err(c::bad());
                }
            }
        }
        let pending = c::list(c::get(state, "pending")?)?;
        if pending.len() > PENDING {
            return Err(c::bad());
        }
        for (i, p) in pending.iter().enumerate() {
            c::fields(p, &["token", "listener", "text", "scene", "visit", "at", "greeting", "introduced"])?;
            uint(p, "token", 1)?;
            if pending[..i].iter().any(|old| old.get("token") == p.get("token")) {
                return Err(c::bad());
            }
            ident(c::get(p, "listener")?)?;
            line(c::get(p, "text")?, 240, 1024)?;
            ident(c::get(p, "scene")?)?;
            uint(p, "visit", 0)?;
            uint(p, "at", 0)?;
            let greeting = c::boolean(c::get(p, "greeting")?)?;
            if c::boolean(c::get(p, "introduced")?)? && !greeting {
                return Err(c::bad());
            }
        }
        let asked = c::list(c::get(state, "asked")?)?;
        if asked.len() > PENDING {
            return Err(c::bad());
        }
        for (i, a) in asked.iter().enumerate() {
            c::fields(a, &["token", "listener", "scene", "visit", "at"])?;
            uint(a, "token", 1)?;
            if pending.iter().chain(&asked[..i]).any(|old| old.get("token") == a.get("token")) {
                return Err(c::bad());
            }
            ident(c::get(a, "listener")?)?;
            ident(c::get(a, "scene")?)?;
            uint(a, "visit", 0)?;
            uint(a, "at", 0)?;
        }
        Ok(Self { config, greetings, listeners: listeners.to_vec(), pending: pending.to_vec(), asked: asked.to_vec() })
    }
    fn listener_mut(&mut self, who: &str) -> Result<&mut Value> {
        listener_mut(&mut self.listeners, who)
    }

    /// Route one player message. Atomic: on error nothing changes.
    ///
    /// Input `{listener, text, at, token, scene:{id,label,place,atmosphere,visit,sight?,sound?,props?},
    /// trust?, choices?, labels?, focus?, reason?, targets?}`. The outcome always starts with
    /// `route` and `kind` (`reply`, `action`, `clarify`, `direction`, `note`, `model` or `silent`).
    pub fn turn(&mut self, t: &Value) -> Result<Value> {
        let mut next = self.clone();
        let out = next.step(t)?;
        *self = next;
        Ok(out)
    }

    fn step(&mut self, t: &Value) -> Result<Value> {
        crate::packet::request::bounded(t, 1_048_576)?;
        keys(
            t,
            &["listener", "text", "at", "token", "scene"],
            &[
                "trust",
                "choices",
                "labels",
                "focus",
                "reason",
                "targets",
                "entrances",
                "needs",
                "quests",
                "state",
                "audience",
                "distance",
                "facing",
                "spot",
                "coords",
                "speaker_facing",
                "speaker_spot",
                "speaker_coords",
                "line_of_sight",
                "blocked_by",
                "behind",
                "dimensions",
                "features",
                "clothing",
                "sensory",
                "habits",
                "tags",
                "attributes",
                "posture",
                "activity",
                "awareness",
                "expression",
                "held",
                "gaze",
                "scent",
            ],
        )?;
        let RoleplayEngine { config, greetings, listeners, pending, asked } = self;
        let config: &EngineConfig = config;
        let (book, codex, sheet) = (&config.book, &config.codex, &config.sheet);
        let me = config.character_id();
        let name = config.character_name();
        let who = ident(c::get(t, "listener")?)?;
        if who == me {
            return Err(c::bad());
        }
        if let Some(aud) = opt(t, "audience") {
            audience_val(aud)?;
        }
        let t_dist = opt(t, "distance").map(dist_val).transpose()?.flatten();
        let my_coords = opt(t, "coords").map(coords).transpose()?;
        let sp_coords = opt(t, "speaker_coords").map(coords).transpose()?;
        let _my_dims = opt(t, "dimensions").map(dimensions_val).transpose()?;
        if let Some(f) = opt(t, "facing") {
            line(f, 48, usize::MAX)?;
        }
        if let Some(s) = opt(t, "spot") {
            ident(s)?;
        }
        if let Some(f) = opt(t, "speaker_facing") {
            line(f, 48, usize::MAX)?;
        }
        if let Some(s) = opt(t, "speaker_spot") {
            ident(s)?;
        }
        if let Some(los) = opt(t, "line_of_sight") {
            c::boolean(los)?;
        }
        if let Some(b) = opt(t, "blocked_by") {
            ident(b)?;
        }
        if let Some(b) = opt(t, "behind") {
            ident(b)?;
        }
        if let Some(f) = opt(t, "features") {
            crate::character::sheet::validate_features(f)?;
        }
        if let Some(cl) = opt(t, "clothing") {
            crate::character::sheet::validate_clothing(cl)?;
        }
        if let Some(sn) = opt(t, "sensory") {
            crate::character::sheet::validate_sensory(sn)?;
        }
        if let Some(h) = opt(t, "habits") {
            crate::character::sheet::validate_habits(h)?;
        }
        if let Some(tg) = opt(t, "tags") {
            crate::character::sheet::validate_tags(tg)?;
        }
        if let Some(att) = opt(t, "attributes") {
            crate::character::sheet::validate_attributes(att)?;
        }
        for k in ["posture", "activity", "awareness", "expression", "scent"] {
            if let Some(val) = opt(t, k) {
                line(val, 80, usize::MAX)?;
            }
        }
        if let Some(g) = opt(t, "gaze") {
            ident(g)?;
        }
        if let Some(held) = opt(t, "held") {
            if let Some(map) = held.as_object() {
                if map.len() > 8 {
                    return Err(c::bad());
                }
                for (k, val) in map {
                    line(&Value::str(k), 32, usize::MAX)?;
                    line(val, 64, usize::MAX)?;
                }
            } else if let Some(arr) = held.as_array() {
                if arr.len() > 8 {
                    return Err(c::bad());
                }
                for x in arr {
                    line(x, 64, usize::MAX)?;
                }
            } else {
                return Err(c::bad());
            }
        }
        let at = uint(t, "at", 0)?;
        let token = uint(t, "token", 1)?;
        if pending.iter().chain(asked.iter()).any(|p| p.get("token") == t.get("token")) {
            return Err(c::bad());
        }
        let sc = c::get(t, "scene")?;
        keys(
            sc,
            &["id", "label", "place", "atmosphere", "visit"],
            &["sight", "sound", "props", "floor", "ceiling", "exterior"],
        )?;
        let scene_id = ident(c::get(sc, "id")?)?;
        let label = line(c::get(sc, "label")?, 48, usize::MAX)?;
        uint(sc, "visit", 0)?;
        let place_kind = c::text(c::get(sc, "place")?)?;
        let atmosphere = c::text(c::get(sc, "atmosphere")?)?;
        if !PLACES.contains(&place_kind) || !ATMOSPHERES.contains(&atmosphere) {
            return Err(c::bad());
        }
        let sight = opt(sc, "sight").map(|v| line(v, 160, usize::MAX)).transpose()?;
        let scene_sound = opt(sc, "sound").map(|v| line(v, 160, usize::MAX)).transpose()?;
        let scene_floor = opt(sc, "floor").map(|v| line(v, 160, usize::MAX)).transpose()?;
        let scene_ceiling = opt(sc, "ceiling").map(|v| line(v, 160, usize::MAX)).transpose()?;
        if let Some(ext) = opt(sc, "exterior") {
            c::boolean(ext)?;
        }
        let no_list = Value::Array(Vec::new());
        let no_map = obj! {};
        let present_raw = list(sc.get("props"), 0, 64)?;
        struct PresentProp<'a> {
            id: &'a str,
            note: Option<&'a str>,
            line_of_sight: bool,
            _dimensions: Option<Vec<f64>>,
            material: Option<&'a str>,
            color: Option<&'a str>,
            texture: Option<&'a str>,
        }
        let mut present = Vec::<PresentProp<'_>>::new();
        for p in present_raw {
            keys(
                p,
                &["id", "note"],
                &[
                    "spot",
                    "facing",
                    "coords",
                    "occludes",
                    "line_of_sight",
                    "blocked_by",
                    "behind",
                    "dimensions",
                    "mass",
                    "state",
                    "material",
                    "color",
                    "texture",
                    "tags",
                    "attributes",
                ],
            )?;
            let id = ident(c::get(p, "id")?)?;
            if present.iter().any(|old| old.id == id) {
                return Err(c::bad());
            }
            if let Some(s) = opt(p, "spot") {
                ident(s)?;
            }
            if let Some(f) = opt(p, "facing") {
                line(f, 48, usize::MAX)?;
            }
            if let Some(cr) = opt(p, "coords") {
                coords(cr)?;
            }
            if let Some(occ) = opt(p, "occludes") {
                c::boolean(occ)?;
            }
            let p_los = opt(p, "line_of_sight").map(c::boolean).transpose()?.unwrap_or(true);
            if let Some(b) = opt(p, "blocked_by") {
                ident(b)?;
            }
            if let Some(b) = opt(p, "behind") {
                ident(b)?;
            }
            let p_dims = opt(p, "dimensions").map(dimensions_val).transpose()?;
            crate::character::sheet::validate_matter(p)?;
            if let Some(tg) = opt(p, "tags") {
                crate::character::sheet::validate_tags(tg)?;
            }
            if let Some(att) = opt(p, "attributes") {
                crate::character::sheet::validate_attributes(att)?;
            }
            present.push(PresentProp {
                id,
                note: opt(p, "note").map(|n| line(n, 160, usize::MAX)).transpose()?,
                line_of_sight: p_los,
                _dimensions: p_dims,
                material: opt(p, "material").and_then(Value::as_str),
                color: opt(p, "color").and_then(Value::as_str),
                texture: opt(p, "texture").and_then(Value::as_str),
            });
        }
        let trust = match t.get("trust") {
            None => 0,
            Some(v) => int(v).filter(|n| (-100..=100).contains(n)).ok_or_else(c::bad)?,
        };
        let choices = c::list(t.get("choices").unwrap_or(&no_list))?;
        if choices.len() > 128 {
            return Err(c::bad());
        }
        for (i, d) in choices.iter().enumerate() {
            if d == &Value::Null || choices[..i].iter().any(|old| old.same(d)) {
                return Err(c::bad());
            }
            decision(d)?;
        }
        let labels = c::list(t.get("labels").unwrap_or(&no_list))?;
        if labels.len() > 32 {
            return Err(c::bad());
        }
        for (i, l) in labels.iter().enumerate() {
            c::fields(l, &["id", "aliases"])?;
            ident(c::get(l, "id")?)?;
            if labels[..i].iter().any(|old| old.get("id") == l.get("id")) {
                return Err(c::bad());
            }
            let aliases = c::list(c::get(l, "aliases")?)?;
            if aliases.len() > 4 {
                return Err(c::bad());
            }
            for a in aliases {
                line(a, 48, 96)?;
            }
        }
        let focus = opt(t, "focus");
        if let Some(f) = focus {
            ident(f)?;
        }
        let reason = opt(t, "reason").map(|v| line(v, 240, usize::MAX)).transpose()?;
        let targets = c::list(t.get("targets").unwrap_or(&no_list))?;
        if targets.len() > 64 {
            return Err(c::bad());
        }
        for (i, g) in targets.iter().enumerate() {
            keys(
                g,
                &["kind", "id", "names"],
                &[
                    "distance",
                    "spot",
                    "facing",
                    "coords",
                    "occludes",
                    "line_of_sight",
                    "blocked_by",
                    "behind",
                    "dimensions",
                    "mass",
                    "state",
                    "material",
                    "color",
                    "texture",
                    "tags",
                    "attributes",
                    "features",
                    "clothing",
                    "sensory",
                    "habits",
                    "posture",
                    "activity",
                    "awareness",
                    "expression",
                    "held",
                    "gaze",
                    "scent",
                ],
            )?;
            if !["character", "place"].contains(&c::text(c::get(g, "kind")?)?) {
                return Err(c::bad());
            }
            ident(c::get(g, "id")?)?;
            if targets[..i].iter().any(|old| old.get("kind") == g.get("kind") && old.get("id") == g.get("id")) {
                return Err(c::bad());
            }
            if let Some(d) = opt(g, "distance") {
                dist_val(d)?;
            }
            if let Some(s) = opt(g, "spot") {
                ident(s)?;
            }
            if let Some(f) = opt(g, "facing") {
                line(f, 48, usize::MAX)?;
            }
            if let Some(cr) = opt(g, "coords") {
                coords(cr)?;
            }
            if let Some(occ) = opt(g, "occludes") {
                c::boolean(occ)?;
            }
            if let Some(los) = opt(g, "line_of_sight") {
                c::boolean(los)?;
            }
            if let Some(b) = opt(g, "blocked_by") {
                ident(b)?;
            }
            if let Some(b) = opt(g, "behind") {
                ident(b)?;
            }
            if let Some(d) = opt(g, "dimensions") {
                dimensions_val(d)?;
            }
            crate::character::sheet::validate_matter(g)?;
            if let Some(f) = opt(g, "features") {
                crate::character::sheet::validate_features(f)?;
            }
            if let Some(cl) = opt(g, "clothing") {
                crate::character::sheet::validate_clothing(cl)?;
            }
            if let Some(st) = opt(g, "state") {
                crate::character::sheet::validate_char_state(st)?;
            }
            if let Some(sn) = opt(g, "sensory") {
                crate::character::sheet::validate_sensory(sn)?;
            }
            if let Some(h) = opt(g, "habits") {
                crate::character::sheet::validate_habits(h)?;
            }
            if let Some(tg) = opt(g, "tags") {
                crate::character::sheet::validate_tags(tg)?;
            }
            if let Some(att) = opt(g, "attributes") {
                crate::character::sheet::validate_attributes(att)?;
            }
            for k in ["posture", "activity", "awareness", "expression", "scent"] {
                if let Some(val) = opt(g, k) {
                    line(val, 80, usize::MAX)?;
                }
            }
            if let Some(gz) = opt(g, "gaze") {
                ident(gz)?;
            }
            let names = c::list(c::get(g, "names")?)?;
            if names.is_empty() || names.len() > 8 {
                return Err(c::bad());
            }
            for n in names {
                if normalize(line(n, 48, usize::MAX)?).is_empty() {
                    return Err(c::bad());
                }
            }
        }
        let place = codex.place(scene_id);
        let mut visible = Vec::<Exit>::new();
        let mut listed = Vec::new();
        for x in list(t.get("entrances"), 0, 64)? {
            keys(x, &["id", "status"], &["note"])?;
            let id = ident(c::get(x, "id")?)?;
            if listed.contains(&id) {
                return Err(c::bad());
            }
            listed.push(id);
            let status = c::text(c::get(x, "status")?)?;
            if !STATUSES.contains(&status) {
                return Err(c::bad());
            }
            let note = opt(x, "note").map(|n| line(n, 160, usize::MAX)).transpose()?;
            let index = codex.entrances().iter().position(|e| e.get("id") == x.get("id")).ok_or_else(c::bad)?;
            let def = &codex.entrances()[index];
            let sides = def.get("sides").and_then(Value::as_array).unwrap_or(&[]);
            let side_index = sides
                .iter()
                .position(|s| s.get("place").and_then(Value::as_str) == Some(scene_id))
                .ok_or_else(c::bad)?;
            let other = sides.iter().enumerate().find(|(i, _)| *i != side_index).map(|(_, s)| s);
            if status != "hidden" {
                visible.push(Exit {
                    id,
                    label: def.get("label").and_then(Value::as_str).unwrap_or_default(),
                    status,
                    note,
                    def,
                    side: &sides[side_index],
                    first: side_index == 0,
                    other_place: other.and_then(|s| s.get("place")).and_then(Value::as_str),
                    names: codex.names("entrances", index),
                });
            }
        }
        let needs = t.get("needs").unwrap_or(&no_map).as_object().ok_or_else(c::bad)?;
        if needs.len() > 16 {
            return Err(c::bad());
        }
        for (k, n) in needs {
            if !sheet.all("needs").iter().any(|x| x.get("id").and_then(Value::as_str) == Some(k.as_str())) {
                return Err(c::bad());
            }
            c::uint(n, 0, 100)?;
        }
        let quests = t.get("quests").unwrap_or(&no_map).as_object().ok_or_else(c::bad)?;
        if quests.len() > 64 {
            return Err(c::bad());
        }
        for (k, st) in quests {
            let q = codex.quest(k).ok_or_else(c::bad)?;
            let st = c::text(st)?;
            if !q
                .get("stages")
                .and_then(Value::as_array)
                .unwrap_or(&[])
                .iter()
                .any(|s| s.get("id").and_then(Value::as_str) == Some(st))
            {
                return Err(c::bad());
            }
        }
        let flags = t.get("state").unwrap_or(&no_map).as_object().ok_or_else(c::bad)?;
        if flags.len() > 64 {
            return Err(c::bad());
        }
        let mut flag_vars = Vec::new();
        for (k, x) in flags {
            ident(&Value::str(k))?;
            flag_vars.push((format!("state.{k}"), Scalar::from_flag(x)?));
        }
        let enabled = Feature::ALL.map(|f| config.enabled(f));
        let on = |f: Feature| Feature::ALL.iter().position(|x| *x == f).is_some_and(|i| enabled[i]);
        let present = if on(Feature::Props) {
            if present.iter().any(|p| config.props.get(p.id).is_none()) {
                return Err(c::bad());
            }
            present
        } else {
            Vec::new()
        };

        let raw = c::text(c::get(t, "text")?)?;
        let (input, long) = if on(Feature::Chunks) {
            let max = config.setting(Feature::Chunks, "max_chars").unwrap_or(1024);
            let sliced = message_chunks(&obj! {"op"=>"player","args"=>obj!{"text"=>raw,"max_chars"=>max}})?;
            let chunks = c::list(c::get(&sliced, "chunks")?)?;
            (chunks.iter().filter_map(Value::as_str).collect::<String>(), chunks.len() > 1)
        } else {
            (line(c::get(t, "text")?, 1024, 1024)?.to_owned(), false)
        };
        let input = input.as_str();
        let q = normalize(input);
        let stripped = input.trim();
        let starred = stripped.chars().count() >= 3 && stripped.starts_with('*') && stripped.ends_with('*');
        let key = if starred { normalize(&stripped[1..stripped.len() - 1]) } else { q.clone() };
        let contains = |ps: &[String]| {
            let padded = format!(" {q} ");
            ps.iter().any(|p| padded.contains(&format!(" {p} ")))
        };
        let relationship = if trust < 0 {
            "guarded"
        } else if trust >= 25 {
            "warm"
        } else {
            "reserved"
        };
        let mut base = Vars::default();
        base.text("name", name)
            .text("scene", label)
            .text("place", place_kind)
            .text("atmosphere", atmosphere)
            .int("trust", trust)
            .text("relationship", relationship);
        if let Some(f) = opt(t, "facing").and_then(Value::as_str) {
            base.text("facing", f);
        }
        if let Some(s) = opt(t, "spot").and_then(Value::as_str) {
            base.text("spot", s);
        }
        let feat = opt(t, "features").or_else(|| sheet.features());
        if let Some(f) = feat {
            if let Some(eyes) = f.get("eyes").and_then(Value::as_str) {
                base.text("eyes", eyes);
            }
            if let Some(hair) = f.get("hair").and_then(Value::as_str) {
                base.text("hair", hair);
            }
            if let Some(skin) = f.get("skin").and_then(Value::as_str) {
                base.text("skin", skin);
            }
        }
        let clothing_owned: Vec<String> = if let Some(cl) = opt(t, "clothing") {
            if let Some(s) = cl.as_str() {
                vec![s.to_owned()]
            } else {
                strings(Some(cl))
            }
        } else {
            sheet.clothing().into_iter().map(str::to_owned).collect()
        };
        let clothing_refs: Vec<&str> = clothing_owned.iter().map(String::as_str).collect();
        base.flag("has_clothing", !clothing_refs.is_empty());
        if !clothing_refs.is_empty() {
            if let Some(j) = book.joined(&clothing_refs).filter(|j| j.chars().count() <= 240) {
                base.text("clothing", &j);
            }
        }
        let posture =
            opt(t, "posture").or_else(|| sheet.char_state().and_then(|s| s.get("posture"))).and_then(Value::as_str);
        base.flag("has_posture", posture.is_some());
        if let Some(p) = posture {
            base.text("posture", p);
        }
        let held_owned: Vec<String> = if let Some(h) = opt(t, "held") {
            if let Some(s) = h.as_str() {
                vec![s.to_owned()]
            } else if let Some(arr) = h.as_array() {
                arr.iter().filter_map(Value::as_str).map(str::to_owned).collect()
            } else if let Some(obj) = h.as_object() {
                obj.iter().filter_map(|(_, v)| v.as_str()).map(str::to_owned).collect()
            } else {
                Vec::new()
            }
        } else if let Some(st) = opt(t, "state").or_else(|| sheet.char_state()) {
            if let Some(h) = st.get("held") {
                if let Some(s) = h.as_str() {
                    vec![s.to_owned()]
                } else if let Some(arr) = h.as_array() {
                    arr.iter().filter_map(Value::as_str).map(str::to_owned).collect()
                } else if let Some(obj) = h.as_object() {
                    obj.iter().filter_map(|(_, v)| v.as_str()).map(str::to_owned).collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let held_refs: Vec<&str> = held_owned.iter().map(String::as_str).collect();
        base.flag("has_held", !held_refs.is_empty());
        if !held_refs.is_empty() {
            if let Some(j) = book.joined(&held_refs).filter(|j| j.chars().count() <= 240) {
                base.text("held", &j);
            }
        }
        let gaze = opt(t, "gaze").or_else(|| sheet.char_state().and_then(|s| s.get("gaze"))).and_then(Value::as_str);
        base.flag("has_gaze", gaze.is_some());
        if let Some(g) = gaze {
            base.text("gaze", g);
        }
        let activity =
            opt(t, "activity").or_else(|| sheet.char_state().and_then(|s| s.get("activity"))).and_then(Value::as_str);
        base.flag("has_activity", activity.is_some());
        if let Some(a) = activity {
            base.text("activity", a);
        }
        let awareness =
            opt(t, "awareness").or_else(|| sheet.char_state().and_then(|s| s.get("awareness"))).and_then(Value::as_str);
        if let Some(a) = awareness {
            base.text("awareness", a);
        }
        let expr = opt(t, "expression")
            .or_else(|| sheet.char_state().and_then(|s| s.get("expression")))
            .and_then(Value::as_str);
        if let Some(e) = expr {
            base.text("expression", e);
        }
        let scent = opt(t, "scent")
            .or_else(|| opt(t, "sensory").and_then(|s| s.get("scent")))
            .or_else(|| sheet.sensory().and_then(|s| s.get("scent")))
            .and_then(Value::as_str);
        if let Some(s) = scent {
            base.text("scent", s);
        }
        for (k, v) in flag_vars {
            base.set(&k, v);
        }
        if on(Feature::Goals) {
            let mut best: Option<(u64, &str)> = None;
            for n in sheet.all("needs") {
                let id = n.get("id").and_then(Value::as_str).unwrap_or_default();
                let Some(level) = needs.iter().find(|(k, _)| k == id).and_then(|(_, v)| v.as_usize()) else { continue };
                let level = level as u64;
                let threshold = n.get("threshold").and_then(Value::as_usize).unwrap_or(0) as u64;
                if level >= threshold && best.is_none_or(|(b, _)| level > b) {
                    best = Some((level, n.get("label").and_then(Value::as_str).unwrap_or_default()));
                }
            }
            base.flag("has_need", best.is_some());
            if let Some((_, l)) = best {
                base.text("need", l);
            }
        }
        let soon = config.setting(Feature::Events, "soon_turns").unwrap_or(10);
        let timing = |e: &Value| {
            let start = e.get("start").and_then(Value::as_usize).unwrap_or(0) as u64;
            let end = e.get("end").and_then(Value::as_usize).unwrap_or(0) as u64;
            if at < start {
                if start - at <= soon {
                    "soon"
                } else {
                    "later"
                }
            } else if at <= end {
                "now"
            } else {
                "over"
            }
        };
        let floor =
            scene_floor.or_else(|| if on(Feature::Scenery) { codex.ambience(scene_id, "floor", &base) } else { None });
        let ceiling =
            scene_ceiling
                .or_else(|| if on(Feature::Scenery) { codex.ambience(scene_id, "ceiling", &base) } else { None });
        let exterior = opt(sc, "exterior")
            .map(c::boolean)
            .transpose()?
            .or_else(|| place.and_then(|p| p.get("exterior")).and_then(Value::as_bool))
            .unwrap_or(false);
        base.flag("exterior", exterior).flag("has_floor", floor.is_some()).flag("has_ceiling", ceiling.is_some());
        if let Some(f) = floor {
            base.text("floor", f);
        }
        if let Some(c) = ceiling {
            base.text("ceiling", c);
        }
        let ambience = |pl: Option<&str>, k: &str| pl.and_then(|p| codex.ambience(p, k, &base));
        let normed = |mut out: Value| -> Value {
            let d = out.get("decision").cloned().unwrap_or(Value::Null);
            if on(Feature::Scenery) && d != Value::Null {
                for n in place.and_then(|p| p.get("norms")).and_then(Value::as_array).unwrap_or(&[]) {
                    let hit = n.get("actions").and_then(Value::as_array).unwrap_or(&[]).iter().any(|a| {
                        a.get("action") == d.get("action")
                            && (a.get("target") == Some(&Value::Null) || a.get("target") == d.get("target"))
                    });
                    if hit {
                        set(&mut out, "norm", obj! {"id"=>n.get("id").cloned(),"reaction"=>n.get("reaction").cloned()});
                        break;
                    }
                }
            }
            out
        };
        let recent = strings(find(listeners, who).and_then(|l| l.get("recent")));
        let turn = Turn { who, at, token, scene: sc, recent: &recent };
        let mut expressive: Option<Value> = None;
        let mut expression = |input: &str| -> Result<Value> {
            if expressive.is_none() {
                expressive = Some(expression_route_expanded(input)?);
            }
            Ok(expressive.clone().unwrap_or(Value::Null))
        };
        let say = |pending: &mut Vec<Value>, route: &str, intent: &str, v: &Vars, extra: Vec<(&str, Value)>| {
            reply(book, pending, &turn, route, intent, v, extra, false)
        };

        if on(Feature::OutOfCharacter) {
            let e = expression(input)?;
            if e.get("mode").and_then(Value::as_str) == Some("ooc") {
                return Ok(obj! {"route"=>"out_of_character","kind"=>"note","note"=>c::get(&e,"note")?.clone()});
            }
        }
        if on(Feature::Addressing) {
            let my_dist = calc_dist(t_dist, my_coords.as_deref(), sp_coords.as_deref());
            let max_dist = config.setting(Feature::Addressing, "max_distance").unwrap_or(20) as f64;
            if my_dist.is_some_and(|d| d > max_dist) {
                return Ok(obj! {"route"=>"addressing","kind"=>"silent","reason"=>"out_of_earshot"});
            }

            let mut inferred = false;
            let audience = if let Some(Value::Array(group)) = opt(t, "audience") {
                if group.iter().any(|x| x.as_str() == Some(me)) {
                    Value::str(me)
                } else {
                    Value::Array(group.clone())
                }
            } else if let Some(aud) = opt(t, "audience").and_then(Value::as_str) {
                Value::str(aud)
            } else if let Some(voc) = parse_vocative(input, me, name, targets, sheet) {
                Value::str(&voc)
            } else {
                inferred = true;
                let conv_dist = config.setting(Feature::Addressing, "conversational_distance").unwrap_or(5) as f64;
                let mut closest_id = me.to_owned();
                let mut closest_dist = my_dist.unwrap_or(conv_dist);
                if let Some(sp_facing) = opt(t, "speaker_facing").and_then(Value::as_str) {
                    let sp_norm = normalize(sp_facing);
                    if sp_norm == normalize(name) || sp_norm == normalize(me) {
                        closest_id = me.to_owned();
                        closest_dist = -1.0;
                    }
                }
                for g in targets {
                    if g.get("kind").and_then(Value::as_str) == Some("character") {
                        if let Some(gid) = g.get("id").and_then(Value::as_str) {
                            if let Some(sp_facing) = opt(t, "speaker_facing").and_then(Value::as_str) {
                                let sp_norm = normalize(sp_facing);
                                let mut gnames = vec![normalize(gid)];
                                if let Some(list) = g.get("names").and_then(Value::as_array) {
                                    for n in list {
                                        if let Some(s) = n.as_str() {
                                            gnames.push(normalize(s));
                                        }
                                    }
                                }
                                if gnames.iter().any(|n| n == &sp_norm) {
                                    closest_id = gid.to_owned();
                                    closest_dist = -1.0;
                                    continue;
                                }
                            }
                            let gd_val = g.get("distance").map(dist_val).transpose()?.flatten();
                            let g_coords = g.get("coords").map(coords).transpose()?;
                            let gd = calc_dist(gd_val, g_coords.as_deref(), sp_coords.as_deref());
                            if let Some(gd) = gd {
                                if gd < closest_dist {
                                    closest_dist = gd;
                                    closest_id = gid.to_owned();
                                }
                            }
                        }
                    }
                }
                Value::str(&closest_id)
            };

            if audience.as_str() == Some(me) {
                let conv_dist = config.setting(Feature::Addressing, "conversational_distance").unwrap_or(5) as f64;
                if inferred && my_dist.is_some_and(|d| d > conv_dist) {
                    return Ok(obj! {"route"=>"addressing","kind"=>"gesture","reason"=>"too_far","audience"=>me});
                }
                let stripped = input.trim();
                if stripped.ends_with("...")
                    || stripped.ends_with('…')
                    || stripped.ends_with("--")
                    || stripped.ends_with('—')
                    || stripped.ends_with('–')
                    || stripped.ends_with(',')
                {
                    return Ok(obj! {"route"=>"addressing","kind"=>"pause","reason"=>"unfinished"});
                }
            } else {
                let action_turns = config.setting(Feature::Addressing, "action_turns").unwrap_or(3);
                let quirks = sheet.quirks();
                if action_turns > 0 && at % action_turns == 0 && !quirks.is_empty() {
                    let idx = ((at / action_turns) as usize) % quirks.len();
                    return Ok(obj! {
                        "route"=>"addressing",
                        "kind"=>"action",
                        "audience"=>audience,
                        "action"=>"quirk",
                        "decision"=>Value::Null,
                        "narration"=>quirks[idx]
                    });
                }
                return Ok(obj! {"route"=>"addressing","kind"=>"silent","audience"=>audience});
            }
        }
        if on(Feature::Content) {
            if let Some((id, _)) = config.avoid.iter().find(|(_, ps)| contains(ps)) {
                let extra = vec![("topic", Value::str(id))];
                return Ok(say(pending, "content", "content_redirect", &base, extra.clone())
                    .unwrap_or_else(|| notice(book, "content", "content_redirect", &base, extra)));
            }
        }
        if on(Feature::StageActions) && starred && stripped.len() <= 1024 {
            if opt(t, "line_of_sight") == Some(&Value::Bool(false)) {
                return Ok(obj! {"route"=>"stage_action","kind"=>"silent","reason"=>"occluded"});
            }
            let s = config.actions.resolve(stripped, &Value::Array(choices.to_vec()))?;
            match s.get("status").and_then(Value::as_str) {
                Some("proposed") => {
                    return Ok(normed(
                        obj! {"route"=>"stage_action","kind"=>"action","id"=>c::get(&s,"id")?.clone(),"decision"=>c::get(&s,"decision")?.clone(),"narration"=>c::get(&s,"narration")?.clone()},
                    ))
                }
                Some("blocked") => {
                    return Ok(notice(
                        book,
                        "stage_action",
                        "stage_action_blocked",
                        &base,
                        vec![("id", c::get(&s, "id")?.clone())],
                    ))
                }
                _ => {}
            }
        }
        if on(Feature::Props) {
            let ids = present.iter().map(|p| p.id).collect::<Vec<_>>();
            if let Some((prop, x)) = config.props.interaction(&ids, input) {
                let extra = vec![("prop", Value::str(prop)), ("interaction", c::get(x, "id")?.clone())];
                let d = c::get(x, "decision")?;
                if d != &Value::Null && !choices.iter().any(|ch| ch.same(d)) {
                    return Ok(notice(book, "props", "stage_action_blocked", &base, extra));
                }
                let mut out =
                    vec![("route".to_owned(), Value::str("props")), ("kind".to_owned(), Value::str("action"))];
                out.extend(extra.into_iter().map(|(k, v)| (k.to_owned(), v)));
                out.push(("decision".into(), d.clone()));
                out.push(("narration".into(), c::get(x, "narration")?.clone()));
                return Ok(normed(Value::Object(out)));
            }
        }
        if on(Feature::Entrances) {
            if let Some(rest) = book.matcher("traverse").and_then(|m| m.hit(&key)).filter(|r| !r.is_empty()) {
                let want = book.bare(&rest);
                let hits = visible.iter().filter(|e| e.names.iter().any(|n| book.bare(n) == want)).collect::<Vec<_>>();
                if let [e] = hits[..] {
                    let one_way = e.def.get("passage").and_then(Value::as_str) == Some("one_way");
                    let d = e.side.get("travel").filter(|d| *d != &Value::Null && !(one_way && !e.first));
                    let extra = vec![("entrance", Value::str(e.id))];
                    match d {
                        Some(d) if e.status == "open" && choices.iter().any(|ch| ch.same(d)) => {
                            return Ok(normed(
                                obj! {"route"=>"entrances","kind"=>"action","entrance"=>e.id,"decision"=>d.clone()},
                            ));
                        }
                        _ => {
                            let mut v = base.clone();
                            v.text("label", e.label).text("status", e.status);
                            if let Some(dest) = e.other_place.and_then(|p| codex.place_label(p)) {
                                v.text("destination", dest);
                            }
                            return Ok(notice(book, "entrances", "entrance_blocked", &v, extra));
                        }
                    }
                }
            }
        }
        if on(Feature::Services) {
            for (x, (ps, when)) in sheet.all("services").iter().zip(&sheet.services) {
                if !ps.contains(&key) || !when.holds(&base) {
                    continue;
                }
                let d = x.get("decision").cloned().unwrap_or(Value::Null);
                let extra = vec![("service", c::get(x, "id")?.clone())];
                if d != Value::Null && !choices.iter().any(|ch| ch.same(&d)) {
                    return Ok(notice(book, "services", "service_unavailable", &base, extra));
                }
                return Ok(normed(
                    obj! {"route"=>"services","kind"=>"action","service"=>c::get(x,"id")?.clone(),"decision"=>d},
                ));
            }
        }
        if on(Feature::Directions) && starred {
            let body = normalize(&stripped[1..stripped.len() - 1]);
            for (verb, kind) in [("approach", "character"), ("enter", "place")] {
                let Some(rest) = book.matcher(verb).and_then(|m| m.hit(&body)) else { continue };
                let want = book.bare(&rest);
                let mut hits = targets
                    .iter()
                    .filter(|g| {
                        g.get("kind").and_then(Value::as_str) == Some(kind)
                            && !rest.is_empty()
                            && strings(g.get("names")).iter().any(|n| book.bare(&normalize(n)) == want)
                    })
                    .filter_map(|g| g.get("id").cloned())
                    .collect::<Vec<_>>();
                let targets_found = hits.len();
                if verb == "approach" && on(Feature::Scenery) && !rest.is_empty() {
                    for s in place.and_then(|p| p.get("spots")).and_then(Value::as_array).unwrap_or(&[]) {
                        if spot_names(s).iter().any(|n| book.bare(n) == want) {
                            hits.extend(s.get("id").cloned());
                        }
                    }
                }
                if hits.len() == 1 {
                    let mut out =
                        obj! {"route"=>"direction","kind"=>"direction","verb"=>verb,"target"=>hits[0].clone()};
                    if targets_found == 0 {
                        set(&mut out, "spot", true.into());
                    }
                    return Ok(out);
                }
                break;
            }
            return Ok(notice(book, "direction", "direction_unsupported", &base, Vec::new()));
        }
        let mut asks = Vec::new();
        if on(Feature::Perception) {
            asks.extend(["ask_location", "ask_sight", "ask_hearing", "ask_name"].map(|i| ("perception", i)));
        }
        if on(Feature::Entrances) && !codex.entrances().is_empty() {
            asks.push(("entrances", "ask_exits"));
        }
        if on(Feature::Scenery) && place.is_some() {
            asks.extend(
                ["ask_smell", "ask_light", "ask_temperature", "ask_floor", "ask_ceiling"].map(|i| ("scenery", i)),
            );
        }
        if on(Feature::Persona) {
            asks.extend(
                ["ask_role", "ask_self", "ask_clothing", "ask_appearance", "ask_activity", "ask_posture", "ask_held"]
                    .map(|i| ("persona", i)),
            );
        }
        if !asks.is_empty() && !long {
            let sound =
                scene_sound.or_else(|| if on(Feature::Scenery) { ambience(Some(scene_id), "sound") } else { None });
            let visible_props = present.iter().filter(|p| p.line_of_sight).collect::<Vec<_>>();
            let mut v = base.clone();
            v.flag("has_sight", sight.is_some())
                .flag("has_sound", sound.is_some())
                .flag("has_props", !visible_props.is_empty());
            if let Some(s) = sight {
                v.text("sight", s);
            }
            if let Some(s) = sound {
                v.text("sound", s);
            }
            if !visible_props.is_empty() {
                let labels = visible_props.iter().filter_map(|p| config.props.label(p.id)).collect::<Vec<_>>();
                if let Some(j) = book.joined(&labels).filter(|j| j.chars().count() <= 240) {
                    v.text("props", &j);
                }
            }
            if on(Feature::Scenery) {
                for k in ["smell", "light", "temperature"] {
                    if let Some(a) = ambience(Some(scene_id), k) {
                        v.text(k, a);
                    }
                }
            }
            if on(Feature::Entrances) {
                if !visible.is_empty() {
                    let labels = visible.iter().map(|e| e.label).collect::<Vec<_>>();
                    if let Some(j) = book.joined(&labels).filter(|j| j.chars().count() <= 240) {
                        v.text("exits", &j);
                    }
                }
                for e in &visible {
                    let mode = e
                        .def
                        .get("carries")
                        .and_then(|c| c.get("sound"))
                        .and_then(Value::as_str)
                        .unwrap_or("when_open");
                    if e.other_place.is_none() || !(mode == "always" || mode == "when_open" && e.status == "open") {
                        continue;
                    }
                    if let Some(heard) = ambience(e.other_place, "sound") {
                        v.text("through", e.label).text("heard", heard);
                        break;
                    }
                }
            }
            if on(Feature::Persona) {
                for k in ["role", "summary"] {
                    if let Some(x) = sheet.persona(k) {
                        v.text(k, x);
                    }
                }
            }
            for (route, intent) in asks {
                if book.matcher(intent).and_then(|m| m.hit(&q)).is_some() {
                    if let Some(out) = say(pending, route, intent, &v, Vec::new()) {
                        return Ok(out);
                    }
                }
            }
        }
        if !long && (on(Feature::Props) || on(Feature::Entrances) || on(Feature::Scenery)) {
            if let Some(rest) = book.matcher("describe_prop").and_then(|m| m.hit(&q)).filter(|r| !r.is_empty()) {
                let want = book.bare(&rest).to_owned();
                let hits = present
                    .iter()
                    .filter(|p| p.line_of_sight && config.props.names(p.id).iter().any(|n| book.bare(n) == want))
                    .collect::<Vec<_>>();
                if let [p] = hits[..] {
                    let mut v = base.clone();
                    v.text("prop", config.props.label(p.id).unwrap_or_default())
                        .text("description", config.props.description(p.id).unwrap_or_default())
                        .flag("has_note", p.note.is_some());
                    if let Some(n) = p.note {
                        v.text("note", n);
                    }
                    let prop_def = config.props.get(p.id);
                    let material =
                        p.material.or_else(|| prop_def.and_then(|d| d.get("material")).and_then(Value::as_str));
                    let color = p.color.or_else(|| prop_def.and_then(|d| d.get("color")).and_then(Value::as_str));
                    let texture = p.texture.or_else(|| prop_def.and_then(|d| d.get("texture")).and_then(Value::as_str));
                    v.flag("has_material", material.is_some())
                        .flag("has_color", color.is_some())
                        .flag("has_texture", texture.is_some())
                        .flag("has_matter", material.is_some() || color.is_some() || texture.is_some());
                    if let Some(m) = material {
                        v.text("material", m);
                    }
                    if let Some(c) = color {
                        v.text("color", c);
                    }
                    if let Some(t) = texture {
                        v.text("texture", t);
                    }
                    if let Some(out) = say(pending, "props", "describe_prop", &v, vec![("prop", Value::str(p.id))]) {
                        return Ok(out);
                    }
                }
                let hits = visible
                    .iter()
                    .filter(|e| on(Feature::Entrances) && e.names.iter().any(|n| book.bare(n) == want))
                    .collect::<Vec<_>>();
                if let [e] = hits[..] {
                    let mut v = base.clone();
                    v.text("label", e.label)
                        .text("description", e.side.get("description").and_then(Value::as_str).unwrap_or_default())
                        .text("status", e.status)
                        .flag("has_note", e.note.is_some());
                    if let Some(n) = e.note {
                        v.text("note", n);
                    }
                    if let Some(dest) = e.other_place.and_then(|p| codex.place_label(p)) {
                        v.text("destination", dest);
                    }
                    if let Some(out) =
                        say(pending, "entrances", "describe_entrance", &v, vec![("entrance", Value::str(e.id))])
                    {
                        return Ok(out);
                    }
                }
                let spots = place.and_then(|p| p.get("spots")).and_then(Value::as_array).unwrap_or(&[]);
                let hits = spots
                    .iter()
                    .filter(|s| on(Feature::Scenery) && spot_names(s).iter().any(|n| book.bare(n) == want))
                    .collect::<Vec<_>>();
                if let [s] = hits[..] {
                    let mut v = base.clone();
                    v.text("label", s.get("label").and_then(Value::as_str).unwrap_or_default())
                        .text("description", s.get("description").and_then(Value::as_str).unwrap_or_default());
                    for k in ["material", "color", "texture"] {
                        if let Some(val) = s.get(k).and_then(Value::as_str) {
                            v.text(k, val);
                        }
                    }
                    if let Some(out) = say(
                        pending,
                        "scenery",
                        "describe_spot",
                        &v,
                        vec![("spot", s.get("id").cloned().unwrap_or(Value::Null))],
                    ) {
                        return Ok(out);
                    }
                }
            }
            if on(Feature::Props) {
                let matter_rest =
                    book.matcher("ask_matter").and_then(|m| m.hit(&q)).filter(|r| !r.is_empty()).or_else(|| {
                        let s = q.as_str();
                        let prefix = ["what is ", "what s ", "what are "].into_iter().find(|p| s.starts_with(p))?;
                        let suffix = [" made of", " made from"].into_iter().find(|e| s.ends_with(e))?;
                        if s.len() > prefix.len() + suffix.len() {
                            Some(s[prefix.len()..s.len() - suffix.len()].trim().to_owned())
                        } else {
                            None
                        }
                    });
                if let Some(rest) = matter_rest {
                    let want = book.bare(&rest).to_owned();
                    let hits = present
                        .iter()
                        .filter(|p| p.line_of_sight && config.props.names(p.id).iter().any(|n| book.bare(n) == want))
                        .collect::<Vec<_>>();
                    if let [p] = hits[..] {
                        let mut v = base.clone();
                        v.text("prop", config.props.label(p.id).unwrap_or_default());
                        let prop_def = config.props.get(p.id);
                        let material =
                            p.material.or_else(|| prop_def.and_then(|d| d.get("material")).and_then(Value::as_str));
                        let color = p.color.or_else(|| prop_def.and_then(|d| d.get("color")).and_then(Value::as_str));
                        let texture =
                            p.texture.or_else(|| prop_def.and_then(|d| d.get("texture")).and_then(Value::as_str));
                        v.flag("has_material", material.is_some())
                            .flag("has_color", color.is_some())
                            .flag("has_texture", texture.is_some())
                            .flag("has_matter", material.is_some() || color.is_some() || texture.is_some());
                        if let Some(m) = material {
                            v.text("material", m);
                        }
                        if let Some(c) = color {
                            v.text("color", c);
                        }
                        if let Some(t) = texture {
                            v.text("texture", t);
                        }
                        if let Some(out) = say(pending, "props", "ask_matter", &v, vec![("prop", Value::str(p.id))]) {
                            return Ok(out);
                        }
                    }
                }
            }
        }
        if on(Feature::Commands) && !long {
            let r = resolve_command(
                &obj! {"input"=>input,"choices"=>choices.to_vec(),"labels"=>labels.to_vec(),"focus"=>focus.cloned()},
            )?;
            match r.get("status").and_then(Value::as_str) {
                Some("resolved") if c::get(&r, "decision")?.get("action").and_then(Value::as_str) != Some("talk") => {
                    return Ok(normed(
                        obj! {"route"=>"command","kind"=>"action","decision"=>c::get(&r,"decision")?.clone()},
                    ));
                }
                Some("clarify") => return Ok(notice(book, "command", "command_clarify", &base, Vec::new())),
                _ => {}
            }
        }
        if on(Feature::Disclosures) && input.len() <= 2048 {
            let claim = expression_claim(input)?;
            if claim != Value::Null && !input.trim_end_matches(['.', '!', '?']).contains(['.', '!', '?']) {
                let slot = c::text(c::get(&claim, "slot")?)?.to_owned();
                let value = c::text(c::get(&claim, "value")?)?.to_owned();
                let old = opt(&claim, "corrects").and_then(Value::as_str);
                let polarity = opt(&claim, "polarity").and_then(Value::as_str);
                let mut v = base.clone();
                v.text("slot", &slot).text("value", &value).flag("corrects", old.is_some());
                if let Some(o) = old {
                    v.text("old", o);
                }
                if let Some(p) = polarity {
                    v.text("polarity", p);
                }
                if let Some(out) = say(pending, "disclosure", "disclosure", &v, Vec::new()) {
                    let max = config.setting(Feature::Disclosures, "max_preferences").unwrap_or(3) as usize;
                    let l = listener_mut(listeners, who)?;
                    if slot == "name" {
                        set(l, "name", Value::str(&value));
                    } else {
                        let lower = value.to_lowercase();
                        let mut prefs = vec![obj! {"value"=>value.as_str(),"polarity"=>polarity}];
                        prefs.extend(
                            l.get("preferences")
                                .and_then(Value::as_array)
                                .unwrap_or(&[])
                                .iter()
                                .filter(|p| {
                                    p.get("value").and_then(Value::as_str).map(str::to_lowercase).as_deref()
                                        != Some(lower.as_str())
                                })
                                .cloned(),
                        );
                        prefs.truncate(max);
                        set(l, "preferences", prefs.into());
                    }
                    return Ok(out);
                }
            }
        }
        if on(Feature::Recall) {
            if book.matcher("recall_name").and_then(|m| m.hit(&q)).is_some() {
                let mut v = base.clone();
                let known =
                    find(listeners, who).and_then(|l| opt(l, "name")).and_then(Value::as_str).map(str::to_owned);
                v.flag("known", known.is_some());
                if let Some(n) = &known {
                    v.text("value", n);
                }
                if let Some(out) = say(pending, "recall", "recall_name", &v, Vec::new()) {
                    return Ok(out);
                }
            }
            if book.matcher("recall_preference").and_then(|m| m.hit(&q)).is_some() {
                let mut v = base.clone();
                let first = find(listeners, who)
                    .and_then(|l| l.get("preferences"))
                    .and_then(Value::as_array)
                    .and_then(|p| p.first())
                    .cloned();
                v.flag("known", first.is_some());
                if let Some(p) = &first {
                    v.text("value", c::text(c::get(p, "value")?)?).text("polarity", c::text(c::get(p, "polarity")?)?);
                }
                if let Some(out) = say(pending, "recall", "recall_preference", &v, Vec::new()) {
                    return Ok(out);
                }
            }
        }
        if on(Feature::Secrets) {
            for (x, (ps, when)) in sheet.all("secrets").iter().zip(&sheet.secrets) {
                if !contains(ps) {
                    continue;
                }
                let id = c::text(c::get(x, "id")?)?;
                let extra = vec![("secret", Value::str(id))];
                if when.holds(&base) {
                    let out = authored(
                        pending,
                        &turn,
                        "secrets",
                        "reveal",
                        &format!("secret:{id}"),
                        &[c::get(x, "reveal")?.clone()],
                        extra,
                    );
                    let l = listener_mut(listeners, who)?;
                    let mut revealed =
                        l.get("revealed").and_then(Value::as_array).map(<[Value]>::to_vec).unwrap_or_default();
                    if !revealed.iter().any(|r| r.as_str() == Some(id)) {
                        revealed.push(Value::str(id));
                    }
                    set(l, "revealed", revealed.into());
                    return Ok(out);
                }
                if let Some(cover) = opt(x, "cover") {
                    return Ok(authored(
                        pending,
                        &turn,
                        "secrets",
                        "cover",
                        &format!("cover:{id}"),
                        std::slice::from_ref(cover),
                        extra,
                    ));
                }
                return Ok(say(pending, "secrets", "secret_withheld", &base, extra)
                    .unwrap_or_else(|| obj! {"route"=>"secrets","kind"=>"silent","secret"=>id}));
            }
        }
        if on(Feature::Opinions) {
            for (x, (ps, when)) in sheet.all("opinions").iter().zip(&sheet.opinions) {
                if contains(ps) && when.holds(&base) {
                    let id = c::text(c::get(x, "id")?)?;
                    let variants = c::list(c::get(x, "variants")?)?;
                    return Ok(authored(
                        pending,
                        &turn,
                        "opinions",
                        "opinion",
                        &format!("opinion:{id}"),
                        variants,
                        vec![("opinion", Value::str(id))],
                    ));
                }
            }
        }
        let about = [Feature::Relationships, Feature::Factions, Feature::Events, Feature::Quests];
        if !long && about.iter().any(|f| on(*f)) {
            if let Some(rest) = book.matcher("ask_about").and_then(|m| m.hit(&q)).filter(|r| !r.is_empty()) {
                let want = book.bare(&rest).to_owned();
                let unique = |items: Vec<(usize, &[String])>| {
                    let hits =
                        items.into_iter().filter(|(_, ns)| ns.iter().any(|n| book.bare(n) == want)).collect::<Vec<_>>();
                    if hits.len() == 1 {
                        Some(hits[0].0)
                    } else {
                        None
                    }
                };
                if on(Feature::Relationships) {
                    let items = sheet.relationship_names.iter().enumerate().map(|(i, n)| (i, n.as_slice())).collect();
                    if let Some(i) = unique(items) {
                        let r = &sheet.all("relationships")[i];
                        let mut v = base.clone();
                        v.text("person", c::text(c::get(r, "name")?)?).text("kind", c::text(c::get(r, "kind")?)?);
                        if let Some(d) = opt(r, "description").and_then(Value::as_str) {
                            v.text("text", d);
                        }
                        if let Some(out) =
                            say(pending, "relationships", "person", &v, vec![("person", c::get(r, "with")?.clone())])
                        {
                            return Ok(out);
                        }
                    }
                }
                if on(Feature::Factions) {
                    let items = (0..codex.factions().len()).map(|i| (i, codex.names("factions", i))).collect();
                    if let Some(i) = unique(items) {
                        let f = &codex.factions()[i];
                        let id = c::text(c::get(f, "id")?)?;
                        let mut v = base.clone();
                        v.text("faction", c::text(c::get(f, "label")?)?)
                            .text("text", c::text(c::get(f, "description")?)?)
                            .text("stance", stance(codex, sheet, id));
                        if let Some(out) = say(pending, "factions", "faction", &v, vec![("faction", Value::str(id))]) {
                            return Ok(out);
                        }
                    }
                }
                if on(Feature::Events) {
                    let items = (0..codex.events().len()).map(|i| (i, codex.names("events", i))).collect();
                    if let Some(i) = unique(items) {
                        let e = &codex.events()[i];
                        let mut v = base.clone();
                        v.text("event", c::text(c::get(e, "label")?)?)
                            .text("text", c::text(c::get(e, "description")?)?)
                            .text("timing", timing(e));
                        if let Some(out) =
                            say(pending, "events", "event", &v, vec![("event", c::get(e, "id")?.clone())])
                        {
                            return Ok(out);
                        }
                    }
                }
                if on(Feature::Quests) {
                    let items = codex
                        .quests()
                        .iter()
                        .enumerate()
                        .filter(|(_, x)| {
                            x.get("givers")
                                .and_then(Value::as_array)
                                .unwrap_or(&[])
                                .iter()
                                .any(|g| g.as_str() == Some(me))
                                && x.get("id")
                                    .and_then(Value::as_str)
                                    .is_some_and(|id| quests.iter().any(|(k, _)| k == id))
                        })
                        .map(|(i, _)| (i, codex.names("quests", i)))
                        .collect();
                    if let Some(i) = unique(items) {
                        let qd = &codex.quests()[i];
                        let qid = c::text(c::get(qd, "id")?)?;
                        let stage_id =
                            quests.iter().find(|(k, _)| k == qid).and_then(|(_, s)| s.as_str()).unwrap_or_default();
                        let stage = c::list(c::get(qd, "stages")?)?
                            .iter()
                            .find(|s| s.get("id").and_then(Value::as_str) == Some(stage_id))
                            .ok_or_else(c::bad)?;
                        let mut v = base.clone();
                        v.text("quest", c::text(c::get(qd, "label")?)?)
                            .text("text", c::text(c::get(stage, "description")?)?)
                            .text("stage", stage_id);
                        if let Some(out) = say(pending, "quests", "quest", &v, vec![("quest", Value::str(qid))]) {
                            return Ok(out);
                        }
                    }
                }
            }
        }
        if !long {
            let services = sheet.all("services");
            if on(Feature::Services)
                && !services.is_empty()
                && book.matcher("ask_services").and_then(|m| m.hit(&q)).is_some()
            {
                let mut v = base.clone();
                let avail = services
                    .iter()
                    .zip(&sheet.services)
                    .filter(|(_, (_, when))| when.holds(&base))
                    .filter_map(|(x, _)| x.get("label").and_then(Value::as_str))
                    .collect::<Vec<_>>();
                if !avail.is_empty() {
                    if let Some(j) = book.joined(&avail).filter(|j| j.chars().count() <= 240) {
                        v.text("services", &j);
                    }
                }
                if let Some(out) = say(pending, "services", "ask_services", &v, Vec::new()) {
                    return Ok(out);
                }
            }
            let goals = sheet.all("goals");
            if on(Feature::Goals) && !goals.is_empty() && book.matcher("ask_goals").and_then(|m| m.hit(&q)).is_some() {
                let mut best: Option<(&Value, u64)> = None;
                for (g, when) in goals.iter().zip(&sheet.goals) {
                    let p = g.get("priority").and_then(Value::as_usize).unwrap_or(0) as u64;
                    if when.holds(&base) && best.is_none_or(|(_, b)| p > b) {
                        best = Some((g, p));
                    }
                }
                if let Some((g, _)) = best {
                    let id = c::text(c::get(g, "id")?)?;
                    return Ok(authored(
                        pending,
                        &turn,
                        "goals",
                        "goal",
                        &format!("goal:{id}"),
                        &[c::get(g, "description")?.clone()],
                        vec![("goal", Value::str(id))],
                    ));
                }
                if let Some(out) = say(pending, "goals", "no_goal", &base, Vec::new()) {
                    return Ok(out);
                }
            }
            if on(Feature::Events)
                && !codex.events().is_empty()
                && book.matcher("ask_events").and_then(|m| m.hit(&q)).is_some()
            {
                let mut v = base.clone();
                let live = codex
                    .events()
                    .iter()
                    .filter(|e| ["now", "soon"].contains(&timing(e)))
                    .filter_map(|e| e.get("label").and_then(Value::as_str))
                    .collect::<Vec<_>>();
                if !live.is_empty() {
                    if let Some(j) = book.joined(&live).filter(|j| j.chars().count() <= 240) {
                        v.text("events", &j);
                    }
                }
                if let Some(out) = say(pending, "events", "ask_events", &v, Vec::new()) {
                    return Ok(out);
                }
            }
        }
        let previous = find(listeners, who).and_then(|l| opt(l, "topic")).and_then(Value::as_str).map(str::to_owned);
        let route = if long {
            obj! {"kind"=>"other","topic"=>Value::Null}
        } else {
            let mut said = input;
            for opener in ["hello", "hi", "hey", "greetings"] {
                let whos = [normalize(name), normalize(me), format!("the {}", normalize(me))];
                if whos.iter().any(|w| q == format!("{opener} {w}")) {
                    said = opener;
                }
            }
            conversation_route(&obj! {"input"=>said,"previous"=>previous.clone()})?
        };
        let topic = opt(&route, "topic").cloned();
        if topic.is_some() || previous.is_some() {
            set(listener_mut(listeners, who)?, "topic", topic.clone().unwrap_or(Value::Null));
        }
        let kind = c::text(c::get(&route, "kind")?)?.to_owned();
        if kind == "greeting" {
            if on(Feature::Greetings) {
                let plan = greetings.plan(&obj! {"actor"=>me,"listener"=>who,"scene"=>scene_id,"visit"=>c::get(sc,"visit")?.clone(),"at"=>at,"trust"=>trust,"atmosphere"=>atmosphere,"place"=>place_kind})?;
                let mut v = base.clone();
                v.text("meeting", c::text(c::get(&plan, "meeting")?)?)
                    .flag("needs_introduction", c::boolean(c::get(&plan, "needs_introduction")?)?);
                if let Some(out) = reply(book, pending, &turn, "greeting", "greeting", &v, Vec::new(), true) {
                    return Ok(out);
                }
            }
            if on(Feature::Social) {
                if let Some(out) = reply(book, pending, &turn, "social", "social_greeting", &base, Vec::new(), true) {
                    return Ok(out);
                }
            }
        }
        if on(Feature::Social) {
            if ["thanks", "farewell", "wellbeing"].contains(&kind.as_str()) {
                if let Some(out) = say(pending, "social", &kind, &base, Vec::new()) {
                    return Ok(out);
                }
            }
            if kind == "elaborate" {
                let intent = match previous.as_deref() {
                    None => "clarify_topic",
                    Some("wellbeing") => "mood_reason",
                    Some(_) => "elaborate",
                };
                let mut v = base.clone();
                v.flag("has_reason", reason.is_some());
                if let Some(r) = reason {
                    v.text("reason", r);
                }
                if let Some(out) = say(pending, "social", intent, &v, Vec::new()) {
                    return Ok(out);
                }
            }
        }
        if on(Feature::Model) {
            let visible_props = present.iter().filter(|p| p.line_of_sight).collect::<Vec<_>>();
            let request = compile(&Compile {
                config,
                input,
                base: &base,
                scene: sc,
                sight,
                sound: scene_sound
                    .or_else(|| if on(Feature::Scenery) { ambience(Some(scene_id), "sound") } else { None }),
                floor,
                ceiling,
                exterior,
                ambience: &|k| if on(Feature::Scenery) { ambience(Some(scene_id), k) } else { None },
                present: &visible_props.iter().filter_map(|p| config.props.label(p.id)).collect::<Vec<_>>(),
                exits: &if on(Feature::Entrances) { visible.iter().map(|e| e.label).collect() } else { Vec::new() },
                listener: find(listeners, who),
                recent: &recent,
                events: &if on(Feature::Events) {
                    codex
                        .events()
                        .iter()
                        .filter(|e| ["now", "soon"].contains(&timing(e)))
                        .map(|e| (e.get("label").and_then(Value::as_str).unwrap_or_default(), timing(e)))
                        .collect()
                } else {
                    Vec::new()
                },
                on: &on,
            });
            if asked.len() == PENDING {
                asked.remove(0);
            }
            asked.push(
                obj! {"token"=>token,"listener"=>who,"scene"=>scene_id,"visit"=>c::get(sc,"visit")?.clone(),"at"=>at},
            );
            return Ok(
                obj! {"route"=>"model","kind"=>"model","input"=>input,"long"=>long,"expression"=>expression(input)?,"topic"=>topic,"request"=>request},
            );
        }
        if on(Feature::Social) {
            if let Some(out) = say(pending, "fallback", "unknown", &base, Vec::new()) {
                return Ok(out);
            }
        }
        Ok(obj! {"route"=>"none","kind"=>"silent"})
    }

    /// Check a model's raw output for the open request `token`: `{token, listener, text}`.
    ///
    /// Only an exact `{"speech": "..."}` object (optionally in a ```json fence) that passes
    /// every bounded check is accepted. It becomes `{route:"model", kind:"reply",
    /// intent:"model", line:null, text}`, pending until [`RoleplayEngine::delivered`]
    /// like any other reply. Otherwise `{route:"model", kind:"rejected", reason}` with
    /// `reason` one of `length`, `format`, `empty`, `control`, `stage`, `sentences`,
    /// `out_of_character`, `avoided`, `secret`, `identifier` or `repeat`; the request stays
    /// open so the host may retry or fall back. These are guardrails, not proof of truth.
    pub fn reply(&mut self, a: &Value) -> Result<Value> {
        let mut next = self.clone();
        let out = next.accept(a)?;
        *self = next;
        Ok(out)
    }
    fn accept(&mut self, a: &Value) -> Result<Value> {
        c::fields(a, &["token", "listener", "text"])?;
        uint(a, "token", 1)?;
        let who = ident(c::get(a, "listener")?)?;
        let raw = c::text(c::get(a, "text")?)?;
        let i = self.asked.iter().position(|x| x.get("token") == a.get("token")).ok_or_else(c::bad)?;
        if self.asked[i].get("listener").and_then(Value::as_str) != Some(who) {
            return Err(c::bad());
        }
        let speech = match check_speech(raw, &self.config, find(&self.listeners, who), who) {
            Ok(s) => s,
            Err(reason) => return Ok(obj! {"route"=>"model","kind"=>"rejected","reason"=>reason}),
        };
        let tk = self.asked.remove(i);
        if self.pending.len() == PENDING {
            self.pending.remove(0);
        }
        self.pending.push(obj! {
            "token"=>c::get(&tk,"token")?.clone(),"listener"=>who,"text"=>speech.as_str(),"scene"=>c::get(&tk,"scene")?.clone(),
            "visit"=>c::get(&tk,"visit")?.clone(),"at"=>c::get(&tk,"at")?.clone(),"greeting"=>false,"introduced"=>false
        });
        Ok(obj! {"route"=>"model","kind"=>"reply","intent"=>"model","line"=>Value::Null,"text"=>speech})
    }

    /// Confirm what the listener actually heard of a reply: `{token, listener, text, heard}`
    /// (`heard` in characters). Only full deliveries update variety and encounter memory.
    pub fn delivered(&mut self, d: &Value) -> Result<Value> {
        let mut next = self.clone();
        let out = next.confirm(d)?;
        *self = next;
        Ok(out)
    }
    fn confirm(&mut self, d: &Value) -> Result<Value> {
        c::fields(d, &["token", "listener", "text", "heard"])?;
        uint(d, "token", 1)?;
        let who = ident(c::get(d, "listener")?)?;
        let text = line(c::get(d, "text")?, 240, 1024)?;
        let heard = c::uint(c::get(d, "heard")?, 0, text.chars().count() as u64)?;
        let Some(i) = self.pending.iter().position(|p| p.get("token") == d.get("token")) else {
            return Ok(obj! {"recorded"=>false,"reason"=>"unknown"});
        };
        let p = self.pending.remove(i);
        if p.get("listener") != d.get("listener") || p.get("text") != d.get("text") {
            return Err(c::bad());
        }
        if heard != text.chars().count() as u64 {
            return Ok(obj! {"recorded"=>false,"reason"=>"partial"});
        }
        let n = self.config.recent();
        if n > 0 {
            let l = self.listener_mut(who)?;
            let mut recent = l.get("recent").and_then(Value::as_array).map(<[Value]>::to_vec).unwrap_or_default();
            recent.push(Value::str(text));
            let skip = recent.len().saturating_sub(n);
            set(l, "recent", recent.split_off(skip).into());
        }
        let mut greeting = Value::Null;
        if self.config.enabled(Feature::Greetings) {
            let me = self.config.character_id().to_owned();
            let expected = self.greetings.contact(&me, who).and_then(|e| e.get("token")).cloned();
            let receipt = obj! {
                "actor"=>me.as_str(),"listener"=>who,"scene"=>c::get(&p,"scene")?.clone(),"visit"=>c::get(&p,"visit")?.clone(),
                "at"=>c::get(&p,"at")?.clone(),"token"=>c::get(&p,"token")?.clone(),"greeting"=>c::get(&p,"greeting")?.clone(),
                "introduced"=>c::get(&p,"introduced")?.clone(),"text"=>text,"heard"=>heard,"expected"=>expected
            };
            greeting = self.greetings.confirm(&receipt)?;
        }
        Ok(obj! {"recorded"=>true,"greeting"=>greeting})
    }
}

struct Exit<'a> {
    id: &'a str,
    label: &'a str,
    status: &'a str,
    note: Option<&'a str>,
    def: &'a Value,
    side: &'a Value,
    first: bool,
    other_place: Option<&'a str>,
    names: &'a [String],
}
fn list(v: Option<&Value>, lo: usize, hi: usize) -> Result<&[Value]> {
    crate::knowledge::codex::list(v, lo, hi)
}
fn spot_names(s: &Value) -> Vec<String> {
    let mut out = vec![normalize(s.get("label").and_then(Value::as_str).unwrap_or_default())];
    out.extend(strings(s.get("aliases")).iter().map(|a| normalize(a)));
    out
}
fn find<'a>(listeners: &'a [Value], who: &str) -> Option<&'a Value> {
    listeners.iter().find(|l| l.get("id").and_then(Value::as_str) == Some(who))
}
fn listener_mut<'a>(listeners: &'a mut Vec<Value>, who: &str) -> Result<&'a mut Value> {
    let i = match listeners.iter().position(|l| l.get("id").and_then(Value::as_str) == Some(who)) {
        Some(i) => i,
        None => {
            if listeners.len() >= LISTENERS {
                return Err(c::bad());
            }
            listeners.push(obj! {"id"=>who,"name"=>Value::Null,"preferences"=>Vec::<Value>::new(),"recent"=>Vec::<Value>::new(),"topic"=>Value::Null,"revealed"=>Vec::<Value>::new()});
            listeners.len() - 1
        }
    };
    Ok(&mut listeners[i])
}
fn stance(codex: &Codex, sheet: &CharacterSheet, faction: &str) -> &'static str {
    let member = sheet.memberships();
    if member.contains(&faction) {
        return "member";
    }
    for m in member {
        let relations = codex.faction(m).and_then(|f| f.get("relations")).and_then(Value::as_array).unwrap_or(&[]);
        if let Some(r) = relations.iter().find(|r| r.get("faction").and_then(Value::as_str) == Some(faction)) {
            return match r.get("stance").and_then(Value::as_str) {
                Some("allied") => "allied",
                Some("friendly") => "friendly",
                Some("wary") => "wary",
                Some("hostile") => "hostile",
                _ => "neutral",
            };
        }
    }
    "neutral"
}
fn record(pending: &mut Vec<Value>, t: &Turn, text: &str, greeting: bool, introduced: bool) {
    if pending.len() == PENDING {
        pending.remove(0);
    }
    pending.push(obj! {
        "token"=>t.token,"listener"=>t.who,"text"=>text,
        "scene"=>t.scene.get("id").cloned(),"visit"=>t.scene.get("visit").cloned(),
        "at"=>t.at,"greeting"=>greeting,"introduced"=>introduced
    });
}
#[allow(clippy::too_many_arguments)]
fn reply(
    book: &LineBook,
    pending: &mut Vec<Value>,
    t: &Turn,
    route: &str,
    intent: &str,
    vars: &Vars,
    extra: Vec<(&str, Value)>,
    greeting: bool,
) -> Option<Value> {
    let (rule, text) = book.say(intent, vars, t.recent)?;
    let introduced = greeting && rule.introduces;
    let mut out = vec![
        ("route".to_owned(), Value::str(route)),
        ("kind".to_owned(), Value::str("reply")),
        ("intent".to_owned(), Value::str(intent)),
        ("line".to_owned(), Value::str(&rule.id)),
        ("text".to_owned(), Value::str(&text)),
    ];
    out.extend(extra.into_iter().map(|(k, v)| (k.to_owned(), v)));
    if greeting {
        out.push(("introduced".into(), introduced.into()));
    }
    record(pending, t, &text, greeting, introduced);
    Some(Value::Object(out))
}
/// Speak authored sheet text: the first variant not recently heard by this listener.
fn authored(
    pending: &mut Vec<Value>,
    t: &Turn,
    route: &str,
    intent: &str,
    line_id: &str,
    texts: &[Value],
    extra: Vec<(&str, Value)>,
) -> Value {
    let texts = texts.iter().filter_map(Value::as_str).collect::<Vec<_>>();
    let text = texts.iter().find(|s| !t.recent.iter().any(|r| r == *s)).or(texts.first()).copied().unwrap_or_default();
    let mut out = vec![
        ("route".to_owned(), Value::str(route)),
        ("kind".to_owned(), Value::str("reply")),
        ("intent".to_owned(), Value::str(intent)),
        ("line".to_owned(), Value::str(line_id)),
        ("text".to_owned(), Value::str(text)),
    ];
    out.extend(extra.into_iter().map(|(k, v)| (k.to_owned(), v)));
    record(pending, t, text, false, false);
    Value::Object(out)
}
fn notice(book: &LineBook, route: &str, intent: &str, vars: &Vars, extra: Vec<(&str, Value)>) -> Value {
    let got = book.say(intent, vars, &[]);
    let mut out = vec![
        ("route".to_owned(), Value::str(route)),
        ("kind".to_owned(), Value::str("clarify")),
        ("intent".to_owned(), Value::str(intent)),
        ("line".to_owned(), got.as_ref().map_or(Value::Null, |(r, _)| Value::str(&r.id))),
        ("text".to_owned(), got.map_or(Value::Null, |(_, s)| Value::str(s))),
    ];
    out.extend(extra.into_iter().map(|(k, v)| (k.to_owned(), v)));
    Value::Object(out)
}

/// `{config, state, ops:[{op:"turn"|"delivered"|"reply", args}]}` → `{config, results, state}`.
pub(crate) fn fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["config", "state", "ops"])?;
    let config = EngineConfig::from_value(c::get(a, "config")?)?;
    let mut engine = match c::get(a, "state")? {
        Value::Null => RoleplayEngine::new(config),
        s => RoleplayEngine::restore(config, s)?,
    };
    let ops = c::list(c::get(a, "ops")?)?;
    if ops.len() > 64 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let args = c::get(op, "args")?;
        results.push(match c::text(c::get(op, "op")?)? {
            "turn" => engine.turn(args)?,
            "delivered" => engine.delivered(args)?,
            "reply" => engine.reply(args)?,
            _ => return Err(c::bad()),
        });
    }
    Ok(obj! {"config"=>engine.config.to_value(),"results"=>results,"state"=>engine.checkpoint()})
}
