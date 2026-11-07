"""Independent capability filter: host levels/resources/reasons, never model authority."""
import argparse,copy,json,pathlib,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[1];enc=lambda v:json.dumps(v,separators=(',',':'))
LEVELS=['trust','affinity','resentment','generosity','prudence','courage']
def name(s):assert isinstance(s,str)and re.fullmatch('[a-zA-Z0-9_:-]{1,64}',s)
def integer(n,lo,hi):assert type(n)is int and lo<=n<=hi
def resource(v):
 if v is None:return
 assert set(v)=={'kind','id','amount'}and v['kind']in ['coins','item'];name(v['id']);integer(v['amount'],1,1000000)
 if v['kind']=='item':assert v['amount']==1

def validate(c,f):
 assert set(c)=={'version','enabled','rules'}and c['version']==1 and type(c['enabled'])is bool and len(c['rules'])<=64
 ids=[]
 for r in c['rules']:
  assert set(r)=={'id','trigger','handler','reason','gates','resource','priority'}
  for k in ['id','trigger','handler']:name(r[k])
  assert r['id']not in ids;ids.append(r['id']);assert r['reason']in ['request','gratitude','emergency','threat'];resource(r['resource']);integer(r['priority'],0,100);assert len(r['gates'])<=6;seen=[]
  for g in r['gates']:
   assert set(g)=={'key','min','max'}and g['key']in LEVELS and g['key']not in seen;seen.append(g['key']);integer(g['min'],-100,100);integer(g['max'],g['min'],100)
 assert set(f)=={'actor','listener','revision','trigger','resource','levels','resources','reasons','threat','handlers','used','social_left'}
 for k in ['actor','listener','trigger']:name(f[k])
 assert f['actor']!=f['listener'];integer(f['revision'],0,4294967295);integer(f['social_left'],0,8);resource(f['resource']);assert set(f['levels'])==set(LEVELS)
 for k,v in f['levels'].items():integer(v,-100 if k in ['trust','affinity','resentment']else 0,100)
 assert len(f['resources'])<=64
 seen=[]
 for v in f['resources']:
  resource(v);key=(v['kind'],v['id']);assert key not in seen;seen.append(key)
 for k in ['handlers','used','reasons']:
  assert len(f[k])<=64 and len(set(f[k]))==len(f[k])
  for v in f[k]:name(v)
 if f['threat']is not None:
  t=f['threat'];assert set(t)=={'event','revision','source','capable','imminent','same_scene'};name(t['event']);integer(t['revision'],0,4294967295);assert t['source']in ['perceived','report','claim']
  for k in ['capable','imminent','same_scene']:assert type(t[k])is bool

def select(c,f):
 validate(c,f);allowed=[];withheld=[]
 for r in c['rules']:
  if r['trigger']!=f['trigger']or r['resource']!=f['resource']:continue
  reason=None
  if not c['enabled']:reason='disabled'
  elif not f['social_left']:reason='social_limit'
  elif r['id']in f['used']:reason='already_used'
  elif r['handler']not in f['handlers']:reason='unregistered_handler'
  elif any(not g['min']<=f['levels'][g['key']]<=g['max']for g in r['gates']):reason='attitude_or_personality'
  elif r['reason']not in f['reasons']:reason='no_host_reason'
  elif r['reason']=='threat'and not(f['threat']and f['threat']['revision']==f['revision']and f['threat']['source']=='perceived'and all(f['threat'][k]for k in ['capable','imminent','same_scene'])):reason='unconfirmed_threat'
  elif r['resource']is not None and not any(v['kind']==r['resource']['kind']and v['id']==r['resource']['id']and v['amount']>=r['resource']['amount']for v in f['resources']):reason='unavailable_resource'
  if reason:withheld.append(dict(id=r['id'],reason=reason))
  else:allowed.append(dict(id=r['id'],handler=r['handler'],resource=r['resource'],reason=r['reason'],revision=f['revision'],priority=r['priority']))
 allowed.sort(key=lambda r:-r['priority'])
 return dict(allowed=allowed,withheld=withheld)
def oracle(v):
 assert set(v)=={'policy','frame','proposal'};out=select(v['policy'],v['frame']);p=v['proposal']
 if p is not None:
  assert set(p)=={'id','revision'};name(p['id']);integer(p['revision'],0,4294967295);assert p['revision']==v['frame']['revision'];chosen=next(r for r in out['allowed']if r['id']==p['id']);out['authorized']=chosen
 return out
def base():
 r=dict(id='helpful_gift',trigger='gift',handler='transfer',reason='request',gates=[dict(key='trust',min=25,max=100),dict(key='generosity',min=40,max=100)],resource=dict(kind='coins',id='coin',amount=2),priority=50)
 f=dict(actor='merchant',listener='player_1',revision=4,trigger='gift',resource=r['resource'],levels=dict(trust=30,affinity=10,resentment=0,generosity=50,prudence=60,courage=40),resources=[dict(kind='coins',id='coin',amount=5)],reasons=['request'],threat=None,handlers=['transfer'],used=[],social_left=1)
 return dict(policy=dict(version=1,enabled=True,rules=[r]),frame=f,proposal=None)
def cases():
 out=[]
 def add(name,change,error=False):v=base();change(v);out.append((name,v,error))
 add('eligible',lambda v:None);add('low_trust',lambda v:v['frame']['levels'].update(trust=24));add('ungenerous',lambda v:v['frame']['levels'].update(generosity=10));add('empty_wallet',lambda v:v['frame'].update(resources=[]));add('no_reason',lambda v:v['frame'].update(reasons=[]));add('used',lambda v:v['frame'].update(used=['helpful_gift']));add('social_limit',lambda v:v['frame'].update(social_left=0));add('unregistered',lambda v:v['frame'].update(handlers=[]));add('disabled',lambda v:v['policy'].update(enabled=False));add('authorize',lambda v:v.update(proposal=dict(id='helpful_gift',revision=4)));add('stale',lambda v:v.update(proposal=dict(id='helpful_gift',revision=3)),True);add('forbidden_proposal',lambda v:(v['frame']['levels'].update(trust=0),v.update(proposal=dict(id='helpful_gift',revision=4))),True)
 for source in ['claim','report','perceived']:
  def change(v,source=source):v['policy']['rules'][0]['reason']='threat';v['frame'].update(reasons=['threat'],threat=dict(event='danger_1',revision=4,source=source,capable=True,imminent=True,same_scene=True))
  add('threat_'+source,change)
 for field in ['capable','imminent','same_scene','revision']:
  def change(v,field=field):v['policy']['rules'][0]['reason']='threat';v['frame'].update(reasons=['threat'],threat=dict(event='danger_1',revision=4,source='perceived',capable=True,imminent=True,same_scene=True));v['frame']['threat'][field]=3 if field=='revision'else False
  add('threat_'+field,change)
 add('generic_help',lambda v:(v['policy']['rules'][0].update(resource=None,handler='call_aid'),v['frame'].update(resource=None,handlers=['call_aid'])));add('duplicate_rule',lambda v:v['policy']['rules'].append(copy.deepcopy(v['policy']['rules'][0])),True);add('bad_level',lambda v:v['frame']['levels'].update(trust=101),True)
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:wanted=oracle(args);assert not error
  except(AssertionError,KeyError,TypeError,StopIteration):assert error;wanted=None
  req=dict(kind='studio-reactions',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(wanted),(name,r.stdout,wanted)
  if a.write_conformance:
   d=ROOT/'conformance/reactions'/name;d.mkdir(parents=True,exist_ok=True)
   for k,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',wanted)]):(d/(k+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
