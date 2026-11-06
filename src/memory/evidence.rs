use super::{
    journal::{fields, get, integer, short},
    *,
};
use std::sync::Arc;
const MAX_BYTES: usize = 1_048_576;
fn bad(s: &str) -> Error {
    Error::new(s)
}
fn labels(v: &Value) -> Result<Vec<String>> {
    let values = v.as_array().ok_or_else(|| bad("privacy labels required"))?;
    if values.len() > 8 {
        return Err(bad("privacy label limit"));
    }
    let mut out = Vec::new();
    for value in values {
        let name = short(value, 32)?;
        let mut chars = name.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_lowercase())
            || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            || out.iter().any(|old| old == name)
        {
            return Err(bad("invalid privacy label"));
        }
        out.push(name.to_owned());
    }
    Ok(out)
}
fn confidence(v: &Value) -> Result<i64> {
    let number = match v {
        Value::Number(s) => s.parse::<f64>().ok(),
        _ => None,
    }
    .filter(|n| n.is_finite() && (0.0..=0.95).contains(n))
    .ok_or_else(|| bad("hypothesis confidence must be 0..0.95"))?;
    Ok((number * 1_000_000.0).round() as i64)
}
fn number(n: i64) -> Value {
    if n % 1_000_000 == 0 {
        Value::from(n / 1_000_000)
    } else {
        Value::Number(format!("{}.{:06}", n / 1_000_000, n % 1_000_000).trim_end_matches('0').into())
    }
}
fn refs(v: &Value) -> Result<Vec<u64>> {
    let rows = v.as_array().ok_or_else(|| bad("citation list required"))?;
    if rows.len() > 3 {
        return Err(bad("citation count"));
    }
    let mut out = Vec::new();
    for r in rows {
        let id = integer(r, 1, u32::MAX as u64)?;
        if out.contains(&id) {
            return Err(bad("duplicate citation"));
        }
        out.push(id);
    }
    out.sort_unstable();
    Ok(out)
}
#[derive(Clone, Debug)]
struct Evidence {
    id: u64,
    token: u64,
    privacy: Vec<String>,
    contradicted: bool,
}
#[derive(Clone, Debug)]
struct Hypothesis {
    id: String,
    statement: String,
    confidence: i64,
    created: u64,
    expires: u64,
    supports: Vec<u64>,
    contradicts: Vec<u64>,
    privacy: Vec<String>,
}
impl Hypothesis {
    fn references(&self) -> Vec<u64> {
        let mut all = self.supports.clone();
        all.extend(&self.contradicts);
        all.sort_unstable();
        all
    }
    fn value(&self) -> Value {
        crate::obj! {"id"=>self.id.as_str(),"statement"=>self.statement.as_str(),"confidence"=>number(self.confidence),"created"=>self.created,"expires"=>self.expires,"supports"=>self.supports.clone(),"contradicts"=>self.contradicts.clone(),"privacy"=>Value::strings(&self.privacy)}
    }
}
/// Sealed host snapshot. Only its payload goes to a model; keep the object to validate
/// replies against the actual supplied evidence. Bound to its originating memory instance.
#[derive(Debug)]
pub struct EvidenceSnapshot {
    owner: Arc<()>,
    revision: u64,
    now: u64,
    ids: Vec<u64>,
    hypotheses: Vec<String>,
    payload: Value,
}
impl EvidenceSnapshot {
    pub fn to_value(&self) -> Value {
        self.payload.clone()
    }
}
/// Observed evidence and separately labeled, expiring model hypotheses. Clock identity
/// and subject are explicit. Confidence is a proposal score, never a calibrated probability.
#[derive(Debug)]
pub struct EvidenceMemory {
    owner: Arc<()>,
    subject: String,
    clock_id: String,
    revision: u64,
    last_token: u64,
    clock: u64,
    max_hypotheses: usize,
    journal: Journal,
    evidence: Vec<Evidence>,
    hypotheses: Vec<Hypothesis>,
}
impl EvidenceMemory {
    pub fn new(subject: &str, clock_id: &str, capacity: usize, max_hypotheses: usize) -> Result<Self> {
        short(&Value::str(subject), 64)?;
        labels(&Value::strings(&[clock_id]))?;
        if !(1..=128).contains(&capacity) || !(1..=16).contains(&max_hypotheses) {
            return Err(bad("evidence capacity limits"));
        }
        Ok(Self {
            owner: Arc::new(()),
            subject: subject.into(),
            clock_id: clock_id.into(),
            revision: 0,
            last_token: 0,
            clock: 0,
            max_hypotheses,
            journal: Journal::new(capacity)?,
            evidence: Vec::new(),
            hypotheses: Vec::new(),
        })
    }
    fn fork(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            subject: self.subject.clone(),
            clock_id: self.clock_id.clone(),
            revision: self.revision,
            last_token: self.last_token,
            clock: self.clock,
            max_hypotheses: self.max_hypotheses,
            journal: self.journal.clone(),
            evidence: self.evidence.clone(),
            hypotheses: self.hypotheses.clone(),
        }
    }
    fn commit(&mut self, mut candidate: Self) -> Result<()> {
        if self.revision >= u32::MAX as u64 {
            return Err(bad("evidence revisions exhausted"));
        }
        candidate.revision = self.revision + 1;
        if candidate.to_value().json_len() > MAX_BYTES {
            return Err(bad("evidence state size limit"));
        }
        *self = candidate;
        Ok(())
    }
    fn active(&self, id: u64, now: u64, denied: &[String]) -> bool {
        let fact = self.journal.records().iter().find(|r| r.record.id == id);
        let meta = self.evidence.iter().find(|r| r.id == id);
        matches!((fact,meta),(Some(f),Some(e))if !e.contradicted&&f.superseded.is_none()&&f.expires.is_none_or(|t|now<t)&&e.privacy.iter().all(|p|!denied.contains(p)))
    }
    /// Native event token is strictly increasing. Replayed/old tokens cannot renew sources.
    /// The host calls this with observed data; it is not a model proposal operation.
    pub fn observe(&mut self, token: u64, record: &Value, privacy: &[&str]) -> Result<u64> {
        if token <= self.last_token || token > u32::MAX as u64 || get(record, "source")?.as_str() != Some("observed") {
            return Err(bad("invalid native observation token/source"));
        }
        let privacy = labels(&Value::strings(privacy))?;
        let turn = integer(get(record, "turn")?, self.clock, u32::MAX as u64)?;
        let mut next = self.fork();
        let id = next.journal.record(record)?;
        let retained: Vec<_> = next.journal.records().iter().map(|r| r.record.id).collect();
        next.evidence.retain(|r| retained.contains(&r.id));
        if retained.contains(&id) {
            next.evidence.push(Evidence { id, token, privacy, contradicted: false });
        }
        next.hypotheses.retain(|h| h.references().iter().all(|r| retained.contains(r)));
        next.last_token = token;
        next.clock = turn;
        self.commit(next)?;
        Ok(id)
    }
    /// Read-only priority/recency snapshot. Privacy filtering happens before selection.
    /// Truncation removes dependent hypotheses and citable refs together with evidence.
    pub fn snapshot(&self, now: u64, count: usize, max_bytes: usize, denied: &[&str]) -> Result<EvidenceSnapshot> {
        if now < self.clock || now > u32::MAX as u64 || count > 16 || !(1..=16_384).contains(&max_bytes) {
            return Err(bad("evidence snapshot bounds"));
        }
        let denied = labels(&Value::strings(denied))?;
        let mut records: Vec<_> =
            self.journal.records().iter().filter(|r| self.active(r.record.id, now, &denied)).collect();
        records.sort_by_key(|r| std::cmp::Reverse((r.record.importance, r.record.turn, r.record.id)));
        records.truncate(count);
        loop {
            let ids: Vec<_> = records.iter().map(|r| r.record.id).collect();
            let hypotheses: Vec<_> = self
                .hypotheses
                .iter()
                .filter(|h| now < h.expires && h.references().iter().all(|r| ids.contains(r)))
                .collect();
            let evidence:Vec<_>=records.iter().map(|r|{let meta=self.evidence.iter().find(|e|e.id==r.record.id).expect("evidence metadata");crate::obj!{"record"=>Journal::fact_value(r),"token"=>meta.token,"privacy"=>Value::strings(&meta.privacy)}}).collect();
            let guesses: Vec<_> = hypotheses
                .iter()
                .map(|h| {
                    let mut value = h.value();
                    if let Value::Object(fields) = &mut value {
                        fields.push((
                            "status".into(),
                            Value::str(if h.contradicts.is_empty() { "tentative" } else { "contested" }),
                        ));
                    }
                    value
                })
                .collect();
            let citable:Vec<_>=records.iter().map(|r|crate::obj!{"id"=>r.record.id,"token"=>self.evidence.iter().find(|e|e.id==r.record.id).expect("metadata").token}).collect();
            let payload = crate::obj! {"subject"=>self.subject.as_str(),"clock"=>self.clock_id.as_str(),"revision"=>self.revision,"now"=>now,"evidence"=>evidence,"hypotheses"=>guesses,"citable"=>citable};
            if payload.json_len() <= max_bytes {
                return Ok(EvidenceSnapshot {
                    owner: self.owner.clone(),
                    revision: self.revision,
                    now,
                    ids,
                    hypotheses: hypotheses.iter().map(|h| h.id.clone()).collect(),
                    payload,
                });
            }
            if records.pop().is_none() {
                return Err(bad("evidence snapshot exceeds byte budget"));
            }
        }
    }
    /// Validate a model reply against the sealed host snapshot and current privacy/time.
    /// Repeating the same evidence cannot raise confidence, renew expiry or reword a claim.
    pub fn propose(&mut self, view: &EvidenceSnapshot, now: u64, data: &Value, denied: &[&str]) -> Result<()> {
        if !Arc::ptr_eq(&self.owner, &view.owner)
            || view.revision != self.revision
            || now < self.clock.max(view.now)
            || now > u32::MAX as u64
        {
            return Err(bad("stale or foreign evidence snapshot"));
        }
        fields(data, &["id", "statement", "confidence", "supports", "contradicts", "ttl"])?;
        let id = short(get(data, "id")?, 32)?;
        labels(&Value::strings(&[id]))?;
        let statement = short(get(data, "statement")?, 280)?;
        let mut score = confidence(get(data, "confidence")?)?;
        let supports = refs(get(data, "supports")?)?;
        let contradicts = refs(get(data, "contradicts")?)?;
        let mut all = supports.clone();
        all.extend(&contradicts);
        all.sort_unstable();
        if supports.is_empty() || all.len() > 3 || all.windows(2).any(|w| w[0] == w[1]) {
            return Err(bad("hypothesis citation bounds"));
        }
        let ttl = integer(get(data, "ttl")?, 1, 1_000_000)?;
        let denied = labels(&Value::strings(denied))?;
        if all.iter().any(|id| !view.ids.contains(id) || !self.active(*id, now, &denied)) {
            return Err(bad("hypothesis evidence unavailable"));
        }
        let old = self.hypotheses.iter().find(|h| h.id == id);
        let old_available =
            old.is_some_and(|h| now < h.expires && h.references().iter().all(|source| self.active(*source, now, &[])));
        if old_available && !view.hypotheses.iter().any(|seen| seen == id) {
            return Err(bad("hypothesis was not supplied"));
        }
        let mut next = self.fork();
        if score == 0 {
            if old.is_none() {
                return Err(bad("unknown hypothesis to retract"));
            }
            next.hypotheses.retain(|h| h.id != id);
            next.clock = now;
            return self.commit(next);
        }
        let mut privacy: Vec<_> =
            self.evidence.iter().filter(|e| all.contains(&e.id)).flat_map(|e| e.privacy.iter().cloned()).collect();
        privacy.sort();
        privacy.dedup();
        let mut expires = now.saturating_add(ttl).min(u32::MAX as u64);
        let mut created = now;
        for fact in self.journal.records().iter().filter(|r| all.contains(&r.record.id)) {
            if let Some(end) = fact.expires {
                expires = expires.min(end);
            }
        }
        if let Some(old) = old {
            if old.references() == all {
                if old.statement != statement {
                    return Err(bad("claim rewording needs new evidence"));
                }
                score = score.min(old.confidence);
                expires = expires.min(old.expires);
                created = old.created;
            }
        } else if self.hypotheses.iter().any(|h| h.statement == statement && h.references() == all) {
            return Err(bad("duplicate claim/evidence under another ID"));
        }
        if expires <= now {
            return Err(bad("hypothesis already expired"));
        }
        let hypothesis = Hypothesis {
            id: id.into(),
            statement: statement.into(),
            confidence: score,
            created,
            expires,
            supports,
            contradicts,
            privacy,
        };
        next.hypotheses.retain(|h| h.id != id);
        if next.hypotheses.len() == next.max_hypotheses {
            next.hypotheses.remove(0);
        }
        next.hypotheses.push(hypothesis);
        next.clock = now;
        self.commit(next)
    }
    pub fn contradict(&mut self, id: u64, now: u64) -> Result<()> {
        if now < self.clock || now > u32::MAX as u64 {
            return Err(bad("contradiction time limit"));
        }
        let mut next = self.fork();
        next.evidence.iter_mut().find(|e| e.id == id).ok_or_else(|| bad("unknown evidence"))?.contradicted = true;
        next.clock = now;
        self.commit(next)
    }
    /// Physical removal cascades through hypotheses. Hidden records cannot reappear after
    /// a later opt-in; the host must observe them again with a fresh native event token.
    pub fn forget_privacy(&mut self, denied: &[&str]) -> Result<()> {
        let denied = labels(&Value::strings(denied))?;
        let mut next = self.fork();
        let ids: Vec<_> =
            next.evidence.iter().filter(|e| e.privacy.iter().any(|p| denied.contains(p))).map(|e| e.id).collect();
        if ids.is_empty() {
            return Ok(());
        }
        for id in &ids {
            next.journal.forget(*id);
        }
        next.evidence.retain(|e| !ids.contains(&e.id));
        next.hypotheses.retain(|h| h.references().iter().all(|r| !ids.contains(r)));
        self.commit(next)
    }
    pub fn to_value(&self) -> Value {
        crate::obj! {"version"=>1,"subject"=>self.subject.as_str(),"clock_id"=>self.clock_id.as_str(),"revision"=>self.revision,"last_token"=>self.last_token,"clock"=>self.clock,"max_hypotheses"=>self.max_hypotheses,
        "journal"=>self.journal.to_value(),"evidence"=>self.evidence.iter().map(|e|crate::obj!{"id"=>e.id,"token"=>e.token,"privacy"=>Value::strings(&e.privacy),"contradicted"=>e.contradicted}).collect::<Vec<_>>(),"hypotheses"=>self.hypotheses.iter().map(Hypothesis::value).collect::<Vec<_>>()}
    }
    pub fn restore_json(text: &str, subject: &str, clock_id: &str) -> Result<Self> {
        Self::restore(&crate::json::parse(text, MAX_BYTES)?, subject, clock_id)
    }
    pub fn restore(v: &Value, subject: &str, clock_id: &str) -> Result<Self> {
        fields(
            v,
            &[
                "version",
                "subject",
                "clock_id",
                "revision",
                "last_token",
                "clock",
                "max_hypotheses",
                "journal",
                "evidence",
                "hypotheses",
            ],
        )?;
        integer(get(v, "version")?, 1, 1)?;
        if get(v, "subject")?.as_str() != Some(subject) || get(v, "clock_id")?.as_str() != Some(clock_id) {
            return Err(bad("evidence subject/clock mismatch"));
        }
        let journal = Journal::restore(get(v, "journal")?)?;
        let capacity = integer(get(get(v, "journal")?, "capacity")?, 1, 128)? as usize;
        let max = integer(get(v, "max_hypotheses")?, 1, 16)? as usize;
        let mut out = Self::new(subject, clock_id, capacity, max)?;
        out.revision = integer(get(v, "revision")?, 0, u32::MAX as u64)?;
        out.last_token = integer(get(v, "last_token")?, 0, u32::MAX as u64)?;
        out.clock = integer(get(v, "clock")?, journal.clock(), u32::MAX as u64)?;
        out.journal = journal;
        let observations = integer(get(get(v, "journal")?, "next_id")?, 1, u32::MAX as u64)? - 1;
        if out.revision < observations || out.last_token < observations {
            return Err(bad("saved evidence counters disagree"));
        }
        let evidence = get(v, "evidence")?.as_array().ok_or_else(|| bad("saved evidence required"))?;
        if evidence.len() != out.journal.records().len() {
            return Err(bad("saved evidence metadata coverage"));
        }
        let mut last_token = 0;
        let mut last_id = 0;
        for row in evidence {
            fields(row, &["id", "token", "privacy", "contradicted"])?;
            let id = integer(get(row, "id")?, last_id + 1, u32::MAX as u64)?;
            last_id = id;
            if out.evidence.iter().any(|e| e.id == id)
                || !out.journal.records().iter().any(|f| f.record.id == id && f.source == "observed")
            {
                return Err(bad("saved evidence ID/source"));
            }
            let token = integer(get(row, "token")?, last_token + 1, out.last_token)?;
            last_token = token;
            let contradicted = get(row, "contradicted")?.as_bool().ok_or_else(|| bad("contradiction flag required"))?;
            out.evidence.push(Evidence { id, token, privacy: labels(get(row, "privacy")?)?, contradicted });
        }
        let hypotheses = get(v, "hypotheses")?.as_array().ok_or_else(|| bad("saved hypotheses required"))?;
        if hypotheses.len() > max {
            return Err(bad("saved hypothesis count"));
        }
        for row in hypotheses {
            fields(
                row,
                &["id", "statement", "confidence", "created", "expires", "supports", "contradicts", "privacy"],
            )?;
            let id = short(get(row, "id")?, 32)?;
            labels(&Value::strings(&[id]))?;
            if out.hypotheses.iter().any(|h| h.id == id) {
                return Err(bad("duplicate saved hypothesis"));
            }
            let score = confidence(get(row, "confidence")?)?;
            if score == 0 {
                return Err(bad("zero saved confidence"));
            }
            let created = integer(get(row, "created")?, 0, out.clock)?;
            let expires = integer(get(row, "expires")?, created + 1, (created + 1_000_000).min(u32::MAX as u64))?;
            let supports = refs(get(row, "supports")?)?;
            let contradicts = refs(get(row, "contradicts")?)?;
            let mut all = supports.clone();
            all.extend(&contradicts);
            all.sort_unstable();
            if supports.is_empty()
                || all.len() > 3
                || all.windows(2).any(|w| w[0] == w[1])
                || all.iter().any(|id| !out.evidence.iter().any(|e| e.id == *id))
            {
                return Err(bad("saved hypothesis references"));
            }
            if out
                .journal
                .records()
                .iter()
                .filter(|f| all.contains(&f.record.id))
                .any(|f| f.record.turn > created || f.expires.is_some_and(|end| expires > end))
            {
                return Err(bad("saved hypothesis exceeds source times"));
            }
            let mut privacy: Vec<_> =
                out.evidence.iter().filter(|e| all.contains(&e.id)).flat_map(|e| e.privacy.iter().cloned()).collect();
            privacy.sort();
            privacy.dedup();
            let saved = get(row, "privacy")?.as_array().ok_or_else(|| bad("saved privacy"))?;
            if Value::strings(&privacy) != Value::Array(saved.to_vec()) {
                return Err(bad("saved hypothesis privacy mismatch"));
            }
            out.hypotheses.push(Hypothesis {
                id: id.into(),
                statement: short(get(row, "statement")?, 280)?.into(),
                confidence: score,
                created,
                expires,
                supports,
                contradicts,
                privacy,
            });
        }
        if out.to_value().json_len() > MAX_BYTES {
            return Err(bad("saved evidence size limit"));
        }
        Ok(out)
    }
}
