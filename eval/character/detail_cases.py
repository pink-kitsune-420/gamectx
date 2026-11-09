#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 D2: level of detail for minds
(full, coarse and crowd levels by distance, exact catch-up, work accounting, crowd summaries),
local kind `detail`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/detail/*/case.json + input.json
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
from detail_reference import workflow as lod_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "detail"
LIMIT_NAMES = ["beings", "needs", "groups"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "detail":
        raise ValueError("unknown local kind")
    return lod_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "detail", "args": args}


HUNGER = [{"id": "hunger", "rate": 10000, "level": 400000}, {"id": "thirst", "rate": 25000, "level": 100000}]


def track(b: str, distance: int, group: Optional[str] = None, needs: Optional[List[dict]] = None) -> Dict[str, Any]:
    a: Dict[str, Any] = {"being": b, "distance": distance, "needs": needs or HUNGER}
    if group is not None:
        a["group"] = group
    return op("track", **a)


def tick(now: int) -> Dict[str, Any]:
    return op("tick", now=now)


def move(b: str, distance: int, group: Optional[str] = None) -> Dict[str, Any]:
    a: Dict[str, Any] = {"being": b, "distance": distance}
    if group is not None:
        a["group"] = group
    return op("move", **a)


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    add("lod_levels_by_distance",
        "Distance 0..20 is full, 21..100 coarse, beyond that a crowd in a named group.",
        wf(track("smith", 0), track("miller", 50), track("farmer", 500, "east_fields"),
           *[op("assess", being=b) for b in ["smith", "miller", "farmer"]]))

    add("lod_full_updates_every_tick",
        "A full being is current after every tick; thirst turns urgent at tick 24 and hunger at tick 30, reported when reached.",
        wf(track("smith", 0), tick(20), tick(25), tick(31)))

    add("lod_coarse_updates_on_stride",
        "A coarse being is only brought up to multiples of 10; at tick 37 it was last updated at 30 (lag 7), though assess shows the exact state.",
        wf(track("miller", 50), tick(37), op("assess", being="miller")))

    add("lod_crowd_frozen_until_promoted",
        "A crowd member is not updated by ticks; moving it near catches it up and reports both urgent events at their exact ticks.",
        wf(track("farmer", 500, "east_fields"), tick(100), move("farmer", 5)))

    add("lod_same_state_every_level",
        "Three beings with the same needs at full, coarse and crowd detail all reach identical levels and event ticks once they come near.",
        wf(track("a", 0), track("b", 60), track("c", 900, "far_village"), tick(17), tick(43), tick(95),
           move("a", 0), move("b", 0), move("c", 0), *[op("assess", being=x) for x in "abc"]))

    add("lod_work_by_level",
        "Work for 100 ticks: a full being with 2 needs costs 200, a coarse one 20 (10 strides), a crowd group 1 per tick call.",
        wf(track("a", 0), track("b", 60), track("c", 900, "g"), track("d", 900, "g"), tick(100)))

    add("lod_crowd_summary",
        "A crowd summary gives members, the mean of each need at the current tick and how many needs are urgent.",
        wf(track("p1", 300, "market"), track("p2", 300, "market",
                                          [{"id": "hunger", "rate": 5000, "level": 0}, {"id": "thirst", "rate": 0}]),
           track("p3", 300, "market", [{"id": "hunger", "rate": 20000, "level": 900000}]), tick(40),
           op("crowd", group="market")))

    add("lod_demote_catches_up_first",
        "Moving a full being out to a crowd first brings it up to date; nothing is lost while it is frozen.",
        wf(track("smith", 0), tick(10), move("smith", 400, "forge_row"), tick(50), op("assess", being="smith"),
           move("smith", 0)))

    add("lod_configure_thresholds",
        "The host sets near 5, far 10, stride 5 before tracking; distance 8 is now coarse and 11 a crowd.",
        wf(op("configure", near=5, far=10, stride=5), track("a", 8), track("b", 11, "g"), tick(12),
           op("assess", being="a")))

    add("lod_need_caps_at_full",
        "Levels stop at 1,000,000 however long the gap.",
        wf(track("a", 0, needs=[{"id": "sleep", "rate": 300000, "level": 900000}]), tick(10), op("assess", being="a")))

    add("lod_zero_rate_never_urgent",
        "A need with rate 0 never becomes urgent.",
        wf(track("a", 0, needs=[{"id": "calm", "rate": 0, "level": 10}]), tick(1000), op("assess", being="a")))

    add("lod_already_urgent_no_event",
        "A need that starts at or above its threshold reports no new event.",
        wf(track("a", 0, needs=[{"id": "hunger", "rate": 1000, "level": 800000}]), tick(50)))

    add("lod_events_sorted_across_beings",
        "Events from one tick call are ordered by tick, then being, then need.",
        wf(track("z", 0, needs=[{"id": "a", "rate": 100000}]), track("y", 0, needs=[{"id": "b", "rate": 100000}]),
           track("x", 0, needs=[{"id": "c", "rate": 50000}]), tick(20)))

    add("lod_assess_does_not_change_state",
        "Assessing a crowd member shows its exact state but changes nothing: the later move still reports the events.",
        wf(track("farmer", 500, "east_fields"), tick(60), op("assess", being="farmer"), move("farmer", 0)))

    add("lod_move_between_groups",
        "A crowd member can move to another group; the summary follows it.",
        wf(track("a", 500, "north"), track("b", 500, "north"), move("a", 600, "south"), op("crowd", group="south"),
           op("crowd", group="north")))

    add("lod_coarse_promotion_mid_stride",
        "A coarse being last updated at 30 is caught up to 37 when promoted at tick 37.",
        wf(track("miller", 50), tick(37), move("miller", 0)))

    add("lod_custom_thresholds",
        "Each need has its own threshold; a low threshold turns urgent first.",
        wf(track("a", 0, needs=[{"id": "fear", "rate": 10000, "threshold": 100000},
                                {"id": "hunger", "rate": 10000, "threshold": 900000}]), tick(100)))

    add("lod_big_gap",
        "A tick to 10^12 caps levels without overflow at every level.",
        wf(track("a", 0), track("b", 60), track("c", 900, "g"), tick(1000000000000), move("c", 0),
           op("assess", being="b")))

    add("limits_echo_and_groups_cap",
        "Limits are echoed; with groups 1, two crowd members share the one group.",
        wf(track("a", 500, "g"), track("b", 500, "g"), op("crowd", group="g"), limits={"groups": 1}))

    add("bad_crowd_without_group",
        "A crowd-level being needs a group.",
        wf(track("a", 500)), error=True)

    add("bad_configure_after_track",
        "Levels are configured before any being is tracked.",
        wf(track("a", 0), op("configure", near=5)), error=True)

    add("bad_near_over_far",
        "near may not exceed far.",
        wf(op("configure", near=50, far=10)), error=True)

    add("bad_tick_backwards",
        "The clock is nondecreasing.",
        wf(tick(10), tick(9)), error=True)

    add("bad_duplicate_need",
        "Need ids are distinct.",
        wf(track("a", 0, needs=[{"id": "n", "rate": 1}, {"id": "n", "rate": 2}])), error=True)

    add("bad_groups_full",
        "With groups 1, a second group is refused.",
        wf(track("a", 500, "g"), track("b", 500, "h"), limits={"groups": 1}), error=True)

    return cases


BEINGS = ["ann", "bo", "cy", "dee", "eve"]
GROUPS = ["north", "south", "market"]


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


def rand_distance(rng: random.Random) -> int:
    return rng.choice([0, 10, 20, 21, 60, 100, 101, 500])


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    b = rng.choice(BEINGS)
    roll = rng.random()
    if roll < 0.04:
        args: Dict[str, Any] = {}
        for k, v in [("near", rng.randint(0, 30)), ("far", rng.randint(20, 150)), ("stride", rng.randint(1, 20))]:
            if rng.random() < 0.6:
                args[k] = v
        return op("configure", **args)
    if roll < 0.30:
        needs = []
        for i in range(rng.randint(1, 3)):
            n: Dict[str, Any] = {"id": f"n{i}", "rate": rng.choice([0, 1, 7, 1000, 25000, rng.randint(0, 1000000)])}
            if rng.random() < 0.6:
                n["level"] = rng.randint(0, 1000000)
            if rng.random() < 0.4:
                n["threshold"] = rng.randint(1, 1000000)
            needs.append(n)
        args = {"being": b, "distance": rand_distance(rng), "needs": needs}
        if rng.random() < 0.7:
            args["group"] = rng.choice(GROUPS)
        return op("track", **args)
    if roll < 0.55:
        args = {"being": b, "distance": rand_distance(rng)}
        if rng.random() < 0.7:
            args["group"] = rng.choice(GROUPS)
        return op("move", **args)
    if roll < 0.85:
        return op("tick", now=t)
    if roll < 0.93:
        return op("assess", being=b)
    return op("crowd", group=rng.choice(GROUPS))


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
    ok_runs = err_runs = events = moves = promotions = 0
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
        events += proc.stdout.count('"need":"')
        moves += proc.stdout.count('"from":"')
        promotions += proc.stdout.count('"from":"crowd","to":"full"') + proc.stdout.count('"from":"crowd","to":"coarse"')
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "urgent_events": events, "level_moves": moves, "crowd_promotions": promotions,
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
        sys.exit("detail checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
