//! Host-bound perception and scoped character tools, independent of any model provider.
use gamectx::{
    character::{AttentionLimits, CharacterTools, CharacterView, MemoryChoice, ToolChoice, TurnPlan},
    json::parse,
    location::Locations,
    obj,
    request::{Count, CountMethod, RequestBudget},
    world::WorldState,
    Result, Value,
};
fn places(revision: u32) -> Result<Locations> {
    Locations::from_value(&obj! {"version"=>1,"revision"=>revision,"places"=>vec![
        obj!{"id"=>"room:shop","label"=>"workshop","summary"=>"A sheltered workbench.","description"=>"A lantern hangs above the counter.","details"=>obj!{"visible_items"=>vec!["oil"],"occupants"=>vec!["player:a"],"weather"=>Value::Null}},
        obj!{"id"=>"room:secret","label"=>"secret","summary"=>"Hidden treasure.","description"=>Value::Null,"details"=>obj!{}}
    ]})
}
fn world(revision: u32) -> Result<WorldState> {
    WorldState::from_value(&obj! {"version"=>1,"id"=>"world:test","revision"=>revision,"states"=>vec![
        obj!{"key"=>"time","value"=>obj!{"watch"=>3}},obj!{"key"=>"weather","value"=>"raining"},obj!{"key"=>"secret","value"=>"future storm"}
    ]})
}
fn character() -> Result<CharacterTools> {
    let mut c = CharacterTools::new("shop:a", "npc:merchant", &vec!["npc:merchant", "player:a"].into())?;
    c.bind(CharacterView::new(7).with_location(places(7)?, "room:shop")?.with_world(world(7)?, &["time"])?)?;
    Ok(c)
}
#[test]
fn look_is_read_only_bound_to_current_visible_scene_and_revision() -> Result<()> {
    let mut c = character()?;
    assert!(c.call("look_around", r#"{"detail":"brief"}"#, 7, 4096, |_| true).is_err());
    c.enable(&["look_around"])?;
    let before = c.call("look_around", r#"{"detail":"full"}"#, 7, 4096, |_| true)?;
    assert!(before.to_json().contains("player:a"));
    assert!(!before.to_json().contains("Hidden treasure"));
    assert!(!before.to_json().contains("raining"));
    for text in
        [r#"{"detail":"full","current":"room:secret"}"#, r#"{"detail":"full","detail":"brief"}"#, r#"{"detail":"all"}"#]
    {
        assert!(c.call("look_around", text, 7, 4096, |_| true).is_err());
    }
    assert!(c.call("look_around", r#"{"detail":"full"}"#, 8, 4096, |_| true).is_err());
    assert!(c.call("look_around", r#"{"detail":"full"}"#, 7, 64, |_| true).is_err());
    assert_eq!(before, c.call("look_around", r#"{"detail":"full"}"#, 7, 4096, |_| true)?);
    c.bind(CharacterView::new(8).with_location(places(8)?, "room:shop")?)?;
    assert!(c.enabled().is_empty());
    assert!(c.call("look_around", r#"{"detail":"brief"}"#, 8, 4096, |_| true).is_err());
    Ok(())
}
#[test]
fn visibility_controls_weather_and_unknown_matches_denied() -> Result<()> {
    let mut c = character()?;
    c.enable(&["world_status"])?;
    let result = c.call("world_status", r#"{"keys":["time","weather","secret","missing"]}"#, 7, 4096, |_| true)?;
    assert_eq!(result.get("states"), Some(&obj! {"time"=>obj!{"watch"=>3}}));
    let outside =
        CharacterView::new(8).with_location(places(8)?, "room:shop")?.with_world(world(8)?, &["time", "weather"])?;
    c.bind(outside)?;
    c.enable(&["world_status"])?;
    assert!(c.call("world_status", r#"{"keys":["weather"]}"#, 8, 4096, |_| true)?.to_json().contains("raining"));
    let places = places(7)?;
    assert_eq!(
        places.call(r#"{"detail":"full"}"#, "room:secret", 7, 4096, |_| false)?,
        places.call(r#"{"detail":"full"}"#, "missing", 7, 4096, |_| true)?
    );
    Ok(())
}
#[test]
fn required_facts_history_and_full_framing_fit_before_enabling_tools() -> Result<()> {
    let mut c = character()?;
    let mut plan = TurnPlan {
        task: "observe".into(),
        input: "Where are we?".into(),
        facts: obj! {"required"=>"The character is indoors."},
        tools: vec![
            ToolChoice { name: "look_around".into(), task: "observe".into(), priority: 50, required: true },
            ToolChoice { name: "world_status".into(), task: "observe".into(), priority: 40, required: false },
        ],
        memories: (0..8)
            .map(|i| MemoryChoice { value: Value::str("Original quote. ".repeat(12)), priority: i, required: false })
            .collect(),
        dialogue: vec![obj! {"user"=>"Hello.","assistant"=>"Welcome."}],
        attention: AttentionLimits::focused(),
    };
    let mut counts = 0;
    let result = c.assemble(
        &plan,
        &RequestBudget::new(2048, 128, 256)?,
        |frame| Ok(obj! {"chat_template"=>"t".repeat(800),"native_payload"=>frame.clone()}),
        |v| {
            counts += 1;
            Ok(Count { input: v.json_len(), method: CountMethod::Estimate })
        },
    )?;
    assert!(counts > 1);
    assert!(result.receipt.fits());
    assert_eq!(c.enabled(), ["look_around"]);
    assert!(result.request.to_json().contains("The character is indoors."));
    assert!(result.request.to_json().contains("Welcome."));
    plan.attention = AttentionLimits::new(8, 32, 8)?;
    c.assemble(
        &plan,
        &RequestBudget::new(131072, 4096, 1024)?,
        |v| Ok(v.clone()),
        |v| Ok(Count { input: v.json_len(), method: CountMethod::Estimate }),
    )?;
    assert_eq!(c.enabled(), ["look_around", "world_status"]);
    assert!(c
        .assemble(
            &plan,
            &RequestBudget::new(1, 0, 0)?,
            |v| Ok(v.clone()),
            |v| Ok(Count { input: v.json_len(), method: CountMethod::Estimate })
        )
        .is_err());
    // A failed new assembly must revoke the previous turn's capabilities.
    assert!(c.enabled().is_empty());
    Ok(())
}
#[test]
fn sessions_isolate_characters_and_reject_unbound_tools() -> Result<()> {
    let session = gamectx::kit::Session::parse(r#"{"version":1,"datasets":[],"tools":[],"recipes":[]}"#)?;
    let call =
        |op: &str, args: Value| session.request(&obj! {"version"=>1,"id"=>"test","op"=>op,"args"=>args}.to_json());
    for id in ["one", "two"] {
        assert_eq!(
            call(
                "character_create",
                obj! {"id"=>id,"scope"=>id,"actor"=>"npc:merchant","participants"=>vec!["npc:merchant","player:a"]}
            )
            .get("ok"),
            Some(&true.into())
        );
    }
    let view = obj! {"revision"=>7,"world"=>world(7)?.to_value(),"world_keys"=>vec!["time"],"locations"=>places(7)?.to_value(),"current"=>"room:shop","inventory"=>Value::Null,"owners"=>Vec::<Value>::new(),"sources"=>Vec::<Value>::new()};
    assert_eq!(call("character_bind", obj! {"id"=>"one","view"=>view}).get("ok"), Some(&true.into()));
    assert_eq!(call("character_enable", obj! {"id"=>"one","tools"=>vec!["look_around"]}).get("ok"), Some(&true.into()));
    let args = obj! {"id"=>"one","name"=>"look_around","text"=>r#"{"detail":"brief"}"#,"revision"=>7,"max_bytes"=>4096,"allowed"=>Vec::<Value>::new()};
    assert_eq!(call("character_call", args.clone()).get("ok"), Some(&true.into()));
    let mut other = parse(&args.to_json(), 4096)?;
    if let Value::Object(fields) = &mut other {
        fields.iter_mut().find(|(k, _)| k == "id").unwrap().1 = "two".into();
    }
    assert_eq!(call("character_call", other).get("ok"), Some(&false.into()));
    call("character_clear", obj! {"id"=>"one"});
    assert_eq!(call("character_call", args).get("ok"), Some(&false.into()));
    Ok(())
}

#[test]
fn note_updates_fit_before_mutation_and_keep_original_sources() -> Result<()> {
    let source = obj! {"id"=>"line:1","speaker"=>"player:a","text"=>"My watch says noon."};
    let mut c = CharacterTools::new("audience:a", "npc:a", &vec!["npc:a", "player:a"].into())?;
    c.bind(CharacterView::new(7).with_sources(&vec![source.clone()].into())?)?;
    c.enable(&["info_remember", "info_lookup"])?;
    let text = r#"{"subject":"world:time","attribute":"reported_time","value":"noon","source":"line:1"}"#;
    assert!(c.call("info_remember", text, 7, 64, |_| true).is_err());
    let lookup = r#"{"subject":"world:time","attribute":"reported_time"}"#;
    assert!(c.call("info_lookup", lookup, 7, 2048, |_| true)?.get("notes").unwrap().as_array().unwrap().is_empty());
    let note = c.call("info_remember", text, 7, 2048, |_| true)?;
    assert_eq!(note.get("id"), Some(&1.into()));
    assert_eq!(note.get("source"), Some(&source));
    assert_eq!(note.get("status"), Some(&"reported".into()));
    Ok(())
}

#[test]
fn session_count_handshake_enables_only_fitted_tools_and_rejects_replay() -> Result<()> {
    let session = gamectx::kit::Session::parse(r#"{"version":1,"datasets":[],"tools":[],"recipes":[]}"#)?;
    let call =
        |op: &str, args: Value| session.request(&obj! {"version"=>1,"id"=>"test","op"=>op,"args"=>args}.to_json());
    call(
        "character_create",
        obj! {"id"=>"one","scope"=>"audience:a","actor"=>"npc:a","participants"=>vec!["npc:a","player:a"]},
    );
    let view = obj! {"revision"=>7,"world"=>Value::Null,"world_keys"=>Vec::<Value>::new(),"locations"=>places(7)?.to_value(),"current"=>"room:shop","inventory"=>Value::Null,"owners"=>Vec::<Value>::new(),"sources"=>Vec::<Value>::new()};
    assert_eq!(call("character_bind", obj! {"id"=>"one","view"=>view.clone()}).get("ok"), Some(&true.into()));
    let plan = obj! {"task"=>"observe","input"=>"Where are we?","facts"=>obj!{"indoors"=>true},"tools"=>vec![obj!{"name"=>"look_around","task"=>"observe","priority"=>50,"required"=>true}],"memories"=>Vec::<Value>::new(),"dialogue"=>Vec::<Value>::new(),"attention"=>obj!{"tools"=>1,"memories"=>2,"exchanges"=>1}};
    let preparation = obj! {"id"=>"one","plan_id"=>"turn:1","identity"=>obj!{"model_digest"=>"arbitrary-provider-model","quantization"=>"host-defined","template_hash"=>"template:1","tokenizer"=>"host-tokenizer","runner"=>"host"},"budget"=>obj!{"context"=>2048,"output"=>128,"safety"=>256},"turn"=>plan};
    let prepare = call("character_prepare", preparation);
    assert_eq!(prepare.get("ok"), Some(&true.into()));
    let state = prepare.get("result").unwrap();
    let args = obj! {"id"=>"one","name"=>"look_around","text"=>r#"{"detail":"brief"}"#,"revision"=>7,"max_bytes"=>2048,"allowed"=>Vec::<Value>::new()};
    assert_eq!(call("character_call", args.clone()).get("ok"), Some(&false.into()));
    let observation = obj! {"sequence"=>state.get("sequence").unwrap().clone(),"candidate"=>state.get("candidate").unwrap().clone(),"count"=>obj!{"input"=>500,"method"=>"estimate"}};
    let ready = call("character_observe", obj! {"id"=>"one","observation"=>observation.clone()});
    assert_eq!(ready.get("result").unwrap().get("status"), Some(&"ready".into()));
    assert_eq!(call("character_call", args.clone()).get("ok"), Some(&true.into()));
    assert_eq!(
        call("character_observe", obj! {"id"=>"one","observation"=>observation.clone()}).get("ok"),
        Some(&false.into())
    );
    call("character_bind", obj! {"id"=>"one","view"=>view});
    assert_eq!(call("character_observe", obj! {"id"=>"one","observation"=>observation}).get("ok"), Some(&false.into()));
    assert_eq!(call("character_call", args).get("ok"), Some(&false.into()));
    Ok(())
}
