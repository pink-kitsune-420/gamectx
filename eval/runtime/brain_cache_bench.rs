//! Offline release benchmark; no model/API calls. See eval/README.md to compile.
use gamectx::{
    runtime::brain::{Brain, CachedBrain},
    obj, Value};
use std::hint::black_box;
use std::time::Instant;

fn time(mut run: impl FnMut(usize), n: usize) -> u128 {
    let start = Instant::now();
    for i in 0..n {
        run(black_box(i));
    }
    start.elapsed().as_nanos()
}

fn main() {
    let states: Vec<Value> =
        (0..256).map(|i| obj! {"x" => i as f64 / 256.0, "y" => 0.5, "z" => 0.25, "w" => 0.75}).collect();
    let mut workloads = Vec::new();
    let mut preload = Vec::new();
    for (size, hidden, allowed) in [("small", 4, vec!["a", "b"]), ("large", 16, vec!["a", "b", "c", "d"])] {
        let brain = Brain::dense(&["x", "y", "z", "w"], &allowed, hidden, 3).unwrap();
        for (name, unique) in [("repeated", 1), ("working_set", 16), ("churn", 256)] {
            let mut verify = CachedBrain::new(brain.clone(), 32).unwrap();
            for input in states.iter().take(unique) {
                assert_eq!(verify.run(input, &allowed).unwrap(), brain.run(input, &allowed).unwrap());
            }
            let mut baseline = Vec::new();
            let mut cached = Vec::new();
            let mut counts = (0, 0);
            for trial in 0..5 {
                let mut cache = CachedBrain::new(brain.clone(), 32).unwrap();
                let base_run = |i| {
                    black_box(brain.run(black_box(&states[i % unique]), &allowed).unwrap());
                };
                let cached_run = |i| {
                    black_box(cache.run(black_box(&states[i % unique]), &allowed).unwrap());
                };
                if trial % 2 == 0 {
                    baseline.push(time(base_run, 20_000));
                    cached.push(time(cached_run, 20_000));
                } else {
                    cached.push(time(cached_run, 20_000));
                    baseline.push(time(base_run, 20_000));
                }
                counts = (cache.stats().hits, cache.stats().misses);
            }
            baseline.sort();
            cached.sort();
            workloads.push(obj! {"size" => size,"pattern" => name,"evaluations" => 20_000,"trials" => 5,
            "capacity" => 32,"uncached_median_ns" => Value::Number(baseline[2].to_string()),
            "cached_median_ns" => Value::Number(cached[2].to_string()),"hits" => counts.0,"misses" => counts.1});
        }
        let saved = brain.to_value().to_json();
        let mut clones = Vec::new();
        let mut restores = Vec::new();
        let mut creates = Vec::new();
        for _ in 0..5 {
            clones.push(time(
                |_| {
                    black_box(brain.clone());
                },
                1000,
            ));
            restores.push(time(
                |_| {
                    black_box(Brain::restore_json(&saved).unwrap());
                },
                1000,
            ));
            creates.push(time(
                |_| {
                    black_box(Brain::dense(&["x", "y", "z", "w"], &allowed, hidden, 3).unwrap());
                },
                1000,
            ));
        }
        clones.sort();
        restores.sort();
        creates.sort();
        preload.push(obj! {"size" => size,"instances" => 1000,"trials" => 5,
        "clone_median_ns" => Value::Number(clones[2].to_string()),
        "restore_median_ns" => Value::Number(restores[2].to_string()),
        "create_median_ns" => Value::Number(creates[2].to_string())});
    }
    println!("{}",(obj! {"api_spend_usd" => 0,"kind" => "offline release microbenchmark; one local environment", "workloads" => Value::Array(workloads),"preload" => Value::Array(preload)}).to_json());
}
