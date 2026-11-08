"""Original fixtures and independent comparisons for optimization contracts."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
import json
import pathlib
import subprocess
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import optimization_reference as ref
ROOT=pathlib.Path(__file__).resolve().parents[2]

def intent_args():
    pairs=[{'action':'inspect','target':'grove:gate'},{'action':'open','target':'grove:gate'},{'action':'wait','target':None}]
    return {'choices':{'version':1,'revision':7,'choices':pairs,'fallback':pairs[-1]},
        'intent':{'request':{'id':'r7','text':'Inspect the gate; do not open it.'},'facts':{'locked':True},'allowed':copy.deepcopy([pairs[0],pairs[-1]])},
        'calls':[{'text':ref.encode(pairs[1]),'revision':7,'available':pairs},{'text':ref.encode(pairs[0]),'revision':7,'available':pairs}]}

def cases():
    out=[]
    def add(n,k,a,error=False):out.append((n,{'kind':k,'args':copy.deepcopy(a)},error))
    a=intent_args();add('intent_repair','intent',a)
    x=copy.deepcopy(a);x['calls'][1]['text']=x['calls'][0]['text'];add('intent_repeated_wrong','intent',x)
    x=copy.deepcopy(a);x['intent']['allowed']=x['intent']['allowed'][:1];x['calls'][1]['text']='{}';add('intent_no_allowed_fallback','intent',x)
    x=copy.deepcopy(a);x['calls']=x['calls'][:1];x['calls'][0]['revision']=8;add('intent_stale','intent',x)
    x=copy.deepcopy(a);x['intent']['allowed']=[];add('intent_empty','intent',x,True)
    x=copy.deepcopy(a);x['intent']['allowed']*=2;add('intent_duplicate','intent',x,True)
    x=copy.deepcopy(a);x['intent']['allowed'][0]['target']='foreign';add('intent_foreign','intent',x,True)
    x=copy.deepcopy(a);x['intent']['request']['text']='x'*8193;add('intent_long','intent',x,True)
    for i in range(30):
        x=copy.deepcopy(a)
        for p in x['choices']['choices']+x['intent']['allowed']:
            if p['target'] is not None:p['target']='grove:gate_'+str(i)
        for j,c in enumerate(x['calls']):c['text']=ref.encode(x['choices']['choices'][1 if j==0 else 0]);c['available']=x['choices']['choices']
        add('intent_generated_'+str(i),'intent',x)
    template={'name':'count_kind','description':'Count visible items of one kind.','sql':'SELECT COUNT(*) FROM stock',
        'parameters':[{'name':'kind','column':'kind','op':'=','kind':'string'}],'max_rows':4096,'result_bytes':512}
    a={'template':template,'arguments':{'kind':'food'},'rows':[{'id':'a','kind':'food','count':4},{'id':'b','kind':'tool','count':2}]}
    add('template_count','query-template',a)
    x=copy.deepcopy(a);x['arguments']['kind']="food' OR kind = 'tool";add('template_injection','query-template',x)
    x=copy.deepcopy(a);x['arguments']['kind']='雪:food.v2';x['rows'][0]['kind']='雪:food.v2';add('template_unicode','query-template',x)
    x=copy.deepcopy(a);x['template']['sql']='SELECT SUM(count) FROM stock ORDER BY count DESC LIMIT 1';add('template_aggregate','query-template',x)
    x=copy.deepcopy(a);x['template']['parameters'][0]['op']='in';x['arguments']['kind']=['food','tool'];add('template_in','query-template',x)
    for n,key,val in [('missing','arguments',{}),('extra','arguments',{'kind':'food','secret':True}),('type','arguments',{'kind':1})]:
        x=copy.deepcopy(a);x[key]=val;add('template_'+n,'query-template',x,True)
    x=copy.deepcopy(a);x['template']['max_rows']=1;add('template_row_bound','query-template',x,True)
    x=copy.deepcopy(a);x['template']['result_bytes']=0;add('template_result_bound','query-template',x,True)
    for i in range(40):
        x=copy.deepcopy(a);x['rows']=[{'id':str(j),'kind':'food' if j%3 else 'tool','count':j} for j in range(i)]
        add('template_generated_'+str(i),'query-template',x)
    records=[{'id':'item','value':'grove:coin','depends':['owner','price'],'priority':10,'required':True},
        {'id':'owner','value':'player','depends':[],'priority':0,'required':False},
        {'id':'price','value':12,'depends':[],'priority':0,'required':False},
        {'id':'lore','value':'old tale '*40,'depends':[],'priority':1,'required':False}]
    a={'records':records,'max_bytes':100};add('evidence_group','evidence',a)
    x=copy.deepcopy(a);x['max_bytes']=20;add('evidence_required_overflow','evidence',x,True)
    x=copy.deepcopy(a);x['records'][0]['required']=False;add('evidence_optional_group','evidence',x)
    x=copy.deepcopy(a);x['records'][0]['depends']=['missing'];add('evidence_missing','evidence',x,True)
    x=copy.deepcopy(a);x['records'][1]['depends']=['item'];add('evidence_cycle','evidence',x,True)
    x=copy.deepcopy(a);x['records'][1]['depends']=['price'];add('evidence_transitive','evidence',x)
    for i in range(40):
        x=copy.deepcopy(a);x['records'][0]['required']=False;x['max_bytes']=2+i*5
        add('evidence_generated_'+str(i),'evidence',x)
    import local_cases
    definition={'id':'request_1','identity':local_cases.identity(),'sections':[
        {'id':'facts','value':{'gate_locked':True},'priority':10,'required':True},
        {'id':'lore','value':'old tale '*50,'priority':0,'required':False}],
        'budget':{'context':2048,'output':96,'safety':256}}
    a={'definition':definition,'observations':[]};add('compose_begin','compose',a)
    def observe(x,n,method='exact'):
        state=ref.compose(x);x['observations'].append({'sequence':state['sequence'],'candidate':copy.deepcopy(state['candidate']),'count':{'input':n,'method':method}})
    x=copy.deepcopy(a);observe(x,1000);add('compose_ready','compose',x)
    x=copy.deepcopy(a);observe(x,1800);add('compose_drop','compose',x)
    observe(x,500);add('compose_recount','compose',x)
    x=copy.deepcopy(a);observe(x,1800);observe(x,1800);add('compose_required_overflow','compose',x)
    x=copy.deepcopy(a);observe(x,1000,'estimate');add('compose_estimate','compose',x)
    x=copy.deepcopy(a);observe(x,1000);x['observations'][0]['candidate']['identity']['template_hash']='changed';add('compose_foreign','compose',x,True)
    x=copy.deepcopy(a);observe(x,1000);x['observations'][0]['sequence']=1;add('compose_sequence','compose',x,True)
    x=copy.deepcopy(a);observe(x,1000);x['observations']*=2;add('compose_terminal','compose',x,True)
    cs=intent_args()['choices']
    a={'choices':cs,'revision':7,'candidates':[{'id':'known_request','source':'rule','match':'exact','decision':cs['choices'][0]}],'available':cs['choices']}
    for source in ['rule','procedure','brain']:
        x=copy.deepcopy(a);x['candidates'][0]['source']=source;add('route_'+source,'route',x)
    x=copy.deepcopy(a);x['candidates'][0]['match']='ambiguous';add('route_ambiguous','route',x)
    x=copy.deepcopy(a);x['candidates'][0]['match']='none';add('route_unmatched','route',x)
    x=copy.deepcopy(a);x['revision']=8;add('route_stale','route',x)
    x=copy.deepcopy(a);x['available']=[];add('route_unavailable','route',x)
    x=copy.deepcopy(a);x['candidates'].append({'id':'conflict','source':'rule','match':'exact','decision':cs['choices'][1]});add('route_conflict','route',x)
    x=copy.deepcopy(a);x['candidates']*=2;add('route_duplicate','route',x,True)
    cs=intent_args()['choices'];names=[{'name':p['action']+'_gate' if p['target'] else 'wait_here','pair':p} for p in cs['choices']]
    a={'choices':cs,'names':names,'calls':[{'text':'{"choice":"inspect_gate"}','revision':7,'available':cs['choices']}]}
    add('codec_success','choice-codec',a)
    x=copy.deepcopy(a);x['calls'].insert(0,{'text':'{"choice":"unknown"}','revision':7,'available':cs['choices']});add('codec_retry','choice-codec',x)
    x=copy.deepcopy(a);x['calls'][0]['text']='{"choice":"open_gate","choice":"inspect_gate"}';add('codec_duplicate_json','choice-codec',x)
    x=copy.deepcopy(a);x['calls'][0]['revision']=8;add('codec_stale','choice-codec',x)
    x=copy.deepcopy(a);x['names'][1]['name']=x['names'][0]['name'];add('codec_duplicate_name','choice-codec',x,True)
    x=copy.deepcopy(a);x['names'][1]['pair']=x['names'][0]['pair'];add('codec_duplicate_pair','choice-codec',x,True)
    x=copy.deepcopy(a);x['names'].pop();add('codec_missing_pair','choice-codec',x,True)
    import local_cases
    manifest=[{'id':'t'+str(i),'split':'tune' if i<2 else 'holdout','expected':{'action':'inspect'}} for i in range(4)]
    candidates=[]
    for label,tokens,wrong in [('baseline',200,False),('compact',100,False),('cheap_wrong',50,True)]:
        profile=local_cases.profile();profile['name']=label
        trials=[local_cases.trial(i) for i in range(4)]
        for t in trials:
            t['condition']=label;t['attempts'][0]['input_tokens']=tokens
            if wrong:t['actual']={'action':'wait'}
        candidates.append({'profile':profile,'recipe':'scene','trials':trials})
    a={'identity':local_cases.identity(),'cases':manifest,'candidates':candidates,
        'limits':{'min_correct_per_mille':1000,'max_p95_micros':10000,'max_rss_bytes':4000000}}
    add('tune_select','tune',a)
    x=copy.deepcopy(a);x['candidates'][1]['trials'][2]['actual']={'action':'wait'};add('tune_holdout_independent','tune',x)
    x=copy.deepcopy(a);x['candidates'][1]['trials'][0]['memory']['runner_rss_bytes']=None;add('tune_missing_metrics','tune',x)
    x=copy.deepcopy(a);x['limits']['max_rss_bytes']=0;add('tune_no_winner','tune',x)
    x=copy.deepcopy(a);x['candidates'][0]['trials'].pop();add('tune_incomplete','tune',x,True)
    x=copy.deepcopy(a);x['candidates'][0]['trials'][0]['expected']={'action':'wait'};add('tune_oracle_changed','tune',x,True)
    x=copy.deepcopy(a);x['candidates'][0]['profile']['identity']['template_hash']='foreign';add('tune_foreign_identity','tune',x,True)
    a=next(v['args'] for n,v,e in out if n=='template_count')
    x=copy.deepcopy(a);x['template']['parameters'][0].update(column='count',kind='number',op='>=');x['arguments']={'kind':3};add('template_number','query-template',x)
    x=copy.deepcopy(a);x['template']['parameters'][0].update(column='active',kind='boolean');x['arguments']={'kind':True};x['rows']=[{'active':True},{'active':1},{'active':False}];add('template_boolean','query-template',x)
    x=copy.deepcopy(a);x['template']['parameters']*=2;add('template_duplicate_parameter','query-template',x,True)
    x=copy.deepcopy(a);x['template']['parameters'][0]['op']='in';x['arguments']['kind']=[];add('template_empty_in','query-template',x,True)
    x=copy.deepcopy(a);x['rows']=[{'count':1}];add('template_missing_column','query-template',x,True)
    a=next(v['args'] for n,v,e in out if n=='evidence_group')
    x=copy.deepcopy(a);x['records'][0]['depends']*=2;add('evidence_duplicate_dependency','evidence',x,True)
    a=next(v['args'] for n,v,e in out if n=='route_rule')
    x=copy.deepcopy(a);x['candidates'][0]['decision']={'action':'fly','target':None};add('route_invalid_draft','route',x)
    x=copy.deepcopy(a);x['candidates'].append({'id':'known_procedure','source':'procedure','match':'exact','decision':cs['choices'][1]});add('route_priority','route',x)
    a=next(v['args'] for n,v,e in out if n=='tune_select')
    x=copy.deepcopy(a);x['candidates'][1]['trials'][0]['supplied']=[];add('tune_missing_required','tune',x)
    x=copy.deepcopy(a);x['candidates'][1]['trials'][0]['attempts'][0]['truncated']=True;add('tune_truncation','tune',x)
    x=copy.deepcopy(a);x['candidates'][1]['trials'][0]['elapsed_micros']=20000;add('tune_slow','tune',x)
    x=copy.deepcopy(a);x['candidates'][1]['trials'][0]['attempts'][0]['input_tokens']=None;add('tune_missing_tokens','tune',x)
    a=next(v['args'] for n,v,e in out if n=='compose_ready')
    x=copy.deepcopy(a);ack=x['observations'][0];ack['candidate']=dict(reversed(list(ack['candidate'].items())))
    add('compose_reordered_candidate','compose',x,True)
    return out

def main():
    result={'cases':0,'accepted':0,'rejected':0,'native_comparisons':0,'api_spend_usd':0}
    clients=[]
    if '--check-bindings' in sys.argv:
        sys.path.insert(0,str(ROOT/'bindings/python'))
        from gamectx_client import Client,NativeClient
        clients=[Client(ROOT/'examples/kit/project.json',ROOT/'target/debug/gamectx'),
                 NativeClient((ROOT/'examples/kit/project.json').read_text(),ROOT/'bindings/native/target/debug/libgamectx_native.so')]
    for name,v,error in cases():
        expected=None
        try:
            expected=ref.process(v);assert not error,name;result['accepted']+=1
        except (ValueError,KeyError,TypeError):
            assert error,name;result['rejected']+=1
        if '--check' in sys.argv:
            p=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=ref.encode(v),text=True,capture_output=True)
            assert bool(p.returncode)==error,(name,p.stderr)
            if not error:assert p.stdout.rstrip('\n')==ref.encode(expected),(name,p.stdout,ref.encode(expected))
            result['native_comparisons']+=1
        for client in clients:
            try:
                actual=client.request('local',v)
                assert not error,name
                assert ref.encode(actual)==ref.encode(expected),(name,actual,expected)
            except ValueError:
                assert error,name
        if '--write-conformance' in sys.argv and '_generated_' not in name:
            d=ROOT/'conformance'/'optimization'/name;d.mkdir(parents=True,exist_ok=True)
            (d/'case.json').write_text(json.dumps({'op':'local',**({'error':True} if error else {})},indent=2)+'\n')
            (d/'input.json').write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
            if not error:(d/'expected.json').write_text(ref.encode(expected)+'\n')
        result['cases']+=1
    for client in clients:client.close()
    if clients:result['bridge_comparisons']=len(clients)*result['cases']
    print(json.dumps(result,indent=2))
if __name__=='__main__':main()
