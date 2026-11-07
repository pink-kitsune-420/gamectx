use super::*;
#[derive(Clone, Debug, PartialEq)]
pub struct AssetRef {
    value: Value,
}
impl AssetRef {
    pub fn new(uri: &str, digest: &str) -> Result<Self> {
        if uri.is_empty()
            || uri.len() > 1024
            || uri.chars().any(char::is_control)
            || !["asset://", "https://", "http://", "file://", "urn:"].iter().any(|s| uri.starts_with(s))
            || digest.len() != 64
            || !digest.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(c::bad());
        }
        Ok(Self { value: obj! {"asset_uri"=>uri,"digest"=>digest} })
    }
    pub fn from_bytes(uri: &str, bytes: &[u8]) -> Result<Self> {
        Self::new(uri, &content_digest(bytes)?)
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    fn from_value(v: &Value) -> Result<Self> {
        c::fields(v, &["asset_uri", "digest"])?;
        Self::new(text(v, "asset_uri")?, text(v, "digest")?)
    }
    fn keyed(&self, key: &str) -> Value {
        obj! {"asset_uri"=>get(&self.value,"asset_uri").expect("uri").clone(),"digest"=>get(&self.value,"digest").expect("digest").clone(),"key"=>key}
    }
}
#[derive(Clone, Debug)]
pub struct RegistryConfig {
    enabled: bool,
    capacity: usize,
    queue_limit: usize,
    epoch: String,
}
impl Default for RegistryConfig {
    fn default() -> Self {
        Self { enabled: false, capacity: 64, queue_limit: 0, epoch: "disabled".into() }
    }
}
impl RegistryConfig {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 4096)?;
        c::fields(v, &["enabled", "capacity", "queue_limit", "epoch"])?;
        c::scope(text(v, "epoch")?)?;
        Ok(Self {
            enabled: c::boolean(get(v, "enabled")?)?,
            capacity: c::uint(get(v, "capacity")?, 1, 128)? as usize,
            queue_limit: c::uint(get(v, "queue_limit")?, 0, 32)? as usize,
            epoch: text(v, "epoch")?.into(),
        })
    }
}
#[derive(Clone, Debug)]
struct Entry {
    spec: VisualSpec,
    state: &'static str,
    asset: Option<AssetRef>,
    candidate: Option<AssetRef>,
    ticket: Option<Value>,
    retired: bool,
}
impl Entry {
    fn new(spec: VisualSpec) -> Self {
        Self { spec, state: "missing", asset: None, candidate: None, ticket: None, retired: false }
    }
    fn value(&self) -> Value {
        obj! {"key"=>self.spec.key.clone(),"spec"=>self.spec.data.clone(),"inspection"=>self.spec.inspection.clone(),"state"=>self.state,"asset"=>self.asset.as_ref().map_or(Value::Null,AssetRef::to_value),"candidate"=>self.candidate.as_ref().map_or(Value::Null,AssetRef::to_value),"ticket"=>self.ticket.clone().unwrap_or(Value::Null),"retired"=>self.retired}
    }
}
#[derive(Debug)]
pub struct AssetRegistry {
    config: RegistryConfig,
    rows: Vec<Entry>,
    aliases: Vec<(String, String)>,
    next: u64,
}
fn subset(a: &Value, b: &Value) -> Result<bool> {
    Ok(c::list(a)?.iter().all(|x| c::list(b).expect("array").iter().any(|y| x.same(y))))
}
fn compatible(a: &Value, b: &Value) -> Result<bool> {
    for k in [
        "bible_digest",
        "scope",
        "observer",
        "audience",
        "entity",
        "scene",
        "scene_epoch",
        "format",
        "width",
        "height",
        "aspect",
        "camera_angle",
        "framing",
        "silhouette",
        "attire",
    ] {
        if get(a, k)? != get(b, k)? {
            return Ok(false);
        }
    }
    if ![text(get(b, "expression")?, "slot")?, "neutral"].contains(&text(get(a, "expression")?, "slot")?)
        || ![text(get(b, "pose")?, "id")?, "idle"].contains(&text(get(a, "pose")?, "id")?)
    {
        return Ok(false);
    }
    // A repaired slot/pose cannot fall back to the obsolete tokens for that
    // same slot. Neutral/idle remain eligible for different requested slots.
    for (field, id) in [("expression", "slot"), ("pose", "id")] {
        if get(get(a, field)?, id)? == get(get(b, field)?, id)? && get(a, field)? != get(b, field)? {
            return Ok(false);
        }
    }
    Ok(subset(get(a, "visible_props")?, get(b, "visible_props")?)?
        && subset(get(a, "environment")?, get(b, "environment")?)?
        && subset(get(b, "negative_tokens")?, get(a, "negative_tokens")?)?
        && subset(get(b, "extra_tokens")?, get(a, "extra_tokens")?)?
        && c::list(get(b, "forbidden_props")?)?
            .iter()
            .all(|p| !c::list(get(a, "visible_props").expect("props")).expect("props").contains(p)))
}
impl AssetRegistry {
    pub fn new(config: RegistryConfig) -> Self {
        Self { config, rows: Vec::new(), aliases: Vec::new(), next: 0 }
    }
    fn index(&self, key: &str) -> Result<usize> {
        self.rows.iter().position(|r| r.spec.key() == key).ok_or_else(c::bad)
    }
    fn resolve(&self, key: &str) -> String {
        self.aliases.iter().find(|(k, _)| k == key).map_or_else(|| key.into(), |(_, v)| v.clone())
    }
    fn fallback(&self, spec: &VisualSpec) -> Result<Value> {
        let mut eligible = Vec::new();
        for row in &self.rows {
            if matches!(row.state, "approved" | "pinned")
                && row.asset.is_some()
                && compatible(&row.spec.data, &spec.data)?
            {
                let score = 4 * usize::from(row.spec.data.get("expression") == spec.data.get("expression"))
                    + 2 * usize::from(row.spec.data.get("pose") == spec.data.get("pose"))
                    + usize::from(row.state == "pinned");
                eligible.push((std::cmp::Reverse(score), row.spec.key(), row));
            }
        }
        eligible.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)));
        Ok(eligible.first().map_or(Value::Null, |(_, key, row)| row.asset.as_ref().expect("approved asset").keyed(key)))
    }
    /// Non-blocking selection. A generation value queues work only; no request is
    /// dispatched. Candidate URIs are never returned as the default reusable asset.
    pub fn request(&mut self, spec: &VisualSpec) -> Result<Value> {
        self.select(spec, true)
    }
    /// Inspect/reuse/register a missing spec without enqueueing generation. This is
    /// the passive display path for an explicit Generate button.
    pub fn lookup(&mut self, spec: &VisualSpec) -> Result<Value> {
        self.select(spec, false)
    }
    /// Render the exact stored variant, including developer repairs.
    pub fn render(&self, key: &str) -> Result<Value> {
        self.rows[self.index(key)?].spec.render()
    }
    fn select(&mut self, spec: &VisualSpec, enqueue: bool) -> Result<Value> {
        let key = self.resolve(spec.key());
        let existing = self.rows.iter().position(|r| r.spec.key() == key);
        let i = if let Some(i) = existing {
            i
        } else if self.rows.len() < self.config.capacity {
            self.rows.push(Entry::new(spec.clone()));
            self.rows.len() - 1
        } else {
            return Ok(
                obj! {"key"=>key,"state"=>"missing","asset"=>self.fallback(spec)?,"exact"=>false,"pending"=>false,"generation"=>Value::Null,"reason"=>"capacity"},
            );
        };
        if key == spec.key() && self.rows[i].spec.data != spec.data {
            return Err(c::bad());
        }
        set(&mut self.rows[i].spec.inspection, "state", get(&spec.inspection, "state")?.clone());
        if matches!(self.rows[i].state, "approved" | "pinned") {
            let row = &self.rows[i];
            return Ok(
                obj! {"key"=>key.clone(),"state"=>row.state,"asset"=>row.asset.as_ref().expect("approved asset").keyed(&key),"exact"=>true,"pending"=>false,"generation"=>Value::Null,"reason"=>"reuse"},
            );
        }
        let fallback = self.fallback(&self.rows[i].spec)?;
        let mut job = Value::Null;
        let state = self.rows[i].state;
        let mut reason = if state == "candidate" {
            "pending"
        } else if state == "rejected" {
            "rejected"
        } else if !self.config.enabled {
            "disabled"
        } else if !enqueue {
            "not_requested"
        } else {
            "queue_limit"
        };
        if enqueue
            && state == "missing"
            && self.config.enabled
            && self.rows.iter().filter(|r| r.ticket.is_some()).count() < self.config.queue_limit
        {
            if self.next >= u32::MAX as u64 {
                return Err(c::bad());
            }
            self.next += 1;
            let ticket = obj! {"epoch"=>self.config.epoch.clone(),"sequence"=>self.next};
            self.rows[i].ticket = Some(ticket.clone());
            self.rows[i].state = "candidate";
            job = obj! {"key"=>key.clone(),"ticket"=>ticket,"spec"=>self.rows[i].spec.data.clone()};
            reason = "queued";
        }
        Ok(
            obj! {"key"=>key,"state"=>self.rows[i].state,"asset"=>fallback,"exact"=>false,"pending"=>self.rows[i].state=="candidate","generation"=>job,"reason"=>reason},
        )
    }
    /// Worker submission is not approval. It consumes its one active epoch/sequence.
    pub fn submit(&mut self, key: &str, ticket: &Value, uri: &str, digest: &str) -> Result<Value> {
        bounded(ticket, 1024)?;
        c::fields(ticket, &["epoch", "sequence"])?;
        c::scope(text(ticket, "epoch")?)?;
        c::uint(get(ticket, "sequence")?, 1, u32::MAX as u64)?;
        let i = self.index(key)?;
        let asset = AssetRef::new(uri, digest)?;
        let r = &mut self.rows[i];
        if r.retired || r.state != "candidate" || !r.ticket.as_ref().is_some_and(|t| t.same(ticket)) {
            return Err(c::bad());
        }
        r.candidate = Some(asset);
        r.ticket = None;
        Ok(obj! {"key"=>key,"state"=>"candidate"})
    }
    /// Privileged human/host approval; URI/digest are host attestations, not fetched or
    /// pixel-validated here. A submitted candidate must match the reviewed reference.
    pub fn approve(&mut self, key: &str, uri: &str, digest: &str) -> Result<Value> {
        let i = self.index(key)?;
        let asset = AssetRef::new(uri, digest)?;
        let r = &mut self.rows[i];
        if r.retired || !matches!(r.state, "missing" | "candidate" | "approved" | "pinned") {
            return Err(c::bad());
        }
        if matches!(r.state, "approved" | "pinned") {
            if r.asset.as_ref() != Some(&asset) {
                return Err(c::bad());
            }
            return Ok(obj! {"key"=>key,"state"=>r.state});
        }
        if r.candidate.as_ref().is_some_and(|a| *a != asset) {
            return Err(c::bad());
        }
        r.state = "approved";
        r.asset = Some(asset);
        r.candidate = None;
        r.ticket = None;
        Ok(obj! {"key"=>key,"state"=>"approved"})
    }
    pub fn reject(&mut self, key: &str, reason: &str) -> Result<Value> {
        token(reason)?;
        let i = self.index(key)?;
        let r = &mut self.rows[i];
        if r.retired {
            return Err(c::bad());
        }
        r.state = "rejected";
        r.asset = None;
        r.candidate = None;
        r.ticket = None;
        Ok(obj! {"key"=>key,"state"=>"rejected","reason"=>reason})
    }
    /// Derive a new content key. Locked style/camera/size/palette cannot be patched.
    /// Old pending work is invalidated only after complete patch/capacity validation.
    pub fn repair(&mut self, key: &str, p: &Value) -> Result<Value> {
        bounded(p, 8192)?;
        c::fields(p, &["replace_tokens", "add_tokens", "negative_tokens", "forbid_props", "seed_delta", "asset"])?;
        let i = self.index(key)?;
        let old = &self.rows[i];
        if old.retired || c::list(get(&old.spec.inspection, "repairs")?)?.len() >= 8 {
            return Err(c::bad());
        }
        let mut d = old.spec.data.clone();
        let replacements = c::list(get(p, "replace_tokens")?)?;
        if replacements.len() > 16 {
            return Err(c::bad());
        }
        let add = tokens(get(p, "add_tokens")?, 16)?;
        let negatives = tokens(get(p, "negative_tokens")?, 32)?;
        let forbid = tokens(get(p, "forbid_props")?, 16)?;
        let seed = num(p, "seed_delta")? as u32;
        let mut editable = vec![text(&d, "silhouette")?.to_owned(), text(&d, "attire")?.to_owned()];
        for k in ["expression", "pose"] {
            editable.extend(tokens(get(get(&d, k)?, "tokens")?, 64)?);
        }
        for cue in c::list(get(&d, "environment")?)? {
            editable.extend(tokens(get(cue, "tokens")?, 64)?);
        }
        editable.extend(tokens(get(&d, "extra_tokens")?, 64)?);
        for (n, q) in replacements.iter().enumerate() {
            c::fields(q, &["from", "to"])?;
            token(text(q, "from")?)?;
            token(text(q, "to")?)?;
            if !editable.iter().any(|x| x == text(q, "from").unwrap_or(""))
                || replacements[..n].iter().any(|x| x.get("from") == q.get("from"))
            {
                return Err(c::bad());
            }
            for k in ["silhouette", "attire"] {
                if d.get(k) == q.get("from") {
                    set(&mut d, k, get(q, "to")?.clone());
                }
            }
            for k in ["expression", "pose"] {
                let mut sub = get(&d, k)?.clone();
                let rows = replace(get(&sub, "tokens")?, q)?;
                set(&mut sub, "tokens", rows);
                set(&mut d, k, sub);
            }
            let extra = replace(get(&d, "extra_tokens")?, q)?;
            set(&mut d, "extra_tokens", extra);
            let mut env = c::list(get(&d, "environment")?)?.to_vec();
            for cue in &mut env {
                let xs = replace(get(cue, "tokens")?, q)?;
                set(cue, "tokens", xs);
            }
            set(&mut d, "environment", env.into());
        }
        for (k, xs) in [
            ("extra_tokens", add),
            ("negative_tokens", negatives.into_iter().chain(forbid.iter().map(|p| format!("no {p}"))).collect()),
            ("forbidden_props", forbid),
        ] {
            let mut rows = tokens(get(&d, k)?, 64)?;
            for x in &xs {
                token(x)?;
            }
            rows.extend(xs);
            rows.sort();
            rows.dedup();
            if rows.len() > 64 {
                return Err(c::bad());
            }
            set(&mut d, k, rows.into());
        }
        let forbidden = tokens(get(&d, "forbidden_props")?, 64)?;
        let props =
            tokens(get(&d, "visible_props")?, 16)?.into_iter().filter(|p| !forbidden.contains(p)).collect::<Vec<_>>();
        set(&mut d, "visible_props", props.into());
        let mut env = c::list(get(&d, "environment")?)?
            .iter()
            .filter(|cue| {
                tokens(get(cue, "props").expect("props"), 64).expect("props").iter().all(|p| !forbidden.contains(p))
            })
            .cloned()
            .collect::<Vec<_>>();
        env.sort_by_key(Value::to_json);
        set(&mut d, "environment", env.into());
        let rotated = (num(&d, "seed")? as u32).wrapping_add(seed);
        set(&mut d, "seed", rotated.into());
        let asset = if get(p, "asset")? == &Value::Null { None } else { Some(AssetRef::from_value(get(p, "asset")?)?) };
        let mut inspection = old.spec.inspection.clone();
        let mut patches = c::list(get(&inspection, "repairs")?)?.to_vec();
        patches.push(p.clone());
        set(&mut inspection, "repairs", patches.into());
        let updated = VisualSpec::new(d, inspection)?;
        let new = updated.key().to_owned();
        if new == key && asset.is_none() {
            return Err(c::bad());
        }
        let existing = self.rows.iter().position(|r| r.spec.key() == new);
        if existing.is_none() && self.rows.len() >= self.config.capacity {
            return Err(c::bad());
        }
        if existing.is_some_and(|j| self.rows[j].spec.data != updated.data) {
            return Err(c::bad());
        }
        let invalidated = self.rows[i].ticket.take().unwrap_or(Value::Null);
        self.rows[i].candidate = None;
        self.rows[i].retired = new != key;
        let index = if let Some(index) = existing {
            index
        } else {
            self.rows.push(Entry::new(updated.clone()));
            self.rows.len() - 1
        };
        self.rows[index].retired = false;
        if self.rows[index].state == "candidate"
            && self.rows[index].ticket.is_none()
            && self.rows[index].candidate.is_none()
        {
            self.rows[index].state = "missing";
        }
        self.rows[index].spec.inspection = updated.inspection;
        if let Some(asset) = asset {
            self.rows[index].state = "pinned";
            self.rows[index].asset = Some(asset);
            self.rows[index].candidate = None;
            self.rows[index].ticket = None;
        }
        for (_, target) in &mut self.aliases {
            if target == key {
                *target = new.clone();
            }
        }
        for source in [key, &new] {
            if let Some((_, target)) = self.aliases.iter_mut().find(|(k, _)| k == source) {
                *target = new.clone();
            } else {
                self.aliases.push((source.into(), new.clone()));
            }
        }
        Ok(obj! {"key"=>new,"state"=>self.rows[index].state,"invalidated"=>invalidated})
    }
    pub fn inspect(&self, key: &str) -> Result<Value> {
        Ok(self.rows[self.index(key)?].value())
    }
    pub fn summary(&self) -> Value {
        obj! {"entries"=>self.rows.iter().map(|r|obj!{"key"=>r.spec.key.clone(),"state"=>r.state,"retired"=>r.retired,"pending"=>r.ticket.is_some()}).collect::<Vec<_>>(),"next"=>self.next,"epoch"=>self.config.epoch.clone()}
    }
}
fn replace(rows: &Value, q: &Value) -> Result<Value> {
    let mut out = c::list(rows)?
        .iter()
        .map(|x| if Some(x) == q.get("from") { get(q, "to").expect("to").clone() } else { x.clone() })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    out.dedup();
    Ok(out.into())
}
pub fn workflow(a: &Value) -> Result<Value> {
    bounded(a, crate::kit::MAX_REQUEST_BYTES)?;
    c::fields(a, &["config", "specs", "operations"])?;
    let specs = c::list(get(a, "specs")?)?;
    if specs.len() > 16 {
        return Err(c::bad());
    }
    let specs = specs.iter().map(compile_fixture).collect::<Result<Vec<_>>>()?;
    let mut registry = AssetRegistry::new(RegistryConfig::from_value(get(a, "config")?)?);
    let ops = c::list(get(a, "operations")?)?;
    if ops.len() > 256 {
        return Err(c::bad());
    }
    let mut keys = Vec::<String>::new();
    let mut jobs = Vec::<Value>::new();
    let mut results = Vec::new();
    let mut queued = 0;
    let mut bytes = 0;
    for op in ops {
        let mut run = || -> Result<Value> {
            c::fields(op, &["op", "args"])?;
            let v = get(op, "args")?;
            let kind = text(op, "op")?;
            if kind == "request" {
                c::fields(v, &["spec"])?;
                let s = specs.get(num(v, "spec")? as usize).ok_or_else(c::bad)?;
                let r = registry.request(s)?;
                keys.push(text(&r, "key")?.into());
                let job = get(&r, "generation")?;
                jobs.push(if job == &Value::Null {
                    Value::Null
                } else {
                    queued += 1;
                    get(job, "ticket")?.clone()
                });
                return Ok(r);
            }
            let key = keys.get(num(v, "key")? as usize).ok_or_else(c::bad)?.clone();
            match kind {
                "approve" => {
                    c::fields(v, &["key", "asset_uri", "digest"])?;
                    registry.approve(&key, text(v, "asset_uri")?, text(v, "digest")?)
                }
                "submit" => {
                    c::fields(v, &["key", "ticket", "asset_uri", "digest"])?;
                    registry.submit(
                        &key,
                        jobs.get(num(v, "ticket")? as usize).ok_or_else(c::bad)?,
                        text(v, "asset_uri")?,
                        text(v, "digest")?,
                    )
                }
                "reject" => {
                    c::fields(v, &["key", "reason"])?;
                    registry.reject(&key, text(v, "reason")?)
                }
                "repair" => {
                    c::fields(v, &["key", "patch"])?;
                    let r = registry.repair(&key, get(v, "patch")?)?;
                    keys.push(text(&r, "key")?.into());
                    jobs.push(Value::Null);
                    Ok(r)
                }
                "inspect" => {
                    c::fields(v, &["key"])?;
                    registry.inspect(&key)
                }
                _ => Err(c::bad()),
            }
        };
        let r = match run() {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        };
        bytes += r.json_len();
        if bytes > crate::kit::MAX_REQUEST_BYTES {
            return Err(c::bad());
        }
        results.push(r);
    }
    Ok(obj! {"results"=>results,"queued_generation_requests"=>queued,"actual_model_calls"=>0})
}
