//! Additive complete-turn telemetry. This is separate from the kit's frozen v1 evaluator.
//! Observations and independent expected answers are host-supplied; missing metrics remain
//! missing. Reported RSS is not a sum of uniquely owned physical pages.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
const METRICS: [&str; 6] =
    ["input_tokens", "output_tokens", "cached_tokens", "prompt_micros", "decode_micros", "load_micros"];
const MEMORY: [&str; 5] = ["runner_rss_bytes", "game_rss_bytes", "available_bytes", "swap_in_bytes", "swap_out_bytes"];
fn list<'a>(v: &'a Value, k: &str) -> Result<&'a [Value]> {
    c::list(c::get(v, k)?)
}
fn optional(v: &Value, high: u64) -> Result<Option<u64>> {
    if *v == Value::Null {
        Ok(None)
    } else {
        c::uint(v, 0, high).map(Some)
    }
}
fn number(n: Option<u64>) -> Value {
    n.map(Value::from).unwrap_or(Value::Null)
}
fn median(mut values: Vec<u64>) -> Option<u64> {
    if values.is_empty() {
        None
    } else {
        values.sort_unstable();
        let n = values.len();
        Some((values[(n - 1) / 2] + values[n / 2]) / 2)
    }
}
pub fn report(v: &Value) -> Result<Value> {
    bounded(v, 1_572_864)?;
    c::fields(v, &["version", "trials"])?;
    c::uint(c::get(v, "version")?, 1, 1)?;
    let trials = list(v, "trials")?;
    if trials.is_empty() || trials.len() > 512 {
        return Err(c::bad());
    }
    let mut groups: Vec<(&str, Vec<&Value>)> = Vec::new();
    let mut ids = Vec::new();
    for t in trials {
        c::fields(
            t,
            &[
                "id",
                "condition",
                "expected",
                "actual",
                "elapsed_micros",
                "ttft_micros",
                "attempts",
                "fallback",
                "required",
                "supplied",
                "memory",
                "dialogue_score",
            ],
        )?;
        let id = c::text(c::get(t, "id")?)?;
        c::scope(id)?;
        if ids.contains(&id) {
            return Err(c::bad());
        }
        ids.push(id);
        let name = c::text(c::get(t, "condition")?)?;
        c::scope(name)?;
        let elapsed = c::uint(c::get(t, "elapsed_micros")?, 0, 1_000_000_000_000)?;
        optional(c::get(t, "ttft_micros")?, elapsed)?;
        c::boolean(c::get(t, "fallback")?)?;
        for k in ["required", "supplied"] {
            let a = list(t, k)?;
            if a.len() > 64 {
                return Err(c::bad());
            }
            for (i, x) in a.iter().enumerate() {
                c::scope(c::text(x)?)?;
                if a[..i].contains(x) {
                    return Err(c::bad());
                }
            }
        }
        optional(c::get(t, "dialogue_score")?, 5)?;
        let memory = c::get(t, "memory")?;
        c::fields(memory, &MEMORY)?;
        for k in MEMORY {
            optional(c::get(memory, k)?, (1u64 << 53) - 1)?;
        }
        let attempts = list(t, "attempts")?;
        if attempts.len() > 8 {
            return Err(c::bad());
        }
        let mut phases = Vec::new();
        for a in attempts {
            c::fields(
                a,
                &[
                    "phase",
                    "input_tokens",
                    "output_tokens",
                    "cached_tokens",
                    "prompt_micros",
                    "decode_micros",
                    "load_micros",
                    "valid",
                    "truncated",
                ],
            )?;
            let phase = c::text(c::get(a, "phase")?)?;
            if !["decision", "query", "narration", "preflight"].contains(&phase)
                || phases.iter().filter(|p| **p == phase).count() >= if phase == "preflight" { 4 } else { 2 }
            {
                return Err(c::bad());
            }
            phases.push(phase);
            for k in METRICS {
                optional(c::get(a, k)?, 1_000_000_000_000)?;
            }
            c::boolean(c::get(a, "valid")?)?;
            c::boolean(c::get(a, "truncated")?)?;
        }
        if let Some((_, rows)) = groups.iter_mut().find(|(n, _)| *n == name) {
            rows.push(t);
        } else {
            groups.push((name, vec![t]));
        }
    }
    let mut reports = Vec::new();
    for (name, rows) in groups {
        let mut elapsed = rows
            .iter()
            .map(|t| c::uint(c::get(t, "elapsed_micros")?, 0, 1_000_000_000_000))
            .collect::<Result<Vec<_>>>()?;
        elapsed.sort_unstable();
        let ttft = rows
            .iter()
            .map(|t| optional(c::get(t, "ttft_micros")?, 1_000_000_000_000))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let attempts = rows
            .iter()
            .map(|t| list(t, "attempts"))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut retries = 0usize;
        for t in &rows {
            let a = list(t, "attempts")?;
            let mut phases = Vec::new();
            for x in a {
                let p = c::text(c::get(x, "phase")?)?;
                if p == "preflight" {
                    continue;
                }
                if phases.contains(&p) {
                    retries += 1;
                } else {
                    phases.push(p);
                }
            }
        }
        let correct =
            rows.iter().filter(|t| t.get("expected").expect("expected").same(t.get("actual").expect("actual"))).count();
        let invalid = attempts.iter().filter(|a| a.get("valid") == Some(&Value::Bool(false))).count();
        let fallback = rows.iter().filter(|t| t.get("fallback") == Some(&Value::Bool(true))).count();
        let truncated = attempts
            .iter()
            .filter(|a| {
                a.get("phase").and_then(Value::as_str) != Some("preflight")
                    && a.get("truncated") == Some(&Value::Bool(true))
            })
            .count();
        let mut missing = 0usize;
        for t in &rows {
            for id in list(t, "required")? {
                if !list(t, "supplied")?.contains(id) {
                    missing += 1;
                }
            }
        }
        let mut fields = vec![
            ("name".into(), Value::str(name)),
            ("trials".into(), Value::from(rows.len())),
            ("correct".into(), Value::from(correct)),
            ("invalid_attempts".into(), Value::from(invalid)),
            ("retries".into(), Value::from(retries)),
            ("fallbacks".into(), Value::from(fallback)),
            ("truncated".into(), Value::from(truncated)),
            ("missing_required".into(), Value::from(missing)),
            ("median_turn_micros".into(), number(median(elapsed.clone()))),
            ("p95_turn_micros".into(), Value::from(elapsed[(95 * elapsed.len()).div_ceil(100) - 1])),
            ("median_ttft_micros".into(), number(median(ttft.clone()))),
            ("ttft_samples".into(), Value::from(ttft.len())),
        ];
        for key in METRICS {
            let values =
                attempts.iter().map(|a| optional(c::get(a, key)?, 1_000_000_000_000)).collect::<Result<Vec<_>>>()?;
            let total = if values.is_empty() || values.iter().any(Option::is_none) {
                None
            } else {
                Some(values.into_iter().flatten().sum())
            };
            fields.push((key.into(), number(total)));
        }
        let mut mem = Vec::new();
        for key in MEMORY {
            let values = rows
                .iter()
                .map(|t| optional(c::get(c::get(t, "memory")?, key)?, (1u64 << 53) - 1))
                .collect::<Result<Vec<_>>>()?;
            let value = if values.iter().any(Option::is_none) {
                None
            } else if key == "available_bytes" {
                values.into_iter().flatten().min()
            } else {
                values.into_iter().flatten().max()
            };
            mem.push((key.into(), number(value)));
        }
        let scores = rows.iter().map(|t| optional(c::get(t, "dialogue_score")?, 5)).collect::<Result<Vec<_>>>()?;
        fields.extend([
            ("memory".into(), Value::Object(mem)),
            ("dialogue_scored".into(), Value::from(scores.iter().flatten().count())),
            ("dialogue_score_total".into(), Value::from(scores.into_iter().flatten().sum::<u64>())),
        ]);
        reports.push(Value::Object(fields));
    }
    Ok(obj! {"version"=>1,"conditions"=>reports})
}
