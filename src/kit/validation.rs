use crate::{checked as c, json, Error, Result, Value};

pub(super) fn bad() -> Error {
    Error::new("invalid or out-of-bounds kit value")
}
pub(super) fn bounded(v: &Value, cap: usize) -> Result<()> {
    if v.json_len() > cap {
        return Err(bad());
    }
    // Also checks manually constructed Values (duplicate keys and invalid number text).
    json::parse(&v.to_json(), cap).map(|_| ())
}
pub(super) fn prose(v: &Value, max: usize) -> Result<()> {
    let s = c::text(v)?;
    if s.is_empty() || s.chars().count() > max || s.chars().any(char::is_control) {
        return Err(bad());
    }
    Ok(())
}
pub(super) fn optional_fields(v: &Value, allowed: &[&str]) -> Result<()> {
    let fields = v.as_object().ok_or_else(bad)?;
    if fields.iter().any(|(k, _)| !allowed.contains(&k.as_str())) {
        return Err(bad());
    }
    Ok(())
}
pub(super) fn named<'a>(values: &'a [Value], name: &str) -> Result<&'a Value> {
    values.iter().find(|v| v.get("name").and_then(Value::as_str) == Some(name)).ok_or_else(bad)
}
pub(super) fn unique_names(values: &[Value], max: usize) -> Result<()> {
    if values.len() > max {
        return Err(bad());
    }
    for (i, v) in values.iter().enumerate() {
        let name = c::text(c::get(v, "name")?)?;
        c::name(name)?;
        if values[..i].iter().any(|v| v.get("name").and_then(Value::as_str) == Some(name)) {
            return Err(bad());
        }
    }
    Ok(())
}
pub(super) fn strings(v: &Value, max: usize) -> Result<Vec<&str>> {
    let a = c::list(v)?;
    if a.len() > max {
        return Err(bad());
    }
    let mut out = Vec::new();
    for v in a {
        let s = c::text(v)?;
        if out.contains(&s) {
            return Err(bad());
        }
        out.push(s);
    }
    Ok(out)
}
pub(super) fn priority(v: &Value) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| (-1_000_000..=1_000_000).contains(n)).ok_or_else(bad),
        _ => Err(bad()),
    }
}
fn finite(v: &Value) -> Result<f64> {
    match v {
        Value::Number(s) if s.len() <= 128 => s.parse::<f64>().ok().filter(|n| n.is_finite()).ok_or_else(bad),
        _ => Err(bad()),
    }
}
fn decimal(v: &Value) -> Result<(bool, String, i64)> {
    finite(v)?;
    let Value::Number(s) = v else { return Err(bad()) };
    let (mantissa, exponent) = s.split_once(['e', 'E']).unwrap_or((s, "0"));
    let exponent = exponent.parse::<i64>().ok().filter(|n| (-10_000..=10_000).contains(n)).ok_or_else(bad)?;
    let fraction = mantissa.split_once('.').map_or(0, |(_, f)| f.len() as i64);
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Ok((false, "0".into(), 0));
    }
    let trailing = digits.len() - digits.trim_end_matches('0').len();
    Ok((s.starts_with('-'), digits.trim_end_matches('0').to_owned(), exponent - fraction + trailing as i64))
}
fn numeric_cmp(a: &Value, b: &Value) -> Result<std::cmp::Ordering> {
    let (an, ad, ae) = decimal(a)?;
    let (bn, bd, be) = decimal(b)?;
    if an != bn {
        return Ok(if an { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater });
    }
    let order = if ad == "0" || bd == "0" {
        (ad != "0").cmp(&(bd != "0"))
    } else {
        (ad.len() as i64 + ae).cmp(&(bd.len() as i64 + be)).then_with(|| {
            let width = ad.len().max(bd.len());
            ad.bytes()
                .chain(std::iter::repeat(b'0'))
                .take(width)
                .cmp(bd.bytes().chain(std::iter::repeat(b'0')).take(width))
        })
    };
    Ok(if an { order.reverse() } else { order })
}
fn equivalent(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => numeric_cmp(a, b).is_ok_and(|o| o.is_eq()),
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b)),
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.iter().any(|(key, w)| k == key && equivalent(v, w)))
        }
        _ => a == b,
    }
}
// Mathematical integer test without rounding through f64. JSON spelling is checked by bounded().
fn integral(s: &str) -> bool {
    let (mantissa, exp) = s.split_once(['e', 'E']).unwrap_or((s, "0"));
    let exponent = match exp.parse::<i64>() {
        Ok(e) => e,
        Err(_) => return mantissa.chars().all(|c| c == '0' || c == '.' || c == '-'),
    };
    let fractional = mantissa.split_once('.').map_or(0, |(_, frac)| frac.len() as i64);
    let digits = mantissa.bytes().filter(u8::is_ascii_digit).collect::<Vec<_>>();
    if digits.iter().all(|c| *c == b'0') {
        return true;
    }
    let trailing = digits.iter().rev().take_while(|c| **c == b'0').count() as i64;
    fractional.saturating_sub(exponent) <= trailing
}
pub(super) fn scalar(v: &Value, kind: &str, nullable: bool) -> bool {
    if *v == Value::Null {
        return nullable || kind == "null";
    }
    match (kind, v) {
        ("string", Value::String(_))
        | ("boolean", Value::Bool(_))
        | ("object", Value::Object(_))
        | ("array", Value::Array(_)) => true,
        ("number", Value::Number(_)) => decimal(v).is_ok(),
        ("integer", Value::Number(s)) => decimal(v).is_ok() && integral(s),
        _ => false,
    }
}
fn kind(s: &Value) -> Result<(&str, bool)> {
    let t = c::get(s, "type")?;
    if let Some(t) = t.as_str() {
        return Ok((t, false));
    }
    let a = c::list(t)?;
    if a.len() != 2 || a[1].as_str() != Some("null") || a[0].as_str() == Some("null") {
        return Err(bad());
    }
    Ok((c::text(&a[0])?, true))
}
/// Validates the deliberately small, closed JSON Schema subset accepted by the toolkit.
pub(super) fn schema(s: &Value, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    if depth > 8 || *nodes > 256 {
        return Err(bad());
    }
    let (t, _) = kind(s)?;
    let mut allowed = vec!["type", "description", "enum"];
    if let Some(v) = s.get("description") {
        prose(v, 1024)?;
    }
    match t {
        "object" => {
            allowed.extend(["properties", "required", "additionalProperties"]);
            if c::get(s, "additionalProperties")? != &Value::Bool(false) {
                return Err(bad());
            }
            let properties = c::get(s, "properties")?.as_object().ok_or_else(bad)?;
            if properties.len() > 32 {
                return Err(bad());
            }
            for (n, p) in properties {
                c::name(n)?;
                schema(p, depth + 1, nodes)?;
            }
            for required in strings(c::get(s, "required")?, 32)? {
                if !properties.iter().any(|(n, _)| n == required) {
                    return Err(bad());
                }
            }
        }
        "array" => {
            allowed.extend(["items", "minItems", "maxItems"]);
            let max = c::uint(c::get(s, "maxItems")?, 0, 128)?;
            if let Some(min) = s.get("minItems") {
                c::uint(min, 0, max)?;
            }
            schema(c::get(s, "items")?, depth + 1, nodes)?;
        }
        "string" => {
            allowed.extend(["minLength", "maxLength"]);
            let max = c::uint(c::get(s, "maxLength")?, 0, 4096)?;
            if let Some(min) = s.get("minLength") {
                c::uint(min, 0, max)?;
            }
        }
        "number" | "integer" => {
            allowed.extend(["minimum", "maximum"]);
            for bound in ["minimum", "maximum"] {
                if let Some(v) = s.get(bound) {
                    decimal(v)?;
                }
            }
            if let (Some(low), Some(high)) = (s.get("minimum"), s.get("maximum")) {
                if numeric_cmp(low, high)?.is_gt() {
                    return Err(bad());
                }
            }
        }
        "boolean" | "null" => {}
        _ => return Err(bad()),
    }
    optional_fields(s, &allowed)?;
    if let Some(v) = s.get("enum") {
        let values = c::list(v)?;
        if values.is_empty() || values.len() > 32 {
            return Err(bad());
        }
        let mut without = s.as_object().ok_or_else(bad)?.to_vec();
        without.retain(|(k, _)| k != "enum");
        for (i, v) in values.iter().enumerate() {
            if values[..i].iter().any(|old| equivalent(old, v)) {
                return Err(bad());
            }
            validate(&Value::Object(without.clone()), v)?;
        }
    }
    Ok(())
}
pub(super) fn validate(s: &Value, v: &Value) -> Result<()> {
    let (t, nullable) = kind(s)?;
    if !scalar(v, t, nullable) {
        return Err(bad());
    }
    if let Some(e) = s.get("enum") {
        if !c::list(e)?.iter().any(|x| equivalent(x, v)) {
            return Err(bad());
        }
    }
    if *v == Value::Null {
        return Ok(());
    }
    match t {
        "object" => {
            let properties = c::get(s, "properties")?;
            for required in strings(c::get(s, "required")?, 32)? {
                if v.get(required).is_none() {
                    return Err(Error::new(format!("missing required argument: {required}")));
                }
            }
            for (n, value) in v.as_object().ok_or_else(bad)? {
                let spec = properties.get(n).ok_or_else(|| Error::new(format!("unknown argument: {n}")))?;
                validate(spec, value).map_err(|e| Error::new(format!("argument {n}: {e}")))?;
            }
        }
        "array" => {
            let values = c::list(v)?;
            let min = s.get("minItems").and_then(Value::as_usize).unwrap_or(0);
            let max = c::get(s, "maxItems")?.as_usize().ok_or_else(bad)?;
            if !(min..=max).contains(&values.len()) {
                return Err(bad());
            }
            for v in values {
                validate(c::get(s, "items")?, v)?;
            }
        }
        "string" => {
            let len = c::text(v)?.chars().count();
            let min = s.get("minLength").and_then(Value::as_usize).unwrap_or(0);
            let max = c::get(s, "maxLength")?.as_usize().ok_or_else(bad)?;
            if !(min..=max).contains(&len) {
                return Err(bad());
            }
        }
        "number" | "integer" => {
            if let Some(low) = s.get("minimum") {
                if numeric_cmp(v, low)?.is_lt() {
                    return Err(Error::new("number below minimum"));
                }
            }
            if let Some(high) = s.get("maximum") {
                if numeric_cmp(v, high)?.is_gt() {
                    return Err(Error::new("number above maximum"));
                }
            }
        }
        _ => {}
    }
    Ok(())
}
