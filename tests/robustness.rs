//! Deterministic mutation testing: malformed input must produce errors, never panics,
//! and anything that parses must survive packing and serialize back to itself.

use gamectx::json::{parse, Value};
use gamectx::{catalog, schema, signatures, table};

const SEEDS: [&str; 6] = [
    r##"[{"id":"a","kind":"x","n":1.50},{"id":"b","kind":"x","n":-2e3}]"##,
    r##"{"entries":[{"kind":"item","id":"lamp","tags":["#lights"]}],"complete":true}"##,
    r##"[{"name":"wait","description":"d","strict":true,"parameters":{"type":"object","properties":{"s":{"type":"integer","minimum":1,"maximum":5}},"required":["s"],"additionalProperties":false}}]"##,
    r##"{"s":"😀 é\n","a":[[[]]],"t":true,"f":false,"z":null}"##,
    "[\"caf\u{e9}\",\"\u{1F600}\"]",
    r#"{"sql":"SELECT COUNT(*) FROM items","extra":null}"#,
];
const NOISE: &[u8] = b"{}[]\":,\\-+.eE0123456789tfnul \t\nabc#\x01\xc3\xa9";

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn mutate(rng: &mut Rng, seed: &str) -> String {
    let mut bytes = seed.as_bytes().to_vec();
    for _ in 0..=rng.below(4) {
        let at = rng.below(bytes.len() + 1);
        match rng.below(3) {
            0 if at < bytes.len() => {
                bytes.remove(at);
            }
            1 => bytes.insert(at, NOISE[rng.below(NOISE.len())]),
            _ if at < bytes.len() => bytes[at] = NOISE[rng.below(NOISE.len())],
            _ => {}
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn exercise(value: &Value) {
    assert_eq!(parse(&value.to_json(), usize::MAX).as_ref(), Ok(value), "serialization must round-trip");
    let mut session = gamectx::query::Session::new();
    session.apply_call(value, &[("items", &[])]).unwrap();
    if !session.is_finished() {
        session.apply_call(value, &[("items", &[])]).unwrap();
    }
    assert!(session.apply_call(value, &[("items", &[])]).is_err());
    if let Some(items) = value.as_array() {
        let _ = gamectx::query::describe(&[("items", items)]);
        if let Ok(packed) = table::pack(items, &[]) {
            assert!(Value::Array(table::unpack(&packed).unwrap()).same(value));
        }
        if let Ok(packed) = table::pack(items, &["kind"]) {
            let restored = Value::Array(table::unpack(&packed).unwrap());
            assert!(restored.same(&Value::Array(table::group_order(items, &["kind"]))));
        }
        if let Ok(packed) = schema::pack(items) {
            assert!(Value::Array(schema::unpack(&packed).unwrap()).same(value));
            let _ = schema::outline(items);
        }
        if let Ok(text) = signatures::pack(items) {
            let restored = Value::Array(signatures::unpack(&text).unwrap());
            assert!(restored.same(&Value::Array(items.iter().map(signatures::bare).collect())));
        }
    }
    if let Ok(c) = catalog::Catalog::from_json(value, catalog::Limits::default()) {
        let _ = c.search("a #b cs", None, 0, 8, None);
    }
}

#[test]
fn mutated_inputs_never_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut parsed = 0;
    for i in 0..40_000 {
        let text = mutate(&mut rng, SEEDS[i % SEEDS.len()]);
        if let Ok(value) = parse(&text, 1 << 16) {
            parsed += 1;
            exercise(&value);
        }
    }
    assert!(parsed > 1000, "mutations should still produce many valid documents ({parsed})");
}

#[test]
fn formula_text_never_panics_or_hangs() {
    use gamectx::formula::Formulas;
    let mut f = Formulas::new();
    f.define("d", &["a", "b"], "max(0, 10 - b) * (1 - a / 100)", "").unwrap();
    f.define("e", &["x"], "d(x, x) + d(x, 1)", "").unwrap();
    let seeds = [
        "d(50, 4) + e(3) * 2",
        "if(1 < 2 and not false, round(2.5, 1), -1)",
        "min(1, 2, 3) % 2 == 1",
        "clamp(sqrt(16), 0, 3) / 0.5",
    ];
    const NOISE: &[u8] = b"()+-*/%<>=!,.0123456789eE adeifnotrux_";
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    for i in 0..30_000 {
        let mut bytes = seeds[i % seeds.len()].as_bytes().to_vec();
        for _ in 0..=rng.below(4) {
            let at = rng.below(bytes.len() + 1);
            match rng.below(3) {
                0 if at < bytes.len() => {
                    bytes.remove(at);
                }
                1 => bytes.insert(at, NOISE[rng.below(NOISE.len())]),
                _ if at < bytes.len() => bytes[at] = NOISE[rng.below(NOISE.len())],
                _ => {}
            }
        }
        let text = String::from_utf8_lossy(&bytes);
        let _ = f.calc(&text);
        let _ = f.clone().define("z", &["a"], &text, "");
    }
}

#[test]
fn query_text_never_panics_or_hangs() {
    use gamectx::query;
    let tables = parse(
        r##"{"items":[{"id":"a","kind":"food","n":3,"w":0.50,"x":null},{"id":"b","kind":"tool","n":-2e3,"w":1,"x":true}],
            "empty":[]}"##,
        1 << 16,
    )
    .unwrap();
    let named: Vec<(&str, &[Value])> =
        tables.as_object().unwrap().iter().map(|(k, v)| (k.as_str(), v.as_array().unwrap())).collect();
    let seeds = [
        "SELECT id, n * w FROM items WHERE kind IN ('food', tool) AND n >= -1 ORDER BY w DESC LIMIT 1;",
        "select max(n) - min(w), round(avg(n * 2), 2) as a, count(x) from items where id <> \"b\"",
        "SELECT SUM(n) FROM empty WHERE n > 0.5 ORDER BY id",
        "SELECT ABS(-n % 0), -(-(w)) / 3 FROM items ORDER BY id",
    ];
    const NOISE: &[u8] = b"()+-*/%<>=!,;'\" .0123456789eE_acdefilmnorstuwxABCDEFILMNORSTW\xc3\xa9";
    let mut rng = Rng(0x0BAD_5EED_1234_5678);
    let mut accepted = 0;
    for i in 0..30_000 {
        let mut bytes = seeds[i % seeds.len()].as_bytes().to_vec();
        for _ in 0..=rng.below(4) {
            let at = rng.below(bytes.len() + 1);
            match rng.below(3) {
                0 if at < bytes.len() => {
                    bytes.remove(at);
                }
                1 => bytes.insert(at, NOISE[rng.below(NOISE.len())]),
                _ if at < bytes.len() => bytes[at] = NOISE[rng.below(NOISE.len())],
                _ => {}
            }
        }
        let text = String::from_utf8_lossy(&bytes);
        let mut session = query::Session::new().result_bytes(128);
        let feedback = session.submit(&text, &named).unwrap();
        assert_eq!(parse(&feedback.to_json(), usize::MAX).unwrap(), feedback);
        if !session.is_finished() {
            session.submit(&text, &named).unwrap();
        }
        assert!(session.is_finished());
        assert!(session.submit("SELECT 1 FROM items", &named).is_err());
        if let Ok(result) = query::run(&text, &named) {
            accepted += 1;
            assert_eq!(parse(&result.to_json(), usize::MAX).as_ref(), Ok(&result));
        }
    }
    assert!(accepted > 1000, "mutations should still produce many valid queries ({accepted})");
    let deep = format!("SELECT {}n{} FROM items", "ABS(".repeat(10_000), ")".repeat(10_000));
    assert!(query::run(&deep, &named).is_err());
}

#[test]
fn brain_definitions_learning_and_saved_text_never_panic() {
    use gamectx::{brain::Brain, obj};
    let definition = obj! {"version" => 1, "inputs" => vec!["x"], "outputs" => vec!["act"],
    "nodes" => vec![obj! {"id" => "act", "weights" => obj! {"x" => 0.5}, "bias" => 0.1,
        "pinned" => false, "learning_limit" => 0.5}]};
    let baseline = Brain::from_value(&definition).unwrap();
    let sample = |x: f64| obj! {"inputs" => obj! {"x" => x}, "targets" => obj! {"act" => 1}};
    let seeds = [
        definition.to_json(),
        (obj! {"id" => "lesson", "expected_revision" => 0,
        "changes" => vec![obj! {"node" => "act", "source" => "$bias", "delta" => 0.1}],
        "validation" => vec![sample(0.5)]})
        .to_json(),
        (obj! {"id" => "fit", "expected_revision" => 0, "examples" => vec![sample(0.2)],
        "validation" => vec![sample(0.8)], "epochs" => 2, "rate" => 0.1})
        .to_json(),
        baseline.to_value().to_json(),
    ];
    let mut rng = Rng(0xB12A_1A01_1122_3344);
    let mut parsed = 0;
    for i in 0..30_000 {
        let text = mutate(&mut rng, &seeds[i % seeds.len()]);
        let Ok(value) = parse(&text, 1 << 16) else { continue };
        parsed += 1;
        let mut brain = baseline.clone();
        let before = brain.to_value();
        let result = match i % 4 {
            0 => Brain::from_value(&value).map(|b| b.to_value()),
            1 => brain.apply_call("teach_brain", &obj! {"request" => text}),
            2 => brain.apply_call("train_brain", &obj! {"request" => text}),
            _ => Brain::restore(&value).map(|b| b.to_value()),
        };
        if let Ok(result) = result {
            assert_eq!(parse(&result.to_json(), usize::MAX).unwrap(), result);
        } else {
            assert_eq!(brain.to_value(), before, "invalid model text must not partially change learning");
        }
        assert_eq!(Brain::restore(&brain.to_value()).unwrap().to_value(), brain.to_value());
    }
    assert!(parsed > 1000);
}

#[test]
fn brain_rate_selection_text_never_panics_or_partially_updates() {
    use gamectx::{brain::Brain, obj};
    let workflow = parse(include_str!("../conformance/brain-select/select-earlier-tie/input.json"), 8192).unwrap();
    let brain = Brain::from_value(workflow.get("definition").unwrap()).unwrap();
    let args = workflow.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap().to_json();
    let mut rng = Rng(0x51EC_710A_54FE_1234);
    for _ in 0..10_000 {
        let text = mutate(&mut rng, &args);
        let mut candidate = brain.clone();
        let before = candidate.to_value();
        match candidate.apply_call("train_select_brain", &obj! {"request" => text}) {
            Ok(reply) => {
                assert_eq!(parse(&reply.to_json(), 1 << 16).unwrap(), reply);
                if reply.get("accepted") == Some(&Value::Bool(false)) {
                    assert_eq!(candidate.to_value(), before);
                }
            }
            Err(_) => assert_eq!(candidate.to_value(), before),
        }
        assert_eq!(Brain::restore(&candidate.to_value()).unwrap().to_value(), candidate.to_value());
    }
}

#[test]
fn cached_brain_workflow_text_matches_uncached_execution() {
    use gamectx::brain;
    let seeds = [
        include_str!("../conformance/brain-cache/repeated-normalized/input.json"),
        include_str!("../conformance/brain-cache/masks-thresholds-unknown/input.json"),
        include_str!("../conformance/brain-cache/learning-and-undo/input.json"),
    ];
    let mut rng = Rng(0xCAC4_ED01_54FE_1234);
    let mut accepted = 0;
    for i in 0..10_000 {
        let text = if i % 4 == 0 {
            // Include targeted numeric edits so valid cache keys vary too; random
            // punctuation edits mostly invalidate the larger workflow envelope.
            seeds[0].replace("0.8", ["0", "null", "0.3", "1", "-1", "true"][rng.below(6)])
        } else {
            mutate(&mut rng, seeds[i % seeds.len()])
        };
        let Ok(value) = parse(&text, 1 << 16) else { continue };
        let plain = brain::process(&value).map(|value| value.to_json()).map_err(|error| error.to_string());
        let cached =
            brain::process_cached(&value, i % 5).map(|value| value.to_json()).map_err(|error| error.to_string());
        if plain.is_ok() {
            accepted += 1;
        }
        assert_eq!(cached, plain, "cache must preserve valid results and reject the same model text");
    }
    assert!(accepted > 100, "exercise valid mutated workflows as well as errors: {accepted}");
}

#[test]
fn deep_nesting_is_refused_not_overflowed() {
    let deep = "[".repeat(100_000) + &"]".repeat(100_000);
    assert!(parse(&deep, usize::MAX).is_err());
}

#[test]
fn hostile_unpack_input_errors_cleanly() {
    for text in [
        r##"{"columns":["a"],"rows":[[1,2]]}"##,
        r##"{"columns":["a"],"rows":[[9]],"symbols":[],"symbol_columns":["a"]}"##,
        r##"{"columns":["a","b"],"rows":[[1]]}"##,
        r##"{"columns":"a","rows":[]}"##,
    ] {
        assert!(table::unpack(&parse(text, 1024).unwrap()).is_err(), "{text}");
    }
    for text in [
        r##"{"wrapper":"bare","rows":[["n",null,{"x":{"$ref":"#/$defs/p9"}},null]]}"##,
        r##"{"wrapper":"bare","rows":[["n",null,{},{"parameters":3}]]}"##,
        r##"{"wrapper":"other","rows":[]}"##,
        r##"{"wrapper":"bare","rows":[[1,null,{},null]]}"##,
    ] {
        assert!(schema::unpack(&parse(text, 1024).unwrap()).is_err(), "{text}");
    }
}
