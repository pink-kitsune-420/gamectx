//! Acceptance Scene (INTELLIGENT_BEING.md §8)
//!
//! An offline end-to-end scene exercising Stages 1–5:
//! 1. Baker routine says "bake" from dawn. Hunger rises over morning.
//! 2. Player promised to bring flour; baker remembers the promise.
//! 3. Player does not come. Flour shortage blocks bake task; baker replans:
//!    walk to the mill, barter for flour.
//! 4. On the way, baker witnesses a cart crash (witness bridge). Baker records
//!    a direct experience; apprentice indoors does not.
//! 5. Back home, baker is hungry and tired; motive arbiter picks eating before baking.
//! 6. Baking succeeds; flour consumed; bread exists.
//! 7. Player arrives; speech is strictly grounded in confirmed state.
//! 8. Pass 2: an explain trace answers "why did the baker eat?" (D4) and the stability
//!    monitors audit the scene's turns and report it healthy (D1).
//! 9. Pass 2: the baker's hearsay about the mill is confirmed by doing (A1), and the baker
//!    informs the apprentice about the crash they missed (B1).
//! 10. Pass 2: the hungry baker picks cooked stew over day-old bread and raw trout; the diet
//!     module's `satisfy` proposal is applied to the body engine.
//! 11. Pass 2: the stew had meat, and the baker keeps the hearth creed; the apprentice saw.
//!     Conscience (B5) turns guilt and shame into a mood proposal applied to the affect
//!     engine, and the creed's food taboo is fed to the diet so tomorrow's choice is bread.
//! 12. Pass 2: the baker expected the player's flour by noon (A3): worry, then a broken
//!     promise whose resentment proposal is applied to the affect engine, then relief when the
//!     player arrives late. Seeded attention (A2) has the baker on the road notice the crash
//!     while the apprentice, out of range in the bakery, does not.
//! 13. Pass 2: experience grows a goal (A4: two thefts → guard against thieves), repeated
//!     evening walks form a habit (A5), and good barters with the miller become a lesson (A6);
//!     the goal and the habit go to the motive arbiter as candidates and the lesson's advice
//!     explains the baker's trust in the mill. Stage 6 modules run with host-set limits.
//!
//! Asserts byte-identical replay.

use gamectx::character::affect::AffectManager;
use gamectx::character::aspiration::Aspirations;
use gamectx::character::attention::AttentionModel;
use gamectx::character::body::BodyManager;
use gamectx::character::creed::CreedManager;
use gamectx::character::curiosity::CuriosityManager;
use gamectx::character::deception::DeceptionLedger;
use gamectx::character::diet::DietManager;
use gamectx::character::expectation::ExpectationBook;
use gamectx::character::explain::ExplainTracer;
use gamectx::character::habit::Habits;
use gamectx::character::lesson::Lessons;
use gamectx::character::life::witness::WitnessBridge;
use gamectx::character::mind::MindManager;
use gamectx::character::motive::MotiveArbiter;
use gamectx::character::offscreen::OffscreenResolver;
use gamectx::character::relationship::RelationshipManager;
use gamectx::character::skills::SkillsManager;
use gamectx::character::speech_act::SpeechActs;
use gamectx::character::stability::StabilityMonitor;
use gamectx::character::task::TaskManager;
use gamectx::data::json::{parse, Value};
use gamectx::knowledge::action_catalog::ActionCatalog;
use gamectx::obj;

fn run_baker_scene() -> String {
    // Stage 1: Action Catalog
    let mut catalog = ActionCatalog::new();
    let desc_req = parse(r#"{"ops":[{"op":"describe","args":{"id":"cook"}}]}"#, 4096).unwrap();
    let catalog_res = catalog.workflow(&desc_req).unwrap();
    assert_eq!(catalog_res.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));

    // Stage 3: Body & Needs init
    let mut body_mgr = BodyManager::new();
    let init_body = parse(
        r#"{
        "ops": [
            {
                "op": "init",
                "args": {
                    "being": "baker",
                    "health": 100,
                    "fatigue": 10,
                    "needs": [
                        {
                            "id": "hunger",
                            "label": "Hunger",
                            "level": 20,
                            "threshold": 70,
                            "rate_per_tick": 5,
                            "satisfiers": [{"action": "eat", "amount": 50}]
                        }
                    ]
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    body_mgr.workflow(&init_body).unwrap();

    // Stage 6: Skills & Affect init
    let mut skills_mgr = SkillsManager::new();
    let init_skills = parse(
        r#"{
        "ops": [
            {
                "op": "init",
                "args": {
                    "being": "baker",
                    "skills": [
                        {"id": "baking", "label": "Baking", "level": 20, "practice": 0}
                    ]
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    skills_mgr.workflow(&init_skills).unwrap();

    let mut affect_mgr = AffectManager::new();
    let init_affect = parse(
        r#"{
        "ops": [
            {
                "op": "init",
                "args": {
                    "being": "baker",
                    "temperament": "forgiving",
                    "mood": {"valence": -10, "arousal": 15},
                    "attitudes": [
                        {
                            "id": "resentment_player",
                            "kind": "resentment",
                            "subject": "player",
                            "level": 25,
                            "baseline": 0,
                            "decay_rate": 2,
                            "reason": "broken flour promise"
                        }
                    ]
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    affect_mgr.workflow(&init_affect).unwrap();

    // Stage 5: Propose initial bake task
    let mut task_mgr = TaskManager::new();
    let propose_task = parse(
        r#"{
        "ops": [
            {
                "op": "propose",
                "args": {
                    "id": "bake_bread_task",
                    "owner": "baker",
                    "priority": 70,
                    "on_fail": "replan",
                    "on_block": "replan",
                    "steps": [
                        {"id": "cook_step", "action": "cook", "target_bindings": {"recipe": "bread"}}
                    ]
                }
            },
            {"op": "accept", "args": {"id": "bake_bread_task", "start": true}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    task_mgr.workflow(&propose_task).unwrap();

    // Step 2 & 3: Player promised flour, but didn't come. Flour missing, task blocks.
    let block_req = parse(
        r#"{
        "ops": [
            {"op": "block", "args": {"id": "bake_bread_task", "reason": "missing_resource:flour"}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    task_mgr.workflow(&block_req).unwrap();

    // Baker replans: deterministic planner finds route to mill and barter
    let replan_req = parse(
        r#"{
        "ops": [
            {
                "op": "replan",
                "args": {
                    "id": "bake_bread_task",
                    "goal": {"kind": "cook", "recipe": "bread"},
                    "world_state": {
                        "location": "bakery",
                        "inventory": {"flour": 0, "coins": 5},
                        "recipes": {
                            "bread": {
                                "station": "bakery",
                                "inputs": [{"item": "flour", "count": 1}],
                                "outputs": [{"item": "bread", "count": 1}]
                            }
                        },
                        "sources": [
                            {
                                "item": "flour",
                                "place": "mill",
                                "partner": "miller",
                                "cost_item": "coins",
                                "cost_count": 2
                            }
                        ]
                    }
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    let replan_res = task_mgr.workflow(&replan_req).unwrap();
    assert_eq!(replan_res.get("results").unwrap().as_array().unwrap()[0].get("replanned"), Some(&Value::Bool(true)));

    // Step 4: On travel to mill, cart crash happens. Witness bridge records direct experience for baker, none for apprentice.
    let witness_req = parse(
        r#"{
        "ops": [
            {
                "op": "witness",
                "args": {
                    "entry": {
                        "at": 6,
                        "kind": "crash",
                        "summary": "A merchant cart broke its wheel and overturned on the road.",
                        "place": "mill_road",
                        "actors": ["carter"],
                        "witnesses": ["baker"],
                        "visibility": "witnessed",
                        "noise": 80
                    },
                    "beings": [
                        {
                            "id": "baker",
                            "place": "mill_road",
                            "senses": {"sight": true, "hearing": true},
                            "token": 101
                        },
                        {
                            "id": "apprentice",
                            "place": "bakery",
                            "senses": {"sight": true, "hearing": true, "occluded": true},
                            "token": 102
                        }
                    ]
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    let mut witness_bridge = WitnessBridge::new();
    let witness_res = witness_bridge.workflow(&witness_req).unwrap();
    let outcomes =
        witness_res.get("results").unwrap().as_array().unwrap()[0].get("outcomes").unwrap().as_array().unwrap();
    let baker_outcome = outcomes.iter().find(|o| o.get("being").and_then(Value::as_str) == Some("baker")).unwrap();
    let apprentice_outcome =
        outcomes.iter().find(|o| o.get("being").and_then(Value::as_str) == Some("apprentice")).unwrap();
    assert_eq!(baker_outcome.get("perceived"), Some(&Value::Bool(true)), "Baker witnessed crash");
    assert_eq!(
        apprentice_outcome.get("perceived"),
        Some(&Value::Bool(false)),
        "Apprentice indoors did not witness crash"
    );

    // Step 5: Advance through travel and barter; ticks pass and hunger rises to 80
    // Advance travel to mill
    task_mgr
        .workflow(
            &parse(
                r#"{"ops":[{"op":"advance","args":{"id":"bake_bread_task","outcome":{"success":true},"tick":7}}]}"#,
                4096,
            )
            .unwrap(),
        )
        .unwrap();
    // Advance barter with miller
    task_mgr
        .workflow(
            &parse(
                r#"{"ops":[{"op":"advance","args":{"id":"bake_bread_task","outcome":{"success":true},"tick":9}}]}"#,
                4096,
            )
            .unwrap(),
        )
        .unwrap();
    // Advance travel back to bakery
    task_mgr
        .workflow(
            &parse(
                r#"{"ops":[{"op":"advance","args":{"id":"bake_bread_task","outcome":{"success":true},"tick":12}}]}"#,
                4096,
            )
            .unwrap(),
        )
        .unwrap();

    // Body tick update: 15 ticks elapsed, hunger rose from 20 to 80 (threshold 70)
    let tick_body = parse(r#"{"ops":[{"op":"tick","args":{"being":"baker","ticks":15}}]}"#, 4096).unwrap();
    body_mgr.workflow(&tick_body).unwrap();
    let assess_body = parse(r#"{"ops":[{"op":"assess","args":{"being":"baker"}}]}"#, 4096).unwrap();
    let assess_res = body_mgr.workflow(&assess_body).unwrap();
    let body_state = &assess_res.get("results").unwrap().as_array().unwrap()[0];

    // Arbiter picks motive: hunger (80) vs routine (50) vs bake task (70)
    let arbiter = MotiveArbiter::new();
    let arbitrate_req = obj! {
        "ops" => Value::Array(vec![
            obj! {
                "op" => Value::str("arbitrate"),
                "args" => obj! {
                    "body" => body_state.clone(),
                    "routine" => obj! {
                        "id" => Value::str("bake_shift"),
                        "label" => Value::str("Morning bake shift"),
                        "priority" => Value::int(50),
                        "window_active" => Value::Bool(true)
                    },
                    "motives" => Value::Array(vec![
                        obj! {
                            "id" => Value::str("task_bake_bread"),
                            "kind" => Value::str("task"),
                            "label" => Value::str("Finish baking"),
                            "priority" => Value::int(70)
                        }
                    ])
                }
            }
        ])
    };
    let arbiter_res = arbiter.workflow(&arbitrate_req).unwrap();
    let winning_motive = arbiter_res.get("results").unwrap().as_array().unwrap()[0].get("winner").unwrap();
    assert_eq!(winning_motive.get("kind").and_then(Value::as_str), Some("need"));
    assert_eq!(winning_motive.get("id").and_then(Value::as_str), Some("need_hunger"));

    // Satisfy hunger by eating
    let satisfy_req = parse(
        r#"{
        "ops": [
            {"op":"satisfy","args":{"being":"baker","need":"hunger","amount":50}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    body_mgr.workflow(&satisfy_req).unwrap();

    // Step 6: Bake step succeeds, task done
    let finish_task = parse(
        r#"{
        "ops": [
            {"op":"advance","args":{"id":"bake_bread_task","outcome":{"success":true,"produced":"bread"},"tick":16}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let final_task_res = task_mgr.workflow(&finish_task).unwrap();
    let final_status =
        final_task_res.get("results").unwrap().as_array().unwrap()[0].get("task_status").and_then(Value::as_str);
    assert_eq!(final_status, Some("done"));

    // Stage 6: Skills practice gain from successful bake
    let bake_practice = parse(
        r#"{
        "ops": [
            {"op":"practice","args":{"being":"baker","skill":"baking","practice":25,"success":true}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let practice_res = skills_mgr.workflow(&bake_practice).unwrap();
    let effective_gain =
        practice_res.get("results").unwrap().as_array().unwrap()[0].get("effective_gain").and_then(Value::as_usize);
    assert_eq!(effective_gain, Some(20)); // factor = 80, 25 * 80 / 100 = 20

    // Stage 6: Affect tick - time passes (10 ticks), forgiving baker's resentment toward player decays
    let tick_affect = parse(
        r#"{
        "ops": [
            {"op":"tick","args":{"being":"baker","ticks":10}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let affect_res = affect_mgr.workflow(&tick_affect).unwrap();
    let current_attitudes =
        affect_res.get("results").unwrap().as_array().unwrap()[0].get("attitudes").unwrap().as_array().unwrap();
    let resentment_att =
        current_attitudes.iter().find(|a| a.get("id").and_then(Value::as_str) == Some("resentment_player")).unwrap();
    let resentment_lvl = match resentment_att.get("level") {
        Some(Value::Number(s)) => s.parse::<i64>().ok(),
        _ => None,
    };
    // Forgiving temperament: decay_rate 2 + 2 = 4, 4 * 10 = 40, clamps to baseline 0
    assert_eq!(resentment_lvl, Some(0));

    // Step 7: Grounded speech facts check (Theory of Mind, Deception Ledger, Relationship Reconciliation)
    // 1. Theory of Mind: baker infers that apprentice indoors did NOT witness cart crash
    let mut mind_mgr = MindManager::new();
    let mind_infer = parse(
        r#"{
        "ops": [
            {
                "op": "infer_from_witness",
                "args": {
                    "subject": "baker",
                    "fact": "cart_crash",
                    "entry": {
                        "at": 6,
                        "kind": "crash",
                        "place": "mill_road",
                        "actors": ["carter"],
                        "visibility": "witnessed",
                        "noise": 80
                    },
                    "beings": [
                        {"id": "baker", "place": "mill_road", "senses": {"sight": true, "hearing": true}},
                        {"id": "apprentice", "place": "bakery", "senses": {"sight": true, "hearing": true, "occluded": true}}
                    ]
                }
            },
            {
                "op": "query",
                "args": {"subject": "baker", "other": "apprentice", "category": "knows", "key": "cart_crash"}
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    let mind_res = mind_mgr.workflow(&mind_infer).unwrap();
    let query_res = &mind_res.get("results").unwrap().as_array().unwrap()[1];
    assert_eq!(query_res.get("found"), Some(&Value::Bool(true)));
    let belief = query_res.get("belief").unwrap();
    assert_eq!(belief.get("status"), Some(&Value::Bool(false)), "Baker believes apprentice does not know about crash");
    assert_eq!(belief.get("source"), Some(&Value::str("inferred")));

    // 2. Deception ledger: baker speaks honestly to player about the crash
    let mut dec_ledger = DeceptionLedger::new();
    let dec_check = parse(
        r#"{
        "ops": [
            {
                "op": "check_consistency",
                "args": {
                    "speaker": "baker",
                    "listener": "player",
                    "topic": "cart_crash",
                    "proposed_claim": "witnessed_cart_crash"
                }
            },
            {
                "op": "record",
                "args": {
                    "speaker": "baker",
                    "listener": "player",
                    "topic": "cart_crash",
                    "claim": "witnessed_cart_crash",
                    "true_value": "witnessed_cart_crash",
                    "at": 16
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    let dec_res = dec_ledger.workflow(&dec_check).unwrap();
    let record_res = &dec_res.get("results").unwrap().as_array().unwrap()[1];
    assert_eq!(record_res.get("is_lie"), Some(&Value::Bool(false)), "Baker tells player the truth");

    // 3. Relationship reconciliation: baker resolves relationship with player
    let mut rel_mgr = RelationshipManager::new();
    let rel_req = parse(
        r#"{
        "ops": [
            {
                "op": "resolve",
                "args": {
                    "subject": "baker",
                    "target": "player",
                    "sheet_relationship": {"with": "player", "kind": "friend"}
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    let rel_res = rel_mgr.workflow(&rel_req).unwrap();
    let resolved = &rel_res.get("results").unwrap().as_array().unwrap()[0];
    assert_eq!(resolved.get("dominant_stance"), Some(&Value::str("friendly")));
    assert_eq!(resolved.get("effective_trust"), Some(&Value::int(40)));

    // 4. Off-screen resolution: apprentice completes chores unobserved
    let mut off_res = OffscreenResolver::new();
    let off_req = parse(
        r#"{
        "ops": [
            {
                "op": "resolve_window",
                "args": {
                    "being": "apprentice",
                    "activity": "sweeping_bakery",
                    "action": "sweep",
                    "observed": false,
                    "at": 18,
                    "place": "bakery",
                    "visibility": "witnessed",
                    "fatigue": 6,
                    "practice": 15,
                    "inputs": [{"item": "broom_durability", "count": 1}],
                    "outputs": [{"item": "clean_bakery", "count": 1}]
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    let off_out = off_res.workflow(&off_req).unwrap();
    let off_record = &off_out.get("results").unwrap().as_array().unwrap()[0];
    assert_eq!(off_record.get("resolved"), Some(&Value::Bool(true)));
    assert_eq!(off_record.get("practice_gained"), Some(&Value::int(15)));

    // 5. Curiosity & novelty detection: baker notices strange visitor and generates initiative candidate
    let mut cur_mgr = CuriosityManager::new();
    let cur_init_req = parse(
        r#"{
        "ops": [
            {
                "op": "init",
                "args": {
                    "being": "baker",
                    "temperament": "curious",
                    "known_people": ["player", "apprentice"],
                    "known_things": ["oven", "bread"],
                    "known_places": ["bakery", "market"],
                    "known_facts": ["cart_crash"]
                }
            },
            {
                "op": "generate_candidates",
                "args": {
                    "being": "baker",
                    "people": ["mysterious_traveler"],
                    "things": ["enchanted_yeast"],
                    "auto_mark": true
                }
            }
        ]
    }"#,
        4096,
    )
    .unwrap();
    let cur_out = cur_mgr.workflow(&cur_init_req).unwrap();
    let candidates_res = &cur_out.get("results").unwrap().as_array().unwrap()[1];
    let candidates = candidates_res.get("candidates").unwrap().as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].get("kind"), Some(&Value::str("ask")));
    assert_eq!(candidates[1].get("kind"), Some(&Value::str("activity")));

    // 6. Explain trace (pass 2, D4): why did the baker eat before baking?
    let ranked = arbiter_res.get("results").unwrap().as_array().unwrap()[0].get("ranked").unwrap().as_array().unwrap();
    let runner_up = &ranked[1];
    let mut tracer = ExplainTracer::new();
    let record_req = obj! {
        "ops" => Value::Array(vec![
            obj! {
                "op" => "record_trace",
                "args" => obj! {
                    "being" => "baker",
                    "action" => "eat",
                    "at" => 15u64,
                    "perceptions" => Value::Array(vec![obj! { "source" => "body", "text" => "stomach growling after the mill walk" }]),
                    "beliefs" => Value::Array(vec![obj! { "about" => "player", "fact" => "the player promised flour", "source" => "told" }]),
                    "motive" => obj! {
                        "id" => winning_motive.get("id").unwrap().clone(),
                        "kind" => winning_motive.get("kind").unwrap().clone(),
                        "score" => winning_motive.get("score").unwrap().clone(),
                        "reason" => winning_motive.get("reason").unwrap().clone(),
                        "rank" => 1u64,
                        "runner_up" => obj! { "id" => runner_up.get("id").unwrap().clone(), "score" => runner_up.get("score").unwrap().clone() },
                    },
                    "task" => obj! { "id" => "bake_bread_task", "step" => "bake", "index" => 3u64 },
                },
            },
            obj! { "op" => "confirm_outcome", "args" => obj! { "being" => "baker", "id" => "tr_1", "status" => "success", "summary" => "hunger fell by 50" } },
        ])
    };
    tracer.workflow(&record_req).unwrap();
    let trace_out = tracer
        .workflow(&parse(r#"{"ops":[{"op":"query_trace","args":{"being":"baker","action":"eat"}}]}"#, 4096).unwrap())
        .unwrap();
    let trace = &trace_out.get("results").unwrap().as_array().unwrap()[0].get("traces").unwrap().as_array().unwrap()[0];
    let why = trace.get("why").and_then(Value::as_str).unwrap();
    assert!(why.starts_with("baker did eat because"), "{why}");
    assert!(why.contains("need motive need_hunger"), "{why}");
    assert_eq!(trace.get("chain").unwrap().as_array().unwrap().len(), 6);

    // 7. Stability monitors (pass 2, D1): the scene's turns stay healthy
    let mut monitor = StabilityMonitor::new();
    let stability_req = parse(
        r#"{
        "ops": [
            {"op":"audit_turn","args":{"turn":1,"beings":[
                {"being":"baker","motive":"routine_bake","activity":"bake","score":50},
                {"being":"apprentice","motive":"routine_chores","activity":"sweep","score":40}]}},
            {"op":"audit_turn","args":{"turn":7,"beings":[
                {"being":"baker","motive":"task_bake_bread","activity":"travel","score":70,"runner_up":50,"holds":["bread"],"wants":["flour"]},
                {"being":"apprentice","motive":"routine_chores","activity":"sweep","score":40}]}},
            {"op":"audit_turn","args":{"turn":15,"beings":[
                {"being":"baker","motive":"need_hunger","activity":"eat","score":80,"runner_up":70,"needs":[{"id":"hunger","level":80}],"recovering":true},
                {"being":"apprentice","motive":"routine_chores","activity":"sweep","score":40}],
                "world":{"grudge_total":0,"rumor_total":1}}},
            {"op":"assess_health"}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let stability_out = monitor.workflow(&stability_req).unwrap();
    let health = &stability_out.get("results").unwrap().as_array().unwrap()[3];
    assert_eq!(health.get("status"), Some(&Value::str("healthy")));

    // 8. Subjective knowledge (pass 2, A1) and speech acts (pass 2, B1): the baker only
    //    heard the mill sells flour; the barter confirms it first-hand. The apprentice was
    //    indoors during the cart crash, so the baker tells them.
    let mut speech = SpeechActs::new();
    let speech_req = parse(
        r#"{
        "ops": [
            {"op":"learn","args":{"being":"baker","about":"mill","relation":"sells","value":"flour","source":"told","from":"carter","at":3}},
            {"op":"outcome","args":{"being":"baker","action":"barter","success":true,"at":9,
                "relied_on":[{"about":"mill","relation":"sells","value":"flour"}]}},
            {"op":"learn","args":{"being":"baker","about":"road","relation":"had","value":"cart_crash","source":"saw","at":6}},
            {"op":"relate","args":{"speaker":"baker","listener":"apprentice","trust":70}},
            {"op":"plan","args":{"speaker":"baker","intent":"share","candidates":["apprentice"],
                "topic":{"about":"road","relation":"had","value":"cart_crash"}}},
            {"op":"perform","args":{"speaker":"baker","listener":"apprentice","act":"inform","at":16,
                "topic":{"about":"road","relation":"had","value":"cart_crash"}}},
            {"op":"gaps","args":{"being":"apprentice","needs":[{"about":"road","relation":"had","value":"cart_crash"}]}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let speech_out = speech.workflow(&speech_req).unwrap();
    let speech_results = speech_out.get("results").unwrap().as_array().unwrap();
    let confirmed = &speech_results[1].get("updates").unwrap().as_array().unwrap()[1];
    assert_eq!(confirmed.get("result"), Some(&Value::str("reinforced")));
    assert_eq!(confirmed.get("belief").unwrap().get("source"), Some(&Value::str("did")));
    assert_eq!(speech_results[5].get("outcome"), Some(&Value::str("informed")));
    assert_eq!(speech_results[6].get("gaps"), Some(&Value::int(0)));

    // 9. Diets and foods (pass 2, stage 3): the hungry baker chooses between day-old bread
    //    and a cooked stew, eats, and the diet proposal is applied to the body engine.
    let mut diet = DietManager::new();
    let diet_req = parse(
        r#"{
        "ops": [
            {"op":"register_food","args":{"pack":"village"}},
            {"op":"set_diet","args":{"being":"baker","likes":["grain"],"dislikes":["fish"]}},
            {"op":"choose","args":{"being":"baker","hunger_level":80,"options":[
                {"food":"rye_bread","made_at":0,"at":16},
                {"food":"hearty_stew","cooked":true,"made_at":14,"at":16},
                {"food":"river_trout","made_at":0,"at":16}]}},
            {"op":"eat","args":{"being":"baker","food":"hearty_stew","cooked":true,"made_at":14,"at":16}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let diet_out = diet.workflow(&diet_req).unwrap();
    let diet_results = diet_out.get("results").unwrap().as_array().unwrap();
    assert_eq!(diet_results[2].get("best"), Some(&Value::int(1)));
    let meal = &diet_results[3];
    assert_eq!(meal.get("eaten"), Some(&Value::Bool(true)));
    let satisfy = &meal.get("effects").unwrap().as_array().unwrap()[0];
    assert_eq!(satisfy.get("engine"), Some(&Value::str("body")));
    let apply = obj! { "ops" => Value::Array(vec![obj! {
        "op" => satisfy.get("op").unwrap().clone(),
        "args" => satisfy.get("args").unwrap().clone(),
    }]) };
    body_mgr.workflow(&apply).unwrap();

    // 10. Creeds and conscience (pass 2, stage 4 and B5): the stew had meat and the baker is
    //     a Keeper of the Hearth; the apprentice saw. The guilt/shame mood proposal goes to
    //     the affect engine and the creed's food taboo goes to the diet, so the next meal
    //     the baker chooses is bread.
    let mut creeds = CreedManager::new();
    let creed_req = parse(
        r#"{
        "ops": [
            {"op":"register_creed","args":{"creed":{"id":"hearthkeepers","label":"Keepers of the Hearth","kind":"religion",
                "tenets":[{"id":"no_flesh","food_tag":"meat","severity":40,"label":"Eat nothing that bled"}],
                "observances":[{"id":"tend_flame","action":"tend_hearth","every":24,"window":4,"offset":6,"severity":50,"label":"Tend the flame at dawn"}]}}},
            {"op":"adhere","args":{"being":"baker","creed":"hearthkeepers","devotion":70}},
            {"op":"adhere","args":{"being":"apprentice","creed":"hearthkeepers","devotion":90}},
            {"op":"judge","args":{"actor":"baker","action":"eat","food_tags":["meat","vegetable"],"witnesses":["apprentice"],"at":16}},
            {"op":"food_taboos","args":{"being":"baker"}},
            {"op":"due","args":{"at":30}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let creed_out = creeds.workflow(&creed_req).unwrap();
    let creed_results = creed_out.get("results").unwrap().as_array().unwrap();
    let verdict = &creed_results[3];
    assert_eq!(verdict.get("guilt"), Some(&Value::int(28)));
    assert_eq!(verdict.get("shame"), Some(&Value::int(14)));
    let remorse = &verdict.get("effects").unwrap().as_array().unwrap()[0];
    assert_eq!(remorse.get("op"), Some(&Value::str("set_mood")));
    affect_mgr
        .workflow(&obj! { "ops" => Value::Array(vec![obj! {
            "op" => remorse.get("op").unwrap().clone(),
            "args" => remorse.get("args").unwrap().clone(),
        }]) })
        .unwrap();
    let forbids = creed_results[4].get("forbids").unwrap().clone();
    assert_eq!(forbids.to_json(), r#"["meat"]"#);
    let due = creed_results[5].get("due").unwrap().as_array().unwrap();
    assert_eq!(due.len(), 2, "both keepers should tend the flame at dawn");
    let next_meal = diet
        .workflow(&obj! { "ops" => Value::Array(vec![
            obj! { "op" => "set_diet", "args" => obj! {
                "being" => "baker",
                "forbids" => forbids,
                "likes" => Value::strings(&["grain"]),
                "dislikes" => Value::strings(&["fish"]),
            } },
            obj! { "op" => "choose", "args" => obj! {
                "being" => "baker",
                "hunger_level" => 60u64,
                "options" => Value::Array(vec![
                    obj! { "food" => "hearty_stew", "cooked" => true },
                    obj! { "food" => "rye_bread" },
                ]),
            } },
        ]) })
        .unwrap();
    let next_choice = &next_meal.get("results").unwrap().as_array().unwrap()[1];
    assert_eq!(next_choice.get("best"), Some(&Value::int(1)), "the creed taboo should steer the baker to bread");

    // 11. Expectations (A3) and attention (A2): the broken flour promise produces worry,
    //     disappointment and resentment (applied to the affect engine), and relief when the
    //     player finally arrives. Seeded attention decides who noticed the road crash.
    let mut book = ExpectationBook::new();
    let expect_req = parse(
        r#"{
        "ops": [
            {"op":"expect","args":{"being":"baker","id":"flour","about":"player","event":"bring_flour","due":12,"grace":2,"importance":80,"relies":true,"label":"the player brings flour by noon"}},
            {"op":"tick","args":{"at":13}},
            {"op":"tick","args":{"at":15}},
            {"op":"happen","args":{"about":"player","event":"bring_flour","at":18}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let expect_out = book.workflow(&expect_req).unwrap();
    let expect_results = expect_out.get("results").unwrap().as_array().unwrap();
    assert!(expect_results[1].to_json().contains(r#""outcome":"worried""#));
    assert!(expect_results[2].to_json().contains(r#""outcome":"disappointed""#));
    assert!(expect_results[3].to_json().contains(r#""outcome":"late""#));
    let grudge = expect_results[2]
        .get("effects")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e.get("op") == Some(&Value::str("appraise")))
        .unwrap()
        .clone();
    affect_mgr
        .workflow(&obj! { "ops" => Value::Array(vec![obj! {
            "op" => grudge.get("op").unwrap().clone(),
            "args" => grudge.get("args").unwrap().clone(),
        }]) })
        .unwrap();

    let mut attention = AttentionModel::new();
    let notice_req = parse(
        r#"{
        "ops": [
            {"op":"profile","args":{"being":"baker","alertness":60,"cares":["accident"]}},
            {"op":"notice","args":{"seed":2026,"at":100,
                "events":[{"id":"cart_crash","kind":"accident","place":"road","salience":90,"noise":40,"subject":"carter"}],
                "beings":[{"id":"baker","place":"road","busy":30},{"id":"apprentice","place":"bakery","busy":60}]}}
        ]
    }"#,
        4096,
    )
    .unwrap();
    let notice_out = attention.workflow(&notice_req).unwrap();
    let noticed = &notice_out.get("results").unwrap().as_array().unwrap()[1];
    assert!(noticed.to_json().contains(r#""noticed_by":["baker"]"#), "{}", noticed.to_json());

    // 12. Goals from experience (A4), habits (A5) and lessons (A6), with host-set limits:
    //     the goal and the habit become motive arbiter candidates; the lesson gives advice.
    let host_limits = parse(r#"{"beings":5000}"#, 64).unwrap();
    let mut aspirations = Aspirations::with_limits(&host_limits).unwrap();
    let mut habits = Habits::with_limits(&host_limits).unwrap();
    let mut lessons = Lessons::with_limits(&host_limits).unwrap();
    let grown = aspirations
        .workflow(
            &parse(
                r#"{"ops":[
                {"op":"experience","args":{"being":"baker","kind":"threat","subject":"thief","weight":60,"at":20}},
                {"op":"experience","args":{"being":"baker","kind":"threat","subject":"thief","weight":60,"at":44}},
                {"op":"goals","args":{"being":"baker"}}]}"#,
                4096,
            )
            .unwrap(),
        )
        .unwrap();
    let grown_goals = grown.get("results").unwrap().as_array().unwrap()[2].get("goals").unwrap().clone();
    assert!(grown_goals.to_json().contains(r#""id":"guard_thief""#));
    let walk = r#"{"op":"choose","args":{"being":"baker","slot":"dusk","action":"walk_river","at":18}}"#;
    let routine_out = habits
        .workflow(
            &parse(
                &format!(
                    r#"{{"ops":[{walk},{walk},{walk},{{"op":"routine","args":{{"being":"baker","slot":"dusk"}}}}]}}"#
                ),
                4096,
            )
            .unwrap(),
        )
        .unwrap();
    let habit_entry =
        routine_out.get("results").unwrap().as_array().unwrap()[3].get("entries").unwrap().as_array().unwrap()[0]
            .clone();
    assert_eq!(habit_entry.get("id"), Some(&Value::str("habit_dusk_walk_river")));
    let barter = |e: &str, at: u64| {
        format!(
            r#"{{"op":"outcome","args":{{"being":"baker","about":"miller","activity":"barter","result":"good","episode":"{e}","at":{at}}}}}"#
        )
    };
    let lesson_out = lessons
        .workflow(
            &parse(
                &format!(
                    r#"{{"ops":[{},{},{},{{"op":"advice","args":{{"being":"baker","about":"miller"}}}}]}}"#,
                    barter("barter_1", 4),
                    barter("barter_2", 28),
                    barter("barter_3", 52)
                ),
                4096,
            )
            .unwrap(),
        )
        .unwrap();
    let advice = &lesson_out.get("results").unwrap().as_array().unwrap()[3];
    assert!(advice.to_json().contains("barter with miller went well 3 of 3 times"), "{}", advice.to_json());
    let grown_choice = arbiter
        .workflow(&obj! { "ops" => Value::Array(vec![obj! {
            "op" => "arbitrate",
            "args" => obj! { "goals" => grown_goals, "routine" => habit_entry },
        }]) })
        .unwrap();
    let grown_winner = grown_choice.get("results").unwrap().as_array().unwrap()[0].get("winner").unwrap().clone();
    assert_eq!(grown_winner.get("id"), Some(&Value::str("goal_guard_thief")), "{}", grown_choice.to_json());

    let final_task_state =
        task_mgr.workflow(&parse(r#"{"ops":[{"op":"get","args":{"id":"bake_bread_task"}}]}"#, 4096).unwrap()).unwrap();
    let final_body_state =
        body_mgr.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"being":"baker"}}]}"#, 4096).unwrap()).unwrap();
    let final_skills_state =
        skills_mgr.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"being":"baker"}}]}"#, 4096).unwrap()).unwrap();
    let final_affect_state =
        affect_mgr.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"being":"baker"}}]}"#, 4096).unwrap()).unwrap();
    let final_mind_state =
        mind_mgr.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"subject":"baker"}}]}"#, 4096).unwrap()).unwrap();
    let final_dec_state =
        dec_ledger.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"speaker":"baker"}}]}"#, 4096).unwrap()).unwrap();
    let final_rel_state =
        rel_mgr.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"subject":"baker"}}]}"#, 4096).unwrap()).unwrap();
    let final_off_state =
        off_res.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"being":"apprentice"}}]}"#, 4096).unwrap()).unwrap();
    let final_cur_state =
        cur_mgr.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"being":"baker"}}]}"#, 4096).unwrap()).unwrap();

    let final_explain_state =
        tracer.workflow(&parse(r#"{"ops":[{"op":"query_trace","args":{"being":"baker"}}]}"#, 4096).unwrap()).unwrap();
    let final_stability_state =
        monitor.workflow(&parse(r#"{"ops":[{"op":"assess_health"},{"op":"get_warnings"}]}"#, 4096).unwrap()).unwrap();

    let final_speech_state = speech
        .workflow(
            &parse(
                r#"{"ops":[{"op":"assess","args":{"being":"apprentice"}},{"op":"query","args":{"being":"baker"}},{"op":"query","args":{"being":"apprentice"}}]}"#,
                4096,
            )
            .unwrap(),
        )
        .unwrap();

    let final_diet_state =
        diet.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"being":"baker"}}]}"#, 4096).unwrap()).unwrap();

    let final_creed_state =
        creeds.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"being":"baker"}}]}"#, 4096).unwrap()).unwrap();

    let final_expect_state =
        book.workflow(&parse(r#"{"ops":[{"op":"assess","args":{"being":"baker"}}]}"#, 4096).unwrap()).unwrap();

    let composite_output = obj! {
        "final_growth" => obj! { "advice" => advice.clone(), "winner" => grown_winner },
        "final_expectations" => final_expect_state,
        "final_attention" => noticed.clone(),
        "final_creed" => final_creed_state,
        "final_curiosity" => final_cur_state,
        "final_diet" => final_diet_state,
        "final_speech" => final_speech_state,
        "final_explain" => final_explain_state,
        "final_stability" => final_stability_state,
        "final_offscreen" => final_off_state,
        "final_task" => final_task_state,
        "final_body" => final_body_state,
        "final_skills" => final_skills_state,
        "final_affect" => final_affect_state,
        "final_mind" => final_mind_state,
        "final_dec" => final_dec_state,
        "final_rel" => final_rel_state,
        "witness_outcome" => baker_outcome.clone(),
        "winning_motive" => winning_motive.clone()
    };

    composite_output.to_json()
}

#[test]
fn test_baker_acceptance_scene_replays_byte_identical() {
    let run1 = run_baker_scene();
    let run2 = run_baker_scene();
    assert_eq!(run1, run2, "Baker acceptance scene must replay to byte-identical state");
    assert!(run1.contains(r#""status":"done""#));
    assert!(run1.contains(r#""witnessed""#) || run1.contains(r#""baker""#));
}
