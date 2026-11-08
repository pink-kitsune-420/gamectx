"""Independent opt-in loadout/floor contract. Original host-only prototype; no network."""
import copy,json
def encode(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def fields(v,keys):assert isinstance(v,dict) and set(v)==set(keys.split())
def integer(v,lo,hi):assert type(v) is int and lo<=v<=hi;return v
def name(v):assert isinstance(v,str) and 0<len(v)<=32 and v[0].isascii() and v[0].islower() and all(c in 'abcdefghijklmnopqrstuvwxyz0123456789_' for c in v);return v
def names(v,cap=16):assert isinstance(v,list) and len(v)<=cap and len(set(v))==len(v);return [name(x) for x in v]
def scope(v):assert isinstance(v,str) and 0<len(v)<=64 and len(v.encode())<=256 and not any(ord(c)<32 or 127<=ord(c)<160 for c in v);return v
def line(v):assert isinstance(v,str) and 0<len(v)<=512 and len(v.encode())<=2048 and not any(ord(c)<32 or 127<=ord(c)<160 for c in v);return v
MODULES='routine brain life stock lore evidence recall promises continuity initiative'.split()
PRESETS={'ambient':'routine brain initiative','service':'routine life stock lore','quest':'routine life lore evidence recall promises','companion':'routine life lore evidence recall promises continuity initiative'}
SIZES=['0b','1b','1.5b','3b','8b','30b','70b']
def blueprint(a):
 fields(a,'version preset size modules policies tools model_calls');assert a['version']==1 and a['preset'] in PRESETS and a['size'] in SIZES
 m={k:k in PRESETS[a['preset']].split() for k in MODULES};assert set(a['modules'])<=set(m)
 for k,v in a['modules'].items():assert type(v) is bool;m[k]=v
 fields(a['policies'],'balanced deception mistaken');assert all(type(x) is bool for x in a['policies'].values())
 tools=names(a['tools'],2);assert set(tools)<={'life_recall','life_change'};calls=integer(a['model_calls'],0,8)
 assert not any(m[k] for k in ('recall','promises','continuity','evidence')) or m['life']
 assert not tools or m['life'];assert 'life_recall' not in tools or m['recall']
 assert not any(a['policies'].values()) or m['life']
 assert 'life_change' not in tools or m['continuity']
 assert a['size']!='0b' or calls==0 and not tools and not any(a['policies'].values())
 i=SIZES.index(a['size']);expanded=i>=4
 assert not (a['policies']['balanced'] or a['policies']['deception'] or 'life_change' in tools) or expanded
 recall=[0,1,1,2,3,4,6][i] if m['recall'] else 0;exchanges=0 if i==0 else [0,1,1,1,2,3,3][i]
 caps=dict(facts=0 if i==0 else 16 if expanded else 4,facets=0 if i==0 or not m['life'] else 8 if expanded else 2,attitudes=0 if i==0 or not m['life'] else 8 if expanded else 2,episodes=recall,exchanges=exchanges)
 return dict(version=1,preset=a['preset'],size=a['size'],modules=m,policies=copy.deepcopy(a['policies']),tools=tools,model_calls=calls,attention='expanded' if expanded else 'focused',caps=caps,life_config=dict(enabled=m['life'],**a['policies'],tools=tools),routine_config=dict(routines=m['routine'],model_calls=0),initiative_config=dict(enabled=m['initiative'],model_calls=0))
def preset(p):return dict(version=1,preset=p,size={'ambient':'0b','service':'3b','quest':'3b','companion':'8b'}[p],modules={},policies=dict(balanced=False,deception=False,mistaken=False),tools=[],model_calls=0 if p=='ambient' else 2 if p=='companion' else 1)
def gate_select(a):
 fields(a,'rules actor speaker listeners participants revision expected_revision attitudes kind items');assert a['revision']==a['expected_revision'];people=names(a['participants']);assert a['actor'] in people and a['speaker'] in people and a['listeners'] and set(names(a['listeners']))<=set(people)
 assert a['kind'] in ('lore','facet','affordance') and len(a['items'])<=512 and len(a['rules'])<=128
 seen=[]
 for r in a['rules']:
  fields(r,'kind id clauses');assert r['kind'] in ('lore','facet','affordance');scope(r['id']);key=(r['kind'],r['id']);assert key not in seen;seen.append(key);assert 0<len(r['clauses'])<=8
  for q in r['clauses']:
   fields(q,'attitude target op value');name(q['attitude']);assert q['target'] in ('speaker','listener') and q['op'] in ('ge','le');integer(q['value'],-100,100)
 out=[];withheld=[];seen=[]
 for item in a['items']:
  scope(item['id']);assert item['id'] not in seen;seen.append(item['id']);reason=None
  for r in a['rules']:
   if r['kind']!=a['kind'] or r['id']!=item['id']:continue
   for q in r['clauses']:
    for subject in [a['speaker']] if q['target']=='speaker' else a['listeners']:
     matches=[x['level'] for x in a['attitudes'] if x['kind']==q['attitude'] and x['subject']==subject]
     if len(matches)!=1:reason='attitude_missing' if not matches else 'attitude_ambiguous';break
     if not (matches[0]>=q['value'] if q['op']=='ge' else matches[0]<=q['value']):reason='attitude_threshold';break
    if reason:break
  if reason:withheld.append(dict(kind=a['kind'],id=item['id'],reason=reason))
  else:out.append(copy.deepcopy(item))
 return dict(packet=out,withheld=withheld,refusal=None if not withheld else 'I cannot share that or offer it yet.')
def route(v):
 fields(v,'name size calls cost_micros');name(v['name']);assert v['size'] in SIZES;integer(v['calls'],0,8);integer(v['cost_micros'],0,10**12)
 assert v['calls']>0 or v['cost_micros']==0;assert v['size']!='0b' or v['calls']==0
 return v
def escalate(policy,signal):
 fields(policy,'enabled significance');assert type(policy['enabled']) is bool;integer(policy['significance'],1,100)
 fields(signal,'significance changes conflicts');integer(signal['significance'],0,100);integer(signal['conflicts'],0,16);assert len(signal['changes'])<=16
 crossing=False
 for q in signal['changes']:
  fields(q,'before after threshold');[integer(q[k],-100,100) for k in q];crossing|=(q['before']<q['threshold']<=q['after'] or q['after']<q['threshold']<=q['before'])
 return 'significance' if policy['enabled'] and signal['significance']>=policy['significance'] else 'attitude_crossing' if policy['enabled'] and crossing else 'conflict' if policy['enabled'] and signal['conflicts']>=2 else 'routine'
class Scene:
 def __init__(self,v):
  fields(v,'version id epoch revision participants');assert v['version']==1;name(v['id']);integer(v['epoch'],0,2**32-1);integer(v['revision'],0,2**32-1);assert 0<len(v['participants'])<=16
  for p in v['participants']:fields(p,'id kind');name(p['id']);assert p['kind'] in ('player','ai')
  names([p['id'] for p in v['participants']]);self.v=copy.deepcopy(v);self.lines=[];self.turn=None;self.social=self.calls=0;self.served={};self.consumed=[];self.grants=[];self.next=0;self.clock=0
 def receipt(self,r):
  fields(r,'token speaker recipients heard text parent');integer(r['token'],1,2**32-1);assert not self.lines or r['token']>self.lines[-1]['token'];assert len(self.lines)<128;people=[p['id'] for p in self.v['participants']];assert r['speaker'] in people;assert r['recipients'] and set(names(r['recipients']))<=set(people) and r['speaker'] not in r['recipients'];assert set(names(r['heard']))<=set(people);line(r['text'])
  if r['parent'] is not None:assert any(x['token']==r['parent'] for x in self.lines)
  self.lines.append(copy.deepcopy(r))
  if r['parent'] is not None and r['heard'] and any(p==dict(id=r['speaker'],kind='ai') for p in self.v['participants']):
   root=r
   while root['parent'] is not None and any(p==dict(id=root['speaker'],kind='ai') for p in self.v['participants']):root=next(x for x in self.lines if x['token']==root['parent'])
   self.served.setdefault(root['token'],{root['speaker']}).add(r['speaker'])
 def render(self,actor,limit):
  assert any(p==dict(id=actor,kind='ai') for p in self.v['participants']);integer(limit,0,16)
  rows=[r for r in self.lines if actor in r['heard']];rows=rows[-limit:] if limit else []
  return [dict(role='assistant' if r['speaker']==actor else 'user',content='['+r['speaker']+' -> '+','.join(r['recipients'])+'] '+r['text']) for r in rows]
 def floor(self,a,ledger):
  fields(a,'turn now trigger policy spotlight candidates');integer(a['turn'],0,2**32-1);integer(a['now'],self.clock,2**32-1);assert self.turn is None or a['turn']>=self.turn
  p=a['policy'];fields(p,'enabled social_limit model_calls max_depth');assert type(p['enabled']) is bool;integer(p['social_limit'],0,16);integer(p['model_calls'],0,8);integer(p['max_depth'],0,4)
  trigger=next(r for r in self.lines if r['token']==a['trigger']);root=trigger;depth=0
  while root['parent'] is not None and any(p==dict(id=root['speaker'],kind='ai') for p in self.v['participants']):root=next(r for r in self.lines if r['token']==root['parent']);depth+=1
  assert len(a['candidates'])<=16;seen=[]
  for c in a['candidates']:
   fields(c,'actor priority focused expanded signal');assert c['actor'] not in seen;seen.append(c['actor']);assert any(x==dict(id=c['actor'],kind='ai') for x in self.v['participants']);integer(c['priority'],-100,100);f=route(c['focused']);assert f['size'] in SIZES[:4];escalate(a['spotlight'],c['signal'])
   if c['expanded'] is not None:assert route(c['expanded'])['size'] in SIZES[4:]
  if self.turn!=a['turn']:self.turn=a['turn'];self.calls=self.social=0
  self.clock=a['now'];selected=[];omitted=[]
  if not p['enabled']:return dict(selected=[],omitted=[],reason='disabled',model_calls=self.calls,social=self.social)
  if a['trigger'] in self.consumed:return dict(selected=[],omitted=[],reason='replayed_trigger',model_calls=self.calls,social=self.social)
  if depth>p['max_depth']:return dict(selected=[],omitted=[],reason='depth',model_calls=self.calls,social=self.social)
  self.consumed.append(a['trigger']);served=self.served.setdefault(root['token'],set());served.add(root['speaker'])
  for _,c in sorted(enumerate(a['candidates']),key=lambda x:(-x[1]['priority'],x[0])):
   actor=c['actor'];reason=None
   if actor not in trigger['heard'] or actor not in trigger['recipients']:reason='not_addressed_or_heard'
   elif actor==trigger['speaker'] or actor in served:reason='circular'
   elif self.social>=p['social_limit']:reason='social_limit'
   elif len(self.grants)>=128 or len([g for g in self.grants if g['status']=='reserved'])>=16:reason='pending_limit'
   choices=[];why=escalate(a['spotlight'],c['signal'])
   if why!='routine' and c['expanded'] is not None:choices.append((c['expanded'],why))
   choices.append((c['focused'],'routine' if why=='routine' else 'focused_fallback'))
   picked=None
   if reason is None:
    for r,why in choices:
     if self.calls+r['calls']>p['model_calls']:reason='model_call_budget';continue
     if r['cost_micros']>ledger['remaining'] or ledger['blocked'] and r['cost_micros']>0:reason='spending_budget';continue
     picked=(r,why);reason=None;break
   if picked:
    r,why=picked;ledger['remaining']-=r['cost_micros'];self.next+=1;reservation=None
    if r['calls']>0:ledger['next']+=1;reservation=ledger['next']
    grant=dict(id=self.next,actor=actor,route=copy.deepcopy(r),reason=why,reservation=reservation,revision=self.v['revision'],epoch=self.v['epoch'],turn=self.turn,trigger=trigger['token']);selected.append(grant);self.grants.append(grant|dict(status='reserved'));self.calls+=r['calls'];self.social+=1;served.add(actor)
   else:omitted.append(dict(actor=actor,reason=reason))
  return dict(selected=selected,omitted=omitted,reason='selected' if selected else 'quiet',model_calls=self.calls,social=self.social)
def workflow(a):
 fields(a,'scene ledger operations');fields(a['ledger'],'limit window');integer(a['ledger']['limit'],0,10**12);integer(a['ledger']['window'],1,1000000);assert len(a['operations'])<=256
 s=Scene(a['scene']);ledger=dict(remaining=a['ledger']['limit'],blocked=False,next=0);results=[];entries=[];clock=0
 for o in a['operations']:
  try:
   fields(o,'op args');v=o['args']
   if o['op']=='receipt':s.receipt(v);result=None
   elif o['op']=='render':fields(v,'actor limit');result=s.render(v['actor'],v['limit'])
   elif o['op']=='floor':
    integer(v['now'],clock,2**32-1)
    active=[e for e in entries if e['state']!='settled' or v['now']-e['at']<a['ledger']['window']];ledger['remaining']=max(0,a['ledger']['limit']-sum(e['amount'] for e in active));before=ledger['next'];result=s.floor(v,ledger)
    for g in result['selected']:
     if g['reservation'] is not None:entries.append(dict(id=g['reservation'],at=v['now'],amount=g['route']['cost_micros'],state='reserved'))
    if ledger['next']!=before:clock=v['now']
   elif o['op'] in ('dispatch','cancel'):
    fields(v,'id now');integer(v['now'],max(s.clock,clock),2**32-1);g=next(g for g in s.grants if g['id']==v['id']);assert g['status']=='reserved'
    if o['op']=='dispatch':assert g['revision']==s.v['revision'] and g['epoch']==s.v['epoch'] and g['turn']==s.turn
    if g['reservation'] is not None:
     e=next(e for e in entries if e['id']==g['reservation']);assert e['state']=='reserved'
     if o['op']=='cancel':entries.remove(e)
     else:e['state']='dispatched'
     clock=v['now']
    s.clock=v['now'];g['status']='dispatched' if o['op']=='dispatch' else 'cancelled';result=dict(actor=g['actor'],route=g['route'],reservation=g['reservation'])
   elif o['op']=='revision':fields(v,'revision');integer(v['revision'],s.v['revision']+1,2**32-1);s.v['revision']=v['revision'];result=None
   else:raise ValueError(o['op'])
   results.append(dict(ok=True,result=copy.deepcopy(result)))
  except (AssertionError,StopIteration,KeyError,ValueError,TypeError):results.append(dict(ok=False))
 return dict(results=results,model_calls=s.calls,social=s.social,ledger=dict(spent=sum(e['amount'] for e in entries if e['state']!='settled' or s.clock-e['at']<a['ledger']['window']),blocked=False,pending=len(entries)))
