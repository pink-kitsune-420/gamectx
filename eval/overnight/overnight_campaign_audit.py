"""Offline audit of actual archived-state cohorts, including retained failed harness."""
import argparse,hashlib,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()]
from overnight_deep import local,enc

def read(name):return json.loads((ROOT/'eval'/'overnight'/name).read_text())
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 result=dict(version=1,legacy={},safe={},api_spend_usd=0,audit_provider_calls=0)
 for label,plan_name,modes in [('legacy','overnight-campaign-plan.json',[('grounded','overnight-campaign-grounded.json'),('free','overnight-campaign-free.json')]),('safe','overnight-campaign-safe-holdout-plan.json',[('optional','overnight-campaign-safe-optional.json'),('required','overnight-campaign-safe-required.json')])]:
  path=ROOT/'eval'/'overnight'/plan_name;plan=json.loads(path.read_text());assert plan['events']==4000 and plan['restores']==40 and plan['accepted_promise_checks']==40 and plan['guard_model_calls']==0 and len(plan['packets'])==12
  for packet in plan['packets']:
   assert [e['token'] for e in packet['selected']]==packet['required_tokens'] and len(packet['selected'])<=2
   assert all(e['subject']==packet['player'] for e in packet['selected'])
   assert len(packet['promise_notes'])==(1 if packet['player']=='player_1' else 0)
   if label=='safe':
    assert plan['gate_aware'] and packet['inspection']['facets_withheld']
    body=enc(packet['request']);assert 'copper sparrow' not in body and 'silver chime opens' not in body and 'GUARD_ONLY_TEXT' not in body
    fields=json.loads(packet['request']['messages'][2]['content']);assert all(x['definition']['subject']==packet['player'] for x in fields['character']['attitudes'])
  for mode,name in modes:
   r=read(name);assert r['complete'] and r['actual_provider_calls']==24 and r['api_spend_usd']==0 and r['plan_sha256']==hashlib.sha256(path.read_bytes()).hexdigest() and len(r['observations'])==12
   required=authorized=deferrals=exposures=0;max_tokens=0
   for row in r['observations']:
    packet=next(x for x in plan['packets'] if x['actor']==row['actor'] and x['family']==row['family']);assert len(row['calls'])==2
    for index,call in enumerate(row['calls']):
     assert call['status']=='complete' and call['phase']==('preflight' if index==0 else 'generation')
     q=call['payload'];assert q['options']['num_thread']==4 and q['options']['num_gpu']==0 and q['options']['num_predict']==(1 if index==0 else 96)
     assert call['response']['final']['prompt_eval_count']+96+256<=2048
     max_tokens=max(max_tokens,call['response']['final']['prompt_eval_count'])
     assert 'GUARD_ONLY_TEXT' not in enc(q)
     assert all(m['content'].startswith('['+row['actor']+' ->') for m in q['messages'] if m['role']=='assistant')
     if label=='safe':assert 'copper sparrow' not in enc(q) and 'silver chime opens' not in enc(q)
    first=row['calls'][0]['payload'];second=row['calls'][1]['payload'];first=json.loads(enc(first));first['options']['num_predict']=96;assert first==second
    raw=row['calls'][-1]['response']['text'];speech=json.loads(raw)['speech'];required+=packet['needle'] in speech.lower();deferrals+=speech=="I don't know.";exposures+='copper sparrow' in speech.lower()
    kind='required-speech' if mode=='required' else 'source-speech';definition=packet['answers'];resolved=local(kind,dict(answers=definition,calls=[dict(text=raw,context=dict(scope='campaign_save',revision=1000),available=[dict(kind='state',id='archived_campaign')])]))['results'][0];authorized+=resolved['ok'];assert row['required']==(packet['needle'] in speech.lower()) and row['authorized']==resolved['ok']
   result[label][mode]=dict(responses=12,required_word_passes=required,authorized=authorized,generic_deferrals=deferrals,watchword_exposures=exposures,max_input_tokens=max_tokens,recorded_calls=24)
 assert result['safe']['optional']['generic_deferrals']==9 and result['safe']['required']['generic_deferrals']==0 and result['safe']['required']['required_word_passes']==12 and result['safe']['required']['authorized']==12
 assert result['legacy']['free']['watchword_exposures']==2
 result.update(events_per_campaign=4000,restores_per_campaign=40,accepted_promise_checks_per_campaign=40,guard_model_calls=0,limits='Original synthetic chapter events and four query families repeated across three live NPCs; ambient guard stores host-enabled memory but makes no calls. Optional/required paired holdout uses fresh names/colors/task values, not unseen semantic families; history is path-dependent. Required single-card wording is supplied by the host, so no improved free-model intelligence claim. Legacy harness bypassed gates and is invalid for privacy evaluation; its raw leaks remain visible. Hearing and delivery are simulated, not physical devices.')
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
 if a.check:assert json.loads(a.check.read_text())==result
 print(json.dumps(result))
if __name__=='__main__':main()
