//! Deterministic mutation testing: malformed input must produce errors, never panics,
//! and anything that parses must survive packing and serialize back to itself.

use gamectx::data::json::{parse, Value};
use gamectx::{data::catalog, data::table, packet::schema, packet::signatures};

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
#[path = "../examples/lantern-vault/game.rs"]
mod dungeon;
use dungeon as game;
#[allow(dead_code)]
#[path = "../examples/lantern-vault/intent.rs"]
mod dungeon_intent;
#[allow(dead_code)] // Rendering is exercised by the dungeon host integration tests.
#[path = "../examples/lantern-vault/social.rs"]
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
    assert!(value.is_valid(value.json_len()));
    assert_eq!(value.json_len(), value.to_json().len());
    assert_eq!(parse(&value.to_json(), usize::MAX).as_ref(), Ok(value), "serialization must round-trip");
    let mut session = gamectx::data::query::Session::new();
    session.apply_call(value, &[("items", &[])]).unwrap();
    if !session.is_finished() {
        session.apply_call(value, &[("items", &[])]).unwrap();
    }
    assert!(session.apply_call(value, &[("items", &[])]).is_err());
    if let Some(items) = value.as_array() {
        let _ = gamectx::data::query::describe(&[("items", items)]);
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
        include_str!("../examples/lantern-vault/cases/win_7/input.json"),
        include_str!("../examples/lantern-vault/cases/stale/input.json"),
        include_str!("../examples/lantern-vault/cases/invalid_shape/input.json"),
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
        include_str!("../examples/lantern-vault/social-cases/remember/input.json"),
        include_str!("../examples/lantern-vault/social-cases/bad_memory/input.json"),
        include_str!("../examples/lantern-vault/social-cases/stale/input.json"),
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
    use gamectx::data::formula::Formulas;
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
    use gamectx::data::query;
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
    use gamectx::{obj, runtime::brain::Brain};
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
    use gamectx::{obj, runtime::brain::Brain};
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
    use gamectx::runtime::brain;
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
        packet::sync::{Publisher, Replica},
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
    use gamectx::runtime::brain::Brain;
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
    use gamectx::{knowledge::memory::Journal, obj, runtime::brain::FeatureSchema};
    let schema = include_str!("../examples/fixtures/brain/bindings.json");
    let metadata = include_str!("../examples/data/query-meanings.json");
    let tables = parse(include_str!("../examples/data/query-meaning-tables.json"), 1 << 16).unwrap();
    let named: Vec<_> = tables.as_object().unwrap().iter().map(|(k, v)| (k.as_str(), v.as_array().unwrap())).collect();
    let row=(obj!{"turn"=>1,"entity"=>"actor_a","key"=>"near_cover","text"=>"The actor approached cover.","tags"=>vec!["activity"],"importance"=>5,"source"=>"observed","expires"=>20}).to_json();
    let trace = include_str!("../examples/fixtures/brain/trace.json");
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
                    if let Ok(result) = gamectx::data::query::describe_with(&named, &value) {
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
                if let Ok(result) = gamectx::runtime::replay::run_json(&text) {
                    assert_eq!(parse(&result.to_json(), 1 << 20).unwrap(), result);
                }
            }
        }
    }
}

#[test]
fn cited_hypothesis_and_saved_evidence_text_never_mutates_observations() {
    use gamectx::{knowledge::memory::EvidenceMemory, obj};
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
    use gamectx::knowledge::facts::{FactOptions, FactRegistry};
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
        knowledge::recall::RecallIndex, runtime::brain::BrainBundle, runtime::playbook::Playbook,
        runtime::requests::RequestContext, runtime::spending::SpendingLedger,
    };
    let workflow = parse(include_str!("../conformance/upgrades/bundle-reuse/input.json"), 1 << 16).unwrap();
    let bundle = BrainBundle::new(
        gamectx::runtime::brain::Brain::from_value(workflow.get("perception").unwrap()).unwrap(),
        gamectx::runtime::brain::Brain::from_value(workflow.get("decision").unwrap()).unwrap(),
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
        data::query::{Query, Shape},
        data::view,
        obj,
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
        let _ = gamectx::data::query::Guidance::parse(&text);
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
            if let Ok(result) = gamectx::kit::local::process(&v) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
            let _ = gamectx::character::dialogue::Thread::from_value(&v);
            let _ = gamectx::character::dialogue::Scene::from_value(&v);
            let _ = gamectx::deploy::deployment::Profile::from_value(&v);
            if let Ok(c) = gamectx::runtime::decision::Choices::from_value(&v) {
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
            if let Ok(result) = gamectx::kit::local::process(&value) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
        }
    }
    let v = parse(seeds[5], 1 << 16).unwrap();
    let a = v.get("args").unwrap();
    let choices = gamectx::runtime::decision::Choices::from_value(a.get("choices").unwrap()).unwrap();
    let codec = gamectx::runtime::decision::ChoiceCodec::new(&choices, a.get("names").unwrap()).unwrap();
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
            if let Ok(result) = gamectx::kit::local::process(&value) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
        }
    }
    let args = gamectx::obj! {"scope"=>"shop","speaker"=>"trader","revision"=>1,"input"=>"Hello.","facts"=>gamectx::obj! {"price"=>3},"recall"=>2,"history"=>2};
    for _ in 0..10_000 {
        let mut c = gamectx::character::dialogue::Conversation::default();
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
            if let Ok(result) = gamectx::kit::local::process(&value) {
                assert_eq!(parse(&result.to_json(), 1_572_864).as_ref(), Ok(&result));
            }
        }
    }
    let input = parse(seeds[1], 65536).unwrap();
    let args = input.get("args").unwrap();
    let answers = gamectx::character::dialogue::Answers::from_value(args.get("definition").unwrap()).unwrap();
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
        let mut inv = gamectx::knowledge::inventory::Inventory::from_value(&snapshot).unwrap();
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
        let mut notes = gamectx::knowledge::info::Notes::new("shop").unwrap();
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
            if let Ok(output) = gamectx::kit::local::process(&value) {
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
        let mut t = gamectx::character::dialogue::InteractionTools::new(
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
            if let Ok(result) = gamectx::kit::local::process(&v) {
                assert_eq!(parse(&result.to_json(), 1_572_864).unwrap(), result);
            }
        }
    }
}

#[test]
fn perception_and_character_model_calls_are_bounded() {
    use gamectx::{
        character::tools::{CharacterTools, CharacterView, TurnPlan},
        knowledge::location::Locations,
        knowledge::world::WorldState,
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
        let _ = gamectx::character::dialogue::render_quote(&text);
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
        character::routine::{Config, Frame, Routine, Runtime},
        character::tools::{CharacterTools, CharacterView},
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
    use gamectx::knowledge::observation::{History, Selection};
    let config =
        gamectx::character::routine::Config::resolve(&gamectx::obj! {"history"=>true}, &gamectx::obj! {}).unwrap();
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
    use gamectx::character::life::{Config, Definition, Life, Setting};
    use std::sync::Arc;
    let setting =
        Setting::from_value(&parse(include_str!("../examples/fixtures/characters/setting.json"), 65536).unwrap())
            .unwrap();
    let definition = Arc::new(
        Definition::draft(
            &parse(include_str!("../examples/fixtures/characters/merchant-life.json"), 65536).unwrap(),
            &setting,
        )
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
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn character_checkpoints_reject_mutations_without_partial_restore() {
    use gamectx::character::life::{Config, Coordinator, Definition, Life, Setting};
    use gamectx::obj;
    use std::sync::Arc;
    let setting =
        Setting::from_value(&parse(include_str!("../examples/fixtures/characters/setting.json"), 65536).unwrap())
            .unwrap();
    let d = Arc::new(
        Definition::draft(
            &parse(include_str!("../examples/fixtures/characters/merchant-life.json"), 65536).unwrap(),
            &setting,
        )
        .unwrap(),
    );
    let mut life = Life::accept(
        "scope",
        "merchant",
        &vec!["merchant", "player"].into(),
        d,
        Config::resolve(&obj! {"enabled"=>true}, &obj! {}).unwrap(),
    )
    .unwrap();
    let id = obj! {"campaign"=>"vault","branch"=>"a","instance"=>"merchant"};
    let checkpoint = life.checkpoint(&id).unwrap().to_json();
    let before = life.inspect();
    let fence = obj! {"min_revision"=>0,"max_revision"=>10,"min_token"=>0,"max_token"=>10,"max_tick"=>10};
    let mut rng = Rng(0x878956);
    for _ in 0..10000 {
        let text = mutate(&mut rng, &checkpoint);
        if let Ok(v) = parse(&text, 524288) {
            let _ = life.restore_checkpoint(&id, &v, &fence);
            let _ = Coordinator::recover(&v, 0, 10);
        }
        assert_eq!(life.inspect(), before);
    }
}

#[test]
fn initiative_candidate_mutations_are_bounded() {
    let mut rng = Rng(0x990154);
    let seed = include_str!("../conformance/initiative/attitude_priority/input.json");
    for _ in 0..10000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn speech_turns_and_sentence_diagnostics_bound_model_written_text() {
    let seed = include_str!("../conformance/life-speech-v2/unknown/input.json");
    let mut rng = Rng(0x125813);
    for _ in 0..10000 {
        let text = mutate(&mut rng, seed);
        if let Ok(value) = parse(&text, 65536) {
            let _ = gamectx::kit::local::process(&value);
        }
        let speech = mutate(&mut rng, "Your lamp is ready. If you need to know more,");
        let before = speech.clone();
        let _ = gamectx::character::life::assess_sentence(&speech, 240);
        assert_eq!(before, speech);
    }
}

#[test]
fn archetype_and_party_frames_reject_mutations_with_bounded_work() {
    let seeds = [
        include_str!("../conformance/archetype-party/compile_untrusted/input.json"),
        include_str!("../conformance/archetype-party/escalate_significance/input.json"),
        include_str!("../conformance/archetype-party/loadout_ambient/input.json"),
    ];
    let mut rng = Rng(0x452891);
    for seed in seeds {
        for _ in 0..4000 {
            let text = mutate(&mut rng, seed);
            if let Ok(value) = parse(&text, 262144) {
                let _ = gamectx::kit::local::process(&value);
            }
        }
    }
}

#[test]
fn cast_world_and_async_speech_inputs_never_panic_or_wait_for_model_work() {
    let seeds = [
        include_str!("../conformance/living-world/pack_valid/input.json"),
        include_str!("../conformance/living-world/rumor_chain_and_ripple/input.json"),
        include_str!("../conformance/living-world/upgrade_before_delivery/input.json"),
    ];
    let mut rng = Rng(0x642753);
    for seed in seeds {
        for _ in 0..3000 {
            let text = mutate(&mut rng, seed);
            if let Ok(v) = parse(&text, 1_572_864) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn visual_contract_worker_and_repair_mutations_are_bounded() {
    let seeds = [
        include_str!("../conformance/visual/occlusion_wins/input.json"),
        include_str!("../conformance/visual/candidate_never_default_art/input.json"),
        include_str!("../conformance/visual/repair_forbids_prop_and_cue/input.json"),
    ];
    let mut rng = Rng(0x515855);
    for seed in seeds {
        for _ in 0..3000 {
            let text = mutate(&mut rng, seed);
            if let Ok(v) = parse(&text, 1_572_864) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn studio_provider_and_voice_inputs_never_dispatch_during_mutation_checks() {
    let seeds = [
        include_str!("../conformance/studio/shared_cap/input.json"),
        include_str!("../conformance/studio/barge_in/input.json"),
    ];
    let mut rng = Rng(0x53545544);
    for seed in seeds {
        for _ in 0..3000 {
            let text = mutate(&mut rng, seed);
            if let Ok(v) = parse(&text, 65536) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn transient_dialogue_mutations_are_bounded_and_never_become_rumor_origins() {
    let seed = include_str!("../conformance/deep-transient/one/input.json");
    let mut rng = Rng(0x7472616e);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 1_572_864) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn scoped_archival_recall_mutations_fail_closed() {
    let seed = include_str!("../conformance/deep-memory/after_eviction/input.json");
    let mut rng = Rng(0x61726368);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 1_572_864) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn source_speech_rejects_mutated_unapproved_text_and_stale_sources() {
    let seed = include_str!("../conformance/deep-speech/denied_invention/input.json");
    let mut rng = Rng(0x73706565);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn human_command_mutations_never_execute_or_escape_current_choices() {
    let seed = include_str!("../conformance/deep-command/phrase_7/input.json");
    let mut rng = Rng(0x636f6d6d);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 65536) {
            if let Ok(answer) = gamectx::kit::local::process(&v) {
                if answer.get("status") == Some(&"resolved".into()) {
                    let args = v.get("args").unwrap();
                    let choices = args.get("choices").unwrap().as_array().unwrap();
                    assert!(choices.iter().any(|c| Some(c) == answer.get("decision")));
                }
            }
        }
    }
}

#[test]
fn required_speech_never_uses_generic_unknown_to_skip_source_validation() {
    let seed = include_str!("../conformance/deep-required-speech/known_absence_keeps_source/input.json");
    let mut rng = Rng(0x72657164);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn model_manager_policy_mutations_are_bounded_and_never_execute_services() {
    let seed = include_str!("../conformance/model-manager/combat/input.json");
    let mut rng = Rng(0x6d6f646c);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn native_audio_plans_never_execute_or_accept_paths() {
    let seed = r#"{"kind":"studio-native-audio","args":{"engine":"piper","request":{"model":"piper","input":"Welcome, Vela.","voice":"merchant","speed":1,"response_format":"wav"}}}"#;
    let mut rng = Rng(0x61756469);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = gamectx::data::json::parse(&text, 4096) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn speech_buffer_plans_are_bounded_under_mutation() {
    let seed = r#"{"kind":"studio-speech-buffer","args":{"text":"Welcome, Vela. The lamp is ready.","start":0}}"#;
    let mut rng = Rng(0x62756666);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 4096) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn voice_archive_retention_mutations_never_access_files() {
    let seed = include_str!("../conformance/voice-archive/fifo/input.json");
    let mut rng = Rng(0x61726368);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn viewpoint_text_and_checkpoints_are_bounded_under_mutation() {
    let seed = include_str!("../conformance/viewpoints/confirmed/input.json");
    let mut rng = Rng(0x76696577);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn reaction_gate_mutations_never_execute_world_actions() {
    let seed = include_str!("../conformance/reactions/eligible/input.json");
    let mut rng = Rng(0x72656163);
    for _ in 0..2000 {
        let text = mutate(&mut rng, seed);
        if let Ok(v) = parse(&text, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn social_routing_mutations_never_promote_text_to_world_actions() {
    let seed = include_str!("../conformance/social-routing/screenshot/input.json");
    let mut rng = Rng(0x736f6369);
    for _ in 0..2000 {
        let s = mutate(&mut rng, seed);
        if let Ok(v) = parse(&s, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn encounter_receipt_mutations_do_not_create_world_actions() {
    let seed = include_str!("../conformance/greeting-memory/confirmed/input.json");
    let mut rng = Rng(0x67726565);
    for _ in 0..2000 {
        let s = mutate(&mut rng, seed);
        if let Ok(v) = parse(&s, 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn deliberation_and_untrusted_hypothesis_mutations_are_bounded() {
    let seeds = [
        include_str!("../conformance/deliberation/enabled/input.json"),
        include_str!("../conformance/deliberation/hypothesis/input.json"),
    ];
    let mut rng = Rng(0x726561736f6e);
    for seed in seeds {
        for _ in 0..1000 {
            if let Ok(v) = parse(&mutate(&mut rng, seed), 65536) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn message_chunk_mutations_never_create_calls_or_unbounded_packets() {
    let seeds = [
        include_str!("../conformance/message-chunks/long/input.json"),
        include_str!("../conformance/message-chunks/two_sentences/input.json"),
    ];
    let mut rng = Rng(0x6368756e6b73);
    for seed in seeds {
        for _ in 0..1000 {
            if let Ok(v) = parse(&mutate(&mut rng, seed), 65536) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn registered_action_and_phrase_mutations_remain_proposals() {
    let seed = include_str!("../conformance/roleplay-actions/buy/input.json");
    let mut rng = Rng(0x7374616765);
    for _ in 0..2000 {
        if let Ok(v) = parse(&mutate(&mut rng, seed), 65536) {
            let _ = gamectx::kit::local::process(&v);
        }
    }
}

#[test]
fn expressive_contracts_and_disclosures_remain_bounded_under_mutation() {
    let mut rng = Rng(0x65787072657373);
    for seed in [
        include_str!("../conformance/expression/packet/input.json"),
        include_str!("../conformance/expression/correction/input.json"),
        include_str!("../conformance/expression/fiction_line/input.json"),
    ] {
        for _ in 0..1000 {
            if let Ok(v) = parse(&mutate(&mut rng, seed), 131072) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn source_bound_wording_and_restatement_filters_reject_mutated_model_text() {
    let mut rng = Rng(0x776f7264696e67);
    for seed in [
        include_str!("../conformance/wording/price/input.json"),
        include_str!("../conformance/expression/paraphrase_repeat/input.json"),
    ] {
        for _ in 0..1000 {
            if let Ok(v) = parse(&mutate(&mut rng, seed), 131072) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn tale_parts_and_rotation_remain_bounded_under_mutation() {
    let mut rng = Rng(0x74616c6573);
    for seed in [
        include_str!("../conformance/expression/story_tale/input.json"),
        include_str!("../conformance/expression/story_least_recent/input.json"),
    ] {
        for _ in 0..1000 {
            if let Ok(v) = parse(&mutate(&mut rng, seed), 8192) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn roleplay_engine_player_text_configs_and_states_survive_mutation_atomically() {
    use gamectx::character::roleplay::engine::{EngineConfig, RoleplayEngine};
    let mut rng = Rng(0x726f6c65_706c6179);
    for seed in [
        include_str!("../conformance/roleplay-engine/disclosure_recall/input.json"),
        include_str!("../conformance/roleplay-engine/prop_interaction/input.json"),
        include_str!("../conformance/roleplay-engine/restore_continues/input.json"),
        include_str!("../conformance/roleplay-engine/custom_line_first/input.json"),
        include_str!("../conformance/roleplay-engine/greeting_continuing/input.json"),
        include_str!("../conformance/roleplay-engine/b_secret_reveal/input.json"),
        include_str!("../conformance/roleplay-engine/b_hear_through/input.json"),
        include_str!("../conformance/roleplay-engine/b_quest/input.json"),
        include_str!("../conformance/roleplay-engine/b_norm/input.json"),
        include_str!("../conformance/roleplay-engine/b_services_member/input.json"),
    ] {
        for _ in 0..600 {
            if let Ok(v) = parse(&mutate(&mut rng, seed), 262144) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
    // A rejected turn or delivery never partially updates the engine.
    let input = parse(include_str!("../conformance/roleplay-engine/disclosure_recall/input.json"), 65536).unwrap();
    let args = input.get("args").unwrap();
    let config = EngineConfig::from_value(args.get("config").unwrap()).unwrap();
    let mut engine = RoleplayEngine::new(config);
    let turn = args.get("ops").unwrap().as_array().unwrap()[0].get("args").unwrap().to_json();
    for i in 0..2000 {
        let before = engine.checkpoint();
        if let Ok(v) = parse(&mutate(&mut rng, &turn), 65536) {
            if engine.turn(&v).is_err() {
                assert_eq!(engine.checkpoint(), before, "failed turn {i} changed state");
            }
            let receipt = gamectx::obj! {"token"=>v.get("token").cloned(),"listener"=>"player_1","text"=>"Arin; thank you.","heard"=>999};
            let before = engine.checkpoint();
            assert!(engine.delivered(&receipt).is_err());
            assert_eq!(engine.checkpoint(), before);
        }
    }
    // The same holds for turns that carry entrances, needs, quests and host flags.
    let input = parse(include_str!("../conformance/roleplay-engine/b_secret_reveal/input.json"), 262144).unwrap();
    let args = input.get("args").unwrap();
    let mut engine = RoleplayEngine::new(EngineConfig::from_value(args.get("config").unwrap()).unwrap());
    let mut turn = args.get("ops").unwrap().as_array().unwrap()[0].get("args").unwrap().clone();
    if let gamectx::Value::Object(fields) = &mut turn {
        fields.push(("needs".into(), gamectx::obj! {"rest"=>80}));
        fields.push(("quests".into(), gamectx::obj! {"lost_ring"=>"asked"}));
        fields.push(("state".into(), gamectx::obj! {"time"=>"morning","trusted_friend"=>true}));
    }
    let turn = turn.to_json();
    assert!(engine.clone().turn(&parse(&turn, 65536).unwrap()).is_ok());
    for i in 0..2000 {
        let before = engine.checkpoint();
        if let Ok(v) = parse(&mutate(&mut rng, &turn), 65536) {
            if engine.turn(&v).is_err() {
                assert_eq!(engine.checkpoint(), before, "failed extended turn {i} changed state");
            }
        }
    }
}

#[test]
fn save_and_game_data_files_survive_mutation() {
    use gamectx::kit::save::{GameData, SaveFile};
    let mut rng = Rng(0x7361_7665_6669_6c65);
    let expected = parse(include_str!("../conformance/save-file/slots_and_fork/expected.json"), 1 << 24).unwrap();
    let results = expected.get("results").unwrap().as_array().unwrap();
    let data = results[0].get("ok").unwrap().to_json();
    let save = results[5].get("ok").unwrap().to_json();
    let game = GameData::parse(&data).unwrap();
    assert!(SaveFile::parse(&save).unwrap().check(&game).unwrap());
    for _ in 0..1500 {
        let _ = GameData::parse(&mutate(&mut rng, &data));
        if let Ok(s) = SaveFile::parse(&mutate(&mut rng, &save)) {
            // Anything that still parses kept every digest consistent; a re-serialized copy
            // parses back to the same value.
            assert_eq!(SaveFile::parse(&s.to_json()).unwrap(), s);
            let _ = s.check(&game);
        }
    }
    for seed in [
        include_str!("../conformance/save-file/slots_and_fork/input.json"),
        include_str!("../conformance/save-file/load_round_trip/input.json"),
        include_str!("../conformance/save-file/put_and_remove_data/input.json"),
    ] {
        for _ in 0..300 {
            if let Ok(v) = parse(&mutate(&mut rng, seed), 1 << 24) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
}

#[test]
fn model_reply_text_is_bounded_and_failed_replies_change_nothing() {
    use gamectx::character::roleplay::engine::{EngineConfig, RoleplayEngine};
    let mut rng = Rng(0x6d6f_6465_6c5f_7478);
    let input = parse(include_str!("../conformance/roleplay-engine/c_reply_accepted/input.json"), 262144).unwrap();
    let args = input.get("args").unwrap();
    let mut engine = RoleplayEngine::new(EngineConfig::from_value(args.get("config").unwrap()).unwrap());
    let ops = args.get("ops").unwrap().as_array().unwrap();
    let out = engine.turn(ops[0].get("args").unwrap()).unwrap();
    assert_eq!(out.get("kind").and_then(gamectx::Value::as_str), Some("model"));
    let seeds = [
        r#"{"speech": "The new mayor seems fair enough to me."}"#,
        "```json\n{\"speech\": \"Caf\\u00e9 bread, \\\"fresh\\\" today.\"}\n```",
        r#"{"speech":"😀 One. Two? Three!"}"#,
        "As an AI, *smiles* <b>player_1</b>",
    ];
    for seed in seeds {
        for _ in 0..1500 {
            let raw = mutate(&mut rng, seed);
            let before = engine.checkpoint();
            let reply = gamectx::obj! {"token"=>1,"listener"=>"player_1","text"=>raw.as_str()};
            let mut trial = engine.clone();
            match trial.reply(&reply) {
                Ok(out) if out.get("kind").and_then(gamectx::Value::as_str) == Some("reply") => {
                    let text = out.get("text").and_then(gamectx::Value::as_str).unwrap();
                    assert!(!text.is_empty() && text.chars().count() <= 240 && !text.chars().any(char::is_control));
                    assert!(!text.contains(['*', '<', '>', '{', '}', '[', ']']));
                }
                Ok(out) => {
                    assert_eq!(out.get("kind").and_then(gamectx::Value::as_str), Some("rejected"));
                    assert_eq!(trial.checkpoint(), before, "a rejected reply changed state");
                }
                Err(_) => panic!("model text must never be a host error"),
            }
        }
    }
}

#[test]
fn world_model_modules_survive_mutation_and_extreme_sizes() {
    use gamectx::knowledge::codex::Codex;
    use gamectx::knowledge::timeline::Timeline;
    let mut rng = Rng(0x776f_726c_645f_6d6f);

    // 1. Timeline mutation fuzzing
    let mut t = Timeline::new();
    let sample_event = r#"{"at":100,"kind":"history","summary":"The outer gate was sealed.","place":"vault","actors":["keeper"],"tags":["lore"]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_event);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let before = t.entries().len();
            if t.record(&v).is_ok() {
                assert!(t.entries().len() >= before);
            }
        }
    }

    // 2. Codex mutation fuzzing (places, things, recipes, species, communities)
    let sample_codex = r#"{
        "places": [{"id":"room_a","label":"Hall","kind":"hall","links":[{"place":"room_b","distance":1,"direction":"north"}]}],
        "things": [{"id":"key_1","label":"Iron Key","kind":"key"}],
        "recipes": [{"id":"craft_key","kind":"crafting","label":"Craft Key","inputs":[{"thing":"key_1","qty":1}],"output":{"thing":"key_1","qty":1}}],
        "species": [{"id":"hound","label":"Hound","kind":"animal","temperament":"loyal"}],
        "communities": [{"id":"artisans","label":"Artisans Guild","members":["smith"]}]
    }"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_codex);
        if let Ok(c) = Codex::parse(&mutated) {
            // Re-serialize check
            assert_eq!(Codex::parse(&c.to_value().to_json()).unwrap().places().len(), c.places().len());
        }
    }

    // 3. Extreme sizes and nested structures
    // Boundary test: maximum allowed places is 128, version 1 is required
    let mut boundary = String::from("{\"version\":1,\"places\":[");
    for i in 0..128 {
        if i > 0 {
            boundary.push(',');
        }
        boundary.push_str(&format!(r#"{{"id":"p_{i}","label":"Room {i}","kind":"room"}}"#));
    }
    boundary.push_str("]}");
    let parsed = Codex::parse(&boundary).unwrap();
    assert_eq!(parsed.places().len(), 128);

    // Out-of-bounds test: 129 places must cleanly return Err without panicking
    let mut too_many = String::from("{\"version\":1,\"places\":[");
    for i in 0..129 {
        if i > 0 {
            too_many.push(',');
        }
        too_many.push_str(&format!(r#"{{"id":"p_{i}","label":"Room {i}","kind":"room"}}"#));
    }
    too_many.push_str("]}");
    assert!(Codex::parse(&too_many).is_err());

    // 4. ActionCatalog mutation fuzzing & boundaries
    use gamectx::knowledge::action_catalog::ActionCatalog;
    let mut cat = ActionCatalog::new();
    let sample_action_req = r#"{"ops":[{"op":"check","args":{"action":"pet","actor":{"id":"hero","species":"person"},"bindings":{"animal":"dog"}}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_action_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = cat.workflow(&v);
        }
    }
    // Boundary test: ActionCatalog from_value accepts up to 256 actions and rejects 257
    let mut actions_json = String::from("{\"version\":1,\"actions\":[");
    for i in 0..256 {
        if i > 0 {
            actions_json.push(',');
        }
        actions_json.push_str(&format!(
            r#"{{"id":"act_{i}","label":"Action {i}","category":"things","description":"Desc {i}","actor":{{}},"targets":[],"preconditions":{{}},"cost":{{"time":1,"fatigue":1}},"effects":{{"world":"test"}},"perception":{{"visibility":"public"}},"risk":{{"failure_chance":0}},"duration":1,"interruptible":true}}"#
        ));
    }
    actions_json.push_str("]}");
    assert!(ActionCatalog::parse(&actions_json).is_ok());

    let mut too_many_actions = String::from("{\"version\":1,\"actions\":[");
    for i in 0..257 {
        if i > 0 {
            too_many_actions.push(',');
        }
        too_many_actions.push_str(&format!(
            r#"{{"id":"act_{i}","label":"Action {i}","category":"things","description":"Desc {i}","actor":{{}},"targets":[],"preconditions":{{}},"cost":{{"time":1,"fatigue":1}},"effects":{{"world":"test"}},"perception":{{"visibility":"public"}},"risk":{{"failure_chance":0}},"duration":1,"interruptible":true}}"#
        ));
    }
    too_many_actions.push_str("]}");
    assert!(ActionCatalog::parse(&too_many_actions).is_err());

    // 5. WitnessBridge mutation fuzzing & boundaries
    use gamectx::character::life::witness::WitnessBridge;
    let mut bridge = WitnessBridge::new();
    let sample_witness_req = r#"{"ops":[{"op":"witness","args":{"entry":{"at":10,"kind":"test","summary":"test"},"beings":[{"id":"baker","place":"road"}]}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_witness_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = bridge.workflow(&v);
        }
    }
    // Boundary test: WitnessBridge accepts up to 64 beings and rejects 65
    let mut beings_64 =
        String::from(r#"{"ops":[{"op":"witness","args":{"entry":{"at":10,"kind":"test","summary":"test"},"beings":["#);
    for i in 0..64 {
        if i > 0 {
            beings_64.push(',');
        }
        beings_64.push_str(&format!(r#"{{"id":"b_{i}","place":"road"}}"#));
    }
    beings_64.push_str("]}}]}");
    assert!(bridge.workflow(&parse(&beings_64, 1 << 20).unwrap()).is_ok());

    let mut beings_65 =
        String::from(r#"{"ops":[{"op":"witness","args":{"entry":{"at":10,"kind":"test","summary":"test"},"beings":["#);
    for i in 0..65 {
        if i > 0 {
            beings_65.push(',');
        }
        beings_65.push_str(&format!(r#"{{"id":"b_{i}","place":"road"}}"#));
    }
    beings_65.push_str("]}}]}");
    assert!(bridge.workflow(&parse(&beings_65, 1 << 20).unwrap()).is_err());

    // 6. BodyManager mutation fuzzing & boundaries
    use gamectx::character::body::BodyManager;
    let mut body_mgr = BodyManager::new();
    let sample_body_req = r#"{"ops":[{"op":"init","args":{"being":"baker","health":100,"fatigue":20,"needs":[{"id":"hunger","threshold":70,"rate_per_tick":1,"level":10,"satisfiers":[{"action":"eat","amount":30}]}]}},{"op":"assess","args":{"being":"baker"}},{"op":"tick","args":{"being":"baker","ticks":3}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_body_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = body_mgr.workflow(&v);
        }
    }

    // Boundary test: BodyManager accepts up to 64 ops and rejects 65
    let mut clean_mgr = BodyManager::new();
    let mut ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            ops_64.push(',');
        }
        ops_64.push_str(&format!(r#"{{"op":"assess","args":{{"being":"b_{i}"}}}}"#));
    }
    ops_64.push_str("]}");
    assert!(clean_mgr.workflow(&parse(&ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_mgr2 = BodyManager::new();
    let mut ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            ops_65.push(',');
        }
        ops_65.push_str(&format!(r#"{{"op":"assess","args":{{"being":"b_{i}"}}}}"#));
    }
    ops_65.push_str("]}");
    assert!(clean_mgr2.workflow(&parse(&ops_65, 1 << 20).unwrap()).is_err());

    // 7. MotiveArbiter mutation fuzzing & boundaries
    use gamectx::character::motive::MotiveArbiter;
    let arbiter = MotiveArbiter::new();
    let sample_motive_req = r#"{"ops":[{"op":"arbitrate","args":{"body":{"health":100,"needs":[{"id":"hunger","level":80,"threshold":70}]},"routine":{"id":"bake","priority":50}}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_motive_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = arbiter.workflow(&v);
        }
    }

    // Boundary test: MotiveArbiter accepts up to 64 motives and rejects 65
    let mut motives_64 = String::from(r#"{"ops":[{"op":"arbitrate","args":{"motives":["#);
    for i in 0..64 {
        if i > 0 {
            motives_64.push(',');
        }
        motives_64.push_str(&format!(r#"{{"id":"m_{i}","kind":"goal","score":50}}"#));
    }
    motives_64.push_str("]}}]}");
    assert!(arbiter.workflow(&parse(&motives_64, 1 << 20).unwrap()).is_ok());

    let mut motives_65 = String::from(r#"{"ops":[{"op":"arbitrate","args":{"motives":["#);
    for i in 0..65 {
        if i > 0 {
            motives_65.push(',');
        }
        motives_65.push_str(&format!(r#"{{"id":"m_{i}","kind":"goal","score":50}}"#));
    }
    motives_65.push_str("]}}]}");
    assert!(arbiter.workflow(&parse(&motives_65, 1 << 20).unwrap()).is_err());

    // 8. TaskManager mutation fuzzing & boundaries
    use gamectx::character::task::TaskManager;
    let mut task_mgr = TaskManager::new();
    let sample_task_req = r#"{"ops":[{"op":"propose","args":{"id":"t1","owner":"baker","steps":[{"id":"s1","action":"look","target_bindings":{}}]}},{"op":"accept","args":{"id":"t1"}},{"op":"advance","args":{"id":"t1","outcome":{"success":true}}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_task_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = task_mgr.workflow(&v);
        }
    }

    // Boundary test: TaskManager accepts up to 64 ops and rejects 65
    let mut clean_task_mgr = TaskManager::new();
    let mut task_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            task_ops_64.push(',');
        }
        task_ops_64.push_str(&format!(r#"{{"op":"get","args":{{"id":"t_{i}"}}}}"#));
    }
    task_ops_64.push_str("]}");
    // get on nonexistent task returns Err, but let's test validate op which succeeds!
    let mut val_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            val_ops_64.push(',');
        }
        val_ops_64.push_str(r#"{"op":"validate","args":{"steps":[{"id":"s1","action":"look","target_bindings":{}}]}}"#);
    }
    val_ops_64.push_str("]}");
    assert!(clean_task_mgr.workflow(&parse(&val_ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_task_mgr2 = TaskManager::new();
    let mut val_ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            val_ops_65.push(',');
        }
        val_ops_65.push_str(r#"{"op":"validate","args":{"steps":[{"id":"s1","action":"look","target_bindings":{}}]}}"#);
    }
    val_ops_65.push_str("]}");
    assert!(clean_task_mgr2.workflow(&parse(&val_ops_65, 1 << 20).unwrap()).is_err());

    // 9. AffectManager mutation fuzzing & boundaries
    use gamectx::character::affect::AffectManager;
    let mut affect_mgr = AffectManager::new();
    let sample_affect_req = r#"{"ops":[{"op":"init","args":{"being":"baker","temperament":"calm","mood":{"valence":20,"arousal":10},"attitudes":[{"id":"fear_wolf","kind":"fear","subject":"wolf","level":30}]}},{"op":"tick","args":{"being":"baker","ticks":5}},{"op":"appraise","args":{"being":"baker","kind":"gratitude","subject":"miller","delta":15}},{"op":"assess","args":{"being":"baker"}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_affect_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = affect_mgr.workflow(&v);
        }
    }

    let mut clean_affect = AffectManager::new();
    clean_affect.workflow(&parse(r#"{"ops":[{"op":"init","args":{"being":"baker"}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut affect_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            affect_ops_64.push(',');
        }
        affect_ops_64.push_str(r#"{"op":"assess","args":{"being":"baker"}}"#);
    }
    affect_ops_64.push_str("]}");
    assert!(clean_affect.workflow(&parse(&affect_ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_affect2 = AffectManager::new();
    clean_affect2.workflow(&parse(r#"{"ops":[{"op":"init","args":{"being":"baker"}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut affect_ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            affect_ops_65.push(',');
        }
        affect_ops_65.push_str(r#"{"op":"assess","args":{"being":"baker"}}"#);
    }
    affect_ops_65.push_str("]}");
    assert!(clean_affect2.workflow(&parse(&affect_ops_65, 1 << 20).unwrap()).is_err());

    // 10. SkillsManager mutation fuzzing & boundaries
    use gamectx::character::skills::SkillsManager;
    let mut skills_mgr = SkillsManager::new();
    let sample_skills_req = r#"{"ops":[{"op":"init","args":{"being":"baker","skills":[{"id":"baking","level":10,"practice":20}]}},{"op":"practice","args":{"being":"baker","skill":"baking","practice":30,"success":true}},{"op":"evaluate_action","args":{"being":"baker","skill":"baking","action":"cook","base_duration":20,"base_risk":25}},{"op":"assess","args":{"being":"baker"}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_skills_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = skills_mgr.workflow(&v);
        }
    }

    let mut clean_skills = SkillsManager::new();
    clean_skills.workflow(&parse(r#"{"ops":[{"op":"init","args":{"being":"baker"}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut skills_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            skills_ops_64.push(',');
        }
        skills_ops_64.push_str(r#"{"op":"assess","args":{"being":"baker"}}"#);
    }
    skills_ops_64.push_str("]}");
    assert!(clean_skills.workflow(&parse(&skills_ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_skills2 = SkillsManager::new();
    clean_skills2.workflow(&parse(r#"{"ops":[{"op":"init","args":{"being":"baker"}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut skills_ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            skills_ops_65.push(',');
        }
        skills_ops_65.push_str(r#"{"op":"assess","args":{"being":"baker"}}"#);
    }
    skills_ops_65.push_str("]}");
    assert!(clean_skills2.workflow(&parse(&skills_ops_65, 1 << 20).unwrap()).is_err());

    // 11. MindManager mutation fuzzing & boundaries
    use gamectx::character::mind::MindManager;
    let mut mind_mgr = MindManager::new();
    let sample_mind_req = r#"{"ops":[{"op":"init","args":{"subject":"baker"}},{"op":"record","args":{"subject":"baker","other":"apprentice","category":"knows","fact":"cart_crash","status":false,"source":"inferred","at":10}},{"op":"record","args":{"subject":"baker","other":"customer","category":"wants","goal":"bread","priority":80,"source":"told","at":12}},{"op":"record","args":{"subject":"baker","other":"rival","category":"feels","emotion":"envy","level":30,"source":"inferred","at":14}},{"op":"assess","args":{"subject":"baker"}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_mind_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = mind_mgr.workflow(&v);
        }
    }

    let mut clean_mind = MindManager::new();
    clean_mind.workflow(&parse(r#"{"ops":[{"op":"init","args":{"subject":"baker"}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut mind_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            mind_ops_64.push(',');
        }
        mind_ops_64.push_str(r#"{"op":"assess","args":{"subject":"baker"}}"#);
    }
    mind_ops_64.push_str("]}");
    assert!(clean_mind.workflow(&parse(&mind_ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_mind2 = MindManager::new();
    clean_mind2.workflow(&parse(r#"{"ops":[{"op":"init","args":{"subject":"baker"}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut mind_ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            mind_ops_65.push(',');
        }
        mind_ops_65.push_str(r#"{"op":"assess","args":{"subject":"baker"}}"#);
    }
    mind_ops_65.push_str("]}");
    assert!(clean_mind2.workflow(&parse(&mind_ops_65, 1 << 20).unwrap()).is_err());

    // 12. DeceptionLedger mutation fuzzing & boundaries
    use gamectx::character::deception::DeceptionLedger;
    let mut dec_ledger = DeceptionLedger::new();
    let sample_dec_req = r#"{"ops":[{"op":"record","args":{"speaker":"thief","listener":"guard","topic":"stolen_ring","claim":"saw_nothing","true_value":"in_pocket","at":10}},{"op":"check_consistency","args":{"speaker":"thief","listener":"guard","topic":"stolen_ring","proposed_claim":"saw_nothing"}},{"op":"appraise_contradiction","args":{"listener":"guard","topic":"stolen_ring","observed_truth":"found_in_pocket"}},{"op":"assess","args":{"listener":"guard"}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_dec_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = dec_ledger.workflow(&v);
        }
    }

    let mut clean_dec = DeceptionLedger::new();
    let mut dec_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            dec_ops_64.push(',');
        }
        dec_ops_64.push_str(r#"{"op":"assess","args":{}}"#);
    }
    dec_ops_64.push_str("]}");
    assert!(clean_dec.workflow(&parse(&dec_ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_dec2 = DeceptionLedger::new();
    let mut dec_ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            dec_ops_65.push(',');
        }
        dec_ops_65.push_str(r#"{"op":"assess","args":{}}"#);
    }
    dec_ops_65.push_str("]}");
    assert!(clean_dec2.workflow(&parse(&dec_ops_65, 1 << 20).unwrap()).is_err());

    // 13. RelationshipManager mutation fuzzing & boundaries
    use gamectx::character::relationship::RelationshipManager;
    let mut rel_mgr = RelationshipManager::new();
    let sample_rel_req = r#"{"ops":[{"op":"init","args":{"sheet_relationships":[{"subject":"baker","with":"player","kind":"friend"}],"attitudes":[{"subject":"baker","target":"player","kind":"trust","level":20}]}},{"op":"resolve","args":{"subject":"baker","target":"player"}},{"op":"appraise_action","args":{"actor":"player","target":"baker","action":"broken_promise"}},{"op":"assess","args":{"subject":"baker"}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_rel_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = rel_mgr.workflow(&v);
        }
    }

    let mut clean_rel = RelationshipManager::new();
    clean_rel.workflow(&parse(r#"{"ops":[{"op":"init","args":{}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut rel_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            rel_ops_64.push(',');
        }
        rel_ops_64.push_str(r#"{"op":"assess","args":{"subject":"baker"}}"#);
    }
    rel_ops_64.push_str("]}");
    assert!(clean_rel.workflow(&parse(&rel_ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_rel2 = RelationshipManager::new();
    clean_rel2.workflow(&parse(r#"{"ops":[{"op":"init","args":{}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut rel_ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            rel_ops_65.push(',');
        }
        rel_ops_65.push_str(r#"{"op":"assess","args":{"subject":"baker"}}"#);
    }
    rel_ops_65.push_str("]}");
    assert!(clean_rel2.workflow(&parse(&rel_ops_65, 1 << 20).unwrap()).is_err());

    // 14. OffscreenResolver mutation fuzzing & boundaries
    use gamectx::character::offscreen::OffscreenResolver;
    let mut off_res = OffscreenResolver::new();
    let sample_off_req = r#"{"ops":[{"op":"init","args":{"clear_records":true}},{"op":"resolve_window","args":{"being":"baker","activity":"baking","action":"cook","observed":false,"at":10,"place":"kitchen","fatigue":5,"practice":10,"inputs":[{"count":1,"item":"flour"}],"outputs":[{"count":1,"item":"bread"}]}},{"op":"batch_resolve","args":{"activities":[{"being":"baker","activity":"baking","action":"cook","observed":false}]}},{"op":"assess","args":{"being":"baker"}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_off_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = off_res.workflow(&v);
        }
    }

    let mut clean_off = OffscreenResolver::new();
    let mut off_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            off_ops_64.push(',');
        }
        off_ops_64.push_str(r#"{"op":"assess","args":{}}"#);
    }
    off_ops_64.push_str("]}");
    assert!(clean_off.workflow(&parse(&off_ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_off2 = OffscreenResolver::new();
    let mut off_ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            off_ops_65.push(',');
        }
        off_ops_65.push_str(r#"{"op":"assess","args":{}}"#);
    }
    off_ops_65.push_str("]}");
    assert!(clean_off2.workflow(&parse(&off_ops_65, 1 << 20).unwrap()).is_err());

    // 15. CuriosityManager mutation fuzzing & boundaries
    use gamectx::character::curiosity::CuriosityManager;
    let mut cur_mgr = CuriosityManager::new();
    let sample_cur_req = r#"{"ops":[{"op":"init","args":{"being":"baker","temperament":"curious","known_people":["alice"],"known_things":["oven"],"known_places":["bakery"],"known_facts":["recipe"]}},{"op":"evaluate_novelty","args":{"being":"baker","people":["bob"],"things":["gem"],"places":["ruins"],"facts":["rumor"]}},{"op":"generate_candidates","args":{"being":"baker","people":["bob"],"limit":4,"auto_mark":true}},{"op":"mark_familiar","args":{"being":"baker","places":["ruins"]}},{"op":"assess","args":{"being":"baker"}}]}"#;
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_cur_req);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = cur_mgr.workflow(&v);
        }
    }

    let mut clean_cur = CuriosityManager::new();
    clean_cur.workflow(&parse(r#"{"ops":[{"op":"init","args":{"being":"baker"}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut cur_ops_64 = String::from(r#"{"ops":["#);
    for i in 0..64 {
        if i > 0 {
            cur_ops_64.push(',');
        }
        cur_ops_64.push_str(r#"{"op":"assess","args":{"being":"baker"}}"#);
    }
    cur_ops_64.push_str("]}");
    assert!(clean_cur.workflow(&parse(&cur_ops_64, 1 << 20).unwrap()).is_ok());

    let mut clean_cur2 = CuriosityManager::new();
    clean_cur2.workflow(&parse(r#"{"ops":[{"op":"init","args":{"being":"baker"}}]}"#, 1 << 20).unwrap()).unwrap();
    let mut cur_ops_65 = String::from(r#"{"ops":["#);
    for i in 0..65 {
        if i > 0 {
            cur_ops_65.push(',');
        }
        cur_ops_65.push_str(r#"{"op":"assess","args":{"being":"baker"}}"#);
    }
    cur_ops_65.push_str("]}");
    assert!(clean_cur2.workflow(&parse(&cur_ops_65, 1 << 20).unwrap()).is_err());
}

#[test]
fn explain_and_stability_survive_mutation_and_boundaries() {
    use gamectx::character::explain::ExplainTracer;
    use gamectx::character::stability::StabilityMonitor;
    let mut rng = Rng(0xD4D1_0E8A_1A57_2026);

    fn ops(n: usize, one: &str) -> String {
        format!(r#"{{"ops":[{}]}}"#, vec![one; n].join(","))
    }

    // 16. ExplainTracer mutation fuzzing & boundaries
    let sample_ex = r#"{"ops":[{"op":"record_trace","args":{"being":"baker","action":"barter","target":"miller","at":9,"perceptions":[{"source":"pantry","text":"the flour bin is empty"}],"beliefs":[{"about":"miller","fact":"the mill sells flour","source":"told","confidence":70}],"motive":{"id":"task_bake","kind":"task","score":70,"reason":"bread is promised","rank":1,"runner_up":{"id":"routine_bake","score":50}},"task":{"id":"bake_task","step":"barter_flour","index":1},"outcome":{"status":"pending"}}},{"op":"confirm_outcome","args":{"being":"baker","id":"tr_1","status":"success","summary":"got flour"}},{"op":"query_trace","args":{"being":"baker","action":"barter","limit":4}},{"op":"init","args":{"being":"baker"}}]}"#;
    let mut tracer = ExplainTracer::new();
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_ex);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = tracer.workflow(&v);
        }
    }
    let rec = r#"{"op":"record_trace","args":{"being":"baker","action":"bake","motive":{"id":"routine_bake","kind":"routine","score":50,"reason":"shift"}}}"#;
    assert!(ExplainTracer::new().workflow(&parse(&ops(64, rec), 1 << 20).unwrap()).is_ok());
    assert!(ExplainTracer::new().workflow(&parse(&ops(65, rec), 1 << 20).unwrap()).is_err());
    let mut ring = ExplainTracer::new();
    for _ in 0..2 {
        ring.workflow(&parse(&ops(64, rec), 1 << 20).unwrap()).unwrap();
    }
    let full = ring.workflow(&parse(&ops(1, rec), 1 << 20).unwrap()).unwrap().to_json();
    assert!(full.contains(r#""evicted":1"#) && full.contains(r#""stored":128"#), "{full}");

    // 17. StabilityMonitor mutation fuzzing & boundaries
    let sample_st = r#"{"ops":[{"op":"init","args":{"window":4,"dither_switches":2,"herd_min":2,"herd_pct":60,"spiral_turns":1}},{"op":"audit_turn","args":{"turn":1,"beings":[{"being":"a","motive":"m1","activity":"x","score":50,"runner_up":45,"fatigue":90,"needs":[{"id":"hunger","level":85}],"recovering":false,"holds":["oven"],"wants":["flour"]},{"being":"b","motive":"m2","activity":"x","holds":["flour"],"wants":["oven"]}],"world":{"grudge_total":120,"rumor_total":5}}},{"op":"assess_health","args":{}},{"op":"get_warnings","args":{"kind":"deadlock","being":"a","since_turn":0,"limit":4}}]}"#;
    let mut mon = StabilityMonitor::new();
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_st);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = mon.workflow(&v);
        }
    }
    let health = r#"{"op":"assess_health"}"#;
    assert!(StabilityMonitor::new().workflow(&parse(&ops(64, health), 1 << 20).unwrap()).is_ok());
    assert!(StabilityMonitor::new().workflow(&parse(&ops(65, health), 1 << 20).unwrap()).is_err());
    let audit = |n: usize| {
        let beings: Vec<String> =
            (0..n).map(|i| format!(r#"{{"being":"b{i}","motive":"m","activity":"a{i}"}}"#)).collect();
        format!(r#"{{"ops":[{{"op":"audit_turn","args":{{"turn":1,"beings":[{}]}}}}]}}"#, beings.join(","))
    };
    assert!(StabilityMonitor::new().workflow(&parse(&audit(64), 1 << 20).unwrap()).is_ok());
    assert!(StabilityMonitor::new().workflow(&parse(&audit(65), 1 << 20).unwrap()).is_err());
}

#[test]
fn beliefs_and_speech_acts_survive_mutation_and_boundaries() {
    use gamectx::character::beliefs::BeliefStore;
    use gamectx::character::speech_act::SpeechActs;
    let mut rng = Rng(0xA1B1_5EED_0BE1_1EF5);

    fn ops(n: usize, one: &str) -> String {
        format!(r#"{{"ops":[{}]}}"#, vec![one; n].join(","))
    }

    // 18. BeliefStore mutation fuzzing & boundaries
    let sample_bl = r#"{"ops":[{"op":"init","args":{"being":"baker","beliefs":[{"about":"baker","relation":"can","value":"bake"},{"about":"well","relation":"has","value":"water","holds":false,"source":"saw","confidence":70}]}},{"op":"learn","args":{"being":"baker","about":"mill","relation":"sells","value":"flour","source":"told","from":"carter","at":4,"holds":true,"confidence":60}},{"op":"outcome","args":{"being":"baker","action":"barter","success":false,"at":9,"relied_on":[{"about":"mill","relation":"sells","value":"flour"}]}},{"op":"query","args":{"being":"baker","about":"mill","min_confidence":10}},{"op":"known_actions","args":{"being":"baker","actions":["bake","mill"]}},{"op":"gaps","args":{"being":"baker","needs":[{"about":"mill","relation":"sells","value":"flour"}],"min_confidence":50}}]}"#;
    let mut store = BeliefStore::new();
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_bl);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = store.workflow(&v);
        }
    }
    let q = r#"{"op":"query","args":{"being":"baker"}}"#;
    assert!(BeliefStore::new().workflow(&parse(&ops(64, q), 1 << 20).unwrap()).is_ok());
    assert!(BeliefStore::new().workflow(&parse(&ops(65, q), 1 << 20).unwrap()).is_err());
    // 128 beliefs fill a being; the 129th evicts the weakest instead of growing.
    let mut full = BeliefStore::new();
    for chunk in 0..3 {
        let learns: Vec<String> = (0..64)
            .map(|i| format!(r#"{{"op":"learn","args":{{"being":"b","about":"x","relation":"r","value":"v{}","source":"told"}}}}"#, chunk * 64 + i))
            .collect();
        full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, learns.join(",")), 1 << 20).unwrap()).unwrap();
    }
    assert_eq!(full.beings["b"].len(), 128);

    // 19. SpeechActs mutation fuzzing & boundaries
    let sample_sp = r#"{"ops":[{"op":"learn","args":{"being":"miller","about":"mill","relation":"sells","value":"flour","source":"did"}},{"op":"relate","args":{"speaker":"miller","listener":"baker","trust":70,"authority":10}},{"op":"perform","args":{"speaker":"miller","listener":"baker","act":"warn","topic":{"about":"mill","relation":"sells","value":"flour"},"at":3}},{"op":"perform","args":{"speaker":"baker","listener":"miller","act":"threaten","goal":"lower_price","strength":80}},{"op":"check","args":{"speaker":"baker","listener":"miller","act":"comfort"}},{"op":"plan","args":{"speaker":"baker","intent":"get_help","goal":"carry","candidates":["miller","carter"],"strength":40}},{"op":"assess","args":{"being":"miller"}}]}"#;
    let mut acts = SpeechActs::new();
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample_sp);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = acts.workflow(&v);
        }
    }
    let a = r#"{"op":"assess","args":{"being":"baker"}}"#;
    assert!(SpeechActs::new().workflow(&parse(&ops(64, a), 1 << 20).unwrap()).is_ok());
    assert!(SpeechActs::new().workflow(&parse(&ops(65, a), 1 << 20).unwrap()).is_err());
    let plan = |n: usize| {
        let c: Vec<String> = (0..n).map(|i| format!(r#""c{i}""#)).collect();
        format!(
            r#"{{"ops":[{{"op":"plan","args":{{"speaker":"s","intent":"reassure","candidates":[{}]}}}}]}}"#,
            c.join(",")
        )
    };
    assert!(SpeechActs::new().workflow(&parse(&plan(16), 1 << 20).unwrap()).is_ok());
    assert!(SpeechActs::new().workflow(&parse(&plan(17), 1 << 20).unwrap()).is_err());
}

#[test]
fn diet_survives_mutation_and_boundaries() {
    use gamectx::character::diet::DietManager;
    let mut rng = Rng(0xD1E7_F00D_5A1D_2026);

    fn ops(n: usize, one: &str) -> String {
        format!(r#"{{"ops":[{}]}}"#, vec![one; n].join(","))
    }

    // 20. DietManager mutation fuzzing & boundaries
    let sample = r#"{"ops":[{"op":"register_food","args":{"pack":"village","foods":[{"id":"smoked_eel","label":"smoked eel","tags":["fish"],"hunger":28,"thirst":2,"joy":3,"spoils_after":30,"preserved_spoils_after":400,"raw_risk":20,"cooked_bonus":10}]}},{"op":"set_diet","args":{"being":"ferrin","diet":"omnivore","forbids":["alcohol"],"allergies":["nut"],"likes":["fish"],"dislikes":["egg"]}},{"op":"evaluate","args":{"being":"ferrin","food":"smoked_eel","cooked":true,"preserved":true,"made_at":5,"at":90}},{"op":"choose","args":{"being":"ferrin","hunger_level":70,"options":[{"food":"honey_cake"},{"food":"river_trout","cooked":true}]}},{"op":"eat","args":{"being":"ferrin","food":"oat_porridge","made_at":0,"at":60,"force":true}},{"op":"assess","args":{"being":"ferrin"}}]}"#;
    let mut diet = DietManager::new();
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = diet.workflow(&v);
        }
    }
    let reg = r#"{"op":"register_food","args":{"pack":"village"}}"#;
    assert!(DietManager::new().workflow(&parse(&ops(64, reg), 1 << 20).unwrap()).is_ok());
    assert!(DietManager::new().workflow(&parse(&ops(65, reg), 1 << 20).unwrap()).is_err());
    let choose = |n: usize| {
        let o: Vec<&str> = vec![r#"{"food":"rye_bread"}"#; n];
        format!(r#"{{"ops":[{reg},{{"op":"choose","args":{{"being":"b","options":[{}]}}}}]}}"#, o.join(","))
    };
    assert!(DietManager::new().workflow(&parse(&choose(16), 1 << 20).unwrap()).is_ok());
    assert!(DietManager::new().workflow(&parse(&choose(17), 1 << 20).unwrap()).is_err());
    // 256 foods fill the registry; the 257th is refused.
    let mut full = DietManager::new();
    for chunk in 0..4 {
        let foods: Vec<String> =
            (0..64).map(|i| format!(r#"{{"id":"f{}","tags":["grain"],"hunger":10}}"#, chunk * 64 + i)).collect();
        let req = format!(r#"{{"ops":[{{"op":"register_food","args":{{"foods":[{}]}}}}]}}"#, foods.join(","));
        full.workflow(&parse(&req, 1 << 20).unwrap()).unwrap();
    }
    let extra = r#"{"ops":[{"op":"register_food","args":{"foods":[{"id":"one_more","tags":["grain"],"hunger":1}]}}]}"#;
    assert!(full.workflow(&parse(extra, 1 << 20).unwrap()).is_err());
}

#[test]
fn creed_survives_mutation_and_boundaries() {
    use gamectx::character::creed::CreedManager;
    let mut rng = Rng(0xC7EE_D5A1_2026_0B5E);

    fn ops(n: usize, one: &str) -> String {
        format!(r#"{{"ops":[{}]}}"#, vec![one; n].join(","))
    }

    // 21. CreedManager mutation fuzzing & boundaries
    let sample = r#"{"ops":[{"op":"register_creed","args":{"creed":{"id":"hearthkeepers","label":"Keepers of the Hearth","kind":"religion","tenets":[{"id":"no_theft","action":"steal","severity":80,"label":"Take nothing"},{"id":"no_ash","action":"scatter","sacred_only":true,"severity":60,"label":"Keep the ash"},{"id":"no_flesh","food_tag":"meat","severity":40,"label":"No flesh"}],"observances":[{"id":"tend_flame","action":"tend_hearth","every":24,"window":4,"offset":6,"severity":50,"label":"Tend the flame"}],"sacred":["great_hearth"],"rivals":[{"creed":"free_tillers","stance":"wary"}]}}},{"op":"adhere","args":{"being":"baker","creed":"hearthkeepers","devotion":90,"at":2}},{"op":"adhere","args":{"being":"miller","creed":"hearthkeepers"}},{"op":"set_norms","args":{"being":"miller","norms":[{"id":"no_lies","action":"lie","severity":40,"label":"Speak plainly"}]}},{"op":"judge","args":{"actor":"baker","action":"steal","target":"great_hearth","witnesses":["miller"],"food_tags":["meat"],"at":9}},{"op":"observe","args":{"being":"baker","observance":"tend_flame","at":7}},{"op":"due","args":{"at":40,"being":"baker"}},{"op":"food_taboos","args":{"being":"baker"}},{"op":"regard","args":{"being":"baker","other":"miller"}},{"op":"assess","args":{"being":"baker"}}]}"#;
    let mut creeds = CreedManager::new();
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = creeds.workflow(&v);
        }
    }
    let reg =
        r#"{"op":"register_creed","args":{"creed":{"id":"stillwater","label":"Stillwater","kind":"philosophy"}}}"#;
    assert!(CreedManager::new().workflow(&parse(&ops(64, reg), 1 << 20).unwrap()).is_ok());
    assert!(CreedManager::new().workflow(&parse(&ops(65, reg), 1 << 20).unwrap()).is_err());
    let judge = |n: usize| {
        let w: Vec<String> = (0..n).map(|i| format!(r#""w{i}""#)).collect();
        format!(
            r#"{{"ops":[{reg},{{"op":"judge","args":{{"actor":"a","action":"boast","witnesses":[{}]}}}}]}}"#,
            w.join(",")
        )
    };
    assert!(CreedManager::new().workflow(&parse(&judge(16), 1 << 20).unwrap()).is_ok());
    assert!(CreedManager::new().workflow(&parse(&judge(17), 1 << 20).unwrap()).is_err());
    // 32 creeds fill the registry; the 33rd is refused, re-registering one is not.
    let creed = |i: usize| {
        format!(r#"{{"op":"register_creed","args":{{"creed":{{"id":"c{i}","label":"C","kind":"tradition"}}}}}}"#)
    };
    let all: Vec<String> = (0..32).map(creed).collect();
    let mut full = CreedManager::new();
    full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, all.join(",")), 1 << 20).unwrap()).unwrap();
    assert!(full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, creed(32)), 1 << 20).unwrap()).is_err());
    assert!(full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, creed(5)), 1 << 20).unwrap()).is_ok());
    // 128 beings fill adherence; the 129th is refused.
    let norms = |b: usize| format!(r#"{{"op":"set_norms","args":{{"being":"b{b}","norms":[]}}}}"#);
    for chunk in 0..2 {
        let batch: Vec<String> = (0..64).map(|i| norms(chunk * 64 + i)).collect();
        full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, batch.join(",")), 1 << 20).unwrap()).unwrap();
    }
    assert!(full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, norms(128)), 1 << 20).unwrap()).is_err());
}

#[test]
fn expectation_and_attention_survive_mutation_and_boundaries() {
    use gamectx::character::attention::AttentionModel;
    use gamectx::character::expectation::ExpectationBook;
    let mut rng = Rng(0xE7EC_7A77_E471_2026);

    fn ops(n: usize, one: &str) -> String {
        format!(r#"{{"ops":[{}]}}"#, vec![one; n].join(","))
    }

    // 22. ExpectationBook mutation fuzzing & boundaries
    let sample = r#"{"ops":[{"op":"expect","args":{"being":"baker","id":"flour","about":"player","event":"bring_flour","due":12,"grace":2,"importance":80,"relies":true,"label":"flour by noon"}},{"op":"expect","args":{"being":"miller","id":"tax","about":"collector","event":"arrive","due":5,"importance":60,"kind":"dread","confidence":90}},{"op":"tick","args":{"at":13}},{"op":"tick","args":{"at":15,"being":"baker"}},{"op":"happen","args":{"about":"player","event":"bring_flour","at":20}},{"op":"happen","args":{"about":"collector","event":"arrive","at":4,"being":"miller"}},{"op":"cancel","args":{"being":"baker","id":"flour"}},{"op":"assess","args":{"being":"baker"}}]}"#;
    let mut book = ExpectationBook::new();
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = book.workflow(&v);
        }
    }
    let tick = r#"{"op":"cancel","args":{"being":"b","id":"x"}}"#;
    assert!(ExpectationBook::new().workflow(&parse(&ops(64, tick), 1 << 20).unwrap()).is_ok());
    assert!(ExpectationBook::new().workflow(&parse(&ops(65, tick), 1 << 20).unwrap()).is_err());
    let expect = |b: &str, id: &str, about: &str, due: u64| {
        format!(
            r#"{{"op":"expect","args":{{"being":"{b}","id":"{id}","about":"{about}","event":"e","due":{due},"importance":10}}}}"#
        )
    };
    // 64 beings fill the book; the 65th is refused.
    let mut full = ExpectationBook::new();
    let beings: Vec<String> = (0..64).map(|i| expect(&format!("b{i}"), "x", "s", 0)).collect();
    full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, beings.join(",")), 1 << 20).unwrap()).unwrap();
    assert!(full
        .workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, expect("b64", "x", "s", 0)), 1 << 20).unwrap())
        .is_err());
    // 64 subjects of reliability; the 65th forgets the least-observed (ties: smallest name).
    let mut rel = ExpectationBook::new();
    for i in 0..65 {
        let about = format!("s{i:02}");
        let mut req = vec![expect("b", "x", &about, 0), r#"{"op":"tick","args":{"at":1}}"#.to_string()];
        if i == 0 {
            // s00 is observed twice, so it survives the eviction.
            req.push(expect("b", "y", "s00", 1));
            req.push(r#"{"op":"tick","args":{"at":2}}"#.to_string());
        }
        rel.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, req.join(",")), 1 << 20).unwrap()).unwrap();
    }
    let r = &rel.beings["b"].reliability;
    assert_eq!(r.len(), 64);
    assert!(r.contains_key("s00") && !r.contains_key("s01") && r.contains_key("s64"));

    // 23. AttentionModel mutation fuzzing & boundaries
    let sample = r#"{"ops":[{"op":"profile","args":{"being":"guard","alertness":80,"perception":30,"cares":["theft","player"],"capacity":2}},{"op":"notice","args":{"seed":42,"at":7,"events":[{"id":"crash","kind":"accident","place":"road","salience":90,"noise":80,"subject":"carter"},{"id":"lift","kind":"theft","place":"market","salience":20,"subject":"player"}],"beings":[{"id":"guard","place":"market","near":[{"place":"road","distance":30}],"light":60,"busy":20},{"id":"sleeper","place":"road","asleep":true,"sight":true,"hearing":true}]}},{"op":"assess","args":{"being":"guard"}}]}"#;
    let mut model = AttentionModel::new();
    for _ in 0..1000 {
        let mutated = mutate(&mut rng, sample);
        if let Ok(v) = parse(&mutated, 1 << 20) {
            let _ = model.workflow(&v);
        }
    }
    let assess = r#"{"op":"assess","args":{"being":"b"}}"#;
    assert!(AttentionModel::new().workflow(&parse(&ops(64, assess), 1 << 20).unwrap()).is_ok());
    assert!(AttentionModel::new().workflow(&parse(&ops(65, assess), 1 << 20).unwrap()).is_err());
    let notice = |events: usize, beings: usize| {
        let e: Vec<String> =
            (0..events).map(|i| format!(r#"{{"id":"e{i}","kind":"k","place":"p","salience":50}}"#)).collect();
        let b: Vec<String> = (0..beings).map(|i| format!(r#"{{"id":"b{i}","place":"p"}}"#)).collect();
        format!(
            r#"{{"ops":[{{"op":"notice","args":{{"seed":1,"events":[{}],"beings":[{}]}}}}]}}"#,
            e.join(","),
            b.join(",")
        )
    };
    assert!(AttentionModel::new().workflow(&parse(&notice(16, 64), 1 << 20).unwrap()).is_ok());
    assert!(AttentionModel::new().workflow(&parse(&notice(17, 1), 1 << 20).unwrap()).is_err());
    assert!(AttentionModel::new().workflow(&parse(&notice(1, 65), 1 << 20).unwrap()).is_err());
    assert!(AttentionModel::new().workflow(&parse(&notice(0, 1), 1 << 20).unwrap()).is_err());
    // 64 profiles; the 65th is refused, re-profiling one is not.
    let profile = |i: usize| format!(r#"{{"op":"profile","args":{{"being":"p{i}"}}}}"#);
    let all: Vec<String> = (0..64).map(profile).collect();
    let mut full = AttentionModel::new();
    full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, all.join(",")), 1 << 20).unwrap()).unwrap();
    assert!(full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, profile(64)), 1 << 20).unwrap()).is_err());
    assert!(full.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, profile(3)), 1 << 20).unwrap()).is_ok());
}

#[test]
fn growth_modules_survive_mutation_limits_and_boundaries() {
    use gamectx::character::aspiration::Aspirations;
    use gamectx::character::habit::Habits;
    use gamectx::character::lesson::Lessons;
    let mut rng = Rng(0x6A0A_15AB_17E5_2026);

    fn ops(n: usize, one: &str) -> String {
        format!(r#"{{"ops":[{}]}}"#, vec![one; n].join(","))
    }
    fn run(f: &mut dyn FnMut(&Value) -> gamectx::Result<Value>, req: &str) -> bool {
        f(&parse(req, 1 << 20).unwrap()).is_ok()
    }

    // 24. Aspirations, Habits and Lessons mutation fuzzing (limits included)
    let samples = [
        r#"{"limits":{"beings":8,"pressures":4,"goals":2},"ops":[{"op":"experience","args":{"being":"baker","kind":"need","subject":"hunger","weight":60,"at":1}},{"op":"experience","args":{"being":"baker","kind":"need","subject":"hunger","weight":60,"at":9}},{"op":"propose","args":{"being":"baker","goal":{"id":"build_oven","label":"Build an oven","action":"build","target":"oven"},"priority":70,"cites":[{"kind":"need","subject":"hunger"}],"at":10}},{"op":"drop","args":{"being":"baker","goal":"stock_hunger"}},{"op":"goals","args":{"being":"baker"}},{"op":"assess","args":{"being":"baker"}}]}"#,
        r#"{"limits":{"beings":8,"entries":4,"slots":4},"ops":[{"op":"set_routine","args":{"being":"baker","entries":[{"id":"bake","slot":"dawn","action":"bake","strength":70}]}},{"op":"choose","args":{"being":"baker","slot":"dusk","action":"walk","at":1}},{"op":"choose","args":{"being":"baker","slot":"dawn","action":"nap","at":2}},{"op":"skip","args":{"being":"baker","entry":"bake","at":3,"reason":"tired"}},{"op":"routine","args":{"being":"baker","slot":"dawn"}},{"op":"assess","args":{"being":"baker"}}]}"#,
        r#"{"limits":{"beings":8,"topics":4,"episodes":4},"ops":[{"op":"outcome","args":{"being":"baker","about":"trader","activity":"deal","result":"bad","episode":"e1","at":1}},{"op":"outcome","args":{"being":"baker","about":"trader","activity":"deal","result":"good","episode":"e2","at":2}},{"op":"advice","args":{"being":"baker","about":"trader","activity":"deal"}},{"op":"forget","args":{"being":"baker","about":"trader","activity":"deal"}},{"op":"assess","args":{"being":"baker"}}]}"#,
    ];
    let mut asp = Aspirations::new();
    let mut hab = Habits::new();
    let mut les = Lessons::new();
    for _ in 0..1000 {
        for (i, s) in samples.iter().enumerate() {
            if let Ok(v) = parse(&mutate(&mut rng, s), 1 << 20) {
                let _ = match i {
                    0 => asp.workflow(&v),
                    1 => hab.workflow(&v),
                    _ => les.workflow(&v),
                };
            }
        }
    }

    // 64/65 ops for each kind.
    let goals = r#"{"op":"goals","args":{"being":"b"}}"#;
    let routine = r#"{"op":"routine","args":{"being":"b"}}"#;
    let advice = r#"{"op":"advice","args":{"being":"b"}}"#;
    assert!(run(&mut |v| Aspirations::new().workflow(v), &ops(64, goals)));
    assert!(!run(&mut |v| Aspirations::new().workflow(v), &ops(65, goals)));
    assert!(run(&mut |v| Habits::new().workflow(v), &ops(64, routine)));
    assert!(!run(&mut |v| Habits::new().workflow(v), &ops(65, routine)));
    assert!(run(&mut |v| Lessons::new().workflow(v), &ops(64, advice)));
    assert!(!run(&mut |v| Lessons::new().workflow(v), &ops(65, advice)));

    // Limits: each ceiling is accepted, ceiling + 1 and 0 are refused.
    for (lim, ok) in [
        (r#"{"beings":1000000,"pressures":4096,"goals":256}"#, true),
        (r#"{"beings":1000001}"#, false),
        (r#"{"goals":0}"#, false),
    ] {
        assert_eq!(Aspirations::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }
    for (lim, ok) in
        [(r#"{"entries":512,"slots":512}"#, true), (r#"{"entries":513}"#, false), (r#"{"other":1}"#, false)]
    {
        assert_eq!(Habits::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }
    for (lim, ok) in [(r#"{"topics":4096,"episodes":256}"#, true), (r#"{"episodes":257}"#, false)] {
        assert_eq!(Lessons::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // A host that needs a large cast: 5,000 beings across workflows with beings = 5000; the 5,001st is refused.
    let mut big = Lessons::with_limits(&parse(r#"{"beings":5000}"#, 64).unwrap()).unwrap();
    for chunk in 0..79 {
        let batch: Vec<String> = (0..64)
            .map(|i| chunk * 64 + i)
            .filter(|n| *n < 5000)
            .map(|n| format!(r#"{{"op":"outcome","args":{{"being":"b{n}","about":"t","activity":"a","result":"good","episode":"e"}}}}"#))
            .collect();
        if !batch.is_empty() {
            big.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, batch.join(",")), 1 << 20).unwrap()).unwrap();
        }
    }
    assert_eq!(big.beings.len(), 5000);
    let extra = r#"{"ops":[{"op":"outcome","args":{"being":"one_more","about":"t","activity":"a","result":"good","episode":"e"}}]}"#;
    assert!(big.workflow(&parse(extra, 4096).unwrap()).is_err());

    // Lowering a limit below current usage keeps state and blocks growth.
    let lower = r#"{"limits":{"beings":10},"ops":[{"op":"advice","args":{"being":"b0"}}]}"#;
    assert!(big.workflow(&parse(lower, 4096).unwrap()).is_ok());
    assert_eq!(big.beings.len(), 5000);
    assert!(big.workflow(&parse(extra, 4096).unwrap()).is_err());
}

#[test]
fn character_modules_take_host_limits_and_survive_mutation() {
    use gamectx::character::limits::Limit;
    use gamectx::character::{
        affect::AffectManager, attention::AttentionModel, beliefs::BeliefStore, body::BodyManager, creed::CreedManager,
        curiosity::CuriosityManager, deception::DeceptionLedger, diet::DietManager, expectation::ExpectationBook,
        explain::ExplainTracer, mind::MindManager, offscreen::OffscreenResolver, relationship::RelationshipManager,
        skills::SkillsManager, speech_act::SpeechActs, stability::StabilityMonitor, task::TaskManager,
    };
    type Run = fn(&Value, &Value) -> gamectx::Result<Value>;
    fn module(kind: &str) -> Option<(&'static [Limit], Run)> {
        macro_rules! m {
            ($m:ident, $t:ty) => {
                (gamectx::character::$m::LIMITS, |lim: &Value, req: &Value| <$t>::with_limits(lim)?.workflow(req))
            };
        }
        Some(match kind {
            "attention" => m!(attention, AttentionModel),
            "beliefs" => m!(beliefs, BeliefStore),
            "creed" => m!(creed, CreedManager),
            "diet" => m!(diet, DietManager),
            "expectation" => m!(expectation, ExpectationBook),
            "explain" => m!(explain, ExplainTracer),
            "speech_act" => m!(speech_act, SpeechActs),
            "stability" => m!(stability, StabilityMonitor),
            "affect" => m!(affect, AffectManager),
            "skills" => m!(skills, SkillsManager),
            "body" => m!(body, BodyManager),
            "mind" => m!(mind, MindManager),
            "deception" => m!(deception, DeceptionLedger),
            "relationship" => m!(relationship, RelationshipManager),
            "offscreen" => m!(offscreen, OffscreenResolver),
            "curiosity" => m!(curiosity, CuriosityManager),
            "task" => m!(task, TaskManager),
            _ => return None,
        })
    }
    let empty = parse("{}", 4).unwrap();

    // 25. Mutated limit-carrying workflows for all seventeen limited character modules never panic.
    let samples = [
        include_str!("../conformance/limits/lim_attention_one_profile/input.json"),
        include_str!("../conformance/limits/lim_beliefs_small_store/input.json"),
        include_str!("../conformance/limits/lim_creed_exact_fit/input.json"),
        include_str!("../conformance/limits/lim_diet_recent_two/input.json"),
        include_str!("../conformance/limits/lim_expectation_two_evicts_broken/input.json"),
        include_str!("../conformance/limits/lim_explain_two_traces/input.json"),
        include_str!("../conformance/limits/lim_speech_store_limits_reach_beliefs/input.json"),
        include_str!("../conformance/limits/lim_stability_two_warnings/input.json"),
        include_str!("../conformance/limits_character/lc_affect_echo_defaults/input.json"),
        include_str!("../conformance/limits_character/lc_skills_teach_within_room/input.json"),
        include_str!("../conformance/body/action_refused_when_exhausted/input.json"),
        include_str!("../conformance/limits_character/lc_mind_existing_fact_updates_at_limit/input.json"),
        include_str!("../conformance/limits_character/lc_deception_null_limits/input.json"),
        include_str!("../conformance/limits_character/lc_relationship_community_room/input.json"),
        include_str!("../conformance/limits_character/lc_offscreen_small_history/input.json"),
        include_str!("../conformance/limits_character/lc_curiosity_known_fills_sorted/input.json"),
        include_str!("../conformance/limits_character/lc_task_repropose_needs_no_room/input.json"),
    ];
    let mut rng = Rng(0x7A57_11E1_75A1_2026);
    for _ in 0..1000 {
        for s in samples {
            let Ok(v) = parse(&mutate(&mut rng, s), 1 << 20) else { continue };
            let (Some(kind), Some(args)) = (v.get("kind").and_then(Value::as_str), v.get("args")) else { continue };
            if let Some((_, run)) = module(kind) {
                let _ = run(&empty, args);
            }
        }
    }

    for s in samples {
        let v = parse(s, 1 << 20).unwrap();
        let kind = v.get("kind").and_then(Value::as_str).unwrap();
        let ops = v.get("args").and_then(|a| a.get("ops")).unwrap().to_json();
        let (spec, run) = module(kind).unwrap();
        let req = |lim: &str| parse(&format!(r#"{{"limits":{lim},"ops":{ops}}}"#), 1 << 20).unwrap();
        let ok = |lim: &str| run(&empty, &req(lim)).is_ok();
        // Every ceiling at once is accepted and echoed; ceiling + 1, zero and unknown names are refused.
        let all: Vec<String> = spec.iter().map(|l| format!(r#""{}":{}"#, l.name, l.ceiling)).collect();
        let out = run(&empty, &req(&format!("{{{}}}", all.join(",")))).unwrap().to_json();
        assert!(out.starts_with(&format!(r#"{{"limits":{{{}}}"#, all.join(","))), "{kind}: {out}");
        for l in spec {
            assert!(l.default >= 1 && l.default <= l.ceiling, "{kind}.{}", l.name);
            assert!(!ok(&format!(r#"{{"{}":{}}}"#, l.name, l.ceiling + 1)), "{kind}.{}", l.name);
            assert!(!ok(&format!(r#"{{"{}":0}}"#, l.name)), "{kind}.{}", l.name);
            let one = parse(&format!(r#"{{"{}":{}}}"#, l.name, l.ceiling + 1), 64).unwrap();
            assert!(run(&one, &req("null")).is_err(), "{kind}.{}", l.name);
        }
        assert!(!ok(r#"{"not_a_limit":1}"#), "{kind}");
        assert!(!ok("[1]"), "{kind}");
        // Without limits nothing is echoed; null limits are accepted and echo the defaults.
        let plain = parse(&format!(r#"{{"ops":{ops}}}"#), 1 << 20).unwrap();
        assert!(!run(&empty, &plain).expect(kind).to_json().contains(r#""limits""#), "{kind}");
        let defaults: Vec<String> = spec.iter().map(|l| format!(r#""{}":{}"#, l.name, l.default)).collect();
        assert!(
            run(&empty, &req("null"))
                .unwrap()
                .to_json()
                .starts_with(&format!(r#"{{"limits":{{{}}}"#, defaults.join(","))),
            "{kind}"
        );
    }

    // Lowering a limit below current usage keeps state and only blocks growth.
    let mut diet = DietManager::with_limits(&parse(r#"{"beings":3000}"#, 64).unwrap()).unwrap();
    for chunk in 0..47 {
        let batch: Vec<String> = (chunk * 64..(chunk + 1) * 64)
            .filter(|n| *n < 3000)
            .map(|n| format!(r#"{{"op":"set_diet","args":{{"being":"b{n}"}}}}"#))
            .collect();
        diet.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, batch.join(",")), 1 << 20).unwrap()).unwrap();
    }
    let extra = r#"{"ops":[{"op":"set_diet","args":{"being":"one_more"}}]}"#;
    assert!(diet.workflow(&parse(extra, 4096).unwrap()).is_err());
    let lower = r#"{"limits":{"beings":10},"ops":[{"op":"set_diet","args":{"being":"b0"}}]}"#;
    assert!(diet.workflow(&parse(lower, 4096).unwrap()).is_ok());
    assert!(diet.workflow(&parse(extra, 4096).unwrap()).is_err());

    // A pass 1 module at a large cast: 3,000 bodies, created on demand by `tick`.
    let mut body = BodyManager::with_limits(&parse(r#"{"beings":3000}"#, 64).unwrap()).unwrap();
    for chunk in 0..47 {
        let batch: Vec<String> = (chunk * 64..(chunk + 1) * 64)
            .filter(|n| *n < 3000)
            .map(|n| format!(r#"{{"op":"tick","args":{{"being":"b{n}","ticks":1}}}}"#))
            .collect();
        body.workflow(&parse(&format!(r#"{{"ops":[{}]}}"#, batch.join(",")), 1 << 20).unwrap()).unwrap();
    }
    let extra = r#"{"ops":[{"op":"tick","args":{"being":"one_more","ticks":1}}]}"#;
    assert!(body.workflow(&parse(extra, 4096).unwrap()).is_err());
    let lower = r#"{"limits":{"beings":10},"ops":[{"op":"assess","args":{"being":"b2999"}}]}"#;
    assert!(body.workflow(&parse(lower, 4096).unwrap()).is_ok(), "lowering keeps existing bodies");
}

#[test]
fn knowledge_modules_take_host_limits_and_survive_mutation() {
    use gamectx::character::limits::Limit;
    use gamectx::knowledge::{
        action_catalog, codex, communities, info, inventory, lore, recipes, selection, species, things, timeline,
    };
    fn spec(kind: &str) -> &'static [Limit] {
        match kind {
            "places" => codex::LIMITS,
            "timeline" => timeline::LIMITS,
            "things" => things::LIMITS,
            "species" => species::LIMITS,
            "recipes" => recipes::LIMITS,
            "communities" => communities::LIMITS,
            "actions" => action_catalog::LIMITS,
            "lore" => lore::LIMITS,
            "inventory" => inventory::LIMITS,
            "evidence" => selection::LIMITS,
            _ => info::LIMITS,
        }
    }
    let run = |kind: &str, args: &Value| {
        gamectx::kit::local::process(&Value::Object(vec![
            ("kind".into(), Value::str(kind)),
            ("args".into(), args.clone()),
        ]))
    };
    let samples = [
        include_str!("../conformance/limits_knowledge/lk_places_echo_defaults/input.json"),
        include_str!("../conformance/limits_knowledge/lk_timeline_entries_limit/input.json"),
        include_str!("../conformance/limits_knowledge/lk_things_things_limit/input.json"),
        include_str!("../conformance/limits_knowledge/lk_species_animals_over_limit/input.json"),
        include_str!("../conformance/limits_knowledge/lk_recipes_echo_low_limit/input.json"),
        include_str!("../conformance/limits_knowledge/lk_communities_members_limit/input.json"),
        include_str!("../conformance/limits_knowledge/lk_actions_register_within_limit/input.json"),
        include_str!("../conformance/limits_knowledge/lk_inventory_holdings_full/input.json"),
        include_str!("../conformance/limits_knowledge/lk_evidence_above_ceiling/input.json"),
        include_str!("../conformance/limits_knowledge/lk_info_notes_limit/input.json"),
    ];
    // Mutated limit-carrying knowledge workflows never panic.
    let mut rng = Rng(0x0C0D_E7C5_2026_1108);
    for _ in 0..300 {
        for s in samples {
            if let Ok(v) = parse(&mutate(&mut rng, s), 1 << 22) {
                let _ = gamectx::kit::local::process(&v);
            }
        }
    }
    let lore_sample = include_str!("../conformance/limits_knowledge/lk_lore_many_entries/input.json");
    for s in samples.iter().chain([&lore_sample]) {
        let v = parse(s, 1 << 22).unwrap();
        let kind = v.get("kind").and_then(Value::as_str).unwrap();
        let Some(Value::Object(fields)) = v.get("args") else { panic!("{kind}") };
        let body: Vec<(String, Value)> = fields.iter().filter(|(k, _)| k != "limits").cloned().collect();
        let with = |lim: &str| {
            let mut f = vec![("limits".to_string(), parse(lim, 4096).unwrap())];
            f.extend(body.clone());
            run(kind, &Value::Object(f))
        };
        let spec = spec(kind);
        // Every ceiling at once is accepted and echoed; ceiling + 1, zero, unknown names and
        // non-objects are refused.
        let all: Vec<String> = spec.iter().map(|l| format!(r#""{}":{}"#, l.name, l.ceiling)).collect();
        let lim_all = format!("{{{}}}", all.join(","));
        if let Ok(out) = with(&lim_all) {
            assert!(out.to_json().starts_with(&format!(r#"{{"limits":{lim_all}"#)), "{kind}");
        }
        for l in spec {
            assert!(l.default >= 1 && l.default <= l.ceiling, "{kind}.{}", l.name);
            assert!(with(&format!(r#"{{"{}":{}}}"#, l.name, l.ceiling + 1)).is_err(), "{kind}.{}", l.name);
            assert!(with(&format!(r#"{{"{}":0}}"#, l.name)).is_err(), "{kind}.{}", l.name);
        }
        assert!(with(r#"{"not_a_limit":1}"#).is_err(), "{kind}");
        assert!(with("[1]").is_err(), "{kind}");
        // Null limits echo the defaults; without limits nothing is echoed and the output is
        // exactly the null-limits output minus the echo.
        let defaults: Vec<String> = spec.iter().map(|l| format!(r#""{}":{}"#, l.name, l.default)).collect();
        let echo = format!(r#"{{"limits":{{{}}},"#, defaults.join(","));
        match (with("null"), run(kind, &Value::Object(body.clone()))) {
            (Ok(a), Ok(b)) => {
                let (a, b) = (a.to_json(), b.to_json());
                assert!(a.starts_with(&echo), "{kind}: {a}");
                assert_eq!(&a[echo.len()..], &b[1..], "{kind}");
            }
            (Err(_), Err(_)) => {}
            other => panic!("{kind}: null limits and no limits disagree: {other:?}"),
        }
    }

    // A long fact dependency chain at the raised capacity checks without exhausting the stack.
    use gamectx::knowledge::facts::{FactOptions, FactRegistry, MAX_CAPACITY};
    let mut facts = FactRegistry::new(MAX_CAPACITY).unwrap();
    assert!(FactRegistry::new(MAX_CAPACITY + 1).is_err());
    let mut last = None;
    for i in 0..512 {
        let options = FactOptions { now: 1, dependencies: last.into_iter().collect(), ..Default::default() };
        last = Some(facts.register("world", &format!("link_{i}"), i as u64, options).unwrap());
    }
    assert!(facts.get("world", "link_511", 1, &[]).unwrap().is_some());
    let first = facts.get("world", "link_0", 1, &[]).unwrap().unwrap().reference.clone();
    facts.forget(&first, gamectx::knowledge::facts::Authority::Host).unwrap();
    assert!(facts.get("world", "link_511", 1, &[]).unwrap().is_none(), "a broken basis hides its dependents");
    // A saved registry at full capacity whose facts form one chain: checking the newest link
    // walks all 4,096 links on the heap, not the thread stack.
    let n = MAX_CAPACITY;
    let rows: Vec<String> = (0..n)
        .map(|i| {
            let dep = if i == 0 {
                String::new()
            } else {
                format!(r#"{{"scope":"world","key":"link_{}","revision":{i}}}"#, i - 1)
            };
            format!(
                r#"{{"scope":"world","key":"link_{i}","revision":{},"value":{i},"authority":"host","pinned":false,"expires":null,"privacy":[],"dependencies":[{dep}]}}"#,
                i + 1
            )
        })
        .collect();
    let saved = format!(r#"{{"version":1,"capacity":{n},"generation":{n},"clock":1,"facts":[{}]}}"#, rows.join(","));
    let chain = FactRegistry::restore_json(&saved).unwrap();
    assert!(chain.get("world", &format!("link_{}", n - 1), 1, &[]).unwrap().is_some());
    let packet = chain.packet(1, 256, 16_384, &[]).unwrap();
    assert!(packet.get("rows").and_then(Value::as_array).is_some_and(|r| !r.is_empty()), "{}", packet.to_json());
}

#[test]
fn cooperation_survives_mutation_limits_and_boundaries() {
    use gamectx::character::cooperation::{Cooperation, M};
    let mut rng = Rng(0xC0_0BE7_2026_0B23);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 25. Cooperation mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"tasks":4,"roles":3,"debts":4},"ops":[{"op":"form","args":{"id":"harvest","organizer":"farmer","goal":"Bring in the barley","roles":[{"role":"reap","being":"farmer"},{"role":"cart"}]}},{"op":"ask_help","args":{"task":"harvest","role":"cart","asker":"farmer","helper":"carter","trust":60,"busy":false,"effort":20,"at":1}},{"op":"hand_off","args":{"task":"harvest","role":"cart","from":"carter","to":"cousin"}},{"op":"complete","args":{"task":"harvest","role":"cart","being":"cousin","at":4}},{"op":"leave","args":{"task":"harvest","role":"reap","being":"farmer"}},{"op":"assess","args":{"being":"farmer"}}]}"#,
        r#"{"limits":{"promises":4,"beings":4,"grudges":2},"ops":[{"op":"promise","args":{"id":"p","from":"smith","to":"miller","what":"mend_wheel","due":3,"at":1}},{"op":"keep","args":{"id":"p","at":2}},{"op":"promise","args":{"id":"q","from":"smith","to":"baker","what":"nails","due":3}},{"op":"release","args":{"id":"q","by":"baker"}},{"op":"tick","args":{"now":9}},{"op":"grudges","args":{"being":"baker"}}]}"#,
        r#"{"limits":{"resources":4,"queue":3},"ops":[{"op":"claim","args":{"resource":"well","being":"ann","urgency":40,"status":60,"at":1}},{"op":"claim","args":{"resource":"well","being":"bo","urgency":90}},{"op":"contest","args":{"resource":"well","challenger":"cy","trust":-20,"urgency":80,"status":10,"holder_urgency":30,"holder_status":90,"at":2}},{"op":"free","args":{"resource":"well","being":"ann","at":3}},{"op":"assess","args":{"being":"cy"}}]}"#,
    ];
    let mut co = Cooperation::new();
    for _ in 0..1500 {
        for s in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, s), 1 << 20) {
                let _ = co.workflow(&v);
            }
        }
    }
    for book in co.grudges.values() {
        assert!(book.values().all(|g| g.weight > 0 && g.weight <= M));
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let assess = r#"{"op":"assess","args":{"being":"b"}}"#.to_string();
    assert!(Cooperation::new().workflow(&req(&vec![assess.clone(); 64])).is_ok());
    assert!(Cooperation::new().workflow(&req(&vec![assess; 65])).is_err());
    for (lim, ok) in [
        (
            r#"{"tasks":1000000,"roles":64,"promises":1000000,"resources":1000000,"queue":1024,"beings":1000000,"grudges":4096,"debts":1000000}"#,
            true,
        ),
        (r#"{"roles":65}"#, false),
        (r#"{"queue":1025}"#, false),
        (r#"{"grudges":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Cooperation::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: a 64-role task, a 1,024-deep queue, and the clock at its bound.
    let mut co = Cooperation::with_limits(&parse(r#"{"roles":64,"queue":1024}"#, 64).unwrap()).unwrap();
    let roles: Vec<String> = (0..64).map(|i| format!(r#"{{"role":"r{i}"}}"#)).collect();
    let form = format!(
        r#"{{"op":"form","args":{{"id":"t","organizer":"o","goal":"Raise the barn","roles":[{}]}}}}"#,
        roles.join(",")
    );
    assert!(co.workflow(&req(&[form])).is_ok());
    let claim = |b: String| {
        format!(r#"{{"op":"claim","args":{{"resource":"well","being":"{b}","urgency":{}}}}}"#, b.len() % 101)
    };
    for chunk in 0..17 {
        let batch: Vec<String> =
            (chunk * 64..(chunk + 1) * 64).filter(|n| *n <= 1024).map(|n| claim(format!("b{n}"))).collect();
        assert!(co.workflow(&req(&batch)).is_ok());
    }
    assert_eq!(co.resources["well"].queue.len(), 1024);
    assert!(co.workflow(&req(&[claim("one_more".into())])).is_err());
    assert!(co.workflow(&req(&[r#"{"op":"tick","args":{"now":1000000000000}}"#.into()])).is_ok());
    assert!(co.workflow(&req(&[r#"{"op":"tick","args":{"now":1000000000001}}"#.into()])).is_err());

    // A refused grudge (beings full) changes nothing: the promise stays pending, the clock stays put.
    let mut co = Cooperation::with_limits(&parse(r#"{"beings":1}"#, 64).unwrap()).unwrap();
    let setup = [
        r#"{"op":"promise","args":{"id":"a","from":"s","to":"m","what":"x","due":0}}"#.to_string(),
        r#"{"op":"promise","args":{"id":"b","from":"s","to":"k","what":"y","due":0}}"#.to_string(),
    ];
    co.workflow(&req(&setup)).unwrap();
    let before = co.clone();
    assert!(co.workflow(&req(&[r#"{"op":"tick","args":{"now":5}}"#.into()])).is_err());
    assert_eq!(co, before);
}

#[test]
fn cognition_survives_mutation_limits_and_boundaries() {
    use gamectx::character::cognition::{Cognition, M};
    let mut rng = Rng(0xC061_7104_2026_0A78);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 26. Cognition mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"beings":4,"motives":3,"steps":4},"ops":[{"op":"profile","args":{"being":"tinker","temperament":"impulsive","plan_depth":3,"horizon":50,"impulsiveness":40,"memory_span":200,"slip":300000}},{"op":"decide","args":{"being":"tinker","seed":7,"at":5,"motives":[{"id":"eat","score":60},{"id":"work","score":70}],"steps":[{"id":"a"},{"id":"b","familiar":false,"eta":40},{"id":"c","eta":900}]}},{"op":"assess","args":{"being":"tinker"}}]}"#,
        r#"{"limits":{"memories":3,"people":2},"ops":[{"op":"profile","args":{"being":"poet","temperament":"dreamy"}},{"op":"remember","args":{"being":"poet","id":"fire","about":"barn","what":"burned","day":12,"people":["miller","smith"],"importance":10,"at":1}},{"op":"remember","args":{"being":"poet","id":"fair","about":"square","what":"dance","day":20,"people":["widow"],"importance":100}},{"op":"tick","args":{"now":200}},{"op":"recall","args":{"being":"poet","id":"fire","seed":9,"errors":true}},{"op":"forget","args":{"being":"poet","id":"fair"}}]}"#,
    ];
    let mut cog = Cognition::new();
    for _ in 0..1500 {
        for s in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, s), 1 << 20) {
                let _ = cog.workflow(&v);
            }
        }
    }
    for book in cog.memories.values() {
        assert!(book.values().all(|m| m.strength > 0 && m.strength <= M && m.importance <= 100));
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let assess = r#"{"op":"assess","args":{"being":"b"}}"#.to_string();
    assert!(Cognition::new().workflow(&req(&vec![assess.clone(); 64])).is_ok());
    assert!(Cognition::new().workflow(&req(&vec![assess; 65])).is_err());
    for (lim, ok) in [
        (r#"{"beings":1000000,"memories":65536,"people":64,"motives":256,"steps":256}"#, true),
        (r#"{"people":65}"#, false),
        (r#"{"steps":257}"#, false),
        (r#"{"memories":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Cognition::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: 256 steps and motives in one decision, the clock at its bound, the longest span.
    let mut cog = Cognition::with_limits(&parse(r#"{"motives":256,"steps":256}"#, 64).unwrap()).unwrap();
    let motives: Vec<String> = (0..256).map(|i| format!(r#"{{"id":"m{i}","score":{}}}"#, i % 101)).collect();
    let steps: Vec<String> = (0..256).map(|i| format!(r#"{{"id":"s{i}","familiar":false,"eta":{i}}}"#)).collect();
    let decide = format!(
        r#"{{"op":"decide","args":{{"being":"b","seed":1000000000000,"motives":[{}],"steps":[{}]}}}}"#,
        motives.join(","),
        steps.join(",")
    );
    assert!(cog.workflow(&req(&[decide])).is_ok());
    let setup = [
        r#"{"op":"profile","args":{"being":"b","memory_span":1000000}}"#.to_string(),
        r#"{"op":"remember","args":{"being":"b","id":"m","about":"x","what":"y","day":1000000000000,"importance":99}}"#
            .to_string(),
        r#"{"op":"tick","args":{"now":1000000000000}}"#.to_string(),
    ];
    assert!(cog.workflow(&req(&setup)).is_ok());
    assert!(cog.memories.is_empty(), "a full horizon of ticks forgets even importance 99");
    assert!(cog.workflow(&req(&[r#"{"op":"tick","args":{"now":1000000000001}}"#.into()])).is_err());

    // A refused op changes nothing: a fourth being over beings 3.
    let mut cog = Cognition::with_limits(&parse(r#"{"beings":3}"#, 64).unwrap()).unwrap();
    let three: Vec<String> = (0..3).map(|i| format!(r#"{{"op":"profile","args":{{"being":"b{i}"}}}}"#)).collect();
    cog.workflow(&req(&three)).unwrap();
    let before = cog.clone();
    let fourth = r#"{"op":"remember","args":{"being":"b3","id":"m","about":"x","what":"y","day":1}}"#.to_string();
    assert!(cog.workflow(&req(&[fourth])).is_err());
    assert_eq!(cog, before);
}

#[test]
fn chatter_survives_mutation_limits_and_boundaries() {
    use gamectx::character::chatter::Chatter;
    let mut rng = Rng(0xC4A7_7E5B_2026_0B06);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 27. Chatter mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"beings":4,"topics":3,"wants":2,"turns":5},"ops":[{"op":"know","args":{"being":"baker","topic":"mill_price","value":"rising","weight":80}},{"op":"know","args":{"being":"widow","topic":"fox_den","value":"north","weight":60,"secret":true}},{"op":"want","args":{"being":"baker","topic":"fox_den","weight":70}},{"op":"bond","args":{"a":"baker","b":"widow","warmth":65}},{"op":"meet","args":{"a":"baker","b":"widow","seed":3,"turns":4,"at":9}},{"op":"assess","args":{"being":"baker"}}]}"#,
        r#"{"limits":{"records":3,"bonds":2},"ops":[{"op":"know","args":{"being":"miller","topic":"bridge","value":"safe"}},{"op":"know","args":{"being":"carter","topic":"bridge","value":"broken"}},{"op":"meet","args":{"a":"miller","b":"carter","seed":2}},{"op":"meet","args":{"a":"carter","b":"miller","seed":5}},{"op":"log","args":{"being":"carter","limit":2}}]}"#,
    ];
    let mut chat = Chatter::new();
    for _ in 0..1500 {
        for s in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, s), 1 << 20) {
                let _ = chat.workflow(&v);
            }
        }
    }
    assert!(chat.bonds.values().all(|w| (-100..=100).contains(w)));
    assert!(chat.records.len() <= 256);

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let log = r#"{"op":"log","args":{}}"#.to_string();
    assert!(Chatter::new().workflow(&req(&vec![log.clone(); 64])).is_ok());
    assert!(Chatter::new().workflow(&req(&vec![log; 65])).is_err());
    for (lim, ok) in [
        (r#"{"beings":1000000,"topics":4096,"wants":256,"bonds":1000000,"records":1000000,"turns":32}"#, true),
        (r#"{"turns":33}"#, false),
        (r#"{"wants":257}"#, false),
        (r#"{"records":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Chatter::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: two beings holding 4,096 topics each meet for 32 turns.
    let mut chat = Chatter::with_limits(&parse(r#"{"topics":4096,"turns":32}"#, 64).unwrap()).unwrap();
    for b in ["ann", "bo"] {
        for chunk in 0..64 {
            let batch: Vec<String> = (chunk * 64..(chunk + 1) * 64)
                .map(|i| {
                    format!(
                        r#"{{"op":"know","args":{{"being":"{b}","topic":"{b}_t{i}","value":"v","weight":{}}}}}"#,
                        i % 101
                    )
                })
                .collect();
            chat.workflow(&req(&batch)).unwrap();
        }
    }
    let meet = r#"{"op":"meet","args":{"a":"ann","b":"bo","seed":1000000000000,"turns":32}}"#.to_string();
    assert!(chat.workflow(&req(&[meet])).is_ok(), "both full: tells are heard but not kept");
    assert_eq!(chat.minds["ann"].knows.len(), 4096);

    // A refused op changes nothing: a meeting that would add a bond past the bonds limit.
    let mut chat = Chatter::with_limits(&parse(r#"{"bonds":1}"#, 64).unwrap()).unwrap();
    let setup = [
        r#"{"op":"bond","args":{"a":"x","b":"y","warmth":0}}"#.to_string(),
        r#"{"op":"know","args":{"being":"m","topic":"bridge","value":"safe"}}"#.to_string(),
        r#"{"op":"know","args":{"being":"c","topic":"bridge","value":"broken"}}"#.to_string(),
    ];
    chat.workflow(&req(&setup)).unwrap();
    let before = chat.clone();
    assert!(chat.workflow(&req(&[r#"{"op":"meet","args":{"a":"m","b":"c","seed":2}}"#.into()])).is_err());
    assert_eq!(chat, before);
}

#[test]
fn survival_survives_mutation_limits_and_boundaries() {
    use gamectx::character::survival::Survival;
    let mut rng = Rng(0x5011_7A1C_2026_0C12);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 28. Survival mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"beings":8,"belongings":3,"children":2},"ops":[{"op":"born","args":{"being":"ma","lifespan":100,"at":0}},{"op":"born","args":{"being":"pa","lifespan":120,"at":0}},{"op":"age","args":{"now":30}},{"op":"pair","args":{"a":"ma","b":"pa"}},{"op":"born","args":{"being":"kid","lifespan":100,"parents":["ma","pa"],"at":30}},{"op":"own","args":{"being":"ma","thing":"loom"}},{"op":"age","args":{"now":100}},{"op":"assess","args":{"being":"kid"}}]}"#,
        r#"{"ops":[{"op":"born","args":{"being":"guard","lifespan":1000}},{"op":"threat","args":{"being":"guard","threat":"bear","danger":90,"temperament":"timid","strength":20,"threat_strength":90,"injury":30,"allies":3,"exit":false,"cover":true}},{"op":"die","args":{"being":"guard","cause":"bear","at":5}},{"op":"assess","args":{"being":"guard"}}]}"#,
    ];
    let mut s = Survival::new();
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                let _ = s.workflow(&v);
            }
        }
    }
    assert!(s.beings.values().all(|b| b.safety <= 100 && (b.alive || b.belongings.is_empty())));

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let age = r#"{"op":"age","args":{"now":0}}"#.to_string();
    assert!(Survival::new().workflow(&req(&vec![age.clone(); 64])).is_ok());
    assert!(Survival::new().workflow(&req(&vec![age; 65])).is_err());
    for (lim, ok) in [
        (r#"{"beings":1000000,"belongings":4096,"children":256}"#, true),
        (r#"{"children":257}"#, false),
        (r#"{"belongings":4097}"#, false),
        (r#"{"beings":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Survival::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: a parent with 256 children leaves 4,096 belongings; lifespans at the clock bound.
    let mut s = Survival::with_limits(&parse(r#"{"children":256,"belongings":4096}"#, 64).unwrap()).unwrap();
    s.workflow(&req(&[
        r#"{"op":"born","args":{"being":"elder","lifespan":1000,"at":0}}"#.into(),
        r#"{"op":"age","args":{"now":300}}"#.into(),
    ]))
    .unwrap();
    for chunk in 0..4 {
        let kids: Vec<String> = (chunk * 64..(chunk + 1) * 64)
            .map(|i| {
                format!(r#"{{"op":"born","args":{{"being":"k{i}","lifespan":1000000000000,"parents":["elder"]}}}}"#)
            })
            .collect();
        s.workflow(&req(&kids)).unwrap();
    }
    for chunk in 0..64 {
        let things: Vec<String> = (chunk * 64..(chunk + 1) * 64)
            .map(|i| format!(r#"{{"op":"own","args":{{"being":"elder","thing":"t{i}"}}}}"#))
            .collect();
        s.workflow(&req(&things)).unwrap();
    }
    s.workflow(&req(&[r#"{"op":"age","args":{"now":1000}}"#.into()])).unwrap();
    assert!(!s.beings["elder"].alive);
    assert_eq!(s.beings["k0"].belongings.len(), 16, "4,096 things round robin over 256 heirs");
    assert!(s.workflow(&req(&[r#"{"op":"age","args":{"now":1000000000000}}"#.into()])).is_ok());
    assert!(s.beings["k0"].alive && s.beings["k0"].stage == "elder", "born at 300, a lifespan of 10^12 has not ended");

    // A refused op changes nothing: a child as a parent.
    let mut s = Survival::new();
    s.workflow(&req(&[r#"{"op":"born","args":{"being":"kid","lifespan":1000}}"#.into()])).unwrap();
    let before = s.clone();
    assert!(s
        .workflow(&req(&[r#"{"op":"born","args":{"being":"baby","lifespan":1000,"parents":["kid"]}}"#.into()]))
        .is_err());
    assert_eq!(s, before);
}

#[test]
fn detail_survives_mutation_limits_and_boundaries() {
    use gamectx::character::detail::Detail;
    let mut rng = Rng(0xDE7A_11D2_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 29. Level-of-detail mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"beings":8,"needs":3,"groups":2},"ops":[{"op":"configure","args":{"near":5,"far":50,"stride":7}},{"op":"track","args":{"being":"a","distance":0,"needs":[{"id":"hunger","rate":10000,"level":400000,"threshold":700000}]}},{"op":"track","args":{"being":"b","distance":30,"needs":[{"id":"thirst","rate":25000}]}},{"op":"track","args":{"being":"c","distance":900,"group":"g","needs":[{"id":"sleep","rate":1000}]}},{"op":"tick","args":{"now":37}},{"op":"move","args":{"being":"c","distance":0}},{"op":"crowd","args":{"group":"g"}}]}"#,
        r#"{"ops":[{"op":"track","args":{"being":"f","distance":500,"group":"east","needs":[{"id":"n","rate":1000000,"level":999999}]}},{"op":"tick","args":{"now":1000000000000}},{"op":"assess","args":{"being":"f"}},{"op":"move","args":{"being":"f","distance":60}}]}"#,
    ];
    let mut d = Detail::new();
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                let _ = d.workflow(&v);
            }
        }
    }
    assert!(d.beings.values().all(|b| b.at <= d.clock && b.needs.values().all(|n| n.level <= 1_000_000)));
    assert!(d.beings.values().all(|b| (b.detail == "crowd") == b.group.is_some()));
    // The group and active indexes match the beings exactly.
    for (k, b) in &d.beings {
        assert_eq!(d.active.contains(k), b.group.is_none(), "{k}");
        assert!(b.group.as_ref().is_none_or(|g| d.groups[g].contains(k)), "{k}");
    }
    assert_eq!(d.groups.values().map(|m| m.len()).sum::<usize>() + d.active.len(), d.beings.len());

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let tick = r#"{"op":"tick","args":{"now":0}}"#.to_string();
    assert!(Detail::new().workflow(&req(&vec![tick.clone(); 64])).is_ok());
    assert!(Detail::new().workflow(&req(&vec![tick; 65])).is_err());
    for (lim, ok) in [
        (r#"{"beings":1000000,"needs":64,"groups":65536}"#, true),
        (r#"{"needs":65}"#, false),
        (r#"{"groups":65537}"#, false),
        (r#"{"beings":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Detail::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: 64 needs at the top rate over 10^12 ticks at every level, without overflow.
    let needs: Vec<String> = (0..64).map(|i| format!(r#"{{"id":"n{i}","rate":1000000}}"#)).collect();
    let needs = needs.join(",");
    let mut d = Detail::with_limits(&parse(r#"{"needs":64}"#, 64).unwrap()).unwrap();
    d.workflow(&req(&[
        format!(r#"{{"op":"track","args":{{"being":"a","distance":0,"needs":[{needs}]}}}}"#),
        format!(r#"{{"op":"track","args":{{"being":"b","distance":60,"needs":[{needs}]}}}}"#),
        format!(r#"{{"op":"track","args":{{"being":"c","distance":900,"group":"g","needs":[{needs}]}}}}"#),
    ]))
    .unwrap();
    let out = d.workflow(&req(&[r#"{"op":"tick","args":{"now":1000000000000}}"#.into()])).unwrap();
    let text = out.to_json();
    assert!(text.contains(r#""full":64000000000000"#), "{text}");
    assert!(d.workflow(&req(&[r#"{"op":"move","args":{"being":"c","distance":0}}"#.into()])).is_ok());
    assert!(d.beings.values().all(|b| b.needs.values().all(|n| n.level == 1_000_000)));

    // A refused op changes nothing: a crowd member without a group.
    let mut d = Detail::new();
    d.workflow(&req(&[r#"{"op":"track","args":{"being":"a","distance":0,"needs":[{"id":"n","rate":1}]}}"#.into()]))
        .unwrap();
    let before = d.clone();
    assert!(d.workflow(&req(&[r#"{"op":"move","args":{"being":"a","distance":500}}"#.into()])).is_err());
    assert_eq!(d, before);
}

#[test]
fn retention_survives_mutation_limits_and_boundaries() {
    use gamectx::character::retention::{Retention, MAX_WORK};
    let mut rng = Rng(0x2E7E_4710_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 30. Retention and migration mutation fuzzing (limits included); one manager keeps state.
    let samples = [
        r#"{"limits":{"collections":3,"entries":4,"migrations":3,"changes":4},"ops":[{"op":"rule","args":{"collection":"m","max_entries":2,"max_bytes":500,"max_age":30,"min_importance":20,"grace":5}},{"op":"add","args":{"collection":"m","id":"a","bytes":300,"importance":90,"now":1}},{"op":"add","args":{"collection":"m","id":"b","bytes":200,"importance":10,"pinned":true,"now":2}},{"op":"touch","args":{"collection":"m","id":"a","importance":5,"pinned":false,"now":40}},{"op":"prune","args":{"now":60,"dry_run":false,"collection":"m"}},{"op":"assess","args":{"collection":"m"}}]}"#,
        r#"{"ops":[{"op":"migration","args":{"version":2,"changes":[{"op":"rename","path":"*.mood","to":"feeling"},{"op":"add","path":"*.trust","value":[1,{"a":null}]},{"op":"remove","path":"x.*.y"},{"op":"clamp","path":"*.stock.*.count","min":-5,"max":100}]}},{"op":"migrate","args":{"from":1,"document":{"baker":{"mood":7,"stock":[{"count":140},{"count":-9}]},"x":[{"y":1},{"y":2}]}}}]}"#,
    ];
    let mut k = Retention::new();
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                let _ = k.workflow(&v);
            }
        }
    }
    for col in k.collections.values() {
        assert_eq!(col.bytes, col.entries.values().map(|e| e.bytes).sum::<u64>());
        assert!(col.entries.values().all(|e| e.at <= e.used && e.used <= k.clock && e.importance <= 100));
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let prune = r#"{"op":"prune","args":{"now":0}}"#.to_string();
    assert!(Retention::new().workflow(&req(&vec![prune.clone(); 64])).is_ok());
    assert!(Retention::new().workflow(&req(&vec![prune; 65])).is_err());
    for (lim, ok) in [
        (r#"{"collections":4096,"entries":1000000,"migrations":256,"changes":256}"#, true),
        (r#"{"changes":257}"#, false),
        (r#"{"migrations":257}"#, false),
        (r#"{"entries":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Retention::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: 4,096 entries pruned to one in a single op; byte and clock bounds.
    let mut k = Retention::with_limits(&parse(r#"{"entries":4096}"#, 64).unwrap()).unwrap();
    k.workflow(&req(&[r#"{"op":"rule","args":{"collection":"m","max_entries":1}}"#.into()])).unwrap();
    for chunk in 0..64 {
        let adds: Vec<String> = (chunk * 64..(chunk + 1) * 64)
            .map(|i| {
                format!(
                    r#"{{"op":"add","args":{{"collection":"m","id":"e{i}","bytes":16777216,"importance":{}}}}}"#,
                    i % 101
                )
            })
            .collect();
        k.workflow(&req(&adds)).unwrap();
    }
    let out = k.workflow(&req(&[r#"{"op":"prune","args":{"now":1000000000000}}"#.into()])).unwrap().to_json();
    assert!(out.contains(r#""kept":1"#) && out.contains(r#""freed":68702699520"#), "{}", &out[out.len() - 200..]);

    // Migration work is capped: a wildcard over a wide document is refused past MAX_WORK.
    let wide: Vec<String> = (0..2000).map(|i| format!(r#"{{"v":{i}}}"#)).collect();
    let doc = format!(r#"{{"rows":[{}]}}"#, wide.join(","));
    let changes: Vec<String> =
        (0..256).map(|_| r#"{"op":"clamp","path":"rows.*.v","min":0,"max":10}"#.to_string()).collect();
    let mut k = Retention::with_limits(&parse(r#"{"changes":256}"#, 64).unwrap()).unwrap();
    let mig = format!(r#"{{"op":"migration","args":{{"version":2,"changes":[{}]}}}}"#, changes.join(","));
    k.workflow(&req(&[mig])).unwrap();
    let small = format!(r#"{{"op":"migrate","args":{{"from":1,"document":{doc}}}}}"#);
    let out = k.workflow(&req(&[small])).unwrap().to_json();
    assert!(
        out.contains(r#""edits":1989"#) && out.contains(&format!(r#""work":{}"#, 256 * 2001)),
        "{}",
        &out[out.len() - 80..]
    );
    const { assert!(256 * 2001 <= MAX_WORK) };
    let wider: Vec<String> = (0..4000).map(|i| format!(r#"{{"v":{i}}}"#)).collect();
    let big = format!(r#"{{"op":"migrate","args":{{"from":1,"document":{{"rows":[{}]}}}}}}"#, wider.join(","));
    assert!(k.workflow(&req(&[big])).is_err(), "256 × 4,001 nodes exceeds the work cap");

    // A refused op changes nothing: a collision during migrate leaves the manager as it was.
    let mut k = Retention::new();
    k.workflow(&req(&[
        r#"{"op":"migration","args":{"version":2,"changes":[{"op":"rename","path":"a","to":"b"}]}}"#.into()
    ]))
    .unwrap();
    let before = k.clone();
    assert!(k.workflow(&req(&[r#"{"op":"migrate","args":{"from":1,"document":{"a":1,"b":2}}}"#.into()])).is_err());
    assert_eq!(k, before);
}

#[test]
fn roles_survive_mutation_limits_and_boundaries() {
    use gamectx::character::roles::Roles;
    let mut rng = Rng(0x2013_5B07_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 31. Roles mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"roles":3,"beings":4,"duties":2,"held":2},"ops":[{"op":"role","args":{"id":"guard","status":60,"seats":2,"duties":[{"id":"patrol","action":"travel","every":8,"at":3,"place":"wall","priority":70}]}},{"op":"assign","args":{"being":"tomas","role":"guard"}},{"op":"esteem","args":{"being":"tomas","delta":-20,"reason":"slept"}},{"op":"tick","args":{"now":50}},{"op":"vacate","args":{"being":"tomas","role":"guard"}},{"op":"staff","args":{"role":"guard"}}]}"#,
        r#"{"ops":[{"op":"role","args":{"id":"elder","status":85}},{"op":"assign","args":{"being":"ana","role":"elder"}},{"op":"esteem","args":{"being":"mira","delta":5,"reason":"bread"}},{"op":"contest","args":{"a":"ana","b":"mira"}},{"op":"persuade","args":{"speaker":"mira","listener":"ana","chance":500000}},{"op":"assess","args":{"being":"mira"}}]}"#,
    ];
    let mut k = Roles::new();
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                let _ = k.workflow(&v);
            }
        }
    }
    for (b, x) in &k.beings {
        assert!((-50..=50).contains(&x.standing) && (0..=100).contains(&k.status(b)));
        assert!(x.held.keys().all(|r| k.holders[r].contains(b) && x.held[r] <= k.clock), "{b}");
    }
    assert_eq!(k.holders.values().map(|h| h.len()).sum::<usize>(), k.beings.values().map(|x| x.held.len()).sum());

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let tick = r#"{"op":"tick","args":{"now":0}}"#.to_string();
    assert!(Roles::new().workflow(&req(&vec![tick.clone(); 64])).is_ok());
    assert!(Roles::new().workflow(&req(&vec![tick; 65])).is_err());
    for (lim, ok) in [
        (r#"{"roles":4096,"beings":1000000,"duties":64,"held":16}"#, true),
        (r#"{"held":17}"#, false),
        (r#"{"duties":65}"#, false),
        (r#"{"roles":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Roles::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: 16 roles of 64 every-tick duties on one being over 10^12 ticks: 1,024 tasks.
    let mut k = Roles::with_limits(&parse(r#"{"duties":64,"held":16}"#, 64).unwrap()).unwrap();
    let duties: Vec<String> = (0..64).map(|i| format!(r#"{{"id":"d{i}","action":"wait","every":1}}"#)).collect();
    let mut ops: Vec<String> = (0..16)
        .map(|r| format!(r#"{{"op":"role","args":{{"id":"r{r}","status":100,"duties":[{}]}}}}"#, duties.join(",")))
        .collect();
    ops.extend((0..16).map(|r| format!(r#"{{"op":"assign","args":{{"being":"busy","role":"r{r}"}}}}"#)));
    k.workflow(&req(&ops)).unwrap();
    let out = k.workflow(&req(&[r#"{"op":"tick","args":{"now":1000000000000}}"#.into()])).unwrap().to_json();
    assert_eq!(out.matches(r#""missed":999999999999"#).count(), 1024);

    // A refused op changes nothing: assigning past the seats.
    let mut k = Roles::new();
    k.workflow(&req(&[
        r#"{"op":"role","args":{"id":"elder","status":85,"seats":1}}"#.into(),
        r#"{"op":"assign","args":{"being":"ana","role":"elder"}}"#.into(),
    ]))
    .unwrap();
    let before = k.clone();
    assert!(k.workflow(&req(&[r#"{"op":"assign","args":{"being":"mira","role":"elder"}}"#.into()])).is_err());
    assert_eq!(k, before);
}

#[test]
fn economy_survives_mutation_limits_and_boundaries() {
    use gamectx::character::economy::Economy;
    let mut rng = Rng(0xEC07_0C04_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 32. Economy mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"goods":2,"markets":2,"traders":3},"ops":[{"op":"good","args":{"id":"flour","base":20,"elasticity":1500,"floor":250,"ceiling":4000,"rate":500,"spoil":100}},{"op":"produce","args":{"market":"square","good":"flour","amount":10,"by":"miller"}},{"op":"want","args":{"market":"square","good":"flour","amount":70,"by":"baker"}},{"op":"buy","args":{"market":"square","good":"flour","amount":10,"by":"baker"}},{"op":"settle","args":{"now":24}},{"op":"price","args":{"market":"square","good":"flour"}}]}"#,
        r#"{"ops":[{"op":"good","args":{"id":"salt","base":1000000000,"elasticity":10000,"ceiling":100000,"rate":1000}},{"op":"want","args":{"market":"harbor","good":"salt","amount":1000000,"by":"cook"}},{"op":"settle","args":{"now":1000000000000}},{"op":"produce","args":{"market":"harbor","good":"salt","amount":1000000,"by":"miner"}},{"op":"buy","args":{"market":"harbor","good":"salt","amount":1000000,"by":"cook"}},{"op":"assess","args":{"market":"harbor"}}]}"#,
    ];
    let mut k = Economy::new();
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                let _ = k.workflow(&v);
            }
        }
    }
    for (m, stalls) in &k.markets {
        for (g, s) in stalls {
            assert!(s.mult >= 1 && s.mult <= 100_000 && s.wants.len() + s.made.len() <= 2 * 65_536, "{m}/{g}");
        }
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let settle = r#"{"op":"settle","args":{"now":0}}"#.to_string();
    assert!(Economy::new().workflow(&req(&vec![settle.clone(); 64])).is_ok());
    assert!(Economy::new().workflow(&req(&vec![settle; 65])).is_err());
    for (lim, ok) in [
        (r#"{"goods":4096,"markets":1024,"traders":65536}"#, true),
        (r#"{"traders":65537}"#, false),
        (r#"{"markets":1025}"#, false),
        (r#"{"goods":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Economy::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: the top base price at the top multiplier for the top amount, and stock at 10^12.
    let mut k = Economy::new();
    k.workflow(&req(&[
        r#"{"op":"good","args":{"id":"gem","base":1000000000,"elasticity":10000,"ceiling":100000,"rate":1000}}"#.into(),
        r#"{"op":"want","args":{"market":"m","good":"gem","amount":1000000,"by":"a"}}"#.into(),
        r#"{"op":"settle","args":{"now":1}}"#.into(),
    ]))
    .unwrap();
    assert_eq!(k.markets["m"]["gem"].mult, 11_000);
    let fill = vec![r#"{"op":"produce","args":{"market":"m","good":"gem","amount":1000000,"by":"a"}}"#.to_string(); 64];
    k.workflow(&req(&fill)).unwrap();
    let out = k
        .workflow(&req(&[r#"{"op":"buy","args":{"market":"m","good":"gem","amount":1000000,"by":"b"}}"#.into()]))
        .unwrap();
    assert!(out.to_json().contains(r#""cost":11000000000000000"#));

    // A refused op changes nothing: a want past the traders limit creates no market.
    let mut k = Economy::with_limits(&parse(r#"{"markets":1}"#, 64).unwrap()).unwrap();
    k.workflow(&req(&[
        r#"{"op":"good","args":{"id":"flour","base":20}}"#.into(),
        r#"{"op":"want","args":{"market":"m1","good":"flour","amount":1,"by":"a"}}"#.into(),
    ]))
    .unwrap();
    let before = k.clone();
    assert!(k
        .workflow(&req(&[r#"{"op":"want","args":{"market":"m2","good":"flour","amount":1,"by":"a"}}"#.into()]))
        .is_err());
    assert_eq!(k, before);
}

#[test]
fn space_survives_mutation_limits_and_boundaries() {
    use gamectx::character::space::Space;
    let mut rng = Rng(0x59AC_E0C3_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 33. Space mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"rooms":2,"beings":4,"spots":2,"queue":2},"ops":[{"op":"room","args":{"id":"yard","width":8,"height":5,"walls":[[3,1],[3,2],[3,3]]}},{"op":"spot","args":{"room":"yard","id":"well","x":6,"y":2,"capacity":1}},{"op":"enter","args":{"being":"mira","room":"yard","x":0,"y":2}},{"op":"enter","args":{"being":"tomas","room":"yard","x":6,"y":1}},{"op":"use","args":{"being":"tomas","spot":"well"}},{"op":"use","args":{"being":"mira","spot":"well"}},{"op":"move","args":{"being":"mira","x":5,"y":2}},{"op":"release","args":{"being":"tomas","spot":"well"}},{"op":"see","args":{"being":"mira","other":"tomas","range":4}}]}"#,
        r#"{"ops":[{"op":"room","args":{"id":"hall","width":256,"height":3}},{"op":"enter","args":{"being":"a","room":"hall","x":0,"y":1}},{"op":"enter","args":{"being":"b","room":"hall","x":255,"y":1}},{"op":"move","args":{"being":"a","x":254,"y":1}},{"op":"exit","args":{"being":"b"}},{"op":"crowd","args":{"room":"hall"}}]}"#,
    ];
    let mut k = Space::new();
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                let _ = k.workflow(&v);
            }
        }
    }
    // Indexes agree: every being stands on its own free, non-wall cell; spot members are present.
    for (b, p) in &k.beings {
        let room = &k.rooms[&p.room];
        assert_eq!(room.cells.get(&p.at), Some(b));
        assert!(!room.walls.contains(&p.at));
    }
    for (r, room) in &k.rooms {
        assert!(room.cells.values().all(|b| k.beings[b].room == *r));
        for sp in room.spots.values() {
            assert!(
                sp.users.len() as u64 <= sp.capacity
                    && sp.users.iter().chain(&sp.queue).all(|b| k.beings[b].room == *r)
            );
        }
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let room = |i: usize| format!(r#"{{"op":"room","args":{{"id":"r{i}","width":1,"height":1}}}}"#);
    assert!(Space::new().workflow(&req(&(0..64).map(room).collect::<Vec<_>>())).is_ok());
    assert!(Space::new().workflow(&req(&(0..65).map(room).collect::<Vec<_>>())).is_err());
    for (lim, ok) in [
        (r#"{"rooms":4096,"beings":1000000,"spots":1024,"queue":4096}"#, true),
        (r#"{"queue":4097}"#, false),
        (r#"{"spots":1025}"#, false),
        (r#"{"beings":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Space::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: a 256 x 256 maze with a single-cell gap per wall row forces a long walk; a full
    // room's line of sight across 255 cells; walls at the 4,096 bound.
    let mut walls = Vec::new();
    for y in (1..256).step_by(2) {
        let gap = if y % 4 == 1 { 255 } else { 0 };
        walls.extend((0..256).filter(|x| *x != gap).map(|x| format!("[{x},{y}]")));
    }
    let maze = format!(
        r#"{{"op":"room","args":{{"id":"maze","width":256,"height":256,"walls":[{}]}}}}"#,
        walls[..4096].join(",")
    );
    let mut k = Space::with_limits(&parse(r#"{"beings":4}"#, 64).unwrap()).unwrap();
    k.workflow(&req(&[maze, r#"{"op":"enter","args":{"being":"a","room":"maze","x":0,"y":0}}"#.into()])).unwrap();
    let out = k.workflow(&req(&[r#"{"op":"move","args":{"being":"a","x":0,"y":32}}"#.into()])).unwrap().to_json();
    let steps: u64 = out.split(r#""steps":"#).nth(1).unwrap().trim_end_matches(['}', ']']).parse().unwrap();
    assert!(steps >= 16 * 255, "the maze forces a walk along every row: {steps}");
    let mut wide = Space::new();
    wide.workflow(&req(&[
        r#"{"op":"room","args":{"id":"f","width":256,"height":256}}"#.into(),
        r#"{"op":"enter","args":{"being":"a","room":"f","x":0,"y":0}}"#.into(),
        r#"{"op":"enter","args":{"being":"b","room":"f","x":255,"y":200}}"#.into(),
    ]))
    .unwrap();
    let seen = wide.workflow(&req(&[r#"{"op":"see","args":{"being":"a","other":"b"}}"#.into()])).unwrap().to_json();
    assert!(seen.contains(r#""distance":255"#) && seen.contains(r#""visible":true"#), "{seen}");

    // A refused op changes nothing: a move with no path.
    let mut k = Space::new();
    k.workflow(&req(&[
        r#"{"op":"room","args":{"id":"r","width":3,"height":3,"walls":[[1,0],[0,1],[2,1],[1,2]]}}"#.into(),
        r#"{"op":"enter","args":{"being":"a","room":"r","x":0,"y":0}}"#.into(),
    ]))
    .unwrap();
    let before = k.clone();
    assert!(k.workflow(&req(&[r#"{"op":"move","args":{"being":"a","x":1,"y":1}}"#.into()])).is_err());
    assert_eq!(k, before);
}

#[test]
fn drift_survives_mutation_limits_and_boundaries() {
    use gamectx::character::drift::Drift;
    let mut rng = Rng(0xD21F_7B04_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 34. Drift mutation fuzzing (limits included); one manager keeps state across runs.
    let samples = [
        r#"{"limits":{"beings":3,"traits":2,"history":2},"ops":[{"op":"trait","args":{"being":"tomas","trait":"trust","base":600000,"cap":150000,"step":10000,"threshold":300,"fade":1}},{"op":"experience","args":{"being":"tomas","trait":"trust","push":-100,"reason":"betrayed"}},{"op":"tick","args":{"now":40}},{"op":"experience","args":{"being":"tomas","trait":"trust","push":-100,"reason":"robbed"}},{"op":"lock","args":{"being":"tomas","trait":"trust","locked":true}},{"op":"revert","args":{"being":"tomas","trait":"trust"}},{"op":"assess","args":{"being":"tomas"}}]}"#,
        r#"{"ops":[{"op":"trait","args":{"being":"saint","trait":"kindness","base":1000000,"cap":300000,"step":100000,"threshold":1,"fade":100000}},{"op":"experience","args":{"being":"saint","trait":"kindness","push":-100,"reason":"war"}},{"op":"tick","args":{"now":1000000000000}},{"op":"lock","args":{"being":"saint","locked":false}},{"op":"assess","args":{"being":"saint","trait":"kindness"}}]}"#,
    ];
    let mut k = Drift::new();
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                let _ = k.workflow(&v);
            }
        }
    }
    for (b, m) in &k.beings {
        for (t, x) in &m.traits {
            assert!((x.value - x.base).abs() <= x.cap && x.value.abs() <= 1_000_000, "{b}/{t}");
            assert!(x.pressure.abs() < x.threshold && x.reasons.len() <= 8, "{b}/{t}");
        }
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let tick = r#"{"op":"tick","args":{"now":0}}"#.to_string();
    assert!(Drift::new().workflow(&req(&vec![tick.clone(); 64])).is_ok());
    assert!(Drift::new().workflow(&req(&vec![tick; 65])).is_err());
    for (lim, ok) in [
        (r#"{"beings":1000000,"traits":64,"history":256}"#, true),
        (r#"{"history":257}"#, false),
        (r#"{"traits":65}"#, false),
        (r#"{"beings":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Drift::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: a lifetime of 6,400 blows at threshold 1 never moves a trait past its cap, and the
    // history keeps exactly its limit; fading across 10^12 ticks does not overflow.
    let mut k = Drift::with_limits(&parse(r#"{"history":256}"#, 64).unwrap()).unwrap();
    k.workflow(&req(&[
        r#"{"op":"trait","args":{"being":"a","trait":"t","base":0,"cap":300000,"step":1,"threshold":1,"fade":100000}}"#
            .into(),
    ]))
    .unwrap();
    for _ in 0..100 {
        let blows =
            vec![r#"{"op":"experience","args":{"being":"a","trait":"t","push":-100,"reason":"blow"}}"#.to_string(); 64];
        k.workflow(&req(&blows)).unwrap();
    }
    let x = &k.beings["a"].traits["t"];
    assert_eq!((x.value, x.history.len()), (-300_000, 256));
    k.workflow(&req(&[r#"{"op":"tick","args":{"now":1000000000000}}"#.into()])).unwrap();
    assert!(k.workflow(&req(&[r#"{"op":"assess","args":{"being":"a"}}"#.into()])).is_ok());

    // A refused op changes nothing: a push out of range.
    let mut k = Drift::new();
    k.workflow(&req(&[r#"{"op":"trait","args":{"being":"a","trait":"t","base":0}}"#.into()])).unwrap();
    let before = k.clone();
    assert!(k
        .workflow(&req(&[r#"{"op":"experience","args":{"being":"a","trait":"t","push":101,"reason":"x"}}"#.into()]))
        .is_err());
    assert_eq!(k, before);
}

#[test]
fn packs_survive_mutation_limits_and_boundaries() {
    use gamectx::character::packs::Packs;
    use gamectx::kit::local::process;
    let mut rng = Rng(0xD5FA_C4B0_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 35. Packs mutation fuzzing (limits included); one manager keeps state across runs, and
    // every request an accepted apply emits is accepted by its own engine.
    let samples = [
        r#"{"limits":{"custom":3,"roles":3,"norms":2},"ops":[{"op":"define","args":{"kind":"species","id":"mule","pack":{"lifespan":262800,"needs":[{"id":"hunger","action":"feed","threshold":65,"rate_per_tick":2,"amount":20,"label":"Hunger"}]}}},{"op":"define","args":{"kind":"role","id":"ferryman","pack":{"status":25,"seats":2,"duties":[{"id":"crossing","action":"travel","every":4,"at":1,"place":"dock","priority":50}]}}},{"op":"apply","args":{"being":"jem","species":"mule","temperament":"bold","roles":["ferryman","guard"],"norms":["village","guild"]}}]}"#,
        r#"{"ops":[{"op":"define","args":{"kind":"temperament","id":"odd","pack":{"cognition":"dreamy","survival":"timid","traits":[{"trait":"greed","base":-1000000}]}}},{"op":"define","args":{"kind":"norms","id":"sea","pack":{"tenets":[{"id":"sea_water","action":"take","severity":100,"label":"Never waste water"}]}}},{"op":"apply","args":{"being":"cato","temperament":"odd","norms":["sea","wanderer"]}},{"op":"list","args":{"kind":"norms"}},{"op":"show","args":{"kind":"role","id":"elder"}}]}"#,
    ];
    let mut k = Packs::new();
    let mut checked = 0;
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                if let Ok(out) = k.workflow(&v) {
                    for r in out.get("results").and_then(Value::as_array).unwrap_or(&[]) {
                        for e in r.get("requests").and_then(Value::as_array).unwrap_or(&[]) {
                            assert!(process(e).is_ok(), "{}", e.to_json());
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 0);
    assert!(k.count <= 3 * 1500 && k.custom.values().map(|m| m.len()).sum::<usize>() == k.count);

    // Every built-in combination at the raised limits is accepted by every engine.
    let mut k = Packs::with_limits(&parse(r#"{"roles":4,"norms":3}"#, 64).unwrap()).unwrap();
    for sp in ["human", "dog", "horse", "goat"] {
        for t in ["bold", "steady", "cautious", "timid", "dreamy"] {
            let a = format!(
                r#"{{"op":"apply","args":{{"being":"b","species":"{sp}","temperament":"{t}","roles":["guard","baker","elder","healer"],"norms":["village","guild","wanderer"]}}}}"#
            );
            let out = k.workflow(&req(&[a])).unwrap();
            let rs = out.get("results").and_then(Value::as_array).unwrap()[0].get("requests").unwrap().clone();
            assert_eq!(rs.as_array().unwrap().len(), 6);
            for e in rs.as_array().unwrap() {
                assert!(process(e).is_ok(), "{}", e.to_json());
            }
        }
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let list = r#"{"op":"list","args":{"kind":"role"}}"#.to_string();
    assert!(Packs::new().workflow(&req(&vec![list.clone(); 64])).is_ok());
    assert!(Packs::new().workflow(&req(&vec![list; 65])).is_err());
    for (lim, ok) in [
        (r#"{"custom":1000000,"roles":4,"norms":4}"#, true),
        (r#"{"roles":5}"#, false),
        (r#"{"norms":5}"#, false),
        (r#"{"custom":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Packs::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: four norm sets of up to sixteen tenets may not merge past sixteen.
    let mut k = Packs::with_limits(&parse(r#"{"norms":4}"#, 64).unwrap()).unwrap();
    let tenets = |p: &str| {
        (0..8)
            .map(|i| format!(r#"{{"id":"{p}{i}","action":"take","severity":1,"label":"x"}}"#))
            .collect::<Vec<_>>()
            .join(",")
    };
    for p in ["a", "b", "c"] {
        let d =
            format!(r#"{{"op":"define","args":{{"kind":"norms","id":"{p}","pack":{{"tenets":[{}]}}}}}}"#, tenets(p));
        k.workflow(&req(&[d])).unwrap();
    }
    assert!(k.workflow(&req(&[r#"{"op":"apply","args":{"being":"x","norms":["a","b"]}}"#.into()])).is_ok());
    assert!(k.workflow(&req(&[r#"{"op":"apply","args":{"being":"x","norms":["a","b","c"]}}"#.into()])).is_err());

    // A refused op changes nothing: replacing a built-in.
    let mut k = Packs::new();
    let before = k.clone();
    let bad = r#"{"op":"define","args":{"kind":"role","id":"guard","pack":{"status":1}}}"#;
    assert!(k.workflow(&req(&[bad.into()])).is_err());
    assert_eq!(k, before);
}

#[test]
fn turn_frame_survives_mutation_limits_and_boundaries() {
    use gamectx::character::frame::Frame;
    let mut rng = Rng(0x7F3A_0E36_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 36. Turn frame mutation fuzzing (limits included); one frame keeps trajectories across runs,
    // and every accepted rank keeps its packing promises.
    let samples = [
        r#"{"limits":{"beings":3,"keys":2,"samples":3,"items":6},"ops":[{"op":"observe","args":{"being":"mira","key":"trust","value":40}},{"op":"tick","args":{"now":500}},{"op":"observe","args":{"being":"mira","key":"trust","value":-10}},{"op":"trend","args":{"being":"mira","key":"trust","window":3,"shock":30,"level":40,"settle":500}},{"op":"outcome","args":{"being":"mira","ok":false}},{"op":"time","args":{"at":480,"ticks_per_hour":60,"due":false}},{"op":"rank","args":{"budget":200,"listener":"player","earshot":["gossip"],"cues":{"entities":["apprentice"],"tags":["theft"],"places":["shop"]},"task_needs":["oven"],"mood":{"valence":-40,"resentment":60,"gratitude":0},"weights":{"urgency":30,"listener":20,"utterance":20,"task":15,"mood":10,"recency":5},"recent":3,"floor":1,"items":[{"id":"fire","bytes":60,"urgency":95,"tags":["oven"],"mandatory":true},{"id":"trail","bytes":70,"entities":["apprentice"],"tags":["theft"],"group":"theft","polarity":"grievance","at":500},{"id":"pouch","bytes":50,"tags":["theft"],"group":"theft","at":499},{"id":"lore","bytes":200,"kind":"lore"}]}}]}"#,
        r#"{"ops":[{"op":"frame","args":{"being":"mira","listener":"player","place":"bakery","ticks_per_hour":60,"bystanders":[{"being":"gossip","earshot":true,"trusted":false}],"body":{"fatigue":75,"pain":10,"needs":[{"id":"hunger","level":80,"threshold":60}],"conditions":["burned"],"held":"peel","posture":"bent"},"motive":{"id":"save_bread","score":92,"reason":"Burning","runner_up":"talk","runner_up_score":40},"task":{"id":"pull","step":"open","progress":80,"deadline":1,"interrupt":"never"},"affect":{"valence":-20,"arousal":70,"label":"harried","emotions":[{"emotion":"affection","toward":"boy","value":55}]},"listener_model":{"stance":"warm","trust":40,"knows":["a"],"wants":["b"],"feels":["c"],"lies":["d"],"overdue":["e"]},"offscreen":{"summary":"Counted the till","at":0},"novelty":{"id":"trail","score":70}}}]}"#,
    ];
    let mut k = Frame::new();
    let mut ranks = 0;
    for _ in 0..1500 {
        for x in &samples {
            if let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) {
                let Ok(out) = k.workflow(&v) else { continue };
                for r in out.get("results").and_then(Value::as_array).unwrap_or(&[]) {
                    let Some(kept) = r.get("kept").and_then(Value::as_array) else { continue };
                    ranks += 1;
                    let n = |v: &Value, f: &str| v.get(f).and_then(|x| x.to_json().parse::<u64>().ok()).unwrap();
                    let used: u64 = kept.iter().map(|x| n(x, "bytes")).sum();
                    assert_eq!(used, n(r, "used"));
                    let mandatory: u64 = kept
                        .iter()
                        .filter(|x| x.get("reason") == Some(&Value::from("mandatory")))
                        .map(|x| n(x, "bytes"))
                        .sum();
                    assert!(used - mandatory <= n(r, "budget"));
                    for x in kept.iter().chain(r.get("dropped").and_then(Value::as_array).unwrap()) {
                        assert!(n(x, "score") <= 1000);
                    }
                }
            }
        }
    }
    assert!(ranks > 0);
    for (b, x) in &k.beings {
        assert!(x.keys.len() <= 256 && x.streak.abs() <= 1000, "{b}");
        assert!(x.keys.values().all(|s| !s.is_empty() && s.len() <= 256), "{b}");
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let tick = r#"{"op":"tick","args":{"now":0}}"#.to_string();
    assert!(Frame::new().workflow(&req(&vec![tick.clone(); 64])).is_ok());
    assert!(Frame::new().workflow(&req(&vec![tick; 65])).is_err());
    for (lim, ok) in [
        (r#"{"beings":1000000,"keys":256,"samples":256,"items":4096}"#, true),
        (r#"{"items":4097}"#, false),
        (r#"{"samples":257}"#, false),
        (r#"{"keys":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Frame::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: 4,096 items of 65,536 bytes each rank within a 10⁷ budget without overflow, all
    // weights at 100; time labels at the 10¹² clock; a streak stays within ±1,000.
    let mut k = Frame::with_limits(&parse(r#"{"items":4096}"#, 64).unwrap()).unwrap();
    k.workflow(&req(&[r#"{"op":"tick","args":{"now":1000000000000}}"#.into()])).unwrap();
    let items: Vec<String> = (0..4096)
        .map(|i| {
            format!(
                r#"{{"id":"i{i}","bytes":65536,"urgency":100,"at":1000000000000,"group":"g{}","mandatory":{}}}"#,
                i % 7,
                i == 0
            )
        })
        .collect();
    let rank = format!(
        r#"{{"op":"rank","args":{{"budget":10000000,"weights":{{"urgency":100,"listener":100,"utterance":100,"task":100,"mood":100,"recency":100}},"items":[{}]}}}}"#,
        items.join(",")
    );
    let out = k.workflow(&req(&[rank])).unwrap().to_json();
    assert!(out.contains(r#""over_budget":true"#));
    assert!(k.workflow(&req(&[r#"{"op":"time","args":{"at":0,"ticks_per_hour":1}}"#.into()])).is_ok());
    let fails = vec![r#"{"op":"outcome","args":{"being":"a","ok":false}}"#.to_string(); 64];
    for _ in 0..20 {
        k.workflow(&req(&fails)).unwrap();
    }
    assert_eq!(k.beings["a"].streak, -1000);

    // A refused op changes nothing: an observation out of range.
    let mut k = Frame::new();
    k.workflow(&req(&[r#"{"op":"observe","args":{"being":"a","key":"k","value":1}}"#.into()])).unwrap();
    let before = k.clone();
    assert!(k.workflow(&req(&[r#"{"op":"observe","args":{"being":"a","key":"k","value":1000001}}"#.into()])).is_err());
    assert_eq!(k, before);
}

#[test]
fn cueing_survives_mutation_limits_and_boundaries() {
    use gamectx::character::cueing::{words, Cueing};
    let mut rng = Rng(0xC0E1_0E37_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 37. Cueing mutation fuzzing (limits included); one manager keeps state across runs, and an
    // accepted gate never passes a reply that contains a masked key word.
    let samples = [
        r#"{"limits":{"beings":3,"episodes":3,"listeners":2},"ops":[{"op":"episode","args":{"being":"mira","id":"loan","features":{"actions":["loan"],"entities":["player"],"tags":["tools"],"places":["shop"]},"valence":40,"emotion":"joy","secret":false}},{"op":"episode","args":{"being":"mira","id":"fire","features":{"tags":["fire"]},"valence":-80,"emotion":"fear"}},{"op":"cue","args":{"being":"mira","listener":"player","situation":{"actions":["loan"],"tags":["fire","tools"]},"cooldown":24,"min_score":2,"max":2}},{"op":"surface","args":{"being":"mira","listener":"player","episode":"loan"}},{"op":"tick","args":{"now":5}}]}"#,
        r#"{"limits":{"facts":3,"covers":2,"secrets":2},"ops":[{"op":"fact","args":{"being":"mira","id":"theft","claim":"The boy took it","source":"inference","keys":["took","stole"],"confidence":60}},{"op":"fact","args":{"being":"mira","id":"rumor","claim":"Mill closing","source":"unverified_rumor","from":"gossip","keys":["mill"]}},{"op":"cover","args":{"being":"mira","id":"shield","fact":"theft","lie":"The pouch is safe","listeners":["gossip"]}},{"op":"secret","args":{"id":"vault","keys":["lantern","seven"]}},{"op":"frame_facts","args":{"being":"mira","listener":"gossip","facts":["theft","rumor"]}},{"op":"gate","args":{"being":"mira","listener":"gossip","reply":"He took it; lantern seven.","fallback":"Not now."}},{"op":"uncover","args":{"being":"mira","id":"shield"}}]}"#,
        r#"{"ops":[{"op":"fact","args":{"being":"bo","id":"theft","claim":"Ana took it","source":"direct_witness","keys":["took","ana"]}},{"op":"cover","args":{"being":"bo","id":"lie","fact":"theft","lie":"Ana was home all day"}},{"op":"secret","args":{"id":"code","keys":["seven"]}},{"op":"gate","args":{"being":"bo","listener":"guard","reply":"Ana was home all day."}},{"op":"gate","args":{"being":"bo","listener":"guard","reply":"Nothing happened here."}}]}"#,
    ];
    let mut k = Cueing::new();
    let mut gates = 0;
    for _ in 0..1500 {
        for x in &samples {
            let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) else { continue };
            if k.clone().workflow(&v).is_err() {
                continue;
            }
            // Replay the accepted workflow op by op and re-check every passed gate against the
            // state it ran on: no masked key word outside the cover story.
            let lim = v.get("limits").map(|l| format!(r#""limits":{},"#, l.to_json())).unwrap_or_default();
            for (n, o) in v.get("ops").and_then(Value::as_array).unwrap().iter().enumerate() {
                let one = format!(r#"{{{}"ops":[{}]}}"#, if n == 0 { lim.as_str() } else { "" }, o.to_json());
                let before = k.clone();
                let out = k.workflow(&parse(&one, 1 << 20).unwrap()).unwrap();
                let r = &out.get("results").and_then(Value::as_array).unwrap()[0];
                if o.get("op") != Some(&Value::from("gate")) || r.get("ok") != Some(&Value::Bool(true)) {
                    continue;
                }
                gates += 1;
                let a = o.get("args").unwrap();
                let (b, lis) =
                    (a.get("being").unwrap().as_str().unwrap(), a.get("listener").unwrap().as_str().unwrap());
                let said = words(a.get("reply").unwrap().as_str().unwrap());
                let x = &before.beings[b];
                // The first cover by id that masks a fact for this listener is the story told.
                let mut told_for: std::collections::BTreeMap<&str, &str> = Default::default();
                for c in x.covers.values().filter(|c| c.listeners.is_empty() || c.listeners.contains(lis)) {
                    told_for.entry(c.fact.as_str()).or_insert(c.lie.as_str());
                }
                for (f, lie) in told_for {
                    let told = words(lie);
                    assert!(x.facts[f].keys.iter().all(|w| !said.contains(w) || told.contains(w)), "{one}");
                }
                for (sid, ks) in &before.secrets {
                    assert!(x.facts.contains_key(sid) || ks.iter().all(|w| !said.contains(w)));
                }
            }
        }
    }
    assert!(gates > 0);
    for (b, x) in &k.beings {
        assert!(x.episodes.values().all(|e| e.surfaced.len() <= 64 && e.valence.abs() <= 100), "{b}");
        assert!(x.covers.values().all(|c| x.facts.contains_key(&c.fact)), "{b}");
    }

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let tick = r#"{"op":"tick","args":{"now":0}}"#.to_string();
    assert!(Cueing::new().workflow(&req(&vec![tick.clone(); 64])).is_ok());
    assert!(Cueing::new().workflow(&req(&vec![tick; 65])).is_err());
    for (lim, ok) in [
        (r#"{"beings":1000000,"episodes":4096,"facts":4096,"covers":256,"secrets":4096,"listeners":64}"#, true),
        (r#"{"covers":257}"#, false),
        (r#"{"listeners":65}"#, false),
        (r#"{"episodes":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Cueing::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: a 2,000-character reply against 4,096 secrets of 8 words each; every word is a
    // leak and the fallback is the default line.
    let mut k = Cueing::with_limits(&parse(r#"{"secrets":4096}"#, 64).unwrap()).unwrap();
    for chunk in (0..4096).collect::<Vec<_>>().chunks(64) {
        let ops: Vec<String> = chunk
            .iter()
            .map(|i| {
                let ks: Vec<String> = (0..8).map(|j| format!(r#""w{}""#, (i * 8 + j) % 300)).collect();
                format!(r#"{{"op":"secret","args":{{"id":"s{i}","keys":[{}]}}}}"#, ks.join(","))
            })
            .collect();
        k.workflow(&req(&ops)).unwrap();
    }
    k.workflow(&req(&[
        r#"{"op":"fact","args":{"being":"a","id":"x","claim":"x","source":"inference","keys":["x"]}}"#.into()
    ]))
    .unwrap();
    let reply: String = (0..300).map(|i| format!("w{i} ")).collect::<String>().chars().take(2000).collect();
    let out = k
        .workflow(&req(&[format!(r#"{{"op":"gate","args":{{"being":"a","listener":"b","reply":"{reply}"}}}}"#)]))
        .unwrap()
        .to_json();
    assert!(out.contains(r#""fallback":"I would rather not say.""#) && out.contains(r#""ok":false"#));

    // A refused op changes nothing: a cover for a fact the being does not hold.
    let mut k = Cueing::new();
    k.workflow(&req(&[
        r#"{"op":"fact","args":{"being":"a","id":"x","claim":"x","source":"inference","keys":["x"]}}"#.into()
    ]))
    .unwrap();
    let before = k.clone();
    assert!(k
        .workflow(&req(&[r#"{"op":"cover","args":{"being":"a","id":"c","fact":"y","lie":"z"}}"#.into()]))
        .is_err());
    assert_eq!(k, before);
}

#[test]
fn pragmatics_survives_mutation_limits_and_boundaries() {
    use gamectx::character::pragmatics::Pragmatics;
    let mut rng = Rng(0x9A6E_0E38_2026_1108);
    fn req(ops: &[String]) -> Value {
        parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).unwrap()
    }

    // 38. Pragmatics mutation fuzzing (limits included) over model-written text; one parser keeps
    // its lexicon and thread stacks across runs. Every accepted parse keeps at most four clauses,
    // each with words, and every accepted validation's clause texts appear in order.
    let samples = [
        r#"{"limits":{"beings":3,"threads":2,"phrases":4,"own":3},"ops":[{"op":"lexicon","args":{"act":"farewell","phrases":["begone","be off"]}},{"op":"parse","args":{"text":"Here is the firewood—also, is it true the apprentice stole from you? Sorry. Begone!","speaker":"player","listener":"mira","context":{"witnessed":["fire"],"failed":["bread"]}}},{"op":"validate","args":{"text":"Sorry I'm late. Here it is.","clauses":[{"text":"Sorry I'm late","act":"apology","subtext":"sincere"},{"text":"Here it is","act":"offer_or_transfer","subtext":"urgent"}]}},{"op":"agenda","args":{"clauses":[{"index":0,"act":"question_fact","subtext":"sincere"},{"index":1,"act":"request_help","subtext":"urgent"}],"motive":{"id":"work","score":40},"own":[{"id":"cart","kind":"grievance","priority":85}]}}]}"#,
        r#"{"ops":[{"op":"open","args":{"being":"mira","id":"t1","topic":"theft","speaker":"player","question":"Did he take it?"}},{"op":"tick","args":{"now":100}},{"op":"interrupt","args":{"being":"mira","cause":"crash"}},{"op":"tick","args":{"now":110}},{"op":"resume","args":{"being":"mira","ticks_per_hour":60}},{"op":"open","args":{"being":"mira","id":"t2","topic":"oven","speaker":"mira"}},{"op":"close","args":{"being":"mira","id":"t2"}},{"op":"threads","args":{"being":"mira"}},{"op":"stance","args":{"trust":40,"affection":30,"resentment":10,"confidential":true,"bystanders":[{"being":"gossip","trusted":false,"earshot":true}],"urgency":90,"deadline_hours":1}}]}"#,
    ];
    let mut k = Pragmatics::new();
    let mut parsed = 0;
    for _ in 0..1500 {
        for x in &samples {
            let Ok(v) = parse(&mutate(&mut rng, x), 1 << 20) else { continue };
            let Ok(out) = k.workflow(&v) else { continue };
            let ops = v.get("ops").and_then(Value::as_array).unwrap();
            for (o, r) in ops.iter().zip(out.get("results").and_then(Value::as_array).unwrap()) {
                let Some(cls) = r.get("clauses").and_then(Value::as_array) else { continue };
                assert!(!cls.is_empty() && cls.len() <= 4);
                if o.get("op") == Some(&Value::from("validate")) {
                    let text = o.get("args").unwrap().get("text").unwrap().as_str().unwrap();
                    let mut pos = 0;
                    for c in cls {
                        let t = c.get("text").unwrap().as_str().unwrap();
                        pos += text[pos..].find(t).expect("validated clause is in the text") + t.len();
                    }
                } else {
                    parsed += 1;
                    for c in cls {
                        assert!(c.get("text").unwrap().as_str().unwrap().chars().any(|ch| ch.is_ascii_alphanumeric()));
                    }
                }
            }
        }
    }
    assert!(parsed > 0);
    assert!(k.stacks.values().all(|s| s.len() <= 16));
    assert!(k.stacks.values().all(|s| s.iter().filter(|t| t.status != "waiting").count() <= 1));

    // 64/65 ops; every limit's ceiling is accepted, ceiling + 1 and 0 are refused.
    let tick = r#"{"op":"tick","args":{"now":0}}"#.to_string();
    assert!(Pragmatics::new().workflow(&req(&vec![tick.clone(); 64])).is_ok());
    assert!(Pragmatics::new().workflow(&req(&vec![tick; 65])).is_err());
    for (lim, ok) in [
        (r#"{"beings":1000000,"threads":16,"phrases":4096,"own":64}"#, true),
        (r#"{"threads":17}"#, false),
        (r#"{"own":65}"#, false),
        (r#"{"phrases":0}"#, false),
        (r#"{"other":1}"#, false),
    ] {
        assert_eq!(Pragmatics::with_limits(&parse(lim, 4096).unwrap()).is_ok(), ok, "{lim}");
    }

    // Extremes: a 500-character turn of 250 boundaries keeps four clauses and counts the rest;
    // multi-byte text around boundaries never splits a character; 4,096 author phrases still parse.
    let mut k = Pragmatics::with_limits(&parse(r#"{"phrases":4096}"#, 64).unwrap()).unwrap();
    let many = "a.".repeat(250);
    let out = k.workflow(&req(&[format!(r#"{{"op":"parse","args":{{"text":"{many}"}}}}"#)])).unwrap().to_json();
    assert!(out.contains(r#""overflow":246"#));
    let out =
        k.workflow(&req(&[r#"{"op":"parse","args":{"text":"café—über! naïve; 日本 ok?"}}"#.into()])).unwrap().to_json();
    assert!(out.contains(r#""overflow":0"#));
    for chunk in 0..256 {
        let phrases: Vec<String> = (0..16).map(|j| format!(r#""w{} x{}""#, chunk, j)).collect();
        let op = format!(r#"{{"op":"lexicon","args":{{"act":"greeting","phrases":[{}]}}}}"#, phrases.join(","));
        k.workflow(&req(&[op])).unwrap();
    }
    assert_eq!(k.count, 4096);
    let out = k.workflow(&req(&[r#"{"op":"parse","args":{"text":"w255 x15 friend"}}"#.into()])).unwrap().to_json();
    assert!(out.contains(r#""act":"greeting""#));

    // A refused op changes nothing: a lexicon entry over the phrase limit.
    let mut k = Pragmatics::with_limits(&parse(r#"{"phrases":1}"#, 64).unwrap()).unwrap();
    let before = k.clone();
    assert!(k
        .workflow(&req(&[r#"{"op":"lexicon","args":{"act":"greeting","phrases":["ahoy","oi"]}}"#.into()]))
        .is_err());
    assert_eq!(k, before);
}
