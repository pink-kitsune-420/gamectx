//! Explicit host/user fact registration and structural claim checking. Matching a
//! registered assertion is not independent verification of its truth. Model text has
//! no registration capability here; retain unverified ideas in evidence memory.
use crate::{
    json::{parse, Error, Result, Value},
    obj,
};
pub const MAX_FACTS: usize = 256;
pub const MAX_STATE_BYTES: usize = 4_194_304;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Authority {
    #[default]
    Host,
    User,
}
impl Authority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::User => "user",
        }
    }
    fn parse(v: &Value) -> Result<Self> {
        match v.as_str() {
            Some("host") => Ok(Self::Host),
            Some("user") => Ok(Self::User),
            _ => Err(bad("fact authority")),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactRef {
    pub scope: String,
    pub key: String,
    pub revision: u64,
}
impl FactRef {
    pub fn to_value(&self) -> Value {
        obj! {"scope"=>self.scope.as_str(),"key"=>self.key.as_str(),"revision"=>self.revision}
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        fields(v, &["scope", "key", "revision"])?;
        Ok(Self {
            scope: name(get(v, "scope")?)?.into(),
            key: name(get(v, "key")?)?.into(),
            revision: uint(get(v, "revision")?, 1, 2_147_483_647)?,
        })
    }
}
#[derive(Clone, Debug, Default)]
pub struct FactOptions {
    pub now: u64,
    pub authority: Authority,
    pub pinned: bool,
    pub expires: Option<u64>,
    pub privacy: Vec<String>,
    pub dependencies: Vec<FactRef>,
    pub expected_revision: Option<u64>,
}
#[derive(Clone, Debug)]
pub struct Fact {
    pub reference: FactRef,
    pub value: Value,
    pub authority: Authority,
    pub pinned: bool,
    pub expires: Option<u64>,
    pub privacy: Vec<String>,
    pub dependencies: Vec<FactRef>,
}
impl Fact {
    fn to_value(&self) -> Value {
        obj! {"scope"=>self.reference.scope.as_str(),"key"=>self.reference.key.as_str(),"revision"=>self.reference.revision,"value"=>self.value.clone(),"authority"=>self.authority.as_str(),"pinned"=>self.pinned,"expires"=>self.expires.map_or(Value::Null,Value::from),"privacy"=>Value::strings(&self.privacy),"dependencies"=>self.dependencies.iter().map(FactRef::to_value).collect::<Vec<_>>()}
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimStatus {
    Supported,
    Conflicting,
    Unknown,
}
#[derive(Clone, Debug)]
pub struct ClaimCheck {
    pub status: ClaimStatus,
    pub reference: Option<FactRef>,
}
impl ClaimCheck {
    pub fn to_value(&self) -> Value {
        obj! {"status"=>match self.status{ClaimStatus::Supported=>"supported",ClaimStatus::Conflicting=>"conflicting",ClaimStatus::Unknown=>"unknown"},"reference"=>self.reference.as_ref().map_or(Value::Null,FactRef::to_value)}
    }
}
#[derive(Clone, Debug)]
pub struct FactRegistry {
    capacity: usize,
    generation: u64,
    clock: u64,
    facts: Vec<Fact>,
}
fn bad(s: &str) -> Error {
    Error::new(s)
}
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(|| bad("missing fact field"))
}
fn fields(v: &Value, names: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(|| bad("fact object required"))?;
    if rows.len() != names.len()
        || names.iter().any(|k| v.get(k).is_none())
        || rows
            .iter()
            .enumerate()
            .any(|(i, (k, _))| !names.contains(&k.as_str()) || rows[..i].iter().any(|(old, _)| old == k))
    {
        return Err(bad("unexpected or missing fact fields"));
    }
    Ok(())
}
fn name_str(s: &str) -> Result<&str> {
    if s.is_empty() || s.len() > 256 || s.chars().count() > 64 || s.chars().any(char::is_control) {
        return Err(bad("fact name must be 1..64 non-control characters"));
    }
    Ok(s)
}
fn name(v: &Value) -> Result<&str> {
    name_str(v.as_str().ok_or_else(|| bad("fact name required"))?)
}

fn uint(v: &Value, low: u64, high: u64) -> Result<u64> {
    match v {
        Value::Number(s) if s.bytes().all(|b| b.is_ascii_digit()) && (s.len() == 1 || !s.starts_with('0')) => {
            s.parse::<u64>().ok().filter(|v| (low..=high).contains(v)).ok_or_else(|| bad("fact integer limit"))
        }
        _ => Err(bad("fact integer required")),
    }
}
fn privacy(values: &[String], limit: usize) -> Result<()> {
    if values.len() > limit {
        return Err(bad("fact privacy limit"));
    }
    for (i, s) in values.iter().enumerate() {
        if s.is_empty()
            || s.len() > 32
            || !s.as_bytes()[0].is_ascii_lowercase()
            || !s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            || values[..i].contains(s)
        {
            return Err(bad("invalid fact privacy label"));
        }
    }
    Ok(())
}
fn decimal(s: &str) -> Result<(bool, String, i64)> {
    if s.len() > 128 {
        return Err(bad("fact numeric text limit"));
    }
    if !matches!(parse(s, 128)?, Value::Number(n) if n == s) {
        return Err(bad("fact number grammar"));
    }
    let negative = s.starts_with('-');
    let s = s.trim_start_matches('-');
    let (mantissa, exponent) = s.split_once(['e', 'E']).unwrap_or((s, "0"));
    let exponent = exponent
        .parse::<i64>()
        .ok()
        .filter(|n| (-1_000_000..=1_000_000).contains(n))
        .ok_or_else(|| bad("fact numeric exponent limit"))?;
    let fraction = mantissa.split_once('.').map_or(0, |(_, f)| f.len()) as i64;
    let digits = mantissa.replace('.', "");
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Ok((false, "0".into(), 0));
    }
    let coefficient = significant.trim_end_matches('0');
    let power = exponent - fraction + (significant.len() - coefficient.len()) as i64;
    Ok((negative, coefficient.into(), power))
}
fn validate_value(v: &Value) -> Result<()> {
    fn visit(v: &Value, depth: usize, nodes: &mut usize) -> Result<()> {
        *nodes += 1;
        if depth > 8 || *nodes > 256 {
            return Err(bad("fact value depth/node limit"));
        }
        match v {
            Value::Number(s) => {
                decimal(s)?;
            }
            Value::String(s) if s.len() > 4096 => return Err(bad("fact string limit")),
            Value::Array(a) => {
                if a.len() > 64 {
                    return Err(bad("fact array limit"));
                }
                for v in a {
                    visit(v, depth + 1, nodes)?;
                }
            }
            Value::Object(o) => {
                if o.len() > 64 {
                    return Err(bad("fact object limit"));
                }
                for (i, (k, v)) in o.iter().enumerate() {
                    if k.len() > 256 || o[..i].iter().any(|(old, _)| old == k) {
                        return Err(bad("fact key limit/duplicate"));
                    }
                    visit(v, depth + 1, nodes)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(v, 0, &mut 0)?;
    if v.json_len() > 4096 {
        return Err(bad("fact value exceeds 4096 bytes"));
    }
    Ok(())
}
fn equivalent(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => decimal(a).ok() == decimal(b).ok(),
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b)),
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(k, a)| b.iter().find(|(old, _)| old == k).is_some_and(|(_, b)| equivalent(a, b)))
        }
        _ => a == b,
    }
}
impl FactRegistry {
    pub fn new(capacity: usize) -> Result<Self> {
        if !(1..=MAX_FACTS).contains(&capacity) {
            return Err(bad("fact capacity must be 1..256"));
        }
        Ok(Self { capacity, generation: 0, clock: 0, facts: Vec::new() })
    }
    fn find(&self, scope: &str, key: &str) -> Option<usize> {
        self.facts.iter().position(|f| f.reference.scope == scope && f.reference.key == key)
    }
    fn time(&self, now: u64) -> Result<()> {
        if now < self.clock || now > u32::MAX as u64 {
            return Err(bad("fact clock moved backward or exceeded bounds"));
        }
        Ok(())
    }
    fn active(&self, index: usize, now: u64, denied: &[String], cache: &mut [u8; MAX_FACTS]) -> bool {
        if cache[index] != 0 {
            return cache[index] == 2;
        }
        cache[index] = 1;
        let f = &self.facts[index];
        let active = f.expires.is_none_or(|t| now < t)
            && !f.privacy.iter().any(|p| denied.contains(p))
            && f.dependencies.iter().all(|r| {
                self.find(&r.scope, &r.key).is_some_and(|i| {
                    self.facts[i].reference.revision == r.revision && self.active(i, now, denied, cache)
                })
            });
        cache[index] = if active { 2 } else { 3 };
        active
    }
    fn advance(&mut self) -> Result<()> {
        if self.generation >= 2_147_483_647 {
            return Err(bad("fact generations exhausted"));
        }
        self.generation += 1;
        Ok(())
    }
    /// Explicit trusted registration. New keys require no expected revision; replacements
    /// require the exact current revision. User assertions outrank host updates; pinned
    /// values require explicit unpinning. Dependencies are exact versioned references.
    pub fn register(
        &mut self,
        scope: &str,
        key: &str,
        value: impl Into<Value>,
        options: FactOptions,
    ) -> Result<FactRef> {
        name_str(scope)?;
        name_str(key)?;
        self.time(options.now)?;
        privacy(&options.privacy, 8)?;
        let value = value.into();
        validate_value(&value)?;
        if options.expires.is_some_and(|t| t <= options.now || t > u32::MAX as u64) || options.dependencies.len() > 8 {
            return Err(bad("fact expiry/dependency limit"));
        }
        let old = self.find(scope, key);
        if let Some(i) = old {
            let f = &self.facts[i];
            if options.expected_revision != Some(f.reference.revision)
                || f.pinned
                || f.authority == Authority::User && options.authority != Authority::User
            {
                return Err(bad("pinned, user-owned or stale fact"));
            }
        } else if options.expected_revision.is_some() || self.facts.len() == self.capacity {
            return Err(bad("fact creation/capacity mismatch"));
        }
        let mut labels = options.privacy.clone();
        let mut cache = [0; MAX_FACTS];
        for (i, r) in options.dependencies.iter().enumerate() {
            if r.scope == scope && r.key == key
                || options.dependencies[..i].iter().any(|old| old.scope == r.scope && old.key == r.key)
            {
                return Err(bad("self/duplicate fact dependency"));
            }
            let dep = self
                .find(&r.scope, &r.key)
                .filter(|i| {
                    self.facts[*i].reference.revision == r.revision && self.active(*i, options.now, &[], &mut cache)
                })
                .ok_or_else(|| bad("fact dependency unavailable"))?;
            labels.extend(self.facts[dep].privacy.iter().cloned());
        }
        labels.sort();
        labels.dedup();
        privacy(&labels, 32)?;
        let mut next = self.clone();
        next.advance()?;
        next.clock = options.now;
        let reference = FactRef { scope: scope.into(), key: key.into(), revision: next.generation };
        if let Some(i) = old {
            next.facts.remove(i);
        }
        next.facts.push(Fact {
            reference: reference.clone(),
            value,
            authority: options.authority,
            pinned: options.pinned,
            expires: options.expires,
            privacy: labels,
            dependencies: options.dependencies,
        });
        if !next.active(next.facts.len() - 1, options.now, &[], &mut [0; MAX_FACTS]) {
            return Err(bad("fact replacement would invalidate its own basis"));
        }
        if next.to_value().json_len() > MAX_STATE_BYTES {
            return Err(bad("fact state size limit"));
        }
        *self = next;
        Ok(reference)
    }
    pub fn get(&self, scope: &str, key: &str, now: u64, denied: &[String]) -> Result<Option<&Fact>> {
        name_str(scope)?;
        name_str(key)?;
        self.time(now)?;
        privacy(denied, 32)?;
        Ok(self.find(scope, key).filter(|i| self.active(*i, now, denied, &mut [0; MAX_FACTS])).map(|i| &self.facts[i]))
    }
    /// Structural comparison only; does not verify arbitrary natural-language claims.
    /// Decimal numbers compare exactly without f64 rounding, and object order is ignored.
    pub fn check(&self, scope: &str, key: &str, value: &Value, now: u64, denied: &[String]) -> Result<ClaimCheck> {
        validate_value(value)?;
        Ok(match self.get(scope, key, now, denied)? {
            None => ClaimCheck { status: ClaimStatus::Unknown, reference: None },
            Some(f) => ClaimCheck {
                status: if equivalent(&f.value, value) { ClaimStatus::Supported } else { ClaimStatus::Conflicting },
                reference: Some(f.reference.clone()),
            },
        })
    }
    pub fn unpin(&mut self, reference: &FactRef, authority: Authority) -> Result<FactRef> {
        let i = self.find(&reference.scope, &reference.key).ok_or_else(|| bad("unknown fact"))?;
        let f = &self.facts[i];
        if &f.reference != reference || f.authority == Authority::User && authority != Authority::User {
            return Err(bad("unpin authority/revision"));
        }
        self.advance()?;
        self.facts[i].pinned = false;
        self.facts[i].reference.revision = self.generation;
        Ok(self.facts[i].reference.clone())
    }
    /// Ordinary deletion invalidates dependents through their stale/missing references.
    /// Privacy erasure below additionally removes dependent payloads physically.
    pub fn forget(&mut self, reference: &FactRef, authority: Authority) -> Result<()> {
        let i = self.find(&reference.scope, &reference.key).ok_or_else(|| bad("unknown fact"))?;
        let f = &self.facts[i];
        if &f.reference != reference || f.pinned || f.authority == Authority::User && authority != Authority::User {
            return Err(bad("forget authority/revision"));
        }
        self.advance()?;
        self.facts.remove(i);
        Ok(())
    }
    pub fn forget_privacy(&mut self, denied: &[String]) -> Result<()> {
        privacy(denied, 32)?;
        if self.facts.iter().any(|f| f.privacy.iter().any(|p| denied.contains(p))) {
            self.advance()?;
            self.facts.retain(|f| !f.privacy.iter().any(|p| denied.contains(p)));
        }
        Ok(())
    }
    /// Bounded table of current visible facts, pinned/user entries first. Dropped counts
    /// exclude hidden/expired/invalid records. No registration is performed by packing.
    pub fn packet(&self, now: u64, count: usize, max_bytes: usize, denied: &[String]) -> Result<Value> {
        self.time(now)?;
        privacy(denied, 32)?;
        if count > MAX_FACTS || !(1..=16_384).contains(&max_bytes) {
            return Err(bad("fact packet bounds"));
        }
        let mut cache = [0; MAX_FACTS];
        let mut selected: Vec<_> = self
            .facts
            .iter()
            .enumerate()
            .filter(|(i, _)| self.active(*i, now, denied, &mut cache))
            .map(|(_, f)| f)
            .collect();
        selected.sort_by_key(|f| (!f.pinned, f.authority != Authority::User, &f.reference.scope, &f.reference.key));
        let total = selected.len();
        selected.truncate(count);
        let columns = Value::strings(&["scope", "key", "value", "authority", "pinned", "revision", "expires"]);
        let mut rows: Vec<_> = selected
            .iter()
            .map(|f| {
                Value::Array(vec![
                    Value::str(&f.reference.scope),
                    Value::str(&f.reference.key),
                    f.value.clone(),
                    Value::str(f.authority.as_str()),
                    Value::Bool(f.pinned),
                    Value::from(f.reference.revision),
                    f.expires.map_or(Value::Null, Value::from),
                ])
            })
            .collect();
        let sizes: Vec<_> = rows.iter().map(Value::json_len).collect();
        let mut sum = sizes.iter().sum::<usize>();
        let base =
            (obj! {"generation"=>self.generation,"columns"=>columns.clone(),"rows"=>Vec::<Value>::new(),"dropped"=>0})
                .json_len();
        loop {
            let dropped = total - rows.len();
            let size = base + sum + rows.len().saturating_sub(1) + dropped.to_string().len() - 1;
            if size <= max_bytes {
                return Ok(obj! {"generation"=>self.generation,"columns"=>columns,"rows"=>rows,"dropped"=>dropped});
            }
            if rows.pop().is_none() {
                return Err(bad("fact packet exceeds byte budget"));
            }
            sum -= sizes[rows.len()];
        }
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1,"capacity"=>self.capacity,"generation"=>self.generation,"clock"=>self.clock,"facts"=>self.facts.iter().map(Fact::to_value).collect::<Vec<_>>()}
    }
    pub fn restore_json(text: &str) -> Result<Self> {
        Self::restore(&parse(text, MAX_STATE_BYTES)?)
    }
    pub fn restore(v: &Value) -> Result<Self> {
        fields(v, &["version", "capacity", "generation", "clock", "facts"])?;
        uint(get(v, "version")?, 1, 1)?;
        let mut out = Self::new(uint(get(v, "capacity")?, 1, MAX_FACTS as u64)? as usize)?;
        out.generation = uint(get(v, "generation")?, 0, 2_147_483_647)?;
        out.clock = uint(get(v, "clock")?, 0, u32::MAX as u64)?;
        let rows = get(v, "facts")?.as_array().ok_or_else(|| bad("fact array required"))?;
        if rows.len() > out.capacity {
            return Err(bad("fact capacity"));
        }
        for row in rows {
            fields(
                row,
                &["scope", "key", "revision", "value", "authority", "pinned", "expires", "privacy", "dependencies"],
            )?;
            let reference = FactRef {
                scope: name(get(row, "scope")?)?.into(),
                key: name(get(row, "key")?)?.into(),
                revision: uint(get(row, "revision")?, 1, out.generation)?,
            };
            if out.find(&reference.scope, &reference.key).is_some()
                || out.facts.iter().any(|f| f.reference.revision == reference.revision)
            {
                return Err(bad("duplicate saved fact identity/revision"));
            }
            let value = get(row, "value")?.clone();
            validate_value(&value)?;
            let labels = get(row, "privacy")?
                .as_array()
                .ok_or_else(|| bad("saved fact privacy"))?
                .iter()
                .map(|v| v.as_str().map(str::to_owned).ok_or_else(|| bad("privacy label")))
                .collect::<Result<Vec<_>>>()?;
            privacy(&labels, 32)?;
            let deps = get(row, "dependencies")?.as_array().ok_or_else(|| bad("saved dependencies"))?;
            if deps.len() > 8 {
                return Err(bad("saved dependency limit"));
            }
            let deps = deps.iter().map(FactRef::from_value).collect::<Result<Vec<_>>>()?;
            if deps.iter().enumerate().any(|(i, r)| {
                r.revision >= reference.revision
                    || r.scope == reference.scope && r.key == reference.key
                    || deps[..i].iter().any(|old| old.scope == r.scope && old.key == r.key)
            }) {
                return Err(bad("saved dependency order/duplicate"));
            }
            let expires = if get(row, "expires")? == &Value::Null {
                None
            } else {
                Some(uint(get(row, "expires")?, 1, u32::MAX as u64)?)
            };
            out.facts.push(Fact {
                reference,
                value,
                authority: Authority::parse(get(row, "authority")?)?,
                pinned: get(row, "pinned")?.as_bool().ok_or_else(|| bad("fact pinned boolean"))?,
                expires,
                privacy: labels,
                dependencies: deps,
            });
        }
        for fact in &out.facts {
            for reference in &fact.dependencies {
                if let Some(i) = out.find(&reference.scope, &reference.key) {
                    let dep = &out.facts[i];
                    if dep.reference.revision == reference.revision
                        && dep.privacy.iter().any(|label| !fact.privacy.contains(label))
                    {
                        return Err(bad("saved fact lost inherited privacy"));
                    }
                }
            }
        }
        if out.to_value().json_len() > MAX_STATE_BYTES {
            return Err(bad("saved fact size"));
        }
        Ok(out)
    }
}
