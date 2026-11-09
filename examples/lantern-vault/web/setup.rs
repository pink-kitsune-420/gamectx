//! Bounded, host-only connection diagnostics and visible worker receipts.
use super::*;
use gamectx::obj;
use std::time::Instant;

#[derive(Default)]
pub struct Monitor {
    models: Vec<String>,
    context: Option<Value>,
    capture_prompts: bool,
    checks: Vec<Value>,
    activity: Vec<(u64, Instant, Option<Instant>, Value)>,
}
impl Monitor {
    pub fn queued(&mut self, id: u64, channel: &str, backend: &str, model: &str, checking: bool) {
        if self.activity.len() == 24 {
            self.activity.remove(0);
        }
        self.activity.push((id, Instant::now(), None, obj! {"id"=>id,"channel"=>channel,"backend"=>backend,"model"=>model,"status"=>"queued","detail"=>if checking{"Connection check only; no generation."}else{"Waiting for the shared model worker."},"elapsed_ms"=>0,"queue_ms"=>0,"run_ms"=>0}));
    }
    pub fn update(&mut self, id: u64, status: &str, detail: &str) {
        if let Some((_, at, started, row)) = self.activity.iter_mut().find(|(n, _, _, _)| *n == id) {
            if status == "running" && started.is_none() {
                *started = Some(Instant::now());
            }
            let queue = started.map_or_else(|| at.elapsed(), |begin| begin.duration_since(*at));
            set(row, "queue_ms", (queue.as_millis().min(u64::MAX as u128) as u64).into());
            set(
                row,
                "run_ms",
                started.map_or(0, |begin| begin.elapsed().as_millis().min(u64::MAX as u128) as u64).into(),
            );
            set(row, "status", status.into());
            set(row, "detail", detail.into());
            set(row, "elapsed_ms", (at.elapsed().as_millis().min(u64::MAX as u128) as u64).into());
        }
    }
    pub fn capture(&mut self, value: bool) {
        self.capture_prompts = value;
        if !value {
            if let Some(c) = &mut self.context {
                set(c, "preview", Value::Null);
            }
        }
    }
    pub fn context(&mut self, id: u64, payload: &Value, request: &providers::Request, config: &providers::Config) {
        let rows = payload.get("messages").and_then(Value::as_array).unwrap_or(&[]);
        let messages=rows.iter().take(64).enumerate().map(|(i,m)|{
            let content=m.get("content").and_then(Value::as_str).unwrap_or("");
            let sections=gamectx::data::json::parse(content,65536).ok().and_then(|v|v.as_object().map(|rows|rows.iter().map(|(k,v)|obj!{"name"=>k.clone(),"json_bytes"=>v.json_len(),"entries"=>v.as_array().map_or(Value::Null,|r|r.len().into())}).collect::<Vec<_>>()));
            obj!{"index"=>i,"role"=>m.get("role").cloned().unwrap_or(Value::Null),"content_bytes"=>content.len(),"characters"=>content.chars().count(),"sections"=>sections.map_or(Value::Null,Value::from)}
        }).collect::<Vec<_>>();
        let preview = if self.capture_prompts {
            Value::str(payload.to_json().chars().take(32768).collect::<String>())
        } else {
            Value::Null
        };
        self.context = Some(
            obj! {"request_id"=>id,"model"=>selected_model(config),"tier"=>config.value.get("size").cloned().unwrap_or(Value::Null),"prepared_body_bytes"=>request.body.len(),"schema_bytes"=>payload.get("speech_schema").map_or(0,Value::json_len),"output_token_limit"=>payload.get("max_tokens").cloned().unwrap_or(Value::Null),"deliberation"=>payload.get("deliberation").cloned().unwrap_or(Value::Null),"messages"=>messages,"capture_prompts"=>self.capture_prompts,"preview"=>preview,"notes"=>"Byte counts describe the compiled request. Actual provider token counts appear after a completed call; no tokenizer/inference is run for profiling."},
        );
    }
    pub fn usage(&mut self, id: u64, response: &Value) {
        if let Some((_, _, _, row)) = self.activity.iter_mut().find(|(n, _, _, _)| *n == id) {
            let mut usage = obj! {};
            for key in [
                "prompt_eval_count",
                "prompt_eval_cached_count",
                "eval_count",
                "total_duration",
                "load_duration",
                "prompt_eval_duration",
                "eval_duration",
            ] {
                if let Ok(n) = num(response, key, u64::MAX) {
                    set(&mut usage, key, n.into());
                }
            }
            let thinking = response
                .get("message")
                .and_then(|m| m.get("thinking"))
                .and_then(Value::as_str)
                .map_or(0, |s| s.chars().count());
            set(&mut usage, "thinking_characters", thinking.into());
            if let Some(v) = response.get("usage") {
                if v.is_valid(2048) {
                    set(&mut usage, "online_usage", v.clone());
                }
            }
            set(row, "provider_usage", usage.clone());
            if let Some(context) = self.context.as_mut().filter(|c| c.get("request_id") == Some(&id.into())) {
                set(context, "provider_usage", usage);
            }
        }
    }
    pub fn stage(&mut self, id: u64, key: &str, ms: u64) {
        if let Some((_, _, _, row)) = self.activity.iter_mut().find(|(n, _, _, _)| *n == id) {
            set(row, key, ms.into());
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
            let v = gamectx::data::json::parse(std::str::from_utf8(&bytes).map_err(|_| err())?, 65536)?;
            if channel == "text" {
                let mut names = Vec::new();
                if let Some(models) = get(&v, "models").ok().and_then(Value::as_array) {
                    if models.len() > 64 {
                        return Err(err());
                    }
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
                } else if let Some(data) = get(&v, "data").ok().and_then(Value::as_array) {
                    if data.len() > 64 {
                        return Err(err());
                    }
                    for model in data {
                        let name = text(model, "id")?;
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
                } else {
                    return Err(err());
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
            let is_loopback = config.endpoint(backend).ok()
                .and_then(|e| text(e, "url").ok())
                .is_some_and(|u| u.starts_with("http://127.0.0.1:") || u.starts_with("http://localhost:"));
            if !backend.starts_with("local_") && !is_loopback {
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
            } else if is_loopback && channel == "text" && self.models.contains(&model) {
                status = "ready";
                detail = "Installed model found. Send a new message to request a reply.".into();
            }
            channels
                .push(obj! {"channel"=>channel,"backend"=>backend,"model"=>model,"status"=>status,"detail"=>detail});
        }
        let activity = self
            .activity
            .iter()
            .rev()
            .map(|(_, at, started, r)| {
                let mut r = r.clone();
                if matches!(r.get("status").and_then(Value::as_str), Some("queued" | "running")) {
                    set(&mut r, "elapsed_ms", (at.elapsed().as_millis().min(u64::MAX as u128) as u64).into());
                    set(
                        &mut r,
                        "queue_ms",
                        (started
                            .map_or_else(|| at.elapsed(), |b| b.duration_since(*at))
                            .as_millis()
                            .min(u64::MAX as u128) as u64)
                            .into(),
                    );
                    set(
                        &mut r,
                        "run_ms",
                        started.map_or(0, |b| b.elapsed().as_millis().min(u64::MAX as u128) as u64).into(),
                    );
                }
                r
            })
            .collect::<Vec<_>>();
        obj! {"channels"=>channels,"installed_models"=>self.models.clone(),"activity"=>activity,"context"=>self.context.clone().unwrap_or(Value::Null),"capture_prompts"=>self.capture_prompts}
    }
}
pub fn selected_model(c: &providers::Config) -> String {
    selected_model_for(c, c.value.get("text").and_then(Value::as_str).unwrap_or(""))
}
pub fn selected_model_for(c: &providers::Config, backend: &str) -> String {
    c.value
        .get("text_models")
        .and_then(|v| v.get(backend))
        .and_then(|v| v.get(c.value.get("size")?.as_str()?.trim_end_matches(['b', 'B'])))
        .and_then(Value::as_str)
        .unwrap_or("")
        .into()
}
pub fn probe(c: &providers::Config, channel: &str) -> Result<providers::Request> {
    let configured = text(&c.value, channel).unwrap_or("");
    let is_openai = configured == "online_openai_compatible"
        || c.endpoint(configured).ok().and_then(|e| text(e, "url").ok()).is_some_and(|u| u.ends_with("/chat/completions"));
    let backend = match channel {
        "text" => if is_openai { "online_openai_compatible" } else { "local_ollama" },
        "image" => "local_sd_cpp",
        "tts" => "local_tts_http",
        "stt" => "local_stt_http",
        _ => return Err(err()),
    };
    let url = text(c.endpoint(backend)?, "url")?;
    let (root, _) = url["http://".len()..].split_once('/').ok_or_else(err)?;
    Ok(providers::Request {
        management: None,
        text_token: None,
        method: if channel == "text" { providers::Method::Get } else { providers::Method::ConnectOnly },
        url: format!("http://{root}/{}", if is_openai { "v1/models" } else if channel == "text" { "api/tags" } else { "health" }),
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

#[cfg(test)]
mod profiler_tests {
    use super::*;
    #[test]
    fn queue_and_execution_are_separate_and_context_has_no_credentials() {
        let c = providers::Config::parse(include_str!("providers.json")).unwrap();
        let mut m = Monitor::default();
        m.queued(1, "text", "local_ollama", "fixture", false);
        m.activity[0].1 = Instant::now() - std::time::Duration::from_secs(5);
        m.update(1, "running", "start");
        m.activity[0].2 = Some(Instant::now() - std::time::Duration::from_millis(100));
        m.update(1, "completed", "done");
        let payload = obj! {"messages"=>vec![obj!{"role"=>"system","content"=>"Speak briefly."}],"max_tokens"=>64};
        let request = providers::prepare(&c, "local_ollama", "text", &payload).unwrap();
        m.context(1, &payload, &request, &c);
        m.usage(1, &obj! {"prompt_eval_count"=>123,"eval_count"=>12});
        let value = m.snapshot(&c, false);
        let row = &value.get("activity").unwrap().as_array().unwrap()[0];
        assert!(num(row, "queue_ms", 10000).unwrap() > 4000);
        assert!(num(row, "run_ms", 10000).unwrap() < 1000);
        assert_eq!(value.get("context").unwrap().get("preview"), Some(&Value::Null));
        assert_eq!(
            value.get("context").unwrap().get("provider_usage").unwrap().get("prompt_eval_count"),
            Some(&123.into())
        );
    }
}
