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
    assert_eq!(value.json_len(), value.to_json().len());
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

#[test]
fn context_sync_model_text_is_bounded_and_atomic() {
    use gamectx::{
        obj,
        sync::{Publisher, Replica},
    };
    let mut sender = Publisher::new("fuzz").unwrap();
    let first = sender.publish(&obj! {"a"=>1,"b"=>"stable"}, None).unwrap();
    let delta = sender.publish(&obj! {"b"=>"stable","a"=>2}, Some(1)).unwrap().to_json();
    let snapshot = first.to_json();
    let mut rng = Rng(0x5A7E_0001_F022_9393);
    for i in 0..30_000 {
        let mut receiver = Replica::new("fuzz").unwrap();
        receiver.apply(&first).unwrap();
        let before = receiver.state().cloned();
        let revision = receiver.revision();
        let text = mutate(&mut rng, if i % 2 == 0 { &delta } else { &snapshot });
        if let Ok(reply) = receiver.apply_json(&text) {
            assert_eq!(parse(&reply.to_json(), 1 << 19).unwrap(), *reply);
            assert!(receiver.revision() > revision);
        } else {
            assert_eq!(receiver.state().cloned(), before);
            assert_eq!(receiver.revision(), revision);
        }
    }
}

#[test]
fn host_validation_and_sliced_model_text_never_partially_update() {
    use gamectx::brain::Brain;
    let workflow = parse(include_str!("../conformance/runtime/host-selection/input.json"), 1 << 16).unwrap();
    let workflow = workflow.get("workflow").unwrap();
    let brain = Brain::from_value(workflow.get("definition").unwrap()).unwrap();
    let seed = workflow.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap().to_json();
    let before = brain.to_value();
    let mut rng = Rng(0xB0A7_519E_0001_1234);
    for i in 0..10_000 {
        let text = mutate(&mut rng, &seed);
        let Ok(request) = parse(&text, 1 << 16) else { continue };
        match brain.start_train_select_host(&request) {
            Ok(mut job) => {
                if i % 2 == 0 {
                    job.advance(job.next_steps()).unwrap();
                }
                // Cancellation and rejected requests never touch the source brain.
                assert_eq!(brain.to_value(), before);
                let mut target = brain.clone();
                if !job.is_finished() {
                    assert!(job.finish(&mut target).is_err());
                    assert_eq!(target.to_value(), before);
                }
            }
            Err(_) => assert_eq!(brain.to_value(), before),
        }
    }
}

#[test]
fn new_binding_metadata_journal_and_replay_text_is_bounded_and_atomic() {
    use gamectx::{brain::FeatureSchema, memory::Journal, obj};
    let schema = include_str!("../examples/brain/bindings.json");
    let metadata = include_str!("../examples/query-meanings.json");
    let tables = parse(include_str!("../examples/query-meaning-tables.json"), 1 << 16).unwrap();
    let named: Vec<_> = tables.as_object().unwrap().iter().map(|(k, v)| (k.as_str(), v.as_array().unwrap())).collect();
    let row=(obj!{"turn"=>1,"entity"=>"actor_a","key"=>"near_cover","text"=>"The actor approached cover.","tags"=>vec!["activity"],"importance"=>5,"source"=>"observed","expires"=>20}).to_json();
    let trace = include_str!("../examples/brain/trace.json");
    let seeds = [schema, metadata, &row, trace];
    let mut rng = Rng(0xDA7A_4A51_7733_0012);
    for i in 0..40_000 {
        let text = mutate(&mut rng, seeds[i % 4]);
        match i % 4 {
            0 => {
                if let Ok(schema) = FeatureSchema::parse(&text) {
                    let _ = schema.normalize(&obj! {"x"=>2,"y"=>0.5});
                    assert_eq!(
                        schema.to_value(),
                        FeatureSchema::parse(&schema.to_value().to_json()).unwrap().to_value()
                    );
                }
            }
            1 => {
                if let Ok(value) = parse(&text, 1 << 16) {
                    if let Ok(result) = gamectx::query::describe_with(&named, &value) {
                        assert_eq!(parse(&result.to_json(), 1 << 20).unwrap(), result);
                    }
                }
            }
            2 => {
                let mut journal = Journal::new(4).unwrap();
                let before = journal.to_value();
                if let Ok(value) = parse(&text, 1 << 16) {
                    if journal.record(&value).is_err() {
                        assert_eq!(journal.to_value(), before);
                    }
                    assert_eq!(Journal::restore(&journal.to_value()).unwrap().to_value(), journal.to_value());
                }
            }
            _ => {
                if let Ok(result) = gamectx::replay::run_json(&text) {
                    assert_eq!(parse(&result.to_json(), 1 << 20).unwrap(), result);
                }
            }
        }
    }
}

#[test]
fn cited_hypothesis_and_saved_evidence_text_never_mutates_observations() {
    use gamectx::{memory::EvidenceMemory, obj};
    let mut native = EvidenceMemory::new("actor_a", "game_ticks", 8, 4).unwrap();
    for id in 1..=2 {
        native.observe(id,&obj!{"turn"=>id,"entity"=>"actor_a","key"=>format!("signal_{id}"),"text"=>"The actor approached cover.","tags"=>vec!["activity"],"importance"=>5,"source"=>"observed","expires"=>30},&["position"]).unwrap();
    }
    let seed=(obj!{"id"=>"seeking_shelter","statement"=>"The actor may be seeking shelter.","confidence"=>0.6,"supports"=>vec![1],"contradicts"=>vec![2],"ttl"=>20}).to_json();
    let first_view = native.snapshot(2, 16, 8000, &[]).unwrap();
    native.propose(&first_view, 2, &parse(&seed, 1 << 16).unwrap(), &[]).unwrap();
    let saved = native.to_value();
    let state = saved.to_json();
    let mut rng = Rng(0xE71D_EACE_0123_ABCD);
    for i in 0..20_000 {
        let text = mutate(&mut rng, if i % 2 == 0 { &seed } else { &state });
        if i % 2 == 0 {
            let mut memory = EvidenceMemory::restore(&saved, "actor_a", "game_ticks").unwrap();
            let view = memory.snapshot(2, 16, 8000, &[]).unwrap();
            if let Ok(request) = parse(&text, 1 << 16) {
                let before = memory.to_value();
                if memory.propose(&view, 2, &request, &[]).is_err() {
                    assert_eq!(memory.to_value(), before);
                }
                assert_eq!(memory.to_value().get("journal"), saved.get("journal"));
            }
        } else if let Ok(memory) = EvidenceMemory::restore_json(&text, "actor_a", "game_ticks") {
            let saved = memory.to_value();
            assert_eq!(EvidenceMemory::restore(&saved, "actor_a", "game_ticks").unwrap().to_value(), saved);
            let _ = memory.snapshot(2, 16, 8000, &["position"]);
        }
    }
}

#[test]
fn registered_fact_claims_and_saved_states_are_bounded_and_atomic() {
    use gamectx::facts::{FactOptions, FactRegistry};
    let mut facts = FactRegistry::new(8).unwrap();
    let source = facts.register("world", "available", true, FactOptions::default()).unwrap();
    facts.register("plan", "ready", true, FactOptions { dependencies: vec![source], ..Default::default() }).unwrap();
    let saved = facts.to_value();
    let state = saved.to_json();
    let seed = r#"{"available":true,"count":9007199254740993,"ratio":1.00e0}"#;
    let mut rng = Rng(0xFAC7_0123_A55E_1789);
    for i in 0..20_000 {
        let text = mutate(&mut rng, if i % 2 == 0 { seed } else { &state });
        if i % 2 == 0 {
            if let Ok(value) = parse(&text, 1 << 16) {
                let _ = facts.check("plan", "ready", &value, 0, &[]);
                assert_eq!(facts.to_value(), saved);
                let mut candidate = facts.clone();
                if candidate.register("world", "candidate", value, FactOptions::default()).is_err() {
                    assert_eq!(candidate.to_value(), saved);
                } else {
                    let next = candidate.to_value();
                    assert_eq!(FactRegistry::restore(&next).unwrap().to_value(), next);
                }
            }
        } else if let Ok(restored) = FactRegistry::restore_json(&text) {
            let saved = restored.to_value();
            assert_eq!(FactRegistry::restore(&saved).unwrap().to_value(), saved);
            if let Ok(packet) = restored.packet(u32::MAX as u64, 8, 1024, &[]) {
                assert!(packet.json_len() <= 1024);
                assert_eq!(parse(&packet.to_json(), 16384).unwrap(), packet);
            }
        }
    }
}
