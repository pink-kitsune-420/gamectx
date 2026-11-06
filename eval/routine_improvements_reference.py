"""Independent offline prototype: explicit evidence selection and due timers."""
import copy

def select(records,now,cfg,q):
 assert set(q)=={'kinds','locations','time_sources','required','limit'}
 for key,max_n in [('kinds',4),('locations',16),('time_sources',4),('required',32)]:
  assert isinstance(q[key],list) and len(q[key])<=max_n and len(set(q[key]))==len(q[key])
 assert set(q['kinds'])<={'direct','instrument','report','inference'}
 assert set(q['time_sources'])<={'sun','instrument','report','inference'}
 assert type(q['limit']) is int and 0<=q['limit']<=cfg['context_observations']
 assert len(q['required'])<=q['limit'] and all(type(t) is int and 1<=t<=2**32-1 for t in q['required'])
 def matches(o):return o['at']<=now<o['expires'] and (not q['kinds'] or o['kind'] in q['kinds']) and (not q['locations'] or o['location'] in q['locations']) and (not q['time_sources'] or o['time'] and o['time']['source'] in q['time_sources'])
 matches=[o for o in records if matches(o)]
 assert all(any(o['token']==t for o in matches) for t in q['required'])
 selected=set(q['required'])
 for o in reversed(matches):
  if len(selected)>=q['limit']:break
  selected.add(o['token'])
 return copy.deepcopy([o for o in matches if o['token'] in selected])

def next_wake(r,records):
 f=r.frame
 if f is None:return None
 now=f['now'];events=[]
 def boundary(first,period):
  if first<=now:
   if period is None:return
   first+=((now-first)//period+1)*period
  if first<=2**32-1:events.append(first)
 for a in r.d['activities']:
  if a['id'] not in f['allowed'] or not set(a['requirements'])<=set(f['conditions']):continue
  for first in (a['start'],a['start']-r.cfg['horizon'],a['end']+(a['grace'] if a['missed']=='defer' else 0)):boundary(first,a['repeat'])
 active=next((e for e in r.rows if e['state']=='active'),None)
 acts={a['id']:a for a in r.d['activities']}
 if active and acts[active['id']]['interrupt']=='allow':
  a=acts[active['id']]
  challenger=any(e['id']!=active['id'] and e['state'] in ('planned','interrupted') and e['start']<=now<acts[e['id']]['end']+e['start']-acts[e['id']]['start']+(acts[e['id']]['grace'] if acts[e['id']]['missed']=='defer' else 0) and e['id'] in f['allowed'] and set(acts[e['id']]['requirements'])<=set(f['conditions']) and acts[e['id']]['priority']>=a['priority']+r.cfg['switch_margin'] for e in r.rows)
  if challenger and active['since']+r.cfg['min_active']>now:events.append(active['since']+r.cfg['min_active'])
 events.extend(o['expires'] for o in records if o['expires']>now)
 if not events:return None
 result=max(min(events),now+r.cfg['interval'])
 return result if result<=2**32-1 else None

def affected(r,conditions,activities):
 return any(a['id'] in activities or set(a['requirements'])&set(conditions) for a in r.d['activities'])
