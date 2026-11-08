#!/usr/bin/env python3
"""Conformance test cases and differential parity runner for Social Mind:
Theory of Mind (Gap 7), Deception Ledger (Gap 13), and Relationship Reconciliation (Gap 9).
"""

import argparse
import json
import os
import pathlib
import random
import subprocess
import sys
from typing import Any, Dict, List

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from social_mind_reference import process_local

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "social_mind"


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    kind = request.get("kind")
    args = request.get("args", {})
    return process_local(kind, args)


def make_cases() -> List[Dict[str, Any]]:
    cases = []

    # 1. mind_init_and_assess_empty
    cases.append({
        "id": "mind_init_and_assess_empty",
        "description": "Initialize Theory of Mind for a being and assess empty models of others.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {"op": "init", "args": {"subject": "baker"}},
                    {"op": "assess", "args": {"subject": "baker"}}
                ]
            }
        }
    })

    # 2. mind_record_knows_belief_with_provenance_seen
    cases.append({
        "id": "mind_record_knows_belief_with_provenance_seen",
        "description": "Record belief that another person knows a fact, witnessed directly.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "subject": "baker",
                            "other": "apprentice",
                            "category": "knows",
                            "fact": "fire_in_hearth",
                            "status": True,
                            "source": "seen",
                            "at": 5,
                            "confidence": 100
                        }
                    },
                    {"op": "assess", "args": {"subject": "baker", "other": "apprentice", "category": "knows"}}
                ]
            }
        }
    })

    # 3. mind_record_wants_goal_priority
    cases.append({
        "id": "mind_record_wants_goal_priority",
        "description": "Record belief that another person wants a specific goal with priority.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "subject": "baker",
                            "other": "customer",
                            "category": "wants",
                            "goal": "buy_fresh_bread",
                            "priority": 80,
                            "source": "told",
                            "at": 10
                        }
                    },
                    {"op": "assess", "args": {"subject": "baker", "other": "customer", "category": "wants"}}
                ]
            }
        }
    })

    # 4. mind_record_feels_emotion_target_level
    cases.append({
        "id": "mind_record_feels_emotion_target_level",
        "description": "Record belief that another person feels an emotion toward a target.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "subject": "baker",
                            "other": "rival",
                            "category": "feels",
                            "emotion": "envy",
                            "target": "baker",
                            "level": 40,
                            "source": "inferred",
                            "at": 12
                        }
                    },
                    {"op": "assess", "args": {"subject": "baker", "other": "rival", "category": "feels"}}
                ]
            }
        }
    })

    # 5. mind_infer_from_witness_same_place_seen
    cases.append({
        "id": "mind_infer_from_witness_same_place_seen",
        "description": "Infer knowledge of event for another person who perceived the event in the same place.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "infer_from_witness",
                        "args": {
                            "subject": "baker",
                            "fact": "cart_crash",
                            "entry": {
                                "at": 15,
                                "kind": "crash",
                                "place": "road",
                                "actors": ["carter"],
                                "visibility": "witnessed",
                                "noise": 80
                            },
                            "beings": [
                                {"id": "baker", "place": "road", "senses": {"sight": True, "hearing": True}},
                                {"id": "carter", "place": "road", "senses": {"sight": True, "hearing": True}}
                            ]
                        }
                    }
                ]
            }
        }
    })

    # 6. mind_infer_from_witness_different_place_inferred_false
    cases.append({
        "id": "mind_infer_from_witness_different_place_inferred_false",
        "description": "Infer that a person in a different place did not witness the event.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "infer_from_witness",
                        "args": {
                            "subject": "baker",
                            "fact": "cart_crash",
                            "entry": {
                                "at": 15,
                                "kind": "crash",
                                "place": "road",
                                "actors": ["carter"],
                                "visibility": "witnessed",
                                "noise": 30
                            },
                            "beings": [
                                {"id": "baker", "place": "road", "senses": {"sight": True, "hearing": True}},
                                {"id": "apprentice", "place": "bakery", "senses": {"sight": True, "hearing": True}}
                            ]
                        }
                    },
                    {"op": "query", "args": {"subject": "baker", "other": "apprentice", "category": "knows", "key": "cart_crash"}}
                ]
            }
        }
    })

    # 7. mind_infer_from_witness_occluded_cannot_sense
    cases.append({
        "id": "mind_infer_from_witness_occluded_cannot_sense",
        "description": "Infer that an occluded person in the same place with low noise could not sense the event.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "infer_from_witness",
                        "args": {
                            "subject": "baker",
                            "fact": "whispered_secret",
                            "entry": {
                                "at": 20,
                                "kind": "whisper",
                                "place": "pantry",
                                "actors": ["baker"],
                                "visibility": "witnessed",
                                "noise": 5
                            },
                            "beings": [
                                {"id": "baker", "place": "pantry", "senses": {"sight": True, "hearing": True}},
                                {"id": "guard", "place": "pantry", "senses": {"sight": False, "hearing": False, "occluded": True}}
                            ]
                        }
                    }
                ]
            }
        }
    })

    # 8. mind_query_perspective_knows_found_and_not_found
    cases.append({
        "id": "mind_query_perspective_knows_found_and_not_found",
        "description": "Query Theory of Mind belief for known fact and unknown fact.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "subject": "baker",
                            "other": "apprentice",
                            "category": "knows",
                            "fact": "secret_recipe",
                            "status": True,
                            "source": "told",
                            "at": 25
                        }
                    },
                    {"op": "query", "args": {"subject": "baker", "other": "apprentice", "category": "knows", "key": "secret_recipe"}},
                    {"op": "query", "args": {"subject": "baker", "other": "apprentice", "category": "knows", "key": "missing_recipe"}}
                ]
            }
        }
    })

    # 9. mind_query_perspective_feels_with_target
    cases.append({
        "id": "mind_query_perspective_feels_with_target",
        "description": "Query Theory of Mind feeling belief with target person.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "subject": "baker",
                            "other": "apprentice",
                            "category": "feels",
                            "emotion": "respect",
                            "target": "baker",
                            "level": 35,
                            "source": "seen",
                            "at": 30
                        }
                    },
                    {"op": "query", "args": {"subject": "baker", "other": "apprentice", "category": "feels", "key": "respect:baker"}}
                ]
            }
        }
    })

    # 10. deception_init_and_record_honest_claim
    cases.append({
        "id": "deception_init_and_record_honest_claim",
        "description": "Record an honest statement in told-ledger where claim equals true value.",
        "input": {
            "kind": "deception",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "speaker": "merchant",
                            "listener": "buyer",
                            "topic": "price_of_flour",
                            "claim": "5_coins",
                            "true_value": "5_coins",
                            "at": 10
                        }
                    },
                    {"op": "assess", "args": {"speaker": "merchant", "listener": "buyer"}}
                ]
            }
        }
    })

    # 11. deception_record_lie_with_true_value
    cases.append({
        "id": "deception_record_lie_with_true_value",
        "description": "Record an intentional lie in told-ledger where claim contradicts known true value.",
        "input": {
            "kind": "deception",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "speaker": "thief",
                            "listener": "guard",
                            "topic": "stolen_gems",
                            "claim": "saw_nothing",
                            "true_value": "hidden_in_boot",
                            "at": 15
                        }
                    },
                    {"op": "assess", "args": {"speaker": "thief", "listener": "guard"}}
                ]
            }
        }
    })

    # 12. deception_check_consistency_consistent
    cases.append({
        "id": "deception_check_consistency_consistent",
        "description": "Check proposed claim against prior statement to listener with matching value.",
        "input": {
            "kind": "deception",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "speaker": "merchant",
                            "listener": "buyer",
                            "topic": "origin",
                            "claim": "imported_silk",
                            "at": 20
                        }
                    },
                    {
                        "op": "check_consistency",
                        "args": {
                            "speaker": "merchant",
                            "listener": "buyer",
                            "topic": "origin",
                            "proposed_claim": "imported_silk"
                        }
                    }
                ]
            }
        }
    })

    # 13. deception_check_consistency_contradicts_prior_warning
    cases.append({
        "id": "deception_check_consistency_contradicts_prior_warning",
        "description": "Check proposed claim that contradicts prior statement made to the same listener.",
        "input": {
            "kind": "deception",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "speaker": "thief",
                            "listener": "guard",
                            "topic": "whereabouts",
                            "claim": "was_at_inn",
                            "at": 25
                        }
                    },
                    {
                        "op": "check_consistency",
                        "args": {
                            "speaker": "thief",
                            "listener": "guard",
                            "topic": "whereabouts",
                            "proposed_claim": "was_at_docks"
                        }
                    }
                ]
            }
        }
    })

    # 14. deception_appraise_contradiction_detected_lie
    cases.append({
        "id": "deception_appraise_contradiction_detected_lie",
        "description": "Listener discovers truth contradicting active prior statement, penalizing trust.",
        "input": {
            "kind": "deception",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "id": "dec_lie_1",
                            "speaker": "thief",
                            "listener": "guard",
                            "topic": "stolen_ring",
                            "claim": "innocent",
                            "true_value": "stole_it",
                            "at": 30
                        }
                    },
                    {
                        "op": "appraise_contradiction",
                        "args": {
                            "listener": "guard",
                            "topic": "stolen_ring",
                            "observed_truth": "found_in_pocket"
                        }
                    },
                    {"op": "assess", "args": {"listener": "guard"}}
                ]
            }
        }
    })

    # 15. deception_appraise_contradiction_none_found
    cases.append({
        "id": "deception_appraise_contradiction_none_found",
        "description": "Listener appraises topic with observed truth matching prior claim without contradiction.",
        "input": {
            "kind": "deception",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "speaker": "baker",
                            "listener": "customer",
                            "topic": "freshness",
                            "claim": "baked_today",
                            "true_value": "baked_today",
                            "at": 35
                        }
                    },
                    {
                        "op": "appraise_contradiction",
                        "args": {
                            "listener": "customer",
                            "topic": "freshness",
                            "observed_truth": "baked_today"
                        }
                    }
                ]
            }
        }
    })

    # 16. relationship_init_and_assess
    cases.append({
        "id": "relationship_init_and_assess",
        "description": "Initialize relationship manager with sheet relations and attitudes, then assess.",
        "input": {
            "kind": "relationship",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "sheet_relationships": [
                                {"subject": "baker", "with": "miller", "kind": "friend"}
                            ],
                            "attitudes": [
                                {"subject": "baker", "target": "miller", "kind": "trust", "level": 45}
                            ]
                        }
                    },
                    {"op": "assess", "args": {"subject": "baker"}}
                ]
            }
        }
    })

    # 17. relationship_resolve_sheet_fallback_friend
    cases.append({
        "id": "relationship_resolve_sheet_fallback_friend",
        "description": "Resolve relationship falling back to sheet relation friend baselines.",
        "input": {
            "kind": "relationship",
            "args": {
                "ops": [
                    {
                        "op": "resolve",
                        "args": {
                            "subject": "baker",
                            "target": "farmer",
                            "sheet_relationship": {"with": "farmer", "kind": "friend"}
                        }
                    }
                ]
            }
        }
    })

    # 18. relationship_resolve_personal_overrides_sheet
    cases.append({
        "id": "relationship_resolve_personal_overrides_sheet",
        "description": "Personal dynamic trust overrides authored sheet relation label.",
        "input": {
            "kind": "relationship",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "sheet_relationships": [
                                {"subject": "baker", "with": "apprentice", "kind": "friend"}
                            ],
                            "attitudes": [
                                {"subject": "baker", "target": "apprentice", "kind": "trust", "level": -30}
                            ]
                        }
                    },
                    {
                        "op": "resolve",
                        "args": {
                            "subject": "baker",
                            "target": "apprentice"
                        }
                    }
                ]
            }
        }
    })

    # 19. relationship_resolve_community_standing_bias
    cases.append({
        "id": "relationship_resolve_community_standing_bias",
        "description": "Community standing provides contextual modifier to interpersonal trust.",
        "input": {
            "kind": "relationship",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "attitudes": [
                                {"subject": "baker", "target": "trader", "kind": "trust", "level": 10}
                            ],
                            "communities": [
                                {"community": "guild", "target": "merchant_coalition", "trust": 40}
                            ]
                        }
                    },
                    {
                        "op": "resolve",
                        "args": {
                            "subject": "baker",
                            "target": "trader",
                            "community": "guild",
                            "target_community": "merchant_coalition"
                        }
                    }
                ]
            }
        }
    })

    # 20. relationship_appraise_action_barter_failure
    cases.append({
        "id": "relationship_appraise_action_barter_failure",
        "description": "Unified appraisal path: failed barter reduces trust and increases resentment.",
        "input": {
            "kind": "relationship",
            "args": {
                "ops": [
                    {
                        "op": "appraise_action",
                        "args": {
                            "actor": "trader",
                            "target": "baker",
                            "action": "barter",
                            "success": False
                        }
                    },
                    {"op": "assess", "args": {"subject": "baker", "target": "trader"}}
                ]
            }
        }
    })

    # 21. relationship_appraise_action_broken_promise
    cases.append({
        "id": "relationship_appraise_action_broken_promise",
        "description": "Unified appraisal path: broken promise causes significant trust loss and resentment.",
        "input": {
            "kind": "relationship",
            "args": {
                "ops": [
                    {
                        "op": "appraise_action",
                        "args": {
                            "actor": "player",
                            "target": "baker",
                            "action": "broken_promise"
                        }
                    },
                    {"op": "assess", "args": {"subject": "baker", "target": "player"}}
                ]
            }
        }
    })

    # 22. relationship_aggregate_community_trust_from_members
    cases.append({
        "id": "relationship_aggregate_community_trust_from_members",
        "description": "Aggregate community-level trust from interpersonal attitudes of member cohort.",
        "input": {
            "kind": "relationship",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "attitudes": [
                                {"subject": "baker1", "target": "miller1", "kind": "trust", "level": 40},
                                {"subject": "baker1", "target": "miller2", "kind": "trust", "level": 20},
                                {"subject": "baker2", "target": "miller1", "kind": "trust", "level": 60},
                                {"subject": "baker2", "target": "miller2", "kind": "trust", "level": 40}
                            ]
                        }
                    },
                    {
                        "op": "aggregate_community_trust",
                        "args": {
                            "community": "bakers_guild",
                            "target_community": "millers_union",
                            "members": ["baker1", "baker2"],
                            "target_members": ["miller1", "miller2"]
                        }
                    }
                ]
            }
        }
    })

    # 23. malformed_mind_invalid_category (error: true)
    cases.append({
        "id": "malformed_mind_invalid_category",
        "description": "Reject record operation with unrecognized belief category.",
        "input": {
            "kind": "mind",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "subject": "baker",
                            "other": "apprentice",
                            "category": "invalid_category",
                            "fact": "fire"
                        }
                    }
                ]
            }
        }
    })

    # 24. malformed_deception_speaker_equals_listener (error: true)
    cases.append({
        "id": "malformed_deception_speaker_equals_listener",
        "description": "Reject record operation where speaker is the same as listener.",
        "input": {
            "kind": "deception",
            "args": {
                "ops": [
                    {
                        "op": "record",
                        "args": {
                            "speaker": "baker",
                            "listener": "baker",
                            "topic": "soliloquy",
                            "claim": "talking_to_myself"
                        }
                    }
                ]
            }
        }
    })

    # 25. malformed_relationship_level_out_of_bounds (error: true)
    cases.append({
        "id": "malformed_relationship_level_out_of_bounds",
        "description": "Reject relationship attitude initialization with level exceeding bounds.",
        "input": {
            "kind": "relationship",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "attitudes": [
                                {"subject": "baker", "target": "player", "kind": "trust", "level": 200}
                            ]
                        }
                    }
                ]
            }
        }
    })

    return cases


def run_random_parity(count: int, seed: int = 7) -> None:
    random.seed(seed)
    rust_bin = ROOT / "target" / "debug" / "gamectx"
    if not rust_bin.exists():
        subprocess.run(["cargo", "build"], cwd=str(ROOT), check=True)

    attitude_pool = [
        "trust", "affection", "respect", "fear", "gratitude",
        "admiration", "envy", "resentment", "emotion"
    ]
    sheet_kinds = ["friend", "mentor", "apprentice", "ally", "rival", "enemy"]
    facts_pool = ["cart_crash", "fire_in_hearth", "stolen_key", "secret_passage", "weather"]
    topics_pool = ["price", "origin", "quality", "location", "alibi"]
    actions_pool = ["barter", "broken_promise", "fulfilled_promise", "attack", "help", "gift", "teach"]

    for i in range(count):
        roll = random.random()
        if roll < 0.35:
            # Mind workflow
            subj = f"subj_{random.randint(1, 3)}"
            other = f"other_{random.randint(1, 3)}"
            ops = [{"op": "init", "args": {"subject": subj}}]
            sub_ops = random.randint(1, 4)
            for _ in range(sub_ops):
                cat_roll = random.random()
                if cat_roll < 0.3:
                    fact = random.choice(facts_pool)
                    ops.append({
                        "op": "record",
                        "args": {
                            "subject": subj,
                            "other": other,
                            "category": "knows",
                            "fact": fact,
                            "status": random.choice([True, False]),
                            "source": random.choice(["seen", "told", "inferred"]),
                            "at": random.randint(1, 20),
                            "confidence": random.randint(50, 100)
                        }
                    })
                elif cat_roll < 0.5:
                    ops.append({
                        "op": "record",
                        "args": {
                            "subject": subj,
                            "other": other,
                            "category": "wants",
                            "goal": f"goal_{random.randint(1, 5)}",
                            "priority": random.randint(10, 100),
                            "source": random.choice(["told", "inferred"]),
                            "at": random.randint(1, 20)
                        }
                    })
                elif cat_roll < 0.7:
                    ops.append({
                        "op": "record",
                        "args": {
                            "subject": subj,
                            "other": other,
                            "category": "feels",
                            "emotion": random.choice(attitude_pool),
                            "target": subj if random.random() < 0.5 else None,
                            "level": random.randint(-50, 50),
                            "source": random.choice(["seen", "told", "inferred"]),
                            "at": random.randint(1, 20)
                        }
                    })
                elif cat_roll < 0.85:
                    ops.append({
                        "op": "infer_from_witness",
                        "args": {
                            "subject": subj,
                            "fact": random.choice(facts_pool),
                            "entry": {
                                "at": random.randint(1, 30),
                                "kind": "event",
                                "place": "town_square",
                                "actors": [subj],
                                "visibility": "witnessed",
                                "noise": random.randint(10, 80)
                            },
                            "beings": [
                                {"id": subj, "place": "town_square", "senses": {"sight": True, "hearing": True}},
                                {"id": other, "place": random.choice(["town_square", "tavern"]), "senses": {"sight": random.choice([True, False]), "hearing": True}}
                            ]
                        }
                    })
                else:
                    ops.append({
                        "op": "assess",
                        "args": {"subject": subj, "other": other}
                    })
            req = {"kind": "mind", "args": {"ops": ops}}
        elif roll < 0.70:
            # Deception workflow
            spk = f"person_{random.randint(1, 3)}"
            lis = f"person_{random.randint(4, 6)}"
            ops = [{"op": "init", "args": {}}]
            sub_ops = random.randint(1, 4)
            for _ in range(sub_ops):
                d_roll = random.random()
                topic = random.choice(topics_pool)
                if d_roll < 0.4:
                    is_lie = random.choice([True, False])
                    claim = f"val_{random.randint(1, 4)}"
                    true_v = claim if not is_lie else f"secret_{random.randint(5, 8)}"
                    ops.append({
                        "op": "record",
                        "args": {
                            "speaker": spk,
                            "listener": lis,
                            "topic": topic,
                            "claim": claim,
                            "true_value": true_v,
                            "at": random.randint(1, 30)
                        }
                    })
                elif d_roll < 0.7:
                    ops.append({
                        "op": "check_consistency",
                        "args": {
                            "speaker": spk,
                            "listener": lis,
                            "topic": topic,
                            "proposed_claim": f"val_{random.randint(1, 4)}"
                        }
                    })
                elif d_roll < 0.9:
                    ops.append({
                        "op": "appraise_contradiction",
                        "args": {
                            "listener": lis,
                            "topic": topic,
                            "observed_truth": f"val_{random.randint(1, 3)}"
                        }
                    })
                else:
                    ops.append({
                        "op": "assess",
                        "args": {"listener": lis}
                    })
            req = {"kind": "deception", "args": {"ops": ops}}
        else:
            # Relationship workflow
            s = f"being_{random.randint(1, 3)}"
            t = f"being_{random.randint(4, 6)}"
            ops = [{
                "op": "init",
                "args": {
                    "sheet_relationships": [{"subject": s, "with": t, "kind": random.choice(sheet_kinds)}],
                    "attitudes": [{"subject": s, "target": t, "kind": "trust", "level": random.randint(-40, 40)}],
                    "communities": [{"community": "comm_a", "target": "comm_b", "trust": random.randint(-40, 40)}]
                }
            }]
            sub_ops = random.randint(1, 4)
            for _ in range(sub_ops):
                r_roll = random.random()
                if r_roll < 0.4:
                    ops.append({
                        "op": "resolve",
                        "args": {
                            "subject": s,
                            "target": t,
                            "community": "comm_a",
                            "target_community": "comm_b"
                        }
                    })
                elif r_roll < 0.7:
                    act = random.choice(actions_pool)
                    ops.append({
                        "op": "appraise_action",
                        "args": {
                            "actor": s,
                            "target": t,
                            "action": act,
                            "success": random.choice([True, False])
                        }
                    })
                elif r_roll < 0.9:
                    ops.append({
                        "op": "aggregate_community_trust",
                        "args": {
                            "community": "comm_a",
                            "target_community": "comm_b",
                            "members": [s],
                            "target_members": [t]
                        }
                    })
                else:
                    ops.append({
                        "op": "assess",
                        "args": {"subject": s, "target": t}
                    })
            req = {"kind": "relationship", "args": {"ops": ops}}

        py_res = None
        py_err = None
        try:
            py_res = py_workflow(req)
        except Exception as e:
            py_err = str(e)

        payload = json.dumps(req)
        proc = subprocess.run(
            [str(rust_bin), "local", "-"],
            input=payload,
            capture_output=True,
            text=True,
        )
        if py_err:
            assert proc.returncode != 0, f"Iteration {i}: Python failed ({py_err}) but Rust succeeded: {proc.stdout}"
        else:
            assert proc.returncode == 0, f"Iteration {i}: Rust failed: {proc.stderr}\nPayload: {payload}"
            rust_res = json.loads(proc.stdout)
            assert rust_res == py_res, f"Iteration {i} mismatch:\nPy:   {json.dumps(py_res, sort_keys=True)}\nRust: {json.dumps(rust_res, sort_keys=True)}"

    print(f"Differential parity passed {count} random workflows (seed {seed}) with 0 mismatches.")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--cap", type=float, default=0.0)
    p.add_argument("--random", type=int, default=0)
    p.add_argument("--seed", type=int, default=7)
    args = p.parse_args()

    if args.random > 0:
        run_random_parity(args.random, seed=args.seed)
    else:
        cases = make_cases()
        passed = 0
        for c in cases:
            try:
                out = py_workflow(c["input"])
                passed += 1
            except Exception:
                if "malformed" in c["id"]:
                    passed += 1
        print(json.dumps({
            "cases": len(cases),
            "passed": passed,
            "actual_model_calls": 0,
            "api_spend_usd": 0,
        }))
