//! Pure bounded plans for separately installed native audio engines.
use crate::{checked as c, obj, Result, Value};
fn err() -> crate::Error {
    c::bad()
}
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    c::get(v, k)
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(get(v, k)?)
}
fn fields(v: &Value, keys: &[&str]) -> Result<()> {
    c::fields(v, keys)
}
fn ident(v: &Value, k: &str) -> Result<String> {
    let s = text(v, k)?;
    if s.is_empty() || s.len() > 128 || !s.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c)) {
        return Err(err());
    }
    Ok(s.into())
}
/// Validate an audio request without accepting executable or model-file paths.
/// The example host resolves these aliases through its private installation file.
pub fn native_audio_plan(a: &Value) -> Result<Value> {
    if !a.is_valid(4096) {
        return Err(err());
    }
    fields(a, &["engine", "request"])?;
    let r = get(a, "request")?;
    ident(r, "model")?;
    match text(a, "engine")? {
        "whisper" => {
            fields(r, &["model"])?;
            Ok(obj! {"model"=>ident(r,"model")?,"threads"=>4,"format"=>"text"})
        }
        "piper" => {
            fields(r, &["model", "input", "voice", "speed", "response_format"])?;
            let input = text(r, "input")?;
            let speed = match get(r, "speed")? {
                Value::Number(n) => n.parse::<f64>().map_err(|_| err())?,
                _ => return Err(err()),
            };
            if input.is_empty()
                || input.chars().count() > 240
                || input.chars().any(char::is_control)
                || !(0.5..=2.0).contains(&speed)
                || text(r, "response_format")? != "wav"
            {
                return Err(err());
            }
            Ok(obj! {"voice"=>ident(r,"voice")?,"text"=>input,"length_scale"=>format!("{:.6}",1.0/speed),"threads"=>4})
        }
        _ => Err(err()),
    }
}
