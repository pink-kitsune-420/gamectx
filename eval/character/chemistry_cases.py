#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 4 stage 6: chemistry, banter and
pacing, local kind `chemistry`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/chemistry/*/case.json + input.json
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
from chemistry_reference import ADVICE, BEATS, HUMOUR, MOVES, TEMPO
from chemistry_reference import workflow as chemistry_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "chemistry"
LIMIT_NAMES = ["beings", "pairs", "jokes", "scenes"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "chemistry":
        raise ValueError("unknown local kind")
    return chemistry_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "chemistry", "args": args}


def sty(being: str, humour: str, tempo: str = "measured", formality: int = 30, assertiveness: int = 50,
        **extra: Any) -> Dict[str, Any]:
    return op("style", being=being, humour=humour, tempo=tempo, formality=formality, assertiveness=assertiveness,
              **extra)


def banter(speaker: str = "mira", listener: str = "bo", mood: int = 20, pacing: str = "steady", **extra: Any):
    return op("banter", speaker=speaker, listener=listener, mood=mood, pacing=pacing, **extra)


def beat(t: int, n: int, e: int, th: int, kind: str = "line") -> Dict[str, Any]:
    return op("beat", scene="market", kind=kind, tension=t, novelty=n, engagement=e, threads_open=th)


MIRA = sty("mira", "dry", assertiveness=60, values=["family", "craft"])
BO = sty("bo", "absurd", tempo="fast", formality=20, assertiveness=20, values=["craft"])
GUARD = sty("guard", "cutting", tempo="slow", formality=80, assertiveness=90)
FRIENDS = [MIRA, BO, op("history", a="mira", b="bo", landmarks=3, favors=2)]
JOKE = op("joke", a="mira", b="bo", id="flour_sneeze", moment="The day the flour sack burst on the bridge.")


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("style_view", "A social style: humour, tempo, formality, assertiveness and sorted values.", wf(MIRA))
    case("chemistry_friends", "A dry wit and an absurd one who share craft and history get on well.",
         wf(*FRIENDS, op("pair", a="bo", b="mira")))
    case("chemistry_clash", "A dry wit and a cutting, formal, assertive guard: nothing in common.",
         wf(MIRA, GUARD, op("pair", a="mira", b="guard")))
    case("chemistry_breaches", "Breaches in a pair's history sour even matching styles.",
         wf(sty("ana", "warm"), sty("ida", "warm"), op("history", a="ana", b="ida", breaches=4),
            op("pair", a="ana", b="ida")))
    case("chemistry_floor_ceiling", "Chemistry is clamped to 0-100: the best fit and the worst.",
         wf(sty("a", "warm", values=["v1", "v2", "v3", "v4", "v5"]), sty("b", "warm", values=["v1", "v2", "v3", "v4", "v5"]),
            op("history", a="a", b="b", landmarks=9, favors=9),
            sty("c", "cutting", tempo="fast", formality=0, assertiveness=95),
            sty("d", "warm", tempo="slow", formality=100, assertiveness=90), op("history", a="c", b="d", breaches=9),
            op("pair", a="a", b="b"), op("pair", a="c", b="d")))
    case("callback_spends_freshness", "A running joke comes back, and each callback spends its freshness.",
         wf(*FRIENDS, JOKE, banter(seed=1), banter(seed=2), op("pair", a="mira", b="bo")))
    case("joke_rests_then_returns", "A tired joke rests while time restores it.",
         wf(*FRIENDS, JOKE, banter(), banter(), banter(), op("tick", now=100), banter()))
    case("joke_retired_after_five", "A joke used five times is retired for good.",
         wf(*FRIENDS, JOKE, *[x for i in range(5) for x in (banter(), op("tick", now=(i + 1) * 300))],
            op("pair", a="mira", b="bo")))
    case("tease_lands_with_rapport", "With rapport, a dry tease lands as play.", wf(*FRIENDS, banter()))
    case("tease_insults_without_rapport", "Proposed without the rapport for it, a tease is heard as an insult.",
         wf(MIRA, GUARD, banter(listener="guard", move="tease")))
    case("escalate_when_pacing_allows", "Good chemistry and room to escalate: the banter escalates.",
         wf(*FRIENDS, banter(pacing="escalate")))
    case("low_mood_checks_rapport", "In a low mood a speaker checks in instead of joking; with no rapport, leaves.",
         wf(*FRIENDS, sty("rival", "cutting", tempo="fast", formality=95, assertiveness=95),
            op("history", a="mira", b="rival", breaches=3), banter(mood=-50), banter(listener="rival", mood=-50)))
    case("end_means_exit", "When pacing says the conversation has run its course, the move is a graceful exit.",
         wf(*FRIENDS, JOKE, banter(pacing="end")))
    case("warm_compliment_in_disguise", "A warm speaker without a joke or tease wraps a compliment.",
         wf(sty("ana", "warm"), sty("ida", "none"), banter("ana", "ida")))
    case("slow_pacing_deflates", "When the scene needs slowing, a speaker deflates.",
         wf(MIRA, sty("ida", "none", formality=90), banter(listener="ida", pacing="slow")))
    case("scene_starts_steady", "A new scene with no beats is steady.", wf(op("scene", id="market")))
    case("scene_escalates_when_flat", "Low tension and novel beats: escalate.",
         wf(op("scene", id="market"), beat(10, 70, 80, 2)))
    case("scene_pauses_after_jump", "A big jump in tension asks for a pause while silence is left.",
         wf(op("scene", id="market"), beat(40, 40, 70, 1)))
    case("scene_slows_when_dense", "Six beats within ten ticks: slow down.",
         wf(op("scene", id="market"), *[beat(1, 40, 60, 2) for _ in range(6)]))
    case("scene_ends_naturally", "No open thread, stale beats and falling engagement: the conversation can end.",
         wf(op("scene", id="market"), beat(5, 20, 80, 1), beat(0, 20, 60, 0), beat(0, 10, 40, 0)))
    case("silence_budget", "Silence beats spend the budget; a pause is no longer advised once it is gone.",
         wf(op("scene", id="market", silence=1), beat(0, 40, 60, 1, kind="silence"), beat(40, 40, 60, 1)))
    case("bad_unknown_humour", "Malformed: humour is dry, warm, absurd, cutting or none.",
         wf(sty("mira", "sardonic")), error=True)
    case("bad_banter_unstyled", "Malformed: banter needs two styled beings.",
         wf(MIRA, banter()), error=True)
    case("bad_callback_without_joke", "Malformed: a proposed callback needs a fresh running joke.",
         wf(*FRIENDS, banter(move="callback")), error=True)
    case("bad_silence_without_budget", "Malformed: no silence beat once the budget is spent.",
         wf(op("scene", id="market", silence=0), beat(0, 10, 10, 0, kind="silence")), error=True)
    case("bad_mood_range", "Malformed: mood is -100 to 100.", wf(*FRIENDS, banter(mood=101)), error=True)
    case("bad_jokes_over_limit", "Malformed: one running joke more than the limit is refused.",
         wf(*FRIENDS, JOKE, op("joke", a="bo", b="mira", id="second", moment="Another."), limits={"jokes": 1}),
         error=True)
    return cases


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {"beings": rng.randint(2, 5), "pairs": rng.randint(1, 4), "jokes": rng.randint(1, 3), "scenes": rng.randint(1, 2)}
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
PREFIX = [sty("b1", "dry", values=["a", "b"]), sty("b2", "absurd", tempo="fast", values=["a"]),
          sty("b3", "cutting", tempo="slow", formality=90, assertiveness=90), sty("b4", "warm", assertiveness=10),
          op("history", a="b1", b="b2", landmarks=4, favors=3),
          op("joke", a="b1", b="b2", id="j1", moment="The goose."), op("scene", id="s1")]


def rand_op(rng: random.Random, state: Dict[str, Any]) -> Dict[str, Any]:
    roll = rng.random()
    a, b = rng.choice(BEINGS), rng.choice(BEINGS)
    if roll < 0.06:
        state["clock"] += rng.choice([0, 3, 50, 200])
        return op("tick", now=state["clock"] if rng.random() < 0.95 else 0)
    if roll < 0.14:
        x = {"being": a, "humour": rng.choice(HUMOUR + ["sardonic"] * (rng.random() < 0.05)),
             "tempo": rng.choice(TEMPO), "formality": rng.choice([0, 30, 60, 100, 101]),
             "assertiveness": rng.choice([0, 20, 70, 100])}
        if rng.random() < 0.5:
            x["values"] = rng.sample(["a", "b", "c", "d"], rng.randint(0, 3))
        return op("style", **x)
    if roll < 0.20:
        return op("history", a=a, b=b, **{k: rng.choice([0, 1, 3, 9, 1001]) for k in
                                          rng.sample(["landmarks", "favors", "breaches"], rng.randint(0, 3))})
    if roll < 0.26:
        return op("pair", a=a, b=b)
    if roll < 0.32:
        return op("joke", a=a, b=b, id=rng.choice(["j1", "j2", "j3"]), moment=rng.choice(["The goose.", ""]))
    if roll < 0.62:
        x = {"speaker": a, "listener": b, "mood": rng.choice([-100, -31, -30, 0, 20, 100, 101]),
             "pacing": rng.choice(ADVICE + ["calm"] * (rng.random() < 0.03))}
        if rng.random() < 0.7:
            x["seed"] = rng.randint(0, 40)
        if rng.random() < 0.2:
            x["move"] = rng.choice(MOVES)
        return op("banter", **x)
    if roll < 0.66:
        return op("scene", id=rng.choice(["s1", "s2"]), **({"tension": rng.choice([0, 500_000, 900_000])}
                                                            if rng.random() < 0.5 else {}))
    if roll < 0.92:
        return op("beat", scene=rng.choice(["s1", "s2"]), kind=rng.choice(BEATS),
                  tension=rng.choice([-100, -20, 0, 10, 30, 60, 100, 101]), novelty=rng.choice([0, 20, 50, 90]),
                  engagement=rng.choice([10, 40, 70, 100]), threads_open=rng.choice([0, 0, 1, 3, 17]))
    return op("pace", scene=rng.choice(["s1", "s2"]))


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
    state = {"clock": 0}
    target = len(ops) + rng.randint(1, 20)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        ops.append(rand_op(rng, state))
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
    ok_runs = err_runs = moves = callbacks = insults = advices = 0
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
            if "move" in r:
                moves += 1
                callbacks += int(r["move"] == "callback")
                insults += int(r["appraisal"] == "insult")
            advices += int(r.get("advice") is not None and r.get("advice") != "steady")
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "moves": moves, "callbacks": callbacks, "insults": insults, "non_steady_advice": advices,
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
        sys.exit("chemistry checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
