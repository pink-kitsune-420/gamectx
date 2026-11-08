#!/usr/bin/env python3
"""Offline reference for versioned bindings, diagnostics/replay, scoped memory and stability."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
import json
import math
from pathlib import Path
import random
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
import brain_eval as b
import query_workflow_eval as q
import runtime_eval as r


def fields(v,keys):
    if not isinstance(v,dict) or set(v)!=set(keys):raise ValueError('fields')


def short(v,limit=64):
    if not isinstance(v,str) or not v or len(v)>limit or any(ord(c)<32 for c in v):raise ValueError('text')
    return v


def physical(v):
    if isinstance(v,bool) or not isinstance(v,(int,float)) or not math.isfinite(v) or abs(v)>1_000_000_000:raise ValueError('physical number')
    return math.floor(v*b.S+.5) if v>=0 else -math.floor(-v*b.S+.5)


class Schema:
    def __init__(self,v):
        fields(v,('version','id','revision','features'))
        b.integer(v['version'],1,1);b.name(v['id']);b.integer(v['revision'],1,1_000_000)
        if not isinstance(v['features'],list) or not 1<=len(v['features'])<=16:raise ValueError('features')
        self.definition=copy.deepcopy(v);self.features=[];names=[]
        for f in v['features']:
            fields(f,('name','unit','min','max','invert','missing','outside'))
            b.name(f['name']);short(f['unit'],32)
            lo,hi=physical(f['min']),physical(f['max'])
            if lo>=hi or type(f['invert']) is not bool or f['name'] in names or f['missing'] not in ('unknown','error') or f['outside'] not in ('unknown','error','clamp'):raise ValueError('feature')
            names.append(f['name']);self.features.append((f,lo,hi))
            f2=self.definition['features'][len(names)-1];f2['min']=b.number(lo);f2['max']=b.number(hi)
        self.names=names
    def normalize(self,raw):
        if not isinstance(raw,dict) or any(k not in self.names for k in raw):raise ValueError('inputs')
        out=[]
        for f,lo,hi in self.features:
            value=raw.get(f['name'])
            if value is None:
                if f['missing']=='error':raise ValueError('missing')
                out.append(None);continue
            value=physical(value)
            if not lo<=value<=hi:
                if f['outside']=='error':raise ValueError('outside')
                if f['outside']=='unknown':out.append(None);continue
                value=max(lo,min(hi,value))
            x=b.div((value-lo)*b.S,hi-lo)
            out.append(b.S-x if f['invert'] else x)
        return out
    def bind(self,brain):
        if brain.inputs!=self.names:raise ValueError('binding order')


def metadata(tables,meaning):
    out=q.schema(tables)
    if not isinstance(meaning,dict) or any(k not in tables for k in meaning):raise ValueError('metadata table')
    for table,m in meaning.items():
        fields(m,('description','columns'));short(m['description'],280)
        if not isinstance(m['columns'],dict) or len(m['columns'])>64:raise ValueError('columns')
        typed=dict(out[table]['columns'])
        for col,spec in m['columns'].items():
            if col not in typed:raise ValueError('column')
            fields(spec,('description','unit','kind','values'));short(spec['description'],280)
            if spec['unit'] is not None:short(spec['unit'],32)
            if spec['kind'] not in ('categorical','quantity','continuous','identifier'):raise ValueError('kind')
            types=set(typed[col].split('|'))-{'null'}
            if spec['kind']=='categorical' and not types<={'string'} or spec['kind'] in ('quantity','continuous') and not types<={'number'}:raise ValueError('kind/type')
            if not isinstance(spec['values'],dict) or len(spec['values'])>16 or spec['values'] and spec['kind']!='categorical':raise ValueError('values')
            for literal,text in spec['values'].items():
                short(literal,64);short(text,280)
                if not any(row[col]==literal for row in tables[table]):raise ValueError('unknown literal')
        out[table]['meaning']=copy.deepcopy(m)
    return out


def diagnose(brain,inputs,mask,low,gap):
    if len(inputs)!=len(brain.inputs) or any(x is not None and (type(x) is not int or not 0<=x<=b.S) for x in inputs) or not 0<=mask<1<<len(brain.outputs) or not 0<=low<=b.S or not 0<=gap<=b.S:raise ValueError('inference')
    values,_=brain.forward(inputs)
    scores=[values[len(brain.inputs)+i] for i in brain.output_indices]
    choice,margin=r.pick(scores,mask,low,gap)
    eligible=[x for i,x in enumerate(scores) if mask&(1<<i)]
    if not eligible:reason='no_eligible'
    elif None in eligible:reason='unknown_eligible'
    elif max(eligible)==0:reason='all_zero'
    elif eligible.count(max(eligible))>1:reason='tie'
    elif max(eligible)<=low:reason='score_threshold'
    elif margin<=gap:reason='margin_threshold'
    else:reason='chosen'
    return dict(revision=brain.revision,scores={k:None if v is None else b.number(v) for k,v in zip(brain.outputs,scores)},choice=None if choice is None else brain.outputs[choice],margin=None if margin is None else b.number(margin),reason=reason)


class Stable:
    def __init__(self,brain,margin,hold):
        b.integer(margin,0,b.S);b.integer(hold,0,1_000_000)
        self.brain,self.margin,self.hold=brain,margin,hold
        self.previous,self.tick,self.switched=None,None,0
    def run(self,frame):
        fields(frame,('tick','inputs','mask','low','gap','urgent'))
        tick=b.integer(frame['tick'],0,2**64-1)
        if type(frame['urgent']) is not bool or self.tick is not None and tick<self.tick:raise ValueError('tick/urgent')
        report=diagnose(self.brain,frame['inputs'],frame['mask'],frame['low'],frame['gap'])
        chosen=None if report['choice'] is None else self.brain.outputs.index(report['choice'])
        reason='raw';old=self.previous
        if chosen is None:reason='abstained'
        elif frame['urgent']:reason='urgent'
        elif old is not None:
            eligible=frame['mask']&(1<<old) and report['scores'][self.brain.outputs[old]] is not None and b.ticks(report['scores'][self.brain.outputs[old]],0,1)>frame['low']
            if not eligible:reason='ineligible'
            elif old!=chosen:
                gain=b.ticks(report['scores'][self.brain.outputs[chosen]],0,1)-b.ticks(report['scores'][self.brain.outputs[old]],0,1)
                if tick-self.switched<self.hold:chosen=old;reason='held_time'
                elif gain<=self.margin:chosen=old;reason='held_margin'
        if chosen!=old:self.switched=tick
        self.previous,self.tick=chosen,tick
        return dict(prediction=report,choice=None if chosen is None else self.brain.outputs[chosen],held=reason in ('held_time','held_margin'),reason=reason)


class Journal:
    def __init__(self,capacity):
        b.integer(capacity,1,1024);self.capacity=capacity;self.clock=0;self.next=1;self.records=[]
    def record(self,data):
        fields(data,('turn','entity','key','text','tags','importance','source','expires'))
        turn=b.integer(data['turn'],self.clock,2**32-1);short(data['entity']);short(data['key']);short(data['text'],280)
        b.integer(data['importance'],1,10)
        if data['source'] not in ('observed','proposed') or not isinstance(data['tags'],list) or len(data['tags'])>8:raise ValueError('provenance/tags')
        tags=[]
        for tag in data['tags']:
            short(tag,32)
            if any(c.isspace() or c==',' for c in tag):raise ValueError('tag')
            tag=tag.lstrip('#').lower()
            if not tag:raise ValueError('tag')
            if tag not in tags:tags.append(tag)
        if data['expires'] is not None:b.integer(data['expires'],turn+1,2**32-1)
        for old in self.records:
            if (old['entity'],old['key'],old['source'])==(data['entity'],data['key'],data['source']) and old['superseded'] is None:old['superseded']=turn
        row=dict(id=self.next,turn=data['turn'],entity=data['entity'],key=data['key'],text=data['text'].strip(),tags=tags,importance=data['importance'],source=data['source'],expires=data['expires'],superseded=None)
        self.records.append(row);self.next+=1;self.clock=turn
        if len(self.records)>self.capacity:
            victim=min(range(len(self.records)),key=lambda i:(not(self.records[i]['superseded'] is not None or self.records[i]['expires'] is not None and self.records[i]['expires']<=turn),self.records[i]['source']=='observed',self.records[i]['importance'],self.records[i]['turn'],self.records[i]['id']))
            self.records.pop(victim)
        return row['id']
    def recall(self,now,text,count,proposals):
        b.integer(now,self.clock,2**32-1);b.integer(count,0,64)
        rows=[row for row in self.records if row['superseded'] is None and (row['expires'] is None or now<row['expires'])]
        candidates=[row for row in rows if row['source']=='observed' or proposals and not any(o['entity']==row['entity'] and o['key']==row['key'] and o['source']=='observed' for o in rows)]
        def words(text):
            import re
            raw=[]
            for w in re.split(r'[^\w]+',text.lower()):
                if w and not(w.isascii() and w.isdigit()):
                    for t in (w,w[:-1] if len(w)>3 and w.endswith('s') else w):
                        if t not in raw:raw.append(t)
            return raw
        query=words(text);parts=[]
        for row in candidates:
            p=words(row['text'])+row['tags'];p += [w[:-1] for w in p if len(w)>3 and w.endswith('s')];parts.append(p)
        weights={w:1/sum(w in p for p in parts) if any(w in p for p in parts) else 0 for w in query}
        pairs=list(zip(candidates,parts));pairs.sort(key=lambda pair:(-sum(weights[w] for w in query if w in pair[1]),-pair[0]['importance'],-pair[0]['turn'],-pair[0]['id']))
        return [copy.deepcopy(row) for row,_ in pairs[:count]]
    def save(self):return dict(version=1,capacity=self.capacity,clock=self.clock,next_id=self.next,records=copy.deepcopy(self.records))


def replay(trace):
    fields(trace,('version','initial','events'));b.integer(trace['version'],1,1)
    if not isinstance(trace['events'],list) or len(trace['events'])>256 or len(b.dumps(trace).encode())>1_048_576:raise ValueError('trace limit')
    brain=b.Brain.restore(trace['initial']);reports=[];last_tick=0;previous=None;correct=labeled=abstentions=switches=0;latencies=[]
    for event in trace['events']:
        tick=b.integer(event['tick'],last_tick,2**32-1);last_tick=tick
        if event['op']=='state':
            fields(event,('op','tick','state'));brain=b.Brain.restore(event['state']);previous=None
            reports.append(dict(revision=brain.revision));continue
        fields(event,('op','tick','inputs','mask','low','gap','expected','elapsed_ns'))
        if event['op']!='run':raise ValueError('event')
        report=diagnose(brain,event['inputs'],event['mask'],event['low'],event['gap'])
        if event['expected'] is not None:
            fields(event['expected'],('choice',));expected=event['expected']['choice']
            if expected is not None:b.integer(expected,0,len(brain.outputs)-1)
            actual=None if report['choice'] is None else brain.outputs.index(report['choice'])
            correct+=actual==expected;labeled+=1
        if event['elapsed_ns'] is not None:latencies.append(b.integer(event['elapsed_ns'],0,10**12))
        abstentions+=report['choice'] is None
        if previous is not None and report['choice'] is not None:switches+=previous!=report['choice']
        previous=report['choice'];reports.append(report)
    latencies.sort()
    percentile=lambda p:latencies[max(0,math.ceil(len(latencies)*p)-1)] if latencies else None
    return dict(reports=reports,summary=dict(events=len(trace['events']),runs=sum(e['op']=='run' for e in trace['events']),labeled=labeled,correct=correct,abstentions=abstentions,switches=switches,latency_samples=len(latencies),p50_ns=percentile(.5),p95_ns=percentile(.95),max_ns=latencies[-1] if latencies else None),state=brain.save())


def initial(workflow):return b.Brain.restore(r.a.process({**workflow,'operations':[]})['state'])


def process(v):
    kind=v['kind']
    if kind=='evidence':
        import evidence_eval
        return evidence_eval.process(v)
    if kind=='bindings':
        schema=Schema(v['schema']);brain=initial(v['workflow']);schema.bind(brain)
        if 'expected' in v and Schema(v['expected']).definition!=schema.definition:raise ValueError('schema mismatch')
        return dict(schema=schema.definition,normalized=[schema.normalize(raw) for raw in v['inputs']],state=dict(version=1,schema=schema.definition,brain=brain.save()))
    if kind=='metadata':return metadata(v['tables'],v['metadata'])
    if kind=='diagnostics':
        brain=initial(v['workflow'])
        return [diagnose(brain,x['inputs'],x['mask'],x['low'],x['gap']) for x in v['runs']]
    if kind=='stability':
        selector=Stable(initial(v['workflow']),v['margin'],v['hold'])
        return [selector.run(frame) for frame in v['frames']]
    if kind=='replay':return replay(v['trace'])
    if kind=='journal':
        journal=Journal(v['capacity']);results=[]
        for op in v['operations']:
            if op['op']=='record':results.append(journal.record(op['data']))
            elif op['op']=='recall':results.append(journal.recall(op['now'],op['text'],op['count'],op['proposals']))
            else:raise ValueError('journal op')
        return dict(results=results,state=journal.save())
    raise ValueError('kind')


def measure():
    definition=dict(version=1,inputs=['x','y'],outputs=['a','b'],nodes=[dict(id=k,weights={source:1},bias=0,pinned=False,learning_limit=1) for k,source in [('a','x'),('b','y')]])
    rows=[]
    for seed in (5001,5002,5003):
        rng=random.Random(seed);selector=Stable(b.Brain(definition),40_000,4)
        previous_raw=previous_stable=None;raw_switches=stable_switches=urgent_errors=illegal=regret=0
        for tick in range(240):
            urgent=tick%40==0
            inputs=[800_000,200_000] if (tick//40)%2==0 else [200_000,800_000]
            if not urgent:inputs=[500_000+rng.randrange(-10_000,10_001),500_000+rng.randrange(-10_000,10_001)]
            mask=2 if 150<=tick<158 else 3
            result=selector.run(dict(tick=tick,inputs=inputs,mask=mask,low=0,gap=0,urgent=urgent))
            raw,stable=result['prediction']['choice'],result['choice']
            raw_switches+=previous_raw is not None and raw is not None and raw!=previous_raw
            stable_switches+=previous_stable is not None and stable is not None and stable!=previous_stable
            if urgent:urgent_errors+=stable!=raw
            if stable is not None:
                index=selector.brain.outputs.index(stable);illegal+=not bool(mask&(1<<index))
                regret+=max(inputs[i] for i in range(2) if mask&(1<<i))-inputs[index]
            previous_raw,previous_stable=raw,stable
        rows.append(dict(seed=seed,frames=240,raw_switches=raw_switches,stable_switches=stable_switches,urgent_errors=urgent_errors,ineligible_choices=illegal,total_utility_regret_millionths=regret))
    return dict(api_spend_usd=0,stability=rows,evidence=measure_evidence(),limitations='Synthetic noisy utilities; stability intentionally trades instantaneous utility for fewer switches. No gameplay or model accuracy claim.')



def measure_evidence():
    import evidence_eval as e
    valid=rejected=0
    for directory in sorted((b.ROOT/'conformance/integration').glob('*evidence*')):
        request=json.loads((directory/'input.json').read_text())
        error=json.loads((directory/'case.json').read_text()).get('error',False)
        try:result=e.process(request)
        except (ValueError,KeyError,TypeError,b.BrainError):
            assert error
            rejected+=1
        else:
            assert not error
            valid+=1
    repeat=json.loads((b.ROOT/'conformance/integration/evidence-repeat-does-not-inflate/input.json').read_text())
    result=e.process(repeat)
    h=result['state']['hypotheses'][0]
    assert h['confidence']==.6 and h['created']==2 and h['expires']==30
    assert len(result['state']['journal']['records'])==2
    return dict(valid_workflows=valid,rejected_invalid_workflows=rejected,observations_before=2,observations_after_repeated_hypothesis=2,confidence_before=.6,confidence_after=.6,expires_before=30,expires_after=30)


def check():
    import subprocess
    binary=str(b.ROOT/'target/integration-check');checked=0
    def compare(v):
        nonlocal checked
        try:expected=b.dumps(process(v))
        except (ValueError,KeyError,TypeError,b.BrainError,q.q.QueryError):expected=None
        p=subprocess.run([binary],input=json.dumps(v),capture_output=True,text=True)
        actual=p.stdout.strip() if p.returncode==0 else None
        if actual!=expected:raise AssertionError(f'Differs: {v}\n{expected}\n{actual}\n{p.stderr}')
        checked+=1
    for d in sorted((b.ROOT/'conformance/integration').iterdir()):compare(json.loads((d/'input.json').read_text()))
    schema=json.loads((b.ROOT/'examples/fixtures/brain/bindings.json').read_text())
    definition=dict(version=1,inputs=['x','y'],outputs=['a','b'],nodes=[dict(id=k,weights={source:1},bias=0,pinned=False,learning_limit=1) for k,source in [('a','x'),('b','y')]])
    rng=random.Random(7719)
    for seed in (5001,5002,5003):
        local=random.Random(seed);frames=[]
        for tick in range(240):
            urgent=tick%40==0
            inputs=[800_000,200_000] if (tick//40)%2==0 else [200_000,800_000]
            if not urgent:inputs=[500_000+local.randrange(-10_000,10_001),500_000+local.randrange(-10_000,10_001)]
            frames.append(dict(tick=tick,inputs=inputs,mask=2 if 150<=tick<158 else 3,low=0,gap=0,urgent=urgent))
        compare(dict(kind='stability',workflow=dict(definition=definition,operations=[]),margin=40_000,hold=4,frames=frames))
        events=[dict(op='run',tick=f['tick'],inputs=f['inputs'],mask=f['mask'],low=0,gap=0,expected=None,elapsed_ns=100+f['tick']*13) for f in frames]
        compare(dict(kind='replay',trace=dict(version=1,initial=b.Brain(definition).save(),events=events)))
    for index in range(100):
        raw=[dict(x=rng.choice([None,rng.randrange(-10_000,20_001)/1000]),y=None if index%4==0 else rng.randrange(-2000,2001)/1000) for _ in range(8)]
        compare(dict(kind='bindings',schema=schema,workflow=dict(definition=definition,operations=[]),inputs=raw))
    print(f'{checked} integration workflows byte-identical; includes 720 stability frames, 720 replay observations and 800 binding inputs')


if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='--check':check()
    else:
        result=measure();(b.ROOT/'eval/runtime/integration-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
