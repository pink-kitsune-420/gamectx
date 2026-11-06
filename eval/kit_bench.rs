//! Seven paired trials: registration every turn versus one shared registration.
use gamectx::{json, kit::Project};
use std::hint::black_box;
use std::time::Instant;
fn run(reuse: bool, iterations: usize) -> u128 {
    let text = include_str!("../examples/kit/project.json");
    let data = json::parse(include_str!("../examples/kit/snapshot.json"), 1 << 20).unwrap();
    let registered = Project::parse(text).unwrap();
    let expected = registered.build("conversation", &registered.snapshot(&data).unwrap()).unwrap().packet.text;
    let start = Instant::now();
    for _ in 0..iterations {
        let fresh;
        let p = if reuse {
            &registered
        } else {
            fresh = Project::parse(black_box(text)).unwrap();
            &fresh
        };
        let snapshot = p.snapshot(black_box(&data)).unwrap();
        let built = p.build("conversation", &snapshot).unwrap();
        assert_eq!(built.packet.text, expected);
        black_box(built);
    }
    start.elapsed().as_nanos()
}
fn main() {
    let mut reparse = Vec::new();
    let mut reuse = Vec::new();
    for i in 0..7 {
        if i % 2 == 0 {
            reparse.push(run(false, 1000));
            reuse.push(run(true, 1000));
        } else {
            reuse.push(run(true, 1000));
            reparse.push(run(false, 1000));
        }
    }
    let mut a = reparse.clone();
    a.sort();
    let mut b = reuse.clone();
    b.sort();
    println!("{{\"trials\":7,\"turns_per_trial\":1000,\"reparse_ns\":{reparse:?},\"reuse_ns\":{reuse:?},\"reparse_median_ns\":{},\"reuse_median_ns\":{},\"api_spend_usd\":0}}",a[3],b[3]);
}
