use super::*;

fn matrix_value(matrix: &Matrix) -> Value {
    Value::Array(matrix.iter().map(|row| Value::Array(row.iter().copied().map(number).collect())).collect())
}

impl Brain {
    fn matrix(&self, raw: &Value) -> Result<Matrix> {
        let rows = list(raw)?;
        if rows.len() != self.nodes.len() {
            return Err(err("invalid weight matrix"));
        }
        rows.iter()
            .zip(&self.nodes)
            .zip(&self.base)
            .map(|((row, node), baseline)| {
                let values = list(row)?;
                if values.len() != node.edges.len() + 1 {
                    return Err(err("invalid weight row"));
                }
                values
                    .iter()
                    .zip(baseline)
                    .map(|(value, base)| {
                        let value = ticks(value, -4 * S, 4 * S)?;
                        if (value - base).abs() > if node.pinned { 0 } else { node.limit } {
                            return Err(err("weight exceeds pin or learning limit"));
                        }
                        Ok(value)
                    })
                    .collect()
            })
            .collect()
    }

    /// Saves the authored baseline, current weights, bounded undo snapshots and consumed
    /// update IDs. Training examples and raw observations are not stored.
    pub fn to_value(&self) -> Value {
        let history = self
            .history
            .iter()
            .map(|(id, weights)| crate::obj! {"id" => id.as_str(), "weights" => matrix_value(weights)})
            .collect();
        crate::obj! {"version" => 1, "definition" => self.definition(), "revision" => self.revision,
        "weights" => matrix_value(&self.weights), "used_ids" => Value::strings(&self.used),
        "history" => Value::Array(history)}
    }

    /// Loads a saved brain, rechecking every weight, pin, learning limit and history shape.
    /// This validates bounds and consistency; it is not cryptographic authentication.
    pub fn restore(saved: &Value) -> Result<Self> {
        fields(saved, &["version", "definition", "revision", "weights", "used_ids", "history"])?;
        integer(get(saved, "version")?, 1, 1)?;
        let mut brain = Self::from_value(get(saved, "definition")?)?;
        brain.used = names(get(saved, "used_ids")?, MAX_UPDATES, true)?;
        brain.revision = integer(get(saved, "revision")?, brain.used.len(), 2 * brain.used.len())?;
        let history = list(get(saved, "history")?)?;
        if history.len() > MAX_HISTORY.min(2 * brain.used.len() - brain.revision) {
            return Err(err("invalid history"));
        }
        let mut previous = None;
        for entry in history {
            fields(entry, &["id", "weights"])?;
            let id = text(get(entry, "id")?)?;
            let index = brain.used.iter().position(|used| used == id).ok_or_else(|| err("invalid history ID"))?;
            if previous.is_some_and(|old| old >= index) {
                return Err(err("invalid history ID order"));
            }
            previous = Some(index);
            brain.history.push((id.to_owned(), brain.matrix(get(entry, "weights")?)?));
        }
        brain.weights = brain.matrix(get(saved, "weights")?)?;
        if brain.used.is_empty() && brain.weights != brain.base {
            return Err(err("unlearned weights must equal baseline"));
        }
        Ok(brain)
    }

    /// Bounded parsing of serialized saved learning.
    pub fn restore_json(text: &str) -> Result<Self> {
        Self::restore(&crate::json::parse(text, MAX_STATE_BYTES)?)
    }

    fn samples(&self, raw: &Value) -> Result<Samples> {
        let rows = list(raw)?;
        if rows.is_empty() || rows.len() > MAX_EXAMPLES {
            return Err(err("sample count outside limits"));
        }
        rows.iter()
            .map(|row| {
                fields(row, &["inputs", "targets"])?;
                let targets = get(row, "targets")?;
                fields(targets, &self.outputs.iter().map(String::as_str).collect::<Vec<_>>())?;
                let features = self
                    .features(get(row, "inputs")?, true)?
                    .into_iter()
                    .map(|value| value.expect("complete features"))
                    .collect();
                let targets = self.outputs.iter().map(|key| ticks(get(targets, key)?, 0, S)).collect::<Result<_>>()?;
                Ok((features, targets))
            })
            .collect()
    }

    fn loss(&self, samples: &Samples) -> i64 {
        let mut total = 0;
        for (features, targets) in samples {
            let features: Vec<Option<i64>> = features.iter().copied().map(Some).collect();
            let (values, _) = self.forward(&features);
            for (index, target) in self.output_indices.iter().zip(targets) {
                let error = values[self.inputs.len() + index].expect("complete sample") - target;
                total += error * error;
            }
        }
        div(total, (samples.len() * self.outputs.len()) as i64 * S)
    }

    fn request(&self, data: &Value) -> Result<String> {
        let id = text(get(data, "id")?)?;
        name(id)?;
        integer(get(data, "expected_revision")?, self.revision, self.revision)?;
        if self.used.iter().any(|used| used == id) || self.used.len() >= MAX_UPDATES {
            return Err(err("replayed ID or update limit"));
        }
        Ok(id.to_owned())
    }

    fn commit(&mut self, candidate: Self, id: String, validation: &Samples, steps: usize) -> Value {
        let before = self.loss(validation);
        let after = candidate.loss(validation);
        let accepted = after < before;
        if accepted {
            self.retain(candidate, id);
        }
        crate::obj! {"accepted" => accepted, "revision" => self.revision,
        "before_loss" => number(before), "after_loss" => number(after), "steps" => steps}
    }

    fn retain(&mut self, candidate: Self, id: String) {
        self.history.push((id.clone(), std::mem::replace(&mut self.weights, candidate.weights)));
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.used.push(id);
        self.revision += 1;
    }

    /// Proposes up to 16 existing parameter changes, each at most ±0.1. `source:"$bias"`
    /// edits a bias. Pins and cumulative displacement from the authored baseline apply.
    /// Caller-supplied validation must improve before any change is retained.
    pub fn teach(&mut self, data: &Value) -> Result<Value> {
        fields(data, &["id", "expected_revision", "changes", "validation"])?;
        let id = self.request(data)?;
        let validation = self.samples(get(data, "validation")?)?;
        let changes = list(get(data, "changes")?)?;
        if changes.is_empty() || changes.len() > 16 {
            return Err(err("change count outside limits"));
        }
        let mut candidate = self.clone();
        let mut seen = Vec::new();
        for change in changes {
            fields(change, &["node", "source", "delta"])?;
            let node = text(get(change, "node")?)?;
            let source = text(get(change, "source")?)?;
            let index = self.nodes.iter().position(|old| old.id == node).ok_or_else(|| err("unknown node"))?;
            if self.nodes[index].pinned {
                return Err(err("pinned node"));
            }
            let parameter = if source == "$bias" {
                0
            } else {
                1 + self.nodes[index]
                    .edges
                    .iter()
                    .position(|(_, key)| key == source)
                    .ok_or_else(|| err("unknown parameter"))?
            };
            if seen.contains(&(index, parameter)) {
                return Err(err("duplicate change"));
            }
            seen.push((index, parameter));
            let delta = ticks(get(change, "delta")?, -S / 10, S / 10)?;
            if delta == 0 {
                return Err(err("zero delta"));
            }
            candidate.weights[index][parameter] += delta;
        }
        candidate.matrix(&matrix_value(&candidate.weights))?;
        let steps = 2 * validation.len() * self.weights.iter().map(Vec::len).sum::<usize>();
        Ok(self.commit(candidate, id, &validation, steps))
    }

    /// Bounded, deterministic online backpropagation on a candidate copy. Validation
    /// inputs must not overlap training inputs on the millionth grid. Lower validation
    /// MSE is required to commit; rejected updates leave the entire brain unchanged.
    /// This guard checks supplied labels, not whether those labels describe the real world.
    pub fn train(&mut self, data: &Value) -> Result<Value> {
        fields(data, &["id", "expected_revision", "examples", "validation", "epochs", "rate"])?;
        let id = self.request(data)?;
        let examples = self.samples(get(data, "examples")?)?;
        let validation = self.samples(get(data, "validation")?)?;
        if examples.iter().any(|(features, _)| validation.iter().any(|(other, _)| features == other)) {
            return Err(err("training and validation inputs overlap"));
        }
        let epochs = integer(get(data, "epochs")?, 1, MAX_EPOCHS)?;
        let rate = ticks(get(data, "rate")?, 0, S)?;
        if rate == 0 {
            return Err(err("zero learning rate"));
        }
        let steps =
            (3 * epochs * examples.len() + 2 * validation.len()) * self.weights.iter().map(Vec::len).sum::<usize>();
        if steps > MAX_STEPS {
            return Err(err("training work limit"));
        }
        let candidate = self.trained(&examples, epochs, rate);
        Ok(self.commit(candidate, id, &validation, steps))
    }

    fn trained(&self, examples: &Samples, epochs: usize, rate: i64) -> Self {
        let mut candidate = self.clone();
        let mut output = vec![false; self.nodes.len()];
        for index in &self.output_indices {
            output[*index] = true;
        }
        for _ in 0..epochs {
            for (features, targets) in examples {
                let features: Vec<Option<i64>> = features.iter().copied().map(Some).collect();
                let (values, active) = candidate.forward(&features);
                let mut gradient = vec![0i64; self.nodes.len()];
                for (index, target) in self.output_indices.iter().zip(targets) {
                    gradient[*index] += values[self.inputs.len() + index].expect("complete features") - target;
                }
                let mut updates = Vec::new();
                for i in (0..self.nodes.len()).rev() {
                    let node = &self.nodes[i];
                    // A straight-through output gradient can recover saturated old policies.
                    // Hidden nodes retain the clamp derivative.
                    let delta = if active[i] || output[i] { gradient[i].clamp(-S, S) } else { 0 };
                    let mut row = vec![delta];
                    for (j, (source, _)) in node.edges.iter().enumerate() {
                        if *source >= self.inputs.len() {
                            gradient[source - self.inputs.len()] += mul(delta, candidate.weights[i][j + 1]);
                        }
                        row.push(mul(delta, values[*source].expect("complete features")));
                    }
                    if node.pinned {
                        row.fill(0);
                    }
                    updates.push((i, row));
                }
                for (i, derivatives) in updates {
                    for (j, derivative) in derivatives.into_iter().enumerate() {
                        let low = (-4 * S).max(self.base[i][j] - self.nodes[i].limit);
                        let high = (4 * S).min(self.base[i][j] + self.nodes[i].limit);
                        candidate.weights[i][j] = (candidate.weights[i][j] - mul(rate, derivative)).clamp(low, high);
                    }
                }
            }
        }
        candidate
    }

    /// Correct default-threshold decisions over complete utility samples. Tied best
    /// targets allow any corresponding unique positive prediction; zero targets expect
    /// abstention. Eligibility/threshold changes used by a host require its own metric.
    fn decisions(&self, samples: &Samples) -> usize {
        let mut correct = 0;
        for (features, targets) in samples {
            let features: Vec<Option<i64>> = features.iter().copied().map(Some).collect();
            let (values, _) = self.forward(&features);
            let mut best = 0;
            let mut choice = None;
            let mut tied = false;
            for (output, index) in self.output_indices.iter().enumerate() {
                let score = values[self.inputs.len() + index].expect("complete sample");
                if score > best {
                    best = score;
                    choice = Some(output);
                    tied = false;
                } else if score == best {
                    tied = true;
                }
            }
            if tied {
                choice = None;
            }
            let desired = targets.iter().copied().max().expect("outputs are nonempty");
            correct += usize::from(if desired == 0 {
                choice.is_none()
            } else {
                choice.is_some_and(|index| targets[index] == desired)
            });
        }
        correct
    }

    /// Compares up to eight positive distinct learning rates on disposable candidate
    /// copies. Selects by validation decision count, then lower validation MSE, then
    /// request order. Both MSE improvement and non-regressing decision count are required.
    /// Commits one revision/snapshot. Selection uses supplied validation data; the host
    /// keeps its independent test/gameplay evaluation separate.
    ///
    /// The request contains `id`, `expected_revision`, `examples`, `validation`, `epochs`,
    /// `rates`, `max_steps`. The explicit aggregate budget is at most MAX_SELECTION_STEPS;
    /// each individual trial also obeys MAX_STEPS. Existing train behavior is unchanged.
    pub fn train_select(&mut self, data: &Value) -> Result<Value> {
        fields(data, &["id", "expected_revision", "examples", "validation", "epochs", "rates", "max_steps"])?;
        let id = self.request(data)?;
        let examples = self.samples(get(data, "examples")?)?;
        let validation = self.samples(get(data, "validation")?)?;
        if examples.iter().any(|(features, _)| validation.iter().any(|(other, _)| features == other)) {
            return Err(err("training and validation inputs overlap"));
        }
        let epochs = integer(get(data, "epochs")?, 1, MAX_EPOCHS)?;
        let raw_rates = list(get(data, "rates")?)?;
        if raw_rates.is_empty() || raw_rates.len() > MAX_RATES {
            return Err(err("rate count outside limits"));
        }
        let mut rates = Vec::new();
        for rate in raw_rates {
            let rate = ticks(rate, 0, S)?;
            if rate == 0 || rates.contains(&rate) {
                return Err(err("zero or duplicate normalized rate"));
            }
            rates.push(rate);
        }
        let cap = integer(get(data, "max_steps")?, 1, MAX_SELECTION_STEPS)?;
        let parameters = self.weights.iter().map(Vec::len).sum::<usize>();
        let per_trial = (3 * epochs * examples.len() + 2 * validation.len()) * parameters;
        let steps = (3 * epochs * examples.len() * rates.len() + 2 * validation.len() * (rates.len() + 1)) * parameters;
        if per_trial > MAX_STEPS || steps > cap {
            return Err(err("training selection work limit"));
        }
        let before_loss = self.loss(&validation);
        let before_correct = self.decisions(&validation);
        let mut selected = None;
        let mut trials = Vec::new();
        for rate in rates {
            let candidate = self.trained(&examples, epochs, rate);
            let loss = candidate.loss(&validation);
            let correct = candidate.decisions(&validation);
            trials.push(crate::obj! {"rate" => number(rate), "loss" => number(loss), "correct" => correct});
            if loss >= before_loss || correct < before_correct {
                continue;
            }
            let better = match &selected {
                None => true,
                Some((_, _, best_correct, best_loss)) => {
                    correct > *best_correct || (correct == *best_correct && loss < *best_loss)
                }
            };
            if better {
                selected = Some((rate, candidate, correct, loss));
            }
        }
        let accepted = selected.is_some();
        let mut selected_rate = Value::Null;
        let (mut after_loss, mut after_correct) = (before_loss, before_correct);
        if let Some((rate, candidate, correct, loss)) = selected {
            self.retain(candidate, id);
            selected_rate = number(rate);
            after_loss = loss;
            after_correct = correct;
        }
        Ok(crate::obj! {"accepted" => accepted, "revision" => self.revision,
        "before_loss" => number(before_loss), "after_loss" => number(after_loss),
        "before_correct" => before_correct, "after_correct" => after_correct,
        "validation_count" => validation.len(), "selected_rate" => selected_rate,
        "steps" => steps, "trials" => Value::Array(trials)})
    }

    /// Restores the most recent retained snapshot. Revision advances and consumed IDs
    /// stay consumed, so an undone lesson cannot be replayed. At most eight undos persist.
    pub fn undo(&mut self, expected_revision: usize) -> Result<Value> {
        if expected_revision != self.revision {
            return Err(err("stale brain revision"));
        }
        let (_, weights) = self.history.pop().ok_or_else(|| err("no undo history"))?;
        self.weights = weights;
        self.revision += 1;
        Ok(crate::obj! {"revision" => self.revision})
    }
}
