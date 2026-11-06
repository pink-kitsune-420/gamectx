//! Small shared validators for the opt-in runtime modules.
use crate::{Error, Result, Value};
pub(crate) fn bad() -> Error {
    Error::new("invalid or out-of-bounds runtime value")
}
pub(crate) fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(bad)
}
pub(crate) fn list(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(bad)
}
pub(crate) fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(bad)
}
pub(crate) fn boolean(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(bad)
}
pub(crate) fn fields(v: &Value, keys: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(bad)?;
    if rows.len() != keys.len()
        || keys.iter().any(|k| v.get(k).is_none())
        || rows
            .iter()
            .enumerate()
            .any(|(i, (k, _))| !keys.contains(&k.as_str()) || rows[..i].iter().any(|(old, _)| old == k))
    {
        return Err(bad());
    }
    Ok(())
}
pub(crate) fn uint(v: &Value, low: u64, high: u64) -> Result<u64> {
    match v {
        Value::Number(s)
            if !s.is_empty() && (s.len() == 1 || !s.starts_with('0')) && s.bytes().all(|b| b.is_ascii_digit()) =>
        {
            s.parse::<u64>().ok().filter(|n| (low..=high).contains(n)).ok_or_else(bad)
        }
        _ => Err(bad()),
    }
}
pub(crate) fn name(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 32
        || !s.as_bytes()[0].is_ascii_lowercase()
        || !s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(bad());
    }
    Ok(())
}
pub(crate) fn scope(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 256 || s.chars().count() > 64 || s.chars().any(char::is_control) {
        return Err(bad());
    }
    Ok(())
}
pub(crate) fn time(now: u64, clock: u64) -> Result<()> {
    if now < clock || now > u32::MAX as u64 {
        return Err(bad());
    }
    Ok(())
}
