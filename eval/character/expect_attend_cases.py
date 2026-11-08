#!/usr/bin/env python3
"""Conformance cases and differential parity runner for expectations and surprise (pass 2
A3, local kind `expectation`) and attention and graded noticing (pass 2 A2, local kind
`attention`).

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/expect_attend/*/case.json + input.json
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
from expect_attend_reference import process_local

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "expect_attend"


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    return process_local(request.get("kind"), request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def ex(*ops: Dict[str, Any]) -> Dict[str, Any]:
    return {"kind": "expectation", "args": {"ops": list(ops)}}


def at(*ops: Dict[str, Any]) -> Dict[str, Any]:
    return {"kind": "attention", "args": {"ops": list(ops)}}


FLOUR = op("expect", being="baker", id="flour", about="player", event="bring_flour", due=12, grace=2,
           importance=80, relies=True, label="the player brings flour by noon")


def ev(eid: str, kind: str, place: str, salience: int, **kw: Any) -> Dict[str, Any]:
    return {"id": eid, "kind": kind, "place": place, "salience": salience, **kw}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    # ---- expectations (A3)
    add("exp_expect_defaults_and_overrides",
        "Defaults: a hope, no grace, confidence 70 for an unknown subject, label '<about> <event>'; explicit values override.",
        ex(op("expect", being="miller", id="cart", about="carter", event="arrive", due=30, importance=40),
           op("expect", being="miller", id="rain", about="sky", event="rain", due=40, importance=100, grace=5,
              kind="dread", confidence=100, label="storm over the mill"),
           op("assess", being="miller")))

    add("exp_worry_then_disappointment_and_resentment",
        "A relied-on promise: past due the baker worries (once), past grace it breaks with a mood drop and resentment toward the player.",
        ex(FLOUR, op("tick", at=12), op("tick", at=13), op("tick", at=14), op("tick", at=15), op("tick", at=16),
           op("assess", being="baker")))

    add("exp_kept_on_time_raises_confidence",
        "A kept promise (early) lifts the mood a little and the next expectation about the same subject starts at confidence 80.",
        ex(FLOUR, op("happen", about="player", event="bring_flour", at=11),
           op("expect", being="baker", id="flour_again", about="player", event="bring_flour", due=36, importance=80),
           op("assess", being="baker")))

    add("exp_late_arrival_brings_relief",
        "A broken hope that finally happens is resolved as late, with relief; the broken count stays.",
        ex(FLOUR, op("tick", at=20), op("happen", about="player", event="bring_flour", at=22),
           op("assess", being="baker")))

    add("exp_dread_met",
        "A dreaded visit that comes darkens the mood by half its importance.",
        ex(op("expect", being="miller", id="tax", about="collector", event="arrive", due=50, grace=10,
              importance=60, kind="dread"),
           op("tick", at=55), op("happen", about="collector", event="arrive", at=56), op("assess", being="miller")))

    add("exp_dread_relieved",
        "A dread that never comes brings relief (a quarter of the surprise) and counts as a miss for the subject.",
        ex(op("expect", being="miller", id="tax", about="collector", event="arrive", due=50, grace=10,
              importance=60, kind="dread"),
           op("tick", at=60), op("tick", at=61), op("assess", being="miller")))

    add("exp_no_grace_breaks_directly",
        "With no grace period there is no worry: a hope goes from open to broken one tick after it is due.",
        ex(op("expect", being="apprentice", id="lesson", about="baker", event="teach", due=8, importance=30),
           op("tick", at=8), op("tick", at=9)))

    add("exp_reliability_floor",
        "Five broken promises push default confidence to its floor of 10.",
        ex(*[x for i in range(5) for x in (
            op("expect", being="baker", id=f"p{i}", about="drifter", event="repay", due=10 * i, importance=50,
               relies=True),
            op("tick", at=10 * i + 1))],
           op("expect", being="baker", id="last", about="drifter", event="repay", due=100, importance=50),
           op("assess", being="baker")))

    add("exp_happen_matches_all_beings_and_cancel",
        "Without a being, an event resolves every matching expectation; with one, only that being's. Cancel removes one.",
        ex(op("expect", being="baker", id="visit", about="player", event="arrive", due=20, importance=50),
           op("expect", being="apprentice", id="visit", about="player", event="arrive", due=25, importance=20),
           op("expect", being="miller", id="visit", about="player", event="arrive", due=25, importance=20),
           op("cancel", being="miller", id="visit"), op("cancel", being="miller", id="visit"),
           op("cancel", being="stranger", id="visit"),
           op("happen", about="player", event="arrive", at=21, being="apprentice"),
           op("happen", about="player", event="arrive", at=22),
           op("happen", about="player", event="leave", at=23)))

    add("exp_full_book_evicts_oldest_broken",
        "A full book (32) makes room by dropping the broken expectation that fell due first.",
        ex(*[op("expect", being="clerk", id=f"e{i:02d}", about="ledger", event="balance", due=100 + i, importance=5)
             for i in range(32)],
           op("tick", at=102),
           op("expect", being="clerk", id="new", about="ledger", event="balance", due=500, importance=5)))

    add("exp_malformed_kind",
        "kind must be hope or dread.",
        ex(op("expect", being="baker", id="x", about="player", event="arrive", due=1, importance=5, kind="maybe")),
        error=True)

    add("exp_malformed_full_book_nothing_broken",
        "A 33rd expectation is refused when none of the 32 has broken.",
        ex(*[op("expect", being="clerk", id=f"e{i:02d}", about="ledger", event="balance", due=100, importance=5)
             for i in range(33)]), error=True)

    add("exp_malformed_tick_unknown_being",
        "tick for a named being needs that being to hold expectations.",
        ex(FLOUR, op("tick", at=5, being="ghost")), error=True)

    # ---- attention (A2)
    add("att_profiles_and_defaults",
        "An unprofiled being has alertness 50, perception 0, capacity 3; profile sets and sorts cares.",
        at(op("assess", being="guard"),
           op("profile", being="guard", alertness=90, perception=40, cares=["thief", "fire", "alarm"], capacity=5),
           op("assess", being="guard")))

    add("att_crash_on_the_road",
        "A loud crash is noticed on the road and from the nearby bakery, but not from the far mill.",
        at(op("notice", seed=7, at=100,
              events=[ev("crash", "accident", "road", 90, noise=80, subject="carter")],
              beings=[{"id": "baker", "place": "road"},
                      {"id": "apprentice", "place": "bakery", "near": [{"place": "road", "distance": 20}]},
                      {"id": "miller", "place": "mill"}])))

    add("att_light_matters",
        "A silent figure is easy to see by day and invisible in the dark.",
        at(op("notice", seed=11,
              events=[ev("figure", "stranger", "lane", 70)],
              beings=[{"id": "day_watch", "place": "lane", "light": 100},
                      {"id": "dusk_watch", "place": "lane", "light": 40},
                      {"id": "night_watch", "place": "lane", "light": 0}])))

    add("att_distance_attenuates",
        "The same shout loses a point of score per five units of distance.",
        at(op("notice", seed=21,
              events=[ev("shout", "voice", "square", 10, noise=70)],
              beings=[{"id": f"listener{d}", "place": "lane", "near": [{"place": "square", "distance": d}]}
                      for d in (1, 50, 150, 400)] + [{"id": "listener0", "place": "square"}])))

    add("att_busy_overlooks",
        "The same quiet event is scored lower for a busy being.",
        at(op("notice", seed=3,
              events=[ev("pickpocket", "theft", "market", 30, noise=5)],
              beings=[{"id": "idle", "place": "market"}, {"id": "haggler", "place": "market", "busy": 90}])))

    add("att_cares_boosts",
        "Caring about an event's kind or subject adds 25.",
        at(op("profile", being="mother", cares=["child"]), op("profile", being="smith", cares=["fire"]),
           op("notice", seed=3,
              events=[ev("cry", "voice", "square", 20, noise=30, subject="child"), ev("sparks", "fire", "square", 20)],
              beings=[{"id": "mother", "place": "square"}, {"id": "smith", "place": "square"},
                      {"id": "passerby", "place": "square"}])))

    add("att_capacity_limits",
        "A being notices at most its capacity (highest scores first); the rest are over capacity.",
        at(op("profile", being="clerk", alertness=100, perception=100, capacity=2),
           op("notice", seed=9,
              events=[ev(f"stall{i}", "stall", "market", 50 + 10 * i, noise=10) for i in range(5)],
              beings=[{"id": "clerk", "place": "market"}])))

    add("att_asleep_hears_only_loud",
        "Asleep, only sound counts and the score is halved; a deaf sleeper perceives nothing.",
        at(op("notice", seed=13,
              events=[ev("bell", "alarm", "square", 100, noise=100), ev("lantern", "light", "square", 100)],
              beings=[{"id": "sleeper", "place": "square", "asleep": True},
                      {"id": "deaf_sleeper", "place": "square", "asleep": True, "hearing": False}])))

    add("att_seed_changes_rolls",
        "The host seed decides borderline rolls; the same seed replays the same result.",
        at(*[op("notice", seed=s, events=[ev("rustle", "sound", "woods", 0, noise=50)],
                beings=[{"id": f"hunter{i}", "place": "woods"} for i in range(6)]) for s in (1, 2, 1)]))

    add("att_out_of_range_and_blind",
        "Events at places a being is neither at nor near are out of range; a blind being cannot perceive a silent wave.",
        at(op("notice", seed=17,
              events=[ev("wave", "gesture", "harbor", 80), ev("shout", "voice", "harbor", 10, noise=60),
                      ev("splash", "sound", "pier", 10, noise=90)],
              beings=[{"id": "fisher", "place": "harbor", "sight": False},
                      {"id": "keeper", "place": "lighthouse", "near": [{"place": "harbor", "distance": 200}]}])))

    add("att_malformed_duplicate_event",
        "Event ids must be distinct.",
        at(op("notice", seed=1, events=[ev("a", "x", "p", 10), ev("a", "x", "p", 10)],
              beings=[{"id": "b", "place": "p"}])), error=True)

    add("att_malformed_near_own_place",
        "A being's own place cannot be listed as near.",
        at(op("notice", seed=1, events=[ev("a", "x", "p", 10)],
              beings=[{"id": "b", "place": "p", "near": [{"place": "p", "distance": 5}]}])), error=True)
    return cases


SUBJECTS = ["player", "carter", "collector", "sky", "baker"]
EVENTS = ["arrive", "repay", "rain", "bring_flour"]
BEINGS = ["baker", "miller", "apprentice", "guard"]


def rand_expect(rng: random.Random) -> Dict[str, Any]:
    ops: List[Dict[str, Any]] = []
    t, known, ids = 0, [], {}
    for _ in range(rng.randint(1, 14)):
        roll = rng.random()
        b = rng.choice(BEINGS)
        if roll < 0.4 or not known:
            e: Dict[str, Any] = {"being": b, "id": f"x{rng.randint(0, 6)}", "about": rng.choice(SUBJECTS),
                                 "event": rng.choice(EVENTS), "due": t + rng.randint(0, 20),
                                 "importance": rng.randint(1, 100)}
            if rng.random() < 0.5:
                e["grace"] = rng.randint(0, 6)
            if rng.random() < 0.4:
                e["kind"] = rng.choice(["hope", "dread"]) if rng.random() > 0.01 else "maybe"
            if rng.random() < 0.4:
                e["relies"] = rng.random() < 0.6
            if rng.random() < 0.2:
                e["confidence"] = rng.randint(1, 100)
            if rng.random() < 0.2:
                e["label"] = "something expected"
            ops.append(op("expect", **e))
            known.append(b)
            ids.setdefault(b, []).append(e["id"])
        elif roll < 0.7:
            t += rng.randint(0, 8)
            a: Dict[str, Any] = {"at": t}
            if rng.random() < 0.2:
                a["being"] = rng.choice(known) if rng.random() < 0.98 else "ghost"
            ops.append(op("tick", **a))
        elif roll < 0.88:
            h: Dict[str, Any] = {"about": rng.choice(SUBJECTS), "event": rng.choice(EVENTS), "at": t + rng.randint(0, 3)}
            if rng.random() < 0.2:
                h["being"] = rng.choice(known)
            ops.append(op("happen", **h))
        elif roll < 0.93:
            ops.append(op("cancel", being=b, id=rng.choice(ids.get(b, ["x0"]))))
        else:
            ops.append(op("assess", being=rng.choice(known) if rng.random() < 0.98 else "ghost"))
    return ex(*ops)


KINDS = ["accident", "theft", "voice", "fire", "stall"]
PLACES = ["road", "market", "bakery", "mill"]


def rand_attend(rng: random.Random) -> Dict[str, Any]:
    ops: List[Dict[str, Any]] = []
    for _ in range(rng.randint(1, 5)):
        if rng.random() < 0.3:
            p: Dict[str, Any] = {"being": rng.choice(BEINGS)}
            for k, lo, hi in [("alertness", 0, 100), ("perception", 0, 100), ("capacity", 1, 16)]:
                if rng.random() < 0.6:
                    p[k] = rng.randint(lo, hi)
            if rng.random() < 0.5:
                p["cares"] = rng.sample(KINDS + SUBJECTS, rng.randint(0, 3))
            ops.append(op("profile", **p))
        elif rng.random() < 0.9:
            events = []
            for i in range(rng.randint(1, 6)):
                e = ev(f"e{i}", rng.choice(KINDS), rng.choice(PLACES), rng.randint(0, 100))
                if rng.random() < 0.6:
                    e["noise"] = rng.randint(0, 100)
                if rng.random() < 0.3:
                    e["subject"] = rng.choice(SUBJECTS)
                events.append(e)
            if rng.random() < 0.01:
                events.append(dict(events[0]))
            beings = []
            for name in rng.sample(BEINGS, rng.randint(1, 4)):
                place = rng.choice(PLACES)
                b: Dict[str, Any] = {"id": name, "place": place}
                others = [x for x in PLACES if x != place]
                if rng.random() < 0.6:
                    b["near"] = [{"place": x, "distance": rng.randint(1, 300)}
                                 for x in rng.sample(others, rng.randint(1, 3))]
                for k in ("light", "busy"):
                    if rng.random() < 0.5:
                        b[k] = rng.randint(0, 100)
                for k, pr in (("asleep", 0.2), ("sight", 0.15), ("hearing", 0.15)):
                    if rng.random() < pr:
                        b[k] = rng.random() < 0.5
                beings.append(b)
            n: Dict[str, Any] = {"seed": rng.randint(0, 2 ** 40), "events": events, "beings": beings}
            if rng.random() < 0.5:
                n["at"] = rng.randint(0, 10 ** 6)
            ops.append(op("notice", **n))
        else:
            ops.append(op("assess", being=rng.choice(BEINGS)))
    return at(*ops)


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    return rand_expect(rng) if rng.random() < 0.5 else rand_attend(rng)


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
    ok_runs = err_runs = surprises = notices = 0
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
            surprises += len(r.get("changes", [])) + len(r.get("resolved", []))
            notices += sum(len(b["noticed"]) for b in r.get("beings", []))
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "expectation_changes": surprises, "notices": notices, "actual_model_calls": 0, "api_spend_usd": 0,
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
        sys.exit("expect_attend checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
