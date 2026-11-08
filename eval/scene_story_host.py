"""Actual host location/sense/novelty journey; offline mocks or installed local model."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--model');p.add_argument('--tokens-only',action='store_true');p.add_argument('--restore-check',action='store_true');p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 if a.output:assert not a.output.exists()
 records=[]
 class Proxy(http.server.BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_POST(self):
   req=json.loads(self.rfile.read(int(self.headers['Content-Length'])));source=None;plot=None
   for m in req['messages']:
    try:
     v=json.loads(m['content']);source=v.get('intended_reply',source);plot=v.get('personality',{}).get('fiction_seed',plot)
    except(ValueError,AttributeError):pass
   if a.model and not a.tokens_only:
    c=http.client.HTTPConnection('127.0.0.1',11434,timeout=180);c.request('POST','/api/chat',json.dumps(req),{'Content-Type':'application/json'});r=c.getresponse();raw=json.loads(r.read());c.close();assert r.status==200
   else:
    speech=source or dict(opening='Imagine '+plot['problem'].rstrip('.')+'.',ending='In this tale, '+plot['ending'].rstrip('.')+'.');raw=dict(message=dict(content=json.dumps(dict(speech=speech))))
   records.append(dict(request=req,plot=plot,raw_response=raw));b=json.dumps(raw).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
 proxy=http.server.ThreadingHTTPServer(('127.0.0.1',0),Proxy);threading.Thread(target=proxy.serve_forever,daemon=True).start()
 with tempfile.TemporaryDirectory(prefix='scene-story-')as tmp:
  d=pathlib.Path(tmp);m=json.loads((ROOT/'examples/dungeon/web/personalities.json').read_text());m['enabled']=True;(d/'personalities.json').write_text(json.dumps(m));config=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());config['text']='local_ollama';config['cpu_threads']=4;config['text_models']['local_ollama']['3']=a.model or'fixture';config['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{proxy.server_port}/api/chat';(d/'config.json').write_text(json.dumps(config));(d/'conversation.json').write_text(json.dumps(dict(version=1,max_sentences=2,max_player_chars=2048,summarize_long=False,ai_wording=True)));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-state',str(d),'--web-config',str(d/'config.json')],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);checks=0;turns=[]
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1])
   def call(op=None,**args):
    nonlocal checks
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=5);c.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=c.getresponse();v=json.loads(r.read());c.close();checks+=1;assert r.status==200,(r.status,v);return v['result']if op else v
   def ready():
    end=time.monotonic()+180
    while True:
     s=call()
     if not s['pending_jobs']:return s
     assert time.monotonic()<end;time.sleep(.05)
   def deliver():
    lines=[]
    for _ in range(3):
     s=call()['speech']
     if not s or s['delivery']['done']:break
     lines.append(dict(text=s['text'],origin=s['origin'],segment=s['segment']));n=len(s['text']);call('delivery',id=s['id'],event='queued',through=0,heard=0);call('delivery',id=s['id'],event='finished',through=n,heard=n)
    return lines
   def chat(text):
    call('chat',text=text);ready();reply=deliver();turns.append(dict(input=text,reply=reply));return' '.join(l['text']for l in reply)
   for room in ['gallery','cistern']:
    s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room));ready();deliver()
   assert'cistern'in chat('Where are we?').lower();assert'dry fountain'in chat('Look around you.').lower()
   count=3 if a.model else 12
   for i in range(count):chat('Could you tell me a little story?'if i==0 else'Another story please.')
   restored=None
   if a.restore_check:
    assert not a.model;call('save',slot=0);chat('Another story please.');expected=records[-1]['plot'];call('load',slot=0);ready();chat('Another story please.');restored=records[-1]['plot']==expected;assert restored
   plots=[r['plot']for r in records if r['plot']];unique=len({json.dumps(p,sort_keys=True)for p in plots});s=call();assert s['spending']['charged_or_reserved_micros']==0
   result=dict(rotation_after_restore=restored,turns=turns,records=records,story_requests=count,distinct_selected_plots=unique,model_story_packets_adopted=sum(t['reply'][0]['origin']=='fiction_model'for t in turns[2:]),http_checks=checks,actual_local_calls=len(records)if a.model and not a.tokens_only else 0,mock_calls=0 if a.model and not a.tokens_only else len(records),api_spend_usd=0,limits='Location/senses use authoritative host data. Seed uniqueness measures selected plots, not semantic novelty. Grammar acceptance does not grade plot quality; raw outputs are retained. No physical audio is tested here.')
   if a.output:a.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
   print(json.dumps({k:v for k,v in result.items()if k not in['records','turns']}));assert a.model or unique==12
  finally:host.terminate();host.wait(timeout=5);proxy.shutdown();proxy.server_close()
if __name__=='__main__':main()
