#!/usr/bin/env python3
"""Conformance cases and differential parity runner for host-set limits on the pass 1
character modules (stage 7b): affect, skills, body, mind, deception, relationship,
offscreen, curiosity, task.

  --cap 0                 check the Python references against every blessed case
  --generate              (re)write conformance/limits_character/*/case.json + input.json
  --random N --seed S     N randomized growth workflows with random limits, Python vs `gamectx local -`

All checks are offline: zero model calls, $0 API spend.
"""

import argparse
import json
import pathlib
import random
import subprocess
import sys
from typing import Any, Callable, Dict, List

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import affect_skills_reference
import body_reference
import offscreen_curiosity_reference
import social_mind_reference
import task_reference

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "limits_character"

NAMES = {
    "affect": ["beings", "attitudes"],
    "skills": ["beings", "skills"],
    "body": ["beings", "needs", "injuries", "conditions"],
    "mind": ["beings", "others", "beliefs", "witnesses"],
    "deception": ["records"],
    "relationship": ["subjects", "relations", "attitudes", "community_pairs"],
    "offscreen": ["records", "work_per_tick"],
    "curiosity": ["beings", "known", "perception_items"],
    "task": ["tasks", "steps", "reserved", "history"],
}


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    kind, args = request["kind"], request.get("args", {})
    if kind in ("affect", "skills"):
        return affect_skills_reference.workflow(request)
    if kind == "body":
        return body_reference.BodyReference().workflow(args)
    if kind in ("mind", "deception", "relationship"):
        return social_mind_reference.process_local(kind, args)
    if kind in ("offscreen", "curiosity"):
        return offscreen_curiosity_reference.process_local(kind, args)
    if kind == "task":
        return task_reference.TaskReference().workflow(args)
    raise ValueError(kind)


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(kind: str, ops: List[Dict[str, Any]], limits: Any = "absent", **extra: Any) -> Dict[str, Any]:
    args: Dict[str, Any] = {} if limits == "absent" else {"limits": limits}
    args.update(extra)
    args["ops"] = ops
    return {"kind": kind, "args": args}


# --- small builders shared by cases and random workflows ---

def affect_init(b: str, attitudes: int = 0) -> Dict[str, Any]:
    atts = [{"id": f"trust_s{j}", "kind": "trust", "subject": f"s{j}", "level": 10 + j} for j in range(attitudes)]
    return op("init", being=b, temperament="calm", mood={"valence": 0, "arousal": 0}, attitudes=atts)


def appraise(b: str, subject: str, kind: str = "fear", delta: int = 20) -> Dict[str, Any]:
    return op("appraise", being=b, kind=kind, subject=subject, delta=delta, reason="a sudden noise")


def skills_init(b: str, skills: List[str], level: int = 1) -> Dict[str, Any]:
    return op("init", being=b, skills=[{"id": s, "level": level} for s in skills])


def practice(b: str, skill: str) -> Dict[str, Any]:
    return op("practice", being=b, skill=skill, practice=30, success=True)


def body_init(b: str, needs: int = 1, conditions: List[str] = ()) -> Dict[str, Any]:
    ns = [{"id": f"need_{j}", "threshold": 70, "rate_per_tick": 1, "level": 4 * j} for j in range(needs)]
    return op("init", being=b, fatigue=10, health=90, needs=ns, conditions=list(conditions))


def injury(b: str, iid: str) -> Dict[str, Any]:
    return op("add_injury", being=b, injury={"id": iid, "severity": 20, "location": "arm"})


def record_knows(s: str, o: str, fact: str) -> Dict[str, Any]:
    return op("record", subject=s, other=o, category="knows", fact=fact, status=True, source="told", at=3)


def witness(s: str, ids: List[str], fact: str = "cart_crash") -> Dict[str, Any]:
    entry = {"at": 15, "kind": "crash", "place": "road", "actors": ["carter"], "visibility": "witnessed", "noise": 30}
    beings = [{"id": i, "place": "road" if n % 2 == 0 else "bakery", "senses": {"sight": True, "hearing": True}}
              for n, i in enumerate(ids)]
    return op("infer_from_witness", subject=s, fact=fact, entry=entry, beings=beings)


def told(n: int, speaker: str = "trader", listener: str = "baker") -> Dict[str, Any]:
    return op("record", id=f"claim_{n}", speaker=speaker, listener=listener, topic=f"topic_{n % 5}",
              claim="cheap", true_value="dear" if n % 2 else "cheap", at=n)


def window(b: str, at: int) -> Dict[str, Any]:
    return {"being": b, "activity": "night_baking", "action": "cook", "observed": False, "at": at, "place": "bakery",
            "outputs": [{"count": 2, "item": "bread"}], "fatigue": 5}


def cur_init(b: str, people: List[str] = (), temperament: str = "curious") -> Dict[str, Any]:
    return op("init", being=b, temperament=temperament, known_people=list(people))


def novelty(b: str, people: List[str], places: List[str] = ()) -> Dict[str, Any]:
    return op("evaluate_novelty", being=b, people=list(people), places=list(places))


WORLD = {"location": "bakery", "inventory": {"flour": 0, "coins": 10},
         "recipes": {"bread": {"station": "bakery", "inputs": [{"item": "flour", "count": 1}],
                               "outputs": [{"item": "bread", "count": 1}]}},
         "sources": [{"item": "flour", "place": "mill", "partner": "miller", "cost_item": "coins", "cost_count": 5}]}


def propose(tid: str, owner: str = "baker", reserved: int = 0) -> Dict[str, Any]:
    args: Dict[str, Any] = {"id": tid, "owner": owner, "priority": 60, "goal": {"kind": "cook", "recipe": "bread"},
                            "world_state": WORLD}
    if reserved:
        args["reserved_resources"] = [f"res_{j}" for j in range(reserved)]
    return {"op": "propose", "args": args}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    wait = {"id": "w", "action": "wait"}
    add("lc_affect_echo_defaults", "An empty limits object echoes the defaults and changes nothing else.",
        wf("affect", [affect_init("guard"), appraise("guard", "wolf"), op("assess", being="guard")], {}))
    add("lc_affect_attitudes_above_old_cap", "Raising attitudes to 70 lets one being start with 70 attitudes.",
        wf("affect", [affect_init("elder", attitudes=70), op("tick", being="elder", ticks=1)], {"attitudes": 70}))
    add("lc_affect_existing_attitude_still_moves", "At the attitude limit an existing attitude still changes.",
        wf("affect", [affect_init("guard"), appraise("guard", "wolf"), appraise("guard", "wolf", delta=15),
                      op("assess", being="guard")], {"attitudes": 1}))
    add("lc_skills_practice_respects_limit", "Practice can no longer add a skill past the per-being limit.",
        wf("skills", [skills_init("smith", ["forging"]), practice("smith", "forging"), practice("smith", "riding")],
           {"skills": 1}), error=True)
    add("lc_skills_many_skills", "Raising skills to 40 lets a master hold 40 skills (the old bound was 32).",
        wf("skills", [skills_init("master", [f"craft_{j}" for j in range(40)], level=5),
                      op("assess", being="master")], {"skills": 40}))
    add("lc_skills_teach_within_room", "Teaching a skill the student already has needs no new room.",
        wf("skills", [skills_init("mentor", ["herbs"], level=50), skills_init("pupil", ["herbs"]),
                      op("teach", teacher="mentor", student="pupil", skill="herbs", hours=2, same_place=True)],
           {"skills": 1}))
    add("lc_body_twenty_injuries", "Raising injuries to 20 lets a knight carry 20 wounds (the old bound was 16); together they are fatal.",
        wf("body", [body_init("knight")] + [injury("knight", f"cut_{j}") for j in range(20)]
           + [op("assess", being="knight")], {"injuries": 20}))
    add("lc_body_many_needs", "Raising needs to 20 lets a body track 20 needs (the old bound was 16).",
        wf("body", [body_init("golem", needs=20), op("tick", being="golem", ticks=3)], {"needs": 20}))
    add("lc_body_tick_creates_within_limit", "Ops that create bodies on demand respect the beings limit.",
        wf("body", [op("tick", being="first", ticks=1), op("tick", being="second", ticks=1)], {"beings": 1}),
        error=True)
    add("lc_mind_two_others", "A mind models at most two others when the limit is two (record was unbounded).",
        wf("mind", [op("init", subject="baker"), record_knows("baker", "miller", "flour_late"),
                    record_knows("baker", "carter", "flour_late"), record_knows("baker", "guard", "flour_late")],
           {"others": 2}), error=True)
    add("lc_mind_witness_crowd", "Raising witnesses to 80 lets 80 villagers be considered by one inference.",
        wf("mind", [witness("baker", ["baker"] + [f"onlooker_{n}" for n in range(79)]),
                    op("query", subject="baker", other="onlooker_78", category="knows", key="cart_crash")],
           {"witnesses": 80, "others": 100}))
    add("lc_mind_existing_fact_updates_at_limit", "At the belief limit an already known fact still updates.",
        wf("mind", [record_knows("baker", "miller", "flour_late"),
                    op("record", subject="baker", other="miller", category="knows", fact="flour_late",
                       status=False, source="seen", at=9),
                    op("query", subject="baker", other="miller", category="knows", key="flour_late")],
           {"beliefs": 1}))
    add("lc_deception_large_ledger", "Raising records to 150 keeps 149 claims (the old bound was 128).",
        wf("deception", [told(n) for n in range(63)] + [op("assess", topic="topic_3")], {"records": 150},
           records=[{"id": f"pre_{n}", "speaker": "trader", "listener": "baker", "topic": "origin", "claim": "east",
                     "true_value": "west"} for n in range(86)]))
    add("lc_deception_null_limits", "A null limits object is ignored but still echoed as the defaults.",
        wf("deception", [told(1), told(2), op("check_consistency", speaker="trader", listener="baker",
                                              topic="topic_1", proposed_claim="dear")], None))
    add("lc_relationship_attitudes_limit", "Appraisal can no longer add attitudes past the limit.",
        wf("relationship", [op("appraise_action", actor="trader", target="baker", action="gift", success=True)],
           {"attitudes": 1}), error=True)
    add("lc_relationship_sheet_above_default", "A sheet of 70 relations for one subject needs relations raised to 70.",
        wf("relationship", [op("init", sheet_relationships=[{"subject": "mayor", "with": f"citizen_{n}",
                                                             "kind": "ally"} for n in range(70)]),
                            op("assess", subject="mayor", target="citizen_69")], {"relations": 70}))
    add("lc_relationship_community_room", "Updating an existing community pair needs no new room.",
        wf("relationship", [op("init", communities=[{"community": "bakers", "target": "millers", "trust": 10}]),
                            op("aggregate_community_trust", community="bakers", target_community="millers",
                               members=["baker1"], target_members=["miller1"])], {"community_pairs": 1}))
    add("lc_offscreen_small_history", "Keeping two records evicts the lowest record ids first.",
        wf("offscreen", [op("resolve_window", **window(f"baker_{n}", 10 * n)) for n in range(4)]
           + [op("assess")], {"records": 2}))
    add("lc_offscreen_big_batch", "Raising work_per_tick to 20 resolves 20 off-screen activities in one batch.",
        wf("offscreen", [op("batch_resolve", activities=[window(f"worker_{n}", n) for n in range(20)])],
           {"work_per_tick": 20}))
    add("lc_curiosity_known_fills_sorted", "Known lists fill in sorted order up to the limit.",
        wf("curiosity", [cur_init("scout", ["mira"]), op("mark_familiar", being="scout", people=["zed", "ann"]),
                         op("assess", being="scout")], {"known": 2}))
    add("lc_curiosity_crowd_perception", "Raising perception_items to 40 lets a scout notice 40 strangers at once.",
        wf("curiosity", [cur_init("scout"), novelty("scout", [f"stranger_{n}" for n in range(40)])],
           {"perception_items": 40}))
    add("lc_curiosity_over_ceiling", "A known limit above its ceiling (65,536) is refused.",
        wf("curiosity", [cur_init("scout")], {"known": 65537}), error=True)
    add("lc_task_reserved_limit", "Reserving three resources is refused with room for two.",
        wf("task", [propose("bake", reserved=3)], {"reserved": 2}), error=True)
    add("lc_task_repropose_needs_no_room", "At the task limit, re-proposing an existing task replaces it.",
        wf("task", [propose("bake"), propose("sweep", owner="apprentice"), propose("bake", owner="apprentice"),
                    op("list")], {"tasks": 2}))
    add("lc_task_validate_step_limit", "Plan validation reports the host step limit.",
        wf("task", [op("validate", steps=[dict(wait, id=f"w{n}") for n in range(3)])], {"steps": 2}))
    return cases


# --- random growth workflows ---

def r_ids(rng: random.Random, prefix: str, pool: int) -> str:
    return f"{prefix}{rng.randint(0, pool - 1)}"


def rand_affect(rng: random.Random) -> Dict[str, Any]:
    ops, known = [], ["b0"]
    for _ in range(rng.randint(1, 20)):
        if rng.random() < 0.35:
            b = r_ids(rng, "b", 6)
            known.append(b)
            ops.append(affect_init(b, rng.randint(0, 3)))
        else:
            ops.append(appraise(rng.choice(known), r_ids(rng, "s", 6), rng.choice(["fear", "trust", "gratitude"]),
                                rng.randint(-30, 30)))
    return wf("affect", [affect_init("b0")] + ops + [op("assess", being="b0")])


def rand_skills(rng: random.Random) -> Dict[str, Any]:
    ops, known = [skills_init("b0", ["s0"], level=40)], ["b0"]
    for _ in range(rng.randint(1, 20)):
        roll = rng.random()
        b = rng.choice(known)
        if roll < 0.3:
            b = r_ids(rng, "b", 5)
            known.append(b)
            ops.append(skills_init(b, sorted({r_ids(rng, "s", 6) for _ in range(rng.randint(0, 3))}),
                                   rng.randint(1, 30)))
        elif roll < 0.8:
            ops.append(practice(b, r_ids(rng, "s", 8)))
        else:
            ops.append(op("teach", teacher="b0", student=b, skill="s0", hours=rng.randint(1, 3), same_place=True))
    return wf("skills", ops + [op("assess", being="b0")])


def rand_body(rng: random.Random) -> Dict[str, Any]:
    ops = [body_init("b0", rng.randint(0, 3))]
    for _ in range(rng.randint(1, 20)):
        roll = rng.random()
        b = r_ids(rng, "b", 6)
        if roll < 0.2:
            ops.append(body_init(b, rng.randint(0, 4), rng.sample(["sick", "asleep"], rng.randint(0, 2))))
        elif roll < 0.45:
            ops.append(injury(b, r_ids(rng, "cut_", 6)))
        elif roll < 0.7:
            ops.append(op("set_condition", being=b, condition=rng.choice(["sick", "asleep", "cold", "lost"])))
        else:
            ops.append(op("tick", being=b, ticks=rng.randint(1, 4)))
    return wf("body", ops + [op("assess", being="b0")])


def rand_mind(rng: random.Random) -> Dict[str, Any]:
    ops = []
    for _ in range(rng.randint(1, 20)):
        roll = rng.random()
        s = r_ids(rng, "s", 5)
        if roll < 0.15:
            ops.append(op("init", subject=s, others=[{"other": r_ids(rng, "o", 6), "knows": [
                {"fact": r_ids(rng, "f", 5), "status": True, "source": "seen"}]}]))
        elif roll < 0.6:
            ops.append(record_knows(s, r_ids(rng, "o", 6), r_ids(rng, "f", 6)))
        elif roll < 0.75:
            ops.append(op("record", subject=s, other=r_ids(rng, "o", 6), category="wants", goal=r_ids(rng, "g", 5),
                          priority=rng.randint(0, 100)))
        else:
            ops.append(witness(s, [r_ids(rng, "o", 8) for _ in range(rng.randint(1, 6))], r_ids(rng, "f", 4)))
    return wf("mind", ops + [op("assess", subject="s0")] if any(o["args"].get("subject") == "s0" for o in ops) else ops)


def rand_deception(rng: random.Random) -> Dict[str, Any]:
    ops = [told(rng.randint(0, 12), r_ids(rng, "p", 3), r_ids(rng, "l", 3)) for _ in range(rng.randint(1, 20))]
    for o in ops:
        if rng.random() < 0.3:
            o["args"].pop("id")
    ops.append(op("check_consistency", speaker="trader", listener="baker", topic="topic_1", proposed_claim="cheap"))
    return wf("deception", ops)


def rand_relationship(rng: random.Random) -> Dict[str, Any]:
    ops = [op("init", sheet_relationships=[{"subject": r_ids(rng, "p", 4), "with": r_ids(rng, "p", 6),
                                            "kind": rng.choice(["friend", "rival", "mentor"])}
                                           for _ in range(rng.randint(0, 6))],
              attitudes=[{"subject": r_ids(rng, "p", 4), "target": r_ids(rng, "p", 4), "kind": "trust",
                          "level": rng.randint(-50, 50)} for _ in range(rng.randint(0, 5))])]
    for _ in range(rng.randint(1, 12)):
        if rng.random() < 0.7:
            ops.append(op("appraise_action", actor=r_ids(rng, "p", 6), target=r_ids(rng, "p", 6),
                          action=rng.choice(["gift", "barter", "teach", "help"]), success=rng.random() < 0.7))
        else:
            ops.append(op("aggregate_community_trust", community=r_ids(rng, "c", 3),
                          target_community=r_ids(rng, "c", 4), members=["p0", "p1"], target_members=["p2"]))
    return wf("relationship", ops + [op("assess", subject="p0")])


def rand_offscreen(rng: random.Random) -> Dict[str, Any]:
    ops = []
    for _ in range(rng.randint(1, 12)):
        if rng.random() < 0.7:
            ops.append(op("resolve_window", **window(r_ids(rng, "w", 5), rng.randint(1, 500))))
        else:
            ops.append(op("batch_resolve", activities=[window(r_ids(rng, "w", 5), rng.randint(1, 500))
                                                       for _ in range(rng.randint(1, 18))]))
    return wf("offscreen", ops + [op("assess")])


def rand_curiosity(rng: random.Random) -> Dict[str, Any]:
    ops = [cur_init("b0")]
    for _ in range(rng.randint(1, 15)):
        roll = rng.random()
        b = r_ids(rng, "b", 5)
        people = sorted({r_ids(rng, "p", 12) for _ in range(rng.randint(0, 6))})
        if roll < 0.2:
            ops.append(cur_init(b, people, rng.choice(["curious", "cautious", "calm"])))
        elif roll < 0.5:
            ops.append(op("mark_familiar", being=b, people=people))
        elif roll < 0.75:
            ops.append(novelty(b, people, [r_ids(rng, "pl", 4)]))
        else:
            ops.append(op("generate_candidates", being=b, people=people, auto_mark=rng.random() < 0.6))
    return wf("curiosity", ops + [op("assess", being="b0")])


def rand_task(rng: random.Random) -> Dict[str, Any]:
    ops, proposed = [propose("t0")], ["t0"]
    for _ in range(rng.randint(1, 12)):
        roll = rng.random()
        t = rng.choice(proposed)
        if roll < 0.5:
            t = r_ids(rng, "t", 6)
            proposed.append(t)
            ops.append(propose(t, r_ids(rng, "o", 3), rng.randint(0, 4)))
        elif roll < 0.7:
            ops.append(op("accept", id=t))
        elif roll < 0.9:
            ops.append(op("advance", id=t, outcome={"success": rng.random() < 0.8}, tick=rng.randint(1, 50)))
        else:
            ops.append(op("replan", id=t, goal={"kind": "cook", "recipe": "bread"}, world_state=WORLD))
    return wf("task", ops + [op("list")])


GENERATORS: List[Callable[[random.Random], Dict[str, Any]]] = [
    rand_affect, rand_skills, rand_body, rand_mind, rand_deception, rand_relationship, rand_offscreen,
    rand_curiosity, rand_task]


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    req = rng.choice(GENERATORS)(rng)
    if rng.random() < 0.85:
        names = NAMES[req["kind"]]
        lim = {n: rng.randint(2, 8) if rng.random() < 0.6 else rng.randint(9, 200)
               for n in rng.sample(names, rng.randint(1, len(names)))}
        if rng.random() < 0.02:
            lim["nonsense"] = 1
        req["args"] = {"limits": lim, **req["args"]}
    return req


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
            problems.append(f"{c['id']}: reference refused: {err!r}")
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
    ok_runs = err_runs = limited = 0
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
                print(f"Iteration {i}: Python refused ({py_err!r}) but Rust accepted\n{payload}", file=sys.stderr)
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
        limited += "limits" in rust_res
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "with_limits": limited, "actual_model_calls": 0, "api_spend_usd": 0,
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
        sys.exit("limits checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
