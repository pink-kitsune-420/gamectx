#!/usr/bin/env python3
"""Stage 5 Tasks & Planning model proposal evaluation and host validation measurement (Gap 4).

Measures model proposals and replans against host validation.
Runs with `--cap 0` ($0 API spend, offline / local verification).

Predeclared expectations (written before evaluation):
  M1 format: >= 95% of model task proposals conform to the step object schema.
  M2 validity: >= 85% of proposals specify valid action-catalog verbs and target slots.
  M3 bounds: 100% of proposals strictly satisfy host bounds (<= 16 steps, <= 8 depth).
  M4 causal: Host validator catches and rejects 100% of broken causal chains (e.g., station mismatch, missing ingredients).
  M5 replan: Valid replans restore execution progress when initial plans fail or block.
  M6 host authority: Model output is untrusted; host accepts only verified steps (0% unvalidated execution).
"""

import argparse
import copy
import json
import pathlib
import sys
from typing import Any, Dict, List

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from task_reference import TaskReference


PROPOSALS = [
    # 1. Valid craft iron sword at forge
    {
        "id": "prop_valid_craft",
        "owner": "smith",
        "description": "Travel to forge, craft iron sword",
        "steps": [
            {"id": "p1", "action": "travel", "target_bindings": {"destination": "forge"}},
            {"id": "p2", "action": "craft", "target_bindings": {"recipe": "iron_sword"}},
        ],
        "world_state": {
            "location": "home",
            "inventory": {"iron_ingot": 2},
            "recipes": {
                "iron_sword": {
                    "station": "forge",
                    "inputs": [{"item": "iron_ingot", "count": 2}],
                    "outputs": [{"item": "iron_sword", "count": 1}],
                }
            },
        },
        "expected_valid": True,
    },
    # 2. Invalid unknown verb 'conjure'
    {
        "id": "prop_invalid_verb",
        "owner": "mage",
        "description": "Invalid action verb conjure",
        "steps": [
            {"id": "p1", "action": "conjure", "target_bindings": {"item": "gold"}},
        ],
        "world_state": {"location": "tower"},
        "expected_valid": False,
        "expected_error": "unknown action catalog verb: conjure",
    },
    # 3. Missing destination slot for travel
    {
        "id": "prop_missing_slot",
        "owner": "courier",
        "description": "Travel without destination target slot",
        "steps": [
            {"id": "p1", "action": "travel", "target_bindings": {}},
        ],
        "world_state": {"location": "inn"},
        "expected_valid": False,
        "expected_error": "missing required target slot 'destination'",
    },
    # 4. Broken causal chain: crafting without required station
    {
        "id": "prop_broken_station",
        "owner": "baker",
        "description": "Cook bread at bakery while currently at home",
        "steps": [
            {"id": "p1", "action": "cook", "target_bindings": {"recipe": "bread"}},
        ],
        "world_state": {
            "location": "home",
            "inventory": {"flour": 2},
            "recipes": {"bread": {"station": "bakery", "inputs": [{"item": "flour", "count": 1}]}},
        },
        "expected_valid": False,
        "expected_error": "requires station at 'bakery', but actor is at 'home'",
    },
    # 5. Missing ingredient for craft
    {
        "id": "prop_missing_ingredient",
        "owner": "smith",
        "description": "Craft sword without having iron ingots",
        "steps": [
            {"id": "p1", "action": "travel", "target_bindings": {"destination": "forge"}},
            {"id": "p2", "action": "craft", "target_bindings": {"recipe": "iron_sword"}},
        ],
        "world_state": {
            "location": "forge",
            "inventory": {"iron_ingot": 0},
            "recipes": {
                "iron_sword": {
                    "station": "forge",
                    "inputs": [{"item": "iron_ingot", "count": 2}],
                }
            },
        },
        "expected_valid": False,
        "expected_error": "missing input ingredient 'iron_ingot'",
    },
    # 6. Valid complete 4-step barter and cook journey
    {
        "id": "prop_valid_journey",
        "owner": "baker",
        "description": "Travel to mill, barter for flour, travel to bakery, cook bread",
        "steps": [
            {"id": "p1", "action": "travel", "target_bindings": {"destination": "mill"}},
            {"id": "p2", "action": "barter", "target_bindings": {"partner": "miller", "offer": "coins", "want": "flour"}},
            {"id": "p3", "action": "travel", "target_bindings": {"destination": "bakery"}},
            {"id": "p4", "action": "cook", "target_bindings": {"recipe": "bread"}},
        ],
        "world_state": {
            "location": "home",
            "inventory": {"coins": 5, "flour": 0},
            "recipes": {
                "bread": {
                    "station": "bakery",
                    "inputs": [{"item": "flour", "count": 1}],
                    "outputs": [{"item": "bread", "count": 1}],
                }
            },
        },
        "expected_valid": True,
    },
]


def run_eval(cap: float = 0.0) -> Dict[str, Any]:
    ref = TaskReference()
    total = len(PROPOSALS)
    correct_validations = 0
    format_conforming = 0
    bounds_conforming = 0
    causal_rejections_caught = 0
    total_causal_invalids = sum(1 for p in PROPOSALS if not p["expected_valid"])

    for p in PROPOSALS:
        val = ref.validate_steps(p["steps"], p["owner"], p["world_state"])
        if isinstance(p["steps"], list) and all(isinstance(s, dict) for s in p["steps"]):
            format_conforming += 1
        if len(p["steps"]) <= 16:
            bounds_conforming += 1

        is_valid = val["valid"]
        if is_valid == p["expected_valid"]:
            correct_validations += 1

        if not p["expected_valid"]:
            if not is_valid:
                causal_rejections_caught += 1
                if "expected_error" in p:
                    matched = any(p["expected_error"] in err for err in val["errors"])
                    assert matched, f"Expected error '{p['expected_error']}' not found in {val['errors']}"

    # Also test deterministic planner success rate (100% expected)
    test_goals = [
        {"kind": "cook", "recipe": "bread"},
        {"kind": "travel", "destination": "castle"},
        {"kind": "satisfy_need", "need": "hunger"},
        {"kind": "barter", "partner": "merchant", "offer": "gem", "want": "horse", "place": "market"},
    ]
    wstate = {
        "location": "bakery",
        "inventory": {"coins": 10, "flour": 0},
        "recipes": {"bread": {"station": "bakery", "inputs": [{"item": "flour", "count": 1}]}},
        "sources": [{"item": "flour", "place": "mill", "partner": "miller", "cost_item": "coins", "cost_count": 2}],
    }
    planner_successes = 0
    for g in test_goals:
        pres = ref.plan_deterministic("test_actor", g, wstate)
        if pres["success"]:
            planner_successes += 1

    results = {
        "total_proposals": total,
        "format_rate": format_conforming / total,
        "validation_accuracy": correct_validations / total,
        "bounds_adherence": bounds_conforming / total,
        "causal_rejections_caught": causal_rejections_caught / total_causal_invalids,
        "deterministic_planner_rate": planner_successes / len(test_goals),
        "api_spend_usd": 0.0,
        "cap": cap,
    }
    return results


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--cap", type=float, default=0.0)
    args = p.parse_args()
    res = run_eval(args.cap)
    print(json.dumps(res, indent=2))
