#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 3 stage 4: ambivalence (G1),
embodied micro-reactions (G2) and voice modulation (G3), local kind `embodiment`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/embodiment/*/case.json + input.json
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
from embodiment_reference import COOP, MODES
from embodiment_reference import workflow as embodiment_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "embodiment"
LIMIT_NAMES = ["beings", "attitudes"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "embodiment":
        raise ValueError("unknown local kind")
    return embodiment_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "embodiment", "args": args}


def motives(top: str, td: str, ts: int, ru: str, rd: str, rs: int) -> Dict[str, Any]:
    return {"top": {"id": top, "domain": td, "score": ts}, "runner_up": {"id": ru, "domain": rd, "score": rs}}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("tension_bittersweet",
         "Mira is fond of the apprentice (55) and freshly hurt (50): bittersweet warmth, tender tone.",
         wf(op("tension", being="mira", attitudes=[{"toward": "apprentice", "affection": 55, "resentment": 50}])))
    case("tension_resentful_obligation",
         "When resentment outweighs affection the tension is resentful obligation, curt.",
         wf(op("tension", being="tomas", attitudes=[{"toward": "brother", "affection": 45, "resentment": 70}])))
    case("tension_respect_and_fear",
         "Respect and fear of the same lord leave the guard torn.",
         wf(op("tension", being="guard", attitudes=[{"toward": "lord", "respect": 60, "fear": 55},
                                                    {"toward": "cook", "affection": 80}])))
    case("tension_motive_near_tie",
         "Fleeing (self-preservation, 70) and keeping a promise (62) are 8 apart in different domains: torn hesitation.",
         wf(op("tension", being="bo", motives=motives("flee", "self_preservation", 70, "stay", "promise", 62))))
    case("tension_same_domain_no_tie",
         "Two close motives from the same domain are not a conflict.",
         wf(op("tension", being="bo", motives=motives("eat_bread", "hunger", 50, "eat_soup", "hunger", 45))))
    case("tension_duress_reluctant",
         "Breaking a norm under duress is reluctant compliance with extra failure risk.",
         wf(op("tension", being="bo", norm_breach={"norm": "no_theft", "severity": 60, "duress": True})))
    case("tension_deceiving_guilty",
         "Lying while fond of the person outranks the warmth: guilty concealment, evasive.",
         wf(op("tension", being="mira", deceiving=True,
               attitudes=[{"toward": "apprentice", "affection": 55, "resentment": 50}])))
    case("react_mira_bread",
         "Bread burning and a gossip watching: Mira keeps working and watches the bystander.",
         wf(op("react", being="mira", task="urgent", bystander_untrusted=True, cooperativeness="guarded")))
    case("react_fear_steps_back",
         "Fear steps back and glances at the exit.",
         wf(op("react", being="ana", fear=80)))
    case("react_hostile_silent_turn",
         "Hostile and shocked over a broken heirloom she holds: she turns away, stares at it, sets down her tool and says nothing.",
         wf(op("react", being="mira", shock=85, cooperativeness="hostile_refusal", task="active", holding="heirloom")))
    case("react_lying_avoids_eyes",
         "A liar avoids eye contact and offers the item.",
         wf(op("react", being="bo", lying=True, offering="pouch", cooperativeness="eager")))
    case("react_threat_reaches_weapon",
         "Anger with a threat present goes for a weapon and squares the shoulders.",
         wf(op("react", being="guard", anger=80, threat_present=True, cooperativeness="deflecting")))
    case("react_repeat_within_window",
         "The same reaction twice within three ticks is reported as repeated.",
         wf(op("react", being="ana", fear=80), op("tick", now=2), op("react", being="ana", fear=75)))
    case("react_repeat_window_passes",
         "After the window the same reaction is fresh again.",
         wf(op("react", being="ana", fear=80), op("tick", now=10), op("react", being="ana", fear=75)))
    case("voice_terse_clipped",
         "Fatigue of 80 compresses the reply to one clipped sentence.",
         wf(op("voice", fatigue=80, base_sentences=5, base_verbosity="full")))
    case("voice_breathless_exertion",
         "Running hard makes the voice breathless.",
         wf(op("voice", exertion=85, fear=90)))
    case("voice_hesitant_whisper",
         "A whispered lie is hesitant and quiet.",
         wf(op("voice", deceiving=True, privacy="whispered")))
    case("voice_warm_relaxed",
         "Affection in a safe place relaxes the voice; full verbosity is kept.",
         wf(op("voice", affection=70, safe_place=True, base_verbosity="full")))

    case("bad_runner_up_above_top",
         "The runner-up cannot outscore the top motive.",
         wf(op("tension", being="bo", motives=motives("a", "x", 10, "b", "y", 20))), error=True)
    case("bad_same_motive",
         "The top motive and runner-up differ.",
         wf(op("tension", being="bo", motives=motives("a", "x", 20, "a", "y", 10))), error=True)
    case("bad_unknown_tension",
         "Tension modes come from the fixed list.",
         wf(op("react", being="bo", tension="angsty")), error=True)
    case("bad_unknown_cooperativeness",
         "Cooperativeness comes from the stance list.",
         wf(op("react", being="bo", cooperativeness="friendly")), error=True)
    case("bad_attitude_duplicate",
         "One attitude per person.",
         wf(op("tension", being="bo", attitudes=[{"toward": "a", "affection": 1}, {"toward": "a", "fear": 2}])),
         error=True)
    case("bad_severity_zero",
         "A norm breach has severity 1-100.",
         wf(op("tension", being="bo", norm_breach={"norm": "n", "severity": 0})), error=True)
    case("bad_base_sentences_too_many",
         "A base of at most 8 sentences.",
         wf(op("voice", base_sentences=9)), error=True)
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


def lvl(rng: random.Random) -> int:
    return rng.choice([0, 39, 40, 49, 50, 59, 60, 69, 70, 79, 80, 89, 90, 100, rng.randint(0, 101)])


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    b = rng.choice(["mira", "bo", "ana"])
    roll = rng.random()
    if roll < 0.08:
        return op("tick", now=t)
    if roll < 0.4:
        a: Dict[str, Any] = {"being": b}
        if rng.random() < 0.5:
            ts = rng.randint(0, 100)
            a["motives"] = motives(rng.choice(["flee", "stay"]), rng.choice(["fear", "promise"]), ts,
                                   rng.choice(["stay", "eat"]), rng.choice(["promise", "hunger", "fear"]),
                                   max(0, ts - rng.choice([0, 5, 12, 13, 30, -1])))
        if rng.random() < 0.5:
            a["attitudes"] = [{"toward": w, **{k: lvl(rng) for k in rng.sample(["affection", "resentment", "respect",
                                                                                 "fear"], rng.randint(0, 4))}}
                              for w in rng.sample(["apprentice", "lord", "cook", "brother"], rng.randint(0, 3))]
        if rng.random() < 0.3:
            a["norm_breach"] = {"norm": "no_theft", "severity": rng.choice([0, 1, 60, 100]),
                                **({"duress": rng.random() < 0.5} if rng.random() < 0.7 else {})}
        if rng.random() < 0.3:
            a["deceiving"] = rng.random() < 0.5
        return op("tension", **a)
    if roll < 0.72:
        a = {"being": b}
        for k in ["fear", "anger", "shock", "guilt"]:
            if rng.random() < 0.4:
                a[k] = lvl(rng)
        for k, v in [("lying", rng.random() < 0.5), ("cooperativeness", rng.choice(COOP + ["friendly"])),
                     ("tension", rng.choice(MODES)), ("task", rng.choice(["none", "active", "urgent", "idle"])),
                     ("holding", "heirloom"), ("offering", "pouch"), ("threat_present", rng.random() < 0.5),
                     ("bystander_untrusted", rng.random() < 0.5), ("window", rng.choice([0, 3, 50]))]:
            if rng.random() < 0.3:
                a[k] = v
        return op("react", **a)
    a = {}
    for k in ["fatigue", "pain", "arousal", "task_urgency", "fear", "affection", "exertion"]:
        if rng.random() < 0.3:
            a[k] = lvl(rng)
    for k, v in [("safe_place", rng.random() < 0.5), ("deceiving", rng.random() < 0.5),
                 ("tension", rng.choice(MODES)), ("base_sentences", rng.choice([1, 4, 8, 9])),
                 ("base_verbosity", rng.choice(["terse", "normal", "full", "chatty"])),
                 ("privacy", rng.choice(["open", "whispered", "deflect_due_to_bystanders"]))]:
        if rng.random() < 0.3:
            a[k] = v
    return op("voice", **a)


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = []
    t = 0
    target = rng.randint(1, 20)
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
    ok_runs = err_runs = tense = silent = repeats = terse = 0
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
            if not isinstance(r, dict):
                continue
            if "mode" in r:
                tense += int(r["mode"] != "none")
            elif "silent" in r:
                silent += int(r["silent"])
                repeats += int(bool(r["repeated"]))
            elif "pacing" in r:
                terse += int(r["terse"])
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "tensions": tense, "silent_turns": silent, "repeated_reactions": repeats, "terse_voices": terse,
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
        sys.exit("embodiment checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
