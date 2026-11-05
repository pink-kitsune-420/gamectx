//! Conservative input-token estimates that learn from what a provider actually reports.
//!
//! Every model family tokenizes differently and exact counting needs that provider's own
//! tokenizer, so this never guesses low. Uncalibrated, the estimate is the byte count (real
//! tokenizers average well under one token per byte). After each reply that reports its
//! input-token usage, [`Calibration::observe`] records the worst tokens-per-byte ratio seen
//! for that provider/model scope, and estimates use it with a 2x safety margin.

use crate::fingerprint_u64;
use crate::json::{Error, Result, Value};

/// Safety multiplier applied to the learned ratio.
pub const MARGIN: u64 = 2;
pub const MAX_SAMPLES: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Calibration {
    /// Fingerprint of the provider/model/endpoint the ratio belongs to (0 = none).
    pub scope: u64,
    pub samples: u32,
    /// Worst observed input tokens per byte, in parts per million.
    pub ratio_ppm: u32,
}

/// Scope fingerprint for a provider/model combination. Only the hash is stored, never the names.
pub fn scope(parts: &[&str]) -> u64 {
    match fingerprint_u64(&parts.join("\n")) {
        0 => 1,
        s => s,
    }
}

impl Calibration {
    pub const NONE: Calibration = Calibration { scope: 0, samples: 0, ratio_ppm: 0 };

    /// Conservative token estimate for `body` plus a fixed `framing` allowance (message
    /// wrappers, tool overhead). Non-ASCII bytes always count as one token each, and the
    /// result never exceeds the uncalibrated byte bound.
    pub fn estimate(&self, scope: u64, body: &[u8], framing: usize) -> usize {
        let bound = body.len() + framing;
        if self.samples == 0 || self.scope != scope {
            return bound;
        }
        let ascii = body.iter().filter(|b| b.is_ascii()).count() as u64;
        let learned = (ascii * self.ratio_ppm as u64 * MARGIN).div_ceil(1_000_000) as usize
            + (body.len() - ascii as usize)
            + framing;
        learned.min(bound)
    }

    /// Learns from one reply. A report above the estimate that was used, or above one token
    /// per byte, means the model changed or the estimate failed, so calibration resets.
    pub fn observe(self, scope: u64, body_bytes: usize, input_tokens: u64, estimate: usize) -> Calibration {
        if input_tokens == 0 {
            return self; // Missing usage teaches nothing; never learn a zero ratio.
        }
        if body_bytes == 0 || input_tokens > estimate as u64 || input_tokens > body_bytes as u64 {
            return Calibration::NONE;
        }
        let base = if self.scope == scope { self } else { Calibration::NONE };
        let ratio = (input_tokens * 1_000_000).div_ceil(body_bytes as u64).min(1_000_000) as u32;
        Calibration { scope, samples: (base.samples + 1).min(MAX_SAMPLES), ratio_ppm: base.ratio_ppm.max(ratio) }
    }

    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("scope", Value::str(format!("{:016x}", self.scope))),
            ("samples", Value::int(self.samples as i64)),
            ("ratio_ppm", Value::int(self.ratio_ppm as i64)),
        ])
    }

    pub fn from_value(value: &Value) -> Result<Calibration> {
        let bad = || Error::new("calibration needs scope (16 hex digits), samples and ratio_ppm");
        let scope =
            value.get("scope").and_then(Value::as_str).and_then(|s| u64::from_str_radix(s, 16).ok()).ok_or_else(bad)?;
        let samples =
            value.get("samples").and_then(Value::as_usize).filter(|s| *s <= MAX_SAMPLES as usize).ok_or_else(bad)?
                as u32;
        let ratio_ppm =
            value.get("ratio_ppm").and_then(Value::as_usize).filter(|r| *r <= 1_000_000).ok_or_else(bad)? as u32;
        if samples == 0 && (scope != 0 || ratio_ppm != 0) {
            return Err(bad());
        }
        Ok(Calibration { scope, samples, ratio_ppm })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncalibrated_is_the_byte_bound() {
        assert_eq!(Calibration::NONE.estimate(7, b"hello", 10), 15);
    }

    #[test]
    fn learns_worst_ratio_with_margin() {
        let s = scope(&["provider", "model"]);
        let body = vec![b'a'; 1000];
        let c = Calibration::NONE.observe(s, 1000, 250, 1000).observe(s, 1000, 200, 1000);
        assert_eq!(c.ratio_ppm, 250_000);
        assert_eq!(c.estimate(s, &body, 0), 500);
        assert_eq!(c.estimate(scope(&["other"]), &body, 0), 1000, "other scopes stay uncalibrated");
    }

    #[test]
    fn resets_on_overrun_and_round_trips() {
        let s = scope(&["p"]);
        let c = Calibration::NONE.observe(s, 1000, 250, 1000);
        assert_eq!(c.observe(s, 1000, 600, 500), Calibration::NONE);
        assert_eq!(c.observe(s, 1000, 0, 500), c);
        assert_eq!(Calibration::from_value(&c.to_value()).unwrap(), c);
    }
}
