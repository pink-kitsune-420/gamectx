//! Communication as action (pass 2, B1): speaking is a planned act with preconditions and
//! effects on the listener.
//!
//! Eight acts: `inform`, `warn` and `ask` move [beliefs](crate::character::beliefs) between
//! beings; `request`, `command`, `persuade` and `threaten` try to give the listener a want;
//! `comfort` eases fear. Each act reads the pair state "how much the listener trusts the
//! speaker" (`trust`, default 50) and "how much authority the speaker has over the listener"
//! (`authority`, default 0), set with `relate`.
//!
//! `perform` applies what the host owns here (beliefs, wants, trust) and returns every effect,
//! including `mind_record` proposals shaped exactly like [`crate::character::mind`] `record`
//! args, so the host can update theory of mind. A failed precondition is reported
//! (`performed: false`), not an error. `plan` picks the act and listener for an intent:
//! `learn` (ask the most trusted), `share`/`alarm` (inform/warn everyone), `get_help`
//! (command, request or persuade, whichever is most likely) and `reassure` (comfort).
//!
//! All belief ops (`init`, `learn`, `outcome`, `query`, `known_actions`, `gaps`) are
//! available in the same workflow. Acts are honest: lying stays with the
//! [deception ledger](crate::character::deception). Bounded: 256 pairs, 32 wants per being.
//! Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::speech_act::SpeechActs;
//!
//! let mut acts = SpeechActs::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "learn", "args": {"being": "guard", "about": "thief", "relation": "returns", "value": "tonight", "source": "saw"}},
//!   {"op": "relate", "args": {"speaker": "guard", "listener": "innkeeper", "trust": 80}},
//!   {"op": "perform", "args": {"speaker": "guard", "listener": "innkeeper", "act": "warn",
//!                              "topic": {"about": "thief", "relation": "returns", "value": "tonight"}}}
//! ]}"#, 4096)?;
//! let out = acts.workflow(&req)?;
//! let warned = &out.get("results").unwrap().as_array().unwrap()[2];
//! assert_eq!(warned.get("outcome").and_then(|v| v.as_str()), Some("warned"));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{list_bound, opt, opt_uint, run_ops, text_bound, Belief, BeliefStore, Triple, MAX_AT};
use crate::checked as c;
use crate::data::persist::{self, durable, is_ident, need as ok, persist, within};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const ACTS: [&str; 8] = ["ask", "command", "comfort", "inform", "persuade", "request", "threaten", "warn"];
pub const INTENTS: [&str; 5] = ["alarm", "get_help", "learn", "reassure", "share"];
pub const MAX_PAIRS: usize = 256;
pub const MAX_WANTS: usize = 32;
pub const MAX_WANT_BEINGS: usize = 64;
pub const MAX_CANDIDATES: usize = 16;
pub const DEFAULT_TRUST: u64 = 50;
/// Confidence a speaker needs before it will inform or warn, and above which it will not ask.
pub const SURE: u64 = 50;

/// Host-set capacity limits: `(name, default, ceiling)`.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "pairs", default: 256, ceiling: 1000000 },
    crate::character::limits::Limit { name: "want_beings", default: 64, ceiling: 1000000 },
    crate::character::limits::Limit { name: "wants", default: 32, ceiling: 1024 },
    crate::character::limits::Limit { name: "beings", default: 64, ceiling: 1000000 },
    crate::character::limits::Limit { name: "beliefs", default: 128, ceiling: 4096 },
];
crate::character::limits::module_limits!(SpeechLimits, LIMITS);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pair {
    pub trust: u64,
    pub authority: u64,
}

impl Default for Pair {
    fn default() -> Self {
        Self { trust: DEFAULT_TRUST, authority: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    pub priority: u64,
    pub from: String,
}

struct Act {
    speaker: String,
    listener: String,
    act: String,
    topic: Option<Triple>,
    goal: Option<String>,
    strength: u64,
    at: u64,
}

/// Belief store plus the social pair state speech acts read and change.
#[derive(Clone, Debug, Default)]
pub struct SpeechActs {
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: SpeechLimits,
    pub beliefs: BeliefStore,
    /// `(speaker, listener)` → how much the listener trusts the speaker, and the speaker's authority.
    pub pairs: BTreeMap<(String, String), Pair>,
    /// Listener → speakers with a pair, kept in step with `pairs` so `assess` does not scan every pair.
    pub trusted: BTreeMap<String, BTreeSet<String>>,
    pub wants: BTreeMap<String, BTreeMap<String, Want>>,
}

fn effect_belief(holder: &str, key: &str, result: &str) -> Value {
    obj! { "holder" => holder, "key" => key, "kind" => "belief", "result" => result }
}

fn mind_knows(subject: &str, other: &str, fact: &str, status: bool, source: &str, at: u64) -> Value {
    obj! {
        "args" => obj! {
            "at" => at,
            "category" => "knows",
            "fact" => fact,
            "other" => other,
            "source" => source,
            "status" => status,
            "subject" => subject,
        },
        "kind" => "mind_record",
    }
}

fn feels(holder: &str, toward: &str, emotion: &str, delta: i64) -> Value {
    obj! { "delta" => delta, "emotion" => emotion, "holder" => holder, "kind" => "feels", "toward" => toward }
}

impl SpeechActs {
    pub fn new() -> Self {
        Self::default()
    }

    fn put_pair(&mut self, speaker: &str, listener: &str, pair: Pair) {
        self.trusted.entry(listener.to_string()).or_default().insert(speaker.to_string());
        self.pairs.insert((speaker.to_string(), listener.to_string()), pair);
    }

    pub fn pair(&self, speaker: &str, listener: &str) -> Pair {
        self.pairs.get(&(speaker.to_string(), listener.to_string())).copied().unwrap_or_default()
    }

    fn pair_room(&self, speaker: &str, listener: &str) -> Result<()> {
        if !self.pairs.contains_key(&(speaker.to_string(), listener.to_string()))
            && self.pairs.len() >= self.limits.get("pairs")
        {
            return Err(c::bad());
        }
        Ok(())
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        self.beliefs.limits.copy_shared(&self.limits);
        run_ops(request, |op, args| match op {
            "relate" => self.op_relate(args),
            "check" => {
                let a = read_act(args)?;
                let failed = self.preconditions(&a);
                Ok(obj! {
                    "act" => a.act.as_str(),
                    "failed" => Value::strings(&failed),
                    "listener" => a.listener.as_str(),
                    "ok" => failed.is_empty(),
                    "speaker" => a.speaker.as_str(),
                })
            }
            "perform" => self.op_perform(args),
            "plan" => self.op_plan(args),
            "assess" => self.op_assess(args),
            "export" => persist::export_op(self, &self.limits, args),
            "import" => {
                let limits = self.limits.clone();
                let out = persist::import(self, &limits, args)?;
                self.beliefs.limits.copy_shared(&self.limits);
                Ok(out)
            }
            _ => self.beliefs.dispatch(op, args)?.ok_or_else(c::bad),
        })
    }

    fn op_relate(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["speaker", "listener"], &["trust", "authority"])?;
        let speaker = ident(c::get(args, "speaker")?)?.to_string();
        let listener = ident(c::get(args, "listener")?)?.to_string();
        if speaker == listener {
            return Err(c::bad());
        }
        let cur = self.pair(&speaker, &listener);
        let next = Pair {
            trust: opt_uint(args, "trust", 0, 100, cur.trust)?,
            authority: opt_uint(args, "authority", 0, 100, cur.authority)?,
        };
        self.pair_room(&speaker, &listener)?;
        self.put_pair(&speaker, &listener, next);
        Ok(obj! { "authority" => next.authority, "listener" => listener, "speaker" => speaker, "trust" => next.trust })
    }

    fn preconditions(&self, a: &Act) -> Vec<&'static str> {
        let p = self.pair(&a.speaker, &a.listener);
        let mine = a.topic.as_ref().and_then(|t| self.beliefs.get(&a.speaker, t));
        let mut failed = Vec::new();
        match a.act.as_str() {
            "inform" | "warn" => match mine {
                None => failed.push("speaker_does_not_know"),
                Some(b) if b.confidence < SURE => failed.push("speaker_unsure"),
                _ => {}
            },
            "ask" if mine.is_some_and(|b| b.confidence >= SURE) => failed.push("already_known"),
            "command" if p.authority < 50 => failed.push("no_authority"),
            "comfort" if p.trust < 30 => failed.push("not_trusted"),
            _ => {}
        }
        failed
    }

    fn add_want(&mut self, holder: &str, goal: &str, priority: u64, from: &str) -> Result<()> {
        if !self.wants.contains_key(holder) && self.wants.len() >= self.limits.get("want_beings") {
            return Err(c::bad());
        }
        let max_wants = self.limits.get("wants");
        let ws = self.wants.entry(holder.to_string()).or_default();
        match ws.get_mut(goal) {
            Some(w) if priority >= w.priority => *w = Want { priority, from: from.to_string() },
            Some(_) => {}
            None => {
                if ws.len() >= max_wants {
                    let victim = ws
                        .iter()
                        .min_by(|a, b| a.1.priority.cmp(&b.1.priority).then(a.0.cmp(b.0)))
                        .map(|(g, _)| g.clone());
                    if let Some(v) = victim {
                        ws.remove(&v);
                    }
                }
                ws.insert(goal.to_string(), Want { priority, from: from.to_string() });
            }
        }
        Ok(())
    }

    fn set_trust(&mut self, a: &Act, value: u64, effects: &mut Vec<Value>) {
        let old = self.pair(&a.speaker, &a.listener);
        let (speaker, listener) = (a.speaker.clone(), a.listener.clone());
        self.put_pair(&speaker, &listener, Pair { trust: value, ..old });
        effects.push(obj! {
            "delta" => value as i64 - old.trust as i64,
            "holder" => a.listener.as_str(),
            "kind" => "trust",
            "toward" => a.speaker.as_str(),
            "value" => value,
        });
    }

    fn op_perform(&mut self, args: &Value) -> Result<Value> {
        let a = read_act(args)?;
        let failed = self.preconditions(&a);
        let mut effects = Vec::new();
        let outcome = if !failed.is_empty() {
            "blocked"
        } else {
            if matches!(a.act.as_str(), "threaten" | "comfort") {
                self.pair_room(&a.speaker, &a.listener)?;
            }
            self.apply(&a, &mut effects)?
        };
        Ok(obj! {
            "act" => a.act.as_str(),
            "effects" => Value::Array(effects),
            "failed" => Value::strings(&failed),
            "listener" => a.listener.as_str(),
            "outcome" => outcome,
            "performed" => failed.is_empty(),
            "speaker" => a.speaker.as_str(),
        })
    }

    fn apply(&mut self, a: &Act, effects: &mut Vec<Value>) -> Result<&'static str> {
        let p = self.pair(&a.speaker, &a.listener);
        let (s, l) = (a.speaker.as_str(), a.listener.as_str());
        match a.act.as_str() {
            "inform" | "warn" => {
                let t = a.topic.clone().ok_or_else(c::bad)?;
                let mine = self.beliefs.get(s, &t).cloned().ok_or_else(c::bad)?;
                let conf = (mine.confidence * p.trust / 100).max(1);
                let key = t.key();
                let told = Belief {
                    triple: t,
                    holds: mine.holds,
                    source: "told".into(),
                    confidence: conf,
                    from: Some(s.into()),
                    at: a.at,
                };
                let (result, _) = self.beliefs.learn(l, told)?;
                effects.push(effect_belief(l, &key, result));
                effects.push(mind_knows(s, l, &key, true, "inferred", a.at));
                if a.act == "warn" {
                    effects.push(feels(l, &mine.triple.about, "fear", (conf / 2) as i64));
                    return Ok("warned");
                }
                Ok("informed")
            }
            "ask" => {
                let t = a.topic.clone().ok_or_else(c::bad)?;
                let key = t.key();
                match self.beliefs.get(l, &t).cloned() {
                    Some(theirs) => {
                        let trust_in_listener = self.pair(l, s).trust;
                        let conf = (theirs.confidence * trust_in_listener / 100).max(1);
                        let told = Belief {
                            triple: t,
                            holds: theirs.holds,
                            source: "told".into(),
                            confidence: conf,
                            from: Some(l.into()),
                            at: a.at,
                        };
                        let (result, _) = self.beliefs.learn(s, told)?;
                        effects.push(effect_belief(s, &key, result));
                        effects.push(mind_knows(l, s, &key, true, "inferred", a.at));
                        Ok("answered")
                    }
                    None => {
                        effects.push(mind_knows(s, l, &key, false, "told", a.at));
                        Ok("no_answer")
                    }
                }
            }
            "request" | "command" | "persuade" | "threaten" => {
                let goal = a.goal.clone().ok_or_else(c::bad)?;
                let (complied, priority) = match a.act.as_str() {
                    "request" => (p.trust >= 40, p.trust),
                    "command" => (true, p.authority),
                    "persuade" => {
                        let score = (p.trust + a.strength) / 2;
                        (score >= 50, score)
                    }
                    _ => {
                        let fear = (30 + a.strength / 2).min(100);
                        effects.push(feels(l, s, "fear", fear as i64));
                        self.set_trust(a, p.trust.saturating_sub(20), effects);
                        (fear >= 50, fear)
                    }
                };
                if !complied {
                    return Ok("refused");
                }
                self.add_want(l, &goal, priority, s)?;
                effects.push(obj! {
                    "from" => s,
                    "goal" => goal.as_str(),
                    "holder" => l,
                    "kind" => "wants",
                    "priority" => priority,
                });
                effects.push(obj! {
                    "args" => obj! {
                        "at" => a.at,
                        "category" => "wants",
                        "goal" => goal.as_str(),
                        "other" => l,
                        "priority" => priority,
                        "source" => "inferred",
                        "subject" => s,
                    },
                    "kind" => "mind_record",
                });
                Ok("complied")
            }
            _ => {
                effects.push(feels(l, s, "fear", -((p.trust / 2) as i64)));
                effects.push(feels(l, s, "gratitude", (p.trust / 4) as i64));
                self.set_trust(a, (p.trust + 5).min(100), effects);
                Ok("comforted")
            }
        }
    }

    fn op_plan(&self, args: &Value) -> Result<Value> {
        keys(args, &["speaker", "intent", "candidates"], &["topic", "goal", "strength"])?;
        let speaker = ident(c::get(args, "speaker")?)?.to_string();
        let intent = text_bound(c::get(args, "intent")?, 16)?;
        if !INTENTS.contains(&intent) {
            return Err(c::bad());
        }
        let raw = list_bound(Some(c::get(args, "candidates")?), MAX_CANDIDATES)?;
        let mut cands = BTreeSet::new();
        for x in raw {
            let x = ident(x)?;
            if x == speaker || !cands.insert(x.to_string()) {
                return Err(c::bad());
            }
        }
        if cands.is_empty() {
            return Err(c::bad());
        }
        let wants_topic = matches!(intent, "learn" | "share" | "alarm");
        let topic = match opt(args, "topic") {
            Some(t) if wants_topic => {
                keys(t, &["about", "relation", "value"], &[])?;
                Some(Triple::read(t)?)
            }
            None if wants_topic => return Err(c::bad()),
            Some(_) => return Err(c::bad()),
            None => None,
        };
        // The goal is validated here; the chosen act is performed with it later.
        match opt(args, "goal") {
            Some(g) if intent == "get_help" => ident(g).map(|_| ())?,
            None if intent == "get_help" => return Err(c::bad()),
            Some(_) => return Err(c::bad()),
            None => {}
        }
        if opt(args, "strength").is_some() && intent != "get_help" {
            return Err(c::bad());
        }
        let strength = opt_uint(args, "strength", 0, 100, 50)?;

        let step = |act: &str, listener: &str, expected: u64, likely: bool| {
            obj! { "act" => act, "expected" => expected, "likely" => likely, "listener" => listener }
        };
        let mine = topic.as_ref().and_then(|t| self.beliefs.get(&speaker, t));
        let mut steps = Vec::new();
        let mut reason = "ok";
        match intent {
            "learn" => {
                if mine.is_some_and(|b| b.confidence >= SURE) {
                    reason = "already_known";
                } else {
                    // Ask whoever the speaker trusts most (ties: first by name).
                    let mut best: Option<(&String, u64)> = None;
                    for cnd in &cands {
                        let t = self.pair(cnd, &speaker).trust;
                        if best.is_none_or(|(_, bt)| t > bt) {
                            best = Some((cnd, t));
                        }
                    }
                    if let Some((who, t)) = best {
                        steps.push(step("ask", who, t, t >= SURE));
                    }
                }
            }
            "share" | "alarm" => match mine {
                None => reason = "speaker_does_not_know",
                Some(b) if b.confidence < SURE => reason = "speaker_unsure",
                Some(b) => {
                    let act = if intent == "share" { "inform" } else { "warn" };
                    for cnd in &cands {
                        let e = (b.confidence * self.pair(&speaker, cnd).trust / 100).max(1);
                        steps.push(step(act, cnd, e, e >= SURE));
                    }
                }
            },
            "get_help" => {
                let mut best: Option<(&String, &str, u64, bool)> = None;
                for cnd in &cands {
                    let p = self.pair(&speaker, cnd);
                    let (act, e, likely) = if p.authority >= 50 {
                        ("command", p.authority, true)
                    } else if p.trust >= 40 {
                        ("request", p.trust, true)
                    } else {
                        let sc = (p.trust + strength) / 2;
                        ("persuade", sc, sc >= 50)
                    };
                    if best.is_none_or(|(_, _, be, _)| e > be) {
                        best = Some((cnd, act, e, likely));
                    }
                }
                if let Some((who, act, e, likely)) = best {
                    steps.push(step(act, who, e, likely));
                }
            }
            _ => {
                let mut best: Option<(&String, u64)> = None;
                for cnd in &cands {
                    let t = self.pair(&speaker, cnd).trust;
                    if t >= 30 && best.is_none_or(|(_, bt)| t > bt) {
                        best = Some((cnd, t));
                    }
                }
                match best {
                    Some((who, t)) => steps.push(step("comfort", who, t, true)),
                    None => reason = "no_trusted_listener",
                }
            }
        }
        Ok(obj! {
            "count" => steps.len(),
            "intent" => intent,
            "reason" => reason,
            "speaker" => speaker,
            "steps" => Value::Array(steps),
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let being = ident(c::get(args, "being")?)?;
        let trusts: Vec<Value> = self
            .trusted
            .get(being)
            .into_iter()
            .flatten()
            .map(|s| (s, self.pair(s, being)))
            .map(|(s, p)| obj! { "authority" => p.authority, "speaker" => s.as_str(), "trust" => p.trust })
            .collect();
        let wants: Vec<Value> = self
            .wants
            .get(being)
            .into_iter()
            .flatten()
            .map(|(g, w)| obj! { "from" => w.from.as_str(), "goal" => g.as_str(), "priority" => w.priority })
            .collect();
        let beliefs = self.beliefs.beings.get(being).map(BTreeMap::len).unwrap_or(0);
        Ok(obj! {
            "being" => being,
            "beliefs" => beliefs,
            "trusts" => Value::Array(trusts),
            "wants" => Value::Array(wants),
        })
    }
}

persist!(Pair { authority [0, 100], trust [0, 100] });
persist!(Want { from, priority [0, 100] });
persist!(rest SpeechActs { beliefs, pairs, wants });
durable!(SpeechActs, "speech_act", LIMITS);

impl SpeechActs {
    /// Invariants a save must meet; `trusted` is rebuilt from `pairs`.
    fn settle_state(&mut self) -> Result<()> {
        crate::character::beliefs::check_beliefs(&self.beliefs.beings)?;
        within::<Self>("pairs", self.pairs.len())?;
        within::<Self>("want_beings", self.wants.len())?;
        self.trusted.clear();
        for (s, l) in self.pairs.keys() {
            ok(is_ident(s) && is_ident(l) && s != l)?;
            self.trusted.entry(l.clone()).or_default().insert(s.clone());
        }
        for (holder, ws) in &self.wants {
            ok(is_ident(holder) && !ws.is_empty())?;
            within::<Self>("wants", ws.len())?;
            for (goal, w) in ws {
                ok(is_ident(goal) && is_ident(&w.from) && w.from != *holder)?;
            }
        }
        Ok(())
    }

    fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
}

fn read_act(args: &Value) -> Result<Act> {
    keys(args, &["speaker", "listener", "act"], &["topic", "goal", "strength", "at"])?;
    let speaker = ident(c::get(args, "speaker")?)?.to_string();
    let listener = ident(c::get(args, "listener")?)?.to_string();
    let act = text_bound(c::get(args, "act")?, 16)?.to_string();
    if speaker == listener || !ACTS.contains(&act.as_str()) {
        return Err(c::bad());
    }
    let needs_topic = matches!(act.as_str(), "inform" | "warn" | "ask");
    let needs_goal = matches!(act.as_str(), "request" | "command" | "persuade" | "threaten");
    let uses_strength = matches!(act.as_str(), "persuade" | "threaten");
    let topic = match opt(args, "topic") {
        Some(t) if needs_topic => {
            keys(t, &["about", "relation", "value"], &[])?;
            Some(Triple::read(t)?)
        }
        None if !needs_topic => None,
        _ => return Err(c::bad()),
    };
    let goal = match opt(args, "goal") {
        Some(g) if needs_goal => Some(ident(g)?.to_string()),
        None if !needs_goal => None,
        _ => return Err(c::bad()),
    };
    if opt(args, "strength").is_some() && !uses_strength {
        return Err(c::bad());
    }
    Ok(Act {
        speaker,
        listener,
        act,
        topic,
        goal,
        strength: opt_uint(args, "strength", 0, 100, 50)?,
        at: opt_uint(args, "at", 0, MAX_AT, 0)?,
    })
}

pub fn workflow(req: &Value) -> Result<Value> {
    SpeechActs::new().workflow(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn run(s: &mut SpeechActs, json: &str) -> String {
        s.workflow(&parse(json, 1 << 16).unwrap()).unwrap().to_json()
    }

    #[test]
    fn ask_moves_a_belief_with_scaled_confidence() {
        let mut s = SpeechActs::new();
        let out = run(
            &mut s,
            r#"{"ops":[
              {"op":"learn","args":{"being":"miller","about":"mill","relation":"sells","value":"flour","source":"did"}},
              {"op":"relate","args":{"speaker":"miller","listener":"baker","trust":70}},
              {"op":"perform","args":{"speaker":"baker","listener":"miller","act":"ask","topic":{"about":"mill","relation":"sells","value":"flour"}}},
              {"op":"query","args":{"being":"baker"}}]}"#,
        );
        assert!(out.contains(r#""outcome":"answered""#), "{out}");
        assert!(out.contains(r#""confidence":70"#), "{out}");
    }

    #[test]
    fn command_without_authority_is_blocked_not_an_error() {
        let mut s = SpeechActs::new();
        let out = run(
            &mut s,
            r#"{"ops":[{"op":"perform","args":{"speaker":"a","listener":"b","act":"command","goal":"fetch_water"}}]}"#,
        );
        assert!(out.contains(r#""failed":["no_authority"]"#) && out.contains(r#""performed":false"#), "{out}");
    }

    #[test]
    fn mismatched_fields_are_refused() {
        let mut s = SpeechActs::new();
        for bad in [
            r#"{"ops":[{"op":"perform","args":{"speaker":"a","listener":"a","act":"comfort"}}]}"#,
            r#"{"ops":[{"op":"perform","args":{"speaker":"a","listener":"b","act":"comfort","goal":"x"}}]}"#,
            r#"{"ops":[{"op":"perform","args":{"speaker":"a","listener":"b","act":"request","goal":"x","strength":5}}]}"#,
        ] {
            assert!(s.workflow(&parse(bad, 4096).unwrap()).is_err(), "{bad}");
        }
    }
}
