"""Fresh paired subjective-turn packets, token-first audit, serial local Qwen cohort."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading,time,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2]
CASES=[('merchant_dream','merchant','workshop','Do you have any dreams?','dream'),('merchant_preference','merchant','workshop','Do you like tea?','tea'),('keeper_hope','keeper','cistern','What do you hope for?','hope')]
def packets():
 out=[];captured=[]
 class Mock(http.server.BaseHTTPRequestHandler):
  def log_message(self,*a):pass
  def do_POST(self):
   v=json.loads(self.rfile.read(int(self.headers['Content-Length'])));captured.append(v);speech=v['format']['properties']['speech'].get('enum',['I hope to make this place welcoming.'])[0];b=json.dumps(dict(message=dict(content=json.dumps(dict(speech=speech))))).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
 mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=mock.serve_forever,daemon=True).start()
 try:
  for name,actor,room,question,expected in CASES:
   for mode in ['baseline','candidate']:
    with tempfile.TemporaryDirectory(prefix='mind-packet-')as tmp:
     d=pathlib.Path(tmp);cfg=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());cfg.update(text='local_ollama',cpu_threads=4);cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(cfg));mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=mode=='candidate';(d/'personalities.json').write_text(json.dumps(mind));child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
     try:
      port=int(re.search(r'127\.0\.0\.1:(\d+)',child.stdout.readline())[1])
      def call(op=None,**args):
       h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();assert r.status==200,(r.status,v);return v['result']if op else v
      for place in ['gallery',room]:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+place))
      call('select',player='player_1',npc=actor);start=len(captured);call('chat',text=question);end=time.monotonic()+5
      while call()['pending_jobs']:assert time.monotonic()<end;time.sleep(.01)
      assert len(captured)==start+1
      req=captured[-1];out.append(dict(case=name,mode=mode,question=question,expected=expected,request=req))
     finally:child.terminate();child.wait(timeout=5)
 finally:mock.shutdown();mock.server_close()
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--tokens-only',action='store_true');p.add_argument('--model');p.add_argument('--holdout',action='store_true');p.add_argument('--stem',default='personality-model');a=p.parse_args();assert a.cap==0;
 if a.holdout:CASES[:]=[('merchant_hobby','merchant','workshop','Do you have any hobbies?','hobby'),('merchant_tea','merchant','workshop','What is your opinion of tea?','tea'),('keeper_future','keeper','cistern','Do you ever dream about the future?','dream')]
 tokenfile=ROOT/'eval'/'studio'/f'{a.stem}-tokens.json'
 if a.tokens_only:
  assert not tokenfile.exists();rows=packets();tokenfile.write_text(json.dumps(dict(cohort=rows,prepared_bytes=[dict(case=r['case'],mode=r['mode'],bytes=len(json.dumps(r['request'],separators=(',',':')).encode()),output_tokens=r['request']['options']['num_predict'])for r in rows],api_spend_usd=0,actual_provider_calls=0,limits='Bytes are measured; exact provider token counts are available only during live evaluation.'),indent=2)+'\n');print(json.dumps(dict(packets=len(rows),token_first=True,api_spend_usd=0)));return
 assert a.model and tokenfile.exists();results=ROOT/'eval'/'studio'/f'{a.stem}-results.json';review=ROOT/'eval'/'studio'/f'{a.stem}-review.json';assert not results.exists()and not review.exists()
 # Avoid overlapping a player's host job; all inference below is serial, CPU four threads.
 with urllib.request.urlopen('http://127.0.0.1:8787/api/state',timeout=3)as f:assert json.load(f)['pending_jobs']==0,'Preview is busy; wait before running cohort.'
 rows=[]
 for c in json.loads(tokenfile.read_text())['cohort']:
  req=c['request'];req['model']=a.model;req['options'].update(num_thread=4,seed=0);start=time.monotonic();request=urllib.request.Request('http://127.0.0.1:11434/api/chat',json.dumps(req).encode(),{'Content-Type':'application/json'})
  with urllib.request.urlopen(request,timeout=180)as f:raw=json.load(f)
  content=raw['message']['content'];r=dict(case=c['case'],mode=c['mode'],question=c['question'],expected=c['expected'],raw=raw,wall_seconds=round(time.monotonic()-start,4));rows.append(r);print(json.dumps(dict(case=c['case'],mode=c['mode'],raw_content=content,wall_seconds=r['wall_seconds'])),flush=True)
 results.write_text(json.dumps(dict(model=a.model,threads=4,rows=rows,api_spend_usd=0,actual_local_calls=len(rows)),indent=2)+'\n')
 scored=[]
 for r in rows:
  try:s=json.loads(r['raw']['message']['content'])['speech']
  except(Exception):s=''
  mental=any(s.lower().startswith(p)for p in ['i hope ','i dream ','i wish ','i prefer ','i value ','i feel ','i think ','i believe ','i would like ']);complete=s.endswith(('.','!'))and len(s)<=240
  relevant=mental and (r['expected']!='tea'or'tea'in s.lower()) and (r['expected']!='hobby'or any(w in s.lower()for w in ['sketch','draw','read','game','tinker','design','repair','craft','care','play','wood']));
  import importlib.util
  spec=importlib.util.spec_from_file_location('views',ROOT/'eval/studio/viewpoint_cases.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
  try:module.line(s);accepted=True
  except AssertionError:accepted=False
  scored.append(dict(case=r['case'],mode=r['mode'],mental=mental,complete=complete,accepted=accepted,relevant=relevant,chars=len(s),speech=s,prompt_tokens=r['raw'].get('prompt_eval_count'),output_tokens=r['raw'].get('eval_count')))
 review.write_text(json.dumps(dict(scores=scored,summary={m:dict(relevant=sum(r['relevant']for r in scored if r['mode']==m),complete=sum(r['complete']for r in scored if r['mode']==m),accepted=sum(r['accepted']for r in scored if r['mode']==m),n=len(CASES))for m in ['baseline','candidate']},api_spend_usd=0,limits='Small original three-question paired cohort; deterministic lexical relevance scoring, raw outputs retained. No claim of general hallucination prevention.'),indent=2)+'\n');print(review.read_text())
if __name__=='__main__':main()
