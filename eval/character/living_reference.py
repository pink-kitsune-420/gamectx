"""Original independent cast/social/counter/ticket rules. No provider or model calls."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
from archetype_reference import fields,integer,name,names,line,scope,blueprint,encode
from life_reference import run as life_run
TAGS=['calm','busy','tense','emergency']
def pack(v):
 fields(v,'version id revision setting players locations groups characters lore features propagation ripple habits');assert v['version']==1;name(v['id']);integer(v['revision'],1,2**32-1)
 s=v['setting'];fields(s,'id calendar entities history');entities=names(s['entities'],128);players=names(v['players'],8);assert players and len(v['characters'])<=12 and v['characters'];ids=[]
 for c in v['characters']:fields(c,'id blueprint definition routine gates affordances home fallbacks');name(c['id']);assert c['id'] not in ids;ids.append(c['id'])
 people=players+ids;assert len(set(people))==len(people)<=16 and set(people)<=set(entities)
 places=v['locations']['places'];locs=[p['id'] for p in places];assert locs and len(set(locs))==len(locs)<=32 and set(locs)<=set(entities)
 groupids=[]
 for g in v['groups']:
  fields(g,'id members');name(g['id']);assert g['id'] in entities and g['id'] not in groupids;groupids.append(g['id']);assert g['members'] and set(names(g['members']))<=set(people)
 assert len(groupids)<=16 and len(set(people+locs+groupids))==len(people+locs+groupids)
 loreids=[e['id'] for e in v['lore']['entries']]
 from lore_reference import Lore
 Lore(v['lore'])
 for e in v['lore']['entries']:assert e['subject'] in entities and (e['known_by']==['*'] or set(e['known_by'])<=set(people))
 history=[h['id'] for h in s['history']];assert len(set(history))==len(history)
 from archetype_reference import gate_select
 for c in v['characters']:
  b=blueprint(c['blueprint']);d=c['definition'];assert c['home'] in locs and d['setting']==s['id'] and d['calendar']==s['calendar'] and set(d['history'])<=set(history)
  assert len({a['id'] for a in d['attitudes']})==len(d['attitudes'])
  for a in d['attitudes']:
   assert a['subject'] in entities and set(a['audience'])<=set(people)
   if a['kind'] in ['trust','resentment','affection','familiarity','obligation','disagreement']:assert a['subject'] in people
  for f in d['facets']:assert set(f['audience'])<=set(people)
  assert b['modules']['routine']==(c['routine'] is not None)
  if c['routine'] is not None:
   assert c['routine']['calendar']==s['calendar']
   for a in c['routine']['activities']:assert a['location'] in locs
  fields(c['fallbacks'],'calm busy tense emergency unknown')
  from life_speech_reference import assess_sentence
  for x in c['fallbacks'].values():line(x);assert len(x)<=240 and assess_sentence(x,240)['boundary_complete']
  pools=dict(lore=loreids,facet=[f['id'] for f in d['facets']],affordance=[a['id'] for a in c['affordances']])
  for g in c['gates']:
   assert g['id'] in pools[g['kind']]
   for q in g['clauses']:
    matches=[a for a in d['attitudes'] if a['kind']==q['attitude']];assert matches and len({a['subject'] for a in matches})==len(matches)
  gate_select(dict(rules=c['gates'],actor=c['id'],speaker=players[0],listeners=[players[0]],participants=people,revision=0,expected_revision=0,attitudes=[],kind='facet',items=[]))
 fields(v['features'],'rumors ripples habits atmosphere');assert all(type(b) is bool for b in v['features'].values());assert not v['features']['ripples'] or v['features']['rumors']
 p=v['propagation'];fields(p,'significance trust max_hops per_tick checks');integer(p['checks'],0,1024);integer(p['significance'],0,100);integer(p['trust'],-100,100);integer(p['max_hops'],0,4);integer(p['per_tick'],0,16)
 r=v['ripple'];fields(r,'numerator denominator max_delta kind groups reason');integer(r['denominator'],1,100);integer(r['numerator'],0,r['denominator']);integer(r['max_delta'],0,5);name(r['kind']);assert set(names(r['groups']))<=set(groupids);line(r['reason'])
 assert not v['features']['ripples'] or r['groups'] and any(a['kind']==r['kind'] for c in v['characters'] for a in c['definition']['attitudes'])
 assert len(v['habits'])<=16 and len({h['id'] for h in v['habits']})==len(v['habits'])
 for h in v['habits']:fields(h,'id counter threshold text tags');name(h['id']);assert h['counter'] in ['fulfilled','broken','interrupted'];integer(h['threshold'],1,1000000);line(h['text']);names(h['tags'],8)
 return dict(version=1,id=v['id'],revision=v['revision'],participants=people,characters=[dict(id=c['id'],home=c['home'],blueprint=blueprint(c['blueprint']),groups=[g['id'] for g in v['groups'] if c['id'] in g['members']]) for c in v['characters']],locations=locs)
class World:
 def __init__(self,p,scope_id):
  self.p=copy.deepcopy(p);self.summary=pack(p);name(scope_id);self.scope=scope_id;self.clock=0;self.last=None;self.revision=0;self.roots=[];self.known=[];self.influences=[];self.counters={};self.counter_water=0;self.last_counter=None;self.atmospheres={};self.cursor=0;self.checked=0;self.operations={c['id']:[] for c in p['characters']}
  self.social={c['id']:[a['level'] for a in c['definition']['attitudes']] for c in p['characters']}
 def character(self,actor):return next(c for c in self.p['characters'] if c['id']==actor)
 def state(self,actor):
  c=self.character(actor);v=dict(setting=self.p['setting'],definition=c['definition'],defaults=blueprint(c['blueprint'])['life_config'],overrides={},scope=self.scope,actor=actor,participants=self.summary['participants'],operations=self.operations[actor]+[dict(op='inspect',args={})]);return life_run(v)['results'][-1]['result']
 def levels(self,actor):
  attrs=copy.deepcopy(self.state(actor)['attitudes'])
  for attr,level in zip(attrs,self.social[actor]):attr['level']=level
  return attrs
 def observe(self,a):
  fields(a,'id actor event significance share_with appraise');name(a['id']);assert not any(r['id']==a['id'] for r in self.roots) and len(self.roots)<64;c=self.character(a['actor']);assert blueprint(c['blueprint'])['modules']['life'];integer(a['significance'],0,100);assert set(names(a['share_with']))<=set(self.operations)
  e=a['event'];assert e['kind'] in ['direct','outcome'] and e['at']>=self.clock and e['audience']==self.summary['participants'] and e['subject'] in self.p['setting']['entities'];before=len(self.operations[a['actor']]);self.operations[a['actor']].append(dict(op='experience',args=copy.deepcopy(e)));change=None;level_update=None
  try:
   state=self.state(a['actor']);assert state['watermark']==e['token']
   if a['appraise'] is not None:
    rule=next(r for r in c['definition']['rules'] if r['id']==a['appraise']);attr=next(x for x in state['attitudes'] if x['definition']['id']==rule['attitude']);assert attr['definition']['subject']==e['subject'];old=attr['level'];self.operations[a['actor']].append(dict(op='appraise',args=dict(rule=rule['id'],event=e['token'],revision=state['revision'])));updated=self.state(a['actor']);assert updated['revision']==state['revision']+1;idx=next(i for i,x in enumerate(state['attitudes']) if x['definition']['id']==rule['attitude']);old_effective=self.social[a['actor']][idx];after=max(-100,min(100,old_effective+rule['delta']));after=0 if old_effective*after<0 else after;level_update=(idx,after);change=dict(kind=attr['definition']['kind'],delta=after-old_effective)
  except Exception:self.operations[a['actor']]=self.operations[a['actor']][:before];raise
  if level_update is not None:self.social[a['actor']][level_update[0]]=level_update[1]
  self.roots.append(copy.deepcopy(a)|dict(change=change));self.known.append(dict(root=a['id'],actor=a['actor'],path=[a['actor']],event=e['token']));self.clock=e['at'];self.revision+=1;return dict(root=a['id'],revision=self.revision)
 def tick(self,a):
  fields(a,'now windows routines');integer(a['now'],self.clock,2**32-1);assert self.last is None or a['now']>self.last;assert len(a['windows'])<=16 and len(a['routines'])<=12
  locs=self.summary['locations'];seen=[]
  for w in a['windows']:
   fields(w,'id location start end participants kind');name(w['id']);assert w['id'] not in seen;seen.append(w['id']);assert w['location'] in locs and w['kind'] in ['scene','routine'];integer(w['start'],0,2**32-1);integer(w['end'],w['start']+1,2**32-1);assert set(names(w['participants']))<=set(self.operations)
  routine_results=[]
  for x in a['routines']:
   fields(x,'actor frame');self.character(x['actor']);f=x['frame'];fields(f,'revision now conditions allowed emergency');assert f['now']==a['now'];routine_results.append(dict(actor=x['actor'],ok=True))
  sent=[];blocked=[];snapshot=copy.deepcopy(self.known);p=self.p['propagation'];ripple=self.p['ripple']
  self.checked=0
  if self.p['features']['rumors']:
   for w,root,k,receiver in self.pairs(a,snapshot,sent):
    if len(sent)>=p['per_tick'] or receiver not in root['share_with'] or any(x['root']==root['id'] and x['actor']==receiver for x in self.known):continue
    if not blueprint(self.character(receiver)['blueprint'])['modules']['life']:continue
    trust=[x['level'] for x in self.levels(receiver) if x['definition']['kind']=='trust' and x['definition']['subject']==k['actor']]
    if len(trust)!=1 or trust[0]<p['trust']:continue
    state=self.state(receiver);event=dict(token=state['watermark']+1,at=a['now'],kind='report',tag=root['event']['tag'],subject=root['event']['subject'],source=dict(id='rumor:'+root['id'],speaker=k['actor'],text=root['event']['text']),audience=self.summary['participants'],text=root['event']['text'],causes=[],retain=True)
    if len(state['events'])>=128:blocked.append(dict(root=root['id'],actor=receiver,reason='backpressure'));continue
    self.operations[receiver].append(dict(op='experience',args=event));path=k['path']+[receiver];known=dict(root=root['id'],actor=receiver,path=path,event=event['token']);self.known.append(known);row=dict(root=root['id'],sender=k['actor'],actor=receiver,hops=len(path)-1,path=path,original_source=root['event']['source'],event=event);sent.append(row)
    shared=any(g['id'] in ripple['groups'] and root['actor'] in g['members'] and receiver in g['members'] for g in self.p['groups'])
    if self.p['features']['ripples'] and shared and root['change'] is not None and root['change']['kind']==ripple['kind'] and root['event']['subject'] in self.p['players']:
     matches=[x for x in state['attitudes'] if x['definition']['kind']==ripple['kind'] and x['definition']['subject']==root['event']['subject']]
     if len(matches)==1:
      delta=int(root['change']['delta']*ripple['numerator']/ripple['denominator']);delta=max(-ripple['max_delta'],min(ripple['max_delta'],delta))
      idx=next(i for i,x in enumerate(state['attitudes']) if x['definition']['id']==matches[0]['definition']['id']);old=self.social[receiver][idx];after=max(-100,min(100,old+delta));after=0 if old*after<0 else after;delta=after-old
      if delta and len(self.influences)<128:self.social[receiver][idx]=after;self.influences.append(dict(root=root['id'],actor=receiver,attitude=matches[0]['definition']['id'],subject=root['event']['subject'],delta=delta,reason=ripple['reason'],event=event['token']))
  self.clock=a['now'];self.last=a['now']
  if sent:self.revision+=1
  return dict(routines=routine_results,reports=sent,blocked=blocked,checked_pairs=self.checked,revision=self.revision,model_calls=0)
 def pairs(self,a,snapshot,sent):
  ids=list(self.operations);n=len(ids);total=len(a['windows'])*len(self.roots)*n*n
  if not total:return
  for _ in range(min(total,self.p['propagation']['checks'])):
   if len(sent)>=self.p['propagation']['per_tick']:return
   x=self.cursor%total;self.cursor=(x+1)%total;self.checked+=1
   receiver=ids[x%n];x//=n;sender=ids[x%n];x//=n;root=self.roots[x%len(self.roots)];w=a['windows'][x//len(self.roots)]
   if not w['start']<=a['now']<w['end'] or sender not in w['participants'] or receiver not in w['participants'] or root['significance']<self.p['propagation']['significance']:continue
   k=next((k for k in snapshot if k['root']==root['id'] and k['actor']==sender),None)
   if k is None or len(k['path'])-1>=self.p['propagation']['max_hops']:continue
   yield w,root,k,receiver
 def counters_update(self,v):
  fields(v,'token player at fulfilled broken interrupted source');integer(v['token'],1,2**32-1);integer(v['at'],0,2**32-1);[integer(v[k],0,1000000) for k in ['fulfilled','broken','interrupted']]
  if v==self.last_counter:return dict(duplicate=True,revision=self.revision)
  assert v['token']>self.counter_water and v['player'] in self.p['players'] and v['at']>=self.clock
  old=self.counters.get(v['player'],dict(fulfilled=0,broken=0,interrupted=0));changed=False
  for k in ['fulfilled','broken','interrupted']:integer(v[k],old[k],1000000);changed|=v[k]!=old[k]
  assert changed;self.counters[v['player']]=copy.deepcopy(v);self.counter_water=v['token'];self.last_counter=copy.deepcopy(v);self.clock=v['at'];self.revision+=1;return dict(duplicate=False,revision=self.revision)
 def atmosphere(self,v):
  fields(v,'location revision tag source');assert v['location'] in self.summary['locations'] and v['tag'] in TAGS;integer(v['revision'],1,2**32-1);assert v['revision']>self.atmospheres.get(v['location'],dict(revision=0))['revision'];self.atmospheres[v['location']]=copy.deepcopy(v);self.revision+=1;return dict(revision=self.revision)
 def context(self,a):
  fields(a,'actor players tags location limit');self.character(a['actor']);assert set(names(a['players']))<=set(self.p['players']) and a['location'] in self.summary['locations'];names(a['tags'],8);integer(a['limit'],0,4);limit=a['limit'];badges=[]
  if self.p['features']['habits']:
   for player in a['players']:
    for h in self.p['habits']:
     counter=self.counters.get(player,{})
     if len(badges)<limit and counter.get(h['counter'],0)>=h['threshold'] and (not a['tags'] or set(a['tags'])&set(h['tags'])):badges.append(dict(player=player,id=h['id'],text=h['text'],counter=h['counter'],count=counter[h['counter']],source=counter['source']))
  atmosphere=None
  if self.p['features']['atmosphere']:
   v=self.atmospheres.get(a['location']);atmosphere=dict(location=a['location'],tag='calm' if v is None else v['tag'],urgency=0 if v is None else TAGS.index(v['tag']),source=None if v is None else v['source'])
  rumors=[]
  for k in self.known:
   if len(rumors)>=limit:break
   root=next(r for r in self.roots if r['id']==k['root'])
   if k['actor']==a['actor'] and len(k['path'])>1 and root['event']['subject'] in a['players'] and (not a['tags'] or root['event']['tag'] in a['tags']):rumors.append(dict(root=root['id'],path=k['path'],hops=len(k['path'])-1,original_source=root['event']['source']))
  influences=[i for i in self.influences if i['actor']==a['actor'] and i['subject'] in a['players']][:limit]
  return dict(badges=badges,atmosphere=atmosphere,rumors=rumors,group_influences=influences)
def replay(a):
 fields(a,'pack scope operations');w=World(a['pack'],a['scope']);assert len(a['operations'])<=256;results=[]
 for op in a['operations']:
  try:
   fields(op,'op args');v=op['args'];k=op['op']
   if k=='observe':result=w.observe(v)
   elif k=='tick':result=w.tick(v)
   elif k=='counters':result=w.counters_update(v)
   elif k=='atmosphere':result=w.atmosphere(v)
   elif k=='context':result=w.context(v)
   elif k=='initiative':
    fields(v,'actor frame candidates');c=w.character(v['actor']);assert blueprint(c['blueprint'])['modules']['initiative'] and v['frame']['now']==w.clock
    from initiative_cases import oracle
    l=dict(setting=w.p['setting'],definition=c['definition'],defaults=blueprint(c['blueprint'])['life_config'],overrides={},scope=w.scope,actor=c['id'],participants=w.summary['participants'],operations=w.operations[c['id']])
    result=oracle(dict(defaults=dict(enabled=True,model_calls=0),overrides={},life=l,promises=[],frames=[v['frame']],candidates=v['candidates'],reserves=[0],receipts=[None]))['results'][0]['decision']
   elif k=='inspect':fields(v,'actor');result=dict(base=w.state(v['actor']),effective_attitudes=w.levels(v['actor']))
   else:raise ValueError(k)
   results.append(dict(ok=True,result=copy.deepcopy(result)))
  except (AssertionError,ValueError,KeyError,StopIteration,TypeError):results.append(dict(ok=False))
 return w,results
def world_workflow(a):
 w,results=replay(a);return dict(results=results,revision=w.revision,model_calls=0)
def speech_context(v):
 fields(v,'actor world schema permissions facts');scope(v['actor']);scope(v['world']);integer(v['schema'],0,2**32-1);integer(v['permissions'],0,2**32-1);assert v['facts']==[]
def strict_json(s):
 import json
 def unique(pairs):
  result={}
  for k,v in pairs:assert k not in result;result[k]=v
  return result
 return json.loads(s,object_pairs_hook=unique)
def speech_workflow(a):
 fields(a,'operations');assert len(a['operations'])<=256;entries={};next_id=0;clock=0;results=[]
 from life_speech_reference import assess_sentence
 for op in a['operations']:
  try:
   fields(op,'op args');v=op['args'];k=op['op'];integer(v['now'],clock,2**32-1)
   if k=='issue':
    fields(v,'context now ttl fallback');speech_context(v['context']);integer(v['ttl'],1,1000000);assert v['now']+v['ttl']<=2**32-1 and len(entries)<32;line(v['fallback']);assert 0<len(v['fallback'])<=240 and assess_sentence(v['fallback'],240)['boundary_complete'];next_id+=1;entries[next_id]=dict(context=copy.deepcopy(v['context']),expires=v['now']+v['ttl'],line=v['fallback'],stage='pending',completed=False,raw=None);result=dict(id=next_id,stage='pending',line=v['fallback'])
   else:
    e=entries[v['id']]
    if k=='complete':
     fields(v,'id now context response approved');speech_context(v['context']);assert type(v['approved']) is bool;assert e['stage']=='pending' and not e['completed'];assert len(v['response'].encode())<=4096;e['completed']=True;e['raw']=v['response'];fresh=v['context']==e['context'] and v['now']<e['expires'];valid=False
     try:
      import json
      reply=strict_json(v['response']);fields(reply,'speech');line(reply['speech']);valid=0<len(reply['speech'])<=240 and assess_sentence(reply['speech'],240)['boundary_complete'] and v['approved']
     except (AssertionError,ValueError,KeyError,TypeError):pass
     if fresh and valid:e['line']=reply['speech'];e['stage']='upgraded'
     result=dict(id=v['id'],stage=e['stage'],line=e['line'],reason='upgraded' if fresh and valid else 'stale' if not fresh else 'invalid_response',raw=e['raw'])
    elif k=='freeze':
     fields(v,'id now context');speech_context(v['context']);assert e['stage'] in ['pending','upgraded'] and v['context']==e['context'];e['stage']='frozen';result=dict(id=v['id'],stage='frozen',line=e['line'])
    elif k=='cancel':fields(v,'id now');assert e['stage'] in ['pending','upgraded'];e['stage']='cancelled';result=dict(id=v['id'],stage='cancelled')
    elif k=='retire':fields(v,'id now');assert e['stage'] in ['frozen','cancelled'];del entries[v['id']];result=None
    else:raise ValueError(k)
   clock=v['now'];results.append(dict(ok=True,result=copy.deepcopy(result)))
  except (AssertionError,ValueError,KeyError,TypeError):results.append(dict(ok=False))
 return dict(results=results,model_calls=0)
