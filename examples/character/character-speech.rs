//! Turn-aware focused speech context without a model, tokenizer or speech dependency.
use gamectx::{
    character::life::{assess_sentence, Config, ContextCaps, Definition, Life, Setting},
    data::json,
    deploy::deployment::{ModelSize, Workload},
    obj, Result,
};
use std::sync::Arc;
fn main() -> Result<()> {
    let setting = Setting::from_value(&json::parse(include_str!("../fixtures/characters/setting.json"), 65536)?)?;
    let definition = Arc::new(Definition::draft(
        &json::parse(include_str!("../fixtures/characters/merchant-life.json"), 65536)?,
        &setting,
    )?);
    let life = Life::accept(
        "counter",
        "merchant",
        &vec!["merchant", "player"].into(),
        definition,
        Config::resolve(&obj! {"enabled"=>true,"balanced"=>true}, &obj! {})?,
    )?;
    let caps = ContextCaps::from_hints(ModelSize::B3.workload_hints(Workload::Dialogue))?;
    let request=life.compile_speech(&obj!{"version"=>2,"rules"=>"Speak as the merchant using supplied facts in one short sentence.","tags"=>vec!["trade"],"caps"=>obj!{"facts"=>caps.facts,"facets"=>caps.facets,"attitudes"=>caps.attitudes,"episodes"=>caps.episodes,"exchanges"=>caps.exchanges},"stable_scene"=>obj!{"id"=>"counter"},"state"=>obj!{},"dialogue"=>Vec::<gamectx::Value>::new(),"input"=>"Where was I born?","tools"=>Vec::<gamectx::Value>::new(),"turn"=>obj!{"profile"=>"focused","knowledge"=>"unknown","creative"=>false,"speaker"=>"player","question_subject"=>"player","facts"=>vec![obj!{"key"=>"player_birthplace","subject"=>"player","status"=>"unknown","value"=>gamectx::Value::Null}]}})?;
    assert!(!request.to_json().contains("imagined hypotheticals"));
    assert!(request.to_json().contains("unknown_to_merchant"));
    // Host validates the original reply; this check never trims or repairs generated text.
    let assessment = assess_sentence("I don't know your birthplace.", 240)?.to_value();
    assert_eq!(assessment.get("boundary_complete"), Some(&true.into()));
    println!("{}", request.to_json());
    Ok(())
}
#[test]
fn runs() {
    main().unwrap();
}
