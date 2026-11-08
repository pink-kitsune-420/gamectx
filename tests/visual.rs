use gamectx::{
    character::life::{Config, Definition, Life, Setting},
    data::json,
    media::visual::{self, Activity, ArtBible, AssetRegistry, RegistryConfig, VisualSpec},
    obj, Value,
};
use std::sync::Arc;
fn parse(s: &str) -> Value {
    json::parse(s, 1_500_000).unwrap()
}
fn set(v: &mut Value, k: &str, x: Value) {
    let Value::Object(xs) = v else { panic!() };
    if let Some(entry) = xs.iter_mut().find(|(key, _)| key == k) {
        entry.1 = x;
    } else {
        xs.push((k.to_string(), x));
    }
}
fn fixture(name: &str) -> Value {
    parse(
        &std::fs::read_to_string(format!("{}/conformance/visual/{name}/input.json", env!("CARGO_MANIFEST_DIR")))
            .unwrap(),
    )
    .get("args")
    .unwrap()
    .clone()
}
fn registry(capacity: u32, epoch: &str) -> AssetRegistry {
    AssetRegistry::new(
        RegistryConfig::from_value(&obj! {"enabled"=>true,"capacity"=>capacity,"queue_limit"=>2,"epoch"=>epoch})
            .unwrap(),
    )
}
fn patch(delta: u32) -> Value {
    obj! {"replace_tokens"=>Vec::<Value>::new(),"add_tokens"=>Vec::<Value>::new(),"negative_tokens"=>Vec::<Value>::new(),"forbid_props"=>Vec::<Value>::new(),"seed_delta"=>delta,"asset"=>Value::Null}
}
fn spec(name: &str) -> VisualSpec {
    visual::compile_fixture(&fixture(name)).unwrap()
}
#[test]
fn live_appraisal_crosses_slot_without_leaking_unobserved_text() {
    let a = fixture("neutral_portrait");
    let l = a.get("life").unwrap();
    let setting = Setting::from_value(l.get("setting").unwrap()).unwrap();
    let mut definition = l.get("definition").unwrap().clone();
    let mut attitudes = definition.get("attitudes").unwrap().as_array().unwrap().to_vec();
    set(&mut attitudes[0], "level", 10.into());
    set(&mut definition, "attitudes", attitudes.into());
    let mut life = Life::accept(
        "counter",
        "merchant",
        &vec!["merchant", "player"].into(),
        Arc::new(Definition::draft(&definition, &setting).unwrap()),
        Config::resolve(&obj! {"enabled"=>true}, &obj! {}).unwrap(),
    )
    .unwrap();
    let bible = ArtBible::from_value(a.get("bible").unwrap()).unwrap();
    let mut frame = a.get("frame").unwrap().clone();
    let before = VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle()).unwrap();
    life.experience(&obj!{"token"=>1,"at"=>1,"kind"=>"outcome","tag"=>"returned","subject"=>"player","source"=>obj!{"id"=>"receipt:1","speaker"=>"merchant","text"=>"The lamp was returned."},"audience"=>vec!["merchant","player"],"text"=>"Returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true}).unwrap();
    life.appraise("returned_lamp", 1, 1).unwrap();
    assert!(VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle()).is_err());
    set(&mut frame, "life_revision", life.revision().into());
    let after = VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle()).unwrap();
    assert_ne!(before.key(), after.key());
    assert_eq!(after.to_value().get("spec").unwrap().get("expression").unwrap().get("slot"), Some(&Value::str("warm")));
    assert_eq!(before.render().unwrap().get("seed"), after.render().unwrap().get("seed"));
    let mut registry = registry(8, "latest_state");
    registry.request(&after).unwrap();
    let repaired = registry.repair(after.key(), &patch(1)).unwrap();
    life.experience(&obj!{"token"=>2,"at"=>3,"kind"=>"outcome","tag"=>"returned","subject"=>"player","source"=>obj!{"id"=>"receipt:2","speaker"=>"merchant","text"=>"Another lamp returned."},"audience"=>vec!["merchant","player"],"text"=>"Returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true}).unwrap();
    life.appraise("returned_lamp", 2, 3).unwrap();
    set(&mut frame, "life_revision", life.revision().into());
    let latest = VisualSpec::compile(&bible, &frame, Some(&life), &Activity::idle()).unwrap();
    assert_eq!(latest.key(), after.key());
    registry.request(&latest).unwrap();
    let record = registry.inspect(repaired.get("key").unwrap().as_str().unwrap()).unwrap();
    assert_eq!(record.get("inspection").unwrap().get("state").unwrap().get("attitude_level"), Some(&20.into()));
}
#[test]
fn render_uses_only_selected_authored_cues() {
    let s = spec("closed_shutters_no_global_sun");
    let text = s.render().unwrap().to_json();
    assert!(text.contains("closed wooden shutters"));
    assert!(!text.contains("sunlight"));
    assert!(!text.contains("Raw world"));
    let empty = spec("guarded_portrait");
    let global = spec("report_omitted");
    assert_eq!(empty.key(), global.key());
    let s = spec("perceived_prop");
    let mut r = registry(8, "one");
    r.request(&s).unwrap();
    let mut p = patch(0);
    set(&mut p, "forbid_props", vec!["lamp"].into());
    let repaired = r.repair(s.key(), &p).unwrap();
    let d = r.inspect(repaired.get("key").unwrap().as_str().unwrap()).unwrap();
    let data = d.get("spec").unwrap();
    assert_eq!(data.get("environment"), Some(&Vec::<Value>::new().into()));
    assert_eq!(data.get("visible_props"), Some(&Vec::<Value>::new().into()));
    assert!(data.get("negative_tokens").unwrap().to_json().contains("no lamp"));
}
#[test]
fn candidate_requires_approval_and_ticket_is_consumed() {
    let s = spec("neutral_portrait");
    let mut r = registry(8, "one");
    let selection = r.request(&s).unwrap();
    let ticket = selection.get("generation").unwrap().get("ticket").unwrap();
    let sha = visual::content_digest(b"host image bytes").unwrap();
    r.submit(s.key(), ticket, "asset://draft.png", &sha).unwrap();
    assert!(r.submit(s.key(), ticket, "asset://draft.png", &sha).is_err());
    assert_eq!(r.request(&s).unwrap().get("asset"), Some(&Value::Null));
    assert!(r.approve(s.key(), "asset://different.png", &sha).is_err());
    r.approve(s.key(), "asset://draft.png", &sha).unwrap();
    let reused = r.request(&s).unwrap();
    assert_eq!(reused.get("exact"), Some(&true.into()));
    assert_eq!(reused.get("generation"), Some(&Value::Null));
}
#[test]
fn repair_capacity_failure_preserves_current_worker() {
    let s = spec("neutral_portrait");
    let mut r = registry(1, "one");
    let first = r.request(&s).unwrap();
    let before = r.inspect(s.key()).unwrap();
    assert!(r.repair(s.key(), &patch(1)).is_err());
    assert_eq!(before, r.inspect(s.key()).unwrap());
    r.submit(s.key(), first.get("generation").unwrap().get("ticket").unwrap(), "asset://ok.png", &"a".repeat(64))
        .unwrap();
}
#[test]
fn epoch_and_repair_cycles_reject_stale_workers() {
    let s = spec("neutral_portrait");
    let mut r = registry(8, "one");
    let initial = r.request(&s).unwrap();
    let stale = initial.get("generation").unwrap().get("ticket").unwrap();
    let b = r.repair(s.key(), &patch(1)).unwrap();
    let b = b.get("key").unwrap().as_str().unwrap();
    r.request(&s).unwrap();
    r.repair(b, &patch(u32::MAX)).unwrap();
    let latest = r.request(&s).unwrap();
    let current = latest.get("generation").unwrap().get("ticket").unwrap();
    assert_ne!(stale, current);
    assert!(r.submit(s.key(), stale, "asset://stale.png", &"a".repeat(64)).is_err());
    r.submit(s.key(), current, "asset://current.png", &"a".repeat(64)).unwrap();
    let mut reboot = registry(8, "two");
    reboot.request(&s).unwrap();
    assert!(reboot.submit(s.key(), stale, "asset://stale.png", &"a".repeat(64)).is_err());
}
#[test]
fn active_routine_and_started_receipt_are_distinct_from_proposals() {
    use gamectx::character::routine::{Config, Frame, Routine, Runtime};
    let a = parse(include_str!("../conformance/routines/interruption_resume/input.json"));
    let a = a.get("args").unwrap();
    let config = Config::resolve(&obj! {"routines"=>true}, &obj! {}).unwrap();
    let routine = Arc::new(Routine::from_value(a.get("definition").unwrap()).unwrap());
    let mut r =
        Runtime::new("counter", "merchant", &vec!["merchant", "player"].into(), "cycle:37", config, Some(routine))
            .unwrap();
    r.update(Frame::from_value(&obj!{"revision"=>1,"now"=>0,"conditions"=>vec!["route"],"allowed"=>vec!["work","walk"],"emergency"=>false}).unwrap()).unwrap();
    let activity = Activity::from_routine(&r).unwrap();
    assert_eq!(activity.to_value().get("id"), Some(&Value::str("work")));
    let ready = obj! {"status"=>"ready","proposal"=>obj!{"id"=>"p1","revision"=>1,"candidate"=>obj!{"id"=>"walk","kind"=>"activity"}}};
    let mut receipt = obj! {"actor"=>"merchant","proposal"=>"p1","revision"=>1,"state"=>"proposed"};
    assert!(Activity::from_initiative("counter", "merchant", &ready, &receipt).is_err());
    set(&mut receipt, "state", "started".into());
    let started = Activity::from_initiative("counter", "merchant", &ready, &receipt).unwrap();
    assert_eq!(started.to_value().get("source"), Some(&Value::str("initiative")));
    set(&mut receipt, "revision", 2.into());
    assert!(Activity::from_initiative("counter", "merchant", &ready, &receipt).is_err());
}
#[test]
fn locked_style_and_malformed_memory_values_fail_closed() {
    let mut a = fixture("neutral_portrait");
    let mut b = a.get("bible").unwrap().clone();
    set(&mut b, "width", Value::Number("1e999".into()));
    set(&mut a, "bible", b);
    assert!(visual::compile_fixture(&a).is_err());
    let s = spec("neutral_portrait");
    let mut r = registry(8, "one");
    r.request(&s).unwrap();
    let before = r.inspect(s.key()).unwrap();
    let mut p = patch(0);
    set(
        &mut p,
        "replace_tokens",
        vec![obj! {"from"=>"Original flat-color ink illustration","to"=>"different style"}].into(),
    );
    assert!(r.repair(s.key(), &p).is_err());
    assert_eq!(before, r.inspect(s.key()).unwrap());
}
#[test]
fn disabled_registry_never_queues_and_uri_is_not_loaded() {
    let mut r = AssetRegistry::new(RegistryConfig::default());
    let s = spec("neutral_portrait");
    let first = r.request(&s).unwrap();
    assert_eq!(first.get("generation"), Some(&Value::Null));
    assert_eq!(first.get("reason"), Some(&Value::str("disabled")));
    let asset = visual::AssetRef::from_bytes("asset://owned.png", b"owned bytes").unwrap();
    assert_eq!(asset.to_value().get("digest"), Some(&visual::content_digest(b"owned bytes").unwrap().into()));
}

#[test]
fn art_bible_living_embodiment_and_physical_matter() {
    let a = fixture("neutral_portrait");
    let mut b = a.get("bible").unwrap().clone();

    // Attach living character features and clothing to anchor
    let mut anchors = b.get("anchors").unwrap().as_array().unwrap().to_vec();
    let mut merchant_anchor = anchors[0].clone();
    set(
        &mut merchant_anchor,
        "features",
        obj! {
            "eyes" => "amber",
            "hair" => "raven black",
            "skin" => "pale",
            "scars" => vec!["thin scar on left cheek"],
            "build" => "lean",
            "height" => Value::Number("1.75".into())
        },
    );
    set(
        &mut merchant_anchor,
        "clothing",
        vec!["heavy woolen cloak", "tailored blue tunic", "sturdy leather boots"].into(),
    );
    set(&mut merchant_anchor, "material", "flesh and bone".into());
    set(&mut merchant_anchor, "color", "copper-tinted".into());
    set(&mut merchant_anchor, "texture", "weathered".into());
    set(&mut merchant_anchor, "mass", Value::Number("68.5".into()));
    set(&mut merchant_anchor, "state", "solid".into());
    set(&mut merchant_anchor, "dimensions", vec![0.4, 0.3, 1.75].into());
    set(&mut merchant_anchor, "tags", vec!["npc", "merchant", "artisan"].into());
    set(&mut merchant_anchor, "attributes", obj! {"guild" => "lantern_keepers", "rank" => 3});
    anchors[0] = merchant_anchor;

    // Attach scenery depth to scene anchor
    let mut counter_anchor = anchors[1].clone();
    set(&mut counter_anchor, "floor", "cobblestone".into());
    set(&mut counter_anchor, "ceiling", "vaulted oak beams".into());
    set(&mut counter_anchor, "exterior", false.into());
    set(&mut counter_anchor, "lighting", "warm lantern light".into());
    set(&mut counter_anchor, "atmosphere", "cozy subterranean vault".into());
    anchors[1] = counter_anchor;

    set(&mut b, "anchors", anchors.into());
    set(&mut b, "lighting", "soft ambient glow".into());
    set(&mut b, "atmosphere", "mysterious vault".into());
    set(&mut b, "tags", vec!["fantasy", "roleplay"].into());

    let bible = ArtBible::from_value(&b).unwrap();
    assert_eq!(bible.id().unwrap(), "chime_art");
    assert_eq!(bible.style().unwrap(), "Original flat-color ink illustration");
    assert_eq!(bible.format().unwrap(), "portrait");

    let merchant = bible.anchor("merchant").unwrap();
    assert!(merchant.get("features").is_some());
    assert!(merchant.get("clothing").is_some());
    assert_eq!(merchant.get("material"), Some(&Value::str("flesh and bone")));

    // Direct anchor describe with default describer
    let desc = bible.describe_anchor("merchant").unwrap();
    let prompt = desc.get("prompt").unwrap().as_str().unwrap();
    assert!(prompt.contains("amber eyes"));
    assert!(prompt.contains("raven black hair"));
    assert!(prompt.contains("pale skin"));
    assert!(prompt.contains("thin scar on left cheek"));
    assert!(prompt.contains("heavy woolen cloak"));
    assert!(prompt.contains("weathered"));
    assert!(prompt.contains("copper-tinted"));
    assert!(prompt.contains("Palette: copper, deep blue, warm gray."));

    // Verify structured descriptors
    let descriptors = desc.get("descriptors").unwrap();
    assert_eq!(descriptors.get("subject"), Some(&Value::str("slender figure with swept-back hair")));
    assert!(descriptors.get("features").is_some());
    assert!(descriptors.get("clothing").is_some());
    assert!(descriptors.get("matter").is_some());

    // Compile VisualSpec with enriched bible
    let mut frame = a.get("frame").unwrap().clone();
    set(&mut frame, "life_revision", Value::Null);
    let spec = VisualSpec::compile(&bible, &frame, None, &Activity::idle()).unwrap();
    let spec_desc = spec.describe().unwrap();
    let spec_prompt = spec_desc.get("prompt").unwrap().as_str().unwrap();
    assert!(spec_prompt.contains("amber eyes"));
    assert!(spec_prompt.contains("heavy woolen cloak"));

    // Render includes prompt and negative prompt
    let rendered = spec.render().unwrap();
    assert!(rendered.get("prompt").is_some());
    assert!(rendered.get("negative_prompt").is_some());
    assert_eq!(spec.prompt().unwrap(), spec_prompt);
}

#[test]
fn visual_describer_model_presets_and_customization() {
    let spec = spec("neutral_portrait");

    // SDXL preset: tags mode, extra negative tokens
    let sdxl_cfg = visual::DescriberConfig::sdxl();
    let sdxl_res = spec.describe_with(&sdxl_cfg).unwrap();
    assert_eq!(sdxl_res.get("target"), Some(&Value::str("sdxl")));
    let neg = sdxl_res.get("negative_prompt").unwrap().as_str().unwrap();
    assert!(neg.contains("blurry"));
    assert!(neg.contains("watermark"));

    // Flux preset: clean tags, empty negative guidance
    let flux_cfg = visual::DescriberConfig::flux();
    let flux_res = spec.describe_with(&flux_cfg).unwrap();
    assert_eq!(flux_res.get("target"), Some(&Value::str("flux")));
    assert_eq!(flux_res.get("negative_prompt").unwrap().as_str().unwrap(), "");

    // Midjourney preset: prose mode, style first, aspect suffix
    let mj_cfg = visual::DescriberConfig::midjourney();
    let mj_res = spec.describe_with(&mj_cfg).unwrap();
    assert_eq!(mj_res.get("target"), Some(&Value::str("midjourney")));
    let mj_prompt = mj_res.get("prompt").unwrap().as_str().unwrap();
    assert!(mj_prompt.starts_with("Original flat-color ink illustration"));
    assert!(mj_prompt.contains("--v 6.1"));

    // DALL-E preset: prose mode, descriptive sentences
    let dalle_cfg = visual::DescriberConfig::dalle();
    let dalle_res = spec.describe_with(&dalle_cfg).unwrap();
    assert_eq!(dalle_res.get("target"), Some(&Value::str("dalle3")));
    assert_eq!(dalle_res.get("negative_prompt").unwrap().as_str().unwrap(), "");

    // Custom configuration: custom separator, prefix, suffix, extra tokens
    let custom_cfg = visual::DescriberConfig::default()
        .with_separator(" | ")
        .with_prefix("MASTERPIECE:")
        .with_suffix("[8k resolution]")
        .with_extra_positive(vec!["highly detailed".into()])
        .with_extra_negative(vec!["jpeg artifacts".into()]);
    let custom_res = spec.describe_with(&custom_cfg).unwrap();
    let custom_prompt = custom_res.get("prompt").unwrap().as_str().unwrap();
    assert!(custom_prompt.starts_with("MASTERPIECE:"));
    assert!(custom_prompt.contains(" | "));
    assert!(custom_prompt.contains("highly detailed"));
    assert!(custom_prompt.ends_with("[8k resolution]"));

    // Dynamic configuration from Value
    let dynamic_val = obj! {
        "mode" => "prose",
        "style_first" => true,
        "target" => "dalle3",
        "extra_positive" => vec!["breathtaking"],
        "prefix" => "Cinematic:"
    };
    let dynamic_cfg = visual::DescriberConfig::from_value(&dynamic_val).unwrap();
    let dynamic_res = spec.describe_with(&dynamic_cfg).unwrap();
    let dyn_prompt = dynamic_res.get("prompt").unwrap().as_str().unwrap();
    assert!(dyn_prompt.starts_with("Cinematic:"));
    assert!(dyn_prompt.contains("breathtaking"));
}

#[test]
fn art_bible_malformed_embodiment_and_matter_rejection() {
    let a = fixture("neutral_portrait");
    let mut b = a.get("bible").unwrap().clone();

    // Reject unknown anchor field
    let mut anchors = b.get("anchors").unwrap().as_array().unwrap().to_vec();
    let mut bad_anchor = anchors[0].clone();
    set(&mut bad_anchor, "unregistered_key", "forbidden".into());
    anchors[0] = bad_anchor;
    set(&mut b, "anchors", anchors.clone().into());
    assert!(ArtBible::from_value(&b).is_err());

    // Reject invalid feature key
    let mut bad_anchor = anchors[0].clone();
    set(&mut bad_anchor, "features", obj! {"laser_eyes" => true});
    anchors[0] = bad_anchor;
    set(&mut b, "anchors", anchors.clone().into());
    assert!(ArtBible::from_value(&b).is_err());

    // Reject negative mass
    let mut bad_anchor = anchors[0].clone();
    set(&mut bad_anchor, "mass", Value::Number("-10.0".into()));
    anchors[0] = bad_anchor;
    set(&mut b, "anchors", anchors.clone().into());
    assert!(ArtBible::from_value(&b).is_err());

    // Reject invalid physical state
    let mut bad_anchor = anchors[0].clone();
    set(&mut bad_anchor, "state", "plasma".into());
    anchors[0] = bad_anchor;
    set(&mut b, "anchors", anchors.clone().into());
    assert!(ArtBible::from_value(&b).is_err());

    // Reject invalid dimensions length
    let mut bad_anchor = anchors[0].clone();
    set(&mut bad_anchor, "dimensions", vec![1.0, 2.0].into());
    anchors[0] = bad_anchor;
    set(&mut b, "anchors", anchors.into());
    assert!(ArtBible::from_value(&b).is_err());
}

#[test]
fn visual_describer_cli_and_session_export() {
    let art_val: Value =
        gamectx::data::json::parse(include_str!("../examples/lantern-vault/web/art.json"), 1_048_576).unwrap();

    // 1. kit::local::process with kind "visual-describe"
    let local_req = obj! {
        "kind" => "visual-describe",
        "args" => obj! {
            "bible" => art_val.clone(),
            "anchor" => "merchant",
            "model" => "flux"
        }
    };
    let local_res = gamectx::kit::local::process(&local_req).unwrap();
    let prompt = local_res.get("prompt").and_then(Value::as_str).unwrap();
    assert!(prompt.contains("amber eyes") || prompt.contains("copper stitching"));
    assert_eq!(local_res.get("target"), Some(&Value::str("flux")));

    // 2. Direct VisualSpec description through CLI
    let spec = spec("neutral_portrait");
    let spec_req = obj! {
        "kind" => "visual-describe",
        "args" => obj! {
            "spec" => spec.to_value(),
            "model" => "sdxl"
        }
    };
    let spec_res = gamectx::kit::local::process(&spec_req).unwrap();
    assert_eq!(spec_res.get("target"), Some(&Value::str("sdxl")));
    assert!(spec_res.get("negative_prompt").and_then(Value::as_str).unwrap().contains("blurry"));
}
