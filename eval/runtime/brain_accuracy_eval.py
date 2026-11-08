#!/usr/bin/env python3
"""Offline prototype: select learning rates by validation decisions, then MSE.

Original brain_eval measurements remain historical. Tune on seed 23 only, freeze rates
before inspecting fresh test seeds 2001/2002/2003. No API or credential access.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import copy
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import brain_eval as b

MAX_RATES = 8
MAX_SELECTION_STEPS = 2_000_000


def decisions(brain, samples):
    count = 0
    for features, targets in samples:
        values, _ = brain.forward(features)
        scores = [values[len(brain.inputs) + i] for i in brain.output_indices]
        best = max(scores)
        chosen = scores.index(best) if best > 0 and scores.count(best) == 1 else None
        desired = max(targets)
        count += (chosen is None) if desired == 0 else (chosen is not None and targets[chosen] == desired)
    return count


def train_select(brain, request):
    b.fields(request, ("id", "expected_revision", "examples", "validation", "epochs", "rates", "max_steps"))
    brain.request(request)
    examples, validation = brain.samples(request["examples"]), brain.samples(request["validation"])
    if any(x == v for x, _ in examples for v, _ in validation):
        raise b.BrainError("training and validation inputs overlap")
    epochs = b.integer(request["epochs"], 1, b.MAX["epochs"])
    rates = request["rates"]
    if not isinstance(rates, list) or not 1 <= len(rates) <= MAX_RATES:
        raise b.BrainError("rate count outside limits")
    rates = [b.ticks(rate, 0, 1) for rate in rates]
    if 0 in rates or len(set(rates)) != len(rates):
        raise b.BrainError("zero or duplicate normalized rate")
    cap = b.integer(request["max_steps"], 1, MAX_SELECTION_STEPS)
    parameters = sum(len(row) for row in brain.weights)
    per_trial = (3 * epochs * len(examples) + 2 * len(validation)) * parameters
    steps = (3 * epochs * len(examples) * len(rates) + 2 * len(validation) * (len(rates) + 1)) * parameters
    if per_trial > b.MAX["steps"] or steps > cap:
        raise b.BrainError("training selection work limit")
    before_loss, before_correct = brain.loss(validation), decisions(brain, validation)
    selected = None
    best_key = None
    trials = []
    for rate in rates:
        candidate = brain.trained(examples, epochs, rate)
        loss, correct = candidate.loss(validation), decisions(candidate, validation)
        trials.append({"rate": b.number(rate), "loss": b.number(loss), "correct": correct})
        if loss >= before_loss or correct < before_correct:
            continue
        key = (correct, -loss)
        if best_key is None or key > best_key:
            selected, best_key = (rate, candidate), key
    after_loss, after_correct, selected_rate = before_loss, before_correct, None
    if selected:
        rate, candidate = selected
        # Only one update is committed on the active brain, with its original undo snapshot.
        brain.retain(candidate, request["id"])
        after_loss, after_correct, selected_rate = -best_key[1], best_key[0], b.number(rate)
    return {"accepted": selected is not None, "revision": brain.revision,
            "before_loss": b.number(before_loss), "after_loss": b.number(after_loss),
            "before_correct": before_correct, "after_correct": after_correct,
            "validation_count": len(validation), "selected_rate": selected_rate,
            "steps": steps, "trials": trials}


def process(request):
    if not isinstance(request, dict):
        raise b.BrainError("workflow must be an object")
    source = [key for key in ("definition", "dense", "state") if key in request]
    if len(source) != 1:
        raise b.BrainError("one brain source required")
    b.fields(request, (source[0], "operations"))
    initial = b.process({source[0]: request[source[0]], "operations": []})
    brain = b.Brain.restore(initial["state"])
    if not isinstance(request["operations"], list) or len(request["operations"]) > 64:
        raise b.BrainError("operation count outside limits")
    results = []
    for operation in request["operations"]:
        b.fields(operation, ("op", "args"))
        if operation["op"] == "train_select":
            results.append(train_select(brain, operation["args"]))
        else:
            old = b.process({"state": brain.save(), "operations": [operation]})
            results += old["results"]
            brain = b.Brain.restore(old["state"])
    return {"results": results, "state": brain.save()}


def measure():
    results = []
    for task in ("stealth", "support", "switch", "trade"):
        examples, validation = b.dataset(task, 11, 32), b.dataset(task, 23, 128)
        baseline = b.dense(list(examples[0]["inputs"]), list(examples[0]["targets"]), 4, 3)
        selected = copy.deepcopy(baseline)
        request = {"id": "fit", "expected_revision": 0, "examples": examples,
                   "validation": validation, "epochs": 128}
        baseline_report = baseline.train(dict(request, rate=0.1))
        selection_report = train_select(selected, dict(request, rates=[0.03, 0.1, 0.3], max_steps=MAX_SELECTION_STEPS))
        test = sum((b.dataset(task, seed, 128) for seed in (2001, 2002, 2003)), [])
        samples = baseline.samples(test) if len(test) <= b.MAX["examples"] else [
            sample for seed in (2001, 2002, 2003) for sample in baseline.samples(b.dataset(task, seed, 128))]
        row = {"task": task, "test_count": len(test), "baseline_correct": decisions(baseline, samples),
               "selected_correct": decisions(selected, samples), "baseline_report": baseline_report,
               "selection_report": selection_report}
        results.append(row)
        print(b.dumps(row), flush=True)
    return {"api_spend_usd": 0, "training_seed": 11, "validation_seed": 23,
            "test_seeds": [2001, 2002, 2003], "rates": [0.03, 0.1, 0.3], "results": results}


def diagnostics():
    configurations = [("baseline", 32, 128, 0.1, 3), ("more64", 64, 64, 0.1, 3),
        ("more128", 128, 32, 0.1, 3), ("slow", 32, 128, 0.03, 3), ("fast", 32, 128, 0.3, 3),
        ("seed1", 32, 128, 0.1, 1), ("seed7", 32, 128, 0.1, 7), ("seed11", 32, 128, 0.1, 11)]
    rows = []
    for task in ("stealth", "support", "switch", "trade"):
        validation = b.dataset(task, 23, 128)
        for label, count, epochs, rate, seed in configurations:
            examples = b.dataset(task, 11, count)
            brain = b.dense(list(examples[0]["inputs"]), list(examples[0]["targets"]), 4, seed)
            report = brain.train({"id": "fit", "expected_revision": 0, "examples": examples,
                "validation": validation, "epochs": epochs, "rate": rate})
            rows.append({"task": task, "variant": label, "examples": count, "epochs": epochs,
                "rate": rate, "initializer_seed": seed, "validation_correct": decisions(brain, brain.samples(validation)),
                "validation_loss": report["after_loss"], "steps": report["steps"]})
    return rows


def check_rust(binary):
    checked = predictions = 0

    def check(request):
        nonlocal checked
        try:
            expected = b.dumps(process(request))
        except b.BrainError:
            expected = None
        proc = subprocess.run([binary, "brain", "-"], input=json.dumps(request, separators=(",", ":")),
                              capture_output=True, text=True)
        actual = proc.stdout.strip() if proc.returncode == 0 else None
        if actual != expected:
            raise AssertionError(f"prototype/Rust selection differs\n{expected}\n{actual}\n{proc.stderr}")
        checked += 1
        return json.loads(actual) if actual is not None else None

    for task in ("stealth", "support", "switch", "trade"):
        examples, validation = b.dataset(task, 11, 32), b.dataset(task, 23, 128)
        config = {"inputs": list(examples[0]["inputs"]), "outputs": list(examples[0]["targets"]), "hidden": 4, "seed": 3}
        result = check({"dense": config, "operations": [{"op": "train_select", "args": {
            "id": "fit", "expected_revision": 0, "examples": examples, "validation": validation,
            "epochs": 128, "rates": [0.03, 0.1, 0.3], "max_steps": MAX_SELECTION_STEPS}}]})
        saved = result["state"]
        test = sum((b.dataset(task, seed, 128) for seed in (2001, 2002, 2003)), [])
        for at in range(0, len(test), 64):
            operations = [{"op": "run", "args": {"inputs": row["inputs"], "allowed": config["outputs"],
                                                   "min_score": 0, "min_margin": 0}} for row in test[at:at + 64]]
            check({"state": saved, "operations": operations})
            predictions += len(operations)
    for directory in sorted((b.ROOT / "conformance/brain-select").iterdir()):
        check(json.loads((directory / "input.json").read_text()))
    print(f"{checked} selection workflows byte-identical, including {predictions} fresh test predictions")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(b.ROOT / "eval/runtime/brain-accuracy-results.json"))
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--gamectx", default=str(b.ROOT / "target/release/gamectx"))
    args = ap.parse_args()
    if args.check:
        return check_rust(args.gamectx)
    result = measure()
    result["validation_diagnostics"] = diagnostics()
    Path(args.out).write_text(b.dumps(result) + "\n")


if __name__ == "__main__":
    main()
