#!/usr/bin/env python3
"""Offline cache parity/ownership/key tests. No API calls."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parent))
import brain_cache_eval as c


class CacheTests(unittest.TestCase):
    def test_shared_fixtures(self):
        for directory in sorted((c.b.ROOT / "conformance/brain-cache").iterdir()):
            with self.subTest(case=directory.name):
                request=json.loads((directory/'input.json').read_text())
                case=json.loads((directory/'case.json').read_text())
                capacity=case['args']['capacity']
                if case.get('error'):
                    with self.assertRaises(c.b.BrainError):c.process(request,capacity)
                else:
                    self.assertEqual(c.b.dumps(c.process(request,capacity)),(directory/'expected.json').read_text().rstrip('\n'))

    def test_input_order_masks_revision_and_owner_are_independent(self):
        brain=c.b.dense(['x','y'],['left','right'],4,3)
        cache=c.Cache(brain,2)
        first=cache.run({'x':0.5,'y':0.25},['left','right'])
        self.assertEqual(cache.run({'y':0.25,'x':0.50000001},['right','left']),first)
        self.assertEqual((cache.hits,cache.misses),(1,1))
        cache.run({'x':None,'y':0.25},['left','right'])
        cache.run({'x':0,'y':0.25},['left','right'])
        self.assertEqual(len(cache.entries),2)
        replacement=c.b.dense(['x','y'],['left','right'],4,99)
        cache.replace(replacement)
        self.assertEqual(cache.run({'x':0.5,'y':0.25},['left','right']),replacement.run({'x':0.5,'y':0.25},['left','right']))


if __name__=='__main__':unittest.main()
