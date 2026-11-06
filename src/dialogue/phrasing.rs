//! Optional deterministic wording for attributed player quotes. This is rendering,
//! not registration, truth verification, or an unrestricted model rewrite pass.
use crate::{checked as c, Result};
pub fn render_quote(input: &str) -> Result<String> {
    if input.trim().is_empty()
        || input.len() > 4096
        || input.chars().count() > 1024
        || input.chars().any(char::is_control)
    {
        return Err(c::bad());
    }
    let text = input.trim();
    let lower = text.to_ascii_lowercase();
    for (prefix, replacement) in [
        ("my name is ", "You told me your name is "),
        ("my home is ", "You told me your home is "),
        ("i live in ", "You told me you live in "),
    ] {
        if lower.starts_with(prefix) {
            let tail = text[prefix.len()..].trim_end_matches('.');
            if !tail.is_empty()
                && tail.chars().count() <= 80
                && tail.chars().all(|c| c.is_alphanumeric() || " '-".contains(c))
            {
                return Ok(format!("{replacement}{tail}."));
            }
        }
    }
    Ok(format!("I remember you saying: \"{input}\""))
}
