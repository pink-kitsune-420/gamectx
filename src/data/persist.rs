//! Durable state: canonical, versioned saves for host-owned managers (pass 5, design `O`).
//!
//! Every stateful pass 2–4 manager can `export` its whole state and `import` it back. A save
//! is one JSON object with sorted keys and integers only:
//!
//! ```text
//! {"format":"gamectx-state","kind":"<local kind>","limits":{…},"state":{…},"version":<n>}
//! ```
//!
//! `limits` records the host limits the save was made under, sorted by name. Inside `state`,
//! struct fields are sorted by name; maps are arrays of `[key, value]` pairs in ascending key
//! order; sets are ascending arrays; an absent optional value is `null`; tuples are arrays.
//!
//! A save is untrusted input, exactly like model output. `import` checks the whole document
//! before it replaces anything:
//! - the compact size (at most [`MAX_SAVE_BYTES`]) and the nesting depth of the JSON parser;
//! - the envelope: format, kind, a known version and limits within their ceilings;
//! - every field's type, enum word and numeric range, with strings of at most
//!   [`MAX_TEXT`] bytes and integers within ±[`MAX_INT`];
//! - canonical form: the parsed state must export to exactly the same document, so unsorted
//!   maps, duplicate set members and unknown or missing fields are refused;
//! - the manager's own invariants (counts within limit ceilings, references that exist,
//!   times not after the clock and so on), through [`Durable::settle`].
//!
//! A refused import changes nothing. An accepted import replaces the state but never the
//! limits: saves cannot raise host limits. A host that wants the saved limits reads them from
//! the save and applies them itself. Older versions are upgraded by [`Durable::migrate`], one
//! pure step per version.

use crate::character::limits::{Limit, Limits};
use crate::checked as c;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The `format` of a single manager save.
pub const FORMAT: &str = "gamectx-state";
/// Largest save accepted by `import` (compact JSON bytes).
pub const MAX_SAVE_BYTES: usize = 67_108_864;
/// Longest string inside a save (bytes).
pub const MAX_TEXT: usize = 4096;
/// Largest integer magnitude inside a save (the JSON safe-integer limit).
pub const MAX_INT: u64 = 9_007_199_254_740_991;

pub(crate) fn need(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(c::bad())
    }
}

/// An identifier as the ops accept it.
pub(crate) fn is_ident(s: &str) -> bool {
    crate::knowledge::props::ident(&Value::str(s)).is_ok()
}

/// A text of 1–`n` characters without control characters, as the ops accept it.
pub(crate) fn is_text(s: &str, n: usize) -> bool {
    crate::character::beliefs::text_bound(&Value::str(s), n).is_ok()
}

/// Strictly ascending (sorted and distinct).
pub(crate) fn ascending<T: Ord>(xs: &[T]) -> bool {
    xs.windows(2).all(|w| w[0] < w[1])
}

/// A value with one canonical save form.
pub trait Persist: Sized {
    fn save(&self) -> Value;
    fn load(v: &Value) -> Result<Self>;
}

impl Persist for u64 {
    fn save(&self) -> Value {
        Value::from(*self)
    }
    fn load(v: &Value) -> Result<Self> {
        c::uint(v, 0, MAX_INT)
    }
}

impl Persist for usize {
    fn save(&self) -> Value {
        Value::from(*self)
    }
    fn load(v: &Value) -> Result<Self> {
        Ok(c::uint(v, 0, MAX_INT)? as usize)
    }
}

impl Persist for i64 {
    fn save(&self) -> Value {
        Value::from(*self)
    }
    fn load(v: &Value) -> Result<Self> {
        let Value::Number(s) = v else { return Err(c::bad()) };
        let digits = s.strip_prefix('-').unwrap_or(s);
        need(
            !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit())
                && (digits.len() == 1 || !digits.starts_with('0'))
                && s != "-0",
        )?;
        let n = s.parse::<i64>().map_err(|_| c::bad())?;
        need(n.unsigned_abs() <= MAX_INT)?;
        Ok(n)
    }
}

impl Persist for bool {
    fn save(&self) -> Value {
        Value::Bool(*self)
    }
    fn load(v: &Value) -> Result<Self> {
        c::boolean(v)
    }
}

impl Persist for String {
    fn save(&self) -> Value {
        Value::str(self.as_str())
    }
    fn load(v: &Value) -> Result<Self> {
        let s = c::text(v)?;
        need(s.len() <= MAX_TEXT)?;
        Ok(s.to_string())
    }
}

impl Persist for Value {
    fn save(&self) -> Value {
        self.clone()
    }
    fn load(v: &Value) -> Result<Self> {
        need(v.is_valid(MAX_SAVE_BYTES) && texts_within(v))?;
        Ok(v.clone())
    }
}

/// Every string and object key inside `v` is at most [`MAX_TEXT`] bytes.
fn texts_within(v: &Value) -> bool {
    match v {
        Value::String(s) => s.len() <= MAX_TEXT,
        Value::Array(a) => a.iter().all(texts_within),
        Value::Object(o) => o.iter().all(|(k, x)| k.len() <= MAX_TEXT && texts_within(x)),
        _ => true,
    }
}

impl<T: Persist> Persist for Option<T> {
    fn save(&self) -> Value {
        self.as_ref().map_or(Value::Null, Persist::save)
    }
    fn load(v: &Value) -> Result<Self> {
        match v {
            Value::Null => Ok(None),
            x => T::load(x).map(Some),
        }
    }
}

impl<T: Persist> Persist for Box<T> {
    fn save(&self) -> Value {
        (**self).save()
    }
    fn load(v: &Value) -> Result<Self> {
        T::load(v).map(Box::new)
    }
}

fn rows(v: &Value) -> Result<&[Value]> {
    c::list(v)
}

impl<T: Persist> Persist for Vec<T> {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(Persist::save).collect())
    }
    fn load(v: &Value) -> Result<Self> {
        rows(v)?.iter().map(T::load).collect()
    }
}

impl<T: Persist> Persist for VecDeque<T> {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(Persist::save).collect())
    }
    fn load(v: &Value) -> Result<Self> {
        rows(v)?.iter().map(T::load).collect()
    }
}

impl<T: Persist, const N: usize> Persist for [T; N] {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(Persist::save).collect())
    }
    fn load(v: &Value) -> Result<Self> {
        let xs: Vec<T> = Persist::load(v)?;
        xs.try_into().map_err(|_| c::bad())
    }
}

/// Sets save as ascending arrays (duplicates or disorder fail the canonical check).
impl<T: Persist + Ord> Persist for BTreeSet<T> {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(Persist::save).collect())
    }
    fn load(v: &Value) -> Result<Self> {
        rows(v)?.iter().map(T::load).collect()
    }
}

/// Maps save as arrays of `[key, value]` pairs in ascending key order.
impl<K: Persist + Ord, V: Persist> Persist for BTreeMap<K, V> {
    fn save(&self) -> Value {
        Value::Array(self.iter().map(|(k, v)| Value::Array(vec![k.save(), v.save()])).collect())
    }
    fn load(v: &Value) -> Result<Self> {
        rows(v)?.iter().map(<(K, V)>::load).collect()
    }
}

impl<A: Persist, B: Persist> Persist for (A, B) {
    fn save(&self) -> Value {
        Value::Array(vec![self.0.save(), self.1.save()])
    }
    fn load(v: &Value) -> Result<Self> {
        match rows(v)? {
            [a, b] => Ok((A::load(a)?, B::load(b)?)),
            _ => Err(c::bad()),
        }
    }
}

impl<A: Persist, B: Persist, C: Persist> Persist for (A, B, C) {
    fn save(&self) -> Value {
        Value::Array(vec![self.0.save(), self.1.save(), self.2.save()])
    }
    fn load(v: &Value) -> Result<Self> {
        match rows(v)? {
            [a, b, x] => Ok((A::load(a)?, B::load(b)?, C::load(x)?)),
            _ => Err(c::bad()),
        }
    }
}

/// A field whose words come from a closed list (`&'static str` and containers of it).
pub trait Pick: Sized {
    fn save_in(&self) -> Value;
    fn load_in(v: &Value, list: &[&'static str]) -> Result<Self>;
}

impl Pick for &'static str {
    fn save_in(&self) -> Value {
        Value::str(*self)
    }
    fn load_in(v: &Value, list: &[&'static str]) -> Result<Self> {
        let s = c::text(v)?;
        list.iter().copied().find(|x| *x == s).ok_or_else(c::bad)
    }
}

impl<T: Pick> Pick for Option<T> {
    fn save_in(&self) -> Value {
        self.as_ref().map_or(Value::Null, Pick::save_in)
    }
    fn load_in(v: &Value, list: &[&'static str]) -> Result<Self> {
        match v {
            Value::Null => Ok(None),
            x => T::load_in(x, list).map(Some),
        }
    }
}

impl<T: Pick> Pick for Vec<T> {
    fn save_in(&self) -> Value {
        Value::Array(self.iter().map(Pick::save_in).collect())
    }
    fn load_in(v: &Value, list: &[&'static str]) -> Result<Self> {
        rows(v)?.iter().map(|x| T::load_in(x, list)).collect()
    }
}

/// Loads a closed-list field.
pub fn pick<T: Pick>(v: &Value, list: &[&'static str]) -> Result<T> {
    T::load_in(v, list)
}

/// A numeric field (or container of numbers) with an inclusive range.
pub trait Ranged {
    fn within(&self, lo: i128, hi: i128) -> bool;
}

impl Ranged for u64 {
    fn within(&self, lo: i128, hi: i128) -> bool {
        (lo..=hi).contains(&(*self as i128))
    }
}

impl Ranged for usize {
    fn within(&self, lo: i128, hi: i128) -> bool {
        (lo..=hi).contains(&(*self as i128))
    }
}

impl Ranged for i64 {
    fn within(&self, lo: i128, hi: i128) -> bool {
        (lo..=hi).contains(&(*self as i128))
    }
}

impl<T: Ranged> Ranged for Option<T> {
    fn within(&self, lo: i128, hi: i128) -> bool {
        self.as_ref().is_none_or(|x| x.within(lo, hi))
    }
}

impl<T: Ranged> Ranged for Vec<T> {
    fn within(&self, lo: i128, hi: i128) -> bool {
        self.iter().all(|x| x.within(lo, hi))
    }
}

impl<T: Ranged, const N: usize> Ranged for [T; N] {
    fn within(&self, lo: i128, hi: i128) -> bool {
        self.iter().all(|x| x.within(lo, hi))
    }
}

/// Loads a numeric field and checks its range.
pub fn ranged<T: Persist + Ranged>(v: &Value, lo: i128, hi: i128) -> Result<T> {
    let x = T::load(v)?;
    need(x.within(lo, hi))?;
    Ok(x)
}

/// An object with its fields sorted by name.
pub fn object(mut fields: Vec<(&str, Value)>) -> Value {
    fields.sort_by(|a, b| a.0.cmp(b.0));
    Value::Object(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// Implements [`Persist`] for a struct, field by field.
///
/// `persist!(Name { a, b in WORDS, c [0, 100] })`: `a` uses its own `Persist`, `b` must be
/// one of `WORDS` (see [`Pick`]), `c` must lie in `0..=100` (see [`Ranged`]). Every field must
/// be listed, so adding a field without a save form fails to compile. `persist!(rest Name
/// {…})` saves only the listed fields and fills the rest (limits, derived indexes) from
/// `Default`; such managers rebuild derived fields in [`Durable::settle`].
macro_rules! persist {
    ($t:ident { $($f:ident $(in $l:path)? $([$lo:expr, $hi:expr])?),* $(,)? }) => {
        impl $crate::data::persist::Persist for $t {
            fn save(&self) -> $crate::Value {
                $crate::data::persist::object(vec![
                    $((stringify!($f), $crate::data::persist::persist!(@save self.$f; $($l)?)),)*
                ])
            }
            fn load(v: &$crate::Value) -> $crate::Result<Self> {
                $crate::checked::fields(v, &[$(stringify!($f)),*])?;
                Ok($t {
                    $($f: $crate::data::persist::persist!(@load $crate::checked::get(v, stringify!($f))?; $($l)?; $($lo, $hi)?),)*
                })
            }
        }
    };
    (rest $t:ident { $($f:ident $(in $l:path)? $([$lo:expr, $hi:expr])?),* $(,)? }) => {
        impl $crate::data::persist::Persist for $t {
            fn save(&self) -> $crate::Value {
                $crate::data::persist::object(vec![
                    $((stringify!($f), $crate::data::persist::persist!(@save self.$f; $($l)?)),)*
                ])
            }
            fn load(v: &$crate::Value) -> $crate::Result<Self> {
                $crate::checked::fields(v, &[$(stringify!($f)),*])?;
                Ok($t {
                    $($f: $crate::data::persist::persist!(@load $crate::checked::get(v, stringify!($f))?; $($l)?; $($lo, $hi)?),)*
                    ..Default::default()
                })
            }
        }
    };
    (@save $e:expr;) => { $crate::data::persist::Persist::save(&$e) };
    (@save $e:expr; $l:path) => { $crate::data::persist::Pick::save_in(&$e) };
    (@load $v:expr; ;) => { $crate::data::persist::Persist::load($v)? };
    (@load $v:expr; $l:path;) => { $crate::data::persist::pick($v, &$l)? };
    (@load $v:expr; ; $lo:expr, $hi:expr) => { $crate::data::persist::ranged($v, ($lo) as i128, ($hi) as i128)? };
}
pub(crate) use persist;

/// A manager with a versioned save (see the [module docs](self)).
pub trait Durable: Persist + Default {
    /// The local kind, also the save's `kind`.
    const KIND: &'static str;
    /// The current save version.
    const VERSION: u64 = 1;
    /// The manager's limit spec.
    const SPEC: &'static [Limit];
    fn limits(&self) -> &Limits;
    fn limits_mut(&mut self) -> &mut Limits;
    /// Rebuilds derived indexes and checks every invariant the ops enforce.
    fn settle(&mut self) -> Result<()>;
    /// Upgrades a `state` from `from` to `from + 1`.
    fn migrate(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }
    /// Runs one workflow request on this manager.
    fn run(&mut self, request: &Value) -> Result<Value>;
}

fn limits_view(spec: &[Limit], l: &Limits) -> Value {
    object(spec.iter().map(|x| (x.name, Value::from(l.get(x.name)))).collect())
}

/// The save of `m` under `limits` (the limits in force for the current request).
pub fn export<T: Durable>(m: &T, limits: &Limits) -> Value {
    obj! {
        "format" => FORMAT, "kind" => T::KIND, "limits" => limits_view(T::SPEC, limits), "state" => m.save(),
        "version" => T::VERSION,
    }
}

/// Validates saved limits (every name, sorted, within ceilings) and returns them.
pub fn saved_limits(spec: &'static [Limit], v: &Value) -> Result<Limits> {
    let rows = v.as_object().ok_or_else(c::bad)?;
    let mut names: Vec<&str> = spec.iter().map(|x| x.name).collect();
    names.sort_unstable();
    need(rows.len() == names.len() && rows.iter().zip(&names).all(|((k, _), n)| k == n))?;
    let mut l = Limits::defaults(spec);
    l.apply(v)?;
    Ok(l)
}

/// Checks a save and returns the settled manager it holds, with `limits` in force, plus the
/// version it was written at. Nothing outside is touched.
pub fn restore<T: Durable>(save: &Value, limits: &Limits) -> Result<(T, u64, Value)> {
    need(save.is_valid(MAX_SAVE_BYTES))?;
    exact_fields(save, &["format", "kind", "limits", "state", "version"])?;
    need(c::text(c::get(save, "format")?)? == FORMAT && c::text(c::get(save, "kind")?)? == T::KIND)?;
    let version = c::uint(c::get(save, "version")?, 1, T::VERSION)?;
    let saved = saved_limits(T::SPEC, c::get(save, "limits")?)?;
    let mut state = c::get(save, "state")?.clone();
    for v in version..T::VERSION {
        state = T::migrate(v, state)?;
        need(state.is_valid(MAX_SAVE_BYTES))?;
    }
    let mut m = T::load(&state)?;
    need(m.save() == state)?;
    m.settle()?;
    *m.limits_mut() = limits.clone();
    Ok((m, version, limits_view(T::SPEC, &saved)))
}

/// A migration step: `state` (an object) with a new field `name` added at its sorted place.
/// Refuses a state that is not an object or already has the field.
pub fn with_field(state: Value, name: &str, value: Value) -> Result<Value> {
    let Value::Object(mut fields) = state else { return Err(c::bad()) };
    let at = fields.partition_point(|(k, _)| k.as_str() < name);
    need(fields.get(at).is_none_or(|(k, _)| k != name))?;
    fields.insert(at, (name.to_string(), value));
    Ok(Value::Object(fields))
}

/// Checks that `v` is an object with exactly `names`, in that (sorted) order.
pub fn exact_fields(v: &Value, names: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(c::bad)?;
    need(rows.len() == names.len() && rows.iter().zip(names).all(|((k, _), n)| k == n))
}

/// Replaces `m` with the manager in `args.save` (`{"save": …}`); its limits stay.
pub fn import<T: Durable>(m: &mut T, limits: &Limits, args: &Value) -> Result<Value> {
    crate::knowledge::codex::keys(args, &["save"], &[])?;
    let (fresh, from, saved) = restore::<T>(c::get(args, "save")?, limits)?;
    *m = fresh;
    Ok(obj! { "from_version" => from, "kind" => T::KIND, "limits" => saved, "version" => T::VERSION })
}

/// The `export` op: no arguments.
pub fn export_op<T: Durable>(m: &T, limits: &Limits, args: &Value) -> Result<Value> {
    crate::knowledge::codex::keys(args, &[], &[])?;
    Ok(export(m, limits))
}

/// Counts and ceilings: refuses `n` above the ceiling of limit `name`.
pub fn within<T: Durable>(name: &str, n: usize) -> Result<()> {
    let l = T::SPEC.iter().find(|x| x.name == name).ok_or_else(c::bad)?;
    need(n <= l.ceiling)
}

/// Implements the boilerplate of [`Durable`] for a manager whose `limits` field holds (or
/// derefs to) a [`Limits`] and whose `workflow(&mut self, &Value)` runs requests.
macro_rules! durable {
    ($t:ty, $kind:expr, $spec:expr) => {
        durable!($t, $kind, $spec, 1);
    };
    ($t:ty, $kind:expr, $spec:expr, $version:expr) => {
        impl $crate::data::persist::Durable for $t {
            const KIND: &'static str = $kind;
            const VERSION: u64 = $version;
            const SPEC: &'static [$crate::character::limits::Limit] = $spec;
            fn limits(&self) -> &$crate::character::limits::Limits {
                &self.limits
            }
            fn limits_mut(&mut self) -> &mut $crate::character::limits::Limits {
                &mut self.limits
            }
            fn settle(&mut self) -> $crate::Result<()> {
                self.settle_state()
            }
            fn migrate(from: u64, state: $crate::Value) -> $crate::Result<$crate::Value> {
                Self::migrate_state(from, state)
            }
            fn run(&mut self, request: &$crate::Value) -> $crate::Result<$crate::Value> {
                self.workflow(request)
            }
        }
    };
}
pub(crate) use durable;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    #[derive(Clone, Debug, PartialEq, Default)]
    struct Row {
        a: u64,
        b: Option<&'static str>,
        c: BTreeMap<String, (i64, bool)>,
    }
    const XY: [&str; 2] = ["x", "y"];
    persist!(Row { c, b in XY, a [0, 10] });

    #[test]
    fn rows_round_trip_and_refuse_non_canonical_forms() {
        let mut r = Row { a: 3, b: Some("y"), ..Default::default() };
        r.c.insert("k".into(), (-4, true));
        let v = r.save();
        assert_eq!(v.to_json(), r#"{"a":3,"b":"y","c":[["k",[-4,true]]]}"#);
        assert_eq!(Row::load(&v).unwrap(), r);
        for bad in [
            r#"{"a":11,"b":null,"c":[]}"#,
            r#"{"a":1,"b":"z","c":[]}"#,
            r#"{"a":1,"b":null}"#,
            r#"{"a":1,"b":null,"c":[],"d":1}"#,
            r#"{"a":1,"b":null,"c":[["k",[-0,true]]]}"#,
            r#"{"a":1,"b":null,"c":[["k",[1.5,true]]]}"#,
            r#"{"a":01,"b":null,"c":[]}"#,
        ] {
            let v = parse(bad, 4096);
            assert!(v.is_err() || Row::load(&v.unwrap()).is_err(), "{bad}");
        }
        // Disorder parses but does not survive the canonical check.
        let v = parse(r#"{"a":1,"b":null,"c":[["z",[1,true]],["k",[1,true]]]}"#, 4096).unwrap();
        assert_ne!(Row::load(&v).unwrap().save(), v);
    }

    #[test]
    fn integers_stay_within_the_safe_range() {
        assert!(i64::load(&parse("9007199254740991", 64).unwrap()).is_ok());
        assert!(i64::load(&parse("-9007199254740992", 64).unwrap()).is_err());
        assert!(u64::load(&parse("9007199254740992", 64).unwrap()).is_err());
        assert!(String::load(&Value::str("x".repeat(MAX_TEXT + 1))).is_err());
    }

    #[test]
    fn raw_values_keep_the_text_limit() {
        let long = "x".repeat(MAX_TEXT + 1);
        assert!(Value::load(&parse(&format!(r#"[{{"a":"{}"}}]"#, "x".repeat(MAX_TEXT)), 1 << 16).unwrap()).is_ok());
        assert!(Value::load(&parse(&format!(r#"[{{"a":"{long}"}}]"#), 1 << 16).unwrap()).is_err());
        assert!(Value::load(&parse(&format!(r#"{{"{long}":1}}"#), 1 << 16).unwrap()).is_err());
    }
}
