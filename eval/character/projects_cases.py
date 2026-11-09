#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 4 stage 5: life projects, local kind
`projects`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/projects/*/case.json + input.json
  --random N --seed S     N randomized workflows, Python reference vs `gamectx local -`

All checks are offline: zero model calls, $0 API spend.
"""

import argparse
import copy
import json
import pathlib
import random
import subprocess
import sys
from typing import Any, Dict, List, Optional

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from projects_reference import CONDITIONS
from projects_reference import workflow as projects_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "projects"
LIMIT_NAMES = ["projects", "milestones"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "projects":
        raise ValueError("unknown local kind")
    return projects_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "projects", "args": args}


def ms(mid: str, kind: str, ref: str, expected: int, level: Optional[int] = None, alt: Any = None) -> Dict[str, Any]:
    cond: Dict[str, Any] = {"kind": kind, "ref": ref}
    if level is not None:
        cond["level"] = level
    m: Dict[str, Any] = {"id": mid, "condition": cond, "expected": expected}
    if alt is not None:
        m["alternative"] = alt
    return m


BREW = [ms("clear_plot", "task", "clear_plot", 40),
        ms("buy_hops", "item", "hops_sack", 20,
           alt=ms("trade_for_hops", "relationship", "farmer_wen", 30, level=60)),
        ms("first_brew", "fact", "ale_tasted", 60)]


def brewery(stake: int = 40, horizon: int = 200, milestones: Any = None, pid: str = "brewery") -> Dict[str, Any]:
    return op("propose", id=pid, owner="tamsin", aim="Open a small brewery", stake=stake, horizon=horizon,
              milestones=BREW if milestones is None else milestones)


def out(m: str, ok: bool, pid: str = "brewery") -> Dict[str, Any]:
    return op("outcome", project=pid, milestone=m, success=ok)


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("propose_view", "A new project: active, starting morale, the first milestone current.", wf(brewery()))
    case("progress_by_outcome", "Confirmed successes complete milestones in order and lift morale.",
         wf(brewery(), out("clear_plot", True), out("buy_hops", True)))
    case("complete_feeds", "Finishing the last milestone completes the project and feeds drift, lessons and aspirations.",
         wf(brewery(), out("clear_plot", True), out("buy_hops", True), out("first_brew", True)))
    case("persist_through_failure", "A failure the owner can bear: it persists.",
         wf(brewery(stake=90), out("clear_plot", False)))
    case("adapt_to_alternative", "After a second failure the owner swaps buying hops for trading for them.",
         wf(brewery(), out("clear_plot", True), out("buy_hops", False), out("buy_hops", False)))
    case("pause_without_alternative", "Low morale and no alternative: the owner pauses.",
         wf(brewery(stake=20), out("clear_plot", False), out("clear_plot", False)))
    case("abandon_grows_new_aim",
         "Sunk cost carries the owner through three failures; at the fourth it stops counting and the project is abandoned, with a new aim able to grow from it.",
         wf(brewery(), op("invest", project="brewery", time=1500), *[out("clear_plot", False) for _ in range(4)]))
    case("sunk_cost_keeps_going", "Heavy investment keeps a low-stake owner going where another would pause.",
         wf(brewery(stake=20), op("invest", project="brewery", time=1000, money=2000), out("clear_plot", False),
            out("clear_plot", False), brewery(stake=20, pid="cider"), out("clear_plot", False, "cider"),
            out("clear_plot", False, "cider")))
    case("sunk_cost_bounded", "After four failures in a row invested time and money no longer count.",
         wf(brewery(stake=100), op("invest", project="brewery", time=5000, money=9000),
            *[out("clear_plot", False) for _ in range(4)]))
    case("setback_lowers_morale", "A lost resource is a setback: morale falls by its severity and the owner decides.",
         wf(brewery(stake=10), op("setback", project="brewery", severity=40), op("setback", project="brewery", severity=100)))
    case("encourage_and_resume", "Support lifts a paused owner's morale and the project resumes.",
         wf(brewery(stake=20), out("clear_plot", False), out("clear_plot", False),
            op("encourage", project="brewery", amount=60), op("resume", project="brewery"),
            out("clear_plot", True)))
    case("weight_and_slot", "An active project proposes a task slot and a motive scored by stake and urgency.",
         wf(brewery(stake=80), op("weight", project="brewery")))
    case("weight_rises_near_deadline", "As the horizon nears with work left, urgency and the motive score rise.",
         wf(brewery(stake=80, horizon=1000), op("weight", project="brewery"), op("tick", now=880),
            op("weight", project="brewery")))
    case("weight_paused_none", "A paused project proposes nothing.",
         wf(brewery(stake=20), out("clear_plot", False), out("clear_plot", False), op("weight", project="brewery")))
    case("offscreen_success", "Unobserved, enough time and fair luck complete the current milestone.",
         wf(brewery(), op("offscreen", project="brewery", elapsed=40, luck=60), op("show", project="brewery")))
    case("offscreen_too_soon", "Too little time with ordinary luck resolves nothing.",
         wf(brewery(), op("offscreen", project="brewery", elapsed=10, luck=60)))
    case("offscreen_bad_luck", "Bad luck off-screen is a failure, decided like any other.",
         wf(brewery(), op("offscreen", project="brewery", elapsed=5, luck=10)))
    case("invest_accumulates", "Time and money invested accumulate and count as sunk cost.",
         wf(brewery(), op("invest", project="brewery", time=120, money=250), op("invest", project="brewery", money=700)))
    case("bad_outcome_not_current", "Malformed: only the current milestone has an outcome.",
         wf(brewery(), out("first_brew", True)), error=True)
    case("bad_duplicate_milestone_ids", "Malformed: milestone and alternative ids are unique in a project.",
         wf(brewery(milestones=[ms("a", "task", "t", 1, alt=ms("a", "fact", "f", 1))])), error=True)
    case("bad_relationship_without_level", "Malformed: a relationship condition names a level.",
         wf(brewery(milestones=[ms("a", "relationship", "wen", 1)])), error=True)
    case("bad_horizon_past", "Malformed: the horizon is not in the past.",
         wf(op("tick", now=50), brewery(horizon=49)), error=True)
    case("bad_resume_active", "Malformed: only a paused project resumes.",
         wf(brewery(), op("resume", project="brewery")), error=True)
    case("bad_outcome_after_abandon", "Malformed: an abandoned project takes no more outcomes.",
         wf(brewery(), op("invest", project="brewery", time=1500), *[out("clear_plot", False) for _ in range(4)],
            out("clear_plot", True)), error=True)
    case("bad_milestones_over_limit", "Malformed: one milestone more than the limit is refused.",
         wf(brewery(milestones=[ms(f"m{i}", "task", "t", 1) for i in range(3)]), limits={"milestones": 2}), error=True)
    return cases


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {"projects": rng.randint(1, 4), "milestones": rng.randint(1, 4)}
    lim = {k: lim[k] for k in rng.sample(LIMIT_NAMES, rng.randint(1, 2))}
    if rng.random() < 0.02:
        lim[rng.choice(LIMIT_NAMES)] = 0
    return lim


def accepts(ops: List[Dict[str, Any]], limits: Optional[Dict[str, int]]) -> bool:
    try:
        py_workflow(wf(*ops, limits=limits))
        return True
    except Exception:  # noqa: BLE001 - any refusal
        return False


PIDS = ["p1", "p2", "p3"]


def rand_ms(rng: random.Random, i: int) -> Dict[str, Any]:
    kind = rng.choice(CONDITIONS + ["wish"] * (rng.random() < 0.03))
    level = rng.choice([0, 60, 100, 101]) if kind == "relationship" and rng.random() < 0.95 else (
        5 if rng.random() < 0.03 else None)
    alt = None
    if rng.random() < 0.4:
        alt = ms(f"a{i}", rng.choice(["task", "fact"]), "r", rng.choice([1, 10, 50]))
    return ms(f"m{i}", kind, "r", rng.choice([1, 10, 50, 200, 0]), level=level, alt=alt)


def rand_op(rng: random.Random, state: Dict[str, Any]) -> Dict[str, Any]:
    roll = rng.random()
    pid = rng.choice(PIDS)
    if roll < 0.06:
        state["clock"] += rng.choice([0, 5, 50, 300])
        return op("tick", now=state["clock"] if rng.random() < 0.95 else 0)
    if roll < 0.18:
        return op("propose", id=pid, owner="o", aim=rng.choice(["Build a mill", "", "x" * 81]),
                  stake=rng.choice([0, 20, 40, 70, 100, 101]),
                  horizon=state["clock"] + rng.choice([0, 50, 500, 5000]) if rng.random() < 0.95 else 0,
                  milestones=[rand_ms(rng, i) for i in range(rng.randint(0, 4))])
    if roll < 0.24:
        a: Dict[str, Any] = {"project": pid}
        if rng.random() < 0.7:
            a["time"] = rng.choice([0, 100, 1000, 1_000_001])
        if rng.random() < 0.7:
            a["money"] = rng.choice([0, 300, 5000])
        return op("invest", **a)
    if roll < 0.62:
        return op("outcome", project=pid, milestone=rng.choice(["m0", "m1", "m2", "a0", "a1", "m3"]),
                  success=rng.choice([True, False, False, 1]))
    if roll < 0.68:
        return op("setback", project=pid, severity=rng.choice([1, 30, 100, 0]))
    if roll < 0.74:
        return op("encourage", project=pid, amount=rng.choice([0, 40, 100]))
    if roll < 0.80:
        return op("resume", project=pid)
    if roll < 0.88:
        return op("weight", project=pid)
    if roll < 0.95:
        return op("offscreen", project=pid, elapsed=rng.choice([0, 10, 50, 500]), luck=rng.choice([0, 19, 20, 49, 50, 100]))
    return op("show", project=pid)


PREFIX = [op("propose", id="p1", owner="o", aim="Build a mill", stake=40, horizon=400,
             milestones=[ms("m0", "task", "dig", 50, alt=ms("a0", "task", "hire", 60)),
                         ms("m1", "relationship", "lord", 30, level=50, alt=ms("a1", "fact", "deed", 10)),
                         ms("m2", "item", "stone", 100)]),
          op("propose", id="p2", owner="o", aim="Learn the lute", stake=80, horizon=100,
             milestones=[ms("m0", "fact", "chords", 20), ms("m1", "task", "recital", 40)])]


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = []
    if rng.random() < 0.8:
        for p in copy.deepcopy(PREFIX):
            ops.append(p)
            if not accepts(ops, limits):
                ops.pop()
    state = {"clock": 0}
    target = len(ops) + rng.randint(1, 18)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        ops.append(rand_op(rng, state))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
    if not ops:
        ops.append(op("tick", now=0))
    if rng.random() < 0.03:
        ops[rng.randrange(len(ops))]["args"]["note"] = "x"
    return wf(*copy.deepcopy(ops), limits=limits)


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
    ok_runs = err_runs = decisions = adapts = abandons = completions = 0
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
            r = r.get("result") or r
            if r.get("decision") is not None:
                decisions += 1
                adapts += int(r["decision"] == "adapt")
                abandons += int(r["decision"] == "abandon")
                completions += int(r["decision"] == "complete")
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "decisions": decisions, "adapts": adapts, "abandons": abandons, "completions": completions,
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
        sys.exit("projects checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
