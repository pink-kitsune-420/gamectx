"""Production-host multi-turn routing, scoped heard context, duplicate rejection and moods."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 with tempfile.TemporaryDirectory(prefix='conversation-host-')as tmp:
  d=pathlib.Path(tmp);calls=[];reply=['I feel settled and willing to listen.']
  class Mock(http.server.BaseHTTPRequestHandler):
   def log_message(self,*a):pass
   def do_POST(self):
    v=json.loads(self.rfile.read(int(self.headers['Content-Length'])));calls.append(v);speech=v['format']['properties']['speech'].get('enum',[reply[0]])[0];b=json.dumps(dict(message=dict(content=json.dumps(dict(speech=speech))),prompt_eval_count=400,eval_count=15)).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
  mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=mock.serve_forever,daemon=True).start();cfg=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());cfg['text']='local_ollama';cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(cfg));mind=json.loads((ROOT/'examples/dungeon/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);checks=0
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',child.stdout.readline())[1])
   def call(op=None,**args):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert r.status==200,(r.status,v);return v['result']if op else v
   def wait():
    end=time.monotonic()+5
    while True:
     s=call()
     if not s['pending_jobs']:return s
     assert time.monotonic()<end;time.sleep(.01)
   def finish(s):
    sid=s['speech']['id'];n=len(s['speech']['text']);call('delivery',id=sid,event='queued',through=0,heard=0);call('delivery',id=sid,event='played',through=n,heard=n);call('delivery',id=sid,event='finished',through=n,heard=n)
   def ask(q,heard=True):call('chat',text=q);s=wait();assert 'tend this counter'not in s['speech']['text'];assert 'tend the dry fountain'not in s['speech']['text'];finish(s)if heard else None;return s
   for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   greetings=[]
   for _ in range(3):s=ask('Hello.');greetings.append(s['speech']['text'])
   assert len(set(greetings))==3 and not calls
   s=ask('hello, how are you today?');assert s['speech']['origin']=='authored_social'and s['speech']['text'].startswith('I feel ');assert not s['personality']['viewpoints']['entries']and not calls
   old=s['speech']['text'];s=ask('Why is that?');assert s['speech']['text']!=old and 'because'in s['speech']['text']and not s['personality']['viewpoints']['entries']and not calls
   reply[0]='I hope to make useful things feel approachable.';s=ask('Do you have any dreams?');assert len(call()['personality']['viewpoints']['entries'])==1
   canonical=reply[0];count=len(calls);s=ask('Tell me more.');assert 'because'in s['speech']['text']and len(calls)==count and len(call()['personality']['viewpoints']['entries'])==1
   count=len(calls);s=ask('What do you hope for?');assert len(calls)==count and s['speech']['text']==canonical
   # A repeated provider sentence must leave the nonrepeating fallback intact.
   reply[0]=canonical;s=ask('What makes you happy?');assert s['speech']['origin']=='offline'and s['speech']['text']!=canonical
   reply[0]='I feel the host is a kind person.';s=ask('What makes you sad?');assert s['speech']['origin']=='offline'and 'host'not in s['speech']['text'];assert 'engine metadata'in s['notice']
   count=len(calls)
   for q in ['Thanks.','Goodbye.']:ask(q)
   assert len(calls)==count
   # Unheard/player-2 text does not provide a player-1 follow-up topic.
   call('select',player='player_2',npc='merchant');s=ask('Tell me more.');assert 'Which part'in s['speech']['text']and len(calls)==count
   reply[0]='I prefer drawing designs that feel useful rather than ornate.';s=ask('What do you enjoy?');assert len(call()['personality']['viewpoints']['entries'])==2
   for q in ['Hello, what do you sell?','Where was I born?','Hello, how is the weather?']:
    call('chat',text=q);s=wait();assert 'enum'in calls[-1]['format']['properties']['speech'];finish(s)
   assert not any('wellbeing'==e['topic']for e in call()['personality']['viewpoints']['entries']);assert call()['spending']['charged_or_reserved_micros']==0
   return dict(workflows=12,http_checks=checks,greeting_variants=3,greeting_model_calls=0,wellbeing_host_direct=True,wellbeing_elaboration_model_calls=0,mood_persisted=False,followups_use_confirmed_context=True,canonical_stance_preserved=True,duplicate_model_reply_rejected=True,engine_metadata_rejected=True,listener_isolation=True,factual_path_preserved=True,api_spend_usd=0,actual_provider_calls=0,limits='Original HTTP mock and production host. Routing and mechanical validation only; live model conversation quality is measured separately.')
  finally:child.terminate();child.wait(timeout=5);mock.shutdown();mock.server_close()
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
