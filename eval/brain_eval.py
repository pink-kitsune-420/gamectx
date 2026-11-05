#!/usr/bin/env python3
"""Offline prototype: portable, bounded, trainable reaction networks. No API or keys.

All network math uses signed millionths and integer rounding, including training.
Named normalized inputs and output utility scores have no engine-specific meanings.
"""
import argparse
import copy
import json
from pathlib import Path
import random
import re
import subprocess
import time

S = 1_000_000
MAX = {"inputs": 16, "nodes": 32, "edges": 16, "outputs": 16, "examples": 128,
       "epochs": 128, "steps": 500_000, "history": 8, "updates": 128}
ROOT = Path(__file__).resolve().parent.parent


class BrainError(ValueError):
    pass


def fields(value, names):
    if not isinstance(value, dict) or set(value) != set(names):
        raise BrainError("unexpected or missing fields")


def integer(value, low, high):
    if type(value) is not int or not low <= value <= high:
        raise BrainError("integer outside limits")
    return value


def div(a, b):
    return (1 if a >= 0 else -1) * ((abs(a) + b // 2) // b)


def mul(a, b):
    return div(a * b, S)


def ticks(value, low=-4, high=4):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not low <= value <= high:
        raise BrainError("number outside limits")
    return int(abs(value) * S + 0.5) * (-1 if value < 0 else 1)


def name(value):
    if not isinstance(value, str) or not re.fullmatch(r"[a-z][a-z0-9_]{0,31}", value):
        raise BrainError("invalid name")
    return value


def names(values, limit):
    if not isinstance(values, list) or not 1 <= len(values) <= limit:
        raise BrainError("name count outside limits")
    values = [name(v) for v in values]
    if len(set(values)) != len(values):
        raise BrainError("duplicate name")
    return values


def number(t):
    return t / S if t % S else t // S


def dumps(value):
    # Persisted/computed numeric values are six-place grid decimals, never exponents.
    if isinstance(value, float):
        return f"{value:.6f}".rstrip("0").rstrip(".") if value else "0"
    if isinstance(value, dict):
        return "{" + ",".join(dumps(k) + ":" + dumps(v) for k, v in value.items()) + "}"
    if isinstance(value, list):
        return "[" + ",".join(dumps(v) for v in value) + "]"
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


class Brain:
    def __init__(self, definition):
        fields(definition, ("version", "inputs", "outputs", "nodes"))
        integer(definition["version"], 1, 1)
        self.inputs = names(definition["inputs"], MAX["inputs"])
        self.outputs = names(definition["outputs"], MAX["outputs"])
        nodes = definition["nodes"]
        if not isinstance(nodes, list) or not 1 <= len(nodes) <= MAX["nodes"]:
            raise BrainError("node count outside limits")
        known = list(self.inputs)
        self.nodes, self.base = [], []
        for node in nodes:
            fields(node, ("id", "weights", "bias", "pinned", "learning_limit"))
            ident = name(node["id"])
            if ident in known or type(node["pinned"]) is not bool:
                raise BrainError("duplicate node or invalid pin")
            weights = node["weights"]
            if not isinstance(weights, dict) or len(weights) > MAX["edges"] or any(k not in known for k in weights):
                raise BrainError("unknown, forward or cyclic connection")
            edges = [(known.index(k), k) for k in weights]
            self.nodes.append((ident, edges, node["pinned"], ticks(node["learning_limit"], 0, 4)))
            self.base.append([ticks(node["bias"])] + [ticks(v) for v in weights.values()])
            known.append(ident)
        if any(o not in known[len(self.inputs):] for o in self.outputs):
            raise BrainError("output must name a node")
        self.output_indices = [known.index(o) - len(self.inputs) for o in self.outputs]
        self.weights = copy.deepcopy(self.base)
        self.revision, self.used, self.history = 0, [], []

    def definition(self, current=False):
        return {"version": 1, "inputs": self.inputs, "outputs": self.outputs, "nodes": [
            {"id": ident, "weights": {key: number(row[j + 1]) for j, (_, key) in enumerate(edges)},
             "bias": number(row[0]), "pinned": pinned, "learning_limit": number(limit)}
            for (ident, edges, pinned, limit), row in zip(self.nodes, self.weights if current else self.base)]}

    def matrix(self, raw):
        if not isinstance(raw, list) or len(raw) != len(self.nodes):
            raise BrainError("invalid weight matrix")
        out = []
        for (_, edges, pinned, limit), baseline, values in zip(self.nodes, self.base, raw):
            if not isinstance(values, list) or len(values) != len(edges) + 1:
                raise BrainError("invalid weight row")
            row = [ticks(v) for v in values]
            if any(abs(v - b) > (0 if pinned else limit) for v, b in zip(row, baseline)):
                raise BrainError("weight exceeds pin or learning limit")
            out.append(row)
        return out

    def save(self):
        matrix = lambda rows: [[number(x) for x in row] for row in rows]
        return {"version": 1, "definition": self.definition(), "revision": self.revision,
                "weights": matrix(self.weights), "used_ids": self.used,
                "history": [{"id": ident, "weights": matrix(weights)} for ident, weights in self.history]}

    @classmethod
    def restore(cls, saved):
        fields(saved, ("version", "definition", "revision", "weights", "used_ids", "history"))
        integer(saved["version"], 1, 1)
        brain = cls(saved["definition"])
        used = saved["used_ids"]
        if not isinstance(used, list) or len(used) > MAX["updates"] or any(not isinstance(x, str) for x in used) or len(set(used)) != len(used):
            raise BrainError("invalid used IDs")
        brain.used = [name(x) for x in used]
        brain.revision = integer(saved["revision"], len(used), 2 * len(used))
        history = saved["history"]
        if not isinstance(history, list) or len(history) > min(MAX["history"], 2 * len(used) - brain.revision):
            raise BrainError("invalid history")
        previous = -1
        for entry in history:
            fields(entry, ("id", "weights"))
            if entry["id"] not in used or used.index(entry["id"]) <= previous:
                raise BrainError("invalid history IDs")
            previous = used.index(entry["id"])
            brain.history.append((entry["id"], brain.matrix(entry["weights"])))
        brain.weights = brain.matrix(saved["weights"])
        if not used and brain.weights != brain.base:
            raise BrainError("unlearned weights must equal baseline")
        return brain

    def features(self, values, complete=False):
        if not isinstance(values, dict) or any(k not in self.inputs for k in values):
            raise BrainError("unknown input")
        out = [None if values.get(k) is None else ticks(values[k], 0, 1) for k in self.inputs]
        if complete and any(v is None for v in out):
            raise BrainError("training inputs must be complete")
        return out

    def forward(self, features):
        values, active = list(features), []
        for (_, edges, _, _), weights in zip(self.nodes, self.weights):
            total, unknown = weights[0] * S, False
            for (source, _), weight in zip(edges, weights[1:]):
                value = values[source]
                if weight and value is None:
                    unknown = True
                elif value is not None:
                    total += weight * value
            raw = div(total, S)
            values.append(None if unknown else max(0, min(S, raw)))
            active.append(not unknown and 0 < raw < S)
        return values, active

    def run(self, values, allowed, min_score=0, min_margin=0):
        if not isinstance(allowed, list) or any(not isinstance(a, str) for a in allowed) or len(set(allowed)) != len(allowed) or any(a not in self.outputs for a in allowed):
            raise BrainError("invalid eligible outputs")
        low, gap = ticks(min_score, 0, 1), ticks(min_margin, 0, 1)
        values, _ = self.forward(self.features(values))
        scores = [values[len(self.inputs) + i] for i in self.output_indices]
        eligible = [(score, i) for i, score in enumerate(scores) if self.outputs[i] in allowed]
        choice, margin = None, None
        if eligible and all(score is not None for score, _ in eligible):
            ranked = sorted(eligible, key=lambda pair: -pair[0])
            best, i = ranked[0]
            margin = best - (ranked[1][0] if len(ranked) > 1 else 0)
            if best > low and margin > gap:
                choice = self.outputs[i]
        return {"revision": self.revision,
                "scores": {key: None if v is None else number(v) for key, v in zip(self.outputs, scores)},
                "choice": choice, "margin": None if margin is None else number(margin)}

    def samples(self, raw):
        if not isinstance(raw, list) or not 1 <= len(raw) <= MAX["examples"]:
            raise BrainError("sample count outside limits")
        out = []
        for row in raw:
            fields(row, ("inputs", "targets"))
            fields(row["targets"], self.outputs)
            out.append((self.features(row["inputs"], True), [ticks(row["targets"][k], 0, 1) for k in self.outputs]))
        return out

    def loss(self, samples):
        total = 0
        for features, targets in samples:
            values, _ = self.forward(features)
            for index, target in zip(self.output_indices, targets):
                total += (values[len(self.inputs) + index] - target) ** 2
        return div(total, len(samples) * len(self.outputs) * S)

    def request(self, data):
        name(data["id"])
        integer(data["expected_revision"], self.revision, self.revision)
        if data["id"] in self.used or len(self.used) >= MAX["updates"]:
            raise BrainError("replayed ID or update limit")

    def commit(self, candidate, ident, validation, steps):
        before, after = self.loss(validation), candidate.loss(validation)
        accepted = after < before
        if accepted:
            self.retain(candidate, ident)
        return {"accepted": accepted, "revision": self.revision,
                "before_loss": number(before), "after_loss": number(after), "steps": steps}

    def retain(self, candidate, ident):
        self.history.append((ident, copy.deepcopy(self.weights)))
        self.history = self.history[-MAX["history"]:]
        self.weights = candidate.weights
        self.used.append(ident)
        self.revision += 1

    def teach(self, data):
        fields(data, ("id", "expected_revision", "changes", "validation"))
        self.request(data)
        validation = self.samples(data["validation"])
        changes = data["changes"]
        if not isinstance(changes, list) or not 1 <= len(changes) <= 16:
            raise BrainError("change count outside limits")
        candidate, seen = copy.deepcopy(self), set()
        for change in changes:
            fields(change, ("node", "source", "delta"))
            ident, source = change["node"], change["source"]
            name(ident)
            if not isinstance(source, str):
                raise BrainError("invalid parameter name")
            ids = [node[0] for node in self.nodes]
            if ident not in ids or (ident, source) in seen:
                raise BrainError("unknown or duplicate change")
            seen.add((ident, source))
            index = ids.index(ident)
            _, edges, pinned, _ = self.nodes[index]
            sources = ["$bias"] + [k for _, k in edges]
            if source not in sources or pinned:
                raise BrainError("unknown parameter or pinned node")
            delta = ticks(change["delta"], -0.1, 0.1)
            if not delta:
                raise BrainError("zero delta")
            candidate.weights[index][sources.index(source)] += delta
        candidate.matrix([[number(x) for x in row] for row in candidate.weights])
        steps = 2 * len(validation) * sum(len(w) for w in self.weights)
        return self.commit(candidate, data["id"], validation, steps)

    def train(self, data):
        fields(data, ("id", "expected_revision", "examples", "validation", "epochs", "rate"))
        self.request(data)
        examples, validation = self.samples(data["examples"]), self.samples(data["validation"])
        if any(x == v for x, _ in examples for v, _ in validation):
            raise BrainError("training and validation inputs overlap")
        epochs = integer(data["epochs"], 1, MAX["epochs"])
        rate = ticks(data["rate"], 0, 1)
        if not rate:
            raise BrainError("zero learning rate")
        steps = (3 * epochs * len(examples) + 2 * len(validation)) * sum(len(w) for w in self.weights)
        if steps > MAX["steps"]:
            raise BrainError("training work limit")
        candidate = self.trained(examples, epochs, rate)
        return self.commit(candidate, data["id"], validation, steps)

    def trained(self, examples, epochs, rate):
        candidate = copy.deepcopy(self)
        for _ in range(epochs):
            for features, targets in examples:
                values, active = candidate.forward(features)
                gradient = [0] * len(self.nodes)
                for index, target in zip(self.output_indices, targets):
                    gradient[index] += values[len(self.inputs) + index] - target
                updates = []
                for i in reversed(range(len(self.nodes))):
                    _, edges, pinned, _ = self.nodes[i]
                    delta = max(-S, min(S, gradient[i])) if self.gradient_active(i, active) else 0
                    row = [delta]
                    for j, (source, _) in enumerate(edges):
                        if source >= len(self.inputs):
                            gradient[source - len(self.inputs)] += mul(delta, candidate.weights[i][j + 1])
                        row.append(mul(delta, values[source]))
                    updates.append((i, row if not pinned else [0] * len(row)))
                for i, derivatives in updates:
                    limit = self.nodes[i][3]
                    for j, derivative in enumerate(derivatives):
                        low, high = max(-4 * S, self.base[i][j] - limit), min(4 * S, self.base[i][j] + limit)
                        candidate.weights[i][j] = max(low, min(high, candidate.weights[i][j] - mul(rate, derivative)))
        return candidate

    def gradient_active(self, index, active):
        # Output straight-through derivative lets a saturated policy unlearn an old target.
        # Hidden nodes retain the actual clamp derivative; applying this everywhere hurt quality.
        return active[index] or index in self.output_indices

    def undo(self, revision):
        integer(revision, self.revision, self.revision)
        if not self.history:
            raise BrainError("no undo history")
        _, self.weights = self.history.pop()
        self.revision += 1
        return {"revision": self.revision}


def dense(inputs, outputs, hidden=4, seed=1):
    """Deterministic initializer; no external random library or platform float math."""
    integer(hidden, 0, 16)
    integer(seed, 0, 2**64 - 1)
    inputs, outputs = names(inputs, MAX["inputs"]), names(outputs, MAX["outputs"])
    rng = seed or 1

    def weight():
        nonlocal rng
        rng ^= (rng << 13) & ((1 << 64) - 1)
        rng ^= rng >> 7
        rng ^= (rng << 17) & ((1 << 64) - 1)
        return number((rng % 1000001) - 500000)

    nodes = []
    previous = inputs
    if hidden:
        previous = [f"hidden_{i}" for i in range(hidden)]
        nodes += [{"id": ident, "weights": {key: weight() for key in inputs}, "bias": 0.5,
                   "pinned": False, "learning_limit": 4} for ident in previous]
    nodes += [{"id": ident, "weights": {key: weight() for key in previous}, "bias": 0.5,
               "pinned": False, "learning_limit": 4} for ident in outputs]
    return Brain({"version": 1, "inputs": inputs, "outputs": outputs, "nodes": nodes})


def dataset(task, seed, count):
    rng = random.Random(seed)
    out = []
    for _ in range(count):
        x, y, z = (rng.randrange(S + 1) / S for _ in range(3))
        if task == "stealth":
            inputs = {"danger": x, "exposure": y}
            targets = {"withdraw": x * (0.5 + y / 2), "approach": (1 - x) * (1 - y)}
        elif task == "support":
            inputs = {"need": x, "supply": y, "threat": z}
            targets = {"assist": x * y * (1 - z), "recover": (1 - y) * x, "guard": z}
        elif task == "switch":
            inputs = {"signal_a": x, "signal_b": y}
            targets = {"alternate": x * (1 - y) + (1 - x) * y, "match": x * y + (1 - x) * (1 - y)}
        else:
            inputs = {"stock": x, "demand": y}
            targets = {"buy": max(0, y - x), "sell": max(0, x - y), "hold": 0.15}
        out.append({"inputs": inputs, "targets": {k: number(ticks(v, 0, 1)) for k, v in targets.items()}})
    return out


def accuracy(brain, examples):
    return sum(brain.run(row["inputs"], brain.outputs)["choice"] == max(row["targets"], key=row["targets"].get)
               for row in examples)


def adapted_dataset(seed, count):
    rows = dataset("trade", seed, count)
    for row in rows:
        row["targets"]["hold"] = 0.35
    return rows


def measure():
    results = []
    for task in ("stealth", "support", "switch", "trade"):
        examples, validation = dataset(task, 11, 32), dataset(task, 23, 32)
        test = sum((dataset(task, seed, 128) for seed in (707, 808, 909)), [])
        brain = dense(list(examples[0]["inputs"]), list(examples[0]["targets"]), 4, 3)
        before = accuracy(brain, test)
        start = time.perf_counter()
        report = brain.train({"id": "first", "expected_revision": 0, "examples": examples,
                              "validation": validation, "epochs": 128, "rate": 0.1})
        seconds = time.perf_counter() - start
        row = {"task": task, "heldout": len(test), "correct_before": before,
               "correct_after": accuracy(brain, test), "training_seconds_python": round(seconds, 6),
               "report": report, "saved_bytes": len(dumps(brain.save()).encode())}
        results.append(row)
        print(dumps(row), flush=True)
        if task == "switch":
            linear = dense(list(examples[0]["inputs"]), list(examples[0]["targets"]), 0, 3)
            linear.train({"id": "linear", "expected_revision": 0, "examples": examples,
                          "validation": validation, "epochs": 128, "rate": 0.1})
            results.append({"task": "switch_linear_only", "heldout": len(test), "correct_after": accuracy(linear, test)})
    # Adapt the trading preference on fresh labeled examples and fresh unseen test states.
    test = sum((adapted_dataset(seed, 128) for seed in (1001, 1002, 1003)), [])
    before = accuracy(brain, test)
    report = brain.train({"id": "adapt", "expected_revision": 1, "examples": adapted_dataset(31, 32),
                          "validation": adapted_dataset(47, 32), "epochs": 128, "rate": 0.1})
    row = {"task": "trade_preference_change", "heldout": len(test), "correct_before": before,
           "correct_after": accuracy(brain, test), "report": report}
    results.append(row)
    print(dumps(row), flush=True)
    return results


def comparisons():
    original = Brain.gradient_active
    results = []
    try:
        for variant, gradient in (
            ("clamp_everywhere", lambda self, i, active: active[i]),
            ("straight_through_everywhere", lambda self, i, active: True),
            ("straight_through_outputs", original),
        ):
            Brain.gradient_active = gradient
            correct = 0
            for task in ("stealth", "support", "switch", "trade"):
                examples, validation = dataset(task, 11, 32), dataset(task, 23, 32)
                test = sum((dataset(task, seed, 128) for seed in (101, 202, 303)), [])
                brain = dense(list(examples[0]["inputs"]), list(examples[0]["targets"]), 4, 3)
                brain.train({"id": "fit", "expected_revision": 0, "examples": examples,
                             "validation": validation, "epochs": 128, "rate": 0.1})
                correct += accuracy(brain, test)
            saturated = Brain({"version": 1, "inputs": ["x"], "outputs": ["act"], "nodes": [
                {"id": "act", "weights": {}, "bias": 4, "pinned": False, "learning_limit": 4}]})
            report = saturated.train({"id": "recover", "expected_revision": 0,
                "examples": [{"inputs": {"x": 0.2}, "targets": {"act": 0.2}}],
                "validation": [{"inputs": {"x": 0.8}, "targets": {"act": 0.2}}], "epochs": 128, "rate": 0.1})
            results.append({"variant": variant, "diagnostic_correct": correct, "diagnostic_total": 1536,
                            "saturated_output_recovered": report["accepted"]})
    finally:
        Brain.gradient_active = original
    return results


def process(request):
    if not isinstance(request, dict):
        raise BrainError("workflow must be an object")
    source = [key for key in ("definition", "dense", "state") if key in request]
    if len(source) != 1:
        raise BrainError("one brain source required")
    fields(request, (source[0], "operations"))
    if source[0] == "definition":
        brain = Brain(request["definition"])
    elif source[0] == "state":
        brain = Brain.restore(request["state"])
    else:
        data = request["dense"]
        fields(data, ("inputs", "outputs", "hidden", "seed"))
        brain = dense(**data)
    operations = request["operations"]
    if not isinstance(operations, list) or len(operations) > 64:
        raise BrainError("operation count outside limits")
    results = []
    for operation in operations:
        fields(operation, ("op", "args"))
        op, args = operation["op"], operation["args"]
        if op == "run":
            fields(args, ("inputs", "allowed", "min_score", "min_margin"))
            results.append(brain.run(args["inputs"], args["allowed"], args["min_score"], args["min_margin"]))
        elif op == "train":
            results.append(brain.train(args))
        elif op == "teach":
            results.append(brain.teach(args))
        elif op == "undo":
            fields(args, ("expected_revision",))
            results.append(brain.undo(args["expected_revision"]))
        elif op == "inspect":
            fields(args, ())
            results.append({"revision": brain.revision, "definition": brain.definition(True)})
        else:
            raise BrainError("unknown brain operation")
    return {"results": results, "state": brain.save()}


def check_rust(binary):
    identical = predictions = 0

    def check(request):
        nonlocal identical
        try:
            expected = dumps(process(request))
        except BrainError:
            expected = None
        result = subprocess.run([binary, "brain", "-"], input=json.dumps(request, separators=(",", ":")),
                                capture_output=True, text=True)
        actual = result.stdout.strip() if result.returncode == 0 else None
        if expected != actual:
            raise AssertionError(f"prototype/Rust differ: {expected}\n{actual}\n{result.stderr}")
        identical += 1
        return json.loads(actual) if actual is not None else None

    for task in ("stealth", "support", "switch", "trade"):
        examples, validation = dataset(task, 11, 32), dataset(task, 23, 32)
        config = {"inputs": list(examples[0]["inputs"]), "outputs": list(examples[0]["targets"]), "hidden": 4, "seed": 3}
        request = {"dense": config, "operations": [{"op": "train", "args": {
            "id": "first", "expected_revision": 0, "examples": examples, "validation": validation,
            "epochs": 128, "rate": 0.1}}]}
        saved = check(request)["state"]
        test = sum((dataset(task, seed, 128) for seed in (707, 808, 909)), [])
        for at in range(0, len(test), 64):
            operations = [{"op": "run", "args": {"inputs": row["inputs"], "allowed": config["outputs"],
                                                   "min_score": 0, "min_margin": 0}} for row in test[at:at + 64]]
            check({"state": saved, "operations": operations})
            predictions += len(operations)
        if task == "switch":
            linear_request = copy.deepcopy(request)
            linear_request["dense"]["hidden"] = 0
            linear_request["operations"][0]["args"]["id"] = "linear"
            linear = check(linear_request)["state"]
            for at in range(0, len(test), 64):
                operations = [{"op": "run", "args": {"inputs": row["inputs"], "allowed": config["outputs"],
                                                       "min_score": 0, "min_margin": 0}} for row in test[at:at + 64]]
                check({"state": linear, "operations": operations})
                predictions += len(operations)
        if task == "trade":
            adapted = check({"state": saved, "operations": [{"op": "train", "args": {
                "id": "adapt", "expected_revision": 1, "examples": adapted_dataset(31, 32),
                "validation": adapted_dataset(47, 32), "epochs": 128, "rate": 0.1}}]})["state"]
            test = sum((adapted_dataset(seed, 128) for seed in (1001, 1002, 1003)), [])
            for at in range(0, len(test), 64):
                operations = [{"op": "run", "args": {"inputs": row["inputs"], "allowed": config["outputs"],
                                                       "min_score": 0, "min_margin": 0}} for row in test[at:at + 64]]
                check({"state": adapted, "operations": operations})
                predictions += len(operations)
    rng = random.Random(17)
    for _ in range(250):
        brain = dense(["x", "y"], ["left", "right"], rng.randrange(5), rng.randrange(2**32))
        definition = brain.definition()
        if rng.randrange(3) == 0:
            node = rng.choice(definition["nodes"])
            node[rng.choice(["bias", "pinned", "id", "weights"])] = rng.choice([None, True, 5, "unknown", {}, []])
        operation = {"op": "run", "args": {"inputs": {"x": rng.choice([None, 0, 0.5, 1]), "y": 0.25},
                                             "allowed": rng.choice([[], ["left"], ["left", "right"]]),
                                             "min_score": rng.choice([0, 0.5]), "min_margin": 0}}
        check({"definition": definition, "operations": [operation]})
    print(f"{identical} workflows byte-identical, including training, saved state and {predictions} held-out predictions")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(ROOT / "eval/brain-results.json"))
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--gamectx", default=str(ROOT / "target/release/gamectx"))
    args = ap.parse_args()
    if args.check:
        return check_rust(args.gamectx)
    results = measure()
    data = {"kind": "offline synthetic utility imitation; no model calls", "api_spend_usd": 0,
            "training_seed": 11, "validation_seed": 23, "diagnostic_seeds": [101, 202, 303],
            "heldout_seeds": [707, 808, 909], "adaptation_heldout_seeds": [1001, 1002, 1003],
            "results": results, "gradient_comparisons": comparisons()}
    Path(args.out).write_text(dumps(data) + "\n")


if __name__ == "__main__":
    main()
