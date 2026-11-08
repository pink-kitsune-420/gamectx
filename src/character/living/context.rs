use super::*;
/// Additive host/bridge compiler adapter. Failed replay operations reject compilation;
/// diagnostic inspection remains outside the model request.
pub fn compile_fixture(a: &Value) -> Result<Value> {
    bounded(a, crate::kit::MAX_REQUEST_BYTES)?;
    c::fields(a, &["world", "scene", "receipts", "actor", "request", "selection"])?;
    let (w, results) = super::replay(get(a, "world")?)?;
    if results.iter().any(|r| r.get("ok") != Some(&true.into())) {
        return Err(c::bad());
    }
    let mut scene = crate::character::party::PartyScene::from_value(get(a, "scene")?)?;
    let receipts = c::list(get(a, "receipts")?)?;
    if receipts.len() > 128 {
        return Err(c::bad());
    }
    for r in receipts {
        scene.receipt(r)?;
    }
    w.compile(&scene, text(a, "actor")?, get(a, "request")?, get(a, "selection")?, None)
}
impl LivingWorld {
    /// Gate-aware Life dispatch against the same effective social view as compilation.
    /// Host permission is rechecked; model changes still remain unaccepted proposals.
    pub fn call_life(
        &self,
        scene: &crate::character::party::PartyScene,
        actor: &str,
        request: &Value,
        call: &Value,
        allowed: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        bounded(call, 8192)?;
        c::fields(call, &["name", "input", "max_bytes", "route"])?;
        let i = self.index(actor)?;
        let ch = &self.pack.characters[i];
        let blueprint = if get(call, "route")? == &Value::Null {
            ch.blueprint.clone()
        } else {
            ch.blueprint.for_route(&crate::character::party::ModelRoute::from_value(get(call, "route")?)?)?
        };
        let life = self.projected(i);
        let turn = crate::character::party::SceneTurn {
            life: &life,
            blueprint: &blueprint,
            gates: &ch.gates,
            lore: &self.pack.lore,
            request,
        };
        scene.call_life(
            &turn,
            text(call, "name")?,
            text(call, "input")?,
            c::uint(get(call, "max_bytes")?, 1, 16384)? as usize,
            allowed,
        )
    }
}
impl LivingWorld {
    /// Monotonic absolute counters confirmed by the host. Counter receipts are not
    /// model claims or inferred personality. Exact retry is idempotent; rollback rejects.
    pub fn update_counters(&mut self, v: &Value) -> Result<Value> {
        bounded(v, 8192)?;
        c::fields(v, &["token", "player", "at", "fulfilled", "broken", "interrupted", "source"])?;
        c::uint(get(v, "token")?, 1, u32::MAX as u64)?;
        num(v, "at")?;
        for k in ["fulfilled", "broken", "interrupted"] {
            c::uint(get(v, k)?, 0, 1_000_000)?;
        }
        crate::knowledge::info::sources(&vec![get(v, "source")?.clone()].into())?;
        if self.last_counter.as_ref().is_some_and(|last| last.same(v)) {
            return Ok(obj! {"duplicate"=>true,"revision"=>self.revision});
        }
        self.can_bump()?;
        if num(v, "token")? <= self.counter_water
            || num(v, "at")? < self.clock
            || !names(get(&self.pack.value, "players")?, 8)?
                .iter()
                .any(|p| Some(p.as_str()) == v.get("player").and_then(Value::as_str))
        {
            return Err(c::bad());
        }
        let previous = self.counters.iter().position(|x| x.get("player") == v.get("player"));
        let mut changed = false;
        for k in ["fulfilled", "broken", "interrupted"] {
            let old = previous.map_or(0, |i| num(&self.counters[i], k).expect("counter"));
            let n = c::uint(get(v, k)?, old, 1_000_000)?;
            changed |= n != old;
        }
        if !changed {
            return Err(c::bad());
        }
        if let Some(i) = previous {
            self.counters[i] = v.clone();
        } else {
            self.counters.push(v.clone());
        }
        self.last_counter = Some(v.clone());
        self.counter_water = num(v, "token")?;
        self.clock = num(v, "at")?;
        self.revision += 1;
        Ok(obj! {"duplicate"=>false,"revision"=>self.revision})
    }
    pub fn set_atmosphere(&mut self, v: &Value) -> Result<Value> {
        bounded(v, 8192)?;
        c::fields(v, &["location", "revision", "tag", "source"])?;
        self.can_bump()?;
        if !self.pack.locations.iter().any(|p| Some(p.as_str()) == v.get("location").and_then(Value::as_str))
            || !ATMOSPHERE.contains(&text(v, "tag")?)
        {
            return Err(c::bad());
        }
        c::uint(get(v, "revision")?, 1, u32::MAX as u64)?;
        crate::knowledge::info::sources(&vec![get(v, "source")?.clone()].into())?;
        let index = self.atmospheres.iter().position(|x| x.get("location") == v.get("location"));
        if let Some(i) = index {
            if num(v, "revision")? <= num(&self.atmospheres[i], "revision")? {
                return Err(c::bad());
            }
            self.atmospheres[i] = v.clone();
        } else {
            self.atmospheres.push(v.clone());
        }
        self.revision += 1;
        Ok(obj! {"revision"=>self.revision})
    }
    /// Select only explicitly relevant players/tags/current location. Upper bounds are
    /// independent of model availability; the compiler further caps focused turns at 2.
    pub fn context(&self, a: &Value) -> Result<Value> {
        bounded(a, 8192)?;
        c::fields(a, &["actor", "players", "tags", "location", "limit"])?;
        let actor = self.index(text(a, "actor")?)?;
        let players = names(get(a, "players")?, 8)?;
        let tags = names(get(a, "tags")?, 8)?;
        let limit = c::uint(get(a, "limit")?, 0, 4)? as usize;
        if players
            .iter()
            .any(|p| !names(get(&self.pack.value, "players").expect("players"), 8).expect("players").contains(p))
            || !self.pack.locations.iter().any(|p| Some(p.as_str()) == a.get("location").and_then(Value::as_str))
        {
            return Err(c::bad());
        }
        let mut badges = Vec::new();
        if self.enabled("habits") {
            for player in &players {
                for h in c::list(get(&self.pack.value, "habits")?)? {
                    let Some(counter) =
                        self.counters.iter().find(|x| x.get("player").and_then(Value::as_str) == Some(player))
                    else {
                        continue;
                    };
                    let count = num(counter, text(h, "counter")?)?;
                    if badges.len() < limit
                        && count >= num(h, "threshold")?
                        && (tags.is_empty() || names(get(h, "tags")?, 8)?.iter().any(|t| tags.contains(t)))
                    {
                        badges.push(obj!{"player"=>player.clone(),"id"=>get(h,"id")?.clone(),"text"=>get(h,"text")?.clone(),"counter"=>get(h,"counter")?.clone(),"count"=>count,"source"=>get(counter,"source")?.clone()});
                    }
                }
            }
        }
        let atmosphere = if self.enabled("atmosphere") {
            let v = self.atmospheres.iter().find(|v| v.get("location") == a.get("location"));
            let tag = v.map_or("calm", |v| text(v, "tag").expect("tag"));
            obj! {"location"=>get(a,"location")?.clone(),"tag"=>tag,"urgency"=>ATMOSPHERE.iter().position(|t|*t==tag).expect("tag"),"source"=>v.map_or(Value::Null,|v|get(v,"source").expect("source").clone())}
        } else {
            Value::Null
        };
        let mut rumors = Vec::new();
        for k in &self.known {
            if rumors.len() >= limit {
                break;
            }
            let root = &self.roots[k.root];
            let event = get(root, "event")?;
            if k.actor == actor
                && k.path.len() > 1
                && players.iter().any(|p| Some(p.as_str()) == event.get("subject").and_then(Value::as_str))
                && (tags.is_empty()
                    || tags.iter().any(|t| Some(t.as_str()) == event.get("tag").and_then(Value::as_str)))
            {
                rumors.push(obj!{"root"=>get(root,"id")?.clone(),"path"=>k.path.clone(),"hops"=>k.path.len()-1,"original_source"=>get(event,"source")?.clone()});
            }
        }
        let influences = self
            .influences
            .iter()
            .filter(|x| {
                x.get("actor").and_then(Value::as_str) == Some(text(a, "actor").expect("actor"))
                    && players.iter().any(|p| Some(p.as_str()) == x.get("subject").and_then(Value::as_str))
            })
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        Ok(obj! {"badges"=>badges,"atmosphere"=>atmosphere,"rumors"=>rumors,"group_influences"=>influences})
    }
    /// Host-only provenance ledger, separate from bounded turn context and legacy Life
    /// checkpoints. Retain/replay this layer's host operations with the base state.
    pub fn rumor_ledger(&self) -> Value {
        self.known.iter().map(|k|obj!{"root"=>self.roots[k.root].get("id").expect("id").clone(),"actor"=>self.pack.characters[k.actor].id.clone(),"path"=>k.path.clone(),"hops"=>k.path.len()-1,"event"=>k.event,"original_source"=>self.roots[k.root].get("event").expect("event").get("source").expect("source").clone(),"original_observer"=>self.roots[k.root].get("actor").expect("actor").clone()}).collect::<Vec<_>>().into()
    }
    pub fn root_ledger(&self) -> Value {
        self.roots.clone().into()
    }
    pub fn influence_ledger(&self) -> Value {
        self.influences.clone().into()
    }
    /// Add living context after the stable prefix. Effective group influence participates
    /// in ordinary party attitude gates. Legacy party/compiler outputs remain unchanged.
    pub fn compile(
        &self,
        scene: &crate::character::party::PartyScene,
        actor: &str,
        request: &Value,
        selection: &Value,
        route: Option<&crate::character::party::ModelRoute>,
    ) -> Result<Value> {
        let i = self.index(actor)?;
        let character = &self.pack.characters[i];
        let blueprint =
            if let Some(r) = route { character.blueprint.for_route(r)? } else { character.blueprint.clone() };
        let life = self.projected(i);
        let mut selected = selection.clone();
        if text(selection, "actor")? != actor {
            return Err(c::bad());
        }
        let cap = if blueprint.caps().exchanges <= 1 { 2 } else { 4 };
        let requested = c::uint(get(selection, "limit")?, 0, 4)? as usize;
        crate::character::archetype::set_field(&mut selected, "limit", requested.min(cap).into());
        let living = self.context(&selected)?;
        let mut result = scene.compile(&crate::character::party::SceneTurn {
            life: &life,
            blueprint: &blueprint,
            gates: &character.gates,
            lore: &self.pack.lore,
            request,
        })?;
        if let Value::Object(fields) = &mut result {
            if let Some((_, Value::Object(r))) = fields.iter_mut().find(|(k, _)| k == "request") {
                if let Some((_, Value::Array(messages))) = r.iter_mut().find(|(k, _)| k == "messages") {
                    messages.insert(3, obj! {"role"=>"user","content"=>obj!{"living"=>living}.to_json()});
                }
            }
        }
        bounded(&result, crate::packet::request::MAX_REQUEST_BYTES)?;
        Ok(result)
    }
}
