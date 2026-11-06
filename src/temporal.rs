//! Bounded native event windows. The host guarantees observation coverage and resets
//! availability on privacy/subject changes. Reads never create samples or renew time.
use crate::{checked::*, obj, Result, Value};
#[derive(Clone, Debug)]
struct Event {
    token: u64,
    at: u64,
    expires: u64,
    kind: String,
    value: i64,
}
#[derive(Clone, Debug)]
pub struct TemporalWindow {
    capacity: usize,
    window: u64,
    start: u64,
    clock: u64,
    last: u64,
    available: bool,
    events: Vec<Event>,
}
impl TemporalWindow {
    pub fn new(capacity: usize, window: u64, start: u64) -> Result<Self> {
        time(start, 0)?;
        if !(1..=256).contains(&capacity) || !(1..=1000000).contains(&window) {
            return Err(bad());
        }
        Ok(Self { capacity, window, start, clock: start, last: 0, available: true, events: Vec::new() })
    }
    fn current(&self, now: u64) -> impl Iterator<Item = &Event> {
        self.events.iter().filter(move |e| e.at <= now && now < e.expires && now - e.at < self.window)
    }
    pub fn availability(&mut self, now: u64, available: bool) -> Result<()> {
        time(now, self.clock)?;
        if self.available != available {
            self.available = available;
            self.events.clear();
            self.start = now
        }
        self.clock = now;
        Ok(())
    }
    pub fn observe(&mut self, token: u64, now: u64, expires: u64, kind: &str, value: i64) -> Result<()> {
        time(now, self.clock)?;
        name(kind)?;
        if !self.available
            || token <= self.last
            || token > u32::MAX as u64
            || expires <= now
            || expires > u32::MAX as u64
            || !(-1000000000..=1000000000).contains(&value)
            || self.current(now).count() >= self.capacity
        {
            return Err(bad());
        }
        self.events.retain(|e| now < e.expires && now - e.at < self.window);
        self.events.push(Event { token, at: now, expires, kind: kind.into(), value });
        self.last = token;
        self.clock = now;
        Ok(())
    }
    pub fn features(&self, kind: &str, now: u64) -> Result<Value> {
        time(now, self.clock)?;
        name(kind)?;
        let rows: Vec<_> = self.current(now).filter(|e| e.kind == kind).collect();
        let count = rows.len() as u64;
        let span = self.window.min(now - self.start + 1);
        Ok(
            obj! {"count"=>if self.available{Value::from(count)}else{Value::Null},"rate_millionths"=>if self.available{Value::from(count*1000000/span)}else{Value::Null},"first"=>rows.first().map_or(Value::Null,|e|Value::from(e.at)),"last"=>rows.last().map_or(Value::Null,|e|Value::from(e.at)),"delta"=>rows.first().zip(rows.last()).map_or(Value::Null,|(a,b)|Value::from(b.value-a.value))},
        )
    }
    pub fn sequence(&self, kinds: &[String], now: u64) -> Result<Value> {
        time(now, self.clock)?;
        if kinds.is_empty() || kinds.len() > 8 {
            return Err(bad());
        }
        for k in kinds {
            name(k)?
        }
        if !self.available {
            return Ok(Value::Null);
        }
        let (mut position, mut count) = (0, 0u64);
        for e in self.current(now) {
            if e.kind == kinds[position] {
                position += 1;
                if position == kinds.len() {
                    count += 1;
                    position = 0
                }
            }
        }
        Ok(Value::from(count))
    }
    /// Native source watermark, retained across availability resets.
    pub fn last_token(&self) -> u64 {
        self.events.last().map_or(self.last, |e| e.token)
    }
}
