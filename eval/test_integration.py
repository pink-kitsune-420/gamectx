#!/usr/bin/env python3
"""Offline conformance and scope/expiry/reference invariants. No model calls."""
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
import integration_eval as i
import evidence_eval as e

class IntegrationTests(unittest.TestCase):
    def test_shared_fixtures(self):
        for directory in sorted((i.b.ROOT/'conformance/integration').iterdir()):
            with self.subTest(case=directory.name):
                request=json.loads((directory/'input.json').read_text());case=json.loads((directory/'case.json').read_text())
                if case.get('error'):
                    with self.assertRaises((ValueError,KeyError,TypeError,i.b.BrainError,i.q.q.QueryError)):i.process(request)
                else:self.assertEqual(i.b.dumps(i.process(request)),(directory/'expected.json').read_text().strip())
    def test_stability_holdout_safeguards(self):
        rows=i.measure()['stability']
        self.assertEqual(sum(r['raw_switches'] for r in rows),355)
        self.assertEqual(sum(r['stable_switches'] for r in rows),15)
        self.assertEqual(sum(r['urgent_errors']+r['ineligible_choices'] for r in rows),0)
    def test_evidence_dependencies_remain_tentative_and_time_is_not_renewed(self):
        request=json.loads((i.b.ROOT/'conformance/integration/evidence-repeat-does-not-inflate/input.json').read_text())
        result=e.process(request);h=result['state']['hypotheses'][0]
        self.assertEqual((h['confidence'],h['created'],h['expires']),(.6,2,30))
        self.assertEqual(len(result['state']['journal']['records']),2)
        self.assertEqual(result['results'][-1]['hypotheses'][0]['status'],'contested')

if __name__=='__main__':unittest.main()
