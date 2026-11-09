#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 A7/A8: bounded rationality
(temperament settings, seeded impulse, plan depth, horizon, slips) and fallible memory (fading,
forgetting, rehearsal, marked distortions), local kind `cognition`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/cognition/*/case.json + input.json
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
from cognition_reference import workflow as cog_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "cognition"
LIMIT_NAMES = ["beings", "memories", "people", "motives", "steps"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "cognition":
        raise ValueError("unknown local kind")
    return cog_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "cognition", "args": args}


def mem(being: str, mid: str, about: str, what: str, day: int, people: List[str], importance: int,
        **kw: Any) -> Dict[str, Any]:
    return op("remember", being=being, id=mid, about=about, what=what, day=day, people=people, importance=importance,
              **kw)


MOTIVES = [{"id": "earn", "score": 70}, {"id": "rest", "score": 40}]
CHORES = [{"id": "fetch_grain"}, {"id": "mend", "familiar": False}, {"id": "grease", "familiar": False},
          {"id": "sweep"}, {"id": "patch", "familiar": False}, {"id": "sell"}, {"id": "count"}, {"id": "lock"}]
FIRE = [op("profile", being="tinker", temperament="dreamy"),
        mem("tinker", "fire", "barn", "burned", 12, ["miller"], 0),
        mem("tinker", "fair", "square", "dance", 20, ["widow", "smith"], 100),
        op("tick", now=240)]


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    # ---- A7 bounded rationality
    add("cog_profiles_from_temperament",
        "Each temperament sets plan depth, horizon, impulsiveness, memory span and slip; overrides replace one setting; a being without a profile reads as steady.",
        wf(*[op("profile", being=t, temperament=t) for t in ["careful", "steady", "impulsive", "dreamy"]],
           op("profile", being="apprentice", temperament="careful", impulsiveness=60),
           op("assess", being="stranger")))

    add("cog_impulse_skips_planning",
        "The impulsive tinker (impulsiveness 70) rolls 0..69 on seeds 0 and 1 and acts on its top motive with no plan.",
        wf(op("profile", being="tinker", temperament="impulsive"),
           op("decide", being="tinker", seed=0, motives=MOTIVES, steps=CHORES),
           op("decide", being="tinker", seed=1, motives=MOTIVES, steps=CHORES)))

    add("cog_plan_depth_cuts_steps",
        "With the same eight chores, the careful miller plans 6 and drops 2; the impulsive tinker, when it plans at all (seed 6), plans 2 and drops 6.",
        wf(op("profile", being="miller", temperament="careful"), op("profile", being="tinker", temperament="impulsive"),
           op("decide", being="miller", seed=0, motives=MOTIVES, steps=CHORES),
           op("decide", being="tinker", seed=6, motives=MOTIVES, steps=CHORES)))

    add("cog_horizon_drops_far_steps",
        "At tick 100 the dreamy miller (horizon 60) leaves out steps due after 160; the careful one (240) keeps the step due at 300.",
        wf(op("profile", being="miller", temperament="dreamy", impulsiveness=0),
           op("profile", being="smith", temperament="careful", impulsiveness=0),
           *[op("decide", being=b, seed=2, at=100, motives=MOTIVES,
                steps=[{"id": "sow", "eta": 120}, {"id": "reap", "eta": 300}, {"id": "thresh", "eta": 400}])
             for b in ["miller", "smith"]]))

    add("cog_unfamiliar_steps_slip",
        "With slip 1,000,000 every unfamiliar step slips and the first is reported; familiar steps never slip.",
        wf(op("profile", being="miller", impulsiveness=0, plan_depth=8, slip=1000000),
           op("decide", being="miller", seed=5, motives=MOTIVES, steps=CHORES)))

    add("cog_slip_rate_is_seeded",
        "At slip 200,000 seed 1 slips 'patch' (roll 132,031) but not 'mend' (472,639) or 'grease' (313,944); slip 0 never slips.",
        wf(op("profile", being="miller", impulsiveness=0, plan_depth=8, slip=200000),
           op("decide", being="miller", seed=1, motives=MOTIVES, steps=CHORES),
           op("profile", being="miller", impulsiveness=0, plan_depth=8, slip=0),
           op("decide", being="miller", seed=1, motives=MOTIVES, steps=CHORES)))

    add("cog_same_seed_same_decision",
        "Decisions are deterministic: the same seed gives the same result, other seeds can differ.",
        wf(op("profile", being="tinker", temperament="impulsive"),
           *[op("decide", being="tinker", seed=s, motives=MOTIVES, steps=CHORES) for s in [6, 6, 3]]))

    add("cog_top_motive_ties_by_id",
        "Equal motive scores pick the first id; a decide with no steps plans nothing.",
        wf(op("profile", being="miller", impulsiveness=0),
           op("decide", being="miller", seed=9, motives=[{"id": "work", "score": 50}, {"id": "eat", "score": 50}])))

    # ---- A8 fallible memory
    add("mem_fade_by_importance",
        "Under a steady span of 1,000, after 500 ticks importance 0 halves, importance 50 loses a quarter and importance 100 stays whole.",
        wf(mem("miller", "rain", "sky", "rained", 1, [], 0), mem("miller", "sale", "market", "sold_flour", 1, ["baker"], 50),
           mem("miller", "wed", "sister", "married", 1, ["sister"], 100), op("tick", now=500),
           op("assess", being="miller")))

    add("mem_span_from_temperament",
        "The same memory after 200 ticks: careful (span 2,000) keeps it vivid, dreamy (span 300) lets it go hazy.",
        wf(op("profile", being="clerk", temperament="careful"), op("profile", being="poet", temperament="dreamy"),
           mem("clerk", "toll", "bridge", "paid", 3, ["guard"], 20), mem("poet", "toll", "bridge", "paid", 3, ["guard"], 20),
           op("tick", now=200), op("assess", being="clerk"), op("assess", being="poet")))

    add("mem_forgotten_at_zero",
        "A trivial memory reaches zero and is forgotten; clarity changes are reported on the way.",
        wf(mem("miller", "rain", "sky", "rained", 1, [], 0), op("tick", now=150), op("tick", now=700),
           op("tick", now=1000), op("assess", being="miller")))

    add("mem_recall_rehearses",
        "Recalling a hazy memory adds 100,000; three recalls bring it back to clear.",
        wf(*FIRE, *[op("recall", being="tinker", id="fire", seed=1, errors=False) for _ in range(3)]))

    add("mem_wrong_day_marked_with_truth",
        "Seed 0 makes the hazy memory come back two days early; the reply keeps the truth beside what is told.",
        wf(*FIRE, op("recall", being="tinker", id="fire", seed=0)))

    add("mem_merged_person",
        "Seed 2 merges a person from another memory: the tinker now says the smith was at the barn fire.",
        wf(*FIRE, op("recall", being="tinker", id="fire", seed=2)))

    add("mem_conflicting_testimony",
        "Two witnesses of the same fire: the careful miller still recalls it clearly, the dreamy tinker misplaces the day.",
        wf(op("profile", being="miller", temperament="careful"), op("profile", being="tinker", temperament="dreamy"),
           mem("miller", "fire", "barn", "burned", 12, ["tinker"], 0), mem("tinker", "fire", "barn", "burned", 12, ["miller"], 0),
           op("tick", now=240), op("recall", being="miller", id="fire", seed=0), op("recall", being="tinker", id="fire", seed=0)))

    add("mem_clear_never_distorts",
        "A clear memory (strength ≥ 600,000) is told truly on every seed.",
        wf(mem("miller", "sale", "market", "sold_flour", 1, ["baker", "widow"], 50), op("tick", now=400),
           *[op("recall", being="miller", id="sale", seed=s) for s in range(4)]))

    add("mem_forget_on_purpose",
        "A memory can be erased on purpose; the being then has none.",
        wf(mem("guard", "bribe", "gate", "took_coin", 4, ["stranger"], 90), op("forget", being="guard", id="bribe"),
           op("assess", being="guard")))

    add("mem_full_book_evicts_weakest",
        "With memories limited to 2, a third memory evicts the weakest (then least important, then id).",
        wf(mem("miller", "a", "x", "y", 1, [], 10), mem("miller", "b", "x", "y", 1, [], 60), op("tick", now=100),
           mem("miller", "c", "x", "y", 2, [], 30), op("assess", being="miller"), limits={"memories": 2}))

    add("limits_echo_and_beings_cap",
        "Limits are echoed; with beings 2, a profile and a memory for two beings fit.",
        wf(op("profile", being="a"), mem("b", "m", "x", "y", 1, [], 50), op("assess", being="b"),
           limits={"beings": 2}))

    # ---- malformed and boundary
    add("bad_unknown_temperament",
        "Temperament must be careful, steady, impulsive or dreamy.",
        wf(op("profile", being="a", temperament="reckless")), error=True)

    add("bad_slip_over_million",
        "Slip is 0..1,000,000 millionths.",
        wf(op("profile", being="a", slip=1000001)), error=True)

    add("bad_tick_backwards",
        "The clock is nondecreasing.",
        wf(op("tick", now=9), op("tick", now=8)), error=True)

    add("bad_duplicate_memory_id",
        "A being cannot hold two memories with one id.",
        wf(mem("a", "m", "x", "y", 1, [], 50), mem("a", "m", "x", "z", 2, [], 50)), error=True)

    add("bad_people_over_limit",
        "With people limited to 1, a memory with two people is refused.",
        wf(mem("a", "m", "x", "y", 1, ["b", "c"], 50), limits={"people": 1}), error=True)

    add("bad_motives_empty",
        "A decision needs at least one motive.",
        wf(op("decide", being="a", seed=1, motives=[])), error=True)

    return cases


BEINGS = ["ann", "bo", "cy"]
TEMPS = ["careful", "steady", "impulsive", "dreamy"]
PEOPLE = ["ann", "bo", "cy", "dee", "eve"]


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {n: rng.randint(1, 4) for n in rng.sample(LIMIT_NAMES, rng.randint(1, 3))}
    if rng.random() < 0.02:
        lim[rng.choice(LIMIT_NAMES)] = 0
    return lim


def accepts(ops: List[Dict[str, Any]], limits: Optional[Dict[str, int]]) -> bool:
    try:
        py_workflow(wf(*ops, limits=limits))
        return True
    except Exception:  # noqa: BLE001 - any refusal
        return False


def rand_op(rng: random.Random, t: int, mids: List[str]) -> Dict[str, Any]:
    b = rng.choice(BEINGS)
    roll = rng.random()
    if roll < 0.12:
        args: Dict[str, Any] = {"being": b}
        if rng.random() < 0.8:
            args["temperament"] = rng.choice(TEMPS)
        for k, lo, hi in [("plan_depth", 1, 8), ("horizon", 1, 300), ("impulsiveness", 0, 100),
                          ("memory_span", 1, 2000), ("slip", 0, 1000000)]:
            if rng.random() < 0.15:
                args[k] = rng.randint(lo, hi)
        return op("profile", **args)
    if roll < 0.32:
        motives = [{"id": f"m{i}", "score": rng.randint(0, 100)} for i in range(rng.randint(1, 4))]
        steps = []
        for i in range(rng.randint(0, 10)):
            s: Dict[str, Any] = {"id": f"s{i}"}
            if rng.random() < 0.5:
                s["familiar"] = rng.random() < 0.5
            if rng.random() < 0.4:
                s["eta"] = t + rng.randint(0, 400)
            steps.append(s)
        args = {"being": b, "seed": rng.randint(0, 10**6), "motives": motives, "at": t}
        if steps or rng.random() < 0.5:
            args["steps"] = steps
        return op("decide", **args)
    if roll < 0.57:
        mid = f"e{rng.randint(1, 6)}"
        mids.append(mid)
        args = {"being": b, "id": mid, "about": "thing", "what": rng.choice(["fire", "fair", "theft"]),
                "day": rng.randint(0, 40), "people": rng.sample(PEOPLE, rng.randint(0, 3)),
                "importance": rng.choice([0, 0, 10, 20, 50, 100, rng.randint(0, 100)]), "at": t}
        return op("remember", **args)
    if roll < 0.70:
        return op("tick", now=t)
    if roll < 0.92:
        args = {"being": b, "id": rng.choice(mids) if mids else "e1", "seed": rng.randint(0, 10**6)}
        if rng.random() < 0.2:
            args["errors"] = rng.random() < 0.5
        return op("recall", **args)
    if roll < 0.95:
        return op("forget", being=b, id=rng.choice(mids) if mids else "e1")
    return op("assess", being=b)


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = []
    mids: List[str] = []
    t = 0
    target = rng.randint(1, 24)
    if rng.random() < 0.3:
        # A dreamy witness with several shared-people memories, ticked into the hazy range.
        b = rng.choice(BEINGS)
        ops.append(op("profile", being=b, temperament="dreamy"))
        for i in range(1, 4):
            mids.append(f"e{i}")
            ops.append(op("remember", being=b, id=f"e{i}", about="thing", what="fire", day=rng.randint(0, 40),
                          people=rng.sample(PEOPLE, rng.randint(1, 3)), importance=rng.choice([0, 10, 20])))
        t = rng.randint(120, 260)
        ops.append(op("tick", now=t))
        ops.extend(op("recall", being=b, id=f"e{rng.randint(1, 3)}", seed=rng.randint(0, 10**6))
                   for _ in range(rng.randint(1, 4)))
        if not accepts(ops, limits):
            ops = []
            mids = []
            t = 0
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        t += rng.choice([0, 10, 50, 150, 300])
        ops.append(rand_op(rng, t, mids))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
    if not ops:
        ops.append(op("assess", being=rng.choice(BEINGS)))
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
    ok_runs = err_runs = impulses = slips = distorted = merged = forgotten = 0
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
        impulses += proc.stdout.count('"mode":"impulse"')
        slips += proc.stdout.count('"slip":true')
        distorted += proc.stdout.count('"distorted":true')
        merged += proc.stdout.count('"kind":"merged_person"')
        forgotten += sum(len(r.get("forgotten", [])) for r in rust_res["results"] if isinstance(r, dict))
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "impulses": impulses, "slips": slips, "distorted_recalls": distorted, "merged_person": merged, "memories_forgotten": forgotten,
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
        sys.exit("cognition checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
