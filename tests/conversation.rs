use gamectx::{
    decision::PendingAction,
    dialogue::{resolve_references, Conversation, ConversationLimits},
    obj, Value,
};
#[test]
fn conversation_limits_scopes_and_failed_replies_preserve_state() {
    let mut c = Conversation::with_limits(ConversationLimits {
        scopes: 1,
        records: 2,
        history: 1,
        tickets: 8,
        ..Default::default()
    })
    .unwrap();
    let args = obj! {"scope"=>"actor:trader","speaker"=>"trader","revision"=>9,"input"=>"My name is Fen.","facts"=>obj! {"price"=>9},"recall"=>2,"history"=>1};
    let ticket = c.prepare(&args).unwrap();
    let reply = r#"{"speech":"Welcome.","tone":"neutral","fact_ids":["price"],"memory_ids":[]}"#;
    let before = c.state("actor:trader").unwrap();
    assert!(c.finish("other", 9, "chat_1", reply).is_err());
    assert_eq!(before, c.state("actor:trader").unwrap());
    c.finish("actor:trader", 9, ticket.get("id").unwrap().as_str().unwrap(), reply).unwrap();
    assert!(c.finish("actor:trader", 9, "chat_1", reply).is_err());
    let other = obj! {"scope"=>"other","speaker"=>"watcher","revision"=>9,"input"=>"Hello.","facts"=>obj! {},"recall"=>0,"history"=>0};
    assert!(c.prepare(&other).is_err());
    assert!(c.state("other").unwrap().get("memories").unwrap().as_array().unwrap().is_empty());
    c.forget("actor:trader").unwrap();
    c.prepare(&other).unwrap();
}
#[test]
fn confirmation_checks_terms_scope_revision_and_permissions_before_host_dispatch() {
    let decision = obj! {"action"=>"buy","target"=>"item:oil"};
    let terms = obj! {"price"=>3,"quantity"=>1};
    let offer = obj! {"id"=>"o1","scope"=>"shop","revision"=>4,"decision"=>decision,"terms"=>terms.clone()};
    let mut p = PendingAction::new(&offer).unwrap();
    assert!(p.respond("shop", 4, true, &obj! {"price"=>1,"quantity"=>1}, |_| panic!("terms checked first")).is_err());
    assert!(p.respond("shop", 4, true, &terms, |_| true).is_err());
    let mut p = PendingAction::new(&offer).unwrap();
    assert!(p.respond("shop", 4, true, &terms, |_| false).is_err());
    assert!(p.is_finished());
    let mut p = PendingAction::new(&offer).unwrap();
    let result = p.respond("shop", 4, true, &terms, |_| true).unwrap();
    assert_eq!(result.get("status"), Some(&Value::str("confirmed")));
}
#[test]
fn repeated_aliases_do_not_cause_pairwise_span_work() {
    let entities = (0..64)
        .map(|i| obj! {"id"=>format!("entity:{i}"),"label"=>format!("key {i}"),"kind"=>"item","aliases"=>vec!["key";8]})
        .collect::<Vec<_>>();
    let r=resolve_references(&obj! {"entities"=>entities,"input"=>"key ".repeat(256),"kind"=>Value::Null,"scope"=>"room","revision"=>1,"max_age"=>4,"focus"=>Value::Null,"selection"=>Value::Null,"pronouns"=>vec!["it"]}).unwrap();
    assert_eq!(r.get("reason"), Some(&Value::str("ambiguous")));
    assert_eq!(r.get("candidates").unwrap().as_array().unwrap().len(), 64);
}

#[test]
fn empty_fact_and_quote_sets_have_usable_zero_citation_schemas() {
    let mut c = Conversation::default();
    c.prepare(&obj! {"scope"=>"talk","speaker"=>"visitor","revision"=>0,"input"=>"Hello.","facts"=>obj! {},"recall"=>0,"history"=>0}).unwrap();
    let schema = c.reply_schema().unwrap();
    for field in ["fact_ids", "memory_ids"] {
        let p = schema.get("properties").unwrap().get(field).unwrap();
        assert_eq!(p.get("maxItems"), Some(&0.into()));
        assert!(p.get("items").unwrap().get("enum").is_none());
    }
}
