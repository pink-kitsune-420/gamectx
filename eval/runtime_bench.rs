//! Offline medians with allocation counts; see eval/README.md.
mod budget_before;
use gamectx::{
    brain::{Brain, Prediction},
    budget::{self, PreparedSection, Section},
    json::parse,
    obj, Value,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
struct Counted;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
// Delegates every operation to System with the unchanged layout/pointer. Single-threaded
// benchmark counters include allocation/reallocation calls, not deallocation calls.
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, layout, n) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
fn bench(mut f: impl FnMut(), n: usize) -> (u128, usize) {
    let mut times = Vec::new();
    let mut allocs = Vec::new();
    for _ in 0..5 {
        ALLOCS.store(0, Ordering::Relaxed);
        let start = Instant::now();
        for _ in 0..n {
            f();
        }
        let elapsed = start.elapsed().as_nanos();
        let count = ALLOCS.load(Ordering::Relaxed);
        times.push(elapsed);
        allocs.push(count);
    }
    times.sort();
    allocs.sort();
    (times[2], allocs[2])
}
fn stats(label: &str, n: usize, r: (u128, usize)) -> Value {
    obj! {"mode"=>label,"calls"=>n,"trials"=>5,"median_ns"=>Value::Number(r.0.to_string()),"allocations"=>r.1}
}
fn main() {
    let mut inference = Vec::new();
    for (name, hidden, outputs) in [("small", 4, vec!["a", "b"]), ("large", 16, vec!["a", "b", "c", "d"])] {
        let brain = Brain::dense(&["x", "y", "z", "w"], &outputs, hidden, 3).unwrap();
        let states: Vec<_> = (0..256).map(|i| obj! {"x"=>i as f64/256.0,"y"=>0.5,"z"=>0.25,"w"=>0.75}).collect();
        let native: Vec<_> = (0..256)
            .map(|i| [Some((i * 1_000_000 + 128) / 256), Some(500_000), Some(250_000), Some(750_000)])
            .collect();
        let mut out = Prediction::default();
        let mask = (1u16 << outputs.len()) - 1;
        let mut cursor = 0;
        let json = bench(
            || {
                black_box(brain.run(black_box(&states[cursor % 256]), &outputs).unwrap());
                cursor += 1;
            },
            20_000,
        );
        let typed = bench(
            || {
                brain.run_into(black_box(&native[cursor % 256]), mask, 0, 0, &mut out).unwrap();
                black_box(&out);
                cursor += 1;
            },
            20_000,
        );
        assert_eq!(typed.1, 0);
        inference.push(obj! {"size"=>name,"json"=>stats("json",20_000,json),"native"=>stats("native",20_000,typed)});
    }
    let mut packets = Vec::new();
    for (name, count, size, cap) in [("fits", 16, 128, 4096), ("pressure", 32, 256, 2048)] {
        let sections: Vec<_> = (0..count)
            .map(|i| Section {
                name: format!("s{i}"),
                priority: (i % 5) as i64,
                stable: i % 2 == 0,
                required: i == 0,
                value: Value::str("x".repeat(size)),
            })
            .collect();
        let old: Vec<_> = sections
            .iter()
            .map(|s| budget_before::Section {
                name: s.name.clone(),
                priority: s.priority,
                stable: s.stable,
                required: s.required,
                value: s.value.clone(),
            })
            .collect();
        let prepared: Vec<_> = sections.iter().cloned().map(PreparedSection::new).collect();
        let refs: Vec<_> = prepared.iter().collect();
        let original = budget_before::assemble_bytes(old.clone(), cap).unwrap();
        let optimized = budget::assemble_bytes(sections.clone(), cap).unwrap();
        assert_eq!(original.text, optimized.text);
        assert_eq!(original.receipt.to_value(), optimized.receipt.to_value());
        let old = bench(
            || {
                black_box(budget_before::assemble_bytes(old.clone(), cap).unwrap());
            },
            500,
        );
        let new = bench(
            || {
                black_box(budget::assemble_bytes(sections.clone(), cap).unwrap());
            },
            500,
        );
        let prepared = bench(
            || {
                black_box(budget::assemble_prepared_bytes(&refs, cap).unwrap());
            },
            500,
        );
        packets.push(obj!{"pattern"=>name,"old"=>stats("old",500,old),"new"=>stats("new",500,new),"prepared"=>stats("prepared",500,prepared)});
    }
    let workflow = parse(include_str!("../examples/brain/selection.json"), 262144).unwrap();
    let request = &workflow.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap();
    let mut source = workflow.as_object().unwrap().to_vec();
    source.retain(|(k, _)| k != "operations");
    source.push(("operations".into(), Value::Array(vec![])));
    let initial = gamectx::brain::process(&Value::Object(source)).unwrap();
    let brain = Brain::restore(initial.get("state").unwrap()).unwrap();
    let mut direct = brain.clone();
    let start = Instant::now();
    let expected = direct.train_select(request).unwrap();
    let direct_ns = start.elapsed().as_nanos();
    let mut job = brain.start_train_select(request).unwrap();
    let mut slices = Vec::new();
    while !job.is_finished() {
        let start = Instant::now();
        job.advance(4096).unwrap();
        slices.push(start.elapsed().as_nanos());
    }
    let mut sliced = brain;
    assert_eq!(job.finish(&mut sliced).unwrap(), expected);
    assert_eq!(sliced.to_value(), direct.to_value());
    let total: u128 = slices.iter().sum();
    slices.sort();
    let training = obj! {"slice_cap"=>4096,"slices"=>slices.len(),"direct_ns"=>Value::Number(direct_ns.to_string()),
    "sliced_total_ns"=>Value::Number(total.to_string()),"slice_median_ns"=>Value::Number(slices[slices.len()/2].to_string()),"slice_max_ns"=>Value::Number(slices[slices.len()-1].to_string()),"identical"=>true};
    println!(
        "{}",
        (obj! {"api_spend_usd"=>0,"inference"=>inference,"packets"=>packets,"training"=>training}).to_json()
    );
}
