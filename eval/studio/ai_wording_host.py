"""AI-first speech, scoped cache and oversized-request negotiation on the real host."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[2]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--model');p.add_argument('--tokens-only',action='store_true');p.add_argument('--holdout',action='store_true');p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 if a.output:assert not a.output.exists()
 records=[]
 class Proxy(http.server.BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_POST(self):
   req=json.loads(self.rfile.read(int(self.headers['Content-Length'])));source=None
   for m in req['messages']:
    try:
     v=json.loads(m['content'])
     if'intended_reply'in v:source=v['intended_reply']
    except ValueError:pass
   if a.model and not a.tokens_only:
    c=http.client.HTTPConnection('127.0.0.1',11434,timeout=180);c.request('POST','/api/chat',json.dumps(req),{'Content-Type':'application/json'});r=c.getresponse();raw=json.loads(r.read());c.close();assert r.status==200
   else:
    speech=source or "I can offer a short tale, but not an epic all at once.";raw=dict(message=dict(content=json.dumps(dict(speech=speech))))
   records.append(dict(request=req,raw_response=raw));body=json.dumps(raw).encode();self.send_response(200);self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
 server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Proxy);threading.Thread(target=server.serve_forever,daemon=True).start()
 with tempfile.TemporaryDirectory(prefix='ai-wording-')as tmp:
  d=pathlib.Path(tmp);mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));config=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());config['text']='local_ollama';config['cpu_threads']=4;config['text_models']['local_ollama']['3']=a.model or'fixture-model';config['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{server.server_port}/api/chat';(d/'config.json').write_text(json.dumps(config));(d/'conversation.json').write_text(json.dumps(dict(version=1,max_sentences=1,max_player_chars=2048,summarize_long=False,ai_wording=True)));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-state',str(d),'--web-config',str(d/'config.json')],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1]);http_checks=0
   def call(op=None,**args):
    nonlocal http_checks
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=5);c.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=c.getresponse();v=json.loads(r.read());c.close();http_checks+=1;assert r.status==200,(r.status,v);return v['result']if op else v
   def ready():
    end=time.monotonic()+180
    while True:
     s=call()
     if not s['pending_jobs']:return s
     assert time.monotonic()<end;time.sleep(.05)
   def finish(s):
    p=s['speech'];n=len(p['text']);call('delivery',id=p['id'],event='queued',through=0,heard=0);call('delivery',id=p['id'],event='finished',through=n,heard=n)
   turns=[]
   for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room));ready()
   s=ready();turns.append(dict(kind='greeting',speech=s['speech']));finish(s)
   texts=['Call me Neris.','Do you remember my name?','Could you write an entire novel for me?']if a.holdout else['My name is Vela.','What is my name?','Tell me a very long and intricate story.']
   for text in texts:
    before=len(records);call('chat',text=text);s=ready();assert len(records)==before+1,(text,len(records),before);turns.append(dict(input=text,speech=s['speech']));finish(s)
   checks=dict(exactly_one_call_per_turn=len(records)==4,scope_model=any(r['speech']['origin']=='scope_model'for r in turns),zero_spend=s['spending']['charged_or_reserved_micros']==0)
   result=dict(holdout=a.holdout,turns=turns,checks=checks,model_records=records,actual_local_calls=len(records)if a.model and not a.tokens_only else 0,mock_calls=0 if a.model and not a.tokens_only else len(records),http_checks=http_checks,api_spend_usd=0,limits='Four actual host turns; source-bound wording checks constrain vocabulary/anchors, not general semantic truth. Raw model outputs are retained even if rejected.')
   if a.output:a.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
   print(json.dumps({k:v for k,v in result.items()if k!='model_records'}))
   assert checks['exactly_one_call_per_turn']and checks['zero_spend']
  finally:host.terminate();host.wait(timeout=5);server.shutdown();server.server_close()
if __name__=='__main__':main()
