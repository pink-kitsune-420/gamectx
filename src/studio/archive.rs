//! Pure bounded FIFO for optional host voice recordings; never stores audio itself.
use crate::{checked as c, obj, Result, Value};
#[derive(Clone, Debug)]
pub struct VoiceArchive {
    config: Value,
    entries: Vec<Value>,
    bytes: u64,
}
impl VoiceArchive {
    pub fn new(config: &Value) -> Result<Self> {
        validate(config)?;
        Ok(Self { config: config.clone(), entries: Vec::new(), bytes: 0 })
    }
    pub fn config(&self) -> Value {
        self.config.clone()
    }
    pub fn configure(&mut self, config: &Value) -> Result<Value> {
        validate(config)?;
        self.config = config.clone();
        Ok(self.trim(Vec::new()))
    }
    pub fn remember(&mut self, v: &Value) -> Result<Value> {
        c::fields(v, &["key", "bytes"])?;
        let key = c::text(c::get(v, "key")?)?;
        let bytes = c::uint(c::get(v, "bytes")?, 1, 6 * 1024 * 1024)?;
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return Err(c::bad());
        }
        if let Some(old) = self.entries.iter().find(|r| r.get("key") == v.get("key")) {
            if old.get("bytes") != v.get("bytes") {
                return Err(c::bad());
            }
        } else if self.config.get("enabled") == Some(&true.into()) {
            self.entries.push(v.clone());
            self.bytes += bytes;
        }
        Ok(self.trim(Vec::new()))
    }
    pub fn clear(&mut self) -> Value {
        let removed = self.entries.iter().map(|r| r.get("key").unwrap().clone()).collect();
        self.entries.clear();
        self.bytes = 0;
        self.trim(removed)
    }
    fn trim(&mut self, mut removed: Vec<Value>) -> Value {
        while !self.entries.is_empty()
            && (self.config.get("enabled") != Some(&true.into())
                || self.entries.len() > self.config.get("limit").and_then(Value::as_usize).unwrap()
                || self.bytes > 32 * 1024 * 1024)
        {
            let r = self.entries.remove(0);
            self.bytes -= r.get("bytes").unwrap().to_json().parse::<u64>().unwrap();
            removed.push(r.get("key").unwrap().clone());
        }
        obj! {"kept"=>self.entries.clone(),"removed"=>removed,"bytes"=>self.bytes}
    }
}
fn validate(v: &Value) -> Result<()> {
    if !v.is_valid(1024) {
        return Err(c::bad());
    }
    c::fields(v, &["version", "enabled", "limit"])?;
    c::uint(c::get(v, "version")?, 1, 1)?;
    c::boolean(c::get(v, "enabled")?)?;
    c::uint(c::get(v, "limit")?, 1, 128)?;
    Ok(())
}
pub(crate) fn fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["config", "ops"])?;
    let mut archive = VoiceArchive::new(c::get(a, "config")?)?;
    let ops = c::list(c::get(a, "ops")?)?;
    if ops.len() > 256 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let args = c::get(op, "args")?;
        results.push(match c::text(c::get(op, "op")?)? {
            "remember" => archive.remember(args)?,
            "configure" => archive.configure(args)?,
            "clear" => {
                c::fields(args, &[])?;
                archive.clear()
            }
            _ => return Err(c::bad()),
        });
    }
    Ok(obj! {"results"=>results,"config"=>archive.config()})
}
