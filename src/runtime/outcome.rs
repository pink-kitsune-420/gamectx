//! Native execution/exposure gates for learning labels. Scores are host measurements,
//! not evidence that the action caused a response. Pending trials do not survive restart.
use crate::{checked::*, obj, Result, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Planned,
    Executed,
    Exposed,
    Observed,
    Cancelled,
}
#[derive(Clone, Debug)]
struct Trial {
    id: u64,
    deadline: u64,
    window: u64,
    phase: Phase,
    score: Option<u64>,
    consumed: bool,
}
#[derive(Clone, Debug)]
pub struct OutcomeTracker {
    capacity: usize,
    last: u64,
    clock: u64,
    rows: Vec<Trial>,
}
impl OutcomeTracker {
    pub fn new(capacity: usize) -> Result<Self> {
        if !(1..=128).contains(&capacity) {
            return Err(bad());
        }
        Ok(Self { capacity, last: 0, clock: 0, rows: Vec::new() })
    }
    pub fn begin(&mut self, id: u64, now: u64, wait: u64, window: u64) -> Result<()> {
        time(now, self.clock)?;
        if id <= self.last
            || id > u32::MAX as u64
            || !(1..=1000000).contains(&wait)
            || !(1..=1000000).contains(&window)
            || now + wait + window > u32::MAX as u64
            || self.rows.len() == self.capacity
        {
            return Err(bad());
        }
        self.rows.push(Trial { id, deadline: now + wait, window, phase: Phase::Planned, score: None, consumed: false });
        self.last = id;
        self.clock = now;
        Ok(())
    }
    fn transition(&mut self, id: u64, now: u64, from: Phase, to: Phase, score: Option<u64>) -> Result<()> {
        time(now, self.clock)?;
        let row = self.rows.iter_mut().find(|r| r.id == id).ok_or_else(bad)?;
        if now >= row.deadline || row.phase != from || score.is_some_and(|s| s > 1000000) {
            return Err(bad());
        }
        row.phase = to;
        row.score = score;
        if to == Phase::Exposed {
            row.deadline = now + row.window
        }
        self.clock = now;
        Ok(())
    }
    pub fn executed(&mut self, id: u64, now: u64) -> Result<()> {
        self.transition(id, now, Phase::Planned, Phase::Executed, None)
    }
    pub fn exposed(&mut self, id: u64, now: u64) -> Result<()> {
        self.transition(id, now, Phase::Executed, Phase::Exposed, None)
    }
    pub fn resolve(&mut self, id: u64, now: u64, score: u64) -> Result<()> {
        self.transition(id, now, Phase::Exposed, Phase::Observed, Some(score))
    }
    pub fn cancel(&mut self, id: u64, now: u64) -> Result<()> {
        time(now, self.clock)?;
        let row = self.rows.iter_mut().find(|r| r.id == id).ok_or_else(bad)?;
        if matches!(row.phase, Phase::Observed | Phase::Cancelled) {
            return Err(bad());
        }
        row.phase = Phase::Cancelled;
        self.clock = now;
        Ok(())
    }
    pub fn forget(&mut self, id: u64, now: u64) -> Result<()> {
        time(now, self.clock)?;
        let i = self.rows.iter().position(|r| r.id == id).ok_or_else(bad)?;
        self.rows.remove(i);
        self.clock = now;
        Ok(())
    }
    pub fn report(&self, id: u64, now: u64) -> Result<Value> {
        time(now, self.clock)?;
        let r = self.rows.iter().find(|r| r.id == id).ok_or_else(bad)?;
        let status = if r.phase == Phase::Observed {
            "observed"
        } else if r.phase == Phase::Cancelled || now >= r.deadline {
            "unknown"
        } else {
            match r.phase {
                Phase::Planned => "planned",
                Phase::Executed => "executed",
                Phase::Exposed => "exposed",
                _ => unreachable!(),
            }
        };
        Ok(
            obj! {"id"=>id,"status"=>status,"score"=>if status=="observed"{r.score.map_or(Value::Null,Value::from)}else{Value::Null},"trainable"=>status=="observed"&&!r.consumed},
        )
    }
    /// Consumes one observed label exactly once. The host joins this ID to the
    /// original features/action and applies its existing validation/training policy.
    pub fn take_label(&mut self, id: u64, now: u64) -> Result<Value> {
        let result = self.report(id, now)?;
        let r = self.rows.iter_mut().find(|r| r.id == id).ok_or_else(bad)?;
        if r.phase != Phase::Observed || r.consumed {
            return Err(bad());
        }
        r.consumed = true;
        self.clock = now;
        Ok(result)
    }
}
