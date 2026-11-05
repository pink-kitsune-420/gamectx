//! Tool (function-calling) schema packing.
//!
//! [`pack`] is lossless: each tool becomes a row, and parameter rules repeated across tools
//! move into shared `$defs` when that saves bytes. [`outline`] is a deliberately lossy
//! one-line summary such as `cast(spell=ward|spark,power:1..5)` for prompts that only need
//! to remind a model which actions exist.

use crate::json::{Error, Result, Value};
use std::collections::HashMap;

pub const MAX_TOOLS: usize = 256;
pub const MAX_FIELDS: usize = 64;
pub const MAX_DEFS: usize = 256;

const COLUMNS: [&str; 4] = ["name", "description", "properties", "extra"];
const EXTRA_NOTE: &str =
    "extra null = strict:true and parameters {type:object,additionalProperties:false,required:all property keys}";

/// One tool definition with its wrapper removed.
struct Tool<'a> {
    name: &'a str,
    description: Option<&'a str>,
    strict: Option<&'a Value>,
    properties: &'a [(String, Value)],
    /// Parameter keys other than `type` and `properties`, in source order.
    parameter_rest: Vec<(String, Value)>,
}

/// Accepts OpenAI-style `{"type":"function","function":{...}}` items or bare `{name,...}` items.
fn read_tools(tools: &[Value]) -> Result<(bool, Vec<Tool<'_>>)> {
    if tools.len() > MAX_TOOLS {
        return Err(Error::new(format!("{} tools; limit is {MAX_TOOLS}", tools.len())));
    }
    let wrapped = tools.first().is_some_and(|t| t.get("function").is_some());
    let mut out = Vec::with_capacity(tools.len());
    for (n, item) in tools.iter().enumerate() {
        let function = if wrapped {
            let keys = keys(item);
            if (keys != ["type", "function"] && keys != ["function", "type"])
                || item.get("type").and_then(Value::as_str) != Some("function")
            {
                return Err(Error::new(format!(
                    "tool {n}: expected {{\"type\":\"function\",\"function\":{{...}}}} like tool 0"
                )));
            }
            item.get("function").unwrap()
        } else {
            item
        };
        for key in keys(function) {
            if !["name", "description", "strict", "parameters"].contains(&key) {
                return Err(Error::new(format!("tool {n}: unsupported key \"{key}\"")));
            }
        }
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new(format!("tool {n}: name must be a string")))?;
        let description = match function.get("description") {
            None => None,
            Some(v) => {
                Some(v.as_str().ok_or_else(|| Error::new(format!("tool {name}: description must be a string")))?)
            }
        };
        let parameters =
            function.get("parameters").ok_or_else(|| Error::new(format!("tool {name}: missing parameters")))?;
        if parameters.get("type").and_then(Value::as_str) != Some("object") {
            return Err(Error::new(format!("tool {name}: parameters.type must be \"object\"")));
        }
        let properties = parameters
            .get("properties")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::new(format!("tool {name}: parameters.properties must be an object")))?;
        if properties.len() > MAX_FIELDS {
            return Err(Error::new(format!("tool {name}: {} properties; limit is {MAX_FIELDS}", properties.len())));
        }
        if properties.iter().any(|(_, rule)| rule.get("$ref").is_some()) {
            return Err(Error::new(format!("tool {name}: properties already use $ref; pack the expanded schema")));
        }
        let parameter_rest =
            parameters.as_object().unwrap().iter().filter(|(k, _)| k != "type" && k != "properties").cloned().collect();
        out.push(Tool { name, description, strict: function.get("strict"), properties, parameter_rest });
    }
    Ok((wrapped, out))
}

fn keys(value: &Value) -> Vec<&str> {
    value.as_object().map(|o| o.iter().map(|(k, _)| k.as_str()).collect()).unwrap_or_default()
}

/// True when the tool uses the common strict defaults, so its `extra` cell can be null.
fn is_strict_default(tool: &Tool) -> bool {
    let required: Vec<Value> = tool.properties.iter().map(|(k, _)| Value::str(k)).collect();
    tool.description.is_some()
        && tool.strict == Some(&Value::Bool(true))
        && tool.parameter_rest.len() == 2
        && tool.parameter_rest.iter().any(|(k, v)| k == "additionalProperties" && *v == Value::Bool(false))
        && tool.parameter_rest.iter().any(|(k, v)| k == "required" && *v == Value::Array(required.clone()))
}

/// Losslessly packs tool schemas into rows plus shared `$defs`.
pub fn pack(tools: &[Value]) -> Result<Value> {
    let (wrapped, tools) = read_tools(tools)?;

    // Count identical property rules across all tools, keyed by compact JSON.
    let mut order: Vec<String> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut rules: HashMap<String, Value> = HashMap::new();
    for tool in &tools {
        for (_, rule) in tool.properties {
            let key = rule.to_json();
            let count = counts.entry(key.clone()).or_insert(0);
            if *count == 0 {
                order.push(key.clone());
                rules.insert(key.clone(), rule.clone());
            }
            *count += 1;
        }
    }

    // A shared definition must save more than its own entry costs.
    let mut defs = Vec::new();
    let mut refs: HashMap<String, Value> = HashMap::new();
    for key in order {
        let count = counts[&key];
        let id = format!("p{}", defs.len());
        let reference = Value::object(vec![("$ref", Value::str(format!("#/$defs/{id}")))]);
        let ref_len = reference.json_len();
        if count < 2 || key.len().saturating_sub(ref_len) * count <= key.len() + id.len() + 4 {
            continue;
        }
        if defs.len() >= MAX_DEFS {
            break;
        }
        defs.push((id, rules[&key].clone()));
        refs.insert(key, reference);
    }

    let mut rows = Vec::with_capacity(tools.len());
    for tool in &tools {
        let properties = tool
            .properties
            .iter()
            .map(|(k, rule)| (k.clone(), refs.get(&rule.to_json()).cloned().unwrap_or_else(|| rule.clone())))
            .collect();
        let extra = if is_strict_default(tool) {
            Value::Null
        } else {
            let mut extra = Vec::new();
            if let Some(strict) = tool.strict {
                extra.push(("strict".to_owned(), strict.clone()));
            }
            extra.push(("parameters".to_owned(), Value::Object(tool.parameter_rest.clone())));
            Value::Object(extra)
        };
        rows.push(Value::Array(vec![
            Value::str(tool.name),
            tool.description.map_or(Value::Null, Value::str),
            Value::Object(properties),
            extra,
        ]));
    }

    let mut out = vec![
        ("wrapper", Value::str(if wrapped { "function" } else { "bare" })),
        ("columns", Value::strings(&COLUMNS)),
        ("extra_null", Value::str(EXTRA_NOTE)),
    ];
    if !defs.is_empty() {
        out.push(("$defs", Value::Object(defs)));
    }
    out.push(("rows", Value::Array(rows)));
    Ok(Value::object(out))
}

/// Restores the original tool list from [`pack`] output.
pub fn unpack(packed: &Value) -> Result<Vec<Value>> {
    let wrapped = match packed.get("wrapper").and_then(Value::as_str) {
        Some("function") => true,
        Some("bare") => false,
        _ => return Err(Error::new("packed schema needs wrapper \"function\" or \"bare\"")),
    };
    let defs = packed.get("$defs").and_then(Value::as_object).unwrap_or(&[]);
    let rows = packed.get("rows").and_then(Value::as_array).ok_or_else(|| Error::new("packed schema needs rows"))?;
    let mut tools = Vec::with_capacity(rows.len());
    for (n, row) in rows.iter().enumerate() {
        let cells = row
            .as_array()
            .filter(|c| c.len() == 4)
            .ok_or_else(|| Error::new(format!("schema row {n} needs 4 cells")))?;
        let name = cells[0].as_str().ok_or_else(|| Error::new(format!("schema row {n}: name must be a string")))?;
        let props_in =
            cells[2].as_object().ok_or_else(|| Error::new(format!("schema row {n}: properties must be an object")))?;
        let mut properties = Vec::with_capacity(props_in.len());
        for (key, rule) in props_in {
            let expanded = match rule.get("$ref").and_then(Value::as_str) {
                Some(target) => {
                    let id = target
                        .strip_prefix("#/$defs/")
                        .ok_or_else(|| Error::new(format!("schema row {n}: bad $ref")))?;
                    defs.iter()
                        .find(|(k, _)| k == id)
                        .map(|(_, v)| v.clone())
                        .ok_or_else(|| Error::new(format!("schema row {n}: unknown $ref {id}")))?
                }
                None => rule.clone(),
            };
            properties.push((key.clone(), expanded));
        }
        let mut parameters = vec![
            ("type".to_owned(), Value::str("object")),
            ("properties".to_owned(), Value::Object(properties.clone())),
        ];
        let mut function = vec![("name".to_owned(), Value::str(name))];
        if !cells[1].eq(&Value::Null) {
            function.push(("description".to_owned(), cells[1].clone()));
        }
        match &cells[3] {
            Value::Null => {
                function.push(("strict".to_owned(), Value::Bool(true)));
                parameters.push((
                    "required".to_owned(),
                    Value::Array(properties.iter().map(|(k, _)| Value::str(k)).collect()),
                ));
                parameters.push(("additionalProperties".to_owned(), Value::Bool(false)));
            }
            extra => {
                if let Some(strict) = extra.get("strict") {
                    function.push(("strict".to_owned(), strict.clone()));
                }
                let rest = extra
                    .get("parameters")
                    .and_then(Value::as_object)
                    .ok_or_else(|| Error::new(format!("schema row {n}: extra.parameters must be an object")))?;
                parameters.extend(rest.iter().cloned());
            }
        }
        function.push(("parameters".to_owned(), Value::Object(parameters)));
        let function = Value::Object(function);
        tools.push(if wrapped {
            Value::object(vec![("type", Value::str("function")), ("function", function)])
        } else {
            function
        });
    }
    Ok(tools)
}

/// Lossy one-line signatures: `name(arg=a|b,arg:min..max,arg);...`. Descriptions, types and
/// nested structure are dropped, so use it as a reminder next to a full schema, not instead of one.
pub fn outline(tools: &[Value]) -> Result<String> {
    let (_, tools) = read_tools(tools)?;
    let mut lines = Vec::with_capacity(tools.len());
    for tool in &tools {
        let args: Vec<String> = tool
            .properties
            .iter()
            .map(|(name, rule)| {
                if let Some(options) = rule.get("enum").and_then(Value::as_array) {
                    let options: Vec<String> =
                        options.iter().map(|o| o.as_str().map_or_else(|| o.to_json(), str::to_owned)).collect();
                    format!("{name}={}", options.join("|"))
                } else if let (Some(Value::Number(min)), Some(Value::Number(max))) =
                    (rule.get("minimum"), rule.get("maximum"))
                {
                    format!("{name}:{min}..{max}")
                } else {
                    name.clone()
                }
            })
            .collect();
        lines.push(format!("{}({})", tool.name, args.join(",")));
    }
    Ok(lines.join(";"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    const TOOLS: &str = r#"[
      {"type":"function","function":{"name":"cast","description":"Cast a spell.","strict":true,"parameters":{"type":"object","properties":{"spell":{"type":"string","enum":["ward","spark"]},"power":{"type":"integer","minimum":1,"maximum":5}},"required":["spell","power"],"additionalProperties":false}}},
      {"type":"function","function":{"name":"charge","description":"Charge a relic.","strict":true,"parameters":{"type":"object","properties":{"relic":{"type":"string"},"power":{"type":"integer","minimum":1,"maximum":5}},"required":["relic","power"],"additionalProperties":false}}},
      {"type":"function","function":{"name":"wait","parameters":{"type":"object","properties":{"seconds":{"type":"integer","minimum":1,"maximum":5}},"required":[]}}}
    ]"#;

    fn tools() -> Vec<Value> {
        parse(TOOLS, 1 << 20).unwrap().as_array().unwrap().to_vec()
    }

    #[test]
    fn pack_is_lossless_and_smaller() {
        let source = tools();
        let packed = pack(&source).unwrap();
        assert!(packed.get("$defs").is_some(), "repeated power rule should be shared");
        assert!(packed.json_len() < Value::Array(source.clone()).json_len());
        assert!(Value::Array(unpack(&packed).unwrap()).same(&Value::Array(source)));
    }

    #[test]
    fn bare_tools_round_trip() {
        let bare: Vec<Value> = tools().iter().map(|t| t.get("function").unwrap().clone()).collect();
        let packed = pack(&bare).unwrap();
        assert!(Value::Array(unpack(&packed).unwrap()).same(&Value::Array(bare)));
    }

    #[test]
    fn outline_lists_enums_and_ranges() {
        assert_eq!(
            outline(&tools()).unwrap(),
            "cast(spell=ward|spark,power:1..5);charge(relic,power:1..5);wait(seconds:1..5)"
        );
    }
}
