#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 stage 6: goals from
experience (A4, local kind `aspiration`), habits (A5, `habit`), lessons (A6, `lesson`) and
host-set limits.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/growth/*/case.json + input.json
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
from growth_reference import process_local

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "growth"


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    return process_local(request.get("kind"), request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(kind: str, *ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": kind, "args": args}


def xp(being: str, kind: str, subject: str, weight: int = 50, at: int = 0) -> Dict[str, Any]:
    return op("experience", being=being, kind=kind, subject=subject, weight=weight, at=at)


def ch(being: str, slot: str, action: str, at: int = 0) -> Dict[str, Any]:
    return op("choose", being=being, slot=slot, action=action, at=at)


def oc(being: str, about: str, activity: str, result: str, episode: str, at: int = 0) -> Dict[str, Any]:
    return op("outcome", being=being, about=about, activity=activity, result=result, episode=episode, at=at)


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    # ---- A4 goals from experience
    add("asp_need_forms_store_goal",
        "Three hungry days (weight 40 each) build pressure to 120 and form 'Keep a store against hunger' at priority 52.",
        wf("aspiration", xp("baker", "need", "hunger", 40, 1), xp("baker", "need", "hunger", 40, 25),
           xp("baker", "need", "hunger", 40, 49), op("goals", being="baker"), op("assess", being="baker")))

    add("asp_threat_forms_and_reinforces",
        "Two thefts form a guard goal; a third reinforces its priority by weight/10.",
        wf("aspiration", xp("miller", "threat", "thief", 60, 3), xp("miller", "threat", "thief", 60, 9),
           xp("miller", "threat", "thief", 60, 12), op("goals", being="miller")))

    add("asp_every_template",
        "One strong experience of each kind forms each template goal; goals list by priority then id.",
        wf("aspiration", *[xp("apprentice", k, s, 100) for k, s in
                           [("need", "cold"), ("threat", "wolves"), ("admiration", "baking"), ("loss", "knife"),
                            ("opportunity", "fair")]],
           op("goals", being="apprentice")))

    add("asp_pressure_window_resets",
        "Pressure forgotten after 200 quiet ticks starts again; experiences close together form the goal.",
        wf("aspiration", xp("smith", "admiration", "carving", 60, 0), xp("smith", "admiration", "carving", 60, 201),
           xp("smith", "admiration", "carving", 60, 300)))

    add("asp_model_proposals_validated",
        "A model-proposed goal is rejected when uncited or weakly evidenced, accepted with priority capped at 60, and refused if it exists.",
        wf("aspiration",
           op("propose", being="widow", goal={"id": "adopt_pup", "label": "Adopt a pup", "action": "adopt"},
              priority=80, cites=[{"kind": "loss", "subject": "dog"}]),
           xp("widow", "loss", "dog", 30),
           op("propose", being="widow", goal={"id": "adopt_pup", "label": "Adopt a pup", "action": "adopt"},
              priority=80, cites=[{"kind": "loss", "subject": "dog"}]),
           xp("widow", "loss", "dog", 30),
           op("propose", being="widow", goal={"id": "adopt_pup", "label": "Adopt a pup", "action": "adopt",
                                              "target": "litter"},
              priority=80, cites=[{"kind": "loss", "subject": "dog"}], at=7),
           op("propose", being="widow", goal={"id": "adopt_pup", "label": "Adopt a pup", "action": "adopt"},
              priority=40, cites=[{"kind": "loss", "subject": "dog"}]),
           op("goals", being="widow")))

    add("asp_goal_room_blocked_then_dropped",
        "With room for one goal, a second ready pressure is blocked until the first goal is dropped.",
        wf("aspiration", xp("guard", "threat", "raiders", 100), xp("guard", "need", "thirst", 100),
           op("drop", being="guard", goal="guard_raiders"), op("drop", being="guard", goal="guard_raiders"),
           xp("guard", "need", "thirst", 10), op("goals", being="guard"), limits={"goals": 1}))

    add("asp_pressure_eviction",
        "With room for two pressures, a third evicts the weakest (then oldest).",
        wf("aspiration", xp("trader", "opportunity", "fair", 20, 1), xp("trader", "opportunity", "port", 30, 2),
           xp("trader", "threat", "bandits", 40, 3), op("assess", being="trader"), limits={"pressures": 2}))

    add("asp_malformed_unknown_kind",
        "Experience kinds come from the fixed template list.",
        wf("aspiration", xp("baker", "boredom", "rain")), error=True)

    add("asp_malformed_long_subject",
        "Subjects are at most 48 characters so template goal ids fit.",
        wf("aspiration", xp("baker", "need", "s" * 49)), error=True)

    # ---- A5 habits
    add("hab_habit_forms_after_three",
        "Walking by the river at dusk three times running forms a habit at strength 40.",
        wf("habit", ch("baker", "dusk", "walk_river", 1), ch("baker", "dusk", "walk_river", 2),
           ch("baker", "dusk", "walk_river", 3), ch("baker", "dusk", "walk_river", 4), op("assess", being="baker")))

    add("hab_authored_kept_and_floored",
        "An authored entry is kept by choosing it (up to 100) and weakened by other choices, never below 30.",
        wf("habit",
           op("set_routine", being="baker", entries=[{"id": "bake_bread", "slot": "dawn", "action": "bake"},
                                                     {"id": "sweep", "slot": "noon", "action": "sweep",
                                                      "strength": 32}]),
           ch("baker", "dawn", "bake", 1), *[ch("baker", "dawn", "bake", 2 + i) for i in range(4)],
           ch("baker", "noon", "nap", 6), ch("baker", "noon", "nap", 7), op("assess", being="baker")))

    add("hab_habit_fades",
        "A formed habit loses 10 per contrary choice and fades at 0; the new choice becomes a habit in turn.",
        wf("habit", *[ch("smith", "evening", "tavern", i) for i in range(3)],
           *[ch("smith", "evening", "whittle", 10 + i) for i in range(4)], op("routine", being="smith")))

    add("hab_skip_with_reason_and_history",
        "An explicit skip weakens an entry and records why; history keeps the last 8 changes.",
        wf("habit",
           op("set_routine", being="miller", entries=[{"id": "grind", "slot": "morning", "action": "grind"}]),
           *[op("skip", being="miller", entry="grind", at=i, reason="river too low") for i in range(9)],
           op("skip", being="miller", entry="grind", at=10), op("assess", being="miller")))

    add("hab_broken_streak_resets",
        "A different choice in the slot resets the streak; the habit forms only after three in a row.",
        wf("habit", ch("guard", "night", "patrol"), ch("guard", "night", "patrol"), ch("guard", "night", "dice"),
           ch("guard", "night", "patrol"), ch("guard", "night", "patrol"), ch("guard", "night", "patrol")))

    add("hab_routine_for_arbiter",
        "routine lists entries as arbiter candidates by strength, optionally for one slot; an unknown being has none.",
        wf("habit",
           op("set_routine", being="baker", entries=[{"id": "bake_bread", "slot": "dawn", "action": "bake"},
                                                     {"id": "market", "slot": "noon", "action": "sell",
                                                      "strength": 90},
                                                     {"id": "prayers", "slot": "dusk", "action": "tend_hearth",
                                                      "strength": 60}]),
           *[ch("baker", "dusk", "walk_river", i) for i in range(3)],
           op("routine", being="baker"), op("routine", being="baker", slot="dusk"),
           op("routine", being="stranger"),
           op("set_routine", being="baker", entries=[{"id": "bake_bread", "slot": "dawn", "action": "bake"}]),
           op("routine", being="baker")))

    add("hab_malformed_authored_habit_prefix",
        "Authored entry ids may not use the habit_ prefix.",
        wf("habit", op("set_routine", being="baker",
                       entries=[{"id": "habit_dawn_bake", "slot": "dawn", "action": "bake"}])), error=True)

    # ---- A6 lessons
    add("les_trader_deals_go_badly",
        "Deals with the trader went badly 3 of 4 times: an avoid lesson citing those episodes, with a -18 motive bias.",
        wf("lesson", oc("baker", "trader", "deal", "bad", "ep1", 10), oc("baker", "trader", "deal", "good", "ep2", 20),
           oc("baker", "trader", "deal", "bad", "ep3", 30), oc("baker", "trader", "deal", "bad", "ep4", 40),
           op("advice", being="baker")))

    add("les_seek_lesson_confirmed",
        "Good outcomes form a seek lesson; another good one confirms it, a bad one contradicts without flipping.",
        wf("lesson", *[oc("apprentice", "miller", "trade", "good", f"t{i}", i) for i in range(3)],
           oc("apprentice", "miller", "trade", "good", "t3", 3), oc("apprentice", "miller", "trade", "bad", "t4", 4),
           op("advice", being="apprentice", about="miller")))

    add("les_revised_by_new_outcomes",
        "With a three-episode window, the old bad deals drop out and the lesson flips from avoid to seek.",
        wf("lesson", *[oc("baker", "trader", "deal", "bad", f"b{i}", i) for i in range(3)],
           *[oc("baker", "trader", "deal", "good", f"g{i}", 10 + i) for i in range(3)],
           limits={"episodes": 3}))

    add("les_doubted_without_majority",
        "With a four-episode window, mixed evidence leaves no majority and the lesson is doubted.",
        wf("lesson", *[oc("baker", "trader", "deal", "bad", f"b{i}", i) for i in range(3)],
           *[oc("baker", "trader", "deal", "good", f"g{i}", 10 + i) for i in range(2)],
           op("assess", being="baker"), limits={"episodes": 4}))

    add("les_advice_filters_and_forget",
        "advice filters by subject or activity; forget removes a topic.",
        wf("lesson", *[oc("guard", "wolves", "hunt", "bad", f"w{i}", i) for i in range(3)],
           *[oc("guard", "fox", "hunt", "good", f"f{i}", i) for i in range(3)],
           *[oc("guard", "wolves", "trap", "good", f"t{i}", i) for i in range(3)],
           op("advice", being="guard", activity="hunt"), op("advice", being="guard", about="wolves"),
           op("forget", being="guard", about="fox", activity="hunt"),
           op("forget", being="guard", about="fox", activity="hunt"),
           op("advice", being="guard"), op("advice", being="nobody")))

    add("les_topic_eviction",
        "With room for two topics, a third evicts the one with the fewest episodes (then the oldest).",
        wf("lesson", oc("smith", "ore", "buy", "bad", "o1", 5), oc("smith", "ore", "buy", "bad", "o2", 6),
           oc("smith", "coal", "buy", "good", "c1", 7), oc("smith", "tin", "buy", "good", "t1", 8),
           op("assess", being="smith"), limits={"topics": 2}))

    add("les_malformed_duplicate_episode",
        "An episode id is recorded once per topic.",
        wf("lesson", oc("baker", "trader", "deal", "bad", "ep1"), oc("baker", "trader", "deal", "good", "ep1")),
        error=True)

    # ---- host limits
    add("lim_limits_echoed_and_enforced",
        "Host limits are echoed with the defaults they leave alone; capacity follows them.",
        wf("habit", op("set_routine", being="a", entries=[]), op("set_routine", being="b", entries=[]),
           limits={"beings": 2, "entries": 500}))

    add("lim_malformed_over_ceiling",
        "A limit above its hard ceiling is refused.",
        wf("habit", op("routine", being="a"), limits={"entries": 513}), error=True)
    return cases


BEINGS = ["baker", "miller", "guard", "smith"]
KINDS = ["admiration", "loss", "need", "opportunity", "threat"]
SUBJECTS = ["hunger", "thief", "baking", "dog", "fair"]


def rlimits(rng: random.Random, names: List[str]) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {n: rng.randint(4 if n == "beings" else 1, 6) for n in rng.sample(names, rng.randint(1, len(names)))}
    if rng.random() < 0.02:
        lim[names[0]] = 0
    return lim


def rand_aspire(rng: random.Random) -> Dict[str, Any]:
    ops = []
    t = 0
    known: List[str] = []
    for _ in range(rng.randint(1, 14)):
        b = rng.choice(BEINGS)
        t += rng.randint(0, 120)
        roll = rng.random()
        if roll < 0.6:
            x: Dict[str, Any] = {"being": b, "kind": rng.choice(KINDS[:2]), "subject": rng.choice(SUBJECTS[:2])}
            known.append(b)
            if rng.random() < 0.8:
                x["weight"] = rng.randint(1, 100)
            if rng.random() < 0.8:
                x["at"] = t
            if rng.random() < 0.01:
                x["kind"] = "boredom"
            ops.append(op("experience", **x))
        elif roll < 0.75:
            cites = [{"kind": rng.choice(KINDS), "subject": rng.choice(SUBJECTS)} for _ in range(rng.randint(1, 2))]
            if len(cites) == 2 and cites[0] == cites[1]:
                cites.pop()
            g: Dict[str, Any] = {"id": rng.choice(["adopt_pup", "build_wall", "guard_thief"]), "label": "Proposed",
                                 "action": "act"}
            if rng.random() < 0.3:
                g["target"] = rng.choice(SUBJECTS)
            ops.append(op("propose", being=b, goal=g, priority=rng.randint(1, 100), cites=cites, at=t))
            known.append(b)
        elif roll < 0.82:
            ops.append(op("drop", being=b, goal=rng.choice(["guard_thief", "stock_hunger", "learn_baking"])))
        elif roll < 0.92:
            ops.append(op("goals", being=b))
        else:
            ops.append(op("assess", being=rng.choice(known)) if known and rng.random() < 0.98 else op("goals", being=b))
    return wf("aspiration", *ops, limits=rlimits(rng, ["beings", "pressures", "goals"]))


SLOTS = ["dawn", "noon", "dusk"]
ACTS = ["bake", "sell", "walk", "nap"]


def rand_habit(rng: random.Random) -> Dict[str, Any]:
    ops = []
    authored: Dict[str, List[str]] = {}
    for i in range(rng.randint(1, 16)):
        b = rng.choice(BEINGS[:2])
        roll = rng.random()
        if roll < 0.12:
            entries = []
            for j in range(rng.randint(0, 3)):
                e: Dict[str, Any] = {"id": f"r{j}", "slot": rng.choice(SLOTS), "action": rng.choice(ACTS)}
                if rng.random() < 0.5:
                    e["strength"] = rng.randint(30, 100)
                if any(x["slot"] == e["slot"] and x["action"] == e["action"] for x in entries):
                    continue
                entries.append(e)
            ops.append(op("set_routine", being=b, entries=entries))
            authored[b] = [e["id"] for e in entries]
        elif roll < 0.75:
            ops.append(ch(b, rng.choice(SLOTS), rng.choice(ACTS[:2]) if rng.random() < 0.7 else rng.choice(ACTS), i))
        elif roll < 0.85:
            if authored.get(b) and rng.random() < 0.98:
                entry = rng.choice(authored[b])
            elif authored and rng.random() < 0.98:
                b = rng.choice(sorted(authored))
                entry = rng.choice(authored[b]) if authored[b] else "r0"
            elif rng.random() < 0.05:
                entry = rng.choice(["r0", "habit_dawn_bake"])
            else:
                ops.append(ch(b, rng.choice(SLOTS), rng.choice(ACTS[:2]), i))
                continue
            if not authored.get(b) and rng.random() < 0.95:
                ops.append(ch(b, rng.choice(SLOTS), rng.choice(ACTS[:2]), i))
                continue
            s: Dict[str, Any] = {"being": b, "entry": entry, "at": i}
            if rng.random() < 0.5:
                s["reason"] = "too tired"
            ops.append(op("skip", **s))
        elif roll < 0.93:
            r: Dict[str, Any] = {"being": b}
            if rng.random() < 0.5:
                r["slot"] = rng.choice(SLOTS)
            ops.append(op("routine", **r))
        else:
            seen = sorted({x["args"]["being"] for x in ops})
            ops.append(op("assess", being=rng.choice(seen)) if seen and rng.random() < 0.98 else op("routine", being=b))
    return wf("habit", *ops, limits=rlimits(rng, ["beings", "entries", "slots"]))


def rand_lesson(rng: random.Random) -> Dict[str, Any]:
    ops = []
    for i in range(rng.randint(1, 16)):
        b = rng.choice(BEINGS[:2])
        roll = rng.random()
        if roll < 0.75:
            ops.append(oc(b, rng.choice(["trader", "miller", "wolves"]), rng.choice(["deal", "hunt"]),
                          "bad" if rng.random() < 0.55 else "good",
                          f"e{i}" if rng.random() > 0.01 else "e0", i))
        elif roll < 0.88:
            a: Dict[str, Any] = {"being": b}
            if rng.random() < 0.3:
                a["about"] = rng.choice(["trader", "wolves"])
            if rng.random() < 0.3:
                a["activity"] = "deal"
            ops.append(op("advice", **a))
        elif roll < 0.94:
            ops.append(op("forget", being=b, about="trader", activity="deal"))
        else:
            seen = sorted({x["args"]["being"] for x in ops if x["op"] == "outcome"})
            ops.append(op("assess", being=rng.choice(seen)) if seen and rng.random() < 0.98 else op("advice", being=b))
    return wf("lesson", *ops, limits=rlimits(rng, ["beings", "topics", "episodes"]))


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    return rng.choice([rand_aspire, rand_habit, rand_lesson])(rng)


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
    ok_runs = err_runs = goals = habits = lessons = 0
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
        goals += proc.stdout.count('"result":"formed"')
        habits += proc.stdout.count('"state":"formed"')
        lessons += proc.stdout.count('"change":"learned"') + proc.stdout.count('"change":"revised"')
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "goals_formed": goals, "habits_formed": habits, "lessons_learned_or_revised": lessons, "actual_model_calls": 0, "api_spend_usd": 0,
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
        sys.exit("growth checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
