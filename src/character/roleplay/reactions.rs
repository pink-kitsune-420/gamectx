//! Deterministic host capability gates for arbitrary registered reaction handlers.
//! Selection and authorization perform no mutation; hosts atomically execute once.
use crate::{checked as c, obj, Result, Value};
const LEVELS: [&str; 6] = ["trust", "affinity", "resentment", "generosity", "prudence", "courage"];
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(c::get(v, k)?)
}
fn name(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 64 || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:-".contains(&b)) {
        return Err(c::bad());
    }
    Ok(())
}
fn integer(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    let Value::Number(s) = v else {
        return Err(c::bad());
    };
    let n = s.parse::<i64>().map_err(|_| c::bad())?;
    if !(lo..=hi).contains(&n) || s.contains(['.', 'e', 'E']) {
        return Err(c::bad());
    }
    Ok(n)
}
fn resource(v: &Value) -> Result<()> {
    if v == &Value::Null {
        return Ok(());
    }
    c::fields(v, &["kind", "id", "amount"])?;
    if !matches!(text(v, "kind")?, "coins" | "item") {
        return Err(c::bad());
    }
    name(text(v, "id")?)?;
    c::uint(c::get(v, "amount")?, 1, if text(v, "kind")? == "item" { 1 } else { 1_000_000 })?;
    Ok(())
}
#[derive(Clone, Debug)]
pub struct ReactionPolicy {
    value: Value,
}
impl ReactionPolicy {
    pub fn from_value(v: &Value) -> Result<Self> {
        if !v.is_valid(128 * 1024) {
            return Err(c::bad());
        }
        c::fields(v, &["version", "enabled", "rules"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        c::boolean(c::get(v, "enabled")?)?;
        let rules = c::list(c::get(v, "rules")?)?;
        if rules.len() > 64 {
            return Err(c::bad());
        }
        for (i, r) in rules.iter().enumerate() {
            c::fields(r, &["id", "trigger", "handler", "reason", "gates", "resource", "priority"])?;
            for k in ["id", "trigger", "handler"] {
                name(text(r, k)?)?;
            }
            if rules[..i].iter().any(|o| o.get("id") == r.get("id"))
                || !matches!(text(r, "reason")?, "request" | "gratitude" | "emergency" | "threat")
            {
                return Err(c::bad());
            }
            resource(c::get(r, "resource")?)?;
            c::uint(c::get(r, "priority")?, 0, 100)?;
            let gates = c::list(c::get(r, "gates")?)?;
            if gates.len() > 6 {
                return Err(c::bad());
            }
            for (j, g) in gates.iter().enumerate() {
                c::fields(g, &["key", "min", "max"])?;
                if !LEVELS.contains(&text(g, "key")?) || gates[..j].iter().any(|o| o.get("key") == g.get("key")) {
                    return Err(c::bad());
                }
                let min = integer(c::get(g, "min")?, -100, 100)?;
                integer(c::get(g, "max")?, min, 100)?;
            }
        }
        Ok(Self { value: v.clone() })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    /// `frame` is authoritative host state, never a model-provided tool argument.
    /// Only `allowed` belongs in prompts; `withheld` is host-only diagnostics.
    pub fn select(&self, frame: &Value) -> Result<Value> {
        validate_frame(frame)?;
        let mut allowed = Vec::new();
        let mut withheld = Vec::new();
        for r in c::list(c::get(&self.value, "rules")?)? {
            if r.get("trigger") != frame.get("trigger") || r.get("resource") != frame.get("resource") {
                continue;
            }
            let list_has = |k: &str, s: &str| -> Result<bool> {
                Ok(c::list(c::get(frame, k)?)?.iter().any(|v| v.as_str() == Some(s)))
            };
            let mut gate_failed = false;
            for g in c::list(c::get(r, "gates")?)? {
                let n = integer(c::get(c::get(frame, "levels")?, text(g, "key")?)?, -100, 100)?;
                if n < integer(c::get(g, "min")?, -100, 100)? || n > integer(c::get(g, "max")?, -100, 100)? {
                    gate_failed = true;
                }
            }
            let item = c::get(r, "resource")?;
            let present = item == &Value::Null
                || c::list(c::get(frame, "resources")?)?.iter().any(|v| {
                    v.get("kind") == item.get("kind")
                        && v.get("id") == item.get("id")
                        && v.get("amount").and_then(Value::as_usize) >= item.get("amount").and_then(Value::as_usize)
                });
            let reason = if self.value.get("enabled") != Some(&true.into()) {
                Some("disabled")
            } else if c::uint(c::get(frame, "social_left")?, 0, 8)? == 0 {
                Some("social_limit")
            } else if list_has("used", text(r, "id")?)? {
                Some("already_used")
            } else if !list_has("handlers", text(r, "handler")?)? {
                Some("unregistered_handler")
            } else if gate_failed {
                Some("attitude_or_personality")
            } else if !list_has("reasons", text(r, "reason")?)? {
                Some("no_host_reason")
            } else if text(r, "reason")? == "threat" && !real_threat(frame)? {
                Some("unconfirmed_threat")
            } else if !present {
                Some("unavailable_resource")
            } else {
                None
            };
            if let Some(reason) = reason {
                withheld.push(obj! {"id"=>c::get(r,"id")?.clone(),"reason"=>reason});
            } else {
                allowed.push(obj!{"id"=>c::get(r,"id")?.clone(),"handler"=>c::get(r,"handler")?.clone(),"resource"=>item.clone(),"reason"=>c::get(r,"reason")?.clone(),"revision"=>c::get(frame,"revision")?.clone(),"priority"=>c::get(r,"priority")?.clone()});
            }
        }
        allowed.sort_by_key(|v| std::cmp::Reverse(v.get("priority").and_then(Value::as_usize).unwrap()));
        Ok(obj! {"allowed"=>allowed,"withheld"=>withheld})
    }
    /// Re-run gates immediately before an atomic host transaction; reject stale proposals.
    /// Hosts must record successful IDs in `used` and advance their revision afterward.
    pub fn authorize(&self, frame: &Value, proposal: &Value) -> Result<Value> {
        if !proposal.is_valid(1024) {
            return Err(c::bad());
        }
        c::fields(proposal, &["id", "revision"])?;
        name(text(proposal, "id")?)?;
        c::uint(c::get(proposal, "revision")?, 0, u32::MAX as u64)?;
        if proposal.get("revision") != frame.get("revision") {
            return Err(c::bad());
        }
        let selected = self.select(frame)?;
        c::list(c::get(&selected, "allowed")?)?
            .iter()
            .find(|r| r.get("id") == proposal.get("id"))
            .cloned()
            .ok_or_else(c::bad)
    }
}
fn validate_frame(v: &Value) -> Result<()> {
    if !v.is_valid(64 * 1024) {
        return Err(c::bad());
    }
    c::fields(
        v,
        &[
            "actor",
            "listener",
            "revision",
            "trigger",
            "resource",
            "levels",
            "resources",
            "reasons",
            "threat",
            "handlers",
            "used",
            "social_left",
        ],
    )?;
    for k in ["actor", "listener", "trigger"] {
        name(text(v, k)?)?;
    }
    if v.get("actor") == v.get("listener") {
        return Err(c::bad());
    }
    c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)?;
    c::uint(c::get(v, "social_left")?, 0, 8)?;
    resource(c::get(v, "resource")?)?;
    let levels = c::get(v, "levels")?;
    c::fields(levels, &LEVELS)?;
    for (k_index, k) in LEVELS.iter().enumerate() {
        integer(c::get(levels, k)?, if k_index < 3 { -100 } else { 0 }, 100)?;
    }
    let resources = c::list(c::get(v, "resources")?)?;
    if resources.len() > 64 {
        return Err(c::bad());
    }
    for (i, r) in resources.iter().enumerate() {
        if r == &Value::Null {
            return Err(c::bad());
        }
        resource(r)?;
        if resources[..i].iter().any(|o| o.get("kind") == r.get("kind") && o.get("id") == r.get("id")) {
            return Err(c::bad());
        }
    }
    for k in ["handlers", "used", "reasons"] {
        let a = c::list(c::get(v, k)?)?;
        if a.len() > 64 {
            return Err(c::bad());
        }
        for (i, s) in a.iter().enumerate() {
            name(c::text(s)?)?;
            if a[..i].contains(s) {
                return Err(c::bad());
            }
        }
    }
    let t = c::get(v, "threat")?;
    if t != &Value::Null {
        c::fields(t, &["event", "revision", "source", "capable", "imminent", "same_scene"])?;
        name(text(t, "event")?)?;
        c::uint(c::get(t, "revision")?, 0, u32::MAX as u64)?;
        if !matches!(text(t, "source")?, "perceived" | "report" | "claim") {
            return Err(c::bad());
        }
        for k in ["capable", "imminent", "same_scene"] {
            c::boolean(c::get(t, k)?)?;
        }
    }
    Ok(())
}
fn real_threat(v: &Value) -> Result<bool> {
    let t = c::get(v, "threat")?;
    Ok(t != &Value::Null
        && t.get("revision") == v.get("revision")
        && t.get("source").and_then(Value::as_str) == Some("perceived")
        && ["capable", "imminent", "same_scene"].iter().all(|k| t.get(k) == Some(&Value::Bool(true))))
}
pub(crate) fn fixture(v: &Value) -> Result<Value> {
    c::fields(v, &["policy", "frame", "proposal"])?;
    let p = ReactionPolicy::from_value(c::get(v, "policy")?)?;
    let f = c::get(v, "frame")?;
    let mut selected = p.select(f)?;
    let proposal = c::get(v, "proposal")?;
    if proposal != &Value::Null {
        if let Value::Object(xs) = &mut selected {
            xs.push(("authorized".into(), p.authorize(f, proposal)?));
        }
    }
    Ok(selected)
}
