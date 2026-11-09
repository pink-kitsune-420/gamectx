#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 3 stage 5 world schemas:
environment cycles, place rules, aftermath traces and item provenance, local kind `surroundings`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/surroundings/*/case.json + input.json
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
from surroundings_reference import ACCESS, HOW, LIGHT, PRIVACY
from surroundings_reference import workflow as surroundings_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "surroundings"
LIMIT_NAMES = ["places", "occupants", "trace_kinds", "traces", "items"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "surroundings":
        raise ValueError("unknown local kind")
    return surroundings_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "surroundings", "args": args}


PHASES = [{"id": "dawn", "start": 0, "end": 30, "light": "dim", "temp_delta": -5},
          {"id": "day", "start": 30, "end": 160, "light": "bright"},
          {"id": "dusk", "start": 160, "end": 190, "light": "dim", "temp_delta": -5},
          {"id": "night", "start": 190, "end": 240, "light": "dark", "temp_delta": -15}]
RAIN = {"id": "heavy_rain", "light_delta": -1, "noise": 25, "travel_pct": 140,
        "need_rates": {"warmth": 3, "dryness": 5}, "seek_tags": ["sheltered", "indoor"]}
ENV = op("environment", day_length=240, phases=PHASES, weather=[RAIN])
FLOUR = op("trace_kind", id="spilled_flour_trail", created_by=["crash_cart", "drop_sack"], cleared_by=["sweep", "clean"],
           visibility="public", ttl=60, implies="flour_spilled_recently",
           sight="White flour is dusted across the cobblestones.")
LOCK = op("trace_kind", id="forced_lock_marks", created_by=["pick_lock", "break_entrance"], cleared_by=["repair"],
          visibility="inspect", ttl=1000, implies="tampered_entrance", sight="Fresh scratches score the iron lockplate.")
BAKERY = op("place", id="bakery", owner="mira", access="customers_only", hours={"start": 30, "end": 180, "outside": "private"},
            privacy="semi_private", capacity=4, stations=[{"id": "oven"}, {"id": "counter", "max_users": 2}])


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("rain_dims_the_day",
         "Mid-morning in heavy rain outdoors: day light drops to dim, noise 25, travel costs 140%, warmth and dryness drain.",
         wf(ENV, op("weather", kind="heavy_rain"), op("tick", now=100), op("conditions")))
    case("indoors_ignores_rain",
         "Inside the bakery the rain changes nothing but travel; the shop is semi-private and empty.",
         wf(ENV, BAKERY, op("weather", kind="heavy_rain"), op("tick", now=100), op("conditions", place="bakery")))
    case("night_is_dark_and_cold",
         "At tick 200 of a 240-tick day it is night: dark, 15 degrees colder.",
         wf(ENV, op("tick", now=200), op("conditions"), op("tick", now=440), op("conditions")))
    case("weather_dropped_with_old_environment",
         "Redeclaring the environment without heavy rain clears the current weather.",
         wf(ENV, op("weather", kind="heavy_rain"), op("environment", day_length=10,
                                                       phases=[{"id": "all", "start": 0, "end": 10, "light": "bright"}]),
            op("conditions")))
    case("shop_private_after_hours",
         "A customer may enter by day; after hours the shop is private, so a customer trespasses while Mira may enter.",
         wf(ENV, BAKERY, op("tick", now=40), op("enter", place="bakery", being="player", customer=True),
            op("enter", place="bakery", being="stranger"), op("tick", now=200),
            op("enter", place="bakery", being="tomas", customer=True), op("enter", place="bakery", being="mira"),
            op("enter", place="bakery", being="apprentice", invited=True)))
    case("guild_members_only",
         "A members-only guild hall admits members and its owner, refuses a customer, and a restricted vault only its owner.",
         wf(op("place", id="guild", owner="master", access="members_only"),
            op("place", id="vault", owner="master", access="restricted"),
            op("enter", place="guild", being="ana", member=True), op("enter", place="guild", being="bo", customer=True),
            op("enter", place="vault", being="ana", member=True, invited=True), op("enter", place="vault", being="master")))
    case("crowding_and_earshot",
         "Three people in a semi-private room for two are crowded and can no longer discuss secrets; an echoing hall carries to everyone.",
         wf(ENV, op("place", id="parlor", privacy="semi_private", capacity=2), op("place", id="hall", privacy="echoing"),
            op("enter", place="parlor", being="a"), op("enter", place="parlor", being="b"),
            op("conditions", place="parlor"), op("enter", place="parlor", being="c"), op("conditions", place="parlor"),
            op("conditions", place="hall")))
    case("oven_queue_promotes",
         "One oven: Mira gets it, the apprentice queues first, then Bo; when Mira releases it the apprentice moves up, and Bo leaving drops him from the queue.",
         wf(ENV, BAKERY, op("tick", now=40), op("enter", place="bakery", being="mira"),
            op("enter", place="bakery", being="apprentice", customer=True), op("enter", place="bakery", being="bo", customer=True),
            op("reserve", place="bakery", station="oven", being="mira"),
            op("reserve", place="bakery", station="oven", being="apprentice"),
            op("reserve", place="bakery", station="oven", being="bo"),
            op("release", place="bakery", station="oven", being="mira"), op("leave", place="bakery", being="bo"),
            op("leave", place="bakery", being="apprentice")))
    case("flour_seen_then_swept",
         "A dropped sack leaves a flour trail anyone can see as an inferred fact (not who did it); sweeping clears it.",
         wf(op("place", id="lane", exterior=True), FLOUR, op("tick", now=3),
            op("act", action="drop_sack", place="lane"), op("tick", now=10), op("perceive", place="lane"),
            op("act", action="sweep", place="lane"), op("perceive", place="lane")))
    case("lock_marks_need_inspection",
         "Forced lock marks are hidden at a glance and found on inspection.",
         wf(op("place", id="shop"), FLOUR, LOCK, op("act", action="pick_lock", place="shop"), op("perceive", place="shop"),
            op("perceive", place="shop", inspect=True)))
    case("trace_fades",
         "The flour trail lasts 60 ticks: still there at 59, gone (and reported) at 60; the lock marks stay.",
         wf(op("place", id="shop"), FLOUR, LOCK, op("act", action="drop_sack", place="shop"),
            op("act", action="break_entrance", place="shop"), op("tick", now=59), op("perceive", place="shop"),
            op("tick", now=60), op("perceive", place="shop", inspect=True)))
    case("trace_refresh_within_limit",
         "With room for one trace, dropping a second sack in the same place refreshes the trail instead of growing.",
         wf(op("place", id="shop"), FLOUR, op("act", action="drop_sack", place="shop"), op("tick", now=30),
            op("act", action="crash_cart", place="shop"), op("tick", now=80), op("perceive", place="shop"),
            limits={"traces": 1}))
    case("loan_and_return",
         "A loaned ladle is borrowed, so its owner sees no theft; returned, it is owned again.",
         wf(op("item", id="ladle", owner="mira", maker="smith"), op("transfer", item="ladle", how="loan", to="bo"),
            op("inspect_item", item="ladle", observer="mira"), op("transfer", item="ladle", how="return", to="mira")))
    case("stolen_heirloom_pouch",
         "The apprentice takes Mira's coin pouch, which she holds dear: a theft appraisal weighing three times a plain loss.",
         wf(op("item", id="coin_pouch", owner="mira", sentimental_to=["mira"]),
            op("transfer", item="coin_pouch", how="take", to="apprentice"),
            op("inspect_item", item="coin_pouch", observer="mira"),
            op("inspect_item", item="coin_pouch", observer="player")))
    case("lost_then_found",
         "A dropped brooch is lost; a stranger who finds it holds it as found, and its owner would object.",
         wf(op("item", id="brooch", owner="ana"), op("transfer", item="brooch", how="drop"),
            op("transfer", item="brooch", how="find", to="stranger"), op("inspect_item", item="brooch", observer="ana")))
    case("fence_sale_keeps_owner",
         "A thief selling a stolen knife to a peddler does not pass ownership: still Mira's, still stolen; Mira's gift to Bo does.",
         wf(op("item", id="knife", owner="mira"), op("transfer", item="knife", how="take", to="thief"),
            op("transfer", item="knife", how="sale", to="peddler"), op("item", id="loaf", owner="mira"),
            op("transfer", item="loaf", how="gift", to="bo")))
    case("bread_spoils_and_breaks",
         "A loaf that keeps 50 ticks is spoiled at 60; a cup damaged by 80 is damaged, then broken.",
         wf(op("item", id="loaf", owner="mira", perishable=50), op("item", id="cup", condition=100),
            op("tick", now=60), op("damage", item="cup", amount=80), op("damage", item="cup", amount=30),
            op("inspect_item", item="loaf", observer="mira")))
    case("unclaimed_found_item",
         "An item with no owner is unclaimed whoever holds it, and its first giver passes ownership.",
         wf(op("item", id="shell", possessor="child"), op("transfer", item="shell", how="gift", to="mira"),
            op("inspect_item", item="shell", observer="mira")))
    case("bad_phase_gap",
         "Malformed: phases must be contiguous from 0 to the day length.",
         wf(op("environment", day_length=240, phases=[{"id": "day", "start": 0, "end": 100, "light": "bright"},
                                                      {"id": "night", "start": 120, "end": 240, "light": "dark"}])),
         error=True)
    case("bad_hours_without_environment",
         "Malformed: opening hours need a declared day.",
         wf(op("place", id="shop", hours={"start": 0, "end": 10, "outside": "private"})), error=True)
    case("bad_reserve_outsider",
         "Malformed: only someone inside the place can reserve a station.",
         wf(ENV, BAKERY, op("reserve", place="bakery", station="oven", being="bo")), error=True)
    case("bad_loan_by_borrower",
         "Malformed: only the owner holding an item can lend it.",
         wf(op("item", id="ladle", owner="mira"), op("transfer", item="ladle", how="loan", to="bo"),
            op("transfer", item="ladle", how="loan", to="ana")), error=True)
    case("bad_trace_kind_no_creator",
         "Malformed: a trace kind needs at least one action that leaves it.",
         wf(op("trace_kind", id="soot", created_by=[], visibility="public", ttl=5, implies="fire")), error=True)
    case("bad_unknown_light",
         "Malformed: light is dark, dim or bright.",
         wf(op("environment", day_length=10, phases=[{"id": "all", "start": 0, "end": 10, "light": "glowing"}])),
         error=True)
    case("bad_trace_limit",
         "Malformed: with room for one trace, a second trace in another place is refused.",
         wf(op("place", id="a"), op("place", id="b"), FLOUR, op("act", action="drop_sack", place="a"),
            op("act", action="drop_sack", place="b"), limits={"traces": 1}), error=True)
    return cases


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {n: rng.randint(1, 3) for n in rng.sample(LIMIT_NAMES, rng.randint(1, 2))}
    if rng.random() < 0.02:
        lim[rng.choice(LIMIT_NAMES)] = 0
    return lim


def accepts(ops: List[Dict[str, Any]], limits: Optional[Dict[str, int]]) -> bool:
    try:
        py_workflow(wf(*ops, limits=limits))
        return True
    except Exception:  # noqa: BLE001 - any refusal
        return False


BEINGS = ["mira", "bo", "ana", "thief"]
PLACES = ["bakery", "lane", "vault"]
ACTIONS = ["drop_sack", "sweep", "pick_lock", "repair", "crash_cart", "dance"]


def prefix(rng: random.Random) -> List[Dict[str, Any]]:
    """A valid starting world (kept only while the limits allow it) so later random ops land."""
    out = [ENV] if rng.random() < 0.7 else []
    out += [op("place", id="bakery", owner="mira", access=rng.choice(ACCESS), privacy=rng.choice(PRIVACY), capacity=2,
               stations=[{"id": "oven"}, {"id": "counter", "max_users": 2}]),
            op("place", id="lane", exterior=True, stations=[{"id": "oven"}])]
    out += [op("trace_kind", id="flour", created_by=["drop_sack", "crash_cart"], cleared_by=["sweep"],
               visibility="public", ttl=rng.choice([5, 60]), implies="flour_seen", sight="Flour everywhere."),
            op("trace_kind", id="marks", created_by=["pick_lock"], cleared_by=["repair"], visibility="inspect",
               ttl=1000, implies="tampered")]
    out += [op("item", id="pouch", owner="mira", sentimental_to=["mira"]), op("item", id="knife", owner="bo")]
    out += [op("enter", place="bakery", being=x, customer=True, member=True, invited=True) for x in BEINGS[:3]]
    out += [op("reserve", place="bakery", station="oven", being=x) for x in rng.sample(BEINGS[:3], 2)]
    return out


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    b, p = rng.choice(BEINGS), rng.choice(PLACES)
    roll = rng.random()
    if roll < 0.07:
        return op("tick", now=t)
    if roll < 0.13:
        day = rng.choice([10, 24, 240])
        cut = rng.choice([1, day // 2, day - 1, day + 1, 0])
        phases = [{"id": "a", "start": 0, "end": cut, "light": rng.choice(LIGHT + ["glow"]),
                   "temp_delta": rng.choice([-100, -5, 0, 7, 101])},
                  {"id": rng.choice(["b", "a"]), "start": cut, "end": day, "light": rng.choice(LIGHT)}]
        weather = [{"id": w, "light_delta": rng.choice([-2, -1, 0, 1, 3]), "noise": rng.choice([0, 25, 100]),
                    "travel_pct": rng.choice([50, 140, 500, 49]), "need_rates": {"warmth": rng.choice([0, 3, 101])},
                    "seek_tags": rng.sample(["indoor", "sheltered", "high"], rng.randint(0, 2))}
                   for w in rng.sample(["rain", "fog", "snow"], rng.randint(0, 2))]
        return op("environment", day_length=day, phases=phases[:rng.choice([1, 2, 2])], weather=weather)
    if roll < 0.18:
        return op("weather", kind=rng.choice(["rain", "fog", "snow", None]))
    if roll < 0.25:
        return op("conditions", **({"place": p} if rng.random() < 0.6 else {}))
    if roll < 0.33:
        a: Dict[str, Any] = {"id": p}
        for k, v in [("owner", rng.choice(BEINGS)), ("access", rng.choice(ACCESS + ["open"])),
                     ("hours", {"start": rng.choice([0, 5, 30]), "end": rng.choice([5, 20, 180]),
                                "outside": rng.choice(ACCESS)}),
                     ("privacy", rng.choice(PRIVACY)), ("capacity", rng.choice([0, 1, 2, 4])),
                     ("exterior", rng.random() < 0.5),
                     ("stations", [{"id": s, "max_users": rng.choice([1, 2, 65])}
                                   for s in rng.sample(["oven", "counter"], rng.randint(0, 2))])]:
            if rng.random() < 0.4:
                a[k] = v
        return op("place", **a)
    if roll < 0.43:
        a = {"place": p, "being": b}
        for k in ["customer", "member", "invited"]:
            if rng.random() < 0.3:
                a[k] = rng.random() < 0.5
        return op("enter", **a)
    if roll < 0.47:
        return op("leave", place=p, being=b)
    if roll < 0.53:
        return op("reserve", place=p, station=rng.choice(["oven", "counter", "well"]), being=b)
    if roll < 0.56:
        return op("release", place=p, station=rng.choice(["oven", "counter"]), being=b)
    if roll < 0.61:
        k = rng.choice(["flour", "marks", "soot"])
        return op("trace_kind", id=k, created_by=rng.sample(ACTIONS, rng.randint(0, 2)),
                  cleared_by=rng.sample(ACTIONS, rng.randint(0, 2)), visibility=rng.choice(["public", "inspect", "secret"]),
                  ttl=rng.choice([0, 1, 5, 60]), implies=k + "_seen",
                  **({"sight": rng.choice(["Flour everywhere.", ""])} if rng.random() < 0.5 else {}))
    if roll < 0.69:
        return op("act", action=rng.choice(ACTIONS), place=p)
    if roll < 0.74:
        return op("perceive", place=p, **({"inspect": rng.random() < 0.5} if rng.random() < 0.5 else {}))
    it = rng.choice(["pouch", "loaf", "knife"])
    if roll < 0.80:
        a = {"id": it}
        for k, v in [("owner", rng.choice(BEINGS)), ("possessor", rng.choice(BEINGS + [None])),
                     ("held_as", rng.choice(["borrowed", "stolen", "found", "kept"])),
                     ("condition", rng.choice([0, 29, 30, 69, 70, 94, 95, 101])), ("maker", "smith"),
                     ("sentimental_to", rng.sample(BEINGS, rng.randint(0, 2))), ("perishable", rng.choice([0, 1, 10]))]:
            if rng.random() < 0.4:
                a[k] = v
        return op("item", **a)
    if roll < 0.90:
        return op("transfer", item=it, how=rng.choice(HOW + ["lend"]),
                  **({"to": rng.choice(BEINGS)} if rng.random() < 0.85 else {}))
    if roll < 0.94:
        return op("damage", item=it, amount=rng.choice([0, 1, 30, 100]))
    return op("inspect_item", item=it, observer=b)


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = []
    for x in (prefix(rng) if rng.random() < 0.8 else []):
        ops.append(copy.deepcopy(x))
        if not accepts(ops, limits):
            ops.pop()
    t = 0
    target = len(ops) + rng.randint(1, 20)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        t += rng.choice([0, 1, 2, 5])
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
    ok_runs = err_runs = trespass = traces = thefts = promoted = 0
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
            trespass += int(r.get("trespass") is True)
            traces += len(r.get("traces", []))
            thefts += int(r.get("theft_appraisal") is True)
            promoted += int(bool(r.get("promoted")))
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "trespasses": trespass, "traces_seen": traces, "theft_appraisals": thefts, "promotions": promoted,
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
        sys.exit("surroundings checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
