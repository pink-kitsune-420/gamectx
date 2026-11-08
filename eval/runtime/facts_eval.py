#!/usr/bin/env python3
"""Offline typed-fact registry prototype. No credentials, model calls or dependencies."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
from decimal import Decimal
import json
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
import brain_eval as b


class Number(Decimal):
    def __new__(cls,raw):
        result=super().__new__(cls,raw);result.raw=raw;return result
    def __deepcopy__(self,memo):return Number(self.raw)


def dumps(v):
    if isinstance(v,Number):return v.raw
    if isinstance(v,dict):return '{'+','.join(json.dumps(k,ensure_ascii=False)+':'+dumps(x) for k,x in v.items())+'}'
    if isinstance(v,list):return '['+','.join(dumps(x) for x in v)+']'
    return json.dumps(v,ensure_ascii=False,separators=(',',':'),allow_nan=False)


def loads(text):
    return json.loads(text,parse_float=Number,parse_constant=lambda v:(_ for _ in ()).throw(ValueError(v)))


def validate_value(value):
    count=0
    def visit(v,depth):
        nonlocal count
        count+=1
        if depth>8 or count>256:raise ValueError('value bounds')
        if isinstance(v,str) and len(v.encode())>4096:raise ValueError('string bytes')
        if isinstance(v,(int,float,Decimal)) and not isinstance(v,bool):
            raw=dumps(v)
            if len(raw)>128:raise ValueError('number length')
            exponent=int(raw.lower().split('e')[1]) if 'e' in raw.lower() else 0
            if abs(exponent)>1_000_000:raise ValueError('numeric exponent')
        if isinstance(v,list):
            if len(v)>64:raise ValueError('array count')
            for item in v:visit(item,depth+1)
        if isinstance(v,dict):
            if len(v)>64 or any(len(k.encode())>256 for k in v):raise ValueError('object count/key')
            for item in v.values():visit(item,depth+1)
    visit(value,0)
    if len(dumps(value).encode())>4096:raise ValueError('value bytes')


def name(s):
    if not isinstance(s,str) or not s or len(s)>64 or any(ord(c)<32 or 127<=ord(c)<=159 for c in s):raise ValueError('fact name')
    return s


def validate_privacy(labels,limit):
    if not isinstance(labels,list) or len(labels)>limit or any(not isinstance(p,str) for p in labels) or len(set(labels))!=len(labels):raise ValueError('privacy')
    for p in labels:b.name(p)


def equivalent(a,z):
    if isinstance(a,bool) or isinstance(z,bool):return type(a)==type(z) and a==z
    if isinstance(a,(int,float,Decimal)) and isinstance(z,(int,float,Decimal)):return Decimal(getattr(a,'raw',str(a)))==Decimal(getattr(z,'raw',str(z)))
    if type(a)!=type(z):return False
    if isinstance(a,dict):return set(a)==set(z) and all(equivalent(a[k],z[k]) for k in a)
    if isinstance(a,list):return len(a)==len(z) and all(equivalent(x,y) for x,y in zip(a,z))
    return a==z


class Facts:
    def __init__(self,capacity):
        b.integer(capacity,1,256);self.capacity=capacity;self.generation=0;self.clock=0;self.facts=[]
    def find(self,scope,key):return next((f for f in self.facts if (f['scope'],f['key'])==(scope,key)),None)
    def active(self,f,now,denied,seen=None):
        seen=set() if seen is None else seen
        if f['revision'] in seen or f['expires'] is not None and now>=f['expires'] or set(f['privacy'])&set(denied):return False
        seen=seen|{f['revision']}
        for ref in f['dependencies']:
            dep=self.find(ref['scope'],ref['key'])
            if dep is None or dep['revision']!=ref['revision'] or not self.active(dep,now,denied,seen):return False
        return True
    def reference(self,f):return {k:f[k] for k in ('scope','key','revision')}
    def register(self,v):
        b.fields(v,('scope','key','value','now','authority','pinned','expires','privacy','dependencies','expected'))
        validate_value(v['value'])
        if v['expected'] is not None:b.integer(v['expected'],1,2**31-1)
        name(v['scope']);name(v['key']);now=b.integer(v['now'],self.clock,2**32-1)
        if v['authority'] not in ('host','user') or type(v['pinned']) is not bool:raise ValueError('authority')
        if v['expires'] is not None:b.integer(v['expires'],now+1,2**32-1)
        validate_privacy(v['privacy'],8)
        if len(v['dependencies'])>8:raise ValueError('dependencies')
        old=self.find(v['scope'],v['key'])
        if old:
            if old['revision']!=v['expected'] or old['pinned'] or old['authority']=='user' and v['authority']!='user':raise ValueError('protected or stale fact')
        elif v['expected'] is not None or len(self.facts)==self.capacity:raise ValueError('creation/capacity')
        privacy=set(v['privacy']);seen=set()
        for ref in v['dependencies']:
            b.fields(ref,('scope','key','revision'));b.integer(ref['revision'],1,2**31-1);identity=(ref['scope'],ref['key'])
            if identity in seen or identity==(v['scope'],v['key']):raise ValueError('duplicate/self dependency')
            seen.add(identity);dep=self.find(*identity)
            if dep is None or dep['revision']!=ref['revision'] or not self.active(dep,now,[]):raise ValueError('unavailable dependency')
            privacy.update(dep['privacy'])
        if len(privacy)>32:raise ValueError('inherited privacy')
        candidate=copy.deepcopy(self)
        candidate.facts=[f for f in candidate.facts if f is not old and (f['scope'],f['key'])!=(v['scope'],v['key'])]
        candidate.generation+=1;candidate.clock=now
        fact=dict(scope=v['scope'],key=v['key'],revision=candidate.generation,value=copy.deepcopy(v['value']),authority=v['authority'],pinned=v['pinned'],expires=v['expires'],privacy=sorted(privacy),dependencies=copy.deepcopy(v['dependencies']))
        candidate.facts.append(fact)
        if not candidate.active(fact,now,[]):raise ValueError('cyclic replacement')
        self.__dict__=candidate.__dict__;return self.reference(fact)
    def check(self,scope,key,value,now,denied):
        name(scope);name(key);validate_privacy(denied,32)
        validate_value(value)
        b.integer(now,self.clock,2**32-1);fact=self.find(scope,key)
        if fact is None or not self.active(fact,now,denied):return dict(status='unknown',reference=None)
        return dict(status='supported' if equivalent(fact['value'],value) else 'conflicting',reference=self.reference(fact))
    def unpin(self,ref,authority):
        if authority not in ('host','user'):raise ValueError('authority')
        fact=self.find(ref['scope'],ref['key'])
        if not fact or fact['revision']!=ref['revision'] or fact['authority']=='user' and authority!='user':raise ValueError('pin authority/revision')
        self.generation+=1;fact['pinned']=False;fact['revision']=self.generation;return self.reference(fact)
    def forget(self,ref,authority):
        if authority not in ('host','user'):raise ValueError('authority')
        fact=self.find(ref['scope'],ref['key'])
        if not fact or fact['revision']!=ref['revision'] or fact['pinned'] or fact['authority']=='user' and authority!='user':raise ValueError('forget authority/revision')
        self.facts.remove(fact);self.generation+=1
    def forget_privacy(self,denied):
        validate_privacy(denied,32)
        removed=[f for f in self.facts if set(f['privacy'])&set(denied)]
        if removed:self.facts=[f for f in self.facts if f not in removed];self.generation+=1
    def packet(self,now,count,cap,denied):
        validate_privacy(denied,32)
        b.integer(now,self.clock,2**32-1);b.integer(count,0,256);b.integer(cap,1,16384)
        selected=[f for f in self.facts if self.active(f,now,denied)]
        selected.sort(key=lambda f:(not f['pinned'],f['authority']!='user',f['scope'],f['key']))
        columns=['scope','key','value','authority','pinned','revision','expires']
        total=len(selected);selected=selected[:count]
        while True:
            out=dict(generation=self.generation,columns=columns,rows=[[f[k] for k in columns] for f in selected],dropped=total-len(selected))
            if len(dumps(out).encode())<=cap:return out
            if not selected:raise ValueError('fact packet budget')
            selected.pop()
    def save(self):return dict(version=1,capacity=self.capacity,generation=self.generation,clock=self.clock,facts=copy.deepcopy(self.facts))


def process(v):
    registry=Facts(v['capacity']);results=[]
    for op in v['operations']:
        method=op['op']
        if method=='register':results.append(registry.register(op['request']))
        elif method=='check':results.append(registry.check(op['scope'],op['key'],op['value'],op['now'],op['denied']))
        elif method=='packet':results.append(registry.packet(op['now'],op['count'],op['bytes'],op['denied']))
        elif method=='unpin':results.append(registry.unpin(op['reference'],op['authority']))
        elif method=='forget':registry.forget(op['reference'],op['authority']);results.append(None)
        elif method=='forget_privacy':registry.forget_privacy(op['denied']);results.append(None)
        else:raise ValueError('fact operation')
    return dict(results=results,state=registry.save())


def check():
    import subprocess,random
    binary=str(b.ROOT/'target/facts-check');checked=0
    def compare(v):
        nonlocal checked
        try:expected=dumps(process(v))
        except (ValueError,TypeError,KeyError,b.BrainError):expected=None
        p=subprocess.run([binary],input=dumps(v),capture_output=True,text=True)
        actual=p.stdout.strip() if p.returncode==0 else None
        if actual!=expected:raise AssertionError(f'Facts differ: {v}\n{expected}\n{actual}\n{p.stderr}')
        checked+=1
    for d in sorted((b.ROOT/'conformance/facts').iterdir()):compare(loads((d/'input.json').read_text()))
    rng=random.Random(8107)
    for _ in range(100):
        value={'n':rng.randrange(-10**15,10**15),'flags':[True,False,None],'ratio':Number(str(rng.randrange(-10000,10001)/1000))}
        request=dict(scope='world',key='state',value=value,now=1,authority='host',pinned=False,expires=None,privacy=[],dependencies=[],expected=None)
        compare(dict(capacity=8,operations=[dict(op='register',request=request),dict(op='check',scope='world',key='state',value=value,now=1,denied=[]),dict(op='packet',now=1,count=8,bytes=1024,denied=[])]))
    print(f'{checked} fact workflows byte-identical, including 100 generated structured values')


def measure():
    registry=Facts(128)
    for n in range(128):registry.register(dict(scope='world',key=f'fact_{n:03}',value={'enabled':bool(n%2),'amount':n,'note':'Native authored state.'},now=1,authority='host',pinned=n<2,expires=None,privacy=[],dependencies=[],expected=None))
    hits=sum(registry.check('world',f'fact_{n:03}',registry.find('world',f'fact_{n:03}')['value'],1,[])['status']=='supported' for n in range(128))
    packet=registry.packet(1,128,1024,[])
    return dict(api_spend_usd=0,registered=128,exact_checks=hits,packet_cap=1024,packet_bytes=len(dumps(packet).encode()),retained=len(packet['rows']),dropped=packet['dropped'],limitations='Exact local assertions and structural checks, not natural-language or gameplay accuracy.')


if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='--check':check()
    else:
        result=measure();(b.ROOT/'eval/runtime/facts-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
