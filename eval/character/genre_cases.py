#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 4 stage 1: genre profiles and
lexicon packs, local kind `genre`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/genre/*/case.json + input.json
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
from genre_reference import BUILTIN_GENRES, BUILTIN_PACKS, ENGINE, SLOT_NAMES, full_vocab
from genre_reference import workflow as genre_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "genre"
LIMIT_NAMES = ["genres", "packs", "nodes"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "genre":
        raise ValueError("unknown local kind")
    return genre_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "genre", "args": args}


KEYS = {"access": "access", "privacy": "privacy", "light": "light", "kind": "contract", "severity_name": "severity",
        "address_a_to_b": "address"}
DECISION = {"access": "private", "allowed": False, "place": {"privacy": "semi_private", "earshot": "nearby"},
            "light": "dim", "contracts": [{"kind": "loan", "weight": 20}, {"kind": "alibi_pact", "weight": 60}],
            "severity_name": "crime", "address_a_to_b": "stranger"}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("list_builtins", "Four built-in genres and three lexicon packs.", wf(op("list")))
    case("show_orbital_station", "The station's words for every slot, its axes and an artificial day.",
         wf(op("show", id="orbital_station")))
    case("show_village_plain", "The village uses the engine words: nothing customized.", wf(op("show", id="village")))
    case("render_station_decision",
         "One refused entry, a theft and two contracts in station words; structure and non-vocabulary fields are untouched.",
         wf(op("render", genre="orbital_station", keys=KEYS, value=DECISION)))
    case("render_noir_decision", "The same decision in noir words.", wf(op("render", genre="noir_city", keys=KEYS, value=DECISION)))
    case("render_counts_unmapped",
         "A word that is not an engine word of its slot is left as it is and counted.",
         wf(op("render", genre="academy", keys={"access": "access"},
               value={"access": ["public", "backstage", "restricted"], "note": {"access": "private"}})))
    case("normalize_noir_back", "Noir words map back to engine words for the engines.",
         wf(op("normalize", genre="noir_city", keys={"privacy": "privacy", "kind": "contract"},
               value={"privacy": "back_room", "kind": "keep_mum"})))
    case("normalize_academy_address", "Address terms are free text: 'new kid' is the engine's stranger.",
         wf(op("normalize", genre="academy", keys={"address": "address"}, value={"address": ["new kid", "classmate"]})))
    case("round_trip_same_decision",
         "Rendering into academy words and normalizing back gives the engine decision unchanged.",
         wf(op("render", genre="academy", keys=KEYS, value=DECISION),
            op("normalize", genre="academy", keys=KEYS,
               value={"access": "private", "allowed": False, "place": {"privacy": "study_corner", "earshot": "nearby"},
                      "light": "dim", "contracts": [{"kind": "borrowed_notes", "weight": 20},
                                                    {"kind": "cover_story", "weight": 60}],
                      "severity_name": "expellable", "address_a_to_b": "new kid"})))
    case("portability_four_genres",
         "The same engine outcome rendered under all four built-in genres.",
         wf(*[op("render", genre=g, keys={"access": "access", "severity_name": "severity"},
                 value={"access": "restricted", "severity_name": "taboo"}) for g in BUILTIN_GENRES]))
    case("define_from_station",
         "A mining outpost starts from the station and renames only its light; the rest is inherited.",
         wf(op("define", id="mining_outpost", base="orbital_station", vocab={"light": ["blackout", "lamps", "floodlights"]},
               axes={"scarcity": 90}), op("show", id="mining_outpost")))
    case("define_axes_cycle_and_replace",
         "A desert caravan with no day cycle, redefined from itself with new access words (keeping its axes and cycle).",
         wf(op("define", id="caravan", day_cycle="none", axes={"tech": 10, "law_reach": 5}),
            op("define", id="caravan", base="caravan", vocab={"access": ["open_camp", "traders_only", "kin_only", "tent", "forbidden"]}),
            op("show", id="caravan"), op("list")))
    case("render_scalars_under_keys",
         "Numbers, booleans and nulls under a mapped key are left alone.",
         wf(op("render", genre="noir_city", keys={"light": "light"}, value={"light": [2, True, None, "bright"]})))
    case("pack_and_requests",
         "An author pack for a harbor town, turned into pragmatics lexicon requests after a built-in pack.",
         wf(op("pack", id="harbor_talk", phrases={"greeting": ["fair winds"], "threat": ["walk the plank"],
                                                   "compliment": ["shipshape"]}),
            op("lexicon_requests", packs=["noir_street", "harbor_talk"])))
    case("lexicon_requests_station", "The station crew pack as one pragmatics request.",
         wf(op("lexicon_requests", packs=["station_crew"])))
    case("nodes_at_limit", "Exactly as many nodes as the limit allows.",
         wf(op("render", genre="village", keys={"a": "light"}, value={"a": ["dim", "dark"]}), limits={"nodes": 4}))
    case("genre_limit_replace", "With room for one authored genre, redefining it is fine.",
         wf(op("define", id="a"), op("define", id="a", base="noir_city"), op("show", id="a"), limits={"genres": 1}))
    case("bad_normalize_unknown_word", "Malformed: normalizing refuses a word outside the genre's vocabulary.",
         wf(op("normalize", genre="noir_city", keys={"privacy": "privacy"}, value={"privacy": "penthouse"})), error=True)
    case("bad_vocab_arity", "Malformed: light needs exactly three names.",
         wf(op("define", id="x", vocab={"light": ["dark", "bright"]})), error=True)
    case("bad_vocab_duplicate", "Malformed: names in a slot are unique.",
         wf(op("define", id="x", vocab={"light": ["dark", "dark", "bright"]})), error=True)
    case("bad_redefine_builtin", "Malformed: built-in genres cannot be replaced.",
         wf(op("define", id="village", axes={"tech": 99})), error=True)
    case("bad_pack_phrase_case", "Malformed: pack phrases are lower-case words separated by single spaces.",
         wf(op("pack", id="p", phrases={"greeting": ["Fair Winds"]})), error=True)
    case("bad_pack_duplicate_across_acts", "Malformed: one phrase cannot belong to two acts in a pack.",
         wf(op("pack", id="p", phrases={"greeting": ["ahoy"], "farewell": ["ahoy"]})), error=True)
    case("bad_nodes_over_limit", "Malformed: one node more than the limit is refused.",
         wf(op("render", genre="village", keys={"a": "light"}, value={"a": ["dim", "dark", "dim"]}), limits={"nodes": 4}),
         error=True)
    case("bad_unknown_slot", "Malformed: keys must name a slot.",
         wf(op("render", genre="village", keys={"mood": "weather"}, value={"mood": "dim"})), error=True)
    return cases


def rlimits(rng: random.Random) -> Optional[Dict[str, int]]:
    if rng.random() > 0.3:
        return None
    lim = {"genres": rng.randint(1, 3), "packs": rng.randint(1, 3), "nodes": rng.choice([5, 20, 60])}
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


GENRE_IDS = list(BUILTIN_GENRES) + ["custom_a", "custom_b"]
KEY_POOL = list(KEYS.items()) + [("mood", "light"), ("zone", "access")]


def rand_word(rng: random.Random, slot: str) -> Any:
    pool = list(ENGINE[slot])
    for g in BUILTIN_GENRES.values():
        pool += full_vocab(g["vocab"])[slot]
    return rng.choice(pool + ["glitter", 7, None, True])


def rand_value(rng: random.Random, depth: int = 0) -> Any:
    roll = rng.random()
    if depth > 3 or roll < 0.3:
        return rng.choice(["private", "dim", "loan", "booth", 3, False, None, "x"])
    if roll < 0.7:
        out = {}
        for k, slot in rng.sample(KEY_POOL, rng.randint(1, 4)):
            out[k] = rand_word(rng, slot) if rng.random() < 0.6 else rand_value(rng, depth + 1)
        if rng.random() < 0.3:
            out["other"] = rand_value(rng, depth + 1)
        return out
    return [rand_value(rng, depth + 1) for _ in range(rng.randint(0, 3))]


def rand_op(rng: random.Random) -> Dict[str, Any]:
    roll = rng.random()
    g = rng.choice(GENRE_IDS)
    if roll < 0.05:
        return op("list")
    if roll < 0.13:
        return op("show", id=g)
    if roll < 0.30:
        a: Dict[str, Any] = {"id": rng.choice(["custom_a", "custom_b", "village"])}
        if rng.random() < 0.5:
            a["base"] = rng.choice(GENRE_IDS + ["nowhere"])
        if rng.random() < 0.4:
            a["axes"] = {k: rng.choice([0, 50, 100, 101]) for k in rng.sample(["tech", "magic", "law_reach", "scarcity"], 2)}
        if rng.random() < 0.3:
            a["day_cycle"] = rng.choice(["natural", "artificial", "none", "eternal"])
        if rng.random() < 0.6:
            slot = rng.choice(SLOT_NAMES)
            n = len(ENGINE[slot]) + rng.choice([0, 0, 0, -1, 1])
            names = [f"w{i}" for i in range(max(n, 0))] if slot != "address" else [f"Term {i}" for i in range(max(n, 0))]
            if names and rng.random() < 0.1:
                names[-1] = names[0]
            a["vocab"] = {slot: names}
        return op("define", **a)
    if roll < 0.40:
        acts = rng.sample(["greeting", "farewell", "threat", "compliment", "chatter"], rng.randint(1, 2))
        return op("pack", id=rng.choice(["p1", "p2", "station_crew"]),
                  phrases={act: rng.sample(["ahoy there", "fair winds", "Ahoy", "walk the plank", "ahoy there"], rng.randint(1, 2))
                           for act in acts})
    if roll < 0.48:
        return op("lexicon_requests", packs=rng.sample(list(BUILTIN_PACKS) + ["p1", "p2"], rng.randint(1, 3)))
    keys = dict(rng.sample(KEY_POOL, rng.randint(1, 4)))
    if rng.random() < 0.05:
        keys["bad"] = "weather"
    return op(rng.choice(["render", "render", "normalize"]), genre=g, keys=keys, value=rand_value(rng))


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
        ops.append(op("list"))
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
    ok_runs = err_runs = rendered = unmapped = normalized = defined = 0
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
            rendered += int("unmapped" in r)
            unmapped += r.get("unmapped", 0)
            normalized += int("value" in r and "unmapped" not in r)
            defined += int("customized" in r and "vocab" not in r)
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "renders": rendered, "unmapped_words": unmapped, "normalizations": normalized, "definitions": defined,
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
        sys.exit("genre checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
