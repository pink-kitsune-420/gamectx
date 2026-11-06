use super::*;

#[derive(Clone, Copy, Debug)]
pub struct SelectionPolicy {
    pub allowed: u16,
    pub min_score: i64,
    pub min_margin: i64,
    pub urgent: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Selection {
    prediction: Prediction,
    choice: Option<usize>,
    reason: &'static str,
}
impl Selection {
    pub fn prediction(&self) -> &Prediction {
        &self.prediction
    }
    pub fn choice(&self) -> Option<usize> {
        self.choice
    }
    pub fn held(&self) -> bool {
        matches!(self.reason, "held_time" | "held_margin")
    }
    pub fn reason(&self) -> &str {
        self.reason
    }
}
/// Optional utility hysteresis for one owned brain. Current eligibility and inference
/// abstention always apply; urgent updates bypass all holding. Utility may be sacrificed.
#[derive(Debug)]
pub struct StableBrain {
    brain: Brain,
    margin: i64,
    hold: u64,
    previous: Option<usize>,
    tick: Option<u64>,
    switched: u64,
}
impl StableBrain {
    pub fn new(brain: Brain, switch_margin: i64, min_hold_ticks: u64) -> Result<Self> {
        if !(0..=S).contains(&switch_margin) || min_hold_ticks > 1_000_000 {
            return Err(err("stability limits"));
        }
        Ok(Self { brain, margin: switch_margin, hold: min_hold_ticks, previous: None, tick: None, switched: 0 })
    }
    pub fn brain(&self) -> &Brain {
        &self.brain
    }
    pub fn brain_mut(&mut self) -> &mut Brain {
        self.previous = None;
        &mut self.brain
    }
    pub fn into_brain(self) -> Brain {
        self.brain
    }
    pub fn reset(&mut self) {
        self.previous = None;
    }
    pub fn run_into(
        &mut self,
        tick: u64,
        inputs: &[Option<i64>],
        policy: SelectionPolicy,
        out: &mut Selection,
    ) -> Result<()> {
        if self.tick.is_some_and(|previous| tick < previous) {
            return Err(err("stability clock moved backward"));
        }
        let mut prediction = Prediction::default();
        self.brain.run_into(inputs, policy.allowed, policy.min_score, policy.min_margin, &mut prediction)?;
        let mut choice = prediction.choice();
        let mut reason = "raw";
        if choice.is_none() {
            reason = "abstained";
        } else if policy.urgent {
            reason = "urgent";
        } else if let Some(old) = self.previous {
            let valid =
                policy.allowed & (1 << old) != 0 && prediction.scores()[old].is_some_and(|s| s > policy.min_score);
            if !valid {
                reason = "ineligible";
            } else if let Some(new) = choice.filter(|new| *new != old) {
                let gain = prediction.scores()[new].expect("chosen score")
                    - prediction.scores()[old].expect("valid old score");
                if tick.saturating_sub(self.switched) < self.hold {
                    choice = Some(old);
                    reason = "held_time";
                } else if gain <= self.margin {
                    choice = Some(old);
                    reason = "held_margin";
                }
            }
        }
        if choice != self.previous {
            self.switched = tick;
        }
        self.previous = choice;
        self.tick = Some(tick);
        *out = Selection { prediction, choice, reason };
        Ok(())
    }
}
