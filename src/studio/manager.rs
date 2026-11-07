//! Deterministic model selection from bounded, authoritative scene signals.
//! This policy never starts a process, downloads weights or calls a provider.
use crate::{checked as c, obj, Result, Value};
const IDS: &[&str] = &["text", "dramatic_text", "image", "dramatic_image", "tts", "stt"];
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    c::get(v, k)
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(get(v, k)?)
}
fn num(v: &Value, k: &str, max: u64) -> Result<u64> {
    c::uint(get(v, k)?, 0, max)
}
fn yes(v: &Value, k: &str) -> Result<bool> {
    c::boolean(get(v, k)?)
}
fn label(s: &str, cap: usize, extra: &[u8]) -> Result<()> {
    if s.len() > cap || !s.bytes().all(|b| b.is_ascii_alphanumeric() || extra.contains(&b)) {
        return Err(c::bad());
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct ModelPolicy {
    value: Value,
    last: u64,
    active: bool,
    until: u64,
    after: u64,
}
impl ModelPolicy {
    pub fn from_value(v: &Value) -> Result<Self> {
        crate::request::bounded(v, 32768)?;
        c::fields(
            v,
            &[
                "version",
                "enabled",
                "mode",
                "text_escalation",
                "image_escalation",
                "residency",
                "idle_seconds",
                "prewarm",
                "ram_limit_mib",
                "reserve_mib",
                "rules",
                "profiles",
            ],
        )?;
        c::uint(get(v, "version")?, 1, 1)?;
        for k in ["enabled", "text_escalation", "image_escalation", "prewarm"] {
            yes(v, k)?;
        }
        if !["auto", "normal", "dramatic"].contains(&text(v, "mode")?)
            || !["on_demand", "warm"].contains(&text(v, "residency")?)
        {
            return Err(c::bad());
        }
        for (k, lo, hi) in [("idle_seconds", 1, 3600), ("ram_limit_mib", 256, 65536), ("reserve_mib", 128, 8192)] {
            c::uint(get(v, k)?, lo, hi)?;
        }
        let r = get(v, "rules")?;
        c::fields(
            r,
            &[
                "combat",
                "emergency",
                "low_health",
                "significance",
                "attitude_change",
                "conflict",
                "hold_turns",
                "cooldown_turns",
            ],
        )?;
        for k in ["combat", "emergency", "conflict"] {
            yes(r, k)?;
        }
        for (k, hi) in [
            ("low_health", 100),
            ("significance", 100),
            ("attitude_change", 200),
            ("hold_turns", 100),
            ("cooldown_turns", 100),
        ] {
            num(r, k, hi)?;
        }
        let profiles = get(v, "profiles")?;
        c::fields(profiles, IDS)?;
        for id in IDS {
            let p = get(profiles, id)?;
            c::fields(
                p,
                if id.contains("text") {
                    &["model", "ram_mib", "backend", "size"]
                } else if id.contains("image") {
                    &["model", "ram_mib", "backend", "recipe", "selection", "steps"]
                } else {
                    &["model", "ram_mib", "backend"]
                },
            )?;
            let channel = if id.contains("text") {
                "text"
            } else if id.contains("image") {
                "image"
            } else {
                id
            };
            let backend = text(p, "backend")?;
            if backend != "inherit" && !super::backends(channel)?.contains(&backend) {
                return Err(c::bad());
            }
            if backend != "inherit"
                && !backend.starts_with("local_")
                && !backend.starts_with("online_")
                && !["tts", "stt"].contains(&channel)
            {
                return Err(c::bad());
            }
            label(text(p, "model")?, 128, b"-_:./")?;
            if text(p, "model")?.is_empty() {
                return Err(c::bad());
            }
            c::uint(get(p, "ram_mib")?, 64, 65536)?;
            if id.contains("text") && !["1", "1.5", "3", "8", "30", "70"].contains(&text(p, "size")?) {
                return Err(c::bad());
            }
            if id.contains("image") {
                let recipe = text(p, "recipe")?;
                label(recipe, 128, b"-_.")?;
                if recipe.is_empty() {
                    return Err(c::bad());
                }
                c::uint(get(p, "steps")?, 1, 64)?;
                let selection = get(p, "selection")?.as_object().ok_or_else(c::bad)?;
                if selection.is_empty() || selection.len() > 8 {
                    return Err(c::bad());
                }
                for (k, v) in selection {
                    label(k, 128, b"-_")?;
                    let id = c::text(v)?;
                    if id.len() > 256 || id.chars().any(char::is_control) {
                        return Err(c::bad());
                    }
                    if k.is_empty() || id.starts_with('/') || id.split('/').any(|s| s == "..") {
                        return Err(c::bad());
                    }
                }
            }
        }
        Ok(Self { value: v.clone(), last: 0, active: false, until: 0, after: 0 })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn enabled(&self) -> bool {
        yes(&self.value, "enabled").expect("validated policy")
    }
    pub fn profile(&self, id: &str) -> Result<&Value> {
        get(get(&self.value, "profiles")?, id)
    }
    pub fn evaluate(&mut self, s: &Value) -> Result<Value> {
        crate::request::bounded(s, 8192)?;
        c::fields(
            s,
            &[
                "turn",
                "health",
                "combat",
                "atmosphere",
                "significance",
                "attitude_delta",
                "conflict",
                "next",
                "available",
            ],
        )?;
        let turn = c::uint(get(s, "turn")?, 1, u32::MAX as u64 - 201)?;
        if turn < self.last {
            return Err(c::bad());
        }
        let health = num(s, "health", 100)?;
        let significance = num(s, "significance", 100)?;
        let delta = num(s, "attitude_delta", 200)?;
        let combat = yes(s, "combat")?;
        let conflict = yes(s, "conflict")?;
        let atmosphere = text(s, "atmosphere")?;
        if !["calm", "busy", "tense", "emergency"].contains(&atmosphere) {
            return Err(c::bad());
        }
        let next = c::list(get(s, "next")?)?;
        let available = c::list(get(s, "available")?)?;
        for (rows, cap, allowed) in [(next, 4, &["text", "image", "tts", "stt"][..]), (available, 6, IDS)] {
            if rows.len() > cap {
                return Err(c::bad());
            }
            for (i, v) in rows.iter().enumerate() {
                if !allowed.contains(&c::text(v)?) || rows[..i].contains(v) {
                    return Err(c::bad());
                }
            }
        }
        let r = get(&self.value, "rules")?;
        let mut reasons = Vec::new();
        for (id, applies) in [
            ("combat", yes(r, "combat")? && combat),
            ("emergency", yes(r, "emergency")? && atmosphere == "emergency"),
            ("low_health", num(r, "low_health", 100)? > 0 && health <= num(r, "low_health", 100)?),
            ("significance", num(r, "significance", 100)? > 0 && significance >= num(r, "significance", 100)?),
            ("attitude_change", num(r, "attitude_change", 200)? > 0 && delta >= num(r, "attitude_change", 200)?),
            ("conflict", yes(r, "conflict")? && conflict),
        ] {
            if applies {
                reasons.push(id);
            }
        }
        let mut active = self.active;
        let mut until = self.until;
        let mut after = self.after;
        let mode = text(&self.value, "mode")?;
        if !self.enabled() || mode == "normal" {
            active = false;
            reasons.clear();
            until = 0;
            after = 0;
        } else if mode == "dramatic" {
            active = true;
            reasons = vec!["manual"];
            until = turn + num(r, "hold_turns", 100)?;
        } else if !reasons.is_empty() && (active || turn >= after || reasons.contains(&"emergency")) {
            active = true;
            until = turn + num(r, "hold_turns", 100)?;
        } else if active && turn <= until {
            reasons = vec!["hold"];
        } else {
            if active {
                after = turn + num(r, "cooldown_turns", 100)?;
            }
            active = false;
            reasons = if reasons.is_empty() { Vec::new() } else { vec!["cooldown"] };
        }
        let mut fallbacks = Vec::new();
        let mut choose = |channel: &str| -> Result<String> {
            let dramatic = active && yes(&self.value, &format!("{channel}_escalation"))?;
            let desired = if dramatic { format!("dramatic_{channel}") } else { channel.into() };
            if !self.enabled() {
                return Ok(channel.into());
            }
            for (i, key) in [desired.as_str(), channel].iter().enumerate() {
                if i == 1 && desired == channel {
                    continue;
                }
                let exists = available.iter().any(|v| v.as_str() == Some(key));
                let fits = text(self.profile(key)?, "backend")?.starts_with("online_")
                    || num(self.profile(key)?, "ram_mib", 65536)? <= num(&self.value, "ram_limit_mib", 65536)?;
                if exists && fits {
                    return Ok((*key).into());
                }
                fallbacks.push(format!("{key}:{}", if exists { "ram_limit" } else { "unavailable" }));
            }
            Ok(if channel == "text" { "fallback" } else { "cached" }.into())
        };
        let text = choose("text")?;
        let image = choose("image")?;
        let mut warm = Vec::<String>::new();
        if self.enabled() && yes(&self.value, "prewarm")? {
            for n in next {
                let id = match c::text(n)? {
                    "text" => text.as_str(),
                    "image" => image.as_str(),
                    x => x,
                };
                if IDS.contains(&id)
                    && available.iter().any(|v| v.as_str() == Some(id))
                    && (c::text(get(self.profile(id)?, "backend")?)? == "inherit"
                        || c::text(get(self.profile(id)?, "backend")?)?.starts_with("local_"))
                    && num(self.profile(id)?, "ram_mib", 65536)? <= num(&self.value, "ram_limit_mib", 65536)?
                    && !warm.iter().any(|v| v == id)
                {
                    warm.push(id.into());
                }
            }
        }
        let result = obj! {"dramatic"=>active,"reasons"=>reasons,"text"=>text,"image"=>image.clone(),"art"=>if image=="dramatic_image"{"detailed"}else{"standard"},"prewarm"=>warm,"held_until"=>until,"retry_after"=>after,"fallbacks"=>fallbacks};
        self.last = turn;
        self.active = active;
        self.until = until;
        self.after = after;
        Ok(result)
    }
}
pub(crate) fn fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["config", "signals"])?;
    let mut p = ModelPolicy::from_value(get(a, "config")?)?;
    let signals = c::list(get(a, "signals")?)?;
    if signals.len() > 64 {
        return Err(c::bad());
    }
    let turns = signals.iter().map(|s| p.evaluate(s)).collect::<Result<Vec<_>>>()?;
    Ok(obj! {"turns"=>turns})
}
