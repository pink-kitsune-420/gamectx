#!/usr/bin/env python3
"""Free checks for the measured brain prototype and conformance fixtures."""
import copy
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import brain_eval as b


class BrainTests(unittest.TestCase):
    def test_conformance(self):
        root = Path(__file__).resolve().parent.parent / "conformance/brain"
        for directory in sorted(root.iterdir()):
            with self.subTest(case=directory.name):
                case = json.loads((directory / "case.json").read_text())
                request = json.loads((directory / "input.json").read_text())
                if case.get("error"):
                    with self.assertRaises(b.BrainError):
                        b.process(request)
                else:
                    self.assertEqual(b.dumps(b.process(request)), (directory / "expected.json").read_text().rstrip("\n"))

    def test_rejected_update_is_atomic_and_undo_blocks_replay(self):
        brain = b.dense(["x"], ["act"], 0, 3)
        validation = [{"inputs": {"x": 0.25}, "targets": {"act": 1}}]
        old = b.dumps(brain.save())
        bad = {"id": "lesson", "expected_revision": 0, "changes": [{"node": "act", "source": "$bias", "delta": -0.1}],
               "validation": validation}
        self.assertFalse(brain.teach(bad)["accepted"])
        self.assertEqual(b.dumps(brain.save()), old)
        good = copy.deepcopy(bad)
        good["changes"][0]["delta"] = 0.1
        self.assertTrue(brain.teach(good)["accepted"])
        brain.undo(1)
        self.assertEqual(brain.weights, brain.base)
        good["expected_revision"] = 2
        with self.assertRaises(b.BrainError):
            brain.teach(good)
        self.assertEqual(b.dumps(b.Brain.restore(brain.save()).save()), b.dumps(brain.save()))


if __name__ == "__main__":
    unittest.main()
