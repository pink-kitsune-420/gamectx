//! Bounded speech parts prepared together before playback, never per-word inference.
use crate::{checked as c, obj, Result, Value};
/// Partition a bounded line at sentence/word boundaries for a single engine session.
/// Offsets count Unicode scalars, and all parts concatenate to the requested suffix.
pub fn speech_buffer(a: &Value) -> Result<Value> {
    if !a.is_valid(4096) {
        return Err(c::bad());
    }
    c::fields(a, &["text", "start"])?;
    let text = c::text(c::get(a, "text")?)?;
    let chars = text.chars().collect::<Vec<_>>();
    if chars.is_empty() || chars.len() > 240 || chars.iter().any(|c| c.is_control()) {
        return Err(c::bad());
    }
    let mut pos = c::uint(c::get(a, "start")?, 0, chars.len() as u64 - 1)? as usize;
    let mut parts = Vec::new();
    while pos < chars.len() {
        let end = (pos + 80).min(chars.len());
        let sentence = (pos..end)
            .find(|&i| matches!(chars[i], '.' | '!' | '?') && (i + 1 == chars.len() || chars[i + 1].is_whitespace()));
        let mut cut = sentence.map_or_else(
            || {
                if end < chars.len() {
                    (pos..end).rfind(|&i| chars[i].is_whitespace()).map_or(end, |i| i + 1)
                } else {
                    end
                }
            },
            |i| i + 1,
        );
        while cut < chars.len() && cut - pos < 80 && chars[cut].is_whitespace() {
            cut += 1;
        }
        parts.push(obj! {"text"=>chars[pos..cut].iter().collect::<String>(),"start"=>pos,"through"=>cut});
        pos = cut;
    }
    Ok(obj! {"parts"=>parts,"through"=>chars.len()})
}
