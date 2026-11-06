#!/usr/bin/env python3
"""Offline reference tests, all shared fixtures and deterministic measurements."""
from pathlib import Path
import sys,unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
import upgrades_eval as u
class UpgradeTests(unittest.TestCase):
    def test_fixtures(self):
        for name,v,error in u.fixtures():
            with self.subTest(case=name):
                if error:
                    with self.assertRaises((ValueError,KeyError,StopIteration,u.b.BrainError)):u.process(v)
                else:self.assertEqual(u.dump(u.process(v)),(u.b.ROOT/'conformance/upgrades'/name/'expected.json').read_text().strip())
    def test_measurements(self):
        self.assertEqual(u.measure(),u.json.loads((u.b.ROOT/'eval/upgrades-results.json').read_text()))
        self.assertEqual(u.measure()['outcomes'],dict(observed_labels=100,unexposed_trainable=0,replay_labels=0))
    def test_generated(self):
        self.assertEqual(len(list(u.generated())),243)
        for v in u.generated():u.process(v)
if __name__=='__main__':unittest.main()
