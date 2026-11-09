#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 C3: space inside a place (grid
rooms, one being per cell, blocked paths, spots with queues, line of sight), local kind `space`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/space/*/case.json + input.json
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
from space_reference import workflow as space_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "space"
LIMIT_NAMES = ["rooms", "beings", "spots", "queue"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "space":
        raise ValueError("unknown local kind")
    return space_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "space", "args": args}


def enter(b: str, x: int, y: int, room: str = "yard") -> Dict[str, Any]:
    return op("enter", being=b, room=room, x=x, y=y)


# A yard 8 x 5 with a wall at x = 3 from y = 1 to 3, a well at (6, 2) and a bakery counter.
YARD = [op("room", id="yard", width=8, height=5, walls=[[3, 1], [3, 2], [3, 3]]),
        op("spot", room="yard", id="well", x=6, y=2),
        op("spot", room="yard", id="stall", x=1, y=4, capacity=2)]


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("space_walk_around_wall",
         "Mira walks from (0,2) to (5,2) around the wall in 9 steps.",
         wf(*YARD, enter("mira", 0, 2), op("move", being="mira", x=5, y=2)))

    case("space_beings_block_paths",
         "Guards stand in both gaps of the wall; once one steps aside to (4,1), Mira gets through the top gap in 9 steps.",
         wf(*YARD, enter("mira", 0, 2), enter("g1", 3, 0), enter("g2", 3, 4), op("move", being="g1", x=4, y=1),
            op("move", being="mira", x=5, y=2)))

    case("space_detour_around_being",
         "A being in a corridor cell forces a longer path.",
         wf(op("room", id="hall", width=5, height=3), enter("a", 0, 1, "hall"), enter("b", 2, 1, "hall"),
            op("move", being="a", x=4, y=1)))

    case("space_one_being_per_cell",
         "Moving onto another being's cell is refused; moving onto your own cell takes 0 steps.",
         wf(*YARD, enter("mira", 0, 2), op("move", being="mira", x=0, y=2), op("crowd", room="yard")))

    case("space_well_queue",
         "The well serves one: Tomas uses it, Mira and Ana wait in order, and each release serves the next.",
         wf(*YARD, enter("tomas", 6, 1), enter("mira", 5, 2), enter("ana", 7, 3),
            op("use", being="tomas", spot="well"), op("use", being="mira", spot="well"), op("use", being="ana", spot="well"),
            op("release", being="tomas", spot="well"), op("release", being="mira", spot="well")))

    case("space_far_being_waits",
         "A being not next to a free spot waits in line instead of using it.",
         wf(*YARD, enter("mira", 0, 0), op("use", being="mira", spot="well"), op("crowd", room="yard")))

    case("space_capacity_two",
         "The stall serves two at once; the third waits.",
         wf(*YARD, enter("a", 0, 4), enter("b", 2, 4), enter("c", 1, 3), *[op("use", being=x, spot="stall") for x in "abc"]))

    case("space_line_of_sight_blocked",
         "The wall blocks the line from (0,2) to (6,1) at (3,1); along the top row (0,0) sees (7,0).",
         wf(*YARD, enter("mira", 0, 2), enter("tomas", 6, 1), enter("ana", 0, 0), enter("pip", 7, 0),
            op("see", being="mira", other="tomas"), op("see", being="ana", other="pip")))

    case("space_sight_range",
         "With range 3 a being 6 cells away is not visible even with a clear line.",
         wf(*YARD, enter("ana", 0, 0), enter("tomas", 6, 0), op("see", being="ana", other="tomas", range=3),
            op("see", being="ana", other="tomas")))

    case("space_other_room_unseen",
         "Beings in different rooms never see each other.",
         wf(*YARD, op("room", id="bakery", width=3, height=3), enter("mira", 1, 1, "bakery"), enter("tomas", 0, 0),
            op("see", being="mira", other="tomas")))

    case("space_exit_serves_queue",
         "When the well's user leaves the room, the first in line is served.",
         wf(*YARD, enter("tomas", 6, 1), enter("mira", 5, 2), op("use", being="tomas", spot="well"),
            op("use", being="mira", spot="well"), op("exit", being="tomas"), op("crowd", room="yard")))

    case("space_leave_queue",
         "Releasing while waiting just leaves the line.",
         wf(*YARD, enter("tomas", 6, 1), enter("mira", 5, 2), enter("ana", 7, 3), op("use", being="tomas", spot="well"),
            op("use", being="mira", spot="well"), op("use", being="ana", spot="well"), op("release", being="mira", spot="well")))

    case("space_reenter_other_room",
         "Entering another room leaves the old one and its queues.",
         wf(*YARD, op("room", id="bakery", width=3, height=3), enter("tomas", 6, 1), enter("mira", 5, 2),
            op("use", being="tomas", spot="well"), op("use", being="mira", spot="well"), enter("tomas", 0, 0, "bakery"),
            op("crowd", room="yard"), op("crowd", room="bakery")))

    case("space_served_from_afar",
         "A being waiting from across the yard is served when the well frees; walking there is up to the host.",
         wf(*YARD, enter("tomas", 6, 1), enter("mira", 0, 0), op("use", being="tomas", spot="well"),
            op("use", being="mira", spot="well"), op("release", being="tomas", spot="well"), op("crowd", room="yard")))

    case("space_crowd_free_cells",
         "crowd counts free cells (area minus walls minus beings) and lists spots.",
         wf(*YARD, enter("a", 0, 0), enter("b", 7, 4), op("crowd", room="yard")))

    case("space_long_path_big_room",
         "A 256 x 256 room crossed corner to corner takes 510 steps.",
         wf(op("room", id="field", width=256, height=256), enter("a", 0, 0, "field"), op("move", being="a", x=255, y=255)))

    case("space_diagonal_line",
         "A perfect diagonal passes between cells only through the diagonal itself.",
         wf(op("room", id="r", width=5, height=5, walls=[[1, 2], [2, 1]]), enter("a", 0, 0, "r"), enter("b", 4, 4, "r"),
            op("see", being="a", other="b")))

    case("limits_echo_and_queue_cap",
         "Limits are echoed; with queue 1 one being may wait.",
         wf(*YARD, enter("t", 6, 1), enter("m", 5, 2), op("use", being="t", spot="well"), op("use", being="m", spot="well"),
            limits={"queue": 1}))

    case("bad_queue_full",
         "With queue 1, a second waiting being is refused.",
         wf(*YARD, enter("t", 6, 1), enter("m", 5, 2), enter("a", 7, 3), op("use", being="t", spot="well"),
            op("use", being="m", spot="well"), op("use", being="a", spot="well"), limits={"queue": 1}), error=True)

    case("bad_enter_wall",
         "Nobody stands in a wall.",
         wf(*YARD, enter("mira", 3, 2)), error=True)

    case("bad_enter_occupied",
         "One being per cell.",
         wf(*YARD, enter("mira", 0, 2), enter("ana", 0, 2)), error=True)

    case("bad_no_path",
         "A cell walled off on all sides cannot be reached.",
         wf(op("room", id="r", width=3, height=3, walls=[[1, 0], [0, 1], [2, 1], [1, 2]]), enter("a", 0, 0, "r"),
            op("move", being="a", x=1, y=1)), error=True)

    case("bad_guards_block_gaps",
         "With guards in both gaps of the wall the far side cannot be reached.",
         wf(*YARD, enter("mira", 0, 2), enter("g1", 3, 0), enter("g2", 3, 4), op("move", being="mira", x=5, y=2)),
         error=True)

    case("bad_use_twice",
         "A being already using or waiting cannot ask again.",
         wf(*YARD, enter("tomas", 6, 1), op("use", being="tomas", spot="well"), op("use", being="tomas", spot="well")),
         error=True)

    case("bad_spot_on_wall",
         "A spot cannot sit in a wall.",
         wf(op("room", id="r", width=3, height=3, walls=[[1, 1]]), op("spot", room="r", id="s", x=1, y=1)), error=True)

    return cases


BEINGS = ["ann", "bo", "cy", "dee", "eve"]


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


def rand_room(rng: random.Random, rid: str) -> Dict[str, Any]:
    w, h = rng.randint(1, 9), rng.randint(1, 9)
    walls = [[rng.randrange(w), rng.randrange(h)] for _ in range(rng.randint(0, w * h // 3))]
    return op("room", id=rid, width=w, height=h, walls=walls)


def rand_op(rng: random.Random) -> Dict[str, Any]:
    b, b2 = rng.sample(BEINGS, 2)
    roll = rng.random()
    if roll < 0.06:
        return rand_room(rng, rng.choice(["r1", "r2"]))
    if roll < 0.14:
        a: Dict[str, Any] = {"room": rng.choice(["r1", "r2"]), "id": rng.choice(["well", "stall"]),
                             "x": rng.randint(0, 8), "y": rng.randint(0, 8)}
        if rng.random() < 0.5:
            a["capacity"] = rng.randint(1, 3)
        return op("spot", **a)
    if roll < 0.34:
        return op("enter", being=b, room=rng.choice(["r1", "r2"]), x=rng.randint(0, 8), y=rng.randint(0, 8))
    if roll < 0.37:
        return op("exit", being=b)
    if roll < 0.57:
        return op("move", being=b, x=rng.randint(0, 8), y=rng.randint(0, 8))
    if roll < 0.72:
        return op("use", being=b, spot=rng.choice(["well", "stall"]))
    if roll < 0.80:
        return op("release", being=b, spot=rng.choice(["well", "stall"]))
    if roll < 0.95:
        a = {"being": b, "other": b2}
        if rng.random() < 0.3:
            a["range"] = rng.randint(0, 6)
        return op("see", **a)
    return op("crowd", room=rng.choice(["r1", "r2"]))


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op. Most workflows start with a room."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = [] if rng.random() < 0.15 else [rand_room(rng, "r1")]
    if not accepts(ops, limits):
        ops = []
    target = len(ops) + rng.randint(1, 24)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        ops.append(rand_op(rng))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
    if not ops:
        ops.append(op("room", id="r1", width=1, height=1))
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
    ok_runs = err_runs = steps = waits = hidden = 0
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
        steps += sum(r.get("steps", 0) for r in rust_res["results"] if isinstance(r, dict))
        waits += proc.stdout.count('"status":"waiting"')
        hidden += proc.stdout.count('"blocked":[')
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "steps_walked": steps, "queued_uses": waits, "sight_blocked": hidden,
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
        sys.exit("space checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
