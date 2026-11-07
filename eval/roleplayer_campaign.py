"""Predeclared roleplayer journey through the real host; offline or local 3B only."""
import argparse,http.client,http.server,threading,json,pathlib,re,subprocess,tempfile,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--model');p.add_argument('--output',type=pathlib.Path);p.add_argument('--tokens-only',action='store_true');a=p.parse_args();assert a.cap==0
 if a.output:assert not a.output.exists()
 turns=[];checks=0;records=[];proxy=None
 if a.model:
  class Proxy(http.server.BaseHTTPRequestHandler):
   def log_message(self,*_):pass
   def do_POST(self):
    request=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
    if a.tokens_only:
     rules=request['messages'][0]['content'];lines=['Imagine a pebble too shy to sing.','In this tale, a stream offers it a rhythm.']if rules.startswith('Tell a tiny')else['I would like to sketch a useful design.']if rules.startswith('Answer the current')else['That sounds tiring; would a quiet story help?'];speech=lines if request['format']['properties']['speech']['type']=='array'else lines[0];response=dict(message=dict(content=json.dumps(dict(speech=speech))))
    else:
     connection=http.client.HTTPConnection('127.0.0.1',11434,timeout=180);connection.request('POST','/api/chat',json.dumps(request),{'Content-Type':'application/json'});r=connection.getresponse();response=json.loads(r.read());connection.close();assert r.status==200
    records.append(dict(request=request,raw_response=response));data=json.dumps(response).encode();self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
  proxy=http.server.ThreadingHTTPServer(('127.0.0.1',0),Proxy);threading.Thread(target=proxy.serve_forever,daemon=True).start()

 with tempfile.TemporaryDirectory(prefix='roleplayer-')as tmp:
  d=pathlib.Path(tmp);mind=json.loads((ROOT/'examples/dungeon/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));(d/'conversation.json').write_text(json.dumps(dict(version=1,max_sentences=2,max_player_chars=2048,summarize_long=False)));cfg=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());cfg['cpu_threads']=4
  if a.model:cfg['text']='local_ollama';cfg['text_models']['local_ollama']['3']=a.model;cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{proxy.server_port}/api/chat'
  (d/'providers.json').write_text(json.dumps(cfg));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-state',str(d),'--web-config',str(d/'providers.json')],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
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
     s=call();reply=s['speech']
     if not reply or reply['delivery']['done']:break
     lines.append(dict(text=reply['text'],origin=reply['origin']));n=len(reply['text']);call('delivery',id=reply['id'],event='queued',through=0,heard=0);call('delivery',id=reply['id'],event='finished',through=n,heard=n)
    return lines
   def send(text,contains=None,absent=None):
    before=call()['game'];start=time.monotonic();result=call('chat',text=text);s=ready();lines=deliver();joined=' '.join(v['text']for v in lines)
    if contains:assert contains.lower()in joined.lower(),(text,joined)
    if absent:assert absent.lower()not in joined.lower(),(text,joined)
    assert s['game']==before,(text,'conversation changed world');row=dict(player=s['player'],npc=s['npc'],input=text,result=result,reply=lines,seconds=round(time.monotonic()-start,3));turns.append(row);print(json.dumps(row),flush=True);return joined
   def go(room):s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room));ready();deliver()
   go('gallery');go('workshop')
   send('My name is Vira.',contains='Vira');send('My name is Vela, not Vira.',contains='Vela');send('I prefer quiet music.',contains='quiet music')
   send('Do you have any plans?',absent='specific answer');send('Can you tell me a story?',absent='specific answer');send('I had a long day; keep me company.',absent='specific answer')
   send('What do you see?',contains='counter');send('What do you hear?',contains='hear you')
   call('save',slot=0)
   for _ in range(12):call('chat',text='Hello.');ready();deliver()
   send('Do you remember my name?',contains='Vela',absent='your name is Vira');send('What do I prefer?',contains='quiet music')
   call('select',player='player_2',npc='merchant');send('Do you remember my name?',contains='not told');send('Call me Sera.',contains='Sera');send('I like loud drums.',contains='loud drums');send('What is my name?',contains='Sera',absent='Vela')
   call('load',slot=0);ready();send('What is my name?',contains='Vela',absent='Sera');send('What do I prefer?',contains='quiet music')
   go('gallery');go('cistern');send('What is my name?',contains='not told');send('My name is Vela.',contains='Vela');send('What do you see?',contains='dry fountain');send('Could you tell me a little fable?',absent='specific answer');send('What are you looking forward to?',absent='specific answer')
   # Off-screen perspective, resources and gates must remain authoritative.
   before=call()['game'];call('chat',text='*takes the glass star and opens the vault*');assert call()['game']==before
   s=call();text_calls=[r for r in s['setup']['activity']if r['channel']=='text'];r=dict(version=1,turns=turns,noise_turns=12,http_checks=checks,local_live=bool(a.model),retained_text_activity=text_calls,model_records=records,real_model_calls=0 if a.tokens_only else len(records),tokens_only=a.tokens_only,api_spend_usd=0,checks=dict(corrected_name_survives_noise=True,player_isolation=True,npc_isolation=True,branch_restore=True,preference_recalled=True,senses_grounded=True,compound_action_did_not_execute=True),limits='Scripted roleplayer with actual host delivery receipts, not physical audio. Live output quality requires manual review; no universal language understanding or hallucination-rate claim. Activity is bounded, so it may omit older calls.')
   assert s['spending']['charged_or_reserved_micros']==0
   if a.output:a.output.write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n')
   print(json.dumps({k:v for k,v in r.items()if k not in ['turns','retained_text_activity','model_records']}))
  finally:
   host.terminate();host.wait(timeout=5)
   if proxy:proxy.shutdown();proxy.server_close()
if __name__=='__main__':main()
