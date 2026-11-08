"""Paired local-only expressive compiler follow-up. Full requests/outputs frozen."""
import argparse,copy,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading,time,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()];import expression_cases as ref
CASES=[('plans','merchant','Do you have any plans?'),('fable','keeper','Can you tell me a story?'),('company','merchant','I had a long day; keep me company.')]
def capture():
 captured=[]
 class Mock(http.server.BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_POST(self):
   v=json.loads(self.rfile.read(int(self.headers['Content-Length'])));captured.append(v);data=json.dumps(dict(message=dict(content=json.dumps(dict(speech='I do not have a specific answer to that; could you ask about one thing?'))))).encode();self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
 server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=server.serve_forever,daemon=True).start();profiles=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());profiles['enabled']=True;pack=[]
 with tempfile.TemporaryDirectory(prefix='expression-pairs-')as tmp:
  d=pathlib.Path(tmp);(d/'personalities.json').write_text(json.dumps(profiles));c=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());c['text']='local_ollama';c['cpu_threads']=4;c['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{server.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(c));h=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',h.stdout.readline())[1])
   def call(op=None,**args):
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=5);c.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=c.getresponse();v=json.loads(r.read());c.close();assert r.status==200,(r.status,v);return v
   for name,actor,question in CASES:
    target='workshop'if actor=='merchant'else'cistern';s=call()
    for room in (['gallery',target]if s['game']['room']=='room:entry'else['gallery',target]if s['game']['room']!='room:'+target else[]):
     s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
    call('chat',text=question);end=time.monotonic()+4
    while call()['pending_jobs']:assert time.monotonic()<end;time.sleep(.01)
    baseline=captured[-1];mode=ref.route(question)['mode'];profile=next(x for x in profiles['profiles']if x['actor']==actor)
    args=dict(mode=mode,speaker=actor,listener='player_1',profile=profile,scene=dict(location=target),context=dict(trust=15,atmosphere='calm',episodes=[],promises=[]),history=[],input=question)
    candidate=ref.oracle(dict(op='packet',args=args));candidate=dict(model=baseline['model'],messages=candidate['messages'],format=candidate['speech_schema'],stream=False,keep_alive=0,options=dict(num_predict=128,num_thread=4,num_ctx=2048,temperature=0))
    pack.extend([dict(case=name,mode='baseline',expression=mode,request=baseline),dict(case=name,mode='candidate',expression=mode,request=candidate)])
  finally:h.terminate();h.wait(timeout=5);server.shutdown();server.server_close()
 return pack

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--model',required=True);p.add_argument('--tokens-only',action='store_true');a=p.parse_args();assert a.cap==0;prefix=ROOT/'eval/expression-model';tokens=pathlib.Path(str(prefix)+'-tokens.json');requests=ROOT/'target/expression-requests.json'
 if a.tokens_only:
  pack=capture();requests.write_text(json.dumps(pack,ensure_ascii=False,indent=2)+'\n');r=dict(provider_calls=0,api_spend_usd=0,method='Prepared UTF-8 content bytes, not tokenization.',cases=[dict(case=v['case'],mode=v['mode'],content_bytes=sum(len(m['content'].encode())for m in v['request']['messages']),enum_locked='enum'in v['request'].get('format',{}).get('properties',{}).get('speech',{}))for v in pack]);assert not tokens.exists();tokens.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r));return
 assert tokens.exists()and requests.exists();out=pathlib.Path(str(prefix)+'-results.json');assert not out.exists();results=[]
 for case in json.loads(requests.read_text()):
  v=copy.deepcopy(case['request']);v['model']=a.model;v['options']['num_thread']=4;start=time.monotonic();c=http.client.HTTPConnection('127.0.0.1',11434,timeout=180);c.request('POST','/api/chat',json.dumps(v),{'Content-Type':'application/json'});r=c.getresponse();raw=json.loads(r.read());c.close();assert r.status==200;content=raw.get('message',{}).get('content','');valid=False
  try:reply=json.loads(content)['speech'];ref.oracle(dict(op='validate',args=dict(mode=case['expression'],speech=reply)));valid=True
  except(AssertionError,ValueError,KeyError,TypeError):reply=content
  result=dict(case=case['case'],mode=case['mode'],expression=case['expression'],request=v,raw_response=raw,contract_valid=valid,seconds=round(time.monotonic()-start,3));results.append(result);print(json.dumps(dict(case=result['case'],mode=result['mode'],contract_valid=valid,seconds=result['seconds'],speech=reply)),flush=True)
 out.write_text(json.dumps(dict(model=a.model,threads=4,api_spend_usd=0,results=results),ensure_ascii=False,indent=2)+'\n');review=dict(pairs=3,api_spend_usd=0,contract_valid={m:sum(r['contract_valid']for r in results if r['mode']==m)for m in ['baseline','candidate']},limits='Three paired prompts, mechanical contract checks only; manual semantic review required. This does not establish general intelligence or hallucination-rate improvements. Baseline is a captured real pre-change host request.');f=pathlib.Path(str(prefix)+'-review.json');assert not f.exists();f.write_text(json.dumps(review,indent=2)+'\n');print(json.dumps(review))
if __name__=='__main__':main()
