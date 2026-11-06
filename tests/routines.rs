use gamectx::{
    character::{CharacterTools, CharacterView},
    json::parse,
    obj,
    observation::History,
    routine::{Config, Frame, Routine, Runtime, Timeline},
    Value,
};
use std::sync::Arc;
fn input() -> Value {
    parse(include_str!("../conformance/routines/interruption_resume/input.json"), 1_500_000)
        .unwrap()
        .get("args")
        .unwrap()
        .clone()
}
fn definition() -> Arc<Routine> {
    Arc::new(Routine::from_value(input().get("definition").unwrap()).unwrap())
}
fn config(extra: Value) -> Config {
    Config::resolve(&obj!{"routines"=>true,"history"=>true,"time"=>true,"time_sources"=>vec!["sun","instrument","report","routine"],"tools"=>vec!["routine_status","activity_propose"]},&extra).unwrap()
}
fn runtime() -> Runtime {
    Runtime::new("scope:a", "merchant", &vec!["merchant"].into(), "cycle:37", config(obj! {}), Some(definition()))
        .unwrap()
}
fn frame(revision: u32, now: u32, customer: bool, emergency: bool) -> Frame {
    Frame::from_value(&obj!{"revision"=>revision,"now"=>now,"conditions"=>if customer{vec!["route","customer"]}else{vec!["route"]},"allowed"=>vec!["work","walk","customer"],"emergency"=>emergency}).unwrap()
}
fn observation() -> Value {
    parse(include_str!("../conformance/routines/remembered_sunlight/input.json"), 1_500_000)
        .unwrap()
        .get("args")
        .unwrap()
        .get("observations")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .clone()
}
fn set(v: &mut Value, k: &str, x: Value) {
    let Value::Object(fields) = v else { panic!() };
    fields.iter_mut().find(|(key, _)| key == k).unwrap().1 = x;
}
fn current(r: &Runtime) -> Value {
    r.context().unwrap().get("current").unwrap().get("id").unwrap().clone()
}
#[test]
fn intent_interruption_completion_and_actual_experience_are_separate() {
    let mut r = runtime();
    r.update(frame(1, 0, false, false)).unwrap();
    assert_eq!(current(&r), Value::str("work"));
    assert_eq!(r.context().unwrap().get("observations"), Some(&Vec::<Value>::new().into()));
    r.update(frame(2, 1, true, false)).unwrap();
    assert_eq!(current(&r), Value::str("work")); // minimum active duration
    r.update(frame(3, 3, true, false)).unwrap();
    assert_eq!(current(&r), Value::str("customer"));
    assert!(r
        .to_value()
        .get("timeline")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.get("state") == Some(&Value::str("interrupted"))));
    r.update(frame(4, 5, false, false)).unwrap();
    assert_eq!(current(&r), Value::str("work"));
    r.complete("work", 0, 4, 1).unwrap();
    assert!(r.complete("work", 0, 4, 2).is_err());
    assert!(r.complete("walk", 12, 4, 1).is_err());
    assert_eq!(r.context().unwrap().get("observations"), Some(&Vec::<Value>::new().into()));
    r.update(frame(5, 6, false, false)).unwrap();
    assert_eq!(r.context().unwrap().get("current"), Some(&Value::Null));
    r.update(frame(6, 13, false, false)).unwrap();
    assert_eq!(current(&r), Value::str("walk"));
    assert_eq!(r.context().unwrap().get("observations"), Some(&Vec::<Value>::new().into()));
}
#[test]
fn bounded_plan_ties_missed_windows_and_unusual_calendars() {
    let mut d = input().get("definition").unwrap().clone();
    let mut acts = d.get("activities").unwrap().as_array().unwrap().to_vec();
    set(&mut acts[1], "priority", 10u32.into());
    set(&mut acts[1], "requirements", Vec::<Value>::new().into());
    set(&mut d, "activities", acts.clone().into());
    let cfg = config(obj! {});
    let mut a = Timeline::new(Routine::from_value(&d).unwrap(), cfg.clone()).unwrap();
    acts.reverse();
    set(&mut d, "activities", acts.into());
    let mut b = Timeline::new(Routine::from_value(&d).unwrap(), cfg).unwrap();
    for (rev, tick) in [(1, 0), (2, 38), (3, 100), (4, u32::MAX - 3), (5, u32::MAX)] {
        a.update(frame(rev, tick, false, false)).unwrap();
        b.update(frame(rev, tick, false, false)).unwrap();
        assert_eq!(a.to_value(), b.to_value());
        assert!(a.to_value().as_array().unwrap().len() <= 32);
    }
    let mut small = Timeline::new(definition(), config(obj! {"timeline"=>1})).unwrap();
    let before = small.to_value();
    assert!(small.update(frame(1, 0, false, false)).is_err());
    assert_eq!(small.to_value(), before);
}
#[test]
fn sun_window_visibility_broken_clock_and_aging_ranges() {
    let cfg = config(obj! {"time_sources"=>vec!["sun","instrument","report"]});
    let mut h = History::new("scope:a", "merchant", &vec!["merchant"].into(), "cycle:37", cfg).unwrap();
    let mut sun = observation();
    h.observe(&sun).unwrap();
    let time = h.time_knowledge(6, None).unwrap();
    let source = &time.get("sources").unwrap().as_array().unwrap()[0];
    assert_eq!(source.get("low"), Some(&3u32.into()));
    assert_eq!(source.get("high"), Some(&15u32.into()));
    assert_eq!(source.get("age"), Some(&5u32.into()));
    set(&mut sun, "token", 2u32.into());
    set(&mut sun, "value", obj! {"visible"=>false,"indoors"=>true,"sunny"=>true});
    let before = h.to_value();
    assert!(h.observe(&sun).is_err());
    assert_eq!(h.to_value(), before); // weather doesn't reveal time
    let mut time = sun.get("time").unwrap().clone();
    set(&mut time, "usable", false.into());
    set(&mut sun, "time", time);
    h.observe(&sun).unwrap();
    // The visible observation (which can be through a window) survives a later blocked view.
    assert_eq!(h.time_knowledge(7, None).unwrap().get("sources").unwrap().as_array().unwrap().len(), 1);
    assert_eq!(h.time_knowledge(50, None).unwrap().get("status"), Some(&Value::str("unknown")));
}
#[test]
fn retention_scope_and_replay_watermarks_survive_eviction_and_clear() {
    let cfg = config(obj! {"observations"=>1});
    let mut h = History::new("scope:a", "merchant", &vec!["merchant"].into(), "cycle:37", cfg).unwrap();
    let mut o = observation();
    h.observe(&o).unwrap();
    set(&mut o, "token", 2u32.into());
    h.observe(&o).unwrap();
    assert_eq!(h.to_value().get("records").unwrap().as_array().unwrap().len(), 1);
    h.clear();
    assert!(h.observe(&o).is_err());
    assert_eq!(h.last_token(), 2);
    set(&mut o, "token", 3u32.into());
    set(&mut o, "audience", vec!["player:b"].into());
    let before = h.to_value();
    assert!(h.observe(&o).is_err());
    assert_eq!(h.to_value(), before);
    set(&mut o, "audience", vec!["merchant"].into());
    set(&mut o, "observer", Value::str("player:b"));
    assert!(h.observe(&o).is_err());
    let mut shared =
        History::new("shared", "merchant", &vec!["merchant", "player:a"].into(), "cycle:37", config(obj! {})).unwrap();
    assert!(shared.observe(&observation()).is_err());
}
#[test]
fn opt_in_registration_exposure_live_permissions_and_revision_are_distinct() {
    let mut character = CharacterTools::new("scope:a", "merchant", &vec!["merchant"].into()).unwrap();
    character.bind(CharacterView::new(1)).unwrap();
    assert!(character.enable(&["routine_status"]).is_err());
    let mut r = runtime();
    r.update(frame(1, 0, false, false)).unwrap();
    character.with_runtime(r).unwrap();
    assert!(character.call("routine_status", "{}", 1, 4096, |_| true).is_err());
    character.enable(&["routine_status"]).unwrap();
    assert!(character.call("activity_propose", r#"{"action":"activity","target":"work"}"#, 1, 4096, |_| true).is_err());
    assert!(character.call("routine_status", "{}", 1, 4096, |_| false).is_err());
    assert!(character.call("routine_status", "{}", 2, 4096, |_| true).is_err());
    assert!(character.call("character_routine_complete", "{}", 1, 4096, |_| true).is_err());
    assert!(character.call("routine_status", r#"{"tools":["activity_propose"]}"#, 1, 4096, |_| true).is_err());
    character.reserve_routine_call(1).unwrap();
    assert!(character.reserve_routine_call(1).is_err());
    assert!(character.call("routine_status", "{}", 1, 4096, |_| true).is_ok());
    character.enable(&["activity_propose"]).unwrap();
    let before = character.routine().unwrap().to_value();
    assert_eq!(
        character
            .call("activity_propose", r#"{"action":"activity","target":"work"}"#, 1, 4096, |_| true)
            .unwrap()
            .get("status"),
        Some(&Value::str("proposed"))
    );
    assert_eq!(before, character.routine().unwrap().to_value());
    assert!(character
        .call("activity_propose", r#"{"action":"activity","target":"customer"}"#, 1, 4096, |_| true)
        .is_err());
    character.routine_mut().unwrap().update(frame(2, 1, true, false)).unwrap();
    assert!(character.enabled().is_empty());
    assert!(character.enable(&["routine_status"]).is_err());
    character.bind(CharacterView::new(2)).unwrap();
    character.enable(&["routine_status"]).unwrap();
    let foreign =
        Runtime::new("scope:b", "merchant", &vec!["merchant"].into(), "cycle:37", config(obj! {}), Some(definition()))
            .unwrap();
    assert!(character.with_runtime(foreign).is_err());
    let cfg = config(obj! {"tools"=>Vec::<Value>::new()});
    let mut disabled =
        Runtime::new("scope:a", "merchant", &vec!["merchant"].into(), "cycle:37", cfg, Some(definition())).unwrap();
    disabled.update(frame(2, 1, false, false)).unwrap();
    character.with_runtime(disabled).unwrap();
    assert!(character.enable(&["routine_status"]).is_err());
}
#[test]
fn save_restore_validates_identity_revision_shape_and_high_watermarks() {
    let mut r = runtime();
    r.update(frame(1, 1, false, false)).unwrap();
    r.observe(&observation()).unwrap();
    r.reserve_model_call(1).unwrap();
    r.complete("work", 0, 1, 3).unwrap();
    let state = r.to_value();
    let mut restored = runtime();
    restored.restore(&state, 1, 1, 3, 1).unwrap();
    assert_eq!(restored.to_value(), state);
    assert!(restored.observe(&observation()).is_err());
    assert!(restored.reserve_model_call(1).is_err());
    for (key, value) in [
        ("scope", Value::str("scope:b")),
        ("actor", Value::str("foreign")),
        ("calls", 0u32.into()),
        ("completion_token", 2u32.into()),
        (
            "timeline",
            vec![obj! {"id"=>"unregistered","start"=>0,"end"=>12,"state"=>"active","since"=>0,"reason"=>"selected"}]
                .into(),
        ),
    ] {
        let mut bad = state.clone();
        set(&mut bad, key, value);
        let before = restored.to_value();
        assert!(restored.restore(&bad, 1, 1, 3, 1).is_err(), "{key}");
        assert_eq!(restored.to_value(), before);
    }
    restored.update(frame(2, 2, false, false)).unwrap();
    assert!(restored.restore(&state, 1, 1, 3, 1).is_err());
    let mut fresh = runtime();
    assert!(fresh.restore(&state, 2, 1, 3, 1).is_err());
    assert!(fresh.restore(&state, 1, 2, 3, 1).is_err());
    assert!(fresh.restore(&state, 1, 1, 4, 1).is_err());
    assert!(fresh.restore(&state, 1, 1, 3, 2).is_err());
}
#[test]
fn history_can_run_without_routines_and_disabled_features_make_no_model_calls() {
    let off = Config::default();
    assert!(Timeline::new(definition(), off.clone()).is_err());
    let mut cfg = off.to_value();
    set(&mut cfg, "history", true.into());
    let cfg = Config::resolve(&cfg, &obj! {}).unwrap();
    let mut r = Runtime::new("scope:a", "merchant", &vec!["merchant"].into(), "cycle:37", cfg, None).unwrap();
    r.update(Frame::from_value(&obj!{"revision"=>1,"now"=>1,"conditions"=>Vec::<Value>::new(),"allowed"=>Vec::<Value>::new(),"emergency"=>false}).unwrap()).unwrap();
    r.observe(&observation()).unwrap();
    assert_eq!(r.context().unwrap().get("time").unwrap().get("status"), Some(&Value::str("unknown")));
}
#[test]
fn scheduler_budgets_many_npc_updates_without_model_work() {
    let mut scheduler = gamectx::schedule::Scheduler::new(128, 16).unwrap();
    let shared = definition();
    let mut npcs = Vec::new();
    for id in 0..128 {
        scheduler.register(id, 1, 1, 16, 0).unwrap();
        npcs.push(Timeline::new(shared.clone(), config(obj! {})).unwrap());
    }
    let mut seen = vec![false; 128];
    for now in 0..8 {
        let ids = scheduler.poll(now).unwrap();
        assert_eq!(ids.len(), 16);
        for id in ids {
            npcs[id as usize].update(frame(1, now as u32, false, false)).unwrap();
            seen[id as usize] = true;
        }
        assert!(scheduler.poll(now).unwrap().is_empty());
    }
    assert!(seen.into_iter().all(|s| s));
    scheduler.changed(127, true).unwrap();
    let ids = scheduler.poll(8).unwrap();
    assert!(ids.contains(&127));
}

#[test]
fn current_instrument_reading_can_be_wrong_and_ages_without_global_clock_access() {
    let mut r = Runtime::new(
        "scope:a",
        "merchant",
        &vec!["merchant"].into(),
        "cycle:37",
        config(obj! {"time_sources"=>vec!["instrument"]}),
        Some(definition()),
    )
    .unwrap();
    r.update(frame(1, 1, false, false)).unwrap();
    let mut o = observation();
    set(&mut o, "kind", Value::str("instrument"));
    set(
        &mut o,
        "time",
        obj! {"source"=>"instrument","calendar"=>"cycle:37","low"=>500,"high"=>500,"rate_low"=>0,"rate_high"=>2,"usable"=>true},
    );
    r.observe(&o).unwrap();
    let source =
        |r: &Runtime| r.context().unwrap().get("time").unwrap().get("sources").unwrap().as_array().unwrap()[0].clone();
    assert_eq!(source(&r).get("precision"), Some(&Value::str("exact")));
    assert_eq!(source(&r).get("low"), Some(&500u32.into()));
    r.update(frame(2, 3, false, false)).unwrap();
    assert_eq!(source(&r).get("precision"), Some(&Value::str("range")));
    assert_eq!(source(&r).get("high"), Some(&504u32.into()));
    let mut unusable = o.get("time").unwrap().clone();
    set(&mut unusable, "usable", false.into());
    set(&mut o, "at", 3u32.into());
    set(&mut o, "token", 2u32.into());
    set(&mut o, "time", unusable);
    r.observe(&o).unwrap();
    assert_eq!(source(&r).get("source"), observation().get("source"));
    // Clear the scope without resetting observation/completion replay watermarks.
    r.clear();
    assert!(r.observe(&o).is_err());
    assert_eq!(r.context().unwrap().get("time").unwrap().get("status"), Some(&Value::str("unknown")));
}
#[test]
fn initial_save_roundtrip_and_private_runtime_roster_are_checked() {
    let mut r = runtime();
    let saved = r.to_value();
    r.restore(&saved, 0, 0, 0, 0).unwrap();
    assert_eq!(saved, r.to_value());
    let mut shared = CharacterTools::new("scope:a", "merchant", &vec!["merchant", "player:a"].into()).unwrap();
    assert!(shared.with_runtime(r).is_err());
}
#[test]
fn focused_and_expanded_context_keep_required_facts_with_full_request_recounts() {
    use gamectx::{
        character::{AttentionLimits, MemoryChoice, ToolChoice, TurnPlan},
        request::{Count, CountMethod, RequestBudget},
    };
    let mut r = runtime();
    r.update(frame(1, 0, false, false)).unwrap();
    let ctx = r.context().unwrap();
    let mut c = CharacterTools::new("scope:a", "merchant", &vec!["merchant"].into()).unwrap();
    c.with_runtime(r).unwrap();
    c.bind(CharacterView::new(1)).unwrap();
    let mut turn=TurnPlan{task:"routine".into(),input:"Choose the next permitted activity.".into(),facts:obj!{"must_keep"=>"The customer has not paid.","current"=>ctx.get("current").unwrap().clone(),"next"=>ctx.get("next").unwrap().clone()},tools:vec![ToolChoice{name:"activity_propose".into(),task:"routine".into(),priority:10,required:true},ToolChoice{name:"routine_status".into(),task:"routine".into(),priority:5,required:false}],memories:(0..12).map(|i|MemoryChoice{value:obj!{"token"=>i,"source"=>"host-selected actual observation","text"=>"An optional relevant experience.".repeat(3)},priority:10-i,required:false}).collect(),dialogue:Vec::new(),attention:AttentionLimits::focused()};
    let budget = |n| RequestBudget::from_value(&obj! {"context"=>n,"output"=>128,"safety"=>256}).unwrap();
    let count = |v: &Value| Ok(Count { input: v.json_len(), method: CountMethod::Estimate });
    let focused = c
        .assemble(
            &turn,
            &budget(8192),
            |f| Ok(obj! {"template"=>"synthetic fixture","frame"=>f.clone(),"tool_results"=>Vec::<Value>::new()}),
            count,
        )
        .unwrap();
    let more_capacity = c
        .assemble(
            &turn,
            &budget(32768),
            |f| Ok(obj! {"template"=>"synthetic fixture","frame"=>f.clone(),"tool_results"=>Vec::<Value>::new()}),
            count,
        )
        .unwrap();
    assert_eq!(focused.request, more_capacity.request);
    assert_eq!(c.enabled(), &["activity_propose"]);
    turn.attention = AttentionLimits::new(2, 12, 0).unwrap();
    let mut recounts = 0;
    let expanded=c.assemble(&turn,&budget(2048),|f|Ok(obj!{"template"=>"synthetic fixture","frame"=>f.clone(),"tool_results"=>vec!["A bounded prior tool result."]}),|v|{recounts+=1;count(v)}).unwrap();
    assert!(recounts > 1);
    assert_eq!(expanded.request.get("frame").unwrap().get("facts"), Some(&turn.facts));
    assert!(expanded.request.json_len() + 384 <= 2048);
    let mut larger = expanded.request.clone();
    set(&mut larger, "tool_results", vec!["x".repeat(2048)].into());
    assert!(larger.json_len() + 384 > 2048); // A previous count cannot authorize a larger follow-up.
}
