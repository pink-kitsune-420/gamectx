"""Frozen follow-up to the first paired prototype, with scene-independent fable seeds."""
import argparse,copy,http.client,json,pathlib,sys,time
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'));import expression_cases as ref
p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--tokens-only',action='store_true');a=p.parse_args();assert a.cap==0
old=json.loads((ROOT/'eval/expression-model-results.json').read_text());pack=[]
for r in old['results']:
 if r['mode']!='candidate':continue
 request=copy.deepcopy(r['request']);m=r['expression'];request['messages'][0]['content']+=ref.EXTRA[m]
 profile=json.loads(request['messages'][1]['content']);profile['personality']['fiction_seed']='A little stone that wants to sing with a stream.'if m=='fiction'else'A lantern that lends a moth a spark.';request['messages'][1]['content']=ref.enc(profile)
 schema=request['format']['properties']['speech'];request['format']['properties']['speech']=dict(type='array',minItems=2 if m=='fiction'else 1,maxItems=2,items=schema);request['messages'].insert(-1,dict(role='system',content='Return speech as an array of up to two complete short sentences. For a fable use two: Imagine starts the problem, In this tale, starts its resolution. For social conversation, a second sentence may ask a relevant question.'));request['options']['num_predict']=192;pack.append(dict(case=r['case'],expression=m,request=request))
prefix=ROOT/'eval/expression-model-v2';tokenfile=pathlib.Path(str(prefix)+'-tokens.json')
if a.tokens_only:
 v=dict(provider_calls=0,api_spend_usd=0,method='Prepared content UTF-8 bytes, not tokenizer counts.',cases=[dict(case=r['case'],content_bytes=sum(len(m['content'].encode())for m in r['request']['messages']))for r in pack]);assert not tokenfile.exists();tokenfile.write_text(json.dumps(v,indent=2)+'\n');print(json.dumps(v));raise SystemExit
assert tokenfile.exists();out=pathlib.Path(str(prefix)+'-results.json');assert not out.exists();results=[]
for r in pack:
 c=http.client.HTTPConnection('127.0.0.1',11434,timeout=180);start=time.monotonic();c.request('POST','/api/chat',json.dumps(r['request']),{'Content-Type':'application/json'});response=c.getresponse();raw=json.loads(response.read());c.close();assert response.status==200;valid=False
 try:
  lines=json.loads(raw['message']['content'])['speech'];assert isinstance(lines,list)and 1<=len(lines)<=2
  for line in lines:ref.oracle(dict(op='validate',args=dict(mode=r['expression'],speech=line)))
  valid=True
 except(AssertionError,KeyError,ValueError,TypeError):lines=raw
 row=dict(case=r['case'],request=r['request'],raw_response=raw,contract_valid=valid,seconds=round(time.monotonic()-start,3));results.append(row);print(json.dumps(dict(case=r['case'],speech=lines,valid=valid)),flush=True)
out.write_text(json.dumps(dict(results=results,api_spend_usd=0),indent=2)+'\n')
