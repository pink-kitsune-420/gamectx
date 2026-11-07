"""Stage 2 host oracle: delivery acknowledgements, epoch/dependency freshness."""
import copy

def run(a):
 scene=None;threads={};pending={};lines={};seq=0;out=[]
 for op in a['operations']:
  x=op['args'];kind=op['op']
  try:
   if kind=='scene':
    assert x['participants']==a['participants'] and len(x['facts'])<=32
    if scene:assert x['revision']>=scene['revision'] and x['at']>=scene['at']
    scene=copy.deepcopy(x);v=dict(bound=True)
   elif kind=='thread':
    assert x['listener'] in a['participants'] and x['participants']==a['participants']
    threads[x['id']]=copy.deepcopy(x);v=dict(bound=True)
   elif kind=='generate':
    assert scene and x['thread'] in threads and len(pending)<8
    selected=[f for f in scene['facts'] if f['id'] in x['dependencies']]
    assert len(selected)==len(x['dependencies']) and all(f['kind']!='global' and set(a['participants'])<=set(f['audience']) for f in selected)
    seq+=1;pending[seq]=dict(scene=scene['id'],epoch=scene['epoch'],deps=selected,life=x['life_revision'],expires=scene['at']+20,thread=copy.deepcopy(threads[x['thread']]))
    lines[seq]=dict(id=seq,thread=x['thread'],text=x['text'],stage='generated',played=0,heard=[])
    v=dict(id=seq,stage='generated')
   elif kind=='validate':
    p=pending.pop(x['id']);line=lines[x['id']]
    valid=scene['at']<p['expires'] and p['scene']==scene['id'] and p['epoch']==scene['epoch'] and p['life']==x['life_revision'] and p['thread']==threads[line['thread']] and all(f in scene['facts'] for f in p['deps'])
    line['stage']='validated' if valid else 'cancelled';v=dict(id=x['id'],stage=line['stage'])
   elif kind=='delivery':
    l=lines[x['id']];s=l['stage'];n=x['through'];event=x['event']
    assert 0<=n<=len(l['text'])
    if event=='queued':assert s=='validated' and n==0
    elif event=='played':assert s in ('queued','played') and n>=l['played'];l['played']=n
    elif event=='heard':
     assert s in ('played','interrupted') and n<=l['played'] and x['recipient'] in a['participants'];old=next((i for i in l['heard'] if i['recipient']==x['recipient']),None)
     assert old is None or n>old['through']
     if old:old['through']=n
     else:l['heard'].append(dict(recipient=x['recipient'],through=n))
    elif event=='interrupted':assert s in ('queued','played') and n==l['played']
    elif event=='cancelled':assert s in ('generated','validated','queued') and n==0
    else:raise ValueError()
    if event!='heard':l['stage']=event
    v=dict(id=x['id'],stage=l['stage'],played=l['played'],heard=copy.deepcopy(l['heard']))
   elif kind=='inspect':v=dict(scene=scene,threads=list(threads.values()),speech=list(lines.values()))
   else:raise ValueError()
   out.append(dict(ok=True,result=copy.deepcopy(v)))
  except (AssertionError,KeyError,ValueError):out.append(dict(ok=False))
 return dict(results=out)
