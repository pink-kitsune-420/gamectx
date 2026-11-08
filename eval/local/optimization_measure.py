"""Offline contract measurements and optional local-only one-token probes. Requires --cap 0."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import hashlib
import json
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import optimization_cases as cases
import optimization_reference as ref
import local_reference as base

def measurements():
    data={name:(v,error) for name,v,error in cases.cases()}
    a=cases.intent_args();legacy=base.decisions({'choices':a['choices'],'calls':a['calls'][:1]})
    guarded=ref.intent(a)
    evidence=data['evidence_group'][0]['args'];selected=ref.evidence(evidence)
    full={r['id']:r['value'] for r in evidence['records']}
    template=data['template_count'][0]['args']
    compiler=ref.process(data['tune_select'][0]);held=ref.process(data['tune_holdout_independent'][0])
    route_names=['route_rule','route_procedure','route_brain','route_ambiguous','route_unmatched','route_stale','route_unavailable','route_conflict']
    routed=[ref.process(data[n][0]) for n in route_names]
    return {'api_spend_usd':0,'kind':'offline contract/byte checks; not model accuracy or token estimates',
        'intent':{'legacy_first_status':legacy['results'][0]['status'],'legacy_first_action':legacy['results'][0]['decision']['action'],
                  'guarded_first_status':guarded['results'][0]['status'],'guarded_final_action':guarded['results'][-1]['decision']['action']},
        'evidence':{'full_packet_bytes':len(ref.encode(full).encode()),'selected_packet_bytes':selected['bytes'],'kept':selected['kept']},
        'query_arguments':{'sql_response_bytes':len(ref.encode({'sql':"SELECT COUNT(*) FROM stock WHERE kind = 'food'"}).encode()),
                           'template_response_bytes':len(ref.encode(template['arguments']).encode()),'tool_schema_bytes':len(ref.encode(ref.query_template(template)['tool']).encode())},
        'routing':{'scripted_cases':len(routed),'local_drafts':sum(r['route']!='llm' for r in routed),'llm_paths':sum(r['route']=='llm' for r in routed)},
        'tuning':{'selected':compiler['selected']['profile']['name'],'selected_after_holdout_error':held['selected']['profile']['name'],
                  'selected_holdout_correct':held['candidates'][1]['holdout']['correct']}}

def requests():
    directions=['north','south','east','west','upper','lower','inner','outer']
    for count in [2,4,8]:
        pairs=[{'action':'inspect','target':'grove:gate.'+d} for d in directions[:count]]
        choices={'version':1,'revision':9,'choices':pairs,'fallback':None}
        names=[{'name':'inspect_'+d+'_gate','pair':p} for d,p in zip(directions,pairs)]
        result=ref.codec({'choices':choices,'names':names,'calls':[]})
        for mode,offer,schema in [('pairs',pairs,base.decision_schema(choices)),('named',names,result['schema'])]:
            data={'request':'Inspect the north gate.','choices':offer,'response_schema':schema}
            yield {'count':count,'mode':mode,'request':{'messages':[{'role':'system','content':'Select the requested choice. Return only the structured decision.'},
                {'role':'user','content':ref.encode(data)}],'format':schema},
                'example_output':ref.encode(pairs[0] if mode=='pairs' else {'choice':'inspect_north_gate'})}

def run_tokens(args):
    import ollama_eval as oe
    server=oe.Server(args.url,timeout=60);tags=server.data('/api/tags')['models']
    model=next(m for m in tags if m['name']==args.model)
    show=server.data('/api/show',{'model':args.model});oe.require_local_weights(model,show)
    if args.output.exists():raise ValueError('output already exists')
    result={'version':1,'kind':'one-token local prompt probes; not accuracy or complete-turn latency','api_spend_usd':0,
        'model':args.model,'digest':model['digest'],'quantization':model['details']['quantization_level'],
        'template_sha256':hashlib.sha256(show.get('template','').encode()).hexdigest(),
        'runner':server.data('/api/version')['version'],'observations':[]}
    oe.checkpoint(args.output,result)
    for case in requests():
        payload={'model':args.model,**case['request'],'keep_alive':'10m',
            'options':{'num_ctx':2048,'num_predict':1,'num_thread':4,'num_gpu':0,'temperature':0,'seed':42}}
        response=server.chat(payload)
        count=response['final'].get('prompt_eval_count')
        if type(count) is not int or count+96+256>2048:raise ValueError('missing count or reserved context overflow')
        result['observations'].append({**case,'payload':payload,'response':response,'input_tokens':count})
        oe.checkpoint(args.output,result)
        print(f"{case['count']} choices / {case['mode']}: {count} input tokens; API spend $0",flush=True)
    current=next(m for m in server.data('/api/tags')['models'] if m['name']==args.model)
    if current['digest']!=model['digest']:raise ValueError('model identity changed')
    result['complete']=True;oe.checkpoint(args.output,result)

def check_tokens(path):
    import ollama_eval as oe
    result=oe.strict(path.read_text());assert result['complete'] and result['api_spend_usd']==0
    expected=list(requests());assert len(result['observations'])==len(expected)
    for observed,case in zip(result['observations'],expected):
        for key in case:assert observed[key]==case[key]
        response=observed['response'];assert response['chunks'][-1]==response['final']
        assert response['text']==''.join(c.get('message',{}).get('content','') for c in response['chunks'])
        assert response['final']['eval_count']<=1
        assert observed['input_tokens']==response['final']['prompt_eval_count']
        payload=observed['payload'];assert payload['model']==result['model']
        assert payload['options']=={'num_ctx':2048,'num_predict':1,'num_thread':4,'num_gpu':0,'temperature':0,'seed':42}
        for key,val in case['request'].items():assert payload[key]==val
        assert observed['input_tokens']+352<=2048
    print(json.dumps({'checked_token_probes':len(expected),'api_spend_usd':0}))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--cap',type=float,required=True)
    parser.add_argument('--tokens-only',action='store_true');parser.add_argument('--model');parser.add_argument('--url',default='http://127.0.0.1:11434')
    parser.add_argument('--output',type=pathlib.Path);parser.add_argument('--check-tokens',type=pathlib.Path);parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    if args.cap!=0:parser.error('--cap must be 0')
    if args.check_tokens:check_tokens(args.check_tokens);return
    if args.tokens_only:
        if not args.model or not args.output:parser.error('--model and --output required')
        run_tokens(args);return
    result=measurements()
    if args.check:assert ref.encode(result)+'\n'==(pathlib.Path(__file__).parent/'optimization-measurements.json').read_text()
    elif args.output:args.output.write_text(ref.encode(result)+'\n')
    print(json.dumps(result,indent=2))
if __name__=='__main__':main()
