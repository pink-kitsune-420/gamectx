//! Bounded player-message slicing and complete sentence reply envelopes.
use crate::{checked as c, obj, Result, Value};
/// Pure local/binding contract. Splitting normalizes layout whitespace only;
/// words, negations, names and order are retained. No summarizer or model calls.
pub fn message_chunks(v: &Value) -> Result<Value> {
    crate::packet::request::bounded(v, 32768)?;
    c::fields(v, &["op", "args"])?;
    let a = c::get(v, "args")?;
    match c::text(c::get(v, "op")?)? {
        "player" => {
            c::fields(a, &["text", "max_chars"])?;
            let cap = c::uint(c::get(a, "max_chars")?, 512, 4096)? as usize;
            let text = c::text(c::get(a, "text")?)?;
            if text.is_empty()
                || text.len() > 16384
                || text.chars().count() > cap
                || text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            {
                return Err(c::bad());
            }
            let normalized = text.replace(['\n', '\r', '\t'], " ");
            let mut tail = normalized.as_str();
            let mut out = Vec::new();
            while !tail.is_empty() {
                let (mut count, mut end, mut last) = (0, 0, None);
                for (i, ch) in tail.char_indices() {
                    let next = i + ch.len_utf8();
                    if count == 384 || next > 960 {
                        break;
                    }
                    count += 1;
                    end = next;
                    if ch.is_whitespace() || matches!(ch, '.' | '!' | '?') {
                        last = Some((next, count));
                    }
                }
                let end =
                    if end < tail.len() { last.filter(|(_, n)| *n >= count / 2).map_or(end, |(i, _)| i) } else { end };
                out.push(Value::str(&tail[..end]));
                tail = &tail[end..];
            }
            if out.len() > 32 {
                return Err(c::bad());
            }
            Ok(obj! {"chunks"=>out,"characters"=>normalized.chars().count(),"summary_authoritative"=>false})
        }
        "reply" => {
            c::fields(a, &["speech", "max_chunks"])?;
            let cap = c::uint(c::get(a, "max_chunks")?, 1, 3)? as usize;
            let xs = c::list(c::get(a, "speech")?)?;
            if xs.is_empty() || xs.len() > cap {
                return Err(c::bad());
            }
            let mut characters = 0;
            for (i, x) in xs.iter().enumerate() {
                let s = c::text(x)?;
                let n = s.chars().count();
                if s.is_empty()
                    || n > 240
                    || s.len() > 1024
                    || s.chars().any(char::is_control)
                    || xs[..i].iter().filter_map(Value::as_str).any(|old| old.to_lowercase() == s.to_lowercase())
                {
                    return Err(c::bad());
                }
                let assessment = crate::character::life::assess_sentence(s, 240)?.to_value();
                if assessment.get("boundary_complete") != Some(&true.into())
                    || assessment.get("single_sentence") != Some(&true.into())
                {
                    return Err(c::bad());
                }
                characters += n;
            }
            Ok(obj! {"chunks"=>xs.to_vec(),"characters"=>characters,"model_calls"=>1})
        }
        _ => Err(c::bad()),
    }
}
