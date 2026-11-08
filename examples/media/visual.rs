//! Editable art contract, live appraisal, approved fallback and host repair. No generator.
use gamectx::{
    character::life::{Config, Definition, Life, Setting},
    data::json,
    media::visual::{Activity, ArtBible, AssetRegistry, RegistryConfig, VisualSpec},
    obj, Result, Value,
};
use std::sync::Arc;
fn main() -> Result<()> {
    let bible = ArtBible::parse(include_str!("../fixtures/visual/chime-art.json"))?;
    let setting = Setting::from_value(&json::parse(include_str!("../fixtures/characters/setting.json"), 65536)?)?;
    let definition = Arc::new(Definition::draft(
        &json::parse(include_str!("../fixtures/characters/merchant-life.json"), 65536)?,
        &setting,
    )?);
    let people: Value = vec!["merchant", "player"].into();
    let life =
        Life::accept("counter", "merchant", &people, definition, Config::resolve(&obj! {"enabled"=>true}, &obj! {})?)?;
    let scene = obj! {"id"=>"counter","revision"=>1,"epoch"=>1,"at"=>10,"participants"=>people,"facts"=>vec![obj!{"id"=>"shutters_closed","revision"=>1,"observed_at"=>10,"source"=>obj!{"id"=>"sight","speaker"=>"player","text"=>"Shutters are closed."},"kind"=>"perceived","audience"=>vec!["merchant","player"],"text"=>"Closed shutters."}],"affordances"=>Vec::<Value>::new()};
    let frame = obj! {"scope"=>"counter","observer"=>"player","entity"=>"merchant","listener"=>"player","scene"=>scene,"audience"=>vec!["player"],"life_revision"=>life.revision(),"activity_revision"=>Value::Null};
    let spec = VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle())?;
    let mut assets = AssetRegistry::new(RegistryConfig::from_value(
        &obj! {"enabled"=>true,"capacity"=>16,"queue_limit"=>2,"epoch"=>"example_run"},
    )?);
    let first = assets.request(&spec)?;
    assert!(first.get("generation") != Some(&Value::Null));
    // A host-owned, reviewed reference. This example does not read image bytes.
    let digest = "a".repeat(64);
    assets.approve(spec.key(), "asset://chime/reviewed.png", &digest)?;
    let reused = assets.request(&spec)?;
    assert_eq!(reused.get("generation"), Some(&Value::Null));
    let patch = obj! {"replace_tokens"=>Vec::<Value>::new(),"add_tokens"=>Vec::<Value>::new(),"negative_tokens"=>vec!["wide-brimmed hat"],"forbid_props"=>vec!["lamp"],"seed_delta"=>1,"asset"=>obj!{"asset_uri"=>"asset://chime/hand-edited.png","digest"=>"b".repeat(64)}};
    let repaired = assets.repair(spec.key(), &patch)?;
    let pinned = assets.request(&spec)?;
    assert_eq!(pinned.get("state"), Some(&Value::str("pinned")));
    assert_eq!(pinned.get("generation"), Some(&Value::Null));
    println!("{}",obj!{"compiled"=>spec.to_value(),"rendered"=>spec.render()?,"repair"=>repaired,"selection"=>pinned,"actual_model_calls"=>0}.to_json());
    Ok(())
}
#[test]
fn runs() {
    main().unwrap();
}
