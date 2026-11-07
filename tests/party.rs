use gamectx::{
    archetype::{Archetype, Blueprint, BlueprintBuilder},
    deployment::{Identity, ModelSize},
    json,
    life::{Definition, Setting},
    lore::Lore,
    obj,
    party::{AttitudeGates, ModelRoute, PartyScene, SceneTurn},
    spending::SpendingLedger,
    Value,
};
use std::sync::Arc;
fn world() -> Value {
    json::parse(include_str!("../examples/party/chime-counter.json"), 262144).unwrap()
}
fn set(v: &mut Value, k: &str, value: Value) {
    if let Value::Object(f) = v {
        *f.iter_mut().find(|(n, _)| n == k).map(|(_, v)| v).unwrap() = value;
    }
}
fn request() -> Value {
    json::parse(include_str!("../conformance/archetype-party/compile_trusted/input.json"), 262144)
        .unwrap()
        .get("args")
        .unwrap()
        .get("request")
        .unwrap()
        .clone()
}
fn load(w: &Value, b: &Blueprint) -> gamectx::life::Life {
    let setting = Setting::from_value(w.get("setting").unwrap()).unwrap();
    let d = Arc::new(Definition::draft(w.get("definitions").unwrap().get("merchant").unwrap(), &setting).unwrap());
    b.life("chime_counter", "merchant", &PartyScene::from_value(w.get("scene").unwrap()).unwrap().participants(), d)
        .unwrap()
}
fn scene(w: &Value) -> PartyScene {
    let mut s = PartyScene::from_value(w.get("scene").unwrap()).unwrap();
    for r in w.get("receipts").unwrap().as_array().unwrap() {
        s.receipt(r).unwrap();
    }
    s
}
#[test]
fn presets_compose_without_enabling_model_tools_or_background_calls() {
    let ambient = Blueprint::preset(Archetype::Ambient).unwrap();
    assert!(ambient.has("brain"));
    assert!(!ambient.has("life"));
    assert!(ambient.route("watch", 1).is_err());
    assert_eq!(ambient.route("watch", 0).unwrap().to_value().get("calls"), Some(&0.into()));
    let mixed = BlueprintBuilder::new(Archetype::Service)
        .module("recall", true)
        .unwrap()
        .module("promises", true)
        .unwrap()
        .tools(&["life_recall"])
        .build()
        .unwrap();
    assert_eq!(mixed.caps().episodes, 2);
    assert_eq!(mixed.caps().exchanges, 1);
    assert!(BlueprintBuilder::new(Archetype::Service).tools(&["life_change"]).build().is_err());
    let inventory=gamectx::inventory::Inventory::from_value(&obj!{"version"=>1,"revision"=>1,"items"=>vec![obj!{"id"=>"lamp","label"=>"Repaired lamp","tags"=>vec!["trade"]}],"holdings"=>vec![obj!{"owner"=>"merchant","item"=>"lamp","quantity"=>1}]}).unwrap();
    assert_eq!(
        mixed.stock(&inventory, &vec!["merchant"].into(), &vec!["trade"].into()).unwrap().as_array().unwrap().len(),
        1
    );
    assert!(ambient.stock(&inventory, &vec!["merchant"].into(), &vec!["trade"].into()).is_err());
    let q = Blueprint::preset(Archetype::Quest).unwrap();
    let evidence=gamectx::selection::Evidence::from_value(&vec![obj!{"id"=>"receipt","value"=>"The lamp was returned.","depends"=>Vec::<Value>::new(),"priority"=>10,"required"=>true}].into()).unwrap();
    assert!(q.evidence(&evidence, 512).unwrap().to_json().contains("lamp was returned"));
    let tools =
        gamectx::dialogue::InteractionTools::new("chime_counter", "merchant", &vec!["merchant", "player_1"].into())
            .unwrap();
    assert!(q.promises(&tools).unwrap().as_array().unwrap().is_empty());
}
#[test]
fn gates_remove_payloads_from_every_compiled_message_and_use_each_listener() {
    let w = world();
    let b = Blueprint::preset(Archetype::Service).unwrap();
    let life = load(&w, &b);
    let s = scene(&w);
    let gates = AttitudeGates::from_value(w.get("gates").unwrap()).unwrap();
    let lore = Lore::from_value(w.get("lore").unwrap()).unwrap();
    let mut q = request();
    for listeners in [vec!["player_2"], vec!["player_1", "player_2"]] {
        set(&mut q, "listeners", listeners.into());
        let result =
            s.compile(&SceneTurn { life: &life, blueprint: &b, gates: &gates, lore: &lore, request: &q }).unwrap();
        let text = result.get("request").unwrap().to_json();
        for secret in
            ["copper sparrow", "silver chime", "vault_visit", "hidden_chime", "old_watch", "attitude_threshold"]
        {
            assert!(!text.contains(secret), "{secret}");
        }
        assert!(text.contains("counter opens"));
        assert!(result.get("inspection").unwrap().to_json().contains("attitude_threshold"));
    }
}
#[test]
fn appraisal_rechecks_gate_and_stale_context_cannot_be_reused() {
    let mut w = world();
    let b = Blueprint::preset(Archetype::Service).unwrap();
    let mut d = w.get("definitions").unwrap().get("merchant").unwrap().clone();
    if let Value::Array(attrs) = d_field(&mut d, "attitudes") {
        set(&mut attrs[0], "level", 14.into());
    }
    set(d_field(&mut w, "definitions"), "merchant", d);
    let mut life = load(&w, &b);
    let s = scene(&w);
    let gates = AttitudeGates::from_value(w.get("gates").unwrap()).unwrap();
    let lore = Lore::from_value(w.get("lore").unwrap()).unwrap();
    let mut q = request();
    let before = s.compile(&SceneTurn { life: &life, blueprint: &b, gates: &gates, lore: &lore, request: &q }).unwrap();
    assert!(!before.get("request").unwrap().to_json().contains("copper sparrow"));
    life.experience(&obj!{"token"=>1,"at"=>1,"kind"=>"direct","tag"=>"returned","subject"=>"player_1","source"=>obj!{"id"=>"return_one","speaker"=>"merchant","text"=>"An intact return."},"audience"=>s.participants(),"text"=>"The borrowed lamp was returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true}).unwrap();
    let change = life.appraise("careful_return", 1, 1).unwrap();
    assert_eq!(change.get("after"), Some(&19.into()));
    assert!(s.compile(&SceneTurn { life: &life, blueprint: &b, gates: &gates, lore: &lore, request: &q }).is_err());
    set(&mut q, "life_revision", 2.into());
    let after = s.compile(&SceneTurn { life: &life, blueprint: &b, gates: &gates, lore: &lore, request: &q }).unwrap();
    assert!(after.get("request").unwrap().to_json().contains("copper sparrow"));
}
fn d_field<'a>(v: &'a mut Value, k: &str) -> &'a mut Value {
    let Value::Object(f) = v else { panic!("object") };
    f.iter_mut().find(|(n, _)| n == k).map(|(_, v)| v).unwrap()
}
#[test]
fn recall_tool_cannot_bypass_gates_or_turn_exposure() {
    let w = world();
    let b = BlueprintBuilder::new(Archetype::Companion).tools(&["life_recall", "life_change"]).build().unwrap();
    let life = load(&w, &b);
    let s = scene(&w);
    let gates = AttitudeGates::from_value(w.get("gates").unwrap()).unwrap();
    let lore = Lore::from_value(w.get("lore").unwrap()).unwrap();
    let mut q = request();
    set(&mut q, "listeners", vec!["player_2"].into());
    set(d_field(&mut q, "turn"), "profile", Value::str("expanded"));
    set(&mut q, "tools", vec![life.schema("life_recall").unwrap()].into());
    let turn = SceneTurn { life: &life, blueprint: &b, gates: &gates, lore: &lore, request: &q };
    let result = s.call_life(&turn, "life_recall", r#"{"tags":["trade"],"limit":16}"#, 8192, |_| true).unwrap();
    assert!(!result.to_json().contains("copper sparrow"));
    assert!(s.call_life(&turn, "life_change", "{}", 8192, |_| true).is_err());
    assert!(s.call_life(&turn, "life_recall", "{}", 8192, |_| false).is_err());
}
#[test]
fn roles_follow_the_active_ai_and_only_confirmed_hearing() {
    let w = world();
    let s = scene(&w);
    let merchant = s.render("merchant", 16).unwrap();
    let guard = s.render("guard", 16).unwrap();
    let keeper = s.render("keeper", 16).unwrap();
    assert_eq!(
        merchant
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r.get("role").and_then(Value::as_str) == Some("assistant"))
            .count(),
        1
    );
    assert!(guard.as_array().unwrap().iter().all(|r| r.get("role").and_then(Value::as_str) == Some("user")));
    assert!(!merchant.to_json().contains("promised lamp inventory"));
    assert!(keeper.to_json().contains("[player_2 -> keeper]"));
    assert!(!guard.to_json().contains("May I hear about"));
}
#[test]
fn hybrid_projection_grounds_small_turns_without_enabling_more_tools() {
    let w = world();
    let rich = BlueprintBuilder::new(Archetype::Companion)
        .size(Some(ModelSize::B70))
        .policies(true, true, false)
        .tools(&["life_recall", "life_change"])
        .build()
        .unwrap();
    let focused = rich
        .for_route(&ModelRoute::from_value(&obj! {"name"=>"local","size"=>"3b","calls"=>1,"cost_micros"=>0}).unwrap())
        .unwrap();
    assert_eq!(focused.caps().exchanges, 1);
    assert_eq!(focused.caps().episodes, 2);
    let life = load(&w, &rich);
    let s = scene(&w);
    let gates = AttitudeGates::from_value(w.get("gates").unwrap()).unwrap();
    let lore = Lore::from_value(w.get("lore").unwrap()).unwrap();
    let q = request();
    let result =
        s.compile(&SceneTurn { life: &life, blueprint: &focused, gates: &gates, lore: &lore, request: &q }).unwrap();
    assert!(!result.get("request").unwrap().to_json().contains("imagined hypotheticals"));
    let identity=Identity::from_value(&obj!{"model_digest"=>"host_pinned","quantization"=>"host_defined","template_hash"=>"template_one","tokenizer"=>"tokenizer_one","runner"=>"host_runner"}).unwrap();
    let route = rich.route("expanded", 0).unwrap();
    let profile = ModelSize::B70.starting_profile("expanded", &identity).unwrap();
    assert!(route.bind_profile(profile.get("profile").unwrap(), &identity).is_ok());
    let changed=Identity::from_value(&obj!{"model_digest"=>"other","quantization"=>"host_defined","template_hash"=>"template_one","tokenizer"=>"tokenizer_one","runner"=>"host_runner"}).unwrap();
    assert!(route.bind_profile(profile.get("profile").unwrap(), &changed).is_err());
}
#[test]
fn ambient_uses_real_routine_and_brain_without_a_model_allowance() {
    let b = Blueprint::preset(Archetype::Ambient).unwrap();
    let people: Value = vec!["guard", "player_1"].into();
    let routine = Arc::new(gamectx::routine::Routine::parse(include_str!("../examples/dungeon/routine.json")).unwrap());
    let mut runtime = b.routine("watch", "guard", &people, "cycle:37", routine).unwrap();
    runtime.update(gamectx::routine::Frame::from_value(&obj!{"revision"=>1,"now"=>0,"conditions"=>vec!["route"],"allowed"=>vec!["work","walk","customer"],"emergency"=>false}).unwrap()).unwrap();
    assert!(runtime.reserve_model_call(1).is_err());
    let brain = gamectx::brain::Brain::dense(&["alert"], &["watch", "idle"], 1, 7).unwrap();
    assert!(b.run_brain(&brain, &obj! {"alert"=>1}, &["watch", "idle"]).is_ok());
    assert!(b.initiative("watch", "guard", &people).is_ok());
}
#[test]
fn invalid_candidates_do_not_spend_or_consume_the_trigger() {
    let v = json::parse(include_str!("../conformance/archetype-party/duplicate_candidate_rejected/input.json"), 262144)
        .unwrap();
    let a = v.get("args").unwrap();
    let mut s = PartyScene::from_value(a.get("scene").unwrap()).unwrap();
    let ops = a.get("operations").unwrap().as_array().unwrap();
    s.receipt(ops[0].get("args").unwrap()).unwrap();
    let mut ledger = SpendingLedger::new(100, 100).unwrap();
    let before = ledger.to_value();
    assert!(s.floor(ops[1].get("args").unwrap(), &mut ledger).is_err());
    assert_eq!(ledger.to_value(), before);
    let mut valid = ops[1].get("args").unwrap().clone();
    if let Value::Array(c) = d_field(&mut valid, "candidates") {
        c.pop();
    }
    assert_eq!(s.floor(&valid, &mut ledger).unwrap().get("selected").unwrap().as_array().unwrap().len(), 1);
}

#[test]
fn gate_sidecars_accept_existing_scoped_lore_and_facet_ids() {
    let w = world();
    let b = Blueprint::preset(Archetype::Service).unwrap();
    let life = load(&w, &b);
    let gates=AttitudeGates::from_value(&vec![obj!{"kind"=>"lore","id"=>"vault:secret","clauses"=>vec![obj!{"attitude"=>"trust","target"=>"listener","op"=>"ge","value"=>15}]}].into()).unwrap();
    let result = gates
        .select(
            &life,
            &obj! {"revision"=>0,"speaker"=>"player_1","listeners"=>vec!["player_2"]},
            "lore",
            &vec![obj! {"id"=>"vault:secret","text"=>"The private chime location."}].into(),
        )
        .unwrap();
    assert!(result.get("packet").unwrap().as_array().unwrap().is_empty());
    assert_eq!(result.get("withheld").unwrap().as_array().unwrap().len(), 1);
}

#[test]
fn player_followup_resets_the_npc_chain_without_replaying_a_trigger() {
    let v = json::parse(include_str!("../conformance/archetype-party/one_responder_and_replay/input.json"), 262144)
        .unwrap();
    let result = gamectx::local::process(&v).unwrap();
    let rows = result.get("results").unwrap().as_array().unwrap();
    assert_eq!(rows[2].get("result").unwrap().get("reason"), Some(&Value::str("replayed_trigger")));
    let selected = rows.last().unwrap().get("result").unwrap().get("selected").unwrap().as_array().unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].get("actor"), Some(&Value::str("merchant")));
}
