"""Fresh paired local-only 3B follow-up. Raw outputs retained; no quality inflation."""
import argparse,copy,http.client,json,pathlib,re,subprocess,time
ROOT=pathlib.Path(__file__).resolve().parents[2]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--model',required=True);p.add_argument('--tokens-only',action='store_true');p.add_argument('--output-prefix',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 original=json.loads((ROOT/'target/conversation-extensions/requests.json').read_text());pack=[]
 for i,c in enumerate(original):
  for mode in ['baseline','chunks']:
   r=copy.deepcopy(c);r['model']=a.model;r['options']['num_thread']=4;r['keep_alive']=0
   if mode=='baseline':
    if r['format']['properties']['speech'].get('type')=='array':r['format']['properties']['speech']=r['format']['properties']['speech']['items']
    r['format']['properties'].pop('player_summary',None);r['format']['required']=[k for k in r['format']['required']if k!='player_summary'];r['messages']=[m for m in r['messages']if not m['content'].startswith('Return speech JSON. Speech ')];r['options']['num_predict']=64
   # Fresh phrasing on the same frozen host context. Labels were declared before generation.
   if i==0:r['messages'][-1]['content']='Current request: What kind of place would you hope to make for your visitors?'
   pack.append(dict(case='visitor_hopes'if i==0 else'long_corrected_preference',mode=mode,request=r))
 base=a.output_prefix or ROOT/'eval/conversation-extensions-model';tokenpath=pathlib.Path(str(base)+'-tokens.json')
 if a.tokens_only:
  result=dict(version=1,api_spend_usd=0,provider_calls=0,counts_are='Prepared UTF-8 bytes and an intentionally rough ceil(bytes/4) token proxy, not provider tokenization.',cases=[dict(case=c['case'],mode=c['mode'],request_bytes=len(json.dumps(c['request'],ensure_ascii=False,separators=(',',':')).encode()),content_bytes=sum(len(m['content'].encode())for m in c['request']['messages']),rough_tokens=(sum(len(m['content'].encode())for m in c['request']['messages'])+3)//4,output_cap=c['request']['options']['num_predict'],context_cap=c['request']['options']['num_ctx'])for c in pack]);assert not tokenpath.exists();tokenpath.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));return
 assert tokenpath.exists(),'Run token-first check before inference.';assert not pathlib.Path(str(base)+'-results.json').exists(),'Choose a fresh --output-prefix; existing raw results stay frozen.';assert not pathlib.Path(str(base)+'-review.json').exists();results=[]
 for c in pack:
  h=http.client.HTTPConnection('127.0.0.1',11434,timeout=180);start=time.monotonic();h.request('POST','/api/chat',json.dumps(c['request']),{'Content-Type':'application/json'});response=h.getresponse();raw=response.read().decode();h.close();assert response.status==200,(response.status,raw[:100]);v=json.loads(raw);content=v.get('message',{}).get('content','');parsed=None;valid=False
  try:
   parsed=json.loads(content);speech=parsed['speech']if c['mode']=='chunks'else[parsed['speech']];r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(dict(kind='studio-message-chunks',args=dict(op='reply',args=dict(speech=speech,max_chunks=3)))),capture_output=True,text=True);valid=r.returncode==0
  except(ValueError,KeyError,TypeError):pass
  results.append(dict(case=c['case'],mode=c['mode'],request=c['request'],raw_response=v,parsed=parsed,complete_sentence_packet=valid,seconds=round(time.monotonic()-start,3)));print(json.dumps(dict(case=c['case'],mode=c['mode'],complete=valid,seconds=results[-1]['seconds'])),flush=True)
 out=pathlib.Path(str(base)+'-results.json');assert not out.exists();out.write_text(json.dumps(dict(model=a.model,threads=4,api_spend_usd=0,results=results),ensure_ascii=False,indent=2)+'\n')
 review=dict(cases=len(results),api_spend_usd=0,by_mode={m:dict(complete_packets=sum(x['complete_sentence_packet']for x in results if x['mode']==m),total=sum(x['mode']==m for x in results))for m in ['baseline','chunks']},limits='Two paired local examples, no statistical quality or speed gain claim. Full requests/raw responses retained. Summaries are unverified and originals remain in context. Hardware audio was not measured.')
 f=pathlib.Path(str(base)+'-review.json');assert not f.exists();f.write_text(json.dumps(review,indent=2)+'\n');print(json.dumps(review))
if __name__=='__main__':main()
