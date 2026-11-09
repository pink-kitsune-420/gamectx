//! Standalone host-work scale timing for character modules with host-set limits.
//! No model, network, sleeps or engine callbacks. A counting allocator reports live heap bytes.
use gamectx::character::{
    affect::AffectManager, aspiration::Aspirations, chatter::Chatter, cognition::Cognition, cooperation::Cooperation, attention::AttentionModel, beliefs::BeliefStore, body::BodyManager,
    creed::CreedManager, curiosity::CuriosityManager, deception::DeceptionLedger, diet::DietManager,
    expectation::ExpectationBook, explain::ExplainTracer, habit::Habits, lesson::Lessons, mind::MindManager,
    offscreen::OffscreenResolver, relationship::RelationshipManager, skills::SkillsManager, speech_act::SpeechActs,
    stability::StabilityMonitor, survival::Survival, detail::Detail, task::TaskManager,
};
use gamectx::{data::json::parse, obj, Result, Value};
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

type Wf = Box<dyn FnMut(&Value) -> Result<Value>>;
macro_rules! wf {
    ($t:ty, $lim:expr) => {{
        let mut m = <$t>::with_limits(&parse(&$lim, 4096)?)?;
        Box::new(move |v: &Value| m.workflow(v)) as Wf
    }};
}

/// (module, limits for n beings, setup ops, per-being op, per-being query). `{b}` is the being, `{i}` its index.
fn modules(n: usize) -> Vec<(&'static str, String, &'static str, &'static str, &'static str)> {
    vec![
        ("beliefs", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"learn","args":{"being":"{b}","source":"told","about":"mill","relation":"sells","value":"flour","from":"carter"}}"#, r#"{"op":"query","args":{"being":"{b}","relation":"sells"}}"#),
        ("speech_act", format!(r#"{{"pairs":{n}}}"#), "", r#"{"op":"relate","args":{"speaker":"{b}","listener":"baker","trust":90}}"#, r#"{"op":"assess","args":{"being":"{b}"}}"#),
        ("diet", format!(r#"{{"beings":{n}}}"#), r#"{"op":"register_food","args":{"pack":"village"}}"#, r#"{"op":"set_diet","args":{"being":"{b}","allergies":["nut"]}}"#, r#"{"op":"choose","args":{"being":"{b}","hunger_level":100,"options":[{"food":"honey_cake"},{"food":"hearty_stew","cooked":true}]}}"#),
        ("creed", format!(r#"{{"beings":{n}}}"#), r#"{"op":"register_creed","args":{"creed":{"id":"hearthkeepers","label":"Keepers of the Hearth","kind":"religion","tenets":[{"id":"no_flesh","food_tag":"meat","severity":40,"label":"Eat nothing that bled"}]}}}"#, r#"{"op":"adhere","args":{"being":"{b}","creed":"hearthkeepers","devotion":90,"at":5}}"#, r#"{"op":"judge","args":{"actor":"{b}","action":"eat","food_tags":["meat"]}}"#),
        ("explain", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"record_trace","args":{"being":"{b}","action":"warn","target":"innkeeper","perceptions":[{"source":"street","text":"a hooded figure left the inn"}],"beliefs":[{"about":"thief","fact":"the thief took the silver","source":"seen"}],"motive":{"id":"goal_protect","kind":"goal","score":65,"reason":"protect the inn"}}}"#, r#"{"op":"query_trace","args":{"being":"{b}"}}"#),
        ("stability", format!(r#"{{"tracked_beings":{n}}}"#), "", r#"{"op":"audit_turn","args":{"turn":{i},"beings":[{"being":"{b}","motive":"m","activity":"bake"}]}}"#, r#"{"op":"get_warnings","args":{}}"#),
        ("expectation", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"expect","args":{"being":"{b}","id":"tax","about":"collector","event":"arrive","due":50,"grace":10,"importance":60,"kind":"dread"}}"#, r#"{"op":"assess","args":{"being":"{b}"}}"#),
        ("attention", format!(r#"{{"profiles":{n}}}"#), "", r#"{"op":"profile","args":{"being":"{b}","alertness":80,"perception":70,"capacity":2}}"#, r#"{"op":"notice","args":{"seed":13,"events":[{"id":"bell","kind":"alarm","place":"square","salience":100,"noise":100}],"beings":[{"id":"{b}","place":"square"}]}}"#),
        ("aspiration", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"experience","args":{"being":"{b}","kind":"need","subject":"cold","weight":60,"at":1}}"#, r#"{"op":"goals","args":{"being":"{b}"}}"#),
        ("habit", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"choose","args":{"being":"{b}","slot":"dawn","action":"bake","at":1}}"#, r#"{"op":"routine","args":{"being":"{b}"}}"#),
        ("lesson", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"outcome","args":{"being":"{b}","about":"wolves","activity":"hunt","result":"bad","episode":"w0","at":1}}"#, r#"{"op":"advice","args":{"being":"{b}"}}"#),
        ("affect", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"init","args":{"being":"{b}","attitudes":[{"id":"trust_mayor","kind":"trust","subject":"mayor","level":20}]}}"#, r#"{"op":"assess","args":{"being":"{b}"}}"#),
        ("skills", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"init","args":{"being":"{b}","skills":[{"id":"baking","level":10}]}}"#, r#"{"op":"evaluate_action","args":{"being":"{b}","action":"cook","skill":"baking","base_duration":40,"base_risk":20}}"#),
        ("body", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"init","args":{"being":"{b}","needs":[{"id":"hunger","threshold":70,"rate_per_tick":1,"level":10}]}}"#, r#"{"op":"assess","args":{"being":"{b}"}}"#),
        ("mind", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"record","args":{"subject":"{b}","other":"miller","category":"knows","fact":"flour_late","status":true,"source":"told"}}"#, r#"{"op":"query","args":{"subject":"{b}","other":"miller","category":"knows","key":"flour_late"}}"#),
        ("deception", format!(r#"{{"records":{n}}}"#), "", r#"{"op":"record","args":{"id":"claim_{b}","speaker":"{b}","listener":"baker","topic":"price","claim":"cheap","true_value":"dear"}}"#, r#"{"op":"check_consistency","args":{"speaker":"{b}","listener":"baker","topic":"price","proposed_claim":"cheap"}}"#),
        ("relationship", format!(r#"{{"attitudes":{}}}"#, 3 * n), "", r#"{"op":"appraise_action","args":{"actor":"{b}","target":"baker","action":"gift","success":true}}"#, r#"{"op":"assess","args":{"subject":"baker","target":"{b}"}}"#),
        ("offscreen", format!(r#"{{"records":{n}}}"#), "", r#"{"op":"resolve_window","args":{"being":"{b}","activity":"night_baking","action":"cook","observed":false,"at":{i},"place":"bakery"}}"#, r#"{"op":"assess","args":{"being":"{b}"}}"#),
        ("curiosity", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"init","args":{"being":"{b}","temperament":"curious","known_people":["ann"]}}"#, r#"{"op":"evaluate_novelty","args":{"being":"{b}","people":["ann","zed"]}}"#),
        ("cooperation", format!(r#"{{"promises":{n}}}"#), "", r#"{"op":"promise","args":{"id":"p_{b}","from":"{b}","to":"baker","what":"bread","due":{i}}}"#, r#"{"op":"assess","args":{"being":"{b}"}}"#),
        ("cognition", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"remember","args":{"being":"{b}","id":"fire","about":"barn","what":"burned","day":{i},"people":["miller"],"importance":40}}"#, r#"{"op":"recall","args":{"being":"{b}","id":"fire","seed":7}}"#),
        ("chatter", format!(r#"{{"beings":{},"bonds":{n}}}"#, n + 1), r#"{"op":"know","args":{"being":"baker","topic":"bread_price","value":"rising","weight":60}}"#, r#"{"op":"know","args":{"being":"{b}","topic":"road","value":"open_{i}","weight":40}}"#, r#"{"op":"meet","args":{"a":"{b}","b":"baker","seed":7}}"#),
        ("survival", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"born","args":{"being":"{b}","lifespan":100000,"at":0}}"#, r#"{"op":"threat","args":{"being":"{b}","threat":"wolf","danger":70,"temperament":"cautious","cover":true}}"#),
        ("detail", format!(r#"{{"beings":{n}}}"#), "", r#"{"op":"track","args":{"being":"{b}","distance":500,"group":"east_fields","needs":[{"id":"hunger","rate":10000,"level":400000},{"id":"thirst","rate":25000}]}}"#, r#"{"op":"assess","args":{"being":"{b}"}}"#),
        ("task", format!(r#"{{"tasks":{n}}}"#), "", r#"{"op":"propose","args":{"id":"t_{b}","owner":"{b}","steps":[{"id":"w","action":"wait"}]}}"#, r#"{"op":"get","args":{"id":"t_{b}"}}"#),
    ]
}

fn make(name: &str, lim: &str) -> Result<Wf> {
    Ok(match name {
        "beliefs" => wf!(BeliefStore, lim),
        "speech_act" => wf!(SpeechActs, lim),
        "diet" => wf!(DietManager, lim),
        "creed" => wf!(CreedManager, lim),
        "explain" => wf!(ExplainTracer, lim),
        "stability" => wf!(StabilityMonitor, lim),
        "expectation" => wf!(ExpectationBook, lim),
        "attention" => wf!(AttentionModel, lim),
        "aspiration" => wf!(Aspirations, lim),
        "habit" => wf!(Habits, lim),
        "lesson" => wf!(Lessons, lim),
        "affect" => wf!(AffectManager, lim),
        "skills" => wf!(SkillsManager, lim),
        "body" => wf!(BodyManager, lim),
        "mind" => wf!(MindManager, lim),
        "deception" => wf!(DeceptionLedger, lim),
        "relationship" => wf!(RelationshipManager, lim),
        "offscreen" => wf!(OffscreenResolver, lim),
        "curiosity" => wf!(CuriosityManager, lim),
        "cooperation" => wf!(Cooperation, lim),
        "cognition" => wf!(Cognition, lim),
        "chatter" => wf!(Chatter, lim),
        "survival" => wf!(Survival, lim),
        "detail" => wf!(Detail, lim),
        _ => wf!(TaskManager, lim),
    })
}

fn op(t: &str, i: usize) -> String {
    t.replace("{b}", &format!("b{i}")).replace("{i}", &(i + 1).to_string())
}
fn req(ops: &[String]) -> Result<Value> {
    parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22)
}

fn measure(n: usize, name: &str, lim: &str, setup: &str, fill: &str, query: &str) -> Result<Value> {
    let mut m = make(name, lim)?;
    if !setup.is_empty() {
        m(&req(&[setup.to_string()])?)?;
    }
    let base = LIVE.load(Relaxed);
    // Fill n beings in 64-op workflows; requests are parsed before timing starts.
    let batches: Vec<Value> = (0..n.div_ceil(64))
        .map(|c| req(&(c * 64..((c + 1) * 64).min(n)).map(|i| op(fill, i)).collect::<Vec<_>>()))
        .collect::<Result<_>>()?;
    let start = Instant::now();
    for b in &batches {
        black_box(m(b)?);
    }
    let fill_ns = start.elapsed().as_nanos() as usize;
    drop(batches);
    let state = LIVE.load(Relaxed).saturating_sub(base);
    // 1,000 single-op queries on beings spread across the cast.
    let queries: Vec<Value> = (0..1000).map(|k| req(&[op(query, k * 7919 % n)])).collect::<Result<_>>()?;
    let mut times = Vec::with_capacity(1000);
    for q in &queries {
        let t = Instant::now();
        black_box(m(q)?);
        times.push(t.elapsed().as_nanos() as usize);
    }
    times.sort_unstable();
    drop(m);
    Ok(obj! {
        "beings" => n,
        "module" => name,
        "fill_ns_per_op" => fill_ns / n,
        "state_bytes" => state,
        "state_bytes_per_being" => state / n,
        "query_median_ns" => times[499],
        "query_p95_ns" => times[949],
        "query_max_ns" => times[999],
    })
}

fn run() -> Result<Value> {
    let mut rows = Vec::new();
    for n in [1000, 10000] {
        for (name, lim, setup, fill, query) in modules(n) {
            rows.push(measure(n, name, &lim, setup, fill, query)?);
        }
    }
    Ok(obj! {"version" => 1, "rows" => Value::Array(rows), "queries_per_row" => 1000, "actual_model_calls" => 0})
}

fn main() {
    println!("{}", run().unwrap().to_json());
}
