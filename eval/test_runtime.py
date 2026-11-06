#!/usr/bin/env python3
"""Offline reference, slicing and local-prefix harness tests; no server required."""
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
import runtime_eval as r
import prefix_cache_eval as p

class RuntimeTests(unittest.TestCase):
    def test_shared_cases(self):
        for directory in sorted((r.b.ROOT/'conformance/runtime').iterdir()):
            with self.subTest(case=directory.name):
                case=json.loads((directory/'case.json').read_text())
                request=json.loads((directory/'input.json').read_text())
                if case.get('error'):
                    with self.assertRaises((ValueError,r.b.BrainError,KeyError,TypeError)):r.process(request)
                else:self.assertEqual(r.b.dumps(r.process(request)),(directory/'expected.json').read_text().strip())
    def test_slicing_matches_training(self):
        examples=r.b.dataset('stealth',11,8);validation=r.b.dataset('stealth',23,8)
        brain=r.b.dense(list(examples[0]['inputs']),list(examples[0]['targets']),4,3)
        request=dict(id='test',expected_revision=0,examples=examples,validation=validation,epochs=8,rate=.1)
        job=r.SlicedTraining(brain,request)
        with self.assertRaises(r.b.BrainError):job.finish()
        while job.cursor<64:job.advance(3)
        self.assertEqual(job.finish(),brain.trained(brain.samples(examples),8,r.b.ticks(.1,0,1)).weights)
    def test_prefix_accounting_and_local_endpoint(self):
        self.assertGreater(p.common_prefix(*[s.encode() for s in p.prompts(2)]),1000)
        self.assertEqual(p.local_url('http://127.0.0.1:8080'),'http://127.0.0.1:8080/completion')
        for url in ('https://127.0.0.1','http://example.invalid','http://user@localhost','http://localhost/remote','http://localhost/?key=x'):
            with self.assertRaises(ValueError):p.local_url(url)
        with self.assertRaises(ValueError):p.NoRedirect().redirect_request(None,None,None,None,None,None)

if __name__=='__main__':unittest.main()
