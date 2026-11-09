#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 B6: off-screen conversation
between characters (topics known and wanted, seeded turn order, tells, disputes, secrets kept
until warm, gossip, alliances and quarrels), local kind `chatter`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/chatter/*/case.json + input.json
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
from chatter_reference import workflow as chat_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "chatter"
LIMIT_NAMES = ["beings", "topics", "wants", "bonds", "records", "turns"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "chatter":
        raise ValueError("unknown local kind")
    return chat_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "chatter", "args": args}


def know(being: str, topic: str, value: str, weight: int = 50, **kw: Any) -> Dict[str, Any]:
    return op("know", being=being, topic=topic, value=value, weight=weight, **kw)


def meet(a: str, b: str, seed: int, **kw: Any) -> Dict[str, Any]:
    return op("meet", a=a, b=b, seed=seed, **kw)


VILLAGE = [know("baker", "mill_price", "rising", 80), know("baker", "new_oven", "built", 30),
           know("widow", "fox_den", "north_field", 60), know("widow", "mill_price", "rising", 40),
           op("want", being="baker", topic="fox_den", weight=70)]


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    add("chat_tell_what_is_wanted",
        "The widow knows where the fox den is and the baker wants it: she tells it first, the baker learns it at half weight and warmth rises by 5.",
        wf(*VILLAGE, meet("baker", "widow", 3), op("assess", being="baker")))

    add("chat_same_value_not_repeated",
        "Both already know the mill price is rising, so neither repeats it; the baker passes on the new oven instead.",
        wf(*VILLAGE, meet("baker", "widow", 3), op("assess", being="widow")))

    add("chat_seed_picks_first_speaker",
        "The seed decides who speaks first (FNV-1a of seed:a:b, even means a); the record names the first speaker.",
        wf(*VILLAGE, meet("baker", "widow", 1), meet("baker", "widow", 2), op("log", being="baker")))

    add("chat_wanted_beats_weight",
        "A wanted topic scores 2·want + weight: the tinker's weight-20 road news (wanted 50 → 120) goes before its weight-90 tale.",
        wf(know("tinker", "road", "flooded", 20), know("tinker", "tale", "dragon", 90),
           op("want", being="smith", topic="road", weight=50), know("smith", "iron", "scarce", 10),
           meet("tinker", "smith", 4, turns=3)))

    add("chat_secret_waits_for_warmth",
        "The guard keeps the gate-key secret at warmth 40 and tells it at warmth 60.",
        wf(know("guard", "gate_key", "under_stone", 90, secret=True), know("niece", "song", "new", 10),
           op("bond", a="guard", b="niece", warmth=40), meet("guard", "niece", 5),
           op("bond", a="guard", b="niece", warmth=60), meet("guard", "niece", 5)))

    add("chat_dispute_becomes_quarrel",
        "The miller says the bridge is safe, the carter says broken: at warmth 0 the dispute is a quarrel, warmth −10 and resentment both ways.",
        wf(know("miller", "bridge", "safe", 70), know("carter", "bridge", "broken", 70), meet("miller", "carter", 2),
           op("assess", being="carter")))

    add("chat_warm_dispute_is_not_quarrel",
        "Old friends (warmth 50) disagree about the bridge without quarrelling; warmth stays.",
        wf(know("miller", "bridge", "safe", 70), know("carter", "bridge", "broken", 70),
           op("bond", a="carter", b="miller", warmth=50), meet("miller", "carter", 2)))

    add("chat_alliance_from_shared_wants",
        "At warmth 55 each tells the other what it wants: warmth reaches 65 and the meeting is an alliance with two help proposals.",
        wf(know("smith", "ore_vein", "east_hill", 40), know("hunter", "wolf_pack", "west_wood", 40),
           op("want", being="hunter", topic="ore_vein"), op("want", being="smith", topic="wolf_pack"),
           op("bond", a="hunter", b="smith", warmth=55), meet("smith", "hunter", 6)))

    add("chat_gossip_spreads_in_steps",
        "News moves one meeting at a time: the baker tells the widow, the widow tells the guard, each copy at half weight.",
        wf(know("baker", "thief", "seen_at_dawn", 80), know("widow", "rain", "coming", 10), know("guard", "curfew", "early", 10),
           meet("baker", "widow", 3), meet("widow", "guard", 3), op("assess", being="guard")))

    add("chat_small_talk_ends_meeting",
        "With nothing new to say, two small-talk turns end the meeting early.",
        wf(know("ann", "x", "same", 10), know("bo", "x", "same", 10), meet("ann", "bo", 1)))

    add("chat_turns_cap",
        "A meeting limited to 1 turn carries one tell even when both have more to share.",
        wf(*VILLAGE, meet("baker", "widow", 3, turns=1)))

    add("chat_learning_needs_room",
        "With topics limited to 2 both already hold 2: they hear the news but keep none of it, so there are no belief or help proposals and warmth stays.",
        wf(*VILLAGE, meet("baker", "widow", 3), op("assess", being="widow"), limits={"topics": 2}))

    add("chat_knowing_clears_want",
        "Learning a wanted topic, or being told it by the host, removes the want.",
        wf(op("want", being="ann", topic="road", weight=30), op("assess", being="ann"),
           know("ann", "road", "open", 10), op("assess", being="ann")))

    add("chat_log_newest_first_and_bounded",
        "The log lists newest records first; with records limited to 2 only the last two meetings stay.",
        wf(*VILLAGE, *[meet("baker", "widow", s, at=s) for s in range(1, 5)], op("log", limit=5),
           limits={"records": 2}))

    add("chat_bonds_symmetric",
        "A bond is stored under the sorted pair and seen from both sides.",
        wf(know("ann", "x", "y"), know("bo", "x", "y"), op("bond", a="bo", b="ann", warmth=-30),
           op("assess", being="ann"), op("assess", being="bo")))

    add("chat_quarrel_clamps_warmth",
        "Three disputes at warmth −80 cost 30 but warmth stops at −100.",
        wf(*[know("ann", t, "yes") for t in ["a", "b", "c"]], *[know("bo", t, "no") for t in ["a", "b", "c"]],
           op("bond", a="ann", b="bo", warmth=-80), meet("ann", "bo", 1)))

    add("chat_secret_travels_as_secret",
        "A secret told to a trusted friend stays secret for the friend, who will not pass it to a stranger.",
        wf(know("guard", "gate_key", "under_stone", 90, secret=True), know("niece", "song", "new", 10),
           know("stranger", "map", "old", 10), op("bond", a="guard", b="niece", warmth=70),
           meet("guard", "niece", 5), meet("niece", "stranger", 5)))

    add("limits_echo_and_beings_cap",
        "Limits are echoed; with beings 2 two minds fit.",
        wf(know("ann", "x", "y"), know("bo", "x", "z"), op("log"), limits={"beings": 2, "turns": 4}))

    add("chat_quarrel_outweighs_gossip",
        "News is told and learned, but a dispute at low warmth still makes the meeting a quarrel.",
        wf(know("miller", "bridge", "safe", 70), know("miller", "fair", "tomorrow", 50),
           know("carter", "bridge", "broken", 70), meet("miller", "carter", 2), op("assess", being="carter")))

    add("bad_meet_unknown_being",
        "A being with no topics or wants cannot meet.",
        wf(know("ann", "x", "y"), meet("ann", "ghost", 1)), error=True)

    add("bad_meet_self",
        "A being cannot meet itself.",
        wf(know("ann", "x", "y"), meet("ann", "ann", 1)), error=True)

    add("bad_warmth_out_of_range",
        "Warmth is −100..100.",
        wf(op("bond", a="ann", b="bo", warmth=101)), error=True)

    add("bad_turns_over_limit",
        "A meeting's turns may not exceed the turns limit.",
        wf(know("ann", "x", "y"), know("bo", "x", "y"), meet("ann", "bo", 1, turns=7)), error=True)

    add("bad_want_known_topic",
        "A being cannot want a topic it already knows.",
        wf(know("ann", "x", "y"), op("want", being="ann", topic="x")), error=True)

    add("bad_topics_full",
        "With topics limited to 1, a second topic is refused.",
        wf(know("ann", "x", "y"), know("ann", "z", "y"), limits={"topics": 1}), error=True)

    return cases


BEINGS = ["ann", "bo", "cy", "dee"]
TOPICS = ["road", "price", "thief", "rain", "fox"]
VALUES = ["yes", "no", "maybe"]


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


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    a, b = rng.sample(BEINGS, 2)
    roll = rng.random()
    if roll < 0.35:
        args: Dict[str, Any] = {"being": a, "topic": rng.choice(TOPICS), "value": rng.choice(VALUES)}
        if rng.random() < 0.7:
            args["weight"] = rng.randint(0, 100)
        if rng.random() < 0.3:
            args["secret"] = rng.random() < 0.5
        return op("know", **args)
    if roll < 0.47:
        args = {"being": a, "topic": rng.choice(TOPICS)}
        if rng.random() < 0.7:
            args["weight"] = rng.randint(1, 100)
        return op("want", **args)
    if roll < 0.57:
        return op("bond", a=a, b=b, warmth=rng.randint(-100, 100))
    if roll < 0.90:
        args = {"a": a, "b": b, "seed": rng.randint(0, 10**6), "at": t}
        if rng.random() < 0.3:
            args["turns"] = rng.randint(1, 6)
        return op("meet", **args)
    if roll < 0.95:
        args = {}
        if rng.random() < 0.6:
            args["being"] = a
        if rng.random() < 0.4:
            args["limit"] = rng.randint(1, 8)
        return op("log", **args)
    return op("assess", being=a)


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
        t += rng.randint(0, 20)
        ops.append(rand_op(rng, t))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
    if not ops:
        ops.append(op("log"))
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
    ok_runs = err_runs = tells = disputes = quarrels = alliances = 0
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
        tells += proc.stdout.count('"kind":"tell"')
        disputes += proc.stdout.count('"kind":"dispute"')
        quarrels += proc.stdout.count('"outcome":"quarrel"')
        alliances += proc.stdout.count('"outcome":"alliance"')
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "tells": tells, "disputes": disputes, "quarrels": quarrels, "alliances": alliances,
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
        sys.exit("chatter checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
