//! Visual director: picks an art bible tier and a format profile, turns host state into
//! perceived scene facts through selectors, and runs the approval/repair/fallback
//! workflow over an [`AssetRegistry`]. It never generates, fetches or decodes an image:
//! the host verifies assets through a callback and approves them explicitly.
use super::{ArtBible, AssetRef, AssetRegistry, RegistryConfig, VisualSpec};
use crate::{
    character::life::{get, num, text},
    checked as c,
    data::conditions::Scalar,
    obj,
    packet::request::bounded,
    Result, Value,
};

const FORMATS: [&str; 3] = ["sprite", "portrait", "scene"];
const SOURCE_TEXT: &str = "Host-selected visual fact.";
const MAX_APPROVALS: usize = 128;
const MAX_REPAIRS: usize = 64;

/// A host flag value for selectors: a scope-like string ≤64 chars, a boolean or an integer
/// within ±1e9. Values only match the same JSON type.
fn flag(v: &Value) -> Result<Scalar> {
    if let Value::String(s) = v {
        c::scope(s)?;
        if s.chars().count() > 64 {
            return Err(c::bad());
        }
    }
    Scalar::from_flag(v)
}

#[derive(Clone, Debug)]
struct Selector {
    fact: String,
    when: Vec<(String, Vec<Scalar>)>,
}
impl Selector {
    fn from_value(v: &Value) -> Result<Self> {
        c::fields(v, &["fact", "when"])?;
        c::scope(text(v, "fact")?)?;
        let Value::Object(rows) = get(v, "when")? else { return Err(c::bad()) };
        if rows.len() > 8 {
            return Err(c::bad());
        }
        let mut when = Vec::new();
        for (k, vs) in rows {
            c::scope(k)?;
            let vs = c::list(vs)?;
            if vs.is_empty() || vs.len() > 16 {
                return Err(c::bad());
            }
            when.push((k.clone(), vs.iter().map(flag).collect::<Result<Vec<_>>>()?));
        }
        Ok(Self { fact: text(v, "fact")?.into(), when })
    }
    fn holds(&self, vars: &[(String, Scalar)]) -> bool {
        self.when.iter().all(|(k, vs)| vars.iter().find(|(n, _)| n == k).is_some_and(|(_, x)| vs.contains(x)))
    }
}

/// Apply a format profile to a bible value and re-validate the result.
fn applied(bible: &Value, profile: &Value) -> Result<ArtBible> {
    let mut b = bible.clone();
    super::set(&mut b, "format", get(profile, "format")?.clone());
    for k in ["framing", "camera_angle"] {
        if let Some(x) = profile.get(k).filter(|x| **x != Value::Null) {
            super::set(&mut b, k, x.clone());
        }
    }
    if let Some(size) = profile.get("size").filter(|x| **x != Value::Null) {
        c::fields(size, &["width", "height", "aspect"])?;
        for k in ["width", "height", "aspect"] {
            super::set(&mut b, k, get(size, k)?.clone());
        }
    }
    ArtBible::from_value(&b)
}

fn key(k: &str) -> Result<()> {
    match k.strip_prefix("visual:") {
        Some(h) if h.len() == 64 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) => Ok(()),
        _ => Err(c::bad()),
    }
}

fn approval(a: &Value) -> Result<()> {
    c::fields(a, &["key", "asset_uri", "digest"])?;
    key(text(a, "key")?)?;
    AssetRef::new(text(a, "asset_uri")?, text(a, "digest")?).map(|_| ())
}

fn repair(x: &Value) -> Result<()> {
    c::fields(x, &["base", "patches"])?;
    key(text(x, "base")?)?;
    let patches = c::list(get(x, "patches")?)?;
    if patches.is_empty() || patches.len() > 8 {
        return Err(c::bad());
    }
    for p in patches {
        c::fields(p, &["replace_tokens", "add_tokens", "negative_tokens", "forbid_props", "seed_delta", "asset"])?;
    }
    Ok(())
}

/// Tiered art bibles, format profiles, host fact selectors and the approval, repair and
/// fallback workflow. See `docs/guides/VISUAL.md`.
///
/// ```
/// use gamectx::{data::json, media::visual::{RegistryConfig, VisualDirector}, obj, Value};
/// let bible = json::parse(include_str!("../../../examples/fixtures/visual/chime-art.json"), 65536)?;
/// let config = obj! {"version"=>1,
///     "tiers"=>vec![obj!{"id"=>"basic","bible"=>bible}],
///     "profiles"=>vec![obj!{"id"=>"portrait","format"=>"portrait","framing"=>Value::Null,"camera_angle"=>Value::Null,"size"=>Value::Null}],
///     "selectors"=>vec![obj!{"fact"=>"visible_lamp","when"=>obj!{"lamp"=>vec!["repaired"]}}]};
/// let registry = RegistryConfig::from_value(&obj! {"enabled"=>true,"capacity"=>8,"queue_limit"=>1,"epoch"=>"demo"})?;
/// let mut director = VisualDirector::new(&config, registry, None)?;
/// let request = obj! {"tier"=>Value::Null,"profile"=>"portrait","entity"=>"merchant","scope"=>"counter",
///     "observer"=>"player","listener"=>"player","audience"=>vec!["player"],
///     "scene"=>obj!{"id"=>"counter","revision"=>1,"epoch"=>1,"at"=>10,"participants"=>vec!["merchant","player"]},
///     "vars"=>obj!{"lamp"=>"repaired"},"life_revision"=>Value::Null,"activity_revision"=>Value::Null,"generate"=>true};
/// let out = director.direct(&request, |_uri, _digest| true)?;
/// assert_eq!(out.get("status"), Some(&Value::str("placeholder")));
/// assert_eq!(out.get("selected_facts"), Some(&vec!["visible_lamp"].into()));
/// let key = out.get("key").and_then(Value::as_str).unwrap().to_owned();
/// director.approve(&key, "asset://demo/merchant.png", &"a".repeat(64))?;
/// let again = director.direct(&request, |_, _| true)?;
/// assert_eq!(again.get("status"), Some(&Value::str("exact")));
/// # Ok::<(), gamectx::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct VisualDirector {
    /// `(tier id, bible value)`, lowest tier first.
    tiers: Vec<(String, Value)>,
    profiles: Vec<Value>,
    selectors: Vec<Selector>,
    registry: AssetRegistry,
    /// Host approvals `{key, asset_uri, digest}`, re-verified after a restore.
    approvals: Vec<Value>,
    /// Queued repairs `{base, patches}`, applied once per base key.
    repairs: Vec<Value>,
    applied: Vec<String>,
}

impl VisualDirector {
    /// `config` is `{version:1, tiers:[{id, bible}] (1–4, lowest first), profiles:[{id,
    /// format, framing, camera_angle, size}] (1–8), selectors:[{fact, when}] (≤32)}`.
    /// `checkpoint` is a value from [`VisualDirector::checkpoint`] or `None`.
    pub fn new(config: &Value, registry: RegistryConfig, checkpoint: Option<&Value>) -> Result<Self> {
        bounded(config, 1_048_576)?;
        c::fields(config, &["version", "tiers", "profiles", "selectors"])?;
        c::uint(get(config, "version")?, 1, 1)?;
        let tiers = c::list(get(config, "tiers")?)?;
        let profiles = c::list(get(config, "profiles")?)?;
        let selectors = c::list(get(config, "selectors")?)?;
        if tiers.is_empty() || tiers.len() > 4 || profiles.is_empty() || profiles.len() > 8 || selectors.len() > 32 {
            return Err(c::bad());
        }
        let mut out = Self {
            tiers: Vec::new(),
            profiles: Vec::new(),
            selectors: Vec::new(),
            registry: AssetRegistry::new(registry),
            approvals: Vec::new(),
            repairs: Vec::new(),
            applied: Vec::new(),
        };
        for t in tiers {
            c::fields(t, &["id", "bible"])?;
            let id = text(t, "id")?;
            c::name(id)?;
            if out.tiers.iter().any(|(old, _)| old == id) {
                return Err(c::bad());
            }
            ArtBible::from_value(get(t, "bible")?)?;
            out.tiers.push((id.into(), get(t, "bible")?.clone()));
        }
        for p in profiles {
            c::fields(p, &["id", "format", "framing", "camera_angle", "size"])?;
            c::name(text(p, "id")?)?;
            if !FORMATS.contains(&text(p, "format")?) {
                return Err(c::bad());
            }
            for k in ["framing", "camera_angle"] {
                if let Some(x) = p.get(k).filter(|x| **x != Value::Null) {
                    super::token(c::text(x)?)?;
                }
            }
            if let Some(size) = p.get("size").filter(|x| **x != Value::Null) {
                c::fields(size, &["width", "height", "aspect"])?;
            }
            if out.profiles.iter().any(|o| o.get("id") == p.get("id")) {
                return Err(c::bad());
            }
            for (_, b) in &out.tiers {
                applied(b, p)?;
            }
            out.profiles.push(p.clone());
        }
        for s in selectors {
            let s = Selector::from_value(s)?;
            if out.selectors.iter().any(|o| o.fact == s.fact) {
                return Err(c::bad());
            }
            out.selectors.push(s);
        }
        if let Some(cp) = checkpoint {
            bounded(cp, 1_048_576)?;
            c::fields(cp, &["version", "approvals", "repairs"])?;
            c::uint(get(cp, "version")?, 1, 1)?;
            let approvals = c::list(get(cp, "approvals")?)?;
            let repairs = c::list(get(cp, "repairs")?)?;
            if approvals.len() > MAX_APPROVALS || repairs.len() > MAX_REPAIRS {
                return Err(c::bad());
            }
            for a in approvals {
                approval(a)?;
                if out.approvals.iter().any(|o| o.get("key") == a.get("key")) {
                    return Err(c::bad());
                }
                out.approvals.push(a.clone());
            }
            for x in repairs {
                repair(x)?;
                if out.repairs.iter().any(|o| o.get("base") == x.get("base")) {
                    return Err(c::bad());
                }
                out.repairs.push(x.clone());
            }
        }
        Ok(out)
    }

    /// The registry, for inspection and summaries.
    pub fn registry(&self) -> &AssetRegistry {
        &self.registry
    }

    /// Saved approvals and queued repairs, sorted by key: `{version:1, approvals, repairs}`.
    pub fn checkpoint(&self) -> Value {
        let mut approvals = self.approvals.clone();
        approvals.sort_by(|a, b| a.get("key").and_then(Value::as_str).cmp(&b.get("key").and_then(Value::as_str)));
        let mut repairs = self.repairs.clone();
        repairs.sort_by(|a, b| a.get("base").and_then(Value::as_str).cmp(&b.get("base").and_then(Value::as_str)));
        obj! {"version"=>1,"approvals"=>approvals,"repairs"=>repairs}
    }

    /// Direct one picture. `request` is `{tier, profile, entity, scope, observer, listener,
    /// audience, scene:{id, revision, epoch, at, participants}, vars, life_revision: null,
    /// activity_revision: null, generate}`. `verify(uri, digest)` tells whether a saved
    /// approval's asset is still present and intact. A failed call changes nothing.
    pub fn direct(&mut self, request: &Value, mut verify: impl FnMut(&str, &str) -> bool) -> Result<Value> {
        bounded(request, 65536)?;
        let q = request;
        c::fields(
            q,
            &[
                "tier",
                "profile",
                "entity",
                "scope",
                "observer",
                "listener",
                "audience",
                "scene",
                "vars",
                "life_revision",
                "activity_revision",
                "generate",
            ],
        )?;
        let generate = c::boolean(get(q, "generate")?)?;
        if get(q, "life_revision")? != &Value::Null || get(q, "activity_revision")? != &Value::Null {
            return Err(c::bad());
        }
        let s = get(q, "scene")?;
        c::fields(s, &["id", "revision", "epoch", "at", "participants"])?;
        let Value::Object(var_rows) = get(q, "vars")? else { return Err(c::bad()) };
        if var_rows.len() > 64 {
            return Err(c::bad());
        }
        let mut vars = Vec::new();
        for (k, v) in var_rows {
            c::scope(k)?;
            vars.push((k.clone(), flag(v)?));
        }
        let requested = match get(q, "tier")? {
            Value::Null => self.tiers.last().expect("tier").0.clone(),
            t => {
                let t = c::text(t)?;
                c::name(t)?;
                t.to_owned()
            }
        };
        let top = self.tiers.iter().position(|(id, _)| *id == requested).ok_or_else(c::bad)?;
        let pid = text(q, "profile")?;
        c::name(pid)?;
        let profile =
            self.profiles.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(pid)).ok_or_else(c::bad)?;
        let kind = if text(profile, "format")? == "scene" { "scene" } else { "character" };
        let mut chosen = None;
        for i in (0..=top).rev() {
            let b = applied(&self.tiers[i].1, profile)?;
            let has = b
                .anchors()?
                .iter()
                .any(|a| a.get("entity") == q.get("entity") && a.get("kind").and_then(Value::as_str) == Some(kind));
            if has {
                chosen = Some((self.tiers[i].0.clone(), b));
                break;
            }
        }
        let (tier, bible) = chosen.ok_or_else(c::bad)?;
        let emitted = self.selectors.iter().filter(|x| x.holds(&vars)).map(|x| x.fact.clone()).collect::<Vec<_>>();
        let facts = emitted
            .iter()
            .map(|f| -> Result<Value> {
                Ok(obj! {"id"=>f.as_str(),"revision"=>get(s,"revision")?.clone(),"observed_at"=>get(s,"at")?.clone(),"source"=>obj!{"id"=>"director","speaker"=>get(q,"observer")?.clone(),"text"=>SOURCE_TEXT},"kind"=>"perceived","audience"=>get(s,"participants")?.clone(),"text"=>SOURCE_TEXT})
            })
            .collect::<Result<Vec<_>>>()?;
        let scene = obj! {"id"=>get(s,"id")?.clone(),"revision"=>get(s,"revision")?.clone(),"epoch"=>get(s,"epoch")?.clone(),"at"=>get(s,"at")?.clone(),"participants"=>get(s,"participants")?.clone(),"facts"=>facts,"affordances"=>Vec::<Value>::new()};
        let frame = obj! {"scope"=>get(q,"scope")?.clone(),"observer"=>get(q,"observer")?.clone(),"entity"=>get(q,"entity")?.clone(),"listener"=>get(q,"listener")?.clone(),"scene"=>scene,"audience"=>get(q,"audience")?.clone(),"life_revision"=>Value::Null,"activity_revision"=>Value::Null};
        let spec = VisualSpec::compile(&bible, &frame, None, &super::Activity::idle())?;

        // Work on copies so a failure leaves the director unchanged.
        let mut reg = self.registry.clone();
        let mut approvals = self.approvals.clone();
        let mut applied_keys = self.applied.clone();
        let mut current = text(&reg.lookup(&spec)?, "key")?.to_owned();
        let exists = |reg: &AssetRegistry, k: &str| reg.inspect(k).ok();
        let mut repaired = false;
        if let Some(x) = self.repairs.iter().find(|x| x.get("base").and_then(Value::as_str) == Some(spec.key())) {
            if !applied_keys.iter().any(|k| k == spec.key()) && exists(&reg, &current).is_some() {
                for patch in c::list(get(x, "patches")?)? {
                    current = text(&reg.repair(&current, patch)?, "key")?.to_owned();
                }
                applied_keys.push(spec.key().to_owned());
                repaired = true;
            }
        }
        let mut state = Value::Null;
        if let Some(i) = approvals.iter().position(|a| a.get("key").and_then(Value::as_str) == Some(current.as_str())) {
            if let Some(row) = exists(&reg, &current) {
                if !matches!(text(&row, "state")?, "approved" | "pinned") {
                    let (uri, digest) =
                        (text(&approvals[i], "asset_uri")?.to_owned(), text(&approvals[i], "digest")?.to_owned());
                    if !verify(&uri, &digest) {
                        approvals.remove(i);
                        state = "unverified".into();
                    } else if reg.approve(&current, &uri, &digest).is_ok() {
                        state = "verified".into();
                    } else {
                        approvals.remove(i);
                        state = "stale".into();
                    }
                }
            }
        }
        let sel = if generate { reg.request(&spec)? } else { reg.lookup(&spec)? };
        self.registry = reg;
        self.approvals = approvals;
        self.applied = applied_keys;
        let exact = get(&sel, "exact")? == &Value::Bool(true);
        let status = if exact {
            "exact"
        } else if get(&sel, "asset")? != &Value::Null {
            "fallback"
        } else {
            "placeholder"
        };
        let selected = get(get(&spec.inspection, "state")?, "selected_facts")?.clone();
        Ok(obj! {
            "key"=>get(&sel,"key")?.clone(),
            "tier"=>tier,
            "requested_tier"=>requested,
            "profile"=>pid,
            "facts"=>emitted.into_iter().map(Value::from).collect::<Vec<_>>(),
            "selected_facts"=>selected,
            "status"=>status,
            "asset"=>get(&sel,"asset")?.clone(),
            "exact"=>exact,
            "pending"=>get(&sel,"pending")?.clone(),
            "generation"=>get(&sel,"generation")?.clone(),
            "reason"=>get(&sel,"reason")?.clone(),
            "repaired"=>repaired,
            "approval"=>state
        })
    }

    /// Host approval of a reviewed asset; it is saved for re-verification after a restore.
    pub fn approve(&mut self, k: &str, uri: &str, digest: &str) -> Result<Value> {
        key(k)?;
        let out = self.registry.approve(k, uri, digest)?;
        let a = obj! {"key"=>k,"asset_uri"=>uri,"digest"=>digest};
        if let Some(old) = self.approvals.iter_mut().find(|a| a.get("key").and_then(Value::as_str) == Some(k)) {
            *old = a;
        } else {
            self.approvals.push(a);
        }
        Ok(out)
    }

    /// Reject a key's art and drop its saved approval.
    pub fn reject(&mut self, k: &str, reason: &str) -> Result<Value> {
        key(k)?;
        let out = self.registry.reject(k, reason)?;
        self.approvals.retain(|a| a.get("key").and_then(Value::as_str) != Some(k));
        Ok(out)
    }

    /// A worker's candidate for a queued ticket (not an approval).
    pub fn submit(&mut self, k: &str, ticket: &Value, uri: &str, digest: &str) -> Result<Value> {
        key(k)?;
        self.registry.submit(k, ticket, uri, digest)
    }

    /// Queue 1–8 repair patches for a base key, replacing an earlier queue for it. They are
    /// applied by the next [`VisualDirector::direct`] that compiles that key.
    pub fn queue_repair(&mut self, base: &str, patches: &Value) -> Result<Value> {
        let x = obj! {"base"=>base,"patches"=>patches.clone()};
        bounded(&x, 65536)?;
        repair(&x)?;
        let n = c::list(patches)?.len();
        if let Some(old) = self.repairs.iter_mut().find(|o| o.get("base").and_then(Value::as_str) == Some(base)) {
            *old = x;
        } else if self.repairs.len() < MAX_REPAIRS {
            self.repairs.push(x);
        } else {
            return Err(c::bad());
        }
        self.applied.retain(|k| k != base);
        Ok(obj! {"base"=>base,"patches"=>n})
    }
}

/// The `visual-director` local kind: `{config, registry, checkpoint, operations}` with
/// operations `direct{request, missing}`, `approve{key, asset_uri, digest}`,
/// `submit{key, ticket, asset_uri, digest}`, `reject{key, reason}`, `repair{base, patches}`
/// and `checkpoint{}`. Keys and tickets are indexes into earlier `direct` results.
pub fn workflow(a: &Value) -> Result<Value> {
    bounded(a, crate::kit::MAX_REQUEST_BYTES)?;
    c::fields(a, &["config", "registry", "checkpoint", "operations"])?;
    let ops = c::list(get(a, "operations")?)?;
    if ops.len() > 256 {
        return Err(c::bad());
    }
    let cp = get(a, "checkpoint")?;
    let mut d = VisualDirector::new(
        get(a, "config")?,
        RegistryConfig::from_value(get(a, "registry")?)?,
        if cp == &Value::Null { None } else { Some(cp) },
    )?;
    let mut keys: Vec<String> = Vec::new();
    let mut tickets: Vec<Value> = Vec::new();
    let mut results = Vec::new();
    for op in ops {
        let r = (|| -> Result<Value> {
            c::fields(op, &["op", "args"])?;
            let v = get(op, "args")?;
            let at = |field: &str, xs: &[String]| -> Result<String> {
                xs.get(num(v, field)? as usize).cloned().ok_or_else(c::bad)
            };
            match text(op, "op")? {
                "direct" => {
                    c::fields(v, &["request", "missing"])?;
                    let missing = c::list(get(v, "missing")?)?;
                    if missing.len() > 128 {
                        return Err(c::bad());
                    }
                    let mut gone = Vec::new();
                    for u in missing {
                        AssetRef::new(c::text(u)?, &"0".repeat(64))?;
                        gone.push(c::text(u)?.to_owned());
                    }
                    let out = d.direct(get(v, "request")?, |uri, _| !gone.iter().any(|g| g == uri))?;
                    keys.push(text(&out, "key")?.to_owned());
                    tickets.push(match get(&out, "generation")? {
                        Value::Null => Value::Null,
                        g => get(g, "ticket")?.clone(),
                    });
                    Ok(out)
                }
                "approve" => {
                    c::fields(v, &["key", "asset_uri", "digest"])?;
                    d.approve(&at("key", &keys)?, text(v, "asset_uri")?, text(v, "digest")?)
                }
                "submit" => {
                    c::fields(v, &["key", "ticket", "asset_uri", "digest"])?;
                    let ticket = tickets.get(num(v, "ticket")? as usize).cloned().ok_or_else(c::bad)?;
                    d.submit(&at("key", &keys)?, &ticket, text(v, "asset_uri")?, text(v, "digest")?)
                }
                "reject" => {
                    c::fields(v, &["key", "reason"])?;
                    d.reject(&at("key", &keys)?, text(v, "reason")?)
                }
                "repair" => {
                    c::fields(v, &["base", "patches"])?;
                    d.queue_repair(&at("base", &keys)?, get(v, "patches")?)
                }
                "checkpoint" => {
                    c::fields(v, &[])?;
                    Ok(d.checkpoint())
                }
                _ => Err(c::bad()),
            }
        })();
        results.push(match r {
            Ok(x) => obj! {"ok"=>true,"result"=>x},
            Err(_) => obj! {"ok"=>false},
        });
    }
    Ok(obj! {"results"=>results,"checkpoint"=>d.checkpoint(),"actual_model_calls"=>0})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_match_only_the_same_type() {
        let s = Selector::from_value(&crate::data::json::parse(r#"{"fact":"f","when":{"a":[true]}}"#, 1024).unwrap())
            .unwrap();
        assert!(s.holds(&[("a".into(), Scalar::Flag(true))]));
        assert!(!s.holds(&[("a".into(), Scalar::Int(1))]));
        assert!(!s.holds(&[]));
    }
}
