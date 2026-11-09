#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 3 stage 5 social schemas: graded
norms and sanction ladders, social contracts, dyad memory, and attention and pragmatic profiles,
local kind `fabric`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/fabric/*/case.json + input.json
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
from fabric_reference import COOP, KINDS, OUTCOMES, SEVERITY, STYLES, TELL_KINDS
from fabric_reference import workflow as fabric_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "fabric"
LIMIT_NAMES = ["beings", "norms", "breaches", "contracts", "dyads", "threads"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "fabric":
        raise ValueError("unknown local kind")
    return fabric_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "fabric", "args": args}


THEFT = op("norm", id="no_theft", severity="crime", report_to="town_watch")
MEET = op("meet", a="mira", b="player", episode="rainy_market", summary="The player carried her flour sacks out of the rain.")


def breach(norm: str, off: str = "bo", wit: str = "mira", **kw: Any) -> Dict[str, Any]:
    return op("breach", norm=norm, offender=off, witness=wit, **kw)


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("theft_reported_with_guilt",
         "Theft is a crime: straight to confrontation and a report to the town watch; the apprentice's conscience (70) costs him guilt and he wants to make amends.",
         wf(THEFT, breach("no_theft", "apprentice", "mira", conscience=70)))
    case("etiquette_ladder_capped",
         "Muddy boots on the rug: a warning, then refused service, and never past that however often.",
         wf(op("norm", id="wipe_boots", severity="etiquette"), breach("wipe_boots"), breach("wipe_boots"),
            breach("wipe_boots")))
    case("custom_repeats_escalate",
         "Haggling over a fixed price: warning, refusal, then confrontation (no role to report to).",
         wf(op("norm", id="no_haggling", severity="custom"), breach("no_haggling"), breach("no_haggling"),
            breach("no_haggling")))
    case("contract_breach_starts_high",
         "Breaking a sworn bargain starts at refused service and a demand for a fine; guilt defaults to half the severity.",
         wf(op("norm", id="keep_bargain", severity="contract"), breach("keep_bargain", conscience=50)))
    case("norm_needs_its_situation",
         "No weapons drawn after dark: by day it does not apply (and is not counted); at night it is a taboo.",
         wf(op("norm", id="no_blades_at_night", severity="taboo", applies_when=["night", "weapon_drawn"]),
            breach("no_blades_at_night", context=["day"]), breach("no_blades_at_night", context=["night"])))
    case("apology_steps_back",
         "An apology after the second breach steps the ladder back, so the next breach is refused service again, not a confrontation.",
         wf(op("norm", id="no_haggling", severity="custom"), breach("no_haggling"), breach("no_haggling"),
            op("apologize", norm="no_haggling", offender="bo", witness="mira"), breach("no_haggling")))
    case("ladder_per_witness",
         "Each witness keeps their own ladder: Mira's second warning is a refusal, Tomas sees a first offense.",
         wf(op("norm", id="no_haggling", severity="custom"), breach("no_haggling"), breach("no_haggling"),
            breach("no_haggling", wit="tomas")))
    case("firewood_overdue",
         "The player promised firewood by tick 10; at 11 it is overdue and the player owes Mira the promise's weight.",
         wf(op("contract", id="firewood", kind="promise", debtor="player", creditor="mira",
               terms="Bring a bundle of firewood before the evening bake", due_at=10),
            op("tick", now=10), op("tick", now=11), op("ledger", a="mira", b="player")))
    case("settle_broken_and_fulfilled",
         "A broken alibi pact costs its full weight in trust and the witnesses hear; a fulfilled loan earns a quarter of its weight; a released favor nothing.",
         wf(op("contract", id="alibi", kind="alibi_pact", debtor="bo", creditor="thief", witnesses=["ana", "cat"],
               terms="Say we were at the mill all night", weight=60),
            op("contract", id="loan", kind="loan", debtor="bo", creditor="mira", terms="Ten copper till market day",
               weight=40),
            op("contract", id="favor", kind="favor", debtor="mira", creditor="bo", terms="One loaf, any day"),
            op("settle", id="alibi", outcome="broken"), op("settle", id="loan", outcome="fulfilled"),
            op("settle", id="favor", outcome="released")))
    case("favor_balance_helps",
         "Bo saved Mira twice (weights 40 and 30) and owes her 10: she is 50 points readier to help him (capped), he 10.",
         wf(op("contract", id="saved_1", kind="favor", debtor="mira", creditor="bo", terms="Pulled her from the river",
               weight=40),
            op("contract", id="saved_2", kind="favor", debtor="mira", creditor="bo", terms="Hid her from the bailiff",
               weight=30),
            op("contract", id="flour", kind="trade_order", debtor="bo", creditor="mira", terms="Two sacks of flour",
               weight=10),
            op("ledger", a="bo", b="mira"), op("ledger", a="bo", b="ana")))
    case("first_meeting_kept",
         "How they met is recorded once; meeting again returns the original encounter.",
         wf(MEET, op("tick", now=50), op("meet", a="player", b="mira", episode="second", summary="Again."),
            op("dyad", a="player", b="mira")))
    case("landmarks_replace_and_evict",
         "Up to four defining moments: a second shared danger replaces the first, a fifth kind drops the oldest.",
         wf(MEET, op("landmark", a="mira", b="player", kind="first_favor", episode="e1"),
            op("landmark", a="mira", b="player", kind="shared_danger", episode="e2", summary="The mill fire."),
            op("landmark", a="mira", b="player", kind="worst_betrayal", episode="e3"),
            op("landmark", a="mira", b="player", kind="shared_danger", episode="e4", summary="The flood."),
            op("landmark", a="mira", b="player", kind="reconciliation", episode="e5"),
            op("landmark", a="mira", b="player", kind="first_gift", episode="e6"),
            op("dyad", a="mira", b="player")))
    case("address_terms_directional",
         "Mira calls the player 'love' while the player calls her 'Master Baker'.",
         wf(MEET, op("address", **{"from": "mira", "to": "player", "term": "love"}),
            op("address", **{"from": "player", "to": "mira", "term": "Master Baker"}), op("tick", now=30),
            op("dyad", a="player", b="mira")))
    case("unfinished_threads",
         "An unanswered question about the old mine stays open until resolved.",
         wf(MEET, op("thread", a="mira", b="player", id="old_mine", opened_by="mira",
                     text="You still haven't told me what you were doing at the old mine."),
            op("thread", a="mira", b="player", id="recipe", opened_by="player", text="The rye recipe you promised."),
            op("resolve", a="player", b="mira", id="old_mine"), op("dyad", a="mira", b="player")))
    case("strangers_by_default",
         "Two beings who never met are strangers to each other.",
         wf(op("dyad", a="bo", b="ana")))
    case("guard_notices_knife",
         "A vigilant guard with a weapons bias notices the concealed knife first, then the limp; the cake is missed.",
         wf(op("profile", being="guard", attention={"bandwidth": 2, "vigilance": 70,
                                                     "biases": {"weapons_and_threats": 150, "injuries": 120}}),
            op("notice", being="guard", stimuli=[{"id": "cake", "category": "food_and_craft", "intensity": 60},
                                                  {"id": "knife", "category": "weapons_and_threats", "intensity": 50},
                                                  {"id": "limp", "category": "injuries", "intensity": 55}])))
    case("baker_notices_bread",
         "A baker with default vigilance notices spoiled grain first (40 raised to 56 by her craft bias) and the knife, but not a whisper below her floor of 50.",
         wf(op("profile", being="mira", attention={"biases": {"food_and_craft": 140}}),
            op("notice", being="mira", stimuli=[{"id": "spoiled_grain", "category": "food_and_craft", "intensity": 40},
                                                 {"id": "whisper", "category": "social", "intensity": 45},
                                                 {"id": "knife", "category": "weapons_and_threats", "intensity": 50}])))
    case("tells_and_style",
         "A poor secret keeper (20) shows both tells to an observer of insight 60; a skilled one (90) hides the speech tell; style follows the profile.",
         wf(op("profile", being="bo", pragmatic={"secret_keeping": 20, "interrupt_tolerance": 20,
                                                  "small_talk_patience": 80, "conflict_style": "avoidant",
                                                  "coping": "overwork", "tells": [
                                                      {"kind": "gaze", "cue": "avoids_eye_contact", "min_insight": 40},
                                                      {"kind": "speech", "cue": "over_explains", "min_insight": 55}]}),
            op("profile", being="ana", pragmatic={"secret_keeping": 90, "tells": [
                {"kind": "gaze", "cue": "avoids_eye_contact", "min_insight": 30},
                {"kind": "speech", "cue": "over_explains", "min_insight": 50}]}),
            op("tells", being="bo", observer_insight=60), op("tells", being="ana", observer_insight=60),
            op("tells", being="bo", observer_insight=60, lying=False), op("style", being="bo"),
            op("style", being="nobody")))
    case("bad_self_breach",
         "Malformed: a being cannot witness its own breach.",
         wf(THEFT, breach("no_theft", "bo", "bo")), error=True)
    case("bad_unknown_severity",
         "Malformed: severity is etiquette, custom, contract, taboo or crime.",
         wf(op("norm", id="x", severity="sin")), error=True)
    case("bad_contract_with_self",
         "Malformed: a contract needs two different parties.",
         wf(op("contract", id="c", kind="promise", debtor="bo", creditor="bo", terms="Nothing")), error=True)
    case("bad_thread_limit",
         "Malformed: with room for one unfinished thread per pair, a second is refused.",
         wf(MEET, op("thread", a="mira", b="player", id="t1", opened_by="mira", text="One."),
            op("thread", a="mira", b="player", id="t2", opened_by="mira", text="Two."), limits={"threads": 1}),
         error=True)
    case("bad_due_in_past",
         "Malformed: a contract cannot fall due before now.",
         wf(op("tick", now=20), op("contract", id="c", kind="loan", debtor="bo", creditor="mira", terms="Ten copper",
                                   due_at=19)), error=True)
    case("bad_landmark_for_strangers",
         "Malformed: a pair that never met has no history to mark.",
         wf(op("landmark", a="bo", b="ana", kind="first_favor", episode="e1")), error=True)
    case("bad_tell_kind",
         "Malformed: tells are gaze, speech, posture or hands.",
         wf(op("profile", being="bo", pragmatic={"tells": [{"kind": "smell", "cue": "sweat"}]})), error=True)
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


BEINGS = ["mira", "bo", "ana"]
NORMS = ["no_theft", "no_haggling", "wipe_boots"]


def prefix(rng: random.Random) -> List[Dict[str, Any]]:
    """A valid starting society (kept only while the limits allow it) so later random ops land."""
    out = [op("norm", id=n, severity=s, **({"applies_when": ["night"]} if rng.random() < 0.2 else {}),
              **({"report_to": "watch"} if rng.random() < 0.5 else {}))
           for n, s in zip(NORMS, ["crime", "custom", "etiquette"])]
    out += [op("meet", a=a, b=b, episode="e0", summary="They met at the well.")
            for a, b in [("mira", "bo"), ("bo", "ana")]]
    out += [op("profile", being="bo", attention={"bandwidth": 2, "biases": {"threat": 150}},
               pragmatic={"secret_keeping": rng.choice([0, 50, 100]), "tells": [{"kind": "gaze", "cue": "looks_away"}]})]
    return out


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    a, b = rng.sample(BEINGS, 2) if rng.random() < 0.95 else ("bo", "bo")
    roll = rng.random()
    if roll < 0.07:
        return op("tick", now=t)
    if roll < 0.12:
        return op("norm", id=rng.choice(NORMS), severity=rng.choice(list(SEVERITY) + ["sin"]),
                  **({"applies_when": rng.sample(["night", "day", "market"], rng.randint(0, 2))} if rng.random() < 0.4 else {}),
                  **({"guilt_cost": rng.choice([0, 40, 101])} if rng.random() < 0.3 else {}),
                  **({"report_to": "watch"} if rng.random() < 0.4 else {}))
    if roll < 0.30:
        return op("breach", norm=rng.choice(NORMS), offender=a, witness=b,
                  **({"context": rng.sample(["night", "day"], rng.randint(0, 2))} if rng.random() < 0.4 else {}),
                  **({"conscience": rng.choice([0, 49, 50, 100, 101])} if rng.random() < 0.4 else {}))
    if roll < 0.35:
        return op("apologize", norm=rng.choice(NORMS), offender=a, witness=b)
    if roll < 0.45:
        return op("contract", id=rng.choice(["c1", "c2", "c3"]), kind=rng.choice(KINDS + ["bet"]), debtor=a, creditor=b,
                  terms=rng.choice(["Bring wood", ""]),
                  **({"witnesses": rng.sample(BEINGS + ["cat"], rng.randint(0, 2))} if rng.random() < 0.4 else {}),
                  **({"due_at": t + rng.choice([-1, 0, 3, 10])} if rng.random() < 0.5 else {}),
                  **({"weight": rng.choice([0, 1, 40, 100])} if rng.random() < 0.4 else {}))
    if roll < 0.50:
        return op("settle", id=rng.choice(["c1", "c2", "c3"]), outcome=rng.choice(OUTCOMES + ["lost"]))
    if roll < 0.55:
        return op("ledger", a=a, b=b)
    if roll < 0.59:
        return op("meet", a=a, b=b, episode="e1", summary=rng.choice(["Met at the fair.", ""]))
    if roll < 0.64:
        return op("landmark", a=a, b=b, kind=rng.choice(["first_favor", "betrayal", "danger", "gift", "feud"]),
                  episode="e2", **({"summary": "It mattered."} if rng.random() < 0.5 else {}))
    if roll < 0.67:
        return op("address", **{"from": a, "to": b, "term": rng.choice(["friend", "Master Baker", "x" * 41])})
    if roll < 0.72:
        return op("thread", a=a, b=b, id=rng.choice(["t1", "t2"]), opened_by=rng.choice([a, b, "cat"]),
                  text="What about the mine?")
    if roll < 0.75:
        return op("resolve", a=a, b=b, id=rng.choice(["t1", "t2"]))
    if roll < 0.80:
        return op("dyad", a=a, b=b)
    if roll < 0.86:
        args: Dict[str, Any] = {"being": a}
        if rng.random() < 0.6:
            args["attention"] = {"bandwidth": rng.choice([0, 1, 4, 17]), "vigilance": rng.choice([0, 50, 100]),
                                 "biases": {"threat": rng.choice([0, 150, 301])}}
        if rng.random() < 0.6:
            args["pragmatic"] = {"conflict_style": rng.choice(STYLES + ["loud"]), "cooperativeness": rng.choice(COOP),
                                 "secret_keeping": rng.choice([0, 50, 100]),
                                 "interrupt_tolerance": rng.choice([0, 29, 30, 70]),
                                 "small_talk_patience": rng.choice([29, 69, 70]),
                                 "tells": [{"kind": rng.choice(TELL_KINDS + ["smell"]), "cue": c,
                                            "min_insight": rng.choice([0, 40, 100])}
                                           for c in rng.sample(["looks_away", "stammers"], rng.randint(0, 2))]}
        return op("profile", **args)
    if roll < 0.92:
        return op("notice", being=a, stimuli=[{"id": s, "category": rng.choice(["threat", "food"]),
                                               "intensity": rng.choice([0, 30, 50, 80, 100, 101])}
                                              for s in rng.sample(["knife", "cake", "limp", "coin"], rng.randint(0, 4))])
    if roll < 0.96:
        return op("tells", being=a, observer_insight=rng.choice([0, 40, 60, 100, 101]),
                  **({"lying": rng.random() < 0.5} if rng.random() < 0.5 else {}))
    return op("style", being=a)


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """A seeded society, then random ops kept only while the reference accepts them; a quarter of
    workflows instead end on their first refused op."""
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
    ok_runs = err_runs = ladder = contracts_late = threads = tells = 0
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
            ladder += int(r.get("step") == 3)
            contracts_late += int(r.get("late") is True or bool(r.get("overdue")))
            threads += len(r.get("threads", []))
            tells += len(r.get("perceived", []))
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "top_of_ladder": ladder, "late_contracts": contracts_late, "open_threads_seen": threads, "tells_seen": tells,
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
        sys.exit("fabric checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
