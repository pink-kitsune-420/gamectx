use super::*;
use crate::{data::json, Error};
use std::io::{BufRead, Write};
use std::path::Path;

/// A line can contain one complete definition and snapshot plus framing.
pub const MAX_REQUEST_BYTES: usize = 1_572_864;
impl Project {
    pub(super) fn execute(&self, op: &str, args: &Value) -> Result<Value> {
        match op {
            "build" => {
                c::fields(args, &["recipe", "snapshot"])?;
                let snapshot = self.snapshot(c::get(args, "snapshot")?)?;
                Ok(self.build(text(args, "recipe")?, &snapshot)?.to_value())
            }
            "query" => {
                c::fields(args, &["recipe", "snapshot", "sql"])?;
                let snapshot = self.snapshot(c::get(args, "snapshot")?)?;
                self.query(text(args, "recipe")?, &snapshot, text(args, "sql")?)
            }
            "validate" => {
                c::fields(args, &["name", "arguments"])?;
                self.validate_call(text(args, "name")?, c::get(args, "arguments")?)
            }
            "doctor" => {
                c::fields(args, &[])?;
                Ok(self.doctor())
            }
            "tools" => {
                c::fields(args, &["recipe"])?;
                self.tools(text(args, "recipe")?)
            }
            "features" => {
                c::fields(args, &["dataset", "revision", "policies"])?;
                let revision = c::uint(c::get(args, "revision")?, 1, 1_000_000)? as usize;
                Ok(self.feature_schema(text(args, "dataset")?, revision, c::get(args, "policies")?)?.to_value())
            }
            "local" => crate::kit::local::process(args),
            "evaluate" => evaluation::run(self, args),
            _ => Err(Error::new("unknown kit operation")),
        }
    }
    /// Version-1 bridge request. Invalid requests return an error envelope; no callbacks
    /// are reachable from the bridge. The caller owns external action dispatch.
    pub fn request(&self, input: &str) -> Value {
        request(input, |op, args| self.execute(op, args))
    }
}
pub(super) fn request(input: &str, mut execute: impl FnMut(&str, &Value) -> Result<Value>) -> Value {
    let mut id = Value::Null;
    let result = (|| {
        let v = json::parse(input, MAX_REQUEST_BYTES)?;
        c::fields(&v, &["version", "id", "op", "args"])?;
        c::uint(c::get(&v, "version")?, 1, 1)?;
        let key = text(&v, "id")?;
        if key.is_empty() || key.chars().count() > 64 {
            return Err(bad());
        }
        id = Value::str(key);
        execute(text(&v, "op")?, c::get(&v, "args")?)
    })();
    match result {
        Ok(result) => obj! {"version" => 1, "id" => id, "ok" => true, "result" => result},
        Err(e) => {
            obj! {"version" => 1, "id" => id, "ok" => false, "error" => obj! {"code" => "invalid_request", "message" => e.to_string()}}
        }
    }
}
/// Reads one bounded line and drains an oversized line in bounded chunks before replying.
/// This permits recovery on the following request without buffering the entire stream.
/// A line above twice the request cap terminates serving to bound recovery work.
pub fn serve(project: &Project, reader: &mut impl BufRead, writer: &mut impl Write) -> Result<()> {
    let session = super::Session::new(project.clone());
    loop {
        let mut line = Vec::new();
        let mut oversized = false;
        let mut consumed = 0usize;
        loop {
            let buffer = reader.fill_buf().map_err(|e| Error::new(e.to_string()))?;
            if buffer.is_empty() {
                break;
            }
            let end = buffer.iter().position(|b| *b == b'\n').map(|i| i + 1);
            let count = end.unwrap_or(buffer.len());
            consumed = consumed.saturating_add(count);
            if consumed > MAX_REQUEST_BYTES * 2 + 2 {
                return Err(Error::new("bridge line exceeds recovery limit"));
            }
            if line.len().saturating_add(count) > MAX_REQUEST_BYTES + 1 {
                oversized = true;
            }
            if !oversized {
                line.extend_from_slice(&buffer[..count]);
            }
            reader.consume(count);
            if end.is_some() {
                break;
            }
        }
        if line.is_empty() && !oversized {
            return Ok(());
        }
        let output = if oversized {
            session.request("")
        } else {
            match std::str::from_utf8(&line) {
                Ok(text) => session.request(text.trim_end_matches(['\n', '\r'])),
                Err(_) => session.request(""),
            }
        };
        writeln!(writer, "{}", output.to_json()).and_then(|_| writer.flush()).map_err(|e| Error::new(e.to_string()))?;
    }
}
/// Generates a runnable, self-contained starter directory. Existing destinations are
/// refused; if writing fails, only files created by this call are removed.
pub fn init(destination: &Path) -> Result<()> {
    std::fs::create_dir(destination).map_err(|e| Error::new(e.to_string()))?;
    let files = [
        ("project.json", include_str!("../../examples/kit/project.json")),
        ("snapshot.json", include_str!("../../examples/kit/snapshot.json")),
        ("suite.json", include_str!("../../examples/kit/suite.json")),
        ("README.txt", include_str!("../../examples/kit/README.txt")),
        ("check.py", include_str!("../../examples/kit/check.py")),
    ];
    let mut created = Vec::new();
    for (name, body) in files {
        let path = destination.join(name);
        let result = (|| {
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
            created.push(path.clone());
            file.write_all(body.as_bytes())
        })();
        if let Err(e) = result {
            for path in created {
                let _ = std::fs::remove_file(path);
            }
            let _ = std::fs::remove_dir(destination);
            return Err(Error::new(e.to_string()));
        }
    }
    Ok(())
}
