//! Pass 5 tier 1 deep tests, offline and deterministic (no model, $0). The expectations were
//! declared in `docs/research/INTELLIGENT_BEING_5.md` before these runs:
//!   1. save anywhere: the pass 4 hundred-agent town (culture, projects, stability) is saved to
//!      text and loaded into fresh managers after every op on one day and after every request on
//!      every tenth day; the continued run must produce the uninterrupted report byte for byte;
//!   2. damaged saves: truncated, oversized, out-of-range and invariant-breaking saves of the
//!      town's final state are refused, by each manager and by a bundle, and a refused import
//!      changes nothing;
//!   3. save size: the bytes each manager's save takes per being.
//! The report holds only deterministic output and is checked byte for byte; wall time is measured
//! separately (`--timing`) with the hardware, and is not a promise.
//!
//!   rustc --edition=2021 -O eval/character/pass5_deep_test.rs -L dependency=target/release/deps \
//!     --extern gamectx=target/release/libgamectx.rlib -o target/pass5-deep
//!   target/pass5-deep --output eval/character/pass5-deep.json   # write the report
//!   target/pass5-deep --check eval/character/pass5-deep.json    # replay: byte-identical
//!   target/pass5-deep --timing eval/character/pass5-deep-timing.json
#[path = "pass4_deep_test.rs"]
#[allow(dead_code)]
mod pass4;

use gamectx::character::projects::Projects;
use gamectx::character::stability::StabilityMonitor;
use gamectx::data::json::{parse, Value};
use gamectx::data::persist::{self, Durable};
use gamectx::kit::state_bundle::StateBundle;
use gamectx::knowledge::culture::Culture;
use gamectx::obj;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

const BEINGS: usize = 100;

fn checker(name: &'static str) -> impl FnMut(&mut Vec<String>, &str, bool) {
    move |checks, what, ok| {
        assert!(ok, "{name}: check failed: {what}");
        checks.push(what.to_string());
    }
}

/// Replaces the first occurrence of `from` in `text`; the fixture must contain it.
fn damage(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "damage target missing: {from}");
    text.replacen(from, to, 1)
}

fn import_req(save: &str) -> Value {
    parse(&format!(r#"{{"ops":[{{"op":"import","args":{{"save":{save}}}}}]}}"#), 1 << 28).expect("request")
}

/// Loads `good` into a fresh manager, then offers each damaged save: the text must not parse, or
/// the import must be refused and the manager's save must stay byte-identical.
fn refusals<T: Durable>(good: &str, cases: &[(&str, String)], checks: &mut Vec<String>) -> Vec<Value> {
    let mut check = checker("damaged saves");
    let mut m = T::default();
    let limits = persist::saved_limits(T::SPEC, parse(good, 1 << 27).unwrap().get("limits").unwrap()).unwrap();
    *m.limits_mut() = limits;
    m.run(&import_req(good)).unwrap_or_else(|e| panic!("{} good save refused: {e:?}", T::KIND));
    let before = persist::export(&m, m.limits()).to_json();
    check(checks, &format!("{}: the town's final save reloads byte for byte", T::KIND), before == good);
    let mut rows = Vec::new();
    for (label, text) in cases {
        let refused_at = match parse(text, 1 << 28) {
            Err(_) => "parse",
            Ok(_) => {
                let ok = m.run(&import_req(text)).is_ok();
                check(checks, &format!("{}: refused: {label}", T::KIND), !ok);
                "import"
            }
        };
        let after = persist::export(&m, m.limits()).to_json();
        check(checks, &format!("{}: unchanged after: {label}", T::KIND), after == before);
        rows.push(obj! { "case" => *label, "refused_at" => refused_at });
    }
    rows
}

/// Damage every save shares; `named` saves (with a clock and `t0..` being ids) get the range and
/// size cases too.
fn generic(text: &str, named: bool) -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("truncated to half its text", text[..text.len() / 2].to_string()),
        ("missing its last byte", text[..text.len() - 1].to_string()),
        ("from a newer version", damage(text, r#""version":1}"#, r#""version":2}"#)),
        ("envelope keys out of order", damage(text, r#"{"format":"gamectx-state","kind":"#, r#"{"kind":"#).replacen(r#","limits""#, r#","format":"gamectx-state","limits""#, 1)),
    ];
    if named {
        out.extend([
            ("a clock beyond the integer range", damage(text, r#""clock":4320"#, r#""clock":99999999999999999999"#)),
            ("a negative clock", damage(text, r#""clock":4320"#, r#""clock":-1"#)),
            ("a string longer than 4,096 bytes", damage(text, r#""t0"#, &format!(r#""t{}"#, "0".repeat(4097)))),
            ("a save over 64 MiB", damage(text, r#""t0"#, &format!(r#""t{}"#, "0".repeat(64 << 20)))),
        ]);
    }
    out
}

fn damaged_saves(checks: &mut Vec<String>) -> (Value, Value) {
    let last = pass4::LAST.lock().unwrap().clone();
    let (cul, proj, mon) = (&last["culture"], &last["projects"], &last["stability"]);
    let mut culture = generic(cul, true);
    culture.extend([
        ("a rumor's hops disagree with its chain", damage(cul, r#""hops":3"#, r#""hops":2"#)),
        ("a link to a being who is not a member", damage(cul, r#""links":["t044""#, r#""links":["t999""#)),
        ("an idiom strength above one", damage(cul, r#""strength":1000000"#, r#""strength":1000001"#)),
        ("a repertoire idiom that does not exist", damage(cul, r#"["mill_home",{"at""#, r#"["mill_gone",{"at""#)),
        ("a belief about a topic with no truth", damage(cul, r#"["r0",{"agent""#, r#"["r9",{"agent""#)),
    ]);
    let mut projects = generic(proj, true);
    projects.extend([
        ("a milestone index past the plan", damage(proj, r#""at":2"#, r#""at":7"#)),
        ("an unknown status", damage(proj, r#""status":"active""#, r#""status":"resting""#)),
        ("a negative morale", damage(proj, r#""morale":300000"#, r#""morale":-1"#)),
        ("a culture save offered as projects", cul.clone()),
    ]);
    let mut stability = generic(mon, false);
    stability.extend([
        ("a warning id the counter would reuse", damage(mon, r#""next_warning":83"#, r#""next_warning":2"#)),
        ("a warning kind it does not know", damage(mon, r#""kind":"rumor_runaway""#, r#""kind":"rumor_storm""#)),
        ("a string longer than 4,096 bytes", damage(mon, r#""detail":"rumor"#, &format!(r#""detail":"{}"#, "r".repeat(4097)))),
    ]);
    let rows = obj! {
        "culture" => Value::Array(refusals::<Culture>(cul, &culture, checks)),
        "projects" => Value::Array(refusals::<Projects>(proj, &projects, checks)),
        "stability" => Value::Array(refusals::<StabilityMonitor>(mon, &stability, checks)),
    };

    // A bundle of all three: one damaged section refuses the whole import.
    let mut check = checker("bundle");
    let bundle = |c: &str, p: &str, s: &str| format!(r#"{{"format":"gamectx-bundle","sections":[["culture",{c}],["projects",{p}],["stability",{s}]],"version":1}}"#);
    let good = bundle(cul, proj, mon);
    let mut b = StateBundle::new();
    b.workflow(&parse(&format!(r#"{{"ops":[{{"op":"import","args":{{"save":{good}}}}}]}}"#), 1 << 27).unwrap()).expect("good bundle");
    let before = b.export().to_json();
    let bad = bundle(cul, proj, &stability.iter().find(|(l, _)| l.contains("reuse")).unwrap().1);
    let refused = b.workflow(&parse(&format!(r#"{{"ops":[{{"op":"import","args":{{"save":{bad}}}}}]}}"#), 1 << 27).unwrap()).is_err();
    check(checks, "bundle: refused when only its last section is damaged", refused);
    check(checks, "bundle: unchanged after the refused import", b.export().to_json() == before);
    let sizes = obj! {
        "beings" => BEINGS,
        "bundle_bytes" => good.len(),
        "bundle_bytes_per_being" => good.len() / BEINGS,
        "culture_bytes" => cul.len(),
        "culture_bytes_per_being" => cul.len() / BEINGS,
        "projects_bytes" => proj.len(),
        "projects_bytes_per_being" => proj.len() / BEINGS,
        "stability_bytes" => mon.len(),
    };
    (rows, sizes)
}

fn save_anywhere() -> (Value, Value) {
    let mut checks = Vec::new();
    let mut check = checker("save anywhere");
    pass4::SAVE_ANYWHERE.store(false, Relaxed);
    let t = Instant::now();
    let (plain, _) = pass4::town();
    let plain_ms = t.elapsed().as_millis() as u64;
    pass4::SAVE_ANYWHERE.store(true, Relaxed);
    pass4::SAVES.store(0, Relaxed);
    let t = Instant::now();
    let (saved, _) = pass4::town();
    let saved_ms = t.elapsed().as_millis() as u64;
    pass4::SAVE_ANYWHERE.store(false, Relaxed);
    let reloads = pass4::SAVES.load(Relaxed);
    check(&mut checks, "the town saved and reloaded after every op on one day and every request on every tenth day", reloads > 1000);
    check(&mut checks, "the reloaded town's report equals the uninterrupted run byte for byte", plain.to_json() == saved.to_json());
    let (refusals, sizes) = damaged_saves(&mut checks);
    let report = obj! {
        "checks" => checks,
        "every_op_day" => pass4::EVERY_OP_DAY,
        "every_tenth_day" => true,
        "name" => "save_anywhere_hundred_agents",
        "refusals" => refusals,
        "reloads" => reloads,
        "save_sizes" => sizes,
        "town_checks" => plain.get("checks").cloned().unwrap_or(Value::Null),
    };
    let timing = obj! {
        "plain_wall_ms" => plain_ms,
        "reloads" => reloads,
        "save_anywhere_wall_ms" => saved_ms,
        "us_per_reload" => (saved_ms.saturating_sub(plain_ms)) * 1000 / reloads.max(1),
    };
    (report, timing)
}

fn report() -> (String, Value) {
    let (save, timing) = save_anywhere();
    let scenes = vec![save];
    let checks: usize = scenes.iter().map(|x| x.get("checks").and_then(Value::as_array).map_or(0, |a| a.len())).sum();
    let out = obj! {
        "actual_model_calls" => 0, "api_spend_usd" => 0, "checks" => checks,
        "scenarios" => Value::Array(scenes), "version" => 1,
    }
    .to_json()
        + "\n";
    (out, obj! { "save_anywhere" => timing })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (a, timing) = report();
    match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("--output"), Some(p)) => std::fs::write(p, &a).expect("write report"),
        (Some("--check"), Some(p)) => {
            let b = std::fs::read_to_string(p).expect("read report");
            assert!(a == b, "{p} differs from a fresh run");
        }
        (Some("--timing"), Some(p)) => {
            let t = obj! {
                "hardware" => obj! { "cpu" => pass4::cpu(), "architecture" => std::env::consts::ARCH },
                "peak_heap_bytes" => pass4::PEAK.load(Relaxed),
                "timing" => timing.clone(),
                "version" => 1,
            };
            std::fs::write(p, t.to_json() + "\n").expect("write timing");
        }
        _ => {}
    }
    let v = parse(&a, 1 << 24).unwrap();
    println!("{{\"pass5_deep\":true,\"checks\":{},\"model_calls\":0,\"api_spend_usd\":0}}", v.get("checks").unwrap().to_json());
}
