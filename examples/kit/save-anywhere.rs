//! Save anywhere: one manager saved to text and loaded into a fresh one, a damaged save refused
//! without changing anything, and a bundle that saves several managers as one document.
use gamectx::character::projects::Projects;
use gamectx::data::json::parse;
use gamectx::data::persist::{self, Durable};
use gamectx::kit::state_bundle::StateBundle;
use gamectx::{Result, Value};

const PROPOSE: &str = r#"{"ops":[{"op":"propose","args":{"id":"brewery","owner":"wen","aim":"Open a small brewery","stake":70,"horizon":500,"milestones":[{"id":"clear_plot","condition":{"kind":"task","ref":"clear_plot"},"expected":40},{"id":"first_brew","condition":{"kind":"fact","ref":"ale_tasted"},"expected":60}]}},{"op":"tick","args":{"now":12}}]}"#;
const NEXT: &str = r#"{"ops":[{"op":"outcome","args":{"project":"brewery","milestone":"clear_plot","success":true}},{"op":"weight","args":{"project":"brewery"}}]}"#;

pub fn main() -> Result<()> {
    let mut wen = Projects::new();
    wen.workflow(&parse(PROPOSE, 1 << 16)?)?;

    // Save to text (a game writes this to disk) and load it into a fresh manager. The save is
    // checked in full first: types, ranges, canonical form and every invariant the ops enforce.
    let limits = Durable::limits(&wen).clone();
    let text = persist::export(&wen, &limits).to_json();
    let (mut loaded, version, _saved_limits) = persist::restore::<Projects>(&parse(&text, 1 << 26)?, &limits)?;
    assert_eq!(version, 1);
    // The loaded manager carries on exactly where the saved one was.
    let next = parse(NEXT, 1 << 16)?;
    assert_eq!(loaded.workflow(&next)?, wen.workflow(&next)?);

    // A damaged save (a stake above 100) is refused by `import`, and the manager is unchanged.
    let bad = text.replacen(r#""stake":70"#, r#""stake":700"#, 1);
    let import = parse(&format!(r#"{{"ops":[{{"op":"import","args":{{"save":{bad}}}}}]}}"#), 1 << 26)?;
    let before = loaded.clone();
    assert!(loaded.workflow(&import).is_err());
    assert_eq!(loaded, before);

    // A bundle runs several managers in place and saves them all as one document, ascending by kind.
    let mut town = StateBundle::new();
    let run = |kind: &str, request: &str| format!(r#"{{"op":"run","args":{{"kind":"{kind}","request":{request}}}}}"#);
    let style = r#"{"ops":[{"op":"style","args":{"being":"wen","humour":"dry","tempo":"measured","formality":30,"assertiveness":60}}]}"#;
    let ops = format!(
        r#"{{"ops":[{},{},{{"op":"export","args":{{}}}}]}}"#,
        run("projects", PROPOSE),
        run("chemistry", style)
    );
    let out = town.workflow(&parse(&ops, 1 << 20)?)?;
    let save = out.get("results").and_then(Value::as_array).and_then(|r| r.last()).cloned().unwrap_or(Value::Null);
    let kinds: Vec<&str> = save
        .get("sections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_array()?.first()?.as_str())
        .collect();
    assert_eq!(kinds, ["chemistry", "projects"]);
    // An import checks every section before replacing any of them.
    let mut later = StateBundle::new();
    let import = Value::Object(vec![(
        "ops".into(),
        Value::Array(vec![Value::Object(vec![
            ("op".into(), Value::from("import")),
            ("args".into(), Value::Object(vec![("save".into(), save.clone())])),
        ])]),
    )]);
    later.workflow(&import)?;
    assert_eq!(later.export(), save);
    Ok(())
}
