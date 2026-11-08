//! Standalone host-work scale timing for knowledge modules with host-set limits.
//! No model, network, sleeps or engine callbacks. A counting allocator reports live heap bytes.
use gamectx::knowledge::{
    action_catalog::ActionCatalog, codex::Codex, communities::CommunityWorld, facts::FactRegistry, info::Notes,
    inventory::Inventory, lore::Lore, places::nearby, recipes::Recipes, selection::Evidence, species::SpeciesWorld,
    things::Things, timeline::Timeline,
};
use gamectx::{character::limits::Limits, data::json::parse, obj, Result, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size(), Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        System.dealloc(p, l)
    }
}
#[global_allocator]
static A: Counting = Counting;

fn v(s: &str) -> Value {
    parse(s, 1 << 28).expect("valid fixture")
}
fn limits(spec: &'static [gamectx::character::limits::Limit], set: &str) -> Limits {
    let mut l = Limits::defaults(spec);
    l.apply(&v(set)).expect("limits");
    l
}
fn list(n: usize, f: impl Fn(usize) -> String) -> String {
    (0..n).map(f).collect::<Vec<_>>().join(",")
}

/// One row: `load` builds `n` entries from pre-built input (timed, heap growth counted), then
/// 1,000 single queries `query(state, k)` on entries spread over the table.
fn row<S>(
    module: &str,
    n: usize,
    load: impl FnOnce() -> Result<S>,
    mut query: impl FnMut(&mut S, usize) -> Result<Value>,
) -> Result<Value> {
    let base = LIVE.load(Relaxed);
    let start = Instant::now();
    let mut state = load()?;
    let load_ns = start.elapsed().as_nanos() as usize;
    let bytes = LIVE.load(Relaxed).saturating_sub(base);
    let mut times = Vec::with_capacity(1000);
    for k in 0..1000 {
        let i = k * 7919 % n;
        let t = Instant::now();
        black_box(query(&mut state, i)?);
        times.push(t.elapsed().as_nanos() as usize);
    }
    times.sort_unstable();
    drop(state);
    Ok(obj! {
        "entries" => n,
        "module" => module,
        "load_ns_per_entry" => load_ns / n,
        "state_bytes" => bytes,
        "state_bytes_per_entry" => bytes / n,
        "query_median_ns" => times[499],
        "query_p95_ns" => times[949],
        "query_max_ns" => times[999],
    })
}

fn places(n: usize) -> Result<Value> {
    // A valley of n fields in a chain, each linked to the next.
    let fields = list(n, |i| {
        let link =
            if i + 1 < n { format!(r#","links":[{{"place":"f{}","distance":1}}]"#, i + 1) } else { String::new() };
        format!(r#"{{"id":"f{i}","label":"Field {i}","kind":"farmland","within":"vale"{link}}}"#)
    });
    let doc = v(&format!(r#"{{"version":1,"places":[{{"id":"vale","label":"Vale","kind":"valley"}},{fields}]}}"#));
    let l = limits(gamectx::knowledge::codex::LIMITS, &format!(r#"{{"places":{},"bytes":268435456}}"#, n + 1));
    let qs: Vec<Value> = (0..n).map(|i| v(&format!(r#"{{"from":"f{i}","max_distance":8}}"#))).collect();
    row("places", n, || Codex::from_value_within(&doc, &l), |c, i| nearby(c, &qs[i]))
}

fn timeline(n: usize) -> Result<Value> {
    let entries: Vec<Value> = (0..n)
        .map(|i| {
            v(&format!(
                r#"{{"at":{i},"kind":"harvest","summary":"Harvest {i} came in.","actors":["b{i}"],"place":"mill","visibility":"witnessed","witnesses":["b{}"],"phrases":["harvest {i}"]}}"#,
                (i + 1) % n
            ))
        })
        .collect();
    let l = limits(gamectx::knowledge::timeline::LIMITS, &format!(r#"{{"entries":{n}}}"#));
    let qs: Vec<Value> = (0..n).map(|i| v(&format!(r#"{{"known_to":"b{i}","limit":16}}"#))).collect();
    row(
        "timeline",
        n,
        || {
            let mut t = Timeline::within(&l);
            for e in &entries {
                t.record(e)?;
            }
            Ok(t)
        },
        |t, i| t.query(&qs[i]),
    )
}

fn things(n: usize) -> Result<Value> {
    let rows = list(n, |i| {
        format!(r#"{{"id":"t{i}","kind":"crate","location":{{"place":"p{}"}},"condition":90,"quantity":1}}"#, i % 64)
    });
    let doc = v(&format!(
        r#"{{"version":1,"materials":[{{"id":"wood","label":"wood"}}],"kinds":[{{"id":"crate","label":"crate","category":"container","materials":[{{"material":"wood","amount":2}}],"functions":[],"weight":5,"base_value":3,"durability":50,"stackable":false}}],"things":[{rows}]}}"#
    ));
    let lim = v(&format!(r#"{{"things":{},"bytes":268435456}}"#, n + 64));
    let qs: Vec<Value> = (0..n)
        .map(|i| v(&format!(r#"{{"ops":[{{"op":"move","args":{{"thing":"t{i}","to_place":"p{}"}}}}]}}"#, i % 7)))
        .collect();
    row(
        "things",
        n,
        || {
            let mut t = Things::with_limits(&lim)?;
            t.workflow(&obj! {"things"=>doc.clone(),"ops"=>Value::Array(vec![])})?;
            Ok(t)
        },
        |t, i| t.workflow(&qs[i]),
    )
}

fn species(n: usize) -> Result<Value> {
    let animals = list(n, |i| {
        format!(r#"{{"id":"a{i}","species":"goat","tameness":50,"trust":{{}},"lead_by":null,"ridden_by":null}}"#)
    });
    let goat = r#"{"id":"goat","label":"Goat","kind":"animal","diet":"herbivore","habitats":["plains"],"lifespan":12,"temperament":{"boldness":50,"sociability":60,"aggression":20,"trainability":40},"tameable":true,"rideable":false,"carrier_capacity":0,"produce":[],"sounds":["bleat"],"gestures":["stamps"]}"#;
    let doc = v(&format!(r#"{{"species":[{goat}],"animals":[{animals}],"ops":[]}}"#));
    let lim = v(&format!(r#"{{"animals":{n}}}"#));
    let qs: Vec<Value> = (0..n)
        .map(|i| {
            v(&format!(r#"{{"ops":[{{"op":"pet","args":{{"actor":"herder","animal":"a{i}","place":"plains"}}}}]}}"#))
        })
        .collect();
    row(
        "species",
        n,
        || {
            let mut w = SpeciesWorld::with_limits(&lim)?;
            w.workflow(&doc)?;
            Ok(w)
        },
        |w, i| w.workflow(&qs[i]),
    )
}

fn recipes(n: usize) -> Result<Value> {
    let book = v(&format!(
        "[{}]",
        list(n, |i| format!(
            r#"{{"id":"r{i}","label":"Recipe {i}","category":"crafting","inputs":[{{"kind":"plank","quantity":2}}],"tools":[],"outputs":[{{"kind":"stool","quantity":1,"condition":100}}],"time":5}}"#
        ))
    ));
    let l = limits(gamectx::knowledge::recipes::LIMITS, &format!(r#"{{"recipes":{n}}}"#));
    let qs: Vec<Value> = (0..n)
        .map(|i| {
            v(&format!(r#"{{"ops":[{{"op":"check","args":{{"recipe":"r{i}","actor":"joiner","place":"shop"}}}}]}}"#))
        })
        .collect();
    row("recipes", n, || Recipes::from_value_within(&book, &l), |r, i| r.workflow(&qs[i]))
}

fn communities(n: usize) -> Result<Value> {
    let doc = v(&format!(
        "[{}]",
        list(n, |i| format!(
            r#"{{"id":"c{i}","label":"Guild {i}","description":"A guild.","members":["m{i}"],"relations":[{{"target":"c{}","stance":"friendly","trust":20}}]}}"#,
            (i + 1) % n
        ))
    ));
    let l = limits(gamectx::knowledge::communities::LIMITS, &format!(r#"{{"communities":{n}}}"#));
    let qs: Vec<Value> = (0..n)
        .map(|i| {
            v(&format!(
                r#"{{"ops":[{{"op":"check_relation","args":{{"community":"c{i}","target":"c{}"}}}}]}}"#,
                (i + 1) % n
            ))
        })
        .collect();
    row("communities", n, || CommunityWorld::parse_within(Some(&doc), &l), |w, i| w.workflow(&qs[i]))
}

fn actions(n: usize) -> Result<Value> {
    let chore = |i: usize| {
        format!(
            r#"{{"op":"register","args":{{"action":{}}}}}"#,
            ActionCatalog::new()
                .get("approach")
                .map(|a| a.to_json().replacen(r#""id":"approach""#, &format!(r#""id":"chore_{i}""#), 1))
                .unwrap_or_default()
        )
    };
    let batches: Vec<Value> = (0..n.div_ceil(64))
        .map(|c| v(&format!(r#"{{"ops":[{}]}}"#, list((c * 64..((c + 1) * 64).min(n)).len(), |j| chore(c * 64 + j)))))
        .collect();
    let lim = v(&format!(r#"{{"actions":{}}}"#, n + 64));
    let qs: Vec<Value> =
        (0..n).map(|i| v(&format!(r#"{{"ops":[{{"op":"describe","args":{{"id":"chore_{i}"}}}}]}}"#))).collect();
    row(
        "actions",
        n,
        || {
            let mut a = ActionCatalog::with_limits(&lim)?;
            for b in &batches {
                a.workflow(b)?;
            }
            Ok(a)
        },
        |a, i| a.workflow(&qs[i]),
    )
}

fn lore(n: usize) -> Result<Value> {
    let doc = v(&format!(
        r#"{{"version":2,"entries":[{}]}}"#,
        list(n, |i| format!(
            r#"{{"id":"rumor_{i}","subject":"town","topic":"rumor","value":"rumor number {i}","tags":["t{}"],"known_by":["*"],"requires":[]}}"#,
            i % 50
        ))
    ));
    let l = limits(gamectx::knowledge::lore::LIMITS, &format!(r#"{{"entries":{n},"bytes":268435456}}"#));
    let qs: Vec<Value> = (0..n)
        .map(|i| {
            v(&format!(
                r#"{{"actor":"npc","grants":[],"query":"rumor number {i}","subjects":[],"required":[],"limit":4,"max_bytes":2048}}"#
            ))
        })
        .collect();
    row("lore", n, || Lore::from_value_within(&doc, &l), |w, i| w.select(&qs[i]))
}

fn inventory(n: usize) -> Result<Value> {
    let doc = v(&format!(
        r#"{{"version":1,"revision":0,"items":[{}],"holdings":[{}]}}"#,
        list(64, |i| format!(r#"{{"id":"item:{i}","label":"item {i}","tags":["t{}"]}}"#, i % 4)),
        list(n, |i| format!(r#"{{"owner":"v{}","item":"item:{}","quantity":3}}"#, i / 4, i % 64))
    ));
    let l = limits(gamectx::knowledge::inventory::LIMITS, &format!(r#"{{"holdings":{n},"bytes":67108864}}"#));
    let qs: Vec<(Value, Value)> = (0..n).map(|i| (v(&format!(r#"["v{}"]"#, i / 4)), v("[]"))).collect();
    row(
        "inventory",
        n,
        || Inventory::from_value_within(&doc, &l),
        |inv, i| inv.rows(&qs[i].0, &qs[i].1).map(Value::Array),
    )
}

fn evidence(n: usize) -> Result<Value> {
    let doc = v(&format!(
        "[{}]",
        list(n, |i| {
            let dep = if i % 4 > 0 { format!(r#""e{}""#, i - 1) } else { String::new() };
            format!(r#"{{"id":"e{i}","value":"clue {i}","depends":[{dep}],"priority":{},"required":false}}"#, i % 97)
        })
    ));
    let l = limits(gamectx::knowledge::selection::LIMITS, &format!(r#"{{"records":{n},"bytes":4194304}}"#));
    row("evidence", n, || Evidence::from_value_within(&doc, &l), |e, i| e.select(512 + i % 1024))
}

fn info(n: usize) -> Result<Value> {
    let l = limits(gamectx::knowledge::info::LIMITS, &format!(r#"{{"notes":{n}}}"#));
    let source = v(r#"[{"id":"line:1","speaker":"player","text":"Fen owns it."}]"#);
    let notes: Vec<String> = (0..n)
        .map(|i| format!(r#"{{"subject":"item:{i}","attribute":"owner","value":"Fen","source":"line:1"}}"#))
        .collect();
    let qs: Vec<String> = (0..n).map(|i| format!(r#"{{"subject":"item:{i}","attribute":"owner"}}"#)).collect();
    let none = v("[]");
    row(
        "info",
        n,
        || {
            let mut book = Notes::within("talk", &l)?;
            for t in &notes {
                book.call("info_remember", t, &source)?;
            }
            Ok(book)
        },
        |b, i| b.call("info_lookup", &qs[i], &none),
    )
}

fn facts(n: usize) -> Result<Value> {
    use gamectx::knowledge::facts::FactOptions;
    let keys: Vec<String> = (0..n).map(|i| format!("fact_{i}")).collect();
    let value = v(r#"{"amount":3,"open":true}"#);
    row(
        "facts",
        n,
        || {
            let mut f = FactRegistry::new(n)?;
            for k in &keys {
                f.register("world", k, value.clone(), FactOptions { now: 1, ..Default::default() })?;
            }
            Ok(f)
        },
        |f, i| f.check("world", &keys[i], &value, 1, &[]).map(|c| c.to_value()),
    )
}

fn run() -> Result<Value> {
    let mut rows = Vec::new();
    for n in [1000, 10000] {
        rows.push(places(n)?);
        rows.push(timeline(n)?);
        rows.push(things(n)?);
        rows.push(species(n)?);
        rows.push(recipes(n)?);
        rows.push(communities(n)?);
        rows.push(actions(n)?);
        rows.push(lore(n)?);
        rows.push(inventory(n)?);
        rows.push(info(n)?);
    }
    // Evidence selection and facts have lower ceilings (1,024 records, 4,096 facts).
    for n in [256, 1024] {
        rows.push(evidence(n)?);
    }
    for n in [1000, 4096] {
        rows.push(facts(n)?);
    }
    Ok(obj! {"version"=>1,"rows"=>Value::Array(rows),"queries_per_row"=>1000,"actual_model_calls"=>0})
}

fn main() {
    println!("{}", run().unwrap().to_json());
}
