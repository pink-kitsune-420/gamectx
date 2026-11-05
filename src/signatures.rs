//! Action signatures: tool schemas written as short text lines instead of JSON Schema.
//!
//! ```text
//! All arguments are required; no other arguments are allowed.
//! Shared arguments (same meaning wherever listed):
//!   target: string; Entity handle.
//!   power: integer 1..5
//! cast(spell, target, power): Cast a spell.
//!   spell: one of spark|hush
//! guard(target, power): Guard someone.
//! ```
//!
//! An argument whose name and rule are identical in two or more actions, and whose name
//! means nothing else, is described once under "Shared arguments". In model tests this
//! used 12–14% fewer input tokens than packed JSON Schema with no loss of accuracy on
//! schema questions or action choice.
//!
//! Signatures are lossless for strict tools whose arguments are strings, numbers,
//! integers, booleans or string enums, optionally with `minimum`, `maximum`,
//! `maxLength` and `description`. Anything else is refused; use [`crate::schema`].

use crate::json::{self, Error, Result, Value};
use crate::schema::{is_strict_default, read_tools};

pub const HEADER: &str = "All arguments are required; no other arguments are allowed.";
pub const SHARED: &str = "Shared arguments (same meaning wherever listed):";

fn name_ok(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn text_ok(s: &str) -> bool {
    !s.is_empty() && s.trim() == s && !s.contains(['\n', '\r'])
}

fn enum_value_ok(s: &str) -> bool {
    text_ok(s) && !s.contains(['|', ';'])
}

/// Renders one argument rule, or explains why signatures cannot express it exactly.
fn rule_text(rule: &Value) -> std::result::Result<String, &'static str> {
    let entries = rule.as_object().ok_or("its rule is not an object")?;
    let has = |k: &str| entries.iter().any(|(key, _)| key == k);
    let ty = rule.get("type").and_then(Value::as_str).ok_or("it has no simple \"type\"")?;
    let mut allowed = vec!["type", "description"];
    let mut out = if has("enum") {
        allowed.push("enum");
        let values = rule.get("enum").and_then(Value::as_array).ok_or("its enum is not an array")?;
        let words: Option<Vec<&str>> = values.iter().map(Value::as_str).collect();
        match words {
            Some(words) if ty == "string" && !words.is_empty() && words.iter().all(|w| enum_value_ok(w)) => {
                format!("one of {}", words.join("|"))
            }
            _ => return Err("its enum is not a list of plain strings"),
        }
    } else {
        let mut text = ty.to_owned();
        match ty {
            "integer" | "number" => {
                allowed.extend(["minimum", "maximum"]);
                let bound = |k: &str| match rule.get(k) {
                    None => Ok(String::new()),
                    Some(Value::Number(n)) => Ok(n.clone()),
                    Some(_) => Err("a bound is not a number"),
                };
                if has("minimum") || has("maximum") {
                    text.push_str(&format!(" {}..{}", bound("minimum")?, bound("maximum")?));
                }
            }
            "string" => {
                allowed.push("maxLength");
                if let Some(v) = rule.get("maxLength") {
                    let n = v.as_usize().ok_or("its maxLength is not a whole number")?;
                    text.push_str(&format!(", at most {n} characters"));
                }
            }
            "boolean" => {}
            _ => return Err("its type is not string, integer, number or boolean"),
        }
        text
    };
    if entries.iter().any(|(k, _)| !allowed.contains(&k.as_str())) {
        return Err("it uses JSON Schema keywords signatures cannot express");
    }
    if let Some(d) = rule.get("description") {
        let d = d.as_str().filter(|d| text_ok(d)).ok_or("its description is empty, multi-line or padded")?;
        out.push_str("; ");
        out.push_str(d);
    }
    Ok(out)
}

/// Renders tools as signatures. Every tool must be strict (`strict: true`, every property
/// required, no additional properties) and have a one-line description.
pub fn pack(tools: &[Value]) -> Result<String> {
    let (_, tools) = read_tools(tools)?;
    let mut rendered: Vec<Vec<(&str, String, String)>> = Vec::new(); // (arg, rule json, text)
    for tool in &tools {
        if !name_ok(tool.name) {
            return Err(Error::new(format!("tool \"{}\": names may use only letters, digits, _ and -", tool.name)));
        }
        if !is_strict_default(tool) || !tool.description.is_some_and(text_ok) {
            return Err(Error::new(format!(
                "tool \"{}\": signatures need strict tools (strict: true, every property required, no additional \
                 properties) with a one-line description; use pack \"schema\"",
                tool.name
            )));
        }
        let mut args = Vec::new();
        for (arg, rule) in tool.properties {
            if !name_ok(arg) {
                return Err(Error::new(format!(
                    "tool \"{}\": argument names may use only letters, digits, _ and -",
                    tool.name
                )));
            }
            let text = rule_text(rule).map_err(|why| {
                Error::new(format!("tool \"{}\", argument \"{arg}\": {why}; use pack \"schema\"", tool.name))
            })?;
            args.push((arg.as_str(), rule.to_json(), text));
        }
        rendered.push(args);
    }

    // Shared: same name and identical rule in two or more tools, and the name means nothing else.
    let mut shared: Vec<(&str, String, String)> = Vec::new();
    for (i, args) in rendered.iter().enumerate() {
        for (arg, rule, text) in args {
            if shared.iter().any(|(a, _, _)| a == arg) {
                continue;
            }
            let uses: Vec<&String> =
                rendered.iter().flat_map(|a| a.iter().filter(|(n, _, _)| n == arg).map(|(_, r, _)| r)).collect();
            let later = rendered[i + 1..].iter().any(|a| a.iter().any(|(n, _, _)| n == arg));
            if later && uses.len() >= 2 && uses.iter().all(|r| *r == rule) {
                shared.push((arg, rule.clone(), text.clone()));
            }
        }
    }

    let mut lines = vec![HEADER.to_owned()];
    if !shared.is_empty() {
        lines.push(SHARED.to_owned());
        lines.extend(shared.iter().map(|(a, _, t)| format!("  {a}: {t}")));
    }
    for (tool, args) in tools.iter().zip(&rendered) {
        let names: Vec<&str> = args.iter().map(|(a, _, _)| *a).collect();
        lines.push(format!("{}({}): {}", tool.name, names.join(", "), tool.description.unwrap_or_default()));
        for (arg, _, text) in args {
            if !shared.iter().any(|(a, _, _)| a == arg) {
                lines.push(format!("  {arg}: {text}"));
            }
        }
    }
    Ok(lines.join("\n"))
}

fn parse_rule(text: &str) -> Result<Value> {
    let bad = || Error::new(format!("unreadable argument rule \"{text}\""));
    let (kind, description) = match text.split_once("; ") {
        Some((k, d)) => (k, Some(d)),
        None => (text, None),
    };
    let mut rule: Vec<(String, Value)> = Vec::new();
    if let Some(values) = kind.strip_prefix("one of ") {
        let words: Vec<&str> = values.split('|').collect();
        if words.iter().any(|w| !enum_value_ok(w)) {
            return Err(bad());
        }
        rule.push(("type".into(), Value::str("string")));
        rule.push(("enum".into(), Value::strings(&words)));
    } else {
        let (head, max_length) = match kind.split_once(", at most ") {
            Some((h, rest)) => {
                let n = rest.strip_suffix(" characters").ok_or_else(bad)?;
                (h, Some(json::parse(n, 32).ok().filter(|v| v.as_usize().is_some()).ok_or_else(bad)?))
            }
            None => (kind, None),
        };
        let (ty, range) = match head.split_once(' ') {
            Some((t, r)) => (t, Some(r)),
            None => (head, None),
        };
        if !["string", "integer", "number", "boolean"].contains(&ty) {
            return Err(bad());
        }
        rule.push(("type".into(), Value::str(ty)));
        if let Some(range) = range {
            if ty != "integer" && ty != "number" {
                return Err(bad());
            }
            let (min, max) = range.split_once("..").ok_or_else(bad)?;
            if min.is_empty() && max.is_empty() {
                return Err(bad());
            }
            for (key, side) in [("minimum", min), ("maximum", max)] {
                if !side.is_empty() {
                    let n = json::parse(side, 64).ok().filter(|v| matches!(v, Value::Number(_))).ok_or_else(bad)?;
                    rule.push((key.into(), n));
                }
            }
        }
        if let Some(n) = max_length {
            if ty != "string" {
                return Err(bad());
            }
            rule.push(("maxLength".into(), n));
        }
    }
    if let Some(d) = description {
        if !text_ok(d) {
            return Err(bad());
        }
        rule.push(("description".into(), Value::str(d)));
    }
    Ok(Value::Object(rule))
}

fn parse_arg_line(line: &str) -> Result<(String, Value)> {
    let body = line.strip_prefix("  ").ok_or_else(|| Error::new(format!("expected an argument line: \"{line}\"")))?;
    let (name, rule) =
        body.split_once(": ").ok_or_else(|| Error::new(format!("expected \"name: rule\": \"{line}\"")))?;
    if !name_ok(name) {
        return Err(Error::new(format!("bad argument name in \"{line}\"")));
    }
    Ok((name.to_owned(), parse_rule(rule)?))
}

/// Restores tool definitions from signatures, as bare strict function objects
/// (`{"name","description","strict","parameters"}`), without any provider wrapper.
pub fn unpack(text: &str) -> Result<Vec<Value>> {
    let mut lines = text.split('\n').peekable();
    if lines.next() != Some(HEADER) {
        return Err(Error::new("signatures must start with the standard header line"));
    }
    let mut shared: Vec<(String, Value)> = Vec::new();
    if lines.peek() == Some(&SHARED) {
        lines.next();
        while let Some(line) = lines.next_if(|l| l.starts_with("  ")) {
            shared.push(parse_arg_line(line)?);
        }
    }
    let mut tools = Vec::new();
    while let Some(line) = lines.next() {
        let bad = || Error::new(format!("expected \"name(args): description\": \"{line}\""));
        let (name, rest) = line.split_once('(').ok_or_else(bad)?;
        let (args, description) = rest.split_once("): ").ok_or_else(bad)?;
        if !name_ok(name) || !text_ok(description) {
            return Err(bad());
        }
        let names: Vec<&str> = if args.is_empty() { Vec::new() } else { args.split(", ").collect() };
        let mut local: Vec<(String, Value)> = Vec::new();
        while let Some(arg_line) = lines.next_if(|l| l.starts_with("  ")) {
            local.push(parse_arg_line(arg_line)?);
        }
        let mut properties = Vec::with_capacity(names.len());
        for arg in &names {
            let rule = local
                .iter()
                .chain(shared.iter())
                .find(|(n, _)| n == arg)
                .map(|(_, r)| r.clone())
                .ok_or_else(|| Error::new(format!("tool \"{name}\": argument \"{arg}\" is not described")))?;
            properties.push((arg.to_string(), rule));
        }
        if let Some((extra, _)) = local.iter().find(|(n, _)| !names.contains(&n.as_str())) {
            return Err(Error::new(format!("tool \"{name}\": \"{extra}\" is described but not listed")));
        }
        tools.push(Value::object(vec![
            ("name", Value::str(name)),
            ("description", Value::str(description)),
            ("strict", Value::Bool(true)),
            (
                "parameters",
                Value::object(vec![
                    ("type", Value::str("object")),
                    ("properties", Value::Object(properties)),
                    ("required", Value::strings(&names)),
                    ("additionalProperties", Value::Bool(false)),
                ]),
            ),
        ]));
    }
    Ok(tools)
}

/// Removes the `{"type":"function","function":...}` wrapper, if present.
pub fn bare(tool: &Value) -> Value {
    tool.get("function").cloned().unwrap_or_else(|| tool.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    fn tool(name: &str, desc: &str, props: &str) -> String {
        let p = parse(props, 4096).unwrap();
        let keys: Vec<String> = p.as_object().unwrap().iter().map(|(k, _)| format!("\"{k}\"")).collect();
        format!(
            r#"{{"type":"function","function":{{"name":"{name}","description":"{desc}","strict":true,"parameters":{{"type":"object","properties":{props},"required":[{}],"additionalProperties":false}}}}}}"#,
            keys.join(",")
        )
    }

    fn tools(list: &[String]) -> Vec<Value> {
        parse(&format!("[{}]", list.join(",")), 1 << 16).unwrap().as_array().unwrap().to_vec()
    }

    #[test]
    fn renders_shares_and_round_trips() {
        let source = tools(&[
            tool(
                "cast",
                "Cast a spell.",
                r#"{"spell":{"type":"string","enum":["spark","hush"]},"target":{"type":"string","description":"Entity handle."},"power":{"type":"integer","minimum":1,"maximum":5}}"#,
            ),
            tool(
                "guard",
                "Guard someone.",
                r#"{"target":{"type":"string","description":"Entity handle."},"power":{"type":"integer","minimum":1,"maximum":5}}"#,
            ),
            tool(
                "say",
                "Speak.",
                r#"{"line":{"type":"string","maxLength":120},"loud":{"type":"boolean"},"bias":{"type":"number","minimum":-0.5}}"#,
            ),
        ]);
        let text = pack(&source).unwrap();
        assert_eq!(
            text,
            "All arguments are required; no other arguments are allowed.\n\
             Shared arguments (same meaning wherever listed):\n  \
             target: string; Entity handle.\n  \
             power: integer 1..5\n\
             cast(spell, target, power): Cast a spell.\n  \
             spell: one of spark|hush\n\
             guard(target, power): Guard someone.\n\
             say(line, loud, bias): Speak.\n  \
             line: string, at most 120 characters\n  \
             loud: boolean\n  \
             bias: number -0.5.."
        );
        let restored = Value::Array(unpack(&text).unwrap());
        assert!(restored.same(&Value::Array(source.iter().map(bare).collect())));
    }

    #[test]
    fn same_name_with_different_rules_is_not_shared() {
        let source = tools(&[
            tool("a", "A.", r#"{"x":{"type":"integer"}}"#),
            tool("b", "B.", r#"{"x":{"type":"string"}}"#),
            tool("c", "C.", r#"{"x":{"type":"string"}}"#),
        ]);
        let text = pack(&source).unwrap();
        assert!(!text.contains(SHARED));
        assert!(Value::Array(unpack(&text).unwrap()).same(&Value::Array(source.iter().map(bare).collect())));
    }

    #[test]
    fn refuses_what_it_cannot_express_exactly() {
        let refuse = |props: &str| pack(&tools(&[tool("a", "A.", props)])).is_err();
        assert!(refuse(r#"{"x":{"type":"object","properties":{}}}"#));
        assert!(refuse(r#"{"x":{"type":"array","items":{"type":"string"}}}"#));
        assert!(refuse(r#"{"x":{"enum":[1,2]}}"#));
        assert!(refuse(r#"{"x":{"type":"string","enum":["a|b"]}}"#));
        assert!(refuse(r#"{"x":{"type":"string","pattern":"^a"}}"#));
        assert!(refuse(r#"{"x":{"type":"string","description":"two\nlines"}}"#));
        let loose =
            r#"[{"name":"a","description":"A.","parameters":{"type":"object","properties":{"x":{"type":"string"}}}}]"#;
        assert!(pack(parse(loose, 1024).unwrap().as_array().unwrap()).is_err(), "non-strict tools are refused");
        assert!(unpack("not signatures").is_err());
        assert!(unpack(&format!("{HEADER}\na(x): A.")).is_err(), "undescribed argument");
    }
}
