//! Optional Rust constructors for the same version-1 project definition used by the CLI.
use super::{Project, Result};
use crate::{obj, Value};

/// Registered scalar field type.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    String,
    Number,
    Integer,
    Boolean,
}
impl Kind {
    fn text(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
        }
    }
}
/// Field registration; visible and non-null by default. Units are never inferred.
#[derive(Clone, Debug)]
pub struct Field {
    value: Value,
}
impl Field {
    pub fn new(name: &str, kind: Kind, description: &str) -> Self {
        Self {
            value: obj! {"name" => name, "type" => kind.text(), "description" => description,
            "unit" => Value::Null, "nullable" => false, "visibility" => "model"},
        }
    }
    pub fn unit(mut self, unit: &str) -> Self {
        replace(&mut self.value, "unit", Value::str(unit));
        self
    }
    pub fn nullable(mut self) -> Self {
        replace(&mut self.value, "nullable", Value::Bool(true));
        self
    }
    pub fn host_only(mut self) -> Self {
        replace(&mut self.value, "visibility", Value::str("host"));
        self
    }
}
/// A dataset's schema and capture-age policy; rows arrive separately in snapshots.
#[derive(Clone, Debug)]
pub struct Dataset {
    name: String,
    description: String,
    id: String,
    age: Option<u32>,
    fields: Vec<Field>,
}
impl Dataset {
    pub fn new(name: &str, description: &str, id: &str) -> Self {
        Self { name: name.into(), description: description.into(), id: id.into(), age: None, fields: Vec::new() }
    }
    pub fn max_age(mut self, ticks: u32) -> Self {
        self.age = Some(ticks);
        self
    }
    pub fn field(mut self, field: Field) -> Self {
        self.fields.push(field);
        self
    }
    fn value(self) -> Value {
        obj! {"name" => self.name, "description" => self.description, "id" => self.id,
        "max_age" => self.age.map(Value::from).unwrap_or(Value::Null), "fields" => self.fields.into_iter().map(|f|f.value).collect::<Vec<_>>()}
    }
}
/// Bounded tool argument schema. Use `schema` for the supported advanced subset.
#[derive(Clone, Debug)]
pub struct Parameter {
    value: Value,
}
impl Parameter {
    pub fn schema(value: Value) -> Self {
        Self { value }
    }
    pub fn string(max_chars: u32) -> Self {
        Self::schema(obj! {"type" => "string", "maxLength" => max_chars})
    }
    pub fn integer() -> Self {
        Self::schema(obj! {"type" => "integer"})
    }
    pub fn number() -> Self {
        Self::schema(obj! {"type" => "number"})
    }
    pub fn boolean() -> Self {
        Self::schema(obj! {"type" => "boolean"})
    }
    pub fn array(items: Parameter, max_items: u32) -> Self {
        Self::schema(obj! {"type" => "array", "maxItems" => max_items, "items" => items.value})
    }
    pub fn object(arguments: Arguments) -> Self {
        Self::schema(arguments.value())
    }
    pub fn nullable(mut self) -> Self {
        if let Some(t) = self.value.get("type").and_then(Value::as_str).map(str::to_owned) {
            replace(&mut self.value, "type", Value::strings(&[t.as_str(), "null"]));
        }
        self
    }
    pub fn minimum(mut self, value: impl Into<Value>) -> Self {
        replace(&mut self.value, "minimum", value.into());
        self
    }
    pub fn maximum(mut self, value: impl Into<Value>) -> Self {
        replace(&mut self.value, "maximum", value.into());
        self
    }
    pub fn description(mut self, value: &str) -> Self {
        replace(&mut self.value, "description", Value::str(value));
        self
    }
    pub fn enum_values(mut self, values: Vec<Value>) -> Self {
        replace(&mut self.value, "enum", Value::Array(values));
        self
    }
}
/// Object arguments are closed; unknown fields are always rejected.
#[derive(Clone, Debug, Default)]
pub struct Arguments {
    properties: Vec<(String, Value)>,
    required: Vec<String>,
}
impl Arguments {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn required(mut self, name: &str, parameter: Parameter) -> Self {
        self.required.push(name.into());
        self.properties.push((name.into(), parameter.value));
        self
    }
    pub fn optional(mut self, name: &str, parameter: Parameter) -> Self {
        self.properties.push((name.into(), parameter.value));
        self
    }
    fn value(self) -> Value {
        obj! {"type" => "object", "properties" => Value::Object(self.properties), "required" => self.required, "additionalProperties" => false}
    }
}
/// A tool declaration; callback registration happens separately on the dispatcher.
#[derive(Clone, Debug)]
pub struct Tool {
    value: Value,
}
impl Tool {
    /// Imports an existing function tool, including `query::tool()`. Provider strict
    /// flags are rebuilt by the project; unsupported parameter keywords fail build.
    pub fn from_function(tool: &Value) -> Result<Self> {
        super::validation::bounded(tool, 65_536)?;
        let f = tool.get("function").unwrap_or(tool);
        super::validation::optional_fields(f, &["name", "description", "parameters", "strict"])?;
        if let Some(strict) = f.get("strict") {
            crate::checked::boolean(strict)?;
        }
        Ok(Self {
            value: obj! {"name" => crate::checked::get(f,"name")?.clone(), "description" => crate::checked::get(f,"description")?.clone(), "parameters" => crate::checked::get(f,"parameters")?.clone()},
        })
    }

    pub fn new(name: &str, description: &str, arguments: Arguments) -> Self {
        Self { value: obj! {"name" => name,"description" => description,"parameters" => arguments.value()} }
    }
}
/// A named selection recipe. `view` uses the existing fields/where/sort_by/limit format.
#[derive(Clone, Debug)]
pub struct Recipe {
    name: String,
    budget: usize,
    datasets: Vec<Value>,
    tools: Vec<String>,
    sections: Vec<Value>,
}
impl Recipe {
    pub fn new(name: &str, budget_bytes: usize) -> Self {
        Self { name: name.into(), budget: budget_bytes, datasets: Vec::new(), tools: Vec::new(), sections: Vec::new() }
    }
    pub fn rows(self, dataset: &str, view: Value, priority: i64, required: bool) -> Self {
        self.dataset(dataset, "rows", view, priority, required)
    }
    pub fn schema(self, dataset: &str, view: Value) -> Self {
        self.dataset(dataset, "schema", view, 5, true)
    }
    fn dataset(mut self, name: &str, mode: &str, view: Value, priority: i64, required: bool) -> Self {
        self.datasets
            .push(obj! {"name" => name,"mode" => mode,"view" => view,"priority" => priority,"required" => required});
        self
    }
    pub fn tool(mut self, name: &str) -> Self {
        self.tools.push(name.into());
        self
    }
    /// Adds a named per-snapshot value, e.g. already filtered memory/facts or the event.
    pub fn section(mut self, name: &str, priority: i64, required: bool) -> Self {
        self.sections.push(obj! {"name" => name,"priority" => priority,"required" => required});
        self
    }
    fn value(self) -> Value {
        obj! {"name" => self.name,"budget_bytes" => self.budget,"datasets" => self.datasets,"tools" => self.tools,"sections" => self.sections}
    }
}
/// All builders validate together at `build`, including cross-references and duplicates.
#[derive(Clone, Debug, Default)]
pub struct Registration {
    datasets: Vec<Dataset>,
    tools: Vec<Tool>,
    recipes: Vec<Recipe>,
}
impl Registration {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn dataset(mut self, dataset: Dataset) -> Self {
        self.datasets.push(dataset);
        self
    }
    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }
    pub fn recipe(mut self, recipe: Recipe) -> Self {
        self.recipes.push(recipe);
        self
    }
    pub fn build(self) -> Result<Project> {
        Project::from_value(
            &obj! {"version" => 1,"datasets" => self.datasets.into_iter().map(Dataset::value).collect::<Vec<_>>(),
            "tools" => self.tools.into_iter().map(|t|t.value).collect::<Vec<_>>(),"recipes" => self.recipes.into_iter().map(Recipe::value).collect::<Vec<_>>()},
        )
    }
}
fn replace(v: &mut Value, key: &str, value: Value) {
    if let Value::Object(fields) = v {
        if let Some((_, old)) = fields.iter_mut().find(|(n, _)| n == key) {
            *old = value;
        } else {
            fields.push((key.into(), value));
        }
    }
}
