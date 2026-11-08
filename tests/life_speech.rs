use gamectx::{
    character::life::{assess_sentence, Config, ContextCaps, Definition, Life, Setting, SpeechTurn},
    data::json,
    deploy::deployment::{ModelSize, Workload},
    obj, Value,
};
use std::sync::Arc;
fn val(s: &str) -> Value {
    json::parse(s, 65536).unwrap()
}
fn life() -> Life {
    let setting = Setting::from_value(&val(include_str!("../examples/fixtures/characters/setting.json"))).unwrap();
    let def = Arc::new(
        Definition::draft(&val(include_str!("../examples/fixtures/characters/merchant-life.json")), &setting).unwrap(),
    );
    Life::accept(
        "counter",
        "merchant",
        &vec!["merchant", "player"].into(),
        def,
        Config::resolve(&obj! {"enabled"=>true,"balanced"=>true}, &obj! {}).unwrap(),
    )
    .unwrap()
}
fn turn(profile: &str, knowledge: &str, creative: bool) -> Value {
    obj! {"profile"=>profile,"knowledge"=>knowledge,"creative"=>creative,"speaker"=>"player","question_subject"=>"player","facts"=>if knowledge=="known"{vec![obj!{"key"=>"player_wick","subject"=>"player","status"=>"known","value"=>"blue"}]}else{vec![obj!{"key"=>"player_birthplace","subject"=>"player","status"=>"unknown","value"=>Value::Null}]}}
}
#[test]
fn focused_and_unknown_policy_is_grounded_while_expansion_is_explicit() {
    let l = life();
    for profile in ["focused", "expanded"] {
        for knowledge in ["known", "unknown"] {
            for creative in [false, true] {
                let t = SpeechTurn::from_value(&turn(profile, knowledge, creative)).unwrap();
                let c = l.context_for_turn(&vec!["trade"].into(), ContextCaps::focused(), &t).unwrap();
                let p = c.get("policy").unwrap();
                let open = knowledge == "known" && (profile == "expanded" || creative);
                assert_eq!(p.get("balanced").and_then(Value::as_bool), Some(open));
                if !open {
                    assert_eq!(p.get("improvisation").and_then(Value::as_str), Some(""));
                    assert!(!p.to_json().contains("imagined"));
                    assert!(!p.to_json().contains("Invent playful"));
                }
            }
        }
    }
}
#[test]
fn compiler_binds_player_unknowns_and_keeps_current_request_outside_json() {
    let l = life();
    let t = turn("focused", "unknown", false);
    let request=l.compile_speech(&obj!{"version"=>2,"rules"=>"Reply briefly from host facts.","tags"=>vec!["trade"],"caps"=>obj!{"facts"=>4,"facets"=>2,"attitudes"=>2,"episodes"=>2,"exchanges"=>1},"stable_scene"=>obj!{"id"=>"counter"},"state"=>obj!{},"dialogue"=>Vec::<Value>::new(),"input"=>"Where was I born?","tools"=>Vec::<Value>::new(),"turn"=>t}).unwrap();
    let m = request.get("messages").unwrap().as_array().unwrap();
    assert_eq!(m.last().unwrap().get("content").and_then(Value::as_str), Some("Current request:\nWhere was I born?"));
    let all = request.to_json();
    assert!(all.contains("unknown_to_merchant"));
    assert!(all.contains("player_birthplace"));
    assert!(!all.contains("imagined hypotheticals"));
    assert!(!all.contains("Invent playful"));
    assert!(!m[2].get("content").unwrap().as_str().unwrap().contains("Where was I born?"));
}
#[test]
fn sentence_wall_is_diagnostic_and_does_not_rewrite_text() {
    let text = format!("{},", "x".repeat(239));
    let before = text.clone();
    let a = assess_sentence(&text, 240).unwrap();
    assert_eq!(a.to_value().get("suspected_cutoff"), Some(&true.into()));
    assert_eq!(before, text);
    let full = format!("{}.", "x".repeat(239));
    let a = assess_sentence(&full, 240).unwrap().to_value();
    assert_eq!(a.get("boundary_complete"), Some(&true.into()));
    assert_eq!(a.get("at_schema_wall"), Some(&true.into()));
    assert_eq!(a.get("suspected_cutoff"), Some(&false.into()));
    assert_eq!(
        assess_sentence("Ready. Anything else?", 240).unwrap().to_value().get("single_sentence"),
        Some(&false.into())
    );
    assert!(assess_sentence(&"x".repeat(1025), 240).is_err());
    assert!(assess_sentence("Ready.", 0).is_err());
}
#[test]
fn tier_hints_do_not_enable_expression_on_unknown_questions() {
    let l = life();
    for size in [ModelSize::B1, ModelSize::B1_5, ModelSize::B3, ModelSize::B8, ModelSize::B30, ModelSize::B70] {
        let hints = size.workload_hints(Workload::Dialogue);
        let t = SpeechTurn::from_hints(
            hints,
            "unknown",
            false,
            "player",
            "player",
            turn("focused", "unknown", false).get("facts").unwrap(),
        )
        .unwrap();
        let c = l.context_for_turn(&vec!["trade"].into(), ContextCaps::from_hints(hints).unwrap(), &t).unwrap();
        assert_eq!(c.get("policy").unwrap().get("balanced"), Some(&false.into()));
    }
}
#[test]
fn coordinator_v2_keeps_host_diagnostics_out_and_shares_the_fact_cap() {
    use gamectx::{character::dialogue::Thread, character::life::Coordinator};
    let l = life();
    let mut c = Coordinator::new("counter", "merchant", &vec!["merchant", "player"].into()).unwrap();
    c.bind_scene(&obj!{"id"=>"counter","revision"=>1,"epoch"=>1,"at"=>1,"participants"=>vec!["merchant","player"],"facts"=>Vec::<Value>::new(),"affordances"=>Vec::<Value>::new()}).unwrap();
    c.thread(&obj!{"id"=>"origin","topic"=>"Player birthplace","questions"=>vec!["Where was I born?"],"promises"=>Vec::<Value>::new(),"unfinished"=>Vec::<Value>::new(),"listener"=>"player","participants"=>vec!["merchant","player"],"shared"=>Vec::<Value>::new(),"referents"=>Vec::<Value>::new()}).unwrap();
    let d=Thread::from_value(&obj!{"version"=>1,"scene"=>"counter","conversation"=>"origin","participants"=>vec!["merchant","player"],"exchanges"=>Vec::<Value>::new()}).unwrap();
    let selection = obj! {"required"=>Vec::<Value>::new(),"optional"=>Vec::<Value>::new(),"tags"=>vec!["trade"],"input"=>"Where was I born?"};
    let t = SpeechTurn::from_value(&turn("focused", "unknown", false)).unwrap();
    let result = c.select_context_for_turn("origin", &l, &selection, ContextCaps::focused(), &d, &t).unwrap();
    let p = result.get("packet").unwrap();
    assert_eq!(p.get("version"), Some(&2.into()));
    assert!(!p.to_json().contains("omitted"));
    assert!(p.to_json().contains("unknown_to_merchant"));
    assert!(!p.to_json().contains("imagined hypotheticals"));
    assert!(c
        .select_context_for_turn("origin", &l, &selection, ContextCaps::new(0, 2, 2, 2, 1).unwrap(), &d, &t)
        .is_err());
}
