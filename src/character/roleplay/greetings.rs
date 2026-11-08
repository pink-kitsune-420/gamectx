//! Bounded mutual-encounter memory and context-sensitive greeting plans. No generation.
use crate::{checked as c, obj, Result, Value};
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    c::get(v, k)
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    c::text(get(v, k)?)
}
fn num(v: &Value, k: &str) -> Result<u64> {
    c::uint(get(v, k)?, 0, u32::MAX as u64)
}
fn ident(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 64 || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:-".contains(&b)) {
        return Err(c::bad());
    }
    Ok(())
}
fn pair(v: &Value) -> Result<()> {
    ident(text(v, "actor")?)?;
    ident(text(v, "listener")?)?;
    if v.get("actor") == v.get("listener") {
        return Err(c::bad());
    }
    Ok(())
}
fn set(v: &mut Value, k: &str, value: Value) {
    if let Value::Object(xs) = v {
        *xs.iter_mut().find(|(key, _)| key == k).unwrap() = (k.into(), value);
    }
}
fn config(v: &Value) -> Result<()> {
    if !v.is_valid(1024) {
        return Err(c::bad());
    }
    c::fields(v, &["version", "long_gap_turns"])?;
    c::uint(get(v, "version")?, 1, 1)?;
    c::uint(get(v, "long_gap_turns")?, 1, 1_000_000)?;
    Ok(())
}
fn receipt(v: &Value) -> Result<()> {
    if !v.is_valid(4096) {
        return Err(c::bad());
    }
    c::fields(
        v,
        &["actor", "listener", "scene", "visit", "at", "token", "greeting", "introduced", "text", "heard", "expected"],
    )?;
    pair(v)?;
    ident(text(v, "scene")?)?;
    num(v, "visit")?;
    num(v, "at")?;
    c::uint(get(v, "token")?, 1, u32::MAX as u64)?;
    let greeting = c::boolean(get(v, "greeting")?)?;
    let introduced = c::boolean(get(v, "introduced")?)?;
    if introduced && !greeting {
        return Err(c::bad());
    }
    let line = text(v, "text")?;
    if line.is_empty() || line.len() > 1024 || line.chars().count() > 240 || line.chars().any(char::is_control) {
        return Err(c::bad());
    }
    c::uint(get(v, "heard")?, 0, line.chars().count() as u64)?;
    if get(v, "expected")? != &Value::Null {
        c::uint(get(v, "expected")?, 1, u32::MAX as u64)?;
    }
    Ok(())
}
/// Host-confirmed, per-NPC/listener encounter ledger. Greetings are proposals until
/// a complete heard receipt is confirmed. No model/file/network/world actions.
#[derive(Clone, Debug)]
pub struct GreetingMemory {
    config: Value,
    entries: Vec<Value>,
    revision: u64,
}
impl GreetingMemory {
    pub fn new(v: &Value) -> Result<Self> {
        config(v)?;
        Ok(Self { config: v.clone(), entries: Vec::new(), revision: 0 })
    }
    pub fn configure(&mut self, v: &Value) -> Result<()> {
        config(v)?;
        self.config = v.clone();
        Ok(())
    }
    pub fn config(&self) -> Value {
        self.config.clone()
    }
    pub fn checkpoint(&self) -> Value {
        obj! {"version"=>1,"revision"=>self.revision,"entries"=>self.entries.clone()}
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn contact(&self, actor: &str, listener: &str) -> Option<&Value> {
        self.entries.iter().find(|e| {
            e.get("actor").and_then(Value::as_str) == Some(actor)
                && e.get("listener").and_then(Value::as_str) == Some(listener)
        })
    }
    pub fn recover(cfg: &Value, state: &Value) -> Result<Self> {
        if !state.is_valid(1024 * 1024) {
            return Err(c::bad());
        }
        c::fields(state, &["version", "revision", "entries"])?;
        c::uint(get(state, "version")?, 1, 1)?;
        let mut out = Self::new(cfg)?;
        out.revision = num(state, "revision")?;
        let entries = c::list(get(state, "entries")?)?;
        if entries.len() > 256 {
            return Err(c::bad());
        }
        for e in entries {
            c::fields(
                e,
                &[
                    "actor",
                    "listener",
                    "first_at",
                    "last_at",
                    "token",
                    "scene",
                    "visit",
                    "exchanges",
                    "greetings",
                    "introduced",
                    "legacy",
                    "receipt",
                ],
            )?;
            pair(e)?;
            if out.contact(text(e, "actor")?, text(e, "listener")?).is_some() {
                return Err(c::bad());
            }
            let first = num(e, "first_at")?;
            let last = num(e, "last_at")?;
            c::uint(get(e, "token")?, 1, u32::MAX as u64)?;
            let exchanges = c::uint(get(e, "exchanges")?, 1, u32::MAX as u64)?;
            let greetings = num(e, "greetings")?;
            let introduced = c::boolean(get(e, "introduced")?)?;
            let legacy = c::boolean(get(e, "legacy")?)?;
            if first > last || greetings > exchanges || !legacy && introduced && greetings == 0 {
                return Err(c::bad());
            }
            let r = get(e, "receipt")?;
            if r == &Value::Null {
                if !legacy
                    || get(e, "scene")? != &Value::Null
                    || get(e, "visit")? != &Value::Null
                    || exchanges != 1
                    || greetings != 0
                {
                    return Err(c::bad());
                }
            } else {
                receipt(r)?;
                if num(r, "heard")? != text(r, "text")?.chars().count() as u64
                    || ["actor", "listener", "scene", "visit", "token"].iter().any(|k| e.get(k) != r.get(k))
                    || last != num(r, "at")?
                    || c::boolean(get(r, "introduced")?)? && !introduced
                    || c::boolean(get(r, "greeting")?)? && greetings == 0
                {
                    return Err(c::bad());
                }
            }
            out.entries.push(e.clone());
        }
        Ok(out)
    }
    /// Pure plan from authoritative current, perceived scene and relationship state.
    /// `visit` is a host monotonic continuous-presence identifier, not a chat count.
    pub fn plan(&self, v: &Value) -> Result<Value> {
        if !v.is_valid(4096) {
            return Err(c::bad());
        }
        c::fields(v, &["actor", "listener", "scene", "visit", "at", "trust", "atmosphere", "place"])?;
        pair(v)?;
        ident(text(v, "scene")?)?;
        let visit = num(v, "visit")?;
        let at = num(v, "at")?;
        let Value::Number(s) = get(v, "trust")? else {
            return Err(c::bad());
        };
        let trust = s.parse::<i64>().map_err(|_| c::bad())?;
        if !(-100..=100).contains(&trust) {
            return Err(c::bad());
        }
        if !["calm", "busy", "tense", "emergency"].contains(&text(v, "atmosphere")?)
            || !["workplace", "social", "private", "other", "hazard"].contains(&text(v, "place")?)
        {
            return Err(c::bad());
        }
        let row = self.contact(text(v, "actor")?, text(v, "listener")?);
        let mut meeting = "first";
        if let Some(e) = row {
            if at < num(e, "last_at")? || get(e, "visit")? != &Value::Null && visit < num(e, "visit")? {
                return Err(c::bad());
            }
            meeting = if get(e, "scene")? == &Value::Null {
                "familiar"
            } else if e.get("scene") != v.get("scene") {
                "elsewhere"
            } else if num(e, "visit")? != visit {
                "returning"
            } else if at - num(e, "last_at")? >= num(&self.config, "long_gap_turns")? {
                "after_gap"
            } else {
                "continuing"
            };
        }
        Ok(
            obj! {"meeting"=>meeting,"needs_introduction"=>row.is_none_or(|e|e.get("introduced")!=Some(&Value::Bool(true))),"expected"=>row.and_then(|e|e.get("token")).cloned().unwrap_or(Value::Null),"relationship"=>if trust<0{"guarded"}else if trust>=25{"warm"}else{"reserved"},"atmosphere"=>get(v,"atmosphere")?.clone(),"place"=>get(v,"place")?.clone(),"scene"=>get(v,"scene")?.clone()},
        )
    }
    /// Import known prior contact from trusted legacy host receipts, without claiming
    /// a known location, visit or total lifetime encounter count. One seed per pair.
    pub fn seed(&mut self, v: &Value) -> Result<Value> {
        if !v.is_valid(1024) {
            return Err(c::bad());
        }
        c::fields(v, &["actor", "listener", "at", "token", "introduced"])?;
        pair(v)?;
        num(v, "at")?;
        c::uint(get(v, "token")?, 1, u32::MAX as u64)?;
        c::boolean(get(v, "introduced")?)?;
        if self.contact(text(v, "actor")?, text(v, "listener")?).is_some()
            || self.entries.len() >= 256
            || self.revision >= u32::MAX as u64
        {
            return Err(c::bad());
        }
        self.entries.push(obj!{"actor"=>get(v,"actor")?.clone(),"listener"=>get(v,"listener")?.clone(),"first_at"=>get(v,"at")?.clone(),"last_at"=>get(v,"at")?.clone(),"token"=>get(v,"token")?.clone(),"scene"=>Value::Null,"visit"=>Value::Null,"exchanges"=>1,"greetings"=>0,"introduced"=>get(v,"introduced")?.clone(),"legacy"=>true,"receipt"=>Value::Null});
        self.revision += 1;
        Ok(obj! {"recorded"=>true,"revision"=>self.revision})
    }
    /// Call only with a host-owned delivery receipt. Partial speech does not establish
    /// contact/introduction; duplicate identical receipts are harmless.
    pub fn confirm(&mut self, v: &Value) -> Result<Value> {
        receipt(v)?;
        if num(v, "heard")? != text(v, "text")?.chars().count() as u64 {
            return Ok(obj! {"recorded"=>false,"reason"=>"partial","revision"=>self.revision});
        }
        let old = self.contact(text(v, "actor")?, text(v, "listener")?);
        if old.is_some_and(|e| e.get("receipt") == Some(v)) {
            return Ok(obj! {"recorded"=>false,"reason"=>"duplicate","revision"=>self.revision});
        }
        let expected = old.and_then(|e| e.get("token")).unwrap_or(&Value::Null);
        if get(v, "expected")? != expected || self.revision >= u32::MAX as u64 {
            return Err(c::bad());
        }
        if let Some(e) = old {
            if num(v, "token")? <= num(e, "token")?
                || num(v, "at")? < num(e, "last_at")?
                || get(e, "visit")? != &Value::Null && num(v, "visit")? < num(e, "visit")?
                || num(e, "exchanges")? >= u32::MAX as u64
            {
                return Err(c::bad());
            }
        } else if self.entries.len() >= 256 {
            return Err(c::bad());
        }
        let mut next=old.cloned().unwrap_or_else(||obj!{"actor"=>get(v,"actor").unwrap().clone(),"listener"=>get(v,"listener").unwrap().clone(),"first_at"=>get(v,"at").unwrap().clone(),"last_at"=>get(v,"at").unwrap().clone(),"token"=>get(v,"token").unwrap().clone(),"scene"=>Value::Null,"visit"=>Value::Null,"exchanges"=>0,"greetings"=>0,"introduced"=>false,"legacy"=>false,"receipt"=>Value::Null});
        let exchanges = num(&next, "exchanges")? + 1;
        let greetings = num(&next, "greetings")? + u64::from(c::boolean(get(v, "greeting")?)?);
        let introduced = c::boolean(get(&next, "introduced")?)? || c::boolean(get(v, "introduced")?)?;
        for (k, value) in [
            ("last_at", get(v, "at")?.clone()),
            ("token", get(v, "token")?.clone()),
            ("scene", get(v, "scene")?.clone()),
            ("visit", get(v, "visit")?.clone()),
            ("exchanges", exchanges.into()),
            ("greetings", greetings.into()),
            ("introduced", introduced.into()),
            ("receipt", v.clone()),
        ] {
            set(&mut next, k, value);
        }
        if let Some(index) =
            self.entries.iter().position(|e| e.get("actor") == v.get("actor") && e.get("listener") == v.get("listener"))
        {
            self.entries[index] = next;
        } else {
            self.entries.push(next);
        }
        self.revision += 1;
        Ok(obj! {"recorded"=>true,"revision"=>self.revision})
    }
}
pub(crate) fn fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["config", "state", "ops"])?;
    let mut memory = GreetingMemory::recover(get(a, "config")?, get(a, "state")?)?;
    let ops = c::list(get(a, "ops")?)?;
    if ops.len() > 256 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let v = get(op, "args")?;
        results.push(match text(op, "op")? {
            "plan" => memory.plan(v)?,
            "confirm" => memory.confirm(v)?,
            "seed" => memory.seed(v)?,
            "configure" => {
                memory.configure(v)?;
                obj! {"configured"=>true}
            }
            _ => return Err(c::bad()),
        });
    }
    Ok(obj! {"results"=>results,"state"=>memory.checkpoint(),"config"=>memory.config()})
}
