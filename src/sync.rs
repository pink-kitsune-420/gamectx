//! Revisioned, acknowledged context transfer. Send snapshots unless the receiver explicitly
//! acknowledges the sender's current revision. A replica reconstructs a complete packet;
//! a language model must receive that packet unless its integration implements this protocol.
use crate::json::{parse, Error, Result, Value};

pub const MAX_STATE_BYTES: usize = 262_144;
pub const MAX_MESSAGE_BYTES: usize = 524_288;
pub const MAX_SECTIONS: usize = 128;
pub const MAX_REVISION: usize = 2_147_483_647;

fn fail(message: &str) -> Error {
    Error::new(message)
}
fn stream_name(stream: &str) -> Result<()> {
    if stream.is_empty()
        || stream.len() > 64
        || !stream.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(fail("invalid context stream"));
    }
    Ok(())
}
fn names(value: &Value) -> Result<Vec<String>> {
    let values = value.as_array().ok_or_else(|| fail("expected context name list"))?;
    if values.len() > MAX_SECTIONS {
        return Err(fail("context section limit"));
    }
    let mut out = Vec::new();
    for value in values {
        let key = value.as_str().filter(|s| s.len() <= 256).ok_or_else(|| fail("invalid context name"))?;
        if out.iter().any(|old| old == key) {
            return Err(fail("duplicate context name"));
        }
        out.push(key.to_owned());
    }
    Ok(out)
}
fn state(value: &Value) -> Result<()> {
    let object = value.as_object().ok_or_else(|| fail("context state must be an object"))?;
    if object.len() > MAX_SECTIONS
        || object.iter().any(|(key, _)| key.len() > 256)
        || value.json_len() > MAX_STATE_BYTES
    {
        return Err(fail("context state limit"));
    }
    Ok(())
}
fn revision(value: &Value) -> Result<usize> {
    value.as_usize().filter(|r| (1..=MAX_REVISION).contains(r)).ok_or_else(|| fail("invalid context revision"))
}

/// One stream's most recently emitted state, never an assumption about delivered state.
#[derive(Clone, Debug)]
pub struct Publisher {
    stream: String,
    revision: usize,
    state: Option<Value>,
}
impl Publisher {
    pub fn new(stream: &str) -> Result<Self> {
        stream_name(stream)?;
        Ok(Self { stream: stream.to_owned(), revision: 0, state: None })
    }
    pub fn revision(&self) -> usize {
        self.revision
    }
    /// `acknowledged` must come from this stream's receiver. Missing/lost/mismatched ACK
    /// emits a full snapshot. No implicit delivery, approximate comparisons or hash keys.
    pub fn publish(&mut self, current: &Value, acknowledged: Option<usize>) -> Result<Value> {
        if self.revision == MAX_REVISION {
            return Err(fail("context revision exhausted; start a new stream"));
        }
        let current = parse(&current.to_json(), MAX_STATE_BYTES)?;
        state(&current)?;
        let delta = self.state.is_some() && acknowledged == Some(self.revision);
        let entries = current.as_object().expect("validated state");
        let changes = Value::Object(
            entries
                .iter()
                .filter(|(key, value)| !delta || self.state.as_ref().and_then(|s| s.get(key)) != Some(value))
                .cloned()
                .collect(),
        );
        let remove = if delta {
            self.state
                .as_ref()
                .expect("delta has baseline")
                .as_object()
                .expect("object")
                .iter()
                .filter(|(key, _)| current.get(key).is_none())
                .map(|(key, _)| key.clone())
                .collect()
        } else {
            Vec::new()
        };
        let message = crate::obj! {"stream" => self.stream.as_str(),
        "base" => if delta { Value::from(self.revision) } else { Value::Null },
        "revision" => self.revision+1,"order" => entries.iter().map(|(key,_)| key.clone()).collect::<Vec<_>>(),
        "set" => changes,"remove" => Value::strings(&remove)};
        // The envelope adds nesting; reject oversize/deep messages before advancing state.
        parse(&message.to_json(), MAX_MESSAGE_BYTES)?;
        self.state = Some(current);
        self.revision += 1;
        Ok(message)
    }
}

/// A receiver bound to one explicit stream. Wrong baselines, replay, duplicate names,
/// invalid removal/order and size/depth errors leave state/revision unchanged.
#[derive(Clone, Debug)]
pub struct Replica {
    stream: String,
    revision: usize,
    state: Option<Value>,
}
impl Replica {
    pub fn new(stream: &str) -> Result<Self> {
        stream_name(stream)?;
        Ok(Self { stream: stream.to_owned(), revision: 0, state: None })
    }
    pub fn revision(&self) -> usize {
        self.revision
    }
    pub fn state(&self) -> Option<&Value> {
        self.state.as_ref()
    }
    pub fn apply_json(&mut self, text: &str) -> Result<&Value> {
        self.apply_validated(&parse(text, MAX_MESSAGE_BYTES)?)
    }
    pub fn apply(&mut self, message: &Value) -> Result<&Value> {
        self.apply_validated(&parse(&message.to_json(), MAX_MESSAGE_BYTES)?)
    }
    fn apply_validated(&mut self, message: &Value) -> Result<&Value> {
        let fields = ["stream", "base", "revision", "order", "set", "remove"];
        let object = message.as_object().ok_or_else(|| fail("expected context message"))?;
        if object.len() != fields.len() || object.iter().any(|(key, _)| !fields.contains(&key.as_str())) {
            return Err(fail("unexpected or missing context fields"));
        }
        let get = |key| message.get(key).ok_or_else(|| fail("missing context field"));
        if get("stream")?.as_str() != Some(self.stream.as_str()) {
            return Err(fail("wrong context stream"));
        }
        let next = revision(get("revision")?)?;
        if next <= self.revision {
            return Err(fail("replayed context revision"));
        }
        let order = names(get("order")?)?;
        let removed = names(get("remove")?)?;
        let changes = get("set")?;
        state(changes)?;
        let mut values = if get("base")? == &Value::Null {
            if !removed.is_empty() {
                return Err(fail("snapshot cannot remove fields"));
            }
            Vec::new()
        } else {
            let base = revision(get("base")?)?;
            if base != self.revision || next != base + 1 {
                return Err(fail("context baseline mismatch; request snapshot"));
            }
            self.state.as_ref().and_then(Value::as_object).ok_or_else(|| fail("context snapshot required"))?.to_vec()
        };
        for key in removed {
            if changes.get(&key).is_some() {
                return Err(fail("context field both set and removed"));
            }
            let at = values.iter().position(|(old, _)| old == &key).ok_or_else(|| fail("unknown context removal"))?;
            values.remove(at);
        }
        for (key, value) in changes.as_object().expect("validated changes") {
            if let Some((_, old)) = values.iter_mut().find(|(old, _)| old == key) {
                *old = value.clone();
            } else {
                values.push((key.clone(), value.clone()));
            }
        }
        if order.len() != values.len() {
            return Err(fail("context order does not match fields"));
        }
        let ordered: Result<Vec<_>> = order
            .into_iter()
            .map(|key| {
                let at = values
                    .iter()
                    .position(|(old, _)| old == &key)
                    .ok_or_else(|| fail("unknown context order field"))?;
                Ok(values.remove(at))
            })
            .collect();
        let current = Value::Object(ordered?);
        state(&current)?;
        self.state = Some(current);
        self.revision = next;
        Ok(self.state.as_ref().expect("just installed state"))
    }
}
