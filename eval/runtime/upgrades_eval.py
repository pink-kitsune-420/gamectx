#!/usr/bin/env python3
"""Offline prototypes for opt-in recall, procedures, outcomes and runtime guards."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,json,re,sys,subprocess,random
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import brain_eval as b
import facts_eval as f

def dump(v):return b.dumps(v)
def integer(v,lo=0,hi=2**32-1):return b.integer(v,lo,hi)
def fields(v,keys):b.fields(v,keys.split())
def name(v):return b.name(v)
def scope(v):return f.name(v)
def getref(v):
    fields(v,'scope key revision');scope(v['scope']);scope(v['key']);integer(v['revision'],1,2**31-1);return v

def tokens(text,normalize,aliases):
    if not isinstance(text,str) or len(text.encode())>4096:raise ValueError('recall text')
    words=re.findall('[a-z0-9_]+',text.translate(str.maketrans('ABCDEFGHIJKLMNOPQRSTUVWXYZ','abcdefghijklmnopqrstuvwxyz')))
    def stem(s):
        if normalize:
            if len(s)>5 and s.endswith('ing'):s=s[:-3]
            elif len(s)>4 and s.endswith('ed'):s=s[:-2]
            elif len(s)>3 and s.endswith('s') and not s.endswith('ss'):s=s[:-1]
        return aliases.get(s,s)
    return sorted(set(stem(w) for w in words if len(w)<=64))

class Recall:
    def __init__(self,records,config):
        fields(config,'normalize aliases')
        if type(config['normalize']) is not bool or not isinstance(config['aliases'],list) or len(config['aliases'])>64 or len(records)>4096:raise ValueError('recall bounds')
        self.aliases={};self.normalize=config['normalize'];self.records=records;self.index={}
        for pair in config['aliases']:
            if not isinstance(pair,list) or len(pair)!=2:raise ValueError('alias pair')
            a,z=map(name,pair)
            if a==z or a in self.aliases:raise ValueError('alias identity')
            self.aliases[a]=z
        if set(self.aliases)&set(self.aliases.values()):raise ValueError('alias chain')
        for i,r in enumerate(records):
            fields(r,'id turn importance text tags');integer(r['id'],1);integer(r['turn']);integer(r['importance'],1,10)
            if i and any(old['id']==r['id'] for old in records[:i]):raise ValueError('duplicate id')
            if len(r['tags'])>8 or len(r['text'])>280:raise ValueError('record bounds')
            for tag in r['tags']:name(tag)
            for t in tokens(r['text']+' '+' '.join(r['tags']),self.normalize,self.aliases):self.index.setdefault(t,[]).append(i)
    def search(self,text,count):
        integer(count,0,128);scores={};visits=0
        for token in tokens(text,self.normalize,self.aliases):
            for i in self.index.get(token,[]):scores[i]=scores.get(i,0)+1;visits+=1
        order=sorted(scores,key=lambda i:(-scores[i],-self.records[i]['importance'],-self.records[i]['turn'],self.records[i]['id']))[:count]
        return dict(records=[self.records[i] for i in order],candidates=len(scores),postings=visits)

class Outcomes:
    def __init__(self,capacity):integer(capacity,1,128);self.capacity=capacity;self.last=0;self.clock=0;self.rows=[]
    def op(self,o):
        now=integer(o['now'],self.clock);method=o['op']
        if method=='begin':
            ident=integer(o['id'],self.last+1);wait=integer(o['wait'],1,1000000);window=integer(o['window'],1,1000000)
            if len(self.rows)==self.capacity or now+wait+window>2**32-1:raise ValueError('outcome bounds')
            self.rows.append(dict(id=ident,created=now,deadline=now+wait,window=window,phase='planned',score=None,consumed=False));self.last=ident
        else:
            row=next(r for r in self.rows if r['id']==o['id'])
            if method=='report':return self.report(row,now)
            if method=='take':
                if row['phase']!='observed' or row['consumed']:raise ValueError('label consumed/unobserved')
                result=self.report(row,now);row['consumed']=True;self.clock=now;return result
            if method=='forget':self.rows.remove(row)
            elif method=='cancel':
                if row['phase'] in ('observed','cancelled'):raise ValueError('terminal')
                row['phase']='cancelled'
            else:
                if now>=row['deadline']:raise ValueError('outcome expired')
                if method=='execute' and row['phase']=='planned':row['phase']='executed'
                elif method=='expose' and row['phase']=='executed':row['phase']='exposed';row['deadline']=now+row['window']
                elif method=='resolve' and row['phase']=='exposed':row['phase']='observed';row['score']=integer(o['score'],0,b.S)
                else:raise ValueError('outcome phase')
        self.clock=now;return None
    def report(self,r,now):
        status=r['phase'] if r['phase']=='observed' or r['phase']!='cancelled' and now<r['deadline'] else 'unknown'
        return dict(id=r['id'],status=status,score=r['score'] if status=='observed' else None,trainable=status=='observed' and not r['consumed'])

class Temporal:
    def __init__(self,capacity,window,start):
        integer(capacity,1,256);integer(window,1,1000000);integer(start)
        self.capacity=capacity;self.window=window;self.start=start;self.clock=start;self.last=0;self.events=[];self.available=True
    def current(self,now):return [e for e in self.events if e['at']<=now<e['expires'] and now-e['at']<self.window]
    def op(self,o):
        now=integer(o['now'],self.clock);method=o['op']
        if method=='availability':
            if type(o['available']) is not bool:raise ValueError('availability')
            if self.available!=o['available']:self.available=o['available'];self.events=[];self.start=now
        elif method=='observe':
            token=integer(o['token'],self.last+1);name(o['kind']);value=integer(o['value'],-1000000000,1000000000);expiry=integer(o['expires'],now+1)
            rows=self.current(now)
            if not self.available or len(rows)==self.capacity:raise ValueError('temporal unavailable/capacity')
            self.events=rows+[dict(token=token,at=now,expires=expiry,kind=o['kind'],value=value)];self.last=token
        elif method=='features':
            name(o['kind']);rows=[e for e in self.current(now) if e['kind']==o['kind']]
            span=min(self.window,now-self.start+1)
            return dict(count=len(rows) if self.available else None,rate_millionths=len(rows)*b.S//span if self.available else None,first=rows[0]['at'] if rows else None,last=rows[-1]['at'] if rows else None,delta=rows[-1]['value']-rows[0]['value'] if rows else None)
        elif method=='sequence':
            if not 1<=len(o['kinds'])<=8:raise ValueError('sequence bound')
            for k in o['kinds']:name(k)
            count=position=0
            for e in self.current(now):
                if e['kind']==o['kinds'][position]:
                    position+=1
                    if position==len(o['kinds']):count+=1;position=0
            return count if self.available else None
        else:raise ValueError('temporal operation')
        self.clock=now;return None

class Ledger:
    def __init__(self,limit,window):integer(limit,0,10**12);integer(window,1,1000000);self.limit=limit;self.window=window;self.clock=0;self.next=0;self.blocked=False;self.rows=[]
    def op(self,o):
        now=integer(o['now'],self.clock);method=o['op'];rows=copy.deepcopy(self.rows);blocked=self.blocked
        if method=='reserve':
            amount=integer(o['amount'],0,10**12);rows=[r for r in rows if r['state']!='settled' or now-r['at']<self.window]
            if len(rows)>=256 or self.next>=2**32-1 or amount and (self.blocked or sum(r['amount'] for r in rows)+amount>self.limit):raise ValueError('ledger cap')
            ident=self.next+1;rows.append(dict(id=ident,at=now,amount=amount,state='reserved'));result=ident
        elif method=='summary':return dict(spent=sum(r['amount'] for r in rows if r['state']!='settled' or now-r['at']<self.window),blocked=blocked,pending=sum(r['state']!='settled' for r in rows))
        elif method=='recover':
            for r in rows:
                if r['state']!='settled':r['state']='settled';r['at']=now
            result=None
        else:
            r=next(r for r in rows if r['id']==o['id'])
            if method=='dispatch' and r['state']=='reserved':r['state']='dispatched';result=None
            elif method=='cancel' and r['state']=='reserved':rows.remove(r);result=None
            elif method=='finish' and r['state']=='dispatched':
                actual=r['amount'] if o['actual'] is None else integer(o['actual'],0,10**12)
                blocked|=actual>r['amount'];r.update(amount=actual,at=now,state='settled');result=None
            else:raise ValueError('ledger phase')
        if method=='reserve':self.next=result
        self.rows=rows;self.clock=now;self.blocked=blocked;return result
    def save(self):return dict(version=1,limit=self.limit,window=self.window,clock=self.clock,next=self.next,blocked=self.blocked,entries=self.rows)

def context(v):
    fields(v,'actor world schema permissions facts');scope(v['actor']);scope(v['world']);integer(v['schema']);integer(v['permissions'])
    if len(v['facts'])>16:raise ValueError('request facts')
    refs=[getref(r) for r in v['facts']]
    if len({(r['scope'],r['key']) for r in refs})!=len(refs):raise ValueError('duplicate context fact')
    return dict(actor=v['actor'],world=v['world'],schema=v['schema'],permissions=v['permissions'],facts=sorted(refs,key=lambda r:(r['scope'],r['key'])))

def process(v):
    kind=v['kind'];results=[]
    if kind=='recall':
        index=Recall(v['records'],v['config']);return [index.search(q['text'],q['count']) for q in v['queries']]
    if kind in ('outcomes','temporal','ledger'):
        state=Outcomes(v['capacity']) if kind=='outcomes' else Temporal(v['capacity'],v['window'],v['start']) if kind=='temporal' else Ledger(v['limit'],v['window'])
        for o in v['operations']:results.append(state.op(o))
        return dict(results=results,state=state.save()) if kind=='ledger' else results
    if kind in ('playbook','requests'):
        registry=f.Facts(v['capacity'])
        for r in v['facts']:registry.register(r)
        if kind=='playbook':
            if len(v['recipes'])>64:raise ValueError('recipes')
            ids=set()
            for r in v['recipes']:
                fields(r,'id action args requires expires');name(r['id']);name(r['action']);f.validate_value(r['args'])
                if not isinstance(r['args'],dict) or r['id'] in ids or not 1<=len(r['requires'])<=8:raise ValueError('recipe bounds')
                ids.add(r['id'])
                if r['expires'] is not None:integer(r['expires'],1)
                refs=set()
                for pre in r['requires']:
                    fields(pre,'reference value');ref=getref(pre['reference']);f.validate_value(pre['value']);identity=(ref['scope'],ref['key'])
                    if identity in refs:raise ValueError('recipe duplicate prerequisite')
                    refs.add(identity)
            for o in v['operations']:
                if o['op']=='register':results.append(registry.register(o['request']));continue
                integer(o['now']);f.validate_privacy(o['denied'],32)
                hits=[]
                for r in sorted(v['recipes'],key=lambda r:r['id']):
                    if r['expires'] is not None and o['now']>=r['expires']:continue
                    valid=True
                    for pre in r['requires']:
                        ref=pre['reference'];check=registry.check(ref['scope'],ref['key'],pre['value'],o['now'],o['denied'])
                        valid &= check['status']=='supported' and check['reference']==ref
                    if valid and r['action'] in o['allowed']:hits.append(dict(id=r['id'],action=r['action'],args=r['args']))
                results.append(hits)
            return results
        pending={};last=clock=0
        for o in v['operations']:
            if o['op']=='register':results.append(registry.register(o['request']));continue
            now=integer(o['now'],clock);clock=now
            if o['op']=='issue':
                ctx=context(o['context']);ttl=integer(o['ttl'],1,1000000)
                if len(pending)>=32 or now+ttl>2**32-1:raise ValueError('request cap')
                last+=1;pending[last]=(ctx,now+ttl);results.append(last)
            elif o['op']=='accept':
                ctx=context(o['context']);saved,until=pending.pop(o['id']);valid=now<until and saved==ctx
                for ref in ctx['facts']:
                    dep=registry.find(ref['scope'],ref['key']);valid &= dep is not None and registry.reference(dep)==ref and registry.active(dep,now,o['denied'])
                results.append(valid)
            elif o['op']=='cancel':pending.pop(o['id']);results.append(None)
            else:raise ValueError('request operation')
        return results
    if kind=='bundle':
        perception=b.Brain(v['perception']);decision=b.Brain(v['decision']);ports=v['ports'];names=set()
        if not 1<=len(ports)<=16:raise ValueError('ports')
        for p in ports:
            fields(p,'input output')
            if p['input'] not in decision.inputs or p['output'] not in perception.outputs or p['input'] in names:raise ValueError('port binding')
            names.add(p['input'])
        prepared=None
        for o in v['operations']:
            if o['op']=='prepare':
                scope(o['scope']);integer(o['frame'],0 if prepared is None else prepared[1]);prepared=(o['scope'],o['frame'],perception.run(o['inputs'],perception.outputs)['scores']);results.append(None)
            elif o['op']=='run':
                if prepared is None or (o['scope'],o['frame'])!=prepared[:2]:raise ValueError('stale bundle frame')
                values=dict(o['inputs'])
                if set(values)!=(set(decision.inputs)-names):raise ValueError('candidate inputs')
                for p in ports:values[p['input']]=prepared[2][p['output']]
                results.append(decision.run(values,o['allowed']))
            else:raise ValueError('bundle operation')
        return results
    raise ValueError('upgrade kind')


def record(n,text):return dict(id=n,turn=n,importance=5,text=text,tags=[])
def fact(key,value,expected=None,**extra):return dict(scope='world',key=key,value=value,now=1,authority='host',pinned=False,expires=None,privacy=[],dependencies=[],expected=expected,**extra)
def fixtures():
    ref=dict(scope='world',key='ready',revision=1);ctx=dict(actor='actor_a',world='world_a',schema=1,permissions=1,facts=[ref])
    recall=dict(kind='recall',records=[record(1,'Lighting near the cellar.'),record(2,'A medic waits by the door.')],config=dict(normalize=True,aliases=[['healer','medic']]),queries=[dict(text='light',count=2),dict(text='healer',count=2),dict(text='unrelated',count=2)])
    yield 'recall-aliases',recall,False
    raw=copy.deepcopy(recall);raw['config']['normalize']=False;raw['config']['aliases']=[];yield 'recall-exact',raw,False
    raw=copy.deepcopy(recall);raw['config']['aliases']=[['a','b'],['b','c']];yield 'error-recall-chain',raw,True
    raw=copy.deepcopy(recall);raw['queries'][0]['count']=129;yield 'error-recall-count',raw,True
    yield 'outcome-observed',dict(kind='outcomes',capacity=4,operations=[dict(op='begin',id=1,now=0,wait=10,window=5),dict(op='execute',id=1,now=1),dict(op='expose',id=1,now=4),dict(op='resolve',id=1,now=6,score=800000),dict(op='report',id=1,now=20)]),False
    raw=dict(kind='outcomes',capacity=4,operations=[dict(op='begin',id=1,now=0,wait=10,window=5),dict(op='execute',id=1,now=1),dict(op='expose',id=1,now=2),dict(op='resolve',id=1,now=3,score=0),dict(op='take',id=1,now=4),dict(op='report',id=1,now=5)])
    yield 'outcome-one-label',raw,False
    raw=copy.deepcopy(raw);raw['operations'].append(dict(op='take',id=1,now=5));yield 'error-outcome-label-replay',raw,True
    yield 'outcome-unseen',dict(kind='outcomes',capacity=4,operations=[dict(op='begin',id=1,now=0,wait=10,window=5),dict(op='execute',id=1,now=1),dict(op='report',id=1,now=10),dict(op='report',id=1,now=20)]),False
    for label,ops in [('unexecuted',[dict(op='expose',id=1,now=1)]),('unexposed',[dict(op='execute',id=1,now=1),dict(op='resolve',id=1,now=2,score=0)]),('expired',[dict(op='execute',id=1,now=10)]),('replay',[dict(op='begin',id=1,now=1,wait=10,window=5)])]:
        yield 'error-outcome-'+label,dict(kind='outcomes',capacity=4,operations=[dict(op='begin',id=1,now=0,wait=10,window=5)]+ops),True
    temporal=dict(kind='temporal',capacity=8,window=10,start=0,operations=[dict(op='observe',token=1,now=1,expires=20,kind='visit',value=8),dict(op='observe',token=2,now=2,expires=20,kind='rest',value=5),dict(op='observe',token=3,now=3,expires=20,kind='visit',value=2),dict(op='features',now=3,kind='visit'),dict(op='sequence',now=3,kinds=['visit','rest','visit']),dict(op='features',now=11,kind='visit'),dict(op='availability',now=12,available=False),dict(op='features',now=12,kind='visit'),dict(op='availability',now=13,available=True),dict(op='sequence',now=13,kinds=['visit'])])
    yield 'temporal-windows',temporal,False
    for label,op in [('replay',dict(op='observe',token=3,now=4,expires=20,kind='visit',value=1)),('sequence',dict(op='sequence',now=4,kinds=[])),('clock',dict(op='features',now=0,kind='visit'))]:
        raw=copy.deepcopy(temporal);raw['operations']=raw['operations'][:3]+[op];yield 'error-temporal-'+label,raw,True
    ledger=dict(kind='ledger',limit=100,window=10,operations=[dict(op='reserve',now=0,amount=60),dict(op='reserve',now=0,amount=30),dict(op='dispatch',now=1,id=1),dict(op='cancel',now=1,id=2),dict(op='finish',now=2,id=1,actual=40),dict(op='summary',now=2),dict(op='reserve',now=3,amount=60),dict(op='recover',now=4),dict(op='summary',now=4),dict(op='summary',now=14)])
    yield 'ledger-concurrent-recovery',ledger,False
    yield 'ledger-overrun',dict(kind='ledger',limit=100,window=10,operations=[dict(op='reserve',now=0,amount=20),dict(op='dispatch',now=1,id=1),dict(op='finish',now=2,id=1,actual=30),dict(op='summary',now=2)]),False
    yield 'ledger-free-after-overrun',dict(kind='ledger',limit=100,window=10,operations=[dict(op='reserve',now=0,amount=20),dict(op='dispatch',now=1,id=1),dict(op='finish',now=2,id=1,actual=150),dict(op='reserve',now=3,amount=0),dict(op='summary',now=3)]),False
    for label,ops in [('cap',[dict(op='reserve',now=0,amount=101)]),('refund-dispatched',[dict(op='reserve',now=0,amount=20),dict(op='dispatch',now=1,id=1),dict(op='cancel',now=2,id=1)]),('finish-unsent',[dict(op='reserve',now=0,amount=20),dict(op='finish',now=1,id=1,actual=0)]),('overrun-block',[dict(op='reserve',now=0,amount=20),dict(op='dispatch',now=1,id=1),dict(op='finish',now=2,id=1,actual=30),dict(op='reserve',now=3,amount=1)])]:yield 'error-ledger-'+label,dict(kind='ledger',limit=100,window=10,operations=ops),True
    recipe=dict(id='rest_when_ready',action='rest',args=dict(duration=2),requires=[dict(reference=ref,value=True)],expires=20)
    play=dict(kind='playbook',capacity=8,facts=[fact('ready',True)],recipes=[recipe],operations=[dict(op='match',now=1,allowed=['rest'],denied=[]),dict(op='match',now=1,allowed=[],denied=[]),dict(op='register',request=fact('ready',False,1)),dict(op='match',now=1,allowed=['rest'],denied=[])])
    yield 'playbook-current-preconditions',play,False
    raw=copy.deepcopy(play);raw['recipes'][0]['requires']=[];yield 'error-playbook-no-premise',raw,True
    raw=copy.deepcopy(play);raw['recipes'][0]['args']='arbitrary command';yield 'error-playbook-arguments',raw,True
    req=dict(kind='requests',capacity=8,facts=[fact('ready',True)],operations=[dict(op='issue',now=1,ttl=10,context=copy.deepcopy(ctx)),dict(op='accept',now=2,id=1,context=copy.deepcopy(ctx),denied=[])])
    yield 'request-current',req,False
    for label,change in [('permissions',dict(permissions=2)),('schema',dict(schema=2)),('actor',dict(actor='actor_b')),('world',dict(world='world_b'))]:
        raw=copy.deepcopy(req);raw['operations'][1]['context'].update(change);yield 'request-reject-'+label,raw,False
    raw=copy.deepcopy(req);raw['operations'][1]['now']=11;yield 'request-expired',raw,False
    raw=copy.deepcopy(req);raw['operations'].insert(1,dict(op='register',request=fact('ready',False,1)));yield 'request-stale-fact',raw,False
    raw=copy.deepcopy(req);raw['operations'].append(copy.deepcopy(raw['operations'][1]));yield 'error-request-replay',raw,True
    raw=copy.deepcopy(req);raw['operations'][0]['ttl']=0;yield 'error-request-ttl',raw,True
    perception=dict(version=1,inputs=['danger'],outputs=['alert'],nodes=[dict(id='alert',weights=dict(danger=1),bias=0,pinned=False,learning_limit=1)])
    decision=dict(version=1,inputs=['alert','cost'],outputs=['wait'],nodes=[dict(id='wait',weights=dict(alert=1,cost=-.5),bias=.1,pinned=False,learning_limit=1)])
    bundle=dict(kind='bundle',perception=perception,decision=decision,ports=[dict(input='alert',output='alert')],operations=[dict(op='prepare',scope='actor_a',frame=1,inputs=dict(danger=.8)),dict(op='run',scope='actor_a',frame=1,inputs=dict(cost=.2),allowed=['wait']),dict(op='run',scope='actor_a',frame=1,inputs=dict(cost=.9),allowed=['wait'])])
    yield 'bundle-reuse',bundle,False
    for label in ['scope','frame','ports','inputs']:
        raw=copy.deepcopy(bundle)
        if label=='scope':raw['operations'][1]['scope']='actor_b'
        elif label=='frame':raw['operations'][1]['frame']=2
        elif label=='ports':raw['ports'][0]['output']='missing'
        else:raw['operations'][1]['inputs']['alert']=1
        yield 'error-bundle-'+label,raw,True


def generated():
    for seed in (9101,9102,9103):
        rng=random.Random(seed)
        for n in range(20):
            records=[record(i+1,f"Stored entry marker_{rng.randrange(100000):05} near room_{i%7}.") for i in range(48)]
            yield dict(kind='recall',records=records,config=dict(normalize=bool(n%2),aliases=[]),queries=[dict(text=f'room_{n%7}',count=n%9),dict(text='absent_marker',count=3)])
            value=rng.randrange(1000001)
            yield dict(kind='outcomes',capacity=4,operations=[dict(op='begin',id=1,now=0,wait=10,window=5),dict(op='execute',id=1,now=1),dict(op='expose',id=1,now=2),dict(op='resolve',id=1,now=3,score=value),dict(op='take',id=1,now=4),dict(op='report',id=1,now=4)])
            yield dict(kind='temporal',capacity=16,window=8,start=0,operations=[dict(op='observe',token=i+1,now=i,expires=15,kind=('visit' if i%2 else 'rest'),value=rng.randrange(-100,101)) for i in range(12)]+[dict(op='features',now=12,kind='visit'),dict(op='sequence',now=12,kinds=['rest','visit'])])
            yield dict(kind='ledger',limit=1000,window=10,operations=[dict(op='reserve',now=0,amount=500),dict(op='dispatch',now=1,id=1),dict(op='finish',now=2,id=1,actual=rng.randrange(500)),dict(op='reserve',now=3,amount=500),dict(op='recover',now=4),dict(op='summary',now=4)])
        perception=b.dense(['signal'],['alert'],4,seed);decision=b.dense(['alert','cost'],['wait'],4,seed+1);ops=[]
        for frame in range(100):
            ops.append(dict(op='prepare',scope='actor_a',frame=frame,inputs=dict(signal=None if frame%17==0 else (frame*7919%1000001)/b.S)))
            for candidate in range(12):ops.append(dict(op='run',scope='actor_a',frame=frame,inputs=dict(cost=(candidate*7919%1000001)/b.S),allowed=['wait']))
        yield dict(kind='bundle',perception=perception.definition(),decision=decision.definition(),ports=[dict(input='alert',output='alert')],operations=ops)


def measure():
    records=[record(n+1,f'Archive entry marker_{n:04}.') for n in range(1000)]
    index=Recall(records,dict(normalize=True,aliases=[]));queries=[dict(text=f'marker_{n:04}',count=1) for n in range(100)]
    visits=sum(index.search(q['text'],q['count'])['postings'] for q in queries)
    variants=Recall([record(1,'Lighting by the door.'),record(2,'A medic waits.')],dict(normalize=True,aliases=[['healer','medic']]))
    successful=unknown=replays=0
    for n in range(100):
        trial=Outcomes(1);trial.op(dict(op='begin',id=1,now=0,wait=10,window=5));trial.op(dict(op='execute',id=1,now=1))
        unknown+=trial.op(dict(op='report',id=1,now=10))['trainable']
        trial=Outcomes(1)
        for op in [dict(op='begin',id=1,now=0,wait=10,window=5),dict(op='execute',id=1,now=1),dict(op='expose',id=1,now=2),dict(op='resolve',id=1,now=3,score=n*10000),dict(op='take',id=1,now=4)]:trial.op(op)
        successful+=1
        try:trial.op(dict(op='take',id=1,now=4))
        except ValueError:pass
        else:replays+=1
    recipe=dict(id='rest_when_ready',action='rest',args=dict(duration=2),requires=[dict(reference=dict(scope='world',key='ready',revision=1),value=True)],expires=None)
    procedures=process(dict(kind='playbook',capacity=8,facts=[fact('ready',True)],recipes=[recipe],operations=[dict(op='match',now=1,allowed=['rest'],denied=[]) for _ in range(100)]))
    return dict(api_spend_usd=0,recall=dict(records=1000,queries=100,scan_record_visits=100000,index_posting_visits=visits,normalized_alias_hits=sum(bool(variants.search(q,1)['records']) for q in ['light','healer'])),bundle=dict(frames=100,candidates_per_frame=12,naive_module_evaluations=2400,shared_module_evaluations=1300),outcomes=dict(observed_labels=successful,unexposed_trainable=unknown,replay_labels=replays),playbook=dict(repeated_matches=sum(bool(p) for p in procedures),checks=100),limitations='Deterministic operation counts and integrity fixtures, not LLM accuracy, token usage, causal learning or measured frame times.')


def main():
    if '--fixtures' in sys.argv:
        for label,v,error in fixtures():
            folder=b.ROOT/'conformance/upgrades'/label;folder.mkdir(parents=True,exist_ok=True);(folder/'case.json').write_text(dump(dict(op='upgrades',**({'error':True} if error else {})))+'\n');(folder/'input.json').write_text(dump(v)+'\n')
            try:result=process(v)
            except (ValueError,KeyError,StopIteration,b.BrainError):
                if not error:raise
            else:
                if error:raise AssertionError(label)
                (folder/'expected.json').write_text(dump(result)+'\n')
        print('Wrote',len(list(fixtures())),'prototype cases')
    elif '--check' in sys.argv:
        checked=0
        for label,v,error in fixtures():
            p=subprocess.run([str(b.ROOT/'target/upgrades-check')],input=dump(v),text=True,capture_output=True)
            if error:assert p.returncode!=0,(label,p.stdout)
            else:assert p.returncode==0 and p.stdout.strip()==dump(process(v)),(label,p.stderr,p.stdout,dump(process(v)))
            checked+=1
        for v in generated():
            p=subprocess.run([str(b.ROOT/'target/upgrades-check')],input=dump(v),text=True,capture_output=True)
            assert p.returncode==0 and p.stdout.strip()==dump(process(v)),(v['kind'],p.stderr,p.stdout[:500])
            checked+=1
        print(checked,'upgrade workflows byte-identical; includes 3600 candidate predictions across seeds 9101/9102/9103')
    else:
        result=measure();(b.ROOT/'eval/runtime/upgrades-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
