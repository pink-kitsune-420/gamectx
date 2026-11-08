"""Offline audit and exact metrics for fresh local probes; never calls a provider."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,hashlib,json,pathlib,statistics,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import character_limit_eval as limits
import ollama_eval as oe
from adversarial_reference import assessment
ROOT=pathlib.Path(__file__).resolve().parents[2]

def check_cache(path,tokens=False):
 r=oe.strict(path.read_text());assert r['complete'] and r['tokens_only']==tokens and r['api_spend_usd']==0
 source=oe.strict((ROOT/'eval/local/character-limit-tokens.json').read_text());assert r['identity']['digest']==source['identity']['digest'] and r['identity']['template_sha256']==source['identity']['template_sha256'] and r['identity']['runner']==source['identity']['runner']
 calls=0
 for mode,m in r['models'].items():
  calls+=len(m['calls'])
  for c in m['calls']:
   assert c['status']=='complete' and c['payload']['model']==limits.MODEL
   opts=c['payload']['options'];assert opts['num_ctx']==2048 and opts['num_thread']==4 and opts['num_gpu']==0 and opts['temperature']==0 and opts['seed']==42
   response=c['response'];assert response['done'] and response['prompt_eval_count']+32+256<=2048
  receipts=m.get('count_receipts',[])
  for receipt in receipts:
   first=m['calls'][receipt['source_preflight']];second=m['calls'][receipt['source_generation']];assert first['phase']=='preflight' and second['phase']=='generation';assert first['response']['prompt_eval_count']==second['response']['prompt_eval_count']==receipt['input_tokens']
   raw=first['payload'];request={k:raw[k] for k in ['messages','format']};key=hashlib.sha256(limits.oe.ref.encode(dict(identity=r['identity'],context=2048,output=32,request=request)).encode()).hexdigest();assert key==receipt['key']
 for o in r['observations']:
  mode=o['mode'];used=r['models'][mode]['calls'][o['calls_start']:o['calls_end']];assert used
  expected=next(c for c in source['suite'] if c['id']==o['id']);p=limits.payload(expected,'focused');request={k:p[k] for k in ['messages','format']};assert o['request']==request
  for c in used:assert {k:c['payload'][k] for k in ['messages','format']}==request
  if not tokens:
   actual=limits.decode(used[-1]['response']['message']['content']);assert o['actual']==actual and o['expected']==expected['expected'] and o['correct']==(actual==expected['expected'])
 assert r['model_calls']==calls
 if not tokens:
  assert len(r['observations'])==18 and len(r['models']['baseline']['calls'])==18 and len(r['models']['cached']['calls'])==12
  for ident in ['no_source','window_sun','clock_reading']:
   for repeat in range(3):
    pair=[o for o in r['observations'] if o['id']==ident and o['repetition']==repeat];assert len(pair)==2 and pair[0]['actual']==pair[1]['actual']
 return r

def metrics(r):
 result={}
 for mode in ['focused','expanded']:
  rows=[o for o in r['observations'] if o['mode']==mode]
  result[mode]=dict(correct=sum(o['correct'] for o in rows),total=len(rows),valid_json=sum(o['actual'] is not None for o in rows),model_calls=sum(len(o['calls']) for o in rows),full_turn_input_tokens=sum(c['response']['final']['prompt_eval_count'] for o in rows for c in o['calls']),median_turn_micros=statistics.median(o['turn_micros'] for o in rows),runner_rss_bytes=max(o['memory']['runner_rss_bytes'] or 0 for o in rows),harness_rss_bytes=max(o['memory']['game_rss_bytes'] or 0 for o in rows),minimum_available_bytes=min(o['memory']['available_bytes'] for o in rows),swap_in_bytes=sum(o['memory']['swap_in_bytes'] or 0 for o in rows),swap_out_bytes=sum(o['memory']['swap_out_bytes'] or 0 for o in rows),retries=sum(o['retries'] for o in rows))
 return result

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--output',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 for name in ['character-limit-tokens.json','character-limit-results.json']:limits.audit(ROOT/'eval'/'local'/name)
 env=oe.strict((ROOT/'eval/local/character-limit-environment.json').read_text());assert hashlib.sha256(env['template'].encode()).hexdigest()==env['template_sha256'];assert env['details']['quantization_level']=='Q4_K_M'
 raw=oe.strict((ROOT/'eval/local/character-limit-results.json').read_text());token=oe.strict((ROOT/'eval/local/character-limit-tokens.json').read_text());assert env['digest']==raw['identity']['digest'] and env['template_sha256']==raw['identity']['template_sha256'];assert raw['identity']==token['identity'] and raw['suite_hash']==oe.digest(raw['suite'])==token['suite_hash']
 for case in raw['suite']:
  category=assessment(case['context']['time'])['kind'];assert ('exact' if category=='exact_reading' else category)==case['expected']
 prior=check_cache(ROOT/'eval/query/count-cache-tokens.json',True);pilot=check_cache(ROOT/'eval/query/count-cache-results.json');production=check_cache(ROOT/'eval/query/count-cache-production.json')
 cache={}
 for mode,m in production['models'].items():
  rows=[o for o in production['observations'] if o['mode']==mode];cache[mode]=dict(turns=len(rows),correct=sum(o['correct'] for o in rows),calls=len(m['calls']),input_tokens=sum(c['response']['prompt_eval_count'] for c in m['calls']),median_turn_micros=statistics.median(o['turn_micros'] for o in rows),median_repeated_micros=statistics.median(o['turn_micros'] for o in rows if o['repetition']>0))
 baseline=oe.strict((ROOT/'eval/local/adversarial-baseline.json').read_text());fixed=oe.strict((ROOT/'eval/local/adversarial-fixed.json').read_text());assert baseline['summary']['failed']==42 and fixed['summary']['failed']==0
 result=dict(version=1,classification=metrics(raw),cache=cache,classification_model_calls=raw['model_calls']+token['model_calls'],cache_model_calls=prior['model_calls']+pilot['model_calls']+production['model_calls'],total_model_calls=159,api_spend_usd=0,adversarial_before=baseline['summary'],adversarial_after=fixed['summary'],limits='Small synthetic time classification and repeated adapter requests, no gameplay/model-speech accuracy claim. Natural caches and global memory pressure are uncontrolled. Pilot latency excludes scoring; production includes it.')
 assert result['total_model_calls']==result['classification_model_calls']+result['cache_model_calls']
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
 if a.check:assert oe.strict(a.check.read_text())==result
 print(json.dumps(result))
if __name__=='__main__':main()
