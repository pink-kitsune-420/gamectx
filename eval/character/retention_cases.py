#!/usr/bin/env python3
"""Conformance cases, differential parity runner and save-growth soak for pass 2 D3: retention
rules and data migration, local kind `retention`.

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/retention/*/case.json + input.json
  --random N --seed S     N randomized workflows, Python reference vs `gamectx local -`
  --soak                  30-day town soak: kept bytes with and without retention rules
                          (checked against eval/character/retention-soak.json when present)

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
from retention_reference import Retention, workflow as keep_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "retention"
LIMIT_NAMES = ["collections", "entries", "migrations", "changes"]


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    if request.get("kind") != "retention":
        raise ValueError("unknown local kind")
    return keep_workflow(request.get("args", {}))


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def wf(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": list(ops)}
    if limits is not None:
        args["limits"] = limits
    return {"kind": "retention", "args": args}


def add(c: str, i: str, size: int, importance: Optional[int] = None, **extra: Any) -> Dict[str, Any]:
    a: Dict[str, Any] = {"collection": c, "id": i, "bytes": size, **extra}
    if importance is not None:
        a["importance"] = importance
    return op("add", **a)


def memories(**rule: Any) -> List[Dict[str, Any]]:
    """A miller's memories: an old fire (important), a rainy day, gossip, a fair and a debt."""
    return [op("rule", collection="memories", **rule),
            add("memories", "fire", 400, 90),
            add("memories", "rain", 120, 10),
            add("memories", "gossip", 200, 30, now=20),
            add("memories", "fair", 300, 60, now=40),
            add("memories", "debt", 150, 70, pinned=True, now=45)]


BAKERY_V1 = {"baker": {"mood": 7, "flour": 3, "stock": [{"loaf": "rye", "count": 140}, {"loaf": "wheat", "count": -5}]},
             "miller": {"mood": 2, "flour": 40}}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    case("keep_stale_entries_go",
         "With max_age 30, at tick 60 rain, fire (last used at 0) and gossip (at 20) are stale; fair and the pinned debt stay.",
         wf(*memories(max_age=30), op("prune", now=60)))

    case("keep_touch_refreshes",
         "Recalling the fire at tick 50 keeps it from going stale at tick 60.",
         wf(*memories(max_age=30), op("touch", collection="memories", id="fire", now=50), op("prune", now=60)))

    case("keep_unimportant_go",
         "min_importance 40 removes rain (10) and gossip (30) whatever their age.",
         wf(*memories(min_importance=40), op("prune", now=45)))

    case("keep_over_entries_least_important_first",
         "max_entries 3 removes the least important, then least recently used, until three remain.",
         wf(*memories(max_entries=3), op("prune", now=45)))

    case("keep_over_bytes",
         "max_bytes 700 removes rain, gossip and fair in importance order until the 550 bytes of fire and debt fit.",
         wf(*memories(max_bytes=700), op("prune", now=45)))

    case("keep_grace_protects_recent",
         "grace 10 protects fair (used at 40) and the debt is pinned, so max_entries 1 cannot be met and the collection is over.",
         wf(*memories(max_entries=1, grace=10), op("prune", now=45)))

    case("keep_pinned_never_removed",
         "Pinned entries survive every reason; unpinning the debt lets it go.",
         wf(*memories(max_entries=1, max_age=0), op("prune", now=100),
            op("touch", collection="memories", id="debt", pinned=False, now=100), op("prune", now=101)))

    case("keep_dry_run_changes_nothing",
         "A dry run reports the removals and the clock and entries stay; the real prune then removes the same.",
         wf(*memories(max_entries=2), op("prune", now=45, dry_run=True), op("assess", collection="memories"),
            op("prune", now=45), op("assess", collection="memories")))

    case("keep_reasons_in_order",
         "One prune reports stale first, then unimportant, then over_entries.",
         wf(*memories(max_age=30, min_importance=50, max_entries=1), op("prune", now=60)))

    case("keep_one_collection",
         "Pruning one collection leaves the others alone.",
         wf(*memories(max_entries=1), op("rule", collection="lessons", max_entries=1),
            add("lessons", "kiln_hot", 100, 80), add("lessons", "dogs_bite", 100, 90),
            op("prune", now=50, collection="lessons"), op("assess", collection="memories")))

    case("keep_importance_update",
         "Touch can raise importance, which changes who goes when over max_entries.",
         wf(*memories(max_entries=3), op("touch", collection="memories", id="rain", importance=95),
            op("prune", now=45)))

    case("keep_rule_replaced",
         "A new rule replaces the old one; entries stay.",
         wf(*memories(max_entries=1), op("rule", collection="memories", max_age=1000), op("prune", now=45),
            op("assess", collection="memories")))

    case("migrate_rename_add_remove",
         "Version 2 renames mood to feeling for every character and adds a default trust; version 3 removes flour.",
         wf(op("migration", version=2, changes=[{"op": "rename", "path": "*.mood", "to": "feeling"},
                                                {"op": "add", "path": "*.trust", "value": 50}]),
            op("migration", version=3, changes=[{"op": "remove", "path": "*.flour"}]),
            op("migrate", **{"from": 1, "document": BAKERY_V1})))

    case("migrate_from_middle_version",
         "A document already at version 2 only gets version 3.",
         wf(op("migration", version=2, changes=[{"op": "add", "path": "*.trust", "value": 50}]),
            op("migration", version=3, changes=[{"op": "remove", "path": "*.flour"}]),
            op("migrate", **{"from": 2, "document": BAKERY_V1})))

    case("migrate_clamp_nested_lists",
         "Clamp stock counts into 0..100 through a list wildcard: 140 becomes 100 and -5 becomes 0.",
         wf(op("migration", version=2, changes=[{"op": "clamp", "path": "*.stock.*.count", "min": 0, "max": 100}]),
            op("migrate", **{"from": 1, "document": BAKERY_V1})))

    case("migrate_latest_is_noop",
         "A document at the latest version is returned unchanged with no work.",
         wf(op("migration", version=2, changes=[{"op": "remove", "path": "*.flour"}]),
            op("migrate", **{"from": 2, "document": BAKERY_V1})))

    case("migrate_missing_paths_skipped",
         "Paths that are absent or reach non-objects are skipped, not errors.",
         wf(op("migration", version=2, changes=[{"op": "rename", "path": "nobody.mood", "to": "x"},
                                                {"op": "remove", "path": "baker.flour.grams"},
                                                {"op": "clamp", "path": "*.age", "min": 0, "max": 9}]),
            op("migrate", **{"from": 1, "document": BAKERY_V1})))

    case("limits_echo_and_entries_cap",
         "Limits are echoed; with entries 2 the collection holds two.",
         wf(op("rule", collection="m"), add("m", "a", 1), add("m", "b", 1), op("assess", collection="m"),
            limits={"entries": 2}))

    case("bad_entries_full",
         "With entries 1, a second entry is refused.",
         wf(op("rule", collection="m"), add("m", "a", 1), add("m", "b", 1), limits={"entries": 1}), error=True)

    case("bad_add_without_rule",
         "A collection exists only once it has a rule.",
         wf(add("m", "a", 1)), error=True)

    case("bad_migration_gap",
         "Migrations are numbered 2, 3, ... without gaps.",
         wf(op("migration", version=3, changes=[{"op": "remove", "path": "a"}])), error=True)

    case("bad_rename_collision",
         "Renaming onto a key that exists is refused.",
         wf(op("migration", version=2, changes=[{"op": "rename", "path": "baker.mood", "to": "flour"}]),
            op("migrate", **{"from": 1, "document": BAKERY_V1})), error=True)

    case("bad_clamp_not_integer",
         "Clamp refuses a value that is not an integer.",
         wf(op("migration", version=2, changes=[{"op": "clamp", "path": "*.stock.*.loaf", "min": 0, "max": 1}]),
            op("migrate", **{"from": 1, "document": BAKERY_V1})), error=True)

    case("bad_wildcard_last",
         "A path may not end in a wildcard.",
         wf(op("migration", version=2, changes=[{"op": "remove", "path": "baker.*"}])), error=True)

    case("bad_prune_backwards",
         "The clock is nondecreasing.",
         wf(op("rule", collection="m"), add("m", "a", 1, now=10), op("prune", now=9)), error=True)

    return cases


COLS = ["memories", "lessons", "ledger"]
IDS = ["a", "b", "c", "d", "e", "f"]


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


def rand_value(rng: random.Random, depth: int = 0) -> Any:
    r = rng.random()
    if depth < 2 and r < 0.25:
        return {k: rand_value(rng, depth + 1) for k in rng.sample(["mood", "trust", "stock", "x", "y"], rng.randint(0, 3))}
    if depth < 2 and r < 0.4:
        return [rand_value(rng, depth + 1) for _ in range(rng.randint(0, 3))]
    return rng.choice([0, 1, -7, 250, "rye", True, None, rng.randint(-1000, 1000)])


def rand_doc(rng: random.Random) -> Dict[str, Any]:
    return {b: rand_value(rng, 0) if rng.random() < 0.2 else
            {k: rand_value(rng, 1) for k in rng.sample(["mood", "trust", "stock", "x", "y"], rng.randint(0, 4))}
            for b in rng.sample(["baker", "miller", "smith"], rng.randint(0, 3))}


def rand_path(rng: random.Random) -> str:
    head = [rng.choice(["*", "*", "baker", "stock", "x"]) for _ in range(rng.randint(0, 3))]
    return ".".join(head + [rng.choice(["mood", "trust", "stock", "x", "y"])])


def rand_change(rng: random.Random) -> Dict[str, Any]:
    k = rng.choice(["rename", "add", "remove", "clamp"])
    c: Dict[str, Any] = {"op": k, "path": rand_path(rng)}
    if k == "rename":
        c["to"] = rng.choice(["mood", "trust", "feeling", "y"])
    elif k == "add":
        c["value"] = rand_value(rng, 1)
    elif k == "clamp":
        lo = rng.randint(-100, 100)
        c["min"], c["max"] = lo, lo + rng.randint(0, 300)
    return c


def rand_op(rng: random.Random, t: int, version: int) -> Dict[str, Any]:
    c, i = rng.choice(COLS), rng.choice(IDS)
    roll = rng.random()
    if roll < 0.12:
        a: Dict[str, Any] = {"collection": c}
        for k, v in [("max_entries", rng.randint(1, 5)), ("max_bytes", rng.randint(1, 1500)),
                     ("max_age", rng.randint(0, 60)), ("min_importance", rng.randint(0, 100)),
                     ("grace", rng.randint(0, 20))]:
            if rng.random() < 0.35:
                a[k] = v
        return op("rule", **a)
    if roll < 0.42:
        a = {"collection": c, "id": i, "bytes": rng.randint(1, 600)}
        if rng.random() < 0.7:
            a["importance"] = rng.randint(0, 100)
        if rng.random() < 0.15:
            a["pinned"] = rng.random() < 0.5
        if rng.random() < 0.6:
            a["now"] = t
        return op("add", **a)
    if roll < 0.55:
        a = {"collection": c, "id": i}
        if rng.random() < 0.3:
            a["importance"] = rng.randint(0, 100)
        if rng.random() < 0.2:
            a["pinned"] = rng.random() < 0.5
        if rng.random() < 0.6:
            a["now"] = t
        return op("touch", **a)
    if roll < 0.72:
        a = {"now": t}
        if rng.random() < 0.25:
            a["dry_run"] = True
        if rng.random() < 0.25:
            a["collection"] = c
        return op("prune", **a)
    if roll < 0.78:
        return op("assess", collection=c)
    if roll < 0.88:
        return op("migration", version=version + (1 if rng.random() < 0.9 else 2),
                  changes=[rand_change(rng) for _ in range(rng.randint(1, 4))])
    return op("migrate", **{"from": rng.randint(1, max(1, version)), "document": rand_doc(rng)})


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    """Random ops kept only while the reference accepts them; a quarter of workflows instead end
    on their first refused op."""
    limits = rlimits(rng)
    end_on_refusal = rng.random() < 0.25
    ops: List[Dict[str, Any]] = []
    t, version = 0, 1
    target = rng.randint(1, 24)
    for _ in range(target * 4):
        if len(ops) >= target:
            break
        t += rng.choice([0, 3, 7, 10, 40])
        ops.append(rand_op(rng, t, version))
        if not accepts(ops, limits):
            if end_on_refusal:
                break
            ops.pop()
        elif ops[-1]["op"] == "migration":
            version += 1
    if not ops:
        ops.append(op("prune", now=0))
    if rng.random() < 0.03:
        ops[rng.randrange(len(ops))]["args"]["note"] = "x"
    return wf(*ops, limits=limits)


def soak() -> int:
    """A town of 50 beings over 30 days (24 ticks a day), seeded. Each day each being adds six
    memories, three expectations, two ledger records and, every third day, a lesson; it recalls
    two older memories. Kept bytes with no rules vs with rules, pruned once a day."""
    rules = {
        "memories": {"max_age": 7 * 24, "min_importance": 20, "grace": 24, "max_entries": 50 * 40},
        "expectations": {"max_age": 3 * 24},
        "ledger": {"max_age": 14 * 24, "min_importance": 30},
        "lessons": {"max_entries": 50 * 10},
    }
    out: Dict[str, Any] = {}
    for mode in ("unbounded", "pruned"):
        rng = random.Random(7)
        r = Retention()
        r.lim.apply({"entries": 1_000_000})
        for c in rules:
            r.op_rule({"collection": c, **(rules[c] if mode == "pruned" else {})})
        n = 0
        daily = []
        for day in range(1, 31):
            now = day * 24
            for b in range(50):
                for k in range(6):
                    n += 1
                    r.op_add({"collection": "memories", "id": f"m{n}", "bytes": rng.randint(120, 400),
                              "importance": rng.randint(0, 100), "now": now})
                for k in range(3):
                    n += 1
                    r.op_add({"collection": "expectations", "id": f"e{n}", "bytes": 80,
                              "importance": rng.randint(20, 80), "now": now})
                for k in range(2):
                    n += 1
                    r.op_add({"collection": "ledger", "id": f"l{n}", "bytes": 150,
                              "importance": rng.randint(0, 100), "pinned": rng.random() < 0.05, "now": now})
                if day % 3 == b % 3:
                    n += 1
                    r.op_add({"collection": "lessons", "id": f"s{n}", "bytes": rng.randint(200, 600),
                              "importance": rng.randint(60, 100), "now": now})
                live = list(r.cols["memories"]["entries"])
                for _ in range(2):
                    r.op_touch({"collection": "memories", "id": rng.choice(live), "now": now})
            r.op_prune({"now": now})
            daily.append(sum(col["bytes"] for col in r.cols.values()))
        out[mode] = {"day_10": daily[9], "day_20": daily[19], "day_30": daily[29],
                     "entries_day_30": sum(len(col["entries"]) for col in r.cols.values()),
                     "by_collection_day_30": {c: r.cols[c]["bytes"] for c in sorted(r.cols)}}
    out["pruned_share_day_30_millionths"] = out["pruned"]["day_30"] * 1_000_000 // out["unbounded"]["day_30"]
    out["actual_model_calls"] = 0
    out["api_spend_usd"] = 0
    print(json.dumps(out))
    retained = ROOT / "eval" / "character" / "retention-soak.json"
    if retained.exists() and json.loads(retained.read_text()) != out:
        print("soak differs from eval/character/retention-soak.json", file=sys.stderr)
        return 1
    return 0


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
    ok_runs = err_runs = removed = migrated = edits = 0
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
        removed += proc.stdout.count('"reason":"')
        migrated += proc.stdout.count('"document":')
        edits += sum(r.get("edits", 0) for r in rust_res["results"] if isinstance(r, dict))
    print(json.dumps({
        "random": count, "seed": seed, "mismatches": 0, "accepted": ok_runs, "refused": err_runs,
        "entries_pruned": removed, "documents_migrated": migrated, "migration_edits": edits,
        "actual_model_calls": 0, "api_spend_usd": 0,
    }))
    return 0


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--cap", type=float, default=0.0)
    p.add_argument("--random", type=int, default=0)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--generate", action="store_true")
    p.add_argument("--soak", action="store_true")
    a = p.parse_args()
    if a.cap != 0:
        sys.exit("retention checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.soak:
        sys.exit(soak())
    elif a.random > 0:
        sys.exit(run_random_parity(a.random, a.seed))
    else:
        sys.exit(check_blessed())
