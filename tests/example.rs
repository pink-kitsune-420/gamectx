//! Runs the Ember Keep example through the library and the command-line tool.

use gamectx::budget::assemble_bytes;
use gamectx::manifest::{self, Check};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

const MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/ember-keep/manifest.json");

#[test]
fn example_round_trips_and_fits_its_budget() {
    let m = manifest::load(Path::new(MANIFEST)).unwrap();
    for s in &m.sections {
        let check = s.verify().unwrap();
        assert_eq!(check != Check::Lossy, s.pack.lossless(), "section {}", s.section.name);
    }
    let budget = m.budget_bytes.unwrap();
    let packet = assemble_bytes(m.sections.iter().map(|p| p.section.clone()).collect(), budget).unwrap();
    assert!(packet.text.len() <= budget);
    let dropped: Vec<&str> = packet.receipt.dropped.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(dropped, ["action_outline", "lights_nearby"]);
    assert!(packet.text.starts_with(r#"{"actions":"#), "stable sections lead the packet");
}

fn gamectx(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_gamectx")).args(args).output().unwrap();
    (out.status.success(), String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

#[test]
fn cli_pack_report_verify_and_errors() {
    let (ok, packet, receipt) = gamectx(&["pack", MANIFEST, "--receipt"]);
    assert!(ok);
    assert!(gamectx::json::parse(packet.trim(), 1 << 20).is_ok());
    assert!(receipt.contains("\"dropped\""));

    let (ok, report, _) = gamectx(&["report", MANIFEST]);
    assert!(ok && report.contains("dropped action_outline"));

    let (ok, verify, _) = gamectx(&["verify", MANIFEST]);
    assert!(ok && verify.contains("round-trips exactly"));

    let (ok, _, err) = gamectx(&["pack", MANIFEST, "--budget", "10"]);
    assert!(!ok && err.contains("required sections alone exceed the budget"));

    let (ok, _, err) = gamectx(&["pack", MANIFEST, "--bogus", "1"]);
    assert!(!ok && err.contains("unknown option"));
}

#[test]
fn cli_search_pages_with_generation() {
    let catalog = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/ember-keep/catalog.json");
    let (ok, first, _) = gamectx(&["search", catalog, "fire", "--count", "2"]);
    assert!(ok);
    let page = gamectx::json::parse(first.trim(), 1 << 16).unwrap();
    let generation = page.get("generation").unwrap().as_str().unwrap().to_owned();
    let next = page.get("next").unwrap().as_usize().unwrap().to_string();
    let (ok, _, _) =
        gamectx(&["search", catalog, "fire", "--count", "2", "--offset", &next, "--generation", &generation]);
    assert!(ok);
    let (ok, _, err) = gamectx(&["search", catalog, "fire", "--generation", "0000000000000000"]);
    assert!(!ok && err.contains("generation changed"));
}

#[test]
fn cli_query_executes_and_reports_errors() {
    let tables = concat!(env!("CARGO_MANIFEST_DIR"), "/conformance/query/select-filter-sort/input.json");
    let (ok, out, _) =
        gamectx(&["query", tables, "SELECT id FROM items WHERE count >= 6 ORDER BY weight DESC LIMIT 3"]);
    assert!(ok);
    assert_eq!(out, "[\"ember_cake\",\"salt_pouch\",\"bone_hook\"]\n");
    let (ok, tool, _) = gamectx(&["query", "--tool"]);
    assert!(ok);
    assert_eq!(tool, format!("{}\n", gamectx::query::tool().to_json()));
    let (ok, _, err) = gamectx(&["query", tables, "DELETE FROM items"]);
    assert!(!ok && err.contains("SELECT"));
    let (ok, _, err) = gamectx(&["query", "--tool", tables]);
    assert!(!ok && err.contains("expected 0 argument"));
}

fn query_input(args: &[&str], text: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gamectx"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn cli_schema_and_session_protocol() {
    let tables = concat!(env!("CARGO_MANIFEST_DIR"), "/conformance/query-schema/basic/input.json");
    let input = std::fs::read_to_string(tables).unwrap();
    let output = query_input(&["query", "--schema", "-"], &input);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/conformance/query-schema/basic/expected.json"))
            .unwrap()
    );
    let output = query_input(
        &["query", "--session", tables],
        "{\"sql\":\"SELECT id FROM food\"}\n{\"sql\":\"SELECT COUNT(*)\\nFROM t\"}\n",
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let replies: Vec<_> = text.lines().map(|line| gamectx::json::parse(line, 8192).unwrap()).collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0].get("retry").and_then(gamectx::Value::as_bool), Some(true));
    assert_eq!(replies[1].to_json(), r#"{"result":2}"#);
    let output = query_input(&["query", "--session", tables], "not json\n{\"sql\":3}\n");
    assert!(output.status.success(), "query failures are protocol replies");
    assert!(String::from_utf8(output.stdout).unwrap().ends_with("\"retry\":false}\n"));
    let output = query_input(&["query", "--session", tables], "{\"sql\":\"bad\"}\n");
    assert!(!output.status.success(), "EOF while a correction is pending is incomplete");
    let output = query_input(&["query", "--session", tables], &("x".repeat(8193) + "\n"));
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("8192 bytes"));
}

#[test]
fn cli_brain_workflows_and_tools() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/conformance/brain/teach-accept-undo/input.json");
    let expected = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/conformance/brain/teach-accept-undo/expected.json"
    ))
    .unwrap();
    let (ok, result, _) = gamectx(&["brain", fixture]);
    assert!(ok);
    assert_eq!(result, expected);
    let input = std::fs::read_to_string(fixture).unwrap();
    let output = query_input(&["brain", "-"], &input);
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    let (ok, tools, _) = gamectx(&["brain", "--tools"]);
    assert!(ok);
    assert_eq!(tools, format!("{}\n", gamectx::brain::tools().to_json()));
    let output = query_input(&["brain", "-"], r#"{"definition":{},"operations":[]}"#);
    assert!(!output.status.success());
    let output = query_input(&["brain", "-"], &" ".repeat(gamectx::brain::MAX_STATE_BYTES + 1));
    assert!(!output.status.success());
    let workflow = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/brain/workflow.json");
    let (ok, result, _) = gamectx(&["brain", workflow]);
    assert!(ok);
    let value = gamectx::json::parse(result.trim(), gamectx::brain::MAX_STATE_BYTES).unwrap();
    let last = value.get("results").unwrap().as_array().unwrap().last().unwrap();
    assert_eq!(last.get("choice").and_then(gamectx::Value::as_str), Some("withdraw"));
}

#[test]
fn manifest_accepts_every_documented_table_key() {
    let text = r#"{"sections":[{"name":"t","pack":"table","priority":1,"stable":false,"required":false,
        "value":[{"id":"a","kind":"tool","count":3,"w":1.25},{"id":"b","kind":"food","count":9,"w":2.5}],
        "group_by":["kind"],"where":{"count":{">":1}},"relevant_to":"any tools?","sort_by":"-count",
        "limit":5,"fields":["id","kind","w"],"round":{"w":1}}]}"#;
    let m = manifest::parse(&gamectx::json::parse(text, 4096).unwrap(), Path::new(".")).unwrap();
    assert_eq!(
        m.sections[0].section.value.to_json(),
        r#"{"group_by":"kind","columns":["id","w"],"groups":{"tool":[["a",1.2]]}}"#
    );
    assert_eq!(m.sections[0].verify().unwrap(), Check::ExactAfterView);
}
