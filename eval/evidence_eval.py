#!/usr/bin/env python3
"""Offline reference for scope-bound evidence snapshots and tentative cited hypotheses."""
import copy
import json
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
import integration_eval as i
b=i.b


def labels(values):
    if not isinstance(values,list) or len(values)>8 or len(set(values))!=len(values):raise ValueError('privacy')
    for v in values:b.name(v)
    return list(values)


class Evidence:
    def __init__(self,subject,clock,capacity,hypotheses):
        i.short(subject);b.name(clock);b.integer(capacity,1,128);b.integer(hypotheses,1,16)
        self.subject,self.clock_id=subject,clock
        self.journal=i.Journal(capacity);self.max_hypotheses=hypotheses
        self.revision=self.last_token=self.clock=0
        self.evidence=[];self.hypotheses=[]
    def observe(self,token,record,privacy):
        b.integer(token,self.last_token+1,2**32-1);privacy=labels(privacy)
        if record['source']!='observed' or record['turn']<self.clock:raise ValueError('observation')
        ident=self.journal.record(record)
        retained={r['id'] for r in self.journal.records}
        self.evidence=[e for e in self.evidence if e['id'] in retained]
        if ident in retained:self.evidence.append(dict(id=ident,token=token,privacy=privacy,contradicted=False))
        self.hypotheses=[h for h in self.hypotheses if set(h['supports']+h['contradicts'])<=retained]
        self.last_token,self.clock=token,record['turn'];self.revision+=1
        return ident
    def active(self,ident,now,denied):
        row=next((r for r in self.journal.records if r['id']==ident),None)
        meta=next((e for e in self.evidence if e['id']==ident),None)
        return row is not None and meta is not None and not meta['contradicted'] and row['superseded'] is None and (row['expires'] is None or now<row['expires']) and not set(meta['privacy'])&set(denied)
    def snapshot(self,now,count,cap,denied):
        b.integer(now,self.clock,2**32-1);b.integer(count,0,16);b.integer(cap,1,16384);denied=labels(denied)
        candidates=[r for r in self.journal.records if self.active(r['id'],now,denied)]
        candidates.sort(key=lambda r:(-r['importance'],-r['turn'],-r['id']))
        evidence=[]
        for row in candidates[:count]:
            meta=next(e for e in self.evidence if e['id']==row['id'])
            evidence.append(dict(record=copy.deepcopy(row),token=meta['token'],privacy=meta['privacy']))
        while True:
            ids={e['record']['id'] for e in evidence}
            hypotheses=[dict(**copy.deepcopy(h),status='contested' if h['contradicts'] else 'tentative') for h in self.hypotheses if now<h['expires'] and set(h['supports']+h['contradicts'])<=ids]
            out=dict(subject=self.subject,clock=self.clock_id,revision=self.revision,now=now,evidence=evidence,hypotheses=hypotheses,citable=[dict(id=e['record']['id'],token=e['token']) for e in evidence])
            if len(b.dumps(out).encode())<=cap:return out
            if not evidence:raise ValueError('snapshot budget')
            evidence.pop()
    def propose(self,view,now,data,denied):
        i.fields(data,('id','statement','confidence','supports','contradicts','ttl'))
        b.integer(now,max(self.clock,view['now']),2**32-1);denied=labels(denied)
        if (view['subject'],view['clock'],view['revision'])!=(self.subject,self.clock_id,self.revision):raise ValueError('stale snapshot')
        b.name(data['id']);i.short(data['statement'],280)
        confidence=b.ticks(data['confidence'],0,.95)
        ttl=b.integer(data['ttl'],1,1_000_000)
        data=copy.deepcopy(data)
        data['supports'].sort();data['contradicts'].sort()
        refs=data['supports']+data['contradicts']
        if not data['supports'] or len(refs)>3 or len(set(refs))!=len(refs):raise ValueError('citations')
        for ident in refs:
            b.integer(ident,1,2**32-1)
            if ident not in [e['id'] for e in view['citable']] or not self.active(ident,now,denied):raise ValueError('unavailable evidence')
        old=next((h for h in self.hypotheses if h['id']==data['id']),None)
        old_available=old is not None and now<old['expires'] and all(self.active(r,now,[]) for r in old['supports']+old['contradicts'])
        if old_available and old['id'] not in [h['id'] for h in view['hypotheses']]:raise ValueError('hidden hypothesis')
        if confidence==0:
            if not old:raise ValueError('missing hypothesis')
            self.hypotheses.remove(old);self.revision+=1;self.clock=now;return
        privacy=sorted({tag for e in self.evidence if e['id'] in refs for tag in e['privacy']})
        expires=min(now+ttl,2**32-1,min((r['expires'] for r in self.journal.records if r['id'] in refs and r['expires'] is not None),default=2**32-1))
        if expires<=now:raise ValueError('expiry')
        created=now
        if old and sorted(old['supports']+old['contradicts'])==sorted(refs):
            if old['statement']!=data['statement']:raise ValueError('rewording needs new evidence')
            confidence=min(confidence,b.ticks(old['confidence'],0,.95));expires=min(expires,old['expires']);created=old['created']
        h=dict(id=data['id'],statement=data['statement'],confidence=b.number(confidence),created=created,expires=expires,supports=data['supports'],contradicts=data['contradicts'],privacy=privacy)
        if old:self.hypotheses.remove(old)
        elif len(self.hypotheses)==self.max_hypotheses:self.hypotheses.pop(0)
        self.hypotheses.append(h);self.revision+=1;self.clock=now
    def forget(self,denied):
        denied=labels(denied);ids={e['id'] for e in self.evidence if set(e['privacy'])&set(denied)}
        self.journal.records=[r for r in self.journal.records if r['id'] not in ids]
        self.evidence=[e for e in self.evidence if e['id'] not in ids]
        self.hypotheses=[h for h in self.hypotheses if not set(h['supports']+h['contradicts'])&ids]
        if ids:self.revision+=1
    def contradict(self,ident,now):
        b.integer(now,self.clock,2**32-1)
        meta=next((e for e in self.evidence if e['id']==ident),None)
        if meta is None:raise ValueError('unknown evidence')
        meta['contradicted']=True;self.clock=now;self.revision+=1
    def save(self):return dict(version=1,subject=self.subject,clock_id=self.clock_id,revision=self.revision,last_token=self.last_token,clock=self.clock,max_hypotheses=self.max_hypotheses,journal=self.journal.save(),evidence=copy.deepcopy(self.evidence),hypotheses=copy.deepcopy(self.hypotheses))


def process(v):
    memory=Evidence(v['subject'],v['clock'],v['capacity'],v['hypotheses'])
    views={};results=[]
    for op in v['operations']:
        if op['op']=='observe':results.append(memory.observe(op['token'],op['record'],op['privacy']))
        elif op['op']=='snapshot':
            view=memory.snapshot(op['now'],op['count'],op['bytes'],op['denied']);views[op['name']]=view;results.append(copy.deepcopy(view))
        elif op['op']=='propose':memory.propose(views[op['view']],op['now'],op['request'],op['denied']);results.append(None)
        elif op['op']=='forget':memory.forget(op['denied']);results.append(None)
        elif op['op']=='contradict':memory.contradict(op['id'],op['now']);results.append(None)
        else:raise ValueError('operation')
    return dict(results=results,state=memory.save())
