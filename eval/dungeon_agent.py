"""Optional local-only dungeon agent. Token probes first; bounded calls; no credentials/pulls."""
import argparse
import hashlib
import json
import pathlib
import sys
import time
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import dungeon_cases as dungeon
import ollama_eval as oe
GOAL='Recover the glass star and escape.'

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cap',type=float,required=True)
    p.add_argument('--model',required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--tokens',type=pathlib.Path)
    p.add_argument('--tokens-only',action='store_true');p.add_argument('--mode',choices=['raw','scene','named'],default='named')
    p.add_argument('--seeds',default='0,7,101');p.add_argument('--max-turns',type=int,default=32);p.add_argument('--max-calls',type=int,default=64)
    a=p.parse_args()
    if a.cap!=0 or not 1<=a.max_turns<=64 or not 1<=a.max_calls<=256:p.error('require --cap 0, 1..64 turns and 1..256 calls')
    try:seeds=[int(x) for x in a.seeds.split(',')]
    except ValueError:p.error('invalid seeds')
    if not 1<=len(seeds)<=8 or len(set(seeds))!=len(seeds) or any(not 0<=s<=65535 for s in seeds):p.error('supply 1..8 unique seeds in 0..65535')
    if a.output.exists():p.error('use a fresh output path')
    server=oe.Server('http://127.0.0.1:11434',60)
    models=server.data('/api/tags')['models'];model=next(m for m in models if m['name']==a.model)
    show=server.data('/api/show',{'model':a.model});oe.require_local_weights(model,show)
    identity={'model':a.model,'digest':model['digest'],'runner':server.data('/api/version')['version'],
              'template_sha256':hashlib.sha256(show.get('template','').encode()).hexdigest()}
    if not a.tokens_only:
        if a.tokens is None:p.error('--tokens from a completed matching token-only run required')
        prior=oe.strict(a.tokens.read_text())
        if not prior.get('complete') or not prior.get('tokens_only') or prior['identity']!=identity or prior['mode']!=a.mode or prior['seeds']!=seeds:p.error('token artifact identity/mode/seeds mismatch')
        if len(prior['episodes'])!=len(seeds) or any(e['stop']!='token_probe' for e in prior['episodes']) or len(prior['calls'])!=len(seeds):p.error('token probe run did not finish successfully')
        if any(type(c['response']['final'].get('prompt_eval_count')) is not int or c['response']['final']['prompt_eval_count']+352>2048 for c in prior['calls']):p.error('token probes did not fit')
    result={'version':1,'identity':identity,'mode':a.mode,'seeds':seeds,'tokens_only':a.tokens_only,'api_spend_usd':0,
        'calls':[],'episodes':[],'scope':'complete host turns include observation, preflight, generation, validation, correction and action; sampled memory not collected'}
    oe.checkpoint(a.output,result)
    def chat(payload,phase):
        if len(result['calls'])>=a.max_calls:raise ValueError('model call cap reached')
        entry={'phase':phase,'payload':payload,'status':'reserved'};result['calls'].append(entry);oe.checkpoint(a.output,result)
        try:response=server.chat(payload)
        except Exception as e:
            entry['status']='failed';entry['error']=type(e).__name__;oe.checkpoint(a.output,result);raise
        entry['response']=response;entry['status']='complete';oe.checkpoint(a.output,result);return response
    for seed_index,seed in enumerate(seeds):
        client=dungeon.Client(seed);episode={'seed':seed,'turns':[],'won':False,'stop':None};result['episodes'].append(episode)
        try:
            for _ in range(1 if a.tokens_only else a.max_turns):
                started=time.perf_counter_ns();state=client.call('state',{})
                if state['won']:episode['won']=True;episode['stop']='won';break
                if not state['choices']:episode['stop']='finished';break
                observation=client.call('observe',{'input':GOAL,'mode':a.mode,'context':2048})
                data={'model':a.model,**observation['request'],'keep_alive':'10m',
                      'options':{'num_ctx':2048,'num_predict':1,'num_thread':4,'num_gpu':0,'temperature':0,'seed':42}}
                if not a.tokens_only and state['revision']==0:
                    if prior['calls'][seed_index]['payload']!=data:raise ValueError('initial token request changed')
                turn={'before':state,'observation':observation,'attempts':[]};episode['turns'].append(turn)
                for attempt in range(1 if a.tokens_only else 2):
                    probe=chat(data,'preflight');count=probe['final'].get('prompt_eval_count')
                    if type(count) is not int or count+96+256>2048:raise ValueError('native prompt count missing or over reserved context')
                    if a.tokens_only:break
                    generated=chat({**data,'options':{**data['options'],'num_predict':96}},'decision')
                    offered=client.request('decide',{'revision':state['revision'],'text':generated['text'],'format':'named' if a.mode=='named' else 'pair'})
                    accepted=offered.get('ok') and offered['result'].get('status')=='accepted'
                    turn['attempts'].append({'text':generated['text'],'reply':offered,'accepted':bool(accepted)})
                    if accepted:break
                    data={**data,'messages':[*data['messages'],{'role':'assistant','content':generated['text'][:4096]},
                        {'role':'user','content':'The host rejected that proposal. Keep the current facts and original goal: '+GOAL+' Return a supplied legal choice.'}]}
                turn['after']=client.call('state',{});turn['elapsed_micros']=(time.perf_counter_ns()-started)//1000
                episode['won']=turn['after']['won'];oe.checkpoint(a.output,result)
                print(f"seed {seed}, revision {state['revision']}: {'token probe' if a.tokens_only else 'won' if episode['won'] else 'continuing'}; API spend $0",flush=True)
                if episode['won']:episode['stop']='won';break
            if episode['stop'] is None:episode['stop']='token_probe' if a.tokens_only else 'turn_cap'
        except (ValueError,OSError,AssertionError) as e:
            episode['stop']='error';episode['error']=str(e)[:256]
            if len(result['calls'])>=a.max_calls:episode['stop']='call_cap'
        finally:client.close();oe.checkpoint(a.output,result)
    if next(m['digest'] for m in server.data('/api/tags')['models'] if m['name']==a.model)!=identity['digest']:raise ValueError('model identity changed')
    result['complete']=True;oe.checkpoint(a.output,result)
    print(json.dumps({'episodes':len(result['episodes']),'wins':sum(e['won'] for e in result['episodes']),'model_calls':len(result['calls']),'api_spend_usd':0}))
if __name__=='__main__':main()
