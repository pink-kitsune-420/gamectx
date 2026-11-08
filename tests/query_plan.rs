use gamectx::{
    data::json::parse,
    data::query::{Query, Shape},
    obj, Value,
};
#[test]
fn contracts_check_structure_without_claiming_semantics() -> gamectx::Result<()> {
    let shape = Shape::from_value(
        &obj! {"table"=>"cargo","filter_columns"=>vec!["mass","mass"],"limit"=>Value::Null,"aggregate"=>false},
    )?;
    assert_eq!(Shape::from_value(&shape.to_value())?.to_value(), shape.to_value());
    Query::parse("SELECT id FROM cargo WHERE mass > 1 AND mass < 3")?.require_shape(&shape)?;
    // Values, operators, selected expressions and sort order deliberately aren't checked.
    Query::parse("SELECT mass FROM cargo WHERE mass = 99 AND mass != 2 ORDER BY mass")?.require_shape(&shape)?;
    assert!(Query::parse("SELECT id FROM cargo WHERE mass > 1")?.require_shape(&shape).is_err());
    assert!(Shape::from_value(
        &obj! {"table"=>"cargo","filter_columns"=>vec!["mass"],"limit"=>Value::Number("1.0".into()),"aggregate"=>false}
    )
    .is_err());
    Ok(())
}
#[test]
fn validation_and_sorting_still_precede_limit() -> gamectx::Result<()> {
    let missing = parse(r#"[{"mass":1},{}]"#, 4096)?;
    assert!(Query::parse("SELECT mass FROM cargo LIMIT 0")?.run(missing.as_array().unwrap()).is_err());
    let bad = parse(r#"[{"mass":1},{"mass":"a"},{"mass":true}]"#, 4096)?;
    let error = Query::parse("SELECT mass FROM cargo ORDER BY mass LIMIT 0")?.run(bad.as_array().unwrap()).unwrap_err();
    assert_eq!(error.to_string(), "row 2: sort_by column \"mass\" must hold numbers or strings");
    let rows = parse(
        r#"[{"id":"grove:café.map","serial":9007199254740993},{"id":"門.open","serial":123456789012345678901234567890}]"#,
        4096,
    )?;
    assert_eq!(
        Query::parse("SELECT id, serial FROM cargo")?.run(rows.as_array().unwrap())?.to_json(),
        r#"[["grove:café.map",9007199254740993],["門.open",123456789012345678901234567890]]"#
    );
    Ok(())
}
#[test]
fn relevance_boundaries_are_shared_by_tables_and_catalogs() -> gamectx::Result<()> {
    use gamectx::{
        data::catalog::{Catalog, Limits},
        data::view,
    };
    for id in ["zone:café_map", "zone.café_map", "zone/café_map", "zone-café_map"] {
        let rows = vec![obj! {"id"=>id}, obj! {"id"=>"zone:caféteria"}];
        for question in ["CAFÉ", "café_map", "map", id] {
            assert_eq!(view::relevant_rows(rows.clone(), question), vec![rows[0].clone()]);
        }
        let catalog = Catalog::from_json(
            &Value::Array(vec![obj! {"kind"=>"item","id"=>id,"tags"=>vec!["#route.gate"]}]),
            Limits::default(),
        )?;
        for question in ["CAFÉ", "café_map", "map", "gate", id] {
            assert_eq!(catalog.relevant(question, None, 1, None)?.entries.len(), 1);
        }
        assert!(catalog.relevant("caf", None, 1, None)?.entries.is_empty());
    }
    Ok(())
}

#[test]
fn explain_cli_reads_no_table_and_rejects_extra_arguments() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gamectx"))
        .args(["query", "--explain", "SELECT COUNT(*) FROM cargo"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let plan = parse(std::str::from_utf8(&output.stdout).unwrap(), 4096).unwrap();
    assert_eq!(plan, Query::parse("SELECT COUNT(*) FROM cargo").unwrap().plan());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gamectx"))
        .args(["query", "--explain", "SELECT COUNT(*) FROM cargo", "unexpected"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn guidance_is_explicit_and_standard_is_empty() -> gamectx::Result<()> {
    use gamectx::data::query::Guidance;
    assert_eq!(Guidance::default(), Guidance::Standard);
    assert!(Guidance::Standard.text().is_empty());
    for name in ["standard", "explicit-selection"] {
        let profile = Guidance::parse(name)?;
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_gamectx"))
            .args(["query", "--guidance", name])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(parse(std::str::from_utf8(&output.stdout).unwrap(), 8192)?, Value::str(profile.text()));
    }
    assert!(Guidance::parse("automatic").is_err());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gamectx"))
        .args(["query", "--guidance", "automatic"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    Ok(())
}
