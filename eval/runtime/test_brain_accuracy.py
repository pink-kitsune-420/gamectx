#!/usr/bin/env python3
"""Free selection-contract tests against shared fixtures and atomic state updates."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import brain_accuracy_eval as a

ROOT = Path(__file__).resolve().parents[2] / "conformance/brain-select"


class SelectionTests(unittest.TestCase):
    def test_shared_cases(self):
        for directory in sorted(ROOT.iterdir()):
            with self.subTest(case=directory.name):
                request = json.loads((directory / "input.json").read_text())
                case = json.loads((directory / "case.json").read_text())
                if case.get("error"):
                    with self.assertRaises(a.b.BrainError):
                        a.process(request)
                else:
                    self.assertEqual(a.b.dumps(a.process(request)), (directory / "expected.json").read_text().rstrip("\n"))

    def test_lower_mse_cannot_replace_correct_reaction_with_wrong_one(self):
        request = json.loads((ROOT / "reject-decision-regression/input.json").read_text())
        brain = a.b.Brain(request["definition"])
        before = a.b.dumps(brain.save())
        result = a.train_select(brain, request["operations"][0]["args"])
        self.assertLess(result["trials"][0]["loss"], result["before_loss"])
        self.assertEqual(result["trials"][0]["correct"], 0)
        self.assertFalse(result["accepted"])
        self.assertEqual(a.b.dumps(brain.save()), before)

    def test_only_winner_changes_active_history(self):
        request = json.loads((ROOT / "select-earlier-tie/input.json").read_text())
        brain = a.b.Brain(request["definition"])
        before = copy.deepcopy(brain.weights)
        result = a.train_select(brain, request["operations"][0]["args"])
        self.assertEqual(result["selected_rate"], 0.5)
        self.assertEqual(brain.revision, 1)
        self.assertEqual(len(brain.history), 1)
        brain.undo(1)
        self.assertEqual(brain.weights, before)


if __name__ == "__main__":
    unittest.main()
