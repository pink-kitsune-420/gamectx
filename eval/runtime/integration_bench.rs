//! Offline paired release timing and separately instrumented transient allocation samples.
mod budget_pre_integration;
use gamectx::{
    packet::budget::{self, PreparedSection, Section},
    obj, Value};
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
fn stats(ns: u128, a: (usize, usize)) -> Value {
    obj! {"median_ns"=>Value::Number(ns.to_string()),"allocations_per_build"=>a.0,"peak_transient_bytes"=>a.1}
}
fn main() {
    let mut rows = Vec::new();
    for (name, count, size, cap) in [
        ("tiny", 1, 8, 128),
        ("fits", 16, 128, 4096),
        ("pressure", 32, 256, 2048),
        ("large", 96, 1024, 8192),
        ("mixed", 16, 128, 4096),
    ] {
        let sections: Vec<_> = (0..count)
            .map(|i| Section {
                name: format!("s{i}"),
                priority: (i % 5) as i64,
                stable: i % 2 == 0,
                required: i == 0,
                value: if name == "mixed" && i % 4 == 3 {
                    obj! {"tick"=>i,"flag"=>true}
                } else {
                    Value::str("x".repeat(size))
                },
            })
            .collect();
        let old: Vec<_> = sections
            .iter()
            .map(|s| budget_pre_integration::Section {
                name: s.name.clone(),
                priority: s.priority,
                stable: s.stable,
                required: s.required,
                value: s.value.clone(),
            })
            .collect();
        let prior = budget_pre_integration::assemble_bytes(old.clone(), cap).unwrap();
        let new = budget::assemble_bytes(sections.clone(), cap).unwrap();
        assert_eq!(prior.text, new.text);
        assert_eq!(prior.receipt.to_value(), new.receipt.to_value());
        let prepared: Vec<_> = sections.iter().cloned().map(PreparedSection::new).collect();
        let refs: Vec<_> = prepared.iter().collect();
        let mut before = || {
            black_box(budget_pre_integration::assemble_bytes(old.clone(), cap).unwrap());
        };
        let mut after = || {
            black_box(budget::assemble_bytes(sections.clone(), cap).unwrap());
        };
        let mut reuse = || {
            black_box(budget::assemble_prepared_bytes(&refs, cap).unwrap());
        };
        let (mut bt, mut at, mut rt) = (Vec::new(), Vec::new(), Vec::new());
        let n = if name == "large" { 100 } else { 1000 };
        for trial in 0..7 {
            if trial % 2 == 0 {
                bt.push(timed(&mut before, n));
                at.push(timed(&mut after, n));
            } else {
                at.push(timed(&mut after, n));
                bt.push(timed(&mut before, n));
            }
            rt.push(timed(&mut reuse, n));
        }
        bt.sort();
        at.sort();
        rt.sort();
        let ba = allocations(&mut before);
        let aa = allocations(&mut after);
        let ra = allocations(&mut reuse);
        rows.push(obj!{"pattern"=>name,"builds"=>n,"trials"=>7,"before"=>stats(bt[3],ba),"after"=>stats(at[3],aa),"prepared"=>stats(rt[3],ra)});
    }
    println!(
        "{}",
        (obj! {"api_spend_usd"=>0,"kind"=>"paired release timing; separate allocation samples","packets"=>rows})
            .to_json()
    );
}
