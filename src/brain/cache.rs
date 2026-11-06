use super::*;
use std::collections::{HashMap, VecDeque};

pub const MAX_CACHE_ENTRIES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    inputs: Vec<Option<i64>>,
    allowed: u16,
    min_score: i64,
    min_margin: i64,
    revision: usize,
}

/// Lifetime hit/miss counts and current bounded occupancy. Clearing entries does not
/// reset counters; invalid requests are neither hits nor misses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
    pub capacity: usize,
}

/// Opt-in FIFO memoization for one owned, preloaded brain. Keys use normalized inputs,
/// eligible outputs, thresholds and revision. Entries are ephemeral and never saved as
/// learning. Repeated states can benefit; constantly changing inputs add cache overhead.
#[derive(Debug)]
pub struct CachedBrain {
    brain: Brain,
    capacity: usize,
    entries: HashMap<Key, Value>,
    order: VecDeque<Key>,
    hits: u64,
    misses: u64,
}

impl CachedBrain {
    /// Takes ownership of an already parsed/preloaded brain. Capacity is 0..256; zero
    /// disables caching. Each owner keeps its own cache and learned state.
    pub fn new(brain: Brain, capacity: usize) -> Result<Self> {
        if capacity > MAX_CACHE_ENTRIES {
            return Err(err("brain cache capacity exceeds 256"));
        }
        Ok(Self {
            brain,
            capacity,
            entries: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            hits: 0,
            misses: 0,
        })
    }

    pub fn brain(&self) -> &Brain {
        &self.brain
    }

    /// Clears cached predictions before exposing mutable access. This covers training,
    /// lessons, undo and even replacement with a different brain at the same revision.
    /// Rejected updates conservatively clear entries too.
    pub fn brain_mut(&mut self) -> &mut Brain {
        self.clear();
        &mut self.brain
    }

    pub fn into_brain(self) -> Brain {
        self.brain
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats { hits: self.hits, misses: self.misses, entries: self.entries.len(), capacity: self.capacity }
    }

    pub fn run(&mut self, inputs: &Value, allowed: &[&str]) -> Result<Value> {
        self.run_with(inputs, allowed, 0.0, 0.0)
    }

    pub fn run_with(&mut self, inputs: &Value, allowed: &[&str], min_score: f64, min_margin: f64) -> Result<Value> {
        if self.capacity == 0 {
            let result = self.brain.run_with(inputs, allowed, min_score, min_margin)?;
            self.misses = self.misses.saturating_add(1);
            return Ok(result);
        }
        // Match the normal validation order, before looking up any cached value.
        if allowed.len() > self.brain.outputs.len()
            || allowed
                .iter()
                .enumerate()
                .any(|(i, key)| !self.brain.outputs.iter().any(|out| out == key) || allowed[..i].contains(key))
        {
            return Err(err("invalid eligible outputs"));
        }
        if !min_score.is_finite() || !min_margin.is_finite() {
            return Err(err("thresholds must be finite"));
        }
        let low = ticks(&Value::from(min_score), 0, S)?;
        let gap = ticks(&Value::from(min_margin), 0, S)?;
        let features = self.brain.features(inputs, false)?;
        let mut mask = 0;
        for (index, name) in self.brain.outputs.iter().enumerate() {
            if allowed.contains(&name.as_str()) {
                mask |= 1u16 << index;
            }
        }
        let key =
            Key { inputs: features, allowed: mask, min_score: low, min_margin: gap, revision: self.brain.revision };
        if let Some(result) = self.entries.get(&key) {
            self.hits = self.hits.saturating_add(1);
            return Ok(result.clone());
        }
        let result = self.brain.run_with(inputs, allowed, min_score, min_margin)?;
        self.misses = self.misses.saturating_add(1);
        if self.entries.len() == self.capacity {
            let oldest = self.order.pop_front().expect("nonzero full cache has an entry");
            self.entries.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, result.clone());
        Ok(result)
    }
}
