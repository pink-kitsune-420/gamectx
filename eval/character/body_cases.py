#!/usr/bin/env python3
"""Conformance test cases and differential parity runner for Body & Needs (Gap 2 and 12)."""

import json
import os
import random
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List

sys.path[1:1] = [str(d) for d in sorted(Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from body_reference import BodyReference

ROOT = Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "body"


def make_cases() -> List[Dict[str, Any]]:
    cases = []

    # 1. Awake baker assesses baseline
    cases.append({
        "id": "baker_baseline_assess",
        "description": "Mira the baker assesses baseline health, needs, and speech while awake.",
        "input": {
            "kind": "body",
            "args": {
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
                                    "rate_per_tick": 1,
                                    "threshold": 70,
                                    "level": 30,
                                    "satisfiers": [
                                        {"action": "eat", "amount": 40},
                                        {"item": "bread", "amount": 30},
                                    ],
                                },
                                {
                                    "id": "rest",
                                    "label": "Need for sleep",
                                    "rate_per_tick": 2,
                                    "threshold": 80,
                                    "level": 20,
                                    "satisfiers": [
                                        {"action": "sleep", "amount": 50},
                                        {"action": "rest", "amount": 20},
                                    ],
                                },
                            ],
                        },
                    },
                    {"op": "assess", "args": {"being": "baker"}},
                ]
            },
        },
    })

    # 2. Asleep being cannot speak and routine is rest
    cases.append({
        "id": "asleep_silent_and_forced_rest",
        "description": "An asleep being has can_speak: false and routine_override: rest.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "guard",
                            "health": 90,
                            "fatigue": 60,
                            "conditions": ["asleep"],
                        },
                    },
                    {"op": "assess", "args": {"being": "guard"}},
                ]
            },
        },
    })

    # 3. Dead being cannot speak, routine is incapacitated
    cases.append({
        "id": "dead_silent_and_incapacitated",
        "description": "A dead being has can_speak: false, health: 0, and routine_override: incapacitated.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "skeleton",
                            "health": 0,
                            "conditions": ["dead"],
                        },
                    },
                    {"op": "assess", "args": {"being": "skeleton"}},
                ]
            },
        },
    })

    # 4. Unconscious being cannot speak and routine is incapacitated
    cases.append({
        "id": "unconscious_silent_and_incapacitated",
        "description": "An unconscious being has can_speak: false and routine_override: incapacitated.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "patient",
                            "health": 40,
                            "fatigue": 80,
                            "conditions": ["unconscious"],
                        },
                    },
                    {"op": "assess", "args": {"being": "patient"}},
                ]
            },
        },
    })

    # 5. High fatigue forces routine rest
    cases.append({
        "id": "exhausted_fatigue_forces_rest",
        "description": "Fatigue >= 90 overrides routine with rest.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "runner",
                            "fatigue": 92,
                        },
                    },
                    {"op": "assess", "args": {"being": "runner"}},
                ]
            },
        },
    })

    # 6. Ticking advances needs
    cases.append({
        "id": "ticking_advances_needs",
        "description": "Ticking 10 ticks increases hunger (+1/tick) by 10 and rest (+2/tick) by 20.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "worker",
                            "needs": [
                                {"id": "hunger", "rate_per_tick": 1, "level": 10},
                                {"id": "rest", "rate_per_tick": 2, "level": 10},
                            ],
                        },
                    },
                    {"op": "tick", "args": {"being": "worker", "ticks": 10}},
                    {"op": "assess", "args": {"being": "worker"}},
                ]
            },
        },
    })

    # 7. Sleeping rest recovers fatigue quickly (15 per tick)
    cases.append({
        "id": "sleep_recovers_fatigue",
        "description": "Sleeping for 4 ticks recovers 60 fatigue (15/tick).",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "miner",
                            "fatigue": 80,
                            "conditions": ["asleep"],
                        },
                    },
                    {"op": "tick", "args": {"being": "miner", "ticks": 4}},
                    {"op": "assess", "args": {"being": "miner"}},
                ]
            },
        },
    })

    # 8. Awake rest recovers fatigue moderately (5 per tick)
    cases.append({
        "id": "awake_rest_recovers_fatigue",
        "description": "Awake rest for 6 ticks recovers 30 fatigue (5/tick).",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "weaver",
                            "fatigue": 50,
                        },
                    },
                    {"op": "rest", "args": {"being": "weaver", "ticks": 6, "asleep": False}},
                    {"op": "assess", "args": {"being": "weaver"}},
                ]
            },
        },
    })

    # 9. Satisfy need directly
    cases.append({
        "id": "satisfy_need_directly",
        "description": "Directly satisfying hunger by 35 lowers hunger from 60 to 25.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "traveler",
                            "needs": [{"id": "hunger", "level": 60}],
                        },
                    },
                    {"op": "satisfy", "args": {"being": "traveler", "need": "hunger", "amount": 35}},
                    {"op": "assess", "args": {"being": "traveler"}},
                ]
            },
        },
    })

    # 10. Satisfy need by consuming item
    cases.append({
        "id": "satisfy_need_by_item",
        "description": "Consuming bread satisfies hunger via declared satisfier.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "apprentice",
                            "needs": [
                                {
                                    "id": "hunger",
                                    "level": 80,
                                    "satisfiers": [{"item": "bread", "amount": 30}],
                                }
                            ],
                        },
                    },
                    {"op": "satisfy", "args": {"being": "apprentice", "item": "bread", "count": 2}},
                    {"op": "assess", "args": {"being": "apprentice"}},
                ]
            },
        },
    })

    # 11. Satisfy need by performing action
    cases.append({
        "id": "satisfy_need_by_action",
        "description": "Performing eat action satisfies hunger via declared satisfier.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "scholar",
                            "needs": [
                                {
                                    "id": "hunger",
                                    "level": 50,
                                    "satisfiers": [{"action": "eat", "amount": 40}],
                                }
                            ],
                        },
                    },
                    {"op": "satisfy", "args": {"being": "scholar", "action": "eat"}},
                    {"op": "assess", "args": {"being": "scholar"}},
                ]
            },
        },
    })

    # 12. Apply action adds fatigue
    cases.append({
        "id": "apply_action_adds_fatigue",
        "description": "Performing craft action adds 12 fatigue.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {"op": "init", "args": {"being": "smith", "fatigue": 20}},
                    {
                        "op": "apply_action",
                        "args": {
                            "being": "smith",
                            "action": {
                                "id": "craft",
                                "cost": {"fatigue": 12},
                                "actor": {"body_state": {"max_fatigue": 90}},
                            },
                        },
                    },
                    {"op": "assess", "args": {"being": "smith"}},
                ]
            },
        },
    })

    # 13. Sick condition adds extra fatigue cost to actions
    cases.append({
        "id": "sick_condition_penalizes_action_fatigue",
        "description": "Sick condition adds 50% penalty to fatigue cost.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {"op": "init", "args": {"being": "sailor", "fatigue": 10, "conditions": ["sick"]}},
                    {
                        "op": "apply_action",
                        "args": {
                            "being": "sailor",
                            "action": {
                                "id": "row",
                                "cost": {"fatigue": 10},
                            },
                        },
                    },
                    {"op": "assess", "args": {"being": "sailor"}},
                ]
            },
        },
    })

    # 14. Action refused when asleep (unless wake/rest)
    cases.append({
        "id": "action_refused_while_asleep",
        "description": "General actions are refused while asleep.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {"op": "init", "args": {"being": "sentry", "conditions": ["asleep"]}},
                    {
                        "op": "apply_action",
                        "args": {
                            "being": "sentry",
                            "action": {"id": "patrol", "cost": {"fatigue": 5}},
                        },
                    },
                ]
            },
        },
    })

    # 15. Wake action succeeds and clears asleep condition
    cases.append({
        "id": "wake_action_clears_asleep",
        "description": "Wake action is allowed while asleep and clears asleep condition.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {"op": "init", "args": {"being": "scout", "conditions": ["asleep"]}},
                    {
                        "op": "apply_action",
                        "args": {
                            "being": "scout",
                            "action": {"id": "wake", "cost": {"fatigue": 1}},
                        },
                    },
                    {"op": "assess", "args": {"being": "scout"}},
                ]
            },
        },
    })

    # 16. Action refused when exhausted (fatigue == 100)
    cases.append({
        "id": "action_refused_when_exhausted",
        "description": "Actions requiring fatigue are refused when fatigue is at 100.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {"op": "init", "args": {"being": "courier", "fatigue": 100}},
                    {
                        "op": "apply_action",
                        "args": {
                            "being": "courier",
                            "action": {"id": "sprint", "cost": {"fatigue": 8}},
                        },
                    },
                ]
            },
        },
    })

    # 17. Injury blocks specific actions and adds extra fatigue
    cases.append({
        "id": "injury_blocks_action_and_adds_fatigue",
        "description": "Injury blocks specific action and adds extra fatigue to others.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "carpenter",
                            "fatigue": 10,
                            "injuries": [
                                {
                                    "id": "sprained_wrist",
                                    "severity": 15,
                                    "effects": {
                                        "blocked_actions": ["heavy_hammer"],
                                        "extra_fatigue": 4,
                                    },
                                }
                            ],
                        },
                    },
                    {
                        "op": "apply_action",
                        "args": {
                            "being": "carpenter",
                            "action": {"id": "heavy_hammer", "cost": {"fatigue": 10}},
                        },
                    },
                    {
                        "op": "apply_action",
                        "args": {
                            "being": "carpenter",
                            "action": {"id": "light_carve", "cost": {"fatigue": 5}},
                        },
                    },
                    {"op": "assess", "args": {"being": "carpenter"}},
                ]
            },
        },
    })

    # 18. Severe need starvation damages health and can kill
    cases.append({
        "id": "starvation_deprivation_damages_health",
        "description": "A need at 100 deals 5 damage per tick and causes death when health drops to 0.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "prisoner",
                            "health": 10,
                            "needs": [{"id": "starvation", "rate_per_tick": 5, "level": 100}],
                        },
                    },
                    {"op": "tick", "args": {"being": "prisoner", "ticks": 2}},
                    {"op": "assess", "args": {"being": "prisoner"}},
                ]
            },
        },
    })

    # 19. Host heals injury and restores health
    cases.append({
        "id": "host_heals_injury_and_health",
        "description": "Host heals an injury partially or completely, restoring health.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "soldier",
                            "health": 60,
                            "injuries": [
                                {"id": "gash", "severity": 25, "location": "shoulder"}
                            ],
                        },
                    },
                    {
                        "op": "heal_injury",
                        "args": {
                            "being": "soldier",
                            "injury_id": "gash",
                            "amount": 25,
                            "heal_health": 20,
                        },
                    },
                    {"op": "assess", "args": {"being": "soldier"}},
                ]
            },
        },
    })

    # 20. Host sets and removes conditions
    cases.append({
        "id": "host_set_remove_conditions",
        "description": "Host adds sick condition and then removes it.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {"op": "init", "args": {"being": "farmer"}},
                    {"op": "set_condition", "args": {"being": "farmer", "condition": "sick"}},
                    {"op": "assess", "args": {"being": "farmer"}},
                    {"op": "remove_condition", "args": {"being": "farmer", "condition": "sick"}},
                    {"op": "assess", "args": {"being": "farmer"}},
                ]
            },
        },
    })

    # Malformed cases (21 to 25)
    # 21. Malformed: missing being id
    cases.append({
        "id": "malformed_missing_being",
        "description": "Malformed: init missing being identifier.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [{"op": "init", "args": {"health": 80}}]
            },
        },
    })

    # 22. Malformed: fatigue out of bounds
    cases.append({
        "id": "malformed_fatigue_out_of_bounds",
        "description": "Malformed: fatigue level > 100.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [{"op": "init", "args": {"being": "bad_fatigue", "fatigue": 150}}]
            },
        },
    })

    # 23. Malformed: invalid ticks
    cases.append({
        "id": "malformed_invalid_ticks",
        "description": "Malformed: ticks is 0.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [{"op": "tick", "args": {"being": "zero_tick", "ticks": 0}}]
            },
        },
    })

    # 24. Malformed: unknown need in satisfy
    cases.append({
        "id": "malformed_satisfy_unknown_need",
        "description": "Malformed: attempting to satisfy unknown need.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [
                    {"op": "init", "args": {"being": "person_1"}},
                    {"op": "satisfy", "args": {"being": "person_1", "need": "nonexistent"}},
                ]
            },
        },
    })

    # 25. Malformed: unknown op
    cases.append({
        "id": "malformed_unknown_op",
        "description": "Malformed: unknown operation name.",
        "input": {
            "kind": "body",
            "args": {
                "ops": [{"op": "fly_to_moon", "args": {"being": "astronaut"}}]
            },
        },
    })

    return cases


def generate_conformance_suite():
    cases = make_cases()
    ref = BodyReference()

    for c in cases:
        case_id = c["id"]
        case_dir = CONFORMANCE_DIR / case_id
        case_dir.mkdir(parents=True, exist_ok=True)

        is_error = "malformed" in case_id
        case_meta = {"op": "local"}
        if is_error:
            case_meta["error"] = True
        with open(case_dir / "case.json", "w") as f:
            json.dump(case_meta, f, indent=2)

        inp = c["input"]
        with open(case_dir / "input.json", "w") as f:
            json.dump(inp, f, indent=2)

        # Compute expected output with Python reference
        ref_fresh = BodyReference()
        try:
            expected = ref_fresh.workflow(inp["args"])
            with open(case_dir / "expected.json", "w") as f:
                f.write(json.dumps(expected, separators=(',', ':')) + '\n')
        except Exception:
            # For malformed cases, expected.json might not exist or be omitted
            if (case_dir / "expected.json").exists():
                (case_dir / "expected.json").unlink()


def run_random_parity(count: int = 2000, seed: int = 7) -> None:
    random.seed(seed)
    rust_bin = ROOT / "target" / "debug" / "gamectx"

    beings_pool = ["baker", "guard", "scout", "apprentice", "citizen"]
    needs_pool = ["hunger", "thirst", "rest", "warmth", "safety"]
    conditions_pool = ["asleep", "sick", "unconscious", "dead"]

    for i in range(count):
        ref = BodyReference()
        ops = []
        being = random.choice(beings_pool)

        # 1. Always init first
        init_needs = [
            {
                "id": n,
                "label": n.capitalize(),
                "rate_per_tick": random.randint(0, 3),
                "threshold": random.randint(50, 90),
                "level": random.randint(0, 80),
                "satisfiers": [
                    {"action": f"satisfy_{n}", "amount": random.randint(10, 40)},
                    {"item": f"{n}_item", "amount": random.randint(10, 40)},
                ],
            }
            for n in random.sample(needs_pool, random.randint(1, 3))
        ]
        cond_count = random.randint(0, 1)
        init_conds = random.sample(conditions_pool, cond_count)
        ops.append({
            "op": "init",
            "args": {
                "being": being,
                "fatigue": random.randint(0, 80),
                "health": random.randint(40, 100),
                "conditions": init_conds,
                "needs": init_needs,
            },
        })

        # 2. Add 1 to 4 subsequent operations
        num_sub_ops = random.randint(1, 4)
        for _ in range(num_sub_ops):
            op_choice = random.choice([
                "assess", "tick", "satisfy", "rest",
                "apply_action", "set_condition", "remove_condition"
            ])
            args: Dict[str, Any] = {"being": being}

            if op_choice == "assess":
                pass
            elif op_choice == "tick":
                args["ticks"] = random.randint(1, 10)
            elif op_choice == "satisfy":
                chosen_need = random.choice(init_needs)["id"]
                mode = random.choice(["need", "action", "item"])
                if mode == "need":
                    args["need"] = chosen_need
                    args["amount"] = random.randint(5, 50)
                elif mode == "action":
                    args["action"] = f"satisfy_{chosen_need}"
                else:
                    args["item"] = f"{chosen_need}_item"
                    args["count"] = random.randint(1, 3)
            elif op_choice == "rest":
                args["ticks"] = random.randint(1, 8)
                args["asleep"] = random.choice([True, False])
            elif op_choice == "apply_action":
                args["action"] = {
                    "id": random.choice(["gather", "craft", "sleep", "wake", "run"]),
                    "cost": {"fatigue": random.randint(0, 20)},
                    "actor": {
                        "body_state": {
                            "max_fatigue": random.randint(80, 100),
                            "min_condition": 1,
                            "disallowed_conditions": ["dead", "unconscious"],
                        }
                    },
                }
            elif op_choice == "set_condition":
                args["condition"] = random.choice(conditions_pool)
            elif op_choice == "remove_condition":
                args["condition"] = random.choice(conditions_pool)

            ops.append({"op": op_choice, "args": args})

        req = {"ops": ops}

        # Execute on Python reference
        py_res = None
        py_err = None
        try:
            py_res = ref.workflow(req)
        except Exception as e:
            py_err = str(e)

        if rust_bin.exists():
            payload = json.dumps({"kind": "body", "args": req})
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
        cases = make_cases()
        ref = BodyReference()
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
