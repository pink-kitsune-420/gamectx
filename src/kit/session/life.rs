//! Privileged persistent host operations. These names are never model tool schemas.
use super::*;
use crate::life::{Config, ContextCaps, Coordinator, Definition, Initiative, InitiativePolicy, Life, Setting};
use std::sync::Arc;
fn num(v: &Value, k: &str) -> Result<u64> {
    crate::life::num(v, k)
}
fn invalidate(r: &mut Resident) -> Result<()> {
    r.plan = None;
    r.character.disable();
    if let Some(c) = &mut r.coordinator {
        c.invalidate()?;
    }
    Ok(())
}
pub(super) fn execute(r: &mut Resident, op: &str, a: &Value) -> Result<Value> {
    match op {
        "character_life_configure" => {
            c::fields(a, &["id", "setting", "definition", "defaults", "overrides"])?;
            if r.character.life().is_some() {
                return Err(bad());
            }
            let d =
                Arc::new(Definition::draft(c::get(a, "definition")?, &Setting::from_value(c::get(a, "setting")?)?)?);
            let (scope, actor, p) = r.character.interactions_mut().identity();
            let life = Life::accept(
                scope,
                actor,
                &Value::strings(p),
                d,
                Config::resolve(c::get(a, "defaults")?, c::get(a, "overrides")?)?,
            )?;
            r.character.with_life(life)?;
            r.plan = None;
            Ok(obj! {"configured"=>text(a,"id")?})
        }
        "character_life_experience" => {
            c::fields(a, &["id", "event"])?;
            invalidate(r)?;
            let mut event = c::get(a, "event")?.clone();
            if !r.retention {
                crate::life::set_value(&mut event, "retain", Value::Bool(false));
            }
            r.character.life_mut()?.experience(&event)?;
            Ok(obj! {"revision"=>r.character.life().unwrap().revision()})
        }
        "character_life_appraise" => {
            c::fields(a, &["id", "rule", "event", "revision"])?;
            invalidate(r)?;
            r.character.life_mut()?.appraise(text(a, "rule")?, num(a, "event")?, num(a, "revision")?)
        }
        "character_life_compile_speech" => {
            c::fields(a, &["id", "request"])?;
            r.character.life().ok_or_else(bad)?.compile_speech(c::get(a, "request")?)
        }
        "character_life_context" => {
            c::fields(a, &["id", "tags", "caps"])?;
            r.character
                .life()
                .ok_or_else(bad)?
                .context_capped(c::get(a, "tags")?, ContextCaps::from_value(c::get(a, "caps")?)?)
        }
        "character_life_recall" => {
            c::fields(a, &["id", "input", "tags", "count"])?;
            r.character.life().ok_or_else(bad)?.recall_episodes(
                text(a, "input")?,
                c::get(a, "tags")?,
                c::uint(c::get(a, "count")?, 0, 16)? as usize,
            )
        }
        "character_life_forget" => {
            c::fields(a, &["id", "tokens", "revision"])?;
            invalidate(r)?;
            let result = r.character.life_mut()?.forget_episodes(c::get(a, "tokens")?, num(a, "revision")?)?;
            if let Some(co) = &mut r.coordinator {
                co.erase()?;
            }
            if let Some(i) = &mut r.initiative {
                i.erase();
            }
            Ok(result)
        }
        "character_life_retention" => {
            c::fields(a, &["id", "retain"])?;
            r.retention = c::boolean(c::get(a, "retain")?)?;
            invalidate(r)?;
            Ok(obj! {"retain"=>r.retention})
        }
        "character_life_ack" => {
            c::fields(a, &["id", "through"])?;
            invalidate(r)?;
            r.character.life_mut()?.acknowledge_archive(num(a, "through")?)?;
            Ok(r.character.life().unwrap().memory_stats())
        }
        "character_life_evict" => {
            c::fields(a, &["id", "through"])?;
            invalidate(r)?;
            let n = r.character.life_mut()?.evict(num(a, "through")?)?;
            Ok(obj! {"evicted"=>n})
        }
        "character_life_consolidate" => {
            c::fields(a, &["id", "start", "end"])?;
            invalidate(r)?;
            r.character.life_mut()?.consolidate(num(a, "start")?, num(a, "end")?)
        }
        "character_life_author_edit" => {
            c::fields(a, &["id", "setting", "definition", "revision"])?;
            let d =
                Arc::new(Definition::draft(c::get(a, "definition")?, &Setting::from_value(c::get(a, "setting")?)?)?);
            invalidate(r)?;
            r.character.life_mut()?.author_edit(d, num(a, "revision")?)?;
            Ok(obj! {"revision"=>r.character.life().unwrap().revision()})
        }
        "character_life_inspect" => {
            c::fields(a, &["id"])?;
            Ok(
                obj! {"life"=>r.character.life().ok_or_else(bad)?.inspect(),"continuity"=>r.coordinator.as_ref().map_or(Value::Null,Coordinator::inspect),"initiative"=>r.initiative.as_ref().map_or(Value::Null,Initiative::inspect),"retention"=>r.retention},
            )
        }
        "character_life_save" => {
            c::fields(a, &["id", "identity"])?;
            let result = obj! {"version"=>2,"interactions"=>r.character.interactions().checkpoint(),"initiative"=>r.initiative.as_ref().map_or(Value::Null,Initiative::checkpoint),"life"=>r.character.life().ok_or_else(bad)?.checkpoint(c::get(a,"identity")?)?,"continuity"=>r.coordinator.as_ref().map_or(Value::Null,Coordinator::checkpoint),"retention"=>r.retention};
            crate::request::bounded(&result, crate::request::MAX_REQUEST_BYTES - 65536)?;
            Ok(result)
        }
        "character_life_restore" => {
            c::fields(
                a,
                &[
                    "id",
                    "identity",
                    "state",
                    "fence",
                    "min_ticket",
                    "min_proposal",
                    "min_receipt",
                    "min_note",
                    "max_tick",
                ],
            )?;
            let state = c::get(a, "state")?;
            c::fields(state, &["version", "life", "continuity", "retention", "initiative", "interactions"])?;
            c::uint(c::get(state, "version")?, 2, 2)?;
            let mut life = r.character.life().ok_or_else(bad)?.clone();
            life.restore_checkpoint(c::get(a, "identity")?, c::get(state, "life")?, c::get(a, "fence")?)?;
            if let Some(current) = &r.coordinator {
                if c::get(state, "continuity")? != &current.checkpoint() {
                    return Err(bad());
                }
            }
            if let Some(current) = &r.initiative {
                if c::get(state, "initiative")? != &current.checkpoint() {
                    return Err(bad());
                }
            }
            if c::get(state, "continuity")? == &Value::Null && num(a, "min_ticket")? > 0
                || c::get(state, "initiative")? == &Value::Null
                    && (num(a, "min_proposal")? > 0 || num(a, "min_receipt")? > 0)
            {
                return Err(bad());
            }
            let co = if c::get(state, "continuity")? == &Value::Null {
                None
            } else {
                let co =
                    Coordinator::recover(c::get(state, "continuity")?, num(a, "min_ticket")?, num(a, "max_tick")?)?;
                if !co.identity_matches_life(&life) {
                    return Err(bad());
                }
                Some(co)
            };
            let initiative = if c::get(state, "initiative")? == &Value::Null {
                None
            } else {
                Some(Initiative::recover(
                    c::get(state, "initiative")?,
                    &life,
                    num(a, "min_proposal")?,
                    num(a, "min_receipt")?,
                    num(a, "max_tick")?,
                )?)
            };
            let current = r.character.interactions().checkpoint();
            let min_note = c::uint(c::get(a, "min_note")?, 0, u32::MAX as u64)?
                .max(crate::life::num(c::get(&current, "notes")?, "sequence")?) as u32;
            let interactions = crate::dialogue::InteractionTools::recover(
                c::get(state, "interactions")?,
                min_note,
                num(a, "max_tick")? as u32,
            )?;
            if interactions.identity() != r.character.interactions().identity() {
                return Err(bad());
            }
            let restored_used = c::list(c::get(c::get(state, "interactions")?, "used")?)?;
            if c::list(c::get(&current, "used")?)?.iter().any(|id| !restored_used.contains(id)) {
                return Err(bad());
            }
            let retention = c::boolean(c::get(state, "retention")?)?;
            invalidate(r)?;
            r.character.with_life(life)?;
            r.character.restore_interactions(interactions)?;
            r.coordinator = co;
            r.retention = retention;
            r.initiative = initiative;
            Ok(obj! {"restored"=>text(a,"id")?})
        }
        "character_continuity_configure" => {
            c::fields(a, &["id"])?;
            if r.coordinator.is_some() {
                return Err(bad());
            }
            let life = r.character.life().ok_or_else(bad)?;
            r.coordinator = Some(Coordinator::new(&life.scope, &life.actor, &Value::strings(&life.participants))?);
            Ok(obj! {"configured"=>text(a,"id")?})
        }
        "character_continuity_scene" => {
            c::fields(a, &["id", "scene"])?;
            r.plan = None;
            r.character.disable();
            r.coordinator.as_mut().ok_or_else(bad)?.bind_scene(c::get(a, "scene")?)?;
            Ok(obj! {"bound"=>true})
        }
        "character_continuity_thread" => {
            c::fields(a, &["id", "thread"])?;
            r.plan = None;
            r.character.disable();
            r.coordinator.as_mut().ok_or_else(bad)?.thread(c::get(a, "thread")?)?;
            Ok(obj! {"bound"=>true})
        }
        "character_continuity_context" => {
            c::fields(a, &["id", "thread", "selection", "caps", "dialogue"])?;
            r.coordinator.as_ref().ok_or_else(bad)?.select_context(
                text(a, "thread")?,
                r.character.life().ok_or_else(bad)?,
                c::get(a, "selection")?,
                ContextCaps::from_value(c::get(a, "caps")?)?,
                &crate::dialogue::Thread::from_value(c::get(a, "dialogue")?)?,
            )
        }
        "character_continuity_speech_context" => {
            c::fields(a, &["id", "thread", "selection", "caps", "dialogue", "turn"])?;
            r.coordinator.as_ref().ok_or_else(bad)?.select_context_for_turn(
                text(a, "thread")?,
                r.character.life().ok_or_else(bad)?,
                c::get(a, "selection")?,
                ContextCaps::from_value(c::get(a, "caps")?)?,
                &crate::dialogue::Thread::from_value(c::get(a, "dialogue")?)?,
                &crate::life::SpeechTurn::from_value(c::get(a, "turn")?)?,
            )
        }
        "character_continuity_generate" => {
            c::fields(a, &["id", "thread", "text", "dependencies"])?;
            let rev = r.character.life().ok_or_else(bad)?.revision();
            r.coordinator.as_mut().ok_or_else(bad)?.generate(
                text(a, "thread")?,
                text(a, "text")?,
                c::get(a, "dependencies")?,
                rev,
            )
        }
        "character_continuity_validate" => {
            c::fields(a, &["id", "ticket"])?;
            let rev = r.character.life().ok_or_else(bad)?.revision();
            r.coordinator.as_mut().ok_or_else(bad)?.validate(num(a, "ticket")?, rev)
        }
        "character_continuity_delivery" => {
            c::fields(a, &["id", "delivery"])?;
            r.coordinator.as_mut().ok_or_else(bad)?.delivery(c::get(a, "delivery")?)
        }
        "character_continuity_recognition" => {
            c::fields(a, &["id", "recognition"])?;
            r.coordinator.as_ref().ok_or_else(bad)?.recognition(c::get(a, "recognition")?)
        }
        "character_continuity_retire" => {
            c::fields(a, &["id", "ticket"])?;
            r.coordinator.as_mut().ok_or_else(bad)?.retire(num(a, "ticket")?)?;
            Ok(obj! {"retired"=>num(a,"ticket")?})
        }
        "character_initiative_configure" => {
            c::fields(a, &["id", "defaults", "overrides"])?;
            if r.initiative.is_some() {
                return Err(bad());
            }
            let life = r.character.life().ok_or_else(bad)?;
            r.initiative = Some(Initiative::new(
                &life.scope,
                &life.actor,
                &Value::strings(&life.participants),
                InitiativePolicy::resolve(c::get(a, "defaults")?, c::get(a, "overrides")?)?,
            )?);
            Ok(obj! {"configured"=>text(a,"id")?})
        }
        "character_initiative_policy" => {
            c::fields(a, &["id", "defaults", "overrides"])?;
            r.initiative
                .as_mut()
                .ok_or_else(bad)?
                .set_policy(InitiativePolicy::resolve(c::get(a, "defaults")?, c::get(a, "overrides")?)?);
            Ok(obj! {"configured"=>text(a,"id")?})
        }
        "character_initiative_consider" => {
            c::fields(a, &["id", "frame", "candidates"])?;
            let character = &r.character;
            let life = character.life().ok_or_else(bad)?;
            let routine = character.routine();
            let tools = character.interactions();
            let i = r.initiative.as_mut().ok_or_else(bad)?;
            if let Some(routine) = routine {
                i.consider_routine(c::get(a, "frame")?, c::get(a, "candidates")?, life, tools, routine)
            } else {
                i.consider(c::get(a, "frame")?, c::get(a, "candidates")?, life, tools)
            }
        }
        "character_initiative_dispatch" => {
            c::fields(a, &["id", "proposal", "frame", "allowed"])?;
            let allowed = c::list(c::get(a, "allowed")?)?;
            if allowed.len() > 32 {
                return Err(bad());
            }
            let life = r.character.life().ok_or_else(bad)?;
            let routine = r.character.routine();
            let tools = r.character.interactions();
            let i = r.initiative.as_mut().ok_or_else(bad)?;
            if let Some(routine) = routine {
                i.dispatch_routine(num(a, "proposal")?, c::get(a, "frame")?, life, tools, routine, |p| {
                    allowed.contains(p)
                })
            } else {
                i.dispatch(num(a, "proposal")?, c::get(a, "frame")?, life, tools, |p| allowed.contains(p))
            }
        }
        "character_initiative_outcome" => {
            c::fields(a, &["id", "proposal", "token", "success"])?;
            r.initiative.as_mut().ok_or_else(bad)?.outcome(
                num(a, "proposal")?,
                num(a, "token")?,
                c::boolean(c::get(a, "success")?)?,
                r.character.life().ok_or_else(bad)?,
            )?;
            Ok(obj! {"confirmed"=>num(a,"proposal")?})
        }
        "character_initiative_reserve" => {
            c::fields(a, &["id", "turn"])?;
            r.initiative.as_mut().ok_or_else(bad)?.reserve_call(num(a, "turn")?)?;
            Ok(obj! {"reserved"=>num(a,"turn")?})
        }
        _ => Err(bad()),
    }
}
