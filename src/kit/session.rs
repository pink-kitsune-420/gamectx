//! Optional bounded persistent character sessions. Existing immutable Project APIs
//! stay unchanged; session operations never execute external game callbacks.
use super::*;
use crate::character::{CharacterTools, CharacterView, TurnPlan};
use crate::request::Composition;
use std::sync::Mutex;
#[derive(Debug)]
pub struct Session {
    project: Project,
    characters: Mutex<Vec<(String, Resident)>>,
}
#[derive(Debug)]
struct Resident {
    character: CharacterTools,
    plan: Option<(Composition, Vec<String>, u32)>,
}
fn planning_state(plan: &Composition) -> Result<Value> {
    let mut state = plan.state();
    let packet = c::get(c::get(&state, "candidate")?, "packet")?;
    let frame = crate::character::frame(packet)?;
    if let Value::Object(fields) = &mut state {
        fields.push(("frame".into(), frame));
    }
    Ok(state)
}
impl Session {
    pub fn new(project: Project) -> Self {
        Self { project, characters: Mutex::new(Vec::new()) }
    }
    pub fn parse(input: &str) -> Result<Self> {
        Ok(Self::new(Project::parse(input)?))
    }
    pub fn request(&self, input: &str) -> Value {
        super::bridge::request(input, |op, args| self.execute(op, args))
    }
    fn execute(&self, op: &str, args: &Value) -> Result<Value> {
        if !op.starts_with("character_") {
            return self.project.execute(op, args);
        }
        let mut slots = self.characters.lock().map_err(|_| bad())?;
        let id = text(args, "id")?;
        c::scope(id)?;
        if op == "character_create" {
            c::fields(args, &["id", "scope", "actor", "participants"])?;
            if slots.len() >= 64 || slots.iter().any(|(old, _)| old == id) {
                return Err(bad());
            }
            let character =
                CharacterTools::new(text(args, "scope")?, text(args, "actor")?, c::get(args, "participants")?)?;
            slots.push((id.into(), Resident { character, plan: None }));
            return Ok(obj! {"created"=>id});
        }
        let index = slots.iter().position(|(old, _)| old == id).ok_or_else(bad)?;
        if op == "character_drop" {
            c::fields(args, &["id"])?;
            slots.remove(index);
            return Ok(obj! {"dropped"=>id});
        }
        let resident = &mut slots[index].1;
        let character = &mut resident.character;
        match op {
            "character_bind" => {
                c::fields(args, &["id", "view"])?;
                character.bind(CharacterView::from_value(c::get(args, "view")?)?)?;
                resident.plan = None;
                Ok(obj! {"bound"=>id})
            }
            "character_enable" => {
                c::fields(args, &["id", "tools"])?;
                let rows = c::list(c::get(args, "tools")?)?;
                let names = rows.iter().map(c::text).collect::<Result<Vec<_>>>()?;
                let schemas = character.enable(&names)?;
                resident.plan = None;
                Ok(schemas)
            }
            "character_routine_configure" => {
                c::fields(args, &["id", "defaults", "overrides", "calendar", "definition"])?;
                let config = crate::routine::Config::resolve(c::get(args, "defaults")?, c::get(args, "overrides")?)?;
                let definition = c::get(args, "definition")?;
                let routine = if definition == &Value::Null {
                    None
                } else {
                    Some(std::sync::Arc::new(crate::routine::Routine::from_value(definition)?))
                };
                character.configure_routine(text(args, "calendar")?, config, routine)?;
                resident.plan = None;
                Ok(obj! {"configured"=>id})
            }
            "character_routine_update" => {
                c::fields(args, &["id", "frame"])?;
                resident.plan = None;
                character.routine_mut()?.update(crate::routine::Frame::from_value(c::get(args, "frame")?)?)?;
                Ok(obj! {"updated"=>id})
            }
            "character_routine_observation" => {
                c::fields(args, &["id", "observation"])?;
                resident.plan = None;
                character.routine_mut()?.observe(c::get(args, "observation")?)?;
                Ok(obj! {"observed"=>id})
            }
            "character_routine_complete" => {
                c::fields(args, &["id", "activity", "start", "revision", "token"])?;
                resident.plan = None;
                character.routine_mut()?.complete(
                    text(args, "activity")?,
                    crate::routine::num(args, "start")?,
                    crate::routine::num(args, "revision")? as u32,
                    crate::routine::num(args, "token")?,
                )?;
                Ok(obj! {"completed"=>id})
            }
            "character_routine_reserve" => {
                c::fields(args, &["id", "revision"])?;
                // Reserving a provider call must not invalidate its counted request.
                character.reserve_routine_call(crate::routine::num(args, "revision")? as u32)?;
                Ok(obj! {"reserved"=>id})
            }
            "character_routine_context" => {
                c::fields(args, &["id", "selection", "revision", "max_bytes"])?;
                let runtime = character.routine().ok_or_else(bad)?;
                if runtime.revision() != Some(crate::routine::num(args, "revision")? as u32) {
                    return Err(bad());
                }
                let result =
                    runtime.context_for(&crate::observation::Selection::from_value(c::get(args, "selection")?)?)?;
                crate::request::bounded(&result, c::uint(c::get(args, "max_bytes")?, 64, 32768)? as usize)?;
                Ok(result)
            }
            "character_routine_wake" => {
                c::fields(args, &["id"])?;
                let runtime = character.routine().ok_or_else(bad)?;
                Ok(
                    obj! {"revision"=>runtime.revision().map_or(Value::Null,Value::from),"at"=>runtime.next_wake().map_or(Value::Null,Value::from)},
                )
            }
            "character_routine_affected" => {
                c::fields(args, &["id", "conditions", "activities"])?;
                let conditions = crate::routine::names(c::get(args, "conditions")?, 128)?;
                let activities = crate::routine::names(c::get(args, "activities")?, 128)?;
                let affected = character.routine().ok_or_else(bad)?.affected(
                    &conditions.iter().map(String::as_str).collect::<Vec<_>>(),
                    &activities.iter().map(String::as_str).collect::<Vec<_>>(),
                )?;
                Ok(obj! {"affected"=>affected})
            }
            "character_routine_save" => {
                c::fields(args, &["id"])?;
                Ok(character.routine().ok_or_else(bad)?.to_value())
            }
            "character_routine_restore" => {
                c::fields(args, &["id", "state", "min_revision", "min_observation", "min_completion", "min_tick"])?;
                resident.plan = None;
                character.routine_mut()?.restore(
                    c::get(args, "state")?,
                    crate::routine::num(args, "min_revision")? as u32,
                    crate::routine::num(args, "min_observation")?,
                    crate::routine::num(args, "min_completion")?,
                    crate::routine::num(args, "min_tick")?,
                )?;
                Ok(obj! {"restored"=>id})
            }
            "character_prepare" => {
                character.disable();
                resident.plan = None;
                c::fields(args, &["id", "plan_id", "identity", "budget", "turn"])?;
                let revision = character.revision().ok_or_else(bad)?;
                let turn = TurnPlan::from_value(c::get(args, "turn")?)?;
                let (sections, tools) = character.sections(&turn)?;
                let rows = sections
                    .into_iter()
                    .map(|s| obj! {"id"=>s.name,"value"=>s.value,"priority"=>s.priority,"required"=>s.required})
                    .collect::<Vec<_>>();
                let composition = Composition::from_value(
                    &obj! {"id"=>c::get(args,"plan_id")?.clone(),"identity"=>c::get(args,"identity")?.clone(),"sections"=>rows,"budget"=>c::get(args,"budget")?.clone()},
                )?;
                let output = planning_state(&composition)?;
                character.disable();
                resident.plan = Some((composition, tools, revision));
                Ok(output)
            }
            "character_observe" => {
                c::fields(args, &["id", "observation"])?;
                let (plan, tools, revision) = resident.plan.as_mut().ok_or_else(bad)?;
                if character.revision() != Some(*revision) {
                    return Err(bad());
                }
                plan.observe(c::get(args, "observation")?)?;
                let output = planning_state(plan)?;
                if output.get("status").and_then(Value::as_str) == Some("ready") {
                    character.select_enabled(c::get(c::get(&output, "candidate")?, "packet")?, tools);
                }
                Ok(output)
            }
            "character_call" => {
                c::fields(args, &["id", "name", "text", "revision", "max_bytes", "allowed"])?;
                let allowed = c::list(c::get(args, "allowed")?)?;
                if allowed.len() > 32 {
                    return Err(bad());
                }
                character.call(
                    text(args, "name")?,
                    text(args, "text")?,
                    c::uint(c::get(args, "revision")?, 0, u32::MAX as u64)? as u32,
                    c::uint(c::get(args, "max_bytes")?, 64, 8192)? as usize,
                    |p| allowed.iter().any(|v| v.same(p)),
                )
            }
            "character_event" => {
                c::fields(args, &["id", "event"])?;
                character.interactions_mut().publish(c::get(args, "event")?)
            }
            "character_progress" => {
                c::fields(args, &["id", "progress"])?;
                character.interactions_mut().set_progress(c::get(args, "progress")?)
            }
            "character_references" => {
                c::fields(args, &["id", "context"])?;
                character.interactions_mut().set_references(c::get(args, "context")?)
            }
            "character_offers" => {
                c::fields(args, &["id", "offers"])?;
                character.interactions_mut().set_offers(c::get(args, "offers")?)
            }
            "character_respond" => {
                c::fields(args, &["id", "recipient", "revision", "confirm", "terms", "allowed"])?;
                let allowed = c::list(c::get(args, "allowed")?)?;
                if allowed.len() > 32 {
                    return Err(bad());
                }
                character.interactions_mut().respond(
                    text(args, "recipient")?,
                    c::uint(c::get(args, "revision")?, 0, u32::MAX as u64)? as u32,
                    c::boolean(c::get(args, "confirm")?)?,
                    c::get(args, "terms")?,
                    |p| allowed.iter().any(|v| v.same(p)),
                )
            }
            "character_clear" => {
                c::fields(args, &["id"])?;
                character.clear();
                resident.plan = None;
                Ok(obj! {"cleared"=>id})
            }
            _ => Err(bad()),
        }
    }
}
