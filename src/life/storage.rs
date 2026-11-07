//! Bounded checkpoints and host-owned archival storage. Authenticate identity/fences in
//! the host. JSON, a filename and a content fingerprint are not authentication.
use super::continuity::set;
use super::*;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
fn identity(v: &Value) -> Result<()> {
    c::fields(v, &["campaign", "branch", "instance"])?;
    for k in ["campaign", "branch", "instance"] {
        c::scope(text(v, k)?)?;
    }
    Ok(())
}
fn config(c: &Config) -> Value {
    obj! {"enabled"=>c.enabled,"balanced"=>c.balanced,"deception"=>c.deception,"mistaken"=>c.mistaken,"tools"=>c.tools.clone()}
}
impl Life {
    /// Capture a recoverable host-only checkpoint at any consequential event, not just
    /// conversation end. Scope and definition content bind identity beyond its label.
    pub fn checkpoint(&self, id: &Value) -> Result<Value> {
        identity(id)?;
        let v = obj! {"version"=>1,"identity"=>id.clone(),"scope"=>self.scope.clone(),"actor"=>self.actor.clone(),"participants"=>self.participants.clone(),"definition"=>self.definition.to_value(),"config"=>config(&self.config),"state"=>self.inspect(),"archive_ack"=>self.archive_ack,"summaries"=>self.summaries.clone()};
        bounded(&v, MAX_BYTES)?;
        Ok(v)
    }
    /// Load only within an authenticated branch-specific fence. Restore an earlier save
    /// into a fresh instance with that save's fence. Live instances never roll backward.
    pub fn restore_checkpoint(&mut self, id: &Value, v: &Value, fence: &Value) -> Result<()> {
        identity(id)?;
        bounded(v, MAX_BYTES)?;
        c::fields(
            v,
            &[
                "version",
                "identity",
                "scope",
                "actor",
                "participants",
                "definition",
                "config",
                "state",
                "archive_ack",
                "summaries",
            ],
        )?;
        c::uint(get(v, "version")?, 1, 1)?;
        if get(v, "identity")? != id
            || text(v, "scope")? != self.scope
            || text(v, "actor")? != self.actor
            || names(get(v, "participants")?, 16)? != self.participants
            || get(v, "definition")? != &self.definition.to_value()
            || get(v, "config")? != &config(&self.config)
        {
            return Err(c::bad());
        }
        c::fields(fence, &["min_revision", "max_revision", "min_token", "max_token", "max_tick"])?;
        let state = get(v, "state")?;
        c::fields(state, &["revision", "watermark", "clock", "attitudes", "events", "changes"])?;
        let rev = num(state, "revision")?;
        let water = num(state, "watermark")?;
        let clock = num(state, "clock")?;
        if rev < num(fence, "min_revision")?
            || rev > num(fence, "max_revision")?
            || water < num(fence, "min_token")?
            || water > num(fence, "max_token")?
            || clock > num(fence, "max_tick")?
            || rev < self.revision
            || water < self.watermark
            || clock < self.clock
        {
            return Err(c::bad());
        }
        if rev == self.revision && v != &self.checkpoint(id)? {
            return Err(c::bad());
        }
        let ack = num(v, "archive_ack")?;
        if ack > water {
            return Err(c::bad());
        }
        let events = c::list(get(state, "events")?)?;
        if events.len() > 128 {
            return Err(c::bad());
        }
        let (mut last, mut at) = (0, 0);
        for e in events {
            validate_event(e, &self.actor, &self.participants)?;
            let token = num(e, "token")?;
            let tick = num(e, "at")?;
            if token <= last
                || token > water
                || tick < at
                || tick > clock
                || !self
                    .definition
                    .entities
                    .iter()
                    .any(|x| Some(x.as_str()) == e.get("subject").and_then(Value::as_str))
            {
                return Err(c::bad());
            }
            last = token;
            at = tick;
        }
        let attitudes = c::list(get(state, "attitudes")?)?;
        if attitudes.len() != self.attitudes.len() {
            return Err(c::bad());
        }
        for (a, initial) in attitudes.iter().zip(&self.attitudes) {
            c::fields(a, &["definition", "level", "supports", "contradicts", "last"])?;
            if a.get("definition") != initial.get("definition") {
                return Err(c::bad());
            }
            signed(get(a, "level")?, 100)?;
            if get(a, "last")? != &Value::Null {
                c::uint(get(a, "last")?, 0, clock)?;
            }
            for k in ["supports", "contradicts"] {
                let refs = c::list(get(a, k)?)?;
                if refs.len() > 16 {
                    return Err(c::bad());
                }
                let mut prev = 0;
                for r in refs {
                    let token = c::uint(r, prev + 1, water)?;
                    prev = token;
                    if token > ack && !events.iter().any(|e| num(e, "token").ok() == Some(token)) {
                        return Err(c::bad());
                    }
                }
            }
            if get(a, "last")? == &Value::Null
                && (a.get("level") != get(a, "definition")?.get("level")
                    || !c::list(get(a, "supports")?)?.is_empty()
                    || !c::list(get(a, "contradicts")?)?.is_empty())
            {
                return Err(c::bad());
            }
        }
        let changes = c::list(get(state, "changes")?)?;
        if changes.len() > 128 {
            return Err(c::bad());
        }
        for (i, x) in changes.iter().enumerate() {
            c::fields(x, &["rule", "event", "at", "before", "after", "reason"])?;
            let r = c::list(get(&self.definition.value, "rules")?)?
                .iter()
                .find(|r| r.get("id") == x.get("rule"))
                .ok_or_else(c::bad)?;
            let token = c::uint(get(x, "event")?, 1, water)?;
            let tick = c::uint(get(x, "at")?, 0, clock)?;
            let before = signed(get(x, "before")?, 100)?;
            let after = signed(get(x, "after")?, 100)?;
            let mut wanted = (before + signed(get(r, "delta")?, 20)?).clamp(-100, 100);
            if before * wanted < 0 {
                wanted = 0;
            }
            if after != wanted
                || x.get("reason") != r.get("reason")
                || changes[..i].iter().any(|o| o.get("rule") == x.get("rule") && o.get("event") == x.get("event"))
            {
                return Err(c::bad());
            }
            if token > ack {
                let e = events.iter().find(|e| num(e, "token").ok() == Some(token)).ok_or_else(c::bad)?;
                if num(e, "at")? != tick
                    || e.get("tag") != r.get("event")
                    || !matches!(text(e, "kind")?, "direct" | "outcome")
                {
                    return Err(c::bad());
                }
            }
        }
        let summaries = c::list(get(v, "summaries")?)?;
        if summaries.len() > 16 {
            return Err(c::bad());
        }
        for (i, s) in summaries.iter().enumerate() {
            c::fields(s, &["start", "end", "events"])?;
            let start = c::uint(get(s, "start")?, 1, water)?;
            let end = c::uint(get(s, "end")?, start, water)?;
            let rows = c::list(get(s, "events")?)?;
            if rows.len() > 128
                || rows.len() as u64 != end - start + 1
                || summaries[..i].iter().any(|o| num(o, "start").unwrap() <= end && num(o, "end").unwrap() >= start)
            {
                return Err(c::bad());
            }
            for (j, e) in rows.iter().enumerate() {
                validate_event(e, &self.actor, &self.participants)?;
                if num(e, "token")? != start + j as u64 || num(e, "at")? > clock {
                    return Err(c::bad());
                }
                if let Some(original) = events.iter().find(|o| o.get("token") == e.get("token")) {
                    if original != e {
                        return Err(c::bad());
                    }
                }
            }
        }
        let mut next = self.clone();
        next.revision = rev;
        next.clock = clock;
        next.watermark = water;
        next.archive_ack = ack;
        next.events = events.to_vec();
        next.changes = changes.to_vec();
        next.attitudes = attitudes.to_vec();
        next.summaries = summaries.to_vec();
        *self = next;
        Ok(())
    }
    pub fn acknowledge_archive(&mut self, through: u64) -> Result<()> {
        self.active()?;
        if through < self.archive_ack || through > self.watermark {
            return Err(c::bad());
        }
        if through == self.archive_ack {
            return Ok(());
        }
        self.bump()?;
        self.archive_ack = through;
        Ok(())
    }
    pub fn evict(&mut self, through: u64) -> Result<usize> {
        self.active()?;
        if through > self.watermark
            || self.events.iter().any(|e| {
                num(e, "token").unwrap() <= through
                    && get(e, "retain").unwrap() == &Value::Bool(true)
                    && num(e, "token").unwrap() > self.archive_ack
            })
        {
            return Err(c::bad());
        }
        let n = self.events.iter().filter(|e| num(e, "token").unwrap() <= through).count();
        self.bump()?;
        self.events.retain(|e| num(e, "token").unwrap() > through);
        self.changes.retain(|e| num(e, "event").unwrap() > through);
        Ok(n)
    }
    /// Incremental extractive consolidation. Each original event range occurs once;
    /// no generated text or summary-of-summary can become stronger evidence.
    pub fn consolidate(&mut self, start: u64, end: u64) -> Result<Value> {
        self.active()?;
        if let Some(s) =
            self.summaries.iter().find(|s| num(s, "start").ok() == Some(start) && num(s, "end").ok() == Some(end))
        {
            return Ok(s.clone());
        }
        if start == 0
            || start > end
            || end > self.watermark
            || end - start >= 128
            || self.summaries.len() >= 16
            || self.summaries.iter().any(|s| num(s, "start").unwrap() <= end && num(s, "end").unwrap() >= start)
        {
            return Err(c::bad());
        }
        let rows = self
            .events
            .iter()
            .filter(|e| num(e, "token").unwrap() >= start && num(e, "token").unwrap() <= end)
            .cloned()
            .collect::<Vec<_>>();
        if rows.len() as u64 != end - start + 1 {
            return Err(c::bad());
        }
        let value = obj! {"start"=>start,"end"=>end,"events"=>rows};
        let mut next = self.clone();
        next.bump()?;
        next.summaries.push(value.clone());
        bounded(&next.checkpoint(&obj! {"campaign"=>"check","branch"=>"check","instance"=>"check"})?, MAX_BYTES)?;
        *self = next;
        Ok(value)
    }
    /// Reuse the existing lexical memory ranker, with retention significance and host
    /// tags. Return original events, including corrections/causes, never rewritten facts.
    pub fn recall_episodes(&self, input: &str, tags: &Value, count: usize) -> Result<Value> {
        self.active()?;
        if input.len() > 2048 || count > 16 {
            return Err(c::bad());
        }
        let tags = names(tags, 8)?;
        let mut memory =
            crate::memory::Memory::with_limits(crate::memory::Limits { records: 128, text_chars: 512, tags: 8 });
        for e in &self.events {
            let mut labels = vec![text(e, "tag")?];
            if tags.iter().any(|t| t == text(e, "subject").unwrap_or("")) {
                labels.push(text(e, "subject")?);
            }
            memory.remember(
                num(e, "at")?,
                text(e, "text")?,
                &labels,
                if get(e, "retain")? == &Value::Bool(true) { 9 } else { 3 },
            )?;
        }
        let query = format!("{} {}", input, tags.join(" "));
        let ranked = memory.recall(&query, count);
        let mut selected = ranked.iter().map(|r| self.events[r.id as usize - 1].clone()).collect::<Vec<_>>();
        // Causal neighbors and corrections must fit the explicit cap, or fail.
        let mut i = 0;
        while i < selected.len() {
            let token = num(&selected[i], "token")?;
            for e in &self.events {
                let related = c::list(get(&selected[i], "causes")?)?
                    .iter()
                    .any(|x| x.as_usize() == num(e, "token").ok().map(|n| n as usize))
                    || c::list(get(e, "causes")?)?.iter().any(|x| x.as_usize() == Some(token as usize));
                if related && !selected.contains(e) {
                    if selected.len() == 16 {
                        return Err(c::bad());
                    }
                    selected.push(e.clone());
                }
            }
            i += 1;
        }
        selected.sort_by_key(|e| num(e, "token").unwrap());
        Ok(selected.into())
    }
    pub fn memory_stats(&self) -> Value {
        obj! {"revision"=>self.revision,"events"=>self.events.len(),"watermark"=>self.watermark,"archive_ack"=>self.archive_ack,"summaries"=>self.summaries.len()}
    }
    pub fn flush(&mut self, id: &Value, store: &mut impl Storage) -> Result<()> {
        identity(id)?;
        for e in &self.events {
            if num(e, "token")? > self.archive_ack && get(e, "retain")? == &Value::Bool(true) {
                store.archive(id, e)?;
            }
        }
        self.acknowledge_archive(self.watermark)?;
        store.checkpoint(&self.checkpoint(id)?)
    }
}
/// Hosts implement authentication, encryption, transactions and retention. Archive
/// writes must deduplicate exact identity/token pairs and reject conflicting duplicates.
pub trait Storage {
    fn archive(&mut self, identity: &Value, event: &Value) -> Result<()>;
    fn checkpoint(&mut self, value: &Value) -> Result<()>;
}
/// Reference adapter: one host-selected directory per character/save branch. Immutable
/// revision files and archive records use atomic rename, file sync and directory sync.
/// Never expose this path or storage operations as model tools.
pub struct FileStore {
    root: PathBuf,
}
fn io(e: std::io::Error) -> crate::Error {
    crate::Error::new(e.to_string())
}
fn read(path: &Path, limit: usize) -> Result<Value> {
    let mut bytes = Vec::new();
    File::open(path).map_err(io)?.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() > limit {
        return Err(c::bad());
    }
    crate::json::parse(std::str::from_utf8(&bytes).map_err(|_| c::bad())?, limit)
}
fn immutable(path: &Path, v: &Value) -> Result<()> {
    if path.exists() {
        if read(path, MAX_BYTES)? == *v {
            return Ok(());
        }
        return Err(c::bad());
    }
    let pending = path.with_extension("pending");
    let bytes = v.to_json();
    if pending.exists() {
        if read(&pending, MAX_BYTES)? != *v {
            return Err(c::bad());
        }
    } else {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&pending).map_err(io)?;
        file.write_all(bytes.as_bytes()).map_err(io)?;
        file.sync_all().map_err(io)?;
    }
    fs::rename(&pending, path).map_err(io)?;
    File::open(path.parent().ok_or_else(c::bad)?).map_err(io)?.sync_all().map_err(io)?;
    Ok(())
}
impl FileStore {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let root = path.into();
        fs::create_dir_all(&root).map_err(io)?;
        Ok(Self { root })
    }
    fn bind(&self, id: &Value) -> Result<()> {
        identity(id)?;
        immutable(&self.root.join("identity.json"), id)
    }
    pub fn load(&self, id: &Value, revision: u64) -> Result<Value> {
        self.bind(id)?;
        let v = read(&self.root.join(format!("save_{revision}.json")), MAX_BYTES)?;
        if get(&v, "identity")? != id {
            return Err(c::bad());
        }
        Ok(v)
    }
    pub fn event(&self, id: &Value, token: u64) -> Result<Value> {
        self.bind(id)?;
        read(&self.root.join(format!("event_{token}.json")), 8192)
    }
}
impl Storage for FileStore {
    fn archive(&mut self, id: &Value, event: &Value) -> Result<()> {
        self.bind(id)?;
        bounded(event, 8192)?;
        immutable(&self.root.join(format!("event_{}.json", num(event, "token")?)), event)
    }
    fn checkpoint(&mut self, v: &Value) -> Result<()> {
        bounded(v, MAX_BYTES)?;
        self.bind(get(v, "identity")?)?;
        immutable(&self.root.join(format!("save_{}.json", num(get(v, "state")?, "revision")?)), v)
    }
}
pub fn fixture(a: &Value) -> Result<Value> {
    c::fields(
        a,
        &["setting", "definition", "defaults", "overrides", "scope", "actor", "participants", "operations", "identity"],
    )?;
    let definition = Arc::new(Definition::draft(get(a, "definition")?, &Setting::from_value(get(a, "setting")?)?)?);
    let config = Config::resolve(get(a, "defaults")?, get(a, "overrides")?)?;
    let fresh = || {
        Life::accept(text(a, "scope")?, text(a, "actor")?, get(a, "participants")?, definition.clone(), config.clone())
    };
    let mut life = fresh()?;
    let id = get(a, "identity")?;
    identity(id)?;
    let mut saved = None;
    let ops = c::list(get(a, "operations")?)?;
    if ops.len() > 2048 {
        return Err(c::bad());
    }
    let mut results = Vec::new();
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let v = get(op, "args")?;
        let result = match text(op, "op")? {
            "experience" => life.experience(v).map(|()| obj! {"revision"=>life.revision}),
            "ack" => {
                c::fields(v, &["through"])?;
                life.acknowledge_archive(num(v, "through")?).map(|()| obj! {"archived"=>life.archive_ack})
            }
            "evict" => {
                c::fields(v, &["through"])?;
                life.evict(num(v, "through")?).map(|n| obj! {"evicted"=>n})
            }
            "consolidate" => {
                c::fields(v, &["start", "end"])?;
                life.consolidate(num(v, "start")?, num(v, "end")?)
            }
            "save" => {
                c::fields(v, &[])?;
                saved = Some(life.checkpoint(id)?);
                Ok(obj! {"saved"=>life.revision})
            }
            "restore" => {
                c::fields(v, &["corrupt"])?;
                if let Some(s) = &saved {
                    let mut s = s.clone();
                    if c::boolean(get(v, "corrupt")?)? {
                        set(&mut s, "identity", obj! {"campaign"=>"foreign","branch"=>"foreign","instance"=>"foreign"});
                    }
                    let mut next = fresh()?;
                    let r=next.restore_checkpoint(id,&s,&obj!{"min_revision"=>0,"max_revision"=>life.revision,"min_token"=>0,"max_token"=>life.watermark,"max_tick"=>life.clock});
                    r.map(|()| {
                        life = next;
                        obj! {"restored"=>life.revision,"events"=>life.events.len()}
                    })
                } else {
                    Err(c::bad())
                }
            }
            "stats" => {
                c::fields(v, &[])?;
                Ok(life.memory_stats())
            }
            _ => Err(c::bad()),
        };
        results.push(match result {
            Ok(r) => obj! {"ok"=>true,"result"=>r},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results})
}
impl Life {
    /// Scoped erasure invalidates causal dependents and derived attitudes/summaries.
    /// Return tombstones for host archives, indices, caches and pending requests.
    pub fn forget_episodes(&mut self, tokens: &Value, expected: u64) -> Result<Value> {
        self.active()?;
        if expected != self.revision {
            return Err(c::bad());
        }
        let raw = c::list(tokens)?;
        if raw.is_empty() || raw.len() > 128 {
            return Err(c::bad());
        }
        let mut removed = Vec::new();
        for t in raw {
            let token = c::uint(t, 1, self.watermark)?;
            if removed.contains(&token) {
                return Err(c::bad());
            }
            removed.push(token);
        }
        let mut changed = true;
        while changed {
            changed = false;
            for e in &self.events {
                let token = num(e, "token")?;
                if !removed.contains(&token)
                    && c::list(get(e, "causes")?)?
                        .iter()
                        .any(|r| r.as_usize().is_some_and(|n| removed.contains(&(n as u64))))
                {
                    removed.push(token);
                    changed = true;
                }
            }
        }
        let mut next = self.clone();
        next.bump()?;
        next.events.retain(|e| !removed.contains(&num(e, "token").unwrap()));
        next.changes.retain(|e| !removed.contains(&num(e, "event").unwrap()));
        next.summaries
            .retain(|s| !removed.iter().any(|n| *n >= num(s, "start").unwrap() && *n <= num(s, "end").unwrap()));
        let mut reset = Vec::new();
        for a in &mut next.attitudes {
            if ["supports", "contradicts"].iter().any(|k| {
                c::list(get(a, k).unwrap())
                    .unwrap()
                    .iter()
                    .any(|r| r.as_usize().is_some_and(|n| removed.contains(&(n as u64))))
            }) {
                reset.push(get(get(a, "definition")?, "id")?.clone());
                let level = get(get(a, "definition")?, "level")?.clone();
                set(a, "level", level);
                set(a, "last", Value::Null);
                set(a, "supports", Vec::<Value>::new().into());
                set(a, "contradicts", Vec::<Value>::new().into());
            }
        }
        next.changes.retain(|x| {
            !c::list(get(&next.definition.value, "rules").unwrap())
                .unwrap()
                .iter()
                .any(|r| r.get("id") == x.get("rule") && reset.iter().any(|id| Some(id) == r.get("attitude")))
        });
        removed.sort_unstable();
        let result = obj! {"revision"=>next.revision,"forgotten"=>removed,"invalidate"=>vec!["summaries","recall","pending","checkpoints","archives"]};
        *self = next;
        Ok(result)
    }
}
impl FileStore {
    /// Remove scoped archival tokens and every earlier checkpoint containing that scope.
    /// Directory scanning is capped at 4,096 entries. Hosts with larger archives supply
    /// an indexed erasure transaction. A failed filesystem removal must be retried.
    pub fn erase(&mut self, id: &Value, tokens: &Value) -> Result<()> {
        self.bind(id)?;
        let raw = c::list(tokens)?;
        if raw.len() > 128 {
            return Err(c::bad());
        }
        let mut removed = Vec::new();
        for t in raw {
            removed.push(c::uint(t, 1, u32::MAX as u64)?);
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(io)? {
            if entries.len() >= 4096 {
                return Err(c::bad());
            }
            entries.push(entry.map_err(io)?.path());
        }
        // Find causal descendants before making any filesystem mutation.
        let mut records = Vec::new();
        for p in &entries {
            if p.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.starts_with("event_") && s.ends_with(".json")) {
                records.push((p.clone(), read(p, 8192)?));
            }
        }
        let mut changed = true;
        while changed {
            changed = false;
            for (_, e) in &records {
                let token = num(e, "token")?;
                if !removed.contains(&token)
                    && c::list(get(e, "causes")?)?
                        .iter()
                        .any(|r| r.as_usize().is_some_and(|n| removed.contains(&(n as u64))))
                {
                    removed.push(token);
                    changed = true;
                }
            }
        }
        for p in entries {
            let name = p.file_name().and_then(|s| s.to_str()).ok_or_else(c::bad)?;
            let remove = name.starts_with("save_") && (name.ends_with(".json") || name.ends_with(".pending"))
                || records.iter().any(|(path, e)| path == &p && removed.contains(&num(e, "token").unwrap()));
            if remove {
                fs::remove_file(p).map_err(io)?;
            }
        }
        File::open(&self.root).map_err(io)?.sync_all().map_err(io)?;
        Ok(())
    }
}
impl Life {
    pub fn retire_consolidation(&mut self, start: u64, end: u64) -> Result<bool> {
        self.active()?;
        if start == 0 || start > end || end > self.archive_ack {
            return Err(c::bad());
        }
        if let Some(i) =
            self.summaries.iter().position(|s| num(s, "start").ok() == Some(start) && num(s, "end").ok() == Some(end))
        {
            self.bump()?;
            self.summaries.remove(i);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
