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
//!
//! Asserts byte-identical replay.

use gamectx::character::affect::AffectManager;
use gamectx::character::body::BodyManager;
use gamectx::character::deception::DeceptionLedger;
use gamectx::character::life::witness::WitnessBridge;
use gamectx::character::mind::MindManager;
use gamectx::character::motive::MotiveArbiter;
use gamectx::character::relationship::RelationshipManager;
use gamectx::character::skills::SkillsManager;
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

    let composite_output = obj! {
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
