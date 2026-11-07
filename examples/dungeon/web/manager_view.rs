//! Authoritative GUI-side model configuration and scene policy; no network calls.
use super::*;
use gamectx::{obj, studio::ModelPolicy, visual::ArtBible};
pub struct State {
    pub policy: ModelPolicy,
    pub route: Value,
    pub report: Value,
    pub art: ArtBible,
}
impl State {
    pub fn new(dir: &std::path::Path) -> Result<Self> {
        let file = dir.join("models.json");
        let value = if file.exists() { parse(&read_file(&file, 32768)?)? } else { parse(include_str!("models.json"))? };
        let artfile = dir.join("dramatic-art.json");
        let art = if artfile.exists() {
            ArtBible::parse(&read_file(&artfile, 65536)?)?
        } else {
            ArtBible::parse(include_str!("art-dramatic.json"))?
        };
        Ok(Self {
            policy: ModelPolicy::from_value(&value)?,
            route: Value::Null,
            report: obj! {"available"=>Vec::<Value>::new(),"text_models"=>Vec::<Value>::new(),"image_files"=>Vec::<Value>::new(),"services"=>Vec::<Value>::new(),"loaded"=>Value::Null,"free_mib"=>Value::Null,"error"=>""},
            art,
        })
    }
    pub fn observe(&mut self, v: Value, base: &providers::Config, dir: &std::path::Path) -> Result<()> {
        if !v.is_valid(131072) {
            return Err(err());
        }
        let mut cfg = self.policy.to_value();
        let names = v.get("text_models").and_then(Value::as_array).unwrap_or(&[]);
        let current = super::setup::selected_model(base);
        if text(self.policy.profile("text")?, "backend")? == "local_ollama"
            && self.policy.profile("text")?.get("model").and_then(Value::as_str) == Some("qwen2.5:3b")
            && !names.iter().any(|n| n.as_str() == Some("qwen2.5:3b"))
        {
            let chosen = names.iter().filter_map(Value::as_str).find(|s| *s == current).or_else(|| {
                names.iter().filter_map(Value::as_str).find(|s| s.to_lowercase().contains("qwen") && s.contains("3b"))
            });
            if let Some(model) = chosen {
                let mut profiles = get(&cfg, "profiles")?.clone();
                let mut p = get(&profiles, "text")?.clone();
                set(&mut p, "model", model.into());
                set(&mut profiles, "text", p);
                set(&mut cfg, "profiles", profiles);
                self.policy = ModelPolicy::from_value(&cfg)?;
                atomic(&dir.join("models.json"), cfg.to_json().as_bytes())?;
            }
        }
        self.report = v;
        Ok(())
    }
    pub fn evaluate(
        &mut self,
        mut signals: Value,
        base: &providers::Config,
        budget: &gamectx::studio::ProviderBudget,
    ) -> Result<()> {
        let mut available = self.report.get("available").and_then(Value::as_array).unwrap_or(&[]).to_vec();
        let names = self.report.get("text_models").and_then(Value::as_array).unwrap_or(&[]);
        for id in ["text", "dramatic_text", "image", "dramatic_image", "tts", "stt"] {
            let p = self.policy.profile(id)?;
            let channel = if id.contains("text") {
                "text"
            } else if id.contains("image") {
                "image"
            } else {
                id
            };
            let configured = text(p, "backend")?;
            let backend = if configured == "inherit" { text(&base.value, channel)? } else { configured };
            if backend.starts_with("online_") {
                available.retain(|v| v.as_str() != Some(id));
                let summary = budget.summary();
                let ceiling = num(base.endpoint(backend)?, "ceiling_micros", 1_000_000_000_000)?;
                if !text(p, "model")?.is_empty()
                    && base.has_credentials(backend)
                    && ceiling > 0
                    && num(&summary, "charged_or_reserved_micros", 1_000_000_000_000)?.saturating_add(ceiling)
                        <= num(&summary, "cap_micros", 1_000_000_000_000)?
                {
                    available.push(id.into());
                }
            } else if channel == "text"
                && names.iter().any(|n| n.as_str() == p.get("model").and_then(Value::as_str))
                && !available.iter().any(|n| n.as_str() == Some(id))
            {
                available.push(id.into());
            }
        }
        set(&mut self.report, "eligible", available.clone().into());
        set(&mut signals, "available", available.into());
        self.route = self.policy.evaluate(&signals)?;
        let mut warm = self.route.get("prewarm").and_then(Value::as_array).unwrap_or(&[]).to_vec();
        warm.retain(|v| {
            v.as_str().and_then(|id| self.policy.profile(id).ok()).is_some_and(|p| {
                let id = v.as_str().unwrap();
                let ch = if id.contains("text") {
                    "text"
                } else if id.contains("image") {
                    "image"
                } else {
                    id
                };
                let backend = text(p, "backend").unwrap_or("");
                let resolved = if backend == "inherit" { text(&base.value, ch).unwrap_or("") } else { backend };
                resolved.starts_with("local_")
            })
        });
        set(&mut self.route, "prewarm", warm.into());
        Ok(())
    }
    pub fn selected<'a>(&'a self, ch: &str) -> Option<&'a str> {
        if !self.policy.enabled() {
            return None;
        }
        if ["tts", "stt"].contains(&ch) {
            return Some(if ch == "tts" { "tts" } else { "stt" });
        }
        let id = self.route.get(ch)?.as_str()?;
        if self.policy.profile(id).is_ok() {
            Some(id)
        } else {
            None
        }
    }
    pub fn provider(&self, base: &providers::Config, ch: &str) -> Result<providers::Config> {
        if !self.policy.enabled() {
            return Ok(base.clone());
        }
        let Some(id) = self.selected(ch) else {
            let mut c = base.value.clone();
            set(&mut c, ch, gamectx::studio::fallback(ch)?.into());
            return providers::Config::parse(&c.to_json());
        };
        let p = self.policy.profile(id)?;
        let mut c = base.value.clone();
        let backend = if text(p, "backend")? == "inherit" { text(&base.value, ch)? } else { text(p, "backend")? };
        set(&mut c, ch, backend.into());
        if ch == "text" {
            set(&mut c, "size", get(p, "size")?.clone());
            if ["local_ollama", "online_openai_compatible"].contains(&backend) {
                let mut all = get(&c, "text_models")?.clone();
                let mut mapping = get(&all, backend)?.clone();
                set(&mut mapping, text(p, "size")?, get(p, "model")?.clone());
                set(&mut all, backend, mapping);
                set(&mut c, "text_models", all);
            }
        } else if backend.starts_with("local_") || backend.starts_with("online_") {
            let mut endpoints = get(&c, "endpoints")?.clone();
            let mut e = get(&endpoints, backend)?.clone();
            set(&mut e, "model", get(p, "model")?.clone());
            set(&mut endpoints, backend, e);
            set(&mut c, "endpoints", endpoints);
        }
        providers::Config::parse(&c.to_json())
    }
    pub fn effective(&self, base: &providers::Config) -> Result<providers::Config> {
        let mut c = base.clone();
        for ch in ["text", "image", "tts", "stt"] {
            c = self.provider(&c, ch)?;
        }
        Ok(c)
    }
    pub fn control(&self, base: &providers::Config, action: &str, id: &str) -> Result<Value> {
        if ["start", "prewarm"].contains(&action) {
            let p = self.policy.profile(id)?;
            let ch = if id.contains("text") {
                "text"
            } else if id.contains("image") {
                "image"
            } else {
                id
            };
            let backend = text(p, "backend")?;
            let resolved = if backend == "inherit" { text(&base.value, ch)? } else { backend };
            if !resolved.starts_with("local_") {
                return Err(Error::new("Online/browser profiles do not need local loading."));
            }
        }
        let mut roots = obj! {};
        for (ch, backend) in
            [("text", "local_ollama"), ("image", "local_sd_cpp"), ("tts", "local_tts_http"), ("stt", "local_stt_http")]
        {
            let url = text(base.endpoint(backend)?, "url")?;
            let tail = url.strip_prefix("http://").ok_or_else(err)?;
            let address = tail.split('/').next().ok_or_else(err)?;
            set(&mut roots, ch, format!("http://{address}").into());
        }
        Ok(obj! {"action"=>action,"profile"=>id,"config"=>self.policy.to_value(),"roots"=>roots})
    }
    pub fn snapshot(&self) -> Value {
        obj! {"config"=>self.policy.to_value(),"route"=>self.route.clone(),"runtime"=>self.report.clone(),"dramatic_art"=>self.art.to_value()}
    }
    pub fn reset_scene(&mut self) -> Result<()> {
        self.policy = ModelPolicy::from_value(&self.policy.to_value())?;
        self.route = Value::Null;
        Ok(())
    }
}

// Display-only previous approved art. This does not approve it under the new spec.
pub fn preview_compatible(old: &Value, current: &Value) -> bool {
    for key in [
        "scope",
        "observer",
        "audience",
        "entity",
        "scene",
        "scene_epoch",
        "format",
        "silhouette",
        "attire",
        "visible_props",
        "environment",
    ] {
        if old.get(key) != current.get(key) {
            return false;
        }
    }
    for (field, key, neutral) in [("expression", "slot", "neutral"), ("pose", "id", "idle")] {
        let a = old.get(field).and_then(|v| v.get(key));
        let b = current.get(field).and_then(|v| v.get(key));
        if a != b && a.and_then(Value::as_str) != Some(neutral) {
            return false;
        }
    }
    let props = old.get("visible_props").and_then(Value::as_array).unwrap_or(&[]);
    !current.get("forbidden_props").and_then(Value::as_array).unwrap_or(&[]).iter().any(|p| props.contains(p))
}

fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(rows) => {
            let mut rows = rows.iter().map(|(k, v)| (k.clone(), canonical(v))).collect::<Vec<_>>();
            rows.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(rows)
        }
        Value::Array(rows) => rows.iter().map(canonical).collect::<Vec<_>>().into(),
        _ => v.clone(),
    }
}
pub fn generation_identity(bible: &Value, profile: &Value) -> Result<String> {
    let selected = obj! {"backend"=>get(profile,"backend")?.clone(),"model"=>get(profile,"model")?.clone(),"recipe"=>get(profile,"recipe")?.clone(),"selection"=>get(profile,"selection")?.clone(),"steps"=>get(profile,"steps")?.clone()};
    gamectx::visual::content_digest(
        format!("{}{}", canonical(bible).to_json(), canonical(&selected).to_json()).as_bytes(),
    )
}

pub fn loading_identity(profile: &Value, base: &str) -> Result<String> {
    gamectx::visual::content_digest(format!("{}{}", canonical(profile).to_json(), base).as_bytes())
}
