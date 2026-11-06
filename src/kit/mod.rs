//! Register game data, tools and named context recipes once, then validate per-turn snapshots.
//!
//! This opt-in layer composes existing table, view, query and budget APIs. Host-only fields
//! are removed before selection. Hosts still decide which rows an actor may see and must
//! check current permissions/world state in every action callback. See `docs/KIT.md`.
mod bridge;
mod builders;
pub mod evaluation;
mod tools;
mod validation;

pub use bridge::{init, serve, MAX_REQUEST_BYTES};
pub use builders::{Arguments, Dataset, Field, Kind, Parameter, Recipe, Registration, Tool};
pub use tools::{Dispatcher, MAX_CALL_BYTES};

use crate::budget::{assemble_prepared_bytes, Packet, PreparedSection, Section};
use crate::checked as c;
use crate::{obj, query, table, view, Result, Value};
use std::collections::BTreeSet;
use std::sync::Arc;
use validation::*;

/// Maximum serialized project definition, in UTF-8 bytes.
pub const MAX_PROJECT_BYTES: usize = 262_144;
/// Maximum serialized snapshot, in UTF-8 bytes.
pub const MAX_SNAPSHOT_BYTES: usize = 1_048_576;

#[derive(Clone, Debug)]
struct RegisteredRecipe {
    definition: Value,
    stable: Vec<PreparedSection>,
}
/// Validated, reusable project registration. Clones share snapshot ownership.
#[derive(Clone, Debug)]
pub struct Project {
    definition: Value,
    recipes: Vec<RegisteredRecipe>,
    owner: Arc<()>,
}
/// Validated snapshot with host-only fields physically removed. It belongs to its project.
#[derive(Clone, Debug)]
pub struct Snapshot {
    value: Value,
    owner: Arc<()>,
}
/// A model packet plus a separate host-only explanation of selection and budget decisions.
#[derive(Clone, Debug)]
pub struct Built {
    pub packet: Packet,
    pub inspection: Value,
}
impl Built {
    pub fn to_value(&self) -> Value {
        obj! {"packet" => self.packet.value.clone(), "inspection" => self.inspection.clone()}
    }
}
struct Selection<'a> {
    spec: &'a Value,
    rows: Vec<Value>,
    diagnostic: Value,
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    c::list(c::get(v, key)?)
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    c::text(c::get(v, key)?)
}
fn field_names(d: &Value, selection: &Value) -> Result<Vec<String>> {
    if let Some(v) = c::get(selection, "view")?.get("fields") {
        return Ok(strings(v, 64)?.into_iter().map(str::to_owned).collect());
    }
    array(d, "fields")?
        .iter()
        .filter(|f| f.get("visibility").and_then(Value::as_str) == Some("model"))
        .map(|f| text(f, "name").map(str::to_owned))
        .collect::<Result<_>>()
}
fn description(d: &Value, selection: &Value) -> Result<Value> {
    let mut columns = Vec::new();
    for name in field_names(d, selection)? {
        let f = named(array(d, "fields")?, &name)?;
        columns.push(Value::Object(
            ["name", "type", "description", "unit", "nullable"]
                .into_iter()
                .map(|k| (k.to_owned(), f.get(k).expect("validated field").clone()))
                .collect(),
        ));
    }
    Ok(obj! {"description" => c::get(d,"description")?.clone(), "columns" => columns})
}
fn section(name: &str, value: Value, priority: i64, stable: bool, required: bool) -> PreparedSection {
    PreparedSection::new(Section { name: name.into(), value, priority, stable, required })
}
fn validate_dataset(d: &Value) -> Result<()> {
    c::fields(d, &["name", "description", "id", "max_age", "fields"])?;
    if ["schemas", "tools"].contains(&text(d, "name")?) {
        return Err(bad());
    }
    prose(c::get(d, "description")?, 280)?;
    if c::get(d, "max_age")? != &Value::Null {
        c::uint(c::get(d, "max_age")?, 0, u32::MAX as u64)?;
    }
    let fs = array(d, "fields")?;
    unique_names(fs, 64)?;
    if fs.is_empty() {
        return Err(bad());
    }
    for f in fs {
        c::fields(f, &["name", "type", "description", "unit", "nullable", "visibility"])?;
        prose(c::get(f, "description")?, 280)?;
        if c::get(f, "unit")? != &Value::Null {
            prose(c::get(f, "unit")?, 32)?;
        }
        c::boolean(c::get(f, "nullable")?)?;
        if !["string", "number", "integer", "boolean"].contains(&text(f, "type")?)
            || !["model", "host"].contains(&text(f, "visibility")?)
        {
            return Err(bad());
        }
    }
    let id = named(fs, text(d, "id")?)?;
    if text(id, "type")? != "string" || c::boolean(c::get(id, "nullable")?)? || text(id, "visibility")? != "model" {
        return Err(bad());
    }
    Ok(())
}
fn validate_selection(d: &Value, x: &Value) -> Result<()> {
    c::fields(x, &["name", "mode", "view", "priority", "required"])?;
    if !["rows", "schema"].contains(&text(x, "mode")?) {
        return Err(bad());
    }
    priority(c::get(x, "priority")?)?;
    c::boolean(c::get(x, "required")?)?;
    let v = c::get(x, "view")?;
    optional_fields(v, &["fields", "where", "sort_by", "limit"])?;
    let visible = |n: &str| -> Result<()> {
        if text(named(array(d, "fields")?, n)?, "visibility")? != "model" {
            return Err(bad());
        }
        Ok(())
    };
    if let Some(fs) = v.get("fields") {
        let fs = strings(fs, 64)?;
        if fs.is_empty() {
            return Err(bad());
        }
        for f in fs {
            visible(f)?;
        }
    }
    if let Some(ws) = v.get("where") {
        for (n, condition) in ws.as_object().ok_or_else(bad)? {
            visible(n)?;
            if let Some(a) = condition.get("in") {
                if c::list(a)?.len() > 128 {
                    return Err(bad());
                }
            }
        }
    }
    let view = view::View::from_json(v)?;
    if let Some((n, _)) = &view.sort_by {
        visible(n)?;
    }
    if let Some(limit) = v.get("limit") {
        c::uint(limit, 0, 4096)?;
    }
    Ok(())
}
impl Project {
    pub fn parse(text: &str) -> Result<Self> {
        Self::from_value(&crate::json::parse(text, MAX_PROJECT_BYTES)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, MAX_PROJECT_BYTES)?;
        c::fields(v, &["version", "datasets", "tools", "recipes"])?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        let datasets = array(v, "datasets")?;
        let tools = array(v, "tools")?;
        let recipes = array(v, "recipes")?;
        unique_names(datasets, 16)?;
        unique_names(tools, 32)?;
        unique_names(recipes, 32)?;
        for d in datasets {
            validate_dataset(d)?;
        }
        for t in tools {
            c::fields(t, &["name", "description", "parameters"])?;
            prose(c::get(t, "description")?, 1024)?;
            let s = c::get(t, "parameters")?;
            schema(s, 0, &mut 0)?;
            if s.get("type").and_then(Value::as_str) != Some("object") {
                return Err(bad());
            }
        }
        let mut registered = Vec::new();
        for r in recipes {
            c::fields(r, &["name", "budget_bytes", "datasets", "tools", "sections"])?;
            c::uint(c::get(r, "budget_bytes")?, 0, MAX_SNAPSHOT_BYTES as u64)?;
            let selections = array(r, "datasets")?;
            let extras = array(r, "sections")?;
            unique_names(selections, 16)?;
            unique_names(extras, 32)?;
            let mut names = BTreeSet::from(["schemas", "tools"]);
            let mut descriptions = Vec::new();
            for x in selections {
                let n = text(x, "name")?;
                if !names.insert(n) {
                    return Err(bad());
                }
                let d = named(datasets, n)?;
                validate_selection(d, x)?;
                descriptions.push((n.to_owned(), description(d, x)?));
            }
            for x in extras {
                c::fields(x, &["name", "priority", "required"])?;
                if !names.insert(text(x, "name")?) {
                    return Err(bad());
                }
                priority(c::get(x, "priority")?)?;
                c::boolean(c::get(x, "required")?)?;
            }
            let selected_tools = strings(c::get(r, "tools")?, 32)?;
            let mut schemas = Vec::new();
            for n in selected_tools {
                let tool = named(tools, n)?;
                let mut t = ["name", "description", "parameters"]
                    .iter()
                    .map(|k| (k.to_string(), tool.get(k).expect("tool field").clone()))
                    .collect::<Vec<_>>();
                t.push(("strict".into(), Value::Bool(false)));
                schemas.push(obj! {"type" => "function", "function" => Value::Object(t)});
            }
            let mut stable = Vec::new();
            if !schemas.is_empty() {
                stable.push(section("tools", Value::Array(schemas), 1_000_001, true, true));
            }
            if !descriptions.is_empty() {
                stable.push(section("schemas", Value::Object(descriptions), 1_000_001, true, true));
            }
            registered.push(RegisteredRecipe { definition: r.clone(), stable });
        }
        Ok(Self { definition: v.clone(), recipes: registered, owner: Arc::new(()) })
    }
    pub fn to_value(&self) -> Value {
        self.definition.clone()
    }
    fn dataset(&self, name: &str) -> Result<&Value> {
        named(array(&self.definition, "datasets")?, name)
    }
    fn recipe(&self, name: &str) -> Result<&RegisteredRecipe> {
        self.recipes.iter().find(|r| text(&r.definition, "name").ok() == Some(name)).ok_or_else(bad)
    }
    /// Checks names/types/IDs and removes host-only fields before retaining any data.
    /// `now` is the host's clock at capture, not a live clock or permission lease.
    pub fn snapshot(&self, v: &Value) -> Result<Snapshot> {
        bounded(v, MAX_SNAPSHOT_BYTES)?;
        c::fields(v, &["now", "tables", "sections"])?;
        let now = c::uint(c::get(v, "now")?, 0, u32::MAX as u64)?;
        let extras = c::get(v, "sections")?.as_object().ok_or_else(bad)?;
        if extras.len() > 32 {
            return Err(bad());
        }
        for (n, _) in extras {
            c::name(n)?;
        }
        let mut tables = Vec::new();
        for (n, t) in c::get(v, "tables")?.as_object().ok_or_else(bad)? {
            let d = self.dataset(n)?;
            c::fields(t, &["updated_at", "rows"])?;
            c::uint(c::get(t, "updated_at")?, 0, now)?;
            let rows = array(t, "rows")?;
            if rows.len() > 4096 {
                return Err(bad());
            }
            let fs = array(d, "fields")?;
            let names: Vec<_> = fs.iter().map(|f| text(f, "name")).collect::<Result<_>>()?;
            let mut ids = BTreeSet::new();
            let mut public = Vec::with_capacity(rows.len());
            for row in rows {
                c::fields(row, &names)?;
                for f in fs {
                    if !scalar(c::get(row, text(f, "name")?)?, text(f, "type")?, c::boolean(c::get(f, "nullable")?)?) {
                        return Err(bad());
                    }
                }
                let id = c::text(c::get(row, text(d, "id")?)?)?;
                if id.is_empty() || id.chars().count() > 256 || !ids.insert(id) {
                    return Err(bad());
                }
                public.push(Value::Object(
                    fs.iter()
                        .filter(|f| text(f, "visibility").ok() == Some("model"))
                        .map(|f| {
                            let name = text(f, "name").expect("field name");
                            (name.to_owned(), row.get(name).expect("validated row").clone())
                        })
                        .collect(),
                ));
            }
            tables.push((n.clone(), obj! {"updated_at" => c::get(t,"updated_at")?.clone(), "rows" => public}));
        }
        Ok(Snapshot {
            value: obj! {"now" => now, "tables" => Value::Object(tables), "sections" => c::get(v,"sections")?.clone()},
            owner: self.owner.clone(),
        })
    }
    fn selections<'a>(&self, s: &Snapshot, r: &'a RegisteredRecipe) -> Result<Vec<Selection<'a>>> {
        if !Arc::ptr_eq(&self.owner, &s.owner) {
            return Err(bad());
        }
        let now = c::uint(c::get(&s.value, "now")?, 0, u32::MAX as u64)?;
        let mut out = Vec::new();
        for x in array(&r.definition, "datasets")? {
            let n = text(x, "name")?;
            let d = self.dataset(n)?;
            let t = c::get(c::get(&s.value, "tables")?, n)?;
            let updated = c::uint(c::get(t, "updated_at")?, 0, now)?;
            let age = now - updated;
            if let Some(max_age) = c::get(d, "max_age")?.as_usize() {
                if age > max_age as u64 {
                    return Err(bad());
                }
            }
            let source = array(t, "rows")?;
            let v = c::get(x, "view")?;
            let mut view = view::View::from_json(v)?;
            view.fields = None;
            let selected = view::apply(source, &view)?;
            let id = text(d, "id")?;
            let positions: std::collections::BTreeMap<_, _> = source
                .iter()
                .enumerate()
                .map(|(i, row)| (row.get(id).and_then(Value::as_str).expect("ID"), i))
                .collect();
            let indices: Vec<usize> =
                selected.iter().map(|row| positions[row.get(id).and_then(Value::as_str).expect("ID")]).collect();
            let names = field_names(d, x)?;
            let omitted: Vec<_> = array(d, "fields")?
                .iter()
                .map(|f| text(f, "name"))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter(|n| !names.iter().any(|c| c == n))
                .collect();
            let rows = selected
                .into_iter()
                .map(|row| {
                    Value::Object(
                        names.iter().map(|n| (n.clone(), row.get(n).expect("projected field").clone())).collect(),
                    )
                })
                .collect();
            let diagnostic = obj! {"name" => n, "source_rows" => source.len(), "selected_rows" => indices.len(),
            "source_indices" => indices, "fields" => names, "omitted_fields" => omitted,
            "updated_at" => updated, "age" => age, "mode" => text(x,"mode")?, "view" => v.clone()};
            out.push(Selection { spec: x, rows, diagnostic });
        }
        Ok(out)
    }
    /// Builds a bounded packet. Inspection is separate and intended only for the host.
    pub fn build(&self, recipe: &str, snapshot: &Snapshot) -> Result<Built> {
        let r = self.recipe(recipe)?;
        let selections = self.selections(snapshot, r)?;
        let mut sections = Vec::new();
        for s in &selections {
            if text(s.spec, "mode")? == "rows" {
                sections.push(section(
                    text(s.spec, "name")?,
                    table::pack(&s.rows, &[])?,
                    priority(c::get(s.spec, "priority")?)?,
                    false,
                    c::boolean(c::get(s.spec, "required")?)?,
                ));
            }
        }
        let extras = c::get(&snapshot.value, "sections")?;
        let mut missing = Vec::new();
        for x in array(&r.definition, "sections")? {
            let n = text(x, "name")?;
            if let Some(v) = extras.get(n) {
                sections.push(section(
                    n,
                    v.clone(),
                    priority(c::get(x, "priority")?)?,
                    false,
                    c::boolean(c::get(x, "required")?)?,
                ));
            } else if c::boolean(c::get(x, "required")?)? {
                return Err(bad());
            } else {
                missing.push(n);
            }
        }
        let prepared = r.stable.iter().chain(&sections).collect::<Vec<_>>();
        let cap = c::get(&r.definition, "budget_bytes")?.as_usize().ok_or_else(bad)?;
        let packet = assemble_prepared_bytes(&prepared, cap)?;
        let datasets = selections
            .into_iter()
            .map(|s| {
                let included = text(s.spec, "mode").ok() == Some("schema")
                    || packet.value.get(text(s.spec, "name").expect("name")).is_some();
                let Value::Object(mut d) = s.diagnostic else { unreachable!() };
                d.push(("included".into(), Value::Bool(included)));
                Value::Object(d)
            })
            .collect::<Vec<_>>();
        let inspection = obj! {"recipe" => recipe, "receipt" => packet.receipt.to_value(), "datasets" => datasets, "missing_sections" => missing};
        Ok(Built { packet, inspection })
    }
    /// Executes only against this recipe's filtered/projected/limited model-visible rows.
    /// Dropping a packed row section for budget does not revoke its registered query schema.
    pub fn query(&self, recipe: &str, snapshot: &Snapshot, sql: &str) -> Result<Value> {
        let selections = self.selections(snapshot, self.recipe(recipe)?)?;
        let tables =
            selections.iter().map(|s| Ok((text(s.spec, "name")?, s.rows.as_slice()))).collect::<Result<Vec<_>>>()?;
        let parsed = query::Query::parse(sql)?;
        let selection =
            selections.iter().find(|s| text(s.spec, "name").ok() == Some(parsed.table())).ok_or_else(bad)?;
        let columns = field_names(self.dataset(parsed.table())?, selection.spec)?;
        for name in array(&parsed.plan(), "columns")? {
            if !columns.iter().any(|c| Some(c.as_str()) == name.as_str()) {
                return Err(bad());
            }
        }
        let rows = tables.iter().find(|(n, _)| *n == parsed.table()).map(|(_, r)| *r).ok_or_else(bad)?;
        parsed.run_bounded(rows, 4096, 65_536)
    }
    /// Derives numeric brain bindings with units from the same registration. Host supplies
    /// ordered ranges/policies; private fields cannot become exposed model features here.
    pub fn feature_schema(
        &self,
        dataset: &str,
        revision: usize,
        policies: &Value,
    ) -> Result<crate::brain::FeatureSchema> {
        bounded(policies, 16_384)?;
        let d = self.dataset(dataset)?;
        let policies = c::list(policies)?;
        if policies.len() > 16 {
            return Err(bad());
        }
        let mut features = Vec::new();
        for p in policies {
            c::fields(p, &["name", "min", "max", "invert", "missing", "outside"])?;
            let f = named(array(d, "fields")?, text(p, "name")?)?;
            if !["number", "integer"].contains(&text(f, "type")?) || text(f, "visibility")? != "model" {
                return Err(bad());
            }
            let mut fields =
                vec![("name".into(), c::get(p, "name")?.clone()), ("unit".into(), c::get(f, "unit")?.clone())];
            fields.extend(p.as_object().expect("policy").iter().filter(|(k, _)| k != "name").cloned());
            features.push(Value::Object(fields));
        }
        crate::brain::FeatureSchema::from_value(
            &obj! {"version" => 1, "id" => dataset, "revision" => revision, "features" => features},
        )
    }
    /// Static diagnostics. Invalid definitions fail registration; these are valid but
    /// potentially surprising configurations, not inferred unit correctness.
    pub fn doctor(&self) -> Value {
        let mut warnings = Vec::new();
        for d in array(&self.definition, "datasets").expect("datasets") {
            let n = text(d, "name").expect("name");
            if d.get("max_age") == Some(&Value::Null) {
                warnings.push(obj! {"code" => "unbounded_age", "path" => n});
            }
            for f in array(d, "fields").expect("fields") {
                if [Some("number"), Some("integer")].contains(&f.get("type").and_then(Value::as_str))
                    && f.get("unit") == Some(&Value::Null)
                {
                    warnings.push(
                        obj! {"code" => "missing_unit", "path" => format!("{}.{}",n,text(f,"name").expect("field"))},
                    );
                }
            }
        }
        obj! {"version" => 1, "datasets" => array(&self.definition,"datasets").expect("datasets").len(), "tools" => array(&self.definition,"tools").expect("tools").len(), "recipes" => self.recipes.len(), "warnings" => warnings}
    }
}

/// Stateless reference adapter used by conformance cases and `gamectx kit`.
pub fn process(v: &Value) -> Result<Value> {
    bounded(v, MAX_REQUEST_BYTES)?;
    c::fields(v, &["project", "op", "args"])?;
    let p = Project::from_value(c::get(v, "project")?)?;
    p.execute(text(v, "op")?, c::get(v, "args")?)
}
