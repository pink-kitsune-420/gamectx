#!/usr/bin/env python3
"""Conformance cases and differential parity runner for pass 5 stage 1: durable state (`export` and
`import` on every stateful pass 2-4 manager, and the `state-bundle` kind).

  --cap 0                 check the Python reference against every blessed case
  --generate              (re)write conformance/durable/*/case.json + input.json
  --random N --seed S     N randomized save-anywhere workflows, Python reference vs `gamectx local -`
  --kinds a,b             limit --random to some kinds

Each random workflow comes from the kind's own random generator. It is cut at a random op: the
prefix ends with `export`, a second request starts with `import` of that save and runs the rest.
Expected, before any run: both implementations write the same save; the continued results equal
the uncut run's results byte for byte (a refusal stays a refusal); and a randomly damaged save is
refused by both or accepted by both with equal continued results.

All checks are offline: zero model calls, $0 API spend.
"""

import argparse
import copy
import importlib
import json
import os
import pathlib
import random
import subprocess
import sys
from typing import Any, Callable, Dict, List, Optional, Tuple

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import durable_kinds  # noqa: F401  (registers every kind)
from durable_reference import KINDS, MAX_INT, Bundle, compact
from durable_reference import workflow as durable_workflow

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "durable"

# kind -> (cases module, generator name)
GENERATORS: Dict[str, Tuple[str, str]] = {
    "chemistry": ("chemistry_cases", "rand_workflow"),
    "minds": ("minds_cases", "rand_workflow"),
    "stability": ("explain_stability_cases", "rand_stability"),
    "explain": ("explain_stability_cases", "rand_explain"),
    "surroundings": ("surroundings_cases", "rand_workflow"),
    "fabric": ("fabric_cases", "rand_workflow"),
    "cueing": ("cueing_cases", "rand_workflow"),
    "pragmatics": ("pragmatics_cases", "rand_workflow"),
    "turn-frame": ("frame_cases", "rand_workflow"),
    "embodiment": ("embodiment_cases", "rand_workflow"),
    "genre": ("genre_cases", "rand_workflow"),
    "culture": ("culture_cases", "rand_workflow"),
    "projects": ("projects_cases", "rand_workflow"),
    "defenses": ("defenses_cases", "rand_workflow"),
    "beliefs": ("beliefs_speech_cases", "rand_beliefs"),
    "speech_act": ("beliefs_speech_cases", "rand_speech"),
    "diet": ("diet_cases", "rand_diet"),
    "creed": ("creed_cases", "rand_creed"),
    "expectation": ("expect_attend_cases", "rand_expect"),
    "attention": ("expect_attend_cases", "rand_attend"),
    "aspiration": ("growth_cases", "rand_aspire"),
    "habit": ("growth_cases", "rand_habit"),
    "lesson": ("growth_cases", "rand_lesson"),
    "cooperation": ("cooperation_cases", "rand_workflow"),
    "cognition": ("cognition_cases", "rand_workflow"),
    "chatter": ("chatter_cases", "rand_workflow"),
    "survival": ("survival_cases", "rand_workflow"),
    "detail": ("detail_cases", "rand_workflow"),
    "retention": ("retention_cases", "rand_workflow"),
    "roles": ("roles_cases", "rand_workflow"),
    "economy": ("economy_cases", "rand_workflow"),
    "space": ("space_cases", "rand_workflow"),
    "drift": ("drift_cases", "rand_workflow"),
    "packs": ("packs_cases", "rand_workflow"),
}


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    kind = request.get("kind")
    if kind == "state-bundle":
        return Bundle().workflow(request.get("args", {}))
    if kind not in KINDS:
        raise ValueError("unknown local kind")
    return durable_workflow(kind, request.get("args", {}))


def generator(kind: str) -> Callable[[random.Random], Dict[str, Any]]:
    if kind == "state-bundle":
        return rand_bundle
    mod, name = GENERATORS[kind]
    return getattr(importlib.import_module(mod), name)


def rand_bundle(rng: random.Random) -> Dict[str, Any]:
    """A bundle workflow: two or three kinds' own random requests, cut into chunks and run in
    turn inside one bundle. A kind's request limits ride on every chunk, as a host that sets
    limits passes them with each request (limits never come from a save)."""
    kinds = rng.sample(sorted(GENERATORS), rng.randint(2, 3))
    queues = []
    for k in kinds:
        req = generator(k)(rng)["args"]
        ops = req["ops"]
        chunks = []
        while ops:
            n = rng.randint(1, 8)
            chunks.append(ops[:n])
            ops = ops[n:]
        rows = []
        for ch in chunks:
            inner: Dict[str, Any] = {"ops": ch}
            if "limits" in req:
                inner["limits"] = req["limits"]
            rows.append({"op": "run", "args": {"kind": k, "request": inner}})
        queues.append(rows)
    ops: List[Dict[str, Any]] = []
    while any(queues) and len(ops) < 60:
        q = rng.choice([q for q in queues if q])
        ops.append(q.pop(0))
    if rng.random() < 0.3:
        ops.insert(rng.randrange(len(ops) + 1), {"op": "inspect", "args": {"save": {"format": "gamectx-bundle",
                                                                                    "sections": [], "version": 1}}})
    req: Dict[str, Any] = {"kind": "state-bundle", "args": {"ops": ops}}
    if rng.random() < 0.2:
        req["args"]["limits"] = {"sections": rng.choice([1, 2, 3, 64])}
    return req


class Rust:
    def __init__(self) -> None:
        self.bin = pathlib.Path(os.environ.get("GAMECTX_BIN", ROOT / "target" / "debug" / "gamectx"))
        if not self.bin.exists():
            subprocess.run(["cargo", "build"], cwd=ROOT, check=True)

    def run(self, request: Dict[str, Any]) -> Tuple[Optional[Dict[str, Any]], Optional[str]]:
        proc = subprocess.run([str(self.bin), "local", "-"], input=json.dumps(request), capture_output=True,
                              text=True)
        if proc.returncode != 0:
            return None, proc.stderr.strip() or "refused"
        return json.loads(proc.stdout), None


def py_run(request: Dict[str, Any]) -> Tuple[Optional[Dict[str, Any]], Optional[str]]:
    try:
        return py_workflow(request), None
    except Exception as e:  # noqa: BLE001 - any refusal counts
        return None, f"{type(e).__name__}: {e}"


def request(kind: str, ops: List[Dict[str, Any]], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    args: Dict[str, Any] = {"ops": ops}
    if limits is not None:
        args["limits"] = limits
    return {"kind": kind, "args": args}


# ------------------------------------------------------------------ conformance cases

# kind -> an existing blessed, accepted case whose ops build a rich state (cut in half below).
SOURCES: Dict[str, str] = {
    "aspiration": "growth/asp_model_proposals_validated", "attention": "expect_attend/att_profiles_and_defaults",
    "beliefs": "limits/lim_beliefs_small_store", "chatter": "chatter/chat_log_newest_first_and_bounded",
    "chemistry": "chemistry/joke_retired_after_five", "cognition": "cognition/mem_recall_rehearses",
    "cooperation": "cooperation/help_favor_owed_tips_refusal", "creed": "creed/creed_regard_stances",
    "cueing": "cueing/surface_forgets_oldest_listener", "culture": "culture/chain_keeps_last_eight",
    "defenses": "defenses/decay_slower_while_denying", "detail": "detail/lod_same_state_every_level",
    "diet": "diet/diet_assess_recent_meals_bounded", "drift": "drift/drift_reasons_newest_eight",
    "economy": "economy/econ_price_recovers_over_periods", "embodiment": "embodiment/react_repeat_within_window",
    "expectation": "expect_attend/exp_reliability_floor", "explain": "explain_stability/explain_init_clear_being",
    "fabric": "fabric/landmarks_replace_and_evict", "genre": "genre/define_axes_cycle_and_replace",
    "habit": "growth/hab_skip_with_reason_and_history", "lesson": "growth/les_advice_filters_and_forget",
    "minds": "minds/hear_three_minds", "packs": "packs/packs_redefine_does_not_count",
    "pragmatics": "pragmatics/threads_interrupt_and_resume", "projects": "projects/sunk_cost_keeps_going",
    "retention": "retention/keep_one_collection", "roles": "roles/roles_vacate_stops_duties",
    "space": "space/space_well_queue", "speech_act": "beliefs_speech/speech_plan_share_alarm_reassure",
    "stability": "explain_stability/stability_health_unstable_then_recovered",
    "surroundings": "surroundings/oven_queue_promotes", "survival": "survival/life_heir_full_passes_on",
    "turn-frame": "turn-frame/frame_trend_window",
}


def source(kind: str) -> Dict[str, Any]:
    inp = json.loads((ROOT / "conformance" / SOURCES[kind] / "input.json").read_text())
    assert inp["kind"] == kind
    return inp["args"]


def save_after(kind: str, ops: List[Dict[str, Any]], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    """The Python reference's save after `ops` (with `limits` in force)."""
    out = py_workflow(request(kind, copy.deepcopy(ops) + [{"op": "export"}], limits))
    return out["results"][-1]


def halves(kind: str) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]], Optional[Dict[str, int]]]:
    args = source(kind)
    ops = args["ops"]
    k = len(ops) // 2
    return ops[:k], ops[k:], args.get("limits")


def imp(save: Any) -> Dict[str, Any]:
    return {"op": "import", "args": {"save": save}}


def bundle(*ops: Dict[str, Any], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    return request("state-bundle", list(ops), limits)


def brun(kind: str, ops: List[Dict[str, Any]], limits: Optional[Dict[str, int]] = None) -> Dict[str, Any]:
    inner: Dict[str, Any] = {"ops": copy.deepcopy(ops)}
    if limits is not None:
        inner["limits"] = limits
    return {"op": "run", "args": {"kind": kind, "request": inner}}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def case(cid: str, description: str, req: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": req, "error": error})

    for kind in sorted(SOURCES):
        first, rest, limits = halves(kind)
        save = save_after(kind, first, limits)
        case(f"resume_{kind.replace('-', '_')}",
             f"Save anywhere ({kind}): load the save taken halfway through {SOURCES[kind]}, run the rest "
             f"(results match the uncut case) and save again.",
             request(kind, [imp(save)] + copy.deepcopy(rest) + [{"op": "export"}], limits))

    case("export_fresh_manager", "A manager with nothing in it saves its empty state and current limits.",
         request("space", [{"op": "export"}], {"rooms": 2}))
    small = save_after("beliefs", source("beliefs")["ops"], {"beings": 3, "beliefs": 5})
    case("import_keeps_current_limits",
         "An import reports the limits written in the save but keeps the limits in force.",
         request("beliefs", [imp(small), {"op": "export"}], {"beings": 9}))

    # Malformed saves: each is refused whole.
    b_save = save_after("beliefs", source("beliefs")["ops"])
    chem = save_after("chemistry", halves("chemistry")[0])
    case("bad_kind_mismatch", "Malformed: a chemistry save cannot load into beliefs.",
         request("beliefs", [imp(chem)]), True)
    future = copy.deepcopy(b_save)
    future["version"] = 2
    case("bad_future_version", "Malformed: a save newer than this build's format.",
         request("beliefs", [imp(future)]), True)
    moved = {k: b_save[k] for k in ["kind", "format", "limits", "state", "version"]}
    case("bad_envelope_order", "Malformed: envelope fields out of canonical order.",
         request("beliefs", [imp(moved)]), True)
    extra = copy.deepcopy(b_save)
    extra["state"]["notes"] = []
    case("bad_extra_field", "Malformed: a field the state does not have.", request("beliefs", [imp(extra)]), True)
    surv = save_after("survival", source("survival")["ops"])
    flipped = copy.deepcopy(surv)
    flipped["state"]["beings"].reverse()
    case("bad_unsorted_map", "Malformed: map rows out of ascending key order.",
         request("survival", [imp(flipped)]), True)
    orphan = copy.deepcopy(surv)
    rows = orphan["state"]["beings"]
    rows[0][1]["parents"] = rows[0][1]["parents"] + ["ghost"]
    case("bad_family_link", "Malformed: a parent that does not list the child (links must agree both ways).",
         request("survival", [imp(orphan)]), True)
    les = save_after("lesson", source("lesson")["ops"])
    wrong = copy.deepcopy(les)
    for _, topics in wrong["state"]["beings"]:
        for _, t in topics:
            t["verdict"] = "seek" if t["verdict"] != "seek" else "avoid"
    case("bad_lesson_verdict", "Malformed: a verdict the episodes do not support.",
         request("lesson", [imp(wrong)]), True)
    att = save_after("attention", [{"op": "profile", "args": {"being": "ana"}}])
    many = copy.deepcopy(att)
    many["state"]["profiles"][0][1]["cares"] = [f"c{i:02d}" for i in range(17)]
    case("bad_over_bound", "Malformed: more cares than any profile can hold (16).",
         request("attention", [imp(many)]), True)
    neg = copy.deepcopy(b_save)
    neg["state"]["beings"][0][1][0]["confidence"] = -1
    case("bad_out_of_range", "Malformed: a confidence below 0.", request("beliefs", [imp(neg)]), True)
    stab = save_after("stability", source("stability")["ops"])
    long = copy.deepcopy(stab)
    need_warning = long["state"]["warnings"]
    assert need_warning, "the stability source must log a warning"
    need_warning[0]["detail"] = "x" * 4097
    case("bad_long_text_in_warning", "Malformed: a warning detail longer than 4,096 bytes (the text limit holds "
         "inside every field, including kept warnings).", request("stability", [imp(long)]), True)
    case("bad_export_args", "Malformed: export takes no arguments.",
         request("beliefs", [{"op": "export", "args": {"compact": True}}]), True)
    case("bad_import_missing_save", "Malformed: import needs a save.", request("beliefs", [{"op": "import"}]), True)

    # The bundle.
    d_first = halves("drift")[0]
    s_first, s_rest, s_lim = halves("space")
    b_first = source("beliefs")["ops"]
    case("bundle_run_and_export", "A bundle runs three managers in place and saves them in one document.",
         bundle(brun("space", s_first, s_lim), brun("drift", d_first), brun("beliefs", b_first), {"op": "export"}))
    one = Bundle()
    one.workflow({"ops": [brun("space", s_first, s_lim), brun("drift", d_first)]})
    saved = one.export()
    case("bundle_import_resume", "A bundle save loads whole and the managers carry on where they were.",
         bundle(imp(saved), brun("space", s_rest, s_lim), {"op": "export"}))
    case("bundle_inspect_changes_nothing", "Inspect checks a bundle save and reports it without loading it.",
         bundle(brun("beliefs", b_first), {"op": "inspect", "args": {"save": saved}}, {"op": "export"}))
    case("bundle_import_replaces_all", "An import replaces the whole bundle: managers absent from the save go.",
         bundle(brun("beliefs", b_first), imp(saved), {"op": "export"}))
    rev = copy.deepcopy(saved)
    rev["sections"].reverse()
    case("bad_bundle_unsorted", "Malformed: bundle sections out of kind order.", bundle(imp(rev)), True)
    unknown = copy.deepcopy(saved)
    unknown["sections"].append(["weather", unknown["sections"][0][1]])
    case("bad_bundle_unknown_kind", "Malformed: a section for a kind with no durable state.",
         bundle(imp(unknown)), True)
    half_bad = copy.deepcopy(saved)
    half_bad["sections"][1][1]["state"]["clock"] = -5
    case("bad_bundle_one_bad_section", "Malformed: one bad section refuses the whole bundle (nothing loads).",
         bundle(brun("beliefs", b_first), imp(half_bad)), True)
    case("bad_bundle_over_sections", "Malformed: more sections than the bundle's sections limit.",
         bundle(imp(saved), limits={"sections": 1}), True)
    return cases


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
    """Python reference vs every blessed expected.json (and error-ness) on disk; every resume case
    must also reproduce its source case's tail exactly."""
    cases = make_cases()
    passed = 0
    problems = []
    for c in cases:
        d = CONFORMANCE_DIR / c["id"]
        on_disk = json.loads((d / "input.json").read_text())
        if on_disk != c["input"]:
            problems.append(f"{c['id']}: input.json is stale (run --generate)")
            continue
        out, err = py_run(c["input"])
        expected_path = d / "expected.json"
        if c["error"]:
            if err is None:
                problems.append(f"{c['id']}: reference accepted a malformed case")
            elif expected_path.exists():
                problems.append(f"{c['id']}: error case has expected.json")
            else:
                passed += 1
            continue
        if err is not None:
            problems.append(f"{c['id']}: reference refused: {err}")
            continue
        if c["id"].startswith("resume_"):
            kind = c["input"]["kind"]
            src = json.loads((ROOT / "conformance" / SOURCES[kind] / "expected.json").read_text())
            k = len(halves(kind)[0])
            if out["results"][1:-1] != src["results"][k:]:
                problems.append(f"{c['id']}: continued results differ from {SOURCES[kind]}")
                continue
        if not expected_path.exists():
            problems.append(f"{c['id']}: not blessed yet")
            continue
        blessed = json.loads(expected_path.read_text())
        saves = [(a, b) for a, b in zip(out["results"], blessed["results"])
                 if isinstance(a, dict) and a.get("format") in ("gamectx-state", "gamectx-bundle")]
        if blessed != out:
            problems.append(f"{c['id']}: reference differs from blessed output")
        elif any(compact(a) != compact(b) for a, b in saves):
            problems.append(f"{c['id']}: a save differs from the blessed save byte for byte")
        else:
            passed += 1
    for p in problems:
        print(p, file=sys.stderr)
    print(json.dumps({"cases": len(cases), "malformed": sum(1 for c in cases if c["error"]), "passed": passed,
                      "actual_model_calls": 0, "api_spend_usd": 0}))
    return 0 if passed == len(cases) else 1


# ------------------------------------------------------------------ damaging saves

def paths(v: Any, here: Tuple = ()) -> List[Tuple]:
    out = [here]
    if isinstance(v, dict):
        for k, x in v.items():
            out += paths(x, here + (k,))
    elif isinstance(v, list):
        for i, x in enumerate(v):
            out += paths(x, here + (i,))
    return out


def at(v: Any, path: Tuple) -> Any:
    for p in path:
        v = v[p]
    return v


def put(v: Any, path: Tuple, x: Any) -> Any:
    if not path:
        return x
    at(v, path[:-1])[path[-1]] = x
    return v


def damage(save: Dict[str, Any], rng: random.Random) -> Dict[str, Any]:
    s = copy.deepcopy(save)
    ps = [p for p in paths(s) if p]
    path = rng.choice(ps)
    v = at(s, path)
    roll = rng.random()
    if isinstance(v, bool):
        return put(s, path, (not v) if roll < 0.7 else 1)
    if isinstance(v, int):
        choices = [0, 1, v + 1, v - 1, v * 2 + 1, 100, 101, 1000, 1001, 1_000_000, 1_000_001, MAX_INT,
                   MAX_INT + 1, -1, -v - 1, None, "1"]
        return put(s, path, rng.choice(choices))
    if isinstance(v, str):
        choices = ["", "x", "Bad Id", v + "x", v[:-1], "x" * 81, "dry", "callback", "none", 7, None,
                   "a\u0001b"]
        return put(s, path, rng.choice(choices))
    if v is None:
        return put(s, path, rng.choice([0, "x", [], {}, True]))
    if isinstance(v, list):
        if not v or roll < 0.25:
            return put(s, path, v + [copy.deepcopy(v[-1])] if v else [0])
        if roll < 0.45:
            return put(s, path, list(reversed(v)))
        if roll < 0.7:
            del v[rng.randrange(len(v))]
            return s
        return put(s, path, v[:-1] + [None])
    if isinstance(v, dict):
        ks = list(v)
        if roll < 0.35 and ks:
            del v[rng.choice(ks)]
            return s
        if roll < 0.6:
            v["extra"] = 0
            return s
        if len(ks) >= 2:
            items = list(v.items())
            rng.shuffle(items)
            v.clear()
            v.update(items)
            return s
        return put(s, path, [])
    return s


# ------------------------------------------------------------------ random parity

def run_random_parity(count: int, seed: int, kinds: List[str]) -> int:
    rng = random.Random(seed)
    rust = Rust()
    stats = {"random": count, "seed": seed, "mismatches": 0, "cut": 0, "refused_before_cut": 0,
             "continued_refusals": 0, "damaged": 0, "damaged_refused": 0, "damaged_accepted": 0}
    per_kind: Dict[str, int] = {}
    for i in range(1, count + 1):
        kind = kinds[(i - 1) % len(kinds)]
        per_kind[kind] = per_kind.get(kind, 0) + 1
        req = generator(kind)(rng)
        if req.get("kind") != kind:
            continue
        ops, limits = req["args"]["ops"], req["args"].get("limits")
        n = len(ops)
        k = rng.randint(max(0, n - 63), min(n, 63))
        full, full_err = py_run(req)
        rs_full, rs_full_err = rust.run(req)
        if (full_err is None) != (rs_full_err is None) or full != rs_full:
            print(f"Iteration {i} ({kind}): uncut runs differ\nPy: {full_err or json.dumps(full)}\n"
                  f"Rust: {rs_full_err or json.dumps(rs_full)}\n{json.dumps(req)}", file=sys.stderr)
            return 1
        first = request(kind, copy.deepcopy(ops[:k]) + [{"op": "export"}], limits)
        py1, py1_err = py_run(first)
        rs1, rs1_err = rust.run(first)
        if (py1_err is None) != (rs1_err is None) or py1 != rs1:
            print(f"Iteration {i} ({kind}): export differs\nPy: {py1_err or json.dumps(py1)}\n"
                  f"Rust: {rs1_err or json.dumps(rs1)}\n{json.dumps(first)}", file=sys.stderr)
            return 1
        if rs1_err is not None:
            stats["refused_before_cut"] += 1
            continue
        stats["cut"] += 1
        save = rs1["results"][-1]
        second = request(kind, [{"op": "import", "args": {"save": save}}] + copy.deepcopy(ops[k:]), limits)
        py2, py2_err = py_run(second)
        rs2, rs2_err = rust.run(second)
        if (py2_err is None) != (rs2_err is None) or py2 != rs2:
            print(f"Iteration {i} ({kind}): continued runs differ\nPy: {py2_err or json.dumps(py2)}\n"
                  f"Rust: {rs2_err or json.dumps(rs2)}\n{json.dumps(second)}", file=sys.stderr)
            return 1
        if full_err is not None:
            if rs2_err is None:
                print(f"Iteration {i} ({kind}): uncut run refused but the continued run was accepted\n"
                      f"{json.dumps(req)}\ncut at {k}", file=sys.stderr)
                return 1
            stats["continued_refusals"] += 1
        else:
            if rs2_err is not None or rs2["results"][1:] != full["results"][k:]:
                print(f"Iteration {i} ({kind}): continued results differ from the uncut run\n"
                      f"cut at {k}\n{rs2_err or json.dumps(rs2)}\n{json.dumps(req)}", file=sys.stderr)
                return 1
        for _ in range(2):
            bad = damage(save, rng)
            third = request(kind, [{"op": "import", "args": {"save": bad}}] + copy.deepcopy(ops[k:]), limits)
            py3, py3_err = py_run(third)
            rs3, rs3_err = rust.run(third)
            stats["damaged"] += 1
            if (py3_err is None) != (rs3_err is None) or py3 != rs3:
                print(f"Iteration {i} ({kind}): damaged save handled differently\n"
                      f"Py: {py3_err or json.dumps(py3)}\nRust: {rs3_err or json.dumps(rs3)}\n"
                      f"{json.dumps(third)}", file=sys.stderr)
                return 1
            stats["damaged_refused" if rs3_err else "damaged_accepted"] += 1
    stats["per_kind"] = per_kind
    stats["actual_model_calls"] = 0
    stats["api_spend_usd"] = 0
    print(json.dumps(stats))
    return 0


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--cap", type=float, default=0.0)
    p.add_argument("--random", type=int, default=0)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--kinds", default="")
    p.add_argument("--generate", action="store_true")
    a = p.parse_args()
    if a.cap != 0:
        sys.exit("durable checks are offline only; use --cap 0")
    if a.generate:
        generate_conformance_cases()
    elif a.random > 0:
        ks = [k for k in a.kinds.split(",") if k] or sorted(GENERATORS) + ["state-bundle"]
        sys.exit(run_random_parity(a.random, a.seed, ks))
    else:
        sys.exit(check_blessed())
