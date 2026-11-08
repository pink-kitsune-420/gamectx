"""Fresh held-out questions on post-fix compiler packets; one local call per turn."""
import argparse,copy,http.client,json,pathlib,re,subprocess,time
ROOT=pathlib.Path(__file__).resolve().parents[2];p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);a=p.parse_args();assert a.cap==0
pack=json.loads((ROOT/'eval/studio/roleplayer-followup-tokens.json').read_text())['model_records'];out=ROOT/'eval/studio/roleplayer-followup-results.json';assert not out.exists();results=[]
for i,record in enumerate(pack):
 request=copy.deepcopy(record['request']);rules=request['messages'][0]['content'];kind='fiction'if rules.startswith('Tell a tiny')else'social'if rules.startswith('Respond naturally')else None
 if kind is None:continue
 request['messages'][-1]['content']=('Current request:\n[player_1 -> '+('merchant'if i<3 else'keeper')+'] '+('Could you tell a new little tale about helping someone?'if kind=='fiction'else'I prefer quiet music, but do not decide my feelings for me; can we talk?'))
 c=http.client.HTTPConnection('127.0.0.1',11434,timeout=180);start=time.monotonic();c.request('POST','/api/chat',json.dumps(request),{'Content-Type':'application/json'});r=c.getresponse();raw=json.loads(r.read());c.close();assert r.status==200;valid=False
 try:
  lines=json.loads(raw['message']['content'])['speech'];assert isinstance(lines,list)
  for line in lines:
   check=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(dict(kind='studio-expression',args=dict(op='validate',args=dict(mode=kind,speech=line)))),capture_output=True,text=True);assert check.returncode==0
  check=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(dict(kind='studio-message-chunks',args=dict(op='reply',args=dict(speech=lines,max_chunks=2)))),capture_output=True,text=True);valid=check.returncode==0
 except(AssertionError,ValueError,KeyError,TypeError):lines=raw
 row=dict(case='story'if kind=='fiction'else'player_agency',kind=kind,request=request,raw_response=raw,structurally_valid=valid,seconds=round(time.monotonic()-start,3));results.append(row);print(json.dumps(dict(case=row['case'],valid=valid,speech=lines)),flush=True)
out.write_text(json.dumps(dict(results=results,api_spend_usd=0,limits='Three fresh local questions on actual compiled packets with controlled prior heard replies. Manual review is required; structural validity is not literary quality or entailment.'),ensure_ascii=False,indent=2)+'\n')
