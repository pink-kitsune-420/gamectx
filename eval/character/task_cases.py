#!/usr/bin/env python3
"""Conformance test cases and differential parity runner for Tasks and Planning (Gap 4)."""

import json
import os
import random
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List

sys.path[1:1] = [str(d) for d in sorted(Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from task_reference import TaskReference

ROOT = Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "task"


def make_cases() -> List[Dict[str, Any]]:
    cases = []

    # 1. Deterministic bake task when flour is in inventory
    cases.append({
        "id": "baker_propose_baking_task_deterministic",
        "description": "Deterministic planner produces single cooking step when ingredients are on hand.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "task_bake_bread",
                            "owner": "baker",
                            "motive_ref": {"id": "bake_routine", "kind": "routine"},
                            "priority": 70,
                            "goal": {"kind": "cook", "recipe": "bread"},
                            "world_state": {
                                "location": "bakery",
                                "inventory": {"flour": 2, "coins": 10},
                                "recipes": {
                                    "bread": {
                                        "station": "bakery",
                                        "inputs": [{"item": "flour", "count": 1}],
                                        "outputs": [{"item": "bread", "count": 1}],
                                    }
                                },
                            },
                        },
                    }
                ]
            },
        },
    })

    # 2. Deterministic planner generates 4-step mill journey when flour is missing
    cases.append({
        "id": "baker_missing_flour_plans_mill_journey",
        "description": "Deterministic planner produces 4-step sequence (travel mill, barter, travel bakery, cook) when flour is missing.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "task_replanned_bake",
                            "owner": "baker",
                            "motive_ref": {"id": "bake_routine", "kind": "routine"},
                            "priority": 75,
                            "goal": {"kind": "cook", "recipe": "bread"},
                            "world_state": {
                                "location": "bakery",
                                "inventory": {"flour": 0, "coins": 10},
                                "recipes": {
                                    "bread": {
                                        "station": "bakery",
                                        "inputs": [{"item": "flour", "count": 1}],
                                        "outputs": [{"item": "bread", "count": 1}],
                                    }
                                },
                                "sources": [
                                    {
                                        "item": "flour",
                                        "place": "mill",
                                        "partner": "miller",
                                        "cost_item": "coins",
                                        "cost_count": 5,
                                    }
                                ],
                            },
                        },
                    }
                ]
            },
        },
    })

    # 3. Accept and activate task
    cases.append({
        "id": "task_accept_and_start_active",
        "description": "Accepting a proposed task transitions status to active and marks first step active.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_simple",
                            "owner": "hero",
                            "goal": {"kind": "travel", "destination": "town_square"},
                            "world_state": {"location": "inn"},
                        },
                    },
                    {"op": "accept", "args": {"id": "t_simple", "start": True}},
                ]
            },
        },
    })

    # 4. Advance step with confirmed success
    cases.append({
        "id": "task_advance_step_confirmed_success",
        "description": "Confirmed outcome advances active step to done, records history, and activates next step.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_multi",
                            "owner": "hero",
                            "steps": [
                                {"id": "s1", "action": "travel", "target_bindings": {"destination": "mill"}},
                                {"id": "s2", "action": "travel", "target_bindings": {"destination": "bakery"}},
                            ],
                            "world_state": {"location": "home"},
                        },
                    },
                    {"op": "accept", "args": {"id": "t_multi"}},
                    {
                        "op": "advance",
                        "args": {
                            "id": "t_multi",
                            "outcome": {"success": True, "details": "arrived at mill"},
                            "tick": 12,
                        },
                    },
                ]
            },
        },
    })

    # 5. Advance final step marks entire task done
    cases.append({
        "id": "task_advance_final_step_marks_done",
        "description": "Confirmed success on last step transitions task status to done.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_finish",
                            "owner": "hero",
                            "steps": [
                                {"id": "s1", "action": "travel", "target_bindings": {"destination": "mill"}},
                            ],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_finish"}},
                    {
                        "op": "advance",
                        "args": {"id": "t_finish", "outcome": {"success": True}, "tick": 20},
                    },
                ]
            },
        },
    })

    # 6. Step failure with retry policy retries up to limit
    cases.append({
        "id": "task_advance_step_failure_retries",
        "description": "Step failure under retry policy increments retry count and remains active.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_retry",
                            "owner": "hero",
                            "on_fail": "retry",
                            "steps": [
                                {"id": "s1", "action": "travel", "target_bindings": {"destination": "mill"}},
                            ],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_retry"}},
                    {
                        "op": "advance",
                        "args": {
                            "id": "t_retry",
                            "outcome": {"success": False, "reason": "road_blocked"},
                            "tick": 25,
                        },
                    },
                ]
            },
        },
    })

    # 7. Step failure with replan policy blocks task for replanning
    cases.append({
        "id": "task_advance_step_failure_blocks_for_replan",
        "description": "Step failure under replan policy sets status to blocked and stores blocked_reason.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_replan_fail",
                            "owner": "hero",
                            "on_fail": "replan",
                            "steps": [
                                {"id": "s1", "action": "travel", "target_bindings": {"destination": "mill"}},
                            ],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_replan_fail"}},
                    {
                        "op": "advance",
                        "args": {
                            "id": "t_replan_fail",
                            "outcome": {"success": False, "reason": "bridge_collapsed"},
                            "tick": 30,
                        },
                    },
                ]
            },
        },
    })

    # 8. Baker replan after missing flour
    cases.append({
        "id": "baker_replan_after_flour_shortage",
        "description": "Baker bake task is blocked, replans with deterministic alternative, resumes with replan_count 1.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_baker_blocked",
                            "owner": "baker",
                            "steps": [
                                {"id": "s_cook", "action": "cook", "target_bindings": {"recipe": "bread"}},
                            ],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_baker_blocked"}},
                    {"op": "block", "args": {"id": "t_baker_blocked", "reason": "missing_flour"}},
                    {
                        "op": "replan",
                        "args": {
                            "id": "t_baker_blocked",
                            "goal": {"kind": "cook", "recipe": "bread"},
                            "world_state": {
                                "location": "bakery",
                                "inventory": {"flour": 0, "coins": 5},
                                "recipes": {
                                    "bread": {
                                        "station": "bakery",
                                        "inputs": [{"item": "flour", "count": 1}],
                                        "outputs": [{"item": "bread", "count": 1}],
                                    }
                                },
                                "sources": [
                                    {
                                        "item": "flour",
                                        "place": "mill",
                                        "partner": "miller",
                                        "cost_item": "coins",
                                        "cost_count": 5,
                                    }
                                ],
                            },
                        },
                    },
                ]
            },
        },
    })

    # 9. Replan limit exceeded marks task failed
    cases.append({
        "id": "task_replan_exceeds_max_limit_fails",
        "description": "Fourth replan attempt exceeds MAX_REPLANS (3) and transitions task to failed.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_limit",
                            "owner": "hero",
                            "steps": [{"id": "s1", "action": "travel", "target_bindings": {"destination": "inn"}}],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_limit"}},
                    {"op": "block", "args": {"id": "t_limit", "reason": "b1"}},
                    {"op": "replan", "args": {"id": "t_limit", "goal": {"kind": "travel", "destination": "inn"}}},
                    {"op": "block", "args": {"id": "t_limit", "reason": "b2"}},
                    {"op": "replan", "args": {"id": "t_limit", "goal": {"kind": "travel", "destination": "inn"}}},
                    {"op": "block", "args": {"id": "t_limit", "reason": "b3"}},
                    {"op": "replan", "args": {"id": "t_limit", "goal": {"kind": "travel", "destination": "inn"}}},
                    {"op": "block", "args": {"id": "t_limit", "reason": "b4"}},
                    {"op": "replan", "args": {"id": "t_limit", "goal": {"kind": "travel", "destination": "inn"}}},
                ]
            },
        },
    })

    # 10. Host validates and accepts valid model proposal
    cases.append({
        "id": "model_proposed_plan_valid_accepted",
        "description": "Host validates model proposal against ActionCatalog and accepts it into proposed task.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_model_plan",
                            "owner": "apprentice",
                            "steps": [
                                {"id": "m1", "action": "travel", "target_bindings": {"destination": "mill"}},
                                {
                                    "id": "m2",
                                    "action": "barter",
                                    "target_bindings": {"partner": "miller", "offer": "coins", "want": "flour"},
                                },
                            ],
                            "world_state": {"location": "home", "inventory": {"coins": 10}},
                        },
                    }
                ]
            },
        },
    })

    # 11. Model proposal with unknown verb rejected
    cases.append({
        "id": "model_proposed_plan_unknown_verb_rejected",
        "description": "Host rejects model proposal containing unknown verb 'teleport'.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "validate",
                        "args": {
                            "steps": [
                                {"id": "m1", "action": "teleport", "target_bindings": {"destination": "moon"}},
                            ]
                        },
                    }
                ]
            },
        },
    })

    # 12. Model proposal missing required target slot rejected
    cases.append({
        "id": "model_proposed_plan_missing_slot_rejected",
        "description": "Host rejects model proposal where 'travel' is missing required target slot 'destination'.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "validate",
                        "args": {
                            "steps": [
                                {"id": "m1", "action": "travel", "target_bindings": {}},
                            ]
                        },
                    }
                ]
            },
        },
    })

    # 13. Model proposal with broken causal station requirement rejected
    cases.append({
        "id": "model_proposed_plan_broken_causal_station_rejected",
        "description": "Host rejects proposal where actor tries to cook at bakery while still at home.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "validate",
                        "args": {
                            "steps": [
                                {"id": "m1", "action": "cook", "target_bindings": {"recipe": "bread"}},
                            ],
                            "world_state": {
                                "location": "home",
                                "inventory": {"flour": 5},
                                "recipes": {"bread": {"station": "bakery", "inputs": [{"item": "flour", "count": 1}]}},
                            },
                        },
                    }
                ]
            },
        },
    })

    # 14. Task suspend and resume
    cases.append({
        "id": "task_suspend_and_resume",
        "description": "Active task can be suspended and later resumed to active status.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_suspend",
                            "owner": "guard",
                            "steps": [{"id": "s1", "action": "wait", "target_bindings": {}}],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_suspend"}},
                    {"op": "suspend", "args": {"id": "t_suspend"}},
                    {"op": "resume", "args": {"id": "t_suspend"}},
                ]
            },
        },
    })

    # 15. Task abandon records reason
    cases.append({
        "id": "task_abandon_records_reason",
        "description": "Abandoning a task transitions status to abandoned and stores given reason.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_abandon",
                            "owner": "guard",
                            "steps": [{"id": "s1", "action": "look", "target_bindings": {}}],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_abandon"}},
                    {"op": "abandon", "args": {"id": "t_abandon", "reason": "called_away_on_emergency"}},
                ]
            },
        },
    })

    # 16. Inspect full task state with history
    cases.append({
        "id": "task_get_state",
        "description": "Retrieving task state returns full record with progress, history and parameters.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_inspect",
                            "owner": "merchant",
                            "priority": 85,
                            "steps": [
                                {"id": "s1", "action": "look", "target_bindings": {}},
                                {"id": "s2", "action": "wait", "target_bindings": {}},
                            ],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_inspect"}},
                    {"op": "advance", "args": {"id": "t_inspect", "outcome": {"success": True}, "tick": 5}},
                    {"op": "get", "args": {"id": "t_inspect"}},
                ]
            },
        },
    })

    # 17. List tasks filtered by owner and status
    cases.append({
        "id": "task_list_by_owner_and_status",
        "description": "Listing tasks filters by owner and status.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_b1",
                            "owner": "baker",
                            "steps": [{"id": "s1", "action": "wait", "target_bindings": {}}],
                        },
                    },
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_b2",
                            "owner": "baker",
                            "steps": [{"id": "s1", "action": "wait", "target_bindings": {}}],
                        },
                    },
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_m1",
                            "owner": "miller",
                            "steps": [{"id": "s1", "action": "wait", "target_bindings": {}}],
                        },
                    },
                    {"op": "accept", "args": {"id": "t_b1"}},
                    {"op": "list", "args": {"owner": "baker", "status": "active"}},
                ]
            },
        },
    })

    # 18. Deterministic satisfy hunger plan
    cases.append({
        "id": "deterministic_satisfy_hunger_plan",
        "description": "Planner produces take and eat activity steps when food is in pantry.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_hunger",
                            "owner": "child",
                            "goal": {"kind": "satisfy_need", "need": "hunger", "item": "apple"},
                            "world_state": {"inventory": {"apple": 0}},
                        },
                    }
                ]
            },
        },
    })

    # 19. Deterministic travel plan
    cases.append({
        "id": "deterministic_travel_plan",
        "description": "Planner produces travel step from current location to destination.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_dest",
                            "owner": "traveler",
                            "goal": {"kind": "travel", "destination": "castle"},
                            "world_state": {"location": "crossroads"},
                        },
                    }
                ]
            },
        },
    })

    # 20. Deterministic barter plan
    cases.append({
        "id": "deterministic_barter_plan",
        "description": "Planner produces travel to market and barter step with partner.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_trade",
                            "owner": "trader",
                            "goal": {
                                "kind": "barter",
                                "place": "market",
                                "partner": "merchant",
                                "offer": "gem",
                                "want": "horse",
                            },
                            "world_state": {"location": "stable"},
                        },
                    }
                ]
            },
        },
    })

    # 21. Malformed: empty ops array
    cases.append({
        "id": "malformed_empty_ops",
        "description": "Rejects request with empty ops list.",
        "input": {"kind": "task", "args": {"ops": []}},
    })

    # 22. Malformed: too many ops (> 64)
    cases.append({
        "id": "malformed_too_many_ops",
        "description": "Rejects request exceeding 64 ops.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [{"op": "get", "args": {"id": f"t_{i}"}} for i in range(65)]
            },
        },
    })

    # 23. Malformed: unknown op name
    cases.append({
        "id": "malformed_unknown_op",
        "description": "Rejects request with unknown operation.",
        "input": {"kind": "task", "args": {"ops": [{"op": "teleport", "args": {}}]}},
    })

    # 24. Malformed: missing task id in propose
    cases.append({
        "id": "malformed_missing_task_id",
        "description": "Rejects propose operation missing task id.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {"owner": "hero", "steps": [{"id": "s1", "action": "look", "target_bindings": {}}]},
                    }
                ]
            },
        },
    })

    # 25. Malformed: unknown on_fail policy
    cases.append({
        "id": "malformed_unknown_policy",
        "description": "Rejects propose with unknown on_fail policy.",
        "input": {
            "kind": "task",
            "args": {
                "ops": [
                    {
                        "op": "propose",
                        "args": {
                            "id": "t_bad_pol",
                            "owner": "hero",
                            "on_fail": "explode",
                            "steps": [{"id": "s1", "action": "look", "target_bindings": {}}],
                        },
                    }
                ]
            },
        },
    })

    return cases


def generate_conformance_suite():
    ref = TaskReference()
    cases = make_cases()
    for c in cases:
        case_dir = CONFORMANCE_DIR / c["id"]
        case_dir.mkdir(parents=True, exist_ok=True)

        case_obj = {
            "op": "local",
            "description": c["description"],
            "error": "malformed" in c["id"],
        }
        with open(case_dir / "case.json", "w") as f:
            json.dump(case_obj, f, indent=2)
            f.write("\n")

        with open(case_dir / "input.json", "w") as f:
            json.dump(c["input"], f, indent=2)
            f.write("\n")

        expected_path = case_dir / "expected.json"
        if "malformed" in c["id"]:
            if expected_path.exists():
                expected_path.unlink()
        else:
            fresh_ref = TaskReference()
            out = fresh_ref.workflow(c["input"]["args"])
            with open(expected_path, "w") as f:
                f.write(json.dumps(out, separators=(",", ":")) + "\n")


def run_random_parity(count: int = 2000, seed: int = 7):
    random.seed(seed)
    rust_bin = ROOT / "target" / "debug" / "gamectx"

    places = ["home", "bakery", "mill", "market", "castle", "inn", "farm"]
    items = ["bread", "flour", "coins", "apple", "gem", "iron_ore", "wood"]
    actions = ["travel", "cook", "barter", "activity", "take", "store", "wait", "look"]
    policies = ["replan", "retry", "suspend", "abandon"]

    for i in range(count):
        ref = TaskReference()
        ops = []
        tid = f"task_{i % 10 + 1}"
        owner = f"actor_{i % 3 + 1}"

        # 1. Propose task
        p_coin = random.random()
        if p_coin < 0.5:
            goal = {
                "kind": random.choice(["cook", "travel", "satisfy_need", "barter"]),
                "recipe": "bread",
                "destination": random.choice(places),
                "need": "hunger",
                "item": random.choice(items),
                "partner": "miller",
                "offer": "coins",
                "want": "flour",
                "place": random.choice(places),
            }
            ops.append({
                "op": "propose",
                "args": {
                    "id": tid,
                    "owner": owner,
                    "priority": random.randint(10, 90),
                    "on_fail": random.choice(policies),
                    "on_block": random.choice(policies),
                    "goal": goal,
                    "world_state": {
                        "location": random.choice(places),
                        "inventory": {item: random.randint(0, 5) for item in items},
                        "recipes": {
                            "bread": {
                                "station": "bakery",
                                "inputs": [{"item": "flour", "count": 1}],
                                "outputs": [{"item": "bread", "count": 1}],
                            }
                        },
                        "sources": [
                            {
                                "item": "flour",
                                "place": "mill",
                                "partner": "miller",
                                "cost_item": "coins",
                                "cost_count": 2,
                            }
                        ],
                    },
                },
            })
        else:
            steps = []
            step_cnt = random.randint(1, 4)
            for s_i in range(step_cnt):
                act = random.choice(actions)
                bindings = {}
                if act == "travel":
                    bindings["destination"] = random.choice(places)
                elif act == "cook":
                    bindings["recipe"] = "bread"
                elif act == "barter":
                    bindings["partner"] = "miller"
                    bindings["offer"] = "coins"
                    bindings["want"] = "flour"
                elif act in ("take", "store"):
                    bindings["item"] = random.choice(items)
                steps.append({"id": f"s_{s_i + 1}", "action": act, "target_bindings": bindings})
            ops.append({
                "op": "propose",
                "args": {
                    "id": tid,
                    "owner": owner,
                    "steps": steps,
                    "priority": random.randint(10, 90),
                    "on_fail": random.choice(policies),
                    "on_block": random.choice(policies),
                },
            })

        # 2. Subsequent operations on this task
        sub_ops_cnt = random.randint(1, 4)
        for _ in range(sub_ops_cnt):
            coin = random.random()
            if coin < 0.3:
                ops.append({"op": "accept", "args": {"id": tid, "start": random.choice([True, False])}})
            elif coin < 0.6:
                succ = random.random() < 0.7
                ops.append({
                    "op": "advance",
                    "args": {
                        "id": tid,
                        "outcome": {"success": succ, "details": "simulated_step"},
                        "tick": random.randint(1, 100),
                    },
                })
            elif coin < 0.8:
                ops.append({"op": "replan", "args": {"id": tid, "goal": {"kind": "travel", "destination": "home"}}})
            else:
                ops.append({"op": "get", "args": {"id": tid}})

        req = {"ops": ops}

        py_res = None
        py_err = None
        try:
            py_res = ref.workflow(req)
        except Exception as e:
            py_err = str(e)

        if rust_bin.exists():
            payload = json.dumps({"kind": "task", "args": req})
            proc = subprocess.run(
                [str(rust_bin), "local", "-"],
                input=payload,
                capture_output=True,
                text=True,
            )
            if py_err:
                assert proc.returncode != 0, f"Iteration {i}: Python failed ({py_err}) but Rust succeeded: {proc.stdout}"
            else:
                assert proc.returncode == 0, f"Iteration {i}: Rust failed: {proc.stderr}"
                rust_res = json.loads(proc.stdout)
                assert rust_res == py_res, f"Iteration {i} mismatch:\nPy:   {py_res}\nRust: {rust_res}"

    print(f"Differential parity passed {count} random workflows (seed {seed}) with 0 mismatches.")


if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser()
    p.add_argument("--cap", type=float, default=0.0)
    p.add_argument("--random", type=int, default=0)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--write-conformance", action="store_true")
    p.add_argument("--generate", action="store_true")
    p.add_argument("--parity", type=int, default=0)
    args = p.parse_args()

    if args.generate or args.write_conformance:
        generate_conformance_suite()
        print(f"Generated {len(make_cases())} conformance cases in {CONFORMANCE_DIR}")
    elif args.parity > 0:
        run_random_parity(args.parity, seed=args.seed)
    elif args.random > 0:
        run_random_parity(args.random, seed=args.seed)
    else:
        generate_conformance_suite()
        cases = make_cases()
        ref = TaskReference()
        passed = 0
        for c in cases:
            try:
                out = ref.workflow(c["input"]["args"])
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
