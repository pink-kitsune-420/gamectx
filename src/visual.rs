//! Opt-in deterministic visual specs. Style/state selection and asset approval are
//! separate; this module never invokes a generator, fetches a URI or decodes an image.
use crate::{
    checked as c,
    life::{get, names, num, signed, text},
    obj,
    request::bounded,
    Result, Value,
};
mod digest;
mod registry;
pub use registry::{workflow as assets_workflow, AssetRef, AssetRegistry, RegistryConfig};
pub(crate) fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(fields) => {
            let mut fields = fields.iter().map(|(k, v)| (k.clone(), canonical(v))).collect::<Vec<_>>();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(fields)
        }
        Value::Array(rows) => rows.iter().map(canonical).collect::<Vec<_>>().into(),
        _ => v.clone(),
    }
}
pub(crate) fn hash(v: &Value) -> String {
    digest::sha256(canonical(v).to_json().as_bytes())
}
/// Digest owned image bytes without loading a URI; bounded to 16 MiB. Approval still
/// requires host verification of dimensions, provenance and actual visual quality.
pub fn content_digest(bytes: &[u8]) -> Result<String> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(c::bad());
    }
    Ok(digest::sha256(bytes))
}
pub(crate) fn token(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 512
        || s.chars().count() > 128
        || s.trim_matches(' ') != s
        || s.chars().any(char::is_control)
    {
        return Err(c::bad());
    }
    Ok(())
}
pub(crate) fn tokens(v: &Value, cap: usize) -> Result<Vec<String>> {
    let rows = c::list(v)?;
    if rows.len() > cap {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for x in rows {
        let s = c::text(x)?;
        token(s)?;
        if out.iter().any(|a| a == s) {
            return Err(c::bad());
        }
        out.push(s.into());
    }
    out.sort();
    Ok(out)
}
pub(crate) fn set(v: &mut Value, k: &str, x: Value) {
    crate::archetype::set_field(v, k, x);
}
#[derive(Clone, Debug)]
pub struct ArtBible {
    value: Value,
    digest: String,
}
impl ArtBible {
    pub fn parse(s: &str) -> Result<Self> {
        Self::from_value(&crate::json::parse(s, 65536)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 65536)?;
        c::fields(
            v,
            &[
                "version",
                "id",
                "revision",
                "style",
                "format",
                "width",
                "height",
                "aspect",
                "camera_angle",
                "framing",
                "palette",
                "negative_tokens",
                "anchors",
                "expressions",
                "poses",
                "facts",
                "max_fact_age",
            ],
        )?;
        c::uint(get(v, "version")?, 1, 1)?;
        c::name(text(v, "id")?)?;
        c::uint(get(v, "revision")?, 1, u32::MAX as u64)?;
        for k in ["style", "camera_angle", "framing"] {
            token(text(v, k)?)?;
        }
        if !matches!(text(v, "format")?, "portrait" | "sprite" | "scene") {
            return Err(c::bad());
        }
        let w = c::uint(get(v, "width")?, 16, 4096)?;
        let h = c::uint(get(v, "height")?, 16, 4096)?;
        let aspect = get(v, "aspect")?;
        c::fields(aspect, &["width", "height"])?;
        if w * c::uint(get(aspect, "height")?, 1, 256)? != h * c::uint(get(aspect, "width")?, 1, 256)? {
            return Err(c::bad());
        }
        let palette = tokens(get(v, "palette")?, 16)?;
        if palette.is_empty() {
            return Err(c::bad());
        }
        tokens(get(v, "negative_tokens")?, 32)?;
        c::uint(get(v, "max_fact_age")?, 0, 1000)?;
        let anchors = c::list(get(v, "anchors")?)?;
        if anchors.is_empty() || anchors.len() > 64 {
            return Err(c::bad());
        }
        for (i, a) in anchors.iter().enumerate() {
            c::fields(a, &["entity", "kind", "silhouette", "attire", "props", "seed"])?;
            c::scope(text(a, "entity")?)?;
            if !matches!(text(a, "kind")?, "character" | "scene")
                || anchors[..i].iter().any(|o| o.get("entity") == a.get("entity"))
            {
                return Err(c::bad());
            }
            token(text(a, "silhouette")?)?;
            token(text(a, "attire")?)?;
            tokens(get(a, "props")?, 16)?;
            num(a, "seed")?;
        }
        for (key, cap, id) in [("expressions", 32, "id"), ("poses", 32, "activity"), ("facts", 64, "id")] {
            let rows = c::list(get(v, key)?)?;
            if rows.len() > cap {
                return Err(c::bad());
            }
            for (i, row) in rows.iter().enumerate() {
                if rows[..i].iter().any(|o| o.get(id) == row.get(id)) {
                    return Err(c::bad());
                }
                c::scope(text(row, id)?)?;
                match key {
                    "expressions" => {
                        c::fields(row, &["id", "slot", "kind", "subject", "min", "max", "priority", "tokens"])?;
                        for k in ["id", "slot", "kind"] {
                            c::name(text(row, k)?)?;
                        }
                        if !matches!(text(row, "subject")?, "listener" | "self")
                            || signed(get(row, "min")?, 100)? > signed(get(row, "max")?, 100)?
                        {
                            return Err(c::bad());
                        }
                        signed(get(row, "priority")?, 100)?;
                    }
                    "poses" => {
                        c::fields(row, &["activity", "tokens"])?;
                    }
                    _ => {
                        c::fields(row, &["id", "group", "priority", "tokens", "props"])?;
                        if get(row, "group")? != &Value::Null {
                            c::name(text(row, "group")?)?;
                        }
                        signed(get(row, "priority")?, 100)?;
                        for prop in tokens(get(row, "props")?, 8)? {
                            if !anchors
                                .iter()
                                .any(|a| tokens(get(a, "props").expect("props"), 16).expect("props").contains(&prop))
                            {
                                return Err(c::bad());
                            }
                        }
                    }
                }
                tokens(get(row, "tokens")?, 8)?;
            }
        }
        let mut value = v.clone();
        set(&mut value, "palette", palette.into());
        set(&mut value, "negative_tokens", tokens(get(v, "negative_tokens")?, 32)?.into());
        for (k, id) in [("anchors", "entity"), ("expressions", "id"), ("poses", "activity"), ("facts", "id")] {
            let mut rows = c::list(get(v, k)?)?.to_vec();
            rows.sort_by(|a, b| a.get(id).and_then(Value::as_str).cmp(&b.get(id).and_then(Value::as_str)));
            for row in &mut rows {
                for key in ["props", "tokens"] {
                    if let Some(xs) = row.get(key) {
                        let sorted = tokens(xs, 32)?;
                        set(row, key, sorted.into());
                    }
                }
            }
            set(&mut value, k, rows.into());
        }
        let value = canonical(&value);
        Ok(Self { digest: hash(&value), value })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}
#[derive(Clone, Debug)]
pub struct Activity {
    value: Option<Value>,
}
impl Activity {
    pub fn idle() -> Self {
        Self { value: None }
    }
    /// Privileged host snapshot. Never expose this constructor as a model tool.
    pub fn from_value(v: &Value) -> Result<Self> {
        if v == &Value::Null {
            return Ok(Self::idle());
        }
        bounded(v, 4096)?;
        c::fields(v, &["scope", "actor", "revision", "source", "state", "id"])?;
        for k in ["scope", "actor", "id"] {
            c::scope(text(v, k)?)?;
        }
        num(v, "revision")?;
        if !matches!(text(v, "source")?, "routine" | "initiative") || text(v, "state")? != "active" {
            return Err(c::bad());
        }
        Ok(Self { value: Some(v.clone()) })
    }
    pub fn from_routine(r: &crate::routine::Runtime) -> Result<Self> {
        let state = r.to_value();
        let context = r.context()?;
        let current = get(&context, "current")?;
        if current == &Value::Null {
            return Ok(Self::idle());
        }
        Self::from_value(
            &obj! {"scope"=>get(&state,"scope")?.clone(),"actor"=>get(&state,"actor")?.clone(),"revision"=>get(&context,"revision")?.clone(),"source"=>"routine","state"=>get(current,"state")?.clone(),"id"=>get(current,"id")?.clone()},
        )
    }
    /// A ready proposal is still insufficient: the host supplies a matching started
    /// receipt after execution begins. This function never executes or confirms it.
    pub fn from_initiative(scope: &str, actor: &str, ready: &Value, receipt: &Value) -> Result<Self> {
        bounded(ready, 16384)?;
        bounded(receipt, 4096)?;
        c::fields(ready, &["status", "proposal"])?;
        c::fields(receipt, &["actor", "proposal", "revision", "state"])?;
        let p = get(ready, "proposal")?;
        let candidate = get(p, "candidate")?;
        if text(ready, "status")? != "ready"
            || text(receipt, "state")? != "started"
            || text(receipt, "actor")? != actor
            || receipt.get("proposal") != p.get("id")
            || receipt.get("revision") != p.get("revision")
            || text(candidate, "kind")? != "activity"
        {
            return Err(c::bad());
        }
        Self::from_value(
            &obj! {"scope"=>scope,"actor"=>actor,"revision"=>get(p,"revision")?.clone(),"source"=>"initiative","state"=>"active","id"=>get(candidate,"id")?.clone()},
        )
    }
    pub fn to_value(&self) -> Value {
        self.value.clone().unwrap_or(Value::Null)
    }
}
#[derive(Clone, Debug)]
pub struct VisualSpec {
    pub(crate) data: Value,
    pub(crate) inspection: Value,
    key: String,
}
impl VisualSpec {
    pub(crate) fn new(data: Value, inspection: Value) -> Result<Self> {
        bounded(&data, 32768)?;
        bounded(&inspection, 65536)?;
        let data = canonical(&data);
        Ok(Self { key: format!("visual:{}", hash(&data)), data, inspection })
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn to_value(&self) -> Value {
        obj! {"key"=>self.key.clone(),"spec"=>self.data.clone(),"inspection"=>self.inspection.clone()}
    }
    pub fn render(&self) -> Result<Value> {
        let d = &self.data;
        let mut positive = vec![
            text(d, "style")?.to_owned(),
            text(d, "format")?.into(),
            text(d, "camera_angle")?.into(),
            text(d, "framing")?.into(),
            text(d, "silhouette")?.into(),
            text(d, "attire")?.into(),
        ];
        for k in ["expression", "pose"] {
            positive.extend(tokens(get(get(d, k)?, "tokens")?, 64)?);
        }
        for cue in c::list(get(d, "environment")?)? {
            positive.extend(tokens(get(cue, "tokens")?, 64)?);
        }
        positive.extend(tokens(get(d, "visible_props")?, 16)?);
        positive.extend(tokens(get(d, "extra_tokens")?, 64)?);
        Ok(
            obj! {"positive_tokens"=>positive,"negative_tokens"=>get(d,"negative_tokens")?.clone(),"palette"=>get(d,"palette")?.clone(),"width"=>get(d,"width")?.clone(),"height"=>get(d,"height")?.clone(),"seed"=>get(d,"seed")?.clone()},
        )
    }
    pub fn compile(b: &ArtBible, f: &Value, life: Option<&crate::life::Life>, activity: &Activity) -> Result<Self> {
        bounded(f, 65536)?;
        c::fields(
            f,
            &["scope", "observer", "entity", "listener", "scene", "audience", "life_revision", "activity_revision"],
        )?;
        for k in ["scope", "observer", "entity", "listener"] {
            c::scope(text(f, k)?)?;
        }
        let audience = tokens(get(f, "audience")?, 16)?;
        if audience.is_empty() {
            return Err(c::bad());
        }
        let scene = get(f, "scene")?;
        let people = names(get(scene, "participants")?, 16)?;
        if !people.iter().any(|p| p == text(f, "observer").unwrap_or(""))
            || !people.iter().any(|p| p == text(f, "listener").unwrap_or(""))
            || audience.iter().any(|p| !people.contains(p))
        {
            return Err(c::bad());
        }
        let mut validator =
            crate::life::Coordinator::new(text(f, "scope")?, text(f, "observer")?, get(scene, "participants")?)?;
        validator.bind_scene(scene)?;
        let anchor = c::list(get(&b.value, "anchors")?)?
            .iter()
            .find(|a| a.get("entity") == f.get("entity"))
            .ok_or_else(c::bad)?;
        let is_scene = text(&b.value, "format")? == "scene";
        if text(anchor, "kind")? != if is_scene { "scene" } else { "character" }
            || is_scene && f.get("entity") != scene.get("id")
        {
            return Err(c::bad());
        }
        let mut expression = obj! {"slot"=>"neutral","tokens"=>vec!["neutral expression"]};
        let mut expr_rule = Value::Null;
        let mut level = Value::Null;
        let mut life_rev = Value::Null;
        let mut attitude_diagnostics = Vec::<Value>::new();
        if let Some(l) = life {
            if !l.config.enabled
                || !l.identity_matches(text(f, "scope")?, text(f, "entity")?, &people)
                || get(f, "life_revision")? != &Value::from(l.revision())
            {
                return Err(c::bad());
            }
            life_rev = l.revision().into();
            for a in &l.attitudes {
                let d = get(a, "definition")?;
                if [text(f, "listener")?, text(f, "entity")?].contains(&text(d, "subject")?)
                    && audience
                        .iter()
                        .all(|p| names(get(d, "audience").expect("audience"), 16).expect("audience").contains(p))
                {
                    attitude_diagnostics.push(obj!{"id"=>get(d,"id")?.clone(),"kind"=>get(d,"kind")?.clone(),"subject"=>get(d,"subject")?.clone(),"level"=>get(a,"level")?.clone()});
                }
            }
            attitude_diagnostics.sort_by_key(|a| text(a, "id").expect("id").to_owned());
            let mut choices = Vec::new();
            for rule in c::list(get(&b.value, "expressions")?)? {
                let subject =
                    if text(rule, "subject")? == "listener" { text(f, "listener")? } else { text(f, "entity")? };
                let attrs = l
                    .attitudes
                    .iter()
                    .filter(|a| {
                        let d = get(a, "definition").expect("definition");
                        d.get("kind") == rule.get("kind")
                            && d.get("subject").and_then(Value::as_str) == Some(subject)
                            && names(get(d, "audience").expect("audience"), 16)
                                .is_ok_and(|xs| audience.iter().all(|p| xs.contains(p)))
                    })
                    .collect::<Vec<_>>();
                if attrs.len() == 1 {
                    let n = signed(get(attrs[0], "level")?, 100)?;
                    if signed(get(rule, "min")?, 100)? <= n && n <= signed(get(rule, "max")?, 100)? {
                        choices.push((rule, n));
                    }
                }
            }
            choices.sort_by_key(|(r, _)| {
                (
                    std::cmp::Reverse(signed(get(r, "priority").expect("priority"), 100).expect("priority")),
                    text(r, "id").expect("id"),
                )
            });
            if let Some((r, n)) = choices.first() {
                expression = obj! {"slot"=>get(r,"slot")?.clone(),"tokens"=>get(r,"tokens")?.clone()};
                expr_rule = get(r, "id")?.clone();
                level = (*n).into();
            }
        } else if get(f, "life_revision")? != &Value::Null {
            return Err(c::bad());
        }
        let mut pose = obj! {"id"=>"idle","tokens"=>vec!["at rest"]};
        let mut pose_source = Value::Null;
        if let Some(a) = &activity.value {
            if a.get("scope") != f.get("scope")
                || a.get("actor") != f.get("entity")
                || a.get("revision") != f.get("activity_revision")
            {
                return Err(c::bad());
            }
            if let Some(r) = c::list(get(&b.value, "poses")?)?.iter().find(|r| r.get("activity") == a.get("id")) {
                pose = obj! {"id"=>get(a,"id")?.clone(),"tokens"=>get(r,"tokens")?.clone()};
                pose_source = get(a, "source")?.clone();
            }
        } else if get(f, "activity_revision")? != &Value::Null {
            return Err(c::bad());
        }
        let mut eligible = Vec::new();
        let mut omitted = Vec::new();
        for fact in c::list(get(scene, "facts")?)? {
            if num(fact, "revision")? > num(scene, "revision")? {
                return Err(c::bad());
            }
            let reason = if text(fact, "kind")? != "perceived" {
                Some("not_perceived")
            } else if text(get(fact, "source")?, "speaker")? != text(f, "observer")? {
                Some("foreign_observer")
            } else if audience
                .iter()
                .any(|p| !names(get(fact, "audience").expect("audience"), 16).expect("audience").contains(p))
            {
                Some("audience")
            } else if num(scene, "at")? - num(fact, "observed_at")? > num(&b.value, "max_fact_age")? {
                Some("aged")
            } else {
                None
            };
            let rule = c::list(get(&b.value, "facts")?)?.iter().find(|r| r.get("id") == fact.get("id"));
            let reason = reason.or(if rule.is_none() { Some("unmapped") } else { None }).or(if let Some(r) = rule {
                if tokens(get(r, "props")?, 8)?
                    .iter()
                    .any(|p| !tokens(get(anchor, "props").expect("props"), 16).expect("props").contains(p))
                {
                    Some("foreign_prop")
                } else {
                    None
                }
            } else {
                None
            });
            if let Some(reason) = reason {
                omitted.push(obj! {"id"=>get(fact,"id")?.clone(),"reason"=>reason});
            } else {
                eligible.push(rule.expect("mapped"));
            }
        }
        eligible.sort_by_key(|r| {
            (
                std::cmp::Reverse(signed(get(r, "priority").expect("priority"), 100).expect("priority")),
                text(r, "id").expect("id"),
            )
        });
        let mut selected = Vec::new();
        let mut groups = Vec::new();
        for r in eligible {
            if get(r, "group")? != &Value::Null && groups.contains(get(r, "group")?) {
                omitted.push(obj! {"id"=>get(r,"id")?.clone(),"reason"=>"occluded"});
            } else {
                selected.push(r);
                if get(r, "group")? != &Value::Null {
                    groups.push(get(r, "group")?.clone());
                }
            }
        }
        let mut props = selected
            .iter()
            .flat_map(|r| tokens(get(r, "props").expect("props"), 8).expect("props"))
            .collect::<Vec<_>>();
        props.sort();
        props.dedup();
        let mut environment=selected.iter().map(|r|obj!{"tokens"=>get(r,"tokens").expect("tokens").clone(),"props"=>get(r,"props").expect("props").clone()}).collect::<Vec<_>>();
        environment = environment.iter().map(canonical).collect();
        environment.sort_by_key(Value::to_json);
        let seed=u32::from_str_radix(&hash(&obj!{"bible"=>get(&b.value,"id")?.clone(),"entity"=>get(f,"entity")?.clone(),"seed"=>get(anchor,"seed")?.clone()})[..8],16).expect("hash");
        let data = obj! {"version"=>1,"bible_id"=>get(&b.value,"id")?.clone(),"art_bible_revision"=>get(&b.value,"revision")?.clone(),"bible_digest"=>b.digest.clone(),"scope"=>get(f,"scope")?.clone(),"observer"=>get(f,"observer")?.clone(),"audience"=>audience,"entity"=>get(f,"entity")?.clone(),"scene"=>get(scene,"id")?.clone(),"scene_epoch"=>get(scene,"epoch")?.clone(),"style"=>get(&b.value,"style")?.clone(),"format"=>get(&b.value,"format")?.clone(),"width"=>get(&b.value,"width")?.clone(),"height"=>get(&b.value,"height")?.clone(),"aspect"=>get(&b.value,"aspect")?.clone(),"camera_angle"=>get(&b.value,"camera_angle")?.clone(),"framing"=>get(&b.value,"framing")?.clone(),"palette"=>get(&b.value,"palette")?.clone(),"silhouette"=>get(anchor,"silhouette")?.clone(),"attire"=>get(anchor,"attire")?.clone(),"expression"=>expression,"pose"=>pose,"visible_props"=>props,"environment"=>environment,"seed"=>seed,"negative_tokens"=>get(&b.value,"negative_tokens")?.clone(),"extra_tokens"=>Vec::<Value>::new(),"forbidden_props"=>Vec::<Value>::new()};
        let mut ids = selected.iter().map(|r| get(r, "id").expect("id").clone()).collect::<Vec<_>>();
        ids.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        omitted.sort_by(|a, b| a.get("id").and_then(Value::as_str).cmp(&b.get("id").and_then(Value::as_str)));
        Self::new(
            data,
            obj! {"state"=>obj!{"scene_revision"=>num(scene,"revision")?,"life_revision"=>life_rev,"attitude_level"=>level,"attitudes"=>attitude_diagnostics,"activity"=>activity.to_value(),"expression_rule"=>expr_rule,"pose_source"=>pose_source,"activity_revision"=>get(f,"activity_revision")?.clone(),"selected_facts"=>ids,"omitted"=>omitted,"bible_rules"=>obj!{"id"=>get(&b.value,"id")?.clone(),"revision"=>num(&b.value,"revision")?,"digest"=>b.digest.clone()}},"repairs"=>Vec::<Value>::new()},
        )
    }
}
pub fn compile_fixture(a: &Value) -> Result<VisualSpec> {
    bounded(a, crate::kit::MAX_REQUEST_BYTES)?;
    c::fields(a, &["bible", "frame", "life", "activity"])?;
    let bible = ArtBible::from_value(get(a, "bible")?)?;
    let activity = Activity::from_value(get(a, "activity")?)?;
    let life = if get(a, "life")? == &Value::Null {
        None
    } else {
        let l = get(a, "life")?;
        c::fields(
            l,
            &["setting", "definition", "defaults", "overrides", "scope", "actor", "participants", "operations"],
        )?;
        let d = std::sync::Arc::new(crate::life::Definition::draft(
            get(l, "definition")?,
            &crate::life::Setting::from_value(get(l, "setting")?)?,
        )?);
        let mut life = crate::life::Life::accept(
            text(l, "scope")?,
            text(l, "actor")?,
            get(l, "participants")?,
            d,
            crate::life::Config::resolve(get(l, "defaults")?, get(l, "overrides")?)?,
        )?;
        let ops = c::list(get(l, "operations")?)?;
        if ops.len() > 128 {
            return Err(c::bad());
        }
        for op in ops {
            c::fields(op, &["op", "args"])?;
            let v = get(op, "args")?;
            match text(op, "op")? {
                "experience" => life.experience(v)?,
                "appraise" => {
                    c::fields(v, &["rule", "event", "revision"])?;
                    life.appraise(text(v, "rule")?, num(v, "event")?, num(v, "revision")?)?;
                }
                _ => return Err(c::bad()),
            }
        }
        Some(life)
    };
    VisualSpec::compile(&bible, get(a, "frame")?, life.as_ref(), &activity)
}
