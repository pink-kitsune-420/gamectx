"""Offline end-to-end residency, dramatic rendering and online-cap routing holdout."""
import argparse,base64,copy,http.client,http.server,json,pathlib,re,socket,struct,subprocess,tempfile,threading,time,zlib
ROOT=pathlib.Path(__file__).resolve().parents[2]
def png(w,h):
 def chunk(k,v):return struct.pack('>I',len(v))+k+v+struct.pack('>I',zlib.crc32(k+v))
 return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,6,0,0,0))+chunk(b'IDAT',zlib.compress((b'\0'+b'\x3a\x4b\x52\xff'*w)*h))+chunk(b'IEND',b'')
def run():
 requests=[];loaded=dict(text=None,image='normal-image');lock=threading.Lock();active=0;max_active=0;checks=0
 class Mock(http.server.BaseHTTPRequestHandler):
  def log_message(self,*args):pass
  def answer(self,v,code=200):
   b=json.dumps(v).encode();self.send_response(code);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
  def do_GET(self):
   requests.append(('GET',self.path,None))
   if self.path=='/api/tags':self.answer(dict(models=[dict(name=n)for n in ['normal:3b','dramatic:7b']]))
   elif self.path=='/api/ps':self.answer(dict(models=[] if loaded['text']is None else [dict(name=loaded['text'])]))
   elif self.path=='/sdcpp/v1/model-files':self.answer(dict(files=[dict(id=n)for n in ['checkpoints/normal.gguf','checkpoints/dramatic.gguf']]))
   elif self.path=='/sdcpp/v1/models/status':self.answer(dict(state='ready' if loaded['image']else'empty',origin='command_line' if loaded['image']=='normal-image' else 'recipe',recipe=None if loaded['image']=='normal-image'else'sdxl-checkpoint',models={} if loaded['image']=='normal-image'else dict(model='checkpoints/dramatic.gguf')))
   elif self.path=='/sdapi/v1/options':self.answer(dict(sd_model_checkpoint=loaded['image']))
   else:self.answer(dict(error='unknown'),404)
  def do_POST(self):
   nonlocal active,max_active
   body=json.loads(self.rfile.read(int(self.headers.get('Content-Length','0')))or b'{}');requests.append(('POST',self.path,copy.deepcopy(body)))
   if self.path=='/api/generate':loaded['text']=None if body.get('keep_alive')==0 else body['model'];self.answer(dict(done=True));return
   if self.path=='/sdcpp/v1/models/refresh':self.answer(dict(errors=[]));return
   if self.path=='/sdcpp/v1/models/unload':loaded['image']=None;self.answer(dict(state='empty'));return
   if self.path=='/sdcpp/v1/models/load':loaded['image']='dramatic-image' if 'dramatic' in body['models']['model']else'normal-image';self.answer(dict(state='ready'));return
   if self.path not in ['/api/chat','/sdapi/v1/txt2img']:self.answer(dict(error='unknown'),404);return
   with lock:active+=1;max_active=max(max_active,active)
   try:
    time.sleep(.03)
    if self.path=='/api/chat':
     assert loaded['image'] is None
     loaded['text']=body['model'];assert 'copper sparrow' not in json.dumps(body);speech=body['format']['properties']['speech']['enum'][0];self.answer(dict(message=dict(content=json.dumps(dict(speech=speech)))))
    else:
     assert loaded['image'] and loaded['text'] is None
     assert 'closed wooden shutters' in body['prompt'] and 'sunlight' not in body['prompt']
     self.answer(dict(images=[base64.b64encode(png(body['width'],body['height'])).decode()]))
   finally:
    with lock:active-=1
 server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=server.serve_forever,daemon=True).start()
 with tempfile.TemporaryDirectory(prefix='model-manager-') as d:
  directory=pathlib.Path(d);pconfig=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());pconfig['text']='local_ollama';pconfig['image']='local_sd_cpp'
  for b,path in [('local_ollama','api/chat'),('local_sd_cpp','sdapi/v1/txt2img')]:pconfig['endpoints'][b]['url']=f'http://127.0.0.1:{server.server_port}/{path}'
  providerfile=directory/'providers.json';providerfile.write_text(json.dumps(pconfig));policy=json.loads((ROOT/'examples/lantern-vault/web/models.json').read_text());policy.update(enabled=True,text_escalation=True,image_escalation=True,prewarm=False,ram_limit_mib=256,reserve_mib=128,idle_seconds=1)
  for p in policy['profiles'].values():p['ram_mib']=64
  policy['profiles']['text']['model']='normal:3b';policy['profiles']['dramatic_text']['model']='dramatic:7b'
  for id,model,file in [('image','normal-image','normal.gguf'),('dramatic_image','dramatic-image','dramatic.gguf')]:policy['profiles'][id].update(model=model,selection=dict(model='checkpoints/'+file))
  (directory/'models.json').write_text(json.dumps(policy));child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-config',str(providerfile),'--web-state',d],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   line=child.stdout.readline();match=re.search(r'127\.0\.0\.1:(\d+)',line);assert match,line;port=int(match[1])
   def call(op=None,args=None,status=200):
    nonlocal checks
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=8);c.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args or{})if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=c.getresponse();v=json.loads(r.read());c.close();checks+=1;assert r.status==status,(op,r.status,v);return v['result']if op and status==200 else v
   def settled(predicate=lambda s:True):
    end=time.monotonic()+8
    while time.monotonic()<end:
     s=call()
     if not s['pending_jobs'] and predicate(s):return s
     time.sleep(.03)
    raise AssertionError(('did not settle',s['notice'],s['model_manager'],s['setup']['activity'][:2]))
   def configure(**kw):
    nonlocal policy
    policy=policy|kw;assert call('manager_settings',dict(config=policy))['configured'];return settled()
   s=settled(lambda s:'text' in s['model_manager']['runtime'].get('eligible',[]));assert not any(path=='/api/chat' for _,path,_ in requests)
   for room in ['gallery','workshop']:
    s=call();call('act',dict(revision=s['game']['revision'],decision=dict(action='go',target='room:'+room)))
   call('chat',dict(text='Hello.'));s=settled();assert s['speech']['origin']=='authorized_model' and loaded['text']is None
   chat=[body for _,path,body in requests if path=='/api/chat'][-1];assert chat['model']=='normal:3b'and chat['options']['num_ctx']==2048
   call('generate',dict(target='scene'));s=settled();normal_key=s['scene_asset']['selection']['key'];assert s['scene_asset']['inspection']['candidate'] is not None and loaded['image']is None
   call('approve',dict(key=normal_key));normal_asset=call()['scene_asset']['selection']['asset'];assert normal_asset
   configure(mode='dramatic');s=call();assert s['model_manager']['route']['text']=='dramatic_text' and s['model_manager']['route']['image']=='dramatic_image'
   assert s['scene_asset']['selection']['key']!=normal_key and s['scene_asset']['preview_asset']==normal_asset
   call('chat',dict(text='Hello again.'));s=settled();chat=[body for _,path,body in requests if path=='/api/chat'][-1];assert chat['model']=='dramatic:7b' and chat['options']['num_ctx']==8192
   call('generate',dict(target='scene'));s=settled();dramatic_key=s['scene_asset']['selection']['key'];image=[body for _,path,body in requests if path=='/sdapi/v1/txt2img'][-1];assert image['width']==1024 and image['steps']==24 and 'fine material texture' in image['prompt']
   call('approve',dict(key=dramatic_key));old_count=len(requests);assert call('generate',dict(target='scene'))['cached'];assert len(requests)==old_count
   # A model-identity change invalidates the content-addressed spec even with the same art bible.
   changed=copy.deepcopy(policy);changed['profiles']['dramatic_image']['model']='renamed-image';policy=changed;configure();assert call()['scene_asset']['selection']['key']!=dramatic_key
   policy['profiles']['dramatic_text'].update(backend='online_openai_compatible',model='configured-online',ram_mib=65536);policy['profiles']['dramatic_image'].update(backend='online_image_api',model='configured-image',ram_mib=65536);configure();s=call();assert s['model_manager']['route']['text']=='text'and s['model_manager']['route']['image']=='image'
   configure(mode='normal',residency='warm',prewarm=True);s=call();call('act',dict(revision=s['game']['revision'],decision=dict(action='go',target='room:gallery')));s=settled();assert loaded['text']=='normal:3b';speech_count=sum(path=='/api/chat'for _,path,_ in requests)
   # A timed lease expires independently of frame ticks and user requests.
   s=settled(lambda s:s['model_manager']['runtime'].get('loaded')is None);assert loaded['text']is None and sum(path=='/api/chat'for _,path,_ in requests)==speech_count
   call('manager_action',dict(action='stop',profile='text'));s=settled();assert 'started elsewhere' in s['notice']
   assert max_active==1 and s['spending']['charged_or_reserved_micros']==0 and not any('pull' in path for _,path,_ in requests)
   return dict(http_checks=checks,policy_workflows=12,mock_text_generations=sum(path=='/api/chat'for _,path,_ in requests),mock_image_generations=sum(path=='/sdapi/v1/txt2img'for _,path,_ in requests),max_concurrent_generations=max_active,api_spend_usd=0,actual_provider_calls=0,normal_context=2048,dramatic_context=8192,dramatic_image_width=1024,cases=['startup metadata without generation','on-demand text and image unload','normal and dramatic model routing','expanded context for dramatic text','detailed art from perceived facts','distinct model/art asset identity','approved fallback while generating','exact approved reuse with no requests','online cap-zero fallback','predictive preload without speech','idle unload without game ticks','external process stop rejected'],limits='Original offline provider mocks and host integration; model IDs, residency acknowledgements and RAM estimates are synthetic. No hardware latency, real RAM savings, image quality, live larger-model capability or paid API measurement.')
  finally:child.terminate();child.wait(timeout=5);server.shutdown();server.server_close()
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
