//! Host-set capacity limits for character modules.
//!
//! Every module declares its limits as `(name, default, ceiling)`. The game decides how much
//! it needs: a workflow envelope may carry `limits` next to `ops`
//! (`{"limits": {"beings": 5000}, "ops": [...]}`), or the host calls a manager's
//! `with_limits`. Each value is an integer from 1 to the module's hard ceiling, which keeps
//! work and memory bounded. Limits only refuse growth: lowering one below current usage keeps
//! what exists and blocks new entries.
//!
//! Limits are host configuration. Never pass model output into `limits`.

use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::{Result, Value};

/// One configurable capacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limit {
    pub name: &'static str,
    pub default: usize,
    pub ceiling: usize,
}

/// Current values for a module's limits, in declaration order.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    spec: &'static [Limit],
    values: Vec<usize>,
}

impl Limits {
    pub fn defaults(spec: &'static [Limit]) -> Self {
        Limits { spec, values: spec.iter().map(|l| l.default).collect() }
    }

    /// The current value of `name` (a name the module declared).
    pub fn get(&self, name: &str) -> usize {
        self.spec.iter().position(|l| l.name == name).map(|i| self.values[i]).unwrap_or(0)
    }

    /// Applies `{name: value}`; refuses unknown names and values outside 1..=ceiling.
    /// Nothing changes when any entry is refused.
    pub fn apply(&mut self, v: &Value) -> Result<()> {
        let names: Vec<&str> = self.spec.iter().map(|l| l.name).collect();
        keys(v, &[], &names)?;
        let mut next = self.values.clone();
        for (i, l) in self.spec.iter().enumerate() {
            if let Some(x) = v.get(l.name).filter(|x| **x != Value::Null) {
                next[i] = c::uint(x, 1, l.ceiling as u64)? as usize;
            }
        }
        self.values = next;
        Ok(())
    }

    /// `{name: value}` in declaration order.
    pub fn view(&self) -> Value {
        Value::Object(self.spec.iter().zip(&self.values).map(|(l, v)| (l.name.to_string(), Value::from(*v))).collect())
    }
}

/// Runs a module workflow with host limits: `{ops: 1–64, limits?}` → `{limits?, results, version: 1}`
/// (`limits` echoed only when supplied).
pub fn run(
    request: &Value,
    limits: &mut Limits,
    f: impl FnMut(&str, &Value, &Limits) -> Result<Value>,
) -> Result<Value> {
    keys(request, &["ops"], &["limits"])?;
    let supplied = match request.get("limits").filter(|x| **x != Value::Null) {
        Some(v) => {
            limits.apply(v)?;
            true
        }
        None => false,
    };
    let mut f = f;
    let snapshot = limits.clone();
    let ops_only = Value::Object(vec![("ops".to_string(), c::get(request, "ops")?.clone())]);
    let out = crate::character::beliefs::run_ops(&ops_only, |op, args| f(op, args, &snapshot))?;
    if !supplied {
        return Ok(out);
    }
    let mut fields = vec![("limits".to_string(), limits.view())];
    if let Value::Object(rest) = out {
        fields.extend(rest);
    }
    Ok(Value::Object(fields))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    const SPEC: &[Limit] =
        &[Limit { name: "beings", default: 4, ceiling: 100 }, Limit { name: "items", default: 2, ceiling: 8 }];

    #[test]
    fn applies_within_ceiling_and_refuses_otherwise() {
        let mut l = Limits::defaults(SPEC);
        l.apply(&parse(r#"{"beings":100}"#, 64).unwrap()).unwrap();
        assert_eq!(l.get("beings"), 100);
        for bad in [r#"{"beings":0}"#, r#"{"beings":101}"#, r#"{"other":1}"#, r#"{"items":3,"beings":-1}"#] {
            assert!(l.apply(&parse(bad, 64).unwrap()).is_err(), "{bad}");
        }
        assert_eq!(l.get("items"), 2, "a refused apply changes nothing");
        assert_eq!(l.view().to_json(), r#"{"beings":100,"items":2}"#);
    }
}
