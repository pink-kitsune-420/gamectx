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

#[allow(dead_code)] // Interactive host methods are covered by the dungeon integration test.
#[path = "../examples/dungeon/game.rs"]
mod dungeon;
use dungeon as game;
#[allow(dead_code)]
#[path = "../examples/dungeon/intent.rs"]
mod dungeon_intent;
#[allow(dead_code)] // Rendering is exercised by the dungeon host integration tests.
#[path = "../examples/dungeon/social.rs"]
mod dungeon_social;

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
fn dungeon_replays_are_bounded_and_never_panic() {
    let seeds = [
        include_str!("../examples/dungeon/cases/win_7/input.json"),
        include_str!("../examples/dungeon/cases/stale/input.json"),
        include_str!("../examples/dungeon/cases/invalid_shape/input.json"),
    ];
    let mut rng = Rng(0x5197_8234_bccc_1101);
    for i in 0..10_000 {
        let input = mutate(&mut rng, seeds[i % seeds.len()]);
        if let Ok(v) = parse(&input, 262144) {
            if let Ok(result) = dungeon::replay(&v) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
        }
    }
}

#[test]
fn npc_reply_and_memory_parsers_are_bounded_and_atomic() {
    let seeds = [
        include_str!("../examples/dungeon/social-cases/remember/input.json"),
        include_str!("../examples/dungeon/social-cases/bad_memory/input.json"),
        include_str!("../examples/dungeon/social-cases/stale/input.json"),
    ];
    let mut rng = Rng(0x219a_1234_bccc_7771);
    let mut world = game::Game::new(7).unwrap();
    world.act(&0.into(), &game::pair("go", Some("room:gallery")));
    world.act(&1.into(), &game::pair("go", Some("room:workshop")));
    let before = world.public();
    for i in 0..10_000 {
        if let Ok(v) = parse(&mutate(&mut rng, seeds[i % seeds.len()]), 262144) {
            let _ = dungeon_social::replay(&v);
        }
        let mut social = dungeon_social::Social::default();
        let ticket = social.prepare(&world, "merchant", "Hello.").unwrap();
        let initial = social.state(&world, "merchant").unwrap();
        let text = mutate(&mut rng, r#"{"speech":"Hello.","tone":"warm","fact_ids":[],"memory_ids":[]}"#);
        if social.finish(&world, ticket.get("id").unwrap().as_str().unwrap(), &text).is_err() {
            assert_eq!(initial, social.state(&world, "merchant").unwrap());
        }
        assert_eq!(before, world.public());
    }
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

#[path = "support/upgrades.rs"]
mod upgrades;

#[test]
fn runtime_upgrade_workflow_text_is_bounded() {
    let seeds = [
        include_str!("../conformance/upgrades/recall-aliases/input.json"),
        include_str!("../conformance/upgrades/outcome-one-label/input.json"),
        include_str!("../conformance/upgrades/temporal-windows/input.json"),
        include_str!("../conformance/upgrades/ledger-concurrent-recovery/input.json"),
        include_str!("../conformance/upgrades/playbook-current-preconditions/input.json"),
        include_str!("../conformance/upgrades/request-current/input.json"),
        include_str!("../conformance/upgrades/bundle-reuse/input.json"),
    ];
    let mut rng = Rng(0x0A11_0123_4567_8910);
    for i in 0..14000 {
        let text = mutate(&mut rng, seeds[i % 7]);
        if let Ok(v) = parse(&text, 1 << 16) {
            if let Ok(result) = upgrades::process(&v) {
                assert_eq!(parse(&result.to_json(), 1 << 20).unwrap(), result);
            }
        }
    }
}

#[test]
fn saved_runtime_and_authored_configuration_mutations_round_trip() {
    use gamectx::{
        brain::BrainBundle, playbook::Playbook, recall::RecallIndex, requests::RequestContext, spending::SpendingLedger,
    };
    let workflow = parse(include_str!("../conformance/upgrades/bundle-reuse/input.json"), 1 << 16).unwrap();
    let bundle = BrainBundle::new(
        gamectx::brain::Brain::from_value(workflow.get("perception").unwrap()).unwrap(),
        gamectx::brain::Brain::from_value(workflow.get("decision").unwrap()).unwrap(),
        workflow.get("ports").unwrap(),
    )
    .unwrap()
    .to_value()
    .to_json();
    let book = parse(include_str!("../conformance/upgrades/playbook-current-preconditions/input.json"), 1 << 16)
        .unwrap()
        .get("recipes")
        .unwrap()
        .to_json();
    let mut ledger = SpendingLedger::new(100, 10).unwrap();
    ledger.reserve(0, 60).unwrap();
    let ledger = ledger.to_value().to_json();
    let context = parse(include_str!("../conformance/upgrades/request-current/input.json"), 1 << 16)
        .unwrap()
        .get("operations")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .get("context")
        .unwrap()
        .to_json();
    let config = r#"{"normalize":true,"aliases":[["healer","medic"]]}"#;
    let seeds = [bundle.as_str(), book.as_str(), ledger.as_str(), context.as_str(), config];
    let mut rng = Rng(0x0B11_9123_4567_8910);
    for i in 0..40000 {
        let text = mutate(&mut rng, seeds[i % 5]);
        match i % 5 {
            0 => {
                if let Ok(state) = BrainBundle::restore_json(&text) {
                    let saved = state.to_value();
                    assert_eq!(BrainBundle::restore(&saved).unwrap().to_value(), saved);
                }
            }
            1 => {
                if let Ok(book) = Playbook::from_json(&text) {
                    let saved = book.to_value();
                    assert_eq!(Playbook::from_value(&saved).unwrap().to_value(), saved);
                }
            }
            2 => {
                if let Ok(ledger) = SpendingLedger::restore_json(&text, u32::MAX as u64) {
                    let saved = ledger.to_value();
                    assert_eq!(SpendingLedger::restore(&saved, u32::MAX as u64).unwrap().to_value(), saved);
                }
            }
            3 => {
                if let Ok(v) = parse(&text, 1 << 16) {
                    let _ = RequestContext::from_value(&v);
                }
            }
            _ => {
                if let Ok(v) = parse(&text, 1 << 16) {
                    if let Ok(index) = RecallIndex::new(&[], &v) {
                        let _ = index.search("healer", 8);
                    }
                }
            }
        }
    }
}

#[test]
fn query_contracts_plans_and_identifier_text_survive_mutation() {
    use gamectx::{
        obj,
        query::{Query, Shape},
        view,
    };
    let seeds = [
        r#"{"table":"cargo","filter_columns":["mass","mass"],"limit":null,"aggregate":false}"#,
        r#"{"table":"cargo","filter_columns":[],"limit":3,"aggregate":true}"#,
        r#"{"table":"門.café","filter_columns":[],"limit":4294967296,"aggregate":true}"#,
    ];
    let rows = vec![obj! {"id"=>"grove:café_map","mass"=>3}, obj! {"id"=>"quest.gate.open","mass"=>2}];
    let query = Query::parse("SELECT SUM(mass) FROM cargo ORDER BY mass DESC LIMIT 3").unwrap();
    let mut rng = Rng(0x1234_8910_5555_2222);
    for i in 0..20_000 {
        let text = mutate(&mut rng, seeds[i % seeds.len()]);
        if let Ok(value) = parse(&text, 8192) {
            if let Ok(shape) = Shape::from_value(&value) {
                assert_eq!(Shape::from_value(&shape.to_value()).unwrap().to_value(), shape.to_value());
                let _ = query.require_shape(&shape);
            }
        }
        let _ = gamectx::query::Guidance::parse(&text);
        let scores = view::relevance_scores(&rows, &text);
        assert!(scores.iter().all(|s| *s <= text.chars().count() * 2));
        assert_eq!(parse(&query.plan().to_json(), 8192).unwrap(), query.plan());
    }
}

#[test]
fn kit_model_text_mutations() {
    let project = gamectx::kit::Project::parse(include_str!("../examples/kit/project.json")).unwrap();
    let seeds = [
        r#"{"version":1,"id":"a","op":"validate","args":{"name":"inspect_item","arguments":{"id":"grove:Élan.1"}}}"#,
        r#"{"version":1,"id":"a","op":"query","args":{"recipe":"conversation","snapshot":{"now":1,"tables":{"stock":{"updated_at":1,"rows":[]}},"sections":{"event":"hi"}},"sql":"SELECT COUNT(*) FROM stock"}}"#,
        r#"{"version":1,"id":"a","op":"features","args":{"dataset":"stock","revision":1,"policies":[{"name":"count","min":0,"max":100,"invert":false,"missing":"unknown","outside":"clamp"}]}}"#,
        include_str!("../conformance/kit/nested/input.json"),
        include_str!("../examples/kit/project.json"),
    ];
    let mut rng = Rng(0x7452_891a_0041_6677);
    for i in 0..20_000 {
        let input = mutate(&mut rng, seeds[i % seeds.len()]);
        let reply = project.request(&input);
        assert_eq!(parse(&reply.to_json(), gamectx::kit::MAX_REQUEST_BYTES).as_ref(), Ok(&reply));
        if let Ok(v) = parse(&input, gamectx::kit::MAX_REQUEST_BYTES) {
            let _ = gamectx::kit::process(&v);
            let _ = gamectx::kit::Project::from_value(&v);
            let _ = project.validate_call("inspect_item", &v);
            let _ = project.snapshot(&v);
        }
    }
}

#[test]
fn local_decision_dialogue_profile_and_telemetry_text_is_bounded() {
    let seeds = [
        include_str!("../conformance/local/decision-correction/input.json"),
        include_str!("../conformance/local/scene/input.json"),
        include_str!("../conformance/local/profile-concise/input.json"),
        include_str!("../conformance/local/report/input.json"),
        include_str!("../conformance/local/budget-1529/input.json"),
        include_str!("../conformance/workflow/direct/input.json"),
        include_str!("../conformance/workflow/direct_dialogue/input.json"),
        include_str!("../conformance/workflow/3_dialogue/input.json"),
    ];
    let mut rng = Rng(0x619b_0a77_5512_0081);
    for i in 0..20_000 {
        let input = mutate(&mut rng, seeds[i % seeds.len()]);
        if let Ok(v) = parse(&input, 1_572_864) {
            if let Ok(result) = gamectx::local::process(&v) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
            let _ = gamectx::dialogue::Thread::from_value(&v);
            let _ = gamectx::dialogue::Scene::from_value(&v);
            let _ = gamectx::deployment::Profile::from_value(&v);
            if let Ok(c) = gamectx::decision::Choices::from_value(&v) {
                let _ = c.session().apply(&input, 0, |_| false);
            }
        }
    }
}

#[test]
fn optimization_contracts_and_model_text_are_bounded() {
    let seeds = [
        include_str!("../conformance/optimization/intent_repair/input.json"),
        include_str!("../conformance/optimization/template_injection/input.json"),
        include_str!("../conformance/optimization/evidence_group/input.json"),
        include_str!("../conformance/optimization/compose_recount/input.json"),
        include_str!("../conformance/optimization/route_conflict/input.json"),
        include_str!("../conformance/optimization/codec_success/input.json"),
        include_str!("../conformance/optimization/tune_select/input.json"),
    ];
    let mut rng = Rng(0x741d_4126_d987_aa93);
    for i in 0..20_000 {
        let input = mutate(&mut rng, seeds[i % seeds.len()]);
        if let Ok(value) = parse(&input, 1_572_864) {
            if let Ok(result) = gamectx::local::process(&value) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
        }
    }
    let v = parse(seeds[5], 1 << 16).unwrap();
    let a = v.get("args").unwrap();
    let choices = gamectx::decision::Choices::from_value(a.get("choices").unwrap()).unwrap();
    let codec = gamectx::decision::ChoiceCodec::new(&choices, a.get("names").unwrap()).unwrap();
    for _ in 0..10_000 {
        let text = mutate(&mut rng, r#"{"choice":"inspect_gate"}"#);
        if let Ok(pair) = codec.decode(&text) {
            choices.validate(&pair).unwrap();
        }
        let _ = codec.session().apply(&text, 7, |_| false);
    }
}

#[test]
fn entity_reference_mutations_never_invent_actions_or_targets() {
    let world = game::Game::new(7).unwrap();
    let pairs = world.choices();
    let items = world.visible_items();
    let seeds = ["Inspect it.", "Take the keeper seal.", "Go to the gallery or the workshop.", "Inspect café.門_1"];
    let mut rng = Rng(0x219b_7700_ac99_4131);
    for i in 0..10_000 {
        let text = mutate(&mut rng, seeds[i % seeds.len()]);
        let resolved = dungeon_intent::resolve(&pairs, &items, &text, Some("room:gallery"), None).unwrap();
        assert!(resolved.pairs.iter().all(|p| pairs.contains(p)));
        for candidate in resolved.candidates {
            let id = candidate.get("id").unwrap();
            assert!(pairs.iter().any(|p| p.get("target") == Some(id)) || items.iter().any(|p| p.get("id") == Some(id)));
        }
    }
}

#[test]
fn shared_conversation_reference_and_confirmation_model_text_is_bounded() {
    let seeds = [
        include_str!("../conformance/conversation/remember/input.json"),
        include_str!("../conformance/conversation/ambiguous/input.json"),
        include_str!("../conformance/conversation/confirm/input.json"),
    ];
    let mut rng = Rng(0x7712_0119_aabb_cc30);
    for i in 0..20_000 {
        let input = mutate(&mut rng, seeds[i % seeds.len()]);
        if let Ok(value) = parse(&input, 65536) {
            if let Ok(result) = gamectx::local::process(&value) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
        }
    }
    let args = gamectx::obj! {"scope"=>"shop","speaker"=>"trader","revision"=>1,"input"=>"Hello.","facts"=>gamectx::obj! {"price"=>3},"recall"=>2,"history"=>2};
    for _ in 0..10_000 {
        let mut c = gamectx::dialogue::Conversation::default();
        c.prepare(&args).unwrap();
        let before = c.state("shop").unwrap();
        let text = mutate(&mut rng, r#"{"speech":"Hello.","tone":"neutral","fact_ids":["price"],"memory_ids":[]}"#);
        if c.finish("shop", 1, "chat_1", &text).is_err() {
            assert_eq!(before, c.state("shop").unwrap());
        }
    }
}

#[test]
fn lore_and_source_answer_text_remain_bounded_and_cannot_supply_new_prose() {
    let seeds = [
        include_str!("../conformance/lore/select/input.json"),
        include_str!("../conformance/lore/answer/input.json"),
        include_str!("../conformance/lore/quote/input.json"),
    ];
    let mut rng = Rng(0x8811_749a_3010_ccdd);
    for i in 0..20_000 {
        let text = mutate(&mut rng, seeds[i % seeds.len()]);
        if let Ok(value) = parse(&text, 262144) {
            if let Ok(result) = gamectx::local::process(&value) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
        }
    }
    let input = parse(seeds[1], 65536).unwrap();
    let args = input.get("args").unwrap();
    let answers = gamectx::dialogue::Answers::from_value(args.get("definition").unwrap()).unwrap();
    let context = gamectx::obj! {"scope"=>"player:trader","revision"=>7};
    for _ in 0..10_000 {
        let text = mutate(&mut rng, r#"{"answer":"price"}"#);
        if let Ok(result) = answers.resolve(&text, &context, |_| true) {
            assert!([
                "The oil costs 3 coins.",
                "You haven't supplied that information.",
                "You told me: \"My home is Reed Hollow.\""
            ]
            .contains(&result.get("text").unwrap().as_str().unwrap()));
        }
    }
}

#[test]
fn informational_tool_text_and_integer_transfer_proposals_fail_atomically() {
    let snapshot = parse(include_str!("../conformance/info-inventory/propose/input.json"), 262144)
        .unwrap()
        .get("args")
        .unwrap()
        .get("snapshot")
        .unwrap()
        .clone();
    let evidence = gamectx::obj! {"id"=>"line:1","speaker"=>"player","text"=>"The oil is mine."};
    let mut rng = Rng(0x99aa_3150_2190_8877);
    for _ in 0..10_000 {
        let mut inv = gamectx::inventory::Inventory::from_value(&snapshot).unwrap();
        let before = inv.to_value();
        let input = mutate(&mut rng, r#"{"revision":0,"item":"item:oil","to":"player","quantity":2}"#);
        if inv.propose("merchant", &input, |_| true).is_err() {
            assert_eq!(before, inv.to_value());
        } else {
            let total: usize = inv
                .to_value()
                .get("holdings")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .filter(|h| h.get("item").and_then(Value::as_str) == Some("item:oil"))
                .map(|h| h.get("quantity").unwrap().as_usize().unwrap())
                .sum();
            assert_eq!(total, 4);
        }
        let mut notes = gamectx::info::Notes::new("shop").unwrap();
        let before = notes.to_value();
        let input =
            mutate(&mut rng, r#"{"subject":"item:oil","attribute":"owner","value":"player","source":"line:1"}"#);
        match notes.call("info_remember", &input, &vec![evidence.clone()].into()) {
            Err(_) => assert_eq!(before, notes.to_value()),
            Ok(v) => {
                assert_eq!(v.get("status"), Some(&Value::str("reported")));
                assert_eq!(v.get("source"), Some(&evidence));
            }
        }
        let input = mutate(&mut rng, include_str!("../conformance/info-inventory/lore_values/input.json"));
        if let Ok(value) = parse(&input, 262144) {
            if let Ok(output) = gamectx::local::process(&value) {
                assert_eq!(parse(&output.to_json(), 262144).unwrap(), output);
            }
        }
    }
}

#[test]
fn conversation_tool_model_text_is_bounded_and_failed_calls_preserve_private_state() {
    let reference = parse(include_str!("../conformance/interactions/clarify_ambiguous/input.json"), 262144)
        .unwrap()
        .get("args")
        .unwrap()
        .get("operations")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .get("args")
        .unwrap()
        .clone();
    let offers = parse(include_str!("../conformance/interactions/offer_confirm/input.json"), 262144)
        .unwrap()
        .get("args")
        .unwrap()
        .get("operations")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .get("args")
        .unwrap()
        .clone();
    let evidence =
        vec![gamectx::obj! {"id"=>"line:1","speaker"=>"player:a","text"=>"I will bring oil. I prefer quiet."}].into();
    let seeds = [
        ("events_recent", r#"{"since":0,"subject":null,"limit":8}"#),
        (
            "promise_remember",
            r#"{"promisor":"player:a","beneficiary":"npc:merchant","task":"bring oil","source":"line:1"}"#,
        ),
        ("promise_lookup", r#"{"participant":"player:a","limit":8}"#),
        ("relationship_note", r#"{"other":"player:a","note":"Prefers quiet.","source":"line:1"}"#),
        ("relationship_lookup", r#"{"other":"player:a","limit":8}"#),
        ("dialogue_resolve", r#"{"kind":"item"}"#),
        ("interaction_offers", r#"{"limit":8}"#),
        ("interaction_offer", r#"{"id":"offer:1"}"#),
    ];
    let mut rng = Rng(0x1237_9870_65aa_42bb);
    for i in 0..20_000 {
        let mut t = gamectx::dialogue::InteractionTools::new(
            "scene:shop",
            "npc:merchant",
            &vec!["npc:merchant", "player:a", "player:b"].into(),
        )
        .unwrap();
        t.set_references(&reference).unwrap();
        t.set_offers(&offers).unwrap();
        let before = t.to_value();
        let (name, seed) = seeds[i % seeds.len()];
        let input = mutate(&mut rng, seed);
        match t.call(name, &input, &evidence, 7, 2048, |_| true) {
            Err(_) => assert_eq!(before, t.to_value()),
            Ok(value) => {
                assert!(value.json_len() <= 2048);
                assert_eq!(parse(&value.to_json(), 2048).unwrap(), value);
                if matches!(name, "promise_remember" | "relationship_note") {
                    assert_eq!(value.get("status"), Some(&Value::str("reported")));
                }
            }
        }
    }
    for i in 0..10_000 {
        let seed = if i % 2 == 0 {
            include_str!("../conformance/interactions/offer_other_player/input.json")
        } else {
            include_str!("../conformance/interactions/promise_lifecycle/input.json")
        };
        if let Ok(v) = parse(&mutate(&mut rng, seed), 1_048_576) {
            if let Ok(result) = gamectx::local::process(&v) {
                assert_eq!(parse(&result.to_json(), 1_572_864).unwrap(), result);
            }
        }
    }
}

#[test]
fn perception_and_character_model_calls_are_bounded() {
    use gamectx::{
        character::{CharacterTools, CharacterView, TurnPlan},
        location::Locations,
        world::WorldState,
    };
    let definition = parse(include_str!("../conformance/location/full/input.json"), 1 << 20).unwrap();
    let locations = Locations::from_value(definition.get("args").unwrap().get("definition").unwrap()).unwrap();
    let current = definition.get("args").unwrap().get("current").unwrap().as_str().unwrap();
    let revision = locations.revision();
    let world_input =
        parse(include_str!("../conformance/world/clock_and_unknown_weather/input.json"), 1 << 20).unwrap();
    let world = WorldState::from_value(world_input.get("args").unwrap().get("definition").unwrap()).unwrap();
    let mut character = CharacterTools::new("audience:a", "npc:a", &vec!["npc:a", "player:a"].into()).unwrap();
    character.bind(CharacterView::new(revision).with_location(locations.clone(), current).unwrap()).unwrap();
    character.enable(&["look_around"]).unwrap();
    let seeds = [r#"{"detail":"full"}"#, r#"{"keys":["time","weather"]}"#, r#"{"detail":"brief","current":"hidden"}"#];
    let mut rng = Rng(0x9aac_8899_bcdd_1122);
    for i in 0..20_000 {
        let text = mutate(&mut rng, seeds[i % seeds.len()]);
        for v in [
            locations.call(&text, current, revision, 4096, |_| true),
            world.call(&text, world.revision(), 4096, |_| false),
            character.call("look_around", &text, revision, 4096, |_| false),
        ]
        .into_iter()
        .flatten()
        {
            assert!(v.json_len() <= 4096);
            assert_eq!(parse(&v.to_json(), 4096).unwrap(), v);
        }
        let _ = gamectx::dialogue::render_quote(&text);
        if let Ok(v) = parse(&text, 4096) {
            let _ = Locations::from_value(&v);
            let _ = WorldState::from_value(&v);
            let _ = CharacterView::from_value(&v);
            let _ = TurnPlan::from_value(&v);
        }
    }
}

#[test]
fn routines_observations_and_proposals_reject_mutated_untrusted_text() {
    use gamectx::{
        character::{CharacterTools, CharacterView},
        routine::{Config, Frame, Routine, Runtime},
    };
    use std::sync::Arc;
    let a = parse(include_str!("../conformance/routines/interruption_resume/input.json"), 1_500_000).unwrap();
    let a = a.get("args").unwrap();
    let definition = a.get("definition").unwrap();
    let cfg=Config::resolve(&gamectx::obj!{"routines"=>true,"history"=>true,"time"=>true,"tools"=>vec!["routine_status","activity_propose"],"time_sources"=>vec!["sun"]},&gamectx::obj!{}).unwrap();
    let mut runtime = Runtime::new(
        "scope:a",
        "merchant",
        &vec!["merchant"].into(),
        "cycle:37",
        cfg,
        Some(Arc::new(Routine::from_value(definition).unwrap())),
    )
    .unwrap();
    runtime.update(Frame::from_value(&a.get("frames").unwrap().as_array().unwrap()[0]).unwrap()).unwrap();
    let mut character = CharacterTools::new("scope:a", "merchant", &vec!["merchant"].into()).unwrap();
    character.with_runtime(runtime.clone()).unwrap();
    character.bind(CharacterView::new(1)).unwrap();
    character.enable(&["routine_status", "activity_propose"]).unwrap();
    let before = runtime.to_value();
    let mut rng = Rng(0xa328_c237_dfa5_1257);
    let snapshot = before.to_json();
    let d = definition.to_json();
    for i in 0..20_000 {
        let seed = if i % 2 == 0 { "{}" } else { r#"{"action":"activity","target":"work"}"# };
        let input = mutate(&mut rng, seed);
        for name in
            ["routine_status", "activity_propose", "character_routine_observation", "character_routine_complete"]
        {
            if let Ok(v) = character.call(name, &input, 1, 8192, |_| true) {
                assert!(v.json_len() <= 8192);
                assert_eq!(parse(&v.to_json(), 8192).unwrap(), v);
            }
        }
        assert_eq!(character.routine().unwrap().to_value(), before);
        let model = mutate(&mut rng, if i % 2 == 0 { &snapshot } else { &d });
        if let Ok(v) = parse(&model, 1_500_000) {
            let _ = Routine::from_value(&v);
            let _ = Config::resolve(&v, &gamectx::obj! {});
            let _ = Frame::from_value(&v);
            let mut copy = runtime.clone();
            if copy.restore(&v, 1, 0, 0, 0).is_err() {
                assert_eq!(copy.to_value(), before);
            }
            let mut copy = runtime.clone();
            if copy.observe(&v).is_err() {
                assert_eq!(copy.to_value(), before);
            }
        }
    }
}

#[test]
fn observation_selection_mutations_are_bounded_and_read_only() {
    use gamectx::observation::{History, Selection};
    let config = gamectx::routine::Config::resolve(&gamectx::obj! {"history"=>true}, &gamectx::obj! {}).unwrap();
    let mut history = History::new("scope:a", "merchant", &vec!["merchant"].into(), "cycle:37", config).unwrap();
    let input = parse(include_str!("../conformance/routines/remembered_sunlight/input.json"), 1_500_000).unwrap();
    history.observe(&input.get("args").unwrap().get("observations").unwrap().as_array().unwrap()[0]).unwrap();
    let before = history.to_value();
    let mut rng = Rng(0x37a3_1239_881d_2256);
    for _ in 0..10_000 {
        let text =
            mutate(&mut rng, r#"{"kinds":["direct"],"locations":[],"time_sources":["sun"],"required":[1],"limit":1}"#);
        if let Ok(v) = parse(&text, 8192) {
            if let Ok(s) = Selection::from_value(&v) {
                if let Ok(result) = history.select(2, &s) {
                    assert!(result.as_array().unwrap().len() <= 3);
                    assert_eq!(parse(&result.to_json(), 32768).unwrap(), result);
                }
            }
        }
        assert_eq!(history.to_value(), before);
    }
}

#[test]
fn life_model_text_is_bounded_and_atomic() {
    use gamectx::life::{Config, Definition, Life, Setting};
    use std::sync::Arc;
    let setting =
        Setting::from_value(&parse(include_str!("../examples/characters/setting.json"), 65536).unwrap()).unwrap();
    let definition = Arc::new(
        Definition::draft(&parse(include_str!("../examples/characters/merchant-life.json"), 65536).unwrap(), &setting)
            .unwrap(),
    );
    let life = Life::accept(
        "scope",
        "merchant",
        &vec!["merchant", "player"].into(),
        definition,
        Config::resolve(&gamectx::obj! {"enabled"=>true}, &gamectx::obj! {}).unwrap(),
    )
    .unwrap();
    let before = life.inspect();
    let mut rng = Rng(0x545195);
    for _ in 0..10000 {
        let text = mutate(&mut rng, r#"{"rule":"returned_lamp","event":1,"reason":"A returned lamp."}"#);
        let _ = life.propose(&text);
        if let Ok(v) = parse(&text, 65536) {
            let _ = Definition::draft(&v, &setting);
            let _ = Setting::from_value(&v);
        }
    }
    assert_eq!(before, life.inspect());
}

#[test]
fn continuity_inputs_never_panic() {
    let mut rng = Rng(0x171912);
    let seed = include_str!("../conformance/continuity/interrupted/input.json");
    for _ in 0..10000 {
        let s = mutate(&mut rng, seed);
        if let Ok(v) = parse(&s, 65536) {
            let _ = gamectx::local::process(&v);
        }
    }
}
