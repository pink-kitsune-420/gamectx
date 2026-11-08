//! Small `when` condition maps over named scalar variables.
//!
//! A condition map has at most 8 entries. Each entry is either a list of 1–16 allowed values
//! (strings up to 64 chars, booleans or integers within ±1e9) or a `{"min","max"}` integer
//! range. Keys are names the caller allows, or host flags written `state.<id>`. A missing
//! variable fails its condition; an empty map always holds.
use crate::{checked as c, Result, Value};

const LIMIT: i64 = 1_000_000_000;

/// A typed variable value.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Text(String),
    Flag(bool),
    Int(i64),
}
impl Scalar {
    /// A host flag value: a string (≤64 chars, no control characters), boolean or integer within ±1e9.
    pub fn from_flag(v: &Value) -> Result<Self> {
        match v {
            Value::String(s) if s.chars().count() <= 64 && !s.chars().any(char::is_control) => {
                Ok(Scalar::Text(s.clone()))
            }
            Value::Bool(b) => Ok(Scalar::Flag(*b)),
            Value::Number(_) => int(v).map(Scalar::Int).ok_or_else(c::bad),
            _ => Err(c::bad()),
        }
    }
}
pub(crate) fn int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| n.abs() <= LIMIT),
        _ => None,
    }
}

/// Named variables for conditions and templates (later `set` calls replace earlier ones).
#[derive(Clone, Debug, Default)]
pub struct Vars(Vec<(String, Scalar)>);
impl Vars {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, k: &str, v: Scalar) -> &mut Self {
        self.0.retain(|(old, _)| old != k);
        self.0.push((k.to_owned(), v));
        self
    }
    pub fn text(&mut self, k: &str, v: &str) -> &mut Self {
        self.set(k, Scalar::Text(v.to_owned()))
    }
    pub fn flag(&mut self, k: &str, v: bool) -> &mut Self {
        self.set(k, Scalar::Flag(v))
    }
    pub fn int(&mut self, k: &str, v: i64) -> &mut Self {
        self.set(k, Scalar::Int(v))
    }
    pub fn get(&self, k: &str) -> Option<&Scalar> {
        self.0.iter().find(|(old, _)| old == k).map(|(_, v)| v)
    }
}

#[derive(Clone, Debug)]
enum Cond {
    Any(Vec<Scalar>),
    Range(Option<i64>, Option<i64>),
}
/// A validated condition map.
#[derive(Clone, Debug, Default)]
pub struct Conditions(Vec<(String, Cond)>);

/// `state.<id>` with an identifier of 1–64 ASCII letters, digits, `_`, `:` or `-`.
pub fn is_flag_key(k: &str) -> bool {
    k.strip_prefix("state.").is_some_and(|id| {
        !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:-".contains(&b))
    })
}

impl Conditions {
    /// Parse a `when` map whose keys are in `known` or are `state.<id>` flags.
    pub fn from_value(v: &Value, known: &[&str]) -> Result<Self> {
        let rows = v.as_object().ok_or_else(c::bad)?;
        if rows.len() > 8 {
            return Err(c::bad());
        }
        let mut out = Vec::new();
        for (k, cond) in rows {
            if !known.contains(&k.as_str()) && !is_flag_key(k) {
                return Err(c::bad());
            }
            let parsed = match cond {
                Value::Object(bounds) => {
                    if bounds.is_empty() || bounds.iter().any(|(b, _)| b != "min" && b != "max") {
                        return Err(c::bad());
                    }
                    let bound = |name: &str| -> Result<Option<i64>> {
                        cond.get(name).map(|x| int(x).ok_or_else(c::bad)).transpose()
                    };
                    Cond::Range(bound("min")?, bound("max")?)
                }
                Value::Array(items) => {
                    if items.is_empty() || items.len() > 16 {
                        return Err(c::bad());
                    }
                    let mut xs = Vec::new();
                    for x in items {
                        xs.push(match x {
                            Value::Bool(b) => Scalar::Flag(*b),
                            Value::String(s) if s.chars().count() <= 64 => Scalar::Text(s.clone()),
                            Value::Number(_) => Scalar::Int(int(x).ok_or_else(c::bad)?),
                            _ => return Err(c::bad()),
                        });
                    }
                    Cond::Any(xs)
                }
                _ => return Err(c::bad()),
            };
            out.push((k.clone(), parsed));
        }
        Ok(Self(out))
    }
    /// An optional `when` field (absent means always).
    pub fn field(v: &Value, key: &str, known: &[&str]) -> Result<Self> {
        v.get(key).map_or(Ok(Self::default()), |w| Self::from_value(w, known))
    }
    pub fn holds(&self, vars: &Vars) -> bool {
        self.0.iter().all(|(k, cond)| match (vars.get(k), cond) {
            (None, _) => false,
            (Some(Scalar::Int(n)), Cond::Range(lo, hi)) => lo.is_none_or(|lo| *n >= lo) && hi.is_none_or(|hi| *n <= hi),
            (Some(_), Cond::Range(..)) => false,
            (Some(v), Cond::Any(xs)) => xs.contains(v),
        })
    }
}
