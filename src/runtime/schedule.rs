//! Optional event-driven work scheduling. Hosts supply monotonic ticks, estimated costs
//! and change/urgency events. A poll reserves work; execute returned jobs or mark them
//! changed again on failure. Costs are work units, not wall-clock guarantees.
use crate::{Error, Result};

#[derive(Clone, Debug)]
struct Job {
    id: u64,
    cost: usize,
    interval: u64,
    max_age: u64,
    next: u64,
    last: Option<u64>,
    dirty: bool,
    urgent: bool,
}

/// Bounded, deterministic scheduling with per-frame budgets and periodic refresh.
#[derive(Clone, Debug)]
pub struct Scheduler {
    jobs: Vec<Job>,
    capacity: usize,
    budget: usize,
    tick: Option<u64>,
    remaining: usize,
}

impl Scheduler {
    pub fn new(capacity: usize, frame_budget: usize) -> Result<Self> {
        if !(1..=4096).contains(&capacity) || !(1..=2_000_000).contains(&frame_budget) {
            return Err(Error::new("scheduler limits"));
        }
        Ok(Self { jobs: Vec::new(), capacity, budget: frame_budget, tick: None, remaining: frame_budget })
    }
    /// Initially dirty, first eligible at absolute `phase` tick. Interval and max_age
    /// are positive; phase < max_age. Urgency bypasses the interval, never the budget.
    pub fn register(&mut self, id: u64, cost: usize, interval: u64, max_age: u64, phase: u64) -> Result<()> {
        if self.jobs.len() == self.capacity
            || self.jobs.iter().any(|j| j.id == id)
            || cost == 0
            || cost > self.budget
            || interval == 0
            || interval > max_age
            || phase >= max_age
        {
            return Err(Error::new("invalid scheduler registration"));
        }
        self.jobs.push(Job { id, cost, interval, max_age, next: phase, last: None, dirty: true, urgent: false });
        Ok(())
    }
    pub fn remove(&mut self, id: u64) -> bool {
        let len = self.jobs.len();
        self.jobs.retain(|j| j.id != id);
        len != self.jobs.len()
    }
    /// Changes include observations, brain revisions, eligibility and thresholds.
    pub fn changed(&mut self, id: u64, urgent: bool) -> Result<()> {
        let job = self.jobs.iter_mut().find(|j| j.id == id).ok_or_else(|| Error::new("unknown scheduled job"))?;
        job.dirty = true;
        job.urgent |= urgent;
        Ok(())
    }
    pub fn remaining(&self) -> usize {
        self.remaining
    }
    /// Urgent jobs first, then overdue jobs, then least recently served, then ID.
    /// Multiple polls at the same tick share a budget and cannot repeat the same job.
    /// Overload can delay deadlines; budgets are always respected. Backward time errors.
    pub fn poll(&mut self, tick: u64) -> Result<Vec<u64>> {
        if self.tick.is_some_and(|old| tick < old) {
            return Err(Error::new("scheduler clock moved backward"));
        }
        if self.tick != Some(tick) {
            self.tick = Some(tick);
            self.remaining = self.budget;
        }
        let mut eligible: Vec<_> = self
            .jobs
            .iter()
            .enumerate()
            .filter_map(|(i, j)| {
                let overdue = j.last.is_some_and(|last| tick.saturating_sub(last) >= j.max_age);
                (j.last != Some(tick) && (j.urgent || tick >= j.next && (j.dirty || overdue))).then_some((
                    !j.urgent,
                    !overdue,
                    j.last.unwrap_or(0),
                    j.id,
                    i,
                ))
            })
            .collect();
        eligible.sort_unstable();
        let mut selected = Vec::new();
        for (_, _, _, _, i) in eligible {
            let j = &mut self.jobs[i];
            if j.cost > self.remaining {
                continue;
            }
            self.remaining -= j.cost;
            selected.push(j.id);
            j.last = Some(tick);
            j.next = tick.saturating_add(j.interval);
            j.dirty = false;
            j.urgent = false;
        }
        Ok(selected)
    }
}
