"""Independent bounded life rules, offline; no network or Rust-produced expectations."""
import copy

def run(a):
 d=a['definition']; setting=a['setting']
 assert set(d)==set('version id revision setting calendar facets attitudes rules history improvisation'.split())
 assert d['version']==1 and d['setting']==setting['id'] and d['calendar']==setting['calendar']
 assert len(d['attitudes'])<=64 and len({x['id'] for x in d['attitudes']})==len(d['attitudes'])
 assert all(x in [h['id'] for h in setting['history']] for x in d['history'])
 for r in d['rules']: assert 0<abs(r['delta'])<=20 and r['cooldown']>=1
 cfg=dict(enabled=False,balanced=False,deception=False,mistaken=False,tools=[])
 for c in (a['defaults'],a['overrides']): assert set(c)<=set(cfg);cfg.update(c)
 assert cfg['enabled'] or not (cfg['balanced'] or cfg['deception'] or cfg['mistaken'] or cfg['tools'])
 attrs=[dict(definition=copy.deepcopy(x),level=x['level'],supports=[],contradicts=[],last=None) for x in d['attitudes']]
 rev=clock=watermark=0;events=[];changes=[];results=[]
 for op in a['operations']:
  v=op['args'];kind=op['op']
  try:
   assert cfg['enabled'] or kind=='inspect'
   if kind=='experience':
    assert set(v)==set('token at kind tag subject source audience text causes retain'.split())
    assert v['token']>watermark and v['at']>=clock and len(events)<128
    assert v['audience']==a['participants'] and v['kind'] in ('direct','report','intended','outcome','correction')
    assert all(0<x<v['token'] for x in v['causes'])
    rev+=1;clock=v['at'];watermark=v['token'];events.append(copy.deepcopy(v));result=dict(revision=rev)
   elif kind in ('appraise','propose'):
    import json
    proposal=json.loads(v['text']) if kind=='propose' else v
    r=next(x for x in d['rules'] if x['id']==proposal['rule'])
    e=next(x for x in events if x['token']==proposal['event'])
    assert e['tag']==r['event'] and e['kind'] in ('direct','outcome')
    if kind=='propose':result=dict(status='proposed',revision=rev,proposal=proposal)
    else:
     assert v['revision']==rev and len(changes)<128
     att=next(x for x in attrs if x['definition']['id']==r['attitude'])
     assert not any(x['rule']==r['id'] and x['event']==e['token'] for x in changes)
     assert att['last'] is None or e['at']>=att['last']+r['cooldown']
     before=att['level'];after=max(-100,min(100,before+r['delta']))
     if before*after<0:after=0
     result=dict(rule=r['id'],event=e['token'],at=e['at'],before=before,after=after,reason=r['reason'])
     att['level']=after;att['last']=e['at'];refs=att['supports' if r['delta']>=0 else 'contradicts'];refs.append(e['token']);refs[:]=refs[-16:]
     changes.append(result);rev+=1
   elif kind=='inspect':result=dict(revision=rev,watermark=watermark,clock=clock,attitudes=attrs,events=events,changes=changes)
   elif kind=='context':
    assert 0<=v['limit']<=16
    def visible(x):return set(a['participants'])<=set(x['audience']) and (not v['tags'] or set(v['tags'])&set(x['tags']))
    result=dict(scope=a['scope'],revision=rev,facets=[x for x in d['facets'] if visible(x)][:v['limit']],attitudes=[x for x in attrs if visible(x['definition']) and (cfg['mistaken'] or x['definition']['kind']!='misconception')][:v['limit']],policy=dict(balanced=cfg['balanced'],deception=cfg['deception'],mistaken=cfg['mistaken'],improvisation=d['improvisation'],expression='Opinions, humor and imagined hypotheticals may be expressed. Separate facts, reports, beliefs, inference and proposals. Answer known parts and relevant uncertainty; invented flavor is not lore or experience.'))
   else:raise ValueError(kind)
   results.append(dict(ok=True,result=copy.deepcopy(result)))
  except (AssertionError,KeyError,ValueError,StopIteration,TypeError):results.append(dict(ok=False))
 return dict(results=results)
