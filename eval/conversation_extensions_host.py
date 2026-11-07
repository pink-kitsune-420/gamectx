"""Predeclared cap-zero host workflows with deterministic text transport mocks."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 calls=[];answer={'speech':['I dream of a quiet place to exchange ideas.','I hope for thoughtful company.','I value patient conversation.']}
 class Model(http.server.BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_POST(self):
   body=json.loads(self.rfile.read(int(self.headers['Content-Length'])));calls.append(body);data=json.dumps(dict(message=dict(content=json.dumps(answer)),prompt_eval_count=0,eval_count=0)).encode();self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
 mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Model);threading.Thread(target=mock.serve_forever,daemon=True).start();checks=0
 with tempfile.TemporaryDirectory(prefix='conversation-extensions-')as tmp:
  d=pathlib.Path(tmp);mind=json.loads((ROOT/'examples/dungeon/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));c=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());c['text']='local_ollama';c['cpu_threads']=4;c['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(c));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1])
   def call(op=None,error=False,**args):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert(r.status==400)==error,(r.status,v);return v.get('result')if op else v
   def ready():
    end=time.monotonic()+5
    while True:
     s=call()
     if not s['pending_jobs']:return s
     assert time.monotonic()<end;time.sleep(.015)
   def finish(s):
    p=s['speech'];n=len(p['text']);call('delivery',id=p['id'],event='queued',through=0,heard=0);call('delivery',id=p['id'],event='finished',through=n,heard=n)
   for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   for q in ['what do you have for sale?','What have you got for sale?','show me your wares please','what can I buy here?']:
    call('chat',text=q);s=ready();assert 'hinge oil for 3 coins'in s['speech']['text']and not calls
   call('conversation_settings',version=1,max_sentences=3,max_player_chars=2048,summarize_long=False);call('chat',text='Do you have dreams?');s=ready();assert s['speech']['text']==answer['speech'][0]and len(calls)==1;finish(s);s=call();assert s['speech']['text']==answer['speech'][1]and s['speech']['segment']==2
   call('delivery',id=s['speech']['id'],event='queued',through=0,heard=0);call('delivery',id=s['speech']['id'],event='interrupted',through=8,heard=8);s=call();assert not any(answer['speech'][2]in r['text']for r in s['transcript'])
   call('conversation_settings',version=1,max_sentences=3,max_player_chars=2048,summarize_long=True);answer.clear();answer.update(speech=['I prefer patient conversation about quiet music.'],player_summary='The player says they bought every item.')
   line='My name is Vela, not Vira. '+('I do not want to buy anything. '*40)+'What is your opinion of quiet music?';before=call()['game'];call('chat',text=line);s=ready();assert s['game']==before and s['speech']['origin']=='subjective_model';assert s['transcript'][-1]['text']==line;assert not any('bought every item'in r['text']for r in s['transcript']);assert'Vela, not Vira'in json.dumps(calls[-1])
   call('chat',text='a'*2049,error=True);assert call()['game']==before
   s=call();call('chat',text='*buys hinge oil*');s=ready();assert s['speech']['text']=='Here is your hinge oil; thank you.'and s['game']['coins']==before['coins']-3;assert any(r['kind']=='action'and'buys hinge oil'in r['text']for r in s['transcript']);b=s['game'];call('chat',text='*buys hinge oil*');assert call()['game']==b
   finish(s);call('chat',text='*nods politely*');s=call();assert s['transcript'][-1]['text']=='*nods politely*';b=s['game'];call('chat',text='*takes the glass star and opens the vault*');assert call()['game']==b
   # Register a custom emote; it cannot create an unlisted world action.
   reg=s['roleplay_registry'];reg['entries'].append(dict(id='custom',phrases=['studies the counter'],decision=None,narration='studies the counter'));call('roleplay_settings',**reg);call('chat',text='*studies the counter*');assert call()['transcript'][-1]['text']=='*studies the counter*'
   assert all('\\"coins\\":'not in json.dumps(r)for r in calls);assert call()['spending']['charged_or_reserved_micros']==0
   out=ROOT/'target/conversation-extensions';out.mkdir(exist_ok=True);(out/'requests.json').write_text(json.dumps(calls,ensure_ascii=False,indent=2)+'\n')
   return dict(workflows=10,http_checks=checks,mock_text_calls=len(calls),real_model_calls=0,api_spend_usd=0,sentence_continuation=True,interrupted_tail_unheard=True,long_message_preserved=True,false_summary_did_not_act=True,stock_aliases_zero_model=True,purse_excluded=True,purchase_acknowledged=True,registered_emote=True,limits='Offline host/HTTP mocks. Punctuation and citation checks are structural, not semantic truth proofs; no hardware audio or latency claim.')
  finally:host.terminate();host.wait(timeout=5);mock.shutdown();mock.server_close()
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
