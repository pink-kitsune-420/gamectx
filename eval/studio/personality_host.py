"""Offline listener isolation, delivery commitment and branch/reset personality tests."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[2]
def run():
 with tempfile.TemporaryDirectory(prefix='mind-host-')as tmp:
  d=pathlib.Path(tmp);calls=[];next_reply=['I hope to make this counter welcoming to careful travelers.']
  class Mock(http.server.BaseHTTPRequestHandler):
   def log_message(self,*a):pass
   def do_POST(self):
    body=json.loads(self.rfile.read(int(self.headers['Content-Length'])));calls.append(body);v=dict(message=dict(content=json.dumps(dict(speech=next_reply[0]))),prompt_eval_count=500,eval_count=21,done=True);b=json.dumps(v).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
  mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=mock.serve_forever,daemon=True).start()
  cfg=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());cfg.update(text='local_ollama',cpu_threads=4);cfg['text_models']['local_ollama']['3']='fixture:3b';cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(cfg))
  mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  checks=0
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',child.stdout.readline())[1])
   def call(op=None,**args):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert r.status==200,(r.status,v);return v['result']if op else v
   def wait():
    end=time.monotonic()+5
    while time.monotonic()<end:
     s=call()
     if not s['pending_jobs']:return s
     time.sleep(.01)
    raise AssertionError('timeout')
   def views():return call()['personality']['viewpoints']['entries']
   def finish(s):
    sid=s['speech']['id'];n=len(s['speech']['text']);call('delivery',id=sid,event='queued',through=0,heard=0);call('delivery',id=sid,event='played',through=n,heard=n);call('delivery',id=sid,event='finished',through=n,heard=n)
   for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   call('chat',text='Do you have any dreams?');s=wait();assert s['speech']['text']==next_reply[0]and s['speech']['origin']=='subjective_model';assert not views();assert calls[-1]['options']['num_predict']==64 and calls[-1]['format']['properties']['speech']['maxLength']==240
   sid=s['speech']['id'];call('delivery',id=sid,event='queued',through=0,heard=0);call('delivery',id=sid,event='played',through=6,heard=6);call('delivery',id=sid,event='interrupted',through=6,heard=6);assert not views()
   call('chat',text='What do you hope for?');s=wait();finish(s);assert len(views())==1;count=len(calls);r=call('chat',text='Tell me your dreams.');assert r['saved_viewpoint'];s=wait();assert len(calls)==count and s['speech']['text']==next_reply[0];finish(s);call('save',slot=0)
   call('select',player='player_2',npc='merchant');next_reply[0]='I dream of a quieter counter with time for patient conversation.';call('chat',text='What do you dream of?');s=wait();finish(s);assert len(views())==2 and views()[0]['listener']!=views()[1]['listener']
   assert json.loads((d/'viewpoints.json').read_text())['entries']==views();call('load',slot=0);assert len(views())==1 and call()['player']=='player_1'
   next_reply[0]='I hope you know I will give you all my coins.';call('chat',text='Do you like tea?');s=wait();assert s['speech']['origin']=='offline'and len(views())==1;assert s['game']['coins']==5;finish(s);assert len(views())==2 and views()[-1]['source']=='profile'
   call('chat',text='Where was I born?');s=wait();assert 'enum'in calls[-1]['format']['properties']['speech'];assert 'personality'not in ''.join(m['content']for m in calls[-1]['messages']);assert len(views())==2
   call('reset');assert not views()and not json.loads((d/'viewpoints.json').read_text())['entries']and call()['personality']['config']['enabled'];assert call()['spending']['charged_or_reserved_micros']==0
   return dict(http_checks=checks,workflows=9,partial_saved=False,repeated_topic_extra_calls=0,listener_isolation=True,branch_restore=True,invalid_action_rejected=True,factual_path_preserved=True,api_spend_usd=0,actual_provider_calls=0,limits='Original local HTTP mock; tests plumbing and lexical validation, not semantic model grounding.')
  finally:child.terminate();child.wait(timeout=5);mock.shutdown();mock.server_close()
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
