#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 B7: status, roles and jobs
(duties that propose tasks, seats, status from roles and standing, contests and persuasion),
local kind `roles`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/roles/*/case.json + input.json
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
from roles_reference import workflow as roles_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "roles"
LIMIT_NAMES = ["roles", "beings", "duties", "held"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "roles":
        raise ValueError("unknown local kind")
    return roles_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "roles", "args": args}


def assign(b: str, r: str) -> Dict[str, Any]:
    return op("assign", being=b, role=r)


TOWN = [
    op("role", id="guard", status=60, seats=2,
       duties=[{"id": "patrol", "action": "travel", "every": 8, "place": "wall", "priority": 70},
               {"id": "report", "action": "talk", "every": 24, "at": 20}]),
    op("role", id="baker", status=30, duties=[{"id": "bake", "action": "cook", "every": 24, "at": 4, "place": "oven"}]),
    op("role", id="elder", status=85, seats=1, duties=[{"id": "council", "action": "talk", "every": 72, "at": 60}]),
    assign("tomas", "guard"), assign("mira", "baker"), assign("old_ana", "elder"),
]


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("roles_duties_propose_tasks",
         "By tick 24 the guard patrols (at 24, with 8 and 16 missed) and reports (at 20), the baker bakes (at 4); each is a task proposal.",
         wf(*TOWN, op("tick", now=24)))

    case("roles_gap_reports_missed",
         "After 50 quiet ticks the guard gets one patrol task for tick 48 with 5 missed and one report at 44 with 1 missed.",
         wf(*TOWN, op("tick", now=50)))

    case("roles_no_repeat_in_same_window",
         "A duty that already ran is not proposed again until it comes due.",
         wf(*TOWN, op("tick", now=8), op("tick", now=12), op("tick", now=16)))

    case("roles_new_holder_starts_now",
         "A guard assigned at tick 30 is not given duties that came due before 30.",
         wf(*TOWN, op("tick", now=30), assign("brun", "guard"), op("tick", now=40)))

    case("roles_unstaffed_seats",
         "The guard role has 2 seats and 1 holder, so every tick lists it as unstaffed until a second guard joins.",
         wf(*TOWN, op("tick", now=1), assign("brun", "guard"), op("tick", now=2), op("staff", role="guard")))

    case("roles_status_from_highest_role",
         "Status is the highest held role: a baker who is also an elder has 85.",
         wf(*TOWN[:3], assign("mira", "baker"), assign("mira", "elder"), op("assess", being="mira")))

    case("roles_esteem_shifts_status",
         "Earned standing adds to role status and is clamped to -50..50; status is clamped to 0..100.",
         wf(*TOWN, op("esteem", being="mira", delta=20, reason="fed_the_poor"),
            op("esteem", being="mira", delta=20, reason="saved_a_child"),
            op("esteem", being="mira", delta=20, reason="won_the_fair"),
            op("esteem", being="old_ana", delta=20, reason="wise_ruling"), op("assess", being="old_ana")))

    case("roles_contest_lower_defers",
         "A guard (60) against a baker (30): edge 300,000 and the baker defers.",
         wf(*TOWN, op("contest", a="tomas", b="mira")))

    case("roles_contest_close_no_defer",
         "With a gap under 20 nobody defers; the edge still leans.",
         wf(*TOWN, op("esteem", being="mira", delta=15, reason="good_bread"), op("contest", a="mira", b="tomas")))

    case("roles_persuade_status_and_shared_role",
         "The elder persuading the baker gains 275,000 from status; two guards share a role and gain 50,000.",
         wf(*TOWN, assign("brun", "guard"),
            op("persuade", speaker="old_ana", listener="mira", chance=400000),
            op("persuade", speaker="brun", listener="tomas", chance=400000),
            op("persuade", speaker="mira", listener="old_ana", chance=100000)))

    case("roles_vacate_stops_duties",
         "A baker who leaves the role gets no more baking tasks and loses its status.",
         wf(*TOWN, op("tick", now=5), op("vacate", being="mira", role="baker"), op("tick", now=60),
            op("assess", being="mira")))

    case("roles_redefine_role",
         "Redefining a role replaces its status and duties for current holders.",
         wf(*TOWN, op("role", id="baker", status=45, duties=[{"id": "sell", "action": "trade", "every": 10}]),
            op("tick", now=20), op("assess", being="mira")))

    case("roles_beings_without_roles",
         "Esteem alone makes a being known with status from standing only.",
         wf(op("esteem", being="stranger", delta=-10, reason="theft"), op("esteem", being="child", delta=5, reason="helped"),
            op("contest", a="child", b="stranger")))

    case("roles_big_gap",
         "A tick to 10^12 gives each duty one task with a large missed count and no overflow.",
         wf(*TOWN, op("tick", now=1000000000000)))

    case("roles_duty_offset_future",
         "A duty that first falls due at tick 60 is not proposed before then.",
         wf(*TOWN, op("tick", now=59), op("tick", now=60)))

    case("roles_same_tick_no_tasks",
         "A tick to the current clock proposes nothing.",
         wf(*TOWN, op("tick", now=0)))

    case("roles_staff_lists_holders",
         "staff lists holders by name with seats and status.",
         wf(*TOWN, assign("brun", "guard"), op("staff", role="guard"), op("staff", role="elder")))

    case("limits_echo_and_held_cap",
         "Limits are echoed; with held 1 a being keeps one role.",
         wf(*TOWN[:3], assign("mira", "baker"), op("assess", being="mira"), limits={"held": 1}))

    case("bad_seats_full",
         "The elder role has one seat.",
         wf(*TOWN, assign("mira", "elder")), error=True)

    case("bad_held_full",
         "With held 1, a second role is refused.",
         wf(*TOWN[:3], assign("mira", "baker"), assign("mira", "elder"), limits={"held": 1}), error=True)

    case("bad_unknown_role",
         "Only defined roles can be assigned.",
         wf(assign("mira", "witch")), error=True)

    case("bad_esteem_too_large",
         "One esteem change is at most 20 either way.",
         wf(op("esteem", being="mira", delta=21, reason="miracle")), error=True)

    case("bad_contest_self",
         "A being cannot contest itself.",
         wf(*TOWN, op("contest", a="mira", b="mira")), error=True)

    case("bad_duty_every_zero",
         "Duties recur at least every tick.",
         wf(op("role", id="x", status=1, duties=[{"id": "d", "action": "wait", "every": 0}])), error=True)

    case("bad_tick_backwards",
         "The clock is nondecreasing.",
         wf(*TOWN, op("tick", now=10), op("tick", now=9)), error=True)

    return cases


BEINGS = ["ann", "bo", "cy", "dee", "eve"]
ROLES = ["guard", "baker", "elder"]


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


def rand_role(rng: random.Random) -> Dict[str, Any]:
    a: Dict[str, Any] = {"id": rng.choice(ROLES), "status": rng.randint(0, 100)}
    if rng.random() < 0.5:
        a["seats"] = rng.randint(1, 3)
    if rng.random() < 0.85:
        duties = []
        for i in range(rng.randint(0, 3)):
            d: Dict[str, Any] = {"id": f"d{i}", "action": rng.choice(["travel", "cook", "talk", "wait"]),
                                 "every": rng.choice([1, 3, 8, 24, rng.randint(1, 100)])}
            if rng.random() < 0.5:
                d["at"] = rng.randint(0, 60)
            if rng.random() < 0.4:
                d["place"] = rng.choice(["wall", "oven", "hall"])
            if rng.random() < 0.3:
                d["priority"] = rng.randint(0, 100)
            duties.append(d)
        a["duties"] = duties
    return op("role", **a)


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    b, b2 = rng.sample(BEINGS, 2)
    roll = rng.random()
    if roll < 0.15:
        return rand_role(rng)
    if roll < 0.40:
        return assign(b, rng.choice(ROLES))
    if roll < 0.46:
        return op("vacate", being=b, role=rng.choice(ROLES))
    if roll < 0.56:
        return op("esteem", being=b, delta=rng.randint(-20, 20), reason=rng.choice(["gift", "theft", "duty"]))
    if roll < 0.76:
        return op("tick", now=t)
    if roll < 0.84:
        return op("contest", a=b, b=b2)
    if roll < 0.92:
        return op("persuade", speaker=b, listener=b2, chance=rng.choice([0, 300000, 1000000, rng.randint(0, 1000000)]))
    if roll < 0.96:
        return op("assess", being=b)
    return op("staff", role=rng.choice(ROLES))


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = []
    t = 0
    target = rng.randint(1, 24)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        t += rng.choice([0, 3, 7, 10, 40])
        ops.append(rand_op(rng, t))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
    if not ops:
        ops.append(op("tick", now=0))
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
    ok_runs = err_runs = tasks = missed = defers = 0
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
        for r in rust_res["results"]:
            if isinstance(r, dict) and "tasks" in r:
                tasks += len(r["tasks"])
                missed += sum(t["missed"] for t in r["tasks"])
            if isinstance(r, dict) and r.get("defers") is not None:
                defers += 1
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "duty_tasks": tasks, "missed_duties": missed, "contests_deferred": defers,
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
        sys.exit("roles checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
