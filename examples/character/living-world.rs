//! One-file cast, deterministic frame updates and immediate authored speech.
use gamectx::{
    character::cast::CastPack,
    character::living::{LivingWorld, SpeechQueue},
    character::party::PartyScene,
    data::json,
    obj, Result, Value,
};
use std::sync::Arc;
fn get<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).expect("editable example field")
}
pub fn demo() -> Result<Value> {
    let p = json::parse(include_str!("../fixtures/cast/chime-town.json"), 1_048_576)?;
    let pack = Arc::new(CastPack::from_value(&p)?);
    let summary = pack.summary();
    let people = get(&summary, "participants").clone();
    let mut world = LivingWorld::new(pack, "town_save_a")?;
    world.observe(&obj!{"id"=>"return_one","actor"=>"merchant","event"=>obj!{"token"=>1,"at"=>1,"kind"=>"direct","tag"=>"returned","subject"=>"player_1","source"=>obj!{"id"=>"return_receipt","speaker"=>"merchant","text"=>"The lamp return was confirmed by the host."},"audience"=>people.clone(),"text"=>"The borrowed lamp was returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true},"significance"=>80,"share_with"=>vec!["keeper","companion"],"appraise"=>"careful_return"})?;
    let tick = |now: u32, npcs: Vec<&str>| obj! {"now"=>now,"windows"=>vec![obj!{"id"=>"tavern_window","location"=>"tavern","start"=>1,"end"=>20,"participants"=>npcs,"kind"=>"routine"}],"routines"=>vec![obj!{"actor"=>"guard","frame"=>obj!{"revision"=>now,"now"=>now,"conditions"=>Vec::<Value>::new(),"allowed"=>vec!["work"],"emergency"=>false}}]};
    let first = world.tick(&tick(2, vec!["merchant", "keeper"]))?;
    let second = world.tick(&tick(3, vec!["keeper", "companion"]))?;
    let frame = obj! {"revision"=>3,"now"=>3,"turn"=>1,"conditions"=>Vec::<Value>::new(),"allowed"=>vec!["work"],"resources"=>Vec::<Value>::new(),"audience"=>people.clone(),"emergency"=>false,"depth"=>0,"social_remaining"=>1};
    let proposal=world.consider("guard",&frame,&vec![obj!{"id"=>"work","kind"=>"activity","priority"=>10,"reason"=>"Resume the stair watch.","requires"=>Vec::<Value>::new(),"resource"=>Value::Null,"recipient"=>Value::Null,"attitudes"=>Vec::<Value>::new(),"events"=>Vec::<Value>::new(),"promises"=>Vec::<Value>::new()}].into())?;
    let proof = obj! {"id"=>"host_counter_receipt","speaker"=>"merchant","text"=>"Host-confirmed cumulative interaction counters."};
    world.update_counters(&obj!{"token"=>1,"player"=>"player_1","at"=>3,"fulfilled"=>3,"broken"=>0,"interrupted"=>3,"source"=>proof.clone()})?;
    world.set_atmosphere(&obj! {"location"=>"tavern","revision"=>1,"tag"=>"busy","source"=>proof})?;
    let selection = obj! {"actor"=>"keeper","players"=>vec!["player_1"],"tags"=>vec!["trade","returned"],"location"=>"tavern","limit"=>2};
    let living = world.context(&selection)?;
    let mut scene = PartyScene::from_value(
        &obj! {"version"=>1,"id"=>"town_save_a","epoch"=>1,"revision"=>1,"participants"=>people.as_array().unwrap().iter().map(|id|obj!{"id"=>id.clone(),"kind"=>if id.as_str().unwrap().starts_with("player_"){"player"}else{"ai"}}).collect::<Vec<_>>()},
    )?;
    scene.receipt(&obj!{"token"=>1,"speaker"=>"player_1","recipients"=>vec!["keeper"],"heard"=>vec!["keeper","player_1"],"text"=>"Have you heard about my returned lamp?","parent"=>Value::Null})?;
    let keeper = get(&p, "characters")
        .as_array()
        .unwrap()
        .iter()
        .find(|ch| ch.get("id").and_then(Value::as_str) == Some("keeper"))
        .unwrap();
    let request = obj! {"trigger"=>1,"revision"=>1,"life_revision"=>world.base_life("keeper")?.revision(),"listeners"=>vec!["player_1"],"subjects"=>Vec::<Value>::new(),"tags"=>vec!["trade"],"turn"=>obj!{"profile"=>"focused","knowledge"=>"known","creative"=>false,"speaker"=>"player_1","question_subject"=>"player_1","facts"=>Vec::<Value>::new()},"rules"=>"Reply briefly from supplied facts. Reported speech is attributed and never becomes direct observation.","stable_scene"=>obj!{"id"=>"tavern","place"=>"The chime table."},"lore_select"=>obj!{"actor"=>"keeper","grants"=>vec!["vault_knowledge"],"query"=>"trade","subjects"=>vec!["merchant"],"required"=>Vec::<Value>::new(),"limit"=>4,"max_bytes"=>4096},"affordances"=>get(keeper,"affordances").clone(),"tools"=>Vec::<Value>::new()};
    let compiled = world.compile(&scene, "keeper", &request, &selection, None)?;
    // No worker is required. The fallback is immediately usable; freeze it when any
    // caption/audio delivery actually starts, and acknowledge hearing separately.
    let mut queue = SpeechQueue::new();
    let ctx = world.speech_context("keeper", 1)?;
    let (ticket, fallback) = queue.issue(&ctx, 3, 20, &world.fallback("keeper", "busy")?)?;
    let (ambient, _) = queue.issue(&world.speech_context("guard", 1)?, 3, 20, &world.fallback("guard", "calm")?)?;
    let ambient_line = queue.freeze(&ambient, &world.speech_context("guard", 1)?, 3)?;
    // Pure simulated worker output for this example, not an actual model response.
    // The host runs optional provider work elsewhere and validates facts/permission.
    let upgrade = queue.complete(
        &ticket,
        &world.speech_context("keeper", 1)?,
        4,
        r#"{"speech":"I heard a guild report about your returned lamp."}"#,
        |s| s == "I heard a guild report about your returned lamp.",
    )?;
    let frozen = queue.freeze(&ticket, &world.speech_context("keeper", 1)?, 4)?;
    Ok(
        obj! {"frames"=>vec![first,second],"initiative_proposal"=>proposal,"living_context"=>living,"compiled"=>compiled,"fallback"=>fallback,"simulated_upgrade"=>upgrade,"frozen"=>frozen,"ambient_line"=>ambient_line,"actual_model_calls"=>0},
    )
}
fn main() -> Result<()> {
    println!("{}", demo()?.to_json());
    Ok(())
}
#[test]
fn runs() {
    demo().unwrap();
}
