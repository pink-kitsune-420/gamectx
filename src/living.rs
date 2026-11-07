//! Bounded deterministic world updates. Rumors, fractional social influence and host
//! counters never invoke a model, confirm delivery or establish new canonical truth.
use crate::{
    cast::CastPack,
    checked as c,
    life::{get, line, names, num, signed, text, Initiative, Life},
    obj,
    request::bounded,
    routine::{Frame, Runtime},
    Result, Value,
};
use std::sync::Arc;
mod context;
pub use context::compile_fixture;
mod speech;
pub use speech::{speech_workflow, SpeechQueue, SpeechTicket};
pub(crate) const ATMOSPHERE: [&str; 4] = ["calm", "busy", "tense", "emergency"];
pub(crate) fn validate_line(s: &str) -> Result<()> {
    line(s)?;
    if s.chars().count() > 240
        || crate::life::assess_sentence(s, 240)?.to_value().get("boundary_complete") != Some(&true.into())
    {
        return Err(c::bad());
    }
    Ok(())
}
#[derive(Clone, Debug)]
struct Actor {
    life: Life,
    levels: Vec<i64>,
    routine: Option<Runtime>,
    initiative: Option<Initiative>,
    interactions: crate::dialogue::InteractionTools,
}
#[derive(Clone, Debug)]
struct Known {
    root: usize,
    actor: usize,
    path: Vec<String>,
    event: u64,
}
#[derive(Debug)]
pub struct LivingWorld {
    pack: Arc<CastPack>,
    scope: String,
    actors: Vec<Actor>,
    clock: u64,
    last_tick: Option<u64>,
    revision: u64,
    cursor: usize,
    roots: Vec<Value>,
    known: Vec<Known>,
    influences: Vec<Value>,
    counters: Vec<Value>,
    counter_water: u64,
    last_counter: Option<Value>,
    atmospheres: Vec<Value>,
}
impl LivingWorld {
    /// Explicit host acceptance of the pack for this save/branch scope. Actors share
    /// immutable definitions; the world keeps separate base experience and influence.
    pub fn new(pack: Arc<CastPack>, scope: &str) -> Result<Self> {
        c::name(scope)?;
        let people = Value::strings(&pack.participants);
        let calendar = text(get(&pack.value, "setting")?, "calendar")?;
        let mut actors = Vec::new();
        for ch in &pack.characters {
            let life = Life::accept(scope, &ch.id, &people, ch.definition.clone(), ch.blueprint.life_config()?)?;
            let routine = if let Some(r) = &ch.routine {
                Some(ch.blueprint.routine(scope, &ch.id, &people, calendar, r.clone())?)
            } else {
                None
            };
            let initiative = if ch.blueprint.has("initiative") {
                Some(ch.blueprint.initiative(scope, &ch.id, &people)?)
            } else {
                None
            };
            let levels =
                life.attitudes.iter().map(|a| signed(get(a, "level").expect("level"), 100).expect("level")).collect();
            actors.push(Actor {
                life,
                levels,
                routine,
                initiative,
                interactions: crate::dialogue::InteractionTools::new(scope, &ch.id, &people)?,
            });
        }
        Ok(Self {
            pack,
            scope: scope.into(),
            actors,
            clock: 0,
            last_tick: None,
            revision: 0,
            cursor: 0,
            roots: Vec::new(),
            known: Vec::new(),
            influences: Vec::new(),
            counters: Vec::new(),
            counter_water: 0,
            last_counter: None,
            atmospheres: Vec::new(),
        })
    }
    fn index(&self, id: &str) -> Result<usize> {
        self.pack.characters.iter().position(|c| c.id == id).ok_or_else(c::bad)
    }
    fn enabled(&self, key: &str) -> bool {
        get(&self.pack.value, "features").expect("features").get(key) == Some(&true.into())
    }
    fn can_bump(&self) -> Result<()> {
        if self.revision >= u32::MAX as u64 {
            Err(c::bad())
        } else {
            Ok(())
        }
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Base Life checkpoints remain valid and unchanged. Group ripples live in a
    /// separate ledger; use the world compiler for effective attitudes and gates.
    pub fn base_life(&self, actor: &str) -> Result<&Life> {
        Ok(&self.actors[self.index(actor)?].life)
    }
    fn effective_level(&self, i: usize, a: &Value) -> i64 {
        let id = get(a, "definition").expect("definition").get("id");
        let n = self.actors[i]
            .life
            .attitudes
            .iter()
            .position(|x| get(x, "definition").expect("definition").get("id") == id)
            .expect("attitude");
        self.actors[i].levels[n]
    }
    fn projected(&self, i: usize) -> Life {
        let mut life = self.actors[i].life.clone();
        for (n, a) in life.attitudes.iter_mut().enumerate() {
            crate::archetype::set_field(a, "level", self.actors[i].levels[n].into());
        }
        life
    }
    pub fn inspect_actor(&self, actor: &str) -> Result<Value> {
        let i = self.index(actor)?;
        Ok(obj! {"base"=>self.actors[i].life.inspect(),"effective_attitudes"=>self.projected(i).attitudes})
    }
    /// A privileged host experience, with explicit sharing permission. An optional
    /// original appraisal must concern this event's subject. Reports cannot be origins.
    pub fn observe(&mut self, a: &Value) -> Result<Value> {
        bounded(a, 16384)?;
        c::fields(a, &["id", "actor", "event", "significance", "share_with", "appraise"])?;
        self.can_bump()?;
        c::name(text(a, "id")?)?;
        let actor = self.index(text(a, "actor")?)?;
        if self.roots.len() >= 64 || self.roots.iter().any(|r| r.get("id") == a.get("id")) {
            return Err(c::bad());
        }
        c::uint(get(a, "significance")?, 0, 100)?;
        for receiver in names(get(a, "share_with")?, 12)? {
            self.index(&receiver)?;
        }
        let event = get(a, "event")?;
        if !matches!(text(event, "kind")?, "direct" | "outcome") || num(event, "at")? < self.clock {
            return Err(c::bad());
        }
        let mut life = self.actors[actor].life.clone();
        life.experience(event)?;
        let mut change = Value::Null;
        let mut level_update = None;
        if get(a, "appraise")? != &Value::Null {
            let d = self.pack.characters[actor].definition.to_value();
            let rule =
                c::list(get(&d, "rules")?)?.iter().find(|r| r.get("id") == a.get("appraise")).ok_or_else(c::bad)?;
            let attr = life
                .attitudes
                .iter()
                .find(|a| get(a, "definition").ok().and_then(|d| d.get("id")) == rule.get("attitude"))
                .ok_or_else(c::bad)?;
            let def = get(attr, "definition")?;
            if def.get("subject") != event.get("subject") {
                return Err(c::bad());
            }
            let kind = get(def, "kind")?.clone();
            let index = life
                .attitudes
                .iter()
                .position(|x| get(x, "definition").expect("definition").get("id") == rule.get("attitude"))
                .expect("attitude");
            let before = self.actors[actor].levels[index];
            let after = (before + signed(get(rule, "delta")?, 20)?).clamp(-100, 100);
            let after = if before * after < 0 { 0 } else { after };
            life.appraise(text(a, "appraise")?, num(event, "token")?, life.revision())?;
            level_update = Some((index, after));
            change = obj! {"kind"=>kind,"delta"=>after-before};
        }
        let mut root = a.clone();
        crate::archetype::set_field(&mut root, "change", change);
        self.known.push(Known {
            root: self.roots.len(),
            actor,
            path: vec![self.pack.characters[actor].id.clone()],
            event: num(event, "token")?,
        });
        self.roots.push(root);
        self.actors[actor].life = life;
        if let Some((n, level)) = level_update {
            self.actors[actor].levels[n] = level;
        }
        self.clock = num(event, "at")?;
        self.revision += 1;
        Ok(obj! {"root"=>get(a,"id")?.clone(),"revision"=>self.revision})
    }
    fn delta(&self, root: &Value, receiver: usize) -> Result<Option<(String, i64)>> {
        if !self.enabled("ripples") || get(root, "change")? == &Value::Null {
            return Ok(None);
        }
        let r = get(&self.pack.value, "ripple")?;
        if get(get(root, "change")?, "kind")? != get(r, "kind")?
            || !names(get(&self.pack.value, "players")?, 8)?.iter().any(|p| {
                Some(p.as_str()) == get(root, "event").ok().and_then(|e| e.get("subject")).and_then(Value::as_str)
            })
        {
            return Ok(None);
        }
        let shared = c::list(get(&self.pack.value, "groups")?)?.iter().any(|g| {
            names(get(r, "groups").expect("groups"), 16)
                .expect("groups")
                .iter()
                .any(|id| Some(id.as_str()) == g.get("id").and_then(Value::as_str))
                && names(get(g, "members").expect("members"), 16).is_ok_and(|m| {
                    m.iter().any(|p| Some(p.as_str()) == root.get("actor").and_then(Value::as_str))
                        && m.contains(&self.pack.characters[receiver].id)
                })
        });
        if !shared {
            return Ok(None);
        }
        let attrs = self.actors[receiver]
            .life
            .attitudes
            .iter()
            .filter(|a| {
                let d = get(a, "definition").expect("definition");
                d.get("kind") == r.get("kind") && d.get("subject") == get(root, "event").expect("event").get("subject")
            })
            .collect::<Vec<_>>();
        if attrs.len() != 1 {
            return Ok(None);
        }
        let delta = signed(get(get(root, "change")?, "delta")?, 20)? * num(r, "numerator")? as i64
            / num(r, "denominator")? as i64;
        let cap = num(r, "max_delta")? as i64;
        let delta = delta.clamp(-cap, cap);
        let before = self.effective_level(receiver, attrs[0]);
        let after = (before + delta).clamp(-100, 100);
        let after = if before * after < 0 { 0 } else { after };
        let applied = after - before;
        Ok(if applied == 0 { None } else { Some((text(get(attrs[0], "definition")?, "id")?.into(), applied)) })
    }
    /// One bounded, synchronous host step; no I/O, sleeping, model calls or blocking
    /// task joins. Host windows confirm actual co-location; routine intent is insufficient.
    /// A snapshot of known rumors permits at most one hop per strictly increasing tick.
    pub fn tick(&mut self, a: &Value) -> Result<Value> {
        bounded(a, 65536)?;
        c::fields(a, &["now", "windows", "routines"])?;
        self.can_bump()?;
        let now = num(a, "now")?;
        if now < self.clock || self.last_tick.is_some_and(|t| now <= t) {
            return Err(c::bad());
        }
        let windows = c::list(get(a, "windows")?)?;
        let routines = c::list(get(a, "routines")?)?;
        if windows.len() > 16 || routines.len() > 12 {
            return Err(c::bad());
        }
        for (i, w) in windows.iter().enumerate() {
            c::fields(w, &["id", "location", "start", "end", "participants", "kind"])?;
            c::name(text(w, "id")?)?;
            let start = num(w, "start")?;
            c::uint(get(w, "end")?, start + 1, u32::MAX as u64)?;
            if !self.pack.locations.iter().any(|l| l == text(w, "location").unwrap_or(""))
                || !matches!(text(w, "kind")?, "scene" | "routine")
                || windows[..i].iter().any(|o| o.get("id") == w.get("id"))
            {
                return Err(c::bad());
            }
            for p in names(get(w, "participants")?, 12)? {
                self.index(&p)?;
            }
        }
        for (i, v) in routines.iter().enumerate() {
            c::fields(v, &["actor", "frame"])?;
            self.index(text(v, "actor")?)?;
            Frame::from_value(get(v, "frame")?)?;
            if num(get(v, "frame")?, "now")? != now || routines[..i].iter().any(|o| o.get("actor") == v.get("actor")) {
                return Err(c::bad());
            }
        }
        let mut routine_results = Vec::new();
        for v in routines {
            let i = self.index(text(v, "actor")?)?;
            let ok = self.actors[i]
                .routine
                .as_mut()
                .is_some_and(|r| r.update(Frame::from_value(get(v, "frame").expect("frame")).expect("frame")).is_ok());
            routine_results.push(obj! {"actor"=>get(v,"actor")?.clone(),"ok"=>ok});
        }
        let snapshot_len = self.known.len();
        let p = get(&self.pack.value, "propagation")?;
        let mut reports = Vec::new();
        let mut blocked = Vec::new();
        let mut checked = 0;
        let n = self.actors.len();
        let total = windows.len() * self.roots.len() * n * n;
        if self.enabled("rumors") && total > 0 {
            for _ in 0..(num(p, "checks")? as usize).min(total) {
                if reports.len() >= num(p, "per_tick")? as usize {
                    break;
                }
                let mut x = self.cursor % total;
                self.cursor = (x + 1) % total;
                checked += 1;
                let actor = x % n;
                x /= n;
                let sender = x % n;
                x /= n;
                let ri = x % self.roots.len();
                let w = &windows[x / self.roots.len()];
                let root = &self.roots[ri];
                if now < num(w, "start")?
                    || now >= num(w, "end")?
                    || num(root, "significance")? < num(p, "significance")?
                {
                    continue;
                }
                let receiver = &self.pack.characters[actor].id;
                let sender_id = &self.pack.characters[sender].id;
                let people = names(get(w, "participants")?, 12)?;
                if !people.contains(receiver)
                    || !people.contains(sender_id)
                    || !names(get(root, "share_with")?, 12)?.contains(receiver)
                    || self.known.iter().any(|k| k.root == ri && k.actor == actor)
                    || !self.pack.characters[actor].blueprint.has("life")
                {
                    continue;
                }
                let Some(known) = self.known[..snapshot_len].iter().find(|k| k.root == ri && k.actor == sender) else {
                    continue;
                };
                if known.path.len() > num(p, "max_hops")? as usize {
                    continue;
                }
                let trust = self.actors[actor]
                    .life
                    .attitudes
                    .iter()
                    .filter(|a| {
                        let d = get(a, "definition").expect("definition");
                        d.get("kind").and_then(Value::as_str) == Some("trust")
                            && d.get("subject").and_then(Value::as_str) == Some(sender_id)
                    })
                    .collect::<Vec<_>>();
                if trust.len() != 1 || self.effective_level(actor, trust[0]) < signed(get(p, "trust")?, 100)? {
                    continue;
                }
                let e = get(root, "event")?;
                let delta = self.delta(root, actor)?;
                if delta.is_some() && self.influences.len() >= 128 {
                    blocked.push(
                        obj! {"root"=>get(root,"id")?.clone(),"actor"=>receiver.as_str(),"reason"=>"backpressure"},
                    );
                    continue;
                }
                let token = self.actors[actor].life.watermark + 1;
                let event = obj! {"token"=>token,"at"=>now,"kind"=>"report","tag"=>get(e,"tag")?.clone(),"subject"=>get(e,"subject")?.clone(),"source"=>obj!{"id"=>format!("rumor:{}",text(root,"id")?),"speaker"=>sender_id.clone(),"text"=>get(e,"text")?.clone()},"audience"=>self.pack.participants.clone(),"text"=>get(e,"text")?.clone(),"causes"=>Vec::<Value>::new(),"retain"=>true};
                if self.actors[actor].life.experience(&event).is_err() {
                    blocked.push(
                        obj! {"root"=>get(root,"id")?.clone(),"actor"=>receiver.as_str(),"reason"=>"backpressure"},
                    );
                    continue;
                }
                let mut path = known.path.clone();
                path.push(receiver.clone());
                self.known.push(Known { root: ri, actor, path: path.clone(), event: token });
                if let Some((attitude, delta)) = delta {
                    let n = self.actors[actor]
                        .life
                        .attitudes
                        .iter()
                        .position(|x| {
                            get(x, "definition").expect("definition").get("id").and_then(Value::as_str)
                                == Some(attitude.as_str())
                        })
                        .expect("attitude");
                    self.actors[actor].levels[n] += delta;
                    self.influences.push(obj!{"root"=>get(root,"id")?.clone(),"actor"=>receiver.as_str(),"attitude"=>attitude,"subject"=>get(e,"subject")?.clone(),"delta"=>delta,"reason"=>get(get(&self.pack.value,"ripple")?,"reason")?.clone(),"event"=>token});
                }
                reports.push(obj!{"root"=>get(root,"id")?.clone(),"sender"=>sender_id.clone(),"actor"=>receiver.as_str(),"hops"=>path.len()-1,"path"=>path,"original_source"=>get(e,"source")?.clone(),"event"=>event});
            }
        }
        self.clock = now;
        self.last_tick = Some(now);
        if !reports.is_empty() {
            self.revision += 1;
        }
        Ok(
            obj! {"routines"=>routine_results,"reports"=>reports,"blocked"=>blocked,"checked_pairs"=>checked,"revision"=>self.revision,"model_calls"=>0},
        )
    }
    /// Existing initiative executes immediately against effective attitudes. Returned
    /// activities remain proposals; neither this method nor `tick` confirms an outcome.
    pub fn consider(&mut self, actor: &str, frame: &Value, candidates: &Value) -> Result<Value> {
        if num(frame, "now")? != self.clock {
            return Err(c::bad());
        }
        let i = self.index(actor)?;
        let life = self.projected(i);
        let a = &mut self.actors[i];
        a.initiative.as_mut().ok_or_else(c::bad)?.consider(frame, candidates, &life, &a.interactions)
    }
    pub fn dispatch_initiative(
        &mut self,
        actor: &str,
        id: u64,
        frame: &Value,
        allowed: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        if num(frame, "now")? != self.clock {
            return Err(c::bad());
        }
        let i = self.index(actor)?;
        let life = self.projected(i);
        let a = &mut self.actors[i];
        a.initiative.as_mut().ok_or_else(c::bad)?.dispatch(id, frame, &life, &a.interactions, allowed)
    }
    /// Revision-bound standard RequestGate context. Clock-only ticks do not invalidate
    /// speech; material world changes do. Hosts supply a current scene/permission revision.
    pub fn speech_context(&self, actor: &str, scene_revision: u64) -> Result<Value> {
        self.index(actor)?;
        c::time(scene_revision, 0)?;
        Ok(
            obj! {"actor"=>actor,"world"=>self.scope.clone(),"schema"=>self.revision,"permissions"=>scene_revision,"facts"=>Vec::<Value>::new()},
        )
    }
    pub fn routine_context(&self, actor: &str) -> Result<Value> {
        self.actors[self.index(actor)?].routine.as_ref().ok_or_else(c::bad)?.context()
    }
    pub fn fallback(&self, actor: &str, mode: &str) -> Result<String> {
        let i = self.index(actor)?;
        if !ATMOSPHERE.contains(&mode) && mode != "unknown" {
            return Err(c::bad());
        }
        Ok(text(get(&c::list(get(&self.pack.value, "characters")?)?[i], "fallbacks")?, mode)?.into())
    }
}
pub fn replay(a: &Value) -> Result<(LivingWorld, Vec<Value>)> {
    bounded(a, crate::kit::MAX_REQUEST_BYTES)?;
    c::fields(a, &["pack", "scope", "operations"])?;
    let mut w = LivingWorld::new(Arc::new(CastPack::from_value(get(a, "pack")?)?), text(a, "scope")?)?;
    let ops = c::list(get(a, "operations")?)?;
    if ops.len() > 256 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    let mut result_bytes = 0;
    for op in ops {
        let mut run = || -> Result<Value> {
            c::fields(op, &["op", "args"])?;
            let v = get(op, "args")?;
            match text(op, "op")? {
                "observe" => w.observe(v),
                "tick" => w.tick(v),
                "counters" => w.update_counters(v),
                "atmosphere" => w.set_atmosphere(v),
                "context" => w.context(v),
                "initiative" => {
                    c::fields(v, &["actor", "frame", "candidates"])?;
                    w.consider(text(v, "actor")?, get(v, "frame")?, get(v, "candidates")?)
                }
                "inspect" => {
                    c::fields(v, &["actor"])?;
                    w.inspect_actor(text(v, "actor")?)
                }
                _ => Err(c::bad()),
            }
        };
        let result = match run() {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        };
        result_bytes += result.json_len() + 1;
        if result_bytes > crate::kit::MAX_REQUEST_BYTES {
            return Err(c::bad());
        }
        results.push(result);
    }
    Ok((w, results))
}
pub fn workflow(a: &Value) -> Result<Value> {
    let (w, results) = replay(a)?;
    Ok(obj! {"results"=>results,"revision"=>w.revision,"model_calls"=>0})
}
