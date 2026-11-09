#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 3 stage 3: the pragmatic parser
(F1), mixed-initiative agenda and thread stack (F2) and conversational stance (F3), local kind
`pragmatics`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/pragmatics/*/case.json + input.json
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
from pragmatics_reference import ACTS, SUBTEXTS
from pragmatics_reference import workflow as pragmatics_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "pragmatics"
LIMIT_NAMES = ["beings", "threads", "phrases", "own"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "pragmatics":
        raise ValueError("unknown local kind")
    return pragmatics_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "pragmatics", "args": args}


def parse(text: str, **a: Any) -> Dict[str, Any]:
    return op("parse", text=text, **a)


def cl(i: int, act: str, sub: str = "sincere") -> Dict[str, Any]:
    return {"index": i, "act": act, "subtext": sub}


THREAD = op("open", being="mira", id="theft_talk", topic="apprentice", speaker="player",
            question="Did the boy take the pouch?")


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("parse_firewood_and_question",
         "Two clauses split at the dash: a transfer to receive and a probing question about the apprentice.",
         wf(parse("Here is the firewood you asked for—also, is it true the apprentice stole from you?",
                  speaker="player", listener="mira")))
    case("parse_apology_transfer_question",
         "An apology, a hand-over and a question, each with its own obligation.",
         wf(parse("Sorry I'm late with the flour. Here it is. Did anyone come looking for me?")))
    case("parse_sarcastic_praise",
         "Praising bread the context says was burned reads as sarcasm.",
         wf(parse("Well done on the bread.", context={"failed": ["bread"]})))
    case("parse_testing_honesty",
         "Asking about a fire the speaker saw is a test of honesty, not a plain question.",
         wf(parse("Did you see the fire last night?", context={"witnessed": ["fire"]})))
    case("parse_threat_beats_request",
         "A threat outranks the request it is wrapped in.",
         wf(parse("Help me carry this or else.")))
    case("parse_accusation",
         "Naming the listener as the thief is an accusation to appraise and answer.",
         wf(parse("You stole my pouch, thief!")))
    case("parse_overflow_four_clauses",
         "Six sentences: the first four are kept and two are counted as overflow.",
         wf(parse("Hello. Nice weather. The mill is busy. Hurry now! Thanks. Goodbye.")))
    case("parse_guilt_and_bargain",
         "Guilt-tripping and bargaining subtexts.",
         wf(parse("After everything I did for you. If you lend me the cart I will pay.")))
    case("lexicon_author_phrase",
         "An author adds 'begone' as a farewell and it is recognised.",
         wf(op("lexicon", act="farewell", phrases=["begone", "be off with you"]), parse("Begone, stranger.")))
    case("validate_model_clauses",
         "Model-proposed clauses found in order in the utterance are accepted with their obligations.",
         wf(op("validate", text="Sorry I'm late. Here it is.", clauses=[
             {"text": "Sorry I'm late", "act": "apology", "subtext": "sincere"},
             {"text": "Here it is", "act": "offer_or_transfer", "subtext": "sincere"}])))
    case("agenda_override",
         "With the bread burning (motive 92) the item is still received but both replies are deferred.",
         wf(op("agenda", clauses=[cl(0, "offer_or_transfer"), cl(1, "question_fact", "probing")],
               motive={"id": "save_bread", "score": 92})))
    case("agenda_counter_ask",
         "The question is answered first, then the smith's own question about the borrowed cart.",
         wf(op("agenda", clauses=[cl(0, "question_fact")], motive={"id": "work", "score": 40},
               own=[{"id": "where_is_cart", "kind": "ask", "priority": 70},
                    {"id": "small_debt", "kind": "followup", "priority": 50}])))
    case("agenda_grievance_gate",
         "A strong grievance withholds the plain question until it is addressed; the urgent plea is still answered.",
         wf(op("agenda", clauses=[cl(0, "question_fact"), cl(1, "request_help", "urgent")],
               own=[{"id": "broken_cart", "kind": "grievance", "priority": 85}])))
    case("agenda_respond_plain",
         "Nothing of its own to raise: the character just responds.",
         wf(op("agenda", clauses=[cl(0, "greeting"), cl(1, "small_talk")],
               own=[{"id": "idle_thought", "kind": "ask", "priority": 30}])))
    case("threads_interrupt_and_resume",
         "A crash interrupts the theft talk; ten minutes later it resumes with its unanswered question.",
         wf(THREAD, op("tick", now=100), op("interrupt", being="mira", cause="crash_outside"), op("tick", now=110),
            op("resume", being="mira", ticks_per_hour=60), op("threads", being="mira")))
    case("threads_stack_drops_oldest",
         "With room for two threads, opening a third drops the oldest.",
         wf(THREAD, op("open", being="mira", id="bread", topic="oven", speaker="mira"),
            op("open", being="mira", id="weather", topic="rain", speaker="player"), limits={"threads": 2}))
    case("threads_close_reactivates",
         "Closing the top thread makes the one below it active again.",
         wf(THREAD, op("open", being="mira", id="bread", topic="oven", speaker="mira"),
            op("close", being="mira", id="bread"), op("threads", being="mira")))
    case("stance_privacy_and_brevity",
         "With the gossip in earshot a confidential reply is deflected at low trust and whispered at high trust; urgency cuts it to one sentence; resentment makes a hostile refusal.",
         wf(op("stance", trust=40, affection=30, confidential=True, bystanders=[{"being": "gossip"}], urgency=90),
            op("stance", trust=70, confidential=True, bystanders=[{"being": "gossip"}, {"being": "wife", "trusted": True}]),
            op("stance", trust=-30, resentment=60, urgency=50, deadline_hours=1)))

    case("bad_validate_text_not_in_utterance",
         "A proposed clause must be found in the utterance.",
         wf(op("validate", text="Here it is.", clauses=[{"text": "I stole it", "act": "accusation",
                                                          "subtext": "sincere"}])), error=True)
    case("bad_validate_unknown_act",
         "Acts come from the fixed list.",
         wf(op("validate", text="Here it is.", clauses=[{"text": "Here it is", "act": "bribe",
                                                          "subtext": "sincere"}])), error=True)
    case("bad_agenda_duplicate_index",
         "Clause indexes are unique.",
         wf(op("agenda", clauses=[cl(0, "greeting"), cl(0, "farewell")])), error=True)
    case("bad_resume_not_interrupted",
         "Only an interrupted thread can be resumed.",
         wf(THREAD, op("resume", being="mira")), error=True)
    case("bad_open_duplicate_thread",
         "Thread ids are unique per being.",
         wf(THREAD, THREAD), error=True)
    case("bad_lexicon_not_normalized",
         "Phrases are lower-case words separated by single spaces.",
         wf(op("lexicon", act="farewell", phrases=["Begone!"])), error=True)
    case("bad_text_only_punctuation",
         "A turn needs at least one word.",
         wf(parse("...?!")), error=True)
    return cases


TEXTS = ["Here is the firewood—also, is it true the apprentice stole from you?", "Sorry. Here it is.",
         "Hello there! How are you, really?", "You stole it, liar.", "Help me now!", "Help me or else.",
         "Well done on the bread.", "After everything I did. If you help me.", "Maybe. I don't know.",
         "Did you see the fire?", "Nice weather; the mill is busy. Thanks. Goodbye. See you. Hi.", "...",
         "Begone, stranger!", "Lend me a hand, friend.", "Café time? Yes.", "Good morning—what news?"]
WORDS = ["bread", "fire", "mill", "cart"]


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {n: rng.randint(1, 4) for n in rng.sample(LIMIT_NAMES, rng.randint(1, 2))}
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
    roll = rng.random()
    b = rng.choice(["mira", "bo"])
    if roll < 0.25:
        a: Dict[str, Any] = {"text": rng.choice(TEXTS)}
        if rng.random() < 0.4:
            a["context"] = {k: rng.sample(WORDS, rng.randint(0, 2)) for k in rng.sample(["witnessed", "failed"], 1)}
        if rng.random() < 0.3:
            a["speaker"] = "player"
        return op("parse", **a)
    if roll < 0.32:
        return op("lexicon", act=rng.choice(["farewell", "greeting", "threat", "small_talk"]),
                  phrases=rng.sample(["begone", "be off", "Begone!", "or else", "mind your back", "ahoy"],
                                     rng.randint(1, 2)))
    if roll < 0.4:
        txt = rng.choice(TEXTS)
        parts = [p for p in ["Sorry", "Here it is", "Help me", "Hello there", "the mill", "Goodbye"]
                 if rng.random() < 0.5]
        return op("validate", text=txt, clauses=[
            {"text": p, "act": rng.choice(ACTS + ["bribe"]), "subtext": rng.choice(SUBTEXTS)} for p in parts])
    if roll < 0.56:
        cls = [{"index": i, "act": rng.choice(ACTS), "subtext": rng.choice(SUBTEXTS)}
               for i in rng.sample(range(5), rng.randint(0, 3))]
        a = {"clauses": cls}
        if rng.random() < 0.5:
            a["motive"] = {"id": "work", "score": rng.choice([40, 89, 90, 100])}
        if rng.random() < 0.6:
            a["own"] = [{"id": i, "kind": rng.choice(["ask", "followup", "grievance", "gift"]),
                         "priority": rng.choice([30, 60, 80, 100])} for i in rng.sample(["o1", "o2", "o3"],
                                                                                        rng.randint(0, 3))]
        return op("agenda", **a)
    if roll < 0.62:
        return op("tick", now=t)
    if roll < 0.72:
        a = {"being": b, "id": rng.choice(["t1", "t2", "t3", "t4", "t5"]), "topic": "talk",
             "speaker": rng.choice(["player", "mira"])}
        if rng.random() < 0.5:
            a["question"] = rng.choice(["Did he take it?", ""])
        return op("open", **a)
    if roll < 0.78:
        return op("interrupt", being=b, cause=rng.choice(["crash", "customer"]))
    if roll < 0.84:
        a = {"being": b}
        if rng.random() < 0.5:
            a["ticks_per_hour"] = rng.choice([1, 60])
        return op("resume", **a)
    if roll < 0.88:
        return op("close", being=b, id=rng.choice(["t1", "t2", "t3"]))
    if roll < 0.9:
        return op("threads", being=b)
    a = {"trust": rng.choice([-100, -30, 0, 40, 60, 100, 101])}
    for k, v in [("affection", rng.randint(0, 100)), ("resentment", rng.randint(0, 100)),
                 ("confidential", rng.random() < 0.5), ("urgency", rng.choice([0, 29, 30, 59, 60, 84, 85, 100])),
                 ("deadline_hours", rng.choice([0, 1, 2, 48]))]:
        if rng.random() < 0.4:
            a[k] = v
    if rng.random() < 0.5:
        a["bystanders"] = [{"being": w, **({"trusted": rng.random() < 0.5} if rng.random() < 0.5 else {}),
                            **({"earshot": rng.random() < 0.7} if rng.random() < 0.5 else {})}
                           for w in rng.sample(["gossip", "wife", "guard"], rng.randint(0, 3))]
    return op("stance", **a)


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
        t += rng.choice([0, 1, 10, 600])
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
    ok_runs = err_runs = clauses = gated = overrides = resumed = 0
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
            if "clauses" in r and "overflow" in r:
                clauses += len(r["clauses"])
            elif "mode" in r:
                gated += int(r["mode"] == "conditional_gate")
                overrides += int(r["mode"] == "interrupt_override")
            elif "unanswered" in r:
                resumed += 1
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "clauses_parsed": clauses, "conditional_gates": gated, "overrides": overrides, "threads_resumed": resumed,
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
        sys.exit("pragmatics checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
