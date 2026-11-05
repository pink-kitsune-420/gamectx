use super::*;

fn request(args: &Value) -> Result<Value> {
    fields(args, &["request"])?;
    crate::json::parse(text(get(args, "request")?)?, MAX_REQUEST_BYTES)
}

/// Validates `create_brain` arguments: one `request` string containing a graph definition.
/// The host chooses the owner/storage slot; this never replaces another brain implicitly.
pub fn create_call(args: &Value) -> Result<Brain> {
    Brain::from_value(&request(args)?)
}

impl Brain {
    /// Dispatches model tool arguments. Request text is strict bounded JSON, not code.
    /// `create_brain` is separate so a call cannot silently reset this brain's pins/history.
    pub fn apply_call(&mut self, action: &str, args: &Value) -> Result<Value> {
        let op = match action {
            "run_brain" => "run",
            "train_brain" => "train",
            "train_select_brain" => "train_select",
            "teach_brain" => "teach",
            "undo_brain" => "undo",
            "inspect_brain" => "inspect",
            _ => return Err(err("unknown brain tool")),
        };
        self.operation(op, &request(args)?)
    }

    fn operation(&mut self, op: &str, args: &Value) -> Result<Value> {
        match op {
            "run" => {
                fields(args, &["inputs", "allowed", "min_score", "min_margin"])?;
                let allowed = names(get(args, "allowed")?, MAX_OUTPUTS, true)?;
                let low = ticks(get(args, "min_score")?, 0, S)?;
                let gap = ticks(get(args, "min_margin")?, 0, S)?;
                self.run_with(
                    get(args, "inputs")?,
                    &allowed.iter().map(String::as_str).collect::<Vec<_>>(),
                    low as f64 / S as f64,
                    gap as f64 / S as f64,
                )
            }
            "train" => self.train(args),
            "train_select" => self.train_select(args),
            "teach" => self.teach(args),
            "undo" => {
                fields(args, &["expected_revision"])?;
                self.undo(integer(get(args, "expected_revision")?, 0, 2 * MAX_UPDATES)?)
            }
            "inspect" => {
                fields(args, &[])?;
                Ok(self.inspect())
            }
            _ => Err(err("unknown brain operation")),
        }
    }
}

/// Bounded, stateless JSON workflow for CLIs and ports. Input contains exactly one of
/// `definition`, `dense` or saved `state`, plus up to 64 `operations` (`op`, `args`).
/// Returns operation results and saved state. No files or external state are changed.
pub fn process(input: &Value) -> Result<Value> {
    let sources: Vec<&str> =
        ["definition", "dense", "state"].into_iter().filter(|key| input.get(key).is_some()).collect();
    if sources.len() != 1 {
        return Err(err("one brain source required"));
    }
    fields(input, &[sources[0], "operations"])?;
    let raw = get(input, sources[0])?;
    let mut brain = match sources[0] {
        "definition" => Brain::from_value(raw)?,
        "state" => Brain::restore(raw)?,
        _ => {
            fields(raw, &["inputs", "outputs", "hidden", "seed"])?;
            let inputs = names(get(raw, "inputs")?, MAX_INPUTS, false)?;
            let outputs = names(get(raw, "outputs")?, MAX_OUTPUTS, false)?;
            let hidden = integer(get(raw, "hidden")?, 0, 16)?;
            let seed = match get(raw, "seed")? {
                Value::Number(n) if n.bytes().all(|c| c.is_ascii_digit()) => n.parse::<u64>().ok(),
                _ => None,
            }
            .ok_or_else(|| err("invalid dense seed"))?;
            Brain::dense(
                &inputs.iter().map(String::as_str).collect::<Vec<_>>(),
                &outputs.iter().map(String::as_str).collect::<Vec<_>>(),
                hidden,
                seed,
            )?
        }
    };
    let operations = list(get(input, "operations")?)?;
    if operations.len() > 64 {
        return Err(err("operation count outside limits"));
    }
    let mut results = Vec::new();
    for operation in operations {
        fields(operation, &["op", "args"])?;
        results.push(brain.operation(text(get(operation, "op")?)?, get(operation, "args")?)?);
    }
    Ok(crate::obj! {"results" => Value::Array(results), "state" => brain.to_value()})
}

/// Tools describe conditional policies and proposed labels, never factual observations.
/// Requests are strings so the schemas pack losslessly as signatures. Real runtime inputs,
/// eligible actions and trusted training/validation labels come from the host.
pub fn tools() -> Value {
    let descriptions = [
        ("create_brain", "Create a proposed local reaction network. Request JSON: {version:1,inputs:[names],outputs:[node names],nodes:[{id,weights:{earlier input or node:weight},bias,pinned,learning_limit}]}. Inputs/scores 0..1, weights/bias -4..4. Ordered acyclic nodes; host chooses ownership and whether to install."),
        ("run_brain", "Evaluate a hypothetical state: {inputs:{name:number or null},allowed:[output names],min_score:0,min_margin:0}. Scores are utilities, not probabilities; unknown eligible scores and ties abstain. The host supplies real observations and action eligibility."),
        ("train_brain", "Propose bounded supervised training: {id,expected_revision,examples:[{inputs:{name:number},targets:{output:number}}],validation:[same shape],epochs,rate}. Complete 0..1 values, distinct training/validation inputs, max 128 each/epochs. Candidate retained only if validation loss falls. Proposed labels are not world facts."),
        ("train_select_brain", "Compare rates on candidate copies: {id,expected_revision,examples:[{inputs,targets}],validation:[same shape],epochs,rates:[positive numbers],max_steps}. At most 8 distinct rates, 500000 work units per trial, 2000000 aggregate. Select by validation reaction accuracy then MSE; require lower MSE and no accuracy regression. One retained update. Labels are proposals, not world facts."),
        ("teach_brain", "Propose a small lesson: {id,expected_revision,changes:[{node,source,delta}],validation:[{inputs,targets}]}. Source is an existing input/node or $bias; max 16 deltas, each nonzero and within -0.1..0.1. Pins/learning limits apply; validation must improve."),
        ("undo_brain", "Undo the latest retained update: {expected_revision}. Revision advances; used IDs remain consumed. Eight snapshots retained."),
        ("inspect_brain", "Inspect current graph parameters and revision without changing them. Request: {}."),
    ];
    Value::Array(descriptions.iter().map(|(name, description)| crate::obj! {
        "type" => "function", "function" => crate::obj! {"name" => *name, "description" => *description, "strict" => true,
            "parameters" => crate::obj! {"type" => "object", "properties" => crate::obj! {
                "request" => crate::obj! {"type" => "string", "maxLength" => MAX_REQUEST_BYTES}},
                "required" => vec!["request"], "additionalProperties" => false}}}).collect())
}
