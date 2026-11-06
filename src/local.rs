//! Stateless reference/CLI adapter for additive local deployment contracts. The byte
//! counter used here is a synthetic fixture estimator, never an actual model tokenizer.
use crate::{
    checked as c, obj,
    request::{Count, CountMethod, RequestBudget},
    Result, Value,
};
fn usize_arg(v: &Value, k: &str, max: u64) -> Result<usize> {
    Ok(c::uint(c::get(v, k)?, 0, max)? as usize)
}
pub fn process(v: &Value) -> Result<Value> {
    crate::request::bounded(v, 1_572_864)?;
    c::fields(v, &["kind", "args"])?;
    let a = c::get(v, "args")?;
    let counter = |request: &Value| Ok(Count { input: request.to_json().len(), method: CountMethod::Estimate });
    match c::text(c::get(v, "kind")?)? {
        "budget" => {
            c::fields(a, &["budget", "count"])?;
            Ok(RequestBudget::from_value(c::get(a, "budget")?)?
                .assess(Count::from_value(c::get(a, "count")?)?)?
                .to_value())
        }
        "dialogue" => {
            c::fields(a, &["thread", "current", "protect", "maximum", "tokens"])?;
            Ok(crate::dialogue::Thread::from_value(c::get(a, "thread")?)?
                .select(
                    c::text(c::get(a, "current")?)?,
                    usize_arg(a, "protect", 3)?,
                    usize_arg(a, "maximum", 128)?,
                    usize_arg(a, "tokens", 1_048_576)?,
                    counter,
                )?
                .to_value())
        }
        "scene" => {
            c::fields(a, &["scene", "protect", "maximum", "dialogue_tokens", "budget"])?;
            Ok(crate::dialogue::Scene::from_value(c::get(a, "scene")?)?
                .assemble(
                    &RequestBudget::from_value(c::get(a, "budget")?)?,
                    usize_arg(a, "protect", 3)?,
                    usize_arg(a, "maximum", 128)?,
                    usize_arg(a, "dialogue_tokens", 1_048_576)?,
                    crate::dialogue::render_chat,
                    counter,
                )?
                .to_value())
        }
        "cancel" => {
            c::fields(a, &["choices", "available"])?;
            let choices = crate::decision::Choices::from_value(c::get(a, "choices")?)?;
            let available = c::list(c::get(a, "available")?)?;
            if available.len() > 32 {
                return Err(c::bad());
            }
            choices.session().cancel(|v| available.iter().any(|p| p.same(v)))
        }
        "decision" => {
            c::fields(a, &["choices", "calls"])?;
            let choices = crate::decision::Choices::from_value(c::get(a, "choices")?)?;
            let calls = c::list(c::get(a, "calls")?)?;
            if calls.len() > 3 {
                return Err(c::bad());
            }
            let mut session = choices.session();
            let mut results = Vec::new();
            for call in calls {
                c::fields(call, &["text", "revision", "available"])?;
                let revision = c::uint(c::get(call, "revision")?, 0, u32::MAX as u64)? as u32;
                let available = c::list(c::get(call, "available")?)?;
                if available.len() > 32 {
                    return Err(c::bad());
                }
                results.push(
                    session
                        .apply(c::text(c::get(call, "text")?)?, revision, |v| available.iter().any(|p| p.same(v)))?,
                );
            }
            Ok(obj! {"schema"=>choices.schema(),"results"=>results,"finished"=>session.is_finished()})
        }
        "preset" => {
            c::fields(a, &["size", "name", "identity"])?;
            crate::deployment::ModelSize::parse(c::text(c::get(a, "size")?)?)?.starting_profile(
                c::text(c::get(a, "name")?)?,
                &crate::deployment::Identity::from_value(c::get(a, "identity")?)?,
            )
        }
        "profile" => {
            c::fields(a, &["profile", "actual_identity"])?;
            crate::deployment::Profile::from_value(c::get(a, "profile")?)?
                .describe(&crate::deployment::Identity::from_value(c::get(a, "actual_identity")?)?)
        }
        "report" => crate::turn::report(a),
        _ => Err(c::bad()),
    }
}
