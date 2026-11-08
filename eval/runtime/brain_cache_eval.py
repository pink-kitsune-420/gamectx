#!/usr/bin/env python3
"""Offline prototype of a bounded, per-brain FIFO prediction cache; no APIs/keys.

The graph is already compiled by Brain. Cache keys normalize values to millionths and
include action eligibility, thresholds and revision. Entries never become saved learning.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import collections
import copy
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import brain_eval as b
import brain_accuracy_eval as a


class Cache:
    def __init__(self, brain, capacity):
        b.integer(capacity, 0, 256)
        self.brain, self.capacity = brain, capacity
        self.entries = collections.OrderedDict()
        self.hits = self.misses = 0
        self.binding = (id(brain), brain.revision)

    def clear(self):
        self.entries.clear()
        self.binding = (id(self.brain), self.brain.revision)

    def replace(self, brain):
        self.brain = brain
        self.clear()

    def run(self, inputs, allowed, min_score=0, min_margin=0):
        if self.binding != (id(self.brain), self.brain.revision):
            self.clear()
        features = tuple(self.brain.features(inputs))
        if not isinstance(allowed, list) or any(not isinstance(k, str) for k in allowed) or len(set(allowed)) != len(allowed) or any(k not in self.brain.outputs for k in allowed):
            raise b.BrainError("invalid eligible outputs")
        mask = tuple(k in allowed for k in self.brain.outputs)
        key = (features, mask, b.ticks(min_score, 0, 1), b.ticks(min_margin, 0, 1), self.brain.revision)
        if self.capacity and key in self.entries:
            self.hits += 1
            return copy.deepcopy(self.entries[key])
        result = self.brain.run(inputs, allowed, min_score, min_margin)
        self.misses += 1
        if self.capacity:
            if len(self.entries) == self.capacity:
                self.entries.popitem(last=False)
            self.entries[key] = copy.deepcopy(result)
        return result


def process(request, capacity):
    if not isinstance(request, dict):
        raise b.BrainError("workflow must be an object")
    source = [key for key in ("definition", "dense", "state") if key in request]
    if len(source) != 1:
        raise b.BrainError("one brain source required")
    b.fields(request, (source[0], "operations"))
    initial = a.process({source[0]: request[source[0]], "operations": []})
    cache = Cache(b.Brain.restore(initial["state"]), capacity)
    operations = request["operations"]
    if not isinstance(operations, list) or len(operations) > 64:
        raise b.BrainError("operation count outside limits")
    results = []
    for operation in operations:
        b.fields(operation, ("op", "args"))
        op, args = operation["op"], operation["args"]
        if op == "run":
            b.fields(args, ("inputs", "allowed", "min_score", "min_margin"))
            result = cache.run(args["inputs"], args["allowed"], args["min_score"], args["min_margin"])
        elif op == "inspect":
            b.fields(args, ())
            result = {"revision": cache.brain.revision, "definition": cache.brain.definition(True)}
        else:
            cache.clear()
            result = a.process({"state": cache.brain.save(), "operations": [operation]})
            cache.replace(b.Brain.restore(result["state"]))
            result = result["results"][0]
        results.append(result)
    return {"results": results, "state": cache.brain.save()}


def check(binary):
    checked = 0
    for category in ("brain", "brain-select", "brain-cache"):
        for directory in sorted((b.ROOT / "conformance" / category).iterdir()):
            request = json.loads((directory / "input.json").read_text())
            case = json.loads((directory / "case.json").read_text())
            capacity = case.get("args", {}).get("capacity", 2)
            try:
                expected = b.dumps(process(request, capacity))
            except b.BrainError:
                expected = None
            proc = subprocess.run([binary, "brain", "-", "--cache", str(capacity)],
                                  input=json.dumps(request), capture_output=True, text=True)
            actual = proc.stdout.strip() if proc.returncode == 0 else None
            if actual != expected:
                raise AssertionError(f"cache differs: {directory.name}\n{expected}\n{actual}\n{proc.stderr}")
            checked += 1
    print(f"{checked} cached workflows byte-identical to Python; capacity 2 unless overridden")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--check":
        check(str(b.ROOT / "target/release/gamectx"))
        raise SystemExit(0)
    brain = b.dense(["x", "y"], ["left", "right"], 4, 3)
    cache = Cache(brain, 2)
    for _ in range(3):
        assert cache.run({"x": 0.25, "y": 0.5}, ["left", "right"]) == brain.run({"x": 0.25, "y": 0.5}, ["left", "right"])
    print(f"prototype cache: {cache.hits} hits, {cache.misses} misses; outputs identical")
