//! Optional voice delivery: confirmed playback/hearing offsets, bounded recording history,
//! utterance buffering and plans for separately installed native audio engines.
mod archive;
mod native_audio;
mod speech_buffer;
use crate::{character::life::Coordinator, checked as c, obj, Result, Value};
pub(crate) use archive::fixture as archive_fixture;
pub use archive::VoiceArchive;
pub use native_audio::native_audio_plan;
pub use speech_buffer::speech_buffer;
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
        crate::character::life::line(text)?;
        if text.chars().count() > 240 {
            return Err(c::bad());
        }
        let mut coordinator = Coordinator::new(scope, actor, people)?;
        coordinator.bind_scene(&obj!{"id"=>"voice_scene","revision"=>1,"epoch"=>1,"at"=>0,"participants"=>people.clone(),"facts"=>Vec::<Value>::new(),"affordances"=>Vec::<Value>::new()})?;
        coordinator.thread(&obj!{"id"=>"voice","topic"=>"Confirmed voice delivery","questions"=>Vec::<Value>::new(),"promises"=>Vec::<Value>::new(),"unfinished"=>Vec::<Value>::new(),"listener"=>recipient,"participants"=>people.clone(),"shared"=>Vec::<Value>::new(),"referents"=>Vec::<Value>::new()})?;
        let generated = coordinator.generate("voice", text, &Vec::<Value>::new().into(), 0)?;
        let id = crate::character::life::num(&generated, "id")?;
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
pub fn voice_workflow(a: &Value) -> Result<Value> {
    crate::packet::request::bounded(a, 65536)?;
    c::fields(a, &["text", "operations"])?;
    let mut voice = VoiceTurn::new(
        "studio",
        "merchant",
        "player_1",
        &vec!["merchant", "player_1", "player_2"].into(),
        c::text(c::get(a, "text")?)?,
    )?;
    super::workflow(c::get(a, "operations")?, |op, v| {
        match op {
            "queue" => {
                c::fields(v, &[])?;
                voice.queue()?
            }
            "progress" | "interrupt" | "finish" => {
                c::fields(v, &["through", "heard"])?;
                let n = crate::character::life::num(v, "through")?;
                let h = crate::character::life::num(v, "heard")?;
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
