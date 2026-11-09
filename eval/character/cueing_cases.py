#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 3 stage 2: associative memory cueing
with anti-nag cooldowns (E3) and the epistemic and cover-story gate (F4), local kind `cueing`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/cueing/*/case.json + input.json
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
from cueing_reference import workflow as cueing_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "cueing"
LIMIT_NAMES = ["beings", "episodes", "facts", "covers", "secrets", "listeners"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "cueing":
        raise ValueError("unknown local kind")
    return cueing_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "cueing", "args": args}


def ep(i: str, valence: int = 0, **features: Any) -> Dict[str, Any]:
    extra = {k: features.pop(k) for k in ("emotion", "secret") if k in features}
    return op("episode", being="mira", id=i, features=features, valence=valence, **extra)


def cue(listener: str = "player", **situation: Any) -> Dict[str, Any]:
    extra = {k: situation.pop(k) for k in ("cooldown", "min_score", "max") if k in situation}
    return op("cue", being="mira", listener=listener, situation=situation, **extra)


LOAN = ep("loan_returned_early", 40, actions=["loan"], entities=["player"], tags=["tools"])
BROKEN = ep("cart_broken", -50, actions=["loan"], entities=["player"], tags=["cart"])
FIRE = ep("fire_of_the_bakery", -80, tags=["fire", "oven"], emotion="fear")
THEFT = op("fact", being="mira", id="apprentice_theft", claim="The apprentice took the coin pouch",
           source="inference", keys=["took", "stole", "theft"])
COVER = op("cover", being="mira", id="protect_boy", fact="apprentice_theft", lie="The pouch is safe in the back",
           listeners=["gossip"])


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("cue_loan_recalls_early_return",
         "Asking to borrow a tool cues the time the player returned a loan early (loan, player and tools shared).",
         wf(LOAN, FIRE, cue(actions=["loan"], entities=["player"], tags=["tools"])))
    case("cue_fear_topic_avoided",
         "Talk of the oven touches a feared memory: it is returned to avoid, not recall.",
         wf(LOAN, FIRE, cue(tags=["oven", "fire"])))
    case("cue_secret_avoided",
         "A secret episode is never a recall candidate.",
         wf(ep("hid_the_ledger", 10, actions=["store"], places=["cellar"], secret=True),
            cue(actions=["store"], places=["cellar"])))
    case("cue_cooldown_suppresses_repeat",
         "Once surfaced to the player, the same memory is cooling for the next 24 ticks.",
         wf(LOAN, op("surface", being="mira", listener="player", episode="loan_returned_early"),
            op("tick", now=5), cue(actions=["loan"], entities=["player"])))
    case("cue_cooldown_expires",
         "After the cooldown the memory can come up again.",
         wf(LOAN, op("surface", being="mira", listener="player", episode="loan_returned_early"),
            op("tick", now=30), cue(actions=["loan"], entities=["player"])))
    case("cue_cooldown_per_listener",
         "Telling the player does not stop Mira telling the smith.",
         wf(LOAN, op("surface", being="mira", listener="player", episode="loan_returned_early"),
            cue("smith", actions=["loan"])))
    case("cue_max_and_unused",
         "Three matches with room for two: the strongest two are offered, one is counted unused.",
         wf(LOAN, BROKEN, ep("lent_the_ladder", 10, actions=["loan"]),
            cue(actions=["loan"], entities=["player"], tags=["tools"])))
    case("cue_min_score_filters",
         "A single shared tag scores 1, below the default minimum of 2.",
         wf(LOAN, cue(tags=["tools"])))
    case("cue_order_by_score_then_valence",
         "Equal scores are ordered by the stronger feeling: the broken cart (-50) before the early return (40).",
         wf(LOAN, BROKEN, cue(actions=["loan"], entities=["player"], max=4)))
    case("surface_forgets_oldest_listener",
         "With room for two listeners per memory, the one told longest ago is forgotten.",
         wf(LOAN, op("surface", being="mira", listener="ana", episode="loan_returned_early"), op("tick", now=1),
            op("surface", being="mira", listener="bo", episode="loan_returned_early"), op("tick", now=2),
            op("surface", being="mira", listener="cy", episode="loan_returned_early"),
            cue("ana", actions=["loan"], cooldown=100), cue("bo", actions=["loan"], cooldown=100),
            limits={"listeners": 2}))
    case("facts_framings",
         "Five ways of knowing give five framings: state, attribute, hedge, guess and correct.",
         wf(op("fact", being="mira", id="saw_trail", claim="Flour trails to the lockbox", source="direct_witness",
               keys=["flour"]),
            op("fact", being="mira", id="guard_said", claim="The gate was shut at dusk", source="trusted_report",
               **{"from": "guard"}, keys=["gate"], confidence=80),
            op("fact", being="mira", id="mill_rumor", claim="The mill is closing", source="unverified_rumor",
               **{"from": "gossip"}, keys=["mill"]),
            THEFT,
            op("fact", being="mira", id="flour_price", claim="Flour costs four coins", source="corrected_belief",
               previous="Flour costs three coins", keys=["four"]),
            op("frame_facts", being="mira", listener="player",
               facts=["saw_trail", "guard_said", "mill_rumor", "apprentice_theft", "flour_price"])))
    case("facts_cover_for_listener",
         "The cover story is told to the gossip; the player still gets the honest guess.",
         wf(THEFT, COVER, op("frame_facts", being="mira", listener="gossip", facts=["apprentice_theft"]),
            op("frame_facts", being="mira", listener="player", facts=["apprentice_theft"])))
    case("facts_cover_everyone",
         "A cover with no listeners masks the fact for everyone.",
         wf(THEFT, op("cover", being="mira", id="shield", fact="apprentice_theft", lie="Nothing is missing"),
            op("frame_facts", being="mira", listener="player", facts=["apprentice_theft"])))
    case("gate_blocks_cover_leak",
         "A reply naming the theft to the gossip is refused and replaced by the cover story.",
         wf(THEFT, COVER, op("gate", being="mira", listener="gossip", reply="Between us, the boy STOLE it.")))
    case("gate_allows_cover_story_words",
         "Saying the cover story itself is not a leak, and the player may hear the truth.",
         wf(THEFT, COVER, op("gate", being="mira", listener="gossip", reply="The pouch is safe in the back."),
            op("gate", being="mira", listener="player", reply="I think the boy took it.")))
    case("gate_blocks_unknown_secret",
         "Mira does not know the vault code, so a reply with its words is refused with the host's fallback.",
         wf(op("secret", id="vault_code", keys=["lantern", "seven"]),
            op("fact", being="mira", id="saw_trail", claim="Flour trails to the lockbox", source="direct_witness",
               keys=["flour"]),
            op("gate", being="mira", listener="player", reply="Lantern-seven opens it, I'd guess.",
               fallback="I know nothing about vaults.")))
    case("gate_known_secret_ok",
         "A being that holds the secret as a fact may speak of it.",
         wf(op("secret", id="vault_code", keys=["lantern", "seven"]),
            op("fact", being="warden", id="vault_code", claim="The code is lantern seven", source="direct_witness",
               keys=["lantern", "seven"]),
            op("gate", being="warden", listener="player", reply="Lantern seven.")))
    case("gate_uncover_lifts_mask",
         "After the cover is dropped the gossip may hear the truth.",
         wf(THEFT, COVER, op("uncover", being="mira", id="protect_boy"),
            op("gate", being="mira", listener="gossip", reply="He stole it.")))

    case("bad_cue_unknown_being",
         "Cueing needs a being with memories.",
         wf(cue(actions=["loan"])), error=True)
    case("bad_episode_no_features",
         "An episode needs at least one feature.",
         wf(op("episode", being="mira", id="x", features={})), error=True)
    case("bad_rumor_without_source",
         "A rumor names who told it.",
         wf(op("fact", being="mira", id="x", claim="y", source="unverified_rumor", keys=["y"])), error=True)
    case("bad_correction_without_previous",
         "A corrected belief says what was believed before.",
         wf(op("fact", being="mira", id="x", claim="y", source="corrected_belief", keys=["y"])), error=True)
    case("bad_key_word_uppercase",
         "Key words are lower-case ASCII letters and digits.",
         wf(op("fact", being="mira", id="x", claim="y", source="inference", keys=["Stole"])), error=True)
    case("bad_cover_unknown_fact",
         "A cover masks a fact the being holds.",
         wf(THEFT, op("cover", being="mira", id="c", fact="nothing", lie="x")), error=True)
    case("bad_episodes_full",
         "With room for one episode, a second is refused.",
         wf(LOAN, FIRE, limits={"episodes": 1}), error=True)
    return cases


BEINGS = ["mira", "bo"]
LISTENERS = ["player", "gossip", "smith"]
ACTIONS = ["loan", "store", "take"]
ENTITIES = ["player", "bo", "apprentice"]
TAGS = ["tools", "oven", "fire", "cart"]
PLACES = ["cellar", "bakery"]
EPISODES = ["e1", "e2", "e3", "e4"]
FACTS = ["f1", "f2", "f3", "vault"]
WORDS = ["took", "stole", "pouch", "lantern", "seven", "mill", "Upper", "x" * 33]
SOURCES = ["direct_witness", "trusted_report", "unverified_rumor", "inference", "corrected_belief", "dreamt"]
REPLIES = ["The pouch is safe.", "He TOOK it!", "lantern seven", "Nothing to say.", "stole-pouch; mill?",
           "Café took", ""]


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


def some(rng: random.Random, xs: List[str], n: int = 2) -> List[str]:
    return rng.sample(xs, rng.randint(0, n))


def feats(rng: random.Random) -> Dict[str, Any]:
    f: Dict[str, Any] = {}
    for k, xs in [("actions", ACTIONS), ("entities", ENTITIES), ("tags", TAGS), ("places", PLACES)]:
        if rng.random() < 0.6:
            f[k] = some(rng, xs)
    return f


def rand_op(rng: random.Random, t: int) -> Dict[str, Any]:
    b = rng.choice(BEINGS)
    roll = rng.random()
    if roll < 0.18:
        a: Dict[str, Any] = {"being": b, "id": rng.choice(EPISODES), "features": feats(rng)}
        if rng.random() < 0.6:
            a["valence"] = rng.choice([0, 40, -80, rng.randint(-100, 100), 101])
        if rng.random() < 0.3:
            a["emotion"] = rng.choice(["fear", "shame", "joy"])
        if rng.random() < 0.15:
            a["secret"] = True
        return op("episode", **a)
    if roll < 0.36:
        a = {"being": b, "listener": rng.choice(LISTENERS), "situation": feats(rng)}
        for k, v in [("cooldown", rng.choice([0, 5, 24])), ("min_score", rng.choice([1, 2, 4])),
                     ("max", rng.choice([1, 2, 16]))]:
            if rng.random() < 0.3:
                a[k] = v
        return op("cue", **a)
    if roll < 0.44:
        return op("surface", being=b, listener=rng.choice(LISTENERS), episode=rng.choice(EPISODES))
    if roll < 0.5:
        return op("tick", now=t)
    if roll < 0.64:
        src = rng.choice(SOURCES)
        a = {"being": b, "id": rng.choice(FACTS), "claim": rng.choice(["The boy took it", "Mill closing", ""]),
             "source": src, "keys": some(rng, WORDS, 3)}
        if src in ("trusted_report", "unverified_rumor") or rng.random() < 0.05:
            a["from"] = rng.choice(LISTENERS)
        if src == "corrected_belief" or rng.random() < 0.05:
            a["previous"] = "It was three"
        if rng.random() < 0.3:
            a["confidence"] = rng.randint(0, 101)
        return op("fact", **a)
    if roll < 0.72:
        a = {"being": b, "id": rng.choice(["c1", "c2"]), "fact": rng.choice(FACTS),
             "lie": rng.choice(["The pouch is safe", "Nothing is missing", "Seven geese"])}
        if rng.random() < 0.6:
            a["listeners"] = some(rng, LISTENERS)
        return op("cover", **a)
    if roll < 0.75:
        return op("uncover", being=b, id=rng.choice(["c1", "c2"]))
    if roll < 0.8:
        return op("secret", id=rng.choice(FACTS), keys=some(rng, WORDS, 2))
    if roll < 0.88:
        return op("frame_facts", being=b, listener=rng.choice(LISTENERS), facts=some(rng, FACTS, 3))
    a = {"being": b, "listener": rng.choice(LISTENERS), "reply": rng.choice(REPLIES)}
    if rng.random() < 0.3:
        a["fallback"] = rng.choice(["Not now.", ""])
    return op("gate", **a)


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
        t += rng.choice([0, 1, 5, 30])
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
    ok_runs = err_runs = recalls = avoided = cooling = blocked = passed = 0
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
            if "recall" in r:
                recalls += len(r["recall"])
                avoided += len(r["avoid"])
                cooling += len(r["cooling"])
            elif "leaks" in r:
                blocked += int(not r["ok"])
                passed += int(r["ok"])
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "recalled": recalls, "avoided": avoided, "cooling": cooling, "replies_blocked": blocked,
        "replies_passed": passed, "actual_model_calls": 0, "api_spend_usd": 0,
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
        sys.exit("cueing checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
