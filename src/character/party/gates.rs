use super::*;
/// Optional sidecar rules; existing lore/facet/affordance formats are unchanged.
/// All clauses and all active listeners must pass. Missing/ambiguous attitudes deny.
#[derive(Clone, Debug)]
pub struct AttitudeGates {
    rules: Value,
}
impl AttitudeGates {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 65536)?;
        let rows = c::list(v)?;
        if rows.len() > 128 {
            return Err(c::bad());
        }
        for (i, r) in rows.iter().enumerate() {
            c::fields(r, &["kind", "id", "clauses"])?;
            if !matches!(text(r, "kind")?, "lore" | "facet" | "affordance")
                || rows[..i].iter().any(|o| o.get("kind") == r.get("kind") && o.get("id") == r.get("id"))
            {
                return Err(c::bad());
            }
            c::scope(text(r, "id")?)?;
            let clauses = c::list(get(r, "clauses")?)?;
            if clauses.is_empty() || clauses.len() > 8 {
                return Err(c::bad());
            }
            for q in clauses {
                c::fields(q, &["attitude", "target", "op", "value"])?;
                c::name(text(q, "attitude")?)?;
                if !matches!(text(q, "target")?, "speaker" | "listener") || !matches!(text(q, "op")?, "ge" | "le") {
                    return Err(c::bad());
                }
                signed(get(q, "value")?, 100)?;
            }
        }
        Ok(Self { rules: v.clone() })
    }
    /// Evaluate against current host Life, rechecking revision and fixed participant
    /// scope. `frame` is exactly revision/speaker/listeners. Never send withheld IDs or
    /// threshold diagnostics to the model; only the selected packet is model-facing.
    pub fn select(
        &self,
        life: &crate::character::life::Life,
        frame: &Value,
        kind: &str,
        items: &Value,
    ) -> Result<Value> {
        let attitudes = life.gate_attitudes(frame)?;
        select_fixture(
            &obj! {"rules"=>self.rules.clone(),"actor"=>life.actor.clone(),"speaker"=>get(frame,"speaker")?.clone(),"listeners"=>get(frame,"listeners")?.clone(),"participants"=>life.participants.clone(),"revision"=>life.revision(),"expected_revision"=>get(frame,"revision")?.clone(),"attitudes"=>attitudes,"kind"=>kind,"items"=>items.clone()},
        )
    }
}
/// Independent offline fixture only: callers supply compact host attitude observations.
/// Runtime selection uses `AttitudeGates::select` with real Life state instead.
pub fn select_fixture(a: &Value) -> Result<Value> {
    bounded(a, 262144)?;
    c::fields(
        a,
        &[
            "rules",
            "actor",
            "speaker",
            "listeners",
            "participants",
            "revision",
            "expected_revision",
            "attitudes",
            "kind",
            "items",
        ],
    )?;
    let gates = AttitudeGates::from_value(get(a, "rules")?)?;
    if num(a, "revision")? != num(a, "expected_revision")? {
        return Err(c::bad());
    }
    let people = names(get(a, "participants")?, 16)?;
    let listeners = names(get(a, "listeners")?, 16)?;
    if !people.iter().any(|p| p == text(a, "actor").unwrap_or(""))
        || !people.iter().any(|p| p == text(a, "speaker").unwrap_or(""))
        || listeners.is_empty()
        || listeners.iter().any(|p| !people.contains(p))
    {
        return Err(c::bad());
    }
    let kind = text(a, "kind")?;
    if !matches!(kind, "lore" | "facet" | "affordance") {
        return Err(c::bad());
    }
    let attitudes = c::list(get(a, "attitudes")?)?;
    if attitudes.len() > 64 {
        return Err(c::bad());
    }
    for a in attitudes {
        c::fields(a, &["kind", "subject", "level"])?;
        c::scope(text(a, "kind")?)?;
        c::scope(text(a, "subject")?)?;
        signed(get(a, "level")?, 100)?;
    }
    let items = c::list(get(a, "items")?)?;
    if items.len() > 512 {
        return Err(c::bad());
    }
    let mut packet = Vec::new();
    let mut withheld = Vec::new();
    for (i, item) in items.iter().enumerate() {
        c::scope(text(item, "id")?)?;
        if items[..i].iter().any(|o| o.get("id") == item.get("id")) {
            return Err(c::bad());
        }
        let mut reason = None;
        for rule in c::list(&gates.rules)? {
            if text(rule, "kind")? != kind || rule.get("id") != item.get("id") {
                continue;
            }
            for q in c::list(get(rule, "clauses")?)? {
                let subjects = if text(q, "target")? == "speaker" {
                    vec![text(a, "speaker")?.to_owned()]
                } else {
                    listeners.clone()
                };
                for subject in subjects {
                    let matches = attitudes
                        .iter()
                        .filter(|x| {
                            x.get("kind") == q.get("attitude")
                                && x.get("subject").and_then(Value::as_str) == Some(subject.as_str())
                        })
                        .collect::<Vec<_>>();
                    if matches.len() != 1 {
                        reason = Some(if matches.is_empty() { "attitude_missing" } else { "attitude_ambiguous" });
                        break;
                    }
                    let level = signed(get(matches[0], "level")?, 100)?;
                    let threshold = signed(get(q, "value")?, 100)?;
                    if !(if text(q, "op")? == "ge" { level >= threshold } else { level <= threshold }) {
                        reason = Some("attitude_threshold");
                        break;
                    }
                }
                if reason.is_some() {
                    break;
                }
            }
        }
        if let Some(reason) = reason {
            withheld.push(obj! {"kind"=>kind,"id"=>get(item,"id")?.clone(),"reason"=>reason});
        } else {
            packet.push(item.clone());
        }
    }
    let refusal = if withheld.is_empty() { Value::Null } else { Value::str("I cannot share that or offer it yet.") };
    Ok(obj! {"packet"=>packet,"withheld"=>withheld,"refusal"=>refusal})
}
