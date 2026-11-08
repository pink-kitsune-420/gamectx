//! Optional media layers: deterministic visual specs and asset registry, voice delivery and
//! provider backend routing. Nothing here generates images/audio or opens a network.
pub mod providers;
pub mod visual;
pub mod voice;
use crate::{checked as c, obj, Result, Value};
/// Runs a bounded list of `{op,args}` operations and records each outcome.
pub(crate) fn workflow(ops: &Value, mut run: impl FnMut(&str, &Value) -> Result<Value>) -> Result<Value> {
    let ops = c::list(ops)?;
    if ops.len() > 256 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        let result = (|| {
            c::fields(op, &["op", "args"])?;
            run(c::text(c::get(op, "op")?)?, c::get(op, "args")?)
        })();
        results.push(match result {
            Ok(r) => obj! {"ok"=>true,"result"=>r},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results,"actual_provider_calls"=>0})
}
