#!/usr/bin/env python3
"""Conformance cases and differential parity runner for subjective world knowledge
(pass 2, A1, local kind `beliefs`) and communication as action (pass 2, B1, local kind
`speech_act`).

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/beliefs_speech/*/case.json + input.json
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
from beliefs_speech_reference import process_local

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "beliefs_speech"


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    return process_local(request.get("kind"), request.get("args", {}))


def bl(*ops: Dict[str, Any]) -> Dict[str, Any]:
    return {"kind": "beliefs", "args": {"ops": list(ops)}}


def sp(*ops: Dict[str, Any]) -> Dict[str, Any]:
    return {"kind": "speech_act", "args": {"ops": list(ops)}}


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def tri(about: str, relation: str, value: str) -> Dict[str, str]:
    return {"about": about, "relation": relation, "value": value}


MILL = tri("mill", "sells", "flour")
THIEF = tri("thief", "returns", "tonight")


def learn(being: str, t: Dict[str, str], source: str, **extra: Any) -> Dict[str, Any]:
    return op("learn", being=being, source=source, **t, **extra)


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    # ---- beliefs (A1)
    add("beliefs_init_authored_and_query",
        "Authored beliefs default to confidence 80; query filters by relation and confidence.",
        bl(op("init", being="baker", beliefs=[
            {**tri("baker", "can", "bake")},
            {**MILL, "confidence": 40},
            {**tri("well", "has", "water"), "holds": False, "source": "saw"},
        ]), op("query", being="baker", relation="sells"), op("query", being="baker", min_confidence=80)))

    add("beliefs_learn_told_with_from",
        "Hearsay is stored with its teller and the default told confidence of 60.",
        bl(learn("baker", MILL, "told", **{"from": "carter", "at": 4})))

    add("beliefs_reinforce_stronger_source",
        "Matching evidence keeps the highest confidence and upgrades the source.",
        bl(learn("baker", MILL, "told", **{"from": "carter", "at": 4}),
           learn("baker", MILL, "saw", at=9, confidence=70),
           op("query", being="baker")))

    add("beliefs_weak_contradiction_kept",
        "A rumor that contradicts what the being saw is kept out.",
        bl(learn("baker", MILL, "saw", at=2),
           learn("baker", MILL, "told", holds=False, at=5, **{"from": "gossip"}),
           op("query", being="baker")))

    add("beliefs_did_revises_told",
        "First-hand experience revises a told belief.",
        bl(learn("baker", MILL, "told", **{"from": "carter"}),
           learn("baker", MILL, "did", holds=False, at=12)))

    add("beliefs_outcome_success_learns_can",
        "A successful action teaches `<being> can <action>` and confirms relied-on beliefs.",
        bl(learn("apprentice", tri("oven", "needs", "firewood"), "told", **{"from": "baker"}),
           op("outcome", being="apprentice", action="bake", success=True, at=20,
              relied_on=[tri("oven", "needs", "firewood")]),
           op("known_actions", being="apprentice", actions=["bake", "mill"])))

    add("beliefs_outcome_failure_corrects",
        "A failed barter corrects the hearsay it relied on; the being learns no new skill.",
        bl(learn("baker", MILL, "told", **{"from": "carter"}),
           op("outcome", being="baker", action="barter", success=False, at=9, relied_on=[MILL]),
           op("gaps", being="baker", needs=[MILL])))

    add("beliefs_known_actions_filter",
        "Catalog actions split into known and unknown for the planner.",
        bl(op("init", being="smith", beliefs=[tri("smith", "can", "forge"), tri("smith", "can", "repair"),
                                            {**tri("smith", "can", "bake"), "holds": False}]),
           op("known_actions", being="smith", actions=["forge", "bake", "repair", "sew"])))

    add("beliefs_gaps_statuses",
        "Needs report unknown, low_confidence, known_true and known_false; only the first two are gaps.",
        bl(op("init", being="baker", beliefs=[{**MILL, "confidence": 30},
                                            tri("baker", "can", "bake"),
                                            {**tri("well", "has", "water"), "holds": False}]),
           op("gaps", being="baker", needs=[MILL, tri("baker", "can", "bake"), tri("well", "has", "water"),
                                            tri("market", "opens", "dawn")])))

    add("malformed_beliefs_unknown_source",
        "Reject a source outside did/saw/told/authored.",
        bl(learn("baker", MILL, "dreamed")), error=True)

    add("malformed_beliefs_key_too_long",
        "Reject a triple whose about:relation:value key exceeds 64 characters.",
        bl(learn("baker", tri("a" * 30, "sells", "b" * 30), "did")), error=True)

    # ---- speech acts (B1)
    add("speech_inform_moves_belief",
        "Inform passes the speaker's belief at confidence scaled by the listener's trust.",
        sp(learn("miller", MILL, "did"),
           op("relate", speaker="miller", listener="baker", trust=70),
           op("perform", speaker="miller", listener="baker", act="inform", topic=MILL, at=3),
           op("query", being="baker")))

    add("speech_warn_adds_fear",
        "Warn is inform plus fear toward the topic for the listener.",
        sp(learn("guard", THIEF, "saw"),
           op("relate", speaker="guard", listener="innkeeper", trust=80),
           op("perform", speaker="guard", listener="innkeeper", act="warn", topic=THIEF, at=30)))

    add("speech_ask_answered",
        "Asking fills a gap; the answer is weighted by the asker's trust in the listener.",
        sp(learn("miller", MILL, "did"),
           op("relate", speaker="miller", listener="baker", trust=90),
           op("gaps", being="baker", needs=[MILL]),
           op("perform", speaker="baker", listener="miller", act="ask", topic=MILL),
           op("gaps", being="baker", needs=[MILL])))

    add("speech_ask_no_answer",
        "A listener who does not know gives no answer; the asker models that.",
        sp(op("perform", speaker="baker", listener="apprentice", act="ask", topic=MILL, at=7)))

    add("speech_request_and_persuade",
        "Request complies at trust >= 40; persuade uses (trust + strength) / 2 >= 50.",
        sp(op("relate", speaker="baker", listener="apprentice", trust=45),
           op("perform", speaker="baker", listener="apprentice", act="request", goal="fetch_flour"),
           op("relate", speaker="baker", listener="carter", trust=20),
           op("perform", speaker="baker", listener="carter", act="request", goal="haul_flour"),
           op("perform", speaker="baker", listener="carter", act="persuade", goal="haul_flour", strength=90)))

    add("speech_command_needs_authority",
        "check and perform report no_authority; with authority 60 the command complies.",
        sp(op("check", speaker="captain", listener="guard", act="command", goal="hold_gate"),
           op("perform", speaker="captain", listener="guard", act="command", goal="hold_gate"),
           op("relate", speaker="captain", listener="guard", authority=60),
           op("perform", speaker="captain", listener="guard", act="command", goal="hold_gate", at=2),
           op("assess", being="guard")))

    add("speech_threaten_drops_trust",
        "Threat fear is 30 + strength/2; trust falls by 20, saturating at 0.",
        sp(op("relate", speaker="bandit", listener="carter", trust=10),
           op("perform", speaker="bandit", listener="carter", act="threaten", goal="hand_over_cart", strength=60),
           op("perform", speaker="bandit", listener="carter", act="threaten", goal="leave_town", strength=20)))

    add("speech_comfort_raises_trust",
        "Comfort eases fear by trust/2, adds gratitude trust/4 and +5 trust (capped at 100).",
        sp(op("relate", speaker="baker", listener="apprentice", trust=98),
           op("perform", speaker="baker", listener="apprentice", act="comfort"),
           op("relate", speaker="stranger", listener="apprentice", trust=10),
           op("perform", speaker="stranger", listener="apprentice", act="comfort")))

    add("speech_plan_learn_asks_most_trusted",
        "To learn, ask whom the speaker trusts most (ties by name); a known topic needs no plan.",
        sp(op("relate", speaker="miller", listener="baker", trust=70),
           op("relate", speaker="carter", listener="baker", trust=70),
           op("relate", speaker="gossip", listener="baker", trust=20),
           op("plan", speaker="baker", intent="learn", topic=MILL, candidates=["gossip", "miller", "carter"]),
           learn("baker", MILL, "saw"),
           op("plan", speaker="baker", intent="learn", topic=MILL, candidates=["miller"])))

    add("speech_plan_get_help_picks_best",
        "get_help chooses command, request or persuade per listener and keeps the best; a distrusted listener only gets an unlikely persuade.",
        sp(op("relate", speaker="baker", listener="apprentice", authority=55, trust=30),
           op("relate", speaker="baker", listener="miller", trust=75),
           op("plan", speaker="baker", intent="get_help", goal="carry_sacks", candidates=["apprentice", "miller", "carter"]),
           op("relate", speaker="baker", listener="carter", trust=20),
           op("plan", speaker="baker", intent="get_help", goal="carry_sacks", candidates=["carter"], strength=20)))

    add("speech_plan_share_alarm_reassure",
        "share/alarm address every candidate; reassure picks the most trusting listener.",
        sp(learn("guard", THIEF, "saw"),
           op("relate", speaker="guard", listener="innkeeper", trust=80),
           op("plan", speaker="guard", intent="alarm", topic=THIEF, candidates=["innkeeper", "baker"]),
           op("plan", speaker="guard", intent="share", topic=MILL, candidates=["baker"]),
           op("plan", speaker="guard", intent="reassure", candidates=["baker", "innkeeper"]),
           op("relate", speaker="guard", listener="baker", trust=10),
           op("relate", speaker="guard", listener="innkeeper", trust=20),
           op("plan", speaker="guard", intent="reassure", candidates=["baker", "innkeeper"])))

    add("speech_assess_pairs_and_wants",
        "assess lists who the being trusts, its wants (higher priority wins) and belief count.",
        sp(op("relate", speaker="baker", listener="apprentice", trust=60),
           op("relate", speaker="miller", listener="apprentice", trust=40, authority=10),
           op("perform", speaker="baker", listener="apprentice", act="request", goal="sweep_floor"),
           op("perform", speaker="miller", listener="apprentice", act="request", goal="sweep_floor"),
           learn("apprentice", tri("oven", "is", "hot"), "saw"),
           op("assess", being="apprentice")))

    add("malformed_speech_self_address",
        "Reject a speaker addressing itself.",
        sp(op("perform", speaker="baker", listener="baker", act="comfort")), error=True)

    add("malformed_speech_act_field_mismatch",
        "Reject a goal on inform (acts take only their own fields).",
        sp(learn("miller", MILL, "did"),
           op("perform", speaker="miller", listener="baker", act="inform", topic=MILL, goal="bake")), error=True)

    return cases


BEINGS = ["baker", "miller", "carter", "guard", "apprentice"]
ABOUTS = ["mill", "well", "oven", "thief", "market"]
RELS = ["sells", "has", "needs", "opens", "returns"]
VALUES = ["flour", "water", "firewood", "dawn", "tonight"]
ACTIONS = ["bake", "barter", "mill", "haul", "sweep"]
GOALS = ["fetch_flour", "hold_gate", "sweep_floor", "carry_sacks"]


def rtri(rng: random.Random) -> Dict[str, str]:
    return tri(rng.choice(ABOUTS), rng.choice(RELS), rng.choice(VALUES))


def rlearn(rng: random.Random, being: str) -> Dict[str, Any]:
    a: Dict[str, Any] = {"being": being, **rtri(rng), "source": rng.choice(["did", "saw", "told", "authored"])}
    if rng.random() < 0.4:
        a["holds"] = rng.random() < 0.6
    if rng.random() < 0.4:
        a["confidence"] = rng.randint(0, 100)
    if rng.random() < 0.3:
        a["from"] = rng.choice(BEINGS)
    if rng.random() < 0.5:
        a["at"] = rng.randint(0, 100)
    if rng.random() < 0.02:
        a["source"] = "dreamed"
    return op("learn", **a)


def rbelief_op(rng: random.Random) -> Dict[str, Any]:
    b = rng.choice(BEINGS)
    roll = rng.random()
    if roll < 0.45:
        return rlearn(rng, b)
    if roll < 0.55:
        bs = []
        for _ in range(rng.randint(0, 5)):
            x: Dict[str, Any] = dict(rtri(rng))
            if rng.random() < 0.3:
                x["holds"] = rng.random() < 0.5
            if rng.random() < 0.3:
                x["source"] = rng.choice(["did", "saw", "told", "authored"])
            if rng.random() < 0.3:
                x["confidence"] = rng.randint(0, 100)
            bs.append(x)
        return op("init", being=b, beliefs=bs)
    if roll < 0.7:
        return op("outcome", being=b, action=rng.choice(ACTIONS), success=rng.random() < 0.5,
                  at=rng.randint(0, 100), relied_on=[rtri(rng) for _ in range(rng.randint(0, 2))])
    if roll < 0.8:
        q: Dict[str, Any] = {"being": b}
        if rng.random() < 0.4:
            q["about"] = rng.choice(ABOUTS)
        if rng.random() < 0.3:
            q["min_confidence"] = rng.randint(0, 100)
        return op("query", **q)
    if roll < 0.9:
        return op("known_actions", being=b, actions=rng.sample(ACTIONS, rng.randint(0, 4)))
    g: Dict[str, Any] = {"being": b, "needs": [rtri(rng) for _ in range(rng.randint(0, 4))]}
    if rng.random() < 0.4:
        g["min_confidence"] = rng.randint(0, 100)
    return op("gaps", **g)


def rand_beliefs(rng: random.Random) -> Dict[str, Any]:
    return bl(*[rbelief_op(rng) for _ in range(rng.randint(1, 8))])


def rand_speech(rng: random.Random) -> Dict[str, Any]:
    ops = []
    for _ in range(rng.randint(1, 10)):
        roll = rng.random()
        s, l = rng.sample(BEINGS, 2)
        if roll < 0.3:
            ops.append(rbelief_op(rng))
        elif roll < 0.45:
            r: Dict[str, Any] = {"speaker": s, "listener": l}
            if rng.random() < 0.8:
                r["trust"] = rng.randint(0, 100)
            if rng.random() < 0.4:
                r["authority"] = rng.randint(0, 100)
            ops.append(op("relate", **r))
        elif roll < 0.8:
            act = rng.choice(["ask", "command", "comfort", "inform", "persuade", "request", "threaten", "warn"])
            a: Dict[str, Any] = {"speaker": s, "listener": l, "act": act}
            if act in ("ask", "inform", "warn"):
                a["topic"] = rtri(rng)
            if act in ("command", "persuade", "request", "threaten"):
                a["goal"] = rng.choice(GOALS)
            if act in ("persuade", "threaten") and rng.random() < 0.7:
                a["strength"] = rng.randint(0, 100)
            if rng.random() < 0.4:
                a["at"] = rng.randint(0, 100)
            if rng.random() < 0.03:
                a["goal" if "topic" in a else "topic"] = rng.choice(GOALS) if "topic" in a else rtri(rng)
            ops.append(op("check" if rng.random() < 0.2 else "perform", **a))
        elif roll < 0.93:
            intent = rng.choice(["alarm", "get_help", "learn", "reassure", "share"])
            p: Dict[str, Any] = {"speaker": s, "intent": intent,
                                 "candidates": rng.sample([b for b in BEINGS if b != s], rng.randint(1, 4))}
            if intent in ("alarm", "learn", "share"):
                p["topic"] = rtri(rng)
            if intent == "get_help":
                p["goal"] = rng.choice(GOALS)
                if rng.random() < 0.5:
                    p["strength"] = rng.randint(0, 100)
            ops.append(op("plan", **p))
        else:
            ops.append(op("assess", being=rng.choice(BEINGS)))
    return sp(*ops)


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
    ok_runs = err_runs = acts = 0
    for i in range(1, count + 1):
        req = rand_beliefs(rng) if rng.random() < 0.5 else rand_speech(rng)
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
        acts += proc.stdout.count('"performed":true')
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "acts_performed": acts, "actual_model_calls": 0, "api_spend_usd": 0,
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
        sys.exit("beliefs/speech checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
