use super::*;

pub const MAX_JOURNAL_BYTES: usize = 4_194_304;
#[derive(Clone, Debug, PartialEq)]
pub struct Fact {
    pub record: Record,
    pub entity: String,
    pub key: String,
    pub source: String,
    pub expires: Option<u64>,
    pub superseded: Option<u64>,
}
/// Optional scoped facts, separate from legacy Memory. Supersession stays within one
/// entity/key/provenance channel. Proposals never shadow an active observed fact.
#[derive(Clone, Debug)]
pub struct Journal {
    capacity: usize,
    clock: u64,
    next_id: u64,
    records: Vec<Fact>,
}
fn bad(message: &str) -> Error {
    Error::new(message)
}
pub(super) fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(|| bad("missing journal field"))
}
pub(super) fn fields(v: &Value, keys: &[&str]) -> Result<()> {
    let rows = v.as_object().ok_or_else(|| bad("journal object required"))?;
    if rows.len() != keys.len()
        || keys.iter().any(|k| v.get(k).is_none())
        || rows
            .iter()
            .enumerate()
            .any(|(i, (k, _))| !keys.contains(&k.as_str()) || rows[..i].iter().any(|(old, _)| old == k))
    {
        return Err(bad("unexpected or missing journal field"));
    }
    Ok(())
}
pub(super) fn integer(v: &Value, low: u64, high: u64) -> Result<u64> {
    v.as_usize()
        .map(|x| x as u64)
        .filter(|x| (low..=high).contains(x))
        .ok_or_else(|| bad("journal integer outside limits"))
}
pub(super) fn short(v: &Value, max: usize) -> Result<&str> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control))
        .ok_or_else(|| bad("invalid journal text"))
}
impl Journal {
    pub fn new(capacity: usize) -> Result<Self> {
        if !(1..=1024).contains(&capacity) {
            return Err(bad("journal capacity must be 1..1024"));
        }
        Ok(Self { capacity, clock: 0, next_id: 1, records: Vec::new() })
    }
    pub fn clock(&self) -> u64 {
        self.clock
    }
    pub fn forget(&mut self, id: u64) -> bool {
        let len = self.records.len();
        self.records.retain(|r| r.record.id != id);
        len != self.records.len()
    }
    pub fn records(&self) -> &[Fact] {
        &self.records
    }
    pub fn record(&mut self, data: &Value) -> Result<u64> {
        fields(data, &["turn", "entity", "key", "text", "tags", "importance", "source", "expires"])?;
        if self.next_id >= u32::MAX as u64 {
            return Err(bad("journal IDs exhausted"));
        }
        let turn = integer(get(data, "turn")?, self.clock, u32::MAX as u64)?;
        let entity = short(get(data, "entity")?, 64)?.to_owned();
        let key = short(get(data, "key")?, 64)?.to_owned();
        let source = short(get(data, "source")?, 16)?;
        if !matches!(source, "observed" | "proposed") {
            return Err(bad("journal source must be observed or proposed"));
        }
        let text = short(get(data, "text")?, 280)?;
        let tags = get(data, "tags")?.as_array().ok_or_else(|| bad("journal tags required"))?;
        if tags.len() > 8 {
            return Err(bad("journal tag limit"));
        }
        let tags: Result<Vec<&str>> = tags.iter().map(|t| short(t, 32)).collect();
        let importance = integer(get(data, "importance")?, 1, 10)? as u8;
        let expires = match get(data, "expires")? {
            Value::Null => None,
            v => Some(integer(v, turn + 1, u32::MAX as u64)?),
        };
        // Reuse legacy normalization and validation without changing its format/ranking.
        let mut normal = Memory::new();
        normal.remember(turn, text, &tags?, importance)?;
        let mut record = normal.records.remove(0);
        record.id = self.next_id;
        for old in &mut self.records {
            if old.entity == entity && old.key == key && old.source == source && old.superseded.is_none() {
                old.superseded = Some(turn);
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.clock = turn;
        self.records.push(Fact { record, entity, key, source: source.into(), expires, superseded: None });
        if self.records.len() > self.capacity {
            let victim = self
                .records
                .iter()
                .enumerate()
                .min_by_key(|(_, r)| {
                    (
                        !(r.superseded.is_some() || r.expires.is_some_and(|t| t <= turn)),
                        r.source == "observed",
                        r.record.importance,
                        r.record.turn,
                        r.record.id,
                    )
                })
                .map(|(i, _)| i)
                .expect("nonempty");
            self.records.remove(victim);
        }
        Ok(id)
    }
    /// Model proposals cannot supply provenance or host time. Expiry is a bounded TTL;
    /// a host wishing to accept an observation calls record itself with source=observed.
    pub fn apply_proposal(&mut self, turn: u64, args: &Value) -> Result<u64> {
        fields(args, &["entity", "key", "text", "tags", "importance", "ttl"])?;
        let ttl = integer(get(args, "ttl")?, 1, 1_000_000)?;
        let expires =
            turn.checked_add(ttl).filter(|t| *t <= u32::MAX as u64).ok_or_else(|| bad("proposal expiry overflow"))?;
        self.record(&crate::obj!{"turn"=>turn,"entity"=>get(args,"entity")?.clone(),"key"=>get(args,"key")?.clone(),"text"=>get(args,"text")?.clone(),"tags"=>get(args,"tags")?.clone(),"importance"=>get(args,"importance")?.clone(),"source"=>"proposed","expires"=>expires})
    }
    pub fn recall(&self, now: u64, text: &str, count: usize, include_proposals: bool) -> Result<Vec<&Fact>> {
        if now < self.clock || now > u32::MAX as u64 || text.chars().count() > 512 || count > 64 {
            return Err(bad("journal recall bounds or backward time"));
        }
        let active: Vec<_> =
            self.records.iter().filter(|r| r.superseded.is_none() && r.expires.is_none_or(|t| now < t)).collect();
        let eligible: Vec<_> = active
            .iter()
            .copied()
            .filter(|r| {
                r.source == "observed"
                    || include_proposals
                        && !active
                            .iter()
                            .any(|old| old.entity == r.entity && old.key == r.key && old.source == "observed")
            })
            .collect();
        let ranked = recall_records(eligible.iter().map(|r| &r.record), text, count);
        Ok(ranked
            .into_iter()
            .map(|r| eligible.iter().find(|f| f.record.id == r.id).copied().expect("eligible fact"))
            .collect())
    }
    pub fn fact_value(f: &Fact) -> Value {
        crate::obj! {"id"=>f.record.id,"turn"=>f.record.turn,"entity"=>f.entity.as_str(),"key"=>f.key.as_str(),"text"=>f.record.text.as_str(),"tags"=>Value::strings(&f.record.tags),"importance"=>f.record.importance as usize,"source"=>f.source.as_str(),"expires"=>f.expires.map_or(Value::Null,Value::from),"superseded"=>f.superseded.map_or(Value::Null,Value::from)}
    }
    pub fn to_value(&self) -> Value {
        crate::obj! {"version"=>1,"capacity"=>self.capacity,"clock"=>self.clock,"next_id"=>self.next_id,"records"=>self.records.iter().map(Self::fact_value).collect::<Vec<_>>()}
    }
    pub fn restore_json(text: &str) -> Result<Self> {
        Self::restore(&crate::json::parse(text, MAX_JOURNAL_BYTES)?)
    }
    pub fn restore(v: &Value) -> Result<Self> {
        fields(v, &["version", "capacity", "clock", "next_id", "records"])?;
        integer(get(v, "version")?, 1, 1)?;
        let mut out = Self::new(integer(get(v, "capacity")?, 1, 1024)? as usize)?;
        out.clock = integer(get(v, "clock")?, 0, u32::MAX as u64)?;
        out.next_id = integer(get(v, "next_id")?, 1, u32::MAX as u64)?;
        let records = get(v, "records")?.as_array().ok_or_else(|| bad("journal records required"))?;
        if records.len() > out.capacity {
            return Err(bad("journal record limit"));
        }
        let mut last = 0;
        let mut last_turn = 0;
        for row in records {
            fields(
                row,
                &["id", "turn", "entity", "key", "text", "tags", "importance", "source", "expires", "superseded"],
            )?;
            let id = integer(get(row, "id")?, last + 1, out.next_id - 1)?;
            last = id;
            let mut data = row.as_object().expect("validated").to_vec();
            data.retain(|(key, _)| !matches!(key.as_str(), "id" | "superseded"));
            let mut one = Self::new(1)?;
            one.record(&Value::Object(data))?;
            let mut fact = one.records.remove(0);
            fact.record.id = id;
            if fact.record.turn < last_turn {
                return Err(bad("journal record time order"));
            }
            last_turn = fact.record.turn;
            if fact.record.turn > out.clock {
                return Err(bad("future journal record"));
            }
            fact.superseded = match get(row, "superseded")? {
                Value::Null => None,
                v => Some(integer(v, fact.record.turn, out.clock)?),
            };
            if fact.superseded.is_none()
                && out.records.iter().any(|r| {
                    r.superseded.is_none() && r.entity == fact.entity && r.key == fact.key && r.source == fact.source
                })
            {
                return Err(bad("multiple active journal facts"));
            }
            out.records.push(fact);
        }
        Ok(out)
    }
}
