//! Small, portable reaction networks with named inputs and utility outputs.
//!
//! The game supplies normalized observations and eligible output names. Networks never
//! execute actions, observe the world, make network calls or interpret scores as probabilities.
//! Clamped-linear acyclic nodes support bounded supervised training and small weight lessons.
//! Candidate updates require lower loss on caller-supplied validation examples before commit.
//! All arithmetic, including training, uses signed millionths for deterministic ports.
//!
//! ```
//! use gamectx::{brain::Brain, obj, Value};
//! let brain = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 4, 3)?;
//! let prediction = brain.run(&obj! {"danger" => 0.8, "need" => 0.2}, &[])?;
//! assert_eq!(prediction.get("choice"), Some(&Value::Null)); // no eligible actions
//! let saved = brain.to_value().to_json();
//! assert_eq!(Brain::restore_json(&saved)?.to_value(), brain.to_value());
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::json::{Error, Result, Value};

mod cache;
mod infer;
mod job;
mod learn;
mod tools;
pub use cache::{CacheStats, CachedBrain, MAX_CACHE_ENTRIES};
pub use infer::{Prediction, SCALE};
pub use job::TrainingJob;
pub use tools::{create_call, process, process_cached, tools};

const S: i64 = 1_000_000;
pub const MAX_INPUTS: usize = 16;
pub const MAX_NODES: usize = 32;
pub const MAX_OUTPUTS: usize = 16;
pub const MAX_EDGES: usize = 16;
pub const MAX_EXAMPLES: usize = 128;
pub const MAX_EPOCHS: usize = 128;
pub const MAX_STEPS: usize = 500_000;
/// Aggregate cap for opt-in learning-rate selection; each trial still obeys MAX_STEPS.
pub const MAX_SELECTION_STEPS: usize = 2_000_000;
pub const MAX_RATES: usize = 8;
pub const MAX_HISTORY: usize = 8;
pub const MAX_UPDATES: usize = 128;
pub const MAX_REQUEST_BYTES: usize = 65_536;
pub const MAX_STATE_BYTES: usize = 262_144;

fn err(message: &str) -> Error {
    Error::new(message)
}

fn fields<'a>(value: &'a Value, keys: &[&str]) -> Result<&'a [(String, Value)]> {
    let entries = value.as_object().ok_or_else(|| err("expected an object"))?;
    if entries.len() != keys.len()
        || entries.iter().any(|(key, _)| !keys.contains(&key.as_str()))
        || keys.iter().any(|key| value.get(key).is_none())
    {
        return Err(err("unexpected or missing fields"));
    }
    Ok(entries)
}

fn get<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value.get(key).ok_or_else(|| err("missing field"))
}

fn text(value: &Value) -> Result<&str> {
    value.as_str().ok_or_else(|| err("expected a string"))
}

fn list(value: &Value) -> Result<&[Value]> {
    value.as_array().ok_or_else(|| err("expected an array"))
}

fn integer(value: &Value, low: usize, high: usize) -> Result<usize> {
    value.as_usize().filter(|v| (low..=high).contains(v)).ok_or_else(|| err("integer outside limits"))
}

fn ticks(value: &Value, low: i64, high: i64) -> Result<i64> {
    let number = match value {
        Value::Number(n) => n.parse::<f64>().ok(),
        _ => None,
    }
    .ok_or_else(|| err("expected a finite number"))?;
    if !number.is_finite() || number < low as f64 / S as f64 || number > high as f64 / S as f64 {
        return Err(err("number outside limits"));
    }
    Ok((number * S as f64).round() as i64)
}

fn name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 32
        || !value.as_bytes()[0].is_ascii_lowercase()
        || !value.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    {
        return Err(err("invalid name"));
    }
    Ok(())
}

fn names(value: &Value, limit: usize, empty: bool) -> Result<Vec<String>> {
    let values = list(value)?;
    if values.len() > limit || (!empty && values.is_empty()) {
        return Err(err("name count outside limits"));
    }
    let mut out = Vec::new();
    for value in values {
        let value = text(value)?;
        name(value)?;
        if out.iter().any(|old| old == value) {
            return Err(err("duplicate name"));
        }
        out.push(value.to_owned());
    }
    Ok(out)
}

fn div(a: i64, b: i64) -> i64 {
    a.signum() * ((a.abs() + b / 2) / b)
}

fn mul(a: i64, b: i64) -> i64 {
    div(a * b, S)
}

fn number(value: i64) -> Value {
    let sign = if value < 0 { "-" } else { "" };
    let n = value.abs();
    let whole = n / S;
    let frac = n % S;
    Value::Number(if frac == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{}", format!("{frac:06}").trim_end_matches('0'))
    })
}

#[derive(Clone, Debug)]
struct Node {
    id: String,
    edges: Vec<(usize, String)>,
    pinned: bool,
    limit: i64,
}

type Matrix = Vec<Vec<i64>>;
type Samples = Vec<(Vec<i64>, Vec<i64>)>;

/// An acyclic network. Inputs and outputs are game-defined names, with no built-in actions.
/// Host-provided observations/targets must be normalized to 0..1. Unknown inputs remain
/// unknown; they are not silently treated as observed zeroes.
#[derive(Clone, Debug)]
pub struct Brain {
    inputs: Vec<String>,
    outputs: Vec<String>,
    nodes: Vec<Node>,
    output_indices: Vec<usize>,
    base: Matrix,
    weights: Matrix,
    revision: usize,
    used: Vec<String>,
    history: Vec<(String, Matrix)>,
}

impl Brain {
    /// Parses a model-written graph, bounded before JSON parsing.
    pub fn parse(definition: &str) -> Result<Self> {
        Self::from_value(&crate::json::parse(definition, 32_768)?)
    }

    /// Validates names, topology, parameter bounds, pins and outputs.
    pub fn from_value(definition: &Value) -> Result<Self> {
        fields(definition, &["version", "inputs", "outputs", "nodes"])?;
        integer(get(definition, "version")?, 1, 1)?;
        let inputs = names(get(definition, "inputs")?, MAX_INPUTS, false)?;
        let outputs = names(get(definition, "outputs")?, MAX_OUTPUTS, false)?;
        let raw = list(get(definition, "nodes")?)?;
        if raw.is_empty() || raw.len() > MAX_NODES {
            return Err(err("node count outside limits"));
        }
        let mut known = inputs.clone();
        let mut nodes = Vec::new();
        let mut base = Vec::new();
        for node in raw {
            fields(node, &["id", "weights", "bias", "pinned", "learning_limit"])?;
            let id = text(get(node, "id")?)?;
            name(id)?;
            if known.iter().any(|old| old == id) {
                return Err(err("duplicate node name"));
            }
            let pinned = get(node, "pinned")?.as_bool().ok_or_else(|| err("invalid pin"))?;
            let limit = ticks(get(node, "learning_limit")?, 0, 4 * S)?;
            let weights = get(node, "weights")?.as_object().ok_or_else(|| err("weights must be an object"))?;
            if weights.len() > MAX_EDGES {
                return Err(err("connection count outside limits"));
            }
            let mut edges = Vec::new();
            let mut row = vec![ticks(get(node, "bias")?, -4 * S, 4 * S)?];
            for (key, value) in weights {
                if edges.iter().any(|(_, old)| old == key) {
                    return Err(err("duplicate connection"));
                }
                let index = known
                    .iter()
                    .position(|old| old == key)
                    .ok_or_else(|| err("unknown, forward or cyclic connection"))?;
                edges.push((index, key.clone()));
                row.push(ticks(value, -4 * S, 4 * S)?);
            }
            nodes.push(Node { id: id.to_owned(), edges, pinned, limit });
            base.push(row);
            known.push(id.to_owned());
        }
        let output_indices = outputs
            .iter()
            .map(|out| nodes.iter().position(|node| &node.id == out).ok_or_else(|| err("output must name a node")))
            .collect::<Result<_>>()?;
        Ok(Self {
            inputs,
            outputs,
            nodes,
            output_indices,
            weights: base.clone(),
            base,
            revision: 0,
            used: Vec::new(),
            history: Vec::new(),
        })
    }

    /// A reproducible one-hidden-layer initializer. `hidden=0` creates a linear network.
    /// Training is opt-in; initialization supplies no game knowledge.
    pub fn dense(inputs: &[&str], outputs: &[&str], hidden: usize, seed: u64) -> Result<Self> {
        if inputs.len() > MAX_INPUTS || outputs.len() > MAX_OUTPUTS || hidden > 16 {
            return Err(err("dense dimensions outside limits"));
        }
        for value in inputs.iter().chain(outputs) {
            name(value)?;
        }
        let mut rng = if seed == 0 { 1 } else { seed };
        let mut weight = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            number((rng % 1_000_001) as i64 - 500_000)
        };
        let mut nodes = Vec::new();
        let previous: Vec<String> = if hidden == 0 {
            inputs.iter().map(|s| s.to_string()).collect()
        } else {
            (0..hidden).map(|i| format!("hidden_{i}")).collect()
        };
        for (ids, sources) in [
            (
                if hidden == 0 { Vec::new() } else { previous.clone() },
                inputs.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            ),
            (outputs.iter().map(|s| s.to_string()).collect(), previous),
        ] {
            for id in ids {
                let weights = Value::Object(sources.iter().map(|key| (key.clone(), weight())).collect());
                nodes.push(crate::obj! {"id" => id, "weights" => weights, "bias" => number(S / 2),
                "pinned" => false, "learning_limit" => 4});
            }
        }
        Self::from_value(&crate::obj! {"version" => 1, "inputs" => Value::strings(inputs),
        "outputs" => Value::strings(outputs), "nodes" => Value::Array(nodes)})
    }

    fn definition_with(&self, matrix: &Matrix) -> Value {
        let nodes = self
            .nodes
            .iter()
            .zip(matrix)
            .map(|(node, row)| {
                let weights = Value::Object(
                    node.edges.iter().enumerate().map(|(j, (_, key))| (key.clone(), number(row[j + 1]))).collect(),
                );
                crate::obj! {"id" => node.id.as_str(), "weights" => weights, "bias" => number(row[0]),
                "pinned" => node.pinned, "learning_limit" => number(node.limit)}
            })
            .collect();
        crate::obj! {"version" => 1, "inputs" => Value::strings(&self.inputs),
        "outputs" => Value::strings(&self.outputs), "nodes" => Value::Array(nodes)}
    }

    /// The original authored definition; learned parameters are in [`Self::inspect`].
    pub fn definition(&self) -> Value {
        self.definition_with(&self.base)
    }

    /// Current parameters, pins and revision, without history or training examples.
    pub fn inspect(&self) -> Value {
        crate::obj! {"revision" => self.revision, "definition" => self.definition_with(&self.weights)}
    }

    pub fn revision(&self) -> usize {
        self.revision
    }

    /// Names the host must bind to normalized observations.
    pub fn inputs(&self) -> &[String] {
        &self.inputs
    }

    /// Utility/reaction names the host may make eligible; no name grants action authority.
    pub fn outputs(&self) -> &[String] {
        &self.outputs
    }

    fn features(&self, values: &Value, complete: bool) -> Result<Vec<Option<i64>>> {
        let entries = values.as_object().ok_or_else(|| err("inputs must be an object"))?;
        if entries.len() > self.inputs.len()
            || entries
                .iter()
                .enumerate()
                .any(|(i, (key, _))| !self.inputs.contains(key) || entries[..i].iter().any(|(old, _)| old == key))
        {
            return Err(err("unknown or duplicate input"));
        }
        self.inputs
            .iter()
            .map(|key| match values.get(key) {
                None | Some(Value::Null) if !complete => Ok(None),
                None | Some(Value::Null) => Err(err("training inputs must be complete")),
                Some(value) => Ok(Some(ticks(value, 0, S)?)),
            })
            .collect()
    }

    fn forward(&self, features: &[Option<i64>]) -> (Vec<Option<i64>>, Vec<bool>) {
        let mut values = features.to_vec();
        let mut active = Vec::with_capacity(self.nodes.len());
        for (node, weights) in self.nodes.iter().zip(&self.weights) {
            let mut total = weights[0] * S;
            let mut unknown = false;
            for ((source, _), weight) in node.edges.iter().zip(&weights[1..]) {
                match values[*source] {
                    Some(value) => total += weight * value,
                    None if *weight != 0 => unknown = true,
                    None => {}
                }
            }
            let raw = div(total, S);
            values.push((!unknown).then_some(raw.clamp(0, S)));
            active.push(!unknown && raw > 0 && raw < S);
        }
        (values, active)
    }

    /// Evaluates utility scores and a strictly winning eligible choice. An empty mask,
    /// ties, all-zero scores or an unknown eligible output abstain (`choice:null`).
    pub fn run(&self, inputs: &Value, allowed: &[&str]) -> Result<Value> {
        self.run_with(inputs, allowed, 0.0, 0.0)
    }

    /// Adds host-chosen minimum score and winning-margin thresholds (0..1). These are
    /// utility thresholds, not confidence probabilities. Actual action validation stays native.
    pub fn run_with(&self, inputs: &Value, allowed: &[&str], min_score: f64, min_margin: f64) -> Result<Value> {
        if allowed.len() > self.outputs.len()
            || allowed
                .iter()
                .enumerate()
                .any(|(i, key)| !self.outputs.iter().any(|out| out == key) || allowed[..i].contains(key))
        {
            return Err(err("invalid eligible outputs"));
        }
        if !min_score.is_finite() || !min_margin.is_finite() {
            return Err(err("thresholds must be finite"));
        }
        let low = ticks(&Value::from(min_score), 0, S)?;
        let gap = ticks(&Value::from(min_margin), 0, S)?;
        let features = self.features(inputs, false)?;
        let mask = self
            .outputs
            .iter()
            .enumerate()
            .fold(0, |mask, (i, name)| mask | if allowed.contains(&name.as_str()) { 1u16 << i } else { 0 });
        let mut prediction = Prediction::default();
        self.run_into(&features, mask, low, gap, &mut prediction)?;
        Ok(self.prediction_value(&prediction))
    }
}
