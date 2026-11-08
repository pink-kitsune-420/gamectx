//! Opt-in native reasoning controls and unverified, source-scoped planning notes.
use crate::{checked as c, obj, Result, Value};
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    c::get(v, k)
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(get(v, k)?)
}
fn line(v: &Value, cap: usize) -> Result<&str> {
    let s = c::text(v)?;
    if s.is_empty() || s.len() > cap || s.chars().any(char::is_control) {
        return Err(c::bad());
    }
    Ok(s)
}
/// Validates a host-owned policy. Online control values are explicit declarations
/// for an exact model ID, never inferred from its name or parameter count.
pub fn validate_deliberation_config(v: &Value) -> Result<()> {
    crate::packet::request::bounded(v, 8192)?;
    c::fields(v, &["version", "mode", "effort", "max_tokens", "hypotheses", "online"])?;
    c::uint(get(v, "version")?, 1, 1)?;
    if !["off", "auto"].contains(&text(v, "mode")?) || !["low", "medium", "high"].contains(&text(v, "effort")?) {
        return Err(c::bad());
    }
    c::uint(get(v, "max_tokens")?, 128, 4096)?;
    c::boolean(get(v, "hypotheses")?)?;
    let profiles = c::list(get(v, "online")?)?;
    if profiles.len() > 16 {
        return Err(c::bad());
    }
    for (i, p) in profiles.iter().enumerate() {
        c::fields(p, &["model", "values"])?;
        line(get(p, "model")?, 128)?;
        if profiles[..i].iter().any(|x| x.get("model") == p.get("model")) {
            return Err(c::bad());
        }
        let values = c::list(get(p, "values")?)?;
        if values.is_empty() || values.len() > 8 {
            return Err(c::bad());
        }
        for (j, x) in values.iter().enumerate() {
            if !["none", "minimal", "low", "medium", "high", "xhigh", "max"].contains(&c::text(x)?)
                || values[..j].contains(x)
            {
                return Err(c::bad());
            }
        }
    }
    Ok(())
}
/// Validates only explicitly advertised Ollama values; absent metadata stays null.
pub fn validate_thinking_values(v: &Value) -> Result<()> {
    if v == &Value::Null {
        return Ok(());
    }
    let xs = c::list(v)?;
    if xs.len() > 8 {
        return Err(c::bad());
    }
    for (i, x) in xs.iter().enumerate() {
        if xs[..i].contains(x) {
            return Err(c::bad());
        }
        match x {
            Value::Bool(_) => {}
            Value::String(_) => {
                let s = line(x, 24)?;
                if !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                    return Err(c::bad());
                }
            }
            _ => return Err(c::bad()),
        }
    }
    Ok(())
}
/// Chooses a native control within a shared answer/reasoning token ceiling. The
/// host supplies the task and actual dispatched model's capabilities. No calls.
pub fn deliberation_plan(v: &Value) -> Result<Value> {
    crate::packet::request::bounded(v, 16384)?;
    c::fields(v, &["config", "backend", "model", "values", "task", "expanded", "output_tokens"])?;
    let cfg = get(v, "config")?;
    validate_deliberation_config(cfg)?;
    let backend = text(v, "backend")?;
    if !["local_ollama", "online_openai_compatible"].contains(&backend) {
        return Err(c::bad());
    }
    let model = line(get(v, "model")?, 128)?;
    validate_thinking_values(get(v, "values")?)?;
    let task = text(v, "task")?;
    if !["routine", "planning", "conflict", "investigation"].contains(&task) {
        return Err(c::bad());
    }
    let expanded = c::boolean(get(v, "expanded")?)?;
    let output = c::uint(get(v, "output_tokens")?, 1, 512)?;
    let values = if backend == "local_ollama" {
        get(v, "values")?.as_array()
    } else {
        c::list(get(cfg, "online")?)?
            .iter()
            .find(|p| p.get("model").and_then(Value::as_str) == Some(model))
            .and_then(|p| p.get("values"))
            .and_then(Value::as_array)
    };
    let auto = text(cfg, "mode")? == "auto";
    let desired = auto && task != "routine";
    let chosen = if !auto {
        None
    } else {
        values.and_then(|xs| {
            if desired {
                let effort = Value::str(text(cfg, "effort").unwrap());
                if xs.contains(&effort) {
                    Some(effort)
                } else if xs.contains(&Value::Bool(true)) {
                    Some(Value::Bool(true))
                } else {
                    None
                }
            } else if xs.contains(&Value::Bool(false)) {
                Some(Value::Bool(false))
            } else if xs.contains(&Value::str("none")) {
                Some(Value::str("none"))
            } else {
                None
            }
        })
    };
    let active = desired && chosen.is_some();
    Ok(
        obj! {"active"=>active,"reason"=>if !auto{"disabled"}else if !desired{"routine"}else if active{"enabled"}else{"controls_unavailable"},"control"=>chosen.map_or(Value::Null,|value|obj!{"field"=>if backend=="local_ollama"{"think"}else{"reasoning_effort"},"value"=>value}),"max_tokens"=>if active{output.max(c::uint(get(cfg,"max_tokens")?,128,4096)?)}else{output},"hypotheses"=>desired&&expanded&&c::boolean(get(cfg,"hypotheses")?)?},
    )
}
/// Unverified notes never become speech, facts, memories or executable actions.
/// Citation membership is checked; semantic correctness is not certified.
pub fn validate_hypotheses(notes: &Value, evidence: &Value) -> Result<Value> {
    crate::packet::request::bounded(notes, 8192)?;
    crate::packet::request::bounded(evidence, 2048)?;
    let ns = c::list(notes)?;
    let es = c::list(evidence)?;
    if ns.len() > 3 || es.len() > 16 {
        return Err(c::bad());
    }
    for e in es {
        line(e, 64)?;
    }
    for n in ns {
        c::fields(n, &["claim", "evidence", "check"])?;
        line(get(n, "claim")?, 180)?;
        line(get(n, "check")?, 180)?;
        let cited = c::list(get(n, "evidence")?)?;
        if cited.is_empty() || cited.len() > 4 {
            return Err(c::bad());
        }
        for (i, e) in cited.iter().enumerate() {
            if !es.contains(e) || cited[..i].contains(e) {
                return Err(c::bad());
            }
        }
    }
    Ok(obj! {"status"=>"unverified","notes"=>notes.clone()})
}
pub(crate) fn fixture(v: &Value) -> Result<Value> {
    c::fields(v, &["op", "args"])?;
    let a = get(v, "args")?;
    match text(v, "op")? {
        "plan" => deliberation_plan(a),
        "hypotheses" => {
            c::fields(a, &["notes", "evidence"])?;
            validate_hypotheses(get(a, "notes")?, get(a, "evidence")?)
        }
        _ => Err(c::bad()),
    }
}
