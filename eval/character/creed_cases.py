#!/usr/bin/env python3
"""Conformance cases and differential parity runner for creeds and conscience (pass 2,
stage 4 and B5, local kind `creed`).

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/creed/*/case.json + input.json
  --random N --seed S     N randomized workflows, Python reference vs `gamectx local -`

All checks are offline: zero model calls, $0 API spend. Every creed here is original.
"""

import argparse
import json
import pathlib
import random
import subprocess
import sys
from typing import Any, Dict, List

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from creed_reference import process_local

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "creed"


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    return process_local(request.get("kind"), request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


HEARTH = {"id": "hearthkeepers", "label": "Keepers of the Hearth", "kind": "religion",
          "tenets": [{"id": "no_theft", "action": "steal", "severity": 80, "label": "Take nothing from another's hearth"},
                     {"id": "no_ash_scatter", "action": "scatter", "sacred_only": True, "severity": 60,
                      "label": "Never scatter sacred ash"},
                     {"id": "no_flesh", "food_tag": "meat", "severity": 40, "label": "Eat nothing that bled"}],
          "observances": [{"id": "tend_flame", "action": "tend_hearth", "every": 24, "window": 4, "offset": 6,
                           "severity": 50, "label": "Tend the flame at dawn"}],
          "sacred": ["great_hearth"],
          "rivals": [{"creed": "free_tillers", "stance": "wary"}]}
TILLERS = {"id": "free_tillers", "label": "The Free Tillers", "kind": "ideology",
           "tenets": [{"id": "no_fences", "action": "fence", "severity": 70, "label": "Common fields stay open"},
                      {"id": "no_tithe", "action": "pay_tithe", "severity": 50, "label": "Owe no lord a sheaf"}],
           "observances": [{"id": "field_moot", "action": "attend_moot", "every": 168, "window": 24,
                            "severity": 30, "label": "Speak at the field moot"}],
           "rivals": [{"creed": "hearthkeepers", "stance": "hostile"}]}
STILL = {"id": "stillwater", "label": "The Stillwater Way", "kind": "philosophy",
         "tenets": [{"id": "no_drink", "food_tag": "alcohol", "severity": 30, "label": "Keep the mind clear"},
                    {"id": "no_boast", "action": "boast", "severity": 20, "label": "Let deeds speak"}],
         "rivals": [{"creed": "free_tillers", "stance": "tolerant"}]}
REG = [op("register_creed", creed=c) for c in (HEARTH, TILLERS, STILL)]


def cr(*ops: Dict[str, Any], reg: bool = True) -> Dict[str, Any]:
    return {"kind": "creed", "args": {"ops": (list(REG) if reg else []) + list(ops)}}


def adhere(b: str, c: Any, **kw: Any) -> Dict[str, Any]:
    return op("adhere", being=b, creed=c, **kw)


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    add("creed_register_three_kinds",
        "A religion, an ideology and a philosophy register; re-registering one replaces it without growing the total.",
        cr(op("register_creed", creed={"id": "stillwater", "label": "Stillwater, revised", "kind": "philosophy"})))

    add("creed_register_tradition_with_offset",
        "A tradition with an offset observance registers alone.",
        cr(op("register_creed", creed={"id": "harvest_rites", "label": "Harvest Rites", "kind": "tradition",
                                        "observances": [{"id": "first_sheaf", "action": "offer_sheaf", "every": 720,
                                                         "window": 48, "offset": 600, "severity": 40,
                                                         "label": "Offer the first sheaf"}]}),
           op("adhere", being="reeve", creed="harvest_rites", devotion=70),
           op("due", at=599), op("due", at=600), reg=False))

    add("creed_adhere_switch_and_leave",
        "Adhering reports the previous creed (devotion defaults to 50); leaving with a null creed zeroes devotion.",
        cr(adhere("baker", "hearthkeepers", devotion=90, at=5), adhere("baker", "free_tillers", at=9),
           adhere("baker", None, at=12), op("assess", being="baker")))

    add("creed_judge_unwitnessed_guilt",
        "A devout baker who steals alone feels guilt (80 x 90 / 100 = 72) but no shame and loses no standing.",
        cr(adhere("baker", "hearthkeepers", devotion=90), op("judge", actor="baker", action="steal")))

    add("creed_judge_witnessed_shame_and_indignation",
        "A fellow keeper sees the theft: shame is half the guilt, the witness resents the thief, standing drops.",
        cr(adhere("baker", "hearthkeepers", devotion=90), adhere("miller", "hearthkeepers", devotion=50),
           op("judge", actor="baker", action="steal", witnesses=["miller"], at=30),
           op("assess", being="baker")))

    add("creed_judge_witness_of_other_creed_indifferent",
        "A Free Tiller does not mind theft, so the thief feels guilt without shame.",
        cr(adhere("baker", "hearthkeepers", devotion=90), adhere("tiller", "free_tillers", devotion=80),
           op("judge", actor="baker", action="steal", witnesses=["tiller"])))

    add("creed_judge_unbeliever_actor_outrages_believers",
        "An unaffiliated actor who fences a common field feels nothing; two tillers are indignant.",
        cr(adhere("tiller", "free_tillers", devotion=80), adhere("tiller_two", "free_tillers", devotion=25),
           op("judge", actor="stranger", action="fence", witnesses=["tiller_two", "tiller"]),
           op("assess", being="stranger")))

    add("creed_judge_sacred_only_needs_sacred_target",
        "Scattering ash only breaks the tenet at a sacred place.",
        cr(adhere("baker", "hearthkeepers", devotion=100),
           op("judge", actor="baker", action="scatter", target="old_hearth"),
           op("judge", actor="baker", action="scatter", target="great_hearth"),
           op("judge", actor="baker", action="scatter")))

    add("creed_judge_food_tenet_only_when_eating",
        "A food-tag tenet is broken by eating meat, not by carrying it.",
        cr(adhere("baker", "hearthkeepers", devotion=60),
           op("judge", actor="baker", action="eat", food_tags=["meat", "herb"]),
           op("judge", actor="baker", action="carry", food_tags=["meat"]),
           op("judge", actor="baker", action="eat", food_tags=["grain"])))

    add("creed_personal_norms_full_weight",
        "Personal norms (B5) count at full weight and list every broken norm with its source.",
        cr(adhere("baker", "hearthkeepers", devotion=50),
           op("set_norms", being="baker", norms=[{"id": "keep_word", "action": "steal", "severity": 30,
                                                  "label": "Never take what was promised"},
                                                 {"id": "no_sweets", "food_tag": "sweet", "severity": 10,
                                                  "label": "Sweets are for guests"}]),
           op("judge", actor="baker", action="steal"),
           op("judge", actor="baker", action="eat", food_tags=["sweet", "grain"]),
           op("assess", being="baker")))

    add("creed_personal_norm_witness_costs_no_standing",
        "A witness outraged only by a personal norm resents the actor, but no creed standing changes.",
        cr(op("set_norms", being="widow", norms=[{"id": "no_lies", "action": "lie", "severity": 45,
                                                  "label": "Speak plainly"}]),
           adhere("liar", "stillwater", devotion=70),
           op("judge", actor="liar", action="lie", witnesses=["widow"])))

    add("creed_standing_loss_floor_of_one",
        "A mild breach still costs at least one point of standing; repeated breaches accumulate.",
        cr(adhere("sage", "stillwater", devotion=10), adhere("pupil", "stillwater", devotion=10),
           op("judge", actor="sage", action="boast", witnesses=["pupil"]),
           op("judge", actor="sage", action="boast", witnesses=["pupil"]),
           op("judge", actor="sage", action="boast", witnesses=["pupil"])))

    add("creed_mood_capped_at_100",
        "Guilt 100 plus shame 50 proposes a mood drop capped at 100.",
        cr(op("set_norms", being="zealot", norms=[{"id": "no_fences_ever", "action": "fence", "severity": 100,
                                                   "label": "Never fence"}]),
           adhere("zealot", "free_tillers", devotion=100), adhere("elder", "free_tillers", devotion=100),
           op("judge", actor="zealot", action="fence", witnesses=["elder"])))

    add("creed_observe_window",
        "Observing outside the window does nothing; inside it raises devotion and standing once per period.",
        cr(adhere("baker", "hearthkeepers", devotion=60),
           op("observe", being="baker", observance="tend_flame", at=5),
           op("observe", being="baker", observance="tend_flame", at=6),
           op("observe", being="baker", observance="tend_flame", at=9),
           op("observe", being="baker", observance="tend_flame", at=10),
           op("observe", being="baker", observance="tend_flame", at=31),
           op("assess", being="baker")))

    add("creed_due_motive_scores_clamped",
        "Open observances become goal motives scored severity x devotion / 100, clamped to 20..70; once the window closes unkept, each adherent is charged.",
        cr(op("register_creed", creed={"id": "vigil", "label": "The Long Vigil", "kind": "tradition",
                                        "observances": [{"id": "night_watch", "action": "keep_watch", "every": 10,
                                                         "window": 5, "severity": 95, "label": "Keep the night watch"}]}),
           adhere("guard", "vigil", devotion=100), adhere("cook", "vigil", devotion=10),
           adhere("baker", "hearthkeepers", devotion=80),
           op("due", at=3), op("due", at=7)))

    add("creed_due_missed_charged_once",
        "A missed window charges guilt and standing once; a kept window is cleared silently.",
        cr(adhere("baker", "hearthkeepers", devotion=80),
           op("observe", being="baker", observance="tend_flame", at=7),
           op("due", at=12), op("due", at=40), op("due", at=41), op("due", at=60),
           op("assess", being="baker")))

    add("creed_due_not_before_joining",
        "Windows that began before a being joined are never charged.",
        cr(adhere("baker", "hearthkeepers", devotion=80, at=20),
           op("due", at=29), op("due", at=40), op("due", at=41)))

    add("creed_due_single_being_and_zero_guilt",
        "due can be limited to one being; a devotion-0 adherent loses standing but has no mood effect.",
        cr(adhere("baker", "hearthkeepers", devotion=0), adhere("miller", "hearthkeepers", devotion=90),
           op("due", at=35, being="baker"), op("due", at=35)))

    add("creed_food_taboos_lax_and_norms",
        "Creed food tenets bind from devotion 30; personal food norms always do.",
        cr(adhere("lax", "hearthkeepers", devotion=29), adhere("firm", "hearthkeepers", devotion=30),
           op("set_norms", being="lax", norms=[{"id": "no_fish", "food_tag": "fish", "severity": 5,
                                                "label": "Fish disagree with me"}]),
           adhere("monk", "stillwater", devotion=90),
           op("food_taboos", being="lax"), op("food_taboos", being="firm"), op("food_taboos", being="monk"),
           op("food_taboos", being="nobody")))

    add("creed_regard_stances",
        "Regard is kin for a shared creed, the creed's stated rival stance, or neutral.",
        cr(adhere("baker", "hearthkeepers"), adhere("miller", "hearthkeepers"), adhere("tiller", "free_tillers"),
           adhere("monk", "stillwater"),
           op("regard", being="baker", other="miller"), op("regard", being="baker", other="tiller"),
           op("regard", being="tiller", other="baker"), op("regard", being="monk", other="tiller"),
           op("regard", being="tiller", other="monk"), op("regard", being="baker", other="wanderer")))

    add("creed_rejoin_resets_observances",
        "Re-adhering clears observed and settled periods and moves the joining time.",
        cr(adhere("baker", "hearthkeepers", devotion=60),
           op("observe", being="baker", observance="tend_flame", at=7),
           adhere("baker", "hearthkeepers", devotion=60, at=50),
           op("due", at=60), op("due", at=80), op("assess", being="baker")))

    add("creed_malformed_tenet_action_and_food",
        "A tenet must name exactly one of action or food_tag.",
        cr(op("register_creed", creed={"id": "muddled", "label": "Muddled", "kind": "religion",
                                        "tenets": [{"id": "both", "action": "eat", "food_tag": "meat",
                                                    "severity": 10, "label": "Both"}]}), reg=False), error=True)

    add("creed_malformed_actor_witnesses_self",
        "An actor cannot witness their own act.",
        cr(adhere("baker", "hearthkeepers"), op("judge", actor="baker", action="steal", witnesses=["baker"])),
        error=True)

    add("creed_malformed_observe_without_creed",
        "Observing needs an adherent of a creed with that observance.",
        cr(op("set_norms", being="drifter", norms=[]), op("observe", being="drifter", observance="tend_flame")),
        error=True)

    add("creed_malformed_window_longer_than_period",
        "An observance window cannot exceed its period, and a creed cannot rival itself.",
        cr(op("register_creed", creed={"id": "lopsided", "label": "Lopsided", "kind": "tradition",
                                        "observances": [{"id": "feast", "action": "feast", "every": 10,
                                                         "window": 11, "severity": 10, "label": "Feast"}]}),
           reg=False), error=True)
    return cases


BEINGS = ["baker", "miller", "tiller", "monk", "reeve", "stranger"]
ACTIONS = ["steal", "scatter", "eat", "fence", "pay_tithe", "boast", "lie", "carry"]
TAGS = ["alcohol", "dairy", "egg", "fish", "fruit", "grain", "herb", "meat", "mineral", "nut", "sweet",
        "vegetable", "water"]
OBS = ["tend_flame", "field_moot", "rand_rite"]


def rnorm(rng: random.Random, i: int, sacred: bool) -> Dict[str, Any]:
    n: Dict[str, Any] = {"id": f"n{i}", "severity": rng.randint(1, 100), "label": f"norm {i}"}
    if rng.random() < 0.6:
        n["action"] = rng.choice(ACTIONS)
        if sacred and rng.random() < 0.3:
            n["sacred_only"] = True
    else:
        n["food_tag"] = rng.choice(TAGS)
    if rng.random() < 0.02:
        n["food_tag"] = "meat"
        n["action"] = "eat"
    return n


def rand_creed_def(rng: random.Random) -> Dict[str, Any]:
    every = rng.randint(1, 60)
    c: Dict[str, Any] = {"id": "rand_faith", "label": "A random faith",
                         "kind": rng.choice(["ideology", "philosophy", "religion", "tradition"]),
                         "tenets": [rnorm(rng, i, True) for i in range(rng.randint(0, 4))],
                         "observances": [{"id": "rand_rite", "action": "pray", "every": every,
                                          "window": rng.randint(1, every), "offset": rng.randint(0, every - 1),
                                          "severity": rng.randint(1, 100), "label": "Pray"}],
                         "sacred": rng.sample(["shrine", "great_hearth", "well"], rng.randint(0, 2)),
                         "rivals": [{"creed": "hearthkeepers", "stance": rng.choice(["hostile", "tolerant", "wary"])}]}
    if rng.random() < 0.03:
        c["rivals"].append({"creed": "rand_faith", "stance": "wary"})
    return c


def rand_creed(rng: random.Random) -> Dict[str, Any]:
    ops: List[Dict[str, Any]] = list(REG) if rng.random() < 0.97 else []
    creeds = ["hearthkeepers", "free_tillers", "stillwater"] if ops else []
    if rng.random() < 0.3 or not creeds:
        ops.append(op("register_creed", creed=rand_creed_def(rng)))
        creeds.append("rand_faith")
    t = 0
    known: List[str] = []
    rites: Dict[str, str] = {}
    rite_of = {"hearthkeepers": "tend_flame", "free_tillers": "field_moot", "rand_faith": "rand_rite"}

    def pick(b: str) -> str:
        return rng.choice(known) if known and rng.random() < 0.98 else b
    for _ in range(rng.randint(1, 12)):
        roll = rng.random()
        b = rng.choice(BEINGS)
        t += rng.randint(0, 30)
        if roll < 0.25:
            a: Dict[str, Any] = {"being": b, "creed": rng.choice(creeds) if rng.random() < 0.9 else None}
            if a["creed"] is not None and rng.random() < 0.8:
                a["devotion"] = rng.randint(0, 100)
            if rng.random() < 0.6:
                a["at"] = t
            if rng.random() < 0.01:
                a["creed"] = "unknown_faith"
            ops.append(op("adhere", **a))
            known.append(b)
            rites.pop(b, None)
            if a["creed"] in rite_of:
                rites[b] = rite_of[a["creed"]]
        elif roll < 0.33:
            ops.append(op("set_norms", being=b, norms=[rnorm(rng, i, False) for i in range(rng.randint(0, 3))]))
            known.append(b)
        elif roll < 0.55:
            j: Dict[str, Any] = {"actor": b, "action": rng.choice(ACTIONS)}
            if rng.random() < 0.3:
                j["target"] = rng.choice(["great_hearth", "shrine", "old_hearth"])
            if j["action"] == "eat" or rng.random() < 0.1:
                j["food_tags"] = rng.sample(TAGS, rng.randint(1, 3))
            others = [x for x in BEINGS if x != b]
            j["witnesses"] = rng.sample(others, rng.randint(0, 3))
            if rng.random() < 0.01:
                j["witnesses"].append(b)
            if rng.random() < 0.5:
                j["at"] = t
            ops.append(op("judge", **j))
        elif roll < 0.68:
            if rng.random() < 0.02:
                ops.append(op("observe", being=b, observance=rng.choice(OBS), at=t))
            elif rites:
                b = rng.choice(sorted(rites))
                ops.append(op("observe", being=b, observance=rites[b], at=t))
            else:
                ops.append(op("adhere", being=b, creed=creeds[0], at=t))
                known.append(b)
                if creeds[0] in rite_of:
                    rites[b] = rite_of[creeds[0]]
        elif roll < 0.8:
            d: Dict[str, Any] = {"at": t}
            if known and rng.random() < 0.3:
                d["being"] = pick(b)
            ops.append(op("due", **d))
        elif roll < 0.87:
            ops.append(op("food_taboos", being=b))
        elif roll < 0.94:
            ops.append(op("regard", being=b, other=rng.choice([x for x in BEINGS if x != b])))
        else:
            ops.append(op("assess", being=pick(b)) if known else op("food_taboos", being=b))
    return {"kind": "creed", "args": {"ops": ops}}


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
    ok_runs = err_runs = verdicts = 0
    for i in range(1, count + 1):
        req = rand_creed(rng)
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
        verdicts += sum(1 for r in rust_res["results"] if "guilt" in r and r["guilt"] > 0)
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "guilty_verdicts": verdicts, "actual_model_calls": 0, "api_spend_usd": 0,
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
        sys.exit("creed checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
