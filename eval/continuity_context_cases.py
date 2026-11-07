"""Stage 5 independent context-scaling oracle; no inference or token assumptions."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from life_cases import base as life_base,event
from life_reference import run
from continuity_cases import base as co_base
ROOT=pathlib.Path(__file__).resolve().parents[1]
def base():
 l=life_base();l['operations']=[dict(op='experience',args=event(n,n)) for n in range(1,7)]
 for n in range(4):
  f=copy.deepcopy(l['definition']['facets'][0]);f['id']='facet:'+str(n);l['definition']['facets'].append(f)
  att=copy.deepcopy(l['definition']['attitudes'][0]);att['id']='attitude:'+str(n);l['definition']['attitudes'].append(att)
 scene=co_base()['operations'][0]['args'];scene['at']=6
 for n in range(8):
  f=copy.deepcopy(scene['facts'][0]);f['id']='fact:'+str(n);scene['facts'].append(f)
 thread=co_base()['operations'][1]['args']
 return dict(life=l,scene=scene,thread=thread,selection=dict(required=['window'],optional=['fact:'+str(n) for n in range(8)],tags=['trade'],input='lamp'),caps=dict(facts=4,facets=2,attitudes=2,episodes=2,exchanges=1),dialogue=dict(version=1,scene='counter',conversation='trade',participants=['merchant','player'],exchanges=[dict(id=str(n),turn=n,user='A wick question.',assistant='An acknowledged answer.') for n in range(1,5)]))
def oracle(a):
 caps=a['caps'];assert all(0<=caps[k]<=m for k,m in [('facts',32),('facets',16),('attitudes',16),('episodes',16),('exchanges',16)])
 selection=a['selection'];assert len(selection['required'])<=caps['facts'];selected=[];omitted=[]
 for needed,ids in [(True,selection['required']),(False,selection['optional'])]:
  for id in ids:
   f=next((f for f in a['scene']['facts'] if f['id']==id and f['kind']!='global' and set(a['life']['participants'])<=set(f['audience'])),None)
   if f and len(selected)<caps['facts']:selected.append(f)
   elif needed:raise ValueError('required missing')
   elif f:omitted.append(id)
 l=copy.deepcopy(a['life']);l['operations'].append(dict(op='context',args=dict(tags=selection['tags'],limit=max(caps['facets'],caps['attitudes']))));character=run(l)['results'][-1]['result'];character['facets']=character['facets'][:caps['facets']];character['attitudes']=character['attitudes'][:caps['attitudes']]
 episodes=[x['args'] for x in l['operations'] if x['op']=='experience'];episodes=episodes[-caps['episodes']:] if caps['episodes'] else []
 history=a['dialogue']['exchanges'];dialogue=[dict(user=x['user'],assistant=x['assistant']) for x in history[-caps['exchanges']:]] if caps['exchanges'] else []
 return dict(packet=dict(version=1,scene=a['scene']['id'],facts=selected,thread=a['thread'],character=character,episodes=episodes,dialogue=dialogue),inspection=dict(omitted_facts=omitted,omitted_exchanges=max(0,len(history)-caps['exchanges']),caps=caps))
def cases():
 out=[('focused',base(),False)]
 for size,recall,exchanges in [('1',1,1),('1.5',1,1),('3',2,1),('8',3,2),('30',4,3),('70',6,3)]:
  a=base();small=exchanges==1;a['caps']=dict(facts=4 if small else 16,facets=2 if small else 8,attitudes=2 if small else 8,episodes=recall,exchanges=exchanges);out.append(('tier_'+size,a,False))
 for n in range(17):
  a=base();a['caps']['facts']=n;out.append(('cap_'+str(n),a,n==0))
 a=base();a['selection']['required']=['missing'];out.append(('missing_required',a,True))
 a=base();a['scene']['facts'][0]['kind']='global';out.append(('global_required',a,True))
 a=base();a['caps']['episodes']=17;out.append(('over_bound',a,True));return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v,error in cases():
  try:wanted=oracle(v);assert not error
  except (AssertionError,ValueError):
   if not error:raise
   wanted=None
  request=dict(kind='character-context',args=v)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(request),text=True,capture_output=True);assert bool(r.returncode)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==json.dumps(wanted,ensure_ascii=False,separators=(',',':')),(name,r.stdout[:500],str(wanted)[:500])
  if a.write_conformance and name in ('focused','tier_70','missing_required','global_required','over_bound'):
   d=ROOT/'conformance/character-context'/name;d.mkdir(parents=True,exist_ok=True)
   for f,o in [('case',dict(op='local',**({'error':True} if error else {}))),('input',request)]+([] if error else [('expected',wanted)]): (d/(f+'.json')).write_text(json.dumps(o,ensure_ascii=False,separators=(',',':'))+'\n')
 print(json.dumps(dict(workflows=len(cases()),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
