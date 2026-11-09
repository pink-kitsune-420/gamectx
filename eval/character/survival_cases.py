#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 C1/C2: life cycle (stages, pairs,
children, belongings, death, heirs, grief) and threat response (danger appraisal, fight, flee,
hide, call for help, freeze, safety as a need), local kind `survival`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/survival/*/case.json + input.json
  --random N --seed S     N randomized workflows, Python reference vs `gamectx local -`

All checks are offline: zero model calls, $0 API spend.
"""

import argparse
import json
import pathlib
import random
import subprocess
import sys
from typing import Any, Dict, List, Optional

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from survival_reference import workflow as surv_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "survival"
LIMIT_NAMES = ["beings", "belongings", "children"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "survival":
        raise ValueError("unknown local kind")
    return surv_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "survival", "args": args}


def born(b: str, life: int = 1000, at: Optional[int] = None, parents: Optional[List[str]] = None) -> Dict[str, Any]:
    a: Dict[str, Any] = {"being": b, "lifespan": life}
    if at is not None:
        a["at"] = at
    if parents is not None:
        a["parents"] = parents
    return op("born", **a)


def age(now: int) -> Dict[str, Any]:
    return op("age", now=now)


def own(b: str, *things: str) -> List[Dict[str, Any]]:
    return [op("own", being=b, thing=t) for t in things]


def threat(b: str, danger: int, **kw: Any) -> Dict[str, Any]:
    return op("threat", being=b, threat=kw.pop("threat", "wolf"), danger=danger, **kw)


# A weaver (lifespan 1000) with a partner and two children, three belongings.
FAMILY = [born("weaver", 1000, 0), born("dyer", 1200, 0), age(300), op("pair", a="weaver", b="dyer"),
          born("elder_son", 1000, 300, ["weaver", "dyer"]), age(320), born("daughter", 1000, 320, ["weaver", "dyer"]),
          *own("weaver", "loom", "shears", "spindle")]


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    # ---- C1 life cycle
    add("life_stages_by_lifespan",
        "A being with lifespan 1,000 is a child until 225, an adult until 750, then an elder.",
        wf(born("apprentice", 1000, 0), age(224), age(225), age(749), age(750), op("assess", being="apprentice")))

    add("life_children_need_adult_parents",
        "Two adults pair and have children; each child lists both parents and each parent lists the children.",
        wf(*FAMILY, op("assess", being="weaver"), op("assess", being="daughter")))

    add("life_children_inherit_round_robin",
        "The weaver dies of age at 1,000: the loom, shears and spindle go to the children in birth order, round robin; the partner and children grieve.",
        wf(*FAMILY, age(1000), op("assess", being="elder_son")))

    add("life_partner_inherits_without_children",
        "A childless smith dies of a fever: the partner inherits everything and grieves (−60).",
        wf(born("smith", 1000, 0), born("cook", 1000, 0), age(300), op("pair", a="smith", b="cook"),
           *own("smith", "anvil", "tongs"), op("die", being="smith", cause="fever", at=410), op("assess", being="cook")))

    add("life_parents_inherit_last",
        "With no children or partner, living parents inherit, sorted by name.",
        wf(born("ma", 1000, 0), born("pa", 1000, 0), age(250), born("lad", 1000, 250, ["ma", "pa"]),
           *own("lad", "bow", "knife", "cap"), op("die", being="lad", cause="fall", at=260)))

    add("life_unclaimed_estate",
        "A hermit with no family leaves an unclaimed estate and no one grieves.",
        wf(born("hermit", 500, 0), *own("hermit", "hut", "goat"), age(500)))

    add("life_heir_full_passes_on",
        "With belongings limited to 2, the full elder son is skipped and the loom goes to the daughter; then both are full and the shears are unclaimed.",
        wf(*FAMILY[:7], *own("elder_son", "sling", "ring"), *own("daughter", "doll"), *own("weaver", "loom", "shears"),
           op("die", being="weaver", cause="flood"), limits={"belongings": 2}))

    add("life_widow_can_pair_again",
        "When a partner dies the survivor is unpaired and may pair again.",
        wf(born("smith", 1000, 0), born("cook", 1000, 0), born("miller", 1000, 0), age(300),
           op("pair", a="smith", b="cook"), op("die", being="smith", cause="fever"), op("pair", a="cook", b="miller")))

    add("life_born_in_the_past",
        "A being entered with a birth tick in the past starts at its current stage.",
        wf(age(600), born("old_tinker", 700, 0), born("young_tinker", 700, 500), op("assess", being="old_tinker")))

    add("life_age_kills_all_due",
        "One age call lets every being past its lifespan die, by name; a parent dying first does not stop a child's death in the same call.",
        wf(born("a", 100, 0), age(30), born("b", 60, 30, ["a"]), age(100)))

    # ---- C2 threat response
    add("threat_ignore_low_danger",
        "A distant fox (danger 10) appraises below 200,000: ignored, no fear, safety unchanged.",
        wf(born("shepherd", 1000, 0), threat("shepherd", 10, threat="fox")))

    add("threat_bold_fights",
        "A strong, bold guard (strength 80 vs 50) facing a thief fights: fight 700,000 beats flee.",
        wf(born("guard", 1000, 0), threat("guard", 50, threat="thief", temperament="bold", strength=80)))

    add("threat_timid_freezes",
        "A timid, hurt child (injury 20) cornered by a bear freezes: freeze 590,000 beats flee 490,000.",
        wf(born("child", 1000, 0), threat("child", 90, threat="bear", temperament="timid", strength=10,
                                          threat_strength=90, injury=20, exit=False)))

    add("threat_cautious_hides_in_cover",
        "A cautious shepherd with cover and no exit hides from wolves.",
        wf(born("shepherd", 1000, 0), threat("shepherd", 70, temperament="cautious", strength=40, threat_strength=60,
                                             cover=True, exit=False)))

    add("threat_steady_flees",
        "A steady traveller with an open road flees a storm.",
        wf(born("traveller", 1000, 0), threat("traveller", 80, threat="storm")))

    add("threat_calls_for_help",
        "With five allies near and no exit, a cautious miller calls for help.",
        wf(born("miller", 1000, 0), threat("miller", 60, threat="raider", temperament="cautious", allies=5, exit=False,
                                           strength=30, threat_strength=70)))

    add("threat_injury_changes_choice",
        "The same guard fights unhurt, but at injury 80 fighting drops below fleeing.",
        wf(born("guard", 1000, 0), threat("guard", 60, threat="boar", temperament="bold", strength=70),
           threat("guard", 60, threat="boar", temperament="bold", strength=70, injury=80)))

    add("safety_need_recovers",
        "A bear attack drops safety to 10 (unsafe); safety recovers by one per tick on age.",
        wf(born("hunter", 1000, 0), threat("hunter", 90, threat="bear"), age(20), op("assess", being="hunter"),
           age(100), op("assess", being="hunter")))

    add("limits_echo_and_children_cap",
        "Limits are echoed; with children 1 a parent has one child.",
        wf(born("ma", 1000, 0), age(300), born("kid", 1000, 300, ["ma"]), op("assess", being="ma"),
           limits={"children": 1}))

    # ---- malformed and boundary
    add("bad_child_cannot_parent",
        "A child (under 22.5% of its lifespan) cannot be a parent.",
        wf(born("kid", 1000, 0), born("baby", 1000, 0, ["kid"])), error=True)

    add("bad_unknown_temperament",
        "Temperament must be bold, steady, cautious or timid.",
        wf(born("a", 1000, 0), threat("a", 50, temperament="reckless")), error=True)

    add("bad_pair_twice",
        "A paired being cannot pair again while the partner lives.",
        wf(born("a", 100, 0), born("b", 100, 0), born("c", 100, 0), age(50), op("pair", a="a", b="b"),
           op("pair", a="a", b="c")), error=True)

    add("bad_age_backwards",
        "The clock is nondecreasing.",
        wf(age(10), age(9)), error=True)

    add("bad_dead_cannot_own",
        "The dead own nothing new.",
        wf(born("a", 100, 0), op("die", being="a", cause="fever"), op("own", being="a", thing="cup")), error=True)

    add("bad_danger_over_100",
        "Danger is 0..100.",
        wf(born("a", 100, 0), threat("a", 101)), error=True)

    return cases


BEINGS = ["ann", "bo", "cy", "dee", "eve"]
TEMPS = ["bold", "steady", "cautious", "timid"]
THINGS = ["cup", "knife", "loom", "goat", "coin"]


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {n: rng.randint(1, 4) for n in rng.sample(LIMIT_NAMES, rng.randint(1, 2))}
    if rng.random() < 0.02:
        lim[rng.choice(LIMIT_NAMES)] = 0
    return lim


def accepts(ops: List[Dict[str, Any]], limits: Optional[Dict[str, int]]) -> bool:
    try:
        py_workflow(wf(*ops, limits=limits))
        return True
    except Exception:  # noqa: BLE001 - any refusal
        return False


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    a, b, c = rng.sample(BEINGS, 3)
    roll = rng.random()
    if roll < 0.25:
        args: Dict[str, Any] = {"being": a, "lifespan": rng.choice([50, 100, 200, rng.randint(1, 400)])}
        if rng.random() < 0.7:
            args["at"] = max(0, t - rng.randint(0, 100))
        if rng.random() < 0.5:
            args["parents"] = rng.sample([b, c], rng.randint(1, 2))
        return op("born", **args)
    if roll < 0.33:
        return op("pair", a=a, b=b)
    if roll < 0.48:
        return op("own", being=a, thing=rng.choice(THINGS))
    if roll < 0.66:
        return op("age", now=t)
    if roll < 0.72:
        args = {"being": a, "cause": rng.choice(["fever", "fall"])}
        if rng.random() < 0.5:
            args["at"] = t
        return op("die", **args)
    if roll < 0.95:
        args = {"being": a, "threat": rng.choice(["wolf", "bear", "storm"]), "danger": rng.randint(0, 100)}
        for k, lo, hi in [("strength", 0, 100), ("threat_strength", 0, 100), ("injury", 0, 100), ("allies", 0, 8)]:
            if rng.random() < 0.5:
                args[k] = rng.randint(lo, hi)
        if rng.random() < 0.6:
            args["temperament"] = rng.choice(TEMPS)
        for k in ("exit", "cover"):
            if rng.random() < 0.5:
                args[k] = rng.random() < 0.5
        return op("threat", **args)
    return op("assess", being=a)


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = []
    t = 0
    target = rng.randint(1, 24)
    if rng.random() < 0.3:
        # A family: two adults (maybe paired) with children and belongings, so deaths have heirs.
        ma, pa, *kids = rng.sample(BEINGS, rng.randint(3, 5))
        life = rng.choice([200, 400])
        ops += [born(ma, life, 0), born(pa, life, 0), age(life // 3)]
        if rng.random() < 0.6:
            ops.append(op("pair", a=ma, b=pa))
        ops += [born(k, life, life // 3, rng.sample([ma, pa], rng.randint(1, 2))) for k in kids]
        ops += [op("own", being=rng.choice([ma, pa, *kids]), thing=th) for th in rng.sample(THINGS, rng.randint(1, 4))]
        t = life // 3
        if not accepts(ops, limits):
            ops, t = [], 0
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        t += rng.choice([0, 5, 20, 60])
        ops.append(rand_op(rng, t))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
    if not ops:
        ops.append(op("age", now=0))
    if rng.random() < 0.03:
        ops[rng.randrange(len(ops))]["args"]["note"] = "x"
    return wf(*ops, limits=limits)


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
    ok_runs = err_runs = births = deaths = inherited = responses = 0
    for i in range(1, count + 1):
        req = rand_workflow(rng)
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
        births += sum(1 for o in req["args"]["ops"] if o["op"] == "born")
        deaths += proc.stdout.count('"estate":')
        inherited += proc.stdout.count('"to":"')
        responses += sum(proc.stdout.count(f'"response":"{r}","safety"') for r in ("fight", "flee", "hide", "call_for_help", "freeze"))
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "births": births, "deaths": deaths, "belongings_inherited": inherited, "threat_responses": responses,
        "actual_model_calls": 0, "api_spend_usd": 0,
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
        sys.exit("survival checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
