#!/usr/bin/env python3
"""Conformance test cases and differential parity runner for Off-screen Life Resolution (Gap 10)
and Curiosity & Exploration Model (Gap 11).
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
from offscreen_curiosity_reference import process_local

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "offscreen_curiosity"


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    kind = request.get("kind")
    args = request.get("args", {})
    return process_local(kind, args)


def make_cases() -> List[Dict[str, Any]]:
    cases = []

    # 1. offscreen_init_clear_records
    cases.append({
        "id": "offscreen_init_clear_records",
        "description": "Initialize offscreen resolver and clear records.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {"op": "init", "args": {"clear_records": True}}
                ]
            }
        }
    })

    # 2. offscreen_resolve_window_private
    cases.append({
        "id": "offscreen_resolve_window_private",
        "description": "Resolve offscreen window with private visibility, inputs and outputs.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "baker",
                            "activity": "baking_shift",
                            "action": "cook",
                            "observed": False,
                            "at": 100,
                            "place": "bakery",
                            "visibility": "private",
                            "fatigue": 8,
                            "practice": 12,
                            "inputs": [{"count": 2, "item": "flour"}],
                            "outputs": [{"count": 2, "item": "bread"}]
                        }
                    }
                ]
            }
        }
    })

    # 3. offscreen_resolve_window_witnessed
    cases.append({
        "id": "offscreen_resolve_window_witnessed",
        "description": "Resolve offscreen window with witnessed visibility.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "apprentice",
                            "activity": "woodchopping",
                            "action": "chop",
                            "observed": False,
                            "at": 105,
                            "place": "forest_clearing",
                            "visibility": "witnessed",
                            "fatigue": 15,
                            "practice": 20,
                            "inputs": [{"count": 1, "item": "axe_durability"}],
                            "outputs": [{"count": 5, "item": "firewood"}]
                        }
                    }
                ]
            }
        }
    })

    # 4. offscreen_resolve_window_observed_skipped
    cases.append({
        "id": "offscreen_resolve_window_observed_skipped",
        "description": "Resolve offscreen window when observed is true skips resolution.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "baker",
                            "activity": "baking_shift",
                            "action": "cook",
                            "observed": True
                        }
                    }
                ]
            }
        }
    })

    # 5. offscreen_batch_resolve
    cases.append({
        "id": "offscreen_batch_resolve",
        "description": "Batch resolve multiple offscreen activities in one tick.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "batch_resolve",
                        "args": {
                            "activities": [
                                {
                                    "being": "baker",
                                    "activity": "flour_milling",
                                    "action": "mill",
                                    "observed": False,
                                    "at": 50,
                                    "place": "mill",
                                    "outputs": [{"count": 10, "item": "flour"}]
                                },
                                {
                                    "being": "guard",
                                    "activity": "perimeter_patrol",
                                    "action": "patrol",
                                    "observed": False,
                                    "at": 55,
                                    "place": "gatehouse",
                                    "fatigue": 10
                                }
                            ]
                        }
                    }
                ]
            }
        }
    })

    # 6. offscreen_assess_all
    cases.append({
        "id": "offscreen_assess_all",
        "description": "Assess all offscreen records recorded so far.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "baker",
                            "activity": "knead_dough",
                            "action": "knead",
                            "observed": False,
                            "at": 120
                        }
                    },
                    {
                        "op": "assess",
                        "args": {}
                    }
                ]
            }
        }
    })

    # 7. offscreen_assess_filter_being
    cases.append({
        "id": "offscreen_assess_filter_being",
        "description": "Assess offscreen records filtered by being identifier.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "baker",
                            "activity": "knead_dough",
                            "action": "knead",
                            "observed": False,
                            "at": 130
                        }
                    },
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "guard",
                            "activity": "watch",
                            "action": "scout",
                            "observed": False,
                            "at": 135
                        }
                    },
                    {
                        "op": "assess",
                        "args": {"being": "baker"}
                    }
                ]
            }
        }
    })

    # 8. offscreen_fatigue_and_practice_defaults
    cases.append({
        "id": "offscreen_fatigue_and_practice_defaults",
        "description": "Resolve offscreen window with default fatigue and practice values.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "apprentice",
                            "activity": "sweeping",
                            "action": "sweep",
                            "observed": False
                        }
                    }
                ]
            }
        }
    })

    # 9. offscreen_multiple_windows_token_increment
    cases.append({
        "id": "offscreen_multiple_windows_token_increment",
        "description": "Verify token increment across sequential resolved windows.",
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "baker",
                            "activity": "oven_stoking",
                            "action": "stoke",
                            "observed": False,
                            "at": 200
                        }
                    },
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "baker",
                            "activity": "bread_pulling",
                            "action": "pull",
                            "observed": False,
                            "at": 210
                        }
                    }
                ]
            }
        }
    })

    # 10. malformed_offscreen_missing_being
    cases.append({
        "id": "malformed_offscreen_missing_being",
        "description": "Reject offscreen resolution when being is missing.",
        "error": True,
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "activity": "baking_shift",
                            "action": "cook",
                            "observed": False
                        }
                    }
                ]
            }
        }
    })

    # 11. malformed_offscreen_invalid_visibility
    cases.append({
        "id": "malformed_offscreen_invalid_visibility",
        "description": "Reject offscreen resolution with invalid visibility kind.",
        "error": True,
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "resolve_window",
                        "args": {
                            "being": "baker",
                            "activity": "baking_shift",
                            "action": "cook",
                            "observed": False,
                            "visibility": "secret_chamber"
                        }
                    }
                ]
            }
        }
    })

    # 12. malformed_offscreen_batch_overflow
    cases.append({
        "id": "malformed_offscreen_batch_overflow",
        "description": "Reject offscreen batch resolve when activities count exceeds max work per tick.",
        "error": True,
        "input": {
            "kind": "offscreen",
            "args": {
                "ops": [
                    {
                        "op": "batch_resolve",
                        "args": {
                            "activities": [
                                {"being": "b", "activity": "a", "action": "c", "observed": False}
                                for _ in range(17)
                            ]
                        }
                    }
                ]
            }
        }
    })

    # 13. curiosity_init_with_knowns
    cases.append({
        "id": "curiosity_init_with_knowns",
        "description": "Initialize curiosity model with predefined known people, things, places, and facts.",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "curious",
                            "known_people": ["alice", "bob"],
                            "known_things": ["oven", "peel"],
                            "known_places": ["bakery", "market"],
                            "known_facts": ["recipe_bread", "recipe_pie"]
                        }
                    },
                    {
                        "op": "assess",
                        "args": {"being": "baker"}
                    }
                ]
            }
        }
    })

    # 14. curiosity_evaluate_novelty_curious
    cases.append({
        "id": "curiosity_evaluate_novelty_curious",
        "description": "Evaluate novelty signal under curious temperament (1.5x scaling).",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "scout",
                            "temperament": "curious",
                            "known_people": ["alice"],
                            "known_things": ["compass"],
                            "known_places": ["camp"],
                            "known_facts": ["trail_map"]
                        }
                    },
                    {
                        "op": "evaluate_novelty",
                        "args": {
                            "being": "scout",
                            "people": ["alice", "stranger_charlie"],
                            "things": ["compass", "relic_stone"],
                            "places": ["ancient_ruins"],
                            "facts": ["hidden_tomb_rumor"]
                        }
                    }
                ]
            }
        }
    })

    # 15. curiosity_evaluate_novelty_cautious
    cases.append({
        "id": "curiosity_evaluate_novelty_cautious",
        "description": "Evaluate novelty signal under cautious temperament (0.7x scaling).",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "guard",
                            "temperament": "cautious",
                            "known_people": ["alice"],
                            "known_things": ["sword"],
                            "known_places": ["barracks"],
                            "known_facts": ["gate_code"]
                        }
                    },
                    {
                        "op": "evaluate_novelty",
                        "args": {
                            "being": "guard",
                            "people": ["stranger_charlie"],
                            "things": ["suspicious_box"],
                            "places": ["dark_alley"],
                            "facts": ["intruder_alert"]
                        }
                    }
                ]
            }
        }
    })

    # 16. curiosity_evaluate_novelty_apathetic
    cases.append({
        "id": "curiosity_evaluate_novelty_apathetic",
        "description": "Evaluate novelty signal under apathetic temperament (0.4x scaling).",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "hermit",
                            "temperament": "apathetic"
                        }
                    },
                    {
                        "op": "evaluate_novelty",
                        "args": {
                            "being": "hermit",
                            "people": ["traveler"],
                            "things": ["book"],
                            "places": ["crossroads"],
                            "facts": ["royal_decree"]
                        }
                    }
                ]
            }
        }
    })

    # 17. curiosity_evaluate_novelty_calm_default
    cases.append({
        "id": "curiosity_evaluate_novelty_calm_default",
        "description": "Evaluate novelty signal under calm default temperament (1.0x scaling).",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "scholar",
                            "temperament": "calm"
                        }
                    },
                    {
                        "op": "evaluate_novelty",
                        "args": {
                            "being": "scholar",
                            "people": ["visitor"],
                            "things": ["scroll"],
                            "places": ["library_archive"],
                            "facts": ["lost_language"]
                        }
                    }
                ]
            }
        }
    })

    # 18. curiosity_generate_candidates_with_people_and_facts
    cases.append({
        "id": "curiosity_generate_candidates_with_people_and_facts",
        "description": "Generate ask-kind initiative candidates for stranger and unknown fact.",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "curious"
                        }
                    },
                    {
                        "op": "generate_candidates",
                        "args": {
                            "being": "baker",
                            "people": ["wanderer_elena"],
                            "facts": ["famine_in_south"]
                        }
                    }
                ]
            }
        }
    })

    # 19. curiosity_generate_candidates_with_places_and_things
    cases.append({
        "id": "curiosity_generate_candidates_with_places_and_things",
        "description": "Generate activity-kind initiative candidates for new location and unfamiliar thing.",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "apprentice",
                            "temperament": "curious"
                        }
                    },
                    {
                        "op": "generate_candidates",
                        "args": {
                            "being": "apprentice",
                            "places": ["hidden_cellar"],
                            "things": ["locked_chest"]
                        }
                    }
                ]
            }
        }
    })

    # 20. curiosity_generate_candidates_auto_mark
    cases.append({
        "id": "curiosity_generate_candidates_auto_mark",
        "description": "Verify auto_mark adds novel entities into familiarity memory.",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "scout",
                            "temperament": "curious"
                        }
                    },
                    {
                        "op": "generate_candidates",
                        "args": {
                            "being": "scout",
                            "people": ["mysterious_figure"],
                            "things": ["strange_key"],
                            "auto_mark": True
                        }
                    },
                    {
                        "op": "evaluate_novelty",
                        "args": {
                            "being": "scout",
                            "people": ["mysterious_figure"],
                            "things": ["strange_key"]
                        }
                    }
                ]
            }
        }
    })

    # 21. curiosity_mark_familiar_manual
    cases.append({
        "id": "curiosity_mark_familiar_manual",
        "description": "Manually mark entities familiar and assess updated memory.",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "merchant"
                        }
                    },
                    {
                        "op": "mark_familiar",
                        "args": {
                            "being": "merchant",
                            "people": ["buyer_dan"],
                            "places": ["harbor_wharf"]
                        }
                    },
                    {
                        "op": "assess",
                        "args": {"being": "merchant"}
                    }
                ]
            }
        }
    })

    # 22. curiosity_generate_candidates_limit
    cases.append({
        "id": "curiosity_generate_candidates_limit",
        "description": "Cap generated initiative candidates to limit parameter.",
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "curious"
                        }
                    },
                    {
                        "op": "generate_candidates",
                        "args": {
                            "being": "baker",
                            "people": ["p1", "p2", "p3", "p4"],
                            "things": ["t1", "t2", "t3"],
                            "limit": 2
                        }
                    }
                ]
            }
        }
    })

    # 23. malformed_curiosity_unknown_temperament
    cases.append({
        "id": "malformed_curiosity_unknown_temperament",
        "description": "Reject curiosity init with unrecognized temperament.",
        "error": True,
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "baker",
                            "temperament": "hyperactive"
                        }
                    }
                ]
            }
        }
    })

    # 24. malformed_curiosity_invalid_being
    cases.append({
        "id": "malformed_curiosity_invalid_being",
        "description": "Reject curiosity init with invalid being identifier.",
        "error": True,
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "init",
                        "args": {
                            "being": "bad being with spaces!"
                        }
                    }
                ]
            }
        }
    })

    # 25. malformed_curiosity_candidate_limit_overflow
    cases.append({
        "id": "malformed_curiosity_candidate_limit_overflow",
        "description": "Reject generate_candidates when limit exceeds bounds.",
        "error": True,
        "input": {
            "kind": "curiosity",
            "args": {
                "ops": [
                    {
                        "op": "generate_candidates",
                        "args": {
                            "being": "baker",
                            "limit": 999
                        }
                    }
                ]
            }
        }
    })

    return cases


def generate_conformance_cases():
    CONFORMANCE_DIR.mkdir(parents=True, exist_ok=True)
    cases = make_cases()
    for c in cases:
        case_dir = CONFORMANCE_DIR / c["id"]
        case_dir.mkdir(parents=True, exist_ok=True)
        case_meta = {
            "op": "local",
            "description": c["description"],
            "error": c.get("error", False)
        }
        with open(case_dir / "case.json", "w") as f:
            json.dump(case_meta, f, indent=2)
            f.write("\n")
        with open(case_dir / "input.json", "w") as f:
            json.dump(c["input"], f, indent=2)
            f.write("\n")
    print(f"Generated {len(cases)} conformance cases in {CONFORMANCE_DIR}")


def run_random_parity(count: int, seed: int = 7):
    random.seed(seed)
    rust_bin = ROOT / "target" / "debug" / "gamectx"
    if not rust_bin.exists():
        subprocess.run(["cargo", "build"], cwd=ROOT, check=True)

    beings_pool = ["alice", "bob", "charlie", "dan", "elena"]
    activities_pool = ["baking", "patrolling", "foraging", "smithing", "cleaning"]
    actions_pool = ["cook", "patrol", "gather", "craft", "sweep"]
    places_pool = ["bakery", "gatehouse", "woods", "forge", "market"]
    things_pool = ["flour", "iron_ingot", "herb_bundle", "key", "gem"]
    facts_pool = ["dragon_seen", "secret_pass", "broken_mill", "gold_cache"]
    temperaments = ["curious", "cautious", "apathetic", "calm"]

    for i in range(1, count + 1):
        mode = random.choice(["offscreen", "curiosity"])
        num_ops = random.randint(1, 5)

        if mode == "offscreen":
            ops = []
            for _ in range(num_ops):
                roll = random.random()
                b = random.choice(beings_pool)
                act = random.choice(activities_pool)
                action = random.choice(actions_pool)
                place = random.choice(places_pool)

                if roll < 0.15:
                    ops.append({"op": "init", "args": {"clear_records": random.choice([True, False])}})
                elif roll < 0.65:
                    ops.append({
                        "op": "resolve_window",
                        "args": {
                            "action": action,
                            "activity": act,
                            "at": random.randint(1, 1000),
                            "being": b,
                            "fatigue": random.randint(-10, 20),
                            "inputs": [{"count": random.randint(1, 3), "item": random.choice(things_pool)}],
                            "observed": random.random() < 0.2,
                            "outputs": [{"count": random.randint(1, 3), "item": random.choice(things_pool)}],
                            "place": place,
                            "practice": random.randint(0, 30),
                            "visibility": random.choice(["private", "witnessed"])
                        }
                    })
                elif roll < 0.85:
                    batch = []
                    for _ in range(random.randint(1, 3)):
                        batch.append({
                            "action": random.choice(actions_pool),
                            "activity": random.choice(activities_pool),
                            "at": random.randint(1, 500),
                            "being": random.choice(beings_pool),
                            "observed": False,
                            "place": random.choice(places_pool)
                        })
                    ops.append({"op": "batch_resolve", "args": {"activities": batch}})
                else:
                    ops.append({"op": "assess", "args": {"being": random.choice(beings_pool + [None])}})
            req = {"kind": "offscreen", "args": {"ops": ops}}

        else:
            ops = []
            for _ in range(num_ops):
                roll = random.random()
                b = random.choice(beings_pool)

                if roll < 0.2:
                    ops.append({
                        "op": "init",
                        "args": {
                            "being": b,
                            "known_facts": random.sample(facts_pool, random.randint(0, 2)),
                            "known_people": random.sample(beings_pool, random.randint(0, 2)),
                            "known_places": random.sample(places_pool, random.randint(0, 2)),
                            "known_things": random.sample(things_pool, random.randint(0, 2)),
                            "temperament": random.choice(temperaments)
                        }
                    })
                elif roll < 0.5:
                    ops.append({
                        "op": "evaluate_novelty",
                        "args": {
                            "being": b,
                            "facts": random.sample(facts_pool, random.randint(0, 3)),
                            "people": random.sample(beings_pool, random.randint(0, 3)),
                            "places": random.sample(places_pool, random.randint(0, 3)),
                            "things": random.sample(things_pool, random.randint(0, 3))
                        }
                    })
                elif roll < 0.75:
                    ops.append({
                        "op": "generate_candidates",
                        "args": {
                            "auto_mark": random.choice([True, False]),
                            "being": b,
                            "facts": random.sample(facts_pool, random.randint(0, 2)),
                            "limit": random.randint(1, 8),
                            "people": random.sample(beings_pool, random.randint(0, 2)),
                            "places": random.sample(places_pool, random.randint(0, 2)),
                            "things": random.sample(things_pool, random.randint(0, 2))
                        }
                    })
                elif roll < 0.9:
                    ops.append({
                        "op": "mark_familiar",
                        "args": {
                            "being": b,
                            "facts": random.sample(facts_pool, random.randint(0, 2)),
                            "people": random.sample(beings_pool, random.randint(0, 2)),
                            "places": random.sample(places_pool, random.randint(0, 2)),
                            "things": random.sample(things_pool, random.randint(0, 2))
                        }
                    })
                else:
                    ops.append({"op": "assess", "args": {"being": b}})
            req = {"kind": "curiosity", "args": {"ops": ops}}

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
    p.add_argument("--generate", action="store_true")
    args = p.parse_args()

    if args.generate:
        generate_conformance_cases()
    elif args.random > 0:
        run_random_parity(args.random, seed=args.seed)
    else:
        cases = make_cases()
        passed = 0
        for c in cases:
            try:
                out = py_workflow(c["input"])
                passed += 1
            except Exception:
                if c.get("error"):
                    passed += 1
        print(json.dumps({
            "cases": len(cases),
            "passed": passed,
            "actual_model_calls": 0,
            "api_spend_usd": 0,
        }))
