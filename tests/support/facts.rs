use gamectx::{
    facts::{Authority, FactOptions, FactRef, FactRegistry},
    json::{Error, Result, Value},
    obj,
};
fn bad() -> Error {
    Error::new("invalid fact workflow")
}
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(bad)
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(bad)
}
fn int(v: &Value) -> Result<u64> {
    v.as_usize().map(|n| n as u64).ok_or_else(bad)
}
fn list(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(bad)
}
fn labels(v: &Value) -> Result<Vec<String>> {
    list(v)?.iter().map(|v| text(v).map(str::to_owned)).collect()
}
fn authority(v: &Value) -> Result<Authority> {
    match text(v)? {
        "host" => Ok(Authority::Host),
        "user" => Ok(Authority::User),
        _ => Err(bad()),
    }
}
fn optional(v: &Value) -> Result<Option<u64>> {
    if v == &Value::Null {
        Ok(None)
    } else {
        int(v).map(Some)
    }
}
pub fn process(v: &Value) -> Result<Value> {
    let mut facts = FactRegistry::new(int(get(v, "capacity")?)? as usize)?;
    let mut results = Vec::new();
    for op in list(get(v, "operations")?)? {
        match text(get(op, "op")?)? {
            "register" => {
                let r = get(op, "request")?;
                let deps =
                    list(get(r, "dependencies")?)?.iter().map(FactRef::from_value).collect::<Result<Vec<_>>>()?;
                let options = FactOptions {
                    now: int(get(r, "now")?)?,
                    authority: authority(get(r, "authority")?)?,
                    pinned: get(r, "pinned")?.as_bool().ok_or_else(bad)?,
                    expires: optional(get(r, "expires")?)?,
                    privacy: labels(get(r, "privacy")?)?,
                    dependencies: deps,
                    expected_revision: optional(get(r, "expected")?)?,
                };
                results.push(
                    facts
                        .register(text(get(r, "scope")?)?, text(get(r, "key")?)?, get(r, "value")?.clone(), options)?
                        .to_value(),
                );
            }
            "check" => results.push(
                facts
                    .check(
                        text(get(op, "scope")?)?,
                        text(get(op, "key")?)?,
                        get(op, "value")?,
                        int(get(op, "now")?)?,
                        &labels(get(op, "denied")?)?,
                    )?
                    .to_value(),
            ),
            "packet" => results.push(facts.packet(
                int(get(op, "now")?)?,
                int(get(op, "count")?)? as usize,
                int(get(op, "bytes")?)? as usize,
                &labels(get(op, "denied")?)?,
            )?),
            "unpin" => results.push(
                facts
                    .unpin(&FactRef::from_value(get(op, "reference")?)?, authority(get(op, "authority")?)?)?
                    .to_value(),
            ),
            "forget" => {
                facts.forget(&FactRef::from_value(get(op, "reference")?)?, authority(get(op, "authority")?)?)?;
                results.push(Value::Null);
            }
            "forget_privacy" => {
                facts.forget_privacy(&labels(get(op, "denied")?)?)?;
                results.push(Value::Null);
            }
            _ => return Err(bad()),
        }
    }
    let saved = facts.to_value();
    if FactRegistry::restore(&saved)?.to_value() != saved {
        return Err(bad());
    }
    Ok(obj! {"results"=>results,"state"=>saved})
}
