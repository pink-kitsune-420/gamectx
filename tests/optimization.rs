use gamectx::{json::parse, obj, Value};
fn fixture(name: &str) -> Value {
    let path = format!("{}/conformance/optimization/{name}/input.json", env!("CARGO_MANIFEST_DIR"));
    parse(&std::fs::read_to_string(path).unwrap(), 1_572_864).unwrap()
}
#[test]
fn invalid_count_ack_does_not_mutate_composition() {
    let fixture = fixture("compose_recount");
    let a = fixture.get("args").unwrap();
    let mut session = gamectx::request::Composition::from_value(a.get("definition").unwrap()).unwrap();
    let before = session.state();
    assert!(session
        .observe(&obj! {"sequence"=>0,"candidate"=>Value::Null,"count"=>obj! {"input"=>1,"method"=>"exact"}})
        .is_err());
    assert_eq!(before, session.state());
    for ack in a.get("observations").unwrap().as_array().unwrap() {
        session.observe(ack).unwrap();
    }
    let done = session.state();
    assert_eq!(done.get("status").and_then(Value::as_str), Some("ready"));
    assert!(session.observe(&Value::Null).is_err());
    assert_eq!(done, session.state());
}
#[test]
fn dense_dependency_graph_is_complete_and_bounded() {
    let records = (0usize..128)
        .map(|i| {
            obj! {"id"=>format!("fact_{i}"),"value"=>i,
            "depends"=>(i.saturating_sub(16)..i).map(|j|Value::str(format!("fact_{j}"))).collect::<Vec<_>>(),
            "priority"=>i,"required"=>i==127}
        })
        .collect::<Vec<_>>();
    let selected = gamectx::selection::Evidence::from_value(&Value::Array(records)).unwrap().select(4096).unwrap();
    assert_eq!(selected.get("kept").unwrap().as_array().unwrap().len(), 128);
    assert!(gamectx::selection::Evidence::from_value(&obj! {"not"=>"records"}).is_err());
}
#[test]
fn template_uses_recipe_visibility_even_on_empty_tables() {
    let root = env!("CARGO_MANIFEST_DIR");
    let project =
        gamectx::kit::Project::parse(&std::fs::read_to_string(format!("{root}/examples/kit/project.json")).unwrap())
            .unwrap();
    let snapshot = project
        .snapshot(
            &parse(&std::fs::read_to_string(format!("{root}/examples/kit/snapshot.json")).unwrap(), 65536).unwrap(),
        )
        .unwrap();
    let template=gamectx::query::Template::from_value(&obj! {"name"=>"count_kind","description"=>"Count food.","sql"=>"SELECT COUNT(*) FROM stock",
        "parameters"=>vec![obj! {"name"=>"kind","column"=>"kind","op"=>"=","kind"=>"string"}],"max_rows"=>4096,"result_bytes"=>512}).unwrap();
    // The example recipe exposes id/count, not kind, despite using kind in its host view.
    assert!(project.query_template("conversation", &snapshot, &template, &obj! {"kind"=>"food"}).is_err());
    let definition = project.to_value();
    let mut empty =
        parse(&std::fs::read_to_string(format!("{root}/examples/kit/snapshot.json")).unwrap(), 65536).unwrap();
    if let Value::Object(fields) = &mut empty {
        if let Some((_, Value::Object(tables))) = fields.iter_mut().find(|(k, _)| k == "tables") {
            for (_, capture) in tables {
                if let Value::Object(fields) = capture {
                    for (name, value) in fields {
                        if name == "rows" {
                            *value = Value::Array(Vec::new());
                        }
                    }
                }
            }
        }
    }
    let fresh = gamectx::kit::Project::from_value(&definition).unwrap();
    let snapshot = fresh.snapshot(&empty).unwrap();
    assert!(fresh.query_template("conversation", &snapshot, &template, &obj! {"kind"=>"food"}).is_err());
}
#[test]
fn bound_query_keeps_original_values_and_host_filters() {
    let template=gamectx::query::Template::from_value(&obj! {"name"=>"visible","description"=>"Visible owned IDs.",
        "sql"=>"SELECT id, price FROM stock WHERE owner = 'player' ORDER BY price DESC LIMIT 1",
        "parameters"=>vec![obj! {"name"=>"kind","column"=>"kind","op"=>"=","kind"=>"string"}],"max_rows"=>4096,"result_bytes"=>512}).unwrap();
    let rows=parse(r#"[{"id":"雪:coin.v2","owner":"player","kind":"food","price":9007199254740993},{"id":"other","owner":"other","kind":"food","price":9007199254741000}]"#,4096).unwrap();
    gamectx::kit::Tool::from_function(&template.tool()).unwrap();
    let query = template.bind(&obj! {"kind"=>"food"}).unwrap();
    assert_eq!(query.run(rows.as_array().unwrap()).unwrap().to_json(), r#"[["雪:coin.v2",9007199254740993]]"#);
    assert_eq!(
        template.run(&obj! {"kind"=>"food' OR owner = 'other"}, rows.as_array().unwrap()).unwrap().to_json(),
        "[]"
    );
    assert_eq!(query.run(rows.as_array().unwrap()).unwrap().to_json(), r#"[["雪:coin.v2",9007199254740993]]"#);
}
