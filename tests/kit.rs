use gamectx::{data::json, kit::Project, obj, Result, Value};
use std::cell::Cell;

fn fixture() -> Result<(Project, Value)> {
    Ok((
        Project::parse(include_str!("../examples/kit/project.json"))?,
        json::parse(include_str!("../examples/kit/snapshot.json"), 1 << 20)?,
    ))
}
fn set(v: &mut Value, key: &str, value: Value) {
    let Value::Object(entries) = v else { panic!("object") };
    entries.iter_mut().find(|(n, _)| n == key).unwrap().1 = value;
}
#[test]
fn sealed_snapshot_and_recipe_visibility() -> Result<()> {
    let (p, data) = fixture()?;
    let s = p.snapshot(&data)?;
    assert!(p.query("conversation", &s, "SELECT secret FROM stock").is_err());
    // Visible globally, but not projected in this recipe.
    assert!(p.query("conversation", &s, "SELECT kind FROM stock").is_err());
    assert!(p.query("conversation", &s, "SELECT id FROM missing").is_err());
    let p2 = Project::from_value(&p.to_value())?;
    assert!(p2.build("conversation", &s).is_err());
    assert_eq!(p.clone().build("conversation", &s)?.packet.text, p.build("conversation", &s)?.packet.text);
    Ok(())
}
#[test]
fn callback_is_gated_and_host_errors_are_private() -> Result<()> {
    let (p, _) = fixture()?;
    let calls = Cell::new(0);
    let mut d = p.dispatcher();
    assert_eq!(
        d.dispatch("conversation", "inspect_item", &obj! {"id"=>"a"}).get("error").unwrap().get("code"),
        Some(&Value::str("missing_handler"))
    );
    d.register("inspect_item", |_| {
        calls.set(calls.get() + 1);
        Err(gamectx::Error::new("private host details"))
    })?;
    assert!(d.register("inspect_item", |_| Ok(Value::Null)).is_err());
    assert_eq!(d.dispatch("conversation", "inspect_item", &obj! {"id"=>12}).get("ok"), Some(&Value::Bool(false)));
    assert_eq!(d.dispatch("unknown", "inspect_item", &obj! {"id"=>"a"}).get("ok"), Some(&Value::Bool(false)));
    assert_eq!(calls.get(), 0);
    let result = d.dispatch("conversation", "inspect_item", &obj! {"id"=>"a"});
    assert_eq!(calls.get(), 1);
    assert!(!result.to_json().contains("private host"));
    Ok(())
}
#[test]
fn streaming_bridge_recovers_and_keeps_requests_independent() -> Result<()> {
    let (p, _) = fixture()?;
    let good = r#"{"version":1,"id":"a","op":"doctor","args":{}}"#;
    let mut input = vec![b'x'; gamectx::kit::MAX_REQUEST_BYTES + 100];
    input.extend_from_slice(b"\n{\n\xff\n");
    input.extend_from_slice(format!("{good}\n{good}").as_bytes());
    let mut output = Vec::new();
    gamectx::kit::serve(&p, &mut std::io::Cursor::new(input), &mut output)?;
    let text = String::from_utf8(output).unwrap();
    let replies: Vec<_> = text.lines().map(|s| json::parse(s, 1 << 20).unwrap()).collect();
    assert_eq!(replies.len(), 5);
    assert_eq!(replies[0].get("ok"), Some(&Value::Bool(false)));
    assert_eq!(replies[1].get("ok"), Some(&Value::Bool(false)));
    assert_eq!(replies[2].get("ok"), Some(&Value::Bool(false)));
    assert_eq!(replies[3], replies[4]);
    assert_eq!(replies[3].get("ok"), Some(&Value::Bool(true)));
    let huge = vec![b'x'; gamectx::kit::MAX_REQUEST_BYTES * 2 + 100];
    assert!(gamectx::kit::serve(&p, &mut std::io::Cursor::new(huge), &mut Vec::new()).is_err());
    Ok(())
}
#[test]
fn precise_numeric_tool_limits_and_mathematical_integers() -> Result<()> {
    let (p, _) = fixture()?;
    let mut definition = p.to_value();
    let Value::Array(tools) = definition.as_object().unwrap().iter().find(|(n, _)| n == "tools").unwrap().1.clone()
    else {
        panic!()
    };
    let mut t = tools[0].clone();
    set(
        &mut t,
        "parameters",
        json::parse(
            r#"{"type":"object","properties":{"n":{"type":"integer","maximum":9007199254740992}},"required":["n"],"additionalProperties":false}"#,
            4096,
        )?,
    );
    set(&mut definition, "tools", Value::Array(vec![t]));
    let p = Project::from_value(&definition)?;
    for input in [r#"{"n":9007199254740993}"#, r#"{"n":9007199254740992.1}"#, r#"{"n":1e-100}"#] {
        assert!(p.validate_call("inspect_item", &json::parse(input, 256)?).is_err(), "{input}");
    }
    for input in [r#"{"n":9007199254740992}"#, r#"{"n":1.0}"#, r#"{"n":10e-1}"#, r#"{"n":-0.00}"#] {
        assert!(p.validate_call("inspect_item", &json::parse(input, 256)?).is_ok(), "{input}");
    }
    Ok(())
}
#[test]
fn recipe_drop_report_and_schema_survive_empty_rows() -> Result<()> {
    let (p, mut data) = fixture()?;
    let Value::Object(tables) = data.get("tables").unwrap().clone() else { panic!() };
    let mut t = tables[0].1.clone();
    set(&mut t, "rows", Value::Array(Vec::new()));
    set(&mut data, "tables", obj! {"stock"=>t});
    let empty = p.snapshot(&data)?;
    assert!(p.build("conversation", &empty)?.packet.text.contains("count"));
    assert!(p.query("conversation", &empty, "SELECT secret FROM stock").is_err());
    assert_eq!(p.query("conversation", &empty, "SELECT COUNT(*) FROM stock")?, Value::int(0));
    Ok(())
}
#[test]
fn init_refuses_existing_directory_and_starter_runs() -> Result<()> {
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("gamectx-kit-{}-{unique}", std::process::id()));
    gamectx::kit::init(&path)?;
    let project = std::fs::read_to_string(path.join("project.json")).unwrap();
    assert!(gamectx::kit::init(&path).is_err());
    assert_eq!(std::fs::read_to_string(path.join("project.json")).unwrap(), project);
    let status = std::process::Command::new("python3")
        .arg(path.join("check.py"))
        .arg(env!("CARGO_BIN_EXE_gamectx"))
        .status()
        .unwrap();
    std::fs::remove_dir_all(&path).unwrap();
    assert!(status.success());
    Ok(())
}
