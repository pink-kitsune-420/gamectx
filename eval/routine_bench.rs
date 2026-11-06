//! Synthetic paired host-update timing; no model, network or engine movement.
use gamectx::{
    json::parse,
    obj,
    routine::{Config, Frame, Routine, Runtime},
    Value,
};
use std::{sync::Arc, time::Instant};
fn frame(rev: u32) -> Frame {
    Frame::from_value(&obj!{"revision"=>rev,"now"=>rev,"conditions"=>vec!["route"],"allowed"=>vec!["work","customer","walk"],"emergency"=>false}).unwrap()
}
fn main() {
    let routine = Arc::new(Routine::parse(include_str!("../examples/dungeon/routine.json")).unwrap());
    let cfg = Config::resolve(
        &obj! {"routines"=>true,"history"=>true,"observations"=>128,"retention"=>1000000,"horizon"=>60},
        &obj! {},
    )
    .unwrap();
    let mut original =
        Runtime::new("bench", "merchant", &vec!["merchant"].into(), "cycle:37", cfg, Some(routine)).unwrap();
    original.update(frame(1)).unwrap();
    let a = parse(include_str!("../conformance/routines/remembered_sunlight/input.json"), 1_500_000).unwrap();
    let seed = a.get("args").unwrap().get("observations").unwrap().as_array().unwrap()[0].clone();
    for token in 1..=128 {
        let mut o = seed.clone();
        if let Value::Object(fields) = &mut o {
            for (k, v) in fields {
                match k.as_str() {
                    "token" => *v = Value::from(token as u32),
                    "expires" => *v = Value::from(1000000u32),
                    "source" => {
                        *v = obj! {"id"=>format!("sensor:{token}"),"speaker"=>"merchant","text"=>"A".repeat(512)}
                    }
                    "value" => *v = obj! {"visible"=>true,"detail"=>"B".repeat(900)},
                    _ => {}
                }
            }
        }
        original.observe(&o).unwrap();
    }
    let mut results = Vec::new();
    for pair in 0..7 {
        let mut baseline_ns = 0;
        let mut optimized_ns = 0;
        let mut outputs = Vec::new();
        for baseline in if pair % 2 == 0 { [true, false] } else { [false, true] } {
            let mut runtime = original.clone();
            let begin = Instant::now();
            for rev in 2..=1001 {
                let f = frame(rev);
                if baseline {
                    let mut draft = runtime.clone();
                    draft.update(f).unwrap();
                    runtime = draft;
                } else {
                    runtime.update(f).unwrap();
                }
            }
            let ns = begin.elapsed().as_nanos() as u64;
            if baseline {
                baseline_ns = ns;
            } else {
                optimized_ns = ns;
            }
            outputs.push(runtime.to_value());
        }
        assert_eq!(outputs[0], outputs[1]);
        results.push(obj! {"pair"=>pair,"snapshot_clone_ns"=>baseline_ns,"preflight_ns"=>optimized_ns});
    }
    println!("{}",obj!{"workload"=>"1000 valid host updates, 128 retained observations, seven alternating pairs; no model calls","baseline"=>"clone complete runtime before each transactional update","optimized"=>"preflight history tick, clone only bounded timeline","initial_state_bytes"=>original.to_value().json_len(),"pairs"=>results,"api_spend_usd"=>0}.to_json());
}
