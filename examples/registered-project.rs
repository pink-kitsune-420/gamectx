//! Shared registration, bounded context, exact queries and validated host callbacks.
use gamectx::{kit::Project, obj, Result, Value};

pub fn main() -> Result<()> {
    from_rust()?;
    query_workflow()?;
    let project = Project::parse(include_str!("kit/project.json"))?;
    let data = gamectx::json::parse(include_str!("kit/snapshot.json"), 1 << 20)?;
    let snapshot = project.snapshot(&data)?;
    let built = project.build("conversation", &snapshot)?;
    assert_eq!(built.packet.receipt.total_bytes, 646);
    assert!(!built.packet.text.contains("host-only"));
    assert_eq!(project.query("conversation", &snapshot, "SELECT COUNT(*) FROM stock")?, Value::int(3));

    let mut dispatch = project.dispatcher();
    dispatch.register("inspect_item", |args| {
        // Replace with a live lookup that also checks this actor's current permissions.
        if args.get("id").and_then(Value::as_str) != Some("grove:item_0") {
            return Err(gamectx::Error::new("item unavailable"));
        }
        Ok(obj! {"id" => "grove:item_0", "available" => true})
    })?;
    let result = dispatch.dispatch("conversation", "inspect_item", &obj! {"id" => "grove:item_0"});
    assert_eq!(result.get("ok"), Some(&Value::Bool(true)));
    let invalid = dispatch.dispatch("conversation", "inspect_item", &obj! {"id" => 42});
    assert_eq!(invalid.get("ok"), Some(&Value::Bool(false)));

    let schema = project.feature_schema(
        "stock",
        1,
        &vec![obj! {
            "name" => "count", "min" => 0, "max" => 100, "invert" => false,
            "missing" => "unknown", "outside" => "clamp"
        }]
        .into(),
    )?;
    assert_eq!(schema.normalize(&obj! {"count" => 25})?.as_slice(), &[Some(250_000)]);
    Ok(())
}

/// The same registration can be authored from Rust without handwritten tool schemas.
pub fn from_rust() -> Result<()> {
    use gamectx::kit::{Arguments, Dataset, Field, Kind, Parameter, Recipe, Registration, Tool};
    let project = Registration::new()
        .dataset(Dataset::new("stock", "Items visible to this actor.", "id").max_age(5)
            .field(Field::new("id", Kind::String, "id"))
            .field(Field::new("kind", Kind::String, "kind"))
            .field(Field::new("count", Kind::Integer, "count").unit("items"))
            .field(Field::new("secret", Kind::String, "secret").host_only()))
        .tool(Tool::new("inspect_item", "Inspect one currently accessible item.",
            Arguments::new().required("id", Parameter::string(256))))
        .recipe(Recipe::new("conversation",4096)
            .rows("stock", obj! {"where" => obj! {"kind" => "food"}, "sort_by" => "-count", "limit" => 3, "fields" => vec!["id","count"]},5,false)
            .tool("inspect_item").section("event",8,true))
        .build()?;
    assert!(project.to_value().same(&Project::parse(include_str!("kit/project.json"))?.to_value()));
    Ok(())
}

/// Schema-only model query calls use the same recipe visibility as packed tables.
pub fn query_workflow() -> Result<()> {
    use gamectx::kit::{Dataset, Field, Kind, Recipe, Registration, Tool};
    let project = Registration::new()
        .dataset(
            Dataset::new("stock", "Actor-visible supplies.", "id")
                .max_age(1)
                .field(Field::new("id", Kind::String, "Item identifier."))
                .field(Field::new("count", Kind::Integer, "Units in this stack.").unit("items")),
        )
        .tool(Tool::from_function(&gamectx::query::tool())?)
        .recipe(Recipe::new("decision", 4096).schema("stock", obj! {}).tool("query"))
        .build()?;
    let snapshot = project.snapshot(&obj! {"now" => 1,"tables" => obj! {"stock" => obj! {
        "updated_at" => 1,"rows" => vec![obj! {"id" => "grove:lamp", "count" => 3}]
    }}, "sections" => obj!{}})?;
    assert!(!project.build("decision", &snapshot)?.packet.text.contains("\"rows\""));
    let mut dispatch = project.dispatcher();
    dispatch.register("query", |args| {
        project.query("decision", &snapshot, args.get("sql").and_then(Value::as_str).unwrap())
    })?;
    assert_eq!(
        dispatch.dispatch("decision", "query", &obj! {"sql" => "SELECT SUM(count) FROM stock"}),
        obj! {"ok" => true,"result" => 3}
    );
    Ok(())
}
