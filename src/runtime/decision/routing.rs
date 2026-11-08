use super::*;

/// Composes host-produced matches in rule/procedure/brain order. `exact` is a host
/// assertion, not a model confidence score. Candidates are drafts, never actions.
/// Ambiguous/conflicting matches at a selected stage go to the LLM. The caller must
/// still bind intent, actor/privacy and atomically recheck execution permissions.
pub fn route(
    choices: &Choices,
    revision: u32,
    candidates: &Value,
    mut available: impl FnMut(&Value) -> bool,
) -> Result<Value> {
    bounded(candidates, 65_536)?;
    let rows = c::list(candidates)?;
    if rows.len() > 32 {
        return Err(c::bad());
    }
    let mut ids = Vec::new();
    for r in rows {
        c::fields(r, &["id", "source", "match", "decision"])?;
        let id = c::text(c::get(r, "id")?)?;
        c::name(id)?;
        if ids.contains(&id) {
            return Err(c::bad());
        }
        ids.push(id);
        if !["rule", "procedure", "brain"].contains(&c::text(c::get(r, "source")?)?)
            || !["exact", "ambiguous", "none"].contains(&c::text(c::get(r, "match")?)?)
        {
            return Err(c::bad());
        }
        let decision = c::get(r, "decision")?;
        if *decision != Value::Null {
            pair(decision)?;
        } else if c::text(c::get(r, "match")?)? == "exact" {
            return Err(c::bad());
        }
    }
    let result = |route: &str, reason: &str, decision: Value, trace: Vec<Value>| obj! {"route"=>route,"reason"=>reason,"decision"=>decision,"trace"=>trace};
    if revision != choices.revision {
        return Ok(result("llm", "stale_revision", Value::Null, Vec::new()));
    }
    let mut trace = Vec::new();
    for r in rows {
        let status = match c::text(c::get(r, "match")?)? {
            "none" => "unmatched",
            "ambiguous" => "ambiguous",
            _ => {
                let p = c::get(r, "decision")?;
                if choices.validate(p).is_err() {
                    "invalid"
                } else if !available(p) {
                    "unavailable"
                } else {
                    "eligible"
                }
            }
        };
        trace.push(obj! {"id"=>c::get(r,"id")?.clone(),"source"=>c::get(r,"source")?.clone(),"status"=>status});
    }
    for source in ["rule", "procedure", "brain"] {
        let indices = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.get("source").and_then(Value::as_str) == Some(source))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if indices.iter().any(|&i| trace[i].get("status").and_then(Value::as_str) == Some("ambiguous")) {
            return Ok(result("llm", "ambiguous", Value::Null, trace));
        }
        let eligible = indices
            .into_iter()
            .filter(|&i| trace[i].get("status").and_then(Value::as_str) == Some("eligible"))
            .collect::<Vec<_>>();
        if let Some(&first) = eligible.first() {
            let p = c::get(&rows[first], "decision")?;
            if eligible.iter().any(|&i| !rows[i].get("decision").expect("decision").same(p)) {
                return Ok(result("llm", "conflict", Value::Null, trace));
            }
            return Ok(result(source, "host_match", p.clone(), trace));
        }
    }
    Ok(result("llm", "no_match", Value::Null, trace))
}
