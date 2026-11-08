//! Public API checks for the configurable roleplay engine (builder, toggles, atomicity).
use gamectx::character::roleplay::engine::{EngineConfig, Feature, RoleplayEngine};
use gamectx::knowledge::props::Props;
use gamectx::{obj, Value};

fn scene() -> Value {
    obj! {"id"=>"forge","label"=>"the forge","place"=>"workplace","atmosphere"=>"calm","visit"=>1,
    "props"=>vec![obj!{"id"=>"anvil","note"=>Value::Null}]}
}
fn turn(text: &str, token: u64) -> Value {
    obj! {"listener"=>"player_1","text"=>text,"at"=>token,"token"=>token,"scene"=>scene(),
    "choices"=>vec![obj!{"action"=>"use","target"=>"prop:anvil"}]}
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}
fn props() -> Props {
    Props::parse(
        r#"{"version":1,"props":[{"id":"anvil","label":"the old anvil","aliases":["anvil"],
        "description":"A scarred iron anvil, pitted from years of work.","interactions":[
        {"id":"strike","phrases":["strikes the anvil","strike the anvil"],
        "decision":{"action":"use","target":"prop:anvil"},"narration":"strikes the anvil"}]}]}"#,
    )
    .unwrap()
}

#[test]
fn builder_round_trips_through_json() {
    let config = EngineConfig::new("smith", "Oren")
        .unwrap()
        .with_feature(Feature::Greetings, false)
        .unwrap()
        .with_setting(Feature::Disclosures, "max_preferences", 5)
        .unwrap()
        .with_recent(4)
        .unwrap()
        .with_props(&props())
        .unwrap()
        .with_line(&obj! {"id"=>"thanks","intent"=>"thanks","variants"=>vec!["Anytime."]})
        .unwrap();
    assert!(!config.enabled(Feature::Greetings));
    assert_eq!(config.setting(Feature::Disclosures, "max_preferences"), Some(5));
    let again = EngineConfig::from_value(&config.to_value()).unwrap();
    assert_eq!(again.to_value(), config.to_value());
    assert!(config.with_setting(Feature::Greetings, "max_chars", 600).is_err());
    assert!(config.with_setting(Feature::Chunks, "max_chars", 100).is_err());
    assert!(config.with_pack("klingon").is_err());
}

#[test]
fn props_describe_and_propose_but_never_execute() {
    let config = EngineConfig::new("smith", "Oren").unwrap().with_props(&props()).unwrap();
    let mut engine = RoleplayEngine::new(config);
    let out = engine.turn(&turn("Look at the anvil.", 1)).unwrap();
    assert_eq!(s(&out, "text"), "A scarred iron anvil, pitted from years of work.");
    let out = engine.turn(&turn("*strikes the anvil*", 2)).unwrap();
    assert_eq!((s(&out, "kind"), s(&out, "interaction")), ("action", "strike"));
    let mut blocked = turn("*strikes the anvil*", 3);
    if let Value::Object(rows) = &mut blocked {
        rows.retain(|(k, _)| k != "choices");
    }
    let out = engine.turn(&blocked).unwrap();
    assert_eq!(s(&out, "kind"), "clarify");
}

#[test]
fn disabled_stages_fall_through_and_failed_turns_change_nothing() {
    // The scene lists an anvil, so an engine without that prop registered rejects the turn.
    let bare = EngineConfig::new("smith", "Oren").unwrap();
    assert!(RoleplayEngine::new(bare.clone()).turn(&turn("Thanks", 1)).is_err());
    let config = bare.with_props(&props()).unwrap();
    let mut engine = RoleplayEngine::new(config.with_feature(Feature::Social, false).unwrap());
    let out = engine.turn(&turn("Thanks", 1)).unwrap();
    assert_eq!(s(&out, "route"), "model");
    let before = engine.checkpoint();
    assert!(engine.turn(&turn("Thanks", 0)).is_err());
    assert_eq!(engine.checkpoint(), before);
    let restored = RoleplayEngine::restore(engine.config().clone(), &before).unwrap();
    assert_eq!(restored.checkpoint(), before);
}

#[test]
fn guide_examples_run() -> gamectx::Result<()> {
    // docs/characters/ROLEPLAY_ENGINE.md, "Configure it" (JSON and builder).
    let json = r#"{"version":1,"character":{"id":"baker","name":"Mira"},
     "features":{"greetings":{"enabled":true,"long_gap_turns":20},"model":{"enabled":false},
                 "props":{"registry":{"version":1,"props":[]}}},
     "variety":{"recent":2},
     "pack":"en",
     "phrases":{"ask_name":{"exact":["who are you","what are you called"]}},
     "lines":[{"id":"thanks","intent":"thanks","variants":["Any time, friend."]}]}"#;
    let from_json = EngineConfig::parse(json)?;
    assert_eq!(from_json.setting(Feature::Greetings, "long_gap_turns"), Some(20));
    let config = EngineConfig::new("baker", "Mira")?
        .with_feature(Feature::Model, false)?
        .with_setting(Feature::Greetings, "long_gap_turns", 20)?
        .with_recent(3)?;
    let _engine = RoleplayEngine::new(config);
    let mut engine = RoleplayEngine::new(from_json);
    let scene = obj! {"id"=>"bakery","label"=>"the bakery","place"=>"workplace","atmosphere"=>"calm","visit"=>1};
    let out =
        engine.turn(&obj! {"listener"=>"player_1","text"=>"Who are you?","at"=>1,"token"=>1,"scene"=>scene.clone()})?;
    assert_eq!(s(&out, "text"), "My name is Mira.");
    let out = engine.turn(&obj! {"listener"=>"player_1","text"=>"Thanks!","at"=>2,"token"=>2,"scene"=>scene})?;
    assert_eq!(s(&out, "text"), "Any time, friend.");
    Ok(())
}

/// The sheet and codex JSON in docs/characters/ROLEPLAY_ENGINE.md, read from the file so
/// the guide cannot drift, plus its "Saving" snippet.
#[test]
fn guide_definitions_and_save_examples_run() -> gamectx::Result<()> {
    use gamectx::character::sheet::CharacterSheet;
    use gamectx::kit::save::{GameData, SaveFile, SaveState};
    use gamectx::knowledge::codex::Codex;
    let guide = include_str!("../docs/characters/ROLEPLAY_ENGINE.md");
    let block = |heading: &str| {
        let rest = &guide[guide.find(heading).unwrap()..];
        let start = rest.find("```json\n").unwrap() + 8;
        rest[start..start + rest[start..].find("```").unwrap()].to_owned()
    };
    let codex = Codex::parse(&block("## World codex"))?;
    let sheet = CharacterSheet::parse(&block("## Character sheet"), &codex)?;
    assert_eq!(sheet.role(), Some("the town baker"));
    let config = EngineConfig::new("baker", "Mira")?.with_codex(&codex)?.with_sheet(&sheet)?;
    let mut engine = RoleplayEngine::new(config);
    let scene = obj! {"id"=>"bakery","label"=>"the bakery","place"=>"workplace","atmosphere"=>"calm","visit"=>1};
    let ask = |e: &mut RoleplayEngine, text: &str, token: u64, extra: Vec<(&str, Value)>| {
        let mut t = obj! {"listener"=>"player_1","text"=>text,"at"=>token,"token"=>token,"scene"=>scene.clone(),
        "entrances"=>vec![obj! {"id"=>"front_door","status"=>"open"}]};
        if let Value::Object(f) = &mut t {
            f.extend(extra.into_iter().map(|(k, v)| (k.to_owned(), v)));
        }
        e.turn(&t).map(|o| s(&o, "text").to_owned())
    };
    assert_eq!(
        ask(&mut engine, "Do you owe anyone money?", 1, vec![])?,
        "My accounts are in perfect order, thank you."
    );
    assert_eq!(
        ask(&mut engine, "Do you owe anyone money?", 2, vec![("trust", Value::int(60))])?,
        "Between us, I owe the mill for three months of flour."
    );
    assert_eq!(
        ask(&mut engine, "What do you hear?", 3, vec![])?,
        "Through the green front door, I can hear traders calling out prices."
    );
    assert_eq!(
        ask(&mut engine, "What do you want?", 4, vec![("state", obj! {"ring_lost"=>true})])?,
        "Honestly, I just want my ring back."
    );

    // "Saving: game data and saves".
    let config = r#"{"version":1,"character":{"id":"baker","name":"Mira"}}"#;
    let mut game = GameData::new("Ember Bakery")?;
    game.insert("characters/baker/engine", "engine-config", &gamectx::data::json::parse(config, 4096)?)?;
    let engine = RoleplayEngine::new(game.engine_config("baker")?);
    let mut slot = SaveState::new("slot-1", 40)?.with_label("Before the cellar")?;
    slot.put_engine(&engine)?;
    let mut save = SaveFile::new(Some(&game));
    save.put(slot, Some(&game))?;
    save.fork("slot-1", "autosave", 41)?;
    let text = save.to_json();
    let loaded = SaveFile::parse(&text)?;
    assert!(loaded.check(&game)?);
    let mira = loaded.state("autosave").unwrap().engine("baker", &game)?;
    assert_eq!(mira.checkpoint(), engine.checkpoint());
    Ok(())
}

/// Stage 2.2: the `model` outcome carries a request; `reply` validates what comes back.
#[test]
fn model_requests_and_replies_round_trip() -> gamectx::Result<()> {
    let mut engine = RoleplayEngine::new(EngineConfig::new("baker", "Mira")?);
    let scene = obj! {"id"=>"bakery","label"=>"the bakery","place"=>"workplace","atmosphere"=>"calm","visit"=>1};
    let out = engine.turn(
        &obj! {"listener"=>"player_1","text"=>"What do you think of the mayor?","at"=>1,"token"=>7,"scene"=>scene},
    )?;
    assert_eq!(s(&out, "route"), "model");
    let request = out.get("request").unwrap();
    let messages = request.get("messages").unwrap().as_array().unwrap();
    assert_eq!(messages.len(), 4);
    assert!(messages[0].get("content").unwrap().as_str().unwrap().starts_with("You are Mira"));
    assert_eq!(messages[3].get("content").unwrap().as_str(), Some("What do you think of the mayor?"));
    let bad = engine.reply(&obj! {"token"=>7,"listener"=>"player_1","text"=>"*bows* As an AI..."})?;
    assert_eq!(s(&bad, "kind"), "rejected");
    let good = engine.reply(&obj! {"token"=>7,"listener"=>"player_1","text"=>r#"{"speech":"He keeps the lamps lit, and that is enough for me."}"#})?;
    assert_eq!(s(&good, "text"), "He keeps the lamps lit, and that is enough for me.");
    let text = s(&good, "text").to_owned();
    let receipt = obj! {"token"=>7,"listener"=>"player_1","text"=>text.as_str(),"heard"=>text.chars().count()};
    assert_eq!(engine.delivered(&receipt)?.get("recorded"), Some(&Value::Bool(true)));
    assert!(engine.reply(&obj! {"token"=>7,"listener"=>"player_1","text"=>"{}"}).is_err(), "the request is closed");
    Ok(())
}
