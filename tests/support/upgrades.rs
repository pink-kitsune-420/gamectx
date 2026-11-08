use gamectx::{
    knowledge::facts::{Authority, FactOptions, FactRef, FactRegistry},
    knowledge::memory::Record,
    knowledge::recall::RecallIndex,
    knowledge::temporal::TemporalWindow,
    obj,
    runtime::brain::{Brain, BrainBundle, PreparedFrame},
    runtime::outcome::OutcomeTracker,
    runtime::playbook::Playbook,
    runtime::requests::{RequestContext, RequestGate, RequestTicket},
    runtime::spending::SpendingLedger,
    Error, Result, Value,
};
fn bad() -> Error {
    Error::new("invalid upgrade workflow")
}
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(bad)
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(bad)
}
fn list(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(bad)
}
fn uint(v: &Value) -> Result<u64> {
    v.as_usize().map(|n| n as u64).ok_or_else(bad)
}
fn signed(v: &Value) -> Result<i64> {
    v.to_json().parse().map_err(|_| bad())
}
fn labels(v: &Value) -> Result<Vec<String>> {
    list(v)?.iter().map(|v| text(v).map(str::to_owned)).collect()
}
fn register(facts: &mut FactRegistry, v: &Value) -> Result<Value> {
    let options = FactOptions {
        now: uint(get(v, "now")?)?,
        authority: match text(get(v, "authority")?)? {
            "host" => Authority::Host,
            "user" => Authority::User,
            _ => return Err(bad()),
        },
        pinned: get(v, "pinned")?.as_bool().ok_or_else(bad)?,
        expires: if get(v, "expires")? == &Value::Null { None } else { Some(uint(get(v, "expires")?)?) },
        privacy: labels(get(v, "privacy")?)?,
        dependencies: list(get(v, "dependencies")?)?.iter().map(FactRef::from_value).collect::<Result<_>>()?,
        expected_revision: if get(v, "expected")? == &Value::Null { None } else { Some(uint(get(v, "expected")?)?) },
    };
    Ok(facts.register(text(get(v, "scope")?)?, text(get(v, "key")?)?, get(v, "value")?.clone(), options)?.to_value())
}
pub fn process(v: &Value) -> Result<Value> {
    let mut results = Vec::new();
    match text(get(v, "kind")?)? {
        "recall" => {
            let records = list(get(v, "records")?)?
                .iter()
                .map(|r| {
                    Ok(Record {
                        id: uint(get(r, "id")?)?,
                        turn: uint(get(r, "turn")?)?,
                        importance: u8::try_from(uint(get(r, "importance")?)?).map_err(|_| bad())?,
                        text: text(get(r, "text")?)?.into(),
                        tags: labels(get(r, "tags")?)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let index = RecallIndex::new(&records, get(v, "config")?)?;
            for q in list(get(v, "queries")?)? {
                results.push(index.search(text(get(q, "text")?)?, uint(get(q, "count")?)? as usize)?)
            }
        }
        "outcomes" => {
            let mut tracker = OutcomeTracker::new(uint(get(v, "capacity")?)? as usize)?;
            for o in list(get(v, "operations")?)? {
                let now = uint(get(o, "now")?)?;
                let id = uint(get(o, "id")?)?;
                let result = match text(get(o, "op")?)? {
                    "begin" => {
                        tracker.begin(id, now, uint(get(o, "wait")?)?, uint(get(o, "window")?)?)?;
                        Value::Null
                    }
                    "execute" => {
                        tracker.executed(id, now)?;
                        Value::Null
                    }
                    "expose" => {
                        tracker.exposed(id, now)?;
                        Value::Null
                    }
                    "resolve" => {
                        tracker.resolve(id, now, uint(get(o, "score")?)?)?;
                        Value::Null
                    }
                    "cancel" => {
                        tracker.cancel(id, now)?;
                        Value::Null
                    }
                    "forget" => {
                        tracker.forget(id, now)?;
                        Value::Null
                    }
                    "report" => tracker.report(id, now)?,
                    "take" => tracker.take_label(id, now)?,
                    _ => return Err(bad()),
                };
                results.push(result);
            }
        }
        "temporal" => {
            let mut window = TemporalWindow::new(
                uint(get(v, "capacity")?)? as usize,
                uint(get(v, "window")?)?,
                uint(get(v, "start")?)?,
            )?;
            for o in list(get(v, "operations")?)? {
                let now = uint(get(o, "now")?)?;
                let result = match text(get(o, "op")?)? {
                    "availability" => {
                        window.availability(now, get(o, "available")?.as_bool().ok_or_else(bad)?)?;
                        Value::Null
                    }
                    "observe" => {
                        window.observe(
                            uint(get(o, "token")?)?,
                            now,
                            uint(get(o, "expires")?)?,
                            text(get(o, "kind")?)?,
                            signed(get(o, "value")?)?,
                        )?;
                        Value::Null
                    }
                    "features" => window.features(text(get(o, "kind")?)?, now)?,
                    "sequence" => window.sequence(&labels(get(o, "kinds")?)?, now)?,
                    _ => return Err(bad()),
                };
                results.push(result);
            }
        }
        "ledger" => {
            let mut ledger = SpendingLedger::new(uint(get(v, "limit")?)?, uint(get(v, "window")?)?)?;
            for o in list(get(v, "operations")?)? {
                let now = uint(get(o, "now")?)?;
                let result = match text(get(o, "op")?)? {
                    "reserve" => Value::from(ledger.reserve(now, uint(get(o, "amount")?)?)?),
                    "dispatch" => {
                        ledger.dispatch(uint(get(o, "id")?)?, now)?;
                        Value::Null
                    }
                    "cancel" => {
                        ledger.cancel_before_dispatch(uint(get(o, "id")?)?, now)?;
                        Value::Null
                    }
                    "finish" => {
                        ledger.finish(
                            uint(get(o, "id")?)?,
                            now,
                            if get(o, "actual")? == &Value::Null { None } else { Some(uint(get(o, "actual")?)?) },
                        )?;
                        Value::Null
                    }
                    "recover" => {
                        ledger.recover(now)?;
                        Value::Null
                    }
                    "summary" => ledger.summary(now)?,
                    _ => return Err(bad()),
                };
                results.push(result);
            }
            return Ok(obj! {"results"=>results,"state"=>ledger.to_value()});
        }
        kind @ ("playbook" | "requests") => {
            let mut facts = FactRegistry::new(uint(get(v, "capacity")?)? as usize)?;
            for r in list(get(v, "facts")?)? {
                register(&mut facts, r)?;
            }
            if kind == "playbook" {
                let book = Playbook::from_value(get(v, "recipes")?)?;
                if Playbook::from_json(&book.to_value().to_json())?.to_value() != book.to_value() {
                    return Err(bad());
                }
                for o in list(get(v, "operations")?)? {
                    if text(get(o, "op")?)? == "register" {
                        results.push(register(&mut facts, get(o, "request")?)?);
                        continue;
                    }
                    let allowed = labels(get(o, "allowed")?)?;
                    results.push(book.select(
                        &facts,
                        uint(get(o, "now")?)?,
                        &labels(get(o, "denied")?)?,
                        |name, _| allowed.iter().any(|a| a == name),
                    )?);
                }
            } else {
                let mut gate = RequestGate::new();
                let mut tickets = std::collections::BTreeMap::<u64, RequestTicket>::new();
                for o in list(get(v, "operations")?)? {
                    let method = text(get(o, "op")?)?;
                    if method == "register" {
                        results.push(register(&mut facts, get(o, "request")?)?);
                        continue;
                    }
                    let now = uint(get(o, "now")?)?;
                    if method == "issue" {
                        let ticket =
                            gate.issue(RequestContext::from_value(get(o, "context")?)?, now, uint(get(o, "ttl")?)?)?;
                        let id = ticket.id();
                        tickets.insert(id, ticket);
                        results.push(Value::from(id));
                    } else {
                        let ticket = tickets.get(&uint(get(o, "id")?)?).ok_or_else(bad)?;
                        match method {
                            "accept" => results.push(Value::Bool(gate.accept(
                                ticket,
                                &RequestContext::from_value(get(o, "context")?)?,
                                now,
                                &facts,
                                &labels(get(o, "denied")?)?,
                            )?)),
                            "cancel" => {
                                gate.cancel(ticket, now)?;
                                results.push(Value::Null)
                            }
                            _ => return Err(bad()),
                        }
                    }
                }
            }
        }
        "bundle" => {
            let mut bundle = BrainBundle::new(
                Brain::from_value(get(v, "perception")?)?,
                Brain::from_value(get(v, "decision")?)?,
                get(v, "ports")?,
            )?;
            let saved = bundle.to_value();
            if BrainBundle::restore(&saved)?.to_value() != saved {
                return Err(bad());
            }
            let mut prepared: Option<PreparedFrame> = None;
            for o in list(get(v, "operations")?)? {
                let scope = text(get(o, "scope")?)?;
                let frame = uint(get(o, "frame")?)?;
                match text(get(o, "op")?)? {
                    "prepare" => {
                        prepared = Some(bundle.prepare_json(scope, frame, get(o, "inputs")?)?);
                        results.push(Value::Null)
                    }
                    "run" => {
                        let allowed = labels(get(o, "allowed")?)?;
                        let allowed: Vec<_> = allowed.iter().map(String::as_str).collect();
                        results.push(bundle.run(
                            prepared.as_ref().ok_or_else(bad)?,
                            scope,
                            frame,
                            get(o, "inputs")?,
                            &allowed,
                        )?)
                    }
                    _ => return Err(bad()),
                }
            }
        }
        _ => return Err(bad()),
    }
    Ok(Value::Array(results))
}
