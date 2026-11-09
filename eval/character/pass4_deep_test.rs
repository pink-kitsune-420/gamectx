//! Pass 4 deep tests, offline and deterministic (no model, $0). The expectations were declared in
//! `docs/research/INTELLIGENT_BEING_4.md` section 7 before these runs:
//!   1. cross-genre: one theft scene (a theft, a witness, an accusation, a resolution) under the
//!      village, orbital station, noir city and academy profiles with the same psychology; the
//!      decisions must be identical and only the words may differ;
//!   2. breaking point: one being under escalating ego threats with rest and support withheld moves
//!      from mature to immature defenses, reaches exactly one crisis, never repeats it within the
//!      cooldown, keeps strain bounded and explains every step; support then brings it back;
//!   3. 100 agents over 180 days: a seeded town with communities, idioms, rumors and projects runs
//!      off-screen; every host-set limit holds, idioms cluster by community and a fad rises and
//!      falls, rumor distance grows with hops and falls after a correction, projects finish, adapt
//!      and are abandoned, the stability monitor only warns while gossip feeds it, and the run
//!      replays byte for byte.
//! The report holds only deterministic output and is checked byte for byte; wall time and memory are
//! measured separately (`--timing`) with the hardware, and are not promises.
//!
//!   rustc --edition=2021 -O eval/character/pass4_deep_test.rs -L dependency=target/release/deps \
//!     --extern gamectx=target/release/libgamectx.rlib -o target/pass4-deep
//!   target/pass4-deep --output eval/character/pass4-deep.json   # write the report
//!   target/pass4-deep --check eval/character/pass4-deep.json    # replay: byte-identical
//!   target/pass4-deep --timing eval/character/pass4-deep-timing.json
use gamectx::character::chemistry::Chemistry;
use gamectx::character::defenses::Defenses;
use gamectx::character::explain::ExplainTracer;
use gamectx::character::pragmatics::Pragmatics;
use gamectx::character::projects::Projects;
use gamectx::character::stability::StabilityMonitor;
use gamectx::data::json::{parse, Value};
use gamectx::data::persist::{self, Durable};
use gamectx::knowledge::culture::Culture;
use gamectx::knowledge::fabric::Fabric;
use gamectx::knowledge::genre::Genres;
use gamectx::{obj, Result};
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::Instant;

struct Counting;
pub static LIVE: AtomicUsize = AtomicUsize::new(0);
pub static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let now = LIVE.fetch_add(l.size(), Relaxed) + l.size();
        PEAK.fetch_max(now, Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        System.dealloc(p, l)
    }
}
#[global_allocator]
static A: Counting = Counting;

pub trait Engine {
    fn run(&mut self, req: &Value) -> Result<Value>;
    /// Saves the manager to text, loads the text into a fresh manager and puts that in its place.
    fn reload(&mut self);
}
macro_rules! engine {
    ($($t:ty),*) => {$(impl Engine for $t {
        fn run(&mut self, req: &Value) -> Result<Value> { self.workflow(req) }
        fn reload(&mut self) {
            let limits = Durable::limits(self).clone();
            let text = persist::export(self, &limits).to_json();
            let save = parse(&text, 1 << 27).expect("a save parses");
            let (fresh, _, _) = persist::restore::<$t>(&save, &limits).unwrap_or_else(|e| panic!("{} save refused: {e:?}", <$t as Durable>::KIND));
            *self = fresh;
            SAVES.fetch_add(1, Relaxed);
            LAST.lock().unwrap().insert(<$t as Durable>::KIND, text);
        }
    })*};
}

/// Save anywhere (pass 5): when set, `go` saves and reloads its manager after every op on
/// `EVERY_OP_DAY` and after every request on every tenth day (and on day 0, the setup).
pub static SAVE_ANYWHERE: AtomicBool = AtomicBool::new(false);
/// The town's current day, read by `go`.
pub static DAY: AtomicU64 = AtomicU64::new(0);
/// The day saved after every single op: a project day before the correction with every op kind.
pub const EVERY_OP_DAY: u64 = 98;
/// How many reloads ran.
pub static SAVES: AtomicU64 = AtomicU64::new(0);
/// The last save text written per kind.
pub static LAST: Mutex<BTreeMap<&'static str, String>> = Mutex::new(BTreeMap::new());
engine!(Genres, Fabric, Pragmatics, Defenses, Chemistry, ExplainTracer, Culture, Projects, StabilityMonitor);

fn req(ops: &[String]) -> Value {
    parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 22).expect("scene JSON")
}

/// Runs ops in requests of at most 64 and returns every result; each request must be accepted.
pub fn go(m: &mut dyn Engine, ops: &[String]) -> Vec<Value> {
    let (saving, day) = (SAVE_ANYWHERE.load(Relaxed), DAY.load(Relaxed));
    let size = if saving && day == EVERY_OP_DAY { 1 } else { 64 };
    let mut out = Vec::new();
    for chunk in ops.chunks(size) {
        let r = m.run(&req(chunk)).unwrap_or_else(|e| panic!("refused: {e:?} in {}", chunk.join("\n")));
        out.extend(r.get("results").and_then(Value::as_array).unwrap().iter().cloned());
        if saving && (day == EVERY_OP_DAY || day % 10 == 0) {
            m.reload();
        }
    }
    out
}

struct Scene {
    name: &'static str,
    beats: Vec<Value>,
    transcript: Vec<String>,
    explain: Vec<String>,
    checks: Vec<String>,
}

impl Scene {
    fn new(name: &'static str) -> Self {
        Scene { name, beats: Vec::new(), transcript: Vec::new(), explain: Vec::new(), checks: Vec::new() }
    }

    /// Runs ops on an engine and records the beat; the request must be accepted.
    fn run(&mut self, label: &str, m: &mut dyn Engine, ops: &[String]) -> Vec<Value> {
        let out = m.run(&req(ops)).unwrap_or_else(|e| panic!("{}: {label} refused: {e:?}", self.name));
        self.beats.push(obj! { "label" => label, "output" => out.clone() });
        out.get("results").and_then(Value::as_array).unwrap().to_vec()
    }

    /// Runs ops that the host must refuse; records the refusal.
    fn refuse(&mut self, label: &str, m: &mut dyn Engine, ops: &[String]) {
        let ok = m.run(&req(ops)).is_ok();
        self.beats.push(obj! { "label" => label, "output" => "refused" });
        self.check(&format!("refused: {label}"), !ok);
    }

    fn check(&mut self, name: &str, ok: bool) {
        if !ok {
            eprintln!("{}", self.transcript.join("\n"));
        }
        assert!(ok, "{}: check failed: {name}", self.name);
        self.checks.push(name.to_string());
    }

    fn say(&mut self, line: String) {
        self.transcript.push(line);
    }

    /// Records a trace and keeps its one-sentence explanation.
    fn why(&mut self, m: &mut ExplainTracer, being: &str, trace: String) -> String {
        let q = format!(r#"{{"op":"query_trace","args":{{"being":"{being}","limit":1}}}}"#);
        let r = self.run(&format!("explain {being}"), m, &[trace, q]);
        let why = r[1].get("traces").and_then(Value::as_array).unwrap()[0].get("why").unwrap().as_str().unwrap();
        self.explain.push(why.to_string());
        why.to_string()
    }

    fn view(self) -> Value {
        obj! {
            "beats" => Value::Array(self.beats),
            "checks" => self.checks,
            "explain" => self.explain,
            "name" => self.name,
            "transcript" => self.transcript,
        }
    }
}

fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.clone(),
        Some(x) => x.to_json(),
        None => String::new(),
    }
}

fn n(v: &Value, k: &str) -> i64 {
    s(v, k).parse().unwrap_or(-1)
}

fn arr(v: &Value, k: &str) -> Vec<Value> {
    v.get(k).and_then(Value::as_array).map(<[Value]>::to_vec).unwrap_or_default()
}

/// A small seeded generator (xorshift64*), so the town is the same on every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

// ------------------------------------------------------------------ scene 1

/// One genre's authored data, written in that genre's own words.
struct Setting {
    genre: &'static str,
    pack: Option<&'static str>,
    /// severity, contract, access, light: the genre's words.
    words: [&'static str; 4],
    /// A greeting, an accusation and a threat in the genre's slang.
    line: &'static str,
    excuse: &'static str,
    moment: &'static str,
}

const SETTINGS: [Setting; 4] = [
    Setting {
        genre: "village",
        pack: None,
        words: ["crime", "keep_secret", "restricted", "dim"],
        line: "Good morning. You took the grain sack! Watch yourself.",
        excuse: "the sack was spilling anyway",
        moment: "the first harvest night",
    },
    Setting {
        genre: "orbital_station",
        pack: Some("station_crew"),
        words: ["station_offense", "sealed_secret", "sealed", "night_cycle"],
        line: "Morning shift. You jacked the ration crate! Out the airlock.",
        excuse: "the crate was logged as surplus",
        moment: "the first docking drill",
    },
    Setting {
        genre: "noir_city",
        pack: Some("noir_street"),
        words: ["felony", "keep_mum", "police_line", "neon"],
        line: "Evening pal. You sold me out! You'll be sorry.",
        excuse: "the case was going cold anyway",
        moment: "the first rainy stakeout",
    },
    Setting {
        genre: "academy",
        pack: Some("school_casual"),
        words: ["expellable", "keep_secret", "staff_only", "dim"],
        line: "Yo. You snitched! You'll pay.",
        excuse: "the answer key was already public",
        moment: "the first exam week",
    },
];

const KEYS: &str = r#"{"severity":"severity","contract":"contract","access":"access","light":"light","address":"address"}"#;

fn one_genre(sc: &mut Scene, st: &Setting) -> (Value, Value) {
    let (mut genres, mut fabric, mut talk) = (Genres::new(), Fabric::new(), Pragmatics::new());
    let (mut defs, mut chem, mut why) = (Defenses::new(), Chemistry::new(), ExplainTracer::new());
    let g = st.genre;
    let [sev, con, acc, light] = st.words;

    // The designer's data arrives in the genre's words; the host maps it to engine words.
    let authored = format!(r#"{{"severity":"{sev}","contract":"{con}","access":"{acc}","light":"{light}"}}"#);
    let r = sc.run(&format!("{g}: normalize the authored scene"), &mut genres, &[format!(
        r#"{{"op":"normalize","args":{{"genre":"{g}","keys":{KEYS},"value":{authored}}}}}"#
    )]);
    let engine = r[0].get("value").unwrap().clone();
    let (sev, con) = (s(&engine, "severity"), s(&engine, "contract"));

    if let Some(p) = st.pack {
        let r = sc.run(&format!("{g}: lexicon pack requests"), &mut genres, &[format!(
            r#"{{"op":"lexicon_requests","args":{{"packs":["{p}"]}}}}"#
        )]);
        let lex = arr(&r[0], "requests")[0].get("args").unwrap().clone();
        talk.workflow(&lex).expect("pack lexicon");
    }
    let r = sc.run(&format!("{g}: the accusation"), &mut talk, &[format!(
        r#"{{"op":"parse","args":{{"text":"{}","speaker":"supervisor","listener":"taker"}}}}"#,
        st.line
    )]);
    let acts: Vec<String> =
        arr(&r[0], "clauses").iter().map(|c| format!("{}/{}/{}", s(c, "act"), s(c, "subtext"), s(c, "update"))).collect();

    let r = sc.run(&format!("{g}: witness, sanction and a hush contract"), &mut fabric, &[
        format!(r#"{{"op":"norm","args":{{"id":"theft","severity":"{sev}","report_to":"warden"}}}}"#),
        r#"{"op":"profile","args":{"being":"supervisor","attention":{"bandwidth":3,"vigilance":50,"biases":{"theft":200,"noise":100,"person":100}}}}"#.into(),
        r#"{"op":"notice","args":{"being":"supervisor","stimuli":[{"id":"hum","category":"noise","intensity":80},{"id":"crate_gap","category":"theft","intensity":60},{"id":"visitor","category":"person","intensity":40}]}}"#.into(),
        r#"{"op":"meet","args":{"a":"supervisor","b":"taker","episode":"e1","summary":"first shift together"}}"#.into(),
        r#"{"op":"breach","args":{"norm":"theft","offender":"taker","witness":"supervisor","conscience":70}}"#.into(),
        format!(r#"{{"op":"contract","args":{{"id":"hush","kind":"{con}","debtor":"friend","creditor":"taker","terms":"say nothing about it"}}}}"#),
        r#"{"op":"dyad","args":{"a":"supervisor","b":"taker"}}"#.into(),
    ]);
    let noticed: Vec<String> = arr(&r[2], "noticed").iter().map(|x| s(x, "id")).collect();
    let (breach, dyad) = (r[4].clone(), r[6].clone());

    let r = sc.run(&format!("{g}: the taker defends"), &mut defs, &[
        r#"{"op":"profile","args":{"being":"taker","maturity":55,"tolerance":20,"crisis":"confession"}}"#.into(),
        format!(
            r#"{{"op":"threat","args":{{"being":"taker","value":"competence","drop":50,"fact":"took_it","excuse":"{}","safer_target":"cadet","blame":"cadet"}}}}"#,
            st.excuse
        ),
    ]);
    let defense = r[1].clone();

    let r = sc.run(&format!("{g}: a friend's banter"), &mut chem, &[
        r#"{"op":"style","args":{"being":"friend","humour":"dry","tempo":"measured","formality":30,"assertiveness":40,"values":["loyalty"]}}"#.into(),
        r#"{"op":"style","args":{"being":"taker","humour":"dry","tempo":"measured","formality":40,"assertiveness":50,"values":["loyalty"]}}"#.into(),
        r#"{"op":"history","args":{"a":"friend","b":"taker","landmarks":2,"favors":1}}"#.into(),
        format!(r#"{{"op":"joke","args":{{"a":"friend","b":"taker","id":"first","moment":"{}"}}}}"#, st.moment),
        r#"{"op":"banter","args":{"speaker":"friend","listener":"taker","mood":10,"pacing":"steady","seed":7}}"#.into(),
    ]);
    let banter = r[4].clone();

    let decision = obj! {
        "acts" => acts,
        "address" => s(&dyad, "address_a_to_b"),
        "banter" => format!("{}/{}", s(&banter, "move"), s(&banter, "appraisal")),
        "chemistry" => n(&banter, "chemistry"),
        "defense" => format!("{}/{}/{}", s(&defense, "mechanism"), s(&defense, "effect"), s(&defense, "band")),
        "kept" => n(&defense, "kept"),
        "noticed" => noticed,
        "sanction_step" => n(&breach, "step"),
        "sanctions" => breach.get("sanctions").cloned().unwrap_or(Value::Null),
    };
    // The decisions go back out in the genre's words.
    let record = format!(
        r#"{{"severity":"{sev}","contract":"{con}","access":"{}","light":"{}","address":"{}"}}"#,
        s(&engine, "access"),
        s(&engine, "light"),
        s(&dyad, "address_a_to_b")
    );
    let r = sc.run(&format!("{g}: render the decisions"), &mut genres, &[format!(
        r#"{{"op":"render","args":{{"genre":"{g}","keys":{KEYS},"value":{record}}}}}"#
    )]);
    let words = r[0].get("value").unwrap().clone();
    sc.why(&mut why, "taker", format!(
        r#"{{"op":"record_trace","args":{{"being":"taker","action":"{}","at":1,"perceptions":[{{"source":"speech","text":"{}"}}],"motive":{{"id":"protect_competence","kind":"reaction","score":{},"reason":"{}"}}}}}}"#,
        s(&defense, "mechanism"),
        st.line.trim_end_matches('.'),
        n(&defense, "kept") / 10_000,
        s(&defense, "explain"),
    ));
    sc.say(format!(
        "{g}: {} -> step {} {}; taker {} ({}); friend {} ({}); words {}",
        s(&words, "severity"),
        n(&breach, "step"),
        s(&breach, "sanctions"),
        s(&defense, "mechanism"),
        s(&defense, "effect"),
        s(&banter, "move"),
        s(&banter, "appraisal"),
        words.to_json(),
    ));
    (decision, obj! { "engine" => engine, "genre" => g, "words" => words })
}

fn cross_genre() -> Value {
    let mut sc = Scene::new("cross_genre_theft");
    let mut decisions = Vec::new();
    let mut renders = Vec::new();
    for st in &SETTINGS {
        let (d, w) = one_genre(&mut sc, st);
        decisions.push(d);
        renders.push(w);
    }
    let engine_same = renders.iter().all(|r| r.get("engine") == renders[0].get("engine"));
    sc.check("every genre's authored words normalize to the same engine words", engine_same);
    let d0 = &decisions[0];
    sc.check("the accusation parses to greeting, accusation, threat in every slang", {
        let acts: Vec<String> = arr(d0, "acts").iter().map(|a| a.as_str().unwrap().split('/').next().unwrap().to_string()).collect();
        acts == ["greeting", "accusation", "threat"]
    });
    sc.check("salience order is the same: the missing crate first", s(d0, "noticed") == r#"["crate_gap","hum"]"#);
    sc.check("the theft is a step 3 sanction", n(d0, "sanction_step") == 3);
    sc.check("the taker rationalizes under the same psychology", s(d0, "defense").starts_with("rationalization/"));
    sc.check("the friend's callback lands", s(d0, "banter") == "callback/playful");
    for (i, d) in decisions.iter().enumerate().skip(1) {
        let g = SETTINGS[i].genre;
        sc.check(&format!("{g}: decisions identical to village"), d == d0);
    }
    let words: BTreeSet<String> = renders.iter().map(|r| r.get("words").unwrap().to_json()).collect();
    sc.check("the four genres render four different vocabularies", words.len() == 4);
    sc.check(
        "the station says station_offense, spacer and sealed where the village says crime, acquaintance and restricted",
        {
            let w = renders[1].get("words").unwrap();
            s(w, "severity") == "station_offense" && s(w, "address") == "spacer" && s(w, "access") == "sealed"
        },
    );
    let mut g = Genres::new();
    sc.refuse("a village word in a station scene", &mut g, &[format!(
        r#"{{"op":"normalize","args":{{"genre":"orbital_station","keys":{KEYS},"value":{{"severity":"crime"}}}}}}"#
    )]);
    let mut v = sc.view();
    if let Value::Object(fields) = &mut v {
        fields.push(("decisions".into(), Value::Array(decisions)));
        fields.push(("renders".into(), Value::Array(renders)));
    }
    v
}

// ------------------------------------------------------------------ scene 2

const BANDS: [&str; 3] = ["high", "mid", "low"];

fn breaking_point() -> Value {
    let mut sc = Scene::new("breaking_point");
    let (mut defs, mut why) = (Defenses::new(), ExplainTracer::new());
    sc.run("profile: a mature, thin-skinned keeper", &mut defs, &[
        r#"{"op":"profile","args":{"being":"keeper","maturity":80,"tolerance":10,"crisis":"confession","threshold":800000,"cooldown":72}}"#.into(),
    ]);
    // Escalating ego threats, one per tick, with rest and support withheld.
    let threats: [(&str, u64, &str); 9] = [
        ("goodness", 40, "skipped_the_count"),
        ("competence", 50, "miscounted_stores"),
        ("competence", 50, "lost_the_ledger"),
        ("belonging", 12, "late_to_muster"),
        ("competence", 30, "spoiled_flour"),
        ("goodness", 30, "hid_the_tally"),
        ("belonging", 40, "public_rebuke"),
        ("competence", 50, "failed_inspection"),
        ("goodness", 60, "exposed_lie"),
    ];
    let mut bands = Vec::new();
    let mut crises = Vec::new();
    let mut strains = Vec::new();
    let mut now = 0u64;
    for (i, (value, drop, fact)) in threats.iter().enumerate() {
        now += 1;
        let r = sc.run(&format!("threat {}: {fact}", i + 1), &mut defs, &[
            format!(r#"{{"op":"tick","args":{{"now":{now}}}}}"#),
            format!(
                r#"{{"op":"threat","args":{{"being":"keeper","value":"{value}","drop":{drop},"fact":"{fact}","excuse":"the numbers were wrong before me","blame":"porter","safer_target":"porter","project":"new_ledger"}}}}"#
            ),
        ]);
        let t = &r[1];
        bands.push(s(t, "band"));
        strains.push(n(t, "strain"));
        if t.get("crisis").is_some_and(|c| *c != Value::Null) {
            crises.push((i + 1, s(t.get("crisis").unwrap(), "outcome")));
        }
        let line = sc.why(&mut why, "keeper", format!(
            r#"{{"op":"record_trace","args":{{"being":"keeper","action":"{}","at":{now},"perceptions":[{{"source":"social","text":"{fact}"}}],"motive":{{"id":"protect_{value}","kind":"reaction","score":{},"reason":"{}"}}}}}}"#,
            s(t, "mechanism"),
            (n(t, "kept") / 10_000).min(100),
            s(t, "explain"),
        ));
        sc.say(format!(
            "t{now} {fact}: band {} -> {} ({}), strain {}{}",
            s(t, "band"),
            s(t, "mechanism"),
            s(t, "effect"),
            n(t, "strain"),
            if crises.last().is_some_and(|c| c.0 == i + 1) { " CRISIS: confession" } else { "" }
        ));
        let _ = line;
    }
    // Contradicting the denials inside the cooldown: strain climbs but no second crisis.
    let mut held: Vec<String> = Vec::new();
    let r = sc.run("status before contradiction", &mut defs, &[r#"{"op":"status","args":{"being":"keeper"}}"#.into()]);
    for d in arr(&r[0], "distortions") {
        if ["denial", "projection", "displacement"].contains(&s(&d, "mechanism").as_str()) {
            held.push(s(&d, "fact"));
        }
    }
    let mut after = Vec::new();
    for fact in &held {
        now += 1;
        let r = sc.run(&format!("contradict {fact}"), &mut defs, &[
            format!(r#"{{"op":"tick","args":{{"now":{now}}}}}"#),
            format!(r#"{{"op":"contradict","args":{{"being":"keeper","fact":"{fact}","drop":60}}}}"#),
        ]);
        after.push(n(&r[1], "strain"));
        if r[1].get("crisis").is_some_and(|c| *c != Value::Null) {
            crises.push((0, "repeat".into()));
        }
    }
    let peak = strains.iter().chain(after.iter()).copied().max().unwrap_or(0);
    let rank: Vec<usize> = bands.iter().map(|b| BANDS.iter().position(|x| x == b).unwrap()).collect();
    let pre: Vec<usize> = match crises.first() {
        Some((at, _)) => rank[..*at].to_vec(),
        None => rank.clone(),
    };
    sc.check("the mechanisms move from mature to immature as strain rises", pre.windows(2).all(|w| w[0] <= w[1]));
    sc.check("all three bands are visited on the way down", [0, 1, 2].iter().all(|b| pre.contains(b)));
    sc.check("exactly one crisis, and it is the confession", crises.len() == 1 && crises[0].1 == "confession");
    sc.check(
        "the crisis comes on the threat that reaches the threshold",
        crises.first().is_some_and(|(at, _)| strains[*at - 1] == 400_000 && strains[..*at - 1].iter().all(|x| *x < 800_000)),
    );
    sc.check(
        "contradicting the denials inside the cooldown keeps strain at the ceiling without a second crisis",
        !held.is_empty() && peak == 1_000_000 && after.iter().all(|x| *x == 1_000_000),
    );
    sc.check("strain stays bounded", strains.iter().chain(after.iter()).all(|x| (0..=1_000_000).contains(x)));
    sc.check("every threat has an explain trace", sc.explain.len() == threats.len());

    // Support comes back: friends, then rest, and the distortions are accepted.
    let mut relief = Vec::new();
    let mut ops: Vec<String> = Vec::new();
    for fact in &held {
        ops.push(format!(r#"{{"op":"accept","args":{{"being":"keeper","fact":"{fact}"}}}}"#));
    }
    let r = sc.run("the keeper accepts what was denied", &mut defs, &ops);
    relief.push(n(r.last().unwrap(), "strain"));
    for (k, (kind, amount)) in [("support", 100), ("support", 80), ("rest", 100), ("coping", 60)].iter().enumerate() {
        now += 1;
        let r = sc.run(&format!("relief {}: {kind}", k + 1), &mut defs, &[
            format!(r#"{{"op":"tick","args":{{"now":{now}}}}}"#),
            format!(r#"{{"op":"relieve","args":{{"being":"keeper","kind":"{kind}","amount":{amount}}}}}"#),
        ]);
        relief.push(n(&r[1], "strain"));
    }
    now += 1;
    let r = sc.run("a small slight after recovery", &mut defs, &[
        format!(r#"{{"op":"tick","args":{{"now":{now}}}}}"#),
        r#"{"op":"threat","args":{"being":"keeper","value":"competence","drop":20,"fact":"misfiled_note","project":"new_ledger"}}"#.into(),
    ]);
    let back = r[1].clone();
    sc.why(&mut why, "keeper", format!(
        r#"{{"op":"record_trace","args":{{"being":"keeper","action":"{}","at":{now},"motive":{{"id":"protect_competence","kind":"reaction","score":{},"reason":"{}"}},"outcome":{{"status":"success","summary":"support restored"}}}}}}"#,
        s(&back, "mechanism"),
        n(&back, "kept") / 10_000,
        s(&back, "explain"),
    ));
    sc.check("support lowers strain at every step", relief.windows(2).all(|w| w[1] <= w[0]) && relief.last() < relief.first());
    sc.check("mature defenses return", s(&back, "band") == "high");
    sc.say(format!("held distortions contradicted: {}; strain after: {:?}", held.join(", "), after));
    sc.say(format!("relief: {relief:?}; then {} ({}) in band {}", s(&back, "mechanism"), s(&back, "effect"), s(&back, "band")));
    sc.view()
}

// ------------------------------------------------------------------ scene 3

const COMMUNITIES: [&str; 4] = ["mill", "harbor", "orchard", "forge"];
const DAYS: u64 = 180;
const HOURS: u64 = 24;
const TALKS: usize = 3;
const FAD: (u64, u64) = (30, 60);
const LAST_GOSSIP: u64 = 130;
const CORRECTION: u64 = 100;
const SAMPLES: [u64; 8] = [15, 30, 45, 60, 75, 90, 120, 180];
const VERBS: [&str; 6] = ["stole", "broke", "lied", "struck", "cheated", "took"];
const DETAILS: [&str; 6] = ["at_night", "with_a_cart", "for_a_friend", "twice", "in_the_rain", "laughing"];

fn being(i: usize) -> String {
    format!("t{i:03}")
}

pub fn town() -> (Value, Value) {
    let base = LIVE.load(Relaxed);
    DAY.store(0, Relaxed);
    let started = Instant::now();
    let mut rng = Rng(7);
    let mut checks: Vec<String> = Vec::new();
    let mut check = |name: &str, ok: bool| {
        if std::env::var("PASS4_SOFT").is_ok() {
            eprintln!("{} {name}", if ok { "ok  " } else { "FAIL" });
        } else {
            assert!(ok, "town: check failed: {name}");
        }
        checks.push(name.to_string());
    };
    let limits = |s: &str| parse(s, 1 << 10).unwrap();
    let mut cul = Culture::with_limits(&limits(r#"{"beings":100,"repertoire":12,"topics":16,"beliefs":1024}"#)).unwrap();
    let mut proj = Projects::with_limits(&limits(r#"{"projects":100,"milestones":4}"#)).unwrap();
    let mut mon = StabilityMonitor::new();
    go(&mut mon, &[r#"{"op":"init","args":{}}"#.into()]);

    // A hundred beings in four communities; three links at home and one away.
    let mut links: Vec<Vec<usize>> = Vec::new();
    let mut ops = Vec::new();
    for i in 0..100 {
        let mut l: Vec<usize> = Vec::new();
        while l.len() < 3 {
            let j = i % 4 + 4 * rng.below(25) as usize;
            if j != i && !l.contains(&j) {
                l.push(j);
            }
        }
        loop {
            let j = rng.below(100) as usize;
            if j % 4 != i % 4 {
                l.push(j);
                break;
            }
        }
        let names: Vec<String> = l.iter().map(|j| format!("\"{}\"", being(*j))).collect();
        ops.push(format!(
            r#"{{"op":"member","args":{{"being":"{}","community":"{}","status":{},"openness":{},"carefulness":{},"credulity":{},"links":[{}]}}}}"#,
            being(i),
            COMMUNITIES[i % 4],
            20 + rng.below(80),
            30 + rng.below(70),
            20 + rng.below(70),
            20 + rng.below(70),
            names.join(",")
        ));
        links.push(l);
    }
    for (k, c) in COMMUNITIES.iter().enumerate() {
        ops.push(format!(
            r#"{{"op":"idiom","args":{{"id":"{c}_home","community":"{c}","phrase":"{c} talk {k}","meaning":"how {c} folk say it","prestige":70}}}}"#
        ));
        ops.push(format!(
            r#"{{"op":"idiom","args":{{"id":"{c}_quip","community":"{c}","phrase":"{c} quip {k}","meaning":"a {c} joke","prestige":30}}}}"#
        ));
    }
    ops.push(r#"{"op":"idiom","args":{"id":"glimmer","community":"harbor","phrase":"proper glimmer","meaning":"excellent, said by the fashionable","prestige":100}}"#.into());
    go(&mut cul, &ops);
    let refused_101 = cul
        .workflow(&req(&[r#"{"op":"member","args":{"being":"t100","community":"mill","status":1,"openness":1,"carefulness":1,"credulity":1}}"#.into()]))
        .is_err();

    // A project for everyone: three milestones, some with a fallback.
    let mut ops = Vec::new();
    let mut started_at = vec![0u64; 100];
    for i in 0..100 {
        let mut ms = Vec::new();
        for k in 0..3 {
            let expected = (10 + rng.below(30)) * HOURS;
            let alt = if rng.below(2) == 0 {
                format!(r#","alternative":{{"id":"m{k}b","condition":{{"kind":"task","ref":"m{k}b"}},"expected":{}}}"#, expected * 2)
            } else {
                String::new()
            };
            ms.push(format!(r#"{{"id":"m{k}","condition":{{"kind":"task","ref":"m{k}"}},"expected":{expected}{alt}}}"#));
        }
        ops.push(format!(
            r#"{{"op":"propose","args":{{"id":"p{i:03}","owner":"{}","aim":"a life's work","stake":{},"horizon":{},"milestones":[{}]}}}}"#,
            being(i),
            20 + rng.below(80),
            DAYS * HOURS,
            ms.join(",")
        ));
    }
    go(&mut proj, &ops);
    let refused_project = proj
        .workflow(&req(&[format!(
            r#"{{"op":"propose","args":{{"id":"p100","owner":"t000","aim":"one too many","stake":1,"horizon":{},"milestones":[{{"id":"m","condition":{{"kind":"task","ref":"m"}},"expected":1}}]}}}}"#,
            DAYS * HOURS
        )]))
        .is_err();

    let mut holders: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut witnesses: BTreeMap<String, usize> = BTreeMap::new();
    let mut fad_series = Vec::new();
    let mut cluster = Vec::new();
    let mut decisions: BTreeMap<String, u64> = BTreeMap::new();
    let mut gossip_days = BTreeSet::new();
    let mut rumor_before = Value::Null;
    let mut rumor_after = Value::Null;
    let mut hop_rows = Value::Null;
    let mut tells = 0u64;
    let mut exposures = 0u64;

    for day in 1..=DAYS {
        DAY.store(day, Relaxed);
        let now = day * HOURS;
        go(&mut cul, &[format!(r#"{{"op":"tick","args":{{"now":{now}}}}}"#)]);
        go(&mut proj, &[format!(r#"{{"op":"tick","args":{{"now":{now}}}}}"#)]);

        // Idioms: everyone speaks three times a day to a link.
        let speech: Vec<String> = (0..100).map(|i| format!(r#"{{"op":"speech","args":{{"being":"{}"}}}}"#, being(i))).collect();
        let usable: Vec<Vec<String>> =
            go(&mut cul, &speech).iter().map(|r| arr(r, "usable").iter().map(|u| s(u, "id")).collect()).collect();
        let mut ops = Vec::new();
        for i in 0..100 {
            let home = COMMUNITIES[i % 4];
            let extra: Vec<&String> =
                usable[i].iter().filter(|x| **x != format!("{home}_home") && **x != format!("{home}_quip")).collect();
            for _ in 0..TALKS {
                let listener = links[i][rng.below(4) as usize];
                let idiom = if home == "harbor" && (FAD.0..=FAD.1).contains(&day) && rng.below(3) != 0 {
                    "glimmer".to_string()
                } else if !extra.is_empty() && rng.below(5) == 0 {
                    extra[rng.below(extra.len() as u64) as usize].clone()
                } else if rng.below(3) == 0 {
                    format!("{home}_quip")
                } else {
                    format!("{home}_home")
                };
                ops.push(format!(
                    r#"{{"op":"expose","args":{{"listener":"{}","speaker":"{}","idiom":"{idiom}"}}}}"#,
                    being(listener),
                    being(i)
                ));
            }
        }
        exposures += ops.len() as u64;
        go(&mut cul, &ops);

        // Rumors: a new one every ten days until day 60, then gossip until LAST_GOSSIP.
        let mut ops = Vec::new();
        if day % 10 == 5 && day <= 55 {
            let topic = format!("r{}", day / 10);
            let witness = rng.below(100) as usize;
            let agent = rng.below(100) as usize;
            let k = rng.below(3) as usize;
            let det: Vec<String> = DETAILS[k..k + 3].iter().map(|d| format!("\"{d}\"")).collect();
            ops.push(format!(
                r#"{{"op":"seed","args":{{"topic":"{topic}","witness":"{}","verb":"{}","agent":"{}","place":"market","details":[{}],"amount":{}}}}}"#,
                being(witness),
                VERBS[(day / 10) as usize % 6],
                being(agent),
                det.join(","),
                1 + rng.below(20)
            ));
            witnesses.insert(topic.clone(), witness);
            holders.entry(topic).or_default().insert(witness);
        }
        if day == CORRECTION {
            // A town meeting: each witness retells the plain truth to everyone who heard it.
            for (topic, hs) in holders.iter_mut() {
                let w = cul.truths[topic].witness.clone();
                for h in hs.iter() {
                    if being(*h) != w {
                        ops.push(format!(
                            r#"{{"op":"tell","args":{{"speaker":"{w}","listener":"{}","topic":"{topic}","seed":{},"trust":90}}}}"#,
                            being(*h),
                            rng.below(1_000_000_000_000)
                        ));
                    }
                }
            }
        } else if day <= LAST_GOSSIP {
            let mut heard: Vec<(String, usize)> = Vec::new();
            for (topic, hs) in holders.iter() {
                for h in hs.iter() {
                    if rng.below(4) == 0 {
                        let l = links[*h][rng.below(4) as usize];
                        // A witness trusts their own eyes over hearsay about what they saw.
                        let trusted = if l == witnesses[topic] { 0 } else { 20 + rng.below(81) };
                        ops.push(format!(
                            r#"{{"op":"tell","args":{{"speaker":"{}","listener":"{}","topic":"{topic}","seed":{},"fondness":{},"trust":{}}}}}"#,
                            being(*h),
                            being(l),
                            rng.below(1_000_000_000_000),
                            rng.below(101),
                            trusted
                        ));
                        heard.push((topic.clone(), l));
                    }
                }
            }
            for (t, l) in heard {
                holders.get_mut(&t).unwrap().insert(l);
            }
        }
        let told = ops.iter().filter(|o| o.contains(r#""op":"tell""#)).count() as u64;
        if told > 0 {
            gossip_days.insert(day);
        }
        tells += told;
        go(&mut cul, &ops);

        // Projects: weekly off-screen progress, fortnightly encouragement for paused ones.
        if day % 7 == 0 {
            let show: Vec<String> = (0..100).map(|i| format!(r#"{{"op":"show","args":{{"project":"p{i:03}"}}}}"#)).collect();
            let views = go(&mut proj, &show);
            let mut ops = Vec::new();
            let mut who = Vec::new();
            for (i, v) in views.iter().enumerate() {
                match s(v, "status").as_str() {
                    "active" => {
                        ops.push(format!(r#"{{"op":"invest","args":{{"project":"p{i:03}","time":7,"money":{}}}}}"#, rng.below(50)));
                        ops.push(format!(
                            r#"{{"op":"offscreen","args":{{"project":"p{i:03}","elapsed":{},"luck":{}}}}}"#,
                            now - started_at[i],
                            rng.below(100)
                        ));
                        who.push(i);
                    }
                    "paused" if day % 14 == 0 => {
                        ops.push(format!(r#"{{"op":"encourage","args":{{"project":"p{i:03}","amount":{}}}}}"#, 20 + rng.below(60)));
                        ops.push(format!(r#"{{"op":"resume","args":{{"project":"p{i:03}"}}}}"#));
                    }
                    _ => {}
                }
            }
            let out = go(&mut proj, &ops);
            let mut k = 0;
            for o in out.iter().filter(|o| o.get("resolved").is_some()) {
                let i = who[k];
                k += 1;
                if *o.get("resolved").unwrap() != Value::Null {
                    started_at[i] = now;
                    let d = match s(o.get("result").unwrap(), "decision").as_str() {
                        "null" => "advance".to_string(),
                        d => d.to_string(),
                    };
                    *decisions.entry(d).or_default() += 1;
                }
            }
        }

        // Stability: the rumor total feeds the runaway monitor.
        let spread: Vec<String> = holders.keys().map(|t| format!(r#"{{"op":"spread","args":{{"topic":"{t}"}}}}"#)).collect();
        let total: i64 = if spread.is_empty() { 0 } else { go(&mut cul, &spread).iter().map(|r| n(r, "total_ops")).sum() };
        go(&mut mon, &[format!(
            r#"{{"op":"audit_turn","args":{{"turn":{day},"beings":[],"world":{{"rumor_total":{total}}}}}}}"#
        )]);

        if SAMPLES.contains(&day) {
            let mut fad = 0;
            let (mut home, mut away) = (0u64, 0u64);
            for (i, u) in usable.iter().enumerate() {
                fad += u.iter().filter(|x| *x == "glimmer").count();
                for id in u.iter().filter(|x| *x != "glimmer") {
                    if id.starts_with(COMMUNITIES[i % 4]) {
                        home += 1;
                    } else {
                        away += 1;
                    }
                }
            }
            fad_series.push(obj! { "day" => day, "glimmer_users" => fad as u64 });
            cluster.push(obj! { "day" => day, "home" => home, "away" => away });
        }
        if day == CORRECTION - 1 || day == CORRECTION + 1 || day == DAYS {
            let mut ops = Vec::new();
            for (t, hs) in holders.iter() {
                for h in hs {
                    ops.push(format!(r#"{{"op":"belief","args":{{"being":"{}","topic":"{t}"}}}}"#, being(*h)));
                }
            }
            let bs = go(&mut cul, &ops);
            let hearsay: Vec<&Value> = bs.iter().filter(|b| n(b, "hops") > 0).collect();
            let mean = |xs: &[&Value]| -> i64 {
                let sum: i64 = xs.iter().map(|b| n(b.get("distance").unwrap(), "ops")).sum();
                sum * 1000 / xs.len().max(1) as i64
            };
            let row = obj! {
                "beliefs" => bs.len(),
                "day" => day,
                "mean_ops_x1000" => mean(&hearsay),
                "max_hops" => bs.iter().map(|b| n(b, "hops")).max().unwrap_or(0),
                "dropped_details" => bs.iter().filter(|b| arr(b, "details").len() < 3).count(),
            };
            if day == CORRECTION - 1 {
                // Distance by hop band.
                let bands: [(i64, i64); 4] = [(1, 1), (2, 3), (4, 7), (8, i64::MAX)];
                let rows: Vec<Value> = bands
                    .iter()
                    .map(|(lo, hi)| {
                        let xs: Vec<&Value> = hearsay.iter().copied().filter(|b| (lo..=hi).contains(&&n(b, "hops"))).collect();
                        obj! { "hops" => if lo == hi { lo.to_string() } else if *hi == i64::MAX { format!("{lo}+") } else { format!("{lo}-{hi}") }, "beliefs" => xs.len(), "mean_ops_x1000" => mean(&xs) }
                    })
                    .collect();
                hop_rows = Value::Array(rows);
                rumor_before = row;
            } else if day == CORRECTION + 1 {
                rumor_after = row;
            }
        }
    }
    let elapsed = started.elapsed();

    // Final views.
    let show: Vec<String> = (0..100).map(|i| format!(r#"{{"op":"show","args":{{"project":"p{i:03}"}}}}"#)).collect();
    let views = go(&mut proj, &show);
    let mut status: BTreeMap<String, u64> = BTreeMap::new();
    let mut adapted = 0;
    for v in &views {
        *status.entry(s(v, "status")).or_default() += 1;
        adapted += (n(v, "adapted") > 0) as u64;
    }
    let health = go(&mut mon, &[r#"{"op":"assess_health","args":{}}"#.into(), r#"{"op":"get_warnings","args":{"kind":"rumor_runaway","limit":256}}"#.into()]);
    let warn_days: Vec<u64> = arr(&health[1], "warnings").iter().map(|w| n(w, "turn") as u64).collect();
    let max_rep = cul.members.values().map(|m| m.repertoire.len()).max().unwrap_or(0);
    let beliefs: usize = cul.beliefs.values().map(BTreeMap::len).sum();
    let max_chain = cul.beliefs.values().flat_map(|b| b.values()).map(|v| v.chain.len()).max().unwrap_or(0);

    check("a 101st member is refused at the host's 100-being limit", refused_101);
    check("a 101st project is refused at the host's limit", refused_project);
    check("no repertoire exceeds the host's limit of 12", max_rep <= 12);
    check("beliefs stay within the host's limit of 1,024", beliefs <= 1024);
    check("rumor chains keep at most 8 tellers", max_chain <= 8);
    let home_share = |r: &Value| n(r, "home") * 100 / (n(r, "home") + n(r, "away")).max(1);
    check("idioms cluster by community: at least 80% of usable idioms are home idioms on every sample day", cluster.iter().all(|r| home_share(r) >= 80));
    check("some idioms cross community lines", cluster.iter().any(|r| n(r, "away") > 0));
    let fad: Vec<i64> = fad_series.iter().map(|r| n(r, "glimmer_users")).collect();
    let peak = *fad.iter().max().unwrap();
    let peak_at = fad.iter().position(|x| *x == peak).unwrap();
    check("the fad starts unused", fad[0] == 0);
    check("the fad rises during the fair", peak >= 10);
    check("the fad falls after the fair", peak_at < fad.len() - 1 && *fad.last().unwrap() * 2 < peak);
    let bands: Vec<(i64, i64)> = match &hop_rows {
        Value::Array(a) => a.iter().map(|r| (n(r, "beliefs"), n(r, "mean_ops_x1000"))).collect(),
        _ => vec![],
    };
    let filled: Vec<i64> = bands.iter().filter(|(c, _)| *c > 0).map(|(_, m)| *m).collect();
    check("rumor distance grows with hops", filled.len() >= 3 && filled.windows(2).all(|w| w[0] < w[1]));
    check("the correction lowers the mean distance", n(&rumor_after, "mean_ops_x1000") * 2 < n(&rumor_before, "mean_ops_x1000"));
    check("some projects finish", status.get("completed").copied().unwrap_or(0) > 0);
    check("some projects adapt to a fallback", adapted > 0);
    check("some projects are abandoned", status.get("abandoned").copied().unwrap_or(0) > 0);
    check(
        "every rumor runaway warning falls on a gossip day",
        !warn_days.is_empty() && warn_days.iter().all(|d| gossip_days.contains(d)),
    );
    check("the monitor recovers once gossip stops", s(&health[0], "status") == "recovered");

    let state = LIVE.load(Relaxed).saturating_sub(base);
    if std::env::var("PASS4_SOFT").is_ok() {
        eprintln!("fad {fad:?} cluster {} decisions {decisions:?} status {status:?} adapted {adapted}", Value::Array(cluster.clone()).to_json());
        eprintln!("hops {} before {} after {} warn {warn_days:?} health {}", hop_rows.to_json(), rumor_before.to_json(), rumor_after.to_json(), health[0].to_json());
    }
    let report = obj! {
        "checks" => checks,
        "cluster" => Value::Array(cluster),
        "decisions" => Value::Object(decisions.into_iter().map(|(k, v)| (k, Value::from(v))).collect()),
        "exposures" => exposures,
        "fad" => Value::Array(fad_series),
        "health" => health[0].clone(),
        "name" => "hundred_agents_180_days",
        "projects" => obj! {
            "adapted" => adapted,
            "status" => Value::Object(status.into_iter().map(|(k, v)| (k, Value::from(v))).collect()),
        },
        "rumor" => obj! {
            "after_correction" => rumor_after,
            "before_correction" => rumor_before,
            "by_hops" => hop_rows,
            "runaway_warning_days" => warn_days,
            "tells" => tells,
        },
    };
    let timing = obj! {
        "beings" => 100,
        "days" => DAYS,
        "wall_ms" => elapsed.as_millis() as u64,
        "state_bytes" => state,
        "state_bytes_per_being" => state / 100,
    };
    (report, timing)
}

fn report() -> (String, Value) {
    let (town, timing) = town();
    let scenes = vec![cross_genre(), breaking_point(), town];
    let checks: usize = scenes.iter().map(|x| arr(x, "checks").len()).sum();
    let out = obj! {
        "actual_model_calls" => 0, "api_spend_usd" => 0, "checks" => checks,
        "scenarios" => Value::Array(scenes), "version" => 1,
    }
    .to_json()
        + "\n";
    (out, timing)
}

pub fn cpu() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|t| t.lines().find(|l| l.starts_with("model name")).map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string()))
        .unwrap_or_else(|| "unknown".into())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (a, timing) = report();
    let (b, _) = report();
    assert_eq!(a, b, "the scenes must replay byte for byte");
    match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("--output"), Some(p)) => std::fs::write(p, &a).expect("write report"),
        (Some("--check"), Some(p)) => {
            let b = std::fs::read_to_string(p).expect("read report");
            assert!(a == b, "{p} differs from a fresh run");
        }
        (Some("--timing"), Some(p)) => {
            let t = obj! {
                "hardware" => obj! { "cpu" => cpu(), "architecture" => std::env::consts::ARCH },
                "peak_heap_bytes" => PEAK.load(Relaxed),
                "town" => timing.clone(),
                "version" => 1,
            };
            std::fs::write(p, t.to_json() + "\n").expect("write timing");
        }
        _ => {}
    }
    let v = parse(&a, 1 << 24).unwrap();
    println!(
        "{{\"pass4_deep\":true,\"scenarios\":3,\"checks\":{},\"town_wall_ms\":{},\"model_calls\":0,\"api_spend_usd\":0}}",
        s(&v, "checks"),
        s(&timing, "wall_ms")
    );
}
