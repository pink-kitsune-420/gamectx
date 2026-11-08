#!/usr/bin/env python3
"""Offline reference for inference, packet assembly, scheduled/sliced work and state sync.
No credentials, APIs or dependencies. Original fixtures and held-out seeds only.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
import brain_eval as b
import brain_accuracy_eval as a


def pick(scores, allowed, low=0, gap=0):
    eligible = [scores[i] for i in range(len(scores)) if allowed & (1 << i)]
    if not eligible or None in eligible:
        return None, None
    ranked = sorted(eligible, reverse=True)
    margin = ranked[0] - (ranked[1] if len(ranked) > 1 else 0)
    choice = next(i for i, s in enumerate(scores) if allowed & (1 << i) and s == ranked[0])
    return (choice if ranked[0] > low and margin > gap else None), margin


def host_cases(brain, rows):
    samples, policies = [], []
    for row in rows:
        b.fields(row, ('inputs','targets','allowed','min_score','min_margin'))
        samples.append({'inputs':row['inputs'],'targets':row['targets']})
        allowed = row['allowed']
        if not isinstance(allowed,list) or len(allowed)>len(brain.outputs) or any(not isinstance(k,str) for k in allowed) or len(set(allowed))!=len(allowed): raise b.BrainError('allowed')
        if any(k not in brain.outputs for k in allowed): raise b.BrainError('invalid eligibility')
        policies.append((sum(1 << brain.outputs.index(k) for k in allowed),
                         b.ticks(row['min_score'],0,1), b.ticks(row['min_margin'],0,1)))
    return brain.samples(samples), policies


def host_correct(brain, samples, policies):
    correct = 0
    for (features, targets), (mask, low, gap) in zip(samples, policies):
        values, _ = brain.forward(features)
        scores = [values[len(brain.inputs)+i] for i in brain.output_indices]
        correct += pick(scores,mask,low,gap)[0] == pick(targets,mask,low,gap)[0]
    return correct


def select_host(brain, request):
    b.fields(request, ('id','expected_revision','examples','validation','epochs','rates','max_steps'))
    brain.request(request)
    examples = brain.samples(request['examples'])
    validation, policies = host_cases(brain,request['validation'])
    if any(x == v for x,_ in examples for v,_ in validation): raise b.BrainError('overlap')
    epochs = b.integer(request['epochs'],1,b.MAX['epochs'])
    rates = [b.ticks(r,0,1) for r in request['rates']]
    if not 1 <= len(rates) <= 8 or 0 in rates or len(set(rates)) != len(rates): raise b.BrainError('rates')
    p = sum(map(len,brain.weights))
    steps = (3*epochs*len(examples)*len(rates)+2*len(validation)*(len(rates)+1))*p
    if (3*epochs*len(examples)+2*len(validation))*p > b.MAX['steps'] or steps > b.integer(request['max_steps'],1,2_000_000):
        raise b.BrainError('work limit')
    before_loss, before_correct = brain.loss(validation), host_correct(brain,validation,policies)
    trials, selected, best = [], None, None
    for rate in rates:
        candidate = brain.trained(examples,epochs,rate)
        loss, correct = candidate.loss(validation), host_correct(candidate,validation,policies)
        trials.append({'rate':b.number(rate),'loss':b.number(loss),'correct':correct})
        if loss < before_loss and correct >= before_correct and (best is None or (correct,-loss) > best):
            best, selected = (correct,-loss), (rate,candidate)
    if selected: brain.retain(selected[1],request['id'])
    return {'accepted':selected is not None,'revision':brain.revision,'before_loss':b.number(before_loss),
            'after_loss':b.number(-best[1] if best else before_loss),'before_correct':before_correct,
            'after_correct':best[0] if best else before_correct,'validation_count':len(validation),
            'selected_rate':b.number(selected[0]) if selected else None,'steps':steps,'trials':trials}


class SlicedTraining:
    """Sample-granularity reference: slices preserve sample order and rounded updates."""
    def __init__(self, brain, request):
        self.original, self.candidate = copy.deepcopy(brain), copy.deepcopy(brain)
        self.request = request
        brain.request(request)
        self.samples = brain.samples(request['examples'])
        self.validation = brain.samples(request['validation'])
        self.rate = b.ticks(request['rate'],0,1)
        self.epochs = b.integer(request['epochs'],1,128)
        self.cursor = 0
    def advance(self, visits):
        if visits < 1: raise b.BrainError('slice must be positive')
        for _ in range(min(visits,self.epochs*len(self.samples)-self.cursor)):
            sample = self.samples[self.cursor % len(self.samples)]
            self.candidate = self.candidate.trained([sample],1,self.rate)
            self.cursor += 1
    def finish(self):
        if self.cursor != self.epochs*len(self.samples): raise b.BrainError('unfinished')
        return self.candidate.weights


def budget(sections, cap):
    if len({s['name'] for s in sections}) != len(sections): raise ValueError('duplicate section')
    ordered = sorted(enumerate(sections),key=lambda pair:(not pair[1].get('stable',False),pair[0]))
    encoded = {i:len(b.dumps(s['name']).encode())+1+len(b.dumps(s['value']).encode()) for i,s in ordered}
    dropped = []
    def line(s): return [s['name'],len(b.dumps(s['value']).encode()),s.get('priority',0),s.get('stable',False)]
    while True:
        size = 2+sum(encoded[i] for i,_ in ordered)+max(0,len(ordered)-1)
        if size <= cap:
            return {'packet':{s['name']:s['value'] for _,s in ordered},
                    'receipt':{'columns':['name','bytes','priority','stable'],
                               'included':[line(s) for _,s in ordered],'dropped':dropped,'total_bytes':size}}
        optional = [(s.get('priority',0),-i,n) for n,(i,s) in enumerate(ordered) if not s.get('required',False)]
        if not optional: raise ValueError('required sections exceed budget')
        _,_,n = min(optional)
        _,s = ordered.pop(n)
        dropped.append(line(s))


class Scheduler:
    def __init__(self, capacity, budget):
        if not 1 <= capacity <= 4096 or not 1 <= budget <= 2_000_000: raise ValueError('limits')
        self.capacity,self.budget = capacity,budget
        self.jobs,self.tick,self.remaining = {},None,budget
    def register(self, ident, cost, interval, max_age, phase):
        if ident in self.jobs or len(self.jobs) == self.capacity or not 1 <= cost <= self.budget or not 1 <= interval <= max_age or not 0 <= phase < max_age:
            raise ValueError('registration')
        self.jobs[ident] = dict(cost=cost,interval=interval,max_age=max_age,next=phase,last=None,dirty=True,urgent=False)
    def changed(self, ident, urgent):
        if ident not in self.jobs: raise ValueError('unknown job')
        self.jobs[ident]['dirty'] = True
        self.jobs[ident]['urgent'] |= urgent
    def poll(self, tick):
        if self.tick is not None and tick < self.tick: raise ValueError('clock moved backward')
        if self.tick != tick: self.tick,self.remaining = tick,self.budget
        eligible = []
        for ident,j in self.jobs.items():
            age = tick-j['last'] if j['last'] is not None else tick
            overdue = j['last'] is not None and age >= j['max_age']
            if j['last'] != tick and (j['urgent'] or tick >= j['next'] and (j['dirty'] or overdue)):
                eligible.append((not j['urgent'],not overdue,j['last'] if j['last'] is not None else 0,ident))
        selected = []
        for *_,ident in sorted(eligible):
            j = self.jobs[ident]
            if j['cost'] > self.remaining: continue
            self.remaining -= j['cost']
            selected.append(ident)
            j.update(last=tick,next=min(2**64-1,tick+j['interval']),dirty=False,urgent=False)
        return selected


class Publisher:
    def __init__(self, stream):
        self.stream,self.revision,self.state = stream,0,None
    def publish(self, state, acknowledged):
        if not isinstance(state,dict) or len(state)>128 or len(b.dumps(state).encode())>262144: raise ValueError('state limit')
        delta = self.state is not None and acknowledged == self.revision
        changes = {k:v for k,v in state.items() if not delta or k not in self.state or b.dumps(self.state[k]) != b.dumps(v)}
        removed = [k for k in self.state if k not in state] if delta else []
        base = self.revision if delta else None
        self.revision += 1
        self.state = copy.deepcopy(state)
        return dict(stream=self.stream,base=base,revision=self.revision,order=list(state),set=changes,remove=removed)


class Replica:
    def __init__(self, stream): self.stream,self.revision,self.state = stream,0,None
    def apply(self, message):
        if message['stream'] != self.stream or message['revision'] <= self.revision: raise ValueError('stream/revision')
        if message['base'] is None:
            if message['remove']: raise ValueError('snapshot removal')
            state = {}
        else:
            if message['base'] != self.revision or message['revision'] != self.revision+1 or self.state is None: raise ValueError('baseline')
            state = copy.deepcopy(self.state)
        if len(set(message['order'])) != len(message['order']) or len(set(message['remove'])) != len(message['remove']): raise ValueError('duplicates')
        for k in message['remove']:
            if k not in state or k in message['set']: raise ValueError('removal')
            del state[k]
        state.update(message['set'])
        if set(state) != set(message['order']): raise ValueError('order')
        state = {k:state[k] for k in message['order']}
        if len(state)>128 or len(b.dumps(state).encode())>262144: raise ValueError('state limit')
        self.state,self.revision = state,message['revision']
        return copy.deepcopy(state)


def measure():
    # Proof of sample slicing against the original training implementation.
    sliced = []
    for task in ('stealth','support','switch','trade'):
        examples,validation = b.dataset(task,11,32),b.dataset(task,23,32)
        brain = b.dense(list(examples[0]['inputs']),list(examples[0]['targets']),4,3)
        request = dict(id='slice',expected_revision=0,examples=examples,validation=validation,epochs=32,rate=.1)
        job = SlicedTraining(brain,request)
        while job.cursor < 1024: job.advance(7)
        assert job.finish() == brain.trained(brain.samples(examples),32,b.ticks(.1,0,1)).weights
        sliced.append(dict(task=task,samples=1024,slices=147,identical=True))
    scheduler = Scheduler(64,8)
    for actor in range(64): scheduler.register(actor,1,1,60,actor%8)
    calls = peak = 0
    for tick in range(240):
        if tick and tick%30 == 0:
            for actor in range(64): scheduler.changed(actor,False)
        due = scheduler.poll(tick)
        calls += len(due); peak = max(peak,len(due))
        assert scheduler.poll(tick)==[]
    pub,replica = Publisher('demo'),Replica('demo')
    full_bytes = sent_bytes = prefix_bytes = 0
    previous = ''
    for tick in range(120):
        state = {'rules':'Keep reactions consistent. '*64,'tools':'act(target): choose a legal target. '*64,'tick':tick,'mood':'wary' if tick%2 else 'calm'}
        full = b.dumps(state)
        common = 0
        for left,right in zip(previous.encode(),full.encode()):
            if left != right: break
            common += 1
        prefix_bytes += common
        message = pub.publish(state,replica.revision)
        assert b.dumps(replica.apply(message)) == full
        full_bytes += len(full.encode()); sent_bytes += len(b.dumps(message).encode()); previous=full
    return dict(api_spend_usd=0,sliced=sliced,
                scheduler=dict(actors=64,frames=240,baseline_calls=64*240,scheduled_calls=calls,peak_per_frame=peak),
                context=dict(turns=120,full_bytes=full_bytes,sync_bytes=sent_bytes,shared_prefix_bytes=prefix_bytes,
                             tokenizer_measured=False,kv_cache_measured=False))



def process(v):
    kind=v['kind']
    if kind=='infer':
        initial=a.process({**v['workflow'],'operations':[]})
        brain=b.Brain.restore(initial['state'])
        result=[]
        for row in v['runs']:
            features=row['inputs']
            if len(features)!=len(brain.inputs) or any(x is not None and (type(x) is not int or not 0<=x<=b.S) for x in features): raise ValueError('inputs')
            mask=row['mask']
            if type(mask) is not int or not 0<=mask<1<<len(brain.outputs): raise ValueError('mask')
            if not 0<=row['low']<=b.S or not 0<=row['gap']<=b.S: raise ValueError('threshold')
            values,_=brain.forward(features)
            scores=[values[len(brain.inputs)+i] for i in brain.output_indices]
            choice,margin=pick(scores,mask,row['low'],row['gap'])
            result.append(dict(revision=brain.revision,scores={k:None if x is None else b.number(x) for k,x in zip(brain.outputs,scores)},
                               choice=None if choice is None else brain.outputs[choice],margin=None if margin is None else b.number(margin)))
        return result
    if kind=='budget': return budget(v['sections'],v['cap'])
    if kind in ('host','job'):
        workflow=v['workflow']
        initial=a.process({**workflow,'operations':[]})
        brain=b.Brain.restore(initial['state'])
        results=[]
        for operation in workflow['operations']:
            op,args=operation['op'],operation['args']
            if op=='train_select_host': report=select_host(brain,args)
            elif op=='train_select': report=a.train_select(brain,args)
            elif op=='train': report=brain.train(args)
            else: raise ValueError('job operation')
            if kind=='job':
                cost=sum(map(len,brain.weights))*3
                if not cost<=v['slice']<=2_000_000: raise ValueError('slice budget')
            results.append(report)
        return dict(results=results,state=brain.save())
    if kind=='schedule':
        scheduler=Scheduler(v['capacity'],v['budget'])
        for j in v['jobs']: scheduler.register(j['id'],j['cost'],j['interval'],j['max_age'],j['phase'])
        results=[]
        for event in v['events']:
            if event['op']=='poll': results.append(scheduler.poll(event['tick']))
            elif event['op']=='changed': scheduler.changed(event['id'],event['urgent'])
            else: raise ValueError('event')
        return dict(results=results)
    if kind=='sync':
        pub,receiver,pending=Publisher(v['stream']),Replica(v['stream']),None
        if not v['stream'] or len(v['stream'])>64 or any(not(c.isascii() and (c.isalnum() or c in '_-')) for c in v['stream']): raise ValueError('stream')
        results=[]
        for op in v['operations']:
            if op['op']=='publish':
                pending=pub.publish(op['state'],op['ack']); results.append(pending)
            elif op['op']=='deliver':results.append(receiver.apply(pending))
            elif op['op']=='apply':results.append(receiver.apply(op['message']))
            else: raise ValueError('operation')
        return dict(results=results,publisher_revision=pub.revision,receiver_revision=receiver.revision,state=receiver.state)
    raise ValueError('kind')


def policies(rows):
    out=[]
    for i,row in enumerate(rows):
        keys=list(row['targets'])
        allowed=[] if i%7==0 else keys[:1] if i%7==1 else keys[-1:] if i%7==2 else keys
        out.append({**row,'allowed':allowed,'min_score':(0,.2,.5)[i%3],'min_margin':(0,.05,.15)[(i//3)%3]})
    return out


def measure_host():
    result=[]
    for task in ('stealth','support','switch','trade'):
        examples=b.dataset(task,11,32)
        validation=policies(b.dataset(task,23,128))
        tests=policies(sum((b.dataset(task,seed,128) for seed in (3001,3002,3003)),[]))
        brain=b.dense(list(examples[0]['inputs']),list(examples[0]['targets']),4,3)
        legacy,host=copy.deepcopy(brain),copy.deepcopy(brain)
        common=dict(id='host_training',expected_revision=0,examples=examples,epochs=128,rates=[.03,.1,.3],max_steps=2_000_000)
        legacy_report=a.train_select(legacy,{**common,'validation':b.dataset(task,23,128)})
        host_report=select_host(host,{**common,'validation':validation})
        cohorts=[host_cases(host,tests[i:i+128]) for i in range(0,len(tests),128)]
        result.append(dict(task=task,tests=len(tests),legacy_correct=sum(host_correct(legacy,s,c) for s,c in cohorts),
                           host_correct=sum(host_correct(host,s,c) for s,c in cohorts),legacy_rate=legacy_report['selected_rate'],
                           host_rate=host_report['selected_rate'],before_validation=host_report['before_correct'],
                           after_validation=host_report['after_correct']))
    return result


def check(binary):
    import random,subprocess
    checked=0
    fixtures=sorted((b.ROOT/'conformance/runtime').iterdir())
    def compare(request):
        nonlocal checked
        try: expected=b.dumps(process(request))
        except (ValueError,b.BrainError,KeyError,TypeError): expected=None
        p=subprocess.run([binary],input=json.dumps(request),capture_output=True,text=True)
        actual=p.stdout.strip() if p.returncode==0 else None
        if actual!=expected: raise AssertionError(f'Runtime differs: {request}\n{expected}\n{actual}\n{p.stderr}')
        checked+=1
    for directory in fixtures: compare(json.loads((directory/'input.json').read_text()))
    rng=random.Random(9137)
    for seed in (31,73,109):
        workflow=dict(dense=dict(inputs=['x','y'],outputs=['left','right'],hidden=4,seed=seed),operations=[])
        runs=[dict(inputs=[rng.choice([None,rng.randrange(b.S+1)]),rng.randrange(b.S+1)],mask=rng.randrange(4),low=rng.randrange(b.S+1),gap=rng.randrange(b.S+1)) for _ in range(256)]
        compare(dict(kind='infer',workflow=workflow,runs=runs))
    for _ in range(100):
        sections=[dict(name=f's{i}',priority=rng.randrange(-3,4),stable=bool(rng.randrange(2)),required=False,value=('é\"' if i%2 else 'x')*rng.randrange(1,50)) for i in range(rng.randrange(1,32))]
        compare(dict(kind='budget',sections=sections,cap=rng.randrange(2,1000)))
    for task in ('stealth','support','switch','trade'):
        examples=b.dataset(task,11,32)
        brain=b.dense(list(examples[0]['inputs']),list(examples[0]['targets']),4,3)
        request=dict(id='host_training',expected_revision=0,examples=examples,validation=policies(b.dataset(task,23,128)),epochs=128,rates=[.03,.1,.3],max_steps=2_000_000)
        workflow=dict(definition=brain.definition(),operations=[dict(op='train_select_host',args=request)])
        compare(dict(kind='host',workflow=workflow))
        result=process(dict(kind='host',workflow=workflow))
        state=result['state']
        runs=[]
        for row in policies(sum((b.dataset(task,seed,128) for seed in (3001,3002,3003)),[])):
            runs.append(dict(inputs=[b.ticks(row['inputs'][key],0,1) for key in brain.inputs],
                             mask=sum(1<<brain.outputs.index(key) for key in row['allowed']),
                             low=b.ticks(row['min_score'],0,1),gap=b.ticks(row['min_margin'],0,1)))
        compare(dict(kind='infer',workflow=dict(state=state,operations=[]),runs=runs))
    events=[]
    for tick in range(240):
        if tick and tick%30==0:
            events.extend(dict(op='changed',id=i,urgent=False) for i in range(64))
        events.extend([dict(op='poll',tick=tick),dict(op='poll',tick=tick)])
    compare(dict(kind='schedule',capacity=64,budget=8,jobs=[dict(id=i,cost=1,interval=1,max_age=60,phase=i%8) for i in range(64)],events=events))
    operations=[]
    for tick in range(120):
        state={'rules':'Keep reactions consistent. '*64,'tools':'act(target): choose a legal target. '*64,'tick':tick,'mood':'wary' if tick%2 else 'calm'}
        operations.extend([dict(op='publish',state=state,ack=tick),dict(op='deliver')])
    compare(dict(kind='sync',stream='demo',operations=operations))
    print(f'{checked} runtime workflows byte-identical; includes 2304 native predictions and 100 generated budgets')


if __name__ == '__main__':
    if len(sys.argv)>1 and sys.argv[1]=='--check':
        check(str(b.ROOT/'target/runtime-check'))
    else:
        result=measure()
        result['host_validation']=measure_host()
        (b.ROOT/'eval/runtime/runtime-results.json').write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps(result,indent=2))
