//! Host intent boundaries and typed queries; no language-model call required here.
use gamectx::{decision::Choices, obj, query::Template, Result, Value};
pub fn main() -> Result<()> {
    let inspect = obj! {"action"=>"inspect","target"=>"grove:gate"};
    let open = obj! {"action"=>"open","target"=>"grove:gate"};
    let choices = Choices::from_value(&obj! {"version"=>1,"revision"=>7,
    "choices"=>vec![inspect.clone(),open],"fallback"=>Value::Null})?;
    // This subset is host-declared. The library does not interpret the request.
    let mut session = choices.with_intent(&obj! {"request"=>obj! {"id"=>"r7","text"=>"Inspect; leave it shut."},
    "facts"=>obj! {"gate_closed"=>true},"allowed"=>vec![inspect]})?;
    let reply = session.apply(r#"{"action":"open","target":"grove:gate"}"#, 7, |_| true)?;
    assert_eq!(reply.get("status").and_then(Value::as_str), Some("retry"));
    assert!(reply.get("correction").is_some());
    let template = Template::from_value(&obj! {"name"=>"count_kind","description"=>"Count visible rows of one kind.",
    "sql"=>"SELECT COUNT(*) FROM stock","parameters"=>vec![obj! {"name"=>"kind","column"=>"kind","op"=>"=","kind"=>"string"}],
    "max_rows"=>4096,"result_bytes"=>512})?;
    let rows = vec![obj! {"kind"=>"food"}, obj! {"kind"=>"tool"}];
    assert_eq!(template.run(&obj! {"kind"=>"food"}, &rows)?.to_json(), "1");
    assert_eq!(template.run(&obj! {"kind"=>"food' OR kind = 'tool"}, &rows)?.to_json(), "0");
    // Register template.tool() with the kit; in its host callback use
    // Project::query_template(recipe, snapshot, &template, args) to retain view boundaries.
    Ok(())
}
