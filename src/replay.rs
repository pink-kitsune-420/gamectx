//! Bounded deterministic replay of normalized observations and explicit brain snapshots.
//! Labels and optional elapsed times are host data. Replay agreement is not a causal
//! gameplay evaluation; recorded latencies describe the original run, not replay speed.
use crate::{
    brain::Brain,
    json::{parse, Error, Result, Value},
    obj,
};

pub const MAX_TRACE_BYTES: usize = 1_048_576;
pub const MAX_EVENTS: usize = 256;
fn bad(s: &str) -> Error {
    Error::new(s)
}
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(|| bad("missing replay field"))
}
fn fields(v: &Value, keys: &[&str]) -> Result<()> {
    let entries = v.as_object().ok_or_else(|| bad("replay object required"))?;
    if entries.len() != keys.len()
        || keys.iter().any(|k| v.get(k).is_none())
        || entries
            .iter()
            .enumerate()
            .any(|(i, (k, _))| !keys.contains(&k.as_str()) || entries[..i].iter().any(|(old, _)| old == k))
    {
        return Err(bad("unexpected or missing replay field"));
    }
    Ok(())
}
fn uint(v: &Value, max: u64) -> Result<u64> {
    match v {
        Value::Number(s) if s.bytes().all(|c| c.is_ascii_digit()) => {
            s.parse::<u64>().ok().filter(|n| *n <= max).ok_or_else(|| bad("replay integer limit"))
        }
        _ => Err(bad("replay integer required")),
    }
}
fn signed(v: &Value) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().map_err(|_| bad("replay millionth integer required")),
        _ => Err(bad("replay millionth integer required")),
    }
}
struct Outcome {
    report: Value,
    brain: Option<Brain>,
    choice: Option<String>,
    run: bool,
    labeled: bool,
    correct: bool,
    elapsed: Option<u64>,
}
fn event(brain: &Brain, v: &Value) -> Result<Outcome> {
    let op = get(v, "op")?.as_str().ok_or_else(|| bad("replay op required"))?;
    uint(get(v, "tick")?, u32::MAX as u64)?;
    if op == "state" {
        fields(v, &["op", "tick", "state"])?;
        let raw = get(v, "state")?;
        if raw.json_len() > crate::brain::MAX_STATE_BYTES {
            return Err(bad("replay brain state limit"));
        }
        let next = Brain::restore(raw)?;
        return Ok(Outcome {
            report: obj! {"revision"=>next.revision()},
            brain: Some(next),
            choice: None,
            run: false,
            labeled: false,
            correct: false,
            elapsed: None,
        });
    }
    if op != "run" {
        return Err(bad("unknown replay op"));
    }
    fields(v, &["op", "tick", "inputs", "mask", "low", "gap", "expected", "elapsed_ns"])?;
    let inputs = get(v, "inputs")?.as_array().ok_or_else(|| bad("replay inputs required"))?;
    if inputs.len() > crate::brain::MAX_INPUTS {
        return Err(bad("replay input count"));
    }
    let values: Result<Vec<_>> =
        inputs.iter().map(|v| if v == &Value::Null { Ok(None) } else { signed(v).map(Some) }).collect();
    let mask = uint(get(v, "mask")?, u16::MAX as u64)? as u16;
    let report = brain.diagnose(&values?, mask, signed(get(v, "low")?)?, signed(get(v, "gap")?)?)?;
    let choice = report.get("choice").and_then(Value::as_str).map(str::to_owned);
    let expected = get(v, "expected")?;
    let labeled = expected != &Value::Null;
    let correct = if labeled {
        fields(expected, &["choice"])?;
        let expected = get(expected, "choice")?;
        let target = if expected == &Value::Null {
            None
        } else {
            Some(brain.outputs()[uint(expected, (brain.outputs().len() - 1) as u64)? as usize].as_str())
        };
        choice.as_deref() == target
    } else {
        false
    };
    let elapsed = match get(v, "elapsed_ns")? {
        Value::Null => None,
        v => Some(uint(v, 1_000_000_000_000)?),
    };
    Ok(Outcome { report, brain: None, choice, run: true, labeled, correct, elapsed })
}

/// Optional recorder. Each successful event is validated and appended atomically; errors
/// preserve the previous trace. Full traces stop at capacity instead of silently evicting
/// the initial state needed for replay. Snapshots are explicit events, never hidden mutation.
#[derive(Debug)]
pub struct Recorder {
    initial: Value,
    brain: Brain,
    events: Vec<Value>,
    capacity: usize,
    tick: u64,
    bytes: usize,
}
impl Recorder {
    pub fn new(brain: &Brain, capacity: usize) -> Result<Self> {
        if !(1..=MAX_EVENTS).contains(&capacity) {
            return Err(bad("replay capacity must be 1..256"));
        }
        let initial = brain.to_value();
        let bytes = (obj! {"version"=>1,"initial"=>initial.clone(),"events"=>Vec::<Value>::new()}).json_len();
        if bytes > MAX_TRACE_BYTES {
            return Err(bad("replay trace size limit"));
        }
        Ok(Self { initial, brain: brain.clone(), events: Vec::new(), capacity, tick: 0, bytes })
    }
    pub fn record(&mut self, raw: &Value) -> Result<Value> {
        if self.events.len() == self.capacity {
            return Err(bad("replay event capacity reached"));
        }
        let raw = parse(&raw.to_json(), MAX_TRACE_BYTES)?;
        let tick = uint(get(&raw, "tick")?, u32::MAX as u64)?;
        if tick < self.tick {
            return Err(bad("replay clock moved backward"));
        }
        let bytes = self.bytes + raw.json_len() + usize::from(!self.events.is_empty());
        if bytes > MAX_TRACE_BYTES {
            return Err(bad("replay trace size limit"));
        }
        let outcome = event(&self.brain, &raw)?;
        if let Some(next) = outcome.brain {
            self.brain = next;
        }
        self.events.push(raw);
        self.tick = tick;
        self.bytes = bytes;
        Ok(outcome.report)
    }
    pub fn to_value(&self) -> Value {
        obj! {"version"=>1,"initial"=>self.initial.clone(),"events"=>Value::Array(self.events.clone())}
    }
}

pub fn run_json(text: &str) -> Result<Value> {
    run(&parse(text, MAX_TRACE_BYTES)?)
}
pub fn run(trace: &Value) -> Result<Value> {
    // Round-trip validation also rejects malformed native number text/duplicate fields.
    let trace = parse(&trace.to_json(), MAX_TRACE_BYTES)?;
    fields(&trace, &["version", "initial", "events"])?;
    if uint(get(&trace, "version")?, 1)? != 1 {
        return Err(bad("replay version"));
    }
    let initial = get(&trace, "initial")?;
    if initial.json_len() > crate::brain::MAX_STATE_BYTES {
        return Err(bad("replay initial state limit"));
    }
    let mut brain = Brain::restore(initial)?;
    let events = get(&trace, "events")?.as_array().ok_or_else(|| bad("replay event list required"))?;
    if events.len() > MAX_EVENTS {
        return Err(bad("replay event limit"));
    }
    let (mut runs, mut labeled, mut correct, mut abstentions, mut switches) = (0, 0, 0, 0, 0);
    let mut previous = None;
    let mut tick = 0;
    let mut latencies = Vec::new();
    let mut reports = Vec::new();
    for row in events {
        let next = uint(get(row, "tick")?, u32::MAX as u64)?;
        if next < tick {
            return Err(bad("replay clock moved backward"));
        }
        tick = next;
        let outcome = event(&brain, row)?;
        if outcome.run {
            runs += 1;
            labeled += usize::from(outcome.labeled);
            correct += usize::from(outcome.correct);
            abstentions += usize::from(outcome.choice.is_none());
            if let (Some(old), Some(new)) = (&previous, &outcome.choice) {
                switches += usize::from(old != new);
            }
            previous = outcome.choice;
            if let Some(elapsed) = outcome.elapsed {
                latencies.push(elapsed);
            }
        } else {
            previous = None;
        }
        if let Some(next) = outcome.brain {
            brain = next;
        }
        reports.push(outcome.report);
    }
    latencies.sort_unstable();
    let percentile = |p: usize| -> Value {
        if latencies.is_empty() {
            Value::Null
        } else {
            Value::from(latencies[(latencies.len() * p).div_ceil(100) - 1])
        }
    };
    Ok(
        obj! {"reports"=>reports,"summary"=>obj!{"events"=>events.len(),"runs"=>runs,"labeled"=>labeled,"correct"=>correct,"abstentions"=>abstentions,"switches"=>switches,
        "latency_samples"=>latencies.len(),"p50_ns"=>percentile(50),"p95_ns"=>percentile(95),"max_ns"=>latencies.last().map_or(Value::Null,|n|Value::from(*n))},"state"=>brain.to_value()},
    )
}
