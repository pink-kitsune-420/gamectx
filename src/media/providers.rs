//! Provider backend names, local fallbacks and a cumulative per-session spending cap.
//! Selection never contacts a provider; hosts dispatch and settle reservations.
use crate::{checked as c, obj, runtime::spending::SpendingLedger, Result, Value};
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
        crate::packet::request::bounded(v, 65536)?;
        c::fields(v, &["authorized", "cap", "ledger"])?;
        c::uint(c::get(v, "authorized")?, 0, 1_000_000_000_000)?;
        c::uint(c::get(v, "cap")?, 0, 1_000_000_000_000)?;
        let recovered = SpendingLedger::restore(c::get(v, "ledger")?, 0)?;
        let spent = c::uint(c::get(&recovered.summary(0)?, "spent")?, 0, 1_000_000_000_000)?;
        let mut raw = recovered.to_value();
        crate::character::archetype::set_field(&mut raw, "limit", authorized.max(spent).into());
        let ledger = SpendingLedger::restore(&raw, 0)?;
        Ok(Self { ledger, authorized, cap: authorized })
    }
}

pub fn routing_workflow(a: &Value) -> Result<Value> {
    crate::packet::request::bounded(a, 65536)?;
    c::fields(a, &["cap", "operations"])?;
    let mut b = ProviderBudget::new(c::uint(c::get(a, "cap")?, 0, 1_000_000_000_000)?)?;
    super::workflow(c::get(a, "operations")?, |op, v| match op {
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
            let id = crate::character::life::num(v, "id")?;
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
