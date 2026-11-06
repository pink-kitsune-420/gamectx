//! Provider-neutral evaluation of snapshots and recorded candidate runs. This never
//! contacts a model. Missing usage/latency stays null rather than becoming a zero claim.
use super::*;

/// Replays complete candidate cohorts against bounded snapshots. Expected results come
/// from the host, not from the model query. Answers and executed queries score separately.
/// Number text is exact; object key order is ignored, array order is significant.
pub fn run(project: &Project, suite: &Value) -> Result<Value> {
    bounded(suite, MAX_REQUEST_BYTES)?;
    c::fields(suite, &["version", "cases", "runs"])?;
    c::uint(c::get(suite, "version")?, 1, 1)?;
    let cases = array(suite, "cases")?;
    let runs = array(suite, "runs")?;
    if cases.is_empty() || cases.len() > 128 || runs.is_empty() || runs.len() > 8 {
        return Err(bad());
    }
    let mut ids = BTreeSet::new();
    let mut snapshots = Vec::new();
    for case in cases {
        c::fields(case, &["id", "recipe", "snapshot", "question", "expected"])?;
        let id = text(case, "id")?;
        c::name(id)?;
        if !ids.insert(id) {
            return Err(bad());
        }
        prose(c::get(case, "question")?, 4096)?;
        let s = project.snapshot(c::get(case, "snapshot")?)?;
        project.build(text(case, "recipe")?, &s)?;
        snapshots.push(s);
    }
    unique_names(runs, 8)?;
    let mut reports = Vec::new();
    for run in runs {
        c::fields(run, &["name", "recipe", "guidance", "observations"])?;
        let guidance = text(run, "guidance")?;
        query::Guidance::parse(guidance)?;
        let override_recipe = match c::get(run, "recipe")? {
            Value::Null => None,
            v => {
                let n = c::text(v)?;
                project.recipe(n)?;
                Some(n)
            }
        };
        let observations = array(run, "observations")?;
        if observations.len() != cases.len() {
            return Err(bad());
        }
        let mut seen = BTreeSet::new();
        let mut details = Vec::new();
        let (mut answers, mut queries, mut invalid_queries, mut invalid_calls, mut packet_bytes) = (0, 0, 0, 0, 0usize);
        let mut totals = [Some(0u64); 3];
        for o in observations {
            c::fields(
                o,
                &["case_id", "sql", "answer", "input_tokens", "output_tokens", "latency_micros", "tool_calls"],
            )?;
            let id = text(o, "case_id")?;
            let i = cases.iter().position(|case| case.get("id").and_then(Value::as_str) == Some(id)).ok_or_else(bad)?;
            if !seen.insert(id) {
                return Err(bad());
            }
            let case = &cases[i];
            let recipe = override_recipe.unwrap_or(text(case, "recipe")?);
            let built = project.build(recipe, &snapshots[i])?;
            let bytes = built.packet.receipt.total_bytes;
            packet_bytes += bytes;
            let answer_ok = c::get(o, "answer")?.same(c::get(case, "expected")?);
            answers += usize::from(answer_ok);
            let mut query_ok = false;
            if c::get(o, "sql")? != &Value::Null {
                let sql = text(o, "sql")?;
                if sql.len() > 4096 {
                    return Err(bad());
                }
                match project.query(recipe, &snapshots[i], sql) {
                    Ok(result) => query_ok = result.same(c::get(case, "expected")?),
                    Err(_) => invalid_queries += 1,
                }
                queries += usize::from(query_ok);
            }
            let calls = array(o, "tool_calls")?;
            if calls.len() > 32 {
                return Err(bad());
            }
            let allowed = strings(c::get(&project.recipe(recipe)?.definition, "tools")?, 32)?;
            let mut bad_calls = 0usize;
            for call in calls {
                c::fields(call, &["name", "arguments"])?;
                let name = text(call, "name")?;
                if !allowed.contains(&name) || project.validate_call(name, c::get(call, "arguments")?).is_err() {
                    bad_calls += 1;
                }
            }
            invalid_calls += bad_calls;
            for (i, k) in ["input_tokens", "output_tokens", "latency_micros"].iter().enumerate() {
                let value = c::get(o, k)?;
                if *value == Value::Null {
                    totals[i] = None;
                } else {
                    let n = c::uint(value, 0, 1_000_000_000_000)?;
                    totals[i] = totals[i].map(|sum| sum + n);
                }
            }
            details.push(obj! {"id" => id, "answer_correct" => answer_ok, "query_correct" => query_ok, "invalid_tool_calls" => bad_calls, "packet_bytes" => bytes});
        }
        reports.push(obj! {"name" => text(run,"name")?, "guidance" => guidance, "cases" => cases.len(),
        "answers_correct" => answers, "queries_correct" => queries, "invalid_queries" => invalid_queries,
        "invalid_tool_calls" => invalid_calls, "packet_bytes" => packet_bytes,
        "input_tokens" => totals[0].map(Value::from).unwrap_or(Value::Null),
        "output_tokens" => totals[1].map(Value::from).unwrap_or(Value::Null),
        "latency_micros" => totals[2].map(Value::from).unwrap_or(Value::Null), "details" => details});
    }
    Ok(obj! {"version" => 1, "runs" => reports})
}
