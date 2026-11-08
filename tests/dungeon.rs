#[path = "../examples/lantern-vault/conversation_policy.rs"]
mod conversation_policy;
#[path = "../examples/lantern-vault/game.rs"]
mod game;
#[path = "../examples/lantern-vault/host.rs"]
mod host;
#[path = "../examples/lantern-vault/intent.rs"]
mod intent;
#[path = "../examples/lantern-vault/knowledge.rs"]
mod knowledge;
#[path = "../examples/lantern-vault/natural.rs"]
mod natural;
#[path = "../examples/lantern-vault/place.rs"]
mod place;
#[path = "../examples/lantern-vault/social.rs"]
mod social;
#[path = "../examples/lantern-vault/trade.rs"]
mod trade;
use gamectx::{obj, Value};
fn act(host: &mut host::Host, action: &str, target: &str) {
    assert_eq!(
        host.act(&obj! {"revision"=>host.game.revision,"decision"=>game::pair(action,Some(target))})
            .unwrap()
            .get("status"),
        Some(&Value::str("accepted"))
    );
}
#[test]
fn packets_keep_current_truth_and_hide_unseen_items() {
    let mut host = host::Host::new(7).unwrap();
    let initial = host.observe(game::GOAL, "named", 2048).unwrap();
    let request = initial.get("request").unwrap().to_json();
    assert!(!request.contains("host-only-diagnostic") && !request.contains("host_tag"));
    assert!(
        !request.contains("item:bronze_key")
            && !request.contains("item:glass_star")
            && !request.contains("item:keeper_seal")
    );
    act(&mut host, "go", "room:gallery");
    act(&mut host, "go", "room:workshop");
    let observed = host.observe(game::GOAL, "scene", 2048).unwrap();
    assert_eq!(observed.get("budget").unwrap().get("method"), Some(&Value::str("estimate")));
    let request = observed.get("request").unwrap();
    let messages = request.get("messages").unwrap().as_array().unwrap();
    let data =
        gamectx::data::json::parse(messages.last().unwrap().get("content").unwrap().as_str().unwrap(), 65536).unwrap();
    let evidence = data.get("scene").unwrap().get("facts").unwrap().get("evidence").unwrap();
    assert_eq!(evidence.get("affordable").unwrap().to_json(), r#"["item:oil","item:chalk"]"#);
    let before = host.game.public();
    assert!(host.observe(game::GOAL, "scene", 512).is_err());
    assert_eq!(before, host.game.public());
    let stale = host.act(&obj! {"revision"=>0,"decision"=>game::pair("buy",Some("item:oil"))}).unwrap();
    assert_eq!(stale.get("reason"), Some(&Value::str("stale_revision")));
    assert_eq!(before, host.game.public());
    assert!(host.request(&obj! {"op"=>"decide","args"=>obj! {"revision"=>2,"format"=>"named","text"=>"{\"choice\":\"wait_here\",\"choice\":\"buy_oil\"}"}}).is_err());
    assert_eq!(before, host.game.public());
}
#[test]
fn bounded_choices_and_live_callback_reject_theft_and_preserve_inspection() {
    let mut host = host::Host::new(0).unwrap();
    act(&mut host, "go", "room:gallery");
    act(&mut host, "go", "room:cistern");
    let before = host.game.public();
    let rejected = host.act(&obj! {"revision"=>2,"decision"=>game::pair("take",Some("item:keeper_seal"))}).unwrap();
    assert_eq!(rejected.get("status"), Some(&Value::str("rejected")));
    assert_eq!(before, host.game.public());
    let mut guard = host
        .game
        .decision_choices()
        .unwrap()
        .with_intent(&obj! {"request"=>obj! {"id"=>"inspect_key","text"=>"Inspect the key; leave it where it is."},
        "facts"=>obj! {"key_owner"=>"world"},"allowed"=>vec![game::pair("inspect",Some("item:bronze_key"))]})
        .unwrap();
    let retry = guard.apply(&game::pair("take", Some("item:bronze_key")).to_json(), 2, |_| true).unwrap();
    assert_eq!(retry.get("status"), Some(&Value::str("retry")));
    assert!(!host.game.owns("item:bronze_key"));
    act(&mut host, "inspect", "item:bronze_key");
    assert!(!host.game.owns("item:bronze_key"));
    let raw = host.observe(game::GOAL, "raw", 2048).unwrap();
    assert!(raw.get("request").is_some());
    let command = obj! {"seed"=>0,"commands"=>Vec::<Value>::new()};
    assert_eq!(game::replay(&command).unwrap().get("won"), Some(&Value::Bool(false)));
}

#[test]
fn independent_dungeon_replay_fixtures() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/lantern-vault/cases");
    let mut count = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let dir = entry.unwrap().path();
        let metadata =
            gamectx::data::json::parse(&std::fs::read_to_string(dir.join("case.json")).unwrap(), 65536).unwrap();
        let input =
            gamectx::data::json::parse(&std::fs::read_to_string(dir.join("input.json")).unwrap(), 262144).unwrap();
        match game::replay(&input) {
            Ok(value) => {
                assert_ne!(metadata.get("error"), Some(&Value::Bool(true)));
                assert_eq!(
                    value.to_json() + "\n",
                    std::fs::read_to_string(dir.join("expected.json")).unwrap(),
                    "{}",
                    dir.display()
                );
            }
            Err(_) => assert_eq!(metadata.get("error"), Some(&Value::Bool(true))),
        }
        count += 1;
    }
    assert_eq!(count, 23);
}

#[test]
fn social_replay_can_leave_world_unchanged() {
    let v = obj! {"seed"=>7,"operations"=>Vec::<Value>::new()};
    assert_eq!(social::replay(&v).unwrap().get("world"), Some(&game::Game::new(7).unwrap().public()));
}

#[test]
fn independent_social_fixtures_and_freshness() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/lantern-vault/social-cases");
    let mut count = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let dir = entry.unwrap().path();
        let input =
            gamectx::data::json::parse(&std::fs::read_to_string(dir.join("input.json")).unwrap(), 262144).unwrap();
        assert_eq!(
            social::replay(&input).unwrap().to_json() + "\n",
            std::fs::read_to_string(dir.join("expected.json")).unwrap(),
            "{}",
            dir.display()
        );
        count += 1;
    }
    assert_eq!(count, 13);
    let mut host = host::Host::new(7).unwrap();
    act(&mut host, "go", "room:gallery");
    act(&mut host, "go", "room:workshop");
    let before = host.game.public();
    let initial=host.request(&obj! {"op"=>"speech_prepare","args"=>obj! {"speaker"=>"merchant","input"=>"My name is Fen.","context"=>2048}}).unwrap();
    assert!(host
        .request(
            &obj! {"op"=>"speech_prepare","args"=>obj! {"speaker"=>"merchant","input"=>"Other message.","context"=>512}}
        )
        .is_err());
    let reply = r#"{"speech":"Welcome, Fen.","tone":"warm","fact_ids":[],"memory_ids":[]}"#;
    host.request(
        &obj! {"op"=>"speech_finish","args"=>obj! {"ticket"=>initial.get("ticket").unwrap().clone(),"text"=>reply}},
    )
    .unwrap();
    assert_eq!(before, host.game.public());
    let state = host.request(&obj! {"op"=>"speech_state","args"=>obj! {"speaker"=>"merchant"}}).unwrap();
    assert_eq!(state.get("memories").unwrap().as_array().unwrap().len(), 1);
    act(&mut host, "go", "room:gallery");
    act(&mut host, "go", "room:cistern");
    let keeper=host.request(&obj! {"op"=>"speech_prepare","args"=>obj! {"speaker"=>"keeper","input"=>"What is my name?","context"=>2048}}).unwrap();
    assert!(!keeper.get("request").unwrap().to_json().contains("Fen"));
}

#[test]
fn entity_references_use_phrases_scope_and_explicit_clarification() {
    let pairs = vec![
        game::pair("inspect", Some("item:bronze_key")),
        game::pair("inspect", Some("item:iron_key")),
        game::pair("talk", Some("npc:keeper")),
        game::pair("inspect", Some("item:keeper_seal")),
    ];
    let items = vec![
        obj! {"id"=>"item:bronze_key","label"=>"bronze key","kind"=>"key"},
        obj! {"id"=>"item:iron_key","label"=>"iron key","kind"=>"key"},
        obj! {"id"=>"item:keeper_seal","label"=>"keeper seal","kind"=>"token"},
    ];
    let seal = intent::resolve(&pairs, &items, "Inspect the keeper seal.", None, None).unwrap();
    assert_eq!(seal.pairs, vec![game::pair("inspect", Some("item:keeper_seal"))]);
    let keys = intent::resolve(&pairs, &items, "Inspect the key.", None, None).unwrap();
    assert_eq!(keys.reason, "ambiguous");
    assert!(keys.pairs.is_empty());
    assert_eq!(keys.candidates.len(), 2);
    let explicit = intent::resolve(&pairs, &items, "Inspect the key.", None, Some("item:iron_key")).unwrap();
    assert_eq!(explicit.pairs, vec![game::pair("inspect", Some("item:iron_key"))]);
    let compound = intent::resolve(&pairs, &items, "Talk to the keeper about the keeper seal.", None, None).unwrap();
    assert_eq!(compound.reason, "phrase");
    assert_eq!(compound.pairs, vec![game::pair("talk", Some("npc:keeper"))]);
    let mut host = host::Host::new(7).unwrap();
    act(&mut host, "go", "room:gallery");
    act(&mut host, "go", "room:workshop");
    act(&mut host, "inspect", "item:bronze_key");
    let request = |input: &str| obj! {"op"=>"interpret","args"=>obj! {"input"=>input,"context"=>2048,"guidance"=>"focused","resolve_ambiguity"=>true}};
    let focus = host.request(&request("Take it.")).unwrap();
    assert_eq!(focus.get("resolution").unwrap().get("reason"), Some(&Value::str("recent_action")));
    let ambiguous = host.request(&request("Inspect the oil or the chalk.")).unwrap();
    assert_eq!(ambiguous.get("resolution").unwrap().get("reason"), Some(&Value::str("ambiguous")));
    for _ in 0..5 {
        let result = host.act(&obj! {"revision"=>host.game.revision,"decision"=>game::pair("wait", None)}).unwrap();
        assert_eq!(result.get("status"), Some(&Value::str("accepted")));
    }
    let expired = host.request(&request("Take it.")).unwrap();
    assert_eq!(expired.get("resolution").unwrap().get("reason"), Some(&Value::str("unresolved_reference")));
    act(&mut host, "inspect", "item:bronze_key");
    let prepared = host
        .request(&obj! {"op"=>"speech_prepare","args"=>obj! {"speaker"=>"merchant","input"=>"Hello.","context"=>2048}})
        .unwrap();
    host.request(&obj! {"op"=>"speech_finish","args"=>obj! {"ticket"=>prepared.get("ticket").unwrap().clone(),"text"=>r#"{"speech":"Hello.","tone":"neutral","fact_ids":[],"memory_ids":[]}"#}}).unwrap();
    let after_talk = host.request(&request("Take it.")).unwrap();
    assert_eq!(after_talk.get("resolution").unwrap().get("reason"), Some(&Value::str("unresolved_reference")));
    act(&mut host, "inspect", "item:bronze_key");
    let rev = host.game.revision;
    act(&mut host, "go", "room:gallery");
    let after = host.request(&request("Take it.")).unwrap();
    assert_eq!(after.get("resolution").unwrap().get("reason"), Some(&Value::str("unresolved_reference")));
    assert!(host.request(&obj! {"op"=>"interpret","args"=>obj! {"input"=>"Inspect it.","context"=>2048,"guidance"=>"focused","resolve_ambiguity"=>true,"target_hint"=>"item:salve","expected_revision"=>rev}}).is_err());
}

#[test]
fn forgetting_dialogue_erases_quotes_and_preserves_purchased_inventory() {
    let mut host = host::Host::new(7).unwrap();
    act(&mut host, "go", "room:gallery");
    act(&mut host, "go", "room:workshop");
    let prepared=host.request(&obj! {"op"=>"speech_prepare","args"=>obj! {"speaker"=>"merchant","input"=>"My name is Fen.","context"=>2048}}).unwrap();
    host.request(&obj! {"op"=>"speech_finish","args"=>obj! {"ticket"=>prepared.get("ticket").unwrap().clone(),"text"=>r#"{"speech":"Welcome, Fen.","tone":"neutral","fact_ids":[],"memory_ids":[]}"#}}).unwrap();
    let trade = |input: &str| obj! {"op"=>"trade","args"=>obj! {"speaker"=>"merchant","input"=>input}};
    host.request(&trade("I'd like to buy oil.")).unwrap();
    let bought = host.request(&trade("yes")).unwrap();
    assert!(bought.get("result").is_some());
    let before = host.game.public();
    host.request(&obj! {"op"=>"trade_cancel","args"=>obj! {}}).unwrap();
    host.request(&obj! {"op"=>"speech_forget","args"=>obj! {"speaker"=>"merchant"}}).unwrap();
    assert_eq!(host.game.public(), before);
    let empty = host.request(&obj! {"op"=>"speech_state","args"=>obj! {"speaker"=>"merchant"}}).unwrap();
    assert!(empty.get("memories").unwrap().as_array().unwrap().is_empty());
    assert!(empty.get("history").unwrap().as_array().unwrap().is_empty());
    let owned = host.request(&trade("I'd like to buy oil.")).unwrap();
    assert_eq!(owned.get("reply"), Some(&Value::str("That item is not for sale by this merchant.")));
    assert_eq!(host.game.public(), before);
}

#[test]
fn stock_paraphrases_do_not_offer_purchases_or_select_seal_lore() {
    let mut h = host::Host::new(7).unwrap();
    act(&mut h, "go", "room:gallery");
    act(&mut h, "go", "room:workshop");
    for input in ["hello, what do you have for sale?", "What can I buy?", "Please tell me what you sell."] {
        assert_eq!(
            h.request(&obj! {"op"=>"trade","args"=>obj!{"speaker"=>"merchant","input"=>input}}).unwrap().get("handled"),
            Some(&Value::Bool(false))
        );
        let r=h.request(&obj!{"op"=>"knowledge_prepare","args"=>obj!{"speaker"=>"merchant","input"=>input,"context"=>2048,"size"=>"3"}}).unwrap();
        let cards = r.get("offer").unwrap().get("definition").unwrap().get("cards").unwrap().as_array().unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].get("id"), Some(&Value::str("stock")));
        assert!(!cards[0].to_json().contains("seal"));
    }
    act(&mut h, "buy", "item:oil");
    let r=h.request(&obj!{"op"=>"knowledge_prepare","args"=>obj!{"speaker"=>"merchant","input"=>"What do you have for sale?","context"=>2048,"size"=>"3"}}).unwrap();
    assert!(!r.get("offer").unwrap().get("definition").unwrap().to_json().contains("hinge oil"));
}
#[test]
fn experimental_natural_dialogue_uses_structured_authorized_facts_and_atomic_budget_checks() {
    let world =
        gamectx::knowledge::lore::Lore::parse(include_str!("../examples/lantern-vault/world-structured.json")).unwrap();
    let mut h = host::Host::with_world(7, world).unwrap();
    act(&mut h, "go", "room:gallery");
    act(&mut h, "go", "room:workshop");
    let before = h.game.public();
    let args = |context| obj! {"speaker"=>"merchant","input"=>"What does the tonic do?","context"=>context,"guidance"=>"focused","size"=>"3"};
    assert!(h.request(&obj! {"op"=>"natural_speech_prepare","args"=>args(512)}).is_err());
    let r = h.request(&obj! {"op"=>"natural_speech_prepare","args"=>args(2048)}).unwrap();
    assert_eq!(r.get("ticket"), Some(&Value::str("chat_1")));
    let request = r.get("request").unwrap().to_json();
    assert!(request.contains("implemented_uses") && request.contains("purpose"));
    assert!(!request.contains("keeper_background") && !request.contains("vault_inscription"));
    h.request(&obj!{"op"=>"speech_finish","args"=>obj!{"ticket"=>"chat_1","text"=>r#"{"speech":"I do not know its purpose.","tone":"neutral","fact_ids":[],"memory_ids":[]}"#}}).unwrap();
    assert_eq!(h.game.public(), before);
}

#[test]
fn npc_item_transfers_check_actual_owner_scene_and_return_custody() {
    let mut game = game::Game::new(7).unwrap();
    assert!(game.npc_transfer("keeper", "item:keeper_seal", true).is_err());
    for room in ["gallery", "cistern"] {
        game.act(&Value::from(game.revision), &game::pair("go", Some(&format!("room:{room}"))));
    }
    let before = game.public();
    assert!(game.npc_transfer("merchant", "item:keeper_seal", true).is_err());
    assert_eq!(game.public(), before);
    game.npc_transfer("keeper", "item:keeper_seal", true).unwrap();
    assert!(game.owns("item:keeper_seal"));
    assert!(game.npc_transfer("keeper", "item:keeper_seal", true).is_err());
    game.npc_transfer("keeper", "item:keeper_seal", false).unwrap();
    assert!(!game.owns("item:keeper_seal"));
    assert!(game.npc_transfer("keeper", "item:keeper_seal", false).is_err());
    assert_eq!(game.public(), before);
}
