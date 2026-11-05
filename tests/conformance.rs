//! Reference runner for the shared conformance cases in `conformance/`. Ports in other
//! languages run the same cases with their own runner; see conformance/README.md.
//!
//! Set `GAMECTX_BLESS=1` to rewrite every `expected.json` from the current behavior.

use gamectx::budget::{assemble_bytes, Section};
use gamectx::catalog::{Catalog, Limits};
use gamectx::estimate::{self, Calibration};
use gamectx::json::{parse, Error, Result, Value};
use gamectx::view::{self, View};
use gamectx::{fingerprint, schema, signatures, table};
use std::fs;
use std::path::{Path, PathBuf};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/conformance");

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn arg_usize(args: &Value, key: &str, default: usize) -> usize {
    args.get(key).map_or(default, |v| v.as_usize().unwrap_or_else(|| panic!("args.{key} must be an integer")))
}

fn strings(value: &Value) -> Vec<String> {
    value.as_array().expect("string array").iter().map(|s| s.as_str().expect("string").to_owned()).collect()
}

fn group_by(args: &Value) -> Result<Vec<String>> {
    args.get("group_by").map(table::group_columns).transpose().map(Option::unwrap_or_default)
}

/// Runs one operation. `Err` is an expected rejection; malformed case files panic.
fn run(op: &str, args: &Value, dir: &Path) -> Result<String> {
    if op == "parse" {
        return parse(&read(&dir.join("input.txt")), 1 << 20).map(|v| v.to_json());
    }
    let input = parse(&read(&dir.join("input.json")), 1 << 20).expect("input.json must be valid JSON");
    let list = || input.as_array().expect("input must be an array");
    let out = match op {
        "table" => table::pack(
            &view::apply(list(), &View::from_json(args)?)?,
            &group_by(args)?.iter().map(String::as_str).collect::<Vec<_>>(),
        )?,
        "schema" => schema::pack(list())?,
        "signatures" => Value::str(signatures::pack(list())?),
        "outline" => Value::str(schema::outline(list())?),
        "catalog" => {
            let d = Limits::default();
            let limits = match args.get("limits") {
                None => d,
                Some(l) => Limits {
                    entries: arg_usize(l, "entries", d.entries),
                    tags: arg_usize(l, "tags", d.tags),
                    results: arg_usize(l, "results", d.results),
                    query_chars: arg_usize(l, "query_chars", d.query_chars),
                    query_words: arg_usize(l, "query_words", d.query_words),
                    offset: arg_usize(l, "offset", d.offset),
                },
            };
            let catalog = Catalog::from_json(&input, limits)?;
            let kind = args.get("kind").and_then(Value::as_str);
            let count = arg_usize(args, "count", limits.results);
            let expected = args.get("expected_generation").and_then(Value::as_str);
            let page = match args.get("relevant_to").and_then(Value::as_str) {
                Some(text) => catalog.relevant(text, kind, count, expected)?,
                None => catalog.search(
                    args.get("query").and_then(Value::as_str).unwrap_or(""),
                    kind,
                    arg_usize(args, "offset", 0),
                    count,
                    expected,
                )?,
            };
            page.to_value()
        }
        "budget" => {
            let sections = list()
                .iter()
                .map(|s| Section {
                    name: s.get("name").and_then(Value::as_str).expect("name").to_owned(),
                    priority: s.get("priority").and_then(|p| p.to_json().parse().ok()).unwrap_or(0),
                    stable: s.get("stable").and_then(Value::as_bool).unwrap_or(false),
                    required: s.get("required").and_then(Value::as_bool).unwrap_or(false),
                    value: s.get("value").expect("value").clone(),
                })
                .collect();
            let packet = assemble_bytes(sections, arg_usize(args, "max_bytes", usize::MAX))?;
            Value::object(vec![("packet", packet.value), ("receipt", packet.receipt.to_value())])
        }
        "estimate" => {
            let parts = strings(input.get("scope").expect("scope"));
            let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
            let scope = estimate::scope(&parts);
            let mut calibration = Calibration::NONE;
            for obs in input.get("observations").and_then(Value::as_array).expect("observations") {
                let n: Vec<usize> =
                    obs.as_array().expect("observation").iter().map(|v| v.as_usize().unwrap()).collect();
                calibration = calibration.observe(scope, n[0], n[1] as u64, n[2]);
            }
            let body = input.get("body").and_then(Value::as_str).expect("body");
            let framing = input.get("framing").and_then(Value::as_usize).unwrap_or(0);
            Value::object(vec![
                ("calibration", calibration.to_value()),
                ("estimate", Value::int(calibration.estimate(scope, body.as_bytes(), framing) as i64)),
            ])
        }
        "fingerprint" => Value::Array(strings(&input).iter().map(|s| Value::str(fingerprint(s))).collect()),
        other => panic!("unknown op {other}"),
    };
    // Lossless operations must also restore their input (grouped tables: in group order).
    let input = match op {
        "table" => {
            let viewed = view::apply(list(), &View::from_json(args)?)?;
            let columns = group_by(args)?;
            Value::Array(table::group_order(&viewed, &columns.iter().map(String::as_str).collect::<Vec<_>>()))
        }
        "signatures" => Value::Array(list().iter().map(signatures::bare).collect()),
        _ => input.clone(),
    };
    let restored = match op {
        "table" => Some(Value::Array(table::unpack(&out)?)),
        "schema" => Some(Value::Array(schema::unpack(&out)?)),
        "signatures" => Some(Value::Array(signatures::unpack(out.as_str().unwrap_or_default())?)),
        _ => None,
    };
    if let Some(r) = restored {
        if !r.same(&input) {
            return Err(Error::new("did not round-trip"));
        }
    }
    Ok(out.to_json())
}

#[test]
fn conformance_cases() {
    let bless = std::env::var("GAMECTX_BLESS").is_ok_and(|v| v == "1");
    let mut dirs: Vec<PathBuf> = fs::read_dir(ROOT)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .flat_map(|op| fs::read_dir(op).unwrap().map(|e| e.unwrap().path()).collect::<Vec<_>>())
        .filter(|p| p.join("case.json").exists())
        .collect();
    dirs.sort();
    assert!(dirs.len() >= 20, "expected the full case set, found {}", dirs.len());

    let mut failures = Vec::new();
    for dir in &dirs {
        let name = dir.strip_prefix(ROOT).unwrap().display().to_string();
        let case = parse(&read(&dir.join("case.json")), 1 << 16).unwrap();
        let op = case.get("op").and_then(Value::as_str).expect("op");
        let args = case.get("args").cloned().unwrap_or(Value::Object(Vec::new()));
        let wants_error = case.get("error").and_then(Value::as_bool).unwrap_or(false);
        let expected_path = dir.join("expected.json");
        match (run(op, &args, dir), wants_error) {
            (Ok(text), false) if bless => fs::write(&expected_path, text + "\n").unwrap(),
            (Ok(text), false) => {
                let expected = read(&expected_path);
                if expected.strip_suffix('\n').unwrap_or(&expected) != text {
                    failures.push(format!("{name}: output differs\n  expected {expected}  actual   {text}"));
                }
            }
            (Err(_), true) => assert!(!expected_path.exists(), "{name}: error cases have no expected.json"),
            (Ok(text), true) => failures.push(format!("{name}: expected an error, got {text}")),
            (Err(e), false) => failures.push(format!("{name}: unexpected error: {e}")),
        }
    }
    assert!(failures.is_empty(), "{} conformance failure(s):\n{}", failures.len(), failures.join("\n"));
}
