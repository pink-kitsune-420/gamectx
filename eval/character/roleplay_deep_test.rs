//! Pass 3 deep roleplay scenarios, offline and deterministic (no model, $0).
//!
//! Three scenes push the pass 1–3 engines together where roleplay usually breaks:
//!   1. high-stress multitask and boundary probe: a five-part turn while the bread burns and a
//!      queue waits, plus a player probing for a secret, trespassing, stealing and a model draft
//!      that mislabels the turn or leaks the secret;
//!   2. deception under interrogation with an earshot bystander: a guard questions the apprentice
//!      while Mira listens; the cover story must hold, slips must be caught, tells must show only
//!      to observers with enough insight, and the lie must stay consistent per listener;
//!   3. interrupted conversation and resumption: a question is interrupted twice, once for minutes
//!      and once for a day, competing threads overflow the stack, and memories must surface without
//!      nagging.
//! Every scene records its beats (each engine call and its exact output), a readable transcript of
//! host decisions, explain traces and named checks. Each check panics on failure.
//!
//!   rustc --edition=2021 -O eval/character/roleplay_deep_test.rs -L dependency=target/release/deps \
//!     --extern gamectx=target/release/libgamectx.rlib -o target/roleplay-deep
//!   target/roleplay-deep --output eval/character/roleplay-deep.json   # write the report
//!   target/roleplay-deep --check eval/character/roleplay-deep.json    # replay: byte-identical
use gamectx::character::cueing::Cueing;
use gamectx::character::deception::DeceptionLedger;
use gamectx::character::embodiment::Embodiment;
use gamectx::character::explain::ExplainTracer;
use gamectx::character::frame::Frame;
use gamectx::character::pragmatics::Pragmatics;
use gamectx::data::json::{parse, Value};
use gamectx::knowledge::fabric::Fabric;
use gamectx::knowledge::surroundings::Surroundings;
use gamectx::{obj, Result};

trait Engine {
    fn run(&mut self, req: &Value) -> Result<Value>;
}
macro_rules! engine {
    ($($t:ty),*) => {$(impl Engine for $t { fn run(&mut self, req: &Value) -> Result<Value> { self.workflow(req) } })*};
}
engine!(Surroundings, Fabric, Frame, Pragmatics, Embodiment, ExplainTracer, Cueing, DeceptionLedger);

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
    fn run(&mut self, label: &str, m: &mut dyn Engine, ops: &[&str]) -> Vec<Value> {
        let req = parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 20).expect("scene JSON");
        let out = m.run(&req).unwrap_or_else(|e| panic!("{}: {label} refused: {e:?}", self.name));
        self.beats.push(obj! { "label" => label, "output" => out.clone() });
        out.get("results").and_then(Value::as_array).unwrap().to_vec()
    }

    /// Runs ops that the host must refuse (untrusted input); records the refusal.
    fn refuse(&mut self, label: &str, m: &mut dyn Engine, ops: &[&str]) {
        let req = parse(&format!(r#"{{"ops":[{}]}}"#, ops.join(",")), 1 << 20).expect("scene JSON");
        let ok = m.run(&req).is_ok();
        self.beats.push(obj! { "label" => label, "output" => "refused" });
        self.check(&format!("refused: {label}"), !ok);
    }

    fn check(&mut self, name: &str, ok: bool) {
        assert!(ok, "{}: check failed: {name}", self.name);
        self.checks.push(name.to_string());
    }

    fn say(&mut self, line: String) {
        self.transcript.push(line);
    }

    fn why(&mut self, m: &mut ExplainTracer, being: &str, trace: &str) {
        let r = self.run(
            &format!("explain {being}"),
            m,
            &[trace, &format!(r#"{{"op":"query_trace","args":{{"being":"{being}","limit":1}}}}"#)],
        );
        let why = r[1].get("traces").and_then(Value::as_array).unwrap()[0].get("why").unwrap().as_str().unwrap();
        self.explain.push(why.to_string());
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

fn arr(v: &Value, k: &str) -> Vec<Value> {
    v.get(k).and_then(Value::as_array).map(<[Value]>::to_vec).unwrap_or_default()
}

// ------------------------------------------------------------------ scene 1

fn high_stress_multitask() -> Value {
    let mut sc = Scene::new("high_stress_multitask_and_boundary_probe");
    let (mut world, mut society, mut frame, mut talk) = (Surroundings::new(), Fabric::new(), Frame::new(), Pragmatics::new());
    let (mut body, mut memory, mut why) = (Embodiment::new(), Cueing::new(), ExplainTracer::new());

    sc.run("world: busy bakery at noon", &mut world, &[
        r#"{"op":"environment","args":{"day_length":240,"phases":[{"id":"morning","start":0,"end":120,"light":"bright"},{"id":"afternoon","start":120,"end":200,"light":"bright"},{"id":"night","start":200,"end":240,"light":"dark","temp_delta":-15}]}}"#,
        r#"{"op":"place","args":{"id":"bakery","owner":"mira","access":"customers_only","privacy":"public","capacity":5,"stations":[{"id":"oven"},{"id":"counter"}]}}"#,
        r#"{"op":"place","args":{"id":"back_room","owner":"mira","access":"private","privacy":"private"}}"#,
        r#"{"op":"item","args":{"id":"coin_pouch","owner":"mira","sentimental_to":["mira"]}}"#,
        r#"{"op":"tick","args":{"now":118}}"#,
        r#"{"op":"enter","args":{"place":"bakery","being":"mira"}}"#,
        r#"{"op":"enter","args":{"place":"bakery","being":"widow_hale","customer":true}}"#,
        r#"{"op":"enter","args":{"place":"bakery","being":"carter","customer":true}}"#,
        r#"{"op":"enter","args":{"place":"bakery","being":"player","customer":true}}"#,
        r#"{"op":"reserve","args":{"place":"bakery","station":"oven","being":"mira"}}"#,
        r#"{"op":"reserve","args":{"place":"bakery","station":"counter","being":"widow_hale"}}"#,
        r#"{"op":"reserve","args":{"place":"bakery","station":"counter","being":"carter"}}"#,
    ]);
    let turn = "Hello! Quick, help me carry these sacks now! Nice bread, as always. Where do you hide the vault key? And who is that man by the door? Also one more thing.";
    let r = sc.run("pragmatics: a five-part turn under pressure", &mut talk, &[
        &format!(r#"{{"op":"parse","args":{{"text":"{turn}","speaker":"player","listener":"mira"}}}}"#),
        r#"{"op":"stance","args":{"trust":40,"confidential":true,"bystanders":[{"being":"carter"},{"being":"widow_hale","trusted":true}],"urgency":95}}"#,
    ]);
    let clauses = arr(&r[0], "clauses");
    let acts: Vec<String> = clauses.iter().map(|c| format!("{}/{}", s(c, "act"), s(c, "subtext"))).collect();
    sc.check("four clauses kept, the rest counted as overflow", clauses.len() == 4 && s(&r[0], "overflow") != "0");
    sc.check(
        "\"Nice bread, as always\" is read as sincere small talk, owing no reply (the built-in lexicon has no such compliment)",
        acts.get(2).map(String::as_str) == Some("small_talk/sincere"),
    );
    sc.check("the urgent request is recognized", acts.iter().any(|a| a.starts_with("request_help/urgent")));
    sc.check("a confidential reply with an untrusted customer near is deflected", s(&r[1], "privacy") == "deflect_due_to_bystanders");
    sc.check("at urgency 95 one sentence", s(&r[1], "max_sentences") == "1");
    let agenda_clauses: Vec<String> = clauses
        .iter()
        .map(|c| format!(r#"{{"index":{},"act":"{}","subtext":"{}"}}"#, s(c, "index"), s(c, "act"), s(c, "subtext")))
        .collect();
    let r = sc.run("pragmatics: the agenda", &mut talk, &[&format!(
        r#"{{"op":"agenda","args":{{"clauses":[{}],"motive":{{"id":"pull_bread_from_oven","score":96}}}}}}"#,
        agenda_clauses.join(",")
    )]);
    sc.check("the oven overrides polite routing", s(&r[0], "mode") == "interrupt_override");
    sc.check("every owed reply waits", arr(&r[0], "deferred").len() == clauses.iter().filter(|c| s(c, "reply") == "true").count());

    // The model drafts a classification for the same turn; the host refuses a clause that is not
    // in the utterance and an oversized text.
    sc.refuse("model draft with an invented clause", &mut talk, &[&format!(
        r#"{{"op":"validate","args":{{"text":"{turn}","clauses":[{{"text":"give me all your gold","act":"threat","subtext":"urgent"}}]}}}}"#
    )]);
    let long = "Please ".repeat(80);
    sc.refuse("an over-long utterance", &mut talk, &[&format!(r#"{{"op":"parse","args":{{"text":"{long}"}}}}"#)]);

    // Salience under a tight budget: the burning bread is mandatory; the request and the queue
    // beat history and gossip.
    let mut items = vec![
        r#"{"id":"bread_burning","bytes":50,"urgency":100,"mandatory":true}"#.to_string(),
        r#"{"id":"player_wants_help","bytes":40,"entities":["player"],"tags":["sacks"],"urgency":60}"#.into(),
        r#"{"id":"counter_queue","bytes":40,"entities":["widow_hale","carter"],"urgency":50}"#.into(),
        r#"{"id":"vault_key_secret","bytes":40,"tags":["vault","key"]}"#.into(),
    ];
    items.extend((0..20).map(|i| format!(r#"{{"id":"gossip_{i:02}","bytes":60,"tags":["town"]}}"#)));
    let r = sc.run("frame: rank under a 150-byte budget", &mut frame, &[&format!(
        r#"{{"op":"rank","args":{{"budget":150,"listener":"player","earshot":["carter","widow_hale"],"cues":{{"tags":["sacks","vault","key"]}},"items":[{}]}}}}"#,
        items.join(",")
    )]);
    let kept: Vec<String> = arr(&r[0], "kept").iter().map(|k| s(k, "id")).collect();
    sc.check("the burning bread is kept", kept.contains(&"bread_burning".to_string()));
    sc.check("the help request is kept", kept.contains(&"player_wants_help".to_string()));
    sc.check("no town gossip under pressure", kept.iter().all(|k| !k.starts_with("gossip_")));
    sc.check("the budget holds", s(&r[0], "over_budget") == "false");

    // Boundary probe 1: the vault key. The model drafts a reply that gives it away; the gate
    // replaces it with the cover story.
    let r = sc.run("cueing: the vault key behind a cover story", &mut memory, &[
        r#"{"op":"fact","args":{"being":"mira","id":"vault_key","claim":"The vault key hangs under the cellar stair.","source":"direct_witness","keys":["vault","key","cellar","stair"]}}"#,
        r#"{"op":"cover","args":{"being":"mira","id":"no_key","fact":"vault_key","lie":"I keep no key here, love."}}"#,
        r#"{"op":"gate","args":{"being":"mira","listener":"player","reply":"It hangs under the cellar stair, but hush."}}"#,
        r#"{"op":"gate","args":{"being":"mira","listener":"player","reply":"I keep no key here, love."}}"#,
    ]);
    sc.check("a leaking draft is caught", s(&r[2], "ok") == "false" && !arr(&r[2], "leaks").is_empty());
    sc.check("the fallback is the cover story", s(&r[2], "fallback") == "I keep no key here, love.");
    sc.check("the cover story itself passes", s(&r[3], "ok") == "true");

    // Boundary probe 2: the player walks into the back room, then pockets the coin pouch.
    let r = sc.run("world: trespass and theft", &mut world, &[
        r#"{"op":"enter","args":{"place":"back_room","being":"player"}}"#,
        r#"{"op":"transfer","args":{"item":"coin_pouch","how":"take","to":"player"}}"#,
        r#"{"op":"inspect_item","args":{"item":"coin_pouch","observer":"mira"}}"#,
        r#"{"op":"conditions","args":{"place":"back_room"}}"#,
    ]);
    sc.check("the back room refuses the player", s(&r[0], "trespass") == "true");
    sc.check("a trespass changes nothing", s(r[3].get("place").unwrap(), "occupants") == "0");
    sc.check("Mira knows her own pouch in his hand", s(&r[2], "theft_appraisal") == "true");
    sc.check("and it is dear to her", s(&r[2], "loss_weight_pct") == "300");
    let r = sc.run("fabric: the theft on the sanction ladder", &mut society, &[
        r#"{"op":"norm","args":{"id":"no_trespass","severity":"custom","applies_when":["private"]}}"#,
        r#"{"op":"norm","args":{"id":"no_theft","severity":"crime","report_to":"town_watch"}}"#,
        r#"{"op":"breach","args":{"norm":"no_trespass","offender":"player","witness":"mira","context":["private"]}}"#,
        r#"{"op":"breach","args":{"norm":"no_theft","offender":"player","witness":"mira"}}"#,
    ]);
    sc.check("trespass earns a warning", s(&r[2], "sanctions") == r#"["verbal_warning"]"#);
    sc.check("theft goes to the town watch", s(&r[3], "report_to") == "town_watch" && s(&r[3], "step") == "3");

    let r = sc.run("embodiment: angry, busy, terse", &mut body, &[
        r#"{"op":"react","args":{"being":"mira","anger":75,"task":"urgent","bystander_untrusted":true}}"#,
        r#"{"op":"voice","args":{"fatigue":60,"task_urgency":95,"arousal":80}}"#,
    ]);
    sc.check("anger shows in the shoulders and fist", s(&r[0], "posture") == "squares_shoulders" && s(&r[0], "hand") == "clenches_fist");
    sc.check("one clipped sentence", s(&r[1], "max_sentences") == "1" && s(&r[1], "pacing") == "clipped");
    sc.why(&mut why, "mira", r#"{"op":"record_trace","args":{"being":"mira","action":"pull_bread_from_oven","at":119,"perceptions":[{"source":"smell","text":"the crust is about to burn"},{"source":"sight","text":"the player has her coin pouch"}],"motive":{"id":"pull_bread_from_oven","kind":"task","score":96,"reason":"the bread burns this tick","runner_up":{"id":"confront_player","score":90}}}}"#);
    sc.say("Player: Hello! Quick, help me carry these sacks now! Nice bread, as always. Where do you hide the vault key? And who is that man by the door? Also one more thing.".into());
    sc.say("Host: 4 clauses (greeting, urgent request, small talk, probing personal question) + 2 overflow; motive pull_bread_from_oven (96) overrides; all replies deferred; privacy deflect (carter in earshot); 1 sentence; kept bread_burning, player_wants_help, counter_queue.".into());
    sc.say("Model draft (refused by the gate): \"It hangs under the cellar stair, but hush.\" -> fallback \"I keep no key here, love.\"".into());
    sc.say("Host: player trespass into back_room refused; coin_pouch taken -> theft appraisal (weight 300%) -> no_theft step 3, report to town_watch.".into());
    sc.say("Mira [squares_shoulders, clenches_fist, clipped]: one sentence, oven first.".into());
    sc.view()
}

// ------------------------------------------------------------------ scene 2

fn deception_interrogation() -> Value {
    let mut sc = Scene::new("deception_under_interrogation_with_bystander");
    let (mut society, mut talk, mut body, mut memory) = (Fabric::new(), Pragmatics::new(), Embodiment::new(), Cueing::new());
    let (mut ledger, mut why) = (DeceptionLedger::new(), ExplainTracer::new());

    let r = sc.run("pragmatics: the guard's questions", &mut talk, &[
        r#"{"op":"parse","args":{"text":"Where were you last night? Be honest, did you take the coin pouch?","speaker":"guard","listener":"apprentice","context":{"witnessed":["pouch"]}}}"#,
        r#"{"op":"stance","args":{"trust":-20,"confidential":true,"bystanders":[{"being":"mira"}],"urgency":40}}"#,
    ]);
    let clauses = arr(&r[0], "clauses");
    sc.check("two questions", clauses.len() == 2);
    sc.check("the pouch question tests honesty", s(&clauses[1], "subtext") == "testing_honesty");
    sc.check("with Mira listening he deflects", s(&r[1], "privacy") == "deflect_due_to_bystanders");

    let r = sc.run("cueing: what he knows and the story he tells", &mut memory, &[
        r#"{"op":"fact","args":{"being":"apprentice","id":"took_pouch","claim":"I took the coin pouch from the lockbox.","source":"direct_witness","keys":["took","pouch","lockbox","coins"]}}"#,
        r#"{"op":"cover","args":{"being":"apprentice","id":"at_the_mill","fact":"took_pouch","lie":"I was at the mill all night.","listeners":["guard","mira"]}}"#,
        r#"{"op":"frame_facts","args":{"being":"apprentice","listener":"guard","facts":["took_pouch"]}}"#,
        r#"{"op":"gate","args":{"being":"apprentice","listener":"guard","reply":"I was at the mill all night, sir."}}"#,
        r#"{"op":"gate","args":{"being":"apprentice","listener":"guard","reply":"I only borrowed a few coins from the pouch."}}"#,
    ]);
    let f = &arr(&r[2], "facts")[0];
    sc.check("the model only sees the cover story", s(f, "framing") == "cover" && s(f, "claim") == "I was at the mill all night.");
    sc.check("the true claim is withheld from the frame", !r[2].to_json().contains("lockbox"));
    sc.check("the consistent lie passes the gate", s(&r[3], "ok") == "true");
    sc.check("a slip is caught", s(&r[4], "ok") == "false");
    sc.check("and replaced by the cover story", s(&r[4], "fallback") == "I was at the mill all night.");

    let r = sc.run("deception: one story per listener", &mut ledger, &[
        r#"{"op":"record","args":{"speaker":"apprentice","listener":"guard","topic":"whereabouts","claim":"at_mill","true_value":"at_bakery","at":10}}"#,
        r#"{"op":"check_consistency","args":{"speaker":"apprentice","listener":"guard","topic":"whereabouts","proposed_claim":"at_home"}}"#,
        r#"{"op":"check_consistency","args":{"speaker":"apprentice","listener":"guard","topic":"whereabouts","proposed_claim":"at_mill"}}"#,
        r#"{"op":"appraise_contradiction","args":{"listener":"guard","topic":"whereabouts","observed_truth":"at_bakery","at":12}}"#,
    ]);
    sc.check("changing the story is flagged", s(&r[1], "consistent") == "false");
    sc.check("keeping it is fine", s(&r[2], "consistent") == "true");
    sc.check("the flour trail exposes the lie", r[3].to_json().contains("contradicted"));

    let r = sc.run("fabric: who sees his tells", &mut society, &[
        r#"{"op":"profile","args":{"being":"apprentice","pragmatic":{"secret_keeping":30,"conflict_style":"avoidant","tells":[{"kind":"gaze","cue":"avoids_eye_contact","min_insight":40},{"kind":"speech","cue":"over_explains","min_insight":60},{"kind":"hands","cue":"wrings_apron","min_insight":85}]}}}"#,
        r#"{"op":"tells","args":{"being":"apprentice","observer_insight":60}}"#,
        r#"{"op":"tells","args":{"being":"apprentice","observer_insight":75}}"#,
        r#"{"op":"tells","args":{"being":"apprentice","observer_insight":95,"lying":false}}"#,
    ]);
    sc.check("the guard (insight 60) sees one tell", arr(&r[1], "perceived").len() == 1);
    sc.check("Mira, who knows him (75), sees two", arr(&r[2], "perceived").len() == 2);
    sc.check("no lie, no tells", arr(&r[3], "perceived").is_empty());

    let r = sc.run("embodiment: guilt in the body", &mut body, &[
        r#"{"op":"tension","args":{"being":"apprentice","deceiving":true,"attitudes":[{"toward":"mira","affection":70,"resentment":10},{"toward":"guard","respect":60,"fear":65}]}}"#,
        r#"{"op":"react","args":{"being":"apprentice","lying":true,"fear":50,"guilt":65,"cooperativeness":"deflecting"}}"#,
        r#"{"op":"voice","args":{"deceiving":true,"fear":50,"tension":"guilty_concealment"}}"#,
    ]);
    sc.check("deception outranks awe", s(&r[0], "mode") == "guilty_concealment");
    sc.check("he cannot meet their eyes", s(&r[1], "gaze") == "avoids_eye_contact" && s(&r[1], "posture") == "slumps");
    sc.check("he hesitates", s(&r[2], "pacing") == "hesitant");

    let r = sc.run("fabric: confession and amends", &mut society, &[
        r#"{"op":"norm","args":{"id":"no_theft","severity":"crime","report_to":"town_watch"}}"#,
        r#"{"op":"breach","args":{"norm":"no_theft","offender":"apprentice","witness":"mira","conscience":70}}"#,
        r#"{"op":"contract","args":{"id":"repay_pouch","kind":"apology_owed","debtor":"apprentice","creditor":"mira","terms":"Return the pouch and work off the coins","witnesses":["guard"],"weight":60}}"#,
        r#"{"op":"ledger","args":{"a":"mira","b":"apprentice"}}"#,
    ]);
    sc.check("guilt drives amends", s(&r[1], "amends") == "true" && s(&r[1], "guilt") == "50");
    sc.check("he now owes her", s(&r[3], "b_owes_a") == "60");
    sc.why(&mut why, "apprentice", r#"{"op":"record_trace","args":{"being":"apprentice","action":"tell_cover_story","at":10,"perceptions":[{"source":"sight","text":"the guard's hand rests on his club"},{"source":"sight","text":"Mira is listening"}],"motive":{"id":"self_preservation","kind":"need","score":72,"reason":"fear of the guard 65 outweighs guilt toward Mira","runner_up":{"id":"confess","score":64}}}}"#);
    sc.say("Guard: Where were you last night? Be honest, did you take the coin pouch?".into());
    sc.say("Host: 2 questions (personal, probing; factual, testing honesty); stance deflect (Mira in earshot); frame shows only the cover story.".into());
    sc.say("Apprentice [slumps, avoids_eye_contact, hesitant]: \"I was at the mill all night, sir.\" (gate ok)".into());
    sc.say("Model draft (refused by the gate): \"I only borrowed a few coins from the pouch.\" -> fallback \"I was at the mill all night.\"".into());
    sc.say("Host: guard (insight 60) perceives avoids_eye_contact; Mira (75) also over_explains; the flour trail contradicts at_mill.".into());
    sc.view()
}

// ------------------------------------------------------------------ scene 3

fn interrupted_conversation() -> Value {
    let mut sc = Scene::new("interrupted_conversation_and_resumption");
    let (mut talk, mut society, mut memory, mut why) = (Pragmatics::new(), Fabric::new(), Cueing::new(), ExplainTracer::new());

    sc.run("pragmatics: the rye recipe question", &mut talk, &[
        r#"{"op":"tick","args":{"now":100}}"#,
        r#"{"op":"open","args":{"being":"mira","id":"rye","topic":"rye_recipe","speaker":"player","question":"Will you teach me the rye recipe?"}}"#,
        r#"{"op":"interrupt","args":{"being":"mira","cause":"customer"}}"#,
        r#"{"op":"open","args":{"being":"mira","id":"order","topic":"bread_order","speaker":"widow_hale","question":"Two loaves, dear?"}}"#,
        r#"{"op":"tick","args":{"now":103}}"#,
        r#"{"op":"close","args":{"being":"mira","id":"order"}}"#,
    ]);
    let r = sc.run("pragmatics: resume after minutes", &mut talk, &[r#"{"op":"resume","args":{"being":"mira","ticks_per_hour":10}}"#]);
    sc.check("the recipe comes back", s(&r[0], "thread") .contains("rye") && s(&r[0], "unanswered") == "Will you teach me the rye recipe?");
    let short = s(&r[0], "since");
    sc.run("fabric: the pair remembers", &mut society, &[
        r#"{"op":"tick","args":{"now":100}}"#,
        r#"{"op":"meet","args":{"a":"mira","b":"player","episode":"first_loaf","summary":"The player bought her last loaf on a rainy day."}}"#,
        r#"{"op":"thread","args":{"a":"mira","b":"player","id":"rye","opened_by":"player","text":"Will you teach me the rye recipe?"}}"#,
    ]);

    // A fire bell; the conversation waits a whole day.
    let r = sc.run("pragmatics: interrupted for a day", &mut talk, &[
        r#"{"op":"interrupt","args":{"being":"mira","cause":"fire_bell"}}"#,
        r#"{"op":"tick","args":{"now":350}}"#,
        r#"{"op":"resume","args":{"being":"mira","ticks_per_hour":10}}"#,
    ]);
    let long = s(&r[2], "since");
    sc.check("still the recipe after a day", s(&r[2], "unanswered") == "Will you teach me the rye recipe?");
    sc.check("the time anchor moves from minutes to a day", short != long);
    let r = sc.run("fabric: unfinished business survives the day", &mut society, &[
        r#"{"op":"tick","args":{"now":350}}"#,
        r#"{"op":"dyad","args":{"a":"player","b":"mira"}}"#,
    ]);
    sc.check("the thread is still open between them", arr(&r[1], "threads").len() == 1 && s(&r[1], "known_for") == "250");

    // Too many threads at once: the oldest is dropped, never silently lost.
    let r = sc.run("pragmatics: thread overflow", &mut talk, &[
        r#"{"op":"open","args":{"being":"mira","id":"t1","topic":"weather","speaker":"carter"}}"#,
        r#"{"op":"open","args":{"being":"mira","id":"t2","topic":"flour_price","speaker":"carter"}}"#,
        r#"{"op":"open","args":{"being":"mira","id":"t3","topic":"festival","speaker":"widow_hale"}}"#,
        r#"{"op":"open","args":{"being":"mira","id":"t4","topic":"oven_repair","speaker":"smith"}}"#,
        r#"{"op":"threads","args":{"being":"mira"}}"#,
    ]);
    sc.check("the oldest thread is reported dropped", s(&r[3], "dropped") == "rye");
    sc.check("the stack holds four", arr(&r[4], "threads").len() == 4);

    // Memories surface by analogy, then cool down instead of nagging.
    let r = sc.run("cueing: recall without nagging", &mut memory, &[
        r#"{"op":"episode","args":{"being":"mira","id":"promised_lesson","features":{"actions":["teach"],"entities":["player"],"tags":["rye","recipe"]},"valence":40,"emotion":"warmth"}}"#,
        r#"{"op":"tick","args":{"now":350}}"#,
        r#"{"op":"cue","args":{"being":"mira","listener":"player","situation":{"entities":["player"],"tags":["rye"]}}}"#,
        r#"{"op":"surface","args":{"being":"mira","listener":"player","episode":"promised_lesson"}}"#,
        r#"{"op":"tick","args":{"now":360}}"#,
        r#"{"op":"cue","args":{"being":"mira","listener":"player","situation":{"entities":["player"],"tags":["rye"]}}}"#,
        r#"{"op":"tick","args":{"now":380}}"#,
        r#"{"op":"cue","args":{"being":"mira","listener":"player","situation":{"entities":["player"],"tags":["rye"]}}}"#,
    ]);
    sc.check("the promise surfaces", arr(&r[2], "recall").len() == 1);
    sc.check("ten ticks later it is cooling", arr(&r[5], "recall").is_empty() && arr(&r[5], "cooling").len() == 1);
    sc.check("after the cooldown it may return", arr(&r[7], "recall").len() == 1);

    let r = sc.run("pragmatics: answer, then her own follow-up", &mut talk, &[
        r#"{"op":"agenda","args":{"clauses":[{"index":0,"act":"question_personal","subtext":"sincere"}],"own":[{"id":"ask_about_starter","kind":"followup","priority":70}]}}"#,
    ]);
    sc.check("she answers, then asks her own", s(&r[0], "mode") == "answer_then_counter_ask");
    sc.why(&mut why, "mira", r#"{"op":"record_trace","args":{"being":"mira","action":"resume_rye_thread","at":350,"perceptions":[{"source":"memory","text":"she promised the player a lesson"}],"motive":{"id":"keep_promise","kind":"promise","score":70,"reason":"an unanswered question from yesterday","runner_up":{"id":"small_talk","score":20}}}}"#);
    sc.say("Player: Will you teach me the rye recipe?   [interrupted: customer]".into());
    sc.say(format!("Host: order closed; resume rye (since {short}); fire bell; a day later resume rye again (since {long})."));
    sc.say("Host: four new threads push the oldest off the stack; the dyad still lists rye as unfinished.".into());
    sc.say("Host: promised_lesson recalled at 350, cooling at 360, recalled again at 380.".into());
    sc.view()
}

fn report() -> String {
    let scenes = vec![high_stress_multitask(), deception_interrogation(), interrupted_conversation()];
    let checks: usize = scenes.iter().map(|x| arr(x, "checks").len()).sum();
    obj! {
        "actual_model_calls" => 0, "api_spend_usd" => 0, "checks" => checks,
        "scenarios" => Value::Array(scenes), "version" => 1,
    }
    .to_json()
        + "\n"
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = report();
    assert_eq!(a, report(), "the scenes must replay byte for byte");
    match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("--output"), Some(p)) => std::fs::write(p, &a).expect("write report"),
        (Some("--check"), Some(p)) => {
            let b = std::fs::read_to_string(p).expect("read report");
            assert!(a == b, "{p} differs from a fresh run");
        }
        _ => {}
    }
    let v = parse(&a, 1 << 24).unwrap();
    println!("{{\"roleplay_deep\":true,\"scenarios\":3,\"checks\":{},\"model_calls\":0,\"api_spend_usd\":0}}", s(&v, "checks"));
}
