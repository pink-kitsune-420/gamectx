//! Game data files and multi-state save files: packing, forking, linking and atomic edits.
use gamectx::character::roleplay::engine::{EngineConfig, RoleplayEngine};
use gamectx::data::json::parse;
use gamectx::kit::save::{GameData, SaveFile, SaveState};
use gamectx::knowledge::memory::{Limits, Memory};
use gamectx::{obj, Value};

fn input(case: &str) -> Value {
    let path = format!("{}/conformance/save-file/{case}/input.json", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap(), 1 << 24).unwrap()
}

/// The data file the `slots_and_fork` conformance case packs, built through the public API.
fn game() -> GameData {
    let ops = input("slots_and_fork");
    let pack = ops.get("args").unwrap().get("ops").unwrap().as_array().unwrap()[0].get("args").unwrap().clone();
    let mut g = GameData::new(pack.get("title").unwrap().as_str().unwrap()).unwrap();
    // Sheets need the codex, so insert in reverse to prove insert order does not matter
    // once everything is present: the codex goes in first here.
    let sections = pack.get("sections").unwrap().as_array().unwrap();
    for s in sections
        .iter()
        .filter(|s| s.get("kind").unwrap().as_str() != Some("sheet"))
        .chain(sections.iter().filter(|s| s.get("kind").unwrap().as_str() == Some("sheet")))
    {
        g.insert(
            s.get("path").unwrap().as_str().unwrap(),
            s.get("kind").unwrap().as_str().unwrap(),
            s.get("data").unwrap(),
        )
        .unwrap();
    }
    g
}

#[test]
fn api_build_matches_conformance_digest() {
    let expected = parse(include_str!("../conformance/save-file/slots_and_fork/expected.json"), 1 << 24).unwrap();
    let packed = expected.get("results").unwrap().as_array().unwrap()[0].get("ok").unwrap().clone();
    let g = game();
    assert_eq!(g.to_value(), packed);
    assert_eq!(GameData::parse(&g.to_json()).unwrap(), g);
}

#[test]
fn many_states_round_trip_and_restore_live_engines() {
    let g = game();
    let mut engine = RoleplayEngine::new(g.engine_config("baker").unwrap());
    let scene = obj! {"id"=>"bakery","label"=>"the bakery","place"=>"workplace","atmosphere"=>"calm","visit"=>1};
    let out = engine
        .turn(&obj! {"listener"=>"player_1","text"=>"My name is Arin.","at"=>1,"token"=>1,"scene"=>scene.clone()})
        .unwrap();
    let text = out.get("text").unwrap().clone();
    engine
        .delivered(&obj! {"token"=>1,"listener"=>"player_1","text"=>text.clone(),"heard"=>text.as_str().unwrap().len()})
        .unwrap();

    let mut memory = Memory::new();
    memory.remember(3, "Arin bought rye bread.", &["trade"], 6).unwrap();

    let mut save = SaveFile::new(Some(&g));
    let mut slot = SaveState::new("slot-1", 10).unwrap().with_label("Before the cellar").unwrap();
    slot.put_engine(&engine).unwrap();
    slot.insert("characters/baker/memory", "memory", &memory.to_value()).unwrap();
    save.put(slot, Some(&g)).unwrap();
    save.fork("slot-1", "autosave", 11).unwrap();
    assert!(save.fork("slot-1", "autosave", 12).is_err(), "names are unique");

    let loaded = SaveFile::parse(&save.to_json()).unwrap();
    assert_eq!(loaded, save);
    assert!(loaded.check(&g).unwrap());
    let names: Vec<_> = loaded.states().iter().map(|s| s.name()).collect();
    assert_eq!(names, ["autosave", "slot-1"]);
    let fork = loaded.state("autosave").unwrap();
    assert_eq!(fork.parent(), Some("slot-1"));
    assert_eq!(fork.sections(), loaded.state("slot-1").unwrap().sections());
    assert_eq!(fork.engine("baker", &g).unwrap().checkpoint(), engine.checkpoint());
    let restored = Memory::from_value(fork.get("characters/baker/memory").unwrap().data(), Limits::default()).unwrap();
    assert_eq!(restored.to_value(), memory.to_value());

    // The restored engine remembers the player's name in either state.
    let mut again = fork.engine("baker", &g).unwrap();
    let out = again
        .turn(&obj! {"listener"=>"player_1","text"=>"What is my name?","at"=>2,"token"=>2,"scene"=>scene})
        .unwrap();
    assert_eq!(out.get("text").unwrap().as_str(), Some("You told me your name is Arin."));
}

#[test]
fn changed_game_data_is_reported_and_bad_edits_change_nothing() {
    let g = game();
    let mut save = SaveFile::new(Some(&g));
    save.put(SaveState::new("slot-1", 1).unwrap(), None).unwrap();
    let mut changed = g.clone();
    changed.insert("host/patch", "host", &obj! {"note"=>"balance pass"}).unwrap();
    assert_ne!(changed.digest(), g.digest());
    assert!(!save.check(&changed).unwrap(), "a save made against other game data says so");

    // Removing the codex would break the sheet that names a codex faction: refused, unchanged.
    let before = g.clone();
    let mut g2 = g.clone();
    assert!(g2.remove("world/codex").is_err());
    assert_eq!(g2, before);
    // An engine config must belong to the character its path names.
    let other = EngineConfig::parse(r#"{"version":1,"character":{"id":"miller","name":"Ilen"}}"#).unwrap();
    assert!(g2.insert("characters/baker/engine", "engine-config", &other.to_value()).is_err());
    assert_eq!(g2, before);
    // Kinds are separate per file: no runtime state in game data, no definitions in saves.
    assert!(g2.insert("world/x", "engine", &obj! {}).is_err());
    assert!(SaveState::new("s", 0).unwrap().insert("world/codex", "codex", &obj! {"version"=>1}).is_err());
}

#[test]
fn tampered_files_are_rejected() {
    let g = game();
    let mut save = SaveFile::new(Some(&g));
    let mut slot = SaveState::new("slot-1", 1).unwrap();
    slot.insert("world/flags", "host", &obj! {"ring_lost"=>true}).unwrap();
    save.put(slot, None).unwrap();
    let text = save.to_json();
    assert!(SaveFile::parse(&text.replace("\"ring_lost\":true", "\"ring_lost\":false")).is_err());
    assert!(GameData::parse(&g.to_json().replace("Mira", "Mara")).is_err());
}
