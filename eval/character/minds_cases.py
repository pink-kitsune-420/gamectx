#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 4 stage 2: mind architectures,
local kind `minds`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/minds/*/case.json + input.json
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
from minds_reference import BUILTIN, CHANNELS, DECEPTION, LANGUAGE, MEMORY
from minds_reference import workflow as minds_workflow
from pragmatics_reference import ACTS

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "minds"
LIMIT_NAMES = ["minds", "beings"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "minds":
        raise ValueError("unknown local kind")
    return minds_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "minds", "args": args}


def admit(who: str, kind: str, name: str, /, **args: Any) -> Dict[str, Any]:
    return op("admit", being=who, kind=kind, op=name, args=args)


HIVE = op("assign", being="drone_7", mind="hive_tender", pool="east_hive")
HERD = op("assign", being="nanny_goat", mind="herd_animal")
STEWARD = op("assign", being="keep_steward", mind="steward_intelligence")
HEARTH = op("assign", being="ash_warden", mind="hearth_spirit")
MOSS = op("define", id="moss_bloom", memory="none", affect=["fear"], language="none", deception="none",
          theory_of_mind=0, time_scale=50_000_000)
MOSSY = op("assign", being="cellar_moss", mind="moss_bloom")


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("show_human", "The human mind: every channel, full language and deception, second-order theory of mind.",
         wf(op("show", id="human")))
    case("show_hearth_spirit", "A slow household spirit with a fixation on keeping the household.",
         wf(op("show", id="hearth_spirit")))
    case("define_custom_mind", "An authored mind: channels are sorted, the time scale and fixation are kept.",
         wf(op("define", id="tide_oracle", memory="individual", affect=["joy", "fear"], language="full",
               deception="concealment", theory_of_mind=1, time_scale=5_000_000,
               fixation={"goal": "read_the_tides", "strength": 70}),
            op("show", id="tide_oracle")))
    case("hive_episode_to_pool", "A hive drone's memory goes to its hive pool, without an emotion the hive lacks.",
         wf(HIVE, admit("drone_7", "cueing", "episode", being="drone_7", id="wasp_raid",
                        features={"tags": ["wasps", "east_comb"]}, emotion="shame", strength=80)))
    case("hive_cue_default_cooldown", "A cue with no cooldown gets the engine default, at human speed.",
         wf(HIVE, admit("drone_7", "cueing", "cue", being="drone_7", features={"tags": ["wasps"]})))
    case("hearth_cue_stretched", "A slow spirit's cue cooldown is stretched twentyfold.",
         wf(HEARTH, admit("ash_warden", "cueing", "cue", being="ash_warden", features={"tags": ["smoke"]},
                          cooldown=6)))
    case("steward_resume_fast", "A fast steward intelligence covers an hour in a tenth of the ticks.",
         wf(STEWARD, admit("keep_steward", "pragmatics", "resume", thread="grain_tally", ticks_per_hour=60)))
    case("steward_cover_cannot_lie", "A steward can conceal but not invent a cover story.",
         wf(STEWARD, admit("keep_steward", "cueing", "cover", being="keep_steward", fact="sealed_door",
                           story="nothing behind it")))
    case("herd_cover_two_reasons", "A goat can neither lie nor model what others believe.",
         wf(HERD, admit("nanny_goat", "cueing", "cover", being="nanny_goat", fact="eaten_turnips", story="none")))
    case("mindless_memory_refused", "A moss bloom without memory cannot surface or store memories.",
         wf(MOSS, MOSSY, admit("cellar_moss", "cueing", "surface", being="cellar_moss", cue={"tags": ["damp"]}),
            admit("cellar_moss", "cueing", "episode", being="cellar_moss", id="flood", features={"tags": []})))
    case("herd_parse_signals_only", "A goat takes in tone, not words: word parsing is refused.",
         wf(HERD, admit("nanny_goat", "pragmatics", "parse", text="Come here, you!")))
    case("mindless_speech_no_language", "A moss bloom has no language for conversation threads or voice.",
         wf(MOSS, MOSSY, admit("cellar_moss", "pragmatics", "open", id="t1", topic="damp"),
            admit("cellar_moss", "embodiment", "voice", being="cellar_moss", fear=40)))
    case("herd_stance_no_bystanders", "A mind with no theory of mind ignores who is watching.",
         wf(HERD, admit("nanny_goat", "pragmatics", "stance", speaker="nanny_goat", bystanders=["miller"])))
    case("herd_react_lying_and_channels",
         "A goat cannot lie; an honest reaction keeps fear and anger but loses guilt and the bystander check.",
         wf(HERD, admit("nanny_goat", "embodiment", "react", being="nanny_goat", lying=True, fear=60),
            admit("nanny_goat", "embodiment", "react", being="nanny_goat", lying=False, fear=60, anger=30,
                  guilt=20, bystander_untrusted=True)))
    case("hive_tension_attitudes", "A hive drone's attitudes keep only fear; affection and resentment are stripped.",
         wf(HIVE, admit("drone_7", "embodiment", "tension", being="drone_7", deceiving=False,
                        attitudes=[{"toward": "beekeeper", "affection": 40, "fear": 10},
                                   {"toward": "bear", "resentment": 70, "fear": 90}, "loose"])))
    case("human_passthrough", "An unassigned being is human: requests pass unchanged.",
         wf(admit("miller", "cueing", "cover", being="miller", fact="short_sack", story="the rats"),
            admit("miller", "embodiment", "react", being="miller", lying=True, guilt=70, bystander_untrusted=True)))
    case("fixate_pins_goal", "The spirit's fixation goes first, scored at least as high as any motive, and once only.",
         wf(HEARTH, op("fixate", being="ash_warden", motives=[{"id": "rest", "score": 40},
                                                              {"id": "chase_rats", "score": 95},
                                                              {"id": "keep_the_household", "score": 10}])))
    case("release_then_fixate", "After release (and a tick), motives are left as the host ordered them.",
         wf(HEARTH, op("release", being="ash_warden"), op("tick", now=12),
            op("fixate", being="ash_warden", motives=[{"id": "rest", "score": 40}])))
    case("hear_three_minds", "The same threat is words to a person, a tone to a goat and nothing to moss.",
         wf(HERD, MOSS, MOSSY, op("hear", being="miller", acts=["threat", "question_fact"]),
            op("hear", being="nanny_goat", acts=["threat", "question_fact"]),
            op("hear", being="cellar_moss", acts=["threat"])))
    case("bad_define_builtin", "Malformed: built-in minds cannot be redefined.",
         wf(op("define", id="human", memory="none", affect=[], language="none", deception="none", theory_of_mind=0)),
         error=True)
    case("bad_define_duplicate_channel", "Malformed: an affect channel is listed once.",
         wf(op("define", id="m", memory="none", affect=["fear", "fear"], language="none", deception="none",
               theory_of_mind=0)), error=True)
    case("bad_assign_missing_pool", "Malformed: a shared-memory mind needs a pool.",
         wf(op("assign", being="drone_7", mind="hive_tender")), error=True)
    case("bad_admit_unknown_kind", "Malformed: admit covers cueing, pragmatics and embodiment only.",
         wf(admit("miller", "dreams", "episode")), error=True)
    case("bad_release_without_fixation", "Malformed: only an assigned being whose mind has a fixation is released.",
         wf(op("assign", being="miller", mind="human"), op("release", being="miller")), error=True)
    case("bad_redefine_breaks_pool", "Malformed: a mind cannot become shared while a being holds it without a pool.",
         wf(MOSS, MOSSY, op("define", id="moss_bloom", memory="shared", affect=[], language="none", deception="none",
                            theory_of_mind=0)), error=True)
    case("bad_beings_over_limit", "Malformed: one being more than the limit is refused.",
         wf(HERD, op("assign", being="kid_goat", mind="herd_animal"), limits={"beings": 1}), error=True)
    return cases


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {"minds": rng.randint(1, 3), "beings": rng.randint(1, 6)}
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


MIND_IDS = list(BUILTIN) + ["custom_a", "custom_b", "nowhere"]
BEINGS = ["b1", "b2", "b3", "b4", "b5"]
OPS = {"cueing": ["episode", "cue", "surface", "fact", "cover", "uncover", "frame_facts", "gate", "forget"],
       "pragmatics": ["parse", "validate", "agenda", "open", "interrupt", "resume", "close", "threads", "stance",
                      "lexicon"],
       "embodiment": ["react", "voice", "tension", "appraise"],
       "dreams": ["episode"]}
NUMS = [0, 1, 6, 24, 60, 1_000_000_000_000, 1_000_000_000_001, -3, 2.5, True, None, "24"]
PREFIX = [op("define", id="custom_a", memory="none", affect=["fear", "shame"], language="none", deception="none",
             theory_of_mind=0, time_scale=3),
          op("define", id="custom_b", memory="shared", affect=[], language="signals", deception="full",
             theory_of_mind=2, time_scale=1_000_000_000, fixation={"goal": "hoard", "strength": 1}),
          op("assign", being="b1", mind="hive_tender", pool="p1"), op("assign", being="b2", mind="herd_animal"),
          op("assign", being="b3", mind="custom_a"), op("assign", being="b4", mind="custom_b", pool="p2"),
          op("assign", being="b5", mind="hearth_spirit")]


def rand_args(rng: random.Random, kind: str, name: str, b: str) -> Dict[str, Any]:
    a: Dict[str, Any] = {}
    if rng.random() < 0.8:
        a["being"] = b
    pool = ["emotion", "cooldown", "ticks_per_hour", "bystanders", "lying", "deceiving", "fear", "anger", "shock",
            "guilt", "affection", "bystander_untrusted", "attitudes", "id"]
    for k in rng.sample(pool, rng.randint(0, 6)):
        if k == "emotion":
            a[k] = rng.choice(CHANNELS + ["awe", 3, None])
        elif k in ("cooldown", "ticks_per_hour"):
            a[k] = rng.choice(NUMS)
        elif k == "bystanders":
            a[k] = rng.choice([None, [], ["x"], "x"])
        elif k in ("lying", "deceiving", "bystander_untrusted"):
            a[k] = rng.choice([True, False, None, 1])
        elif k == "attitudes":
            a[k] = rng.choice([None, "x", [{"toward": "t", "affection": 1, "fear": 2, "resentment": 3}, 4,
                                           {"resentment": 9}, {"fear": 1, "affection": 2}]])
        else:
            a[k] = rng.choice([0, 50, 100, "x", None])
    return a


def rand_op(rng: random.Random) -> Dict[str, Any]:
    roll = rng.random()
    b = rng.choice(BEINGS)
    if roll < 0.04:
        return op("tick", now=rng.choice([0, 5, 10, 10, 1_000_000_000_001]))
    if roll < 0.09:
        return op("show", id=rng.choice(MIND_IDS))
    if roll < 0.17:
        a: Dict[str, Any] = {"id": rng.choice(["custom_a", "custom_b", "custom_c", "herd_animal"]),
                             "memory": rng.choice(MEMORY + ["hive"]), "language": rng.choice(LANGUAGE),
                             "deception": rng.choice(DECEPTION),
                             "affect": rng.sample(CHANNELS, rng.randint(0, 4)) + rng.choice([[], [], ["awe"], ["fear"]]),
                             "theory_of_mind": rng.choice([0, 1, 2, 2, 3])}
        if rng.random() < 0.5:
            a["time_scale"] = rng.choice([1, 500_000, 1_000_000, 7_000_000, 1_000_000_000, 0, None])
        if rng.random() < 0.4:
            a["fixation"] = rng.choice([None, {"goal": "g", "strength": rng.choice([1, 50, 100, 0, 101])},
                                        {"goal": "g"}])
        return op("define", **a)
    if roll < 0.32:
        mid = rng.choice(MIND_IDS)
        a = {"being": b, "mind": mid}
        shared = mid in ("hive_tender", "custom_b")
        if (shared and rng.random() < 0.9) or (not shared and rng.random() < 0.1):
            a["pool"] = rng.choice(["p1", "p2"])
        return op("assign", **a)
    if roll < 0.75:
        kind = rng.choice(["cueing", "pragmatics", "embodiment", "cueing", "pragmatics", "embodiment", "dreams"])
        name = rng.choice(OPS[kind])
        return op("admit", being=b, kind=kind, op=name,
                  args=rand_args(rng, kind, name, b) if rng.random() < 0.97 else "x")
    if roll < 0.85:
        ids = rng.sample(["rest", "eat", "hoard", "keep_the_household", "flee"], rng.randint(0, 4))
        ms = [{"id": i, "score": rng.choice([0, 30, 90, 100, 101])} for i in ids]
        if ms and rng.random() < 0.05:
            ms.append(dict(ms[0]))
        return op("fixate", being=b, motives=ms)
    if roll < 0.90:
        return op("release", being=b)
    acts = rng.sample(ACTS, rng.randint(1, 4)) if rng.random() < 0.95 else rng.choice([[], ["sing"]])
    return op("hear", being=b, acts=acts)


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
    target = len(ops) + rng.randint(1, 16)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        ops.append(rand_op(rng))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
    if not ops:
        ops.append(op("show", id="human"))
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
    ok_runs = err_runs = admits = refusals = strips = rerouted = 0
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
            if "allowed" in r:
                admits += 1
                refusals += int(not r["allowed"])
                strips += len(r["stripped"])
                rerouted += int(r["target"] != r["being"])
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "admits": admits, "admit_refusals": refusals, "stripped_inputs": strips, "pool_routes": rerouted,
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
        sys.exit("minds checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
