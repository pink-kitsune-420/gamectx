"""Independent stage 4 bounded initiative oracle; deterministic host work only."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from life_cases import base as life_base,event
from life_reference import run as life_oracle
ROOT=pathlib.Path(__file__).resolve().parents[2]
def base():
 b=dict(defaults=dict(enabled=True,cooldown=3,margin=2,social_limit=2,attention=4,model_calls=1,replans=2),overrides={},frames=[dict(revision=1,now=1,turn=1,conditions=['stock'],allowed=['polish','greet'],resources=['bench'],audience=['merchant','player'],emergency=False,depth=0,social_remaining=2),dict(revision=2,now=2,turn=1,conditions=['stock'],allowed=['polish','greet'],resources=['bench'],audience=['merchant','player'],emergency=False,depth=0,social_remaining=2),dict(revision=3,now=5,turn=2,conditions=['stock'],allowed=['polish','greet'],resources=['bench'],audience=['merchant','player'],emergency=False,depth=0,social_remaining=2)],candidates=[dict(id='polish',kind='activity',priority=10,reason='Resume the unfinished lamps.',requires=['stock'],resource='bench',recipient=None,attitudes=[],events=[],promises=[]),dict(id='greet',kind='ask',priority=20,reason='A customer arrived.',requires=[],resource=None,recipient='player',attitudes=[],events=[],promises=[])])
 b['life']=life_base();b['life']['scope']='counter';b['life']['operations']=b['life']['operations'][:2];b.update(promises=[],reserves=[0,0,0],receipts=[None,None,None]);return b
def oracle(a):
 cfg=dict(enabled=False,cooldown=3,margin=2,social_limit=2,attention=4,model_calls=0,replans=2)
 for c in (a['defaults'],a['overrides']):assert set(c)<=set(cfg);cfg.update(c)
 assert 0<=cfg['model_calls']<=8 and 1<=cfg['attention']<=16
 l=copy.deepcopy(a['life']);l['operations'].append(dict(op='inspect',args={}));state=life_oracle(l)['results'][-1]['result'];life_rev=state['revision'];events=state['events'];attrs=state['attitudes']
 active=[dict(id=i+1,promisor=p['promisor'],beneficiary=p['beneficiary'],revision=1) for i,p in enumerate(a['promises']) if p['state']=='accepted']
 out=[];last=None;chosen=None;seq=0;turn=None;social=0;replans=0;calls=0;seen=[];failures={};water=state['watermark']
 for fi,f in enumerate(a['frames']):
  if turn!=f['turn']:turn=f['turn'];social=0;replans=0;calls=0
  if not cfg['enabled']:r=dict(status='quiet',reason='disabled')
  elif last is not None and f['now']<last+cfg['cooldown'] and not f['emergency']:r=dict(status='quiet',reason='cooldown')
  elif replans>=cfg['replans']:r=dict(status='quiet',reason='replan_limit')
  else:
   available=[c for c in a['candidates'] if c['id'] in f['allowed'] and set(c['requires'])<=set(f['conditions']) and (c['resource'] is None or c['resource'] in f['resources']) and (c['recipient'] is None or c['recipient'] in f['audience'] and f['depth']==0 and min(cfg['social_limit']-social,f['social_remaining'])>0 and (c['id'],c['recipient'],c['events']) not in seen) and all(any(e['token']==n for e in events) for n in c['events']) and all(any(p['id']==n and p['promisor']==a['life']['actor'] for p in active) for n in c['promises']) and not(c['id'] in failures and water<=failures[c['id']]['event'] and (f['now']<failures[c['id']]['until'] or failures[c['id']]['count']>=3))]
   def score(c):return c['priority']+sum(int(next(x for x in attrs if x['definition']['id']==n)['level']/10) for n in c['attitudes'])+20*len(c['promises'])
   available.sort(key=lambda c:(-score(c),c['id']));available=available[:cfg['attention']]
   if not available:r=dict(status='quiet',reason='no_usable_candidate')
   else:
    best=available[0];old=next((c for c in available if c['id']==chosen),None)
    if old and score(old)+cfg['margin']>=score(best):best=old
    if best['kind']=='silence':r=dict(status='quiet',reason=best['reason'])
    else:
     seq+=1;replans+=1;last=f['now'];chosen=best['id'];social+=best['recipient'] is not None
     if best['recipient'] is not None:seen.append((best['id'],best['recipient'],copy.deepcopy(best['events'])))
     r=dict(status='proposed',id=seq,revision=f['revision'],candidate=copy.deepcopy(best),score=score(best),life_revision=life_rev,promises=[p for p in active if p['id'] in best['promises']])
  reserved=[]
  for _ in range(a['reserves'][fi]):
   ok=cfg['enabled'] and calls<cfg['model_calls'];reserved.append(ok)
   if ok:calls+=1
  receipt=a['receipts'][fi]
  if receipt is not None and r['status']=='proposed':
   # Fixture host executes and publishes an outcome; initiative itself never creates it.
   water+=1;life_rev+=1;events.append(event(water,f['now'],kind='outcome',tag=r['candidate']['id'],subject=a['life']['actor'],source=dict(id='host:'+str(water),speaker=a['life']['actor'],text='Host action succeeded.' if receipt else 'Host action failed.')))
   if receipt:failures.pop(r['candidate']['id'],None)
   else:failures[r['candidate']['id']]=dict(count=min(3,failures.get(r['candidate']['id'],{}).get('count',0)+1),until=f['now']+cfg['cooldown'],event=water)
  out.append(dict(decision=r,reservations=reserved,model_calls=calls,life_revision=life_rev,events=len(events)))
 return dict(results=out)
def cases():
 out=[('cooldown',base())]
 for i in range(64):
  b=base();b['candidates'][0]['priority']=i-20
  if i%4==0:b['frames'][0]['conditions']=[]
  if i%4==1:b['frames'][0]['resources']=[]
  if i%4==2:b['frames'][0]['depth']=1
  if i%4==3:b['frames'][0]['social_remaining']=0
  out.append(('many_'+str(i),b))
 for name,fn in [('disabled',lambda b:b.update(defaults={})),('emergency',lambda b:b['frames'][1].update(emergency=True)),('quiet',lambda b:b['candidates'][1].update(kind='silence',reason='Let the customer browse.')),('no_actions',lambda b:[f.update(allowed=[]) for f in b['frames']])]:
  b=base();fn(b);out.append((name,b))
 for name in ['attitude_priority','commitment_priority','missing_event','accepted_promise','fulfilled_promise','reported_promise','zero_model_calls','whole_turn_quota','failed_recovery','circular_chatter','duplicate_greeting']:
  b=base()
  if name=='attitude_priority':
   b['life']['definition']['attitudes'][0]['level']=100;b['candidates'][0].update(priority=15,attitudes=['reliability'],events=[1])
  if name=='commitment_priority':
   att=copy.deepcopy(b['life']['definition']['attitudes'][0]);att.update(id='repair_commitment',kind='commitment',level=90);b['life']['definition']['attitudes'].append(att);b['candidates'][0].update(priority=15,attitudes=['repair_commitment'])
  if name=='missing_event':b['candidates'][1]['events']=[999]
  if name in ('accepted_promise','fulfilled_promise','reported_promise'):
   b['promises']=[dict(promisor='merchant',beneficiary='player',task='Repair the lantern.',source=dict(id='promise:1',speaker='merchant',text='I will repair the lantern.'),state=name.split('_')[0])];b['candidates'][0]['promises']=[1]
  if name=='zero_model_calls':b['defaults']['model_calls']=0;b['reserves']=[1,1,1]
  if name=='whole_turn_quota':b['reserves']=[1,2,2]
  if name=='failed_recovery':b['candidates']=b['candidates'][:1];b['receipts']=[False,None,True]
  if name=='circular_chatter':
   for f in b['frames']:f['depth']=1
  out.append((name,b))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v in cases():
  wanted=oracle(v)
  first=wanted['results'][0]['decision']
  if name in ('attitude_priority','commitment_priority','accepted_promise','missing_event'):assert first['candidate']['id']=='polish'
  if name in ('reported_promise','fulfilled_promise'):assert first['candidate']['id']=='greet'
  if name=='whole_turn_quota':assert [r['reservations'] for r in wanted['results']]==[[True],[False,False],[True,False]]
  if name=='zero_model_calls':assert all(r['model_calls']==0 for r in wanted['results'])
  if name=='duplicate_greeting':assert wanted['results'][-1]['decision']['candidate']['id']=='polish'
  if name=='failed_recovery':assert [r['decision']['status'] for r in wanted['results']]==['proposed','quiet','proposed']
  request=dict(kind='initiative',args=v)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(request),capture_output=True,text=True);assert r.returncode==0,(name,r.stderr);assert r.stdout.strip()==json.dumps(wanted,ensure_ascii=False,separators=(',',':')),(name,r.stdout,wanted)
  if a.write_conformance and not name.startswith('many_'):
   d=ROOT/'conformance/initiative'/name;d.mkdir(parents=True,exist_ok=True)
   for f,o in [('case',dict(op='local')),('input',request),('expected',wanted)]: (d/(f+'.json')).write_text(json.dumps(o,ensure_ascii=False,separators=(',',':'))+'\n')
 print(json.dumps(dict(workflows=len(cases()),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
