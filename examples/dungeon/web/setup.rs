//! Bounded, host-only connection diagnostics and visible worker receipts.
use super::*;
use gamectx::obj;
use std::time::Instant;

#[derive(Default)]
pub struct Monitor {
    models: Vec<String>,
    checks: Vec<Value>,
    activity: Vec<(u64, Instant, Value)>,
}
impl Monitor {
    pub fn queued(&mut self, id: u64, channel: &str, backend: &str, model: &str, checking: bool) {
        if self.activity.len() == 24 {
            self.activity.remove(0);
        }
        self.activity.push((id, Instant::now(), obj! {"id"=>id,"channel"=>channel,"backend"=>backend,"model"=>model,"status"=>"queued","detail"=>if checking{"Connection check only; no generation."}else{"Waiting for the shared model worker."},"elapsed_ms"=>0}));
    }
    pub fn update(&mut self, id: u64, status: &str, detail: &str) {
        if let Some((_, at, row)) = self.activity.iter_mut().find(|(n, _, _)| *n == id) {
            set(row, "status", status.into());
            set(row, "detail", detail.into());
            set(row, "elapsed_ms", (at.elapsed().as_millis().min(u64::MAX as u128) as u64).into());
        }
    }
    fn check(&mut self, channel: &str, value: Value) {
        self.checks.retain(|r| r.get("channel").and_then(Value::as_str) != Some(channel));
        self.checks.push(value);
    }
    pub fn checking(&mut self, channel: &str, backend: &str) {
        self.check(channel, obj! {"channel"=>channel,"backend"=>backend,"status"=>"checking","detail"=>"Checking local connection; no model generation."});
    }
    pub fn finish_check(&mut self, channel: &str, backend: &str, response: Result<Vec<u8>>) -> bool {
        let result = response.and_then(|bytes| {
            let v = gamectx::json::parse(std::str::from_utf8(&bytes).map_err(|_| err())?, 65536)?;
            if channel == "text" {
                let models = get(&v, "models")?.as_array().ok_or_else(err)?;
                if models.len() > 64 {
                    return Err(err());
                }
                let mut names = Vec::new();
                for model in models {
                    let name = text(model, "name")?;
                    if name.is_empty()
                        || name.len() > 128
                        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_:./".contains(&c))
                    {
                        return Err(err());
                    }
                    if !names.iter().any(|n| n == name) {
                        names.push(name.to_string());
                    }
                }
                self.models = names;
            } else if v.get("reachable") != Some(&Value::Bool(true)) {
                return Err(err());
            }
            Ok(())
        });
        let ok = result.is_ok();
        if channel == "text" && !ok {
            self.models.clear();
        }
        self.check(channel, obj! {"channel"=>channel,"backend"=>backend,"status"=>if ok{"reachable"}else{"unavailable"},"detail"=>if ok {if channel=="text"{"Ollama answered; installed models listed below."}else{"Port accepts connections; API compatibility and model readiness are not tested."}}else{"Local server did not pass this check. Start it or choose an offline/browser option."}});
        ok
    }
    pub fn problem(&mut self, channel: &str, backend: &str, status: &str, detail: &str) {
        self.check(channel, obj! {"channel"=>channel,"backend"=>backend,"status"=>status,"detail"=>detail});
    }
    pub fn succeeded(&mut self, channel: &str, backend: &str, model: &str) {
        if !backend.starts_with("local_") {
            return;
        }
        if channel == "text" && !self.models.iter().any(|m| m == model) {
            if self.models.len() == 64 {
                self.models.remove(0);
            }
            self.models.push(model.into());
        }
        self.check(channel,obj! {"channel"=>channel,"backend"=>backend,"status"=>"reachable","detail"=>"Most recent request completed and passed validation."});
    }
    pub fn cancel_check(&mut self, channel: &str) {
        self.checks.retain(|r| r.get("channel").and_then(Value::as_str) != Some(channel));
    }
    pub fn snapshot(&self, config: &providers::Config, paused: bool) -> Value {
        let mut channels = Vec::new();
        for channel in ["text", "image", "tts", "stt"] {
            let backend = text(&config.value, channel).unwrap();
            let model = if channel == "text" { selected_model(config) } else { String::new() };
            let check = self.checks.iter().find(|r| {
                r.get("channel").and_then(Value::as_str) == Some(channel)
                    && r.get("backend").and_then(Value::as_str) == Some(backend)
            });
            let mut status = "unchecked";
            let mut detail = "Not checked. Apply settings, then check local servers.".to_string();
            if !backend.starts_with("local_") {
                status = if backend.starts_with("online_") { "online" } else { "offline" };
                detail = if backend.starts_with("online_") {
                    "Online access still requires credentials and spending allowance."
                } else {
                    "Works without a local model server."
                }
                .into();
            } else if paused {
                status = "paused";
                detail =
                    "Local work is paused after an uncertain failure. Confirm the server is idle, then Resume local."
                        .into();
            } else if channel == "text" && model.is_empty() {
                status = "unconfigured";
                detail = "Choose a model for this tier, then Apply.".into();
            } else if let Some(c) = check {
                status = c.get("status").and_then(Value::as_str).unwrap();
                detail = c.get("detail").and_then(Value::as_str).unwrap().into();
                if channel == "text" && status == "reachable" {
                    if model.is_empty() {
                        status = "unconfigured";
                        detail = "Choose an installed model for this tier, then Apply.".into();
                    } else if self.models.contains(&model) {
                        status = "ready";
                        detail = "Installed model found. Send a new message to request a reply.".into();
                    } else {
                        status = "missing_model";
                        detail =
                            format!("{model} is not installed. Choose a model from the installed list, then Apply.");
                    }
                }
            }
            channels
                .push(obj! {"channel"=>channel,"backend"=>backend,"model"=>model,"status"=>status,"detail"=>detail});
        }
        let activity = self
            .activity
            .iter()
            .rev()
            .map(|(_, at, r)| {
                let mut r = r.clone();
                if matches!(r.get("status").and_then(Value::as_str), Some("queued" | "running")) {
                    set(&mut r, "elapsed_ms", (at.elapsed().as_millis().min(u64::MAX as u128) as u64).into());
                }
                r
            })
            .collect::<Vec<_>>();
        obj! {"channels"=>channels,"installed_models"=>self.models.clone(),"activity"=>activity}
    }
}
pub fn selected_model(c: &providers::Config) -> String {
    let backend = c.value.get("text").and_then(Value::as_str).unwrap_or("");
    c.value
        .get("text_models")
        .and_then(|v| v.get(backend))
        .and_then(|v| v.get(c.value.get("size")?.as_str()?.trim_end_matches(['b', 'B'])))
        .and_then(Value::as_str)
        .unwrap_or("")
        .into()
}
pub fn probe(c: &providers::Config, channel: &str) -> Result<providers::Request> {
    let backend = match channel {
        "text" => "local_ollama",
        "image" => "local_sd_cpp",
        "tts" => "local_tts_http",
        "stt" => "local_stt_http",
        _ => return Err(err()),
    };
    let url = text(c.endpoint(backend)?, "url")?;
    let (root, _) = url["http://".len()..].split_once('/').ok_or_else(err)?;
    Ok(providers::Request {
        management: None,
        method: if channel == "text" { providers::Method::Get } else { providers::Method::ConnectOnly },
        url: format!("http://{root}/{}", if channel == "text" { "api/tags" } else { "health" }),
        content_type: "application/json".into(),
        body: Vec::new(),
        key_env: None,
    })
}
pub fn definite_failure(e: &Error) -> bool {
    e.to_string().starts_with("manager:")
        || matches!(e.to_string().as_str(), "provider_connection" | "provider_http_404" | "provider_http_400")
}
pub fn failure_detail(e: &Error, backend: &str) -> &'static str {
    if e.to_string().starts_with("manager:") {
        return "Model/voice manager rejected or cancelled this request. Check Settings and the host log; fallback remains available.";
    }
    match e.to_string().as_str() {
        "provider_connection" => {
            "Server connection failed. Start the local service or choose an offline/browser provider."
        }
        "provider_http_404" if backend == "local_ollama" => {
            "Ollama returned 404. Check installed models and select the exact model name."
        }
        "provider_http_404" => "Provider returned 404. The configured API endpoint is unavailable.",
        "provider_http_400" => "Provider rejected the request (400). Check its model and API configuration.",
        _ => "Request did not finish reliably. Local dispatch is paused; confirm the server is idle before resuming.",
    }
}
