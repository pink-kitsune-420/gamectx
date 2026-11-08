//! Compare complete developer-recorded configurations. Select on tuning cases only;
//! held-out outcomes are reported separately. Metrics/provenance are host assertions.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};

fn n(v: &Value, k: &str) -> Result<u64> {
    c::uint(c::get(v, k)?, 0, u64::MAX)
}
fn optional(v: &Value, k: &str) -> Result<Option<u64>> {
    let v = c::get(v, k)?;
    if *v == Value::Null {
        Ok(None)
    } else {
        Ok(Some(c::uint(v, 0, u64::MAX)?))
    }
}

/// Input is a pinned identity, independent split case manifest, profile/recipe/trial
/// candidates, and explicit quality/latency/RSS limits. Emits a Pareto frontier and
/// an exportable tuning winner; it never changes a running deployment.
pub fn compare(a: &Value) -> Result<Value> {
    bounded(a, 1_572_864)?;
    c::fields(a, &["identity", "cases", "candidates", "limits"])?;
    let identity = crate::deploy::deployment::Identity::from_value(c::get(a, "identity")?)?;
    let cases = c::list(c::get(a, "cases")?)?;
    if cases.len() < 2 || cases.len() > 512 {
        return Err(c::bad());
    }
    let mut ids = Vec::new();
    for case in cases {
        c::fields(case, &["id", "split", "expected"])?;
        let id = c::text(c::get(case, "id")?)?;
        c::scope(id)?;
        if ids.contains(&id) {
            return Err(c::bad());
        }
        ids.push(id);
        if !["tune", "holdout"].contains(&c::text(c::get(case, "split")?)?) {
            return Err(c::bad());
        }
    }
    for split in ["tune", "holdout"] {
        if !cases.iter().any(|case| case.get("split").and_then(Value::as_str) == Some(split)) {
            return Err(c::bad());
        }
    }
    let limits = c::get(a, "limits")?;
    c::fields(limits, &["min_correct_per_mille", "max_p95_micros", "max_rss_bytes"])?;
    let minimum = c::uint(c::get(limits, "min_correct_per_mille")?, 0, 1000)?;
    let max_latency = c::uint(c::get(limits, "max_p95_micros")?, 0, 1_000_000_000_000)?;
    let max_memory = c::uint(c::get(limits, "max_rss_bytes")?, 0, (1u64 << 54) - 1)?;
    let candidates = c::list(c::get(a, "candidates")?)?;
    if candidates.is_empty() || candidates.len() > 16 {
        return Err(c::bad());
    }
    let mut rows = Vec::new();
    let mut names = Vec::<String>::new();
    let mut costs = Vec::new();
    let mut eligible = Vec::new();
    for candidate in candidates {
        c::fields(candidate, &["profile", "recipe", "trials"])?;
        let profile = c::get(candidate, "profile")?;
        crate::deploy::deployment::Profile::from_value(profile)?.require_identity(&identity)?;
        let name = c::text(c::get(profile, "name")?)?;
        if names.iter().any(|n| n == name) {
            return Err(c::bad());
        }
        names.push(name.into());
        let recipe = c::text(c::get(candidate, "recipe")?)?;
        c::name(recipe)?;
        let trials = c::list(c::get(candidate, "trials")?)?;
        if trials.len() != cases.len() {
            return Err(c::bad());
        }
        let mut reports = Vec::new();
        for split in ["tune", "holdout"] {
            let mut selected = Vec::new();
            for case in cases {
                let found = trials.iter().filter(|t| t.get("id") == case.get("id")).collect::<Vec<_>>();
                if found.len() != 1 {
                    return Err(c::bad());
                }
                let trial = found[0];
                if trial.get("condition").and_then(Value::as_str) != Some(name)
                    || !c::get(trial, "expected")?.same(c::get(case, "expected")?)
                {
                    return Err(c::bad());
                }
                if case.get("split").and_then(Value::as_str) == Some(split) {
                    selected.push(trial.clone());
                }
            }
            let report = crate::deploy::turn::report(&obj! {"version"=>1,"trials"=>selected})?;
            reports.push(c::list(c::get(&report, "conditions")?)?[0].clone());
        }
        let t = &reports[0];
        let mut reasons = Vec::<&str>::new();
        let correct = n(t, "correct")?;
        if correct * 1000 < minimum * n(t, "trials")? {
            reasons.push("correctness");
        }
        if n(t, "missing_required")? > 0 {
            reasons.push("required_facts");
        }
        if n(t, "truncated")? > 0 {
            reasons.push("truncation");
        }
        let memory = c::get(t, "memory")?;
        let rss = optional(memory, "runner_rss_bytes")?.zip(optional(memory, "game_rss_bytes")?).map(|(a, b)| a + b);
        let tokens = optional(t, "input_tokens")?.zip(optional(t, "output_tokens")?).map(|(a, b)| a + b);
        let latency = n(t, "p95_turn_micros")?;
        if rss.is_none() || tokens.is_none() {
            reasons.push("missing_metrics");
        }
        if latency > max_latency {
            reasons.push("latency");
        }
        if rss.is_some_and(|n| n > max_memory) {
            reasons.push("memory");
        }
        if reasons.is_empty() {
            eligible.push(rows.len());
        }
        costs.push((tokens.unwrap_or(u64::MAX), latency, rss.unwrap_or(u64::MAX), std::cmp::Reverse(correct)));
        rows.push(obj! {"name"=>name,"recipe"=>recipe,"eligible"=>reasons.is_empty(),"reasons"=>Value::strings(&reasons),"tune"=>reports[0].clone(),"holdout"=>reports[1].clone()});
    }
    let frontier = eligible
        .iter()
        .copied()
        .filter(|&i| {
            !eligible.iter().any(|&j| {
                let a = costs[j];
                let b = costs[i];
                a.0 <= b.0 && a.1 <= b.1 && a.2 <= b.2 && a.3 <= b.3 && a != b
            })
        })
        .collect::<Vec<_>>();
    let best = frontier.iter().copied().min_by_key(|&i| (costs[i], i));
    let selected=best.map(|i|obj! {"profile"=>candidates[i].get("profile").expect("profile").clone(),"recipe"=>candidates[i].get("recipe").expect("recipe").clone()}).unwrap_or(Value::Null);
    Ok(
        obj! {"version"=>1,"selection_split"=>"tune","candidates"=>rows,"frontier"=>Value::strings(&frontier.iter().map(|&i|names[i].clone()).collect::<Vec<_>>()),"selected"=>selected},
    )
}
