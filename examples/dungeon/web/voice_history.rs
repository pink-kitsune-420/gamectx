//! Optional bounded session WAV history; replay is review-only and never a receipt.
use super::*;
use gamectx::{obj, studio::VoiceArchive};
use std::path::{Path, PathBuf};
pub struct History {
    dir: PathBuf,
    policy: VoiceArchive,
    records: Vec<Value>,
    warning: String,
}
impl History {
    pub fn new(root: &Path) -> Result<Self> {
        let config = root.join("voice-history.json");
        let config = if config.exists() {
            parse(&read_file(&config, 1024)?)?
        } else {
            obj! {"version"=>1,"enabled"=>false,"limit"=>32}
        };
        let mut out = Self {
            dir: root.join("voice-history"),
            policy: VoiceArchive::new(&config)?,
            records: Vec::new(),
            warning: String::new(),
        };
        std::fs::create_dir_all(&out.dir).map_err(|_| err())?;
        if !out.enabled() {
            out.clear()?;
            return Ok(out);
        }
        let index = out.dir.join("index.json");
        if index.exists() {
            let v = parse(&read_file(&index, 1_048_576)?)?;
            let rows = v.as_array().filter(|r| r.len() <= 128).ok_or_else(err)?;
            for row in rows {
                fields(row, &["key", "digest", "bytes", "speaker", "player", "text", "voice", "metadata"])?;
                if !["merchant", "keeper"].contains(&text(row, "speaker")?)
                    || !["player_1", "player_2"].contains(&text(row, "player")?)
                    || text(row, "text")?.chars().count() > 240
                    || text(row, "digest")?.len() != 64
                {
                    return Err(err());
                }
                providers::validate_voice(get(row, "voice")?)?;
                let result =
                    out.policy.remember(&obj! {"key"=>get(row,"key")?.clone(),"bytes"=>get(row,"bytes")?.clone()})?;
                out.records.push(row.clone());
                out.prune(&result)?;
            }
        }
        out.persist()?;
        Ok(out)
    }
    pub fn enabled(&self) -> bool {
        self.policy.config().get("enabled") == Some(&true.into())
    }
    pub fn configure(&mut self, config: &Value) -> Result<()> {
        let result = self.policy.configure(config)?;
        self.prune(&result)?;
        if !self.enabled() {
            self.clear()?;
        }
        atomic(&self.dir.with_extension("json"), self.policy.config().to_json().as_bytes())?;
        self.persist()
    }
    pub fn clear(&mut self) -> Result<()> {
        let result = self.policy.clear();
        self.prune(&result)?;
        let _ = std::fs::remove_dir_all(&self.dir);
        std::fs::create_dir_all(&self.dir).map_err(|_| err())?;
        self.persist()
    }
    fn persist(&self) -> Result<()> {
        atomic(&self.dir.join("index.json"), Value::from(self.records.clone()).to_json().as_bytes())
    }
    fn prune(&mut self, result: &Value) -> Result<()> {
        let kept = get(result, "kept")?.as_array().ok_or_else(err)?;
        self.records.retain(|r| kept.iter().any(|k| k.get("key") == r.get("key")));
        for key in get(result, "removed")?.as_array().ok_or_else(err)? {
            if let Some(key) = key.as_str() {
                let _ = std::fs::remove_file(self.dir.join(format!("{key}.wav")));
            }
        }
        Ok(())
    }
    pub fn key(&self, speaker: &str, player: &str, line: &str, voice: &Value, provider: &Value) -> Result<String> {
        let root = self.dir.parent().ok_or_else(err)?;
        let file = root.join("native-voice.json");
        let native = if file.exists() { parse(&read_file(&file, 32768)?)? } else { Value::Null };
        let identity = obj! {"speaker"=>speaker,"player"=>player,"text"=>line,"voice"=>voice.clone(),"provider"=>provider.clone(),"native"=>native};
        gamectx::visual::content_digest(identity.to_json().as_bytes())
    }
    pub fn bytes(&self, key: &str) -> Result<Vec<u8>> {
        let row = self.records.iter().find(|r| r.get("key").and_then(Value::as_str) == Some(key)).ok_or_else(err)?;
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(self.dir.join(format!("{key}.wav")))
            .map_err(|_| err())?
            .take(6 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| err())?;
        if bytes.len() > 6 * 1024 * 1024
            || get(row, "bytes")?.as_usize() != Some(bytes.len())
            || text(row, "digest")? != gamectx::visual::content_digest(&bytes)?
        {
            return Err(err());
        }
        super::speech_wav::pcm(&bytes)?;
        Ok(bytes)
    }
    pub fn cached(&mut self, key: &str) -> Option<Value> {
        if !self.enabled() {
            return None;
        }
        let row = self.records.iter().find(|r| r.get("key").and_then(Value::as_str) == Some(key))?.clone();
        match self.bytes(key) {
            Ok(bytes) => {
                let mut audio = row.get("metadata")?.clone();
                set(&mut audio, "audio", providers::encode64(&bytes).into());
                set(&mut audio, "archive_key", key.into());
                Some(audio)
            }
            Err(_) => {
                self.records.retain(|r| r.get("key").and_then(Value::as_str) != Some(key));
                self.policy = VoiceArchive::new(&self.policy.config()).ok()?;
                for row in &self.records {
                    self.policy
                        .remember(&obj! {"key"=>row.get("key")?.clone(),"bytes"=>row.get("bytes")?.clone()})
                        .ok()?;
                }
                let _ = std::fs::remove_file(self.dir.join(format!("{key}.wav")));
                let _ = self.persist();
                None
            }
        }
    }
    pub fn remember(
        &mut self,
        key: &str,
        speaker: &str,
        player: &str,
        line: &str,
        voice: &Value,
        audio: &Value,
    ) -> Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        let bytes = providers::decode64(text(audio, "audio")?)?;
        super::speech_wav::pcm(&bytes)?;
        let result = self.policy.remember(&obj! {"key"=>key,"bytes"=>bytes.len()})?;
        let mut metadata = audio.clone();
        if let Value::Object(rows) = &mut metadata {
            rows.retain(|(k, _)| k != "audio" && k != "archive_key");
        }
        let row = obj! {"key"=>key,"digest"=>gamectx::visual::content_digest(&bytes)?,"bytes"=>bytes.len(),"speaker"=>speaker,"player"=>player,"text"=>line,"voice"=>voice.clone(),"metadata"=>metadata};
        atomic(&self.dir.join(format!("{key}.wav")), &bytes)?;
        self.records.retain(|r| r.get("key").and_then(Value::as_str) != Some(key));
        self.records.push(row);
        self.prune(&result)?;
        self.persist()?;
        self.warning.clear();
        Ok(())
    }
    pub fn problem(&mut self) {
        self.warning = "The recording could not be archived; current playback remains available.".into();
    }
    pub fn snapshot(&self) -> Value {
        obj! {"config"=>self.policy.config(),"records"=>self.records.iter().map(|r|{let mut row=r.clone();if let Value::Object(v)=&mut row{v.retain(|(k,_)|k!="metadata");}row}).collect::<Vec<_>>(),"warning"=>self.warning.clone()}
    }
}
