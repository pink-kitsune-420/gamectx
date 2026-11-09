#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 4 stage 4: community idioms and rumor
mutation, local kind `culture`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/culture/*/case.json + input.json
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
from culture_reference import SOFTER
from culture_reference import workflow as culture_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "culture"
LIMIT_NAMES = ["beings", "idioms", "repertoire", "topics", "beliefs"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "culture":
        raise ValueError("unknown local kind")
    return culture_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "culture", "args": args}


def mem(being: str, community: str = "dock", status: int = 50, openness: int = 50, carefulness: int = 50,
        credulity: int = 50, **extra: Any) -> Dict[str, Any]:
    return op("member", being=being, community=community, status=status, openness=openness,
              carefulness=carefulness, credulity=credulity, **extra)


def tell(speaker: str, listener: str, topic: str = "missing_coins", seed: int = 7, **extra: Any) -> Dict[str, Any]:
    return op("tell", speaker=speaker, listener=listener, topic=topic, seed=seed, **extra)


SALT = op("idiom", id="salt_in_the_barrel", community="dock", phrase="salt in the barrel",
          meaning="a loss made worse by carelessness", prestige=60)
HARBOR = [mem("ada", status=80), mem("newcomer", community="hill", status=10, openness=90)]
COINS = op("seed", topic="missing_coins", witness="fay", verb="stole", agent="ada", place="mill",
           details=["after_dark", "with_a_sack"], amount=3)
# A careless teller and a credulous listener: every telling mutates, with one operator applicable.
LOOSE = [mem("bo", carefulness=0), mem("cy", credulity=100)]


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("member_view", "A member's community, status, openness, carefulness, credulity and sorted links.",
         wf(mem("ada", status=80, openness=40, carefulness=20, credulity=80, links=["cy", "bo"])))
    case("idiom_define", "A community idiom with its phrase, meaning and prestige.", wf(SALT))
    case("newcomer_sounds_local", "A newcomer hears a respected dock speaker until the saying is its own.",
         wf(*HARBOR, SALT, *[op("expose", listener="newcomer", speaker="ada", idiom="salt_in_the_barrel")
                             for _ in range(9)], op("speech", being="newcomer")))
    case("outsider_not_heard", "An idiom heard from someone who does not use it is not picked up.",
         wf(*HARBOR, SALT, mem("hill_elder", community="hill", status=90),
            op("expose", listener="newcomer", speaker="hill_elder", idiom="salt_in_the_barrel")))
    case("adopter_passes_it_on", "Once the newcomer uses the saying, it passes it on outside the docks.",
         wf(*HARBOR, SALT, mem("hill_child", community="hill", status=5, openness=100),
            *[op("expose", listener="newcomer", speaker="ada", idiom="salt_in_the_barrel") for _ in range(9)],
            op("expose", listener="hill_child", speaker="newcomer", idiom="salt_in_the_barrel")))
    case("fad_fades", "Unrepeated, a saying fades from use and then from memory.",
         wf(*HARBOR, SALT, *[op("expose", listener="newcomer", speaker="ada", idiom="salt_in_the_barrel")
                             for _ in range(9)], op("tick", now=100), op("speech", being="newcomer"),
            op("tick", now=600), op("speech", being="newcomer")))
    case("status_and_prestige_scale", "A high-status speaker and a prestigious idiom spread faster.",
         wf(*HARBOR, SALT, mem("deckhand", status=10),
            op("idiom", id="bilge_talk", community="dock", phrase="bilge talk", meaning="idle chatter", prestige=0),
            op("expose", listener="newcomer", speaker="deckhand", idiom="bilge_talk"),
            op("expose", listener="newcomer", speaker="ada", idiom="salt_in_the_barrel")))
    case("repertoire_prunes_faded", "A faded saying makes room in a full repertoire.",
         wf(*HARBOR, SALT,
            op("idiom", id="bilge_talk", community="dock", phrase="bilge talk", meaning="idle chatter", prestige=0),
            op("expose", listener="newcomer", speaker="ada", idiom="salt_in_the_barrel"), op("tick", now=200),
            op("expose", listener="newcomer", speaker="ada", idiom="bilge_talk"), op("speech", being="newcomer"),
            limits={"repertoire": 1}))
    case("seed_witness_version", "A witness's own version: the truth, at distance zero, not hearsay.",
         wf(mem("fay", community="hill"), COINS))
    case("careful_teller_exact", "A careful teller passes the story on unchanged, one hop longer.",
         wf(mem("fay", community="hill", carefulness=100), mem("bo", credulity=100), COINS, tell("fay", "bo")))
    case("exaggerate_hop", "Three coins become more in the retelling; the truth stays three.",
         wf(*LOOSE, op("seed", topic="missing_coins", witness="bo", verb="took", agent="ada", amount=3),
            tell("bo", "cy"), op("spread", topic="missing_coins")))
    case("drop_detail_hop", "A qualifier drops out in the retelling.",
         wf(*LOOSE, op("seed", topic="missing_coins", witness="bo", verb="took", agent="ada",
                       details=["after_dark", "with_a_sack"]), tell("bo", "cy")))
    case("shift_attribution_to_link", "The deed shifts to someone plausibly linked to the agent.",
         wf(*LOOSE, mem("ada", links=["apprentice"]),
            op("seed", topic="missing_coins", witness="bo", verb="took", agent="ada"), tell("bo", "cy")))
    case("merge_held_topics", "Two stories the teller holds become one.",
         wf(*LOOSE, op("seed", topic="missing_coins", witness="bo", verb="took", agent="ada"),
            op("seed", topic="mill_fire", witness="bo", verb="lit", agent="miller"), tell("bo", "cy")))
    case("soften_when_fond", "A teller fond of the agent says borrowed, not stole.",
         wf(*LOOSE, op("seed", topic="missing_coins", witness="bo", verb="stole", agent="ada"),
            tell("bo", "cy", fondness=80)))
    case("not_softened_without_fondness", "Without fondness there is nothing to soften and nothing else applies.",
         wf(*LOOSE, op("seed", topic="missing_coins", witness="bo", verb="stole", agent="ada"),
            tell("bo", "cy", fondness=59)))
    case("six_hops_distance_grows", "Down a chain of gossips the distance from the truth grows.",
         wf(mem("fay", community="hill", carefulness=80, credulity=30), mem("bo", carefulness=10, credulity=90),
            mem("cy", carefulness=5, credulity=95), mem("dee", community="hill", carefulness=30, credulity=70),
            mem("eli", community="hill", carefulness=50, credulity=60), mem("newcomer", community="hill", credulity=50),
            COINS, tell("fay", "bo", fondness=70), tell("bo", "cy", fondness=70), tell("cy", "dee", fondness=70),
            tell("dee", "eli", fondness=70), tell("eli", "newcomer", fondness=70), op("spread", topic="missing_coins")))
    case("correction_by_trusted_witness", "A trusted witness retelling the original brings a listener back to the truth.",
         wf(mem("fay", community="hill", carefulness=100), *LOOSE, COINS,
            tell("fay", "bo"), tell("bo", "cy"), tell("fay", "cy", trust=90), op("belief", being="cy", topic="missing_coins")))
    case("distrusted_teller_kept", "A listener keeps its own version over a teller it does not trust.",
         wf(mem("fay", community="hill", carefulness=100), *LOOSE, COINS, tell("fay", "bo"), tell("bo", "cy"),
            tell("fay", "cy", trust=20)))
    case("chain_keeps_last_eight", "Ten careful hops: the chain keeps the last eight tellers, the hop count all ten.",
         wf(*[mem(f"p{i}", carefulness=100) for i in range(11)],
            op("seed", topic="missing_coins", witness="p0", verb="took", agent="ada"),
            *[tell(f"p{i}", f"p{i + 1}") for i in range(10)], op("belief", being="p10", topic="missing_coins")))
    case("bad_tell_unheld_topic", "Malformed: a teller must hold the story.",
         wf(*LOOSE, mem("fay"), COINS, tell("bo", "cy")), error=True)
    case("bad_seed_duplicate_topic", "Malformed: a topic is seeded once.",
         wf(mem("fay"), COINS, COINS), error=True)
    case("bad_member_self_link", "Malformed: a being is not its own plausible link.",
         wf(mem("ada", links=["ada"])), error=True)
    case("bad_expose_self", "Malformed: a being does not hear an idiom from itself.",
         wf(*HARBOR, SALT, op("expose", listener="ada", speaker="ada", idiom="salt_in_the_barrel")), error=True)
    case("bad_amount_zero", "Malformed: an amount is at least 1.",
         wf(mem("fay"), op("seed", topic="t", witness="fay", verb="took", agent="ada", amount=0)), error=True)
    case("bad_beliefs_over_limit", "Malformed: one belief more than the limit is refused.",
         wf(mem("fay"), mem("bo"), COINS, tell("fay", "bo"), limits={"beliefs": 1}), error=True)
    case("bad_idiom_phrase_too_long", "Malformed: an idiom phrase is at most 60 characters.",
         wf(op("idiom", id="long", community="dock", phrase="x" * 61, meaning="m", prestige=1)), error=True)
    return cases


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {"beings": rng.randint(2, 6), "idioms": rng.randint(1, 3), "repertoire": rng.randint(1, 3),
           "topics": rng.randint(1, 3), "beliefs": rng.randint(1, 8)}
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


BEINGS = ["b1", "b2", "b3", "b4", "b5"]
TOPICS = ["t1", "t2", "t3"]
IDIOMS = ["i1", "i2", "i3"]
PREFIX = [mem("b1", carefulness=0, credulity=100, status=90, links=["b2", "b3"]),
          mem("b2", community="hill", carefulness=20, credulity=90, openness=100),
          mem("b3", carefulness=60, credulity=40, links=["b4"]), mem("b4", community="hill", credulity=100),
          op("idiom", id="i1", community="dock", phrase="salt in the barrel", meaning="worse", prestige=80),
          op("seed", topic="t1", witness="b1", verb="stole", agent="b3", place="mill", details=["a", "b", "c"], amount=7),
          op("seed", topic="t2", witness="b1", verb="lit", agent="b1", amount=100)]
PREFIX += [op("expose", listener="b2", speaker="b1", idiom="i1") for _ in range(7)]


def rand_op(rng: random.Random, clock: List[int]) -> Dict[str, Any]:
    roll = rng.random()
    b, b2 = rng.choice(BEINGS), rng.choice(BEINGS)
    if roll < 0.06:
        clock[0] += rng.choice([0, 1, 50, 300, 1000])
        return op("tick", now=rng.choice([clock[0]] * 9 + [0, 1_000_000_000_001]))
    if roll < 0.14:
        a = {k: rng.choice([0, 30, 70, 100, 101]) for k in ("status", "openness", "carefulness", "credulity")}
        if rng.random() < 0.5:
            a["links"] = rng.sample(BEINGS + ["apprentice"], rng.randint(0, 3))
        return op("member", being=b, community=rng.choice(["dock", "hill"]), **a)
    if roll < 0.20:
        return op("idiom", id=rng.choice(IDIOMS), community=rng.choice(["dock", "hill"]),
                  phrase=rng.choice(["bilge talk", "", "x" * 61, "fair winds"]), meaning="m",
                  prestige=rng.choice([0, 50, 100, 101]))
    if roll < 0.40:
        return op("expose", listener=b, speaker=b2, idiom=rng.choice(IDIOMS))
    if roll < 0.46:
        return op("speech", being=b)
    if roll < 0.54:
        a = {"topic": rng.choice(TOPICS), "witness": b, "verb": rng.choice(list(SOFTER) + ["took"]), "agent": b2}
        if rng.random() < 0.5:
            a["details"] = rng.sample(["x", "y", "z", "w"], rng.randint(0, 3))
        if rng.random() < 0.5:
            a["amount"] = rng.choice([1, 3, 1000, 1_000_000_000, 0])
        if rng.random() < 0.3:
            a["place"] = rng.choice(["mill", None])
        return op("seed", **a)
    if roll < 0.88:
        a = {"speaker": b, "listener": b2, "topic": rng.choice(TOPICS + TOPICS[:1] * 2)}
        if rng.random() < 0.8:
            a["seed"] = rng.randint(0, 50)
        if rng.random() < 0.5:
            a["fondness"] = rng.choice([0, 59, 60, 100])
        if rng.random() < 0.4:
            a["trust"] = rng.choice([0, 49, 50, 100])
        return op("tell", **a)
    if roll < 0.94:
        return op("belief", being=b, topic=rng.choice(TOPICS))
    return op("spread", topic=rng.choice(TOPICS))


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
    clock = [0]
    target = len(ops) + rng.randint(1, 20)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        ops.append(rand_op(rng, clock))
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
    ok_runs = err_runs = tellings = mutations = adoptions = corrections = 0
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
            if "kept" in r:
                tellings += 1
                mutations += int(r["mutated"] is not None)
                corrections += int(r["before"] is not None and not r["kept"]
                                   and r["belief"]["distance"]["ops"] < r["before"]["ops"])
            adoptions += int(r.get("adopted") is True and r.get("gain", 0) > 0)
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "tellings": tellings, "mutations": mutations, "corrections": corrections, "adopting_exposures": adoptions,
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
        sys.exit("culture checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
