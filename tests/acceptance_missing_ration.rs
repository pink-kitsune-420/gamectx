//! Pass 4 acceptance scene, "The Missing Ration" (docs/research/INTELLIGENT_BEING_4.md, section 8).
//!
//! A ration crate is missing. A junior mechanic took it for a sick friend; the steward that logs the
//! stores cannot feel fear and cannot lie, only conceal; the mechanic's certification is two
//! milestones from done. The supervisor questions the mechanic in front of the crew, the story is
//! retold four times by the next shift, the crew's slang for a ration thief spreads from one
//! prestigious speaker, and a friend's support and dry humour bring the scene to its end. The same
//! scene runs from data alone in two genres, an orbital station and a village (a granary keeper and
//! an apprentice), and must make the same decisions in different words, explain every decision and
//! replay byte for byte. Pure host computation: no model, $0.

use gamectx::character::chemistry::Chemistry;
use gamectx::character::defenses::Defenses;
use gamectx::character::explain::ExplainTracer;
use gamectx::character::minds::Minds;
use gamectx::character::projects::Projects;
use gamectx::data::json::{parse, Value};
use gamectx::knowledge::culture::Culture;
use gamectx::knowledge::fabric::Fabric;
use gamectx::knowledge::genre::Genres;

/// Every engine's raw JSON output, in order.
struct Log(Vec<String>);

macro_rules! run {
    ($log:expr, $m:expr, $($op:expr),+ $(,)?) => {{
        let ops: Vec<String> = vec![$($op.to_string()),+];
        let req = parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 20).unwrap();
        let out = $m.workflow(&req).unwrap_or_else(|e| panic!("refused {e:?}: {}", ops.join("\n")));
        $log.0.push(out.to_json());
        out.get("results").and_then(Value::as_array).unwrap().to_vec()
    }};
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

/// What a genre's author writes: words in the genre's vocabulary, role titles and flavour text.
struct Setting {
    genre: &'static str,
    /// The steward's mind: the built-in one, or an authored mind with the same fields.
    mind: &'static str,
    define_mind: bool,
    /// severity, contract, access, privacy in the genre's words.
    words: [&'static str; 4],
    roles: [(&'static str, &'static str); 4],
    excuse: &'static str,
    slang: &'static str,
    moment: &'static str,
    /// The goods, the work crew, the record and the qualification, in the setting's nouns.
    nouns: [&'static str; 4],
}

const STATION: Setting = Setting {
    genre: "orbital_station",
    mind: "steward_intelligence",
    define_mind: false,
    words: ["regulation", "sealed_secret", "sealed", "common"],
    roles: [
        ("steward", "the steward intelligence"),
        ("mechanic", "the junior mechanic"),
        ("supervisor", "the deck supervisor"),
        ("cadet", "the cadet"),
    ],
    excuse: "it was surplus from the last resupply",
    slang: "crate rat",
    moment: "their first shift on the reactor ring",
    nouns: ["ration crate", "crew", "camera record", "certification"],
};

const VILLAGE: Setting = Setting {
    genre: "village",
    mind: "granary_keeper",
    define_mind: true,
    words: ["custom", "keep_secret", "restricted", "public"],
    roles: [
        ("steward", "the granary keeper"),
        ("mechanic", "the apprentice"),
        ("supervisor", "the reeve"),
        ("cadet", "the errand boy"),
    ],
    excuse: "it was spillage from the last harvest",
    slang: "sack mouse",
    moment: "their first harvest in the low field",
    nouns: ["grain sack", "farmhands", "tally board", "journeyman test"],
};

const KEYS: &str =
    r#"{"severity":"severity","contract":"contract","access":"access","privacy":"privacy","address":"address"}"#;

/// The scene's decisions (identical across genres), its words (genre-specific) and the raw log.
struct Played {
    decisions: Vec<String>,
    words: Value,
    explain: Vec<String>,
    transcript: Vec<String>,
    log: Vec<String>,
}

fn trace(being: &str, action: &str, at: u64, (kind, id, score): (&str, &str, i64), reason: &str, seen: &str) -> String {
    format!(
        r#"{{"op":"record_trace","args":{{"being":"{being}","action":"{action}","at":{at},"perceptions":[{{"source":"scene","text":"{seen}"}}],"motive":{{"id":"{id}","kind":"{kind}","score":{},"reason":"{reason}"}}}}}}"#,
        score.clamp(0, 100)
    )
}

/// One pacing beat; returns the controller's advice.
fn beat(
    log: &mut Log,
    chem: &mut Chemistry,
    kind: &str,
    tension: i64,
    novelty: u64,
    engagement: u64,
    threads: u64,
) -> String {
    let r = run!(
        log,
        chem,
        format!(
            r#"{{"op":"beat","args":{{"scene":"questioning","kind":"{kind}","tension":{tension},"novelty":{novelty},"engagement":{engagement},"threads_open":{threads}}}}}"#
        )
    );
    s(&r[0], "advice")
}

fn scene(st: &Setting) -> Played {
    let mut log = Log(Vec::new());
    let (mut genres, mut minds, mut fabric, mut defs) = (Genres::new(), Minds::new(), Fabric::new(), Defenses::new());
    let (mut culture, mut projects, mut chem, mut why) =
        (Culture::new(), Projects::new(), Chemistry::new(), ExplainTracer::new());
    let mut decisions: Vec<String> = Vec::new();
    let mut transcript: Vec<String> = Vec::new();
    let role = |id: &str| st.roles.iter().find(|r| r.0 == id).map(|r| r.1).unwrap_or("someone");
    let g = st.genre;
    let [goods, crew, record_noun, qual] = st.nouns;

    // 1. Setup, from the genre's own words.
    let [sev, con, acc, priv_] = st.words;
    let r = run!(
        log,
        genres,
        format!(
            r#"{{"op":"normalize","args":{{"genre":"{g}","keys":{KEYS},"value":{{"severity":"{sev}","contract":"{con}","access":"{acc}","privacy":"{priv_}"}}}}}}"#
        )
    );
    let engine = r[0].get("value").unwrap().clone();
    decisions.push(format!("engine words {}", engine.to_json()));
    let (sev, con) = (s(&engine, "severity"), s(&engine, "contract"));

    if st.define_mind {
        run!(
            log,
            minds,
            format!(
                r#"{{"op":"define","args":{{"id":"{}","memory":"individual","affect":["shock"],"language":"full","deception":"concealment","theory_of_mind":2,"time_scale":100000}}}}"#,
                st.mind
            )
        );
    }
    let r = run!(
        log,
        minds,
        format!(r#"{{"op":"assign","args":{{"being":"steward","mind":"{}"}}}}"#, st.mind),
        r#"{"op":"admit","args":{"being":"steward","kind":"cueing","op":"cover","args":{"being":"steward","id":"c1","fact":"camera_record","lie":"The cameras were offline."}}}"#,
        r#"{"op":"admit","args":{"being":"steward","kind":"cueing","op":"episode","args":{"being":"steward","id":"count_short","features":{"actions":["count"],"entities":["crate"],"tags":["stores"]},"valence":-20,"emotion":"fear"}}}"#,
        r#"{"op":"admit","args":{"being":"steward","kind":"cueing","op":"frame_facts","args":{"being":"steward","listener":"crew_a","facts":["ration_count"]}}}"#,
    );
    let (cover, episode, frame) = (&r[1], &r[2], &r[3]);
    decisions.push(format!(
        "steward: cover allowed {} {}; episode stripped {}; frame allowed {}",
        s(cover, "allowed"),
        s(cover, "reasons"),
        s(episode, "stripped"),
        s(frame, "allowed")
    ));
    assert_eq!(s(cover, "allowed"), "false", "a concealment-only mind cannot tell a cover story");
    assert!(s(cover, "reasons").contains("cannot_lie"));
    assert!(s(episode, "stripped").contains("emotion"), "the steward feels no fear: {}", episode.to_json());
    assert_eq!(s(frame, "allowed"), "true");

    run!(
        log,
        fabric,
        format!(r#"{{"op":"norm","args":{{"id":"ration_rule","severity":"{sev}","report_to":"quartermaster"}}}}"#),
        r#"{"op":"meet","args":{"a":"supervisor","b":"mechanic","episode":"posting","summary":"assigned to the same crew"}}"#,
        r#"{"op":"breach","args":{"norm":"ration_rule","offender":"mechanic","witness":"supervisor","conscience":70}}"#,
        format!(
            r#"{{"op":"contract","args":{{"id":"camera_hold","kind":"{con}","debtor":"steward","creditor":"supervisor","terms":"the camera record goes to the supervisor only"}}}}"#
        ),
    );
    let r = run!(
        log,
        projects,
        r#"{"op":"propose","args":{"id":"certification","owner":"mechanic","aim":"earn the full certification","stake":30,"horizon":1000,"milestones":[{"id":"basics","condition":{"kind":"task","ref":"basics"},"expected":40},{"id":"practical","condition":{"kind":"task","ref":"practical"},"expected":40},{"id":"review","condition":{"kind":"relationship","ref":"supervisor","level":60},"expected":20,"alternative":{"id":"review_retake","condition":{"kind":"relationship","ref":"supervisor","level":60},"expected":60}},{"id":"final","condition":{"kind":"task","ref":"final"},"expected":30}]}}"#,
        r#"{"op":"outcome","args":{"project":"certification","milestone":"basics","success":true}}"#,
        r#"{"op":"outcome","args":{"project":"certification","milestone":"practical","success":true}}"#,
        r#"{"op":"show","args":{"project":"certification"}}"#,
    );
    assert_eq!(s(&r[3], "remaining"), r#"["review","final"]"#, "two milestones from done: {}", r[3].to_json());
    run!(
        log,
        chem,
        r#"{"op":"style","args":{"being":"friend","humour":"dry","tempo":"measured","formality":30,"assertiveness":40,"values":["loyalty"]}}"#,
        r#"{"op":"style","args":{"being":"mechanic","humour":"dry","tempo":"measured","formality":40,"assertiveness":50,"values":["loyalty"]}}"#,
        r#"{"op":"history","args":{"a":"friend","b":"mechanic","landmarks":2,"favors":1}}"#,
        format!(
            r#"{{"op":"joke","args":{{"a":"friend","b":"mechanic","id":"first_shift","moment":"{}"}}}}"#,
            st.moment
        ),
        r#"{"op":"scene","args":{"id":"questioning","silence":3}}"#,
    );
    transcript.push(format!(
        "{} logs a missing {goods}; {} took it for a sick friend; the {qual} is two milestones from done.",
        role("steward"),
        role("mechanic")
    ));

    // 2. Pressure: questioned in front of the crew.
    let mut pacing = vec![beat(&mut log, &mut chem, "conflict", 40, 80, 70, 2)];
    let r = run!(
        log,
        defs,
        r#"{"op":"profile","args":{"being":"mechanic","maturity":50,"tolerance":20,"crisis":"confession","style":["rationalization","displacement"]}}"#,
        format!(
            r#"{{"op":"threat","args":{{"being":"mechanic","value":"competence","drop":60,"fact":"took_the_crate","excuse":"{}"}}}}"#,
            st.excuse
        ),
    );
    let first = r[1].clone();
    decisions.push(format!(
        "mechanic: {} ({}), strain {}",
        s(&first, "mechanism"),
        s(&first, "effect"),
        n(&first, "strain")
    ));
    assert_eq!(s(&first, "mechanism"), "rationalization", "first, an excuse");
    why.workflow(
        &parse(
            &format!(
                r#"{{"ops":[{}]}}"#,
                trace(
                    "mechanic",
                    "rationalization",
                    1,
                    ("reaction", "protect_competence", n(&first, "kept") / 10_000),
                    &s(&first, "explain"),
                    "questioned in front of the others"
                )
            ),
            1 << 16,
        )
        .unwrap(),
    )
    .unwrap();
    transcript.push(format!("{}: \"{}.\" ({})", role("mechanic"), st.excuse, s(&first, "effect")));

    // The supervisor contradicts the excuse; the exposed truth threatens belonging.
    let r = run!(
        log,
        defs,
        r#"{"op":"tick","args":{"now":2}}"#,
        r#"{"op":"contradict","args":{"being":"mechanic","fact":"took_the_crate","drop":40}}"#,
        r#"{"op":"threat","args":{"being":"mechanic","value":"belonging","drop":50,"fact":"crew_saw_it","safer_target":"cadet","blame":"cadet"}}"#,
    );
    let second = r[2].clone();
    decisions.push(format!(
        "contradiction distorted {}; then {} at {} ({}), band {}, strain {}",
        s(&r[1], "distorted"),
        s(&second, "mechanism"),
        s(&second, "distortion"),
        s(&second, "effect"),
        s(&second, "band"),
        n(&second, "strain")
    ));
    assert_eq!(s(&second, "mechanism"), "displacement", "then anger at someone safer");
    assert!(s(&second, "distortion").contains("cadet"));
    assert!(n(&second, "strain") > n(&first, "strain"));
    why.workflow(
        &parse(
            &format!(
                r#"{{"ops":[{}]}}"#,
                trace(
                    "mechanic",
                    "displacement",
                    2,
                    ("reaction", "protect_belonging", n(&second, "kept") / 10_000),
                    &s(&second, "explain"),
                    "the supervisor read out the log"
                )
            ),
            1 << 16,
        )
        .unwrap(),
    )
    .unwrap();
    transcript.push(format!("{} snaps at {} ({}).", role("mechanic"), role("cadet"), s(&second, "effect")));
    pacing.push(beat(&mut log, &mut chem, "conflict", 20, 60, 80, 2));

    // The steward conceals the camera record from the crew, not from the supervisor.
    let r = run!(log, fabric, r#"{"op":"ledger","args":{"a":"steward","b":"supervisor"}}"#,);
    let owes = format!("{} on {}", s(&r[0], "a_owes_b"), s(&r[0], "open"));
    let crew_facts = r#"["ration_count"]"#;
    let boss_facts = r#"["camera_record","ration_count"]"#;
    decisions.push(format!("steward owes {owes}; crew hears {crew_facts}; supervisor hears {boss_facts}"));
    assert_eq!(owes, r#"20 on ["camera_hold"]"#, "{}", r[0].to_json());
    why.workflow(
        &parse(
            &format!(
                r#"{{"ops":[{}]}}"#,
                trace(
                    "steward",
                    "withhold_camera_record",
                    2,
                    ("promise", "camera_hold", 80),
                    "the record goes to the supervisor only",
                    "the others asked what the record showed"
                )
            ),
            1 << 16,
        )
        .unwrap(),
    )
    .unwrap();
    transcript.push(format!(
        "{} tells the {crew} only the count; the {record_noun} goes to {}.",
        role("steward"),
        role("supervisor")
    ));

    // 3. Culture: by the next shift the story has been retold four times.
    let member = |b: &str, status: u64, openness: u64, care: u64| {
        format!(
            r#"{{"op":"member","args":{{"being":"{b}","community":"crew","status":{status},"openness":{openness},"carefulness":{care},"credulity":100}}}}"#
        )
    };
    run!(
        log,
        culture,
        member("cadet", 50, 80, 0),
        member("crew_a", 50, 80, 0),
        member("crew_b", 50, 80, 0),
        member("crew_c", 50, 80, 100),
        member("crew_d", 50, 10, 0),
        member("chief", 95, 50, 50),
        format!(
            r#"{{"op":"idiom","args":{{"id":"ration_thief","community":"crew","phrase":"{}","meaning":"someone who takes from the shared stores","prestige":60}}}}"#,
            st.slang
        ),
        r#"{"op":"tick","args":{"now":12}}"#,
        r#"{"op":"seed","args":{"topic":"missing_ration","witness":"cadet","verb":"took","agent":"mechanic","place":"stores","details":["for_a_sick_friend","late_at_night"],"amount":1}}"#,
    );
    let mut tells = Vec::new();
    for (spk, lis, seed) in
        [("cadet", "crew_a", 2), ("crew_a", "crew_b", 1), ("crew_b", "crew_c", 2), ("crew_c", "crew_d", 0)]
    {
        tells.push(format!(r#"{{"op":"tell","args":{{"speaker":"{spk}","listener":"{lis}","topic":"missing_ration","seed":{seed},"trust":80}}}}"#));
    }
    tells.push(r#"{"op":"belief","args":{"being":"crew_d","topic":"missing_ration"}}"#.into());
    let req = parse(&format!(r#"{{"ops":[{}]}}"#, tells.join(",")), 1 << 20).unwrap();
    let out = culture.workflow(&req).unwrap();
    log.0.push(out.to_json());
    let r = out.get("results").and_then(Value::as_array).unwrap().to_vec();
    let heard = &r[4];
    let mutations: Vec<String> = r[..4].iter().map(|t| s(t, "mutated")).collect();
    decisions.push(format!(
        "rumor after 4 hops: {} crates, details {}, distance {}, mutations {}",
        n(heard, "amount"),
        s(heard, "details"),
        s(heard, "distance"),
        mutations.join(",")
    ));
    assert_eq!(n(heard, "hops"), 4);
    assert_eq!(n(heard, "amount"), 3, "the crate became three crates");
    assert!(!s(heard, "details").contains("sick_friend"), "the sick friend was dropped");
    assert_eq!(s(heard, "distance"), r#"{"drift":2000000,"ops":3}"#);
    transcript.push(format!(
        "By the next shift, the fourth teller has heard that {} took three {goods}s late at night.",
        role("mechanic")
    ));

    // The chief's slang spreads to the open-minded crew, not to the closed one.
    let mut ops = Vec::new();
    for _ in 0..9 {
        for l in ["crew_a", "crew_b", "crew_d"] {
            ops.push(format!(
                r#"{{"op":"expose","args":{{"listener":"{l}","speaker":"chief","idiom":"ration_thief"}}}}"#
            ));
        }
    }
    for l in ["crew_a", "crew_b", "crew_d"] {
        ops.push(format!(r#"{{"op":"speech","args":{{"being":"{l}"}}}}"#));
    }
    let r = run!(log, culture, ops.join(","));
    let users: Vec<bool> = r[27..].iter().map(|sp| s(sp, "usable").contains("ration_thief")).collect();
    decisions.push(format!("slang usable by crew_a, crew_b, crew_d: {users:?}"));
    assert_eq!(users, [true, true, false], "the slang spreads from a prestigious speaker to open listeners");
    transcript.push(format!("The chief's \"{}\" is on the lips of two of the {crew}, not the third.", st.slang));

    // 4. Resolution: support, a confession, a lighter sanction, an adapted project, a callback.
    let r = run!(
        log,
        defs,
        r#"{"op":"tick","args":{"now":14}}"#,
        r#"{"op":"relieve","args":{"being":"mechanic","kind":"support","amount":80}}"#,
        r#"{"op":"accept","args":{"being":"mechanic","fact":"crew_saw_it"}}"#,
        r#"{"op":"accept","args":{"being":"mechanic","fact":"took_the_crate"}}"#,
        r#"{"op":"status","args":{"being":"mechanic"}}"#,
    );
    let calm = r[4].clone();
    assert!(n(&r[1], "strain") < n(&second, "strain"), "support lowers strain");
    assert_eq!(s(&calm, "distortions"), "[]");
    pacing.push(beat(&mut log, &mut chem, "reveal", -30, 25, 70, 1));
    why.workflow(
        &parse(
            &format!(
                r#"{{"ops":[{}]}}"#,
                trace(
                    "mechanic",
                    "confess",
                    14,
                    ("goal", "make_it_right", 70),
                    "a trusted friend stood by them",
                    "the friend said they would stay"
                )
            ),
            1 << 16,
        )
        .unwrap(),
    )
    .unwrap();
    transcript.push(format!(
        "With the friend beside them, {} confesses: the {goods} was for a sick friend.",
        role("mechanic")
    ));

    // The confession is a mature path: the sanction is one step lower than it would have been.
    let mut without = fabric.clone();
    let req = parse(r#"{"ops":[{"op":"breach","args":{"norm":"ration_rule","offender":"mechanic","witness":"supervisor","conscience":70}}]}"#, 1 << 16).unwrap();
    let unconfessed = without.workflow(&req).unwrap();
    let r = run!(
        log,
        fabric,
        r#"{"op":"apologize","args":{"norm":"ration_rule","offender":"mechanic","witness":"supervisor"}}"#,
        r#"{"op":"breach","args":{"norm":"ration_rule","offender":"mechanic","witness":"supervisor","conscience":70}}"#,
        r#"{"op":"dyad","args":{"a":"supervisor","b":"mechanic"}}"#,
    );
    let step = n(&r[1], "step");
    let would = n(&unconfessed.get("results").and_then(Value::as_array).unwrap()[0], "step");
    decisions.push(format!("sanction step {step} {} (without the confession: {would})", s(&r[1], "sanctions")));
    assert_eq!(step, would - 1, "one step lower for the confession");
    let address = s(&r[2], "address_a_to_b");
    why.workflow(
        &parse(
            &format!(
                r#"{{"ops":[{}]}}"#,
                trace(
                    "supervisor",
                    "verbal_warning",
                    14,
                    ("routine", "ration_rule", 60),
                    "a confessed breach steps one rung down the ladder",
                    "the mechanic confessed"
                )
            ),
            1 << 16,
        )
        .unwrap(),
    )
    .unwrap();
    let sanction = s(&r[1], "sanctions").replace(['[', ']', '"'], "").replace('_', " ");
    transcript.push(format!(
        "{} gives a {sanction}: step {step}, where it would have been step {would} without the confession.",
        role("supervisor")
    ));
    pacing.push(beat(&mut log, &mut chem, "line", -10, 20, 60, 1));

    let r = run!(
        log,
        projects,
        r#"{"op":"tick","args":{"now":14}}"#,
        r#"{"op":"setback","args":{"project":"certification","severity":100}}"#,
        r#"{"op":"show","args":{"project":"certification"}}"#,
    );
    decisions.push(format!(
        "project: {} (replaced {}), status {}, current {}",
        s(&r[1], "decision"),
        s(&r[1], "replaced"),
        s(&r[2], "status"),
        s(&r[2], "current")
    ));
    assert_eq!(s(&r[1], "decision"), "adapt", "the certification adapts: {}", r[1].to_json());
    assert_eq!(s(&r[2], "status"), "active", "not abandoned");
    why.workflow(
        &parse(
            &format!(
                r#"{{"ops":[{}]}}"#,
                trace(
                    "mechanic",
                    "retake_review",
                    14,
                    ("goal", "project:certification", n(&r[2], "score")),
                    "the review moves to a retake and is not given up",
                    "the supervisor's warning"
                )
            ),
            1 << 16,
        )
        .unwrap(),
    )
    .unwrap();
    transcript.push(format!("The {qual} review becomes a retake; {} keeps going.", role("mechanic")));
    pacing.push(beat(&mut log, &mut chem, "action", -10, 15, 50, 1));

    let r = run!(
        log,
        chem,
        r#"{"op":"banter","args":{"speaker":"friend","listener":"mechanic","mood":20,"pacing":"steady","seed":7}}"#,
    );
    let callback = r[0].clone();
    decisions.push(format!(
        "friend: {} on {} ({}), chemistry {}",
        s(&callback, "move"),
        callback.get("joke").map(|j| format!("{} (use {})", s(j, "id"), s(j, "uses"))).unwrap_or_default(),
        s(&callback, "appraisal"),
        n(&callback, "chemistry")
    ));
    assert_eq!(s(&callback, "move"), "callback");
    assert_eq!(s(&callback, "appraisal"), "playful", "the dry humour lands");
    why.workflow(
        &parse(
            &format!(
                r#"{{"ops":[{}]}}"#,
                trace(
                    "friend",
                    "callback",
                    14,
                    ("reaction", "first_shift", n(&callback, "chemistry")),
                    "a fresh running joke with someone who gets it",
                    "the mechanic finally exhaled"
                )
            ),
            1 << 16,
        )
        .unwrap(),
    )
    .unwrap();
    transcript.push(format!("The friend: \"Still better than {}.\" ({} laughs.)", st.moment, role("mechanic")));
    pacing.push(beat(&mut log, &mut chem, "line", -10, 10, 30, 0));
    let r = run!(
        log,
        chem,
        r#"{"op":"banter","args":{"speaker":"friend","listener":"mechanic","mood":40,"pacing":"end"}}"#,
    );
    decisions.push(format!("pacing {}; then {}", pacing.join(","), s(&r[0], "move")));
    assert_eq!(pacing.last().unwrap(), "end", "the pacing controller ends the scene on the callback: {pacing:?}");
    assert!(pacing[..pacing.len() - 1].iter().all(|a| a != "end"));
    assert_eq!(s(&r[0], "move"), "exit");
    transcript.push("Scene ends.".into());

    // Every decision is explained.
    let mut explain = Vec::new();
    for being in ["steward", "mechanic", "supervisor", "friend"] {
        let r = run!(log, why, format!(r#"{{"op":"query_trace","args":{{"being":"{being}","limit":8}}}}"#));
        for t in r[0].get("traces").and_then(Value::as_array).unwrap().iter().rev() {
            explain.push(s(t, "why"));
        }
    }

    // The decisions go back out in the genre's words.
    let record = format!(
        r#"{{"severity":"{sev}","contract":"{con}","access":"{}","privacy":"{}","address":"{address}"}}"#,
        s(&engine, "access"),
        s(&engine, "privacy")
    );
    let r = run!(log, genres, format!(r#"{{"op":"render","args":{{"genre":"{g}","keys":{KEYS},"value":{record}}}}}"#));
    let words = r[0].get("value").unwrap().clone();
    let transcript = transcript.into_iter().map(|l| l[..1].to_uppercase() + &l[1..]).collect();
    Played { decisions, words, explain, transcript, log: log.0 }
}

#[test]
fn missing_ration_plays_like_people_in_both_genres() {
    for st in [&STATION, &VILLAGE] {
        let p = scene(st);
        assert_eq!(p.explain.len(), 7, "{}: every decision has a trace: {:?}", st.genre, p.explain);
        assert!(p.explain.iter().any(|w| w.starts_with("mechanic did rationalization because")));
        assert!(p.explain.iter().any(|w| w.starts_with("mechanic did displacement because")));
        assert!(p.explain.iter().any(
            |w| w.starts_with("steward did withhold_camera_record because the record goes to the supervisor only")
        ));
        assert!(p.explain.iter().any(|w| w.starts_with("friend did callback because")));
        assert!(p.transcript.len() >= 8);
    }
}

#[test]
fn missing_ration_same_decisions_different_words() {
    let (a, b) = (scene(&STATION), scene(&VILLAGE));
    assert_eq!(a.decisions, b.decisions, "the two genres must decide the same");
    assert_eq!(a.explain, b.explain);
    assert_eq!(
        a.words.to_json(),
        r#"{"severity":"regulation","contract":"sealed_secret","access":"sealed","privacy":"common","address":"spacer"}"#
    );
    assert_eq!(
        b.words.to_json(),
        r#"{"severity":"custom","contract":"keep_secret","access":"restricted","privacy":"public","address":"acquaintance"}"#
    );
    assert_ne!(a.transcript, b.transcript, "the words differ");
}

#[test]
fn missing_ration_replays_byte_for_byte() {
    for st in [&STATION, &VILLAGE] {
        let (a, b) = (scene(st), scene(st));
        assert_eq!(a.log, b.log);
        assert_eq!(a.transcript, b.transcript);
    }
}

#[test]
#[ignore]
fn print_missing_ration() {
    for st in [&STATION, &VILLAGE] {
        let p = scene(st);
        println!("== {}", st.genre);
        p.transcript.iter().for_each(|x| println!("T {x}"));
        p.explain.iter().for_each(|x| println!("E {x}"));
        p.decisions.iter().for_each(|x| println!("D {x}"));
    }
}
