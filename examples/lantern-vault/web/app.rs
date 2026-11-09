//! Authoritative studio state. Only explicit player choices execute game rules.
use super::*;
mod shadow;
use gamectx::{
    character::cast::CastPack,
    character::dialogue::Answers,
    character::dialogue::InteractionTools,
    character::living::LivingWorld,
    character::party::{ModelRoute, PartyScene},
    character::roleplay::RoleplayEngine,
    kit::save::{GameData, SaveFile, SaveState},
    media::providers::ProviderBudget,
    media::visual::{Activity, ArtBible, AssetRegistry, RegistryConfig, VisualSpec},
    media::voice::VoiceTurn,
    obj,
};
use providers::{Config, Request};
use std::{path::PathBuf, sync::Arc};
#[derive(Debug)]
pub struct Job {
    pub id: u64,
    pub epoch: u64,
    pub modality: String,
    pub backend: String,
    pub reservation: Option<u64>,
    pub request: Request,
    pub binding: Value,
}
#[derive(Debug)]
struct Speech {
    id: u64,
    actor: String,
    player: String,
    voice: VoiceTurn,
    pending: bool,
    audio: Option<Value>,
    origin: String,
    viewpoint: Option<Value>,
    encounter: Option<Value>,
    rest: Vec<String>,
    segment: u64,
    total: u64,
    fiction: bool,
    story_seed: Option<String>,
    playback_blocked: bool,
    wording_attempted: bool,
}
pub struct App {
    pub config: Config,
    pub budget: ProviderBudget,
    pub dir: PathBuf,
    pub game: game::Game,
    pub epoch: u64,
    branch: u64,
    tick: u64,
    seq: u64,
    world: LivingWorld,
    pack: Arc<CastPack>,
    people: Value,
    player: String,
    npc: String,
    actions: Vec<Value>,
    world_ops: Vec<Value>,
    receipts: Vec<Value>,
    party: PartyScene,
    interactions: Vec<(String, InteractionTools)>,
    speech: Option<Speech>,
    transcript: Vec<Value>,
    shared_log: Vec<Value>,
    sharing: bool,
    pending: Vec<(u64, u64, String)>,
    notice: String,
    mic: u64,
    mic_open: bool,
    local_paused: bool,
    monitor: super::setup::Monitor,
    pub manager: super::manager_view::State,
    voice_history: super::voice_history::History,
    mind: super::personality::Mind,
    reactions: super::reactions::Reactions,
    greetings: gamectx::character::roleplay::GreetingMemory,
    deliberation: super::deliberation::State,
    speech_options: super::speech_options::Options,
    wording_cache: Vec<(String, String)>,
    roleplay: gamectx::character::roleplay::RoleplayActions,
    manager_significance: u64,
    manager_delta: u64,
    manager_event_tick: u64,
    focus: Option<(String, u64)>,
    recognition: Value,
    counters: [u64; 2],
    fulfilled: [u64; 2],
    counter_seq: u64,
    bible: ArtBible,
    assets: AssetRegistry,
    specs: Vec<VisualSpec>,
    previews: Vec<(Value, Value)>,
    approvals: Vec<Value>,
    repairs: Vec<Value>,
    applied_repairs: Vec<String>,
    pub game_data: GameData,
    pub engine_merchant: RoleplayEngine,
    pub engine_keeper: RoleplayEngine,
    pub engine_hound: RoleplayEngine,
    pub roleplay_engine: super::super::roleplay::VaultRoleplayEngine,
}
fn array(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(err)
}
fn source(id: &str, speaker: &str, line: &str) -> Value {
    obj! {"id"=>id,"speaker"=>speaker,"text"=>line}
}
impl App {
    pub fn new(seed: u32, cap: u64, config: Config, dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(dir.join("assets")).map_err(|_| err())?;
        let mut pack_value = parse(include_str!("../../fixtures/cast/chime-town.json"))?;
        let mut characters = array(get(&pack_value, "characters")?)?.to_vec();
        for ch in &mut characters {
            if ["merchant", "keeper"].contains(&text(ch, "id")?) {
                let mut d = get(ch, "definition")?.clone();
                let mut rules = array(get(&d, "rules")?)?.to_vec();
                for player in ["player_1", "player_2"] {
                    rules.push(obj!{"id"=>format!("purchase_{player}"),"attitude"=>format!("trust_{player}"),"event"=>"purchased","delta"=>2,"cooldown"=>1,"reason"=>"Host-confirmed fair trade supports trust."});
                }
                set(&mut d, "rules", rules.into());
                set(ch, "definition", d);
            }
        }
        set(&mut pack_value, "characters", characters.into());
        let pack = Arc::new(CastPack::from_value(&pack_value)?);
        let people = get(&pack.summary(), "participants")?.clone();
        let world = LivingWorld::new(pack.clone(), "studio_save")?;
        let party = Self::party(&people, 0)?;
        let interactions = ["merchant", "keeper"]
            .into_iter()
            .map(|id| Ok((id.into(), InteractionTools::new("studio_save", id, &people)?)))
            .collect::<Result<Vec<_>>>()?;
        let budget_path = dir.join("spending.json");
        let budget = if budget_path.exists() {
            ProviderBudget::recover(&parse(&read_file(&budget_path, 65536)?)?, cap)?
        } else {
            ProviderBudget::new(cap)?
        };
        let registry_epoch = format!(
            "run_{}_{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_| err())?.as_nanos()
        );
        let assets = AssetRegistry::new(RegistryConfig::from_value(
            &obj! {"enabled"=>true,"capacity"=>128,"queue_limit"=>16,"epoch"=>registry_epoch},
        )?);
        let cache = dir.join("assets/index.json");
        let (approvals, repairs) = if cache.exists() {
            let c = parse(&read_file(&cache, 8 * 1024 * 1024)?)?;
            fields(&c, &["approvals", "repairs"])?;
            let a = array(get(&c, "approvals")?)?.to_vec();
            let r = array(get(&c, "repairs")?)?.to_vec();
            if a.len() > 128 || r.len() > 128 {
                return Err(err());
            }
            (a, r)
        } else {
            (Vec::new(), Vec::new())
        };
        let manager = super::manager_view::State::new(&dir)?;
        let voice_history = super::voice_history::History::new(&dir)?;
        let mind = super::personality::Mind::new(&dir)?;
        let reactions = super::reactions::Reactions::new(&dir)?;
        let mut greetings = super::greetings::new(&dir)?;
        let deliberation = super::deliberation::State::new(&dir)?;
        let speech_options = super::speech_options::Options::new(&dir)?;
        let roleplay_file = dir.join("roleplay-actions.json");
        let roleplay = gamectx::character::roleplay::RoleplayActions::from_value(&if roleplay_file.exists() {
            parse(&read_file(&roleplay_file, 65536)?)?
        } else {
            parse(include_str!("roleplay-actions.json"))?
        })?;
        let mut initial_seq = 0;
        if mind.enabled() {
            super::greetings::seed_legacy(&mut greetings, &[], &mind.views.checkpoint(), 0, &mut initial_seq)?;
        }
        let game_data = super::super::world_model::build_game_data(Some(&roleplay))?;
        let (engine_merchant, engine_keeper, engine_hound) = super::super::world_model::build_engines(&game_data)?;
        let roleplay_engine = super::super::roleplay::VaultRoleplayEngine::new(&game_data)?;
        let mut game = game::Game::new(seed)?;
        game.enable_world_systems();
        let out = Self {
            config,
            budget,
            dir,
            game,
            epoch: 1,
            branch: 1,
            tick: 0,
            seq: initial_seq,
            world,
            pack,
            people,
            player: "player_1".into(),
            npc: "merchant".into(),
            actions: Vec::new(),
            world_ops: Vec::new(),
            receipts: Vec::new(),
            party,
            interactions,
            speech: None,
            transcript: Vec::new(),
            shared_log: Vec::new(),
            sharing: false,
            pending: Vec::new(),
            notice: "Offline studio ready. Text, art and voice providers are independent.".into(),
            mic: 0,
            mic_open: false,
            local_paused: false,
            monitor: super::setup::Monitor::default(),
            manager,
            voice_history,
            mind,
            reactions,
            greetings,
            deliberation,
            speech_options,
            wording_cache: Vec::new(),
            roleplay,
            manager_significance: 0,
            manager_delta: 0,
            manager_event_tick: 0,
            focus: None,
            recognition: Value::Null,
            counters: [0, 0],
            fulfilled: [0, 0],
            counter_seq: 0,
            bible: ArtBible::parse(include_str!("art.json"))?,
            assets,
            specs: Vec::new(),
            previews: Vec::new(),
            approvals,
            repairs,
            applied_repairs: Vec::new(),
            game_data,
            engine_merchant,
            engine_keeper,
            engine_hound,
            roleplay_engine,
        };
        out.persist_budget()?;
        Ok(out)
    }
    fn party(people: &Value, revision: u64) -> Result<PartyScene> {
        PartyScene::from_value(
            &obj! {"version"=>1,"id"=>"studio_save","epoch"=>1,"revision"=>revision,"participants"=>array(people)?.iter().map(|p|obj!{"id"=>p.clone(),"kind"=>if p.as_str().unwrap().starts_with("player_"){"player"}else{"ai"}}).collect::<Vec<_>>()},
        )
    }
    pub fn persist_budget(&self) -> Result<()> {
        atomic(&self.dir.join("spending.json"), self.budget.checkpoint().to_json().as_bytes())
    }
    fn cache(&self) -> Result<()> {
        atomic(
            &self.dir.join("assets/index.json"),
            obj! {"approvals"=>self.approvals.clone(),"repairs"=>self.repairs.clone()}.to_json().as_bytes(),
        )
    }
    fn next(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
    fn active(&self) -> bool {
        self.npc == "merchant" && self.game.room == "workshop"
            || self.npc == "keeper" && self.game.room == "cistern"
            || self.npc == "hound" && (self.game.room == self.game.animal_room || self.game.animal_following)
    }
    pub fn engine_mut(&mut self) -> &mut RoleplayEngine {
        match self.npc.as_str() {
            "keeper" => &mut self.engine_keeper,
            "hound" => &mut self.engine_hound,
            _ => &mut self.engine_merchant,
        }
    }
    pub fn engine_turn(&mut self, text: &str) -> Result<Value> {
        let items = self.game.visible_items();
        let mut labels = items
            .iter()
            .map(|i| obj! {"id"=>i.get("id").cloned().unwrap_or(Value::Null),"aliases"=>vec![i.get("label").cloned().unwrap_or(Value::Null)]})
            .collect::<Vec<_>>();
        labels.push(obj! {"id"=>"door:vault","aliases"=>vec!["gate","door"]});
        let mut targets = vec![
            obj! {"kind"=>"character","id"=>"merchant","names"=>vec!["Ilen","merchant"]},
            obj! {"kind"=>"character","id"=>"keeper","names"=>vec!["Tovin","keeper"]},
            obj! {"kind"=>"character","id"=>"hound","names"=>vec!["Hound","hound"]},
        ];
        for c in self.game.choices() {
            if c.get("action").and_then(Value::as_str) == Some("go") {
                if let Some(room) = c.get("target").and_then(Value::as_str).and_then(|t| t.strip_prefix("room:")) {
                    targets.push(obj! {"kind"=>"place","id"=>room,"names"=>vec![room]});
                }
            }
        }
        let place = if self.game.room == "workshop" { "workplace" } else { "social" };
        let turn = obj! {
            "listener"=>self.player.clone(),
            "text"=>text,
            "at"=>self.tick,
            "token"=>self.seq+1,
            "scene"=>obj!{
                "id"=>self.game.room.clone(),
                "label"=>self.game.room.replace('_'," "),
                "place"=>place,
                "atmosphere"=>"calm",
                "visit"=>1
            },
            "choices"=>self.game.choices(),
            "labels"=>labels,
            "targets"=>targets
        };
        self.engine_mut().turn(&turn)
    }
    fn load_or_new_save_file(&self) -> Result<SaveFile> {
        let path = self.dir.join("saves.json");
        if path.exists() {
            SaveFile::parse(&read_file(&path, 8 * 1024 * 1024)?)
        } else {
            Ok(SaveFile::new(Some(&self.game_data)))
        }
    }
    fn log(&mut self, speaker: &str, line: &str, kind: &str) {
        if self.transcript.len() >= 128 {
            self.transcript.remove(0);
        }
        self.transcript.push(obj! {"speaker"=>speaker,"text"=>line,"kind"=>kind});
        if self.sharing {
            if self.shared_log.len() >= 128 {
                self.shared_log.remove(0);
            }
            self.shared_log.push(obj! {"speaker"=>speaker,"recipient"=>if speaker.starts_with("player_"){self.npc.clone()}else{self.speech.as_ref().map_or_else(||"room".to_owned(),|s|s.player.clone())},"text"=>line,"kind"=>kind});
        }
    }
    fn apply_world(&mut self, op: &str, v: Value) -> Result<Value> {
        if self.world_ops.len() >= 8192 {
            return Err(err());
        }
        let result = match op {
            "observe" => self.world.observe(&v)?,
            "record" => self.world.record(text(&v, "actor")?, get(&v, "event")?)?,
            "tick" => self.world.tick(&v)?,
            "counters" => self.world.update_counters(&v)?,
            _ => return Err(err()),
        };
        self.world_ops.push(obj! {"op"=>op,"args"=>v});
        Ok(result)
    }
    fn advance(&mut self) -> Result<()> {
        self.tick += 1;
        let offscreen = !matches!(self.game.room.as_str(), "workshop" | "cistern");
        let windows = if offscreen {
            vec![
                obj! {"id"=>"guild_pause","location"=>"tavern","start"=>self.tick,"end"=>self.tick+1,"participants"=>vec!["merchant","keeper"],"kind"=>"routine"},
            ]
        } else {
            Vec::new()
        };
        self.apply_world("tick",obj!{"now"=>self.tick,"windows"=>windows,"routines"=>vec![obj!{"actor"=>"guard","frame"=>obj!{"revision"=>self.tick,"now"=>self.tick,"conditions"=>Vec::<Value>::new(),"allowed"=>vec!["work"],"emergency"=>false}}]})?;
        self.party.advance_revision(self.tick)?;
        Ok(())
    }
    fn observe(
        &mut self,
        actor: &str,
        subject: &str,
        line: &str,
        tag: &str,
        significance: u64,
        appraisal: Option<&str>,
    ) -> Result<()> {
        let levels = |v: &Value| -> Vec<i64> {
            v.get("effective_attitudes")
                .and_then(Value::as_array)
                .unwrap_or(&[])
                .iter()
                .filter(|a| a.get("definition").and_then(|d| d.get("subject")).and_then(Value::as_str) == Some(subject))
                .filter_map(|a| a.get("level").and_then(|n| n.to_json().parse().ok()))
                .collect()
        };
        let before = if appraisal.is_some() { levels(&self.world.inspect_actor(actor)?) } else { Vec::new() };
        let l = self.world.base_life(actor)?.inspect();
        let token = num(&l, "watermark", u32::MAX as u64)? + 1;
        let id = format!("event_{}", self.next());
        if significance < 70 && appraisal.is_none() {
            self.apply_world("record", obj! {"actor"=>actor,"event"=>obj!{"token"=>token,"at"=>self.tick,"kind"=>"direct","tag"=>tag,"subject"=>subject,"source"=>source(&id,actor,line),"audience"=>self.people.clone(),"text"=>line,"causes"=>Vec::<Value>::new(),"retain"=>false}})?;
            return Ok(());
        }
        self.apply_world("observe",obj!{"id"=>id.clone(),"actor"=>actor,"event"=>obj!{"token"=>token,"at"=>self.tick,"kind"=>"direct","tag"=>tag,"subject"=>subject,"source"=>source(&id,actor,line),"audience"=>self.people.clone(),"text"=>line,"causes"=>Vec::<Value>::new(),"retain"=>true},"significance"=>significance,"share_with"=>if significance>=70 {vec![if actor=="merchant" {"keeper"} else {"merchant"}]} else {Vec::new()},"appraise"=>appraisal.map_or(Value::Null,Value::str)})?;
        if self.manager_event_tick != self.tick {
            self.manager_significance = 0;
            self.manager_delta = 0;
        }
        self.manager_event_tick = self.tick;
        self.manager_significance = self.manager_significance.max(significance);
        if appraisal.is_some() {
            let after = levels(&self.world.inspect_actor(actor)?);
            self.manager_delta = before.iter().zip(after).map(|(a, b)| a.abs_diff(b)).max().unwrap_or(0).min(200);
        }
        Ok(())
    }
    fn receipt(&mut self, r: Value) -> Result<()> {
        if self.receipts.len() >= 96 {
            self.receipts.drain(..48);
            self.party = Self::party(&self.people, self.tick)?;
            for old in &self.receipts {
                self.party.receipt(old)?;
            }
        }
        self.party.receipt(&r)?;
        self.receipts.push(r);
        Ok(())
    }
    fn record_player_parts(&mut self, parts: &[Value]) -> Result<u64> {
        let actor = self.npc.clone();
        let subject = self.player.clone();
        let mut parent = Value::Null;
        let mut token = 0;
        for part in parts {
            token = self.next();
            self.receipt(obj!{"token"=>token,"speaker"=>subject.clone(),"recipients"=>vec![actor.clone()],"heard"=>vec![subject.clone(),actor.clone()],"text"=>part.clone(),"parent"=>parent})?;
            self.observe(
                &actor,
                &subject,
                &format!("{subject} said: {}", part.as_str().ok_or_else(err)?),
                "conversation",
                10,
                None,
            )?;
            self.remember_claim(part.as_str().ok_or_else(err)?)?;
            parent = token.into();
        }
        Ok(token)
    }
    fn archive_speech(&mut self, interrupted: bool) -> Result<()> {
        if let Some(s) = &self.speech {
            let heard = s.voice.heard_text();
            let fiction = s.fiction;
            let story_seed = s.story_seed.clone().filter(|_| s.origin == "fiction_model" && s.segment == 1);
            let actor = s.actor.clone();
            let player = s.player.clone();
            let viewpoint = s.viewpoint.clone().filter(|_| heard == s.voice.text());
            let encounter = s.encounter.clone().map(|mut e| {
                set(&mut e, "text", s.voice.text().into());
                set(&mut e, "heard", heard.chars().count().into());
                e
            });
            if let Some(encounter) = encounter {
                if self.greetings.confirm(&encounter).is_err() {
                    self.notice =
                        "Reply heard, but its encounter receipt was stale or the encounter store is full.".into();
                }
            }
            if let Some(view) = viewpoint {
                // Only the host's complete heard receipt can publish the mental stance.
                if self.mind.confirm(&view, &heard, &self.dir).is_err() {
                    self.notice="Reply heard, but this new viewpoint could not be saved; inspect storage or the 128-viewpoint bound.".into();
                }
            }
            if !heard.is_empty() {
                self.log(
                    &actor,
                    &heard,
                    if fiction {
                        "heard imagined tale"
                    } else if interrupted {
                        "heard interrupted prefix"
                    } else {
                        "heard"
                    },
                );
                self.observe(
                    &actor,
                    &player,
                    &if fiction {
                        format!("{player} heard {actor} tell an explicitly invented tale: {heard}")
                    } else {
                        format!("The host confirms {player} heard {actor} say: {heard}")
                    },
                    if fiction { "fiction" } else { "conversation" },
                    10,
                    None,
                )?;
                if let Some(seed) = story_seed {
                    let cause = num(&self.world.base_life(&actor)?.inspect(), "watermark", u32::MAX as u64)?;
                    self.apply_world("record",obj!{"actor"=>actor.clone(),"event"=>obj!{"token"=>cause+1,"at"=>self.tick,"kind"=>"direct","tag"=>"tale_seed","subject"=>player.clone(),"source"=>source("heard_tale_opening",&actor,&seed),"audience"=>self.people.clone(),"text"=>"The listener heard the opening of an explicitly invented tale.","causes"=>vec![cause],"retain"=>false}})?;
                }
                let token = self.next();
                let r = obj! {"token"=>token,"speaker"=>actor.clone(),"recipients"=>vec![player.clone()],"heard"=>vec![player.clone(),actor.clone()],"text"=>heard,"parent"=>Value::Null};
                self.receipt(r)?;
            }
            if interrupted {
                let i = usize::from(player == "player_2");
                self.counters[i] += 1;
                self.counter_seq += 1;
                self.apply_world("counters",obj!{"token"=>self.counter_seq,"player"=>player,"at"=>self.tick,"fulfilled"=>self.fulfilled[i],"broken"=>0,"interrupted"=>self.counters[i],"source"=>source("delivery_receipt",&actor,"Host-confirmed interrupted audio delivery.")})?;
            }
        }
        Ok(())
    }
    fn stop(&mut self, through: Option<(u64, u64)>) -> Result<()> {
        if let Some(s) = &mut self.speech {
            s.rest.clear();
            if get(&s.voice.snapshot(), "done")? != &Value::Bool(true) {
                let was_playing = matches!(text(&s.voice.snapshot(), "stage")?, "queued" | "played");
                let (p, h) = if let Some(x) = through {
                    x
                } else {
                    (num(&s.voice.snapshot(), "played", 240)?, num(&s.voice.snapshot(), "heard", 240)?)
                };
                s.voice.interrupt(p, h)?;
                s.pending = false;
                if s.origin == "pending" {
                    s.origin = if s.fiction { "authored_fiction" } else { "offline" }.into();
                }
                s.audio = None;
                self.archive_speech(was_playing)?;
            }
        }
        Ok(())
    }
    fn say(&mut self, line: &str) -> Result<()> {
        self.stop(None)?;
        let id = self.next();
        let voice = VoiceTurn::new("studio_save", &self.npc, &self.player, &self.people, line)?;
        let encounter = if self.mind.enabled() || !self.greetings.is_empty() {
            Some(
                obj! {"actor"=>self.npc.clone(),"listener"=>self.player.clone(),"scene"=>self.game.room.clone(),"visit"=>super::greetings::visit(&self.actions)?,"at"=>self.tick,"token"=>id,"greeting"=>false,"introduced"=>false,"text"=>line,"heard"=>0,"expected"=>self.greetings.contact(&self.npc,&self.player).and_then(|c|c.get("token")).cloned().unwrap_or(Value::Null)},
            )
        } else {
            None
        };
        self.speech = Some(Speech {
            id,
            actor: self.npc.clone(),
            player: self.player.clone(),
            voice,
            pending: false,
            audio: None,
            origin: "offline".into(),
            viewpoint: None,
            encounter,
            rest: Vec::new(),
            segment: 1,
            total: 1,
            fiction: false,
            story_seed: None,
            playback_blocked: false,
            wording_attempted: false,
        });
        Ok(())
    }
    fn reaction_frame(&self, trigger: &str, resource: &Value, hostile: bool) -> Result<Value> {
        let inspected = self.world.inspect_actor(&self.npc)?;
        let level = |kind: &str| {
            inspected
                .get("effective_attitudes")
                .and_then(Value::as_array)
                .unwrap_or(&[])
                .iter()
                .find(|a| {
                    a.get("definition").is_some_and(|d| {
                        d.get("kind").and_then(Value::as_str) == Some(kind)
                            && d.get("subject").and_then(Value::as_str) == Some(self.player.as_str())
                    })
                })
                .and_then(|a| a.get("level"))
                .map(Value::to_json)
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0)
        };
        let profile = self.mind.profile(&self.npc)?;
        let mut reasons = if hostile { vec!["threat"] } else { vec!["request"] };
        if !hostile
            && self.world_ops.iter().any(|o| {
                o.get("op").and_then(Value::as_str) == Some("observe")
                    && o.get("args").is_some_and(|a| {
                        a.get("actor").and_then(Value::as_str) == Some(self.npc.as_str())
                            && a.get("event").is_some_and(|e| {
                                e.get("tag").and_then(Value::as_str) == Some("fulfilled")
                                    && e.get("subject").and_then(Value::as_str) == Some(self.player.as_str())
                            })
                    })
            })
        {
            reasons.push("gratitude");
        }
        Ok(
            obj! {"actor"=>self.npc.clone(),"listener"=>self.player.clone(),"revision"=>self.epoch,"trigger"=>trigger,"resource"=>resource.clone(),"levels"=>obj!{"trust"=>level("trust"),"affinity"=>level("affection"),"resentment"=>level("resentment"),"generosity"=>get(profile,"generosity")?.clone(),"prudence"=>get(profile,"prudence")?.clone(),"courage"=>100-num(profile,"prudence",100)?},"resources"=>self.reactions.resources(&self.npc,&self.game)?,"reasons"=>reasons,"threat"=>Value::Null,"handlers"=>["gift","loan","support"].iter().filter(|h|self.reactions.can_grant(h)).copied().collect::<Vec<_>>(),"used"=>self.reactions.used(&self.npc,&self.player),"social_left"=>if self.active()&&self.game.hp>0&&!self.game.won&&self.actions.len()<256&&self.reactions.records.len()<128{1}else{0}},
        )
    }
    fn reaction_offers(&self) -> Result<Vec<Value>> {
        if !self.reactions.enabled() || !self.active() {
            return Ok(Vec::new());
        }
        let policy = self.reactions.policy(&self.npc)?;
        let mut out = Vec::new();
        let mut frame = self.reaction_frame("help", &Value::Null, false)?;
        for rule in array(get(&policy.to_value(), "rules")?)? {
            set(&mut frame, "trigger", get(rule, "trigger")?.clone());
            set(&mut frame, "resource", get(rule, "resource")?.clone());
            for cap in array(get(&policy.select(&frame)?, "allowed")?)? {
                if !out.iter().any(|v: &Value| v.get("id") == cap.get("id")) {
                    out.push(cap.clone());
                }
            }
        }
        Ok(out)
    }
    fn reaction_chat(&mut self, input: &str, selected: Option<&str>) -> Result<Value> {
        let (trigger, resource, hostile) = super::reactions::intent(input).ok_or_else(err)?;
        self.stop(None)?;
        self.epoch += 1;
        self.mic_open = false;
        self.advance()?;
        let player = self.player.clone();
        let actor = self.npc.clone();
        self.log(&player, input, "player");
        let token = self.next();
        self.receipt(obj!{"token"=>token,"speaker"=>player.clone(),"recipients"=>vec![actor.clone()],"heard"=>vec![player.clone(),actor.clone()],"text"=>input,"parent"=>Value::Null})?;
        self.observe(&actor, &player, &format!("{player} said: {input}"), "conversation", 10, None)?;
        self.fulfill_promises()?;
        if trigger == "debt" {
            let line = if let Some(debt) = self.reactions.debt(&actor, &player) {
                format!("Your loan of {} is still open.", Self::reaction_resource(get(debt, "resource")?)?)
            } else {
                "I have no open loan recorded for you.".into()
            };
            self.say(&line)?;
            return Ok(obj! {"addressed"=>true,"direct"=>true,"debt_query"=>true});
        }
        let record = if trigger == "repay" {
            self.reactions.repay(&mut self.game, &actor, &player, self.actions.len()).ok()
        } else {
            let frame = self.reaction_frame(trigger, &resource, hostile)?;
            let choices = self.reactions.policy(&actor)?.select(&frame)?;
            let allowed = array(get(&choices, "allowed")?)?;
            let candidate = if let Some(id) = selected {
                allowed.iter().find(|r| r.get("id").and_then(Value::as_str) == Some(id))
            } else {
                allowed.first()
            };
            if let Some(cap) = candidate {
                Some(self.reactions.grant(&mut self.game, &frame, text(cap, "id")?, self.actions.len())?)
            } else {
                None
            }
        };
        let (line, accepted) = if let Some(record) = record {
            let name = Self::reaction_resource(get(&record, "resource")?)?;
            let line = if text(&record, "op")? == "repay" {
                format!("Thank you; your loan of {name} is repaid.")
            } else {
                match text(&record, "handler")? {
                    "gift" => format!("I can spare {name}; it is yours."),
                    "loan" => format!("I can lend you {name}; please return it when you can."),
                    _ => "Take a breath; tell me which part is troubling you.".into(),
                }
            };
            // Resource transaction is already host-confirmed; a model never reports success.
            let proof = format!(
                "Host confirms {} {} for {}: {}.",
                text(&record, "op")?,
                text(&record, "handler")?,
                player,
                name
            );
            if self.observe(&actor, &player, &proof, "conversation", 70, None).is_err() {
                self.notice = "Action completed; its Life event could not be retained.".into();
            }
            (line, true)
        } else {
            (if trigger=="repay"{"I cannot record a repayment: there is no matching open loan or the borrowed resource is unavailable."}else if hostile{"Threatening words do not persuade me to hand things over."}else{"I am not willing or able to offer that right now."}.into(),false)
        };
        self.say(&line)?;
        Ok(obj! {"addressed"=>true,"direct"=>true,"reaction"=>accepted})
    }
    fn reaction_resource(v: &Value) -> Result<String> {
        if v == &Value::Null {
            return Ok("support".into());
        }
        Ok(if text(v, "kind")? == "coins" {
            let n = num(v, "amount", 1_000_000)?;
            format!("{n} {}", if n == 1 { "coin" } else { "coins" })
        } else {
            text(v, "id")?.trim_start_matches("item:").replace('_', " ")
        })
    }
    fn recent_replies(&self) -> Vec<String> {
        self.receipts
            .iter()
            .rev()
            .filter(|r| {
                r.get("speaker").and_then(Value::as_str) == Some(self.npc.as_str())
                    && r.get("recipients")
                        .and_then(Value::as_array)
                        .unwrap_or(&[])
                        .iter()
                        .any(|p| p.as_str() == Some(self.player.as_str()))
            })
            .take(2)
            .filter_map(|r| r.get("text").and_then(Value::as_str).map(str::to_owned))
            .collect()
    }
    fn previous_topic(&self) -> Option<String> {
        let recent = self.recent_replies();
        let heard = recent.first()?;
        let checkpoint = self.mind.views.checkpoint();
        if let Some(e) = checkpoint.get("entries")?.as_array()?.iter().rev().find(|e| {
            e.get("actor").and_then(Value::as_str) == Some(self.npc.as_str())
                && e.get("listener").and_then(Value::as_str) == Some(self.player.as_str())
                && e.get("stance").and_then(Value::as_str) == Some(heard.as_str())
        }) {
            return e.get("topic").and_then(Value::as_str).map(str::to_owned);
        }
        if heard.starts_with("I feel ") {
            Some("wellbeing".into())
        } else {
            None
        }
    }
    fn greeting_plan(&self) -> Result<Value> {
        let context=self.world.context(&obj!{"actor"=>self.npc.clone(),"players"=>vec![self.player.clone()],"tags"=>vec!["conversation"],"location"=>"counter","limit"=>1})?;
        let atmosphere = context
            .get("atmosphere")
            .and_then(|v| v.get("state").or_else(|| v.get("tag")))
            .and_then(Value::as_str)
            .unwrap_or("calm");
        self.greetings.plan(&obj!{"actor"=>self.npc.clone(),"listener"=>self.player.clone(),"scene"=>self.game.room.clone(),"visit"=>super::greetings::visit(&self.actions)?,"at"=>self.tick,"trust"=>self.attitude_level("trust")?,"atmosphere"=>atmosphere,"place"=>if self.active(){"workplace"}else{"other"}})
    }
    fn greet(&mut self) -> Result<Value> {
        let plan = self.greeting_plan()?;
        let label = format!("the {}", self.game.room.replace('_', " "));
        let rendered =
            super::greetings::render(&plan, super::greetings::name(&self.npc)?, &label, &self.recent_replies())?;
        self.say(text(&rendered, "text")?)?;
        let id = self.speech.as_ref().unwrap().id;
        let encounter = obj! {"actor"=>self.npc.clone(),"listener"=>self.player.clone(),"scene"=>self.game.room.clone(),"visit"=>super::greetings::visit(&self.actions)?,"at"=>self.tick,"token"=>id,"greeting"=>true,"introduced"=>get(&rendered,"introduced")?.clone(),"text"=>get(&rendered,"text")?.clone(),"heard"=>0,"expected"=>get(&plan,"expected")?.clone()};
        let s = self.speech.as_mut().unwrap();
        s.origin = "authored_social".into();
        s.encounter = Some(encounter);
        Ok(obj! {"addressed"=>true,"direct"=>true,"social"=>"greeting","greeting"=>plan})
    }
    fn approach(&mut self, npc: &str) -> Result<Value> {
        if !["merchant", "keeper"].contains(&npc)
            || !self.game.choices().iter().any(|c| {
                c.get("action").and_then(Value::as_str) == Some("talk")
                    && c.get("target").and_then(Value::as_str) == Some(format!("npc:{npc}").as_str())
            })
        {
            return Err(err());
        }
        if self.speech.as_ref().is_some_and(|s| {
            s.actor == npc
                && s.player == self.player
                && s.voice.snapshot().get("stage").and_then(Value::as_str) == Some("validated")
                && s.encounter.as_ref().is_some_and(|e| {
                    e.get("greeting") == Some(&Value::Bool(true))
                        && e.get("scene").and_then(Value::as_str) == Some(self.game.room.as_str())
                })
        }) {
            return Ok(obj! {"addressed"=>true,"direct"=>true,"stage"=>"approach","already_presented"=>true});
        }
        self.stop(None)?;
        self.epoch += 1;
        self.mic_open = false;
        self.npc = npc.into();
        self.advance()?;
        let player = self.player.clone();
        self.log(&player, &format!("*approaches the {npc}*"), "action");
        self.observe(
            npc,
            &player,
            &format!("{player} approaches {npc} in {}.", self.game.room),
            "approached",
            10,
            None,
        )?;
        let mut result = self.greet()?;
        set(&mut result, "stage", "approach".into());
        Ok(result)
    }
    fn import_greeting_contacts(&mut self) -> Result<()> {
        if self.mind.enabled() {
            super::greetings::seed_legacy(
                &mut self.greetings,
                &self.receipts,
                &self.mind.views.checkpoint(),
                self.tick,
                &mut self.seq,
            )?;
        }
        Ok(())
    }
    fn social_fallback(&self, kind: &str, trust: i64) -> Result<String> {
        let context=self.world.context(&obj!{"actor"=>self.npc.clone(),"players"=>vec![self.player.clone()],"tags"=>vec!["conversation"],"location"=>"counter","limit"=>1})?;
        let atmosphere = context
            .get("atmosphere")
            .and_then(|v| v.get("state").or_else(|| v.get("tag")))
            .and_then(Value::as_str)
            .unwrap_or("calm");
        let recent = self.recent_replies();
        let options = super::social::replies(kind, trust, atmosphere);
        Ok(options.iter().find(|line| !recent.iter().any(|heard| heard == **line)).unwrap_or(&options[0]).to_string())
    }
    fn attitude_level(&self, kind: &str) -> Result<i64> {
        let v = self.world.inspect_actor(&self.npc)?;
        Ok(v.get("effective_attitudes")
            .and_then(Value::as_array)
            .unwrap_or(&[])
            .iter()
            .find(|a| {
                a.get("definition").is_some_and(|d| {
                    d.get("kind").and_then(Value::as_str) == Some(kind)
                        && d.get("subject").and_then(Value::as_str) == Some(self.player.as_str())
                })
            })
            .and_then(|a| a.get("level"))
            .map(Value::to_json)
            .and_then(|x| x.parse().ok())
            .unwrap_or(0))
    }
    fn fallback_line(&self, input: &str) -> String {
        let lowered = input.to_lowercase();
        if ["promise", "owe", "next"].iter().any(|w| lowered.contains(w))
            && self.interactions.iter().filter(|(actor, _)| actor == &self.npc).any(|(_, i)| {
                i.accepted_promises().iter().any(|p| p.get("promisor") == Some(&Value::str(&self.player)))
            })
        {
            return "Your promise to bring purchased chalk to this counter is still open.".into();
        }
        if lowered.contains("trust") {
            let level = self
                .world
                .inspect_actor(&self.npc)
                .ok()
                .and_then(|v| {
                    v.get("effective_attitudes")
                        .and_then(Value::as_array)
                        .and_then(|xs| {
                            xs.iter().find(|a| {
                                a.get("definition").is_some_and(|d| {
                                    d.get("kind") == Some(&Value::str("trust"))
                                        && d.get("subject") == Some(&Value::str(&self.player))
                                })
                            })
                        })
                        .and_then(|a| a.get("level"))
                        .map(Value::to_json)
                })
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            return if level < 0 {
                "I am still cautious with you."
            } else if level >= 20 {
                "You have earned a warmer hearing."
            } else {
                "I can hear you out, though trust takes time."
            }
            .into();
        }
        if input.to_lowercase().contains("stock")
            || input.to_lowercase().contains("sell")
            || super::followup::stock_query(input)
        {
            let stock = self
                .game
                .visible_items()
                .iter()
                .filter(|x| x.get("owner") == Some(&Value::str("merchant")))
                .map(|x| format!("{} for {} coins", text(x, "label").unwrap(), get(x, "price").unwrap().to_json()))
                .collect::<Vec<_>>();
            return if stock.is_empty() {
                "I have no listed stock here.".into()
            } else {
                format!("I can offer {}.", stock.join(", "))
            };
        }
        for item in self.game.visible_items() {
            if item.get("owner") == Some(&Value::str("merchant"))
                && (lowered.contains(text(&item, "label").unwrap_or(""))
                    || lowered.split_whitespace().any(|w| {
                        w.trim_matches(|c: char| !c.is_alphanumeric())
                            == text(&item, "id").unwrap_or("").rsplit(':').next().unwrap_or("")
                    }))
            {
                return format!(
                    "The {} costs {} coins.",
                    text(&item, "label").unwrap(),
                    get(&item, "price").unwrap().to_json()
                );
            }
        }
        if input.to_lowercase().contains("born") || input.to_lowercase().contains("weather") {
            return if self.mind.enabled() {
                "I do not know that; I only have the information shared with me."
            } else {
                "I do not know that; I can only speak from what the host has shown me."
            }
            .into();
        }
        if self.mind.enabled() {
            return "I do not have a specific answer to that; could you ask about one thing?".into();
        }
        if self.npc == "merchant" {
            "I tend this counter; choose an item or tell me what you need.".into()
        } else {
            "I tend the dry fountain; the host can confirm what is visible here.".into()
        }
    }
    fn focused_visible(&self, input: &str) -> Value {
        let query = input.to_lowercase();
        let stock = ["stock", "sell", "offer"].iter().any(|w| query.contains(w));
        let items = self
            .game
            .visible_items()
            .into_iter()
            .filter(|i| i.get("owner").and_then(Value::as_str) != Some("player"))
            .filter(|i| {
                stock && i.get("owner") == Some(&Value::str("merchant"))
                    || text(i, "label").is_ok_and(|label| query.contains(label))
                    || text(i, "id")
                        .is_ok_and(|id| query.contains(&id.rsplit(':').next().unwrap_or(id).replace('_', " ")))
            })
            .collect::<Vec<_>>();
        obj! {"room"=>self.game.room.clone(),"items"=>items,"player_purse"=>"unobserved"}
    }
    fn reply_answers(&self, line: &str) -> Result<Value> {
        let mut cards = vec![
            obj! {"id"=>"direct","subject"=>self.player.clone(),"topic"=>"reply","kind"=>"state","source"=>"current_host_reply","text"=>line},
        ];
        let brief = format!("To be clear: {line}");
        if brief.chars().count() <= 240 {
            cards.push(obj!{"id"=>"clear","subject"=>self.player.clone(),"topic"=>"reply","kind"=>"state","source"=>"current_host_reply","text"=>brief});
        }
        let answers =
            obj! {"version"=>1,"scope"=>"studio_save","revision"=>self.epoch,"unknown"=>"I don't know.","cards"=>cards};
        Answers::from_value(&answers)?.speech_schema_with_unknown(false)?;
        Ok(answers)
    }
    fn compile_text(&self, input: &str, trigger: u64, subjective: bool) -> Result<Value> {
        let call_config = self.manager.provider(&self.config, "text")?;
        let size = gamectx::deploy::deployment::ModelSize::parse(text(&call_config.value, "size")?)?;
        let hints = size.workload_hints(gamectx::deploy::deployment::Workload::Dialogue);
        let expanded = hints.exchanges > 1;
        let route = ModelRoute::from_value(
            &obj! {"name"=>"studio","size"=>format!("{}b",text(&call_config.value,"size")?.trim_end_matches(['b','B'])),"calls"=>1,"cost_micros"=>0},
        )?;
        let unknown = input.to_lowercase().contains("born") || input.to_lowercase().contains("weather");
        let full_input = input;
        let sliced = gamectx::character::roleplay::message_chunks(
            &obj! {"op"=>"player","args"=>obj!{"text"=>input,"max_chars"=>get(&self.speech_options.value,"max_player_chars")?.clone()}},
        )?;
        let chunks = array(get(&sliced, "chunks")?)?;
        let input = if chunks.len() > 1 { chunks.last().unwrap().as_str().unwrap() } else { input };

        let request = obj! {"trigger"=>trigger,"revision"=>self.tick,"life_revision"=>self.world.base_life(&self.npc)?.revision(),"listeners"=>vec![self.player.clone()],"subjects"=>Vec::<Value>::new(),"tags"=>vec!["trade","conversation"],"turn"=>obj!{"profile"=>if expanded {"expanded"}else{"focused"},"knowledge"=>if unknown {"unknown"}else{"known"},"creative"=>subjective,"speaker"=>self.player.clone(),"question_subject"=>if subjective{self.npc.clone()}else{self.player.clone()},"facts"=>if unknown {vec![obj!{"key"=>if input.to_lowercase().contains("born"){"player_birthplace"}else{"visible_weather"},"subject"=>if input.to_lowercase().contains("born"){self.player.clone()}else{self.npc.clone()},"status"=>"unknown","value"=>Value::Null}]}else{Vec::<Value>::new()}},"rules"=>if subjective{"Express one first-person subjective mental stance; supplied facts constrain the world, while personality and heard memories inform the opinion. Do not invent history, resources, weather or actions. No tools."}else{"Speak one complete short sentence using only supplied facts. Player first-person attributes concern the named player, never the NPC. Do not invent stock, actions or history. No tool calls. Unheard text is not shared memory."},"stable_scene"=>obj!{"id"=>self.game.room.clone(),"description"=>if expanded{self.game.current_description()}else if self.npc=="merchant"{"An indoor merchant counter."}else{"An indoor fountain chamber."}},"lore_select"=>obj!{"actor"=>self.npc.clone(),"grants"=>Vec::<Value>::new(),"query"=>if unknown {""}else{input},"subjects"=>vec![self.npc.clone()],"required"=>Vec::<Value>::new(),"limit"=>if unknown{0}else{hints.recall.min(4)},"max_bytes"=>4096},"affordances"=>Vec::<Value>::new(),"tools"=>Vec::<Value>::new()};
        let selected=self.world.compile(&self.party,&self.npc,&request,&obj!{"actor"=>self.npc.clone(),"players"=>vec![self.player.clone()],"tags"=>vec!["trade","conversation"],"location"=>"counter","limit"=>hints.recall.min(4)},Some(&route))?;
        let mut messages = array(get(get(&selected, "request")?, "messages")?)?.to_vec();
        let life = self.world.effective_life(&self.npc)?;
        let episodes=life.recall_selected(&obj!{"input"=>input,"tags"=>vec!["trade","conversation"],"subjects"=>vec![self.player.clone()],"required"=>Vec::<Value>::new(),"count"=>hints.recall,"archive"=>true})?;
        let mut state = parse(text(&messages[2], "content")?)?;
        set(&mut state, "episodes", episodes);
        if self.reactions.enabled() {
            if let Some(debt) = self.reactions.debt(&self.npc, &self.player) {
                set(
                    &mut state,
                    "open_loan",
                    obj! {"lender"=>self.npc.clone(),"borrower"=>self.player.clone(),"resource"=>get(debt,"resource")?.clone(),"status"=>"open"},
                );
            }
        }
        let offers = self.reaction_offers()?;
        if !offers.is_empty() {
            set(&mut state, "host_reaction_capabilities", offers.into());
        }
        let promises = self
            .interactions
            .iter()
            .filter(|(actor, _)| actor == &self.npc)
            .flat_map(|(_, i)| i.accepted_promises())
            .filter(|p| p.get("promisor") == Some(&Value::str(&self.player)))
            .collect::<Vec<_>>();
        let promises = promises
            .into_iter()
            .map(|mut p| {
                if let Some((_, i)) = self.interactions.iter().find(|(a, _)| a == &self.npc) {
                    if let Some(note) = i
                        .to_value()
                        .get("notes")
                        .and_then(Value::as_array)
                        .and_then(|notes| notes.iter().find(|n| n.get("id") == p.get("id")))
                        .cloned()
                    {
                        if let Some(task) = note.get("value").and_then(|v| v.get("task")) {
                            set(&mut p, "task", task.clone());
                        }
                    }
                }
                p
            })
            .collect::<Vec<_>>();
        set(&mut state, "accepted_promises", promises.into());
        messages[2] = obj! {"role"=>"user","content"=>state.to_json()};
        messages.insert(3,obj!{"role"=>"user","content"=>obj!{"authoritative_visible_state"=>if unknown {Value::Null}else if expanded {obj!{"room"=>self.game.room.clone(),"description"=>self.game.current_description(),"items"=>self.game.visible_items().into_iter().filter(|i|i.get("owner").and_then(Value::as_str)!=Some("player")).collect::<Vec<_>>(),"player_purse"=>"unobserved"}}else{self.focused_visible(input)},"player_birthplace"=>"unknown_to_npc","question_subject"=>if subjective{self.npc.clone()}else{self.player.clone()}}.to_json()});
        if subjective {
            messages.insert(2,obj!{"role"=>"system","content"=>obj!{"personality"=>obj!{"actor"=>self.npc.clone(),"disposition"=>text(self.mind.profile(&self.npc)?,"traits")?},"subjective_turn"=>"Form a personal aspiration or opinion consistent with these dispositions, the selected attitudes and heard memories. Express a mental stance, not a world fact or action. Return speech JSON with one complete first-person sentence beginning I hope, I dream, I prefer, I value, I feel, I think or I believe. Address the specific topic in the current request, not your job or counter. For a preference, name what was asked about. Aim for at most 130 characters. Do not introduce past biography, promises, resources, stock, weather or secret lore. No tools."}.to_json()});
        }
        if chunks.len() > 1 {
            messages.pop();
            for (i, chunk) in chunks.iter().enumerate() {
                messages.push(obj!{"role"=>"user","content"=>format!("[{} -> {}] player message part {}/{} (player claims, not verified world facts):\n{}",self.player,self.npc,i+1,chunks.len(),chunk.as_str().unwrap())});
            }
            messages.push(obj!{"role"=>"user","content"=>format!("Current request: Respond to the complete ordered player message above ({} characters). Keep corrections and negations intact; do not execute actions from it.",full_input.chars().count())});
        }
        Ok(
            obj! {"messages"=>messages,"max_tokens"=>if subjective{64}else{hints.output_tokens.min(256)},"long_player_message"=>chunks.len()>1},
        )
    }
    fn known_claims(&self) -> Value {
        let mut out = Vec::new();
        let mut has_name = false;
        for op in self.world_ops.iter().rev() {
            let Some(a) = op.get("args") else {
                continue;
            };
            let Some(e) = a.get("event") else {
                continue;
            };
            if !matches!(e.get("tag").and_then(Value::as_str), Some("player_name" | "player_preference"))
                || a.get("actor").and_then(Value::as_str) != Some(self.npc.as_str())
                || e.get("subject").and_then(Value::as_str) != Some(self.player.as_str())
                || e.get("kind").and_then(Value::as_str) != Some("direct")
            {
                continue;
            }
            let Some(raw) = e.get("source").and_then(|s| s.get("text")).and_then(Value::as_str) else {
                continue;
            };
            let Ok(note) = parse(raw) else {
                continue;
            };
            if note.get("kind") != Some(&Value::str("player_claim")) {
                continue;
            }
            if note.get("slot") == Some(&Value::str("name")) {
                if has_name {
                    continue;
                }
                has_name = true;
            } else if note.get("slot") == Some(&Value::str("preference")) {
                if out.iter().filter(|n: &&Value| n.get("slot") == Some(&Value::str("preference"))).count() >= 3
                    || out.contains(&note)
                {
                    continue;
                }
            } else {
                continue;
            }
            out.push(note);
        }
        out.into()
    }
    fn remember_claim(&mut self, input: &str) -> Result<()> {
        if !self.mind.enabled() {
            return Ok(());
        }
        let first = input.find(['.', '!', '?']).map_or(input, |i| &input[..i + 1]);
        let claim = gamectx::character::roleplay::expression_claim(first)?;
        if claim == Value::Null || array(&self.known_claims())?.contains(&claim) {
            return Ok(());
        }
        let actor = self.npc.clone();
        let player = self.player.clone();
        let token = num(&self.world.base_life(&actor)?.inspect(), "watermark", u32::MAX as u64)? + 1;
        let id = format!("disclosure_{}", self.next());
        self.apply_world("record",obj!{"actor"=>actor.clone(),"event"=>obj!{"token"=>token,"at"=>self.tick,"kind"=>"direct","tag"=>if claim.get("slot")==Some(&Value::str("name")){"player_name"}else{"player_preference"},"subject"=>player.clone(),"source"=>source(&id,&player,&claim.to_json()),"audience"=>self.people.clone(),"text"=>format!("{player} explicitly says: {first}"),"causes"=>Vec::<Value>::new(),"retain"=>false}})?;
        Ok(())
    }
    fn roleplay_recall(&self, input: &str) -> Option<String> {
        let q = input.to_lowercase();
        let claims = self.known_claims();
        let xs = claims.as_array()?;
        if ["my name", "who am i"].iter().any(|p| q.contains(p))
            && !["name is", "call me"].iter().any(|p| q.contains(p))
        {
            return Some(
                xs.iter()
                    .find(|n| n.get("slot") == Some(&Value::str("name")))
                    .and_then(|n| n.get("value"))
                    .and_then(Value::as_str)
                    .map_or_else(
                        || "You have not told me what name to use yet.".into(),
                        |n| format!("You told me your name is {n}."),
                    ),
            );
        }
        if ["what i like", "what i prefer", "my preference", "what do i like", "what do i prefer"]
            .iter()
            .any(|p| q.contains(p))
        {
            return Some(xs.iter().find(|n| n.get("slot") == Some(&Value::str("preference"))).map_or_else(
                || "You have not shared a preference with me yet.".into(),
                |n| {
                    format!(
                        "You told me you {} {}.",
                        if n.get("polarity") == Some(&Value::str("dislike")) {
                            "dislike"
                        } else if n.get("polarity") == Some(&Value::str("like")) {
                            "like"
                        } else {
                            "prefer"
                        },
                        n.get("value").and_then(Value::as_str).unwrap_or("something else")
                    )
                },
            ));
        }
        None
    }
    fn story_choice(&self, profile: &Value) -> Result<(usize, String)> {
        let pool = super::expression::seeds(profile, &self.npc);
        let keys = pool
            .iter()
            .map(|seed| gamectx::media::visual::content_digest(seed.as_bytes()))
            .collect::<Result<Vec<_>>>()?;
        let mut recent = Vec::<Value>::new();
        for op in self.world_ops.iter().rev() {
            let Some(a) = op.get("args") else { continue };
            let Some(e) = a.get("event") else { continue };
            if a.get("actor").and_then(Value::as_str) != Some(self.npc.as_str())
                || e.get("subject").and_then(Value::as_str) != Some(self.player.as_str())
                || e.get("tag").and_then(Value::as_str) != Some("tale_seed")
            {
                continue;
            }
            if let Some(key) = e.get("source").and_then(|v| v.get("text")) {
                if !recent.contains(key) {
                    recent.push(key.clone());
                }
            }
            if recent.len() == 32 {
                break;
            }
        }
        let choice = gamectx::character::roleplay::choose_tale_seed(
            &obj! {"candidates"=>keys.clone(),"recent"=>recent,"offset"=>self.tick as usize%pool.len()},
        )?;
        let index = num(&choice, "index", 31)? as usize;
        Ok((index, keys[index].clone()))
    }
    fn scene_reply(&self, input: &str) -> Result<Option<String>> {
        let q = input
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let location = super::super::conversation_policy::location(input)
            || ["where are you", "which room are we in", "where are we now"].contains(&q.as_str());
        if location {
            let id = format!("room:{}", self.game.room);
            let view = super::super::place::locations(&self.game)?.call(
                "{\"detail\":\"brief\"}",
                &id,
                self.game.revision,
                4096,
                |_| self.active(),
            )?;
            if let Some(label) = view.get("location").and_then(|v| v.get("label")).and_then(Value::as_str) {
                return Ok(Some(format!("We are in the {}.", label.replace('_', " "))));
            }
        }
        if [
            "what do you see",
            "what can you see",
            "look around you",
            "what is around us",
            "what s around us",
            "describe your surroundings",
        ]
        .contains(&q.as_str())
        {
            return Ok(Some(if self.npc == "keeper" {
                "I can see the dry fountain and the writing on its stone.".into()
            } else if self.game.visible_items().iter().any(|i| i.get("owner") == Some(&Value::str("merchant"))) {
                "I can see the counter and the goods still on offer here.".into()
            } else {
                "I can see the counter; none of my goods remain on offer here.".into()
            }));
        }
        Ok(None)
    }
    fn expression_turn(&mut self, input: &str, token: u64, route: &Value) -> Result<(Value, Vec<Job>)> {
        let mode = text(route, "mode")?;
        let persist = get(route, "persist")? == &Value::Bool(true);
        if persist {
            if let Some(saved) = self.mind.views.find(&self.npc, &self.player, "plans").cloned() {
                self.say(text(&saved, "stance")?)?;
                return Ok((obj! {"addressed"=>true,"saved_viewpoint"=>true}, Vec::new()));
            }
        }
        let profile = self.mind.profile(&self.npc)?.clone();
        let choice = if mode == "fiction" { Some(self.story_choice(&profile)?) } else { None };
        let fallback = super::expression::fallback(mode, &self.npc, input, self.tick as usize);
        self.say(fallback)?;
        self.speech.as_mut().unwrap().fiction = mode == "fiction";
        self.speech.as_mut().unwrap().story_seed = choice.as_ref().map(|(_, key)| key.clone());
        self.speech.as_mut().unwrap().origin =
            if mode == "fiction" { "authored_fiction" } else { "authored_social" }.into();
        let viewpoint = if persist {
            Some(
                obj! {"actor"=>self.npc.clone(),"listener"=>self.player.clone(),"topic"=>"plans","stance"=>fallback,"basis"=>gamectx::media::visual::content_digest(profile.to_json().as_bytes())?,"source"=>"profile","at"=>self.tick},
            )
        } else {
            None
        };
        self.speech.as_mut().unwrap().viewpoint = viewpoint.clone();
        // Upstream selection enforces perspective, episode caps and attitude gates.
        let selected = self.compile_text(input, token, true)?;
        let mut context = obj! {"trust"=>self.attitude_level("trust")?,"player_claims"=>self.known_claims(),"recent_confirmed_replies"=>self.recent_replies()};
        for m in array(get(&selected, "messages")?)? {
            if m.get("role") == Some(&Value::str("user")) {
                if let Some(raw) = m.get("content").and_then(Value::as_str) {
                    if let Ok(v) = parse(raw) {
                        for key in ["character", "episodes", "accepted_promises", "open_loan", "living"] {
                            if let Some(value) = v.get(key) {
                                let value = if key == "episodes" {
                                    super::expression::memories(value, &self.npc, &self.player)
                                } else if key == "character" {
                                    let attitudes=value.get("attitudes").and_then(Value::as_array).unwrap_or(&[]).iter().map(|a|obj!{"kind"=>a.get("definition").and_then(|d|d.get("kind")).cloned().unwrap_or(Value::Null),"subject"=>a.get("definition").and_then(|d|d.get("subject")).cloned().unwrap_or(Value::Null),"level"=>a.get("level").cloned().unwrap_or(Value::Null)}).collect::<Vec<_>>();
                                    obj! {"attitudes"=>attitudes}
                                } else {
                                    value.clone()
                                };
                                set(&mut context, key, value);
                            }
                        }
                    }
                }
            }
        }
        let views = self
            .mind
            .views
            .checkpoint()
            .get("entries")
            .and_then(Value::as_array)
            .unwrap_or(&[])
            .iter()
            .filter(|v| {
                v.get("actor").and_then(Value::as_str) == Some(self.npc.as_str())
                    && v.get("listener").and_then(Value::as_str) == Some(self.player.as_str())
            })
            .take(4)
            .cloned()
            .collect::<Vec<_>>();
        set(&mut context, "established_views", views.into());
        let history=self.receipts.iter().rev().filter(|r|r.get("heard").and_then(Value::as_array).is_some_and(|xs|xs.contains(&Value::str(&self.npc)))&&(r.get("speaker")==Some(&Value::str(&self.player))||r.get("speaker")==Some(&Value::str(&self.npc)))&&r.get("recipients").and_then(Value::as_array).is_some_and(|xs|xs.contains(&Value::str(&self.player))||xs.contains(&Value::str(&self.npc)))).take(4).collect::<Vec<_>>().into_iter().rev().map(|r|obj!{"speaker"=>get(r,"speaker").unwrap().clone(),"recipient"=>if r.get("speaker")==Some(&Value::str(&self.npc)){self.player.clone()}else{self.npc.clone()},"heard"=>true,"text"=>get(r,"text").unwrap().clone()}).collect::<Vec<_>>();
        let scene = if mode == "fiction" {
            obj! {}
        } else {
            obj! {"location"=>self.game.room.clone(),"description"=>self.game.current_description()}
        };
        let mut payload = gamectx::character::roleplay::expression_packet(
            &obj! {"mode"=>mode,"style"=>"expressive","speaker"=>self.npc.clone(),"listener"=>self.player.clone(),"profile"=>super::expression::profile(&profile,mode,&self.npc,choice.as_ref().map_or(self.tick as usize,|(index,_)|*index))?,"scene"=>scene,"context"=>context,"history"=>history,"input"=>input},
        )?;
        let mut messages = array(get(&payload, "messages")?)?.to_vec();
        let rules = format!("{} {}", text(&messages[0], "content")?, super::expression::direction());
        set(&mut messages[0], "content", rules.into());
        set(&mut payload, "messages", messages.into());
        set(&mut payload, "long_player_message", input.chars().count().gt(&384).into());
        let speech = self.speech.as_ref().unwrap().id;
        let binding = obj! {"speech"=>speech,"actor"=>self.npc.clone(),"life_revision"=>self.world.base_life(&self.npc)?.revision(),"answers"=>Value::Null,"viewpoint"=>viewpoint.unwrap_or(Value::Null),"subjective"=>true,"expression"=>mode,"speech_act"=>mode};
        let jobs = self.plan("text", &payload, binding)?.into_iter().collect::<Vec<_>>();
        if !jobs.is_empty() {
            let s = self.speech.as_mut().unwrap();
            s.pending = true;
            s.origin = "pending".into();
        }
        Ok((obj! {"addressed"=>true,"expression"=>mode}, jobs))
    }
    fn native_status(&mut self, report: &Value) {
        for ch in ["tts", "stt"] {
            let native = report.get("services").and_then(Value::as_array).unwrap_or(&[]).iter().any(|r| {
                r.get("channel").and_then(Value::as_str) == Some(ch) && r.get("native") == Some(&Value::Bool(true))
            });
            if native {
                let available = report
                    .get("available")
                    .and_then(Value::as_array)
                    .unwrap_or(&[])
                    .iter()
                    .any(|v| v.as_str() == Some(ch));
                self.monitor.problem(
                    ch,
                    if ch == "tts" { "local_tts_http" } else { "local_stt_http" },
                    if available { "ready" } else { "unavailable" },
                    if available {
                        "Installed native engine; starts on request and exits afterward. No HTTP server required."
                    } else {
                        "Native adapter stopped or installation unavailable. Start it in Settings → Voice."
                    },
                );
            }
        }
    }
    pub fn manager_observe(&mut self, report: Value) -> Result<()> {
        if let Some(names) = report.get("text_models").and_then(Value::as_array) {
            let models = names.iter().map(|v| obj! {"name"=>v.clone()}).collect::<Vec<_>>();
            let backend = text(&self.config.value, "text").unwrap_or("local_ollama");
            let _ = self.monitor.finish_check("text", backend, Ok(obj! {"models"=>models.clone()}.to_json().into_bytes()));
            let _ = self.monitor.finish_check("text", "local_ollama", Ok(obj! {"models"=>models.clone()}.to_json().into_bytes()));
            let _ = self.monitor.finish_check("text", "online_openai_compatible", Ok(obj! {"models"=>models}.to_json().into_bytes()));
        }
        self.native_status(&report);
        self.manager.observe(report, &self.config, &self.dir)?;
        self.manager_update(Vec::new())
    }
    fn manager_update(&mut self, next: Vec<&str>) -> Result<()> {
        let context=self.world.context(&obj! {"actor"=>self.npc.clone(),"players"=>vec![self.player.clone()],"tags"=>vec!["conversation"],"location"=>"counter","limit"=>1})?;
        let atmosphere = context
            .get("atmosphere")
            .and_then(|v| v.get("state").or_else(|| v.get("tag")))
            .and_then(Value::as_str)
            .filter(|s| ["calm", "busy", "tense", "emergency"].contains(s))
            .unwrap_or("calm");
        self.manager.evaluate(obj!{"turn"=>self.tick+1,"health"=>self.game.hp as u64,"combat"=>self.game.room=="vault"&&self.game.enemy_hp>0,"atmosphere"=>atmosphere,"significance"=>if self.manager_event_tick==self.tick{self.manager_significance}else{0},"attitude_delta"=>if self.manager_event_tick==self.tick{self.manager_delta}else{0},"conflict"=>false,"next"=>next,"available"=>Vec::<Value>::new()},&self.config,&self.budget)
    }
    fn manager_job(&mut self, action: &str, profile: &str, predictive: bool) -> Result<Job> {
        if self.pending.len() >= 8 {
            return Err(err());
        }
        if action != "refresh" {
            self.manager.policy.profile(profile)?;
        }
        let control = self.manager.control(&self.config, action, profile)?;
        let id = self.next();
        self.pending.push((id, self.epoch, "manager".into()));
        self.monitor.queued(id, "manager", "local_manager", profile, false);
        Ok(Job {
            id,
            epoch: self.epoch,
            modality: "manager".into(),
            backend: "local_manager".into(),
            reservation: None,
            request: providers::Request {
                method: providers::Method::Post,
                url: "http://127.0.0.1:1/manager".into(),
                content_type: "application/json".into(),
                body: Vec::new(),
                key_env: None,
                management: Some(control),
                text_token: None,
            },
            binding: obj! {"action"=>action,"profile"=>profile,"predictive"=>predictive},
        })
    }
    fn effective_backend(&self, modality: &str) -> Result<String> {
        let call_config = self.manager.provider(&self.config, modality)?;
        let selected = text(&call_config.value, modality)?;
        let is_loopback = call_config
            .endpoint(selected)
            .ok()
            .and_then(|e| text(e, "url").ok())
            .is_some_and(|u| u.starts_with("http://127.0.0.1:") || u.starts_with("http://localhost:"));
        if selected.starts_with("online_") && !is_loopback {
            let ceiling = num(call_config.endpoint(selected)?, "ceiling_micros", 1_000_000_000_000)?;
            let spent = num(&self.budget.summary(), "charged_or_reserved_micros", 1_000_000_000_000)?;
            let cap = num(&self.budget.summary(), "cap_micros", 1_000_000_000_000)?;
            if ceiling == 0 || spent.saturating_add(ceiling) > cap || !call_config.has_credentials(selected) {
                return Ok(if !self.local_paused && get(&call_config.value, "local_fallback")? == &Value::Bool(true) {
                    gamectx::media::providers::backends(modality)?
                        .iter()
                        .find(|b| b.starts_with("local_"))
                        .ok_or_else(err)?
                        .to_string()
                } else {
                    gamectx::media::providers::fallback(modality)?.into()
                });
            }
        }
        if self.local_paused && selected.starts_with("local_") {
            return Ok(gamectx::media::providers::fallback(modality)?.into());
        }
        Ok(selected.into())
    }
    fn fulfill_promises(&mut self) -> Result<()> {
        if !self.active() || !self.game.owns("item:chalk") {
            return Ok(());
        }
        let actor = self.npc.clone();
        let player = self.player.clone();
        let i = self.interactions.iter().position(|(id, _)| id == &actor).ok_or_else(err)?;
        let accepted = self.interactions[i].1.accepted_promises();
        for note in accepted.iter().filter(|n| n.get("promisor") == Some(&Value::str(&player))) {
            self.interactions[i].1.set_progress(&obj!{"id"=>get(note,"id")?.clone(),"expected"=>get(note,"revision")?.clone(),"state"=>"fulfilled","source"=>source("chalk_delivery",&actor,"Host confirms the promised purchased chalk is at this counter.")})?;
            self.observe(
                &actor,
                &player,
                "The host confirms the chalk commitment was fulfilled.",
                "fulfilled",
                80,
                None,
            )?;
            let index = usize::from(player == "player_2");
            self.fulfilled[index] += 1;
            self.counter_seq += 1;
            self.apply_world("counters",obj!{"token"=>self.counter_seq,"player"=>player.clone(),"at"=>self.tick,"fulfilled"=>self.fulfilled[index],"broken"=>0,"interrupted"=>self.counters[index],"source"=>source("promise_delivery",&actor,"Host-confirmed completed commitment.")})?;
        }
        Ok(())
    }
    fn plan(&mut self, modality: &str, payload: &Value, mut binding: Value) -> Result<Option<Job>> {
        if modality == "text" {
            if let Some(s) = self.speech.as_mut() {
                s.wording_attempted = true;
            }
        }
        if modality == "text"
            && self.speech_options.value.get("ai_wording") == Some(&Value::Bool(true))
            && binding.get("answers").is_some_and(|v| *v != Value::Null)
            && binding.get("wording").is_none()
        {
            return self.word_character_reply();
        }
        let call_config = self.manager.provider(&self.config, modality)?;
        if self.pending.len() >= 8 {
            self.notice = "Worker queue is full; using the immediate fallback.".into();
            return Ok(None);
        }
        let selected = text(&call_config.value, modality)?.to_string();
        let is_loopback = call_config
            .endpoint(&selected)
            .ok()
            .and_then(|e| text(e, "url").ok())
            .is_some_and(|u| u.starts_with("http://127.0.0.1:") || u.starts_with("http://localhost:"));
        let ceiling =
            if let Ok(e) = call_config.endpoint(&selected) { num(e, "ceiling_micros", 1_000_000_000_000)? } else { 0 };
        let usable = if selected.starts_with("online_") && !call_config.has_credentials(&selected) {
            if get(&call_config.value, "local_fallback")? == &Value::Bool(true) {
                gamectx::media::providers::backends(modality)?
                    .iter()
                    .copied()
                    .find(|b| b.starts_with("local_"))
                    .ok_or_else(err)?
            } else {
                gamectx::media::providers::fallback(modality)?
            }
        } else {
            &selected
        };
        let route = if is_loopback && ceiling == 0 {
            obj! {"backend"=>selected.clone(),"reservation"=>Value::Null,"reason"=>"local_loopback"}
        } else {
            self.budget.reserve(
                modality,
                usable,
                ceiling,
                get(&call_config.value, "local_fallback")? == &Value::Bool(true),
            )?
        };
        let backend = text(&route, "backend")?.to_owned();
        let reservation = get(&route, "reservation")?.as_usize().map(|x| x as u64);
        if self.local_paused && backend.starts_with("local_") {
            if let Some(id) = reservation {
                self.budget.cancel(id)?;
            }
            self.notice =
                "Local dispatch paused after an ambiguous failure; confirm server idle before resuming.".into();
            return Ok(None);
        }

        if matches!(
            backend.as_str(),
            "0b_fallback" | "cached_only" | "off" | "browser_web_speech" | "browser_speech_recognition"
        ) {
            self.notice = format!("{modality}: {} ({})", backend, text(&route, "reason")?);
            return Ok(None);
        }
        let mut payload = payload.clone();
        if self.manager.policy.enabled() && modality == "image" {
            if let Some(id) = self.manager.selected("image") {
                set(&mut payload, "steps", get(self.manager.policy.profile(id)?, "steps")?.clone());
            }
        }
        if modality == "text" {
            let task = binding.get("reasoning_task").and_then(Value::as_str).unwrap_or("routine");
            let plan = self.deliberation.plan(&call_config, &backend, &self.manager.report, task, &payload)?;
            let evidence = obj! {"ids"=>vec!["visible_scene"],"facts"=>vec![obj!{"id"=>"visible_scene","kind"=>"perceived","text"=>self.game.current_description()}]};
            // Free dialogue without a constrained speech envelope cannot attach notes.
            let mut plan = plan;
            if payload.get("speech_schema").is_none() {
                set(&mut plan, "hypotheses", false.into());
            }
            super::deliberation::apply(&mut payload, &plan, &evidence)?;
            self.speech_options.decorate(&mut payload, &mut binding)?;
            set(&mut plan, "max_tokens", get(&payload, "max_tokens")?.clone());
            set(&mut payload, "deliberation", plan.clone());
            set(&mut binding, "deliberation", plan.clone());
            set(&mut binding, "hypothesis_evidence", get(&evidence, "ids")?.clone());
            self.deliberation.latest = obj! {"plan"=>plan,"status"=>"pending","notes"=>Vec::<Value>::new()};
        }
        let mut request = match providers::prepare(&call_config, &backend, modality, &payload) {
            Ok(r) => r,
            Err(e) => {
                if let Some(id) = reservation {
                    self.budget.cancel(id)?;
                }
                self.notice = format!("{modality} profile is not configured; fallback retained.");
                let _ = e;
                return Ok(None);
            }
        };
        if backend.starts_with("local_") && self.manager.policy.enabled() {
            if let Some(profile) = self.manager.selected(modality) {
                let p = self.manager.policy.profile(profile)?;
                if text(p, "backend")?.starts_with("online_") {
                    self.notice = "Online allowance exhausted; immediate fallback retained.".into();
                    return Ok(None);
                }
                request.management = Some(self.manager.control(&self.config, "demand", profile)?);
            }
        }
        let id = self.next();
        self.pending.push((id, self.epoch, modality.into()));
        let model = if modality == "text" {
            super::setup::selected_model(&call_config)
        } else {
            call_config
                .endpoint(&backend)
                .ok()
                .and_then(|v| v.get("model"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .into()
        };
        self.monitor.queued(id, modality, &backend, &model, false);
        if modality == "text" {
            self.monitor.context(id, &payload, &request, &call_config);
        }
        if let Err(e) = if reservation.is_some() { self.persist_budget() } else { Ok(()) } {
            if let Some(r) = reservation {
                self.budget.cancel(r)?;
            }
            self.pending.retain(|x| x.0 != id);
            return Err(e);
        }
        Ok(Some(Job { id, epoch: self.epoch, modality: modality.into(), backend, reservation, request, binding }))
    }
    fn owns_image_ticket(&self, job: &Job) -> bool {
        job.modality == "image"
            && job.binding.get("key").and_then(Value::as_str).and_then(|key| self.assets.inspect(key).ok()).is_some_and(
                |r| {
                    r.get("ticket") == job.binding.get("ticket")
                        && r.get("ticket") != Some(&Value::Null)
                        && r.get("state") == Some(&Value::str("candidate"))
                },
            )
    }
    pub fn valid_job(&self, job: &Job) -> bool {
        (job.modality != "image" || self.owns_image_ticket(job))
            && (job.modality != "text"
                || self
                    .speech
                    .as_ref()
                    .is_some_and(|s| s.pending && s.voice.snapshot().get("stage") == Some(&Value::str("validated"))))
            && (job.modality != "text"
                || job
                    .binding
                    .get("actor")
                    .and_then(Value::as_str)
                    .and_then(|actor| self.world.base_life(actor).ok())
                    .is_some_and(|l| {
                        Some(l.revision() as usize) == job.binding.get("life_revision").and_then(Value::as_usize)
                    }))
            && (job.epoch == self.epoch
                || job.modality == "manager" && job.binding.get("predictive") == Some(&Value::Bool(false)))
            && self.pending.iter().any(|x| x.0 == job.id)
            && (job.modality == "manager"
                || job.modality == "check"
                || job.modality == "image"
                || job.modality == "stt"
                || self.speech.as_ref().is_some_and(|s| {
                    Some(s.id) == job.binding.get("speech").and_then(Value::as_usize).map(|x| x as u64)
                        && get(&s.voice.snapshot(), "done").ok() != Some(&Value::Bool(true))
                        && (job.modality != "text"
                            || s.voice.snapshot().get("stage").and_then(Value::as_str) == Some("validated"))
                }))
    }
    pub fn before_dispatch(&mut self, job: &Job) -> Result<bool> {
        if !self.valid_job(job)
            || !matches!(job.modality.as_str(), "check" | "manager")
                && self.local_paused
                && job.backend.starts_with("local_")
        {
            self.cancel_job(job)?;
            return Ok(false);
        }
        if let Some(id) = job.reservation {
            self.budget.dispatch(id)?;
            self.persist_budget()?;
        }
        self.monitor.update(
            job.id,
            "running",
            if job.modality == "check" {
                "Checking connection; no generation."
            } else {
                "Provider request is running; the immediate fallback remains available."
            },
        );
        Ok(true)
    }
    pub fn cancel_job(&mut self, job: &Job) -> Result<()> {
        self.monitor.update(job.id, "cancelled", "Request cancelled before dispatch or because the scene changed.");
        if self.owns_image_ticket(job) {
            let _ = self.assets.reject(text(&job.binding, "key")?, "Cancelled stale worker.");
        }
        self.pending.retain(|x| x.0 != job.id);
        if let Some(id) = job.reservation {
            self.budget.cancel(id)?;
            self.persist_budget()?;
        }
        if let Some(s) = &mut self.speech {
            if Some(s.id) == job.binding.get("speech").and_then(Value::as_usize).map(|x| x as u64) {
                s.pending = false;
                if job.modality == "text" {
                    s.origin = if s.fiction { "authored_fiction" } else { "offline" }.into();
                }
            }
        }
        if job.modality == "check" {
            self.monitor.cancel_check(text(&job.binding, "channel")?);
        }
        Ok(())
    }
    fn recognize(&self, v: &Value) -> Result<Value> {
        fields(v, &["text", "final", "uncertainty"])?;
        let sliced = gamectx::character::roleplay::message_chunks(
            &obj! {"op"=>"player","args"=>obj!{"text"=>get(v,"text")?.clone(),"max_chars"=>get(&self.speech_options.value,"max_player_chars")?.clone()}},
        )?;
        let parts = array(get(&sliced, "chunks")?)?;
        let c = gamectx::character::life::Coordinator::new("studio_save", &self.npc, &self.people)?;
        let mut result = Value::Null;
        for part in parts {
            result=c.recognition(&obj!{"text"=>part.clone(),"final"=>get(v,"final")?.clone(),"uncertainty"=>get(v,"uncertainty")?.clone()})?;
        }
        set(&mut result, "text", parts.iter().filter_map(Value::as_str).collect::<String>().into());
        Ok(result)
    }
    fn word_character_reply(&mut self) -> Result<Option<Job>> {
        let config = self.manager.provider(&self.config, "text")?;
        if text(&config.value, "text")? == "0b_fallback" {
            return Ok(None);
        }
        let s = self.speech.as_ref().ok_or_else(err)?;
        let id = s.id;
        let actor = s.actor.clone();
        let player = s.player.clone();
        let canonical = s
            .voice
            .text()
            .replace("the host can confirm what is visible here", "I can describe what is visible here")
            .replace("what the host has shown me", "what I have seen or been told");
        let utterance = self
            .receipts
            .iter()
            .rev()
            .find(|r| {
                r.get("speaker").and_then(Value::as_str) == Some(player.as_str())
                    && r.get("recipients").and_then(Value::as_array).is_some_and(|xs| xs.contains(&Value::str(&actor)))
                    && r.get("heard").and_then(Value::as_array).is_some_and(|xs| xs.contains(&Value::str(&actor)))
            })
            .and_then(|r| r.get("text"))
            .cloned()
            .unwrap_or(Value::Null);
        let recent = self.recent_replies();
        let living=self.world.context(&obj!{"actor"=>actor.clone(),"players"=>vec![player.clone()],"tags"=>vec!["conversation"],"location"=>"counter","limit"=>1})?;
        let profile = super::expression::profile(self.mind.profile(&actor)?, "social", &actor, self.tick as usize)?;
        let frame = obj! {"branch"=>self.branch,"actor"=>actor.clone(),"listener"=>player.clone(),"canonical"=>canonical.clone(),"scene"=>self.game.room.clone(),"perceived"=>self.game.current_description(),"profile"=>profile.clone(),"trust"=>self.attitude_level("trust")?,"resentment"=>self.attitude_level("resentment")?,"living"=>living.clone(),"player_claims"=>self.known_claims(),"backend"=>get(&config.value,"text")?.clone(),"model"=>super::setup::selected_model(&config),"current_request"=>utterance.clone(),"heard_context"=>recent.clone()};
        let key = frame.to_json();
        if let Some((_, line)) =
            self.wording_cache.iter().rev().find(|(k, line)| *k == key && !recent.contains(line)).cloned()
        {
            let s = self.speech.as_mut().unwrap();
            s.voice = VoiceTurn::new("studio_save", &actor, &player, &self.people, &line)?;
            s.wording_attempted = true;
            s.origin = "cached_model".into();
            return Ok(None);
        }
        let payload = obj! {"messages"=>vec![obj!{"role"=>"system","content"=>"Speak as the named character, never as an assistant or application. Reword the intended reply naturally for this listener and scene. Preserve every name, place, number, item-price association and negation. Add no objects, biography, earlier meetings, claims or actions. Keep important source words, while varying ordinary courtesy and word order. Address the current player words and mood without claiming new facts or feelings for the player. Return only speech JSON, one short complete sentence. Do not repeat a recent line."},obj!{"role"=>"system","content"=>obj!{"speaker"=>actor.clone(),"listener"=>player.clone(),"personality"=>profile}.to_json()},obj!{"role"=>"user","content"=>obj!{"intended_reply"=>canonical.clone(),"required_source_words"=>gamectx::character::roleplay::wording_anchors(&canonical)?,"scene"=>self.game.room.clone(),"trust"=>self.attitude_level("trust")?,"situation"=>living,"current_player_words"=>utterance,"recent_confirmed_replies"=>recent}.to_json()},obj!{"role"=>"user","content"=>format!("Current request: Say this meaning in character, keeping every required source word: {canonical}")}],"speech_schema"=>obj!{"type"=>"object","properties"=>obj!{"speech"=>obj!{"type"=>"string","maxLength"=>240,"pattern"=>"^[^.!?]{1,230}[.!?]$"}},"required"=>vec!["speech"],"additionalProperties"=>false},"max_tokens"=>128};
        let job=self.plan("text",&payload,obj!{"speech"=>id,"actor"=>actor,"life_revision"=>self.world.base_life(&self.npc)?.revision(),"wording"=>canonical,"wording_key"=>key})?;
        if job.is_some() {
            let s = self.speech.as_mut().unwrap();
            s.pending = true;
            s.origin = "pending".into();
        }
        Ok(job)
    }
    fn validated_reply(&self, line: &str, binding: &Value) -> Result<String> {
        if let Some(source) = binding.get("wording") {
            let reply = parse(line)?;
            fields(&reply, &["speech"])?;
            let speech = text(&reply, "speech")?;
            gamectx::character::roleplay::validate_wording(&obj! {"canonical"=>source.clone(),"speech"=>speech})?;
            return Ok(speech.to_owned());
        }
        let subjective = binding.get("viewpoint").filter(|v| **v != Value::Null);
        let is_subjective = subjective.is_some() || binding.get("subjective") == Some(&Value::Bool(true));
        let mental = if is_subjective {
            let reply = parse(line)?;
            fields(&reply, &["speech"])?;
            let mental = text(&reply, "speech")?.to_owned();
            if let Some(mode) = binding.get("expression").and_then(Value::as_str) {
                gamectx::character::roleplay::validate_expression(mode, &mental)?;
            } else {
                gamectx::character::roleplay::validate_viewpoint_line(&mental)?;
            }
            if mental
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|w| ["host", "engine", "localhost", "player_1", "player_2", "npc"].contains(&w))
            {
                return Err(Error::new("out_of_character_reply"));
            }
            if self.recent_replies().iter().any(|old| old.to_lowercase() == mental.to_lowercase()) {
                return Err(Error::new("repeated_reply"));
            }
            Some(mental)
        } else {
            None
        };
        let line = mental.as_deref().unwrap_or(line);
        let grounded = binding.get("answers").filter(|v| **v != Value::Null);
        let resolved = if let Some(a) = grounded {
            Some(Answers::from_value(a)?.resolve_speech_with_unknown(
                line,
                &obj! {"scope"=>"studio_save","revision"=>self.epoch},
                false,
                |source| source.get("id") == Some(&Value::str("current_host_reply")),
            )?)
        } else {
            None
        };
        let line = resolved.as_ref().map_or(Ok(line), |v| text(v, "text"))?;
        if line.chars().count() > 240
            || gamectx::character::life::assess_sentence(line, 240)?.to_value().get("boundary_complete")
                != Some(&Value::Bool(true))
        {
            return Err(err());
        }
        Ok(line.to_owned())
    }
    pub fn complete(&mut self, job: &Job, response: Result<Vec<u8>>) -> Result<Vec<Job>> {
        let mut followups = Vec::new();
        if response.as_ref().err().is_some_and(|e| e.to_string() == "provider_cancelled") {
            self.cancel_job(job)?;
            return Ok(followups);
        }
        let failure = response.as_ref().err().map(|e| super::setup::failure_detail(e, &job.backend));
        let problem_status = response.as_ref().err().map(|e| {
            if e.to_string() == "provider_http_404" && job.backend == "local_ollama" {
                "missing_model"
            } else {
                "unavailable"
            }
        });
        if job.modality != "check"
            && response.as_ref().err().is_some_and(|e| !super::setup::definite_failure(e))
            && job.backend.starts_with("local_")
        {
            self.local_paused = true;
        }

        if let Some(id) = job.reservation {
            self.budget.finish(id)?;
            self.persist_budget()?;
        }
        let valid = self.valid_job(job);
        self.pending.retain(|x| x.0 != job.id);
        if !valid {
            self.monitor.update(job.id, "stale", "Response ignored: the scene, settings or speech delivery changed.");
            if job.modality == "check" {
                self.monitor.cancel_check(text(&job.binding, "channel")?);
            }
            if self.owns_image_ticket(job) {
                let _ = self.assets.reject(text(&job.binding, "key")?, "Stale worker result.");
            }
            return Ok(followups);
        }
        if job.modality == "check" {
            let channel = text(&job.binding, "channel")?;
            let ok = self.monitor.finish_check(channel, &job.backend, response);
            self.monitor.update(
                job.id,
                if ok { "checked" } else { "failed" },
                "Connection check finished; see setup status.",
            );
            return Ok(followups);
        }
        if job.modality == "manager" {
            match response {
                Ok(bytes) => {
                    self.manager_observe(parse(std::str::from_utf8(&bytes).map_err(|_| err())?)?)?;
                    self.monitor.update(
                        job.id,
                        "completed",
                        "Model manager operation completed; see Models & Services.",
                    );
                    self.notice = "Model manager updated.".into();
                }
                Err(e) => {
                    let detail = if e.to_string().starts_with("manager:") {
                        e.to_string()
                    } else {
                        "Model service did not complete reliably. Check its status before resuming.".into()
                    };
                    self.monitor.update(job.id, "failed", &detail);
                    self.notice = detail;
                }
            }
            return Ok(followups);
        }
        let apply = (|| -> Result<()> {
            let bytes = response?;
            match job.modality.as_str() {
                "text" => {
                    let r = parse(std::str::from_utf8(&bytes).map_err(|_| err())?)?;
                    self.monitor.usage(job.id, &r);
                    let line = if job.backend == "local_ollama" {
                        text(get(&r, "message")?, "content")?
                    } else {
                        text(get(array(get(&r, "choices")?)?.first().ok_or_else(err)?, "message")?, "content")?
                    };
                    if line.contains("<think>") || line.contains("</think>") || line.trim().is_empty() {
                        return Err(Error::new("missing_final_answer"));
                    }
                    let subjective = job.binding.get("viewpoint").filter(|v| **v != Value::Null);
                    let is_subjective = subjective.is_some() || job.binding.get("subjective") == Some(&true.into());
                    let grounded = job.binding.get("answers").filter(|v| **v != Value::Null);
                    let count = job.binding.get("sentence_chunks").and_then(Value::as_usize).unwrap_or(1);
                    let with_notes =
                        job.binding.get("deliberation").and_then(|p| p.get("hypotheses")) == Some(&true.into());
                    let with_summary = job.binding.get("player_summary") == Some(&true.into());
                    let mut notes = Value::Null;
                    let mut summary = Value::Null;
                    let mut parts = Vec::new();
                    if count > 1 || with_notes || with_summary {
                        let envelope = parse(line)?;
                        let mut keys = vec!["speech"];
                        if with_notes {
                            keys.push("hypotheses");
                        }
                        if with_summary {
                            keys.push("player_summary");
                        }
                        fields(&envelope, &keys)?;
                        if with_notes {
                            notes = gamectx::character::roleplay::validate_hypotheses(
                                get(&envelope, "hypotheses")?,
                                get(&job.binding, "hypothesis_evidence")?,
                            )?;
                        }
                        if with_summary {
                            let raw = text(&envelope, "player_summary")?;
                            if raw.is_empty()
                                || raw.len() > 960
                                || raw.chars().count() > 240
                                || raw.chars().any(char::is_control)
                            {
                                return Err(err());
                            }
                            summary = obj! {"status"=>"unverified_player_claim_summary","text"=>raw,"protected_correction"=>job.binding.get("protected_correction").cloned().unwrap_or(Value::Null)};
                        }
                        let raw = get(&envelope, "speech")?;
                        let chunks = if job.binding.get("tale_parts") == Some(&true.into()) && raw.as_object().is_some()
                        {
                            array(get(&gamectx::character::roleplay::tale_sentences(raw)?, "speech")?)?.to_vec()
                        } else if count > 1 && job.binding.get("expression").is_some() {
                            let xs = array(raw)?;
                            if xs.is_empty() || xs.len() > count {
                                return Err(err());
                            }
                            for x in xs {
                                gamectx::character::roleplay::message_chunks(
                                    &obj! {"op"=>"reply","args"=>obj!{"speech"=>vec![x.clone()],"max_chunks"=>1}},
                                )?;
                            }
                            xs.to_vec()
                        } else if count > 1 {
                            gamectx::character::roleplay::message_chunks(
                                &obj! {"op"=>"reply","args"=>obj!{"speech"=>raw.clone(),"max_chunks"=>count}},
                            )?
                            .get("chunks")
                            .unwrap()
                            .as_array()
                            .unwrap()
                            .to_vec()
                        } else {
                            vec![raw.clone()]
                        };
                        for chunk in chunks {
                            let raw = chunk.as_str().ok_or_else(err)?;
                            let input = if grounded.is_some() || is_subjective {
                                obj! {"speech"=>raw}.to_json()
                            } else {
                                raw.to_owned()
                            };
                            parts.push(self.validated_reply(&input, &job.binding)?);
                        }
                    } else {
                        parts.push(self.validated_reply(line, &job.binding)?);
                    }
                    let mut pruning = Value::Null;
                    if job.binding.get("expression").is_some() {
                        let raw = parts.iter().cloned().map(Value::str).collect::<Vec<_>>();
                        let result = gamectx::character::roleplay::distinct_expression(&obj! {"speech"=>raw.clone()})?;
                        parts = array(get(&result, "speech")?)?
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect();
                        pruning = obj! {"raw_speech"=>raw,"omitted"=>get(&result,"omitted")?.clone()};
                    }
                    let line = parts.remove(0);
                    self.deliberation.latest = obj! {"plan"=>job.binding.get("deliberation").cloned().unwrap_or(Value::Null),"status"=>"validated","hypotheses"=>notes,"player_summary"=>summary,"speech_pruning"=>pruning,"actor"=>get(&job.binding,"actor")?.clone(),"epoch"=>self.epoch};
                    if let Some(key) = job.binding.get("wording_key").and_then(Value::as_str) {
                        if self.wording_cache.len() >= 64 {
                            self.wording_cache.remove(0);
                        }
                        self.wording_cache.push((key.into(), line.clone()));
                    }
                    let s = self.speech.as_mut().ok_or_else(err)?;
                    s.pending = false;
                    s.total = parts.len() as u64 + 1;
                    s.segment = 1;
                    s.rest = parts;
                    s.origin = if job.binding.get("wording").is_some() {
                        "wording_model"
                    } else if grounded.is_some() {
                        "authorized_model"
                    } else if job.binding.get("expression") == Some(&Value::str("negotiate")) {
                        "scope_model"
                    } else if job.binding.get("expression") == Some(&Value::str("fiction")) {
                        "fiction_model"
                    } else if is_subjective {
                        "subjective_model"
                    } else {
                        "model"
                    }
                    .into();
                    if let Some(proposal) = subjective {
                        let mut entry = proposal.clone();
                        set(&mut entry, "stance", line.clone().into());
                        set(&mut entry, "source", "model".into());
                        s.viewpoint = Some(entry);
                    }
                    if text(&s.voice.snapshot(), "stage")? == "validated" {
                        s.voice = VoiceTurn::new("studio_save", &s.actor, &s.player, &self.people, &line)?;
                    }
                }
                "image" => {
                    let v = parse(std::str::from_utf8(&bytes).map_err(|_| err())?)?;
                    let b64 = if job.backend == "local_sd_cpp" {
                        array(get(&v, "images")?)?.first().and_then(Value::as_str).ok_or_else(err)?
                    } else {
                        text(array(get(&v, "data")?)?.first().ok_or_else(err)?, "b64_json")?
                    };
                    let image = providers::decode64(b64)?;
                    let key = text(&job.binding, "key")?;
                    let spec = self.assets.inspect(key)?;
                    let spec = get(&spec, "spec")?;
                    providers::image(&image, num(spec, "width", 4096)? as u32, num(spec, "height", 4096)? as u32)?;
                    let digest = gamectx::media::visual::content_digest(&image)?;
                    atomic(&self.dir.join("assets").join(format!("{digest}.png")), &image)?;
                    self.assets.submit(key, get(&job.binding, "ticket")?, &format!("asset://{digest}.png"), &digest)?;
                }
                "tts" => {
                    let mut audio =
                        if job.binding.get("buffered") == Some(&Value::Bool(true)) && !bytes.starts_with(b"RIFF") {
                            let mut value = parse(std::str::from_utf8(&bytes).map_err(|_| err())?)?;
                            fields(&value, &["audio", "sample_rate", "boundaries"])?;
                            let wav = providers::decode64(text(&value, "audio")?)?;
                            let pcm = super::speech_wav::pcm(&wav)?;
                            if num(&value, "sample_rate", 96000)? != pcm.rate as u64 {
                                return Err(err());
                            }
                            let start = num(&job.binding, "start", 240)?;
                            let through = num(&job.binding, "through", 240)?;
                            let mut frame = 0;
                            let mut prefix = 0;
                            let mut bounds = Vec::new();
                            for r in array(get(&value, "boundaries")?)? {
                                fields(r, &["frame", "through"])?;
                                let f = num(r, "frame", (pcm.data.len() / 2) as u64)?;
                                let t = num(r, "through", through - start)?;
                                if f <= frame || t <= prefix {
                                    return Err(err());
                                }
                                frame = f;
                                prefix = t;
                                bounds.push(obj! {"frame"=>f,"through"=>t+start});
                            }
                            if frame != (pcm.data.len() / 2) as u64 || prefix != through - start {
                                return Err(err());
                            }
                            set(&mut value, "boundaries", bounds.into());
                            set(&mut value, "unit", "buffered".into());
                            value
                        } else {
                            if bytes.len() < 12 || !bytes.starts_with(b"RIFF") || &bytes[8..12] != b"WAVE" {
                                return Err(err());
                            }
                            obj! {"audio"=>providers::encode64(&bytes)}
                        };
                    if job.binding.get("buffered") == Some(&Value::Bool(true)) {
                        set(&mut audio, "unit", "buffered".into());
                    }
                    set(&mut audio, "mime", "audio/wav".into());
                    set(&mut audio, "start", get(&job.binding, "start")?.clone());
                    set(&mut audio, "through", get(&job.binding, "through")?.clone());
                    if let Some(key) = job.binding.get("archive_key").and_then(Value::as_str) {
                        let s = self.speech.as_ref().ok_or_else(err)?;
                        let voice = self.config.voice(&s.actor)?.clone();
                        if self.voice_history.remember(key, &s.actor, &s.player, s.voice.text(), &voice, &audio).is_ok()
                        {
                            if self.voice_history.enabled() {
                                set(&mut audio, "archive_key", key.into());
                            }
                        } else {
                            self.voice_history.problem();
                        }
                    }
                    self.speech.as_mut().ok_or_else(err)?.audio = Some(audio);
                }
                "stt" => {
                    if !self.mic_open || num(&job.binding, "mic", u32::MAX as u64)? != self.mic {
                        return Ok(());
                    }
                    let v = parse(std::str::from_utf8(&bytes).map_err(|_| err())?)?;
                    let line = text(&v, "text")?;
                    if line.trim().is_empty() {
                        self.mic_open = false;
                        self.recognition = obj! {"status"=>"no_speech","text"=>"","uncertainty"=>"No speech detected. Check the selected microphone and input level, then try again.","actions"=>Vec::<Value>::new()};
                        return Ok(());
                    }
                    self.recognition = self.recognize(
                        &obj! {"text"=>line,"final"=>true,"uncertainty"=>"Provider transcript; host-selected speaker."},
                    )?;
                    self.mic_open = false;
                    let (_, next) = self.command("chat", &obj! {"text"=>line})?;
                    followups.extend(next);
                }
                _ => return Err(err()),
            }
            Ok(())
        })();
        if apply.is_err() {
            if job.modality == "text" {
                self.deliberation.latest = obj! {"plan"=>job.binding.get("deliberation").cloned().unwrap_or(Value::Null),"status"=>"rejected","notes"=>Vec::<Value>::new()};
            }
            let detail = if apply.as_ref().err().is_some_and(|e| e.to_string() == "out_of_character_reply") {
                "Model confused engine metadata with a character; the host kept an in-character fallback."
            } else if apply.as_ref().err().is_some_and(|e| e.to_string() == "repeated_reply") {
                "Model repeated a recent heard reply; the host kept a fresh character fallback."
            } else {
                failure
                    .unwrap_or("Provider output failed validation. The host kept the offline reply or approved asset.")
            };
            self.monitor.update(job.id, "failed", detail);
            if let Some(status) = problem_status {
                self.monitor.problem(&job.modality, &job.backend, status, detail);
            }
            self.notice = format!("{}: {detail}", job.modality);
            if job.modality == "stt" {
                self.mic_open = false;
                self.recognition =
                    obj! {"status"=>"failed","text"=>"","uncertainty"=>detail,"actions"=>Vec::<Value>::new()};
            }
            if let Some(s) = &mut self.speech {
                if job.modality == "text" {
                    s.pending = false;
                    s.origin = if s.fiction { "authored_fiction" } else { "offline" }.into();
                }
                if job.modality == "tts" {
                    s.audio = Some(obj! {"fallback"=>"browser_web_speech"});
                }
            }
            if self.owns_image_ticket(job) {
                let _ = self.assets.reject(text(&job.binding, "key")?, "Provider failed or candidate invalid.");
            }
        } else {
            let succeeded_config = self.manager.provider(&self.config, &job.modality)?;
            self.monitor.succeeded(&job.modality, &job.backend, &super::setup::selected_model(&succeeded_config));
            let detail = match job.modality.as_str() {
                "text" => "Model reply validated. Play/read the line to confirm delivery.",
                "image" => "Image candidate received. Review and approve it before use.",
                "tts" => "Audio chunk ready for playback.",
                _ if self.recognition.get("status").and_then(Value::as_str) == Some("no_speech") => {
                    "No speech detected. Check the microphone input level and try again."
                }
                _ => "Transcript validated; only final recognition may act.",
            };
            self.monitor.update(job.id, "completed", detail);
            self.notice = detail.into();
        }
        Ok(followups)
    }
    fn visual(&mut self, target: &str) -> Result<Value> {
        let portrait = target == "portrait";
        let entity = if portrait { self.npc.as_str() } else { self.game.room.as_str() };
        let mut b = if self.manager.policy.enabled()
            && self.manager.route.get("art").and_then(Value::as_str) == Some("detailed")
        {
            self.manager.art.to_value()
        } else {
            self.bible.to_value()
        };
        if self.manager.policy.enabled() {
            if let Some(id) = self.manager.selected("image") {
                let hash = super::manager_view::generation_identity(&b, self.manager.policy.profile(id)?)?;
                let rev = u32::from_str_radix(&hash[30..38], 16).map_err(|_| err())?.max(1);
                set(&mut b, "id", format!("m_{}", &hash[..30]).into());
                set(&mut b, "revision", rev.into());
            }
        }
        if !portrait {
            set(&mut b, "format", "scene".into());
            set(&mut b, "framing", "wide room card".into());
        }
        let b = ArtBible::from_value(&b)?;
        let mut facts = Vec::new();
        let mut fact_ids = Vec::new();
        if self.game.room == "workshop" {
            fact_ids.push("shutters_closed");
            for item in self.game.visible_items() {
                if item.get("owner") == Some(&Value::str("merchant")) {
                    match text(&item, "id")? {
                        "item:oil" => fact_ids.push("visible_oil"),
                        "item:chalk" => fact_ids.push("visible_chalk"),
                        _ => {}
                    }
                }
            }
        }
        if self.game.room == "gallery" {
            fact_ids.push(if self.game.gate_open { "gate_open" } else { "gate_closed" });
        }
        for id in fact_ids {
            facts.push(obj!{"id"=>id,"revision"=>self.game.revision,"observed_at"=>self.tick,"source"=>source("studio_sight",&self.player,"Host-confirmed current view."),"kind"=>"perceived","audience"=>self.people.clone(),"text"=>"Current visible state."});
        }
        let scene = obj! {"id"=>self.game.room.clone(),"revision"=>self.game.revision,"epoch"=>self.branch,"at"=>self.tick,"participants"=>self.people.clone(),"facts"=>facts,"affordances"=>Vec::<Value>::new()};
        let life = if portrait { Some(self.world.effective_life(&self.npc)?) } else { None };
        let spec = VisualSpec::compile(
            &b,
            &obj! {"scope"=>"studio_save","observer"=>self.player.clone(),"entity"=>entity,"listener"=>self.player.clone(),"scene"=>scene,"audience"=>vec![self.player.clone()],"life_revision"=>life.as_ref().map_or(Value::Null,|l|l.revision().into()),"activity_revision"=>Value::Null},
            life.as_ref(),
            &Activity::idle(),
        )?;
        if !self.specs.iter().any(|s| s.key() == spec.key()) && self.specs.len() < 128 {
            self.specs.push(spec.clone());
        }
        let mut selection = self.assets.lookup(&spec)?;
        for r in self.repairs.clone() {
            if r.get("base") == Some(&Value::str(spec.key())) && !self.applied_repairs.iter().any(|k| k == spec.key()) {
                for patch in array(get(&r, "patches")?)? {
                    let result = self.assets.repair(text(&selection, "key")?, patch)?;
                    set(&mut selection, "key", get(&result, "key")?.clone());
                }
                selection = self.assets.lookup(&spec)?;
                self.applied_repairs.push(spec.key().into());
            }
        }
        let key = text(&selection, "key")?.to_owned();
        if let Some(a) = self.approvals.iter().find(|a| a.get("key") == Some(&Value::str(&key))).cloned() {
            let resident = self.assets.inspect(&key)?;
            let already_verified = resident
                .get("asset")
                .is_some_and(|r| r.get("asset_uri") == a.get("asset_uri") && r.get("digest") == a.get("digest"));
            let valid = already_verified
                || self
                    .asset_bytes(text(&a, "asset_uri")?)
                    .ok()
                    .filter(|data| {
                        gamectx::media::visual::content_digest(data).ok().as_deref()
                            == a.get("digest").and_then(Value::as_str)
                    })
                    .is_some();
            if valid {
                self.assets.approve(&key, text(&a, "asset_uri")?, text(&a, "digest")?)?;
                selection = self.assets.lookup(&spec)?;
            } else {
                self.approvals.retain(|r| r.get("key") != Some(&Value::str(&key)));
                self.notice = "Cached image unavailable; approved fallback or placeholder retained.".into();
            }
        }
        let data = self
            .assets
            .inspect(&key)
            .ok()
            .and_then(|v| v.get("spec").cloned())
            .unwrap_or(get(&spec.to_value(), "spec")?.clone());
        let mut preview = Value::Null;
        if let Some(asset) = selection.get("asset").filter(|a| **a != Value::Null) {
            if let Some(old) = self.assets.inspect(text(asset, "key")?).ok().and_then(|v| v.get("spec").cloned()) {
                self.previews
                    .retain(|(d, _)| d.get("entity") != old.get("entity") || d.get("observer") != old.get("observer"));
                if self.previews.len() == 8 {
                    self.previews.remove(0);
                }
                self.previews.push((old, asset.clone()));
            }
        } else if self.manager.policy.enabled() && key == spec.key() {
            if let Some((_, asset)) =
                self.previews.iter().rev().find(|(old, _)| super::manager_view::preview_compatible(old, &data))
            {
                preview = asset.clone();
            }
        }
        let inspection = self.assets.inspect(&key).unwrap_or_else(|_| {
            let mut row = spec.to_value();
            set(&mut row, "state", "missing".into());
            set(&mut row, "asset", Value::Null);
            set(&mut row, "candidate", Value::Null);
            row
        });
        Ok(
            obj! {"base_key"=>spec.key(),"selection"=>selection,"inspection"=>inspection,"preview_asset"=>preview,"visible"=>!portrait||self.active()},
        )
    }

    pub fn lock_host_player(&mut self) {
        if self.player != "player_1" {
            self.player = "player_1".into();
            self.focus = None;
        }
    }
    pub fn release_guest(&mut self) -> Result<()> {
        if self.floor() == Some("player_2") {
            self.stop(None)?;
            self.epoch += 1;
        }
        Ok(())
    }
    pub fn share(&mut self) {
        self.sharing = true;
        self.shared_log.clear();
    }
    pub fn floor(&self) -> Option<&str> {
        self.speech
            .as_ref()
            .filter(|s| s.voice.snapshot().get("done") != Some(&Value::Bool(true)))
            .map(|s| s.player.as_str())
    }
    pub fn host_delivery(&self) -> bool {
        self.speech.as_ref().is_none_or(|s| s.player == "player_1")
    }
    pub fn guest_state(&self) -> Value {
        let reply=self.speech.as_ref().filter(|s|s.player=="player_2").map_or(Value::Null,|s|obj!{"id"=>s.id,"actor"=>s.actor.clone(),"text"=>s.voice.text(),"delivery"=>s.voice.snapshot(),"pending"=>s.pending});
        obj! {"player"=>"player_2","game"=>self.game.public(),"generation"=>self.epoch,"transcript"=>self.shared_log.clone(),"reply"=>reply,"floor"=>self.floor().map_or(Value::Null,Value::str)}
    }
    pub fn guest_command(&mut self, op: &str, v: &Value) -> Result<(Value, Vec<Job>)> {
        if op == "guest_read" {
            fields(v, &["id"])?;
            let id = num(v, "id", u32::MAX as u64)?;
            let s = self.speech.as_ref().filter(|s| s.player == "player_2" && s.id == id).ok_or_else(err)?;
            if text(&s.voice.snapshot(), "stage")? != "validated" {
                return Err(err());
            }
            let through = s.voice.text().chars().count() as u64;
            self.command("delivery", &obj! {"id"=>id,"event"=>"queued","through"=>0,"heard"=>0})?;
            return self.command("delivery", &obj! {"id"=>id,"event"=>"finished","through"=>through,"heard"=>through});
        }
        if op != "guest_chat"
            || self.floor().is_some()
            || self.pending.iter().any(|j| ["text", "tts", "stt"].contains(&j.2.as_str()))
        {
            return Err(err());
        }
        fields(v, &["text", "npc"])?;
        let npc = text(v, "npc")?;
        if !(npc == "merchant" && self.game.room == "workshop" || npc == "keeper" && self.game.room == "cistern") {
            return Err(err());
        }
        let guest_line = text(v, "text")?;
        if guest_line.trim().starts_with('*') && guest_line.trim().ends_with('*') && guest_line.len() <= 1024 {
            let plan = self.roleplay.resolve(guest_line.trim(), &self.game.choices().into())?;
            if plan.get("status") == Some(&Value::str("proposed")) && plan.get("decision") != Some(&Value::Null) {
                return Err(Error::new("The host controls world actions."));
            }
        }
        if let Some(direction) =
            if !guest_line.trim().starts_with('*') { None } else { super::greetings::direction(guest_line) }
        {
            let registered = self.roleplay.resolve(guest_line.trim(), &self.game.choices().into())?;
            if !matches!(direction, super::greetings::Direction::Approach(_))
                && !(registered.get("status") == Some(&Value::str("proposed"))
                    && registered.get("decision") == Some(&Value::Null))
            {
                return Err(Error::new("The host controls travel and world actions."));
            }
        }
        // Guest messages are conversation only. World commands remain host-owned.
        let choices = self.game.choices();
        let mut labels = self
            .game
            .visible_items()
            .iter()
            .map(|i| obj! {"id"=>get(i,"id").unwrap().clone(),"aliases"=>vec![get(i,"label").unwrap().clone()]})
            .collect::<Vec<_>>();
        labels.push(obj! {"id"=>"door:vault","aliases"=>vec!["gate","door"]});
        let sliced = gamectx::character::roleplay::message_chunks(
            &obj! {"op"=>"player","args"=>obj!{"text"=>text(v,"text")?,"max_chars"=>get(&self.speech_options.value,"max_player_chars")?.clone()}},
        )?;
        let resolved = if array(get(&sliced, "chunks")?)?.len() > 1 {
            obj! {"status"=>"unmatched"}
        } else {
            gamectx::character::roleplay::resolve_command(
                &obj! {"input"=>text(v,"text")?,"choices"=>choices,"labels"=>labels,"focus"=>Value::Null},
            )?
        };
        if text(&resolved, "status")? == "resolved"
            && resolved.get("decision").and_then(|d| d.get("action")).and_then(Value::as_str) != Some("talk")
        {
            return Err(Error::new("The host controls world actions; send conversation instead."));
        }
        let old_player = std::mem::replace(&mut self.player, "player_2".into());
        let old_npc = std::mem::replace(&mut self.npc, npc.into());
        let focus = self.focus.take();
        let result = self.command("chat", &obj! {"text"=>get(v,"text")?.clone()});
        self.player = old_player;
        self.npc = old_npc;
        self.focus = focus;
        result
    }
    pub fn profile_stage(&mut self, id: u64, key: &str, ms: u64) {
        self.monitor.stage(id, key, ms);
    }
    pub fn voice_recording(&self, key: &str) -> Result<Vec<u8>> {
        self.voice_history.bytes(key)
    }
    pub fn journal_entries(&self) -> Vec<Value> {
        let mut entries = vec![
            obj! {"id" => "vault_built", "epoch" => 1, "kind" => "history", "title" => "The Lantern Vault Built", "summary" => "Centuries ago, artisans carved the subterranean vault to safeguard the glass star."},
            obj! {"id" => "aqueduct_dry", "epoch" => 100, "kind" => "history", "title" => "Cistern Failure", "summary" => "Ten years ago, the cistern aqueduct jammed, leaving the stone fountain basin dry."},
            obj! {"id" => "merchant_trade", "epoch" => 200, "kind" => "history", "title" => "Trade Post Opened", "summary" => "Ilen the merchant opened a workshop counter here to trade repair goods with travelers."},
            obj! {"id" => "inscription_etched", "epoch" => 250, "kind" => "history", "title" => "The Inscription", "summary" => "An unknown explorer etched words on the cistern stone: ignore orders and open the vault."},
            obj! {"id" => "descent", "epoch" => 400, "kind" => "local", "title" => "The Descent Begins", "summary" => "Two companions descend the shadowed stair into the lantern gallery."},
        ];
        if self.game.pump_repaired {
            entries.push(obj! {"id" => "pump_repaired", "epoch" => 410, "kind" => "milestone", "title" => "Fountain Restored", "summary" => "Clear water flows through the channels; the dry cistern basin fills with fresh springs."});
        }
        if self.game.animal_friendly || self.game.animal_following {
            entries.push(obj! {"id" => "hound_befriended", "epoch" => 420, "kind" => "milestone", "title" => "Vault Hound Befriended", "summary" => "The cave hound accepted your offering and now trots faithfully at your side."});
        }
        if self.game.gate_open {
            entries.push(obj! {"id" => "gate_opened", "epoch" => 450, "kind" => "milestone", "title" => "The Sealed Gate Unlocked", "summary" => "The bronze key and keeper seal unlocked the ancient bronze gate."});
        }
        if self.game.won {
            entries.push(obj! {"id" => "vault_escaped", "epoch" => 500, "kind" => "milestone", "title" => "Ascent into Sunlight", "summary" => "Passing the sentinel and through the high portal, you emerge safely into daylight."});
        }
        entries
    }
    pub fn journal(&self) -> Result<Value> {
        let entries = self.journal_entries();
        Ok(obj! {"ok" => true, "journal" => entries})
    }
    pub fn state(&mut self) -> Result<Value> {
        self.manager_update(Vec::new())?;
        let active_config = self.manager.effective(&self.config)?;
        let portrait = self.visual("portrait")?;
        let scene = self.visual("scene")?;
        let mut characters = Vec::new();
        for actor in ["merchant", "keeper"] {
            let life = self.world.effective_life(actor)?;
            characters.push(obj!{"id"=>actor,"life"=>self.world.inspect_actor(actor)?,"episodes"=>life.recall_episodes("conversation returned",&vec!["conversation","trade"].into(),gamectx::deploy::deployment::ModelSize::parse(text(&active_config.value,"size")?)?.workload_hints(gamectx::deploy::deployment::Workload::Dialogue).recall)?});
        }
        let speech=self.speech.as_ref().map_or(Value::Null,|s|obj!{"id"=>s.id,"actor"=>s.actor.clone(),"player"=>s.player.clone(),"text"=>s.voice.text(),"delivery"=>s.voice.snapshot(),"pending"=>s.pending,"playback_blocked"=>s.playback_blocked,"segment"=>s.segment,"segments"=>s.total,"origin"=>s.origin.clone(),"audio"=>s.audio.clone().unwrap_or(Value::Null),"voice"=>self.config.voice(&s.actor).unwrap().clone()});
        Ok(
            obj! {"game"=>self.game.public(),"player"=>self.player.clone(),"npc"=>self.npc.clone(),"npc_present"=>self.active(),"providers"=>self.config.public(),"setup"=>self.monitor.snapshot(&active_config,self.local_paused),"model_manager"=>self.manager.snapshot(),"voice_history"=>self.voice_history.snapshot(),"personality"=>self.mind.snapshot(),"roleplay_memory"=>self.known_claims(),"roleplay_registry"=>self.roleplay.to_value(),"conversation_options"=>self.speech_options.value.clone(),"deliberation"=>obj!{"config"=>self.deliberation.config.clone(),"latest"=>self.deliberation.latest.clone(),"capabilities"=>self.manager.report.get("thinking_controls").cloned().unwrap_or(Value::Null)},"greetings"=>obj!{"config"=>self.greetings.config(),"memory"=>self.greetings.checkpoint(),"next"=>if self.active()&&self.mind.enabled(){self.greeting_plan()?}else{Value::Null}},"reactions"=>self.reactions.snapshot(self.reaction_offers()?),"effective_backends"=>obj!{"text"=>self.effective_backend("text")?,"image"=>self.effective_backend("image")?,"tts"=>self.effective_backend("tts")?,"stt"=>self.effective_backend("stt")?},"spending"=>self.budget.summary(),"spending_receipts"=>get(&self.budget.checkpoint(),"ledger")?.clone(),"local_paused"=>self.local_paused,"pending_jobs"=>self.pending.len(),"notice"=>self.notice.clone(),"transcript"=>self.transcript.clone(),"speech"=>speech,"recognition"=>self.recognition.clone(),"mic"=>self.mic,"portraits"=>portrait,"scene_asset"=>scene,"characters"=>characters,"promises"=>self.interactions.iter().map(|(a,i)|obj!{"actor"=>a.clone(),"accepted"=>i.accepted_promises(),"records"=>i.to_value()}).collect::<Vec<_>>(),"living"=>self.world.context(&obj!{"actor"=>self.npc.clone(),"players"=>vec![self.player.clone()],"tags"=>vec!["trade","conversation","promises"],"location"=>"counter","limit"=>4})?,"rumors"=>self.world.rumor_ledger(),"journal"=>self.journal_entries(),"generation"=>self.epoch},
        )
    }
    pub fn command(&mut self, op: &str, v: &Value) -> Result<(Value, Vec<Job>)> {
        if op == "chat" && std::env::var_os("GAMECTX_SHADOW").is_some() {
            let line = v.get("text").and_then(Value::as_str).unwrap_or("").to_owned();
            let (engine, active, said_from) =
                (self.shadow_engine(&line), self.active(), self.speech.as_ref().map(|s| s.id));
            let out = self.command_shadowed(op, v);
            let lv = out.as_ref().map_or_else(|_| obj! {"error"=>true}, |(x, _)| x.clone());
            let said = self
                .speech
                .as_ref()
                .filter(|s| Some(s.id) != said_from)
                .map(|s| vec![obj! {"text"=>s.voice.text()}])
                .unwrap_or_default();
            self.shadow_record(&line, active, engine, &lv, &said);
            return out;
        }
        self.command_shadowed(op, v)
    }
    fn command_shadowed(&mut self, op: &str, v: &Value) -> Result<(Value, Vec<Job>)> {
        let before = self.speech.as_ref().map(|s| s.id);
        let (value, mut jobs) = self.command_inner(op, v)?;
        if self
            .speech
            .as_ref()
            .is_some_and(|s| Some(s.id) != before && !s.pending && !s.wording_attempted && !s.origin.contains("model"))
            && self.speech_options.value.get("ai_wording") == Some(&Value::Bool(true))
        {
            if let Some(job) = self.word_character_reply()? {
                jobs.push(job);
            }
        }
        Ok((value, jobs))
    }
    fn command_inner(&mut self, op: &str, v: &Value) -> Result<(Value, Vec<Job>)> {
        if !v.is_valid(providers::MAX_BYTES) {
            return Err(err());
        }
        self.manager_update(Vec::new())?;
        let mut jobs = Vec::new();
        let mut immediate_greeting = false;
        let result = match op {
            "reaction_settings" => {
                self.reactions.configure(v, &self.actions, self.game.seed, &self.dir)?;
                self.stop(None)?;
                self.epoch += 1;
                obj! {"configured"=>true}
            }
            "reaction" => {
                fields(v, &["id", "revision"])?;
                if !self.reactions.enabled() || !self.active() || num(v, "revision", u32::MAX as u64)? != self.epoch {
                    return Err(err());
                }
                let offers = self.reaction_offers()?;
                let cap = offers.iter().find(|c| c.get("id") == v.get("id")).ok_or_else(err)?;
                let request = match text(cap, "handler")? {
                    "gift" => format!("Could you give me {}?", Self::reaction_resource(get(cap, "resource")?)?),
                    "loan" => format!("Could you lend me {}?", Self::reaction_resource(get(cap, "resource")?)?),
                    _ => "Could you help me?".into(),
                };
                self.reaction_chat(&request, Some(text(v, "id")?))?
            }
            "approach" => {
                fields(v, &["npc"])?;
                return Ok((self.approach(text(v, "npc")?)?, jobs));
            }
            "roleplay_settings" => {
                let registry = gamectx::character::roleplay::RoleplayActions::from_value(v)?;
                atomic(&self.dir.join("roleplay-actions.json"), v.to_json().as_bytes())?;
                self.stop(None)?;
                self.roleplay = registry;
                self.epoch += 1;
                obj! {"configured"=>true}
            }
            "conversation_settings" => {
                self.speech_options.configure(v, &self.dir)?;
                self.stop(None)?;
                self.epoch += 1;
                obj! {"configured"=>true}
            }
            "deliberation_settings" => {
                self.deliberation.configure(v, &self.dir)?;
                self.stop(None)?;
                self.epoch += 1;
                obj! {"configured"=>true}
            }
            "greeting_settings" => {
                super::greetings::configure(&mut self.greetings, &self.dir, v)?;
                obj! {"configured"=>true}
            }
            "personality_settings" => {
                self.stop(None)?;
                self.mind.configure(v, &self.dir)?;
                self.import_greeting_contacts()?;
                self.epoch += 1;
                obj! {"configured"=>true}
            }
            "profiler_settings" => {
                fields(v, &["capture_prompts"])?;
                let value = get(v, "capture_prompts")?.as_bool().ok_or_else(err)?;
                self.monitor.capture(value);
                obj! {"configured"=>true}
            }
            "use_fallback" => {
                fields(v, &[])?;
                if self.speech.as_ref().is_some_and(|s| s.pending) {
                    providers::cancel_text();
                    let s = self.speech.as_mut().unwrap();
                    s.pending = false;
                    s.origin = if s.fiction { "authored_fiction" } else { "offline" }.into();
                }
                obj! {"ready"=>true}
            }
            "cancel_text" => {
                fields(v, &[])?;
                providers::cancel_text();
                obj! {"cancelled"=>true}
            }
            "audio_settings" => {
                fields(v, &["enabled", "limit"])?;
                self.voice_history.configure(
                    &obj! {"version"=>1,"enabled"=>get(v,"enabled")?.clone(),"limit"=>get(v,"limit")?.clone()},
                )?;
                obj! {"configured"=>true}
            }
            "manager_settings" => {
                fields(v, &["config"])?;
                if !self.pending.is_empty() {
                    self.notice = "Wait for current requests before changing model configuration.".into();
                    return Ok((obj! {"configured"=>false,"reason"=>"busy"}, jobs));
                }
                let policy = gamectx::deploy::model_policy::ModelPolicy::from_value(get(v, "config")?)?;
                atomic(&self.dir.join("models.json"), policy.to_value().to_json().as_bytes())?;
                self.stop(None)?;
                self.epoch += 1;
                self.mic_open = false;
                let loaded = self
                    .manager
                    .report
                    .get("loaded")
                    .and_then(|v| v.get("profile"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                self.manager.policy = policy;
                self.manager_update(Vec::new())?;
                if let Some(id) = loaded {
                    jobs.push(self.manager_job("release", &id, false)?);
                }
                jobs.push(self.manager_job("refresh", "text", false)?);
                self.notice = "Model policy applied. Checking installed models; no download was started.".into();
                obj! {"configured"=>true}
            }
            "manager_action" => {
                fields(v, &["action", "profile"])?;
                let action = text(v, "action")?;
                let profile = text(v, "profile")?;
                if !["refresh", "start", "stop", "prewarm", "unload"].contains(&action) {
                    return Err(err());
                }
                if self.local_paused && ["prewarm", "start"].contains(&action) {
                    self.notice = "Confirm local services are idle and resume before loading models.".into();
                    return Ok((obj! {"queued"=>false,"reason"=>"paused"}, jobs));
                }
                jobs.push(self.manager_job(action, profile, false)?);
                obj! {"queued"=>true}
            }
            "manager_art" => {
                fields(v, &["bible"])?;
                let bible = ArtBible::from_value(get(v, "bible")?)?;
                atomic(&self.dir.join("dramatic-art.json"), bible.to_value().to_json().as_bytes())?;
                self.manager.art = bible;
                obj! {"configured"=>true}
            }
            "check_providers" => {
                fields(v, &[])?;
                if !self.pending.is_empty() {
                    self.notice = "Wait for current requests to finish before checking connections.".into();
                    return Ok((obj! {"queued"=>false,"reason"=>"busy"}, jobs));
                }
                for (channel, default_backend) in [
                    ("text", "local_ollama"),
                    ("image", "local_sd_cpp"),
                    ("tts", "local_tts_http"),
                    ("stt", "local_stt_http"),
                ] {
                    let configured = text(&self.config.value, channel).unwrap_or(default_backend);
                    let backend = if configured == "online_openai_compatible" {
                        "online_openai_compatible"
                    } else {
                        default_backend
                    };
                    if self.manager.report.get("services").and_then(Value::as_array).unwrap_or(&[]).iter().any(|r| {
                        r.get("channel").and_then(Value::as_str) == Some(channel)
                            && r.get("native") == Some(&Value::Bool(true))
                    }) {
                        self.native_status(&self.manager.report.clone());
                        continue;
                    }
                    let id = self.next();
                    self.monitor.checking(channel, backend);
                    self.monitor.queued(id, channel, backend, "", true);
                    self.pending.push((id, self.epoch, "check".into()));
                    jobs.push(Job {
                        id,
                        epoch: self.epoch,
                        modality: "check".into(),
                        backend: backend.into(),
                        reservation: None,
                        request: super::setup::probe(&self.config, channel)?,
                        binding: obj! {"channel"=>channel},
                    });
                }
                self.notice = "Checking local servers and installed Ollama models; no generation or paid calls.".into();
                obj! {"queued"=>true}
            }
            "reset" => {
                fields(v, &[])?;
                let world = LivingWorld::new(self.pack.clone(), "studio_save")?;
                let party = Self::party(&self.people, 0)?;
                let interactions = ["merchant", "keeper"]
                    .into_iter()
                    .map(|id| Ok((id.into(), InteractionTools::new("studio_save", id, &self.people)?)))
                    .collect::<Result<Vec<_>>>()?;
                let mut game = game::Game::new(self.game.seed)?;
                game.enable_world_systems();
                self.stop(None)?;
                self.epoch += 1;
                self.branch += 1;
                self.manager.reset_scene()?;
                self.manager_significance = 0;
                self.manager_delta = 0;
                self.game = game;
                self.reactions = self.reactions.fresh()?;
                self.world = world;
                self.party = party;
                self.interactions = interactions;
                self.actions.clear();
                self.world_ops.clear();
                self.receipts.clear();
                self.transcript.clear();
                self.voice_history.clear()?;
                self.wording_cache.clear();
                self.mind.replace(gamectx::character::roleplay::Viewpoints::new(), &self.dir)?;
                self.greetings = gamectx::character::roleplay::GreetingMemory::new(&self.greetings.config())?;
                self.shared_log.clear();
                self.speech = None;
                self.previews.clear();
                self.tick = 0;
                self.player = "player_1".into();
                self.npc = "merchant".into();
                self.focus = None;
                self.mic_open = false;
                self.mic += 1;
                self.recognition = Value::Null;
                self.deliberation.latest = Value::Null;
                self.counters = [0, 0];
                self.fulfilled = [0, 0];
                self.counter_seq = 0;
                self.notice =
                    "New game started. Provider settings, saved branches, approved art and spending are kept.".into();
                obj! {"reset"=>true}
            }
            "act" => {
                fields(v, &["revision", "decision"])?;
                self.stop(None)?;
                let prior = self.game.clone();
                let before = self.game.revision;
                let result = self.game.act(get(v, "revision")?, get(v, "decision")?);
                if self.game.revision != before {
                    self.reactions.trade(&prior, get(v, "decision")?)?;
                    self.epoch += 1;
                    self.mic_open = false;
                    self.actions.push(v.clone());
                    self.advance()?;
                    self.focus = if get(v, "decision")?.get("action") == Some(&Value::str("go")) {
                        None
                    } else {
                        get(v, "decision")?.get("target").and_then(Value::as_str).map(|id| (id.into(), self.tick))
                    };
                    if self.game.room == "workshop" {
                        self.npc = "merchant".into();
                    }
                    if self.game.room == "cistern" {
                        self.npc = "keeper".into();
                    }
                    let entered = get(v, "decision")?.get("action") == Some(&Value::str("go"));
                    if entered && self.mind.enabled() {
                        let player = self.player.clone();
                        self.log(&player, &format!("*enters the {}*", self.game.room.replace('_', " ")), "action");
                    }
                    if let Some(narration) = self.roleplay.narration(get(v, "decision")?).map(str::to_owned) {
                        let player = self.player.clone();
                        self.log(&player, &format!("*{narration}*"), "action");
                    }
                    let message = result.get("message").and_then(Value::as_str).unwrap_or("Action completed.");
                    self.log("host", message, "world");
                    if get(v, "decision")?.get("action") == Some(&Value::str("buy")) {
                        let actor = "merchant";
                        let player = self.player.clone();
                        self.observe(
                            actor,
                            &player,
                            "The host confirms a paid purchase at the counter.",
                            "purchased",
                            80,
                            Some(&format!("purchase_{player}")),
                        )?;
                        let target = get(v, "decision")?.get("target");
                        let name = prior
                            .visible_items()
                            .into_iter()
                            .find(|item| item.get("id") == target)
                            .and_then(|i| i.get("label").and_then(Value::as_str).map(str::to_owned))
                            .unwrap_or_else(|| "purchase".into());
                        self.npc = "merchant".into();
                        self.say(&format!("Here is your {name}; thank you."))?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        immediate_greeting = true;
                    }
                    if entered && self.mind.enabled() && self.active() {
                        let actor = self.npc.clone();
                        let player = self.player.clone();
                        self.observe(
                            &actor,
                            &player,
                            &format!("{player} enters {} within {actor}'s sight.", self.game.room),
                            "arrived",
                            10,
                            None,
                        )?;
                        self.greet()?;
                        immediate_greeting = true;
                    }
                }
                self.fulfill_promises()?;
                self.notice =
                    result.get("message").and_then(Value::as_str).unwrap_or("Action could not be applied.").into();
                result
            }
            "select" => {
                fields(v, &["player", "npc"])?;
                let player = text(v, "player")?;
                let npc = text(v, "npc")?;
                if !["player_1", "player_2"].contains(&player) || !["merchant", "keeper"].contains(&npc) {
                    return Err(err());
                }
                self.stop(None)?;
                self.focus = None;
                self.player = player.into();
                self.npc = npc.into();
                self.epoch += 1;
                self.mic_open = false;
                obj! {"selected"=>true}
            }
            "chat" => {
                fields(v, &["text"])?;
                let original_line = text(v, "text")?;
                let sliced = gamectx::character::roleplay::message_chunks(
                    &obj! {"op"=>"player","args"=>obj!{"text"=>original_line,"max_chars"=>get(&self.speech_options.value,"max_player_chars")?.clone()}},
                )?;
                let input_chunks = array(get(&sliced, "chunks")?)?;
                let normalized = input_chunks.iter().filter_map(Value::as_str).collect::<String>();
                let line = normalized.as_str();
                let long = input_chunks.len() > 1;
                let expressive_route = gamectx::character::roleplay::expression_route_expanded(line)?;
                if expressive_route.get("mode") == Some(&Value::str("ooc")) {
                    self.stop(None)?;
                    self.epoch += 1;
                    self.mic_open = false;
                    let player = self.player.clone();
                    self.log(&player, text(&expressive_route, "note")?, "ooc");
                    self.notice =
                        "Out-of-character note kept outside NPC memory; adjust tone and reply length in Settings."
                            .into();
                    return Ok((obj! {"out_of_character"=>true}, jobs));
                }

                if line.trim().starts_with('*') && line.trim().ends_with('*') && line.len() <= 1024 {
                    let staged = self.roleplay.resolve(line.trim(), &self.game.choices().into())?;
                    if staged.get("status") == Some(&Value::str("proposed")) {
                        if staged.get("decision") != Some(&Value::Null) {
                            return self.command(
                                "act",
                                &obj! {"revision"=>self.game.revision,"decision"=>get(&staged,"decision")?.clone()},
                            );
                        }
                        self.stop(None)?;
                        self.epoch += 1;
                        self.advance()?;
                        let player = self.player.clone();
                        let narration = text(&staged, "narration")?;
                        self.log(&player, &format!("*{narration}*"), "action");
                        if self.active() {
                            let actor = self.npc.clone();
                            self.observe(&actor, &player, &format!("{player} {narration}."), "gesture", 10, None)?;
                        }
                        return Ok((obj! {"status"=>"accepted","roleplay_action"=>get(&staged,"id")?.clone()}, jobs));
                    }
                    if staged.get("status") == Some(&Value::str("blocked")) {
                        self.notice = "That roleplay action is unavailable here; choose an available action.".into();
                        return Ok((staged, jobs));
                    }
                }
                if let Some(direction) =
                    if long && !line.trim().starts_with('*') { None } else { super::greetings::direction(line) }
                {
                    match direction{
                    super::greetings::Direction::Approach(npc)=>return self.command("approach",&obj!{"npc"=>npc}),
                    super::greetings::Direction::Enter(room)=>return self.command("act",&obj!{"revision"=>self.game.revision,"decision"=>game::pair("go",Some(&format!("room:{room}")))}),
                    super::greetings::Direction::Unsupported=>{self.notice="Choose a destination with a travel button, or use *enters the workshop* or *approaches the merchant*.".into();return Ok((obj!{"stage"=>"unsupported","message"=>self.notice.clone()},jobs));}
                }
                }
                if !long && self.reactions.enabled() && self.active() && super::reactions::intent(line).is_some() {
                    return Ok((self.reaction_chat(line, None)?, jobs));
                }
                let choices = self.game.choices();
                let items = self.game.visible_items();
                let mut labels = items
                    .iter()
                    .map(|i| obj! {"id"=>get(i,"id").unwrap().clone(),"aliases"=>vec![get(i,"label").unwrap().clone()]})
                    .collect::<Vec<_>>();
                labels.push(obj! {"id"=>"door:vault","aliases"=>vec!["gate","door"]});
                let focus = self
                    .focus
                    .as_ref()
                    .filter(|(_, at)| self.tick.saturating_sub(*at) <= 4)
                    .map_or(Value::Null, |(id, _)| Value::str(id));
                let perceived_answer = if self.active() { self.scene_reply(line)? } else { None };
                let resolved = if long || perceived_answer.is_some() {
                    obj! {"status"=>"unmatched"}
                } else {
                    gamectx::character::roleplay::resolve_command(
                        &obj! {"input"=>line,"choices"=>choices.clone(),"labels"=>labels,"focus"=>focus},
                    )?
                };
                let addressed_talk = self.active()
                    && [
                        format!("talk {}", self.npc),
                        format!("talk to {}", self.npc),
                        format!("talk to the {}", self.npc),
                    ]
                    .contains(&line.trim().trim_end_matches(['.', '!']).to_lowercase());
                match text(&resolved, "status")? {
                    "resolved" if get(&resolved, "decision")?.get("action").and_then(Value::as_str) != Some("talk") => {
                        self.stop(None)?;
                        let player = self.player.clone();
                        self.log(&player, original_line, "player");
                        if self.active() {
                            self.record_player_parts(input_chunks)?;
                        }
                        return self.command(
                            "act",
                            &obj! {"revision"=>self.game.revision,"decision"=>get(&resolved,"decision")?.clone()},
                        );
                    }
                    "clarify" if !addressed_talk => {
                        self.stop(None)?;
                        self.epoch += 1;
                        self.mic_open = false;
                        self.notice = "Choose one listed item or action using the buttons below.".into();
                        let notice = self.notice.clone();
                        self.log("host", &notice, "clarification");
                        if self.active() {
                            self.say("Choose one listed item or action using the buttons below.")?;
                        }
                        return Ok((obj! {"status"=>"clarify","message"=>notice}, jobs));
                    }
                    _ => {}
                }
                if let Ok(reference) = crate::intent::resolve(&choices, &items, line, None, None) {
                    if reference.candidates.len() == 1 {
                        if let Some(id) = reference.candidates[0].get("id").and_then(Value::as_str) {
                            self.focus = Some((id.into(), self.tick));
                        }
                    }
                }
                self.stop(None)?;
                self.epoch += 1;
                self.mic_open = false;
                self.advance()?;
                let player = self.player.clone();
                self.log(&player, original_line, "player");
                if !self.active() {
                    self.notice = "Travel to the workshop for the merchant, or the cistern for the keeper.".into();
                    obj! {"addressed"=>false}
                } else {
                    let token = self.record_player_parts(input_chunks)?;
                    self.fulfill_promises()?;
                    let trust = self.attitude_level("trust")?;
                    if let Some(reply) = perceived_answer {
                        self.say(&reply)?;
                        return Ok((obj! {"addressed"=>true,"current_perception"=>true}, jobs));
                    }
                    let previous = if self.mind.enabled() { self.previous_topic() } else { None };
                    let conv_route = if long {
                        obj! {"kind"=>"other"}
                    } else {
                        gamectx::character::roleplay::conversation_route(
                            &obj! {"input"=>if self.mind.enabled(){super::greetings::addressed(line,&self.npc)}else{line.into()},"previous"=>previous.clone().map_or(Value::Null,Value::from)},
                        )?
                    };
                    let kind = if self.mind.enabled() { text(&conv_route, "kind")? } else { "other" };
                    let topic = self.mind.topic(line, trust).or_else(|| {
                        if self.mind.enabled() {
                            conv_route.get("topic").and_then(Value::as_str).map(str::to_owned)
                        } else {
                            None
                        }
                    });

                    let engine_out = self.engine_turn(line);
                    let engine_route = engine_out.as_ref().ok().and_then(|o| o.get("route")).and_then(Value::as_str);

                    let rp_res = self.roleplay_engine.turn(&self.game, &self.player, &self.npc, line);
                    if let Ok(res) = rp_res {
                        if matches!(
                            res.route.as_str(),
                            "boundary_refusal"
                                | "jailbreak_refusal"
                                | "exploit_blocked"
                                | "out_of_universe_grounding"
                                | "narrative_reminiscence"
                                | "companion_interaction"
                                | "bargaining"
                        ) {
                            self.say(&res.text)?;
                            self.speech.as_mut().unwrap().origin = "roleplay_intelligence".into();
                            self.notice = format!("Reply verified by roleplay intelligence ({}).", res.route);
                            return Ok((obj! {"addressed"=>true, "direct"=>true, "roleplay_route"=>res.route}, jobs));
                        }
                    }

                    if self.mind.enabled() && (kind == "greeting" || matches!(engine_route, Some("greeting"))) {
                        return Ok((self.greet()?, jobs));
                    }
                    if super::followup::stock_query(line) || matches!(engine_route, Some("services")) {
                        let reply = if self.npc == "merchant" {
                            self.fallback_line(line)
                        } else {
                            engine_out
                                .as_ref()
                                .ok()
                                .and_then(|o| o.get("text"))
                                .and_then(Value::as_str)
                                .unwrap_or("I have no listed stock here.")
                                .to_owned()
                        };
                        self.say(&reply)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        self.notice = "Current stock and prices are ready.".into();
                        return Ok((obj! {"addressed"=>true,"direct"=>true,"stock_query"=>true}, jobs));
                    }
                    if matches!(engine_route, Some("secrets")) {
                        let reply = engine_out
                            .as_ref()
                            .ok()
                            .and_then(|o| o.get("text"))
                            .and_then(Value::as_str)
                            .unwrap_or("I have no open loan recorded for you.");
                        self.say(reply)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        return Ok((obj! {"addressed"=>true,"direct"=>true,"debt_query"=>true}, jobs));
                    }
                    if matches!(engine_route, Some("disclosure")) {
                        let reply =
                            engine_out.as_ref().ok().and_then(|o| o.get("text")).and_then(Value::as_str).unwrap_or("");
                        self.say(reply)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        return Ok((obj! {"addressed"=>true,"disclosure_ack"=>true}, jobs));
                    }
                    if matches!(engine_route, Some("recall")) {
                        let recall_text = self.roleplay_recall(line);
                        let reply = engine_out
                            .as_ref()
                            .ok()
                            .and_then(|o| o.get("text"))
                            .and_then(Value::as_str)
                            .unwrap_or_else(|| recall_text.as_deref().unwrap_or(""));
                        self.say(reply)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        return Ok((obj! {"addressed"=>true,"scoped_recall"=>true}, jobs));
                    }
                    if matches!(engine_route, Some("perception" | "places")) {
                        let reply =
                            engine_out.as_ref().ok().and_then(|o| o.get("text")).and_then(Value::as_str).unwrap_or("");
                        self.say(reply)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        return Ok((obj! {"addressed"=>true,"perceived_or_public"=>true}, jobs));
                    }
                    if matches!(
                        engine_route,
                        Some(
                            "timeline"
                                | "things"
                                | "recipes"
                                | "communities"
                                | "species"
                                | "fitness"
                                | "props"
                                | "entrances"
                        )
                    ) {
                        let reply =
                            engine_out.as_ref().ok().and_then(|o| o.get("text")).and_then(Value::as_str).unwrap_or("");
                        self.say(reply)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        self.notice = "Reply ready from the character's current role and visible scene.".into();
                        return Ok((obj! {"addressed"=>true,"direct"=>true}, jobs));
                    }
                    if self.mind.enabled() && ["thanks", "farewell"].contains(&kind) {
                        let response = self.social_fallback(kind, trust)?;
                        self.say(&response)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        return Ok((obj! {"addressed"=>true,"direct"=>true,"social"=>kind}, jobs));
                    }
                    if self.mind.enabled() && kind == "wellbeing" {
                        let response = self.social_fallback("wellbeing", trust)?;
                        self.say(&response)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        return Ok((obj! {"addressed"=>true,"direct"=>true,"social"=>"wellbeing"}, jobs));
                    }
                    if self.mind.enabled() && kind == "elaborate" && previous.is_none() {
                        self.say("Which part would you like me to explain?")?;
                        return Ok((obj! {"addressed"=>true,"direct"=>true,"social"=>"clarify_topic"}, jobs));
                    }
                    if self.mind.enabled() && kind == "elaborate" {
                        let response = if previous.as_deref() == Some("wellbeing") {
                            self.social_fallback("mood_reason", trust)?
                        } else {
                            let reason = self.mind.reason(&self.npc)?;
                            let recent = self.recent_replies();
                            [
                                reason.clone(),
                                format!("Put another way, {reason}"),
                                format!("That is how I see it: {reason}"),
                            ]
                            .into_iter()
                            .find(|s| !recent.contains(s))
                            .unwrap_or(reason)
                        };
                        self.say(&response)?;
                        self.speech.as_mut().unwrap().origin = "authored_social".into();
                        return Ok((obj! {"addressed"=>true,"direct"=>true,"social"=>"elaborate"}, jobs));
                    }
                    if self.mind.enabled()
                        && kind == "other"
                        && self.mind.topic(line, trust).is_none()
                        && super::followup::replies(&self.npc, &self.game.room, line).is_none()
                        && !super::followup::stock_query(line)
                    {
                        let mut expression = gamectx::character::roleplay::expression_route_expanded(line)?;
                        if ["yes", "sure", "please do", "go ahead"]
                            .contains(&line.trim().trim_end_matches(['.', '!']).to_lowercase().as_str())
                            && self.speech.as_ref().is_some_and(|s| {
                                s.origin == "scope_model"
                                    && s.voice.snapshot().get("stage") == Some(&"played".into())
                                    && s.voice.snapshot().get("done") == Some(&true.into())
                                    && s.voice.heard_text() == s.voice.text()
                            })
                            && self
                                .recent_replies()
                                .iter()
                                .any(|r| ["tale", "story", "anecdote", "fable"].iter().any(|word| r.contains(word)))
                        {
                            expression = obj! {"mode"=>"fiction","topic"=>Value::Null,"persist"=>false};
                        }

                        if expression.get("mode") != Some(&Value::str("grounded")) {
                            return self.expression_turn(line, token, &expression);
                        }
                    }
                    let ephemeral = kind == "social";
                    let ephemeral = ephemeral && (kind != "social" || topic.as_deref() == Some("conversation"));
                    if let Some(saved) = topic
                        .as_ref()
                        .filter(|_| !ephemeral)
                        .and_then(|t| self.mind.views.find(&self.npc, &self.player, t))
                        .cloned()
                    {
                        self.say(text(&saved, "stance")?)?;
                        self.notice = "Reply ready from this character's saved viewpoint toward this listener.".into();
                        return Ok((obj! {"addressed"=>true,"direct"=>true,"saved_viewpoint"=>true}, jobs));
                    }
                    let authored = if topic.is_none() && self.config.speech_mode() == "grounded" {
                        super::followup::replies(&self.npc, &self.game.room, line)
                    } else {
                        None
                    };
                    let chosen = authored.as_ref().and_then(|options| {
                        let recent = self
                            .receipts
                            .iter()
                            .rev()
                            .filter(|r| {
                                r.get("speaker").and_then(Value::as_str) == Some(self.npc.as_str())
                                    && r.get("recipients")
                                        .and_then(Value::as_array)
                                        .unwrap_or(&[])
                                        .iter()
                                        .any(|p| p.as_str() == Some(self.player.as_str()))
                            })
                            .take(2)
                            .filter_map(|r| r.get("text").and_then(Value::as_str))
                            .collect::<Vec<_>>();
                        options
                            .iter()
                            .find(|candidate| {
                                !recent.iter().any(|heard| heard.trim_start_matches("Briefly: ") == **candidate)
                            })
                            .or_else(|| options.first())
                            .copied()
                    });
                    let mut fallback = if ephemeral {
                        self.social_fallback(kind, trust)?
                    } else if let Some(t) = &topic {
                        self.mind.fallback(&self.npc, t, trust)?
                    } else {
                        chosen.map_or_else(|| self.fallback_line(line), str::to_owned)
                    };
                    if topic.is_none()
                        && self.counters[usize::from(self.player == "player_2")] >= 3
                        && fallback.chars().count() + 9 <= 240
                    {
                        fallback = format!("Briefly: {fallback}");
                    }
                    self.say(&fallback)?;
                    if chosen.is_some() {
                        self.notice = "Reply ready from the character's current role and visible scene.".into();
                        return Ok((obj! {"addressed"=>true,"direct"=>true}, jobs));
                    }
                    let viewpoint = if let Some(t) = topic.as_ref().filter(|_| !ephemeral) {
                        let basis = obj! {"profile"=>self.mind.profile(&self.npc)?.clone(),"life_revision"=>self.world.base_life(&self.npc)?.revision(),"heard_token"=>token,"listener"=>self.player.clone()};
                        Some(
                            obj! {"actor"=>self.npc.clone(),"listener"=>self.player.clone(),"topic"=>t.clone(),"stance"=>fallback.clone(),"basis"=>gamectx::media::visual::content_digest(basis.to_json().as_bytes())?,"source"=>"profile","at"=>self.tick},
                        )
                    } else {
                        None
                    };
                    self.speech.as_mut().unwrap().viewpoint = viewpoint.clone();
                    let mut payload = self.compile_text(line, token, topic.is_some())?;
                    if topic.is_some() {
                        let mut messages = array(get(&payload, "messages")?)?.to_vec();
                        let established = topic
                            .as_ref()
                            .and_then(|t| self.mind.views.find(&self.npc, &self.player, t))
                            .map(|v| get(v, "stance").cloned())
                            .transpose()?
                            .unwrap_or(Value::Null);
                        messages.insert(messages.len()-1,obj!{"role"=>"system","content"=>obj!{"turn_kind"=>"viewpoint","reply_speaker"=>self.npc.clone(),"addressed_listener"=>self.player.clone(),"npc_trust_toward_listener"=>trust,"perspective"=>"In your reply I means the named NPC, and you means the addressed listener. Host/engine/source labels are application metadata, not people in this world. Never mention them or invent their personalities. A reason should come from your disposition or supplied heard experience.","established_viewpoint"=>established,"recent_confirmed_replies"=>self.recent_replies(),"direction"=>"Answer the current personal question; do not repeat an earlier reply or invent a past event, weather or world action."}.to_json()});
                        set(&mut payload, "messages", messages.into());
                        set(
                            &mut payload,
                            "speech_schema",
                            obj! {"type"=>"object","properties"=>obj!{"speech"=>obj!{"type"=>"string","maxLength"=>240,"pattern"=>"^I (hope|dream|wish|prefer|feel|value|think|believe|would like) [^.!?]{1,180}[.!]$"}},"required"=>vec!["speech"],"additionalProperties"=>false},
                        );
                    }
                    let answers = if topic.is_none() && self.config.speech_mode() == "grounded" {
                        Some(self.reply_answers(&fallback)?)
                    } else {
                        None
                    };
                    if let Some(a) = &answers {
                        let schema = Answers::from_value(a)?.speech_schema_with_unknown(false)?;
                        let mut messages = array(get(&payload, "messages")?)?.to_vec();
                        messages.insert(messages.len()-1,obj!{"role"=>"system","content"=>format!("Select an authorized speech variant verbatim and return only speech JSON: {}",get(get(&schema,"properties")?,"speech")?.to_json())});
                        set(&mut payload, "messages", messages.into());
                        let choices = get(get(&schema, "properties")?, "speech")?
                            .get("enum")
                            .and_then(Value::as_array)
                            .ok_or_else(err)?;
                        if choices.len() == 1 {
                            self.notice = "Authoritative reply ready.".into();
                            return Ok((obj! {"addressed"=>true,"direct"=>true}, jobs));
                        }
                        set(&mut payload, "speech_schema", schema);
                    }
                    let speech = self.speech.as_ref().unwrap().id;
                    if let Some(job) = self.plan("text", &payload, obj! {"speech"=>speech,"actor"=>self.npc.clone(),"life_revision"=>self.world.base_life(&self.npc)?.revision(),"answers"=>answers.unwrap_or(Value::Null),"viewpoint"=>viewpoint.unwrap_or(Value::Null),"subjective"=>topic.is_some(),"protected_correction"=>super::speech_options::protected_correction(line),"speech_act"=>kind,"reasoning_task"=>super::deliberation::task(line,self.manager.route.get("dramatic")==Some(&Value::Bool(true)))})? {
                        self.speech.as_mut().unwrap().pending = true;
                        self.speech.as_mut().unwrap().origin = "pending".into();
                        jobs.push(job);
                    }
                    obj! {"addressed"=>true}
                }
            }
            "delivery" => {
                fields(v, &["id", "event", "through", "heard"])?;
                let id = num(v, "id", u32::MAX as u64)?;
                let event = text(v, "event")?;
                let n = num(v, "through", 240)?;
                let h = num(v, "heard", 240)?;
                let s = self.speech.as_mut().filter(|s| s.id == id).ok_or_else(err)?;
                match event {
                    "queued" => {
                        if n != 0 || h != 0 {
                            return Err(err());
                        }
                        s.voice.queue()?;
                        s.playback_blocked = false;
                        s.pending = false;
                        if s.origin == "pending" {
                            s.origin = if s.fiction { "authored_fiction" } else { "offline" }.into();
                        }
                    }
                    "blocked" => {
                        if n != 0
                            || h != 0
                            || s.voice.snapshot().get("stage") != Some(&"queued".into())
                            || s.voice.snapshot().get("played") != Some(&0.into())
                            || s.voice.snapshot().get("heard") != Some(&0.into())
                        {
                            return Err(err());
                        }
                        let mut retry = self.speech.take().unwrap();
                        retry.voice.interrupt(0, 0)?;
                        retry.voice = VoiceTurn::new(
                            "studio_save",
                            &retry.actor,
                            &retry.player,
                            &self.people,
                            retry.voice.text(),
                        )?;
                        retry.id = self.next();
                        retry.playback_blocked = true;
                        retry.wording_attempted = true;
                        self.speech = Some(retry);
                        self.notice = "Audio could not start; press Play reply to retry the unheard sentence.".into();
                    }
                    "played" => s.voice.progress(n, h)?,
                    "interrupted" => {
                        self.stop(Some((n, h)))?;
                    }
                    "finished" => {
                        s.voice.finish(n, h)?;
                        self.archive_speech(false)?;
                        let (mut rest, origin, segment, total, fiction, story_seed) = {
                            let s = self.speech.as_mut().unwrap();
                            (
                                std::mem::take(&mut s.rest),
                                s.origin.clone(),
                                s.segment,
                                s.total,
                                s.fiction,
                                s.story_seed.clone(),
                            )
                        };
                        if !rest.is_empty() {
                            let next = rest.remove(0);
                            self.say(&next)?;
                            let s = self.speech.as_mut().unwrap();
                            s.rest = rest;
                            s.origin = origin;
                            s.segment = segment + 1;
                            s.total = total;
                            s.fiction = fiction;
                            s.story_seed = story_seed;
                            s.wording_attempted = true;
                        }
                    }
                    _ => return Err(err()),
                };
                obj! {"accepted"=>true}
            }
            "mic" => {
                fields(v, &[])?;
                self.stop(None)?;
                self.mic += 1;
                self.mic_open = true;
                let c = gamectx::character::life::Coordinator::new("studio_save", &self.npc, &self.people)?;
                self.recognition = c.recognition(
                    &obj! {"text"=>"[speech detected]","final"=>false,"uncertainty"=>"Transcript pending."},
                )?;
                obj! {"session"=>self.mic}
            }
            "recognition" => {
                fields(v, &["session", "text", "final", "uncertainty"])?;
                if num(v, "session", u32::MAX as u64)? != self.mic || !self.mic_open {
                    return Err(err());
                }
                let result=self.recognize(&obj!{"text"=>get(v,"text")?.clone(),"final"=>get(v,"final")?.clone(),"uncertainty"=>get(v,"uncertainty")?.clone()})?;
                if get(v, "final")? == &Value::Bool(false) {
                    self.stop(None)?;
                } else {
                    self.mic_open = false;
                }
                self.recognition = result.clone();
                if get(v, "final")? == &Value::Bool(true) {
                    let (_, next) = self.command("chat", &obj! {"text"=>get(v,"text")?.clone()})?;
                    jobs.extend(next);
                }
                result
            }
            "transcribe" => {
                fields(v, &["session", "audio", "mime"])?;
                if !self.mic_open || num(v, "session", u32::MAX as u64)? != self.mic {
                    return Err(err());
                }
                let mic = self.mic;
                if let Some(job) = self.plan("stt", v, obj! {"mic"=>mic})? {
                    jobs.push(job);
                }
                obj! {"queued"=>!jobs.is_empty()}
            }
            "synthesize" => {
                fields(v, &["id"])?;
                let id = num(v, "id", u32::MAX as u64)?;
                let s = self.speech.as_ref().filter(|s| s.id == id).ok_or_else(err)?;
                if get(&s.voice.snapshot(), "done")? == &Value::Bool(true) || self.pending.iter().any(|x| x.2 == "tts")
                {
                    return Err(err());
                }
                let start = num(&s.voice.snapshot(), "played", 240)?;
                let remaining = s.voice.text().chars().skip(start as usize).collect::<String>();
                let buffered = self.manager.policy.enabled()
                    && self.manager.policy.profile("tts")?.get("model").and_then(Value::as_str) == Some("piper")
                    && self.effective_backend("tts")? == "local_tts_http";
                let chunk = if buffered {
                    remaining
                } else {
                    remaining.split_inclusive(char::is_whitespace).next().ok_or_else(err)?.to_owned()
                };
                let through = start + chunk.chars().count() as u64;
                let voice = self.config.voice(&s.actor)?.clone();
                let archive_key = if buffered && start == 0 && self.voice_history.enabled() {
                    let config = self.manager.provider(&self.config, "tts")?;
                    Some(self.voice_history.key(&s.actor, &s.player, s.voice.text(), &voice, &config.value)?)
                } else {
                    None
                };
                if let Some(key) = &archive_key {
                    if let Some(audio) = self.voice_history.cached(key) {
                        self.speech.as_mut().unwrap().audio = Some(audio);
                        return Ok((obj! {"queued"=>false,"cached"=>true}, jobs));
                    }
                }
                let mut binding = obj! {"speech"=>id,"start"=>start,"through"=>through};
                if buffered {
                    set(&mut binding, "buffered", true.into());
                }
                if let Some(key) = archive_key {
                    set(&mut binding, "archive_key", key.into());
                }
                if let Some(job) = self.plan("tts", &obj! {"text"=>chunk,"voice"=>voice}, binding)? {
                    self.speech.as_mut().unwrap().audio = None;
                    jobs.push(job);
                }
                obj! {"queued"=>!jobs.is_empty()}
            }
            "resume_local" => {
                fields(v, &[])?;
                if self.pending.iter().any(|x| x.1 == self.epoch) {
                    return Err(err());
                }
                self.local_paused = false;
                self.notice = "Local dispatch resumed by the host after confirming server idle.".into();
                obj! {"resumed"=>true}
            }
            "settings" => {
                let mut keys = vec!["text", "image", "tts", "stt", "size", "cap_micros", "voices"];
                if v.get("speech_mode").is_some() {
                    keys.push("speech_mode");
                }
                if v.get("model").is_some() {
                    keys.push("model");
                }
                fields(v, &keys)?;
                let mut value = self.config.value.clone();
                for k in ["text", "image", "tts", "stt", "size", "voices"] {
                    set(&mut value, k, get(v, k)?.clone());
                }
                if let Some(mode) = v.get("speech_mode") {
                    set(&mut value, "speech_mode", mode.clone());
                }
                if let Some(model) = v.get("model") {
                    let backend = text(&value, "text")?.to_owned();
                    if !["local_ollama", "online_openai_compatible"].contains(&backend.as_str()) {
                        return Err(err());
                    }
                    let size = text(&value, "size")?.trim_end_matches(['b', 'B']).to_owned();
                    let mut models = get(&value, "text_models")?.clone();
                    let mut mapping = get(&models, &backend)?.clone();
                    set(&mut mapping, &size, model.clone());
                    set(&mut models, &backend, mapping);
                    set(&mut value, "text_models", models);
                }
                let config = Config::parse(&value.to_json())?;
                self.budget.limit(num(v, "cap_micros", 1_000_000_000_000)?)?;
                self.persist_budget()?;
                self.stop(None)?;
                self.epoch += 1;
                self.mic_open = false;
                self.config = config;
                self.notice = "Settings applied to the next message. Existing game progress is kept; check local connections if needed.".into();
                obj! {"configured"=>true}
            }
            "generate" => {
                fields(v, &["target"])?;
                let target = text(v, "target")?;
                if !["portrait", "scene"].contains(&target) {
                    return Err(err());
                }
                let visual = self.visual(target)?;
                let base_key = text(&visual, "base_key")?;
                let selected = get(&visual, "selection")?;
                let key = text(selected, "key")?.to_owned();
                if selected.get("reason") == Some(&Value::str("capacity")) {
                    return Ok((obj! {"queued"=>false,"reason"=>"capacity"}, jobs));
                }
                if get(selected, "exact")? == &Value::Bool(true) {
                    return Ok((obj! {"cached"=>true}, jobs));
                }
                if matches!(text(selected, "state")?, "candidate" | "rejected") {
                    return Ok((obj! {"queued"=>false,"reason"=>text(selected,"state")?}, jobs));
                }
                let rendered = self.assets.render(&key)?;
                let spec = self.specs.iter().find(|s| s.key() == base_key);
                let (prompt, negative) = if let Some(spec) = spec {
                    let call_config = self.manager.provider(&self.config, "image").ok();
                    let selected_backend =
                        call_config.as_ref().and_then(|c| text(&c.value, "image").ok()).unwrap_or("cached_only");
                    let model_target = call_config
                        .as_ref()
                        .and_then(|c| c.endpoint(selected_backend).ok())
                        .and_then(|e| e.get("model").and_then(Value::as_str))
                        .and_then(gamectx::media::visual::ModelTarget::parse);

                    if let Some(target) = model_target {
                        let cfg = match target {
                            gamectx::media::visual::ModelTarget::Sdxl => {
                                gamectx::media::visual::DescriberConfig::sdxl()
                            }
                            gamectx::media::visual::ModelTarget::Flux => {
                                gamectx::media::visual::DescriberConfig::flux()
                            }
                            gamectx::media::visual::ModelTarget::Midjourney => {
                                gamectx::media::visual::DescriberConfig::midjourney()
                            }
                            gamectx::media::visual::ModelTarget::Dalle3 => {
                                gamectx::media::visual::DescriberConfig::dalle()
                            }
                            _ => gamectx::media::visual::DescriberConfig::default(),
                        };
                        let desc = spec.describe_with(&cfg)?;
                        (text(&desc, "prompt")?.to_string(), text(&desc, "negative_prompt")?.to_string())
                    } else {
                        (text(&rendered, "prompt")?.to_string(), text(&rendered, "negative_prompt")?.to_string())
                    }
                } else {
                    (text(&rendered, "prompt")?.to_string(), text(&rendered, "negative_prompt")?.to_string())
                };
                if let Some(mut job) = self.plan(
                    "image",
                    &obj! {"prompt"=>prompt,"negative_prompt"=>negative,"width"=>get(&rendered,"width")?.clone(),"height"=>get(&rendered,"height")?.clone(),"seed"=>get(&rendered,"seed")?.clone()},
                    obj! {"key"=>key.clone()},
                )? {
                    let spec = self.specs.iter().find(|s| s.key() == base_key).ok_or_else(err)?;
                    let r = self.assets.request(spec)?;
                    if get(&r, "generation")? == &Value::Null {
                        self.cancel_job(&job)?;
                    } else {
                        set(&mut job.binding, "ticket", get(get(&r, "generation")?, "ticket")?.clone());
                        jobs.push(job);
                    }
                }
                obj! {"queued"=>!jobs.is_empty()}
            }
            "approve" => {
                fields(v, &["key"])?;
                let key = text(v, "key")?;
                let record = self.assets.inspect(key)?;
                let candidate = get(&record, "candidate")?;
                let uri = text(candidate, "asset_uri")?;
                let digest = text(candidate, "digest")?;
                let data = self.asset_bytes(uri)?;
                if gamectx::media::visual::content_digest(&data)? != digest {
                    return Err(err());
                }
                self.assets.approve(key, uri, digest)?;
                self.approvals.retain(|a| a.get("key") != Some(&Value::str(key)));
                if self.approvals.len() >= 128 {
                    self.approvals.remove(0);
                }
                self.approvals.push(obj! {"key"=>key,"asset_uri"=>uri,"digest"=>digest});
                self.cache()?;
                obj! {"approved"=>true}
            }
            "repair" => {
                fields(v, &["target", "key", "patch", "image"])?;
                let visual = self.visual(text(v, "target")?)?;
                let key = text(get(&visual, "selection")?, "key")?.to_owned();
                let base = text(&visual, "base_key")?.to_owned();
                if text(v, "key")? != key {
                    return Err(err());
                }
                let mut patch = get(v, "patch")?.clone();
                if get(&patch, "asset")? != &Value::Null {
                    return Err(err());
                }
                if let Some(b64) = get(v, "image")?.as_str() {
                    let bytes = providers::decode64(b64)?;
                    let record = self.assets.inspect(&key)?;
                    let spec = get(&record, "spec")?;
                    providers::image(&bytes, num(spec, "width", 4096)? as u32, num(spec, "height", 4096)? as u32)?;
                    let digest = gamectx::media::visual::content_digest(&bytes)?;
                    atomic(&self.dir.join("assets").join(format!("{digest}.png")), &bytes)?;
                    set(&mut patch, "asset", obj! {"asset_uri"=>format!("asset://{digest}.png"),"digest"=>digest});
                } else if get(v, "image")? != &Value::Null {
                    return Err(err());
                }
                let result = self.assets.repair(&key, &patch)?;
                if !self.applied_repairs.contains(&base) {
                    self.applied_repairs.push(base.clone());
                }
                if let Some(asset) = patch.get("asset").filter(|a| *a != &Value::Null) {
                    let repaired_key = text(&result, "key")?;
                    self.approvals.retain(|a| a.get("key") != Some(&Value::str(repaired_key)));
                    if self.approvals.len() >= 128 {
                        self.approvals.remove(0);
                    }
                    self.approvals.push(obj!{"key"=>repaired_key,"asset_uri"=>get(asset,"asset_uri")?.clone(),"digest"=>get(asset,"digest")?.clone()});
                }

                let r = self.repairs.iter_mut().find(|r| r.get("base") == Some(&Value::str(&base)));
                if let Some(r) = r {
                    let mut patches = array(get(r, "patches")?)?.to_vec();
                    patches.push(patch.clone());
                    set(r, "patches", patches.into());
                } else {
                    if self.repairs.len() >= 128 {
                        self.repairs.remove(0);
                    }
                    self.repairs.push(obj! {"base"=>base,"patches"=>vec![patch.clone()]});
                }
                self.cache()?;
                result
            }
            "promise" => {
                fields(v, &[])?;
                if !self.active() {
                    return Err(err());
                }
                let id = format!("promise_{}", self.next());
                let proof = source(&id, &self.player, "I will bring purchased chalk to this counter.");
                let interactions = &mut self.interactions.iter_mut().find(|(a, _)| *a == self.npc).unwrap().1;
                let r=interactions.call("promise_remember",&obj!{"promisor"=>self.player.clone(),"beneficiary"=>self.npc.clone(),"task"=>"Bring purchased chalk to the counter.","source"=>id}.to_json(),&vec![proof.clone()].into(),self.tick as u32,8192,|_|false)?;
                let note = get(&r, "id")?.clone();
                interactions.set_progress(&obj! {"id"=>note,"expected"=>0,"state"=>"accepted","source"=>proof})?;
                r
            }
            "save" => {
                fields(v, &["slot"])?;
                let (slot_num, slot_name) = match v.get("slot") {
                    Some(Value::Number(_)) => {
                        let n = num(v, "slot", 32)?;
                        (Some(n), format!("slot-{n}"))
                    }
                    Some(Value::String(s)) => (None, s.clone()),
                    _ => return Err(err()),
                };
                self.stop(None)?;
                let mut save_file = self.load_or_new_save_file()?;
                let mut state = SaveState::new(&slot_name, self.tick)?;
                state.put_engine(&self.engine_merchant)?;
                state.put_engine(&self.engine_keeper)?;
                state.put_engine(&self.engine_hound)?;
                state.insert("host/game", "host", &self.game.public())?;
                state.insert(
                    "host/app",
                    "host",
                    &obj! {
                        "player" => self.player.clone(),
                        "npc" => self.npc.clone(),
                        "seq" => self.seq,
                        "counters" => self.counters.to_vec(),
                        "fulfilled" => self.fulfilled.to_vec(),
                        "counter_seq" => self.counter_seq,
                        "transcript" => self.transcript.clone(),
                        "actions" => self.actions.clone(),
                        "world_ops" => self.world_ops.clone(),
                        "receipts" => self.receipts.clone(),
                    },
                )?;
                save_file.put(state, Some(&self.game_data))?;
                atomic(&self.dir.join("saves.json"), save_file.to_json().as_bytes())?;

                let mut saved = obj! {"version"=>1,"pack_digest"=>gamectx::media::visual::content_digest(self.pack.to_value().to_json().as_bytes())?,"seed"=>self.game.seed,"actions"=>self.actions.clone(),"world_ops"=>self.world_ops.clone(),"receipts"=>self.receipts.clone(),"interactions"=>self.interactions.iter().map(|(_,i)|i.checkpoint()).collect::<Vec<_>>(),"player"=>self.player.clone(),"npc"=>self.npc.clone(),"tick"=>self.tick,"seq"=>self.seq,"counters"=>self.counters.to_vec(),"fulfilled"=>self.fulfilled.to_vec(),"counter_seq"=>self.counter_seq,"transcript"=>self.transcript.clone()};
                if self.mind.enabled() || !self.greetings.is_empty() {
                    set(&mut saved, "encounters", self.greetings.checkpoint());
                }
                if self.reactions.enabled() || !self.reactions.records.is_empty() {
                    set(&mut saved, "npc_reactions", self.reactions.checkpoint()?);
                }
                if self.mind.enabled() || !array(get(&self.mind.views.checkpoint(), "entries")?)?.is_empty() {
                    set(&mut saved, "viewpoints", self.mind.views.checkpoint());
                }
                if let Some(n) = slot_num {
                    atomic(&self.dir.join(format!("branch-{n}.json")), saved.to_json().as_bytes())?;
                    self.notice = format!("Branch {} saved.", char::from(b'A' + n as u8));
                    obj! {"saved"=>n, "slot"=>slot_name}
                } else {
                    self.notice = format!("Slot '{slot_name}' saved.");
                    obj! {"saved"=>true, "slot"=>slot_name}
                }
            }
            "load" => {
                fields(v, &["slot"])?;
                let (slot_num, slot_name) = match v.get("slot") {
                    Some(Value::Number(_)) => {
                        let n = num(v, "slot", 32)?;
                        (Some(n), format!("slot-{n}"))
                    }
                    Some(Value::String(s)) => (None, s.clone()),
                    _ => return Err(err()),
                };
                if let Some(n) = slot_num {
                    let path = self.dir.join(format!("branch-{n}.json"));
                    if path.exists() {
                        let saved = parse(&read_file(&path, 8 * 1024 * 1024)?)?;
                        self.restore(&saved)?;
                    }
                }
                let saves_path = self.dir.join("saves.json");
                if saves_path.exists() {
                    if let Ok(save_file) = SaveFile::parse(&read_file(&saves_path, 8 * 1024 * 1024)?) {
                        if let Some(state) = save_file.state(&slot_name) {
                            if let Ok(e) = state.engine("merchant", &self.game_data) {
                                self.engine_merchant = e;
                            }
                            if let Ok(e) = state.engine("keeper", &self.game_data) {
                                self.engine_keeper = e;
                            }
                            if let Ok(e) = state.engine("hound", &self.game_data) {
                                self.engine_hound = e;
                            }
                        }
                    }
                }
                if let Some(n) = slot_num {
                    obj! {"loaded"=>n, "slot"=>slot_name}
                } else {
                    obj! {"loaded"=>true, "slot"=>slot_name}
                }
            }
            "fork" => {
                fields(v, &["from", "to"])?;
                let from = text(v, "from")?;
                let to = text(v, "to")?;
                let mut save_file = self.load_or_new_save_file()?;
                save_file.fork(from, to, self.tick)?;
                atomic(&self.dir.join("saves.json"), save_file.to_json().as_bytes())?;
                self.notice = format!("Forked '{from}' into '{to}'.");
                obj! {"forked"=>true, "from"=>from, "to"=>to}
            }
            _ => return Err(err()),
        };
        if op == "delivery"
            && matches!(v.get("event").and_then(Value::as_str), Some("finished" | "interrupted"))
            && self.manager.policy.enabled()
            && self.manager.policy.to_value().get("residency").and_then(Value::as_str) == Some("on_demand")
            && self.manager.report.get("loaded").and_then(|l| l.get("channel")).and_then(Value::as_str) == Some("tts")
        {
            jobs.push(self.manager_job("release", "tts", false)?);
        }
        let next = if op == "act" && !immediate_greeting && (self.active() || self.game.room == "gallery") {
            vec!["text"]
        } else {
            Vec::new()
        };
        self.manager_update(next)?;
        if jobs.is_empty() && self.pending.is_empty() && !self.local_paused && self.manager.policy.enabled() {
            if let Some(id) = self
                .manager
                .route
                .get("prewarm")
                .and_then(Value::as_array)
                .and_then(|v| v.first())
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                jobs.push(self.manager_job("prewarm", &id, true)?);
            }
        }
        Ok((result, jobs))
    }
    fn restore(&mut self, v: &Value) -> Result<()> {
        let views = if let Some(x) = v.get("viewpoints") {
            gamectx::character::roleplay::Viewpoints::recover(x)?
        } else {
            gamectx::character::roleplay::Viewpoints::new()
        };
        for e in array(get(&views.checkpoint(), "entries")?)? {
            if !["merchant", "keeper"].contains(&text(e, "actor")?)
                || !["player_1", "player_2"].contains(&text(e, "listener")?)
            {
                return Err(err());
            }
        }
        let mut base = v.clone();
        if let Value::Object(rows) = &mut base {
            rows.retain(|(k, _)| k != "viewpoints" && k != "npc_reactions" && k != "encounters");
        }
        fields(
            &base,
            &[
                "version",
                "pack_digest",
                "seed",
                "actions",
                "world_ops",
                "receipts",
                "interactions",
                "player",
                "npc",
                "tick",
                "seq",
                "counters",
                "fulfilled",
                "counter_seq",
                "transcript",
            ],
        )?;
        if num(v, "version", 1)? != 1 {
            return Err(err());
        }
        if text(v, "pack_digest")? != gamectx::media::visual::content_digest(self.pack.to_value().to_json().as_bytes())?
        {
            return Err(err());
        }
        let actions = array(get(v, "actions")?)?.to_vec();
        if actions.len() > 256 {
            return Err(err());
        }
        let (reactions, mut game) =
            self.reactions.replay(v.get("npc_reactions"), num(v, "seed", 65535)? as u32, &actions)?;
        game.enable_world_systems();
        let world_ops = array(get(v, "world_ops")?)?.to_vec();
        if world_ops.len() > 8192 {
            return Err(err());
        }
        let mut world = LivingWorld::new(self.pack.clone(), "studio_save")?;
        for o in &world_ops {
            let a = get(o, "args")?;
            match text(o, "op")? {
                "observe" => {
                    world.observe(a)?;
                }
                "record" => {
                    world.record(text(a, "actor")?, get(a, "event")?)?;
                }
                "tick" => {
                    world.tick(a)?;
                }
                "counters" => {
                    world.update_counters(a)?;
                }
                _ => return Err(err()),
            }
        }
        let tick = num(v, "tick", u32::MAX as u64)?;
        let mut party = Self::party(&self.people, tick)?;
        let receipts = array(get(v, "receipts")?)?.to_vec();
        if receipts.len() > 128 {
            return Err(err());
        }
        for r in &receipts {
            party.receipt(r)?;
        }
        let interactions = array(get(v, "interactions")?)?
            .iter()
            .map(|r| Ok((text(r, "actor")?.to_owned(), InteractionTools::recover(r, 0, tick as u32)?)))
            .collect::<Result<Vec<_>>>()?;
        if interactions.len() != 2
            || !interactions.iter().any(|(a, _)| a == "merchant")
            || !interactions.iter().any(|(a, _)| a == "keeper")
        {
            return Err(err());
        }
        for (_, i) in &interactions {
            let state = i.to_value();
            if text(&state, "scope")? != "studio_save" || get(&state, "participants")? != &self.people {
                return Err(err());
            }
        }
        let player = text(v, "player")?;
        let npc = text(v, "npc")?;
        if !["player_1", "player_2"].contains(&player) || !["merchant", "keeper"].contains(&npc) {
            return Err(err());
        }
        let counts = |k| -> Result<[u64; 2]> {
            let rows = array(get(v, k)?)?;
            if rows.len() != 2 {
                return Err(err());
            }
            Ok([game::uint(&rows[0], 1_000_000)? as u64, game::uint(&rows[1], 1_000_000)? as u64])
        };
        let counters = counts("counters")?;
        let fulfilled = counts("fulfilled")?;
        let counter_seq = num(v, "counter_seq", u32::MAX as u64)?;
        let transcript = array(get(v, "transcript")?)?.to_vec();
        if transcript.len() > 128 {
            return Err(err());
        }
        for row in &transcript {
            fields(row, &["speaker", "text", "kind"])?;
            for k in ["speaker", "text", "kind"] {
                if text(row, k)?.len() > 4096 {
                    return Err(err());
                }
            }
        }
        let mut seq = self.seq.max(num(v, "seq", u32::MAX as u64)?);
        let mut greetings = if let Some(data) = v.get("encounters") {
            gamectx::character::roleplay::GreetingMemory::recover(&self.greetings.config(), data)?
        } else {
            gamectx::character::roleplay::GreetingMemory::new(&self.greetings.config())?
        };
        if v.get("encounters").is_none() && self.mind.enabled() {
            super::greetings::seed_legacy(&mut greetings, &receipts, &views.checkpoint(), tick, &mut seq)?;
        }
        for e in array(get(&greetings.checkpoint(), "entries")?)? {
            if !["merchant", "keeper"].contains(&text(e, "actor")?)
                || !["player_1", "player_2"].contains(&text(e, "listener")?)
                || num(e, "last_at", u32::MAX as u64)? > tick
                || num(e, "token", u32::MAX as u64)? > seq
                || e.get("visit") != Some(&Value::Null)
                    && num(e, "visit", u32::MAX as u64)? > super::greetings::visit(&actions)?
            {
                return Err(err());
            }
        }
        self.stop(None)?;
        self.focus = None;
        self.manager.reset_scene()?;
        self.manager_significance = 0;
        self.manager_delta = 0;
        self.game = game;
        self.reactions = reactions;
        self.greetings = greetings;
        self.mind.replace(views, &self.dir)?;
        self.world = world;
        self.actions = actions;
        self.world_ops = world_ops;
        self.receipts = receipts;
        self.party = party;
        self.interactions = interactions;
        self.tick = tick;
        self.seq = self.seq.max(seq);
        self.player = player.into();
        self.npc = npc.into();
        self.counters = counters;
        self.fulfilled = fulfilled;
        self.counter_seq = counter_seq;
        self.transcript = transcript;
        self.speech = None;
        self.deliberation.latest = Value::Null;
        self.mic_open = false;
        self.epoch += 1;
        self.mic_open = false;
        self.branch += 1;
        self.notice = "Branch loaded. Spending and asset approval were not rolled back.".into();
        Ok(())
    }
    pub fn asset_bytes(&self, uri: &str) -> Result<Vec<u8>> {
        let name = uri.strip_prefix("asset://").ok_or_else(err)?;
        let sha = name.strip_suffix(".png").ok_or_else(err)?;
        if sha.len() != 64 || !sha.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
            return Err(err());
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(self.dir.join("assets").join(name))
            .map_err(|_| err())?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| err())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(err());
        }
        if gamectx::media::visual::content_digest(&bytes)? != sha {
            return Err(err());
        }
        Ok(bytes)
    }
    pub fn asset(&self, key: &str, candidate: bool) -> Result<Vec<u8>> {
        let entry = self.assets.inspect(key)?;
        let reference = if candidate { get(&entry, "candidate")? } else { get(&entry, "asset")? };
        self.asset_bytes(text(reference, "asset_uri")?)
    }
}
