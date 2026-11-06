#!/usr/bin/env python3
"""Independent offline reference for additive local-model contracts. No network access."""
import json

def encode(v):
    return json.dumps(v,ensure_ascii=False,separators=(',', ':'),allow_nan=False)
def need(ok):
    if not ok: raise ValueError('invalid local value')
def fields(v, keys):
    need(isinstance(v,dict) and set(v)==set(keys))
def uint(v, hi=1048576, lo=0):
    need(type(v) is int and lo <= v <= hi)
    return v
def text(v, cap=64):
    need(isinstance(v,str) and 0<len(v)<=cap and not any(ord(c)<32 or 127<=ord(c)<=159 for c in v))
    if cap==64:need(len(v.encode())<=256)
    return v
def budget(v, count):
    fields(v,['context','output','safety'])
    ctx=uint(v['context'],lo=1); out=uint(v['output']); safety=uint(v['safety'])
    need(out+safety<=ctx)
    fields(count,['input','method'])
    n=uint(count['input']); need(count['method'] in ['exact','estimate'])
    used=n+out+safety
    return {'context':ctx,'input':n,'reserved_output':out,'safety':safety,'remaining':max(0,ctx-used),'overflow':max(0,used-ctx),'method':count['method'],'fits':used<=ctx}

def thread(v):
    fields(v,['version','scene','conversation','participants','exchanges'])
    uint(v['version'],1,1); text(v['scene']); text(v['conversation'])
    need(isinstance(v['participants'],list) and 0<len(v['participants'])<=16 and len(set(v['participants']))==len(v['participants']))
    for p in v['participants']: text(p)
    need(isinstance(v['exchanges'],list) and len(v['exchanges'])<=128 and len(encode(v).encode())<=262144)
    ids=set(); previous=-1
    for e in v['exchanges']:
        fields(e,['id','turn','user','assistant']); text(e['id']); uint(e['turn'],4294967295)
        need(e['id'] not in ids and e['turn']>previous); ids.add(e['id']); previous=e['turn']
        for k in ['user','assistant']: need(isinstance(e[k],str) and 0<len(e[k].encode())<=4096)
    return v

def dialogue(v,current,protect,maximum,cap):
    thread(v); text(current); uint(protect,3); uint(maximum,128); need(protect<=maximum)
    available=[e for e in v['exchanges'] if e['id']!=current]
    selected=available[-maximum:] if maximum else []
    def messages(rows):
        return [m for e in rows for m in [{'role':'user','content':e['user']},{'role':'assistant','content':e['assistant']}]]
    while selected and len(encode({'messages':messages(selected)}).encode())>cap:
        need(len(selected)>min(protect,len(available)))
        selected.pop(0)
    ids=[e['id'] for e in selected]
    return {'messages':messages(selected),'kept':ids,'omitted':[e['id'] for e in v['exchanges'] if e['id'] not in ids]}

def scene(v,a):
    fields(v,['version','id','rules','current','outcome','required_facts','dialogue','optional'])
    uint(v['version'],1,1); text(v['id']); need(isinstance(v['rules'],str) and 0<len(v['rules'].encode())<=8192)
    fields(v['current'],['id','text']); text(v['current']['id']); need(isinstance(v['current']['text'],str) and 0<len(v['current']['text'].encode())<=8192)
    thread(v['dialogue']); need(v['dialogue']['scene']==v['id'])
    need(isinstance(v['required_facts'],dict) and len(v['required_facts'])<=64)
    need(isinstance(v['optional'],list) and len(v['optional'])<=32)
    names={'rules','scene','facts','outcome','dialogue','input'}
    for x in v['optional']:
        fields(x,['name','priority','value']); text(x['name']); need(x['name'] not in names); names.add(x['name'])
        need(type(x['priority']) is int and abs(x['priority'])<=1000000)
    d=dialogue(v['dialogue'],v['current']['id'],a['protect'],a['maximum'],a['dialogue_tokens'])
    sections=[('rules',v['rules'],1000001,True,True),('scene',v['id'],1000001,False,True),('facts',v['required_facts'],1000001,False,True)]
    sections += [(x['name'],x['value'],x['priority'],False,False) for x in v['optional']]
    sections += [('outcome',v['outcome'],1000001,False,True),('dialogue',d['messages'],1000001,False,True),('input',v['current'],1000001,False,True)]
    kept=list(enumerate(sections)); dropped=[]
    def request(packet):
        data={k:x for k,x in packet.items() if k not in ['rules','dialogue']}
        return {'messages':[{'role':'system','content':packet['rules']},*packet['dialogue'],{'role':'user','content':encode(data)}]}
    while True:
        packet={x[0]:x[1] for i,x in kept}; req=request(packet)
        receipt=budget(a['budget'],{'input':len(encode(req).encode()),'method':'estimate'})
        if receipt['fits']: break
        optional=[ix for ix in kept if not ix[1][4]]
        if not optional:
            need(len(d['kept'])>a['protect'])
            d=dialogue(v['dialogue'],v['current']['id'],a['protect'],len(d['kept'])-1,a['dialogue_tokens'])
            kept=[(i,(x[0],d['messages'],x[2],x[3],x[4]) if x[0]=='dialogue' else x) for i,x in kept]
            continue
        victim=min(optional,key=lambda ix:(ix[1][2],-ix[0])); kept.remove(victim); dropped.append(victim)
    def line(ix):
        i,x=ix; return [x[0],len(encode(x[1]).encode()),x[2],x[3]]
    return {'packet':packet,'request':req,'sections':{'columns':['name','bytes','priority','stable'],'included':[line(x) for x in kept],'dropped':[line(x) for x in dropped],'total_bytes':len(encode(packet).encode())},'budget':receipt,'dialogue':d}

def choices(v):
    fields(v,['version','revision','choices','fallback']); uint(v['version'],1,1); uint(v['revision'],4294967295)
    need(isinstance(v['choices'],list) and 0<len(v['choices'])<=32)
    pairs=[]
    for x in v['choices']:
        fields(x,['action','target']); text(x['action'],32)
        need('a'<=x['action'][0]<='z' and all(c in 'abcdefghijklmnopqrstuvwxyz_0123456789' for c in x['action']))
        if x['target'] is not None: text(x['target'],64)
        need(x not in pairs); pairs.append(x)
    need(v['fallback'] is None or v['fallback'] in pairs)
    return v

def decision_schema(v):
    choices(v)
    actions=list(dict.fromkeys(x['action'] for x in v['choices']))
    targets=[]
    for x in v['choices']:
        if x['target'] not in targets: targets.append(x['target'])
    return {'type':'object','properties':{'action':{'type':'string','maxLength':32,'enum':actions},
        'target':{'type':['string','null'],'maxLength':64,'enum':targets}},'required':['action','target'],'additionalProperties':False,'enum':v['choices']}

def decisions(v):
    fields(v,['choices','calls']); cs=choices(v['choices']); schema=decision_schema(cs)
    need(isinstance(v['calls'],list) and len(v['calls'])<=3)
    results=[]; terminal=False
    for i,call in enumerate(v['calls']):
        need(not terminal); fields(call,['text','revision','available']); uint(call['revision'],4294967295)
        need(isinstance(call['text'],str))
        try:
            need(len(call['text'].encode())<=4096)
            def unique(pairs):
                d={}
                for k,val in pairs: need(k not in d); d[k]=val
                return d
            proposal=json.loads(call['text'],object_pairs_hook=unique,parse_constant=lambda _:need(False))
            valid=any(proposal==x for x in cs['choices'])
        except (ValueError,TypeError): valid=False; proposal=None
        stale=call['revision']!=cs['revision']
        if valid and not stale and proposal in call['available']:
            result={'status':'accepted','decision':proposal}; terminal=True
        elif not stale and i==0:
            result={'status':'retry','error':'invalid_decision','schema':schema}
        elif cs['fallback'] is not None and cs['fallback'] in call['available']:
            result={'status':'fallback','decision':cs['fallback']}; terminal=True
        else:
            result={'status':'abstain','decision':None}; terminal=True
        results.append(result)
    return {'schema':schema,'results':results,'finished':terminal}

GUIDANCE={
 'standard':'',
 'concise':'Choose only from the supplied legal action/target pairs. Use current facts and the latest player request. Return exactly one JSON object with action and target; no explanation.',
 'example':'Return one supplied action/target pair as JSON. Example shape: {"action":"inspect","target":"grove:gate"}. The example is a shape only; use a pair from the current choices, never copy this target.'}
def identity(v):
    fields(v,['model_digest','quantization','template_hash','tokenizer','runner'])
    for x in v.values(): text(x,128)
    return {k:v[k] for k in ['model_digest','quantization','template_hash','tokenizer','runner']}
def profile(v,a):
    fields(v,['version','name','identity','budget','threads','transport','guidance','temperature_milli'])
    uint(v['version'],1,1);text(v['name'],64);i=identity(v['identity']);need(i==identity(a))
    b=v['budget']; budget(b,{'input':0,'method':'estimate'})
    uint(v['threads'],64,1);uint(v['temperature_milli'],2000)
    need(v['transport'] in ['json','schema','tools','signatures'] and v['guidance'] in [*GUIDANCE,'explicit-selection'])
    guidance=GUIDANCE.get(v['guidance'])
    if guidance is None:
        guidance='Return all matching rows for a list: do not add LIMIT unless the question requests a top-N or one row. Use only filters requested by the question; field meanings do not add filters. For a top-N total use SELECT SUM(column) FROM table ORDER BY column DESC LIMIT N, substituting listed names. Never put SELECT inside SELECT or IN. Do not add eligibility filters to an all-row total.'
    return {'identity':i,'guidance':guidance,'transport':v['transport'],'options':{'num_ctx':b['context'],'num_predict':b['output'],'num_thread':v['threads'],'temperature':v['temperature_milli']//1000 if v['temperature_milli']%1000==0 else v['temperature_milli']/1000}}

def same(a,b):
    if isinstance(a,dict) and isinstance(b,dict):return set(a)==set(b) and all(same(a[k],b[k]) for k in a)
    if isinstance(a,list) and isinstance(b,list):return len(a)==len(b) and all(same(x,y) for x,y in zip(a,b))
    return encode(a)==encode(b)

def report(v):
    fields(v,['version','trials']);uint(v['version'],1,1)
    need(isinstance(v['trials'],list) and 0<len(v['trials'])<=512)
    ids=set(); groups={}
    for t in v['trials']:
        fields(t,['id','condition','expected','actual','elapsed_micros','ttft_micros','attempts','fallback','required','supplied','memory','dialogue_score'])
        text(t['id'],64);text(t['condition'],64);need(t['id'] not in ids);ids.add(t['id'])
        uint(t['elapsed_micros'],10**12)
        if t['ttft_micros'] is not None: uint(t['ttft_micros'],t['elapsed_micros'])
        need(type(t['fallback']) is bool and isinstance(t['attempts'],list) and len(t['attempts'])<=8)
        for k in ['required','supplied']:
            need(isinstance(t[k],list) and len(t[k])<=64 and len(set(t[k]))==len(t[k]))
            for x in t[k]: text(x,64)
        if t['dialogue_score'] is not None: uint(t['dialogue_score'],5)
        fields(t['memory'],['runner_rss_bytes','game_rss_bytes','available_bytes','swap_in_bytes','swap_out_bytes'])
        for x in t['memory'].values():
            if x is not None: uint(x,2**53-1)
        phase={}
        for a in t['attempts']:
            fields(a,['phase','input_tokens','output_tokens','cached_tokens','prompt_micros','decode_micros','load_micros','valid','truncated'])
            need(a['phase'] in ['decision','query','narration','preflight']);phase[a['phase']]=phase.get(a['phase'],0)+1;need(phase[a['phase']]<=(4 if a['phase']=='preflight' else 2))
            need(type(a['valid']) is bool and type(a['truncated']) is bool)
            for k in ['input_tokens','output_tokens','cached_tokens','prompt_micros','decode_micros','load_micros']:
                if a[k] is not None:uint(a[k],10**12)
        groups.setdefault(t['condition'],[]).append(t)
    def median(nums):
        if not nums:return None
        x=sorted(nums);return (x[(len(x)-1)//2]+x[len(x)//2])//2
    def total(rows,key):
        values=[a[key] for t in rows for a in t['attempts']]
        return sum(values) if values and all(v is not None for v in values) else None
    out=[]
    for name,rows in groups.items():
        elapsed=sorted(t['elapsed_micros'] for t in rows)
        ttft=[t['ttft_micros'] for t in rows if t['ttft_micros'] is not None]
        retries=0
        for t in rows:
            phases=[a['phase'] for a in t['attempts'] if a['phase']!='preflight']; retries+=len(phases)-len(set(phases))
        memory={}
        for key in ['runner_rss_bytes','game_rss_bytes','available_bytes','swap_in_bytes','swap_out_bytes']:
            values=[t['memory'][key] for t in rows]
            memory[key]=None if any(x is None for x in values) else (min(values) if key=='available_bytes' else max(values))
        out.append({'name':name,'trials':len(rows),'correct':sum(same(t['expected'],t['actual']) for t in rows),
            'invalid_attempts':sum(not a['valid'] for t in rows for a in t['attempts']),
            'retries':retries,'fallbacks':sum(t['fallback'] for t in rows),
            'truncated':sum(a['truncated'] for t in rows for a in t['attempts'] if a['phase']!='preflight'),
            'missing_required':sum(len(set(t['required'])-set(t['supplied'])) for t in rows),
            'median_turn_micros':median(elapsed),'p95_turn_micros':elapsed[(95*len(elapsed)+99)//100-1],
            'median_ttft_micros':median(ttft),'ttft_samples':len(ttft),
            **{k:total(rows,k) for k in ['input_tokens','output_tokens','cached_tokens','prompt_micros','decode_micros','load_micros']},
            'memory':memory,'dialogue_scored':sum(t['dialogue_score'] is not None for t in rows),
            'dialogue_score_total':sum(t['dialogue_score'] or 0 for t in rows)})
    return {'version':1,'conditions':out}

def preset(a):
    fields(a,['size','name','identity']);text(a['name']);i=identity(a['identity'])
    size=a['size'];need(isinstance(size,str));size=size[:-1] if size.endswith(('b','B')) else size
    settings={
        '1':('1b',1000,400,700,2048,64,4,'Classify one intent or select a supplied legal pair.'),
        '1.5':('1.5b',1500,500,900,2048,96,6,'Extract one intent and entity, or select a few prefiltered legal choices.'),
        '3':('3b',3000,800,1400,2048,256,8,'Handle one scene and a bounded decision or short grounded narration.'),
        '8':('8b',8000,1000,2200,4096,256,16,'Test richer single-scene reasoning with bounded tool use.'),
        '30':('30b',30000,1200,3200,8192,384,32,'Test short explicit multi-step plans with host validation at each step.'),
        '70':('70b',70000,1500,4800,8192,512,32,'Test broader planning or nuanced dialogue while retaining explicit state and tool bounds.')}
    need(size in settings);label,millions,low,high,ctx,out,choices,task=settings[size]
    return {'guideline':{'size_class':label,'nominal_parameter_millions':millions,'task':task,'content_tokens_min_hint':low,'content_tokens_max_hint':high,'choices_hint':choices,'context_start':ctx,'output_start':out,'measured':False,
        'host_work':'Filter candidates, compute arithmetic, enforce permissions and resolve state changes locally.'},
        'profile':{'version':1,'name':a['name'],'identity':i,'budget':{'context':ctx,'output':out,'safety':256},'threads':4,'transport':'schema','guidance':'standard','temperature_milli':0}}

def process(v):
    fields(v,['kind','args'])
    a=v['args']
    if v['kind']=='budget':
        fields(a,['budget','count'])
        return budget(a['budget'],a['count'])
    if v['kind']=='dialogue':
        fields(a,['thread','current','protect','maximum','tokens'])
        return dialogue(a['thread'],a['current'],a['protect'],a['maximum'],a['tokens'])
    if v['kind']=='scene':
        fields(a,['scene','protect','maximum','dialogue_tokens','budget'])
        return scene(a['scene'],a)
    if v['kind']=='cancel':
        fields(a,['choices','available']);cs=choices(a['choices'])
        need(isinstance(a['available'],list) and len(a['available'])<=32)
        return {'status':'fallback','decision':cs['fallback']} if cs['fallback'] is not None and cs['fallback'] in a['available'] else {'status':'abstain','decision':None}
    if v['kind']=='decision': return decisions(a)
    if v['kind']=='preset': return preset(a)
    if v['kind']=='profile':
        fields(a,['profile','actual_identity']); return profile(a['profile'],a['actual_identity'])
    if v['kind']=='report': return report(a)
    raise ValueError('unknown operation')

if __name__=='__main__':
    for n in [0,1400,1528,1529,2048]:
        print(encode(budget({'context':2048,'output':256,'safety':264},{'input':n,'method':'exact'})))
