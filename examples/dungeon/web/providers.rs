//! Original bounded adapters. Default GUI/local HTTP use std only. Optional HTTPS
//! uses the host's curl executable for verified TLS, never a Rust TLS dependency.
use super::*;
use gamectx::obj;
use std::{
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    time::{Duration, Instant},
};
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Config {
    pub value: Value,
}
impl Config {
    pub fn parse(s: &str) -> Result<Self> {
        let v = gamectx::json::parse(s, 65536)?;
        let mut config_keys = vec![
            "text",
            "image",
            "tts",
            "stt",
            "size",
            "local_fallback",
            "cpu_threads",
            "endpoints",
            "voices",
            "text_models",
        ];
        if v.get("speech_mode").is_some() {
            config_keys.push("speech_mode");
            if !["grounded", "dialogue"].contains(&text(&v, "speech_mode")?) {
                return Err(err());
            }
        }
        fields(&v, &config_keys)?;
        for m in ["text", "image", "tts", "stt"] {
            if !gamectx::studio::backends(m)?.contains(&text(&v, m)?) {
                return Err(err());
            }
        }
        gamectx::deployment::ModelSize::parse(text(&v, "size")?)?;
        let n = num(&v, "cpu_threads", 4)?;
        if n == 0 || !matches!(get(&v, "local_fallback")?, Value::Bool(_)) {
            return Err(err());
        }
        let endpoints = get(&v, "endpoints")?;
        fields(
            endpoints,
            &[
                "local_ollama",
                "local_sd_cpp",
                "local_tts_http",
                "local_stt_http",
                "online_openai_compatible",
                "online_image_api",
                "online_tts_api",
                "online_stt_api",
            ],
        )?;
        for (name, e) in endpoints.as_object().ok_or_else(err)? {
            let is_text = matches!(name.as_str(), "local_ollama" | "online_openai_compatible");
            fields(
                e,
                if is_text {
                    &["url", "key_env", "ceiling_micros"]
                } else {
                    &["url", "model", "key_env", "ceiling_micros"]
                },
            )?;
            endpoint(text(e, "url")?, name.starts_with("online_"))?;
            if !is_text {
                let model = text(e, "model")?;
                if model.is_empty()
                    || model.len() > 128
                    || !model.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_:./".contains(&c))
                {
                    return Err(err());
                }
            }
            num(e, "ceiling_micros", 1_000_000_000_000)?;
            if name.starts_with("online_") {
                let key = text(e, "key_env")?;
                if key.is_empty()
                    || key.len() > 64
                    || !key.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                {
                    return Err(err());
                }
            } else if get(e, "key_env")? != &Value::Null {
                return Err(err());
            }
        }
        fields(get(&v, "text_models")?, &["local_ollama", "online_openai_compatible"])?;
        for (_, models) in get(&v, "text_models")?.as_object().ok_or_else(err)? {
            fields(models, &["1", "1.5", "3", "8", "30", "70"])?;
            for (_, id) in models.as_object().ok_or_else(err)? {
                let id = id.as_str().ok_or_else(err)?;
                if id.len() > 128 || !id.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_:./".contains(&c)) {
                    return Err(err());
                }
            }
        }
        fields(get(&v, "voices")?, &["merchant", "keeper"])?;
        for (_, voice) in get(&v, "voices")?.as_object().ok_or_else(err)? {
            validate_voice(voice)?;
        }
        Ok(Self { value: v })
    }
    pub fn speech_mode(&self) -> &str {
        self.value.get("speech_mode").and_then(Value::as_str).unwrap_or("dialogue")
    }
    pub fn public(&self) -> Value {
        obj! {"speech_mode"=>self.speech_mode(),"text"=>get(&self.value,"text").unwrap().clone(),"image"=>get(&self.value,"image").unwrap().clone(),"tts"=>get(&self.value,"tts").unwrap().clone(),"stt"=>get(&self.value,"stt").unwrap().clone(),"size"=>get(&self.value,"size").unwrap().clone(),"local_fallback"=>get(&self.value,"local_fallback").unwrap().clone(),"voices"=>get(&self.value,"voices").unwrap().clone(),"text_models"=>get(&self.value,"text_models").unwrap().clone()}
    }
    pub fn endpoint(&self, b: &str) -> Result<&Value> {
        get(get(&self.value, "endpoints")?, b)
    }
    pub fn voice(&self, actor: &str) -> Result<&Value> {
        get(get(&self.value, "voices")?, actor)
    }
    pub fn has_credentials(&self, b: &str) -> bool {
        if !b.starts_with("online_") {
            return true;
        }
        self.endpoint(b)
            .and_then(|e| text(e, "key_env"))
            .ok()
            .and_then(|key| std::env::var(key).ok())
            .is_some_and(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\'))
    }
}
pub fn validate_voice(v: &Value) -> Result<()> {
    fields(v, &["voice_id", "pitch", "rate"])?;
    let id = text(v, "voice_id")?;
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err(err());
    }
    for k in ["pitch", "rate"] {
        let n = get(v, k)?.to_json().parse::<f64>().map_err(|_| err())?;
        if !n.is_finite() || !(0.5..=2.0).contains(&n) {
            return Err(err());
        }
    }
    Ok(())
}
fn endpoint(url: &str, online: bool) -> Result<()> {
    if url.len() > 512 || !url.bytes().all(|c| c.is_ascii_alphanumeric() || b":/.-_?&=%".contains(&c)) {
        return Err(err());
    }
    if online {
        let tail = url.strip_prefix("https://").ok_or_else(err)?;
        let host = tail.split('/').next().ok_or_else(err)?;
        if host.is_empty() || host.starts_with('.') || !host.contains('.') || host.contains(':') {
            return Err(err());
        }
    } else {
        let tail = url.strip_prefix("http://127.0.0.1:").ok_or_else(err)?;
        let (port, path) = tail.split_once('/').ok_or_else(err)?;
        if port.parse::<u16>().ok().filter(|n| *n > 0).is_none() || path.is_empty() {
            return Err(err());
        }
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Request {
    pub url: String,
    pub content_type: String,
    pub body: Vec<u8>,
    pub key_env: Option<String>,
}
pub fn prepare(c: &Config, backend: &str, modality: &str, payload: &Value) -> Result<Request> {
    let e = c.endpoint(backend)?;
    let model = if modality == "text" {
        text(get(get(&c.value, "text_models")?, backend)?, text(&c.value, "size")?.trim_end_matches(['b', 'B']))?
    } else {
        text(e, "model")?
    };
    if model.is_empty() {
        return Err(err());
    }
    let mut content_type = "application/json".to_string();
    let body = match modality {
        "text" => {
            let messages = get(payload, "messages")?.clone();
            let limit = num(payload, "max_tokens", 512)?;
            let mut request = if backend == "local_ollama" {
                obj! {"model"=>model,"messages"=>messages,"stream"=>false,"keep_alive"=>0,"options"=>obj!{"num_predict"=>limit,"num_thread"=>num(&c.value,"cpu_threads",4)?,"num_ctx"=>if gamectx::deployment::ModelSize::parse(text(&c.value,"size")?)?.workload_hints(gamectx::deployment::Workload::Dialogue).exchanges<=1 {2048}else{8192},"temperature"=>0}}
            } else {
                obj! {"model"=>model,"messages"=>messages,"stream"=>false,"max_completion_tokens"=>limit}
            };
            if let Some(schema) = payload.get("speech_schema") {
                if backend == "local_ollama" {
                    set(&mut request, "format", schema.clone());
                } else {
                    set(
                        &mut request,
                        "response_format",
                        obj! {"type"=>"json_schema","json_schema"=>obj!{"name"=>"grounded_speech","strict"=>true,"schema"=>schema.clone()}},
                    );
                }
            }
            request.to_json().into_bytes()
        }
        "image" => {
            let width = num(payload, "width", 4096)?;
            let height = num(payload, "height", 4096)?;
            let positive = text(payload, "prompt")?;
            let negative = text(payload, "negative_prompt")?;
            if backend == "local_sd_cpp" {
                obj!{"prompt"=>positive,"negative_prompt"=>negative,"width"=>width,"height"=>height,"seed"=>num(payload,"seed",u32::MAX as u64)?,"steps"=>12,"batch_size"=>1}.to_json().into_bytes()
            } else {
                let mut request = obj! {"model"=>model,"prompt"=>format!("{positive}. Avoid: {negative}"),"n"=>1,"size"=>format!("{width}x{height}")};
                if model.starts_with("dall-e-") {
                    set(&mut request, "response_format", "b64_json".into());
                }
                request.to_json().into_bytes()
            }
        }
        "tts" => {
            let voice = get(payload, "voice")?;
            validate_voice(voice)?;
            let input = text(payload, "text")?;
            if input.is_empty() || input.chars().count() > 240 {
                return Err(err());
            }
            obj!{"model"=>model,"input"=>input,"voice"=>get(voice,"voice_id")?.clone(),"speed"=>get(voice,"rate")?.clone(),"response_format"=>"wav"}.to_json().into_bytes()
        }
        "stt" => {
            let mime = text(payload, "mime")?;
            if mime != "audio/wav" {
                return Err(err());
            }
            let extension = "wav";
            let audio = decode64(text(payload, "audio")?)?;
            validate_capture(&audio)?;
            let boundary = "gamectx_studio_audio_boundary";
            if audio.windows(boundary.len()).any(|w| w == boundary.as_bytes()) {
                return Err(err());
            }
            content_type = format!("multipart/form-data; boundary={boundary}");
            let mut bytes=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{model}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"capture.{extension}\"\r\nContent-Type: {mime}\r\n\r\n").into_bytes();
            bytes.extend(audio);
            bytes.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
            bytes
        }
        _ => return Err(err()),
    };
    if body.len() > MAX_BYTES {
        return Err(err());
    }
    Ok(Request {
        url: text(e, "url")?.into(),
        content_type,
        body,
        key_env: get(e, "key_env")?.as_str().map(str::to_owned),
    })
}
pub trait Transport: Send + 'static {
    fn send(&mut self, request: &Request) -> Result<Vec<u8>>;
}
pub struct Network {
    pub dir: std::path::PathBuf,
}
impl Transport for Network {
    fn send(&mut self, r: &Request) -> Result<Vec<u8>> {
        if r.url.starts_with("http://127.0.0.1:") {
            local_http(r)
        } else {
            https(r, &self.dir)
        }
    }
}
fn local_http(r: &Request) -> Result<Vec<u8>> {
    endpoint(&r.url, false)?;
    let tail = &r.url["http://127.0.0.1:".len()..];
    let (port, path) = tail.split_once('/').ok_or_else(err)?;
    let mut stream =
        TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().map_err(|_| err())?, Duration::from_secs(3))
            .map_err(|_| err())?;
    stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(|_| err())?;
    write!(stream,"POST /{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",r.content_type,r.body.len()).map_err(|_|err())?;
    stream.write_all(&r.body).map_err(|_| err())?;
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let remaining = deadline.checked_duration_since(Instant::now()).ok_or_else(err)?;
        stream.set_read_timeout(Some(remaining)).map_err(|_| err())?;
        let n = stream.read(&mut chunk).map_err(|_| err())?;
        if n == 0 {
            break;
        }
        if bytes.len() + n > MAX_BYTES + 65536 {
            return Err(err());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(err)?;
    if split > 16384 {
        return Err(err());
    }
    let headers = std::str::from_utf8(&bytes[..split]).map_err(|_| err())?;
    let code = headers
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(err)?;
    if code != 200 {
        return Err(err());
    }
    let raw = &bytes[split + 4..];
    let result = if headers.lines().any(|l| l.eq_ignore_ascii_case("transfer-encoding: chunked")) {
        unchunk(raw)?
    } else {
        raw.to_vec()
    };
    if result.len() > MAX_BYTES {
        return Err(err());
    }
    Ok(result)
}
fn unchunk(mut raw: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for _ in 0..65536 {
        let at = raw.windows(2).position(|w| w == b"\r\n").ok_or_else(err)?;
        if at > 64 {
            return Err(err());
        }
        let n =
            usize::from_str_radix(std::str::from_utf8(&raw[..at]).map_err(|_| err())?.split(';').next().unwrap(), 16)
                .map_err(|_| err())?;
        raw = &raw[at + 2..];
        if n == 0 {
            return Ok(out);
        }
        if n > MAX_BYTES - out.len() || raw.len() < n + 2 || &raw[n..n + 2] != b"\r\n" {
            return Err(err());
        }
        out.extend_from_slice(&raw[..n]);
        raw = &raw[n + 2..];
    }
    Err(err())
}
fn https(r: &Request, dir: &Path) -> Result<Vec<u8>> {
    endpoint(&r.url, true)?;
    let key = std::env::var(r.key_env.as_ref().ok_or_else(err)?).map_err(|_| err())?;
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\') {
        return Err(err());
    }
    let input = dir.join("provider-request.bin");
    let output = dir.join("provider-response.bin");
    // One worker owns these files. Secrets travel only on curl's stdin, never argv/logs.
    let run = (|| {
        std::fs::write(&input, &r.body).map_err(|_| err())?;
        let body_path = input.canonicalize().map_err(|_| err())?;
        let body_path = body_path.to_str().ok_or_else(err)?.replace('\\', "\\\\").replace('"', "\\\"");
        let config=format!("url = \"{}\"\nheader = \"Authorization: Bearer {}\"\nheader = \"Content-Type: {}\"\ndata-binary = \"@{}\"\n",r.url,key,r.content_type,body_path);
        let mut child = std::process::Command::new("curl")
            .args([
                "--disable",
                "--silent",
                "--fail",
                "--max-time",
                "90",
                "--connect-timeout",
                "3",
                "--max-filesize",
                "8388608",
                "--proto",
                "=https",
                "--max-redirs",
                "0",
                "--config",
                "-",
                "--output",
            ])
            .arg(&output)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|_| err())?;
        let written = child
            .stdin
            .take()
            .ok_or_else(err)
            .and_then(|mut stdin| stdin.write_all(config.as_bytes()).map_err(|_| err()));
        if written.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err());
        }
        if !child.wait().map_err(|_| err())?.success() {
            return Err(err());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&output)
            .map_err(|_| err())?
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| err())?;
        if bytes.len() > MAX_BYTES {
            return Err(err());
        }
        Ok(bytes)
    })();
    let _ = std::fs::remove_file(input);
    let _ = std::fs::remove_file(output);
    run
}
pub fn encode64(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        for (i, shift) in [18, 12, 6, 0].iter().enumerate() {
            out.push(if i > c.len() { '=' } else { A[((n >> shift) & 63) as usize] as char });
        }
    }
    out
}
pub fn decode64(s: &str) -> Result<Vec<u8>> {
    if s.len() > MAX_BYTES || s.len() % 4 != 0 {
        return Err(err());
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    for (index, c) in s.as_bytes().chunks_exact(4).enumerate() {
        let mut n = 0u32;
        let mut pad = 0;
        for &b in c {
            n <<= 6;
            let v = match b {
                b'A'..=b'Z' => b - b'A',
                b'a'..=b'z' => b - b'a' + 26,
                b'0'..=b'9' => b - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => {
                    pad += 1;
                    0
                }
                _ => return Err(err()),
            };
            if pad > 0 && b != b'=' {
                return Err(err());
            }
            n |= v as u32;
        }
        if pad > 2 || pad > 0 && index + 1 != s.len() / 4 || pad == 1 && n & 255 != 0 || pad == 2 && n & 65535 != 0 {
            return Err(err());
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad == 0 {
            out.push(n as u8);
        }
    }
    Ok(out)
}
pub fn image(bytes: &[u8], width: u32, height: u32) -> Result<()> {
    if bytes.len() < 24
        || bytes.len() > 4 * 1024 * 1024
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
        || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) != width
        || u32::from_be_bytes(bytes[20..24].try_into().unwrap()) != height
    {
        return Err(err());
    }
    Ok(())
}

/// Canonical mono 16 kHz PCM16 WAV, at most ten seconds. The actual frame count,
/// not an untrusted browser duration field, bounds transcription billing input.
pub fn validate_capture(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 46
        || bytes.len() > 320044
        || bytes.len() % 2 != 0
        || &bytes[..4] != b"RIFF"
        || &bytes[8..16] != b"WAVEfmt "
        || &bytes[36..40] != b"data"
    {
        return Err(err());
    }
    let u32_at = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    if u32_at(4) as usize != bytes.len() - 8
        || u32_at(16) != 16
        || bytes[20..24] != [1, 0, 1, 0]
        || u32_at(24) != 16000
        || u32_at(28) != 32000
        || bytes[32..36] != [2, 0, 16, 0]
        || u32_at(40) as usize != bytes.len() - 44
    {
        return Err(err());
    }
    Ok(())
}
