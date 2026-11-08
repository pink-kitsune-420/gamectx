"""Independent original visual contract/registry, using only Python stdlib."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,hashlib,json
from archetype_reference import fields,integer,name,scope
from life_reference import run as life_run
def encode(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def canon(v):
 if isinstance(v,dict):return {k:canon(v[k]) for k in sorted(v)}
 if isinstance(v,list):return [canon(x) for x in v]
 return v
def digest(v):return hashlib.sha256(encode(canon(v)).encode()).hexdigest()
def token(v):assert isinstance(v,str) and 0<len(v)<=128 and len(v.encode())<=512 and v==v.strip(' ') and not any(ord(c)<32 or 127<=ord(c)<160 for c in v);return v
def tokens(v,cap=32):assert isinstance(v,list) and len(v)<=cap and len(set(v))==len(v);return sorted(token(x) for x in v)
def bible(v):
 fields(v,'version id revision style format width height aspect camera_angle framing palette negative_tokens anchors expressions poses facts max_fact_age');assert v['version']==1;name(v['id']);integer(v['revision'],1,2**32-1)
 for k in ['style','camera_angle','framing']:token(v[k])
 assert v['format'] in ['sprite','portrait','scene'];integer(v['width'],16,4096);integer(v['height'],16,4096);assert v['width']*v['height']<=16777216;fields(v['aspect'],'width height');integer(v['aspect']['width'],1,256);integer(v['aspect']['height'],1,256);assert v['width']*v['aspect']['height']==v['height']*v['aspect']['width']
 tokens(v['palette'],16);tokens(v['negative_tokens']);assert v['palette'];integer(v['max_fact_age'],0,1000);assert 0<len(v['anchors'])<=64 and len(v['expressions'])<=32 and len(v['poses'])<=32 and len(v['facts'])<=64
 ids=[]
 for a in v['anchors']:
  fields(a,'entity kind silhouette attire props seed');scope(a['entity']);assert a['entity'] not in ids;ids.append(a['entity']);assert a['kind'] in ['character','scene'];token(a['silhouette']);token(a['attire']);tokens(a['props'],16);integer(a['seed'],0,2**32-1)
 ids=[]
 for e in v['expressions']:
  fields(e,'id slot kind subject min max priority tokens');name(e['id']);name(e['slot']);name(e['kind']);assert e['id'] not in ids and e['subject'] in ['listener','self'];ids.append(e['id']);integer(e['min'],-100,100);integer(e['max'],e['min'],100);integer(e['priority'],-100,100);tokens(e['tokens'],8)
 ids=[]
 for e in v['poses']:
  fields(e,'activity tokens');scope(e['activity']);assert e['activity'] not in ids;ids.append(e['activity']);tokens(e['tokens'],8)
 ids=[]
 for f in v['facts']:
  fields(f,'id group priority tokens props');scope(f['id']);assert f['id'] not in ids;ids.append(f['id']);assert f['group'] is None or name(f['group']);integer(f['priority'],-100,100);tokens(f['tokens'],8);tokens(f['props'],8);assert all(any(p in a['props'] for a in v['anchors']) for p in f['props'])
 n=copy.deepcopy(v)
 for k in ['palette','negative_tokens']:n[k]=tokens(n[k])
 for a in n['anchors']:a['props']=tokens(a['props'],16)
 for k,field in [('anchors','entity'),('expressions','id'),('poses','activity'),('facts','id')]:
  n[k].sort(key=lambda x:x[field])
  for row in n[k]:
   for t in ['tokens','props']:
    if t in row:row[t]=tokens(row[t])
 return canon(n)
def activity(v):
 if v is None:return
 fields(v,'scope actor revision source state id');scope(v['scope']);scope(v['actor']);scope(v['id']);integer(v['revision'],0,2**32-1);assert v['source'] in ['routine','initiative'] and v['state']=='active'
def compile(a):
 fields(a,'bible frame life activity');b=bible(a['bible']);f=a['frame'];fields(f,'scope observer entity listener scene audience life_revision activity_revision');scope(f['scope']);scope(f['observer']);scope(f['entity']);scope(f['listener']);audience=tokens(f['audience'],16);assert audience
 s=f['scene'];fields(s,'id revision epoch at participants facts affordances');scope(s['id']);integer(s['revision'],0,2**32-1);integer(s['epoch'],0,2**32-1);integer(s['at'],0,2**32-1);assert f['observer'] in s['participants'] and f['listener'] in s['participants'] and set(audience)<=set(s['participants']) and len(s['facts'])<=32
 anchor=next(x for x in b['anchors'] if x['entity']==f['entity']);assert anchor['kind']==('scene' if b['format']=='scene' else 'character');assert b['format']!='scene' or f['entity']==s['id']
 attrs=[];level=None;expression=dict(slot='neutral',tokens=['neutral expression']);expr_rule=None;life_rev=None
 if a['life'] is not None:
  l=copy.deepcopy(a['life']);assert l['scope']==f['scope'] and l['actor']==f['entity'] and l['participants']==s['participants'] and l['defaults'].get('enabled',False);l['operations'].append(dict(op='inspect',args={}));state=life_run(l)['results'][-1]['result'];assert state['revision']==f['life_revision'];attrs=state['attitudes'];life_rev=state['revision']
 else:assert f['life_revision'] is None
 matches=[]
 for e in b['expressions']:
  subject=f['listener'] if e['subject']=='listener' else f['entity'];aa=[x for x in attrs if x['definition']['kind']==e['kind'] and x['definition']['subject']==subject and set(audience)<=set(x['definition']['audience'])]
  if len(aa)==1 and e['min']<=aa[0]['level']<=e['max']:matches.append((e,aa[0]['level']))
 if matches:
  e,level=sorted(matches,key=lambda x:(-x[0]['priority'],x[0]['id']))[0];expression=dict(slot=e['slot'],tokens=e['tokens']);expr_rule=e['id']
 activity(a['activity']);pose=dict(id='idle',tokens=['at rest']);pose_source=None
 if a['activity'] is None:assert f['activity_revision'] is None
 else:
  ac=a['activity'];assert ac['scope']==f['scope'] and ac['actor']==f['entity'] and ac['revision']==f['activity_revision'];mapped=next((x for x in b['poses'] if x['activity']==ac['id']),None)
  if mapped:pose=dict(id=ac['id'],tokens=mapped['tokens']);pose_source=ac['source']
 eligible=[];omitted=[];ids=[]
 for fact in s['facts']:
  fields(fact,'id revision observed_at source kind audience text');scope(fact['id']);assert fact['id'] not in ids;ids.append(fact['id']);integer(fact['revision'],0,s['revision']);integer(fact['observed_at'],0,s['at']);assert fact['kind'] in ['global','perceived','mutual','report']
  reason='not_perceived' if fact['kind']!='perceived' else 'foreign_observer' if fact['source']['speaker']!=f['observer'] else 'audience' if not set(audience)<=set(fact['audience']) else 'aged' if s['at']-fact['observed_at']>b['max_fact_age'] else None
  rule=next((x for x in b['facts'] if x['id']==fact['id']),None)
  if reason is None and rule is None:reason='unmapped'
  if reason is None and not set(rule['props'])<=set(anchor['props']):reason='foreign_prop'
  if reason:omitted.append(dict(id=fact['id'],reason=reason))
  else:eligible.append(rule)
 selected=[];groups=[]
 for rule in sorted(eligible,key=lambda x:(-x['priority'],x['id'])):
  if rule['group'] is not None and rule['group'] in groups:omitted.append(dict(id=rule['id'],reason='occluded'));continue
  selected.append(rule)
  if rule['group'] is not None:groups.append(rule['group'])
 data=dict(version=1,bible_id=b['id'],art_bible_revision=b['revision'],bible_digest=digest(b),scope=f['scope'],observer=f['observer'],audience=audience,entity=f['entity'],scene=s['id'],scene_epoch=s['epoch'],style=b['style'],format=b['format'],width=b['width'],height=b['height'],aspect=b['aspect'],camera_angle=b['camera_angle'],framing=b['framing'],palette=b['palette'],silhouette=anchor['silhouette'],attire=anchor['attire'],expression=expression,pose=pose,visible_props=sorted({x for r in selected for x in r['props']}),environment=sorted([canon(dict(tokens=r['tokens'],props=r['props'])) for r in selected],key=encode),seed=int(digest(dict(bible=b['id'],entity=f['entity'],seed=anchor['seed']))[:8],16),negative_tokens=b['negative_tokens'],extra_tokens=[],forbidden_props=[])
 state=dict(scene_revision=s['revision'],life_revision=life_rev,attitude_level=level,attitudes=sorted([dict(id=x['definition']['id'],kind=x['definition']['kind'],subject=x['definition']['subject'],level=x['level']) for x in attrs if x['definition']['subject'] in [f['listener'],f['entity']] and set(audience)<=set(x['definition']['audience'])],key=lambda x:x['id']),activity=a['activity'],expression_rule=expr_rule,pose_source=pose_source,activity_revision=f['activity_revision'],selected_facts=sorted(x['id'] for x in selected),omitted=sorted(omitted,key=lambda x:x['id']),bible_rules=dict(id=b['id'],revision=b['revision'],digest=digest(b)))
 return dict(key='visual:'+digest(data),spec=canon(data),inspection=dict(state=state,repairs=[]))
def asset(uri,sha):assert isinstance(uri,str) and 0<len(uri.encode())<=1024 and not any(ord(c)<32 or 127<=ord(c)<160 for c in uri);assert any(uri.startswith(s) for s in ['asset://','https://','http://','file://','urn:']);assert len(sha)==64 and all(c in '0123456789abcdef' for c in sha);return dict(asset_uri=uri,digest=sha)
def compatible(a,b):
 same=['bible_digest','scope','observer','audience','entity','scene','scene_epoch','format','width','height','aspect','camera_angle','framing','silhouette','attire']
 for field,id in [('expression','slot'),('pose','id')]:
  if a[field][id]==b[field][id] and a[field]!=b[field]:return False
 return all(a[k]==b[k] for k in same) and a['expression']['slot'] in [b['expression']['slot'],'neutral'] and a['pose']['id'] in [b['pose']['id'],'idle'] and set(a['visible_props'])<=set(b['visible_props']) and {encode(canon(x)) for x in a['environment']}<={encode(canon(x)) for x in b['environment']} and set(b['negative_tokens'])<=set(a['negative_tokens']) and set(b['forbidden_props']).isdisjoint(a['visible_props']) and set(b['extra_tokens'])<=set(a['extra_tokens'])
class Registry:
 def __init__(self,cfg):
  fields(cfg,'enabled capacity queue_limit epoch');assert type(cfg['enabled']) is bool;integer(cfg['capacity'],1,128);integer(cfg['queue_limit'],0,32);scope(cfg['epoch']);self.cfg=copy.deepcopy(cfg);self.rows={};self.aliases={};self.next=0
 def resolve(self,key):return self.aliases.get(key,key)
 def fallback(self,spec):
  rows=[r for r in self.rows.values() if r['state'] in ['approved','pinned'] and r['asset'] is not None and compatible(r['spec'],spec)]
  rows.sort(key=lambda r:(-(4*(r['spec']['expression']==spec['expression'])+2*(r['spec']['pose']==spec['pose'])+int(r['state']=='pinned')),r['key']))
  return None if not rows else rows[0]['asset']|dict(key=rows[0]['key'])
 def request(self,s,enqueue=True):
  key=self.resolve(s['key']);existing=self.rows.get(key)
  if existing is None and len(self.rows)<self.cfg['capacity']:existing=dict(key=s['key'],spec=copy.deepcopy(s['spec']),inspection=copy.deepcopy(s['inspection']),state='missing',asset=None,candidate=None,ticket=None,retired=False);self.rows[key]=existing
  if existing is None:return dict(key=key,state='missing',asset=self.fallback(s['spec']),exact=False,pending=False,generation=None,reason='capacity')
  if key==s['key']:assert existing['spec']==s['spec']
  existing['inspection']['state']=copy.deepcopy(s['inspection']['state'])
  if existing['state'] in ['approved','pinned']:return dict(key=key,state=existing['state'],asset=existing['asset']|dict(key=key),exact=True,pending=False,generation=None,reason='reuse')
  fallback=self.fallback(existing['spec']);job=None;reason='pending' if existing['state']=='candidate' else 'rejected' if existing['state']=='rejected' else 'disabled' if not self.cfg['enabled'] else 'not_requested' if not enqueue else 'queue_limit'
  if enqueue and existing['state']=='missing' and self.cfg['enabled'] and sum(x['ticket'] is not None for x in self.rows.values())<self.cfg['queue_limit']:
   self.next+=1;existing['ticket']=dict(epoch=self.cfg['epoch'],sequence=self.next);existing['state']='candidate';job=dict(key=key,ticket=existing['ticket'],spec=existing['spec']);reason='queued'
  return dict(key=key,state=existing['state'],asset=fallback,exact=False,pending=existing['state']=='candidate',generation=copy.deepcopy(job),reason=reason)
 def submit(self,key,ticket,uri,sha):
  fields(ticket,'epoch sequence');scope(ticket['epoch']);integer(ticket['sequence'],1,2**32-1);r=self.rows[key];ref=asset(uri,sha);assert not r['retired'] and r['state']=='candidate' and r['ticket']==ticket;r['candidate']=ref;r['ticket']=None;return dict(key=key,state='candidate')
 def approve(self,key,uri,sha):
  r=self.rows[key];ref=asset(uri,sha);assert not r['retired'] and r['state'] in ['missing','candidate','approved','pinned']
  if r['state'] in ['approved','pinned']:assert ref==r['asset'];return dict(key=key,state=r['state'])
  if r['candidate'] is not None:assert r['candidate']==ref
  r.update(state='approved',asset=ref,candidate=None,ticket=None);return dict(key=key,state='approved')
 def reject(self,key,reason):
  r=self.rows[key];token(reason);assert not r['retired'];r.update(state='rejected',asset=None,candidate=None,ticket=None);return dict(key=key,state='rejected',reason=reason)
 def repair(self,key,p):
  fields(p,'replace_tokens add_tokens negative_tokens forbid_props seed_delta asset');r=self.rows[key];assert not r['retired'] and len(r['inspection']['repairs'])<8
  d=copy.deepcopy(r['spec']);tokens(p['add_tokens'],16);tokens(p['negative_tokens']);tokens(p['forbid_props'],16);integer(p['seed_delta'],0,2**32-1);assert len(p['replace_tokens'])<=16
  editable=[d['silhouette'],d['attire']]+d['expression']['tokens']+d['pose']['tokens']+[t for cue in d['environment'] for t in cue['tokens']]+d['extra_tokens'];seen=[]
  for q in p['replace_tokens']:
   fields(q,'from to');token(q['from']);token(q['to']);assert q['from'] in editable and q['from'] not in seen;seen.append(q['from'])
   for k in ['silhouette','attire']:
    if d[k]==q['from']:d[k]=q['to']
   for sub in [d['expression']['tokens'],d['pose']['tokens'],d['extra_tokens']]+[cue['tokens'] for cue in d['environment']]:sub[:]=[q['to'] if x==q['from'] else x for x in sub]
  d['extra_tokens']=sorted(set(d['extra_tokens']+p['add_tokens']));d['negative_tokens']=sorted(set(d['negative_tokens']+p['negative_tokens']+['no '+x for x in p['forbid_props']]));d['forbidden_props']=sorted(set(d['forbidden_props']+p['forbid_props']));d['visible_props']=[x for x in d['visible_props'] if x not in d['forbidden_props']];d['environment']=[cue for cue in d['environment'] if not set(cue['props'])&set(d['forbidden_props'])];d['seed']=(d['seed']+p['seed_delta'])%(2**32)
  for sub in [d['expression'],d['pose']]:sub['tokens']=sorted(set(sub['tokens']))
  for k in ['extra_tokens','negative_tokens','forbidden_props']:d[k]=sorted(set(d[k]));tokens(d[k],64)
  for cue in d['environment']:cue['tokens']=sorted(set(cue['tokens']))
  d['environment']=sorted([canon(cue) for cue in d['environment']],key=encode)
  if p['asset'] is not None:fields(p['asset'],'asset_uri digest')
  d=canon(d);new='visual:'+digest(d);ref=None if p['asset'] is None else asset(p['asset']['asset_uri'],p['asset']['digest']);assert new!=key or ref is not None
  if new not in self.rows:assert len(self.rows)<self.cfg['capacity']
  invalidated=r['ticket'];inspection=copy.deepcopy(r['inspection']);inspection['repairs'].append(copy.deepcopy(p));r.update(retired=new!=key,ticket=None,candidate=None)
  if new not in self.rows:self.rows[new]=dict(key=new,spec=d,inspection=inspection,state='missing',asset=None,candidate=None,ticket=None,retired=False)
  target=self.rows[new];target['retired']=False;target['inspection']=inspection
  if target['state']=='candidate' and target['ticket'] is None and target['candidate'] is None:target['state']='missing'
  if ref is not None:target.update(state='pinned',asset=ref,candidate=None,ticket=None)
  for k in list(self.aliases):
   if self.aliases[k]==key:self.aliases[k]=new
  self.aliases[key]=new;self.aliases[new]=new
  return dict(key=new,state=target['state'],invalidated=invalidated)
def workflow(a):
 fields(a,'config specs operations');specs=[compile(x) for x in a['specs']];assert len(specs)<=16 and len(a['operations'])<=256;registry=Registry(a['config']);keys=[];jobs=[];results=[]
 for op in a['operations']:
  try:
   fields(op,'op args');v=op['args'];k=op['op']
   if k=='request':fields(v,'spec');result=registry.request(specs[v['spec']]);keys.append(result['key']);jobs.append(None if result['generation'] is None else result['generation']['ticket'])
   else:
    key=keys[v['key']]
    if k=='approve':fields(v,'key asset_uri digest');result=registry.approve(key,v['asset_uri'],v['digest'])
    elif k=='submit':fields(v,'key ticket asset_uri digest');result=registry.submit(key,jobs[v['ticket']],v['asset_uri'],v['digest'])
    elif k=='reject':fields(v,'key reason');result=registry.reject(key,v['reason'])
    elif k=='repair':fields(v,'key patch');result=registry.repair(key,v['patch']);keys.append(result['key']);jobs.append(None)
    elif k=='inspect':fields(v,'key');result=copy.deepcopy(registry.rows[key])
    else:raise ValueError(k)
   results.append(dict(ok=True,result=copy.deepcopy(result)))
  except (AssertionError,ValueError,KeyError,IndexError,TypeError,StopIteration):results.append(dict(ok=False))
 return dict(results=results,queued_generation_requests=sum(x.get('result',{}).get('generation') is not None for x in results),actual_model_calls=0)
