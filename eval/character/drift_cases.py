#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 B4: slow personality change
(capped trait drift from experience with reason chains, fading pressure, author locks),
local kind `drift`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/drift/*/case.json + input.json
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
from drift_reference import workflow as drift_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "drift"
LIMIT_NAMES = ["beings", "traits", "history"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "drift":
        raise ValueError("unknown local kind")
    return drift_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "drift", "args": args}


def exp(b: str, t: str, push: int, reason: str) -> Dict[str, Any]:
    return op("experience", being=b, trait=t, push=push, reason=reason)


TOMAS = [op("trait", being="tomas", trait="trust", base=600000),
         op("trait", being="tomas", trait="boldness", base=-200000, cap=50000, step=25000, threshold=200, fade=0)]


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("drift_betrayal_makes_guarded",
         "Three blows of -100 reach the threshold 300: trust moves one step to 590,000 with the three reasons.",
         wf(*TOMAS, exp("tomas", "trust", -100, "betrayed"), exp("tomas", "trust", -100, "robbed"),
            exp("tomas", "trust", -100, "lied_to")))

    case("drift_one_event_is_not_enough",
         "A single betrayal leaves trust unchanged; it only builds pressure.",
         wf(*TOMAS, exp("tomas", "trust", -100, "betrayed"), op("assess", being="tomas", trait="trust")))

    case("drift_pressure_fades",
         "Pressure fades by 1 per tick: two blows 200 ticks apart never add up to a shift.",
         wf(*TOMAS, exp("tomas", "trust", -100, "betrayed"), op("tick", now=100), exp("tomas", "trust", -100, "robbed"),
            op("tick", now=300), exp("tomas", "trust", -100, "lied_to"), op("assess", being="tomas", trait="trust")))

    case("drift_hard_cap",
         "Boldness may move at most 50,000 from its base: after two steps the pressure is spent without change.",
         wf(*TOMAS, *[exp("tomas", "boldness", 100, f"won_fight_{i}") for i in range(8)],
            op("assess", being="tomas", trait="boldness")))

    case("drift_kindness_undoes",
         "Kindness pushes the other way and can bring trust back toward its base.",
         wf(*TOMAS, *[exp("tomas", "trust", -100, "betrayed") for _ in range(3)],
            *[exp("tomas", "trust", 100, "kindness") for _ in range(3)], op("assess", being="tomas", trait="trust")))

    case("drift_mixed_signals_cancel",
         "Opposite pushes cancel inside the pressure, so nothing changes.",
         wf(*TOMAS, *[exp("tomas", "trust", s, r) for s, r in [(-100, "betrayed"), (100, "helped"), (-100, "robbed"),
                                                                (100, "gift")]], op("assess", being="tomas")))

    case("drift_reasons_newest_eight",
         "A shift keeps the newest 8 reasons that built it.",
         wf(op("trait", being="ana", trait="warmth", base=0, threshold=1000),
            *[exp("ana", "warmth", 100, f"kind_word_{i}") for i in range(10)]))

    case("drift_big_push_many_steps",
         "With threshold 50 one push of 100 moves two steps at once.",
         wf(op("trait", being="ana", trait="patience", base=0, threshold=50, step=5000), exp("ana", "patience", 100, "long_wait")))

    case("drift_author_lock_trait",
         "A locked trait ignores pushes; unlocking lets experience work again.",
         wf(*TOMAS, op("lock", being="tomas", trait="trust", locked=True),
            *[exp("tomas", "trust", -100, "betrayed") for _ in range(3)],
            op("lock", being="tomas", trait="trust", locked=False),
            *[exp("tomas", "trust", -100, "betrayed") for _ in range(3)]))

    case("drift_author_lock_being",
         "Locking the whole being freezes every trait.",
         wf(*TOMAS, op("lock", being="tomas", locked=True), exp("tomas", "boldness", 100, "won"),
            exp("tomas", "boldness", 100, "won"), op("assess", being="tomas")))

    case("drift_revert",
         "An author can revert a trait to its base, clearing pressure but keeping the history.",
         wf(*TOMAS, *[exp("tomas", "trust", -100, "betrayed") for _ in range(4)], op("revert", being="tomas", trait="trust")))

    case("drift_history_limit",
         "With history 2 only the two newest shifts are kept.",
         wf(op("trait", being="ana", trait="calm", base=0, threshold=100, cap=300000),
            *[exp("ana", "calm", 100, f"quiet_day_{i}") for i in range(4)], op("assess", being="ana"), limits={"history": 2}))

    case("drift_edges_of_range",
         "A base near 1,000,000 cannot drift past 1,000,000 even inside its cap.",
         wf(op("trait", being="saint", trait="kindness", base=995000, threshold=100),
            *[exp("saint", "kindness", 100, "good_deed") for _ in range(2)]))

    case("drift_redefine_trait",
         "Redefining a trait starts it again from the new base.",
         wf(*TOMAS, *[exp("tomas", "trust", -100, "betrayed") for _ in range(3)],
            op("trait", being="tomas", trait="trust", base=700000), op("assess", being="tomas", trait="trust")))

    case("drift_no_fade_setting",
         "With fade 0 pressure keeps however long the gap.",
         wf(*TOMAS, exp("tomas", "boldness", 100, "won"), op("tick", now=1000000), exp("tomas", "boldness", 100, "won")))

    case("drift_zero_cap",
         "A cap of 0 makes a trait fixed even without a lock.",
         wf(op("trait", being="ana", trait="faith", base=500000, cap=0, threshold=1),
            exp("ana", "faith", -100, "doubt"), op("assess", being="ana")))

    case("drift_assess_shows_faded_pressure",
         "assess reports pressure as faded to now without changing it.",
         wf(*TOMAS, exp("tomas", "trust", -100, "betrayed"), op("tick", now=40), op("assess", being="tomas", trait="trust"),
            exp("tomas", "trust", -100, "robbed")))

    case("limits_echo_and_traits_cap",
         "Limits are echoed; with traits 2 a being holds two traits.",
         wf(*TOMAS, op("assess", being="tomas"), limits={"traits": 2}))

    case("bad_traits_full",
         "With traits 2, a third trait is refused.",
         wf(*TOMAS, op("trait", being="tomas", trait="greed", base=0), limits={"traits": 2}), error=True)

    case("bad_push_too_large",
         "A push is -100..100.",
         wf(*TOMAS, exp("tomas", "trust", -101, "massacre")), error=True)

    case("bad_cap_too_large",
         "No trait may drift more than 300,000.",
         wf(op("trait", being="a", trait="t", base=0, cap=300001)), error=True)

    case("bad_unknown_trait",
         "Experience needs a defined trait.",
         wf(*TOMAS, exp("tomas", "greed", 10, "gold")), error=True)

    case("bad_lock_not_bool",
         "locked is true or false.",
         wf(*TOMAS, op("lock", being="tomas", locked=1)), error=True)

    case("bad_base_out_of_range",
         "Bases are -1,000,000..1,000,000.",
         wf(op("trait", being="a", trait="t", base=1000001)), error=True)

    case("bad_tick_backwards",
         "The clock is nondecreasing.",
         wf(op("tick", now=5), op("tick", now=4)), error=True)

    return cases


BEINGS = ["ann", "bo", "cy"]
TRAITS = ["trust", "boldness", "warmth"]


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


def rand_trait(rng: random.Random) -> Dict[str, Any]:
    a: Dict[str, Any] = {"being": rng.choice(BEINGS), "trait": rng.choice(TRAITS),
                         "base": rng.choice([0, -1000000, 1000000, 995000, rng.randint(-1000000, 1000000)])}
    for k, lo, hi in [("cap", 0, 300000), ("step", 1, 100000), ("threshold", 1, 400), ("fade", 0, 20)]:
        if rng.random() < 0.4:
            a[k] = rng.randint(lo, hi)
    return op("trait", **a)


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    b, tr = rng.choice(BEINGS), rng.choice(TRAITS)
    roll = rng.random()
    if roll < 0.2:
        return rand_trait(rng)
    if roll < 0.7:
        return exp(b, tr, rng.choice([-100, 100, -50, 50, rng.randint(-100, 100)]),
                   rng.choice(["betrayed", "helped", "won", "lost", "gift"]))
    if roll < 0.8:
        return op("tick", now=t)
    if roll < 0.86:
        a: Dict[str, Any] = {"being": b, "locked": rng.random() < 0.5}
        if rng.random() < 0.6:
            a["trait"] = tr
        return op("lock", **a)
    if roll < 0.9:
        return op("revert", being=b, trait=tr)
    a = {"being": b}
    if rng.random() < 0.5:
        a["trait"] = tr
    return op("assess", **a)


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
    ok_runs = err_runs = shifts = capped = locked = 0
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
            if isinstance(r, dict) and "shifts" in r:
                shifts += len(r["shifts"])
                capped += int(abs(r["drift"]) == r["cap"] and r["cap"] > 0)
                locked += int(r["locked"])
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "shifts": shifts, "experiences_at_cap": capped, "experiences_locked": locked,
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
        sys.exit("drift checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
