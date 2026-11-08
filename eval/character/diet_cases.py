#!/usr/bin/env python3
"""Conformance cases and differential parity runner for diets and foods (pass 2,
stage 3, local kind `diet`).

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/diet/*/case.json + input.json
  --random N --seed S     N randomized workflows, Python reference vs `gamectx local -`

All checks are offline: zero model calls, $0 API spend.
"""

import argparse
import json
import pathlib
import random
import subprocess
import sys
from typing import Any, Dict, List

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from diet_reference import process_local

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "diet"


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    return process_local(request.get("kind"), request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


PACK = op("register_food", pack="village")


def dt(*ops: Dict[str, Any], pack: bool = True) -> Dict[str, Any]:
    return {"kind": "diet", "args": {"ops": ([PACK] if pack else []) + list(ops)}}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    add("diet_register_village_pack",
        "The original village pack registers twelve foods.",
        dt())

    add("diet_register_custom_food_defaults",
        "A custom food defaults its label to its id and its preserved shelf life to four times fresh.",
        dt(op("register_food", foods=[{"id": "smoked_eel", "tags": ["fish"], "hunger": 28, "spoils_after": 30,
                                        "raw_risk": 20}]),
           op("evaluate", being="miller", food="smoked_eel", cooked=True, preserved=True, made_at=0, at=100),
           op("evaluate", being="miller", food="smoked_eel", cooked=True, made_at=0, at=100), pack=False))

    add("diet_set_diet_profile",
        "set_diet stores a sorted profile; diet defaults to omnivore.",
        dt(op("set_diet", being="ferrin", forbids=["alcohol"], allergies=["nut", "honey_cake"],
              likes=["fruit", "dairy"], dislikes=["fish"])))

    add("diet_evaluate_fresh_stale_spoiled",
        "Trout is fresh until half its 24-tick shelf life, stale until 24, then spoiled.",
        dt(op("evaluate", being="baker", food="river_trout", cooked=True, made_at=90, at=100),
           op("evaluate", being="baker", food="river_trout", cooked=True, made_at=80, at=100),
           op("evaluate", being="baker", food="river_trout", cooked=True, made_at=70, at=100)))

    add("diet_preserved_lasts_longer",
        "Preserved trout keeps for 720 ticks instead of 24.",
        dt(op("evaluate", being="baker", food="river_trout", cooked=True, preserved=True, made_at=0, at=300),
           op("evaluate", being="baker", food="river_trout", cooked=True, preserved=True, made_at=0, at=500)))

    add("diet_cooking_bonus_and_raw_risk",
        "Raw trout is unsafe (sickness 35); cooking adds 20% fill and removes the risk.",
        dt(op("evaluate", being="baker", food="river_trout"),
           op("evaluate", being="baker", food="river_trout", cooked=True)))

    add("diet_herbivore_refuses_meat",
        "A herbivore refuses stew and trout but eats berries.",
        dt(op("set_diet", being="goat", diet="herbivore"),
           op("evaluate", being="goat", food="hearty_stew", cooked=True),
           op("evaluate", being="goat", food="river_trout", cooked=True),
           op("evaluate", being="goat", food="bramble_berries")))

    add("diet_carnivore_needs_meat",
        "A carnivore refuses bread and accepts cooked stew.",
        dt(op("set_diet", being="hound", diet="carnivore"),
           op("evaluate", being="hound", food="rye_bread"),
           op("evaluate", being="hound", food="hearty_stew", cooked=True)))

    add("diet_photosynthetic_and_mineral",
        "Photosynthetic beings eat nothing; mineral eaters only take mineral food, which others refuse.",
        dt(op("set_diet", being="mossling", diet="photosynthetic"),
           op("set_diet", being="stonekin", diet="mineral"),
           op("evaluate", being="mossling", food="orchard_apple"),
           op("evaluate", being="stonekin", food="salt_lick"),
           op("evaluate", being="stonekin", food="rye_bread"),
           op("evaluate", being="baker", food="salt_lick")))

    add("diet_forbidden_taboo",
        "A taboo on alcohol refuses ale but not water.",
        dt(op("set_diet", being="ferrin", forbids=["alcohol"]),
           op("evaluate", being="ferrin", food="barley_ale"),
           op("evaluate", being="ferrin", food="spring_water")))

    add("diet_allergy_by_tag_and_id",
        "Allergies match a food id or any of its tags; sickness is 40.",
        dt(op("set_diet", being="ferrin", allergies=["nut", "honey_cake"]),
           op("evaluate", being="ferrin", food="roasted_nuts"),
           op("evaluate", being="ferrin", food="honey_cake"),
           op("evaluate", being="ferrin", food="rye_bread")))

    add("diet_likes_dislikes_joy",
        "Each liked tag adds 5 joy and each disliked tag removes 5.",
        dt(op("set_diet", being="ferrin", likes=["sweet", "grain"], dislikes=["egg"]),
           op("evaluate", being="ferrin", food="honey_cake"),
           op("evaluate", being="ferrin", food="rye_bread")))

    add("diet_monotony_penalty",
        "Repeating a meal costs 3 joy per recent repeat.",
        dt(op("eat", being="baker", food="rye_bread", at=1),
           op("eat", being="baker", food="rye_bread", at=2),
           op("eat", being="baker", food="rye_bread", at=3),
           op("evaluate", being="baker", food="rye_bread", at=4),
           op("evaluate", being="baker", food="orchard_apple", at=4)))

    add("diet_choose_ranks_by_hunger",
        "A starving being picks the filling stew; a sated one picks the enjoyable cake.",
        dt(op("choose", being="baker", hunger_level=100,
              options=[{"food": "honey_cake"}, {"food": "hearty_stew", "cooked": True}]),
           op("choose", being="baker", hunger_level=0,
              options=[{"food": "honey_cake"}, {"food": "hearty_stew", "cooked": True}])))

    add("diet_choose_refused_listed",
        "Refused options are listed with reasons; no edible option gives best null.",
        dt(op("set_diet", being="goat", diet="herbivore"),
           op("choose", being="goat", options=[{"food": "hearty_stew", "cooked": True},
                                               {"food": "river_trout", "cooked": True}]),
           op("choose", being="goat", options=[{"food": "hearty_stew", "cooked": True},
                                               {"food": "orchard_apple"}, {"food": "orchard_apple"}])))

    add("diet_eat_effects_body_affect",
        "Eating proposes body satisfy for hunger and thirst and an affect mood change.",
        dt(op("eat", being="baker", food="hearty_stew", cooked=True, made_at=10, at=12),
           op("assess", being="baker")))

    add("diet_eat_refused_without_force",
        "A refused serving is not eaten and not recorded.",
        dt(op("set_diet", being="ferrin", forbids=["meat"]),
           op("eat", being="ferrin", food="hearty_stew", cooked=True),
           op("assess", being="ferrin")))

    add("diet_eat_forced_spoiled_sick",
        "A starving being forced to eat spoiled porridge gets half fill and falls sick.",
        dt(op("eat", being="porter", food="oat_porridge", made_at=0, at=50, force=True)))

    add("diet_assess_recent_meals_bounded",
        "Recent meals keep the last 16; the meal count keeps going.",
        dt(*[op("eat", being="baker", food=["rye_bread", "orchard_apple", "spring_water"][i % 3], at=i)
             for i in range(18)], op("assess", being="baker")))

    add("diet_never_spoils",
        "Food with no shelf life stays fresh.",
        dt(op("evaluate", being="baker", food="barley_ale", made_at=0, at=1000000)))

    add("diet_unknown_profile_defaults_omnivore",
        "A being with no profile is evaluated as an omnivore with no preferences.",
        dt(op("evaluate", being="stranger", food="hearty_stew", cooked=True),
           op("evaluate", being="stranger", food="salt_lick")))

    add("malformed_diet_unknown_food",
        "Reject a serving of an unregistered food.",
        dt(op("evaluate", being="baker", food="dragon_fruit")), error=True)

    add("malformed_diet_made_after_at",
        "Reject a serving made after the time it is eaten.",
        dt(op("eat", being="baker", food="rye_bread", made_at=9, at=5)), error=True)

    add("malformed_diet_like_and_dislike",
        "Reject a profile that both likes and dislikes a tag.",
        dt(op("set_diet", being="baker", likes=["fruit"], dislikes=["fruit"])), error=True)

    add("malformed_diet_bad_tag",
        "Reject a food tag outside the fixed list.",
        dt(op("register_food", foods=[{"id": "moonbread", "tags": ["magic"], "hunger": 10}]), pack=False),
        error=True)

    return cases


FOODS = ["rye_bread", "oat_porridge", "orchard_apple", "hard_cheese", "river_trout", "hearty_stew",
         "bramble_berries", "honey_cake", "barley_ale", "spring_water", "roasted_nuts", "salt_lick"]
TAGS = ["alcohol", "dairy", "egg", "fish", "fruit", "grain", "herb", "meat", "mineral", "nut", "sweet",
        "vegetable", "water"]
BEINGS = ["baker", "goat", "hound", "ferrin", "stonekin"]


def rserving(rng: random.Random, menu: List[str]) -> Dict[str, Any]:
    s: Dict[str, Any] = {"food": rng.choice(menu) if rng.random() > 0.02 else "unlisted_pie"}
    if rng.random() < 0.5:
        s["cooked"] = rng.random() < 0.6
    if rng.random() < 0.3:
        s["preserved"] = rng.random() < 0.5
    if rng.random() < 0.7:
        at = rng.randint(0, 1000)
        s["at"] = at
        if rng.random() < 0.7:
            s["made_at"] = max(0, at - rng.randint(0, 120)) if rng.random() > 0.02 else at + 1
    return s


def rand_diet(rng: random.Random) -> Dict[str, Any]:
    ops: List[Dict[str, Any]] = [PACK] if rng.random() < 0.97 else []
    menu = list(FOODS) if ops else []
    if rng.random() < 0.4 or not menu:
        menu.append("custom_pie")
        f: Dict[str, Any] = {"id": "custom_pie", "tags": rng.sample(TAGS, rng.randint(1, 3)),
                             "hunger": rng.randint(0, 100)}
        for k, lo, hi in [("thirst", 0, 100), ("joy", -20, 20), ("spoils_after", 0, 200),
                          ("preserved_spoils_after", 0, 900), ("raw_risk", 0, 100), ("cooked_bonus", 0, 100)]:
            if rng.random() < 0.5:
                f[k] = rng.randint(lo, hi)
        ops.append(op("register_food", foods=[f]))
    for _ in range(rng.randint(1, 10)):
        roll = rng.random()
        b = rng.choice(BEINGS)
        if roll < 0.2:
            d: Dict[str, Any] = {"being": b}
            if rng.random() < 0.7:
                d["diet"] = rng.choice(["herbivore", "carnivore", "omnivore", "photosynthetic", "scavenger",
                                        "mineral"])
            pool = rng.sample(TAGS, 6)
            if rng.random() < 0.5:
                d["forbids"] = rng.sample(TAGS, rng.randint(0, 2))
            if rng.random() < 0.5:
                d["allergies"] = rng.sample(TAGS + FOODS, rng.randint(0, 2))
            if rng.random() < 0.6:
                d["likes"] = pool[:rng.randint(0, 3)]
            if rng.random() < 0.6:
                d["dislikes"] = pool[3:3 + rng.randint(0, 3)]
            if rng.random() < 0.02:
                d["dislikes"] = d.get("likes", ["fruit"])[:1] or ["fruit"]
                d["likes"] = d["dislikes"]
            ops.append(op("set_diet", **d))
        elif roll < 0.45:
            ops.append(op("evaluate", being=b, **rserving(rng, menu)))
        elif roll < 0.6:
            c: Dict[str, Any] = {"being": b, "options": [rserving(rng, menu) for _ in range(rng.randint(1, 4))]}
            for o in c["options"]:
                o.pop("at", None) if rng.random() < 0.2 else None
                if "made_at" in o and "at" not in o:
                    o.pop("made_at")
            if rng.random() < 0.7:
                c["hunger_level"] = rng.randint(0, 100)
            ops.append(op("choose", **c))
        elif roll < 0.9:
            e = {"being": b, **rserving(rng, menu)}
            if rng.random() < 0.3:
                e["force"] = rng.random() < 0.6
            ops.append(op("eat", **e))
        else:
            known = [x["args"]["being"] for x in ops if x["op"] in ("set_diet", "eat")]
            ops.append(op("assess", being=rng.choice(known) if known and rng.random() < 0.95 else b))
    return {"kind": "diet", "args": {"ops": ops}}


def generate_conformance_cases() -> None:
    cases = make_cases()
    for c in cases:
        d = CONFORMANCE_DIR / c["id"]
        d.mkdir(parents=True, exist_ok=True)
        meta = {"op": "local", "description": c["description"], "error": c["error"]}
        (d / "case.json").write_text(json.dumps(meta, indent=2) + "\n")
        (d / "input.json").write_text(json.dumps(c["input"], indent=2) + "\n")
    print(f"Generated {len(cases)} conformance cases in {CONFORMANCE_DIR.relative_to(ROOT)}")


def check_blessed() -> int:
    """Python reference vs every blessed expected.json (and error-ness) on disk."""
    cases = make_cases()
    passed = 0
    problems = []
    for c in cases:
        d = CONFORMANCE_DIR / c["id"]
        on_disk = json.loads((d / "input.json").read_text())
        if on_disk != c["input"]:
            problems.append(f"{c['id']}: input.json is stale (run --generate)")
            continue
        try:
            out = py_workflow(c["input"])
            err = None
        except Exception as e:  # noqa: BLE001 - any refusal counts as an error case
            out, err = None, e
        expected_path = d / "expected.json"
        if c["error"]:
            if err is None:
                problems.append(f"{c['id']}: reference accepted a malformed case")
            elif expected_path.exists():
                problems.append(f"{c['id']}: error case has expected.json")
            else:
                passed += 1
        elif err is not None:
            problems.append(f"{c['id']}: reference refused: {err}")
        elif not expected_path.exists():
            problems.append(f"{c['id']}: not blessed yet")
        elif json.loads(expected_path.read_text()) != out:
            problems.append(f"{c['id']}: reference differs from blessed output")
        else:
            passed += 1
    for p in problems:
        print(p, file=sys.stderr)
    print(json.dumps({
        "cases": len(cases),
        "malformed": sum(1 for c in cases if c["error"]),
        "passed": passed,
        "actual_model_calls": 0,
        "api_spend_usd": 0,
    }))
    return 0 if passed == len(cases) else 1


def run_random_parity(count: int, seed: int) -> int:
    rng = random.Random(seed)
    rust_bin = ROOT / "target" / "debug" / "gamectx"
    if not rust_bin.exists():
        subprocess.run(["cargo", "build"], cwd=ROOT, check=True)
    ok_runs = err_runs = meals = 0
    for i in range(1, count + 1):
        req = rand_diet(rng)
        try:
            py_res, py_err = py_workflow(req), None
        except Exception as e:  # noqa: BLE001
            py_res, py_err = None, e
        payload = json.dumps(req)
        proc = subprocess.run([str(rust_bin), "local", "-"], input=payload, capture_output=True, text=True)
        if py_err is not None:
            if proc.returncode == 0:
                print(f"Iteration {i}: Python refused ({py_err}) but Rust accepted\n{payload}", file=sys.stderr)
                return 1
            err_runs += 1
            continue
        if proc.returncode != 0:
            print(f"Iteration {i}: Rust refused: {proc.stderr}\n{payload}", file=sys.stderr)
            return 1
        rust_res = json.loads(proc.stdout)
        if rust_res != py_res:
            print(f"Iteration {i} mismatch\nPy:   {json.dumps(py_res, sort_keys=True)}\n"
                  f"Rust: {json.dumps(rust_res, sort_keys=True)}\n{payload}", file=sys.stderr)
            return 1
        ok_runs += 1
        meals += proc.stdout.count('"eaten":true')
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "meals_eaten": meals, "actual_model_calls": 0, "api_spend_usd": 0,
    }))
    return 0


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--cap", type=float, default=0.0)
    p.add_argument("--random", type=int, default=0)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--generate", action="store_true")
    a = p.parse_args()
    if a.cap != 0:
        sys.exit("diet checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
