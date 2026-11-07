"""Bounded local paired count-reuse experiment, with token-only prerequisite."""
import argparse,copy,json,pathlib,statistics,sys,time
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import ollama_eval as oe
import character_limit_eval as suite
from count_cache_reference import CachedLocalModel
from dungeon_social_eval import ui

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--tokens-only',action='store_true');p.add_argument('--production',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 assert not a.output.exists();source=oe.strict((suite.ROOT/'eval/character-limit-tokens.json').read_text());assert source['complete']
 cases=[c for c in source['suite'] if c['id'] in ['no_source','window_sun','clock_reading']]
 models={'baseline':ui.LocalModel(suite.MODEL,2048,32,None),'cached':ui.LocalModel(suite.MODEL,2048,32,None,count_cache=8) if a.production else CachedLocalModel(suite.MODEL,2048,32,None,capacity=8)};assert models['baseline'].identity==models['cached'].identity
 expected_identity=source['identity'];actual_identity=models['baseline'].identity
 assert actual_identity['digest']==expected_identity['digest'] and actual_identity['template_sha256']==expected_identity['template_sha256'] and actual_identity['runner']==expected_identity['runner']
 r=dict(version=1,implementation='production' if a.production else 'prototype',timing='production includes packet-to-request assembly, request copy, identity checks, all model calls, scoring and memory sampling; captured engine snapshot is outside the timer' if a.production else 'adapter-only pilot; scoring and memory sampling excluded',tokens_only=a.tokens_only,identity=actual_identity,api_spend_usd=0,observations=[],models={m:obj.records for m,obj in models.items()},complete=False)
 if not a.tokens_only:
  assert a.tokens;prior=oe.strict(a.tokens.read_text());assert prior['complete'] and prior['tokens_only'] and prior['identity']==actual_identity
 ui.checkpoint(a.output,r)
 for repetition in range(1 if a.tokens_only else 3):
  for i,c in enumerate(cases):
   if not a.production:p=suite.payload(c,'focused');request={k:p[k] for k in ['messages','format']}
   modes=['baseline'] if a.tokens_only else (['baseline','cached'] if (i+repetition)%2==0 else ['cached','baseline'])
   for mode in modes:
    model=models[mode];begin=time.perf_counter_ns();call_start=len(model.calls);raw=None
    if a.production:p=suite.payload(c,'focused');request={k:p[k] for k in ['messages','format']}
    if a.tokens_only:
     response=model.chat(copy.deepcopy(request),1,'preflight');assert response['prompt_eval_count']+32+256<=2048
    else:
     before=next(o for o in prior['observations'] if o['id']==c['id']);assert before['request']==request
     raw=model.generate(copy.deepcopy(request),32)
    if not a.production:elapsed=(time.perf_counter_ns()-begin)//1000
    calls=model.calls[call_start:];memory=oe.memory();actual=None if a.tokens_only else suite.decode(raw)
    if a.production:elapsed=(time.perf_counter_ns()-begin)//1000
    r['observations'].append(dict(id=c['id'],repetition=repetition,mode=mode,request=request,expected=c['expected'],actual=actual,correct=None if a.tokens_only else actual==c['expected'],calls_start=call_start,calls_end=len(model.calls),turn_micros=elapsed,memory=memory))
    ui.checkpoint(a.output,r);print(c['id']+' / '+mode+' / '+str(repetition)+': '+str(len(calls))+' calls; API spend $0',flush=True)
 r['model_calls']=sum(len(m.calls) for m in models.values());r['complete']=True;ui.checkpoint(a.output,r);print(json.dumps(dict(rows=len(r['observations']),model_calls=r['model_calls'],api_spend_usd=0)))
if __name__=='__main__':main()
