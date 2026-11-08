//! Host-only visual registry sessions, shared by JSON-lines and the native ABI.
use super::*;
use crate::media::visual::{Activity, ArtBible, AssetRegistry, RegistryConfig, VisualSpec};
#[derive(Debug)]
pub(super) struct VisualResident {
    bible: ArtBible,
    registry: AssetRegistry,
}
pub(super) fn execute(s: &Session, op: &str, a: &Value) -> Result<Value> {
    crate::packet::request::bounded(a, super::super::MAX_REQUEST_BYTES)?;
    let id = text(a, "id")?;
    c::scope(id)?;
    // Always acquire character state before visual state, including operations
    // that need only one. No provider callback or file access occurs under a lock.
    let characters = s.characters.lock().map_err(|_| bad())?;
    let mut slots = s.visuals.lock().map_err(|_| bad())?;
    if op == "visual_create" {
        c::fields(a, &["id", "bible", "config"])?;
        if slots.len() >= 32 || slots.iter().any(|(old, _)| old == id) {
            return Err(bad());
        }
        let bible = ArtBible::from_value(c::get(a, "bible")?)?;
        let registry = AssetRegistry::new(RegistryConfig::from_value(c::get(a, "config")?)?);
        slots.push((id.into(), VisualResident { bible, registry }));
        return Ok(obj! {"created"=>id});
    }
    let i = slots.iter().position(|(old, _)| old == id).ok_or_else(bad)?;
    if op == "visual_drop" {
        c::fields(a, &["id"])?;
        slots.remove(i);
        return Ok(obj! {"dropped"=>id});
    }
    let r = &mut slots[i].1;
    match op {
        "visual_prepare" => {
            c::fields(a, &["id", "frame", "character", "activity"])?;
            let character = c::get(a, "character")?;
            let resident = if character == &Value::Null {
                None
            } else {
                let name = c::text(character)?;
                Some(&characters.iter().find(|(id, _)| id == name).ok_or_else(bad)?.1)
            };
            let life = resident.map(|r| r.character.life().ok_or_else(bad)).transpose()?;
            let activity = Activity::from_value(c::get(a, "activity")?)?;
            let spec = VisualSpec::compile(&r.bible, c::get(a, "frame")?, life, &activity)?;
            let selection = r.registry.request(&spec)?;
            Ok(obj! {"compiled"=>spec.to_value(),"selection"=>selection})
        }
        "visual_submit" => {
            c::fields(a, &["id", "key", "ticket", "asset_uri", "digest"])?;
            r.registry.submit(text(a, "key")?, c::get(a, "ticket")?, text(a, "asset_uri")?, text(a, "digest")?)
        }
        "visual_approve" => {
            c::fields(a, &["id", "key", "asset_uri", "digest"])?;
            r.registry.approve(text(a, "key")?, text(a, "asset_uri")?, text(a, "digest")?)
        }
        "visual_reject" => {
            c::fields(a, &["id", "key", "reason"])?;
            r.registry.reject(text(a, "key")?, text(a, "reason")?)
        }
        "visual_repair" => {
            c::fields(a, &["id", "key", "patch"])?;
            r.registry.repair(text(a, "key")?, c::get(a, "patch")?)
        }
        "visual_inspect" => {
            c::fields(a, &["id", "key"])?;
            if c::get(a, "key")? == &Value::Null {
                Ok(obj! {"bible"=>r.bible.to_value(),"registry"=>r.registry.summary()})
            } else {
                r.registry.inspect(text(a, "key")?)
            }
        }
        _ => Err(bad()),
    }
}
