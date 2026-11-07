//! Opt-in host policies for the local example studio. No network, audio device or
//! world executor: external transports acknowledge only confirmed character offsets.
use crate::{checked as c, life::Coordinator, obj, spending::SpendingLedger, Result, Value};
pub fn backends(modality: &str) -> Result<&'static [&'static str]> {
    match modality {
        "text" => Ok(&["0b_fallback", "local_ollama", "online_openai_compatible"]),
        "image" => Ok(&["cached_only", "local_sd_cpp", "online_image_api"]),
        "tts" => Ok(&["off", "browser_web_speech", "local_tts_http", "online_tts_api"]),
        "stt" => Ok(&["off", "browser_speech_recognition", "local_stt_http", "online_stt_api"]),
        _ => Err(c::bad()),
    }
}
pub fn fallback(modality: &str) -> Result<&'static str> {
    match modality {
        "text" => Ok("0b_fallback"),
        "image" => Ok("cached_only"),
        "tts" => Ok("browser_web_speech"),
        "stt" => Ok("browser_speech_recognition"),
        _ => Err(c::bad()),
    }
}
#[derive(Debug)]
pub struct ProviderBudget {
    ledger: SpendingLedger,
    authorized: u64,
    cap: u64,
}
impl ProviderBudget {
    pub fn new(cap: u64) -> Result<Self> {
        Ok(Self { ledger: SpendingLedger::new(cap, 1)?, authorized: cap, cap })
    }
    /// Clock zero intentionally makes this a cumulative session cap, not a rolling window.
    pub fn reserve(&mut self, modality: &str, backend: &str, ceiling: u64, local_fallback: bool) -> Result<Value> {
        let choices = backends(modality)?;
        if !choices.contains(&backend) || ceiling > 1_000_000_000_000 {
            return Err(c::bad());
        }
        let mut selected = backend;
        let mut reservation = Value::Null;
        let mut reason = "selected";
        if backend.starts_with("online_") {
            let spent = c::uint(c::get(&self.ledger.summary(0)?, "spent")?, 0, 1_000_000_000_000)?;
            let ticket = if ceiling == 0 || spent.saturating_add(ceiling) > self.cap {
                None
            } else {
                self.ledger.reserve(0, ceiling).ok()
            };
            if let Some(id) = ticket {
                reservation = id.into();
            } else {
                selected = if local_fallback {
                    choices.iter().copied().find(|b| b.starts_with("local_")).ok_or_else(c::bad)?
                } else {
                    fallback(modality)?
                };
                reason = "cap_or_unpriced";
            }
        }
        Ok(obj! {"backend"=>selected,"reservation"=>reservation,"reason"=>reason})
    }
    pub fn dispatch(&mut self, id: u64) -> Result<()> {
        self.ledger.dispatch(id, 0)
    }
    pub fn cancel(&mut self, id: u64) -> Result<()> {
        self.ledger.cancel_before_dispatch(id, 0)
    }
    /// Without authoritative billing retain the entire declared upper bound, even on error.
    pub fn finish(&mut self, id: u64) -> Result<()> {
        self.ledger.finish(id, 0, None)
    }
    pub fn limit(&mut self, cap: u64) -> Result<()> {
        let spent = c::uint(c::get(&self.ledger.summary(0)?, "spent")?, 0, 1_000_000_000_000)?;
        if cap < spent && cap != self.cap || cap > self.authorized {
            return Err(c::bad());
        }
        self.cap = cap;
        Ok(())
    }
    pub fn summary(&self) -> Value {
        let s = self.ledger.summary(0).expect("fixed clock");
        obj! {"cap_micros"=>self.cap,"authorized_cap_micros"=>self.authorized,"charged_or_reserved_micros"=>s.get("spent").expect("spent").clone(),"pending"=>s.get("pending").expect("pending").clone()}
    }
    pub fn checkpoint(&self) -> Value {
        obj! {"authorized"=>self.authorized,"cap"=>self.cap,"ledger"=>self.ledger.to_value()}
    }
    pub fn recover(v: &Value, authorized: u64) -> Result<Self> {
        crate::request::bounded(v, 65536)?;
        c::fields(v, &["authorized", "cap", "ledger"])?;
        c::uint(c::get(v, "authorized")?, 0, 1_000_000_000_000)?;
        c::uint(c::get(v, "cap")?, 0, 1_000_000_000_000)?;
        let recovered = SpendingLedger::restore(c::get(v, "ledger")?, 0)?;
        let spent = c::uint(c::get(&recovered.summary(0)?, "spent")?, 0, 1_000_000_000_000)?;
        let mut raw = recovered.to_value();
        crate::archetype::set_field(&mut raw, "limit", authorized.max(spent).into());
        let ledger = SpendingLedger::restore(&raw, 0)?;
        Ok(Self { ledger, authorized, cap: authorized })
    }
}
#[derive(Debug)]
pub struct VoiceTurn {
    coordinator: Coordinator,
    id: u64,
    text: String,
    recipient: String,
    played: u64,
    heard: u64,
    stage: &'static str,
    done: bool,
}
impl VoiceTurn {
    pub fn new(scope: &str, actor: &str, recipient: &str, people: &Value, text: &str) -> Result<Self> {
        crate::life::line(text)?;
        if text.chars().count() > 240 {
            return Err(c::bad());
        }
        let mut coordinator = Coordinator::new(scope, actor, people)?;
        coordinator.bind_scene(&obj!{"id"=>"voice_scene","revision"=>1,"epoch"=>1,"at"=>0,"participants"=>people.clone(),"facts"=>Vec::<Value>::new(),"affordances"=>Vec::<Value>::new()})?;
        coordinator.thread(&obj!{"id"=>"voice","topic"=>"Confirmed voice delivery","questions"=>Vec::<Value>::new(),"promises"=>Vec::<Value>::new(),"unfinished"=>Vec::<Value>::new(),"listener"=>recipient,"participants"=>people.clone(),"shared"=>Vec::<Value>::new(),"referents"=>Vec::<Value>::new()})?;
        let generated = coordinator.generate("voice", text, &Vec::<Value>::new().into(), 0)?;
        let id = crate::life::num(&generated, "id")?;
        coordinator.validate(id, 0)?;
        Ok(Self {
            coordinator,
            id,
            text: text.into(),
            recipient: recipient.into(),
            played: 0,
            heard: 0,
            stage: "validated",
            done: false,
        })
    }
    fn deliver(&mut self, event: &str, through: u64) -> Result<()> {
        self.coordinator.delivery(&obj!{"id"=>self.id,"event"=>event,"through"=>through,"recipient"=>if event=="heard" {Value::str(&self.recipient)}else{Value::Null}}).map(|_|())
    }
    pub fn queue(&mut self) -> Result<()> {
        if self.done || self.stage != "validated" {
            return Err(c::bad());
        }
        self.deliver("queued", 0)?;
        self.stage = "queued";
        Ok(())
    }
    fn bounds(&self, through: u64, heard: u64) -> Result<()> {
        if self.done
            || through < self.played
            || through > self.text.chars().count() as u64
            || heard < self.heard
            || heard > through
        {
            return Err(c::bad());
        }
        Ok(())
    }
    pub fn progress(&mut self, through: u64, heard: u64) -> Result<()> {
        self.bounds(through, heard)?;
        if !matches!(self.stage, "queued" | "played") {
            return Err(c::bad());
        }
        self.deliver("played", through)?;
        if heard > self.heard {
            self.deliver("heard", heard)?;
        }
        self.played = through;
        self.heard = heard;
        self.stage = "played";
        Ok(())
    }
    pub fn interrupt(&mut self, through: u64, heard: u64) -> Result<()> {
        self.bounds(through, heard)?;
        if self.stage == "validated" {
            if through != 0 || heard != 0 {
                return Err(c::bad());
            }
            self.deliver("cancelled", 0)?;
            self.stage = "cancelled";
        } else {
            self.progress(through, heard)?;
            self.deliver("interrupted", through)?;
            self.stage = "interrupted";
        }
        self.done = true;
        Ok(())
    }
    pub fn finish(&mut self, through: u64, heard: u64) -> Result<()> {
        if through != self.text.chars().count() as u64 || heard != through {
            return Err(c::bad());
        }
        self.progress(through, heard)?;
        self.done = true;
        Ok(())
    }
    pub fn recognition(&self, v: &Value) -> Result<Value> {
        self.coordinator.recognition(v)
    }
    pub fn snapshot(&self) -> Value {
        obj! {"stage"=>self.stage,"played"=>self.played,"heard"=>self.heard,"done"=>self.done,"heard_text"=>self.heard_text()}
    }
    pub fn heard_text(&self) -> String {
        self.text.chars().take(self.heard as usize).collect()
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}
pub fn routing_workflow(a: &Value) -> Result<Value> {
    crate::request::bounded(a, 65536)?;
    c::fields(a, &["cap", "operations"])?;
    let mut b = ProviderBudget::new(c::uint(c::get(a, "cap")?, 0, 1_000_000_000_000)?)?;
    workflow(c::get(a, "operations")?, |op, v| match op {
        "reserve" => {
            c::fields(v, &["modality", "backend", "ceiling", "local_fallback"])?;
            b.reserve(
                c::text(c::get(v, "modality")?)?,
                c::text(c::get(v, "backend")?)?,
                c::uint(c::get(v, "ceiling")?, 0, 1_000_000_000_000)?,
                c::boolean(c::get(v, "local_fallback")?)?,
            )
        }
        "dispatch" | "finish" | "cancel" => {
            c::fields(v, &["id"])?;
            let id = crate::life::num(v, "id")?;
            match op {
                "dispatch" => b.dispatch(id)?,
                "finish" => b.finish(id)?,
                _ => b.cancel(id)?,
            };
            Ok(obj! {"accepted"=>true})
        }
        "limit" => {
            c::fields(v, &["cap"])?;
            let cap = c::uint(c::get(v, "cap")?, 0, 1_000_000_000_000)?;
            b.limit(cap)?;
            Ok(obj! {"cap_micros"=>cap})
        }
        "inspect" => {
            c::fields(v, &[])?;
            Ok(b.summary())
        }
        _ => Err(c::bad()),
    })
}
pub fn voice_workflow(a: &Value) -> Result<Value> {
    crate::request::bounded(a, 65536)?;
    c::fields(a, &["text", "operations"])?;
    let mut voice = VoiceTurn::new(
        "studio",
        "merchant",
        "player_1",
        &vec!["merchant", "player_1", "player_2"].into(),
        c::text(c::get(a, "text")?)?,
    )?;
    workflow(c::get(a, "operations")?, |op, v| {
        match op {
            "queue" => {
                c::fields(v, &[])?;
                voice.queue()?
            }
            "progress" | "interrupt" | "finish" => {
                c::fields(v, &["through", "heard"])?;
                let n = crate::life::num(v, "through")?;
                let h = crate::life::num(v, "heard")?;
                match op {
                    "progress" => voice.progress(n, h)?,
                    "interrupt" => voice.interrupt(n, h)?,
                    _ => voice.finish(n, h)?,
                }
            }
            "recognition" => return voice.recognition(v),
            "inspect" => {
                c::fields(v, &[])?;
                return Ok(voice.snapshot());
            }
            _ => return Err(c::bad()),
        }
        Ok(obj! {"accepted"=>true})
    })
}
fn workflow(ops: &Value, mut run: impl FnMut(&str, &Value) -> Result<Value>) -> Result<Value> {
    let ops = c::list(ops)?;
    if ops.len() > 256 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        let result = (|| {
            c::fields(op, &["op", "args"])?;
            run(c::text(c::get(op, "op")?)?, c::get(op, "args")?)
        })();
        results.push(match result {
            Ok(r) => obj! {"ok"=>true,"result"=>r},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results,"actual_provider_calls"=>0})
}
