#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 B2/B3: cooperation (joint tasks,
help, hand-offs, promises) and conflict (claims, queues, contests, grudges), local kind
`cooperation`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/cooperation/*/case.json + input.json
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
from cooperation_reference import workflow as coop_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "cooperation"
LIMIT_NAMES = ["tasks", "roles", "promises", "resources", "queue", "beings", "grudges", "debts"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "cooperation":
        raise ValueError("unknown local kind")
    return coop_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "cooperation", "args": args}


def form(tid: str, organizer: str, *roles: Any, goal: str = "Shared work") -> Dict[str, Any]:
    rows = [{"role": r} if isinstance(r, str) else {"role": r[0], "being": r[1]} for r in roles]
    return op("form", id=tid, organizer=organizer, goal=goal, roles=rows)


def ask(tid: str, role: str, asker: str, helper: str, trust: int, **kw: Any) -> Dict[str, Any]:
    return op("ask_help", task=tid, role=role, asker=asker, helper=helper, trust=trust, **kw)


HARVEST = form("harvest", "farmer", ("reap", "farmer"), "cart", "stack", goal="Bring in the barley")


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    # ---- B2 joint tasks and help
    add("task_form_assign_complete",
        "The farmer forms a harvest with three roles, assigns the stacking, and the task goes open → ready → done.",
        wf(HARVEST, op("assign", task="harvest", role="cart", being="carter", by="farmer"),
           op("assign", task="harvest", role="stack", being="niece", by="farmer"),
           op("complete", task="harvest", role="reap", being="farmer", at=3),
           op("complete", task="harvest", role="cart", being="carter", at=4),
           op("complete", task="harvest", role="stack", being="niece", at=5)))

    add("help_accept_and_refuse",
        "Trust 60 and light effort reach exactly 500,000 (accepted); a busy neighbor with trust 10 scores below and refuses.",
        wf(HARVEST, ask("harvest", "stack", "farmer", "neighbor", 10, busy=True, effort=30),
           ask("harvest", "cart", "farmer", "carter", 60, effort=20)))

    add("help_completed_puts_asker_in_debt",
        "A helper completes the role: a relationship 'help' proposal and the farmer now owes the carter one favor.",
        wf(HARVEST, ask("harvest", "cart", "farmer", "carter", 80, effort=10),
           op("complete", task="harvest", role="cart", being="carter", at=9), op("assess", being="farmer"),
           op("assess", being="carter")))

    add("help_favor_owed_tips_refusal",
        "With trust 0 the carter would refuse, but the carter owes the farmer two favors (+200,000) and helps; completing repays one.",
        wf(form("mill", "carter", ("grind", "carter"), "haul"), ask("mill", "haul", "carter", "farmer", 90, effort=0),
           op("complete", task="mill", role="haul", being="farmer"),
           form("loft", "carter", ("sweep", "carter"), "lift"), ask("loft", "lift", "carter", "farmer", 90, effort=0),
           op("complete", task="loft", role="lift", being="farmer"),
           HARVEST, ask("harvest", "cart", "farmer", "carter", 0, effort=0),
           op("complete", task="harvest", role="cart", being="carter"), op("assess", being="carter")))

    add("help_leave_breaks_promise_and_grudge",
        "A helper walks off the job: 'broken_promise', a 200,000 grudge for the asker, and the role reopens.",
        wf(HARVEST, ask("harvest", "cart", "farmer", "carter", 70, effort=20),
           op("leave", task="harvest", role="cart", being="carter", at=6), op("grudges", being="farmer")))

    add("help_grudge_weighs_against",
        "The carter still resents the farmer (a broken promise, 300,000): half of it comes off the help score, which now fails.",
        wf(op("promise", id="seed_loan", **{"from": "farmer"}, to="carter", what="repay_seed", due=2),
           op("tick", now=3), HARVEST, ask("harvest", "cart", "farmer", "carter", 60, effort=20)))

    add("hand_off_clears_helper_link",
        "The carter hands the cart role to a cousin; the cousin was not asked, so completing proposes no help and no debt.",
        wf(HARVEST, ask("harvest", "cart", "farmer", "carter", 70, effort=20),
           op("hand_off", task="harvest", role="cart", **{"from": "carter"}, to="cousin"),
           op("complete", task="harvest", role="cart", being="cousin"), op("assess", being="farmer")))

    add("member_may_ask_help",
        "A role holder (not only the organizer) may ask for help; trust 50 with no effort scores exactly 500,000.",
        wf(HARVEST, op("assign", task="harvest", role="cart", being="carter", by="farmer"),
           ask("harvest", "stack", "carter", "cousin", 50, effort=0)))

    # ---- B2 promises
    add("promise_kept_on_time_and_late",
        "A promise proposes an expectation 'hope' for the receiver; keeping it on time is 'kept', after due 'kept_late'.",
        wf(op("promise", id="bread", **{"from": "baker"}, to="widow", what="deliver_bread", due=10, at=1),
           op("promise", id="nails", **{"from": "smith"}, to="baker", what="forge_nails", due=5),
           op("keep", id="bread", at=10), op("keep", id="nails", at=8), op("assess", being="baker")))

    add("promise_released_never_breaks",
        "The receiver releases one promise; a tick past both dues breaks only the other.",
        wf(op("promise", id="a", **{"from": "smith"}, to="miller", what="mend_wheel", due=4),
           op("promise", id="b", **{"from": "smith"}, to="baker", what="forge_nails", due=4),
           op("release", id="a", by="miller"), op("tick", now=5), op("grudges", being="baker")))

    add("grudge_fades_with_time",
        "A 300,000 grudge fades by 1,000 per tick: 100 ticks later it weighs 200,000, after 300 it is gone.",
        wf(op("promise", id="p", **{"from": "smith"}, to="miller", what="mend_wheel", due=0),
           op("tick", now=1), op("tick", now=101), op("grudges", being="miller"), op("tick", now=301),
           op("grudges", being="miller")))

    # ---- B3 resources
    add("claim_queue_by_urgency_and_status",
        "The well is held; later claimants queue by urgency·6,000 + status·4,000, ties by arrival; freeing passes to the head.",
        wf(op("claim", resource="well", being="ann", at=1),
           op("claim", resource="well", being="bo", urgency=30, status=90, at=2),
           op("claim", resource="well", being="cy", urgency=90, status=10, at=3),
           op("claim", resource="well", being="dee", urgency=30, status=90, at=4),
           op("free", resource="well", being="ann", at=5), op("assess", being="bo")))

    add("contest_yield_with_resentment",
        "An urgent, higher-status stranger (trust 0) scores 910,000: the holder yields, queues, and resents it (+200,000).",
        wf(op("claim", resource="forge", being="smith"),
           op("contest", resource="forge", challenger="lord", trust=0, urgency=90, status=90, holder_urgency=50,
              holder_status=40, at=7), op("grudges", being="smith")))

    add("contest_yield_without_resentment",
        "A trusted friend (trust 50) scores exactly 650,000; the holder yields gladly and holds no grudge.",
        wf(op("claim", resource="oven", being="baker"),
           op("contest", resource="oven", challenger="niece", trust=50), op("assess", being="baker")))

    add("contest_stand_queues_and_resents",
        "A low-status, distrusted challenger scores 100,000: the holder stands; the challenger queues and resents it (+220,000).",
        wf(op("claim", resource="bench", being="elder"),
           op("contest", resource="bench", challenger="youth", trust=-50, status=20, holder_status=70, urgency=40,
              holder_urgency=40, at=3), op("grudges", being="youth")))

    add("contest_turns_jumps_queue",
        "An even contest offers turns: the challenger moves ahead of an earlier, more urgent claimant.",
        wf(op("claim", resource="well", being="ann"), op("claim", resource="well", being="bo", urgency=100, status=100),
           op("contest", resource="well", challenger="cy", trust=10), op("free", resource="well", being="ann")))

    add("contest_grudge_hardens_holder",
        "The lord scores 700,000 and the smith yields resentfully (+200,000); the same claim later costs half the grudge, scores 600,000 and only wins turns.",
        wf(op("claim", resource="forge", being="smith"),
           op("contest", resource="forge", challenger="lord", trust=0, status=80, holder_status=40),
           op("free", resource="forge", being="lord"), op("free", resource="forge", being="smith"),
           op("claim", resource="forge", being="smith"),
           op("contest", resource="forge", challenger="lord", trust=0, status=80, holder_status=40)))

    add("grudge_book_evicts_weakest",
        "With grudges limited to 2 per being, a third grudge evicts the weakest (equal weights: the first by name).",
        wf(op("promise", id="a", **{"from": "smith"}, to="miller", what="x", due=0),
           op("promise", id="b", **{"from": "carter"}, to="miller", what="y", due=0), op("tick", now=1),
           op("claim", resource="mill", being="baker"),
           op("contest", resource="mill", challenger="miller", trust=-80), op("grudges", being="miller"),
           limits={"grudges": 2}))

    # ---- host limits
    add("limits_echo_and_task_cap",
        "Limits are echoed; with tasks 1 and roles 2, one two-role task forms (a second would be refused).",
        wf(form("a", "farmer", "reap", "cart"), op("assess", being="farmer"), limits={"tasks": 1, "roles": 2}))

    # ---- malformed and boundary
    add("bad_trust_out_of_range",
        "Trust must be −100..100; 101 is refused.",
        wf(HARVEST, ask("harvest", "cart", "farmer", "carter", 101)), error=True)

    add("bad_tick_backwards",
        "The clock is nondecreasing: tick 5 after tick 9 is refused.",
        wf(op("tick", now=9), op("tick", now=5)), error=True)

    add("bad_assign_by_non_organizer",
        "Only the organizer assigns roles.",
        wf(HARVEST, op("assign", task="harvest", role="cart", being="carter", by="carter")), error=True)

    add("bad_queue_full",
        "With queue 1, a second waiting claimant is refused.",
        wf(op("claim", resource="well", being="ann"), op("claim", resource="well", being="bo"),
           op("claim", resource="well", being="cy"), limits={"queue": 1}), error=True)

    add("bad_new_grudge_holder_over_beings",
        "With beings 1, a second grudge holder is refused before any change.",
        wf(op("promise", id="a", **{"from": "smith"}, to="miller", what="x", due=0),
           op("promise", id="b", **{"from": "smith"}, to="baker", what="y", due=0), op("tick", now=1),
           limits={"beings": 1}), error=True)

    add("bad_promise_to_self",
        "A being cannot promise itself.",
        wf(op("promise", id="a", **{"from": "smith"}, to="smith", what="rest", due=3)), error=True)

    return cases


BEINGS = ["ann", "bo", "cy", "dee"]
TASKS = ["t1", "t2"]
ROLES = ["r1", "r2", "r3"]
RESOURCES = ["well", "oven"]


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {n: rng.randint(1, 4) for n in rng.sample(LIMIT_NAMES, rng.randint(1, 4))}
    if rng.random() < 0.02:
        lim[rng.choice(LIMIT_NAMES)] = 0
    return lim


def accepts(ops: List[Dict[str, Any]], limits: Optional[Dict[str, int]]) -> bool:
    try:
        py_workflow(wf(*ops, limits=limits))
        return True
    except Exception:  # noqa: BLE001 - any refusal
        return False


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op, so most run deep into tasks, queues and grudges and some still
    test refusals."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    organizers: Dict[str, str] = {}
    ops: List[Dict[str, Any]] = []
    t = 0
    pid = 0
    target = rng.randint(1, 24)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        n = len(ops)
        b, o = rng.sample(BEINGS, 2)
        task, role, res = rng.choice(TASKS), rng.choice(ROLES), rng.choice(RESOURCES)
        t += rng.randint(0, 40)
        lvl = lambda: rng.choice([None, rng.randint(0, 100)])  # noqa: E731
        roll = rng.random()
        if task in organizers and rng.random() < 0.6:
            b = organizers[task]
            o = rng.choice([x for x in BEINGS if x != b])
        if roll < 0.08:
            roles = [{"role": r} if rng.random() < 0.5 else {"role": r, "being": rng.choice(BEINGS)}
                     for r in rng.sample(ROLES, rng.randint(1, 3))]
            ops.append(op("form", id=task, organizer=b, goal="Work", roles=roles))
        elif roll < 0.14:
            ops.append(op("assign", task=task, role=role, being=b, by=o))
        elif roll < 0.28:
            args = {"task": task, "role": role, "asker": b, "helper": o, "trust": rng.randint(-40, 100)}
            if rng.random() < 0.5:
                args["busy"] = rng.random() < 0.5
            if (e := lvl()) is not None:
                args["effort"] = e
            ops.append(op("ask_help", **args))
        elif roll < 0.32:
            ops.append(op("hand_off", task=task, role=role, **{"from": b}, to=o))
        elif roll < 0.40:
            ops.append(op("complete", task=task, role=role, being=b, at=t))
        elif roll < 0.44:
            ops.append(op("leave", task=task, role=role, being=b, at=t))
        elif roll < 0.52:
            pid += 1
            ops.append(op("promise", id=f"p{rng.randint(1, pid)}", **{"from": b}, to=o, what="favor",
                          due=t + rng.randint(0, 60), at=t))
        elif roll < 0.56:
            ops.append(op("keep", id=f"p{rng.randint(1, pid + 1)}", at=t))
        elif roll < 0.58:
            ops.append(op("release", id=f"p{rng.randint(1, pid + 1)}", by=o))
        elif roll < 0.64:
            ops.append(op("tick", now=t))
        elif roll < 0.76:
            args = {"resource": res, "being": b, "at": t}
            for k in ("urgency", "status"):
                if (v := lvl()) is not None:
                    args[k] = v
            ops.append(op("claim", **args))
        elif roll < 0.82:
            ops.append(op("free", resource=res, being=b, at=t))
        elif roll < 0.94:
            args = {"resource": res, "challenger": b, "trust": rng.randint(-100, 100), "at": t}
            for k in ("urgency", "status", "holder_urgency", "holder_status"):
                if (v := lvl()) is not None:
                    args[k] = v
            ops.append(op("contest", **args))
        elif roll < 0.97:
            ops.append(op("grudges", being=b))
        else:
            ops.append(op("assess", being=b))
        if len(ops) > n:
            if not accepts(ops, limits):
                if end_on_refusal:
                    break
                ops.pop()
            elif ops[-1]["op"] == "form":
                organizers[task] = b
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
    ok_runs = err_runs = helped = contests = grudges = 0
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
        helped += proc.stdout.count('"accepted":true')
        contests += proc.stdout.count('"outcome":')
        grudges += proc.stdout.count('"kind":"resentment"')
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "help_accepted": helped, "contests_resolved": contests, "grudges_proposed": grudges,
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
        sys.exit("cooperation checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
