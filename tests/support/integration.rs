use gamectx::{
    data::json::{parse, Error, Result, Value},
    knowledge::memory::Journal,
    obj,
    runtime::brain::{BoundBrain, Brain, FeatureSchema, Selection, SelectionPolicy, StableBrain},
    runtime::replay,
};
fn bad() -> Error {
    Error::new("invalid integration workflow")
}
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(bad)
}
fn integer(v: &Value) -> Result<usize> {
    v.as_usize().ok_or_else(bad)
}
fn signed(v: &Value) -> Result<i64> {
    v.to_json().parse().map_err(|_| bad())
}
fn list(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(bad)
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(bad)
}
fn boolean(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(bad)
}
fn inputs(v: &Value) -> Result<Vec<Option<i64>>> {
    list(v)?.iter().map(|v| if v == &Value::Null { Ok(None) } else { signed(v).map(Some) }).collect()
}
fn brain(workflow: &Value) -> Result<Brain> {
    let mut raw = workflow.as_object().ok_or_else(bad)?.to_vec();
    raw.retain(|(key, _)| key != "operations");
    raw.push(("operations".into(), Value::Array(vec![])));
    let initial = gamectx::runtime::brain::process(&Value::Object(raw))?;
    Brain::restore(get(&initial, "state")?)
}
pub fn process(v: &Value) -> Result<Value> {
    match text(get(v, "kind")?)? {
        "evidence" => {
            use gamectx::knowledge::memory::{EvidenceMemory, EvidenceSnapshot};
            let subject = text(get(v, "subject")?)?;
            let clock = text(get(v, "clock")?)?;
            let mut memory =
                EvidenceMemory::new(subject, clock, integer(get(v, "capacity")?)?, integer(get(v, "hypotheses")?)?)?;
            let mut views: std::collections::HashMap<String, EvidenceSnapshot> = std::collections::HashMap::new();
            let mut results = Vec::new();
            let labels =
                |v: &Value| -> Result<Vec<String>> { list(v)?.iter().map(|v| text(v).map(str::to_owned)).collect() };
            for op in list(get(v, "operations")?)? {
                match text(get(op, "op")?)? {
                    "observe" => {
                        let labels = labels(get(op, "privacy")?)?;
                        let labels: Vec<_> = labels.iter().map(String::as_str).collect();
                        results.push(Value::from(memory.observe(
                            integer(get(op, "token")?)? as u64,
                            get(op, "record")?,
                            &labels,
                        )?));
                    }
                    "snapshot" => {
                        let labels = labels(get(op, "denied")?)?;
                        let labels: Vec<_> = labels.iter().map(String::as_str).collect();
                        let view = memory.snapshot(
                            integer(get(op, "now")?)? as u64,
                            integer(get(op, "count")?)?,
                            integer(get(op, "bytes")?)?,
                            &labels,
                        )?;
                        results.push(view.to_value());
                        views.insert(text(get(op, "name")?)?.into(), view);
                    }
                    "propose" => {
                        let labels = labels(get(op, "denied")?)?;
                        let labels: Vec<_> = labels.iter().map(String::as_str).collect();
                        let view = views.get(text(get(op, "view")?)?).ok_or_else(bad)?;
                        memory.propose(view, integer(get(op, "now")?)? as u64, get(op, "request")?, &labels)?;
                        results.push(Value::Null);
                    }
                    "contradict" => {
                        memory.contradict(integer(get(op, "id")?)? as u64, integer(get(op, "now")?)? as u64)?;
                        results.push(Value::Null);
                    }
                    "forget" => {
                        let labels = labels(get(op, "denied")?)?;
                        let labels: Vec<_> = labels.iter().map(String::as_str).collect();
                        memory.forget_privacy(&labels)?;
                        results.push(Value::Null);
                    }
                    _ => return Err(bad()),
                }
            }
            let saved = memory.to_value();
            if EvidenceMemory::restore(&saved, subject, clock)?.to_value() != saved {
                return Err(bad());
            }
            Ok(obj! {"results"=>results,"state"=>saved})
        }
        "bindings" => {
            let schema = FeatureSchema::from_value(get(v, "schema")?)?;
            let bound = BoundBrain::new(brain(get(v, "workflow")?)?, schema.clone())?;
            let saved = bound.to_value()?;
            if let Some(expected) = v.get("expected") {
                BoundBrain::restore(&saved, &FeatureSchema::from_value(expected)?)?;
            }
            let values: Result<Vec<_>> = list(get(v, "inputs")?)?
                .iter()
                .map(|v| {
                    schema.normalize(v).map(|n| {
                        Value::Array(n.as_slice().iter().map(|v| v.map_or(Value::Null, Value::from)).collect())
                    })
                })
                .collect();
            Ok(obj! {"schema"=>schema.to_value(),"normalized"=>values?,"state"=>saved})
        }
        "metadata" => {
            let tables = get(v, "tables")?.as_object().ok_or_else(bad)?;
            let named: Result<Vec<_>> = tables.iter().map(|(name, rows)| Ok((name.as_str(), list(rows)?))).collect();
            gamectx::data::query::describe_with(&named?, get(v, "metadata")?)
        }
        "diagnostics" => {
            let brain = brain(get(v, "workflow")?)?;
            let result: Result<Vec<_>> = list(get(v, "runs")?)?
                .iter()
                .map(|r| {
                    brain.diagnose(
                        &inputs(get(r, "inputs")?)?,
                        u16::try_from(integer(get(r, "mask")?)?).map_err(|_| bad())?,
                        signed(get(r, "low")?)?,
                        signed(get(r, "gap")?)?,
                    )
                })
                .collect();
            Ok(Value::Array(result?))
        }
        "stability" => {
            let mut selector = StableBrain::new(
                brain(get(v, "workflow")?)?,
                signed(get(v, "margin")?)?,
                integer(get(v, "hold")?)? as u64,
            )?;
            let mut out = Selection::default();
            let mut results = Vec::new();
            for f in list(get(v, "frames")?)? {
                let policy = SelectionPolicy {
                    allowed: u16::try_from(integer(get(f, "mask")?)?).map_err(|_| bad())?,
                    min_score: signed(get(f, "low")?)?,
                    min_margin: signed(get(f, "gap")?)?,
                    urgent: boolean(get(f, "urgent")?)?,
                };
                let input = inputs(get(f, "inputs")?)?;
                selector.run_into(integer(get(f, "tick")?)? as u64, &input, policy, &mut out)?;
                let report = selector.brain().diagnose(&input, policy.allowed, policy.min_score, policy.min_margin)?;
                results.push(obj!{"prediction"=>report,"choice"=>out.choice().map_or(Value::Null,|i|Value::str(&selector.brain().outputs()[i])),"held"=>out.held(),"reason"=>out.reason()});
            }
            Ok(Value::Array(results))
        }
        "replay" => replay::run(get(v, "trace")?),
        "journal" => {
            let mut journal = Journal::new(integer(get(v, "capacity")?)?)?;
            let mut results = Vec::new();
            for op in list(get(v, "operations")?)? {
                match text(get(op, "op")?)? {
                    "record" => results.push(Value::from(journal.record(get(op, "data")?)?)),
                    "recall" => results.push(Value::Array(
                        journal
                            .recall(
                                integer(get(op, "now")?)? as u64,
                                text(get(op, "text")?)?,
                                integer(get(op, "count")?)?,
                                boolean(get(op, "proposals")?)?,
                            )?
                            .iter()
                            .map(|f| Journal::fact_value(f))
                            .collect(),
                    )),
                    _ => return Err(bad()),
                }
            }
            let saved = journal.to_value();
            if Journal::restore(&parse(&saved.to_json(), gamectx::knowledge::memory::MAX_JOURNAL_BYTES)?)?.to_value()
                != saved
            {
                return Err(bad());
            }
            Ok(obj! {"results"=>results,"state"=>saved})
        }
        _ => Err(bad()),
    }
}
