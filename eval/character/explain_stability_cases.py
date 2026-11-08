#!/usr/bin/env python3
"""Conformance cases and differential parity runner for the behavior explanation trace
(pass 2, D4, local kind `explain`) and stability monitors (pass 2, D1, local kind
`stability`).

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/explain_stability/*/case.json + input.json
  --random N --seed S     N randomized workflows, Python reference vs `gamectx local -`

All checks are offline: zero model calls, $0 API spend.
"""

import argparse
import json
import pathlib
import random
import subprocess
import sys
from typing import Any, Dict, List

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
from explain_stability_reference import process_local

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "explain_stability"


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    return process_local(request.get("kind"), request.get("args", {}))


def ex(*ops: Dict[str, Any]) -> Dict[str, Any]:
    return {"kind": "explain", "args": {"ops": list(ops)}}


def st(*ops: Dict[str, Any]) -> Dict[str, Any]:
    return {"kind": "stability", "args": {"ops": list(ops)}}


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def motive(mid: str, kind: str, score: int, reason: str, **extra: Any) -> Dict[str, Any]:
    m = {"id": mid, "kind": kind, "score": score, "reason": reason}
    m.update(extra)
    return m


def being(name: str, mot: str, activity: str, **extra: Any) -> Dict[str, Any]:
    b = {"being": name, "motive": mot, "activity": activity}
    b.update(extra)
    return b


HUNGER = motive("need_hunger", "need", 80, "hunger 80 is past threshold 70",
                runner_up={"id": "task_bake_bread", "score": 70})
BAKE = motive("routine_bake", "routine", 50, "the dawn bake shift is active")


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    # ---- explain (D4)
    add("explain_record_minimal",
        "Record a trace with only being, action and motive; query renders a pending chain.",
        ex(op("record_trace", being="baker", action="bake", motive=BAKE),
           op("query_trace", being="baker")))

    add("explain_record_full_chain",
        "Full perception -> belief -> motive -> task step -> action -> outcome chain with a target.",
        ex(op("record_trace", being="baker", action="barter", target="miller", at=9,
              perceptions=[{"source": "pantry", "text": "the flour bin is empty"}],
              beliefs=[{"about": "miller", "fact": "the mill sells flour", "source": "told", "confidence": 70}],
              motive=motive("task_bake_bread", "task", 70, "bread is promised for the market",
                            rank=1, runner_up={"id": "routine_bake", "score": 50}),
              task={"id": "bake_bread_task", "step": "barter_flour", "index": 1},
              outcome={"status": "success", "summary": "traded two loaves for a sack of flour"}),
           op("query_trace", being="baker")))

    add("explain_query_newest_first",
        "Three traces; query with limit 2 returns the two newest, newest first.",
        ex(op("record_trace", being="baker", action="bake", at=1, motive=BAKE),
           op("record_trace", being="baker", action="eat", at=2, motive=HUNGER),
           op("record_trace", being="baker", action="rest", at=3,
              motive=motive("need_rest", "need", 75, "fatigue 75 after the long walk")),
           op("query_trace", being="baker", limit=2)))

    add("explain_query_by_action",
        "Filter history by action name.",
        ex(op("record_trace", being="baker", action="bake", at=1, motive=BAKE),
           op("record_trace", being="baker", action="eat", at=2, motive=HUNGER),
           op("record_trace", being="baker", action="bake", at=3, motive=BAKE),
           op("query_trace", being="baker", action="bake")))

    add("explain_query_by_id",
        "Explain one specific trace by id; an unmatched id yields an empty list.",
        ex(op("record_trace", being="baker", action="bake", motive=BAKE),
           op("record_trace", being="baker", action="eat", motive=HUNGER),
           op("query_trace", being="baker", id="tr_1"),
           op("query_trace", being="baker", id="tr_99")))

    add("explain_confirm_outcome",
        "A pending trace is confirmed later by the host; the chain and why sentence follow.",
        ex(op("record_trace", being="apprentice", action="sweep", at=18,
              perceptions=[{"source": "baker", "text": "the baker asked for a clean floor"}],
              motive=motive("routine_chores", "routine", 40, "the chores slot is active")),
           op("confirm_outcome", being="apprentice", id="tr_1", status="failure", summary="the broom snapped"),
           op("query_trace", being="apprentice")))

    add("explain_belief_sources",
        "Beliefs carry source and confidence (seen/told/inferred/authored; default 100).",
        ex(op("record_trace", being="guard", action="warn", target="innkeeper",
              perceptions=[{"source": "street", "text": "a hooded figure left the inn"}],
              beliefs=[{"about": "thief", "fact": "the thief took the silver", "source": "seen"},
                       {"about": "innkeeper", "fact": "she did not see the theft", "source": "inferred",
                        "confidence": 85},
                       {"about": "thief", "fact": "thieves return at night", "source": "authored",
                        "confidence": 40}],
              motive=motive("goal_protect", "goal", 65, "the guard protects the innkeeper")),
           op("query_trace", being="guard")))

    add("explain_init_clear_being",
        "init with a being clears only that being; init without one clears everyone.",
        ex(op("record_trace", being="baker", action="bake", motive=BAKE),
           op("record_trace", being="miller", action="mill",
              motive=motive("routine_mill", "routine", 45, "the mill runs at dawn")),
           op("init", being="baker"),
           op("query_trace", being="miller"),
           op("init")))

    add("malformed_explain_unknown_motive_kind",
        "Reject a motive kind the arbiter does not define.",
        ex(op("record_trace", being="baker", action="bake",
              motive=motive("whim_dance", "whim", 50, "felt like it"))), error=True)

    add("malformed_explain_confirm_twice",
        "Reject confirming an outcome that is no longer pending.",
        ex(op("record_trace", being="baker", action="bake", motive=BAKE,
              outcome={"status": "success"}),
           op("confirm_outcome", being="baker", id="tr_1", status="failure")), error=True)

    add("malformed_explain_query_unknown_being",
        "Reject a query for a being with no recorded history.",
        ex(op("query_trace", being="ghost")), error=True)

    # ---- stability (D1)
    add("stability_init_custom",
        "init replaces the thresholds and returns the full config.",
        st(op("init", window=6, dither_switches=2, dither_margin=5, critical=90, herd_pct=60),
           op("assess_health")))

    flip = [("bake", 60, 55), ("eat", 58, 54), ("bake", 61, 57), ("eat", 59, 56)]
    add("stability_dithering_detected",
        "Switching between two motives with margins <= 10 three times flags dithering.",
        st(*[op("audit_turn", turn=t + 1, beings=[being("baker", m, m, score=s, runner_up=r)])
             for t, (m, s, r) in enumerate(flip)]))

    add("stability_dithering_wide_margin_ok",
        "The same switching with wide margins is deliberate, not dithering.",
        st(*[op("audit_turn", turn=t + 1, beings=[being("baker", m, m, score=s, runner_up=r - 30)])
             for t, (m, s, r) in enumerate(flip)]))

    def tired(t: int, level: int, recovering: bool = False) -> Dict[str, Any]:
        return op("audit_turn", turn=t, beings=[being(
            "porter", "routine_haul", "haul", score=40, fatigue=level, recovering=recovering,
            needs=[{"id": "hunger", "level": level - 2}, {"id": "thirst", "level": 30}])])

    add("stability_starvation_spiral",
        "Fatigue stays critical and worsens for three turns without recovery.",
        st(tired(1, 82), tired(2, 85), tired(3, 85), tired(4, 90), op("assess_health")))

    add("stability_starvation_recovering_ok",
        "A being trying to recover resets the spiral streak.",
        st(tired(1, 82), tired(2, 85), tired(3, 88, recovering=True), tired(4, 90), op("assess_health")))

    add("stability_deadlock_cycle",
        "Three beings in a wait-for cycle plus a bystander waiting on the cycle; four idle waiters also trip herding.",
        st(op("audit_turn", turn=1, beings=[
            being("baker", "task_bake", "wait", holds=["oven"], wants=["flour"]),
            being("miller", "task_mill", "wait", holds=["flour"], wants=["cart"]),
            being("carter", "task_haul", "wait", holds=["cart"], wants=["oven"]),
            being("apprentice", "task_sweep", "wait", wants=["oven", "broom"]),
            being("smith", "routine_forge", "forge", holds=["anvil"]),
        ])))

    add("stability_herding",
        "Four of five beings pick the same activity (default herd_pct 80).",
        st(op("audit_turn", turn=1, beings=[
            being("baker", "routine_festival", "festival"),
            being("miller", "routine_festival", "festival"),
            being("guard", "goal_patrol", "patrol"),
            being("smith", "routine_festival", "festival"),
            being("carter", "routine_festival", "festival"),
        ])))

    add("stability_grudge_and_rumor_runaway",
        "World grudge total rises three turns running past the floor; rumors plateau.",
        st(op("audit_turn", turn=1, beings=[], world={"grudge_total": 90, "rumor_total": 40}),
           op("audit_turn", turn=2, beings=[], world={"grudge_total": 110, "rumor_total": 60}),
           op("audit_turn", turn=3, beings=[], world={"grudge_total": 140, "rumor_total": 60}),
           op("audit_turn", turn=4, beings=[], world={"grudge_total": 180, "rumor_total": 70})))

    add("stability_health_unstable_then_recovered",
        "Health is unstable on a warning turn and recovered once a quiet turn follows.",
        st(op("init", herd_min=2, herd_pct=100),
           op("assess_health"),
           op("audit_turn", turn=10, beings=[being("baker", "m", "market"), being("smith", "m", "market")]),
           op("assess_health"),
           op("audit_turn", turn=11, beings=[being("baker", "m", "bake"), being("smith", "m", "forge")]),
           op("assess_health")))

    add("stability_get_warnings_filtered",
        "Warnings filter by kind, being and turn; limit keeps the most recent.",
        st(op("init", herd_min=2, herd_pct=100, spiral_turns=1),
           op("audit_turn", turn=1, beings=[being("baker", "m", "market", fatigue=95),
                                            being("smith", "m", "market")]),
           op("audit_turn", turn=2, beings=[being("baker", "m", "market", fatigue=96),
                                            being("smith", "m", "forge")]),
           op("get_warnings", kind="herding"),
           op("get_warnings", being="baker", limit=1),
           op("get_warnings", since_turn=2)))

    add("stability_audit_defaults_quiet",
        "A calm turn with defaults reports nothing; null optionals are treated as absent.",
        st(op("audit_turn", turn=0, beings=[
            being("baker", "routine_bake", "bake", score=50, runner_up=None, needs=None),
            being("miller", "routine_mill", "mill", score=45, runner_up=20, fatigue=30),
        ], world=None), op("assess_health"), op("get_warnings")))

    add("malformed_stability_turn_not_increasing",
        "Reject an audit whose turn does not advance.",
        st(op("audit_turn", turn=5, beings=[]), op("audit_turn", turn=5, beings=[])), error=True)

    add("malformed_stability_duplicate_being",
        "Reject an audit that reports the same being twice.",
        st(op("audit_turn", turn=1, beings=[being("baker", "m", "bake"), being("baker", "m", "eat")])),
        error=True)

    add("malformed_stability_bad_config",
        "Reject dither_switches that can never fit in the window.",
        st(op("init", window=4, dither_switches=4)), error=True)

    return cases


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


BEINGS = ["baker", "miller", "smith", "guard", "carter", "apprentice"]
ACTIONS = ["bake", "eat", "rest", "mill", "barter", "patrol", "sweep", "haul"]
MOTIVES = ["need_hunger", "need_rest", "routine_bake", "task_bread", "goal_store", "promise_flour"]
TEXTS = ["the bin is empty", "smoke over the mill", "a stranger at the gate", "café is open",
         "bread smells done", "the cart lost a wheel", "x" * 160]
RESOURCES = ["oven", "flour", "cart", "anvil", "broom", "well"]
ACTIVITIES = ["bake", "market", "festival", "patrol", "rest"]


def rand_explain(rng: random.Random) -> Dict[str, Any]:
    ops = []
    recorded = 0
    known: List[str] = []
    for _ in range(rng.randint(1, 6)):
        roll = rng.random() if known else 0.0
        b = rng.choice(known) if known and roll >= 0.5 and rng.random() < 0.95 else rng.choice(BEINGS[:3])
        if roll < 0.5:
            args: Dict[str, Any] = {
                "being": b,
                "action": rng.choice(ACTIONS),
                "motive": {
                    "id": rng.choice(MOTIVES),
                    "kind": rng.choice(["need", "routine", "task", "goal", "promise", "emergency", "reaction"]),
                    "score": rng.randint(0, 100),
                    "reason": rng.choice(TEXTS),
                },
            }
            if rng.random() < 0.5:
                args["motive"]["runner_up"] = {"id": rng.choice(MOTIVES), "score": rng.randint(0, 100)}
            if rng.random() < 0.3:
                args["motive"]["rank"] = rng.randint(1, 5)
            if rng.random() < 0.4:
                args["target"] = rng.choice(BEINGS)
            if rng.random() < 0.6:
                args["at"] = rng.randint(0, 5000)
            if rng.random() < 0.6:
                args["perceptions"] = [{"source": rng.choice(["body", "street", "pantry"]), "text": rng.choice(TEXTS)}
                                       for _ in range(rng.randint(0, 3))]
            if rng.random() < 0.5:
                args["beliefs"] = []
                for _ in range(rng.randint(0, 3)):
                    bel = {"about": rng.choice(BEINGS), "fact": rng.choice(TEXTS),
                           "source": rng.choice(["seen", "told", "inferred", "authored"])}
                    if rng.random() < 0.5:
                        bel["confidence"] = rng.randint(0, 100)
                    args["beliefs"].append(bel)
            if rng.random() < 0.4:
                args["task"] = {"id": "bake_bread_task", "step": rng.choice(["fetch_flour", "knead", "bake"]),
                                "index": rng.randint(0, 5)}
            if rng.random() < 0.4:
                out = {"status": rng.choice(["pending", "success", "failure", "interrupted"])}
                if rng.random() < 0.5:
                    out["summary"] = rng.choice(TEXTS)
                args["outcome"] = out
            if rng.random() < 0.04:  # inject an invalid field
                args["motive"]["score"] = rng.choice([101, -1, "50", 5.5])
            ops.append(op("record_trace", **args))
            recorded += 1
            if b not in known:
                known.append(b)
        elif roll < 0.6:
            args = {"being": b, "id": f"tr_{rng.randint(1, max(1, recorded + 1))}",
                    "status": rng.choice(["success", "failure", "interrupted"])}
            if rng.random() < 0.5:
                args["summary"] = rng.choice(TEXTS)
            ops.append(op("confirm_outcome", **args))
        elif roll < 0.92:
            args = {"being": b}
            if rng.random() < 0.3:
                args["action"] = rng.choice(ACTIONS)
            if rng.random() < 0.2:
                args["id"] = f"tr_{rng.randint(1, 6)}"
            if rng.random() < 0.5:
                args["limit"] = rng.randint(1, 10)
            ops.append(op("query_trace", **args))
        else:
            ops.append(op("init", **({"being": b} if rng.random() < 0.6 else {})))
    return ex(*ops)


def rand_stability(rng: random.Random) -> Dict[str, Any]:
    ops = []
    if rng.random() < 0.6:
        cfg: Dict[str, Any] = {}
        for k, lo, hi in [("window", 2, 10), ("dither_switches", 1, 4), ("dither_margin", 0, 30),
                          ("critical", 50, 95), ("spiral_turns", 1, 4), ("herd_min", 2, 4),
                          ("herd_pct", 51, 100), ("runaway_turns", 1, 4), ("runaway_floor", 0, 200)]:
            if rng.random() < 0.5:
                cfg[k] = rng.randint(lo, hi)
        ops.append(op("init", **cfg))
    turn = rng.randint(0, 3)
    pool = rng.sample(BEINGS, rng.randint(2, 5))
    for _ in range(rng.randint(1, 10)):
        roll = rng.random()
        if roll < 0.7:
            beings = []
            for name in rng.sample(pool, rng.randint(0, len(pool))):
                b: Dict[str, Any] = {"being": name, "motive": rng.choice(MOTIVES[:3]),
                                     "activity": rng.choice(ACTIVITIES)}
                if rng.random() < 0.8:
                    b["score"] = rng.randint(30, 70)
                if rng.random() < 0.7:
                    b["runner_up"] = rng.randint(25, 70)
                if rng.random() < 0.6:
                    b["fatigue"] = rng.randint(60, 100)
                if rng.random() < 0.6:
                    ids = rng.sample(["hunger", "thirst", "warmth"], rng.randint(0, 3))
                    b["needs"] = [{"id": i, "level": rng.randint(50, 100)} for i in ids]
                if rng.random() < 0.4:
                    b["recovering"] = rng.random() < 0.3
                if rng.random() < 0.5:
                    b["holds"] = rng.sample(RESOURCES, rng.randint(0, 2))
                if rng.random() < 0.5:
                    b["wants"] = rng.sample(RESOURCES, rng.randint(0, 2))
                beings.append(b)
            if rng.random() < 0.02 and beings:
                beings.append(dict(beings[0]))  # duplicate being -> refused
            args: Dict[str, Any] = {"turn": turn, "beings": beings}
            if rng.random() < 0.5:
                world = {}
                if rng.random() < 0.8:
                    world["grudge_total"] = rng.randint(50, 50 + 40 * turn)
                if rng.random() < 0.6:
                    world["rumor_total"] = rng.randint(0, 30 * turn + 1)
                args["world"] = world
            ops.append(op("audit_turn", **args))
            turn += rng.choice([1, 1, 1, 2, 3]) if rng.random() > 0.02 else 0
        elif roll < 0.85:
            ops.append(op("assess_health"))
        else:
            args = {}
            if rng.random() < 0.4:
                args["kind"] = rng.choice(["deadlock", "dithering", "grudge_runaway", "herding",
                                           "rumor_runaway", "starvation_spiral"])
            if rng.random() < 0.4:
                args["being"] = rng.choice(pool)
            if rng.random() < 0.3:
                args["since_turn"] = rng.randint(0, turn)
            if rng.random() < 0.4:
                args["limit"] = rng.randint(1, 8)
            ops.append(op("get_warnings", **args))
    return st(*ops)


def run_random_parity(count: int, seed: int) -> int:
    rng = random.Random(seed)
    rust_bin = ROOT / "target" / "debug" / "gamectx"
    if not rust_bin.exists():
        subprocess.run(["cargo", "build"], cwd=ROOT, check=True)
    ok_runs = err_runs = warn_runs = 0
    for i in range(1, count + 1):
        req = rand_explain(rng) if rng.random() < 0.5 else rand_stability(rng)
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
        if '"warn"' in proc.stdout:
            warn_runs += 1
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "with_warnings": warn_runs, "actual_model_calls": 0, "api_spend_usd": 0,
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
        sys.exit("explain/stability checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
