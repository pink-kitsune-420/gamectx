#[path = "../examples/dungeon/game.rs"]
mod game;
#[path = "../examples/dungeon/host.rs"]
mod host;
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
    let data = gamectx::json::parse(messages.last().unwrap().get("content").unwrap().as_str().unwrap(), 65536).unwrap();
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dungeon/cases");
    let mut count = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let dir = entry.unwrap().path();
        let metadata = gamectx::json::parse(&std::fs::read_to_string(dir.join("case.json")).unwrap(), 65536).unwrap();
        let input = gamectx::json::parse(&std::fs::read_to_string(dir.join("input.json")).unwrap(), 262144).unwrap();
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
