#!/usr/bin/env python3
"""Offline plan/shape reference tests; guidance experiments are never run here."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import json,sys,unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import query_upgrade_eval as u
class UpgradeTests(unittest.TestCase):
    def test_shared_fixtures(self):
        for label,v,error in u.fixtures():
            with self.subTest(case=label):
                if error:
                    with self.assertRaises((ValueError,u.q.QueryError)):u.process(v)
                else:self.assertEqual(u.dump(u.process(v)),(u.r.ROOT/'conformance/query-plan'/label/'expected.json').read_text().strip())
    def test_literal_precision_and_column_order(self):
        p=u.plan("SELECT SUM(mass), COUNT(*) FROM cargo WHERE kind IN ('food','tool') AND mass >= -1.234567890123 ORDER BY mass DESC LIMIT 3")
        self.assertEqual(p['columns'],['mass','kind']);self.assertIn('-1.234567890123',u.dump(p));self.assertEqual(p['limit'],3)
    def test_strict_shapes_are_not_automatic_intent_inference(self):
        m=u.offline_measure();self.assertEqual(m['recorded_query_shapes'],dict(accepted=69,rejected=27,correct_rejected=22,total=96));self.assertEqual(m['unsafe_in_rewrite']['top_three_sum'],28);self.assertEqual(m['unsafe_in_rewrite']['in_membership_sum'],37)
    def test_fresh_unseen_states_have_valid_oracles(self):
        self.assertFalse(set(x[1] for x in u.FRESH)&set(x[1] for x in u.r.STATES))
        for family,seed,size in u.FRESH:
            data=u.r.state(family,seed,size)
            for question in data['questions']:self.assertTrue(u.r.correct(question['expected'],u.q.run_sql(question['oracle'],data['tables'])))
if __name__=='__main__':unittest.main()
