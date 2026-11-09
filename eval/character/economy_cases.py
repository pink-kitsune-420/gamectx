#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 C4: economy feedback (prices that
follow scarcity from production and needs, with aspiration proposals), local kind `economy`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/economy/*/case.json + input.json
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
from economy_reference import workflow as economy_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "economy"
LIMIT_NAMES = ["goods", "markets", "traders"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "economy":
        raise ValueError("unknown local kind")
    return economy_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "economy", "args": args}


def flow(kind: str, good: str, amount: int, by: str, market: str = "square") -> Dict[str, Any]:
    return op(kind, market=market, good=good, amount=amount, by=by)


GOODS = [op("good", id="flour", base=20), op("good", id="salt", base=50, elasticity=500),
         op("good", id="fish", base=30, spoil=500)]


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("econ_shortage_raises_price",
         "Demand 30 against no stock: pressure 1000, target 2500, the multiplier moves halfway to 1750 and flour is scarce.",
         wf(*GOODS, flow("want", "flour", 30, "baker"), op("settle", now=24)))

    case("econ_glut_lowers_price",
         "Stock 40 with no demand: pressure -1000, target clamped to the floor 250, multiplier 625, a glut.",
         wf(*GOODS, flow("produce", "flour", 40, "miller"), op("settle", now=24)))

    case("econ_balance_holds_price",
         "Equal stock and demand keep the price at base.",
         wf(*GOODS, flow("produce", "flour", 20, "miller"), flow("want", "flour", 20, "baker"), op("settle", now=24)))

    case("econ_scarcity_proposes_aspirations",
         "Supply 10 against demand 70 makes flour scarce: the baker, 60 short, gets a need (weight 10 + unmet) and the miller an opportunity (multiplier / 50).",
         wf(*GOODS, flow("produce", "flour", 10, "miller"), flow("want", "flour", 70, "baker"),
            flow("buy", "flour", 10, "baker"), op("settle", now=24)))

    case("econ_met_wants_no_need",
         "A baker who bought all it wanted gets no need proposal even when the good is scarce.",
         wf(*GOODS, flow("produce", "flour", 5, "miller"), flow("want", "flour", 5, "baker"), flow("want", "flour", 40, "cook"),
            flow("buy", "flour", 5, "baker"), op("settle", now=24)))

    case("econ_price_recovers_over_periods",
         "After a shortage, balanced periods bring the multiplier back toward 1000 by half the gap each time.",
         wf(*GOODS, flow("want", "flour", 30, "baker"), op("settle", now=24),
            flow("produce", "flour", 10, "miller"), flow("want", "flour", 10, "baker"), flow("buy", "flour", 10, "baker"),
            op("settle", now=48),
            flow("produce", "flour", 10, "miller"), flow("want", "flour", 10, "baker"), flow("buy", "flour", 10, "baker"),
            op("settle", now=72)))

    case("econ_low_elasticity_salt",
         "Salt with elasticity 500 rises only to 1250 at full shortage, so it is not scarce.",
         wf(*GOODS, flow("want", "salt", 10, "cook"), op("settle", now=24)))

    case("econ_spoilage",
         "Fish spoils by half each settlement, so unsold stock shrinks.",
         wf(*GOODS, flow("produce", "fish", 40, "fisher"), op("settle", now=24), op("settle", now=48),
            op("price", market="square", good="fish")))

    case("econ_buy_costs_current_price",
         "A purchase costs amount times the price at the current multiplier.",
         wf(*GOODS, flow("want", "flour", 30, "baker"), op("settle", now=24), flow("produce", "flour", 10, "miller"),
            flow("buy", "flour", 4, "baker")))

    case("econ_markets_independent",
         "The same good has its own price in each market.",
         wf(*GOODS, flow("want", "flour", 30, "baker", "square"), flow("produce", "flour", 30, "miller", "harbor"),
            op("settle", now=24), op("assess", market="square"), op("assess", market="harbor")))

    case("econ_stock_carries_over",
         "Unsold, unspoiled stock stays for the next period and eases the next shortage.",
         wf(*GOODS, flow("produce", "flour", 30, "miller"), op("settle", now=24), flow("want", "flour", 30, "baker"),
            op("settle", now=48)))

    case("econ_custom_floor_ceiling_rate",
         "A good with ceiling 3000 and rate 1000 jumps straight to its target.",
         wf(op("good", id="iron", base=100, ceiling=3000, rate=1000), flow("want", "iron", 5, "smith"),
            op("settle", now=24)))

    case("econ_multiple_producers_and_wanters",
         "Several wanters and producers are listed in name order in the proposals.",
         wf(*GOODS, flow("produce", "flour", 2, "miller_b"), flow("produce", "flour", 1, "miller_a"),
            flow("want", "flour", 9, "cook"), flow("want", "flour", 20, "baker"), op("settle", now=24)))

    case("econ_settle_same_tick",
         "Settling twice at the same tick is allowed; the second period is empty.",
         wf(*GOODS, flow("want", "flour", 30, "baker"), op("settle", now=24), op("settle", now=24)))

    case("econ_redefine_good",
         "Redefining a good keeps its markets and uses the new base price.",
         wf(*GOODS, flow("produce", "flour", 10, "miller"), op("good", id="flour", base=40),
            op("price", market="square", good="flour")))

    case("econ_assess_market",
         "assess lists every good traded in a market.",
         wf(*GOODS, flow("produce", "flour", 10, "miller"), flow("want", "salt", 3, "cook"), op("assess", market="square")))

    case("econ_large_amounts",
         "Amounts at the bound of 10^6 settle without overflow.",
         wf(*GOODS, *[flow("want", "flour", 1000000, f"b{i}") for i in range(10)], op("settle", now=24)))

    case("limits_echo_and_traders_cap",
         "Limits are echoed; with traders 2 two beings may trade a good in a period.",
         wf(*GOODS, flow("produce", "flour", 1, "a"), flow("want", "flour", 1, "b"), flow("want", "flour", 1, "a"),
            limits={"traders": 2}))

    case("bad_traders_full",
         "With traders 1, a second being is refused.",
         wf(*GOODS, flow("produce", "flour", 1, "a"), flow("want", "flour", 1, "b"), limits={"traders": 1}), error=True)

    case("bad_unknown_good",
         "Only defined goods trade.",
         wf(flow("want", "gold", 1, "a")), error=True)

    case("bad_buy_more_than_stock",
         "A purchase needs stock.",
         wf(*GOODS, flow("produce", "flour", 3, "miller"), flow("buy", "flour", 4, "baker")), error=True)

    case("bad_amount_zero",
         "Amounts are at least 1.",
         wf(*GOODS, flow("produce", "flour", 0, "miller")), error=True)

    case("bad_floor_over_1000",
         "The floor is at most the base multiplier 1000.",
         wf(op("good", id="x", base=1, floor=1001)), error=True)

    case("bad_markets_full",
         "With markets 1, a second market is refused.",
         wf(*GOODS, flow("want", "flour", 1, "a", "m1"), flow("want", "flour", 1, "a", "m2"), limits={"markets": 1}),
         error=True)

    case("bad_settle_backwards",
         "The clock is nondecreasing.",
         wf(*GOODS, op("settle", now=10), op("settle", now=9)), error=True)

    return cases


BEINGS = ["ann", "bo", "cy", "dee"]
GOOD_IDS = ["flour", "salt", "fish"]
MARKETS = ["square", "harbor"]


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


def rand_good(rng: random.Random) -> Dict[str, Any]:
    a: Dict[str, Any] = {"id": rng.choice(GOOD_IDS), "base": rng.choice([1, 20, 999, rng.randint(1, 1000000000)])}
    for k, lo, hi in [("elasticity", 0, 10000), ("floor", 1, 1000), ("ceiling", 1000, 100000), ("rate", 1, 1000),
                      ("spoil", 0, 1000)]:
        if rng.random() < 0.3:
            a[k] = rng.randint(lo, hi)
    return op("good", **a)


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    roll = rng.random()
    if roll < 0.12:
        return rand_good(rng)
    kind = None
    if roll < 0.40:
        kind = "produce"
    elif roll < 0.68:
        kind = "want"
    elif roll < 0.80:
        kind = "buy"
    if kind:
        return op(kind, market=rng.choice(MARKETS), good=rng.choice(GOOD_IDS),
                  amount=rng.choice([1, 5, 30, rng.randint(1, 1000000)]), by=rng.choice(BEINGS))
    if roll < 0.92:
        return op("settle", now=t)
    if roll < 0.96:
        return op("price", market=rng.choice(MARKETS), good=rng.choice(GOOD_IDS))
    return op("assess", market=rng.choice(MARKETS))


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op. Most workflows start by defining the goods."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = [] if rng.random() < 0.2 else [op("good", id=g, base=10 * (i + 1)) for i, g in enumerate(GOOD_IDS)]
    if not accepts(ops, limits):
        ops = []
    t = 0
    target = len(ops) + rng.randint(1, 24)
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
        ops.append(op("settle", now=0))
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
    ok_runs = err_runs = scarce = glut = proposals = 0
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
        scarce += proc.stdout.count('"state":"scarce"')
        glut += proc.stdout.count('"state":"glut"')
        proposals += proc.stdout.count('"engine":"aspiration"')
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "scarce_settlements": scarce, "glut_settlements": glut, "aspiration_proposals": proposals,
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
        sys.exit("economy checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
