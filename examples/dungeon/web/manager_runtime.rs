//! Local-only residency and owned-process supervisor, executed on the one worker.
use super::{
    providers::{self, Method, Request},
    *,
};
use gamectx::{obj, studio::ModelPolicy};
use std::{
    collections::BTreeMap,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
fn issue(s: &str) -> Error {
    Error::new(format!("manager: {s}"))
}
fn root(url: &str) -> Result<String> {
    let tail = url.strip_prefix("http://127.0.0.1:").ok_or_else(err)?;
    let port = tail.split('/').next().ok_or_else(err)?;
    port.parse::<u16>().ok().filter(|p| *p > 0).ok_or_else(err)?;
    Ok(format!("http://127.0.0.1:{port}"))
}
fn req(base: &str, path: &str, body: Option<Value>) -> Request {
    Request {
        method: if body.is_some() { Method::Post } else { Method::Get },
        url: format!("{base}{path}"),
        content_type: "application/json".into(),
        body: body.map_or_else(Vec::new, |v| v.to_json().into_bytes()),
        key_env: None,
        management: None,
    }
}
fn json(r: Request) -> Result<Value> {
    let b = providers::local_http(&r)?;
    gamectx::json::parse(std::str::from_utf8(&b).map_err(|_| err())?, 1_048_576)
}
fn channel(id: &str) -> &str {
    if id.contains("text") {
        "text"
    } else if id.contains("image") {
        "image"
    } else {
        id
    }
}
fn available_memory() -> Option<u64> {
    std::fs::read_to_string("/proc/meminfo").ok()?.lines().find_map(|l| {
        l.strip_prefix("MemAvailable:")
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
            .map(|v| v / 1024)
    })
}
struct Lease {
    identity: String,
    profile: String,
    model: String,
    channel: String,
    base: String,
    last: Instant,
    ttl: u64,
}
pub struct Runtime {
    children: BTreeMap<String, Child>,
    lease: Option<Lease>,
    roots: Value,
    config: Option<Value>,
    launchers: Value,
    last_catalog: Value,
    last_error: String,
    next_idle: Instant,
}
impl Runtime {
    pub fn new(dir: &Path) -> Result<Self> {
        let file = dir.join("launchers.json");
        let v = if file.exists() { parse(&read_file(&file, 32768)?)? } else { parse(include_str!("launchers.json"))? };
        fields(&v, &["version", "services"])?;
        num(&v, "version", 1)?;
        let services = get(&v, "services")?;
        fields(services, &["text", "image", "tts", "stt"])?;
        for (_, s) in services.as_object().ok_or_else(err)? {
            let mut keys = vec!["argv", "env"];
            if s.get("models").is_some() {
                keys.push("models");
            }
            fields(s, &keys)?;
            if let Some(models) = s.get("models") {
                let rows = models.as_object().ok_or_else(err)?;
                if rows.len() > 16 {
                    return Err(err());
                }
                for (k, args) in rows {
                    if k.is_empty()
                        || k.len() > 128
                        || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:./".contains(&b))
                    {
                        return Err(err());
                    }
                    let args = args.as_array().ok_or_else(err)?;
                    if args.is_empty()
                        || args.len() > 32
                        || args.iter().any(|a| {
                            a.as_str().is_none_or(|s| s.is_empty() || s.len() > 2048 || s.chars().any(char::is_control))
                        })
                    {
                        return Err(err());
                    }
                }
            }
            let args = get(s, "argv")?.as_array().ok_or_else(err)?;
            if args.len() > 32 {
                return Err(err());
            }
            for a in args {
                let s = a.as_str().ok_or_else(err)?;
                if s.len() > 2048 || s.chars().any(char::is_control) {
                    return Err(err());
                }
            }
            let env = get(s, "env")?.as_object().ok_or_else(err)?;
            if env.len() > 16 {
                return Err(err());
            }
            for (k, v) in env {
                if k.is_empty()
                    || k.len() > 64
                    || !k.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                    || !v.as_str().is_some_and(|s| s.len() <= 2048 && !s.chars().any(char::is_control))
                {
                    return Err(err());
                }
            }
        }
        Ok(Self {
            children: BTreeMap::new(),
            lease: None,
            roots: obj! {},
            config: None,
            launchers: v,
            last_catalog: obj! {},
            last_error: String::new(),
            next_idle: Instant::now(),
        })
    }
    fn base(&self, ch: &str) -> Result<String> {
        Ok(text(&self.roots, ch)?.into())
    }
    fn reachable(&self, ch: &str) -> bool {
        self.base(ch).ok().is_some_and(|base| {
            providers::local_http(&Request {
                method: Method::ConnectOnly,
                url: format!("{base}/health"),
                content_type: "application/json".into(),
                body: Vec::new(),
                key_env: None,
                management: None,
            })
            .is_ok()
        })
    }
    fn spawn(&mut self, ch: &str, model: &str, dir: &Path) -> Result<()> {
        if self.reachable(ch) {
            return Ok(());
        }
        if self.children.get_mut(ch).is_some_and(|p| p.try_wait().ok().flatten().is_none()) {
            return Err(Error::new("provider_starting"));
        }
        let launcher = get(get(&self.launchers, "services")?, ch)?;
        let raw = launcher
            .get("models")
            .and_then(|v| v.get(model))
            .unwrap_or(get(launcher, "argv")?)
            .as_array()
            .ok_or_else(err)?;
        if raw.is_empty() {
            return Err(issue("No launcher is configured for this service. Edit the host launchers file, or use a running service/browser option."));
        }
        let mut args = Vec::new();
        for v in raw {
            let s = v.as_str().ok_or_else(err)?;
            args.push(if s.starts_with("${GAMECTX_") && s.ends_with('}') {
                std::env::var(&s[2..s.len() - 1])
                    .ok()
                    .filter(|s| !s.is_empty() && s.len() <= 2048 && !s.contains('\0'))
                    .ok_or_else(|| issue("A required GAMECTX launcher environment variable is unset."))?
            } else {
                s.into()
            });
        }
        let output = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("managed-{ch}.log")))
            .map_err(|_| err())?;
        let spec = obj! {"argv"=>args,"env"=>get(launcher,"env")?.clone()};
        let mut command = Command::new(std::env::current_exe().map_err(|_| err())?);
        command
            .arg("--managed-service")
            .stdin(Stdio::piped())
            .stdout(output.try_clone().map_err(|_| err())?)
            .stderr(output);
        let mut child = command.spawn().map_err(|_| issue("Service supervisor could not start."))?;
        use std::io::Write;
        let pipe = child.stdin.as_mut().ok_or_else(err)?;
        pipe.write_all(spec.to_json().as_bytes()).map_err(|_| err())?;
        pipe.write_all(b"\n").map_err(|_| err())?;
        self.children.insert(ch.into(), child);
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if self.reachable(ch) {
                return Ok(());
            }
            if self.children.get_mut(ch).and_then(|c| c.try_wait().ok()).flatten().is_some() {
                return Err(issue("Managed service exited before becoming ready. Check its host log."));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(Error::new("provider_starting"))
    }
    fn stop(&mut self, ch: &str) -> Result<()> {
        let mut child = self.children.remove(ch).ok_or_else(|| {
            issue("This server was started elsewhere. Only manager-owned servers can be stopped here.")
        })?;
        drop(child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(3);
        while child.try_wait().map_err(|_| err())?.is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if child.try_wait().map_err(|_| err())?.is_none() {
            child.kill().map_err(|_| err())?;
        }
        child.wait().map_err(|_| err())?;
        Ok(())
    }
    fn release(&mut self) -> Result<()> {
        if let Some(l) = &self.lease {
            match l.channel.as_str() {
                "text" => {
                    let ack = json(req(
                        &l.base,
                        "/api/generate",
                        Some(obj! {"model"=>l.model.clone(),"keep_alive"=>0,"stream"=>false}),
                    ))?;
                    if ack.get("done") != Some(&Value::Bool(true)) {
                        return Err(issue("Text server did not confirm unloading."));
                    }
                }
                "image" => {
                    let ack = json(req(&l.base, "/sdcpp/v1/models/unload", Some(obj! {})))?;
                    if ack.get("state").and_then(Value::as_str) != Some("empty") {
                        return Err(issue("Image server did not confirm unloading."));
                    }
                }
                ch => {
                    let ch = ch.to_string();
                    if self.children.contains_key(&ch) {
                        self.stop(&ch)?;
                    }
                }
            }
        }
        if let Some(l) = &self.lease {
            if l.channel == "image" {
                set(&mut self.last_catalog, "observed_image", "empty".into());
            }
            if l.channel == "text" {
                let rows = self
                    .last_catalog
                    .get("observed_text")
                    .and_then(Value::as_array)
                    .unwrap_or(&[])
                    .iter()
                    .filter(|v| v.as_str() != Some(l.model.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                set(&mut self.last_catalog, "observed_text", rows.into());
            }
        }
        self.lease = None;
        self.last_error.clear();
        Ok(())
    }
    fn start_configured(&self, ch: &str, model: &str) -> bool {
        let service = self.launchers.get("services").and_then(|v| v.get(ch));
        service
            .and_then(|v| v.get("models"))
            .and_then(|v| v.get(model))
            .or_else(|| service.and_then(|v| v.get("argv")))
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty())
    }
    fn scan(&mut self, policy: &ModelPolicy) -> Value {
        let tags = self.base("text").ok().and_then(|b| json(req(&b, "/api/tags", None)).ok());
        let names =
            tags.as_ref().and_then(|v| v.get("models")).and_then(Value::as_array).map_or_else(Vec::new, |rows| {
                rows.iter()
                    .take(64)
                    .filter_map(|r| r.get("name").and_then(Value::as_str))
                    .filter(|s| s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:./".contains(&b)))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            });
        let files = self.base("image").ok().and_then(|b| json(req(&b, "/sdcpp/v1/model-files", None)).ok());
        let ids = files.as_ref().and_then(|v| v.get("files")).and_then(Value::as_array).map_or_else(Vec::new, |rows| {
            rows.iter()
                .take(256)
                .filter_map(|r| r.get("id").and_then(Value::as_str))
                .filter(|s| s.len() <= 256 && !s.starts_with('/') && !s.chars().any(char::is_control))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        });
        let observed_text = self
            .base("text")
            .ok()
            .and_then(|b| json(req(&b, "/api/ps", None)).ok())
            .and_then(|v| v.get("models").cloned())
            .and_then(|v| {
                v.as_array().map(|rows| {
                    rows.iter()
                        .take(64)
                        .filter_map(|r| r.get("name").and_then(Value::as_str))
                        .filter(|s| s.len() <= 128 && !s.chars().any(char::is_control))
                        .map(Value::str)
                        .collect::<Vec<_>>()
                })
            })
            .map_or(Value::Null, Value::from);
        let observed_image = self
            .base("image")
            .ok()
            .and_then(|b| json(req(&b, "/sdcpp/v1/models/status", None)).ok())
            .and_then(|v| v.get("state").cloned())
            .filter(|v| v.as_str().is_some_and(|s| ["ready", "empty", "loading", "error"].contains(&s)))
            .unwrap_or(Value::str("unknown"));
        let mut available = Vec::new();
        for id in ["text", "dramatic_text", "image", "dramatic_image", "tts", "stt"] {
            let p = policy.profile(id).expect("validated");
            let backend = text(p, "backend").unwrap();
            if backend.starts_with("online_") {
                continue;
            }
            let yes = if id.contains("text") {
                names.iter().any(|s| Some(s.as_str()) == p.get("model").and_then(Value::as_str))
            } else if id.contains("image") {
                get(p, "selection").ok().and_then(Value::as_object).is_some_and(|s| {
                    s.iter().all(|(_, v)| v.as_str().is_some_and(|v| v.is_empty() || ids.iter().any(|i| i == v)))
                })
            } else {
                self.reachable(id) || self.start_configured(id, text(p, "model").unwrap())
            };
            if yes {
                available.push(id);
            }
        }
        let services = ["text", "image", "tts", "stt"].into_iter().map(|ch| {
            let owned = self.children.get_mut(ch).is_some_and(|c| c.try_wait().ok().flatten().is_none());
            obj! {"channel"=>ch,"ownership"=>if owned {"managed"} else if self.reachable(ch) {"external"} else {"stopped"},"start_configured"=>self.start_configured(ch, text(policy.profile(ch).unwrap(), "model").unwrap())}
        }).collect::<Vec<_>>();
        let report = obj! {"observed_text"=>observed_text,"observed_image"=>observed_image,"available"=>available,"text_models"=>names,"image_files"=>ids,"services"=>services,"free_mib"=>available_memory().map_or(Value::Null,Value::from),"loaded"=>self.lease.as_ref().map_or(Value::Null,|l|obj!{"profile"=>l.profile.clone(),"channel"=>l.channel.clone(),"model"=>l.model.clone()}),"error"=>self.last_error.clone()};
        self.last_catalog = report.clone();
        report
    }
    fn ensure(&mut self, id: &str, policy: &ModelPolicy, prewarm: bool, dir: &Path) -> Result<()> {
        let p = policy.profile(id)?;
        let ch = channel(id);
        let base = self.base(ch)?;
        let model = text(p, "model")?.to_string();
        if self.lease.as_ref().is_some_and(|l| {
            l.profile == id
                && l.model == model
                && l.identity == super::manager_view::loading_identity(p, &base).unwrap_or_default()
        }) && self.last_error.is_empty()
        {
            self.lease.as_mut().unwrap().last = Instant::now();
            return Ok(());
        }
        if self.lease.is_none() && ch != "image" {
            if let Ok(image_base) = self.base("image") {
                let image = policy.profile("image")?;
                let matches = json(req(&image_base, "/sdapi/v1/options", None))
                    .ok()
                    .is_some_and(|v| v.get("sd_model_checkpoint") == image.get("model"));
                let ready = json(req(&image_base, "/sdcpp/v1/models/status", None))
                    .ok()
                    .is_some_and(|v| v.get("state").and_then(Value::as_str) == Some("ready"));
                if matches && ready {
                    self.lease = Some(Lease {
                        identity: super::manager_view::loading_identity(image, &image_base)?,
                        profile: "image".into(),
                        model: text(image, "model")?.into(),
                        channel: "image".into(),
                        base: image_base,
                        last: Instant::now(),
                        ttl: 60,
                    });
                }
            }
        }
        self.release()?;
        let cfg = policy.to_value();
        let required = num(p, "ram_mib", 65536)?;
        if required > num(&cfg, "ram_limit_mib", 65536)?
            || available_memory().is_some_and(|free| free < required + num(&cfg, "reserve_mib", 8192).unwrap_or(8192))
        {
            return Err(issue("Insufficient free memory or profile exceeds the RAM allowance. Existing approved assets/offline replies remain usable."));
        }
        self.spawn(ch, &model, dir)?;
        if ch == "text" {
            let v = json(req(&base, "/api/tags", None))?;
            if !v
                .get("models")
                .and_then(Value::as_array)
                .is_some_and(|v| v.iter().any(|r| r.get("name").and_then(Value::as_str) == Some(model.as_str())))
            {
                return Err(issue("Selected text model is not installed. Choose an installed model; the manager never downloads implicitly."));
            }
            if prewarm {
                let ack = json(req(
                    &base,
                    "/api/generate",
                    Some(
                        obj! {"model"=>model.clone(),"stream"=>false,"keep_alive"=>format!("{}s",num(&cfg,"idle_seconds",3600)?),"options"=>obj!{"num_predict"=>0,"num_thread"=>4,"num_gpu"=>0}},
                    ),
                ))?;
                if ack.get("done") != Some(&Value::Bool(true)) {
                    return Err(issue("Text server did not confirm model readiness."));
                }
            }
        } else if ch == "image" {
            let state = json(req(&base, "/sdcpp/v1/models/status", None))?;
            if state.get("state").and_then(Value::as_str) == Some("loading") {
                return Err(issue("Image server is busy loading; no second model was started."));
            }
            let same = state.get("state").and_then(Value::as_str) == Some("ready")
                && state.get("recipe") == p.get("recipe")
                && state.get("models").is_some_and(|m| m.same(get(p, "selection").unwrap()));
            let command_line = state.get("origin").and_then(Value::as_str) == Some("command_line")
                && json(req(&base, "/sdapi/v1/options", None))
                    .ok()
                    .is_some_and(|v| v.get("sd_model_checkpoint").and_then(Value::as_str) == Some(model.as_str()));
            if !same && !command_line {
                let ack = json(req(
                    &base,
                    "/sdcpp/v1/models/load",
                    Some(obj! {"recipe"=>get(p,"recipe")?.clone(),"models"=>get(p,"selection")?.clone()}),
                ))?;
                if ack.get("state").and_then(Value::as_str) != Some("ready") {
                    return Err(issue("Image server did not confirm a ready model."));
                }
            }
        }
        self.lease = Some(Lease {
            identity: super::manager_view::loading_identity(p, &base)?,
            profile: id.into(),
            model,
            channel: ch.into(),
            base,
            last: Instant::now(),
            ttl: num(&cfg, "idle_seconds", 3600)?,
        });
        self.last_error.clear();
        Ok(())
    }
    pub fn execute(&mut self, r: &Request, dir: &Path) -> Result<Vec<u8>> {
        let control = r.management.as_ref().ok_or_else(err)?;
        let policy = ModelPolicy::from_value(get(control, "config")?)?;
        let roots = get(control, "roots")?;
        fields(roots, &["text", "image", "tts", "stt"])?;
        for (_, v) in roots.as_object().ok_or_else(err)? {
            root(v.as_str().ok_or_else(err)?)?;
        }
        self.roots = roots.clone();
        self.config = Some(policy.to_value());
        let action = text(control, "action")?;
        let id = text(control, "profile")?;
        if ["start", "prewarm", "demand"].contains(&action) {
            let backend = text(policy.profile(id)?, "backend")?;
            if backend != "inherit" && !backend.starts_with("local_") {
                return Err(issue("Online/browser profiles have no local weights to load."));
            }
        }
        let outcome = (|| -> Result<Vec<u8>> {
            match action {
                "refresh" => {
                    self.launchers = Runtime::new(dir)?.launchers.clone();
                    if let Ok(base) = self.base("image") {
                        let _ = json(req(&base, "/sdcpp/v1/models/refresh", Some(obj! {})));
                    }
                    Ok(self.scan(&policy).to_json().into_bytes())
                }
                "start" => {
                    let ch = channel(id);
                    if !self.reachable(ch) {
                        self.release()?;
                        let p = policy.profile(id)?;
                        let c = policy.to_value();
                        let required = num(p, "ram_mib", 65536)?;
                        if required > num(&c, "ram_limit_mib", 65536)?
                            || available_memory()
                                .is_some_and(|free| free < required + num(&c, "reserve_mib", 8192).unwrap_or(8192))
                        {
                            return Err(issue("Not enough memory allowance to start this service."));
                        }
                    }
                    self.spawn(ch, text(policy.profile(id)?, "model")?, dir)?;
                    if ["tts", "stt"].contains(&ch) && self.children.contains_key(ch) {
                        self.lease = Some(Lease {
                            identity: super::manager_view::loading_identity(policy.profile(id)?, &self.base(ch)?)?,
                            profile: id.into(),
                            model: text(policy.profile(id)?, "model")?.into(),
                            channel: ch.into(),
                            base: self.base(ch)?,
                            last: Instant::now(),
                            ttl: num(&policy.to_value(), "idle_seconds", 3600)?,
                        });
                    }
                    Ok(self.scan(&policy).to_json().into_bytes())
                }
                "stop" => {
                    self.stop(channel(id))?;
                    if self.lease.as_ref().is_some_and(|l| l.channel == channel(id)) {
                        self.lease = None;
                    }
                    Ok(self.scan(&policy).to_json().into_bytes())
                }
                "release" => {
                    if self.lease.as_ref().is_some_and(|l| l.channel == channel(id)) {
                        self.release()?;
                    }
                    Ok(self.scan(&policy).to_json().into_bytes())
                }
                "unload" => {
                    if !self.lease.as_ref().is_some_and(|l| l.channel == channel(id)) {
                        let p = policy.profile(id)?;
                        let ch = channel(id);
                        if ch == "text" {
                            json(req(
                                &self.base(ch)?,
                                "/api/generate",
                                Some(obj! {"model"=>get(p,"model")?.clone(),"keep_alive"=>0,"stream"=>false}),
                            ))?;
                        } else if ch == "image" {
                            json(req(&self.base(ch)?, "/sdcpp/v1/models/unload", Some(obj! {})))?;
                        } else {
                            self.stop(ch)?;
                        }
                    } else {
                        self.release()?;
                    }
                    Ok(self.scan(&policy).to_json().into_bytes())
                }
                "prewarm" => {
                    self.ensure(id, &policy, true, dir)?;
                    Ok(self.scan(&policy).to_json().into_bytes())
                }
                "demand" => {
                    self.ensure(id, &policy, false, dir)?;
                    let mut request = r.clone();
                    request.management = None;
                    if channel(id) == "text" {
                        let mut v = parse(std::str::from_utf8(&request.body).map_err(|_| err())?)?;
                        set(
                            &mut v,
                            "keep_alive",
                            format!("{}s", num(&policy.to_value(), "idle_seconds", 3600)?).into(),
                        );
                        request.body = v.to_json().into_bytes();
                    }
                    let result = providers::local_http(&request);
                    if result.is_ok()
                        && text(&policy.to_value(), "residency")? == "on_demand"
                        && ["text", "image", "stt"].contains(&channel(id))
                    {
                        self.release()?;
                    }
                    result
                }
                _ => Err(err()),
            }
        })();
        if let Err(e) = &outcome {
            self.last_error = if e.to_string().starts_with("manager:") {
                e.to_string()
            } else {
                "Local lifecycle request failed; confirm the service is idle before retrying.".into()
            };
        }
        outcome
    }
    pub fn status(&self) -> Option<Value> {
        self.config.as_ref()?;
        let mut v = self.last_catalog.clone();
        set(
            &mut v,
            "loaded",
            self.lease.as_ref().map_or(
                Value::Null,
                |l| obj! {"profile"=>l.profile.clone(),"channel"=>l.channel.clone(),"model"=>l.model.clone()},
            ),
        );
        if let Some(rows) = v.get("services").and_then(Value::as_array) {
            let rows = rows
                .iter()
                .map(|row| {
                    let mut row = row.clone();
                    if row.get("channel").and_then(Value::as_str).is_some_and(|ch| self.children.contains_key(ch)) {
                        set(&mut row, "ownership", "managed".into());
                    } else if row.get("ownership").and_then(Value::as_str) == Some("managed") {
                        set(&mut row, "ownership", "stopped".into());
                    }
                    row
                })
                .collect::<Vec<_>>();
            set(&mut v, "services", rows.into());
        }
        set(&mut v, "free_mib", available_memory().map_or(Value::Null, Value::from));
        set(&mut v, "error", self.last_error.clone().into());
        Some(v)
    }
    pub fn idle(&mut self) -> Result<Option<Value>> {
        if Instant::now() < self.next_idle {
            return Ok(None);
        }
        self.next_idle = Instant::now() + Duration::from_secs(5);
        if self.lease.as_ref().is_some_and(|l| l.last.elapsed() >= Duration::from_secs(l.ttl)) {
            if let Err(e) = self.release() {
                self.last_error = e.to_string();
            }
            if let Some(c) = &self.config {
                let p = ModelPolicy::from_value(c)?;
                return Ok(Some(self.scan(&p)));
            }
        }
        Ok(None)
    }
}

/// The parent keeps stdin open. EOF on parent exit stops the launched server.
pub fn service_guard() -> Result<()> {
    use std::io::Read;
    let mut input = std::io::BufReader::new(std::io::stdin());
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        if input.read(&mut byte).map_err(|_| err())? == 0 {
            return Err(err());
        }
        bytes.push(byte[0]);
        if byte[0] == b'\n' || bytes.len() > 65536 {
            break;
        }
    }
    let line = String::from_utf8(bytes).map_err(|_| err())?;
    if line.len() > 65536 || !line.ends_with('\n') {
        return Err(err());
    }
    let v = gamectx::json::parse(line.trim_end(), 65536)?;
    fields(&v, &["argv", "env"])?;
    let args = get(&v, "argv")?.as_array().ok_or_else(err)?;
    if args.is_empty() || args.len() > 32 {
        return Err(err());
    }
    let args = args
        .iter()
        .map(|v| {
            v.as_str().filter(|s| !s.is_empty() && s.len() <= 2048 && !s.chars().any(char::is_control)).ok_or_else(err)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]).stdin(Stdio::null());
    let env = get(&v, "env")?.as_object().ok_or_else(err)?;
    if env.len() > 16 {
        return Err(err());
    }
    for (k, v) in env {
        let value = v.as_str().ok_or_else(err)?;
        if k.len() > 64
            || !k.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            || value.len() > 2048
            || value.contains('\0')
        {
            return Err(err());
        }
        cmd.env(k, value);
    }
    let mut child = cmd.spawn().map_err(|_| issue("Configured service executable could not start."))?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut byte = [0];
        while input.read(&mut byte).unwrap_or(0) != 0 {}
        let _ = tx.send(());
    });
    loop {
        if child.try_wait().map_err(|_| err())?.is_some() {
            return Ok(());
        }
        if rx.recv_timeout(Duration::from_millis(100)).is_ok() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        for child in self.children.values_mut() {
            drop(child.stdin.take());
        }
    }
}
