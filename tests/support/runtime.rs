use gamectx::{
    obj,
    packet::budget::{assemble_bytes, Section},
    packet::sync::{Publisher, Replica},
    runtime::brain::{Brain, Prediction},
    runtime::schedule::Scheduler,
    Error, Result, Value,
};
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(|| Error::new("missing runtime field"))
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| Error::new("expected string"))
}
fn int(v: &Value) -> Result<usize> {
    v.as_usize().ok_or_else(|| Error::new("expected integer"))
}
fn list(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(|| Error::new("expected list"))
}
fn boolean(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(|| Error::new("expected bool"))
}
fn number(n: i64) -> Value {
    let whole = n / 1_000_000;
    let fraction = n % 1_000_000;
    if fraction == 0 {
        return Value::from(whole);
    }
    Value::Number(format!("{whole}.{:06}", fraction).trim_end_matches('0').to_owned())
}
fn initial(workflow: &Value) -> Result<Brain> {
    let mut data = workflow.as_object().ok_or_else(|| Error::new("workflow"))?.to_vec();
    data.retain(|(k, _)| k != "operations");
    data.push(("operations".into(), Value::Array(vec![])));
    Brain::restore(get(&gamectx::runtime::brain::process(&Value::Object(data))?, "state")?)
}
pub fn process(v: &Value) -> Result<Value> {
    match text(get(v, "kind")?)? {
        "infer" => {
            let brain = initial(get(v, "workflow")?)?;
            let mut predictions = Vec::new();
            let mut output = Prediction::default();
            for row in list(get(v, "runs")?)? {
                let inputs: Result<Vec<_>> = list(get(row, "inputs")?)?
                    .iter()
                    .map(|v| {
                        if v == &Value::Null {
                            Ok(None)
                        } else {
                            v.to_json().parse::<i64>().map(Some).map_err(|_| Error::new("native input integer"))
                        }
                    })
                    .collect();
                let mask = int(get(row, "mask")?)?;
                let mask = u16::try_from(mask).map_err(|_| Error::new("mask"))?;
                let threshold = |key| -> Result<i64> {
                    get(row, key)?.to_json().parse().map_err(|_| Error::new("threshold integer"))
                };
                brain.run_into(&inputs?, mask, threshold("low")?, threshold("gap")?, &mut output)?;
                predictions.push(obj! {"revision"=>output.revision(),"scores"=>Value::Object(brain.outputs().iter().cloned().zip(output.scores().iter().map(|s|s.map_or(Value::Null,number))).collect()),
                    "choice"=>output.choice().map_or(Value::Null,|i|Value::str(&brain.outputs()[i])),"margin"=>output.margin().map_or(Value::Null,number)});
            }
            Ok(Value::Array(predictions))
        }
        "budget" => {
            let sections: Result<Vec<_>> = list(get(v, "sections")?)?
                .iter()
                .map(|s| {
                    Ok(Section {
                        name: text(get(s, "name")?)?.to_owned(),
                        priority: s
                            .get("priority")
                            .map_or(Ok(0), |v| v.to_json().parse().map_err(|_| Error::new("priority")))?,
                        stable: s.get("stable").map_or(Ok(false), boolean)?,
                        required: s.get("required").map_or(Ok(false), boolean)?,
                        value: get(s, "value")?.clone(),
                    })
                })
                .collect();
            let p = assemble_bytes(sections?, int(get(v, "cap")?)?)?;
            Ok(obj! {"packet"=>p.value,"receipt"=>p.receipt.to_value()})
        }
        "host" => gamectx::runtime::brain::process(get(v, "workflow")?),
        "job" => {
            let workflow = get(v, "workflow")?;
            let mut brain = initial(workflow)?;
            let operations = list(get(workflow, "operations")?)?;
            if operations.len() != 1 {
                return Err(Error::new("one job operation"));
            }
            let op = text(get(&operations[0], "op")?)?;
            let args = get(&operations[0], "args")?;
            let mut job = match op {
                "train" => brain.start_train(args),
                "train_select" => brain.start_train_select(args),
                "train_select_host" => brain.start_train_select_host(args),
                _ => Err(Error::new("job operation")),
            }?;
            let slice = int(get(v, "slice")?)?;
            while !job.is_finished() {
                job.advance(slice)?;
            }
            if job.completed_steps() != job.total_steps() {
                return Err(Error::new("job accounting"));
            }
            let report = job.finish(&mut brain)?;
            Ok(obj! {"results"=>vec![report],"state"=>brain.to_value()})
        }
        "schedule" => {
            let mut scheduler = Scheduler::new(int(get(v, "capacity")?)?, int(get(v, "budget")?)?)?;
            for j in list(get(v, "jobs")?)? {
                scheduler.register(
                    int(get(j, "id")?)? as u64,
                    int(get(j, "cost")?)?,
                    int(get(j, "interval")?)? as u64,
                    int(get(j, "max_age")?)? as u64,
                    int(get(j, "phase")?)? as u64,
                )?;
            }
            let mut results = Vec::new();
            for event in list(get(v, "events")?)? {
                match text(get(event, "op")?)? {
                    "changed" => scheduler.changed(int(get(event, "id")?)? as u64, boolean(get(event, "urgent")?)?)?,
                    "poll" => results.push(Value::Array(
                        scheduler.poll(int(get(event, "tick")?)? as u64)?.into_iter().map(Value::from).collect(),
                    )),
                    _ => return Err(Error::new("scheduler event")),
                }
            }
            Ok(obj! {"results"=>results})
        }
        "sync" => {
            let stream = text(get(v, "stream")?)?;
            let mut publisher = Publisher::new(stream)?;
            let mut receiver = Replica::new(stream)?;
            let mut results = Vec::new();
            let mut pending = None;
            for operation in list(get(v, "operations")?)? {
                match text(get(operation, "op")?)? {
                    "publish" => {
                        let ack = get(operation, "ack")?;
                        let ack = if ack == &Value::Null { None } else { Some(int(ack)?) };
                        let message = publisher.publish(get(operation, "state")?, ack)?;
                        pending = Some(message.clone());
                        results.push(message);
                    }
                    "deliver" => results.push(
                        receiver.apply(pending.as_ref().ok_or_else(|| Error::new("no pending message"))?)?.clone(),
                    ),
                    "apply" => results.push(receiver.apply(get(operation, "message")?)?.clone()),
                    _ => return Err(Error::new("sync operation")),
                }
            }
            Ok(
                obj! {"results"=>results,"publisher_revision"=>publisher.revision(),"receiver_revision"=>receiver.revision(),"state"=>receiver.state().cloned().unwrap_or(Value::Null)},
            )
        }
        _ => Err(Error::new("runtime kind")),
    }
}
