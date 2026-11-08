use super::*;
use std::sync::Arc;
/// A sealed single-scope/frame perception result. A new prepare or module replacement
/// invalidates older frames, including frames from an identical but separate bundle.
#[derive(Debug)]
pub struct PreparedFrame {
    owner: Arc<()>,
    scope: String,
    frame: u64,
    scores: Prediction,
}
/// Two independently owned/trainable modules, connected by explicit typed name ports.
/// Train cloned modules with existing APIs, then replace one atomically. Prepared frames
/// are ephemeral; persistence stores only modules and port bindings.
#[derive(Debug)]
pub struct BrainBundle {
    perception: Brain,
    decision: Brain,
    ports: Vec<(usize, usize)>,
    binding: Value,
    owner: Arc<()>,
    frame: u64,
}
impl BrainBundle {
    pub fn new(perception: Brain, decision: Brain, ports: &Value) -> Result<Self> {
        let rows = list(ports)?;
        if rows.is_empty() || rows.len() > MAX_INPUTS {
            return Err(err("bundle ports"));
        }
        let mut bindings = Vec::new();
        for p in rows {
            fields(p, &["input", "output"])?;
            let input = text(get(p, "input")?)?;
            let output = text(get(p, "output")?)?;
            let i = decision.inputs.iter().position(|n| n == input).ok_or_else(|| err("unknown bundle input"))?;
            let j = perception.outputs.iter().position(|n| n == output).ok_or_else(|| err("unknown bundle output"))?;
            if bindings.iter().any(|(old, _)| *old == i) {
                return Err(err("duplicate bundle port"));
            }
            bindings.push((i, j));
        }
        Ok(Self { perception, decision, ports: bindings, binding: ports.clone(), owner: Arc::new(()), frame: 0 })
    }
    pub fn perception(&self) -> &Brain {
        &self.perception
    }
    pub fn decision(&self) -> &Brain {
        &self.decision
    }
    pub fn replace_perception(&mut self, brain: Brain) -> Result<()> {
        let next = Self::new(brain, self.decision.clone(), &self.binding)?;
        self.perception = next.perception;
        self.ports = next.ports;
        self.owner = next.owner;
        Ok(())
    }
    pub fn replace_decision(&mut self, brain: Brain) -> Result<()> {
        let next = Self::new(self.perception.clone(), brain, &self.binding)?;
        self.decision = next.decision;
        self.ports = next.ports;
        self.owner = next.owner;
        Ok(())
    }
    pub fn prepare(&mut self, scope: &str, frame: u64, inputs: &[Option<i64>]) -> Result<PreparedFrame> {
        crate::checked::scope(scope)?;
        crate::checked::time(frame, self.frame)?;
        let mut scores = Prediction::default();
        self.perception.run_into(inputs, 0, 0, 0, &mut scores)?;
        self.owner = Arc::new(());
        self.frame = frame;
        Ok(PreparedFrame { owner: self.owner.clone(), scope: scope.into(), frame, scores })
    }
    pub fn prepare_json(&mut self, scope: &str, frame: u64, inputs: &Value) -> Result<PreparedFrame> {
        let features = self.perception.features(inputs, false)?;
        self.prepare(scope, frame, &features)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn run_into(
        &self,
        prepared: &PreparedFrame,
        scope: &str,
        frame: u64,
        candidate: &[Option<i64>],
        allowed: u16,
        min_score: i64,
        min_margin: i64,
        out: &mut Prediction,
    ) -> Result<()> {
        if !Arc::ptr_eq(&self.owner, &prepared.owner)
            || prepared.scope != scope
            || prepared.frame != frame
            || candidate.len() != self.decision.inputs.len()
            || self.ports.iter().any(|(i, _)| candidate[*i].is_some())
        {
            return Err(err("stale or incompatible bundle frame"));
        }
        let mut inputs = [None; MAX_INPUTS];
        inputs[..candidate.len()].copy_from_slice(candidate);
        for (i, j) in &self.ports {
            inputs[*i] = prepared.scores.scores()[*j]
        }
        self.decision.run_into(&inputs[..candidate.len()], allowed, min_score, min_margin, out)
    }
    pub fn run(
        &self,
        prepared: &PreparedFrame,
        scope: &str,
        frame: u64,
        candidate: &Value,
        allowed: &[&str],
    ) -> Result<Value> {
        let raw = candidate.as_object().ok_or_else(|| err("bundle candidate object"))?;
        if raw.len() != self.decision.inputs.len() - self.ports.len()
            || raw.iter().any(|(k, _)| self.ports.iter().any(|(i, _)| self.decision.inputs[*i] == *k))
        {
            return Err(err("candidate contains imported or missing inputs"));
        }
        let features = self.decision.features(candidate, false)?;
        let mut mask = 0u16;
        for a in allowed {
            let i = self.decision.outputs.iter().position(|n| n == a).ok_or_else(|| err("unknown allowed output"))?;
            if mask & (1 << i) != 0 {
                return Err(err("duplicate allowed output"));
            }
            mask |= 1 << i;
        }
        let mut prediction = Prediction::default();
        self.run_into(prepared, scope, frame, &features, mask, 0, 0, &mut prediction)?;
        Ok(self.decision.prediction_value(&prediction))
    }
    pub fn to_value(&self) -> Value {
        crate::obj! {"version"=>1,"perception"=>self.perception.to_value(),"decision"=>self.decision.to_value(),"ports"=>self.binding.clone()}
    }
    pub fn restore_json(text: &str) -> Result<Self> {
        Self::restore(&crate::data::json::parse(text, 2 * MAX_STATE_BYTES + 4096)?)
    }
    pub fn restore(v: &Value) -> Result<Self> {
        fields(v, &["version", "perception", "decision", "ports"])?;
        integer(get(v, "version")?, 1, 1)?;
        Self::new(Brain::restore(get(v, "perception")?)?, Brain::restore(get(v, "decision")?)?, get(v, "ports")?)
    }
}
