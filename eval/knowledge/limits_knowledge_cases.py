#!/usr/bin/env python3
"""Conformance cases and differential parity runner for host-set limits on the knowledge
modules (pass 2 stage 7c): places (codex), timeline, things, species, recipes, communities,
actions, lore, inventory, evidence selection and attributed notes (`info`).

  --cap 0                 check the Python references against every blessed case
  --generate              (re)write conformance/limits_knowledge/*/case.json + input.json
  --random N --seed S     N randomized workflows with random limits, Python vs `gamectx local -`

All checks are offline: zero model calls, $0 API spend.
"""

import argparse
import copy
import json
import pathlib
import random
import subprocess
import sys
from typing import Any, Callable, Dict, List

HERE = pathlib.Path(__file__).resolve().parent
sys.path[1:1] = [str(d) for d in sorted(HERE.parent.iterdir()) if d.is_dir()]
import actions_reference
import communities_cases
import communities_reference
import info_reference
import inventory_reference
import lore_reference
import optimization_reference
import places_cases
import places_reference
import recipes_cases
import recipes_reference
import species_cases
import species_reference
import things_cases
import things_reference
import timeline_reference

ROOT = HERE.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "limits_knowledge"

NAMES = {
    "places": ["places", "entrances", "factions", "events", "quests", "bytes"],
    "timeline": ["entries", "bytes"],
    "things": ["materials", "kinds", "things", "bytes"],
    "species": ["species", "animals"],
    "recipes": ["recipes"],
    "communities": ["communities", "members", "customs", "relations", "rules"],
    "actions": ["actions"],
    "lore": ["entries", "bytes"],
    "inventory": ["items", "holdings", "bytes"],
    "evidence": ["records", "bytes"],
    "info": ["notes", "bytes"],
}

PY: Dict[str, Callable[[Any], Any]] = {
    "places": places_reference.workflow,
    "timeline": timeline_reference.workflow,
    "things": things_reference.workflow,
    "species": species_reference.workflow,
    "recipes": recipes_reference.workflow,
    "communities": communities_reference.workflow,
    "actions": actions_reference.workflow,
    "lore": lambda a: lore_reference.process({"kind": "lore", "args": a}),
    "inventory": inventory_reference.process,
    "evidence": optimization_reference.evidence,
    "info": info_reference.process,
}


def py_workflow(request: Dict[str, Any]) -> Any:
    return PY[request["kind"]](copy.deepcopy(request["args"]))


def enc(v: Any) -> str:
    return json.dumps(v, ensure_ascii=False, separators=(",", ":"))


def req(kind: str, args: Dict[str, Any], limits: Any = "absent") -> Dict[str, Any]:
    out: Dict[str, Any] = {} if limits == "absent" else {"limits": limits}
    out.update(copy.deepcopy(args))
    return {"kind": kind, "args": out}


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


# --- fixtures, all original (Ember Vale / Ember Keep) ---

VALE = places_cases.VALE
FROM_BAKERY = {"from": "bakery"}


def vale_with_fields(n: int) -> Dict[str, Any]:
    """The sample codex plus `n` small fields inside Emberford."""
    v = copy.deepcopy(VALE)
    v["places"] += [{"id": f"field_{i}", "label": f"Field {i}", "kind": "well", "within": "emberford"} for i in range(n)]
    return v


def record(at: int, n: int = 0) -> Dict[str, Any]:
    return op("record", at=at, kind="harvest", summary=f"Harvest {n} came in at the mill.", actors=[f"farmer_{n % 3}"],
              place="mill", phrases=[f"harvest {n}"])


THINGS = things_cases.BASE_THINGS


def things_with_kinds(n: int) -> Dict[str, Any]:
    t = copy.deepcopy(THINGS)
    t["kinds"] += [{"id": f"trinket_{i}", "label": f"Trinket {i}", "category": "tool", "materials": [], "functions": [],
                    "weight": 1, "base_value": 1, "durability": 10, "stackable": True} for i in range(n)]
    return t


SPECIES = species_cases.SAMPLE_SPECIES
ANIMALS = species_cases.SAMPLE_ANIMALS


def herd(n: int) -> List[Dict[str, Any]]:
    a = ANIMALS[0]
    return ANIMALS + [dict(copy.deepcopy(a), id=f"{a['id']}_{i}", trust={}) for i in range(n)]


RECIPES = recipes_cases.SAMPLE_RECIPES


def recipe_book(n: int) -> List[Dict[str, Any]]:
    r = RECIPES[0]
    return RECIPES + [dict(copy.deepcopy(r), id=f"{r['id']}_{i}", label=f"Forge Sword {i}") for i in range(n)]


COMMUNITIES = communities_cases.SAMPLE_COMMUNITIES


def guilds(n: int) -> List[Dict[str, Any]]:
    return COMMUNITIES + [{"id": f"guild_{i}", "label": f"Guild {i}", "description": "A small craft guild of the vale."}
                          for i in range(n)]


def action(i: int) -> Dict[str, Any]:
    return actions_reference.make_action(f"chore_{i}", f"Chore {i}", "things", "A small household chore.")


LORE = json.loads((ROOT / "examples/lantern-vault/world-structured.json").read_text())
LORE_SELECT = {"actor": "npc:merchant", "grants": [], "query": "tonic purpose", "subjects": ["item:tonic"],
               "required": ["tonic_use"], "limit": 4, "max_bytes": 2048}


def lore_with(n: int) -> Dict[str, Any]:
    w = copy.deepcopy(LORE)
    w["entries"] += [{"id": f"rumor_{i}", "subject": "item:tonic", "topic": "rumor", "value": f"rumor {i}", "tags": [],
                      "known_by": ["*"], "requires": []} for i in range(n)]
    return w


def stock(holders: int) -> Dict[str, Any]:
    """An inventory with two items; `holders` villagers each hold one salve."""
    return {"version": 1, "revision": 0,
            "items": [{"id": "item:oil", "label": "hinge oil", "tags": ["supply"]},
                      {"id": "item:salve", "label": "salve", "tags": ["heal"]}],
            "holdings": [{"owner": "merchant", "item": "item:oil", "quantity": 9}]
            + [{"owner": f"villager_{i}", "item": "item:salve", "quantity": 1} for i in range(holders)]}


def give_oil(to: str, n: int = 1) -> Dict[str, Any]:
    m = {"item": "item:oil", "from": "merchant", "to": to, "quantity": n}
    return {"op": "transfer", "revision": 0, "moves": [m], "allow": [m]}


def evidence_records(n: int) -> List[Dict[str, Any]]:
    return [{"id": f"clue_{i}", "value": f"clue {i}", "depends": [f"clue_{i - 1}"] if i % 4 else [],
             "priority": i % 7, "required": i == 0} for i in range(n)]


SOURCE = {"id": "line:1", "speaker": "player", "text": "The oil belongs to Fen."}


def remember(i: int) -> Dict[str, Any]:
    return {"name": "info_remember", "text": enc({"subject": f"item:{i}", "attribute": "owner", "value": "Fen",
                                                  "source": "line:1"}), "sources": [SOURCE]}


LOOKUP = {"name": "info_lookup", "text": enc({"subject": "item:0", "attribute": "owner"}), "sources": []}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    add("lk_places_echo_defaults", "An empty limits object echoes the codex defaults; routes are unchanged.",
        req("places", {"codex": VALE, "queries": [FROM_BAKERY]}, {}))
    add("lk_places_many_places", "Raising places to 200 accepts a codex with 154 places (the old bound was 128).",
        req("places", {"codex": vale_with_fields(140), "queries": [FROM_BAKERY, {"from": "field_139"}]}, {"places": 200}))
    add("lk_places_bytes_refused", "A codex larger than the host's bytes limit is refused before any query.",
        req("places", {"codex": VALE, "queries": [FROM_BAKERY]}, {"bytes": 512}), error=True)
    add("lk_timeline_entries_limit", "With entries 2 the third record fails and changes nothing; queries still run.",
        req("timeline", {"timeline": None, "ops": [record(1, 1), record(2, 2), record(3, 3),
                                                   op("query", order="oldest")]}, {"entries": 2}))
    add("lk_timeline_null_echoes", "A null limits value counts as supplied: defaults are echoed.",
        req("timeline", {"timeline": None, "ops": [record(5)]}, None))
    add("lk_timeline_zero_refused", "A zero entries limit is outside 1..ceiling and refuses the request.",
        req("timeline", {"timeline": None, "ops": [record(5)]}, {"entries": 0}), error=True)
    add("lk_things_things_limit", "At the things limit, create fails while moves of existing things still work.",
        req("things", {"things": THINGS, "ops": [
            op("create", id="spare_axe", kind="wood_axe", location={"place": "forge"}),
            op("move", thing="starter_axe", to_place="square")]}, {"things": len(THINGS["things"])}))
    add("lk_things_many_kinds", "Raising kinds to 600 loads 520 thing kinds (the old bound was 512).",
        req("things", {"things": things_with_kinds(520 - len(THINGS["kinds"])), "ops": [
            op("create", id="trinket_box", kind="trinket_0", location={"place": "forge"}, quantity=3)]}, {"kinds": 600}))
    add("lk_things_unknown_limit_refused", "An unknown limit name refuses the request.",
        req("things", {"things": THINGS, "ops": []}, {"wagons": 3}), error=True)
    add("lk_species_many_animals", "Raising animals to 300 loads a 263-animal herd; a pet still resolves.",
        req("species", {"species": SPECIES, "animals": herd(260), "ops": [
            op("pet", actor="farmer", animal=f"{ANIMALS[0]['id']}_259", place="pasture")]}, {"animals": 300}))
    add("lk_species_animals_over_limit", "A herd larger than the animals limit is refused at load.",
        req("species", {"species": SPECIES, "animals": ANIMALS, "ops": []}, {"animals": 1}), error=True)
    add("lk_recipes_many_recipes", "Raising recipes to 300 loads 263 recipes (the old bound was 256).",
        req("recipes", {"recipes": recipe_book(260 - len(RECIPES) + 3), "things": recipes_cases.BASE_THINGS, "ops": [
            op("check", recipe=f"{RECIPES[0]['id']}_1", actor="smith", place="forge")]}, {"recipes": 300}))
    add("lk_recipes_echo_low_limit", "A low recipes limit is echoed and an empty book still answers.",
        req("recipes", {"ops": [op("check", recipe="craft_sword", actor="smith", place="forge")]}, {"recipes": 1}))
    add("lk_communities_members_limit", "At a full members limit an existing member is a no-op; a smaller "
        "community still takes a new member.",
        req("communities", {"communities": COMMUNITIES, "ops": [
            op("add_member", community="ember_wardens", actor="warden_kael"),
            op("add_member", community="iron_clan", actor="squire_ivo")]}, {"members": 2}))
    add("lk_communities_relations_limit", "At a full relations limit an existing relation still updates.",
        req("communities", {"communities": COMMUNITIES, "ops": [
            op("set_relation", community="ember_wardens", target="iron_clan", stance="allied", trust=60),
            op("check_relation", community="ember_wardens", target="iron_clan")]}, {"relations": 3}))
    add("lk_communities_many_communities", "Raising communities to 80 loads 70 (the old bound was 64).",
        req("communities", {"communities": guilds(70 - len(COMMUNITIES)), "ops": [
            op("add_member", community="guild_0", actor="apprentice_tam")]}, {"communities": 80}))
    add("lk_actions_register_within_limit", "With actions 51 (49 built in) two custom actions fit.",
        req("actions", {"ops": [op("register", action=action(1)), op("register", action=action(2)),
                                op("describe", id="chore_2")]}, {"actions": 51}))
    add("lk_lore_many_entries", "Raising entries to 600 loads 540 lore entries (the old bound was 512).",
        req("lore", {"world": lore_with(540 - len(LORE["entries"])), "select": LORE_SELECT},
            {"entries": 600, "bytes": 1048576}))
    add("lk_inventory_many_holdings", "Raising holdings to 400 loads 301 holdings and transfers into a new one.",
        req("inventory", {"snapshot": stock(300), "calls": [give_oil("villager_7"),
                                                            {"op": "rows", "owners": ["villager_7"], "tags": []}]},
            {"holdings": 400}))
    add("lk_inventory_holdings_full", "A transfer that would add a holding past the limit fails atomically.",
        req("inventory", {"snapshot": stock(3), "calls": [give_oil("villager_1"), dict(give_oil("merchant_ana"), revision=1),
                                                          {"op": "rows", "owners": ["villager_1", "merchant_ana"],
                                                           "tags": []}]}, {"holdings": 5}))
    add("lk_inventory_holdings_over_limit", "A snapshot with more holdings than the limit is refused.",
        req("inventory", {"snapshot": stock(5), "calls": []}, {"holdings": 3}), error=True)
    add("lk_evidence_many_records", "Raising records to 256 selects from 200 dependency-linked clues.",
        req("evidence", {"records": evidence_records(200), "max_bytes": 400}, {"records": 256}))
    add("lk_evidence_above_ceiling", "A records limit above its ceiling (1024) refuses the request.",
        req("evidence", {"records": evidence_records(4), "max_bytes": 400}, {"records": 2000}), error=True)
    add("lk_info_notes_limit", "With notes 2 the third remember fails; lookups still answer.",
        req("info", {"scope": "talk", "calls": [remember(0), remember(1), remember(2), LOOKUP]}, {"notes": 2}))
    add("lk_info_many_notes", "Raising notes to 100 keeps 70 notes (the old bound was 64).",
        req("info", {"scope": "talk", "calls": [remember(i) for i in range(70)] + [LOOKUP]}, {"notes": 100}))
    return cases


# --- randomized parity ---

def rand_limits(rng: random.Random, kind: str, small: Dict[str, int]) -> Dict[str, Any]:
    names = NAMES[kind]
    lim: Dict[str, Any] = {}
    for n in rng.sample(names, rng.randint(1, len(names))):
        lo = small.get(n, 2)  # about the fixture's own size: limits near it refuse growth, not loading
        lim[n] = rng.randint(max(1, lo - 2), lo + 3) if rng.random() < 0.6 else rng.randint(lo + 4, 4 * lo + 300)
    if rng.random() < 0.02:
        lim["nonsense"] = 1
    if rng.random() < 0.02:
        lim[rng.choice(names)] = rng.choice([0, -1, "3", 1.5, 10**9])
    return lim


def rand_places(rng):
    n = rng.randint(0, 40)
    queries = [{"from": rng.choice(["bakery", "keep", "clearing", f"field_{rng.randint(0, 50)}"])}
               for _ in range(rng.randint(1, 3))]
    return "places", {"codex": vale_with_fields(n), "queries": queries}, {"places": 14 + n, "bytes": 3000 + 80 * n}


def rand_timeline(rng):
    ops = []
    for i in range(rng.randint(1, 10)):
        r = rng.random()
        if r < 0.6:
            ops.append(record(rng.randint(0, 50), i))
        elif r < 0.75:
            ops.append(op("retract", seq=rng.randint(1, 6), reason="mistaken"))
        elif r < 0.85:
            ops.append(op("compact", through=rng.randint(1, 6), summary="Early harvests."))
        else:
            ops.append(op("query", known_to="miller", limit=rng.randint(1, 5)))
    return "timeline", {"timeline": None, "ops": ops}, {"entries": 4, "bytes": 2000}


def rand_things(rng):
    t = things_with_kinds(rng.randint(0, 6))
    ops = []
    for i in range(rng.randint(1, 8)):
        if rng.random() < 0.6:
            ops.append(op("create", id=f"new_{rng.randint(0, 5)}",
                          kind=rng.choice([k["id"] for k in t["kinds"]]), location={"place": "forge"}))
        else:
            ops.append(op("whats_here", place=rng.choice(["forge", "square"])))
    return "things", {"things": t, "ops": ops}, {"materials": len(t["materials"]), "kinds": len(t["kinds"]),
                                                 "things": len(t["things"]) + 2, "bytes": len(enc(t))}


def rand_species(rng):
    animals = herd(rng.randint(0, 8))
    ops = [op("pet", actor="farmer", animal=rng.choice(animals)["id"], place="pasture") for _ in range(rng.randint(1, 3))]
    return "species", {"species": SPECIES, "animals": animals, "ops": ops}, {"species": len(SPECIES),
                                                                           "animals": len(animals)}


def rand_recipes(rng):
    book = recipe_book(rng.randint(0, 6))
    ops = [op("check", recipe=rng.choice(book)["id"], actor="smith", place="forge") for _ in range(rng.randint(1, 3))]
    return "recipes", {"recipes": book, "things": recipes_cases.BASE_THINGS, "ops": ops}, {"recipes": len(book)}


def rand_communities(rng):
    comms = guilds(rng.randint(0, 6))
    ops = []
    for i in range(rng.randint(1, 8)):
        c = rng.choice(comms)["id"]
        r = rng.random()
        if r < 0.4:
            ops.append(op("add_member", community=c, actor=f"villager_{rng.randint(0, 6)}"))
        elif r < 0.7:
            ops.append(op("set_relation", community=c, target=rng.choice(["iron_clan", "river_folk", "guild_1"]),
                          stance=rng.choice(["friendly", "wary"]), trust=rng.randint(-50, 50)))
        else:
            ops.append(op("set_customs", community=c, customs=[f"custom {j}" for j in range(rng.randint(0, 6))]))
    return "communities", {"communities": comms, "ops": ops}, {"communities": len(comms), "members": 3, "customs": 3,
                                                               "relations": 3, "rules": 3}


def rand_actions(rng):
    ops = []
    for i in range(rng.randint(1, 6)):
        if rng.random() < 0.7:
            ops.append(op("register", action=action(rng.randint(0, 6))))
        else:
            ops.append(op("describe", id=f"chore_{rng.randint(0, 6)}"))
    args: Dict[str, Any] = {"ops": ops}
    if rng.random() < 0.3:
        args["actions"] = [action(100 + j) for j in range(rng.randint(0, 4))]
    return "actions", args, {"actions": 50}


def rand_lore(rng):
    w = lore_with(rng.randint(0, 20))
    sel = dict(LORE_SELECT, limit=rng.randint(1, 6), query=rng.choice(["tonic purpose", "vault door", "oil"]))
    return "lore", {"world": w, "select": sel}, {"entries": len(w["entries"]), "bytes": len(enc(w).encode())}


def rand_inventory(rng):
    s = stock(rng.randint(0, 8))
    calls = [give_oil(rng.choice(["villager_1", "merchant_ana", "player"]), rng.randint(1, 3))
             if rng.random() < 0.7 else {"op": "rows", "owners": ["merchant", "player"], "tags": []}
             for _ in range(rng.randint(1, 4))]
    for c in calls:
        if c["op"] == "transfer":
            c["revision"] = rng.randint(0, 1)
    return "inventory", {"snapshot": s, "calls": calls}, {"items": 2, "holdings": len(s["holdings"]),
                                                          "bytes": len(enc(s).encode())}


def rand_evidence(rng):
    rec = evidence_records(rng.randint(1, 30))
    return "evidence", {"records": rec, "max_bytes": rng.randint(20, 600)}, {"records": len(rec),
                                                                             "bytes": len(enc(rec).encode())}


def rand_info(rng):
    calls = []
    for _ in range(rng.randint(1, 10)):
        r = rng.random()
        calls.append(remember(rng.randint(0, 4)) if r < 0.7 else
                     LOOKUP if r < 0.85 else {"name": "info_forget", "text": enc({"id": rng.randint(1, 5)}), "sources": []})
    return "info", {"scope": "talk", "calls": calls}, {"notes": 4, "bytes": 2000}


GENERATORS = [rand_places, rand_timeline, rand_things, rand_species, rand_recipes, rand_communities, rand_actions,
              rand_lore, rand_inventory, rand_evidence, rand_info]


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    kind, args, small = rng.choice(GENERATORS)(rng)
    if rng.random() < 0.85:
        return req(kind, args, None if rng.random() < 0.03 else rand_limits(rng, kind, small))
    return req(kind, args)


def generate_conformance_cases() -> None:
    cases = make_cases()
    for c in cases:
        d = CONFORMANCE_DIR / c["id"]
        d.mkdir(parents=True, exist_ok=True)
        meta = {"op": "local", "description": c["description"], "error": c["error"]}
        (d / "case.json").write_text(json.dumps(meta, indent=2) + "\n")
        # Inputs are compact: several cases carry hundreds of rows to pass an old bound.
        (d / "input.json").write_text(json.dumps(c["input"], ensure_ascii=False, separators=(",", ":")) + "\n")
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
            out, err = py_workflow(c["input"]), None
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
    print(json.dumps({"cases": len(cases), "malformed": sum(1 for c in cases if c["error"]), "passed": passed,
                      "actual_model_calls": 0, "api_spend_usd": 0}))
    return 0 if passed == len(cases) else 1


def run_random_parity(count: int, seed: int) -> int:
    rng = random.Random(seed)
    rust_bin = ROOT / "target" / "debug" / "gamectx"
    if not rust_bin.exists():
        subprocess.run(["cargo", "build"], cwd=ROOT, check=True)
    ok_runs = err_runs = limited = 0
    kinds: Dict[str, int] = {}
    for i in range(1, count + 1):
        r = rand_workflow(rng)
        try:
            py_res, py_err = py_workflow(r), None
        except Exception as e:  # noqa: BLE001
            py_res, py_err = None, e
        payload = json.dumps(r)
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
        kinds[r["kind"]] = kinds.get(r["kind"], 0) + 1
    print(json.dumps({"random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
                      "with_limits": limited, "accepted_by_kind": dict(sorted(kinds.items())),
                      "actual_model_calls": 0, "api_spend_usd": 0}))
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
