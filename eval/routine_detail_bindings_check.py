"""Additional host context/timer fixtures; reuses the frozen 43 baseline checks."""
import argparse,copy,importlib.util,json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from routine_bindings_check import cases as base_cases
from routines_cases import enc
import routines_reference as ref
ROOT=pathlib.Path(__file__).resolve().parents[1]
def cases():
 original=base_cases();out=original[:-3];saved=next(c['request']['args']['state'] for c in original if c['request']['op']=='character_routine_restore');record=saved['history']['records'][0]
 selection=dict(kinds=['direct'],locations=['court'],time_sources=['sun'],required=[1],limit=1)
 def add(op,a,wanted=None,error=False):
  ident='routine_detail_'+str(len(out));out.append(dict(request=dict(version=1,id=ident,op='character_'+op,args=a),expected='ERROR' if error else enc(dict(version=1,id=ident,ok=True,result=wanted))))
 context=dict(revision=1,current=None,next=dict(id='walk',location='court',state='planned'),observations=[record],time=ref.time_knowledge([record],0,saved['config'],'cycle:37'))
 add('routine_context',dict(id='one',selection=selection,revision=1,max_bytes=8192),context)
 add('routine_wake',dict(id='one'),dict(revision=1,at=11))
 add('routine_affected',dict(id='one',conditions=['distant'],activities=[]),dict(affected=False))
 add('routine_affected',dict(id='one',conditions=['customer'],activities=[]),dict(affected=True))
 add('routine_affected',dict(id='one',conditions=[],activities=['walk']),dict(affected=True))
 add('routine_context',dict(id='one',selection=selection|dict(required=[99]),revision=1,max_bytes=8192),error=True)
 add('routine_context',dict(id='one',selection=selection,revision=2,max_bytes=8192),error=True)
 add('routine_context',dict(id='one',selection=selection,revision=1,max_bytes=64),error=True)
 add('call',dict(id='one',name='character_routine_context',text='{}',revision=1,max_bytes=8192,allowed=[]),error=True)
 return out+original[-3:]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args();assert a.cap==0
 spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
 fixtures=cases()
 for client in [bridge.Client(None,ROOT/'target/debug/gamectx'),bridge.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
  with client:
   for case in fixtures:
    req=case['request']
    try:
     got=client.request(req['op'],req['args']);assert case['expected']!='ERROR',req;assert enc(got)==enc(json.loads(case['expected'])['result']),(req,got,case['expected'])
    except ValueError:assert case['expected']=='ERROR',req
   helper=bridge.Character(client,'helper','scope:a','merchant',['merchant']);setup=fixtures[2]['request']['args'];helper.routine_configure(setup['calendar'],setup['definition'],setup['defaults']);helper.routine_update(fixtures[3]['request']['args']['frame']);assert helper.routine_wake()==dict(revision=1,at=11);assert helper.routine_affected(['customer'])==dict(affected=True);helper.close()
 d=ROOT/'target/routine-detail-csharp';d.mkdir(parents=True,exist_ok=True);(d/'cases.json').write_text(enc(fixtures)+'\n');(d/'requests.txt').write_text('\n'.join(enc(c['request']) for c in fixtures)+'\n');(d/'expected.txt').write_text('\n'.join(c['expected'] for c in fixtures)+'\n')
 print(enc(dict(fixtures=len(fixtures),additional=9,python_checks=2*len(fixtures),api_spend_usd=0)))
if __name__=='__main__':main()
