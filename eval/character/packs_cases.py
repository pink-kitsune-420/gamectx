#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 2 D5: default packs (species,
temperaments, roles and norm sets that become plain engine requests; author-defined packs),
local kind `packs`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/packs/*/case.json + input.json
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
from packs_reference import workflow as packs_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "packs"
LIMIT_NAMES = ["custom", "roles", "norms"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "packs":
        raise ValueError("unknown local kind")
    return packs_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "packs", "args": args}


def apply(being: str, **a: Any) -> Dict[str, Any]:
    return op("apply", being=being, **a)


MULE = {"lifespan": 262800, "needs": [{"id": "hunger", "action": "feed", "threshold": 65, "rate_per_tick": 2},
                                      {"id": "load_rest", "action": "unload", "label": "Rest from loads"}]}
SEA = {"tenets": [{"id": "sea_no_waste_water", "action": "take", "severity": 70, "label": "Never waste fresh water"}]}
FERRYMAN = {"status": 25, "seats": 2, "duties": [{"id": "crossing", "action": "travel", "every": 4, "place": "dock"}]}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("packs_human_bold_baker",
         "A bold human baker of the village: body, birth, cognition, three traits, the baker role and two norms.",
         wf(apply("mira", species="human", temperament="bold", roles=["baker"], norms=["village"])))
    case("packs_dog_steady",
         "A steady dog: dog needs (feeding, rest, petting), a dog lifespan and a steady mind.",
         wf(apply("rook", species="dog", temperament="steady")))
    case("packs_goat_timid",
         "A timid goat: the threat hint is timid and boldness starts at -600,000.",
         wf(apply("nell", species="goat", temperament="timid")))
    case("packs_dreamy_mind_only",
         "A temperament alone gives cognition and drift requests and no body or birth.",
         wf(apply("ivo", temperament="dreamy")))
    case("packs_cautious_guard",
         "A cautious guard: careful cognition, cautious threat hint, a patrol duty at the gate.",
         wf(apply("bram", temperament="cautious", roles=["guard"])))
    case("packs_elder_has_seats",
         "The elder role carries its three seats into the roles request.",
         wf(apply("oda", roles=["elder"])))
    case("packs_two_roles",
         "Two roles are defined first, then both are assigned.",
         wf(apply("sela", roles=["baker", "healer"])))
    case("packs_two_norm_sets",
         "Village and wanderer norms merge into one set_norms request of four tenets.",
         wf(apply("tam", norms=["village", "wanderer"])))
    case("packs_list_species",
         "The four built-in species, no custom ones yet.",
         wf(op("list", kind="species")))
    case("packs_list_roles_with_custom",
         "A custom ferryman role is listed apart from the built-in roles.",
         wf(op("define", kind="role", id="ferryman", pack=FERRYMAN), op("list", kind="role")))
    case("packs_show_builtin_temperament",
         "show returns the normalized bold temperament.",
         wf(op("show", kind="temperament", id="bold")))
    case("packs_define_species_fills_defaults",
         "A custom mule species: missing labels, rates, thresholds and amounts take their defaults.",
         wf(op("define", kind="species", id="mule", pack=MULE), op("show", kind="species", id="mule")))
    case("packs_apply_custom_species",
         "A defined species is applied like a built-in one.",
         wf(op("define", kind="species", id="mule", pack=MULE), apply("jem", species="mule", temperament="steady")))
    case("packs_temperament_without_traits",
         "A custom temperament with no traits gives a cognition request and no drift request.",
         wf(op("define", kind="temperament", id="placid", pack={"traits": [], "cognition": "steady"}),
            apply("lio", temperament="placid")))
    case("packs_redefine_does_not_count",
         "With room for one custom pack, redefining the same id is still accepted.",
         wf(op("define", kind="norms", id="sea", pack=SEA),
            op("define", kind="norms", id="sea", pack={"tenets": SEA["tenets"] + [
                {"id": "sea_share_catch", "action": "store", "severity": 30, "label": "Share the catch"}]}),
            op("show", kind="norms", id="sea"), limits={"custom": 1}))
    case("packs_raise_roles_limit",
         "The host raises the roles per apply to four.",
         wf(apply("una", roles=["guard", "baker", "farmer", "healer"]), limits={"roles": 4}))
    case("packs_custom_norms_merge",
         "A custom sea norm set merges with the village set.",
         wf(op("define", kind="norms", id="sea", pack=SEA), apply("cato", norms=["sea", "village"])))
    case("packs_custom_role_assign",
         "A custom role with seats and a duty without a set time.",
         wf(op("define", kind="role", id="ferryman", pack=FERRYMAN),
            apply("vos", species="human", roles=["ferryman"])))

    case("bad_unknown_species",
         "Applying a species that is neither built in nor defined is refused.",
         wf(apply("x", species="dragon")), error=True)
    case("bad_replace_builtin",
         "Built-in ids cannot be redefined.",
         wf(op("define", kind="species", id="human", pack=MULE)), error=True)
    case("bad_unknown_action",
         "Duties and needs must use a known action.",
         wf(op("define", kind="role", id="flier", pack={"status": 10, "duties": [
             {"id": "fly", "action": "fly", "every": 2}]})), error=True)
    case("bad_too_many_roles",
         "Three roles exceed the default two per apply.",
         wf(apply("x", roles=["guard", "baker", "farmer"])), error=True)
    case("bad_duplicate_tenet",
         "Merged norm sets may not repeat a tenet id.",
         wf(op("define", kind="norms", id="copy", pack={"tenets": [
             {"id": "village_no_theft", "action": "take", "severity": 10, "label": "Again"}]}),
            apply("x", norms=["village", "copy"])), error=True)
    case("bad_empty_apply",
         "apply needs at least one pack.",
         wf(apply("x")), error=True)
    case("bad_custom_full",
         "With room for one custom pack, a second id is refused.",
         wf(op("define", kind="norms", id="sea", pack=SEA), op("define", kind="role", id="ferryman", pack=FERRYMAN),
            limits={"custom": 1}), error=True)
    return cases


BEINGS = ["ann", "bo", "cy"]
SPECIES = ["human", "dog", "horse", "goat", "mule", "imp"]
TEMPERS = ["bold", "steady", "cautious", "timid", "dreamy", "placid", "odd"]
ROLES = ["guard", "baker", "farmer", "elder", "healer", "ferryman", "smith"]
NORMS = ["village", "guild", "wanderer", "sea", "copy"]
ACTS = ["feed", "wait", "talk", "move", "cook", "fly", "take", "store"]


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


def rand_pack(rng: random.Random, kind: str) -> Dict[str, Any]:
    if kind == "species":
        needs = []
        for i in range(rng.randint(0 if rng.random() < 0.05 else 1, 3)):
            n: Dict[str, Any] = {"id": rng.choice(["hunger", "rest", "roam", f"n{i}"]), "action": rng.choice(ACTS)}
            for k, lo, hi in [("threshold", 0, 101), ("rate_per_tick", 0, 100), ("amount", 0, 100)]:
                if rng.random() < 0.4:
                    n[k] = rng.randint(lo, hi)
            if rng.random() < 0.3:
                n["label"] = rng.choice(["Hunger", "Long rest", "", "x" * 41])
            needs.append(n)
        return {"lifespan": rng.choice([1, 8760, 0, rng.randint(1, 1_000_000)]), "needs": needs}
    if kind == "temperament":
        p: Dict[str, Any] = {"traits": [{"trait": rng.choice(["boldness", "trust", "warmth", "greed"]),
                                         "base": rng.choice([0, 1_000_000, -1_000_000, 1_000_001, rng.randint(-M, M)])}
                                        for _ in range(rng.randint(0, 3))]}
        if rng.random() < 0.5:
            p["cognition"] = rng.choice(["careful", "dreamy", "impulsive", "steady", "manic"])
        if rng.random() < 0.5:
            p["survival"] = rng.choice(["bold", "cautious", "steady", "timid", "brave"])
        return p
    if kind == "role":
        p = {"status": rng.choice([0, 50, 100, 101])}
        if rng.random() < 0.3:
            p["seats"] = rng.choice([1, 2, 0])
        duties = []
        for i in range(rng.randint(0, 2)):
            d: Dict[str, Any] = {"id": rng.choice(["work", f"d{i}"]), "action": rng.choice(ACTS),
                                 "every": rng.choice([1, 6, 24, 0])}
            for k, v in [("at", rng.randint(0, 30)), ("place", rng.choice(["gate", "dock"])),
                         ("priority", rng.randint(0, 101))]:
                if rng.random() < 0.4:
                    d[k] = v
            duties.append(d)
        p["duties"] = duties
        return p
    return {"tenets": [{"id": rng.choice(["village_no_theft", "t1", "t2", "t3"]), "action": rng.choice(ACTS),
                        "severity": rng.choice([1, 50, 100, 0]), "label": rng.choice(["Be fair", "Share", ""])}
                       for _ in range(rng.randint(0 if rng.random() < 0.05 else 1, 3))]}


M = 1_000_000
CUSTOM_IDS = {"species": ["mule", "imp", "human"], "temperament": ["placid", "odd"],
              "role": ["ferryman", "smith", "guard"], "norms": ["sea", "copy"]}


def rand_op(rng: random.Random) -> Dict[str, Any]:
    roll = rng.random()
    kind = rng.choice(["species", "temperament", "role", "norms"])
    if roll < 0.3:
        return op("define", kind=kind, id=rng.choice(CUSTOM_IDS[kind]), pack=rand_pack(rng, kind))
    if roll < 0.38:
        return op("list", kind=rng.choice([kind, kind, "spell"]))
    if roll < 0.46:
        ids = {"species": SPECIES, "temperament": TEMPERS, "role": ROLES, "norms": NORMS}[kind]
        return op("show", kind=kind, id=rng.choice(ids))
    a: Dict[str, Any] = {"being": rng.choice(BEINGS)}
    if rng.random() < 0.5:
        a["species"] = rng.choice(SPECIES)
    if rng.random() < 0.5:
        a["temperament"] = rng.choice(TEMPERS)
    if rng.random() < 0.5:
        a["roles"] = rng.sample(ROLES, rng.randint(0, 3))
    if rng.random() < 0.4:
        a["norms"] = rng.sample(NORMS, rng.randint(0, 2))
    return op("apply", **a)


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = []
    target = rng.randint(1, 16)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        ops.append(rand_op(rng))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
    if not ops:
        ops.append(op("list", kind="species"))
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
    ok_runs = err_runs = applies = requests = defines = 0
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
            if isinstance(r, dict) and "requests" in r:
                applies += 1
                requests += len(r["requests"])
            elif isinstance(r, dict) and "pack" in r and "builtin" not in r:
                defines += 1
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "applies": applies, "requests_emitted": requests, "custom_defines": defines,
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
        sys.exit("packs checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
