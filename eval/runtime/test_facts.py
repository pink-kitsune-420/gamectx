#!/usr/bin/env python3
"""Offline fact assertions, exact decimals and byte-budget reference checks."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
import facts_eval as f


class FactTests(unittest.TestCase):
    def test_shared_fixtures(self):
        for directory in sorted((f.b.ROOT/'conformance/facts').iterdir()):
            with self.subTest(case=directory.name):
                request=f.loads((directory/'input.json').read_text())
                case=f.loads((directory/'case.json').read_text())
                if case.get('error'):
                    with self.assertRaises((ValueError, KeyError, TypeError, f.b.BrainError)):
                        f.process(request)
                else:
                    self.assertEqual(f.dumps(f.process(request)), (directory/'expected.json').read_text().strip())

    def test_exact_numbers_and_typed_values(self):
        self.assertTrue(f.equivalent(f.loads('1e1000'), f.loads('10e999')))
        self.assertTrue(f.equivalent(f.loads('-0.0'), 0))
        self.assertFalse(f.equivalent(9007199254740993, 9007199254740992))
        self.assertFalse(f.equivalent(True, 1))
        self.assertFalse(f.equivalent('1', 1))
        for text in ['1e1000001', '['+','.join(['0']*65)+']', '"'+'x'*4097+'"']:
            with self.assertRaises(ValueError):
                f.validate_value(f.loads(text))

    def test_measurement_is_reproducible(self):
        self.assertEqual(f.measure(), f.loads((f.b.ROOT/'eval/runtime/facts-results.json').read_text()))


if __name__ == '__main__':
    unittest.main()
