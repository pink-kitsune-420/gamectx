"""Independent, offline prototype for optional routines and source time knowledge.
No model/provider calls. Tick coordinates and calendar meanings belong to the host.
"""
import copy

DEFAULTS = dict(routines=False, history=False, time=False, tools=[], timeline=32,
                observations=32, retention=1000, horizon=100, interval=1,
                min_active=2, switch_margin=1, choices=4, context_observations=3,
                model_calls=1, time_sources=[])

def config(defaults, overrides):
    result = copy.deepcopy(DEFAULTS)
    for layer in (defaults, overrides):
        assert isinstance(layer, dict) and set(layer) <= set(result)
        result.update(copy.deepcopy(layer))
    for key in ('routines','history','time'): assert type(result[key]) is bool
    for key, hi in dict(timeline=128,observations=128,retention=1000000,horizon=1000000,interval=1000000,min_active=1000000,switch_margin=1000000,choices=32,context_observations=32,model_calls=32).items():
        assert type(result[key]) is int and (0 if key in ('min_active','switch_margin','model_calls','context_observations') else 1) <= result[key] <= hi
    assert len(set(result['tools'])) == len(result['tools']) and set(result['tools']) <= {'routine_status','activity_propose'}
    assert len(set(result['time_sources'])) == len(result['time_sources']) and set(result['time_sources']) <= {'sun','instrument','report','inference','routine'}
    assert not result['time'] or result['history']
    assert not result['tools'] or result['routines']
    return result

def definition(d):
    assert set(d)=={'version','id','revision','calendar','activities'} and d['version']==1
    assert len(d['activities'])<=128
    ids=set()
    for a in d['activities']:
        assert set(a)=={'id','location','start','end','repeat','priority','requirements','interrupt','resume','missed','grace'}
        assert a['id'] not in ids; ids.add(a['id'])
        assert 0<=a['start']<a['end']<=2**32-1
        assert a['repeat'] is None or a['end']-a['start'] <= a['repeat'] <= 1000000
        assert 0<=a['priority']<=1000000 and len(a['requirements'])<=16 and len(set(a['requirements']))==len(a['requirements'])
        assert a['interrupt'] in ('allow','locked') and a['resume'] in ('resume','cancel') and a['missed'] in ('cancel','defer')
        assert 0<=a['grace']<=1000000 and (a['missed']=='defer' or a['grace']==0)
    return copy.deepcopy(d)

class Runtime:
    def __init__(self, d, cfg):
        self.d=definition(d);self.cfg=cfg;self.rows=[];self.now=0;self.revision=None;self.frame=None
    def update(self, f):
        assert self.revision is None or f['revision']>self.revision
        assert f['now']>=self.now and (self.revision is None or f['now']-self.now>=self.cfg['interval'] or f['emergency'])
        rows=copy.deepcopy(self.rows); now=f['now']; acts={a['id']:a for a in self.d['activities']}
        def deadline(a,start): return start+a['end']-a['start']+(a['grace'] if a['missed']=='defer' else 0)
        def eligible(r):
            a=acts[r['id']]
            return r['state'] in ('planned','interrupted','active') and r['start']<=now<deadline(a,r['start']) and a['id'] in f['allowed'] and set(a['requirements'])<=set(f['conditions'])
        for r in rows:
            if r['state'] in ('planned','interrupted','active') and now>=deadline(acts[r['id']],r['start']):r.update(state='cancelled',since=now,reason='missed')
        rows=[r for r in rows if r['state'] not in ('completed','cancelled') or deadline(acts[r['id']],r['start'])>now]
        for a in self.d['activities']:
            start=a['start']; period=a['repeat']; duration=a['end']-a['start']
            if period and now>=deadline(a,start): start+=((now-deadline(a,start))//period+1)*period
            while start<=min(2**32-1,now+self.cfg['horizon']):
                if deadline(a,start)>now and start+duration<=2**32-1 and not any(r['id']==a['id'] and r['start']==start for r in rows):rows.append(dict(id=a['id'],start=start,end=start+duration,state='planned',since=now,reason='window'))
                if period is None:break
                start+=period
                assert len(rows)<=self.cfg['timeline']
        assert len(rows)<=self.cfg['timeline']
        rows.sort(key=lambda r:(r['start'],r['id']))
        active=next((r for r in rows if r['state']=='active'),None)
        candidates=sorted((r for r in rows if eligible(r)),key=lambda r:(-acts[r['id']]['priority'],r['start'],r['id']))
        best=candidates[0] if candidates else None
        if active and eligible(active) and best and not f['emergency']:
            a=acts[active['id']]
            if a['interrupt']=='locked' or now-active['since']<self.cfg['min_active'] or acts[best['id']]['priority']<a['priority']+self.cfg['switch_margin']:best=active
        if active is not best and active:
            active.update(state='interrupted' if acts[active['id']]['resume']=='resume' else 'cancelled',since=now,reason='preempted')
        if best and best['state']!='active':best.update(state='active',since=now,reason='selected')
        self.rows=rows;self.now=now;self.revision=f['revision'];self.frame=copy.deepcopy(f)
        return copy.deepcopy(rows)

def time_knowledge(records, now, cfg, calendar, phase=None):
    sources=[]
    if cfg['time']:
        for o in reversed(records):
            if len(sources)>=cfg['context_observations']:break
            t=o['time']; age=now-o['at']
            if now>=o['expires'] or not t or not t['usable'] or t['source'] not in cfg['time_sources']:continue
            lo=t['low']+age*t['rate_low'];hi=t['high']+age*t['rate_high']
            if hi>2**32-1:continue
            sources.append(dict(source=copy.deepcopy(o['source']),basis=t['source'],observed_at=o['at'],age=age,calendar=t['calendar'],low=lo,high=hi,precision='exact' if lo==hi and t['source']=='instrument' else 'range',uncertainty=o['uncertainty']))
            if len(sources)>=cfg['context_observations']:break
        if 'routine' in cfg['time_sources'] and phase is not None:
            sources.append(dict(source=phase['routine'],basis='routine',phase=phase['id'],precision='approximate'))
    conflicts=any(a.get('calendar')==b.get('calendar') and 'low' in a and 'low' in b and (a['high']<b['low'] or b['high']<a['low']) for i,a in enumerate(sources) for b in sources[:i])
    return dict(status='known' if sources else 'unknown',sources=sources,conflicting=conflicts)
