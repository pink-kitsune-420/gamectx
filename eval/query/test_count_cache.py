"""Fake transport tests: no network/model calls; count caching is opt-in."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,pathlib,sys,unittest
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import ui
from count_cache_reference import CachedLocalModel

def model(proto=False,capacity=2,maximum=32):
 cls=CachedLocalModel if proto else ui.LocalModel;m=object.__new__(cls)
 m.model='fixture';m.context=2048;m.maximum=maximum;m.calls=[];m.log=None;m.records={'calls':m.calls};m.identity=dict(model='fixture',digest='a',template_sha256='b',runner='c');m.identify=lambda:copy.deepcopy(m.identity)
 m.count_cache=capacity;m._counts={};m.count_receipts=[];m.capacity=capacity;m.counts={};m.receipts=[]
 m.response_count=100
 m.data=lambda path,payload:dict(done=True,prompt_eval_count=m.response_count,message=dict(content='{"answer":"unknown"}'),done_reason='stop')
 return m
REQUEST=dict(messages=[dict(role='user',content='Choose from current facts.')],format=dict(type='object'))
class Tests(unittest.TestCase):
 def test_independent_prototype_traces_match(self):
  actual=model();ref=model(True)
  for request in [REQUEST,REQUEST,REQUEST|dict(tools=[]),REQUEST,REQUEST|dict(messages=[*REQUEST['messages'],dict(role='tool',content='result')])]:
   self.assertEqual(actual.generate(request,32),ref.generate(request,32))
  trace=lambda m:[(c['phase'],c['payload'],c['response']) for c in m.calls]
  self.assertEqual(trace(actual),trace(ref))
  receipts=lambda rs:[{k:v for k,v in r.items() if k!='status'} for r in rs]
  self.assertEqual(receipts(actual.count_receipts),ref.receipts)
 def test_disabled_cache_keeps_recounting(self):
  m=model(capacity=0)
  for _ in range(3):m.generate(REQUEST,32)
  self.assertEqual([c['phase'] for c in m.calls],['preflight','generation']*3)
 def test_changed_results_corrections_grammar_output_or_context_recount(self):
  for field in ['tool','correction','format','output','context']:
   m=model();m.generate(REQUEST,32);request=copy.deepcopy(REQUEST);out=32
   if field=='tool':request['messages'].append(dict(role='tool',content='Fresh result.'))
   elif field=='correction':request=ui.corrected(request,'bad response')
   elif field=='format':request['format']['additionalProperties']=False
   elif field=='output':out=64
   elif field=='context':m.context=4096
   m.generate(request,out);self.assertEqual([c['phase'] for c in m.calls],['preflight','generation']*2,field)
 def test_identity_change_clears_cached_counts(self):
  m=model();m.generate(REQUEST,32);m.identify=lambda:m.identity|dict(digest='changed')
  with self.assertRaises(ValueError):m.generate(REQUEST,32)
  self.assertFalse(m._counts);self.assertEqual(len(m.calls),2)
 def test_count_mismatch_fails_closed_and_discards_entry(self):
  m=model();m.generate(REQUEST,32);m.response_count=101
  with self.assertRaises(ValueError):m.generate(REQUEST,32)
  self.assertFalse(m._counts);self.assertEqual(m.count_receipts[-1]['status'],'failed')
 def test_capacity_and_complete_turn_budget(self):
  m=model(capacity=1);m.generate(REQUEST,32);m.generate(REQUEST|dict(tools=[]),32);m.generate(REQUEST,32);self.assertEqual(len(m.calls),6);self.assertEqual(len(m._counts),1)
  m=model(maximum=1)
  with self.assertRaises(RuntimeError):m.generate(REQUEST,32)
  self.assertEqual(m.calls,[])
  m=model(maximum=3);m.generate(REQUEST,32);m.generate(REQUEST,32);self.assertEqual(len(m.calls),3)
 def test_transport_settings_and_negative_output_never_reach_backend(self):
  for key in ['model','options','keep_alive','stream']:
   m=model()
   with self.assertRaises(ValueError):m.chat(REQUEST|{key:'override'},1,'preflight')
   self.assertEqual(m.calls,[])
  for output in [-1,0,True,4096]:
   m=model()
   with self.assertRaises(ValueError):m.generate(REQUEST,output)
   self.assertEqual(m.calls,[])
if __name__=='__main__':unittest.main()
