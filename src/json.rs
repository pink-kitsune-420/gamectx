//! Strict, order-preserving JSON. Numbers keep their original text, so a value that is
//! parsed, packed, unpacked and written again is byte-for-byte the same number.

use std::collections::HashSet;
use std::fmt;

/// Nesting deeper than this is rejected instead of risking stack exhaustion.
pub const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// The exact number text from the source (already validated as JSON grammar).
    Number(String),
    String(String),
    Array(Vec<Value>),
    /// Insertion-ordered; keys are unique.
    Object(Vec<(String, Value)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Error(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::String(s.into())
    }

    pub fn int(n: i64) -> Value {
        Value::Number(n.to_string())
    }

    pub fn object(entries: Vec<(&str, Value)>) -> Value {
        Value::Object(entries.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    pub fn strings<S: AsRef<str>>(items: &[S]) -> Value {
        Value::Array(items.iter().map(|s| Value::str(s.as_ref())).collect())
    }

    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Non-negative integers only; fractions, exponents and huge values are refused.
    pub fn as_usize(&self) -> Option<usize> {
        match self {
            Value::Number(n) if n.bytes().all(|b| b.is_ascii_digit()) => n.parse().ok(),
            _ => None,
        }
    }

    /// Object member lookup.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object()?.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Equality that ignores object key order (JSON objects are unordered in meaning).
    pub fn same(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(k, v)| b.iter().find(|(k2, _)| k2 == k).is_some_and(|(_, v2)| v.same(v2)))
            }
            (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same(y)),
            _ => self == other,
        }
    }

    /// Compact serialization with no insignificant whitespace.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    /// Byte length of the compact serialization.
    pub fn json_len(&self) -> usize {
        self.to_json().len()
    }

    fn write(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(true) => out.push_str("true"),
            Value::Bool(false) => out.push_str("false"),
            Value::Number(n) => out.push_str(n),
            Value::String(s) => write_string(s, out),
            Value::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Value::Object(entries) => {
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(s)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

macro_rules! from_integer {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(n: $t) -> Value {
                Value::Number(n.to_string())
            }
        }
    )*};
}
from_integer!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl From<f64> for Value {
    /// Finite numbers use Rust's shortest round-trip form (`0.1`, `1.5`, `3`); NaN and
    /// infinities, which JSON cannot represent, become `null`.
    fn from(n: f64) -> Value {
        if n.is_finite() {
            Value::Number(format!("{n}"))
        } else {
            Value::Null
        }
    }
}

impl From<f32> for Value {
    fn from(n: f32) -> Value {
        if n.is_finite() {
            Value::Number(format!("{n}"))
        } else {
            Value::Null
        }
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Value {
        v.map_or(Value::Null, Into::into)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(items: Vec<T>) -> Value {
        Value::Array(items.into_iter().map(Into::into).collect())
    }
}

/// Builds a JSON object: `obj!{"id" => "salt_pouch", "count" => 12, "tags" => vec!["#wards"]}`.
/// Values may be anything that converts into [`Value`], including other `obj!` objects.
#[macro_export]
macro_rules! obj {
    ($($key:expr => $value:expr),* $(,)?) => {
        $crate::json::Value::Object(vec![$((::std::string::String::from($key), $crate::json::Value::from($value))),*])
    };
}

pub fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Strictly parses one JSON document, rejecting duplicate keys, deep nesting, trailing
/// characters and inputs longer than `max_bytes`.
pub fn parse(input: &str, max_bytes: usize) -> Result<Value> {
    if input.len() > max_bytes {
        return Err(Error::new(format!("JSON input is {} bytes; limit is {max_bytes}", input.len())));
    }
    let mut p = Parser { bytes: input.as_bytes(), pos: 0 };
    let value = p.value(0)?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(p.error("trailing characters after JSON document"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn error(&self, message: &str) -> Error {
        Error::new(format!("{message} at byte {}", self.pos))
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(self.error("JSON nesting limit exceeded"));
        }
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("invalid JSON token")),
            None => Err(self.error("unexpected end of JSON")),
        }
    }

    fn literal(&mut self, text: &str, value: Value) -> Result<Value> {
        if self.bytes[self.pos..].starts_with(text.as_bytes()) {
            self.pos += text.len();
            Ok(value)
        } else {
            Err(self.error("invalid JSON literal"))
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value> {
        self.pos += 1;
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected string key"));
            }
            let key = self.string()?;
            if !seen.insert(key.clone()) {
                return Err(self.error(&format!("duplicate key \"{key}\"")));
            }
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':' after key"));
            }
            self.pos += 1;
            let value = self.value(depth + 1)?;
            entries.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(entries));
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32> {
        let digits = self.bytes.get(self.pos..self.pos + 4).ok_or_else(|| self.error("incomplete \\u escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.error("invalid \\u escape"))?;
        let code = u32::from_str_radix(text, 16).map_err(|_| self.error("invalid \\u escape"))?;
        self.pos += 4;
        Ok(code)
    }

    fn string(&mut self) -> Result<String> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            // Copy the longest run that needs no decoding in one step.
            let start = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            // The source is a &str and runs stop only at ASCII bytes, so this slice is valid UTF-8.
            out.push_str(std::str::from_utf8(&self.bytes[start..self.pos]).expect("UTF-8 run"));
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let escape = self.peek().ok_or_else(|| self.error("unterminated escape"))?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut code = self.hex4()?;
                            if (0xD800..0xDC00).contains(&code) {
                                // High surrogate: JSON spells characters outside the BMP as a pair.
                                if !self.bytes[self.pos..].starts_with(b"\\u") {
                                    return Err(self.error("unpaired surrogate"));
                                }
                                self.pos += 2;
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) {
                                    return Err(self.error("invalid surrogate pair"));
                                }
                                code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            }
                            out.push(char::from_u32(code).ok_or_else(|| self.error("invalid unicode scalar"))?);
                        }
                        _ => return Err(self.error("invalid escape sequence")),
                    }
                }
                Some(_) => return Err(self.error("unescaped control character in string")),
                None => return Err(self.error("unterminated string")),
            }
        }
    }

    fn number(&mut self) -> Result<Value> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                if let Some(b'0'..=b'9') = self.peek() {
                    return Err(self.error("leading zeros are not allowed"));
                }
            }
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.error("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("expected digit after decimal point"));
            }
            self.digits();
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("expected digit in exponent"));
            }
            self.digits();
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).expect("ASCII number");
        Ok(Value::Number(text.to_owned()))
    }

    fn digits(&mut self) {
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_exact_numbers_and_order() {
        let text = r#"{"b":1.50,"a":[1e3,-0,12345678901234567890],"s":"x\"y"}"#;
        assert_eq!(parse(text, 1024).unwrap().to_json(), text);
    }

    #[test]
    fn rejects_duplicates_trailing_and_oversize() {
        assert!(parse(r#"{"a":1,"a":2}"#, 64).is_err());
        assert!(parse("[1] x", 64).is_err());
        assert!(parse("[1,2,3]", 3).is_err());
        assert!(parse("01", 64).is_err());
    }

    #[test]
    fn decodes_surrogate_pairs() {
        assert_eq!(parse(r#""😀""#, 64).unwrap(), Value::str("\u{1F600}"));
        assert!(parse(r#""\ud83d""#, 64).is_err());
    }

    #[test]
    fn conversions_and_obj_macro() {
        let v = crate::obj! {
            "id" => "salt_pouch", "count" => 12u32, "weight" => 0.1, "rare" => false,
            "tags" => vec!["#wards"], "owner" => None::<&str>, "bad" => f64::NAN,
            "nested" => crate::obj!{"x" => -3},
        };
        assert_eq!(
            v.to_json(),
            r##"{"id":"salt_pouch","count":12,"weight":0.1,"rare":false,"tags":["#wards"],"owner":null,"bad":null,"nested":{"x":-3}}"##
        );
        assert_eq!(parse(&v.to_json(), 1024).unwrap(), v, "conversions produce valid JSON numbers");
        assert_eq!(Value::from(1e21).to_json(), "1000000000000000000000");
    }

    #[test]
    fn same_ignores_object_order() {
        let a = parse(r#"{"x":1,"y":[{"p":1,"q":2}]}"#, 64).unwrap();
        let b = parse(r#"{"y":[{"q":2,"p":1}],"x":1}"#, 64).unwrap();
        assert!(a.same(&b));
        assert_ne!(a, b);
    }
}
