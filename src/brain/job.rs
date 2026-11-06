use super::infer::choose;
use super::*;

#[derive(Clone, Copy, Debug)]
struct Policy {
    mask: u16,
    low: i64,
    gap: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Baseline,
    Train,
    Validate,
    Done,
}

/// Disposable, sample-sliced training. The live brain is untouched until `finish`.
/// Work uses the existing deterministic parameter-operation estimates, not elapsed time.
/// Drop to cancel. Finish checks the entire starting state, not only its revision.
#[derive(Debug)]
pub struct TrainingJob {
    original: Brain,
    candidate: Brain,
    examples: Samples,
    validation: Samples,
    policies: Option<Vec<Policy>>,
    rates: Vec<i64>,
    epochs: usize,
    id: String,
    select: bool,
    phase: Phase,
    cursor: usize,
    rate_index: usize,
    parameters: usize,
    completed: usize,
    total: usize,
    loss_sum: i64,
    correct: usize,
    before_loss: i64,
    before_correct: usize,
    trials: Vec<Value>,
    selected: Option<(i64, Brain, usize, i64)>,
}

impl TrainingJob {
    pub fn is_finished(&self) -> bool {
        self.phase == Phase::Done
    }
    pub fn completed_steps(&self) -> usize {
        self.completed
    }
    pub fn total_steps(&self) -> usize {
        self.total
    }
    /// Minimum budget needed to advance the next indivisible sample.
    pub fn next_steps(&self) -> usize {
        self.parameters
            * match self.phase {
                Phase::Train => 3,
                Phase::Done => 0,
                _ if self.select => 2,
                _ => 1,
            }
    }
    /// Advances by at most `max_steps` (1..=2,000,000). An undersized budget errors
    /// before changing this job. Every call ends on a sample boundary.
    pub fn advance(&mut self, max_steps: usize) -> Result<usize> {
        if max_steps == 0 || max_steps > MAX_SELECTION_STEPS || max_steps < self.next_steps() {
            return Err(err("training slice budget too small or outside limits"));
        }
        let mut spent = 0;
        while !self.is_finished() && self.next_steps() <= max_steps - spent {
            let cost = self.next_steps();
            if self.phase == Phase::Train {
                let (features, targets) = &self.examples[self.cursor % self.examples.len()];
                self.candidate.train_sample(features, targets, self.rates[self.rate_index]);
                self.cursor += 1;
                if self.cursor == self.epochs * self.examples.len() {
                    self.phase = Phase::Validate;
                    self.cursor = 0;
                    self.loss_sum = 0;
                    self.correct = 0;
                }
            } else {
                let model = if self.phase == Phase::Baseline { &self.original } else { &self.candidate };
                let (features, targets) = &self.validation[self.cursor];
                let mut input = [None; MAX_INPUTS];
                for (slot, value) in input.iter_mut().zip(features) {
                    *slot = Some(*value);
                }
                let policy = self
                    .policies
                    .as_ref()
                    .map_or(Policy { mask: ((1u32 << model.outputs.len()) - 1) as u16, low: 0, gap: 0 }, |p| {
                        p[self.cursor]
                    });
                let mut prediction = Prediction::default();
                model.run_into(&input[..features.len()], policy.mask, policy.low, policy.gap, &mut prediction)?;
                for (score, target) in prediction.scores().iter().zip(targets) {
                    let difference = score.expect("complete sample") - target;
                    self.loss_sum += difference * difference;
                }
                if self.select {
                    self.correct += usize::from(if self.policies.is_some() {
                        let targets: Vec<_> = targets.iter().copied().map(Some).collect();
                        prediction.choice() == choose(&targets, policy.mask, policy.low, policy.gap).0
                    } else {
                        let desired = *targets.iter().max().expect("nonempty outputs");
                        if desired == 0 {
                            prediction.choice().is_none()
                        } else {
                            prediction.choice().is_some_and(|i| targets[i] == desired)
                        }
                    });
                }
                self.cursor += 1;
                if self.cursor == self.validation.len() {
                    self.end_validation();
                }
            }
            self.completed += cost;
            spent += cost;
        }
        Ok(spent)
    }
    fn end_validation(&mut self) {
        let loss = div(self.loss_sum, (self.validation.len() * self.original.outputs.len()) as i64 * S);
        if self.phase == Phase::Baseline {
            self.before_loss = loss;
            self.before_correct = self.correct;
            self.phase = Phase::Train;
        } else {
            let rate = self.rates[self.rate_index];
            if self.select {
                self.trials.push(crate::obj! {"rate" => number(rate),"loss" => number(loss),"correct" => self.correct});
            }
            if loss < self.before_loss
                && (!self.select || self.correct >= self.before_correct)
                && self.selected.as_ref().is_none_or(|(_, _, count, old_loss)| {
                    self.correct > *count || self.correct == *count && loss < *old_loss
                })
            {
                self.selected = Some((rate, self.candidate.clone(), self.correct, loss));
            }
            self.rate_index += 1;
            if self.rate_index == self.rates.len() {
                self.phase = Phase::Done;
            } else {
                self.candidate = self.original.clone();
                self.phase = Phase::Train;
            }
        }
        self.cursor = 0;
        self.loss_sum = 0;
        self.correct = 0;
    }
    /// Atomically installs the validated winner, or reports rejection. Stale/same-revision
    /// replacements with different content error. Rejected/unfinished/stale jobs change nothing.
    pub fn finish(self, brain: &mut Brain) -> Result<Value> {
        if !self.is_finished() {
            return Err(err("unfinished training job"));
        }
        if brain.to_value() != self.original.to_value() {
            return Err(err("stale training job"));
        }
        let accepted = self.selected.is_some();
        let (mut after_loss, mut after_correct, mut selected_rate) =
            (self.before_loss, self.before_correct, Value::Null);
        if let Some((rate, candidate, correct, loss)) = self.selected {
            brain.retain(candidate, self.id);
            after_loss = loss;
            after_correct = correct;
            selected_rate = number(rate);
        }
        if !self.select {
            return Ok(crate::obj! {"accepted" => accepted,"revision" => brain.revision,
            "before_loss" => number(self.before_loss),"after_loss" => number(after_loss),"steps" => self.total});
        }
        Ok(crate::obj! {"accepted" => accepted,"revision" => brain.revision,
        "before_loss" => number(self.before_loss),"after_loss" => number(after_loss),
        "before_correct" => self.before_correct,"after_correct" => after_correct,
        "validation_count" => self.validation.len(),"selected_rate" => selected_rate,
        "steps" => self.total,"trials" => Value::Array(self.trials)})
    }
}

impl Brain {
    pub fn start_train(&self, request: &Value) -> Result<TrainingJob> {
        self.start_job(request, false, false)
    }
    pub fn start_train_select(&self, request: &Value) -> Result<TrainingJob> {
        self.start_job(request, true, false)
    }
    /// Validation rows also contain `allowed`, `min_score`, `min_margin`. Compares the
    /// predicted choice/abstention to target utilities under that same policy.
    pub fn start_train_select_host(&self, request: &Value) -> Result<TrainingJob> {
        self.start_job(request, true, true)
    }
    pub fn train_select_host(&mut self, request: &Value) -> Result<Value> {
        let mut job = self.start_train_select_host(request)?;
        while !job.is_finished() {
            job.advance(MAX_SELECTION_STEPS)?;
        }
        job.finish(self)
    }
    fn start_job(&self, data: &Value, select: bool, host: bool) -> Result<TrainingJob> {
        fields(
            data,
            if select {
                &["id", "expected_revision", "examples", "validation", "epochs", "rates", "max_steps"]
            } else {
                &["id", "expected_revision", "examples", "validation", "epochs", "rate"]
            },
        )?;
        let id = self.request(data)?;
        let examples = self.samples(get(data, "examples")?)?;
        let (validation, policies) = if host {
            let raw = list(get(data, "validation")?)?;
            if raw.is_empty() || raw.len() > MAX_EXAMPLES {
                return Err(err("sample count outside limits"));
            }
            let mut clean = Vec::new();
            let mut policies = Vec::new();
            for row in raw {
                fields(row, &["inputs", "targets", "allowed", "min_score", "min_margin"])?;
                let allowed = names(get(row, "allowed")?, MAX_OUTPUTS, true)?;
                let mut mask = 0;
                for name in allowed {
                    let i =
                        self.outputs.iter().position(|s| s == &name).ok_or_else(|| err("invalid eligible outputs"))?;
                    mask |= 1u16 << i;
                }
                policies.push(Policy {
                    mask,
                    low: ticks(get(row, "min_score")?, 0, S)?,
                    gap: ticks(get(row, "min_margin")?, 0, S)?,
                });
                clean.push(
                    crate::obj! {"inputs" => get(row,"inputs")?.clone(),"targets" => get(row,"targets")?.clone()},
                );
            }
            (self.samples(&Value::Array(clean))?, Some(policies))
        } else {
            (self.samples(get(data, "validation")?)?, None)
        };
        if examples.iter().any(|(features, _)| validation.iter().any(|(other, _)| features == other)) {
            return Err(err("training and validation inputs overlap"));
        }
        let epochs = integer(get(data, "epochs")?, 1, MAX_EPOCHS)?;
        let raw_rates = if select { list(get(data, "rates")?)?.to_vec() } else { vec![get(data, "rate")?.clone()] };
        if raw_rates.is_empty() || raw_rates.len() > MAX_RATES {
            return Err(err("rate count outside limits"));
        }
        let mut rates = Vec::new();
        for raw in raw_rates {
            let rate = ticks(&raw, 0, S)?;
            if rate == 0 || rates.contains(&rate) {
                return Err(err("zero or duplicate normalized rate"));
            }
            rates.push(rate);
        }
        let parameters = self.weights.iter().map(Vec::len).sum::<usize>();
        let per_trial = (3 * epochs * examples.len() + 2 * validation.len()) * parameters;
        let total = (3 * epochs * examples.len() * rates.len()
            + if select { 2 * validation.len() * (rates.len() + 1) } else { 2 * validation.len() })
            * parameters;
        let cap = if select { integer(get(data, "max_steps")?, 1, MAX_SELECTION_STEPS)? } else { MAX_STEPS };
        if per_trial > MAX_STEPS || total > cap {
            return Err(err("training work limit"));
        }
        Ok(TrainingJob {
            original: self.clone(),
            candidate: self.clone(),
            examples,
            validation,
            policies,
            rates,
            epochs,
            id,
            select,
            phase: Phase::Baseline,
            cursor: 0,
            rate_index: 0,
            parameters,
            completed: 0,
            total,
            loss_sum: 0,
            correct: 0,
            before_loss: 0,
            before_correct: 0,
            trials: Vec::new(),
            selected: None,
        })
    }
}
