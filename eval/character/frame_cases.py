#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 3 stage 1: the unified turn frame
(E1), trajectory sense (E4) and salience ranker (E2), local kind `turn-frame`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/turn-frame/*/case.json + input.json
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
from frame_reference import workflow as frame_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "turn-frame"
LIMIT_NAMES = ["beings", "keys", "samples", "items"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "turn-frame":
        raise ValueError("unknown local kind")
    return frame_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "turn-frame", "args": args}


def obs(b: str, k: str, v: int) -> Dict[str, Any]:
    return op("observe", being=b, key=k, value=v)


MIRA_FRAME = op("frame", being="mira", listener="player", place="bakery", ticks_per_hour=60,
                bystanders=[{"being": "gossip", "earshot": True}],
                body={"fatigue": 75, "needs": [{"id": "hunger", "level": 40}], "held": "peel", "posture": "bent"},
                motive={"id": "save_bread", "score": 92, "reason": "Bread burns in a moment", "runner_up": "talk",
                        "runner_up_score": 40},
                task={"id": "pull_bread", "step": "open_oven", "progress": 80, "deadline": 501, "interrupt": "never"},
                affect={"valence": -20, "arousal": 70, "label": "harried",
                        "emotions": [{"emotion": "affection", "toward": "apprentice", "value": 55},
                                     {"emotion": "resentment", "toward": "apprentice", "value": 50}]},
                listener_model={"stance": "friendly", "trust": 40, "knows": ["firewood_order"],
                                "overdue": ["firewood_delivery"]},
                offscreen={"summary": "Counted the till and found the pouch gone", "at": 440},
                novelty={"id": "flour_trail", "score": 70})
MIRA_ITEMS = [
    {"id": "bread_burning", "kind": "fact", "bytes": 60, "urgency": 95, "tags": ["oven"], "mandatory": True},
    {"id": "firewood_promise", "kind": "promise", "bytes": 80, "entities": ["player"], "tags": ["firewood"],
     "at": 499},
    {"id": "flour_trail", "kind": "episode", "bytes": 70, "entities": ["apprentice"], "tags": ["theft"],
     "group": "theft", "polarity": "grievance", "at": 500},
    {"id": "missing_pouch", "kind": "fact", "bytes": 50, "tags": ["theft"], "group": "theft", "at": 500},
    {"id": "apprentice_kindness", "kind": "episode", "bytes": 60, "entities": ["apprentice"], "polarity": "favor"},
    {"id": "gossip_untrusted", "kind": "attitude", "bytes": 40, "entities": ["gossip"], "urgency": 40},
    {"id": "shop_founding", "kind": "lore", "bytes": 200, "tags": ["history"]},
    {"id": "grandmother_recipe", "kind": "biography", "bytes": 150, "tags": ["family"]},
]
MIRA_RANK = op("rank", budget=360, listener="player", earshot=["gossip"],
               cues={"entities": ["apprentice"], "tags": ["theft", "firewood"]}, task_needs=["oven"],
               mood={"valence": -20, "resentment": 50}, items=MIRA_ITEMS)


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("frame_trust_acute_drop",
         "Trust falling 50 within the window is an acute drop (shock), not settled enmity.",
         wf(obs("mira", "trust_apprentice", 40), op("tick", now=500), obs("mira", "trust_apprentice", -10),
            op("trend", being="mira", key="trust_apprentice")))
    case("frame_trust_settled_enmity",
         "Trust of -45 that has not moved for 600 ticks is settled and cold.",
         wf(obs("tomas", "trust_player", -45), op("tick", now=600), op("trend", being="tomas", key="trust_player")))
    case("frame_trend_window",
         "A short window sees a rise of 20 (rising); a long one sees 40 (acute rise).",
         wf(obs("ana", "mood", 0), op("tick", now=10), obs("ana", "mood", 20), op("tick", now=20),
            obs("ana", "mood", 40), op("trend", being="ana", key="mood", window=10),
            op("trend", being="ana", key="mood", window=20)))
    case("frame_samples_window_limit",
         "With two samples kept, the oldest value is forgotten and the baseline moves.",
         wf(obs("ana", "mood", -50), op("tick", now=10), obs("ana", "mood", 0), op("tick", now=20),
            obs("ana", "mood", 10), op("trend", being="ana", key="mood", window=100), limits={"samples": 2}))
    case("frame_streak_frustration",
         "Three failures in a row are a frustration spiral.",
         wf(op("outcome", being="bo", ok=False), op("outcome", being="bo", ok=False),
            op("outcome", being="bo", ok=False), op("streak", being="bo")))
    case("frame_streak_flow_resets",
         "A success resets a failing streak; three successes are flow.",
         wf(op("outcome", being="bo", ok=False), op("outcome", being="bo", ok=False),
            op("outcome", being="bo", ok=True), op("outcome", being="bo", ok=True),
            op("outcome", being="bo", ok=True)))
    case("frame_time_labels",
         "Minute ticks: now, 10 minutes, 3 hours, 20 hours, 30 hours, 5 days and 30 days ago.",
         wf(op("tick", now=100000),
            *[op("time", at=100000 - d, ticks_per_hour=60) for d in [0, 10, 180, 1200, 1800, 7200, 43200]]))
    case("frame_time_due",
         "A deadline ahead is due in hours; one behind is overdue by hours.",
         wf(op("tick", now=50), op("time", at=55, due=True), op("time", at=40, due=True)))
    case("frame_mira_turn",
         "Mira mid-task: fatigue is urgent, the bread motive overrides talk, the task is pressing and the gossip is an untrusted ear.",
         wf(obs("mira", "trust_apprentice", 40), op("tick", now=500), obs("mira", "trust_apprentice", -10),
            op("outcome", being="mira", ok=False), MIRA_FRAME))
    case("frame_minimal",
         "A frame with only a being has empty slices.",
         wf(op("frame", being="lone")))
    case("frame_untrusted_bystanders",
         "Trusted bystanders and those out of earshot are not counted as untrusted ears.",
         wf(op("frame", being="mira", listener="player", bystanders=[
             {"being": "wife", "trusted": True}, {"being": "guard", "earshot": False},
             {"being": "gossip"}, {"being": "child", "trusted": False, "earshot": True}])))
    case("frame_body_urgent_order",
         "Urgent body items sort by level: pain 90, thirst 85, fatigue 70; hunger below its threshold is left out.",
         wf(op("frame", being="ana", body={"fatigue": 70, "pain": 90, "conditions": ["limping", "bleeding"],
                                            "needs": [{"id": "hunger", "level": 60},
                                                      {"id": "thirst", "level": 85, "threshold": 50}]})))
    case("frame_affect_top_three",
         "Only the three strongest emotions are kept.",
         wf(op("frame", being="ana", affect={"valence": 10, "emotions": [
             {"emotion": "joy", "value": 20}, {"emotion": "fear", "toward": "wolf", "value": 80},
             {"emotion": "anger", "toward": "bo", "value": 50}, {"emotion": "pride", "value": 50},
             {"emotion": "grief", "value": 5}]})))
    case("rank_mira_scene",
         "Out of eight items Mira keeps the burning bread, the firewood promise, the theft clues, the ear in the room and the conflicted kindness; lore and biography are dropped.",
         wf(op("tick", now=500), MIRA_RANK))
    case("rank_budget_drops_group_whole",
         "A causal group that does not fit is dropped whole, never split; the smaller lone item still fits.",
         wf(op("rank", budget=100, cues={"tags": ["theft"]}, items=[
             {"id": "clue_a", "bytes": 60, "tags": ["theft"], "group": "theft"},
             {"id": "clue_b", "bytes": 60, "tags": ["theft"], "group": "theft"},
             {"id": "small", "bytes": 50, "tags": ["theft"]}])))
    case("rank_mandatory_over_budget",
         "Mandatory items are kept even past the budget, which is reported.",
         wf(op("rank", budget=50, items=[{"id": "secret_rule", "bytes": 80, "mandatory": True},
                                         {"id": "chatty", "bytes": 10, "urgency": 100}])))
    case("rank_mood_favor",
         "In a warm mood past favors rise and grievances do not.",
         wf(op("rank", budget=1000, mood={"gratitude": 60}, items=[
             {"id": "gift_last_week", "bytes": 40, "polarity": "favor"},
             {"id": "old_insult", "bytes": 40, "polarity": "grievance"}])))
    case("rank_custom_weights",
         "With only the utterance weight, items the speaker named win.",
         wf(op("rank", budget=1000, weights={"urgency": 0, "listener": 0, "utterance": 100, "task": 0, "mood": 0,
                                             "recency": 0},
               cues={"places": ["mill"]}, items=[
                   {"id": "mill_fire", "bytes": 40, "places": ["mill"], "urgency": 100},
                   {"id": "wolf_attack", "bytes": 40, "urgency": 100}])))

    case("bad_trend_unknown_key",
         "trend needs an observed key.",
         wf(obs("a", "mood", 1), op("trend", being="a", key="trust")), error=True)
    case("bad_listener_model_without_listener",
         "A listener model needs a listener.",
         wf(op("frame", being="a", listener_model={"trust": 10})), error=True)
    case("bad_duplicate_item",
         "Item ids are unique.",
         wf(op("rank", budget=10, items=[{"id": "x", "bytes": 1}, {"id": "x", "bytes": 2}])), error=True)
    case("bad_zero_weights",
         "At least one salience weight must be positive.",
         wf(op("rank", budget=10, weights={"urgency": 0, "listener": 0, "utterance": 0, "task": 0, "mood": 0,
                                           "recency": 0}, items=[])), error=True)
    case("bad_item_at_future",
         "An item cannot come from after the clock.",
         wf(op("rank", budget=10, items=[{"id": "x", "bytes": 1, "at": 5}])), error=True)
    case("bad_runner_up_score_above_score",
         "The runner-up cannot outscore the winning motive.",
         wf(op("frame", being="a", motive={"id": "eat", "score": 50, "runner_up": "sleep", "runner_up_score": 60})),
         error=True)
    case("bad_keys_full",
         "With room for one key per being, a second key is refused.",
         wf(obs("a", "mood", 1), obs("a", "trust", 1), limits={"keys": 1}), error=True)
    return cases


BEINGS = ["ann", "bo", "cy"]
KEYS = ["mood", "trust", "fear"]
IDS = ["i1", "i2", "i3", "i4", "i5", "i6"]
TAGS = ["oven", "theft", "firewood", "mill"]


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


def some(rng: random.Random, xs: List[str], n: int = 2) -> List[str]:
    return rng.sample(xs, rng.randint(0, n))


def rand_frame(rng: random.Random, t: int) -> Dict[str, Any]:
    b = rng.choice(BEINGS)
    a: Dict[str, Any] = {"being": b}
    if rng.random() < 0.6:
        a["listener"] = rng.choice(BEINGS + ["player"])
    if rng.random() < 0.3:
        a["ticks_per_hour"] = rng.choice([1, 60, 0])
    if rng.random() < 0.4:
        a["bystanders"] = [{"being": w, **({"trusted": rng.random() < 0.5} if rng.random() < 0.5 else {}),
                            **({"earshot": rng.random() < 0.7} if rng.random() < 0.5 else {})}
                           for w in rng.sample(["gossip", "guard", "wife", "player"], rng.randint(0, 3))]
    if rng.random() < 0.5:
        a["body"] = {"fatigue": rng.randint(0, 100), "pain": rng.choice([0, 50, 49, 101]),
                     "needs": [{"id": n, "level": rng.randint(0, 100),
                                **({"threshold": rng.randint(0, 100)} if rng.random() < 0.5 else {})}
                               for n in rng.sample(["hunger", "thirst", "rest", "fatigue"], rng.randint(0, 2))],
                     "conditions": some(rng, ["limping", "bleeding"])}
    if rng.random() < 0.5:
        s = rng.randint(0, 100)
        m: Dict[str, Any] = {"id": rng.choice(["eat", "flee", "work"]), "score": s}
        if rng.random() < 0.5:
            m["runner_up"], m["runner_up_score"] = "talk", rng.randint(0, s + 5)
        if rng.random() < 0.3:
            m["reason"] = rng.choice(["Hungry", "", "x" * 81])
        a["motive"] = m
    if rng.random() < 0.5:
        a["task"] = {"id": "chore", "deadline": t + rng.randint(-5, 5) if t >= 5 else t,
                     "interrupt": rng.choice(["any", "never", "urgent", "maybe"]), "progress": rng.randint(0, 100)}
    if rng.random() < 0.4:
        a["affect"] = {"valence": rng.randint(-101, 100), "emotions": [
            {"emotion": e, "value": rng.randint(0, 100), **({"toward": "bo"} if rng.random() < 0.5 else {})}
            for e in rng.sample(["joy", "fear", "anger", "pride"], rng.randint(0, 4))]}
    if rng.random() < 0.3:
        a["listener_model"] = {"trust": rng.randint(-100, 100), "knows": some(rng, TAGS), "lies": some(rng, TAGS)}
    if rng.random() < 0.3:
        a["offscreen"] = {"summary": "Fetched water", "at": rng.randint(0, t + 1)}
    if rng.random() < 0.2:
        a["novelty"] = {"id": "crate", "score": rng.randint(0, 101)}
    return op("frame", **a)


def rand_rank(rng: random.Random, t: int) -> Dict[str, Any]:
    items = []
    for i in rng.sample(IDS, rng.randint(0, 6)):
        it: Dict[str, Any] = {"id": i, "bytes": rng.choice([1, 40, 120, 65536])}
        if rng.random() < 0.4:
            it["urgency"] = rng.randint(0, 100)
        if rng.random() < 0.4:
            it["entities"] = some(rng, ["player", "gossip", "bo"])
        if rng.random() < 0.4:
            it["tags"] = some(rng, TAGS)
        if rng.random() < 0.3:
            it["group"] = rng.choice(["g1", "g2"])
        if rng.random() < 0.15:
            it["mandatory"] = True
        if rng.random() < 0.3:
            it["polarity"] = rng.choice(["favor", "grievance", "neutral"])
        if rng.random() < 0.3:
            it["at"] = max(0, t - rng.randint(0, 6))
        items.append(it)
    a: Dict[str, Any] = {"budget": rng.choice([0, 50, 150, 400, 10000]), "items": items}
    if rng.random() < 0.6:
        a["listener"] = "player"
    if rng.random() < 0.4:
        a["earshot"] = ["gossip"]
    if rng.random() < 0.5:
        a["cues"] = {"tags": some(rng, TAGS), "entities": some(rng, ["bo", "player"])}
    if rng.random() < 0.3:
        a["task_needs"] = some(rng, TAGS + ["i1"])
    if rng.random() < 0.4:
        a["mood"] = {"valence": rng.randint(-100, 100), "resentment": rng.randint(0, 100)}
    if rng.random() < 0.2:
        a["weights"] = {k: rng.randint(0, 100) for k in rng.sample(["urgency", "listener", "utterance", "task",
                                                                   "mood", "recency"], 3)}
    if rng.random() < 0.2:
        a["recent"] = rng.randint(0, 10)
    if rng.random() < 0.2:
        a["floor"] = rng.randint(0, 300)
    return op("rank", **a)


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    roll = rng.random()
    b = rng.choice(BEINGS)
    if roll < 0.25:
        return obs(b, rng.choice(KEYS), rng.choice([0, 40, -45, rng.randint(-100, 100), 1_000_001]))
    if roll < 0.35:
        return op("tick", now=t)
    if roll < 0.45:
        a: Dict[str, Any] = {"being": b, "key": rng.choice(KEYS)}
        for k, v in [("window", rng.randint(0, 50)), ("shock", rng.choice([1, 30, 60])),
                     ("level", rng.choice([0, 40])), ("settle", rng.choice([0, 20, 500]))]:
            if rng.random() < 0.3:
                a[k] = v
        return op("trend", **a)
    if roll < 0.52:
        return op("outcome", being=b, ok=rng.random() < 0.5)
    if roll < 0.55:
        return op("streak", being=b)
    if roll < 0.62:
        a = {"at": rng.randint(max(0, t - 5000), t + 10)}
        if rng.random() < 0.5:
            a["ticks_per_hour"] = rng.choice([1, 60])
        if rng.random() < 0.4:
            a["due"] = True
        return op("time", **a)
    if roll < 0.8:
        return rand_frame(rng, t)
    return rand_rank(rng, t)


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
        t += rng.choice([0, 1, 3, 10, 500])
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
    ok_runs = err_runs = frames = ranks = kept = dropped = acute = 0
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
            if "untrusted_in_earshot" in r:
                frames += 1
            elif "kept" in r:
                ranks += 1
                kept += len(r["kept"])
                dropped += len(r["dropped"])
            elif r.get("label") in ("acute_rise", "acute_drop"):
                acute += 1
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "frames": frames, "ranks": ranks, "items_kept": kept, "items_dropped": dropped, "acute_trends": acute,
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
        sys.exit("turn-frame checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
