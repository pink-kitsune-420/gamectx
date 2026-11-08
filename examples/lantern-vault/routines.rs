//! Optional surface-annex sensor demo. It supplies synthetic engine confirmations;
//! the ordinary underground game's topology, time policy and gameplay stay unchanged.
use gamectx::{
    character::routine::{Config, Frame, Routine, Runtime},
    character::tools::{CharacterTools, CharacterView},
    data::json,
    obj, Result, Value,
};
use std::{io::Read, sync::Arc};
fn read(path: &str) -> Result<Value> {
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|e| gamectx::Error::new(e.to_string()))?
        .take(131073)
        .read_to_string(&mut text)
        .map_err(|e| gamectx::Error::new(e.to_string()))?;
    json::parse(&text, 131072)
}
pub fn demo(definition: Option<&str>, policy: Option<&str>) -> Result<Value> {
    let d = match definition {
        Some(p) => read(p)?,
        None => json::parse(include_str!("routine.json"), 131072)?,
    };
    let p = match policy {
        Some(p) => read(p)?,
        None => json::parse(include_str!("routine-config.json"), 8192)?,
    };
    let config = Config::resolve(
        p.get("defaults").ok_or_else(|| gamectx::Error::new("missing defaults"))?,
        p.get("overrides").ok_or_else(|| gamectx::Error::new("missing overrides"))?,
    )?;
    let mut runtime = Runtime::new(
        "annex:solo",
        "npc:merchant",
        &vec!["npc:merchant"].into(),
        "cycle:37",
        config,
        Some(Arc::new(Routine::from_value(&d)?)),
    )?;
    let mut scheduler = gamectx::runtime::schedule::Scheduler::new(1, 1)?;
    scheduler.register(0, 1, 1, 10, 0)?;
    let mut character = CharacterTools::new("annex:solo", "npc:merchant", &vec!["npc:merchant"].into())?;
    let mut results = Vec::new();
    for (revision, now, scene, customer, route) in [
        (1, 0, "sun_through_annex_window", false, true),
        (2, 3, "customer_interrupts_work", true, true),
        (3, 5, "customer_leaves_work_resumes", false, true),
        (4, 13, "postponed_work_blocked_route", false, false),
        (5, 21, "sunlight_remembered_indoors", false, false),
        (6, 60, "earlier_sunlight_expired", false, false),
    ] {
        scheduler.changed(0, false)?;
        assert_eq!(scheduler.poll(now)?, vec![0]);
        runtime.update(Frame::from_value(&obj!{"revision"=>revision,"now"=>now,"conditions"=>if customer{vec!["route","customer"]}else if route{vec!["route"]}else{Vec::<&str>::new()},"allowed"=>vec!["work","customer","walk"],"emergency"=>false})?)?;
        if now == 0 || now == 3 {
            // Real engines replace these fixtures with their visibility/occlusion sensors.
            let visible = now == 0;
            runtime.observe(&obj!{"token"=>revision,"observer"=>"npc:merchant","location"=>"annex:shop","at"=>now,"expires"=>50,"kind"=>"direct","source"=>obj!{"id"=>format!("sensor:{revision}"),"speaker"=>"npc:merchant","text"=>if visible{"Sun visible through the open annex window."}else{"The inner shop has no visible sun."}},"audience"=>vec!["npc:merchant"],"value"=>obj!{"visible"=>visible,"indoors"=>true},"uncertainty"=>if visible{Value::str("Host angular estimate.")}else{Value::Null},"time"=>if visible{obj!{"source"=>"sun","calendar"=>"cycle:37","low"=>3,"high"=>5,"rate_low"=>0,"rate_high"=>2,"usable"=>true}}else{Value::Null}})?;
        }
        character.with_runtime(runtime.clone())?;
        character.bind(CharacterView::new(revision))?;
        character.enable(&["routine_status"])?;
        let context = character.call("routine_status", "{}", revision, 8192, |p| {
            p == &obj! {"action"=>"routine_status","target"=>"npc:merchant"}
        })?;
        results.push(obj! {"scene"=>scene,"context"=>context});
    }
    Ok(obj! {"demo"=>"Lantern Vault optional surface annex","model_calls"=>0,"steps"=>results})
}
#[cfg(test)]
mod tests {
    #[test]
    fn demo_records_actual_sensors_and_preserves_uncertainty() {
        let v = super::demo(None, None).unwrap();
        let steps = v.get("steps").unwrap().as_array().unwrap();
        let ctx = |i: usize| steps[i].get("context").unwrap();
        assert_eq!(ctx(1).get("current").unwrap().get("id"), Some(&gamectx::Value::str("customer")));
        assert_eq!(ctx(2).get("current").unwrap().get("id"), Some(&gamectx::Value::str("work")));
        assert_eq!(ctx(3).get("current").unwrap().get("id"), Some(&gamectx::Value::str("work")));
        assert_eq!(ctx(4).get("observations").unwrap().as_array().unwrap().len(), 2);
        let source = &ctx(4).get("time").unwrap().get("sources").unwrap().as_array().unwrap()[0];
        assert_eq!(source.get("high"), Some(&gamectx::Value::from(47u32)));
        assert_eq!(ctx(5).get("time").unwrap().get("status"), Some(&gamectx::Value::str("unknown")));
    }
}
