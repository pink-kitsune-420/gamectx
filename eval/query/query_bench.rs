//! Paired query execution timing and separate transient allocation measurements.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};
struct Alloc;
static TRACK: AtomicBool = AtomicBool::new(false);
static COUNT: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn add(n: usize) {
    let live = LIVE.fetch_add(n, Ordering::Relaxed) + n;
    PEAK.fetch_max(live, Ordering::Relaxed);
}
// All tracked values are allocated/freed within one measured closure. System receives
// unchanged pointers/layouts. Timing trials disable the counters entirely.
unsafe impl GlobalAlloc for Alloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() && TRACK.load(Ordering::Relaxed) {
            COUNT.fetch_add(1, Ordering::Relaxed);
            add(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if TRACK.load(Ordering::Relaxed) {
            LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        }
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let result = unsafe { System.realloc(p, l, n) };
        if !result.is_null() && TRACK.load(Ordering::Relaxed) {
            COUNT.fetch_add(1, Ordering::Relaxed);
            LIVE.fetch_sub(l.size(), Ordering::Relaxed);
            add(n);
        }
        result
    }
}
#[global_allocator]
static ALLOC: Alloc = Alloc;
fn timed(f: &mut dyn FnMut(), n: usize) -> u128 {
    let start = Instant::now();
    for _ in 0..n {
        f();
    }
    start.elapsed().as_nanos()
}
fn allocations(f: &mut dyn FnMut()) -> (usize, usize) {
    COUNT.store(0, Ordering::Relaxed);
    LIVE.store(0, Ordering::Relaxed);
    PEAK.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    f();
    TRACK.store(false, Ordering::Relaxed);
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
    (COUNT.load(Ordering::Relaxed), PEAK.load(Ordering::Relaxed))
}

#[allow(dead_code, unused_imports)]
mod query_before;
pub use gamectx::{data, data::json, data::view, obj};
use gamectx::{data::query, Value};
fn main() {
    let mut measurements = Vec::new();
    for (name, size, width, sql, calls) in [
        ("tiny", 1, 8, "SELECT SUM(mass) FROM cargo", 10000),
        ("wide-total", 1024, 1024, "SELECT SUM(mass) FROM cargo", 1000),
        ("filtered-list", 1024, 1024, "SELECT id FROM cargo WHERE units >= 97 LIMIT 8", 1000),
        ("top-n-total", 1024, 1024, "SELECT SUM(mass) FROM cargo ORDER BY mass DESC LIMIT 3", 1000),
        ("text-sort", 1024, 1024, "SELECT id FROM cargo ORDER BY id DESC LIMIT 3", 1000),
    ] {
        let rows: Vec<Value> = (0..size)
            .map(|i| obj! {"id"=>format!("zone:cache.{i}"),"mass"=>i%137,"units"=>i%100,"payload"=>"x".repeat(width)})
            .collect();
        let old = query_before::Query::parse(sql).unwrap();
        let new = query::Query::parse(sql).unwrap();
        assert_eq!(old.run(&rows).unwrap().to_json(), new.run(&rows).unwrap().to_json());
        let mut before = || {
            black_box(old.run(black_box(&rows)).unwrap());
        };
        let mut after = || {
            black_box(new.run(black_box(&rows)).unwrap());
        };
        let (mut bt, mut at) = (Vec::new(), Vec::new());
        for trial in 0..7 {
            if trial % 2 == 0 {
                bt.push(timed(&mut before, calls));
                at.push(timed(&mut after, calls));
            } else {
                at.push(timed(&mut after, calls));
                bt.push(timed(&mut before, calls));
            }
        }
        bt.sort();
        at.sort();
        let ba = allocations(&mut before);
        let aa = allocations(&mut after);
        let stats = |ns: u128, (count, peak): (usize, usize)| obj! {"median_ns"=>Value::Number(ns.to_string()),"allocations_per_query"=>count,"peak_transient_bytes"=>peak};
        measurements.push(obj!{"fixture"=>name,"rows"=>size,"payload_bytes"=>width,"calls"=>calls,"trials"=>7,"before"=>stats(bt[3],ba),"after"=>stats(at[3],aa)});
    }
    println!(
        "{}",
        (obj! {"api_spend_usd"=>0,"parsed_reuse"=>true,"measurements"=>Value::Array(measurements)}).to_json()
    );
}
