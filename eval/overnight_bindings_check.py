"""Python CLI/native parity and shared Node/C# fixture preparation; no providers."""
import argparse,importlib.util,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
from overnight_memory_cases import cases,select
from life_cases import base

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);a=p.parse_args();assert a.cap==0
 spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b);count=0
 for client in [b.Client(None,ROOT/'target/debug/gamectx'),b.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
  with client:
   for i,(name,es,q,archived,error) in enumerate(cases()):
    l=base();npc=b.Character(client,'selected_'+str(i),'deep_save','merchant',l['participants']);npc.life_configure(l['setting'],l['definition'],l['defaults'],{})
    for e in es:npc.life_experience(e)
    if archived:npc.life('consolidate',start=1,end=5);npc.life('ack',through=5);npc.life('evict',through=5)
    before=npc.life('inspect')
    try:
     got=npc.life_recall_selected(q);assert not error,name;assert got==select([] if archived else es,[dict(events=es)] if archived else [],q),name
    except ValueError:assert error,name
    assert npc.life('inspect')==before,name;npc.close();count+=1
 fixtures=[]
 for family in ['deep-memory','deep-transient','deep-causality','deep-speech','deep-command','deep-required-speech']:
  for f in sorted((ROOT/'conformance'/family).glob('*/input.json')):
   i=str(len(fixtures));request=dict(version=1,id=i,op='local',args=json.loads(f.read_text()));error=json.loads(f.with_name('case.json').read_text()).get('error',False);expected=None if error else dict(version=1,id=i,ok=True,result=json.loads(f.with_name('expected.json').read_text()));fixtures.append(dict(request=request,expected=expected,error=error))
 out=ROOT/'target/overnight-bindings';out.mkdir(exist_ok=True)
 def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
 (out/'cases.json').write_text(enc(fixtures)+'\n');(out/'requests.txt').write_text(''.join(enc(f['request'])+'\n' for f in fixtures));(out/'expected.txt').write_text(''.join(('ERROR' if f['error'] else enc(f['expected']))+'\n' for f in fixtures))
 print(enc(dict(python_stateful_checks=count,shared_fixtures=len(fixtures),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
