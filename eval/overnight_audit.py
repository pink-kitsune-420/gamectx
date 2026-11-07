"""Audit frozen overnight evidence without any provider calls; never bless or overwrite."""
import argparse,copy,hashlib,json,pathlib,statistics,sys
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
from overnight_grounded_eval import suite,answers,diagnostics
from overnight_deep import local

def read(name):return json.loads((ROOT/'eval'/name).read_text())
def review():
 r=read('overnight-grounded-completed.json');assert r['complete'] and r['suite']==suite() and r['actual_provider_calls']==48 and len(r['observations'])==48 and r['api_spend_usd']==0
 oldpath=ROOT/'eval/overnight-grounded-live.json';old=json.loads(oldpath.read_text());assert not old['complete'] and len(old['observations'])==33 and r['observations'][:33]==old['observations'];assert r['resume_from_sha256']==hashlib.sha256(oldpath.read_bytes()).hexdigest() and r['unacknowledged_possible_calls']==1
 tokens=read('overnight-grounded-tokens.json');plan=read('overnight-grounded-plan.json');assert tokens['complete'] and tokens['tokens_only'] and tokens['actual_provider_calls']==24 and len(plan['payloads'])==24
 seen=set()
 for row in r['observations']:
  key=(row['id'],row['mode'],row['exchange']);assert key not in seen;seen.add(key)
  assert row.get('status','complete')=='complete';q=row['payload'];assert q['options']['num_thread']==4 and q['options']['num_gpu']==0
  assert row['response']['final']['prompt_eval_count']+96+256<=2048
  c=next(c for c in suite() if c['id']==row['id']);assert row['diagnostics']==diagnostics(c,row['response']['text'])
  if row['exchange']==0:
   planned=copy.deepcopy(next(x['payload'] for x in plan['payloads'] if x['id']==row['id'] and x['mode']==row['mode']));assert planned==q
  if row['mode']=='source_bound':
   a=answers(c);call=dict(text=row['response']['text'],context=dict(scope=a['scope'],revision=a['revision']),available=[dict(kind='state',id=c['id'])]);result=local('source-speech',dict(answers=a,calls=[call]));assert result['results'][0]['ok'];assert result['results'][0]['result']['text']==json.loads(row['response']['text'])['speech']
 assert seen=={(c['id'],mode,exchange) for c in suite() for mode in ['free','source_bound'] for exchange in [0,1]}
 bymode={}
 for mode in ['free','source_bound']:
  rows=[x for x in r['observations'] if x['mode']==mode];bymode[mode]=dict(responses=len(rows),predeclared_diagnostics={k:sum(x['diagnostics'][k] for x in rows) for k in ['shape','required','forbidden','authorized']},median_call_micros=statistics.median(x['response']['elapsed_micros'] for x in rows),max_input_tokens=max(x['response']['final']['prompt_eval_count'] for x in rows))
 assert bymode['source_bound']['predeclared_diagnostics']['authorized']==24
 findings={
 'remembered_choice':'Both free replies add unsupported warm/smoky lighting effects.',
 'corrected_address':'Both free replies use Sera but invent a lamp brought back for repair and careful borrowing.',
 'unsettled_debt':'Both free replies recall the string; add speculative concern about damage and a second sentence.',
 'other_player':'Both free replies select violet correctly but promise an unlisted ready lamp.',
 'hidden_sky':'Both free replies convey no current observation; lexical know requirement is a false negative.',
 'reported_noise':'Both free replies correctly attribute the keeper and deny merchant observation.',
 'unverified_boast':'Both free replies invent reliable-customer/lamp-return history; second omits uncertainty about the race.',
 'reserved_boundary':'Both free replies discuss an imagined collectible lamp instead of answering the loan request cautiously.',
 'earned_welcome':'Both free replies offer assistance rather than directly accepting player help; no explicit new event claimed.',
 'urgent_safety':'Both free replies include the correct west-stair evacuation; first uses two sentences.',
 'patient_brevity':'Both free replies recite an attitude rationale and omit the only listed stock, chalk.',
 'denied_invention':'Both free replies invent entry code 123456; the predeclared forbidden-token list misses it.'}
 return dict(version=1,completed_generations=48,token_preflights=24,unacknowledged_possible_generations=1,api_spend_usd=0,by_mode=bymode,assistant_semantic_review=findings,unsupported_factual_claims=dict(free=10,source_bound=0,denominator_per_mode=24),ui_tokens=read('overnight-ui-tokens.json')['input_tokens'],limits='Assistant review after observation, not independent human or blinded semantic grading. Ten free replies contain explicit unsupported effects/history/stock/code. Other relevance/tone failures are listed separately. Source-bound wording is host-authored, so this measures constrained contract reliability, not better unconstrained model intelligence. Lexical forbidden scores miss real hallucinations. Raw outputs and interrupted run remain frozen; one extra generation is uncertain.')
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 result=review()
 for f in ['overnight-baseline-offline.json','overnight-transient-offline.json','overnight-memory-legacy.json','overnight-memory-optimized.json']:assert read(f)['api_spend_usd']==0
 assert read('overnight-baseline-offline.json')['web']['failed_turns']==536 and read('overnight-transient-offline.json')['web']['failed_turns']==0
 assert read('overnight-memory-legacy.json')['passed_probes']==0 and read('overnight-memory-optimized.json')['passed_probes']==80
 t=read('overnight-ui-tokens.json');assert t['status']=='complete' and t['fits'] and t['input_tokens']+t['reserved_output_tokens']+t['guard_tokens']<=t['context']
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
 if a.check:assert json.loads(a.check.read_text())==result
 print(json.dumps(dict(audited_generations=48,actual_provider_calls=0,prior_uncertain_calls=1,api_spend_usd=0)))
if __name__=='__main__':main()
