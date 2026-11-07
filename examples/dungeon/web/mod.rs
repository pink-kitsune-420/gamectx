//! Opt-in loopback studio. Game rules remain in the original deterministic host.
mod app;
mod http;
mod manager_runtime;
mod manager_view;
mod native_voice;
pub fn configure_voice(root: &str, dir: &str) -> Result<()> {
    native_voice::configure(std::path::Path::new(root), std::path::Path::new(dir))
}
mod providers;
mod session;
mod setup;
mod speech_wav;
#[cfg(test)]
mod tests;
use super::game;
use game::{fields, get, text};
use gamectx::{Error, Result, Value};
pub fn service_guard() -> Result<()> {
    manager_runtime::service_guard()
}
// Linux keeps this executable handle valid even after a development rebuild unlinks it.
fn executable() -> Result<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        Ok(std::path::PathBuf::from("/proc/self/exe"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::current_exe().map_err(|_| err())
    }
}
fn err() -> Error {
    Error::new("invalid studio request")
}
fn num(v: &Value, k: &str, max: u64) -> Result<u64> {
    match get(v, k)? {
        Value::Number(s)
            if !s.is_empty() && (s.len() == 1 || !s.starts_with('0')) && s.bytes().all(|b| b.is_ascii_digit()) =>
        {
            s.parse::<u64>().ok().filter(|n| *n <= max).ok_or_else(err)
        }
        _ => Err(err()),
    }
}
fn set(v: &mut Value, k: &str, x: Value) {
    if let Value::Object(xs) = v {
        if let Some((_, old)) = xs.iter_mut().find(|(n, _)| n == k) {
            *old = x;
        } else {
            xs.push((k.into(), x));
        }
    }
}
fn parse(s: &str) -> Result<Value> {
    gamectx::json::parse(s, 8 * 1024 * 1024)
}
fn read_file(path: &std::path::Path, cap: usize) -> Result<String> {
    use std::io::Read;
    let mut out = String::new();
    std::fs::File::open(path).map_err(|_| err())?.take(cap as u64 + 1).read_to_string(&mut out).map_err(|_| err())?;
    if out.len() > cap {
        return Err(err());
    }
    Ok(out)
}
fn atomic(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension("pending");
    std::fs::write(&temp, bytes).map_err(|_| err())?;
    std::fs::File::open(&temp).and_then(|f| f.sync_all()).map_err(|_| err())?;
    std::fs::rename(&temp, path).map_err(|_| err())?;
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent).and_then(|f| f.sync_all()).map_err(|_| err())?;
    }
    Ok(())
}
pub fn run(seed: u32, port: u16, cap: u64, config: Option<&str>, dir: &str) -> Result<()> {
    let config = if let Some(file) = config {
        providers::Config::parse(&read_file(std::path::Path::new(file), 65536)?)?
    } else {
        providers::Config::parse(include_str!("providers.json"))?
    };
    http::serve(seed, port, cap, config, std::path::PathBuf::from(dir))
}

/// Exact decimal USD to integer microdollars; no floating-point or rounding upward.
pub fn parse_cap(s: &str) -> Result<u64> {
    let (whole, fraction) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty()
        || whole.len() > 7
        || fraction.len() > 6
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(err());
    }
    let a = whole.parse::<u64>().map_err(|_| err())?;
    let b = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u64>().map_err(|_| err())? * 10u64.pow(6 - fraction.len() as u32)
    };
    a.checked_mul(1_000_000).and_then(|x| x.checked_add(b)).filter(|n| *n <= 1_000_000_000_000).ok_or_else(err)
}

pub fn run_lan(seed: u32, port: u16, cap: u64, config: Option<&str>, dir: &str, address: &str) -> Result<()> {
    let config = if let Some(file) = config {
        providers::Config::parse(&read_file(std::path::Path::new(file), 65536)?)?
    } else {
        providers::Config::parse(include_str!("providers.json"))?
    };
    let address = address.parse().map_err(|_| Error::new("Expected a private IPv4 LAN address."))?;
    http::serve_lan(seed, port, cap, config, std::path::PathBuf::from(dir), Some(address))
}
