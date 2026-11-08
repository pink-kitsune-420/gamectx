//! Original CLI integration. No engine, library, weight, or third-party code is bundled.
use super::*;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static CANCEL: AtomicU64 = AtomicU64::new(0);
pub fn cancel() {
    CANCEL.fetch_add(1, Ordering::SeqCst);
}
fn issue() -> Error {
    Error::new("manager: Native voice failed. Check the separately installed executable/model, voice alias and host voice log.")
}
#[derive(Clone)]
pub struct Native {
    config: Value,
    stopped: Vec<String>,
}
impl Native {
    pub fn load(dir: &Path) -> Result<Self> {
        let p = dir.join("native-voice.json");
        let config =
            if p.exists() { parse(&read_file(&p, 32768)?)? } else { parse(include_str!("native-voice.example.json"))? };
        fields(&config, &["version", "tts", "stt"])?;
        if num(&config, "version", 1)? != 1 {
            return Err(err());
        }
        for (ch, key) in [("tts", "voices"), ("stt", "models")] {
            let r = get(&config, ch)?;
            fields(r, &["executable", key])?;
            let exe = text(r, "executable")?;
            if exe.len() > 2048 || exe.chars().any(char::is_control) {
                return Err(err());
            }
            let entries = get(r, key)?.as_object().ok_or_else(err)?;
            if entries.len() > 16 {
                return Err(err());
            }
            for (alias, path) in entries {
                gamectx::media::voice::native_audio_plan(
                    &gamectx::obj! {"engine"=>"whisper","request"=>gamectx::obj!{"model"=>alias.clone()}},
                )?;
                let path = path.as_str().ok_or_else(err)?;
                if path.is_empty() || path.len() > 2048 || path.chars().any(char::is_control) {
                    return Err(err());
                }
            }
        }
        Ok(Self { config, stopped: Vec::new() })
    }
    pub fn handles(&self, ch: &str, model: &str) -> bool {
        if !["tts", "stt"].contains(&ch) {
            return false;
        }
        let r = get(&self.config, ch).unwrap();
        !text(r, "executable").unwrap().is_empty()
            && (ch == "tts" && model == "piper" || ch == "stt" && get(r, "models").unwrap().get(model).is_some())
    }
    pub fn available(&self, ch: &str, model: &str) -> bool {
        if !self.handles(ch, model) || self.stopped.iter().any(|s| s == ch) {
            return false;
        }
        let r = get(&self.config, ch).unwrap();
        Path::new(text(r, "executable").unwrap()).is_file()
            && get(r, if ch == "tts" { "voices" } else { "models" })
                .unwrap()
                .as_object()
                .unwrap()
                .iter()
                .any(|(alias, p)| (ch == "tts" || alias == model) && Path::new(p.as_str().unwrap()).is_file())
    }
    pub fn start(&mut self, ch: &str, model: &str) -> Result<()> {
        self.stopped.retain(|s| s != ch);
        if !self.available(ch, model) {
            return Err(issue());
        }
        Ok(())
    }
    pub fn stop(&mut self, ch: &str) {
        if !self.stopped.iter().any(|s| s == ch) {
            self.stopped.push(ch.into());
        }
    }
    pub fn run(&self, ch: &str, model: &str, r: &providers::Request, dir: &Path) -> Result<Vec<u8>> {
        if !self.available(ch, model) {
            return Err(issue());
        }
        let serial = CANCEL.load(Ordering::SeqCst);
        let folder = dir.canonicalize().map_err(|_| err())?.join("voice-job");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).map_err(|_| err())?;
        let result = (|| {
            let input_path = folder.join("capture.wav");
            let output = folder.join("result");
            let engine = get(&self.config, ch)?;
            let absolute = |s: &str| -> Result<String> {
                Ok(std::env::current_dir().map_err(|_| err())?.join(s).to_str().ok_or_else(err)?.to_owned())
            };
            let mut argv = vec![Value::str(&absolute(text(engine, "executable")?)?)];
            let expected;
            let mut buffered = Vec::new();
            let stdin = if ch == "tts" {
                let request = gamectx::data::json::parse(std::str::from_utf8(&r.body).map_err(|_| err())?, 4096)?;
                let plan =
                    gamectx::media::voice::native_audio_plan(&gamectx::obj! {"engine"=>"piper","request"=>request})?;
                let voice = text(&plan, "voice")?;
                let model = absolute(text(get(engine, "voices")?, voice).map_err(|_| issue())?)?;
                let path = model.as_str();
                if !Path::new(path).is_file() {
                    return Err(issue());
                }
                expected = output.with_extension("wav");
                for a in [
                    "--model",
                    path,
                    "--output_file",
                    expected.to_str().ok_or_else(err)?,
                    "--length_scale",
                    text(&plan, "length_scale")?,
                    "--json-input",
                ] {
                    argv.push(a.into());
                }
                // One engine session, bounded phrases, then one continuous PCM buffer.
                let parts =
                    gamectx::media::voice::speech_buffer(&gamectx::obj! {"text"=>text(&plan,"text")?,"start"=>0})?;
                let mut lines = String::new();
                for (i, part) in get(&parts, "parts")?.as_array().ok_or_else(err)?.iter().enumerate() {
                    let path =
                        std::env::current_dir().map_err(|_| err())?.join(&folder).join(format!("part-{i:03}.wav"));
                    lines.push_str(&gamectx::obj!{"text"=>get(part,"text")?.clone(),"output_file"=>path.to_str().ok_or_else(err)?}.to_json());
                    lines.push('\n');
                    buffered.push((path, num(part, "through", 240)?));
                }
                lines
            } else {
                let (alias, audio) = capture(r)?;
                if alias != model {
                    return Err(err());
                }
                gamectx::media::voice::native_audio_plan(
                    &gamectx::obj! {"engine"=>"whisper","request"=>gamectx::obj!{"model"=>alias.clone()}},
                )?;
                let model = absolute(text(get(engine, "models")?, &alias)?)?;
                let path = model.as_str();
                std::fs::write(&input_path, audio).map_err(|_| err())?;
                expected = output.with_extension("txt");
                for a in [
                    "--model",
                    path,
                    "--file",
                    input_path.to_str().ok_or_else(err)?,
                    "--threads",
                    "4",
                    "--no-gpu",
                    "--output-txt",
                    "--output-file",
                    output.to_str().ok_or_else(err)?,
                ] {
                    argv.push(a.into());
                }
                String::new()
            };
            if stdin.len() > 16384 {
                return Err(err());
            }
            let log = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(dir.join("native-voice.log"))
                .map_err(|_| err())?;
            let mut child = Command::new(super::executable()?)
                .arg("--managed-service")
                .stdin(Stdio::piped())
                .stdout(log.try_clone().map_err(|_| err())?)
                .stderr(log)
                .spawn()
                .map_err(|_| issue())?;
            let spec = gamectx::obj! {"argv"=>argv,"env"=>gamectx::obj!{},"input"=>stdin,"process_group"=>true};
            let write = child.stdin.as_mut().ok_or_else(err)?.write_all(format!("{}\n", spec.to_json()).as_bytes());
            let deadline = Instant::now() + Duration::from_secs(90);
            let result = (|| {
                write.map_err(|_| err())?;
                loop {
                    if CANCEL.load(Ordering::SeqCst) != serial {
                        return Err(Error::new("manager: Voice request cancelled; no trailing speech was confirmed."));
                    }
                    if Instant::now() > deadline
                        || std::fs::metadata(dir.join("native-voice.log")).is_ok_and(|m| m.len() > 1_048_576)
                        || std::fs::metadata(&expected).is_ok_and(|m| m.len() > providers::MAX_BYTES as u64)
                        || buffered
                            .iter()
                            .any(|(p, _)| std::fs::metadata(p).is_ok_and(|m| m.len() > providers::MAX_BYTES as u64))
                    {
                        return Err(issue());
                    }
                    if let Some(status) = child.try_wait().map_err(|_| err())? {
                        if !status.success() {
                            return Err(issue());
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                if ch == "tts" {
                    let mut parts = Vec::new();
                    let mut total = 0;
                    for (path, through) in &buffered {
                        let bytes = bounded_bytes(path, providers::MAX_BYTES)?;
                        total += bytes.len();
                        if total > providers::MAX_BYTES {
                            return Err(err());
                        }
                        parts.push((bytes, *through));
                    }
                    Ok(super::speech_wav::join(&parts)?.to_json().into_bytes())
                } else {
                    let bytes = bounded_bytes(&expected, 4096)?;
                    let text = std::str::from_utf8(&bytes)
                        .map_err(|_| err())?
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    if text.chars().count() > 1024 {
                        return Err(err());
                    }
                    Ok(gamectx::obj! {"text"=>text}.to_json().into_bytes())
                }
            })();
            drop(child.stdin.take());
            let deadline = Instant::now() + Duration::from_secs(3);
            while child.try_wait().map_err(|_| err())?.is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            if child.try_wait().map_err(|_| err())?.is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
            result
        })();
        let _ = std::fs::remove_dir_all(folder);
        result
    }
}
fn bounded_bytes(path: &Path, cap: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path).map_err(|_| issue())?.take(cap as u64 + 1).read_to_end(&mut bytes).map_err(|_| err())?;
    if bytes.len() > cap {
        return Err(err());
    }
    Ok(bytes)
}
fn capture(r: &providers::Request) -> Result<(String, &[u8])> {
    const HEAD: &[u8] = b"--gamectx_studio_audio_boundary\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n";
    const MIDDLE:&[u8]=b"\r\n--gamectx_studio_audio_boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"capture.wav\"\r\nContent-Type: audio/wav\r\n\r\n";
    const END: &[u8] = b"\r\n--gamectx_studio_audio_boundary--\r\n";
    let tail = r.body.strip_prefix(HEAD).ok_or_else(err)?;
    let split = tail.windows(MIDDLE.len()).position(|w| w == MIDDLE).ok_or_else(err)?;
    let alias = std::str::from_utf8(&tail[..split]).map_err(|_| err())?.to_owned();
    let audio = tail[split + MIDDLE.len()..].strip_suffix(END).ok_or_else(err)?;
    providers::validate_capture(audio)?;
    Ok((alias, audio))
}
pub fn configure(root: &Path, dir: &Path) -> Result<()> {
    let root = root.canonicalize().map_err(|_| issue())?;
    let path = |s: &str| -> Result<Value> {
        let p = root.join(s);
        if !p.is_file() {
            return Err(issue());
        }
        Ok(p.to_str().ok_or_else(err)?.into())
    };
    let v = gamectx::obj! {"version"=>1,"tts"=>gamectx::obj!{"executable"=>path("bin/piper")?,"voices"=>gamectx::obj!{"merchant"=>path("models/tts/en_US-ljspeech-medium.onnx")?,"keeper"=>path("models/tts/en_US-norman-medium.onnx")?,"alloy"=>path("models/tts/en_US-ljspeech-medium.onnx")?,"onyx"=>path("models/tts/en_US-norman-medium.onnx")?}},"stt"=>gamectx::obj!{"executable"=>path("bin/whisper-cli")?,"models"=>gamectx::obj!{"whisper-tiny"=>path("models/stt/ggml-tiny.en-q8_0.bin")?,"whisper-base"=>path("models/stt/ggml-base.en-q5_1.bin")?}}};
    std::fs::create_dir_all(dir).map_err(|_| err())?;
    if dir.join("native-voice.json").exists() {
        return Err(Error::new("Native voice configuration already exists; edit it instead of overwriting."));
    }
    atomic(&dir.join("native-voice.json"), v.to_json().as_bytes())?;
    println!("Native voice configured. In Settings choose local TTS model piper and STT model whisper-tiny (or whisper-base). Engines launch per request and exit afterward.");
    Ok(())
}
