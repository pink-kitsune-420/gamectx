//! Pass 3 acceptance scene, "The Interrupted Accusation" (docs/research pass 3, section 7).
//!
//! Mira the baker is exhausted, one tick from burning her bread, fond of her apprentice and freshly
//! hurt: she found a flour trail by the lockbox and her coin pouch gone. The player walks in with
//! the firewood she asked for while a gossip stands in earshot and asks, in the same breath,
//! whether the apprentice stole from her. The scene checks that every pass 3 engine makes the
//! choice a real person would, that each choice can be explained, and that the whole scene
//! replays byte for byte. Pure host computation: no model, $0.

use gamectx::character::embodiment::Embodiment;
use gamectx::character::explain::ExplainTracer;
use gamectx::character::frame::Frame;
use gamectx::character::pragmatics::Pragmatics;
use gamectx::data::json::{parse, Value};
use gamectx::knowledge::fabric::Fabric;
use gamectx::knowledge::surroundings::Surroundings;

/// Every engine's raw JSON output, in order, plus the results of the last call.
struct Log(Vec<String>);

macro_rules! run {
    ($log:expr, $m:expr, $($op:expr),+ $(,)?) => {{
        let ops: Vec<String> = vec![$($op.to_string()),+];
        let req = parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 20).unwrap();
        let out = $m.workflow(&req).unwrap();
        $log.0.push(out.to_json());
        out.get("results").and_then(Value::as_array).unwrap().to_vec()
    }};
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).map(Value::to_json).unwrap_or_default()
}

fn scene() -> Vec<String> {
    let mut log = Log(Vec::new());
    let mut world = Surroundings::new();
    let mut society = Fabric::new();
    let mut frame = Frame::new();
    let mut talk = Pragmatics::new();
    let mut body = Embodiment::new();
    let mut why = ExplainTracer::new();

    // 1. Setup: the bakery by day, the flour trail by the lockbox, the missing pouch, the promise.
    let r = run!(
        log,
        world,
        r#"{"op":"environment","args":{"day_length":240,"phases":[{"id":"dawn","start":0,"end":30,"light":"dim"},{"id":"day","start":30,"end":190,"light":"bright"},{"id":"night","start":190,"end":240,"light":"dark","temp_delta":-15}]}}"#,
        r#"{"op":"place","args":{"id":"bakery","owner":"mira","access":"customers_only","privacy":"semi_private","capacity":4,"stations":[{"id":"oven"}]}}"#,
        r#"{"op":"trace_kind","args":{"id":"spilled_flour_trail","created_by":["drop_sack"],"cleared_by":["sweep"],"visibility":"public","ttl":60,"implies":"flour_spilled_recently","sight":"White flour is dusted across the floor by the lockbox."}}"#,
        r#"{"op":"item","args":{"id":"coin_pouch","owner":"mira","sentimental_to":["mira"]}}"#,
        r#"{"op":"tick","args":{"now":40}}"#,
        r#"{"op":"act","args":{"action":"drop_sack","place":"bakery"}}"#,
        r#"{"op":"transfer","args":{"item":"coin_pouch","how":"take","to":"apprentice"}}"#,
        r#"{"op":"tick","args":{"now":44}}"#,
        r#"{"op":"enter","args":{"place":"bakery","being":"mira"}}"#,
        r#"{"op":"reserve","args":{"place":"bakery","station":"oven","being":"mira"}}"#,
        r#"{"op":"perceive","args":{"place":"bakery"}}"#,
    );
    // Mira infers that flour was spilled; the trace never says who did it.
    let seen = r[10].to_json();
    assert!(seen.contains(r#""belief":"inferred","implies":"flour_spilled_recently""#), "{seen}");
    assert!(!seen.contains("apprentice"));
    assert!(r[6].to_json().contains(r#""status":"stolen""#));
    run!(
        log,
        society,
        r#"{"op":"norm","args":{"id":"no_theft","severity":"crime","report_to":"town_watch"}}"#,
        r#"{"op":"contract","args":{"id":"firewood","kind":"promise","debtor":"player","creditor":"mira","terms":"Bring a bundle of firewood for the oven","due_at":60}}"#,
        r#"{"op":"meet","args":{"a":"mira","b":"apprentice","episode":"hired","summary":"She took him in off the street and taught him to knead."}}"#,
    );

    // 2. The compound turn under pressure: the player and the gossip come in.
    let r = run!(
        log,
        world,
        r#"{"op":"tick","args":{"now":45}}"#,
        r#"{"op":"enter","args":{"place":"bakery","being":"player","customer":true}}"#,
        r#"{"op":"enter","args":{"place":"bakery","being":"gossip","customer":true}}"#,
        r#"{"op":"conditions","args":{"place":"bakery"}}"#,
    );
    let room = r[3].get("place").unwrap().clone();
    assert_eq!(s(&room, "discuss_secrets"), "false", "three in a semi-private shop: not the place for secrets");
    assert_eq!(s(&room, "earshot"), r#""nearby""#);
    let r = run!(
        log,
        talk,
        r#"{"op":"parse","args":{"text":"Here is the firewood you asked for—also, is it true the apprentice stole from you?","speaker":"player","listener":"mira"}}"#,
        r#"{"op":"stance","args":{"trust":70,"affection":40,"confidential":true,"bystanders":[{"being":"gossip"}],"urgency":90}}"#,
        r#"{"op":"agenda","args":{"clauses":[{"index":0,"act":"offer_or_transfer","subtext":"sincere"},{"index":1,"act":"question_fact","subtext":"probing"}],"motive":{"id":"pull_bread_from_oven","score":95}}}"#,
        r#"{"op":"open","args":{"being":"mira","id":"theft","topic":"apprentice_theft","speaker":"player","question":"Is it true the apprentice stole from you?"}}"#,
    );
    let clauses = r[0].get("clauses").and_then(Value::as_array).unwrap();
    assert_eq!(clauses.len(), 2);
    assert_eq!(s(&clauses[0], "act"), r#""offer_or_transfer""#);
    assert_eq!(s(&clauses[0], "update"), r#""receive_item""#);
    assert_eq!(s(&clauses[1], "subtext"), r#""probing""#);
    // Trusted player, untrusted gossip in earshot: whisper, one sentence.
    assert_eq!(s(&r[1], "privacy"), r#""whispered""#);
    assert_eq!(s(&r[1], "max_sentences"), "1");
    // The burning bread overrides polite routing: the oven first, the firewood acknowledged,
    // the theft question deferred.
    assert_eq!(s(&r[2], "mode"), r#""interrupt_override""#);
    assert!(s(&r[2], "steps").starts_with(
        r#"[{"clause":null,"kind":"own","own":"pull_bread_from_oven","what":"urgent"},{"clause":0,"kind":"update","own":null,"what":"receive_item"}"#
    ));
    assert_eq!(s(&r[2], "deferred"), "[0,1]");
    run!(log, society, r#"{"op":"settle","args":{"id":"firewood","outcome":"fulfilled"}}"#);

    // Salience: out of 50 stored items, the five that matter now are kept.
    let mut items = vec![
        r#"{"id":"bread_burning","bytes":40,"urgency":100,"mandatory":true,"tags":["oven"]}"#.to_string(),
        r#"{"id":"firewood_promise","bytes":40,"entities":["player"],"tags":["firewood"]}"#.into(),
        r#"{"id":"theft_suspicion","bytes":60,"tags":["theft","coin_pouch"],"entities":["apprentice"]}"#.into(),
        r#"{"id":"apprentice_feelings","bytes":50,"entities":["apprentice"],"polarity":"grievance"}"#.into(),
        r#"{"id":"gossip_in_earshot","bytes":30,"entities":["gossip"]}"#.into(),
    ];
    items.extend((0..45).map(|i| format!(r#"{{"id":"lore_{i:02}","bytes":80,"tags":["biography"]}}"#)));
    let r = run!(
        log,
        frame,
        format!(
            r#"{{"op":"rank","args":{{"budget":400,"listener":"player","earshot":["gossip"],"cues":{{"entities":["apprentice"],"tags":["firewood","theft"]}},"mood":{{"valence":-10,"resentment":50}},"items":[{}]}}}}"#,
            items.join(",")
        ),
    );
    let kept = r[0].get("kept").and_then(Value::as_array).unwrap();
    let ids: Vec<String> = kept.iter().map(|k| s(k, "id").trim_matches('"').to_string()).collect();
    for want in ["bread_burning", "firewood_promise", "theft_suspicion", "apprentice_feelings", "gossip_in_earshot"] {
        assert!(ids.iter().any(|x| x == want), "{want} missing from {ids:?}");
    }
    assert!(ids.iter().all(|x| !x.starts_with("lore_")), "{ids:?}");
    assert_eq!(r[0].get("dropped").and_then(Value::as_array).unwrap().len(), 45);

    // Embodied action: fond but hurt, she keeps working, watches the gossip and speaks in one
    // clipped, breathless, whispered sentence.
    let r = run!(
        log,
        body,
        r#"{"op":"tension","args":{"being":"mira","attitudes":[{"toward":"apprentice","affection":55,"resentment":50}]}}"#,
        r#"{"op":"react","args":{"being":"mira","task":"urgent","bystander_untrusted":true,"cooperativeness":"cooperative"}}"#,
        r#"{"op":"voice","args":{"fatigue":75,"task_urgency":90,"exertion":70,"affection":40,"privacy":"whispered"}}"#,
    );
    assert_eq!(s(&r[0], "mode"), r#""bittersweet_warmth""#);
    assert_eq!(s(&r[1], "hand"), r#""continues_working""#);
    assert_eq!(s(&r[1], "gaze"), r#""watches_bystander""#);
    assert_eq!(s(&r[2], "max_sentences"), "1");
    assert_eq!(s(&r[2], "pacing"), r#""breathless""#);
    assert_eq!(s(&r[2], "volume"), "400");
    let r = run!(
        log,
        why,
        r#"{"op":"record_trace","args":{"being":"mira","action":"pull_bread_from_oven","at":45,"perceptions":[{"source":"smell","text":"the crust is about to burn"},{"source":"sight","text":"the gossip is listening"}],"motive":{"id":"pull_bread_from_oven","kind":"task","score":95,"reason":"the bread burns in one tick","runner_up":{"id":"answer_player","score":60}}}}"#,
        r#"{"op":"query_trace","args":{"being":"mira","limit":1}}"#,
    );
    let because = r[1].to_json();
    assert!(because.contains("mira did pull_bread_from_oven because the bread burns in one tick"), "{because}");

    // 3. Interruption: a crash outside. Then the gossip leaves and the shop is quiet.
    let r = run!(
        log,
        talk,
        r#"{"op":"tick","args":{"now":46}}"#,
        r#"{"op":"interrupt","args":{"being":"mira","cause":"crash_outside"}}"#,
    );
    assert!(r[1].to_json().contains("interrupted"), "{}", r[1].to_json());
    run!(
        log,
        body,
        r#"{"op":"tick","args":{"now":46}}"#,
        r#"{"op":"react","args":{"being":"mira","shock":60,"task":"active"}}"#
    );
    let r = run!(
        log,
        world,
        r#"{"op":"tick","args":{"now":50}}"#,
        r#"{"op":"leave","args":{"place":"bakery","being":"gossip"}}"#,
        r#"{"op":"conditions","args":{"place":"bakery"}}"#,
    );
    assert_eq!(s(r[2].get("place").unwrap(), "discuss_secrets"), "true", "alone with the player now");

    // 4. Resumption: she brings the theft back up herself, openly and with felt ambivalence.
    let r = run!(
        log,
        talk,
        r#"{"op":"tick","args":{"now":50}}"#,
        r#"{"op":"stance","args":{"trust":70,"affection":40,"confidential":true,"bystanders":[]}}"#,
        r#"{"op":"resume","args":{"being":"mira"}}"#,
    );
    assert_eq!(s(&r[1], "privacy"), r#""open""#);
    let back = r[2].to_json();
    assert!(back.contains(r#""unanswered":"Is it true the apprentice stole from you?""#), "{back}");
    let r = run!(
        log,
        body,
        r#"{"op":"tension","args":{"being":"mira","attitudes":[{"toward":"apprentice","affection":55,"resentment":50}]}}"#,
        r#"{"op":"voice","args":{"fatigue":75,"affection":40,"tension":"bittersweet_warmth"}}"#,
    );
    assert_eq!(s(&r[0], "mode"), r#""bittersweet_warmth""#);
    assert!(s(&r[0], "effects").contains(r#""tone":"tender""#));
    assert_eq!(s(&r[1], "max_sentences"), "4", "no longer rushed, she can say more");
    log.0
}

#[test]
fn interrupted_accusation_plays_like_a_person() {
    let log = scene();
    assert!(log.len() >= 10);
}

#[test]
fn interrupted_accusation_replays_byte_for_byte() {
    assert_eq!(scene(), scene());
}
