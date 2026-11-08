#!/usr/bin/env python3
"""Offline budget/protocol/oracle checks; never reads real credentials or calls a model."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,json,sys,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parent))
import release_model_eval as r
class Reply:
    def __init__(self,v):self.v=json.dumps(v).encode()
    def __enter__(self):return self
    def __exit__(self,*args):pass
    def read(self,*args):return self.v
class ReleaseTests(unittest.TestCase):
    def test_frozen_native_prompts_stay_identical(self):
        saved=r.ROOT/'eval/query/release-model-tokens.json'
        if not saved.exists():self.skipTest('no recorded token run')
        with tempfile.TemporaryDirectory() as folder:
            actual=[{k:v for k,v in d.items() if k!='tables_path'} for d in r.prepare(str(r.ROOT/'target/release/gamectx'),Path(folder))]
        self.assertEqual(json.loads(saved.read_text())['states'],actual)
    def test_native_retriable_eof_and_terminal_feedback(self):
        with tempfile.TemporaryDirectory() as folder:
            tables=Path(folder)/'tables.json';tables.write_text('{"t":[{"id":"a","x":2}]}')
            first=r.feedback(str(r.ROOT/'target/release/gamectx'),tables,[''])
            self.assertTrue(first[0]['retry'])
            replies=r.feedback(str(r.ROOT/'target/release/gamectx'),tables,['','SELECT id FROM t'])
            self.assertEqual(replies[0],first[0]);self.assertEqual(replies[1],{'result':['a']})
            self.assertFalse(r.feedback(str(r.ROOT/'target/release/gamectx'),tables,['',''])[1]['retry'])
    def test_resume_reuses_completed_answers_without_dispatch(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);credential=root/'.config/gamectx-eval';credential.mkdir(parents=True);(credential/'venice.key').write_text('dummy')
            prior=root/'prior.json';output=root/'new.json';prices={'test':dict(input=1.,output=1.)}
            bundle=dict(seed=1,family='test',size=1,tables={},metadata={},questions=[dict(kind='count',question='Count?',expected=1)],prompts={'json':'frozen'},tables_path=root/'tables.json')
            state={k:v for k,v in bundle.items() if k!='tables_path'}
            row=dict(model='test',seed=1,mode='json',question='Count?',correct=True,first_correct=False,executed_correct=False,attempts=[],calls=[])
            commit=r.subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
            prior.write_text(json.dumps(dict(source_commit=commit,release='0.1.0-alpha.3',tokens_only=False,prices=prices,states=[state],spend_usd=.001,call_ledger=[],rows=[row],eval_sha256='original')))
            args=argparse.Namespace(resume=prior,output=output,tokens_only=False,base_url='unused',cap=.002)
            with patch.object(r.Path,'home',return_value=root),patch.object(r.m,'get_prices',return_value=prices),patch.object(r,'MODELS',('test',)),patch.object(r,'MODES',('json',)),patch.object(r,'ask') as ask:
                r.evaluate(args,[bundle]);ask.assert_not_called()
            saved=json.loads(output.read_text());self.assertTrue(saved['completed']);self.assertEqual(saved['rows'],[row]);self.assertEqual(saved['spend_usd'],.001)
    def test_fresh_families_have_independent_oracles(self):
        self.assertEqual(r.MODELS,('llama-3.2-3b','mistral-small-3-2-24b-instruct'))
        self.assertEqual(len(r.STATES),6)
        for family,seed,size in r.STATES:
            data=r.state(family,seed,size);self.assertEqual(len(data['questions']),8);self.assertEqual(len(data['tables'][family]),size)
            for question in data['questions']:
                self.assertTrue(r.correct(question['expected'],r.q.run_sql(question['oracle'],data['tables'])))
    def test_strict_json_and_scoring(self):
        self.assertEqual(r.extract('```json\n{"answer":2}\n```'),{'answer':2})
        for text in ['{"answer":1,"answer":2}','{"answer":NaN}','{"answer":1} trailing']:self.assertIsNone(r.extract(text))
        self.assertFalse(r.correct(1,True));self.assertFalse(r.correct(['a'],['a','a']));self.assertTrue(r.correct(['b','a'],['a','b']))
        self.assertFalse(r.correct(1,float('nan')))
    def test_reserved_budget_and_unknown_usage(self):
        args=argparse.Namespace(cap=.002,base_url='https://api.venice.ai/api/v1');ledger=dict(spent=0.,calls=[]);messages=[dict(role='user',content='x'*1000)];prices={'test':{'input':1.,'output':1.}}
        reply={'choices':[{'message':{'content':'{}'}}],'usage':{'prompt_tokens':1000,'completion_tokens':10}}
        with patch.object(r.urllib.request,'urlopen',return_value=Reply(reply)) as request:
            r.ask(messages,'test',10,args,'dummy',prices,ledger,lambda:None);self.assertAlmostEqual(ledger['spent'],.00101)
            with self.assertRaises(StopIteration):r.ask(messages,'test',10,args,'dummy',prices,ledger,lambda:None)
            self.assertEqual(request.call_count,1)
        ledger=dict(spent=0.,calls=[]);reply.pop('usage')
        with patch.object(r.urllib.request,'urlopen',return_value=Reply(reply)):r.ask(messages,'test',10,args,'dummy',prices,ledger,lambda:None)
        self.assertAlmostEqual(ledger['spent'],.001522)
        ledger=dict(spent=0.,calls=[])
        with patch.object(r.urllib.request,'urlopen',side_effect=TimeoutError('uncertain')) as request:
            with self.assertRaises(TimeoutError):r.ask(messages,'test',10,args,'dummy',prices,ledger,lambda:None)
            self.assertEqual(request.call_count,1)
        self.assertAlmostEqual(ledger['spent'],.001522)
    def test_overrun_stops_dispatches(self):
        args=argparse.Namespace(cap=.002,base_url='https://api.venice.ai/api/v1');ledger=dict(spent=0.,calls=[])
        reply={'choices':[{'message':{'content':'{}'}}],'usage':{'prompt_tokens':2000,'completion_tokens':10}}
        with patch.object(r.urllib.request,'urlopen',return_value=Reply(reply)):
            with self.assertRaises(StopIteration):r.ask([dict(role='user',content='x')],'test',10,args,'dummy',{'test':{'input':1.,'output':1.}},ledger,lambda:None)
        self.assertAlmostEqual(ledger['spent'],.00201)
if __name__=='__main__':unittest.main()
