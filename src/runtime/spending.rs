//! Integer microdollar reservations. The host dispatches requests and durably saves
//! state before network I/O. Interrupted reservations retain their worst-case cost.
use crate::{checked::*, obj, Result, Value};
#[derive(Clone, Debug)]
struct Charge {
    id: u64,
    at: u64,
    amount: u64,
    state: String,
}
#[derive(Debug)]
pub struct SpendingLedger {
    limit: u64,
    window: u64,
    clock: u64,
    next: u64,
    blocked: bool,
    entries: Vec<Charge>,
}
impl SpendingLedger {
    pub fn new(limit: u64, window: u64) -> Result<Self> {
        if limit > 1000000000000 || !(1..=1000000).contains(&window) {
            return Err(bad());
        }
        Ok(Self { limit, window, clock: 0, next: 0, blocked: false, entries: Vec::new() })
    }
    pub fn reserve(&mut self, now: u64, amount: u64) -> Result<u64> {
        time(now, self.clock)?;
        let current = self.entries.iter().filter(|r| r.state != "settled" || now - r.at < self.window);
        let total: u64 = current.clone().map(|r| r.amount).sum();
        if amount > 1000000000000
            || current.count() >= 256
            || self.next >= u32::MAX as u64
            || amount > 0 && (self.blocked || total + amount > self.limit)
        {
            return Err(bad());
        }
        self.entries.retain(|r| r.state != "settled" || now - r.at < self.window);
        self.next += 1;
        self.entries.push(Charge { id: self.next, at: now, amount, state: "reserved".into() });
        self.clock = now;
        Ok(self.next)
    }
    pub fn dispatch(&mut self, id: u64, now: u64) -> Result<()> {
        time(now, self.clock)?;
        let row = self.entries.iter_mut().find(|r| r.id == id).ok_or_else(bad)?;
        if row.state != "reserved" {
            return Err(bad());
        }
        row.state = "dispatched".into();
        self.clock = now;
        Ok(())
    }
    pub fn cancel_before_dispatch(&mut self, id: u64, now: u64) -> Result<()> {
        time(now, self.clock)?;
        let i = self.entries.iter().position(|r| r.id == id).ok_or_else(bad)?;
        if self.entries[i].state != "reserved" {
            return Err(bad());
        }
        self.entries.remove(i);
        self.clock = now;
        Ok(())
    }
    pub fn finish(&mut self, id: u64, now: u64, actual: Option<u64>) -> Result<()> {
        time(now, self.clock)?;
        if actual.is_some_and(|n| n > 1000000000000) {
            return Err(bad());
        }
        let row = self.entries.iter_mut().find(|r| r.id == id).ok_or_else(bad)?;
        if row.state != "dispatched" {
            return Err(bad());
        }
        let amount = actual.unwrap_or(row.amount);
        self.blocked |= amount > row.amount;
        row.amount = amount;
        row.at = now;
        row.state = "settled".into();
        self.clock = now;
        Ok(())
    }
    /// Crash recovery: both reserved and dispatched requests are ambiguous and charged.
    pub fn recover(&mut self, now: u64) -> Result<()> {
        time(now, self.clock)?;
        for r in &mut self.entries {
            if r.state != "settled" {
                r.state = "settled".into();
                r.at = now
            }
        }
        self.clock = now;
        Ok(())
    }
    pub fn summary(&self, now: u64) -> Result<Value> {
        time(now, self.clock)?;
        let spent: u64 =
            self.entries.iter().filter(|r| r.state != "settled" || now - r.at < self.window).map(|r| r.amount).sum();
        Ok(
            obj! {"spent"=>spent,"blocked"=>self.blocked,"pending"=>self.entries.iter().filter(|r|r.state!="settled").count()},
        )
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1,"limit"=>self.limit,"window"=>self.window,"clock"=>self.clock,"next"=>self.next,"blocked"=>self.blocked,"entries"=>self.entries.iter().map(|r|obj!{"id"=>r.id,"at"=>r.at,"amount"=>r.amount,"state"=>r.state.as_str()}).collect::<Vec<_>>()}
    }
    /// Loads and conservatively settles unfinished reservations at the host recovery time.
    pub fn restore_json(text: &str, now: u64) -> Result<Self> {
        Self::restore(&crate::data::json::parse(text, 65536)?, now)
    }
    pub fn restore(v: &Value, now: u64) -> Result<Self> {
        fields(v, &["version", "limit", "window", "clock", "next", "blocked", "entries"])?;
        uint(get(v, "version")?, 1, 1)?;
        let mut out = Self::new(uint(get(v, "limit")?, 0, 1000000000000)?, uint(get(v, "window")?, 1, 1000000)?)?;
        out.clock = uint(get(v, "clock")?, 0, u32::MAX as u64)?;
        out.next = uint(get(v, "next")?, 0, u32::MAX as u64)?;
        out.blocked = boolean(get(v, "blocked")?)?;
        let rows = list(get(v, "entries")?)?;
        if rows.len() > 256 {
            return Err(bad());
        }
        let mut previous = 0;
        for r in rows {
            fields(r, &["id", "at", "amount", "state"])?;
            let id = uint(get(r, "id")?, previous + 1, out.next)?;
            let at = uint(get(r, "at")?, 0, out.clock)?;
            let amount = uint(get(r, "amount")?, 0, 1000000000000)?;
            let state = text(get(r, "state")?)?;
            if !["reserved", "dispatched", "settled"].contains(&state) {
                return Err(bad());
            }
            previous = id;
            out.entries.push(Charge { id, at, amount, state: state.into() });
        }
        let total: u64 = out
            .entries
            .iter()
            .filter(|r| r.state != "settled" || out.clock - r.at < out.window)
            .map(|r| r.amount)
            .sum();
        if total > out.limit && !out.blocked {
            return Err(bad());
        }
        out.recover(now)?;
        Ok(out)
    }
}
