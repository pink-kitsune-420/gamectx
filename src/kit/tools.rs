use super::*;
use crate::{Error, Result, Value};

/// Maximum serialized model arguments and callback result, independently.
pub const MAX_CALL_BYTES: usize = 65_536;
type Handler<'a> = Box<dyn FnMut(&Value) -> Result<Value> + 'a>;
/// Local callback dispatcher. Each callback must check current permissions and entity
/// availability before changing the world. Validation succeeds before any callback runs.
pub struct Dispatcher<'a> {
    project: &'a Project,
    handlers: Vec<(String, Handler<'a>)>,
}
pub(super) fn failure(code: &str, message: &str) -> Value {
    obj! {"ok" => false, "error" => obj! {"code" => code, "message" => message}}
}
impl Project {
    /// Validates a model's tool arguments, without dispatch or world mutations.
    pub fn validate_call(&self, name: &str, arguments: &Value) -> Result<Value> {
        bounded(arguments, MAX_CALL_BYTES)?;
        let t = named(array(&self.definition, "tools")?, name)?;
        validate(c::get(t, "parameters")?, arguments)?;
        Ok(obj! {"name" => name, "arguments" => arguments.clone()})
    }
    /// Provider-neutral schemas for just this recipe's allowed tools. Optional arguments
    /// are supported, so provider strict mode is explicitly false.
    pub fn tools(&self, recipe: &str) -> Result<Value> {
        let r = self.recipe(recipe)?;
        Ok(r.stable
            .iter()
            .find(|s| s.section().name == "tools")
            .map(|s| s.section().value.clone())
            .unwrap_or(Value::Array(Vec::new())))
    }
    pub fn dispatcher(&self) -> Dispatcher<'_> {
        Dispatcher { project: self, handlers: Vec::new() }
    }
}
impl<'a> Dispatcher<'a> {
    /// Registers one handler for an already declared tool; duplicates are rejected.
    pub fn register(&mut self, name: &str, handler: impl FnMut(&Value) -> Result<Value> + 'a) -> Result<&mut Self> {
        named(array(&self.project.definition, "tools")?, name)?;
        if self.handlers.iter().any(|(n, _)| n == name) {
            return Err(Error::new("tool handler already registered"));
        }
        self.handlers.push((name.into(), Box::new(handler)));
        Ok(self)
    }
    /// Returns a consistent success/error envelope. A recipe is mandatory so registering
    /// a tool globally does not authorize it in every decision context. No automatic retry.
    pub fn dispatch(&mut self, recipe: &str, name: &str, arguments: &Value) -> Value {
        let allowed = self.project.recipe(recipe).and_then(|r| strings(c::get(&r.definition, "tools")?, 32));
        if !allowed.is_ok_and(|ns| ns.contains(&name)) {
            return failure("unavailable_tool", "tool is not available in this recipe");
        }
        if let Err(e) = self.project.validate_call(name, arguments) {
            return failure("invalid_arguments", &e.to_string());
        }
        let Some((_, handler)) = self.handlers.iter_mut().find(|(n, _)| n == name) else {
            return failure("missing_handler", "host has not registered a handler");
        };
        match handler(arguments) {
            Ok(result) => {
                if bounded(&result, MAX_CALL_BYTES).is_err() {
                    failure("result_limit", "callback result is invalid or exceeds 65536 bytes")
                } else {
                    obj! {"ok" => true, "result" => result}
                }
            }
            Err(e) => {
                // Host errors may contain private data: details stay in host logs.
                let _ = e;
                failure("host_error", "host rejected the action")
            }
        }
    }
}
