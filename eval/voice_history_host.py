"""Offline archive reuse/FIFO/reset, role replies and local-text cancellation holdout."""
import argparse,http.client,http.server,json,pathlib,re,shutil,subprocess,sys,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 with tempfile.TemporaryDirectory(prefix='voice-history-')as tmp:
  d=pathlib.Path(tmp);active=d/'model-active';started=threading.Event();cancelled=threading.Event();requests=[]
  class Mock(http.server.BaseHTTPRequestHandler):
   def log_message(self,*args):pass
   def answer(self,v):
    b=json.dumps(v).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
   def do_GET(self):
    self.answer(dict(models=[dict(name='fixture:3b')])if self.path=='/api/tags'else dict(models=[]))
   def do_POST(self):
    v=json.loads(self.rfile.read(int(self.headers['Content-Length'])));requests.append(self.path)
    if self.path=='/api/generate':assert not active.exists();self.answer(dict(done=True));return
    assert self.path=='/api/chat';active.touch();started.set();self.connection.settimeout(6)
    try:assert self.connection.recv(1)==b''
    finally:active.unlink(missing_ok=True);cancelled.set()
  mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=mock.serve_forever,daemon=True).start()
  engine=d/'piper';engine.write_text('#!'+sys.executable+'\n'+'''import json,pathlib,sys,wave
root=pathlib.Path(__file__).parent;assert not(root/'model-active').exists()
with(root/'native-count').open('a')as f:f.write('job\\n')
for row in sys.stdin:
 v=json.loads(row)
 with wave.open(v['output_file'],'wb')as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(22050);w.writeframes(b'\\0\\0'*2205)
''');engine.chmod(0o700);(d/'weights').write_text('original fixture');(d/'native-voice.json').write_text(json.dumps(dict(version=1,tts=dict(executable=str(engine),voices=dict(alloy=str(d/'weights'),onyx=str(d/'weights'))),stt=dict(executable='',models={}))))
  cfg=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());cfg['text']='local_ollama';cfg['text_models']['local_ollama']['3']='fixture:3b'
  for k,e in cfg['endpoints'].items():
   if k.startswith('local_'):e['url']='http://127.0.0.1:1/unused'
  cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(cfg));policy=json.loads((ROOT/'examples/dungeon/web/models.json').read_text());policy.update(enabled=True,ram_limit_mib=256,reserve_mib=128)
  for p in policy['profiles'].values():p['ram_mib']=64
  policy['profiles']['text']['model']='fixture:3b';policy['profiles']['tts'].update(model='piper',backend='local_tts_http');(d/'models.json').write_text(json.dumps(policy))
  child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);checks=0
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',child.stdout.readline())[1])
   def call(op=None,**args):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert r.status==200,(r.status,v);return v['result']if op else v
   def wait(test=lambda s:not s['pending_jobs']):
    end=time.monotonic()+8
    while time.monotonic()<end:
     s=call()
     if test(s):return s
     time.sleep(.02)
    raise AssertionError(('timeout',s['notice']))
   def audio_status(key):
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('GET','/api/voice/'+key);r=h.getresponse();r.read();h.close();return r.status
   def finish(s):
    sid=s['speech']['id'];n=len(s['speech']['text']);call('delivery',id=sid,event='played',through=n,heard=n);call('delivery',id=sid,event='finished',through=n,heard=n)
   wait();call('audio_settings',enabled=True,limit=2)
   for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   call('chat',text='Hello.');assert started.wait(3);s=call();sid=s['speech']['id'];begin=time.monotonic();call('delivery',id=sid,event='queued',through=0,heard=0);call('synthesize',id=sid);s=wait();assert cancelled.is_set()and not active.exists();assert time.monotonic()-begin<4;assert not s['local_paused'];first=s['speech']['audio']['archive_key'];assert audio_status(first)==200;finish(s)
   text_row=next(r for r in s['setup']['activity']if r['channel']=='text');tts=next(r for r in s['setup']['activity']if r['channel']=='tts');assert text_row['status']=='cancelled'and'queue_ms'in tts and'run_ms'in tts
   started.clear();call('chat',text='Hello.');assert started.wait(3);s=call();call('delivery',id=s['speech']['id'],event='queued',through=0,heard=0);assert call('synthesize',id=s['speech']['id'])['cached'];s=wait();assert len((d/'native-count').read_text().splitlines())==1;finish(s)
   before=call()['transcript'];assert audio_status(first)==200;assert call()['transcript']==before
   lines=[]
   for question in ['What do you do?','What else can you do?']:
    result=call('chat',text=question);assert result['direct'];s=wait();lines.append(s['speech']['text']);call('delivery',id=s['speech']['id'],event='queued',through=0,heard=0);call('synthesize',id=s['speech']['id']);s=wait();finish(s)
   assert lines[0]!=lines[1]and all('counter; choose an item'not in l for l in lines);assert len(call()['voice_history']['records'])==2 and audio_status(first)==404
   call('audio_settings',enabled=False,limit=2);assert not call()['voice_history']['records']and not list((d/'voice-history').glob('*.wav'));call('audio_settings',enabled=True,limit=2)
   call('chat',text='What do you do?');s=wait();call('delivery',id=s['speech']['id'],event='queued',through=0,heard=0);call('synthesize',id=s['speech']['id']);s=wait();assert s['voice_history']['records'];call('reset');assert not call()['voice_history']['records']and not list((d/'voice-history').glob('*.wav'))
   assert call()['spending']['charged_or_reserved_micros']==0
   return dict(http_checks=checks,workflows=9,mock_llm_calls=requests.count('/api/chat'),cached_repeat_native_jobs=0,cancelled_text_before_voice=True,replay_created_memories=False,role_followups=lines,archive_limit=2,off_and_reset_cleared=True,api_spend_usd=0,actual_provider_calls=0,limits='Original CLI and delayed Ollama mocks. Real host sockets, policy and storage; no hardware latency, physical playback, real CPU savings or live model quality measurement.')
  finally:child.terminate();child.wait(timeout=5);mock.shutdown();mock.server_close()
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
