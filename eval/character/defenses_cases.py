#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 4 stage 3: defense mechanisms and
strain, local kind `defenses`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/defenses/*/case.json + input.json
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
from defenses_reference import CRISES, MECHANISMS, RELIEF, VALUES
from defenses_reference import workflow as defenses_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "defenses"
LIMIT_NAMES = ["beings", "distortions"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "defenses":
        raise ValueError("unknown local kind")
    return defenses_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "defenses", "args": args}


def prof(being: str, maturity: int, **extra: Any) -> Dict[str, Any]:
    a = {"being": being, "maturity": maturity, "tolerance": extra.pop("tolerance", 20),
         "crisis": extra.pop("crisis", "confession")}
    a.update(extra)
    return op("profile", **a)


def threat(being: str, fact: str, drop: int = 50, value: str = "competence", **extra: Any) -> Dict[str, Any]:
    return op("threat", being=being, value=value, drop=drop, fact=fact, **extra)


QM = "quartermaster"


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("profile_view", "A new profile: no strain, its band, threshold and crisis outcome.",
         wf(prof(QM, 55, threshold=600_000, cooldown=48, crisis="flight", style=["withdrawal"],
                 protects=["open_lockbox", "forged_tally"])))
    case("below_tolerance_no_defense", "A drop within the tolerance is not ego-threatening: nothing happens.",
         wf(prof(QM, 55, tolerance=30), threat(QM, "late_delivery", drop=30)))
    case("secret_exposed_denial", "An exposed protected secret threatens even at a small drop; a low-maturity being denies it.",
         wf(prof(QM, 25, protects=["open_lockbox"]), threat(QM, "open_lockbox", drop=10, value="secret")))
    case("unprotected_secret_ignored", "A secret the being does not protect is no threat.",
         wf(prof(QM, 25, protects=["open_lockbox"]), threat(QM, "spilled_ink", drop=90, value="secret")))
    case("projection_by_style", "A being whose style projects blames the apprentice for the open lockbox.",
         wf(prof(QM, 25, style=["projection"]), threat(QM, "open_lockbox", blame="apprentice")))
    case("displacement_to_safer_target", "Anger at the guard captain lands on the apprentice instead.",
         wf(prof(QM, 25, style=["displacement", "projection"]),
            threat(QM, "captain_rebuke", value="belonging", safer_target="apprentice", blame="captain")))
    case("style_skipped_without_input", "Projection needs someone to blame; without one the band default (denial) is used.",
         wf(prof(QM, 25, style=["projection"]), threat(QM, "open_lockbox")))
    case("rationalization_self_serving", "A mid-maturity being with an excuse rationalizes; the excuse is tagged self-serving.",
         wf(prof(QM, 55), threat(QM, "short_rations", value="goodness", excuse="The barrels were mislabelled.")))
    case("mid_without_excuse", "Without an excuse a mid-maturity being talks procedure; a withdrawing style goes quiet.",
         wf(prof(QM, 55), prof("scribe", 55, style=["withdrawal"]), threat(QM, "short_rations"),
            threat("scribe", "smudged_ledger")))
    case("humour_high", "A mature being jokes the threat away, keeping less of it.",
         wf(prof(QM, 85), threat(QM, "lost_bet", drop=40)))
    case("sublimation_project", "A mature being with a project works harder on it.",
         wf(prof(QM, 85, style=["sublimation"]), threat(QM, "failed_audit", drop=60, project="new_ledger")))
    case("strain_lowers_band", "Repeated threats raise strain and move the being from mature to immature defenses.",
         wf(prof(QM, 80, threshold=1_000_000, style=["sublimation"]),
            *[threat(QM, f"blow_{i}", drop=70, project="new_ledger", excuse="Bad luck.") for i in range(5)]))
    case("crisis_once_then_cooldown", "Strain at the threshold breaks the being once; within the cooldown it does not break again; after it, it does.",
         wf(prof(QM, 25, threshold=300_000, cooldown=10, crisis="outburst"),
            threat(QM, "a", drop=100), threat(QM, "b", drop=100), threat(QM, "c", drop=100),
            op("tick", now=10), threat(QM, "d", drop=100)))
    case("contradiction_costs_more", "Contradicting a denied fact costs half again as much; contradicting an accepted fact costs nothing.",
         wf(prof(QM, 25), threat(QM, "open_lockbox", drop=40), op("contradict", being=QM, fact="open_lockbox", drop=40),
            op("contradict", being=QM, fact="spilled_ink", drop=40)))
    case("defense_loop", "The same denial three times is a defense loop.",
         wf(prof(QM, 25, threshold=1_000_000), threat(QM, "open_lockbox", drop=30),
            op("contradict", being=QM, fact="open_lockbox", drop=10),
            op("contradict", being=QM, fact="open_lockbox", drop=10), op("status", being=QM)))
    case("decay_slower_while_denying", "Strain decays at half speed while a denial is held.",
         wf(prof(QM, 25), prof("scribe", 85), threat(QM, "open_lockbox", drop=100),
            threat("scribe", "lost_bet", drop=75), op("tick", now=50), op("status", being=QM),
            op("status", being="scribe")))
    case("relieve_kinds", "Rest, support and coping relieve strain at different rates, never below zero.",
         wf(prof(QM, 55), threat(QM, "short_rations", drop=100, excuse="Mislabelled."),
            op("relieve", being=QM, kind="rest", amount=50), op("relieve", being=QM, kind="support", amount=50),
            op("relieve", being=QM, kind="coping", amount=100), op("relieve", being=QM, kind="rest", amount=100)))
    case("accept_drops_distortion", "Accepting a denied fact drops the distortion, and strain decays at full speed again.",
         wf(prof(QM, 25), threat(QM, "open_lockbox", drop=100), op("accept", being=QM, fact="open_lockbox"),
            op("tick", now=20), op("status", being=QM)))
    case("reprofile_keeps_strain", "A new profile (maturity drifted) keeps strain and distortions.",
         wf(prof(QM, 25), threat(QM, "open_lockbox", drop=100), op("tick", now=10), prof(QM, 75, tolerance=10),
            threat(QM, "lost_bet", drop=20)))
    case("bad_style_unknown_mechanism", "Malformed: style names only defense mechanisms.",
         wf(prof(QM, 55, style=["sulking"])), error=True)
    case("bad_unprofiled_threat", "Malformed: a threat needs a profiled being.",
         wf(threat("stranger", "open_lockbox")), error=True)
    case("bad_value_name", "Malformed: the self-value is competence, goodness, belonging or secret.",
         wf(prof(QM, 55), threat(QM, "x", value="pride")), error=True)
    case("bad_blame_self", "Malformed: a being cannot be its own scapegoat.",
         wf(prof(QM, 25), threat(QM, "x", blame=QM)), error=True)
    case("bad_accept_unknown_fact", "Malformed: only a held distortion can be accepted.",
         wf(prof(QM, 25), op("accept", being=QM, fact="open_lockbox")), error=True)
    case("bad_distortions_over_limit", "Malformed: one distortion more than the limit is refused.",
         wf(prof(QM, 25), threat(QM, "a"), threat(QM, "b"), limits={"distortions": 1}), error=True)
    case("bad_tick_backwards", "Malformed: the clock never runs backwards.",
         wf(op("tick", now=5), op("tick", now=4)), error=True)
    return cases


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {"beings": rng.randint(1, 4), "distortions": rng.randint(1, 4)}
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


BEINGS = ["b1", "b2", "b3", "b4"]
FACTS = ["f1", "f2", "f3", "f4", "f5"]


def rand_op(rng: random.Random, clock: List[int]) -> Dict[str, Any]:
    roll = rng.random()
    b = rng.choice(BEINGS)
    if roll < 0.08:
        clock[0] += rng.choice([0, 1, 5, 40, 200, 1000])
        return op("tick", now=rng.choice([clock[0]] * 9 + [0, 1_000_000_000_001]))
    if roll < 0.20:
        a: Dict[str, Any] = {"being": b, "maturity": rng.choice([0, 25, 39, 40, 55, 69, 70, 85, 100, 101]),
                             "tolerance": rng.choice([0, 10, 20, 50, 100]), "crisis": rng.choice(CRISES + ["sulk"])}
        if rng.random() < 0.5:
            a["threshold"] = rng.choice([1, 100_000, 300_000, 800_000, 1_000_000, 0, None])
        if rng.random() < 0.4:
            a["cooldown"] = rng.choice([1, 10, 72, 0, None])
        if rng.random() < 0.6:
            a["style"] = rng.sample(MECHANISMS, rng.randint(0, 3)) + rng.choice([[]] * 8 + [["sulking"], ["denial"]])
        if rng.random() < 0.5:
            a["protects"] = rng.sample(FACTS, rng.randint(0, 3))
        return op("profile", **a)
    if roll < 0.62:
        a = {"being": b, "value": rng.choice(VALUES + VALUES + ["pride"]), "drop": rng.choice([0, 10, 30, 50, 70, 100, 101]),
             "fact": rng.choice(FACTS[:2] * 2 + FACTS)}
        for k in ("blame", "safer_target", "project"):
            if rng.random() < 0.4:
                a[k] = rng.choice(BEINGS + ["apprentice", "ledger"])
        if rng.random() < 0.4:
            a["excuse"] = rng.choice(["Bad luck.", "", "x" * 121, "The lock was old."])
        return op("threat", **a)
    if roll < 0.76:
        return op("contradict", being=b, fact=rng.choice(FACTS[:2] * 3 + FACTS), drop=rng.choice([0, 20, 60, 100, 101]))
    if roll < 0.86:
        return op("relieve", being=b, kind=rng.choice(list(RELIEF) + ["sleep"]), amount=rng.choice([0, 30, 100, 101]))
    if roll < 0.92:
        return op("accept", being=b, fact=rng.choice(FACTS))
    return op("status", being=rng.choice(BEINGS + ["nobody"]))


PREFIX = [prof("b1", 85, style=["sublimation"], protects=["f1"]), prof("b2", 55, threshold=400_000, cooldown=30),
          prof("b3", 25, style=["projection", "displacement"], protects=["f2", "f3"], crisis="flight"),
          prof("b4", 40, tolerance=0, threshold=200_000, cooldown=5, crisis="collapse")]


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
    clock = [0]
    target = len(ops) + rng.randint(1, 16)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        ops.append(rand_op(rng, clock))
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
    ok_runs = err_runs = defenses = crises = loops = contradictions = 0
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
            defenses += int(r.get("mechanism") is not None)
            crises += int(r.get("crisis") is not None and "crisis" in r)
            loops += int(r.get("loop") is True)
            contradictions += int(r.get("distorted") is True)
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "defenses": defenses, "crises": crises, "loops": loops, "costly_contradictions": contradictions,
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
        sys.exit("defenses checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
