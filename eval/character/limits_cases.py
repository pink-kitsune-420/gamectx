#!/usr/bin/env python3
"""Conformance cases and differential parity runner for host-set limits on the pass 2
character modules (stage 7a): beliefs, speech_act, diet, creed, explain, stability,
expectation, attention.

  --cap 0                 check the Python references against every blessed case
  --generate              (re)write conformance/limits/*/case.json + input.json
  --random N --seed S     N randomized workflows with random limits, Python vs `gamectx local -`

All checks are offline: zero model calls, $0 API spend.
"""

import argparse
import json
import pathlib
import random
import subprocess
import sys
from typing import Any, Dict, List

sys.path[1:1] = [str(d) for d in sorted(pathlib.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import beliefs_speech_cases
import beliefs_speech_reference
import creed_cases
import creed_reference
import diet_cases
import diet_reference
import expect_attend_cases
import expect_attend_reference
import explain_stability_cases
import explain_stability_reference

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CONFORMANCE_DIR = ROOT / "conformance" / "limits"

REFS = {"beliefs": beliefs_speech_reference, "speech_act": beliefs_speech_reference, "diet": diet_reference,
        "creed": creed_reference, "explain": explain_stability_reference, "stability": explain_stability_reference,
        "expectation": expect_attend_reference, "attention": expect_attend_reference}
NAMES = {"beliefs": ["beings", "beliefs"], "speech_act": ["pairs", "want_beings", "wants", "beings", "beliefs"],
         "diet": ["foods", "beings", "recent"], "creed": ["creeds", "beings"], "explain": ["beings", "traces"],
         "stability": ["tracked_beings", "audit_beings", "warnings"],
         "expectation": ["beings", "expectations", "subjects"],
         "attention": ["profiles", "notice_beings", "notice_events"]}


def py_workflow(request: Dict[str, Any]) -> Dict[str, Any]:
    kind = request.get("kind")
    return REFS[kind].process_local(kind, request.get("args", {}))


def from_case(suite: str, case: str, limits: Dict[str, Any]) -> Dict[str, Any]:
    req = json.loads((ROOT / "conformance" / suite / case / "input.json").read_text())
    req["args"] = {"limits": limits, **req["args"]}
    return req


def op(name: str, **args: Any) -> Dict[str, Any]:
    return {"op": name, "args": args}


def make_cases() -> List[Dict[str, Any]]:
    cases: List[Dict[str, Any]] = []

    def add(cid: str, description: str, request: Dict[str, Any], error: bool = False) -> None:
        cases.append({"id": cid, "description": description, "input": request, "error": error})

    add("lim_beliefs_echo_defaults", "Supplying an empty limits object echoes every default and changes nothing else.",
        from_case("beliefs_speech", "beliefs_reinforce_stronger_source", {}))
    add("lim_beliefs_small_store", "With room for one belief per being, each new belief evicts the weakest.",
        from_case("beliefs_speech", "beliefs_init_authored_and_query", {"beliefs": 1}))
    add("lim_beliefs_one_being", "With room for one being, a second being is refused.",
        {"kind": "beliefs", "args": {"limits": {"beings": 1}, "ops": [
            op("learn", being=b, source="told", about="mill", relation="sells", value="flour") for b in ("baker", "miller")]}},
        error=True)
    add("lim_speech_one_want", "With room for one want per being, wants compete by priority.",
        from_case("beliefs_speech", "speech_request_and_persuade", {"wants": 1}))
    add("lim_speech_store_limits_reach_beliefs", "Speech acts pass beings/beliefs limits to their belief store.",
        from_case("beliefs_speech", "speech_inform_moves_belief", {"beliefs": 1, "pairs": 4}))
    add("lim_diet_recent_two", "Keeping two recent meals shortens monotony memory.",
        from_case("diet", "diet_monotony_penalty", {"recent": 2}))
    add("lim_diet_foods_below_pack", "Room for ten foods refuses the twelve-food village pack.",
        from_case("diet", "diet_register_village_pack", {"foods": 10}), error=True)
    add("lim_diet_foods_above_old_cap", "Raising the food limit registers 300 foods (the old fixed cap was 256).",
        {"kind": "diet", "args": {"limits": {"foods": 300}, "ops": [
            op("register_food", foods=[{"id": f"f{c * 64 + i}", "tags": ["grain"], "hunger": 5} for i in range(64)
                                       if c * 64 + i < 300]) for c in range(5)]}})
    add("lim_creed_above_old_cap", "Raising the creed limit registers 33 creeds (the old fixed cap was 32).",
        {"kind": "creed", "args": {"limits": {"creeds": 33}, "ops": [
            op("register_creed", creed={"id": f"c{i}", "label": "A creed", "kind": "tradition"}) for i in range(33)]}})
    add("lim_creed_exact_fit", "A limit equal to current need is enough: two beings judge and witness.",
        from_case("creed", "creed_judge_witnessed_shame_and_indignation", {"beings": 2}))
    add("lim_explain_two_traces", "Keeping two traces per being evicts the oldest.",
        from_case("explain_stability", "explain_query_newest_first", {"traces": 2}))
    add("lim_stability_audit_above_old_cap", "Raising audit_beings audits 65 beings in one turn (the old cap was 64).",
        {"kind": "stability", "args": {"limits": {"audit_beings": 65}, "ops": [
            op("audit_turn", turn=1, beings=[{"being": f"b{i}", "motive": "work", "activity": "working"} for i in range(65)])]}})
    add("lim_explain_one_trace", "Keeping one trace per being still answers why for the newest action.",
        from_case("explain_stability", "explain_record_full_chain", {"traces": 1}))
    add("lim_stability_two_warnings", "Keeping two warnings drops the oldest.",
        from_case("explain_stability", "stability_get_warnings_filtered", {"warnings": 2}))
    add("lim_expectation_two_subjects", "Remembering two subjects' reliability forgets the least observed.",
        {"kind": "expectation", "args": {"limits": {"subjects": 2}, "ops": [
            *[op("expect", being="clerk", id=f"x{i}", about=f"s{i}", event="e", due=0, importance=10) for i in range(3)],
            op("happen", about="s0", event="e", at=0), op("tick", at=1), op("assess", being="clerk")]}})
    add("lim_expectation_two_evicts_broken", "With room for two expectations, a third evicts the broken one due first.",
        {"kind": "expectation", "args": {"limits": {"expectations": 2}, "ops": [
            op("expect", being="clerk", id="a", about="s", event="e", due=0, importance=10),
            op("expect", being="clerk", id="b", about="s", event="e", due=3, importance=10),
            op("tick", at=4), op("expect", being="clerk", id="c", about="s", event="e", due=9, importance=10),
            op("assess", being="clerk")]}})
    add("lim_expectation_one_open", "With room for one expectation and none broken, a second is refused.",
        {"kind": "expectation", "args": {"limits": {"expectations": 1}, "ops": [
            op("expect", being="clerk", id="a", about="s", event="e", due=5, importance=10),
            op("expect", being="clerk", id="b", about="s", event="e", due=5, importance=10)]}}, error=True)
    add("lim_attention_crowd_above_old_cap", "Raising notice_beings lets 100 beings notice one event in one call.",
        {"kind": "attention", "args": {"limits": {"notice_beings": 100}, "ops": [
            op("notice", seed=4, events=[{"id": "bell", "kind": "alarm", "place": "square", "salience": 90,
                                          "noise": 90}],
               beings=[{"id": f"b{i}", "place": "square"} for i in range(100)])]}})
    add("lim_attention_twenty_events", "Raising notice_events notices among 20 events (the old cap was 16), capacity still applies.",
        {"kind": "attention", "args": {"limits": {"notice_events": 20}, "ops": [
            op("notice", seed=8, events=[{"id": f"e{i:02d}", "kind": "stall", "place": "market", "salience": 40 + 3 * i}
                                         for i in range(20)],
               beings=[{"id": "clerk", "place": "market"}])]}})
    add("lim_diet_one_being_two_recent", "One diner with a two-meal memory eats; the limits fit the scene.",
        from_case("diet", "diet_eat_effects_body_affect", {"beings": 1, "recent": 2}))
    add("lim_stability_tracked_raised", "Raising tracked_beings and warnings leaves a normal audit unchanged apart from the echo.",
        from_case("explain_stability", "stability_herding", {"tracked_beings": 5000, "warnings": 4096}))
    add("lim_attention_one_profile", "With room for one profile, a second is refused.",
        from_case("expect_attend", "att_cares_boosts", {"profiles": 1}), error=True)
    add("lim_attention_over_ceiling", "A limit above its ceiling is refused.",
        from_case("expect_attend", "att_profiles_and_defaults", {"notice_events": 257}), error=True)
    add("lim_unknown_name_refused", "Unknown limit names are refused.",
        from_case("creed", "creed_regard_stances", {"pews": 3}), error=True)
    add("lim_null_limits_is_none", "A null limits object is ignored but still echoed as the defaults.",
        from_case("expect_attend", "exp_kept_on_time_raises_confidence", None))
    return cases


def rand_workflow(rng: random.Random) -> Dict[str, Any]:
    gen = rng.choice([beliefs_speech_cases.rand_beliefs, beliefs_speech_cases.rand_speech, diet_cases.rand_diet,
                      creed_cases.rand_creed, explain_stability_cases.rand_explain,
                      explain_stability_cases.rand_stability, expect_attend_cases.rand_expect,
                      expect_attend_cases.rand_attend])
    req = gen(rng)
    if rng.random() < 0.8:
        names = NAMES[req["kind"]]
        lim = {n: rng.randint(1, 8) if rng.random() < 0.7 else rng.randint(9, 300)
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
