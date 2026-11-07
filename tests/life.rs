use gamectx::{
    json,
    life::{Config, Definition, Life, Setting},
    obj, Value,
};
use std::sync::Arc;
fn parse(s: &str) -> Value {
    json::parse(s, 524288).unwrap()
}
fn life() -> Life {
    let setting = Setting::from_value(&parse(include_str!("../examples/characters/setting.json"))).unwrap();
    let definition = Arc::new(
        Definition::draft(&parse(include_str!("../examples/characters/merchant-life.json")), &setting).unwrap(),
    );
    Life::accept(
        "counter",
        "merchant",
        &vec!["merchant", "player"].into(),
        definition,
        Config::resolve(&obj! {"enabled"=>true}, &obj! {}).unwrap(),
    )
    .unwrap()
}
fn event(token: u64, at: u64, kind: &str) -> Value {
    obj! {"token"=>token,"at"=>at,"kind"=>kind,"tag"=>"returned","subject"=>"player","source"=>obj!{"id"=>format!("receipt:{token}"),"speaker"=>"merchant","text"=>"Lamp returned intact."},"audience"=>vec!["merchant","player"],"text"=>"Lamp returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true}
}
#[test]
fn appraisal_is_bounded_evidenced_and_cannot_replay() {
    let mut l = life();
    l.experience(&event(1, 1, "report")).unwrap();
    assert!(l.appraise("returned_lamp", 1, 1).is_err());
    l.experience(&event(2, 2, "outcome")).unwrap();
    let proposed = l.propose(r#"{"rule":"returned_lamp","event":2,"reason":"An intact return."}"#).unwrap();
    assert_eq!(proposed.get("status").and_then(Value::as_str), Some("proposed"));
    assert_eq!(l.revision(), 2);
    l.appraise("returned_lamp", 2, 2).unwrap();
    assert!(l.appraise("returned_lamp", 2, 3).is_err());
    assert!(l.experience(&event(2, 3, "outcome")).is_err());
    let before = l.inspect();
    assert!(l.appraise("missing", 2, 3).is_err());
    assert_eq!(before, l.inspect());
}
#[test]
fn parser_bounds() {
    let l = life();
    assert!(l.propose(&"x".repeat(4097)).is_err());
    assert!(l.propose(r#"{"rule":"returned_lamp","event":1,"reason":"x","enabled":true}"#).is_err());
}
#[test]
fn private_scope_is_not_shared() {
    let l = life();
    assert!(!l.identity_matches("elsewhere", "merchant", &["merchant".into(), "player".into()]));
}
fn coordinator() -> gamectx::life::Coordinator {
    let mut c = gamectx::life::Coordinator::new("counter", "merchant", &vec!["merchant", "player"].into()).unwrap();
    c.bind_scene(&scene(1, 1)).unwrap();
    c.thread(&thread("trade")).unwrap();
    c
}
fn scene(revision: u64, epoch: u64) -> Value {
    obj! {"id"=>"counter","revision"=>revision,"epoch"=>epoch,"at"=>revision,"participants"=>vec!["merchant","player"],"facts"=>vec![obj!{"id"=>"sun","revision"=>1,"observed_at"=>1,"source"=>obj!{"id"=>"sensor","speaker"=>"merchant","text"=>"Visible sun."},"kind"=>"perceived","audience"=>vec!["merchant","player"],"text"=>"Sun through a window."}],"affordances"=>vec!["serve"]}
}
fn thread(id: &str) -> Value {
    obj! {"id"=>id,"topic"=>"Lamp repairs","questions"=>vec!["Which wick?"],"promises"=>vec!["repair"],"unfinished"=>vec!["polish"],"listener"=>"player","participants"=>vec!["merchant","player"],"shared"=>vec!["sun"],"referents"=>vec!["lamp"]}
}
#[test]
fn harmless_updates_two_threads_and_one_use_validation() {
    let mut c = coordinator();
    c.thread(&thread("weather")).unwrap();
    c.generate("trade", "That wick is ready.", &vec!["sun"].into(), 0).unwrap();
    c.bind_scene(&scene(2, 1)).unwrap();
    assert_eq!(c.validate(1, 0).unwrap().get("stage").and_then(Value::as_str), Some("validated"));
    assert!(c.validate(1, 0).is_err());
    c.generate("weather", "The window is bright.", &vec!["sun"].into(), 0).unwrap();
    c.bind_scene(&scene(3, 2)).unwrap();
    assert_eq!(c.validate(2, 0).unwrap().get("stage").and_then(Value::as_str), Some("cancelled"));
}
#[test]
fn fake_speech_transport_does_not_create_hearing_or_actions() {
    use gamectx::life::SpeechTransport;
    #[derive(Default)]
    struct Fake {
        queued: Vec<u64>,
        cancelled: Vec<u64>,
    }
    impl SpeechTransport for Fake {
        fn queue(&mut self, id: u64, _: &str) -> gamectx::Result<()> {
            self.queued.push(id);
            Ok(())
        }
        fn cancel(&mut self, id: u64) -> gamectx::Result<()> {
            self.cancelled.push(id);
            Ok(())
        }
    }
    let mut c = coordinator();
    let mut transport = Fake::default();
    c.generate("trade", "I can fix it.", &vec!["sun"].into(), 0).unwrap();
    c.validate(1, 0).unwrap();
    transport.queue(1, "I can fix it.").unwrap();
    for (event, n, recipient) in [
        ("queued", 0, Value::Null),
        ("played", 5, Value::Null),
        ("interrupted", 5, Value::Null),
        ("heard", 3, Value::str("player")),
    ] {
        c.delivery(&obj! {"id"=>1,"event"=>event,"through"=>n,"recipient"=>recipient}).unwrap();
    }
    transport.cancel(1).unwrap();
    let speech = c.inspect();
    assert_eq!(speech.get("speech").unwrap().as_array().unwrap()[0].get("played"), Some(&Value::from(5)));
    assert!(c.delivery(&obj! {"id"=>1,"event"=>"heard","through"=>6,"recipient"=>"player"}).is_err());
    let interim = c.recognition(&obj! {"text"=>"buy the lamp","final"=>false,"uncertainty"=>"Name unclear"}).unwrap();
    assert_eq!(interim.get("actions").unwrap().as_array().unwrap().len(), 0);
    c.retire(1).unwrap();
    assert!(c.validate(1, 0).is_err());
}
fn identity() -> Value {
    obj! {"campaign"=>"vault","branch"=>"save:a","instance"=>"merchant:1"}
}
fn fence() -> Value {
    obj! {"min_revision"=>0,"max_revision"=>10000,"min_token"=>0,"max_token"=>10000,"max_tick"=>10000}
}
#[test]
fn checkpoint_identity_fences_and_corrupt_state_fail_atomically() {
    let mut l = life();
    l.experience(&event(1, 1, "direct")).unwrap();
    l.appraise("returned_lamp", 1, 1).unwrap();
    let saved = l.checkpoint(&identity()).unwrap();
    let mut loaded = life();
    loaded.restore_checkpoint(&identity(), &saved, &fence()).unwrap();
    assert_eq!(l.inspect(), loaded.inspect());
    let foreign = obj! {"campaign"=>"vault","branch"=>"save:b","instance"=>"merchant:1"};
    assert!(life().restore_checkpoint(&foreign, &saved, &fence()).is_err());
    let before = loaded.inspect();
    let mut bad = saved.clone();
    if let Value::Object(fields) = &mut bad {
        fields.retain(|(k, _)| k != "archive_ack");
    }
    assert!(loaded.restore_checkpoint(&identity(), &bad, &fence()).is_err());
    assert_eq!(before, loaded.inspect());
    let strict = obj! {"min_revision"=>3,"max_revision"=>10,"min_token"=>0,"max_token"=>10,"max_tick"=>10};
    assert!(life().restore_checkpoint(&identity(), &saved, &strict).is_err());
}
#[test]
fn archive_failure_retry_and_crash_recovery_are_idempotent() {
    use gamectx::life::{FileStore, Storage};
    struct Fail;
    impl Storage for Fail {
        fn archive(&mut self, _: &Value, _: &Value) -> gamectx::Result<()> {
            Err(gamectx::Error::new("offline"))
        }
        fn checkpoint(&mut self, _: &Value) -> gamectx::Result<()> {
            unreachable!()
        }
    }
    let mut l = life();
    l.experience(&event(1, 1, "outcome")).unwrap();
    assert!(l.flush(&identity(), &mut Fail).is_err());
    assert!(l.evict(1).is_err());
    let root = std::env::temp_dir().join(format!("gamectx-life-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut store = FileStore::new(&root).unwrap();
    l.flush(&identity(), &mut store).unwrap();
    l.flush(&identity(), &mut store).unwrap();
    assert_eq!(store.event(&identity(), 1).unwrap(), event(1, 1, "outcome"));
    let snapshot = store.load(&identity(), l.revision()).unwrap();
    let mut loaded = life();
    loaded.restore_checkpoint(&identity(), &snapshot, &fence()).unwrap();
    assert!(loaded.experience(&event(1, 2, "outcome")).is_err());
    assert_eq!(loaded.evict(1).unwrap(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn saved_conversation_never_resurrects_pending_work() {
    let mut c = coordinator();
    c.generate("trade", "Welcome back.", &vec!["sun"].into(), 0).unwrap();
    let saved = c.checkpoint();
    let mut recovered = gamectx::life::Coordinator::recover(&saved, 1, 10).unwrap();
    assert!(recovered.validate(1, 0).is_err());
    let next = recovered.generate("trade", "Welcome back.", &vec!["sun"].into(), 0).unwrap();
    assert_eq!(next.get("id"), Some(&Value::from(2)));
    assert!(gamectx::life::Coordinator::recover(&saved, 2, 10).is_err());
}
#[test]
fn erasure_removes_causal_and_subjective_derivations() {
    let mut l = life();
    l.experience(&event(1, 1, "direct")).unwrap();
    l.appraise("returned_lamp", 1, 1).unwrap();
    let mut e = event(2, 2, "correction");
    if let Value::Object(fields) = &mut e {
        fields.iter_mut().find(|(k, _)| k == "causes").unwrap().1 = vec![1].into();
    }
    l.experience(&e).unwrap();
    l.consolidate(1, 2).unwrap();
    let r = l.forget_episodes(&vec![1].into(), l.revision()).unwrap();
    assert_eq!(r.get("forgotten"), Some(&vec![1, 2].into()));
    assert_eq!(l.memory_stats().get("events"), Some(&Value::from(0)));
    assert_eq!(l.memory_stats().get("summaries"), Some(&Value::from(0)));
    assert!(l.experience(&event(1, 3, "direct")).is_err());
    assert!(l.recall_episodes("lamp", &vec!["trade"].into(), 2).unwrap().as_array().unwrap().is_empty());
}
fn initiative_frame(now: u64, turn: u64) -> Value {
    obj! {"revision"=>now,"now"=>now,"turn"=>turn,"conditions"=>vec!["stock"],"allowed"=>vec!["polish","greet"],"resources"=>vec!["bench"],"audience"=>vec!["merchant","player"],"emergency"=>false,"depth"=>0,"social_remaining"=>2}
}
fn candidate(id: &str, recipient: Value) -> Value {
    obj! {"id"=>id,"kind"=>if recipient==Value::Null{"activity"}else{"ask"},"priority"=>10,"reason"=>"Resume useful work.","requires"=>Vec::<Value>::new(),"resource"=>Value::Null,"recipient"=>recipient,"attitudes"=>Vec::<Value>::new(),"events"=>Vec::<Value>::new(),"promises"=>Vec::<Value>::new()}
}
fn initiative(calls: u64) -> gamectx::life::Initiative {
    gamectx::life::Initiative::new(
        "counter",
        "merchant",
        &vec!["merchant", "player"].into(),
        gamectx::life::InitiativePolicy::resolve(&obj! {"enabled"=>true,"model_calls"=>calls}, &obj! {}).unwrap(),
    )
    .unwrap()
}
fn interactions() -> gamectx::dialogue::InteractionTools {
    gamectx::dialogue::InteractionTools::new("counter", "merchant", &vec!["merchant", "player"].into()).unwrap()
}
#[test]
fn initiative_quota_duplicate_greetings_and_chatter_loop() {
    let l = life();
    let tools = interactions();
    let mut i = initiative(1);
    let greet: Value = vec![candidate("greet", Value::str("player"))].into();
    let before = l.inspect();
    assert_eq!(
        i.consider(&initiative_frame(1, 1), &greet, &l, &tools).unwrap().get("status").and_then(Value::as_str),
        Some("proposed")
    );
    assert_eq!(before, l.inspect());
    i.reserve_call(1).unwrap();
    assert!(i.reserve_call(1).is_err());
    assert_eq!(
        i.consider(&initiative_frame(5, 2), &greet, &l, &tools).unwrap().get("status").and_then(Value::as_str),
        Some("quiet")
    );
    assert_eq!(i.inspect().get("model_calls"), Some(&Value::from(0)));
    let mut loop_frame = initiative_frame(9, 3);
    if let Value::Object(fields) = &mut loop_frame {
        fields.iter_mut().find(|(k, _)| k == "depth").unwrap().1 = 1.into();
    }
    assert_eq!(
        initiative(0).consider(&loop_frame, &greet, &l, &tools).unwrap().get("status").and_then(Value::as_str),
        Some("quiet")
    );
    let mut zero = initiative(0);
    zero.consider(&initiative_frame(1, 1), &greet, &l, &tools).unwrap();
    assert!(zero.reserve_call(1).is_err());
}
#[test]
fn initiative_receipts_backoff_recovery_and_dispatch_permission() {
    let mut l = life();
    let tools = interactions();
    let mut i = initiative(0);
    let candidates: Value = vec![candidate("polish", Value::Null)].into();
    let f = initiative_frame(1, 1);
    i.consider(&f, &candidates, &l, &tools).unwrap();
    assert!(i.dispatch(1, &f, &l, &tools, |_| false).is_err());
    assert!(i.dispatch(1, &f, &l, &tools, |_| true).is_err());
    assert!(l.inspect().get("events").unwrap().as_array().unwrap().is_empty());
    let f = initiative_frame(5, 2);
    i.consider(&f, &candidates, &l, &tools).unwrap();
    i.dispatch(2, &f, &l, &tools, |_| true).unwrap();
    assert!(i.outcome(2, 1, true, &l).is_err());
    let mut receipt = event(1, 5, "outcome");
    if let Value::Object(fields) = &mut receipt {
        fields.iter_mut().find(|(k, _)| k == "tag").unwrap().1 = Value::str("polish");
    }
    l.experience(&receipt).unwrap();
    i.outcome(2, 1, false, &l).unwrap();
    assert!(i.outcome(2, 1, false, &l).is_err());
    assert_eq!(
        i.consider(&initiative_frame(6, 3), &candidates, &l, &tools).unwrap().get("status").and_then(Value::as_str),
        Some("quiet")
    );
    assert_eq!(
        i.consider(&initiative_frame(9, 4), &candidates, &l, &tools).unwrap().get("status").and_then(Value::as_str),
        Some("proposed")
    );
}
#[test]
fn scheduler_limits_96_character_scopes_without_model_work() {
    use gamectx::{
        life::{Initiative, InitiativePolicy},
        schedule::Scheduler,
    };
    let mut scheduler = Scheduler::new(96, 16).unwrap();
    let mut npcs = Vec::new();
    for id in 0..96 {
        let mut l = life();
        l.clear();
        let scope = format!("counter:{id}");
        // Same approved actor definition, independent host character instances/save scopes.
        let setting = Setting::from_value(&parse(include_str!("../examples/characters/setting.json"))).unwrap();
        let def = Arc::new(
            Definition::draft(&parse(include_str!("../examples/characters/merchant-life.json")), &setting).unwrap(),
        );
        let l = Life::accept(
            &scope,
            "merchant",
            &vec!["merchant", "player"].into(),
            def,
            Config::resolve(&obj! {"enabled"=>true}, &obj! {}).unwrap(),
        )
        .unwrap();
        let tools =
            gamectx::dialogue::InteractionTools::new(&scope, "merchant", &vec!["merchant", "player"].into()).unwrap();
        let i = Initiative::new(
            &scope,
            "merchant",
            &vec!["merchant", "player"].into(),
            InitiativePolicy::resolve(&obj! {"enabled"=>true}, &obj! {}).unwrap(),
        )
        .unwrap();
        npcs.push((l, tools, i));
        scheduler.register(id, 1, 1, 100, 0).unwrap();
    }
    let mut served = 0;
    for tick in 1..=6 {
        let jobs = scheduler.poll(tick).unwrap();
        assert_eq!(jobs.len(), 16);
        for id in jobs {
            let (l, tools, i) = &mut npcs[id as usize];
            i.consider(&initiative_frame(tick, tick), &vec![candidate("polish", Value::Null)].into(), l, tools)
                .unwrap();
            assert_eq!(i.inspect().get("model_calls"), Some(&Value::from(0)));
            served += 1;
        }
    }
    assert_eq!(served, 96);
}
#[test]
fn completed_promise_invalidates_initiative_before_dispatch() {
    let l = life();
    let mut tools = interactions();
    let source = obj! {"id"=>"promise:1","speaker"=>"merchant","text"=>"I will polish your lamp."};
    tools
        .call(
            "promise_remember",
            r#"{"promisor":"merchant","beneficiary":"player","task":"Polish the lamp.","source":"promise:1"}"#,
            &vec![source.clone()].into(),
            1,
            8192,
            |_| false,
        )
        .unwrap();
    tools.set_progress(&obj! {"id"=>1,"expected"=>0,"state"=>"accepted","source"=>source.clone()}).unwrap();
    let mut c = candidate("polish", Value::Null);
    if let Value::Object(fields) = &mut c {
        fields.iter_mut().find(|(k, _)| k == "promises").unwrap().1 = vec![1].into();
    }
    let mut i = initiative(0);
    let f = initiative_frame(1, 1);
    let p = i.consider(&f, &vec![c].into(), &l, &tools).unwrap();
    assert_eq!(p.get("score"), Some(&Value::from(30)));
    tools.set_progress(&obj! {"id"=>1,"expected"=>1,"state"=>"fulfilled","source"=>source}).unwrap();
    assert!(i.dispatch(1, &f, &l, &tools, |_| true).is_err());
}
