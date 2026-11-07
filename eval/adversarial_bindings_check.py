"""Persistent regression fixtures for time assessment and stricter restore guards."""
import argparse,copy,importlib.util,json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from routine_detail_bindings_check import cases as prior_cases
from adversarial_reference import assessment
from routines_cases import enc
import routines_reference as reference
ROOT=pathlib.Path(__file__).resolve().parents[1]
def cases():
 prior=prior_cases();out=prior[:-3];saved=next(c['request']['args']['state'] for c in prior if c['request']['op']=='character_routine_restore');time=reference.time_knowledge(saved['history']['records'],saved['frame']['now'],saved['config'],saved['calendar']);wanted=assessment(time)
 def add(op,args,result=None,error=False):
  ident='adversarial_'+str(len(out));out.append(dict(request=dict(version=1,id=ident,op='character_'+op,args=args),expected='ERROR' if error else enc(dict(version=1,id=ident,ok=True,result=result))))
 args=dict(id='one',revision=1,max_bytes=8192);add('routine_assess_time',args,wanted);add('routine_assess_time',args|dict(revision=2),error=True);add('routine_assess_time',args|dict(max_bytes=64),error=True);add('routine_assess_time',args|dict(kind='exact_reading'),error=True)
 add('call',dict(id='one',name='character_routine_assess_time',text='{}',revision=1,max_bytes=8192,allowed=[]),error=True)
 restore=dict(id='one',state=copy.deepcopy(saved),min_revision=1,min_observation=1,min_completion=1,min_tick=0)
 invalid=copy.deepcopy(restore);next(r for r in invalid['state']['timeline'] if r['id']=='walk').update(state='completed',since=0,reason='host');add('routine_restore',invalid,error=True)
 invalid=copy.deepcopy(restore);invalid['state']['frame']['conditions'].append('customer');add('routine_restore',invalid,error=True);add('routine_restore',restore,dict(restored='one'));add('routine_assess_time',args|dict(id='two'),error=True)
 return out+prior[-3:]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args();assert a.cap==0
 spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge);fixtures=cases()
 for client in [bridge.Client(None,ROOT/'target/debug/gamectx'),bridge.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
  with client:
   for case in fixtures:
    req=case['request']
    try:
     got=client.request(req['op'],req['args']);assert case['expected']!='ERROR',req;assert enc(got)==enc(json.loads(case['expected'])['result']),(req,got,case['expected'])
    except ValueError:assert case['expected']=='ERROR',req
   helper=bridge.Character(client,'helper','scope:a','merchant',['merchant']);setup=fixtures[2]['request']['args'];helper.routine_configure(setup['calendar'],setup['definition'],setup['defaults']);helper.routine_update(fixtures[3]['request']['args']['frame']);assert helper.routine_assess_time(1)['kind']=='approximate';helper.close()
 d=ROOT/'target/adversarial-csharp';d.mkdir(parents=True,exist_ok=True);(d/'cases.json').write_text(enc(fixtures)+'\n');(d/'requests.txt').write_text('\n'.join(enc(c['request']) for c in fixtures)+'\n');(d/'expected.txt').write_text('\n'.join(c['expected'] for c in fixtures)+'\n');print(enc(dict(fixtures=len(fixtures),python_checks=2*len(fixtures),api_spend_usd=0)))
if __name__=='__main__':main()
