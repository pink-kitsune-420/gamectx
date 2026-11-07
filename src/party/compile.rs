use super::*;
use crate::{
    archetype::Blueprint,
    life::{Life, SpeechTurn},
    lore::Lore,
};
/// Borrowed host-authorized dependencies for one actor and scene revision. Kept separate
/// from floor permissions and tool dispatch. Never put the returned inspection in prompts.
pub struct SceneTurn<'a> {
    pub life: &'a Life,
    pub blueprint: &'a Blueprint,
    pub gates: &'a AttitudeGates,
    pub lore: &'a Lore,
    pub request: &'a Value,
}
impl PartyScene {
    fn validate_turn<'a>(&self, t: &SceneTurn<'a>) -> Result<&Value> {
        let q = t.request;
        bounded(q, 65536)?;
        c::fields(
            q,
            &[
                "trigger",
                "revision",
                "life_revision",
                "listeners",
                "subjects",
                "tags",
                "turn",
                "rules",
                "stable_scene",
                "lore_select",
                "affordances",
                "tools",
            ],
        )?;
        let l = t.life;
        let b = t.blueprint;
        if !b.has("life")
            || text(b.value(), "size")? == "0b"
            || !self.is_ai(&l.actor)
            || !l.identity_matches(text(&self.value, "id")?, &l.actor, &self.people())
            || num(q, "revision")? != num(&self.value, "revision")?
            || num(q, "life_revision")? != l.revision()
        {
            return Err(c::bad());
        }
        let trigger = self.lines.iter().find(|r| r.get("token") == q.get("trigger")).ok_or_else(c::bad)?;
        if !names(get(trigger, "heard")?, 16)?.contains(&l.actor)
            || !names(get(trigger, "recipients")?, 16)?.contains(&l.actor)
        {
            return Err(c::bad());
        }
        let turn = get(q, "turn")?;
        SpeechTurn::from_value(turn)?;
        if turn.get("speaker") != trigger.get("speaker") || turn.get("profile") != b.value().get("attention") {
            return Err(c::bad());
        }
        let cfg = b.life_config()?;
        if l.config.balanced && !cfg.balanced
            || l.config.deception && !cfg.deception
            || l.config.mistaken && !cfg.mistaken
        {
            return Err(c::bad());
        }
        line(text(q, "rules")?)?;
        names(get(q, "tags")?, 8)?;
        Ok(trigger)
    }
    fn subjects(&self, q: &Value) -> Result<Value> {
        let mut subjects = vec![text(get(q, "turn")?, "speaker")?.to_owned()];
        for k in ["listeners", "subjects"] {
            for p in names(get(q, k)?, 16)? {
                if !self.people().contains(&p) {
                    return Err(c::bad());
                }
                if !subjects.contains(&p) {
                    subjects.push(p);
                }
            }
        }
        Ok(subjects.into())
    }
    /// Select gates before caps/ranking and render only actor-heard receipts. A denied
    /// required lore item returns a deterministic deferral with no model request.
    pub fn compile(&self, t: &SceneTurn<'_>) -> Result<Value> {
        let trigger = self.validate_turn(t)?;
        let (q, l, b) = (t.request, t.life, t.blueprint);
        let caps = b.caps();
        let turn = SpeechTurn::from_value(get(q, "turn")?)?;
        let frame = obj! {"revision"=>num(q,"life_revision")?,"speaker"=>get(trigger,"speaker")?.clone(),"listeners"=>get(q,"listeners")?.clone()};
        let selected = l.party_context(
            &obj! {"tags"=>get(q,"tags")?.clone(),"subjects"=>self.subjects(q)?},
            caps,
            &turn,
            t.gates,
            &frame,
        )?;
        let character = get(&selected, "context")?;
        let lore = t.lore.to_value();
        let selected_lore = t.gates.select(l, &frame, "lore", get(&lore, "entries")?)?;
        let select = get(q, "lore_select")?;
        c::fields(select, &["actor", "grants", "query", "subjects", "required", "limit", "max_bytes"])?;
        names(get(select, "grants")?, 64)?;
        names(get(select, "subjects")?, 32)?;
        if text(select, "query")?.len() > 2048 {
            return Err(c::bad());
        }
        c::uint(get(select, "max_bytes")?, 2, 32768)?;
        if text(select, "actor")? != l.actor || num(select, "limit")? as usize > caps.facts {
            return Err(c::bad());
        }
        let required = names(get(select, "required")?, 32)?;
        let required_denied = b.has("lore")
            && c::list(get(&selected_lore, "withheld")?)?
                .iter()
                .any(|w| required.iter().any(|id| w.get("id").and_then(Value::as_str) == Some(id)));
        let affordances = c::list(get(q, "affordances")?)?;
        if affordances.len() > 32 {
            return Err(c::bad());
        }
        for f in affordances {
            c::fields(f, &["id", "text", "audience"])?;
            c::name(text(f, "id")?)?;
            line(text(f, "text")?)?;
            names(get(f, "audience")?, 16)?;
        }
        let visible = affordances
            .iter()
            .filter(|f| {
                names(get(f, "audience").expect("audience"), 16)
                    .is_ok_and(|a| l.participants.iter().all(|p| a.contains(p)))
            })
            .cloned()
            .collect::<Vec<_>>();
        let offered = t.gates.select(l, &frame, "affordance", &visible.into())?;
        let inspection = obj! {"facets_withheld"=>get(&selected,"withheld")?.clone(),"lore_withheld"=>get(&selected_lore,"withheld")?.clone(),"affordances_withheld"=>get(&offered,"withheld")?.clone(),"scene_revision"=>num(&self.value,"revision")?,"life_revision"=>l.revision()};
        let tools = c::list(get(q, "tools")?)?;
        if tools.len() > 2 {
            return Err(c::bad());
        }
        for (i, tool) in tools.iter().enumerate() {
            let name = text(get(tool, "function")?, "name")?;
            if !names(get(b.value(), "tools")?, 2)?.iter().any(|n| n == name)
                || !tool.same(&l.schema(name)?)
                || tools[..i].iter().any(|x| x == tool)
            {
                return Err(c::bad());
            }
        }
        if required_denied {
            return Ok(
                obj! {"request"=>Value::Null,"refusal"=>"I cannot share that or offer it yet.","inspection"=>inspection},
            );
        }
        let cards = if b.has("lore") {
            let filtered = Lore::from_value(
                &obj! {"version"=>get(&lore,"version")?.clone(),"entries"=>get(&selected_lore,"packet")?.clone()},
            )?;
            get(&filtered.select(select)?, "packet")?.clone()
        } else {
            Vec::<Value>::new().into()
        };
        if c::list(&cards)?.len() + c::list(get(get(q, "turn")?, "facts")?)?.len() > caps.facts {
            return Err(c::bad());
        }
        let stable = Value::Object(
            character
                .as_object()
                .ok_or_else(c::bad)?
                .iter()
                .filter(|(k, _)| k == "facets" || k == "policy")
                .cloned()
                .collect(),
        );
        let changing = Value::Object(
            character
                .as_object()
                .ok_or_else(c::bad)?
                .iter()
                .filter(|(k, _)| k != "facets" && k != "policy")
                .cloned()
                .collect(),
        );
        let episodes = if b.has("recall") {
            l.recall_capped(text(trigger, "text")?, get(q, "tags")?, caps.episodes)?
        } else {
            Vec::<Value>::new().into()
        };
        let state = obj! {"character"=>changing,"facts"=>turn.scoped_facts(&l.actor)?,"lore"=>cards,"affordances"=>get(&offered,"packet")?.clone(),"episodes"=>episodes};
        let mut messages = vec![
            obj! {"role"=>"system","content"=>text(q,"rules")?},
            obj! {"role"=>"system","content"=>obj!{"character"=>stable,"scene"=>get(q,"stable_scene")?.clone()}.to_json()},
            obj! {"role"=>"user","content"=>state.to_json()},
        ];
        messages.extend(
            c::list(&self.render_before(&l.actor, caps.exchanges * 2, num(trigger, "token")?)?)?.iter().cloned(),
        );
        messages.push(obj!{"role"=>"system","content"=>format!("Current utterance speaker: {}. First-person I/my refers to that speaker. Question subject: {}.",text(trigger,"speaker")?,text(get(q,"turn")?,"question_subject")?)});
        messages.push(obj!{"role"=>"user","content"=>format!("Current request:\n[{} -> {}] {}",text(trigger,"speaker")?,names(get(trigger,"recipients")?,16)?.join(","),text(trigger,"text")?)});
        let request = obj! {"messages"=>messages,"tools"=>tools.to_vec()};
        bounded(&request, crate::request::MAX_REQUEST_BYTES)?;
        Ok(obj! {"request"=>request,"refusal"=>Value::Null,"inspection"=>inspection})
    }
    /// Gate-aware alternative to direct Life calls in this scene. Registration,
    /// per-turn exposure, current permission and current revisions all remain distinct.
    /// `life_change` only proposes; `life_recall` cannot bypass private facet gates.
    pub fn call_life(
        &self,
        t: &SceneTurn<'_>,
        name: &str,
        input: &str,
        max_bytes: usize,
        mut allowed: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        let compiled = self.compile(t)?;
        if get(&compiled, "request")? == &Value::Null
            || !c::list(get(t.request, "tools")?)?
                .iter()
                .any(|tool| tool.get("function").and_then(|f| f.get("name")).and_then(Value::as_str) == Some(name))
            || !allowed(&obj! {"action"=>name,"target"=>t.life.actor.clone()})
        {
            return Err(c::bad());
        }
        let result = match name {
            "life_change" => t.life.propose(input)?,
            "life_recall" => {
                let a = crate::json::parse(input, 4096)?;
                c::fields(&a, &["tags", "limit"])?;
                let limit = c::uint(get(&a, "limit")?, 0, 16)? as usize;
                let mut caps = t.blueprint.caps();
                caps.facets = caps.facets.min(limit);
                caps.attitudes = caps.attitudes.min(limit);
                let q = t.request;
                let turn = SpeechTurn::from_value(get(q, "turn")?)?;
                let selected=t.life.party_context(&obj!{"tags"=>get(&a,"tags")?.clone(),"subjects"=>self.subjects(q)?},caps,&turn,t.gates,&obj!{"revision"=>num(q,"life_revision")?,"speaker"=>text(get(q,"turn")?,"speaker")?,"listeners"=>get(q,"listeners")?.clone()})?;
                get(&selected, "context")?.clone()
            }
            _ => return Err(c::bad()),
        };
        bounded(&result, max_bytes)?;
        Ok(result)
    }
}
/// Offline parity adapter; host acceptance is simulated from the supplied original draft.
pub fn compile_fixture(a: &Value) -> Result<Value> {
    bounded(a, crate::life::MAX_BYTES)?;
    c::fields(a, &["life", "blueprint", "gates", "scene", "receipts", "lore", "request"])?;
    let l = get(a, "life")?;
    c::fields(l, &["setting", "definition", "defaults", "overrides", "scope", "actor", "participants", "operations"])?;
    let definition = std::sync::Arc::new(crate::life::Definition::draft(
        get(l, "definition")?,
        &crate::life::Setting::from_value(get(l, "setting")?)?,
    )?);
    let mut life = Life::accept(
        text(l, "scope")?,
        text(l, "actor")?,
        get(l, "participants")?,
        definition,
        crate::life::Config::resolve(get(l, "defaults")?, get(l, "overrides")?)?,
    )?;
    let operations = c::list(get(l, "operations")?)?;
    if operations.len() > 128 {
        return Err(c::bad());
    }
    for op in operations {
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
    let mut scene = PartyScene::from_value(get(a, "scene")?)?;
    let receipts = c::list(get(a, "receipts")?)?;
    if receipts.len() > 128 {
        return Err(c::bad());
    }
    for r in receipts {
        scene.receipt(r)?;
    }
    scene.compile(&SceneTurn {
        life: &life,
        blueprint: &Blueprint::from_value(get(a, "blueprint")?)?,
        gates: &AttitudeGates::from_value(get(a, "gates")?)?,
        lore: &Lore::from_value(get(a, "lore")?)?,
        request: get(a, "request")?,
    })
}
