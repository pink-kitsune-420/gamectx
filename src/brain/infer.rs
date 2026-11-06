use super::*;

/// Integer millionths used by the native inference API.
pub const SCALE: i64 = S;

/// Reusable native prediction. Scores/margin use millionths; choices index `Brain::outputs`.
/// Successful inference needs no heap allocation. Convert to JSON only at a boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prediction {
    pub(super) scores: [Option<i64>; MAX_OUTPUTS],
    pub(super) len: usize,
    pub(super) choice: Option<usize>,
    pub(super) margin: Option<i64>,
    pub(super) revision: usize,
}

impl Default for Prediction {
    fn default() -> Self {
        Self { scores: [None; MAX_OUTPUTS], len: 0, choice: None, margin: None, revision: 0 }
    }
}

impl Prediction {
    pub fn scores(&self) -> &[Option<i64>] {
        &self.scores[..self.len]
    }
    pub fn choice(&self) -> Option<usize> {
        self.choice
    }
    pub fn margin(&self) -> Option<i64> {
        self.margin
    }
    pub fn revision(&self) -> usize {
        self.revision
    }
}

pub(super) fn choose(scores: &[Option<i64>], allowed: u16, low: i64, gap: i64) -> (Option<usize>, Option<i64>) {
    let (mut best, mut second, mut index) = (-1, 0, None);
    for (i, score) in scores.iter().enumerate() {
        if allowed & (1u16 << i) == 0 {
            continue;
        }
        let Some(score) = score else { return (None, None) };
        if *score > best {
            second = best.max(0);
            best = *score;
            index = Some(i);
        } else {
            second = second.max(*score);
        }
    }
    let Some(index) = index else { return (None, None) };
    let margin = best - second;
    ((best > low && margin > gap).then_some(index), Some(margin))
}

impl Brain {
    /// Inputs are in declared order, exactly one `Option` per input; None is unknown.
    /// Values and thresholds must be 0..=SCALE. Eligibility is an output-index bit mask.
    /// Validates before changing `out`. Fixed stack storage avoids allocation and gradients.
    pub fn run_into(
        &self,
        inputs: &[Option<i64>],
        allowed: u16,
        min_score: i64,
        min_margin: i64,
        out: &mut Prediction,
    ) -> Result<()> {
        if inputs.len() != self.inputs.len() || inputs.iter().flatten().any(|v| !(0..=S).contains(v)) {
            return Err(err("invalid native inputs"));
        }
        if u32::from(allowed) >> self.outputs.len() != 0 {
            return Err(err("invalid eligible outputs"));
        }
        if !(0..=S).contains(&min_score) || !(0..=S).contains(&min_margin) {
            return Err(err("thresholds outside limits"));
        }
        let mut values = [None; MAX_INPUTS + MAX_NODES];
        values[..inputs.len()].copy_from_slice(inputs);
        for (i, (node, weights)) in self.nodes.iter().zip(&self.weights).enumerate() {
            let mut total = weights[0] * S;
            let mut known = true;
            for ((source, _), weight) in node.edges.iter().zip(&weights[1..]) {
                match values[*source] {
                    Some(value) => total += weight * value,
                    None if *weight != 0 => known = false,
                    None => {}
                }
            }
            values[inputs.len() + i] = known.then_some(div(total, S).clamp(0, S));
        }
        out.scores.fill(None);
        out.len = self.outputs.len();
        for (i, index) in self.output_indices.iter().enumerate() {
            out.scores[i] = values[inputs.len() + index];
        }
        (out.choice, out.margin) = choose(out.scores(), allowed, min_score, min_margin);
        out.revision = self.revision;
        Ok(())
    }

    pub(super) fn prediction_value(&self, prediction: &Prediction) -> Value {
        let scores = Value::Object(
            self.outputs
                .iter()
                .zip(prediction.scores())
                .map(|(key, value)| (key.clone(), value.map_or(Value::Null, number)))
                .collect(),
        );
        crate::obj! {"revision" => prediction.revision,"scores" => scores,
        "choice" => prediction.choice.map_or(Value::Null,|i| Value::str(&self.outputs[i])),
        "margin" => prediction.margin.map_or(Value::Null,number)}
    }
}
