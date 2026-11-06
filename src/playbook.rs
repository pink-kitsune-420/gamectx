//! Host-validated reusable procedures. Matching returns drafts, never executes actions.
use crate::{
    checked::*,
    facts::{ClaimStatus, FactRef, FactRegistry},
    obj, Result, Value,
};
#[derive(Clone, Debug)]
struct Recipe {
    id: String,
    action: String,
    args: Value,
    requires: Vec<(FactRef, Value)>,
    expires: Option<u64>,
}
#[derive(Clone, Debug)]
pub struct Playbook {
    recipes: Vec<Recipe>,
}
impl Playbook {
    /// Registration is a host operation. Validate the action schema before storing it;
    /// the current validator passed to `select` is checked again at every use.
    pub fn from_value(value: &Value) -> Result<Self> {
        let rows = list(value)?;
        if rows.len() > 64 {
            return Err(bad());
        }
        let mut recipes = Vec::<Recipe>::new();
        for r in rows {
            fields(r, &["id", "action", "args", "requires", "expires"])?;
            let id = text(get(r, "id")?)?;
            let action = text(get(r, "action")?)?;
            name(id)?;
            name(action)?;
            if recipes.iter().any(|r| r.id == id) {
                return Err(bad());
            }
            let args = get(r, "args")?;
            crate::facts::validate_value(args)?;
            if args.as_object().is_none() {
                return Err(bad());
            }
            let raw = list(get(r, "requires")?)?;
            if raw.is_empty() || raw.len() > 8 {
                return Err(bad());
            }
            let mut requires = Vec::<(FactRef, Value)>::new();
            for p in raw {
                fields(p, &["reference", "value"])?;
                let reference = FactRef::from_value(get(p, "reference")?)?;
                let value = get(p, "value")?;
                crate::facts::validate_value(value)?;
                if requires.iter().any(|(r, _)| r.scope == reference.scope && r.key == reference.key) {
                    return Err(bad());
                }
                requires.push((reference, value.clone()));
            }
            let expiry = get(r, "expires")?;
            let expires = if expiry == &Value::Null { None } else { Some(uint(expiry, 1, u32::MAX as u64)?) };
            recipes.push(Recipe { id: id.into(), action: action.into(), args: args.clone(), requires, expires });
        }
        if value.json_len() > 524288 {
            return Err(bad());
        }
        recipes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self { recipes })
    }
    pub fn from_json(text: &str) -> Result<Self> {
        Self::from_value(&crate::json::parse(text, 524288)?)
    }
    pub fn select(
        &self,
        facts: &FactRegistry,
        now: u64,
        denied: &[String],
        mut validate: impl FnMut(&str, &Value) -> bool,
    ) -> Result<Value> {
        // Validate time/privacy even when no recipe needs lookup.
        facts.validate_read(now, denied)?;
        let mut hits = Vec::new();
        for r in &self.recipes {
            if r.expires.is_some_and(|t| now >= t) {
                continue;
            }
            let mut valid = true;
            for (reference, value) in &r.requires {
                let check = facts.check(&reference.scope, &reference.key, value, now, denied)?;
                if check.status != ClaimStatus::Supported || check.reference.as_ref() != Some(reference) {
                    valid = false;
                    break;
                }
            }
            if valid && validate(&r.action, &r.args) {
                hits.push(obj! {"id"=>r.id.as_str(),"action"=>r.action.as_str(),"args"=>r.args.clone()})
            }
        }
        Ok(Value::Array(hits))
    }
    pub fn to_value(&self) -> Value {
        Value::Array(self.recipes.iter().map(|r|obj!{"id"=>r.id.as_str(),"action"=>r.action.as_str(),"args"=>r.args.clone(),"requires"=>r.requires.iter().map(|(reference,value)|obj!{"reference"=>reference.to_value(),"value"=>value.clone()}).collect::<Vec<_>>(),"expires"=>r.expires.map_or(Value::Null,Value::from)}).collect())
    }
}
