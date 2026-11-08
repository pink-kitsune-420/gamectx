"""Predeclared scoped archive selection and transient compaction oracle; no network."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,json,pathlib,re,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from life_cases import base,event
ROOT=pathlib.Path(__file__).resolve().parents[2]
def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def words(s):
 values=set(re.findall(r'[^\W_]+',s.lower(),re.UNICODE));return values|{w[:-1] for w in values if len(w)>3 and w.endswith('s')}
def select(events,summaries,q):
 assert set(q)==set('input tags subjects required count archive'.split())
 assert isinstance(q['input'],str) and len(q['input'].encode())<=2048 and type(q['archive']) is bool
 assert type(q['count']) is int and 0<=q['count']<=16
 for key,cap in [('tags',8),('subjects',16)]:
  assert len(q[key])<=cap and len(set(q[key]))==len(q[key]) and all(isinstance(s,str) and re.fullmatch(r'[A-Za-z0-9_:.-]{1,128}',s) for s in q[key])
 assert q['subjects'] and set(q['subjects'])<=set(['player','merchant','colony'])
 assert len(q['required'])<=16 and len(set(q['required']))==len(q['required']) and all(type(n)is int and n>0 for n in q['required'])
 pool={e['token']:e for s in (summaries if q['archive'] else []) for e in s['events']};pool.update({e['token']:e for e in events})
 want=words(q['input']+' '+' '.join(q['tags']))
 def group(token):
  chosen={token};changed=True
  while changed:
   changed=False
   for n,e in pool.items():
    if n in chosen or set(e['causes'])&chosen:
     for t in [n]+e['causes']:
      assert t in pool
      if t not in chosen:chosen.add(t);changed=True
  assert all(pool[n]['subject'] in q['subjects'] for n in chosen)
  return chosen
 chosen=set()
 for t in q['required']:chosen|=group(t)
 assert len(chosen)<=q['count']
 ranked=sorted(pool,key=lambda n:(-len(want&words(pool[n]['text']+' '+pool[n]['tag'])),-int(pool[n]['retain']),-pool[n]['at'],-n))
 for n in ranked:
  if not want&words(pool[n]['text']+' '+pool[n]['tag']):continue
  try:g=group(n)
  except AssertionError:continue
  if len(g|chosen)<=q['count']:chosen|=g
 return [copy.deepcopy(pool[n]) for n in sorted(chosen)]
def cases():
 es=[event(1,1,tag='preference',text='player prefers amber wick'),event(2,2,tag='name',text='player name Vira'),event(3,3,tag='correction',text='player name Vela, not Vira',causes=[2]),event(4,4,tag='promise',text='player promised to return borrowed lamp'),event(5,5,subject='colony',tag='preference',text='colony prefers blue wick')]
 q=dict(input='amber Vela promise',tags=[],subjects=['player'],required=[2,4],count=4,archive=True)
 out=[]
 def add(name,q=q,archived=False,error=False):out.append((name,copy.deepcopy(es),copy.deepcopy(q),archived,error))
 add('anchors');add('after_eviction',archived=True);add('other_subject',q=dict(q,subjects=['colony'],required=[],input='wick'));add('causal_cap',q=dict(q,count=2),error=True);add('missing_anchor',q=dict(q,required=[99]),error=True);add('wrong_subject_anchor',q=dict(q,required=[5]),error=True);add('no_archive',q=dict(q,archive=False),archived=True,error=True);add('no_irrelevant',q=dict(q,input='unrelated',required=[],count=2));add('zero',q=dict(q,required=[],count=0));add('duplicate',q=dict(q,required=[2,2]),error=True);add('foreign_subject',q=dict(q,subjects=['outsider']),error=True)
 plural=copy.deepcopy(es);plural[0]['text']='The player said: I prefer copper wicks.';plural[0]['source'].update(speaker='player',text=plural[0]['text'])
 out.append(('optional_plural_after_eviction',plural,dict(q,input='Which wick would suit me?',required=[],count=2),True,False))
 return out
def request(es,q,archived):
 b=base();b['identity']=dict(campaign='deep',branch='save_a',instance='merchant');b['operations']=[dict(op='experience',args=e) for e in es]
 if archived:b['operations'] += [dict(op='consolidate',args=dict(start=1,end=5)),dict(op='ack',args=dict(through=5)),dict(op='evict',args=dict(through=5))]
 b['operations'].append(dict(op='recall_selected',args=q));return dict(kind='episodes',args=b)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,es,q,archived,error in cases():
  try:wanted=select([] if archived else es,[dict(events=es)] if archived else [],q);assert not error,name
  except (AssertionError,KeyError):
   assert error,name;wanted=None
  req=request(es,q,archived)
  if not a.prototype_only:
   cp=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),text=True,capture_output=True);assert cp.returncode==0,(name,cp.stderr);actual=json.loads(cp.stdout)['results'][-1];assert actual['ok']!=error,(name,actual)
   if not error:assert enc(actual['result'])==enc(wanted),(name,actual,wanted)
  if a.write_conformance:
   assert not a.prototype_only
   # Every earlier result is independently established by the frozen episode oracle.
   from episodic_cases import oracle
   old=copy.deepcopy(req['args']);old['operations']=old['operations'][:-1];prior=oracle(old)['results'];expected=dict(results=prior+[dict(ok=False) if error else dict(ok=True,result=wanted)])
   assert enc(json.loads(cp.stdout))==enc(expected),name
   d=ROOT/'conformance/deep-memory'/name;d.mkdir(parents=True,exist_ok=True)
   for f,v in [('case',dict(op='local')),('input',req),('expected',expected)]: (d/(f+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
