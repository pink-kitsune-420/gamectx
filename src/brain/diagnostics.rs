use super::*;

pub(super) fn reason(prediction: &Prediction, allowed: u16, low: i64, gap: i64) -> &'static str {
    if allowed == 0 {
        return "no_eligible";
    }
    let eligible: Vec<_> =
        prediction.scores().iter().enumerate().filter(|(i, _)| allowed & (1 << i) != 0).map(|(_, v)| *v).collect();
    if eligible.iter().any(Option::is_none) {
        return "unknown_eligible";
    }
    let best = eligible.iter().flatten().copied().max().unwrap_or(0);
    if best == 0 {
        "all_zero"
    } else if eligible.iter().filter(|v| **v == Some(best)).count() > 1 {
        "tie"
    } else if best <= low {
        "score_threshold"
    } else if prediction.margin().is_some_and(|m| m <= gap) {
        "margin_threshold"
    } else {
        "chosen"
    }
}
impl Brain {
    /// Optional deterministic diagnostics. Reports numeric selection conditions, not
    /// inferred intent or calibrated confidence. Normal inference remains unchanged.
    pub fn diagnose(&self, inputs: &[Option<i64>], allowed: u16, low: i64, gap: i64) -> Result<Value> {
        let mut prediction = Prediction::default();
        self.run_into(inputs, allowed, low, gap, &mut prediction)?;
        let mut result = self.prediction_value(&prediction);
        if let Value::Object(fields) = &mut result {
            fields.push(("reason".into(), Value::str(reason(&prediction, allowed, low, gap))));
        }
        Ok(result)
    }
}
