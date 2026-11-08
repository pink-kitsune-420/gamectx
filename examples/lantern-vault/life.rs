//! Original continuous merchant scenario. All speech is authored for the offline demo;
//! source/delivery/action assertions are checked independently of prose quality.
use gamectx::{
    character::dialogue::{InteractionTools, Thread},
    character::life::{Config, ContextCaps, Coordinator, Definition, Initiative, InitiativePolicy, Life, Setting},
    character::tools::{CharacterTools, CharacterView},
    data::json,
    deploy::deployment::{ModelSize, Workload},
    obj, Result, Value,
};
use std::sync::Arc;
fn val(s: &str) -> Result<Value> {
    json::parse(s, 524288)
}
fn event(token: u64, at: u64, kind: &str, tag: &str, text: &str, causes: Vec<u64>) -> Value {
    obj! {"token"=>token,"at"=>at,"kind"=>kind,"tag"=>tag,"subject"=>"player","source"=>obj!{"id"=>format!("host:{token}"),"speaker"=>"player","text"=>text},"audience"=>vec!["merchant","player"],"text"=>text,"causes"=>causes,"retain"=>true}
}
fn scene(id: &str, revision: u64, epoch: u64, at: u64, visible: bool) -> Value {
    obj! {"id"=>id,"revision"=>revision,"epoch"=>epoch,"at"=>at,"participants"=>vec!["merchant","player"],"facts"=>vec![obj!{"id"=>"view","revision"=>revision,"observed_at"=>at,"source"=>obj!{"id"=>format!("sensor:{revision}"),"speaker"=>"merchant","text"=>if visible{"Sun visible through the annex window."}else{"Clouds and a closed shutter block the view."}},"kind"=>"perceived","audience"=>vec!["merchant","player"],"text"=>if visible{"A visible patch of sun lies beyond the window; exact time unknown."}else{"No current sun observation; indoor time is unknown."}}],"affordances"=>vec!["discuss","polish"]}
}
fn topic() -> Value {
    obj! {"id"=>"lamps","topic"=>"A quiet lamp and the postponed repair","questions"=>vec!["Which wick should be fitted?"],"promises"=>vec!["1"],"unfinished"=>vec!["polish"],"listener"=>"player","participants"=>vec!["merchant","player"],"shared"=>vec!["view"],"referents"=>vec!["lamp"]}
}
pub fn demo(size: ModelSize, tools: bool) -> Result<Value> {
    let p = val(include_str!("life-config.json"))?;
    let setting = Setting::from_value(&val(include_str!("../fixtures/characters/setting.json"))?)?;
    let definition =
        Arc::new(Definition::draft(&val(include_str!("../fixtures/characters/merchant-life.json"))?, &setting)?);
    let mut overrides = p.get("overrides").unwrap().clone();
    let hints = size.workload_hints(Workload::Dialogue);
    let small = hints.exchanges <= 1;
    if tools {
        if let Value::Object(fields) = &mut overrides {
            fields.push((
                "tools".into(),
                if small { vec!["life_recall"] } else { vec!["life_recall", "life_change"] }.into(),
            ));
        }
    }
    let config = Config::resolve(p.get("defaults").unwrap(), &overrides)?;
    let people: Value = vec!["merchant", "player"].into();
    let mut life = Life::accept("annex:player", "merchant", &people, definition.clone(), config.clone())?;
    let mut co = Coordinator::new("annex:player", "merchant", &people)?;
    co.bind_scene(&scene("annex", 1, 1, 1, true))?;
    co.thread(&topic())?;
    life.experience(&event(1, 1, "report", "preference", "I prefer a quiet blue wick to a hissing one.", vec![]))?;
    let mut exchanges = vec![
        obj! {"id"=>"one","turn"=>1,"user"=>"I prefer a quiet blue wick.","assistant"=>"A quiet flame suits this counter too."},
    ];
    let mut interactions = InteractionTools::new("annex:player", "merchant", &people)?;
    let promise =
        obj! {"id"=>"repair:1","speaker"=>"merchant","text"=>"I will polish the lamp after serving this customer."};
    interactions.call(
        "promise_remember",
        r#"{"promisor":"merchant","beneficiary":"player","task":"Polish the lamp.","source":"repair:1"}"#,
        &vec![promise.clone()].into(),
        1,
        8192,
        |_| false,
    )?;
    interactions.set_progress(&obj! {"id"=>1,"expected"=>0,"state"=>"accepted","source"=>promise.clone()})?;
    let mut initiative = Initiative::new(
        "annex:player",
        "merchant",
        &people,
        InitiativePolicy::resolve(p.get("initiative").unwrap(), &obj! {})?,
    )?;
    // A generated line is interrupted: only the first eight characters were played,
    // and the player confirms hearing five. No complete utterance becomes a memory.
    let generated =
        co.generate("lamps", "I will polish it after this customer.", &vec!["view"].into(), life.revision())?;
    let id = generated.get("id").unwrap().as_usize().unwrap() as u64;
    co.validate(id, life.revision())?;
    for (stage, through, recipient) in [
        ("queued", 0, Value::Null),
        ("played", 8, Value::Null),
        ("interrupted", 8, Value::Null),
        ("heard", 5, Value::str("player")),
    ] {
        co.delivery(&obj! {"id"=>id,"event"=>stage,"through"=>through,"recipient"=>recipient})?;
    }
    let partial = co.inspect();
    co.retire(id)?;
    life.experience(&event(
        2,
        3,
        "outcome",
        "returned",
        "The host confirms the borrowed lamp was returned intact.",
        vec![],
    ))?;
    let changed = life.appraise("returned_lamp", 2, life.revision())?;
    life.experience(&event(3, 4, "report", "name", "My name is Vela.", vec![]))?;
    life.experience(&event(4, 5, "correction", "name", "I said Vela, not Vira.", vec![3]))?;
    co.bind_scene(&scene("inner_workshop", 2, 2, 6, false))?;
    let candidates:Value=vec![obj!{"id"=>"polish","kind"=>"activity","priority"=>10,"reason"=>"Resume the accepted postponed repair.","requires"=>vec!["bench_free"],"resource"=>"bench","recipient"=>Value::Null,"attitudes"=>vec!["reliability"],"events"=>vec![2],"promises"=>vec![1]}].into();
    let frame = obj! {"revision"=>2,"now"=>6,"turn"=>1,"conditions"=>vec!["bench_free"],"allowed"=>vec!["polish"],"resources"=>vec!["bench"],"audience"=>vec!["merchant","player"],"emergency"=>false,"depth"=>0,"social_remaining"=>1};
    let proposed = initiative.consider(&frame, &candidates, &life, &interactions)?;
    initiative.dispatch(1, &frame, &life, &interactions, |p| p == &obj! {"action"=>"activity","target"=>"polish"})?;
    life.experience(&event(5, 7, "outcome", "polish", "The host confirms the promised lamp is polished.", vec![]))?;
    initiative.outcome(1, 5, true, &life)?;
    interactions.set_progress(&obj!{"id"=>1,"expected"=>1,"state"=>"fulfilled","source"=>obj!{"id"=>"receipt:5","speaker"=>"merchant","text"=>"The lamp was polished."}})?;
    exchanges.push(obj!{"id"=>"two","turn"=>2,"user"=>"What do you remember about my lamp?","assistant"=>"You prefer a quiet blue wick. Your lamp is polished; I cannot see the sky here."});
    let clarification = gamectx::character::dialogue::resolve_references(
        &obj! {"entities"=>vec![obj!{"id"=>"lamp:blue","label"=>"Blue lamp","kind"=>"lamp","aliases"=>vec!["blue lamp","lamp"]},obj!{"id"=>"lamp:amber","label"=>"Amber lamp","kind"=>"lamp","aliases"=>vec!["amber lamp","lamp"]}],"input"=>"that lamp","kind"=>"lamp","scope"=>"annex:player","revision"=>3,"max_age"=>3,"focus"=>Value::Null,"selection"=>Value::Null,"pronouns"=>vec!["that"]},
    )?;
    assert_eq!(clarification.get("candidates").unwrap().as_array().unwrap().len(), 2);
    let identity = obj! {"campaign"=>"lantern_vault","branch"=>"annex:a","instance"=>"ilen:1"};
    let saved = life.checkpoint(&identity)?;
    let conversation = co.checkpoint();
    let mut loaded = Life::accept("annex:player", "merchant", &people, definition, config)?;
    loaded.restore_checkpoint(&identity,&saved,&obj!{"min_revision"=>life.revision(),"max_revision"=>life.revision(),"min_token"=>5,"max_token"=>5,"max_tick"=>7})?;
    let mut reunion = Coordinator::recover(&conversation, id, 7)?;
    reunion.bind_scene(&scene("annex_reunion", 3, 3, 8, true))?;
    let dialogue = Thread::from_value(
        &obj! {"version"=>1,"scene"=>"annex_reunion","conversation"=>"lamps","participants"=>people.clone(),"exchanges"=>exchanges},
    )?;
    let caps = ContextCaps::from_hints(hints)?;
    let turn = gamectx::character::life::SpeechTurn::from_hints(
        hints,
        "known",
        false,
        "player",
        "player",
        &vec![obj! {"key"=>"player_preferred_wick","subject"=>"player","status"=>"known","value"=>"quiet blue wick"}]
            .into(),
    )?;
    let selected=reunion.select_context_for_turn("lamps",&loaded,&obj!{"required"=>vec!["view"],"optional"=>Vec::<Value>::new(),"tags"=>vec!["trade"],"input"=>"A quiet blue wick for the lamp?"},caps,&dialogue,&turn)?;
    let packet = selected.get("packet").unwrap();
    let mut character = CharacterTools::new("annex:player", "merchant", &people)?;
    character.with_life(loaded)?;
    character.bind(CharacterView::new(3))?;
    let selected_tools = if tools {
        if small {
            vec!["life_recall"]
        } else {
            vec!["life_recall", "life_change"]
        }
    } else {
        vec![]
    };
    let schemas = character.enable(&selected_tools)?;
    let rendered=character.life().unwrap().compile_speech(&obj!{"version"=>2,"rules"=>"Speak naturally using supplied facts in one short sentence; confirmed results come from the host.","tags"=>vec!["trade"],"caps"=>obj!{"facts"=>caps.facts,"facets"=>caps.facets,"attitudes"=>caps.attitudes,"episodes"=>caps.episodes,"exchanges"=>caps.exchanges},"stable_scene"=>obj!{"id"=>"annex_reunion","place"=>"The lamp counter beside the vault stair."},"state"=>obj!{"facts"=>packet.get("facts").unwrap().clone(),"episodes"=>packet.get("episodes").unwrap().clone()},"dialogue"=>packet.get("dialogue").unwrap().clone(),"input"=>"A quiet blue wick for the lamp?","tools"=>schemas,"turn"=>turn.to_value()})?;
    // An explicit heuristic demonstrates the accounting API; a real host must recount
    // the complete runner rendering with its tokenizer before sending.
    let counted =
        gamectx::packet::request::RequestBudget::new(if small { 2048 } else { 8192 }, hints.output_tokens, 256)?
            .check(&rendered, |v| {
                Ok(gamectx::packet::request::Count {
                    input: v.json_len().div_ceil(3) + 64,
                    method: gamectx::packet::request::CountMethod::Estimate,
                })
            })?;
    assert_eq!(initiative.inspect().get("model_calls"), Some(&Value::from(0)));
    Ok(
        obj! {"demo"=>"Lantern Vault: Ilen's interrupted repair and reunion","model_calls"=>0,"authored_speech_quality"=>"not model evaluation","clarification"=>clarification,"partial_delivery"=>partial,"opinion_change"=>changed,"resumed_promise"=>proposed,"context"=>selected,"request"=>rendered,"budget"=>counted.to_value(),"sensory_continuity"=>super::routines::demo(None,None)?},
    )
}
#[test]
fn focused_and_expanded_demo() {
    let small = demo(ModelSize::B3, false).unwrap();
    let large = demo(ModelSize::B70, true).unwrap();
    assert!(small.get("request").unwrap().get("tools").unwrap().as_array().unwrap().is_empty());
    assert_eq!(large.get("request").unwrap().get("tools").unwrap().as_array().unwrap().len(), 2);
}
