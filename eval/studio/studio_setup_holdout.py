"""Offline setup-to-play/recovery holdout with an original loopback Ollama mock."""
import argparse,copy,http.client,http.server,json,pathlib,re,socket,subprocess,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[2]

def run():
 checks=0;requests=[];mode=['ok'];started=threading.Event();release=threading.Event()
 class Mock(http.server.BaseHTTPRequestHandler):
  def log_message(self,*args):pass
  def answer(self,code,value):
   body=json.dumps(value).encode();self.send_response(code);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
  def do_GET(self):
   requests.append(('GET',self.path));assert self.path=='/api/tags';self.answer(200,dict(models=[dict(name='fixture-qwen:3b')]))
  def do_POST(self):
   q=json.loads(self.rfile.read(int(self.headers['Content-Length'])));requests.append(('POST',self.path));assert self.path=='/api/chat' and q['model']=='fixture-qwen:3b'
   if mode[0]=='missing':self.answer(404,dict(error='model not found'));return
   if mode[0]=='blocked':started.set();assert release.wait(8)
   line=q['format']['properties']['speech']['enum'][0];self.answer(200,dict(message=dict(content=json.dumps(dict(speech=line)))))
 server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
 with tempfile.TemporaryDirectory(prefix='studio-setup-') as d:
  c=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());c['text']='local_ollama';c['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{server.server_port}/api/chat'
  # Held sockets refuse connections consistently; no real voice/image services touched.
  unavailable=socket.socket();unavailable.bind(('127.0.0.1',0));closed_port=unavailable.getsockname()[1]
  for backend in ['local_sd_cpp','local_tts_http','local_stt_http']:c['endpoints'][backend]['url']=f'http://127.0.0.1:{closed_port}/unused'
  file=pathlib.Path(d)/'providers.json';file.write_text(json.dumps(c));p=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-config',str(file),'--web-state',d],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',p.stdout.readline())[1])
   def call(op=None,args=None,path='/api/state',status=200,generation=None):
    nonlocal checks
    conn=http.client.HTTPConnection('127.0.0.1',port,timeout=5);headers={'Content-Type':'application/json','X-Studio-Request':'1'}
    if generation is not None:headers['X-Studio-Generation']=str(generation)
    conn.request('POST' if op else 'GET','/api/'+op if op else path,json.dumps(args or {}) if op else None,headers);r=conn.getresponse();body=r.read();assert r.status==status,(op,r.status,body);checks+=1;conn.close()
    return json.loads(body) if r.getheader('Content-Type','').startswith('application/json') else body
   def state():return call()
   def cmd(op,**args):return call(op,args)['result']
   def wait(predicate):
    deadline=time.monotonic()+6
    while time.monotonic()<deadline:
     s=state()
     if predicate(s):return s
     time.sleep(.02)
    raise AssertionError('mock worker failed to settle')
   html=call(path='/').decode();assert all(x in html for x in ['id="new-game"','id="model"','id="work-status"','id="check-providers"','id="settings-state"']);assert call(path='/setup.mjs')
   assert cmd('check_providers')['queued'];s=wait(lambda s:s['pending_jobs']==0)
   assert requests==[('GET','/api/tags')]
   assert s['setup']['installed_models']==['fixture-qwen:3b'] and s['setup']['channels'][0]['status']=='missing_model'
   assert not s['local_paused'] and s['spending']['charged_or_reserved_micros']==0
   settings={k:s['providers'][k] for k in ['text','image','tts','stt','size','speech_mode','voices']};settings.update(model='fixture-qwen:3b',cap_micros=0);cmd('settings',**settings);s=state();assert s['setup']['channels'][0]['status']=='ready';assert s['game']['room']=='room:entry'
   before=copy.deepcopy(s['providers']);call('settings',dict(settings,model='unsafe\nname'),status=400);assert state()['providers']==before
   for room in ['gallery','workshop']:
    s=state();cmd('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   cmd('chat',text='talk to merchant');s=wait(lambda s:s['pending_jobs']==0);assert s['speech']['origin']=='authorized_model',(s['notice'],s['speech'],s['setup']['activity'][:2])
   assert any(r['status']=='completed' and r['model']=='fixture-qwen:3b' for r in s['setup']['activity'])
   mode[0]='missing';cmd('chat',text='Hello again.');s=wait(lambda s:s['pending_jobs']==0);assert not s['local_paused'] and s['speech']['origin']=='offline' and s['setup']['channels'][0]['status']=='missing_model' and 'exact model name' in s['notice']
   mode[0]='ok';cmd('chat',text='Please try again.');s=wait(lambda s:s['pending_jobs']==0);assert s['speech']['origin']=='authorized_model' and s['setup']['channels'][0]['status']=='ready'
   cmd('save',slot=0);saved=state()['game'];mode[0]='blocked';cmd('chat',text='A final question.');assert started.wait(3)
   old=state();t=time.perf_counter();cmd('reset');reset_ms=(time.perf_counter()-t)*1000;s=state();assert s['game']['room']=='room:entry' and s['speech'] is None and not s['transcript'] and s['providers']==before
   assert not release.is_set();release.set();s=wait(lambda s:s['pending_jobs']==0);assert s['speech'] is None
   call('chat',dict(text='buy oil'),generation=old['generation'],status=409);assert state()['game']['coins']==5
   cmd('load',slot=0);assert state()['game']==saved
   assert len([r for r in requests if r[0]=='POST'])==4
   assert s['spending']['charged_or_reserved_micros']==0
   return dict(http_checks=checks,mock_discovery_requests=1,mock_generation_requests=4,reset_while_mock_blocked_ms=round(reset_ms,3),cases=['installed-model discovery without generation','missing-model explanation','scoped model setting and invalid-name rejection','setup after entering the game','talk command invokes conversation','visible model completion','404 retains fallback without global pause','subsequent request recovery','nonblocking reset while model is held','late response and stale command rejected','saved branch preserved'],actual_provider_calls=0,api_spend_usd=0,limits='Offline mock HTTP and game state. Port checks prove reachability only, not provider compatibility. Reset timing is one uncontrolled local observation, not a frame-rate or general performance claim.')
  finally:release.set();p.terminate();p.wait(timeout=5);unavailable.close();server.shutdown();server.server_close()

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;result=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result))
if __name__=='__main__':main()
