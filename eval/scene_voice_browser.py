"""Real Firefox, original mock Piper/model; blocked ending recovery and two tabs."""
import argparse,http.server,json,pathlib,re,subprocess,sys,tempfile,threading
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'));from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 if a.output:assert not a.output.exists()
 requests=[]
 class Model(http.server.BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_GET(self):self.answer(dict(models=[dict(name='fixture:3b')]))
  def answer(self,v):
   data=json.dumps(v).encode();self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
  def do_POST(self):
   req=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
   if self.path=='/api/generate':self.answer(dict(done=True));return
   if 'messages'not in req:self.answer(dict(capabilities=['completion']));return
   source=None;plot=None
   for m in req['messages']:
    try:v=json.loads(m['content']);source=v.get('intended_reply',source);plot=v.get('personality',{}).get('fiction_seed',plot)
    except(ValueError,AttributeError):pass
   requests.append(req);speech=source or dict(opening='Imagine '+plot['problem'].rstrip('.')+'.',ending='In this tale, '+plot['ending'].rstrip('.')+'.');self.answer(dict(message=dict(content=json.dumps(dict(speech=speech)))))
 mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Model);threading.Thread(target=mock.serve_forever,daemon=True).start()
 with tempfile.TemporaryDirectory(prefix='scene-browser-')as tmp:
  d=pathlib.Path(tmp);engine=d/'piper';engine.write_text('#!'+sys.executable+'\nimport json,sys,wave\nfor line in sys.stdin:\n v=json.loads(line)\n with wave.open(v["output_file"],"wb")as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(22050);w.writeframes(b"\\0\\0"*2205)\n');engine.chmod(0o700);(d/'weight').write_text('fixture');(d/'native-voice.json').write_text(json.dumps(dict(version=1,tts=dict(executable=str(engine),voices=dict(alloy=str(d/'weight'))),stt=dict(executable='',models={}))))
  cfg=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());cfg.update(text='local_ollama',tts='local_tts_http');cfg['text_models']['local_ollama']['3']='fixture:3b';cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'config.json').write_text(json.dumps(cfg));policy=json.loads((ROOT/'examples/dungeon/web/models.json').read_text());policy.update(enabled=True,ram_limit_mib=256,reserve_mib=128)
  for profile in policy['profiles'].values():profile['ram_mib']=64
  policy['profiles']['text']['model']='fixture:3b';policy['profiles']['tts'].update(model='piper',backend='local_tts_http');(d/'models.json').write_text(json.dumps(policy));mind=json.loads((ROOT/'examples/dungeon/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));(d/'conversation.json').write_text(json.dumps(dict(version=1,max_sentences=2,max_player_chars=2048,summarize_long=False,ai_wording=True)))
  host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'config.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);b=None
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1]);b=Bidi(9223);b.s.settimeout(18);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));main=b.call('browsingContext.create',dict(type='tab'))['context'];b.context=main
   b.call('script.addPreloadScript',dict(functionDeclaration="""()=>{try{localStorage.setItem('lantern-auto-speak','true');}catch{}window.__plays=0;window.__denyEnding=false;window.Audio=class{constructor(){this.currentTime=0;this.duration=.2;this.stopped=false;}async play(){window.__plays++;if(window.__denyEnding&&document.getElementById('speech-text').textContent.startsWith('In this tale,')){window.__denyEnding=false;throw Error('fixture autoplay blocked');}setTimeout(()=>{if(!this.stopped){this.currentTime=.2;this.ontimeupdate?.();this.onended?.();}},500);}pause(){this.stopped=true;}removeAttribute(){}load(){}};}"""))
   b.call('browsingContext.navigate',dict(context=main,url=f'http://127.0.0.1:{port}/',wait='complete'))
   def js(code):return b.evaluate('(async()=>{'+code+'})()')
   def wait(expr):assert js("return new Promise((r,j)=>{const end=performance.now()+12000;const p=async()=>{"+"if(await("+expr+"))r(true);else if(performance.now()>end)j(Error('timeout'));else setTimeout(p,30)};p()})")
   wait("document.querySelector('#quick-actions button')")
   for room in ['gallery','workshop']:
    js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
   wait("document.getElementById('play').disabled&&document.getElementById('reply-thinking').hidden")
   js("window.__denyEnding=true;document.getElementById('input').value='Tell me a story.';document.getElementById('chat').requestSubmit();return true")
   wait("document.getElementById('voice-status').textContent.includes('blocked')&&!document.getElementById('play').disabled")
   s=json.loads(js("return JSON.stringify(await(await fetch('/api/state')).json())"));assert s['speech']['playback_blocked']and s['speech']['segment']==2 and s['speech']['delivery']['heard']==0
   js("document.getElementById('play').click();return true");wait("document.getElementById('play').disabled&&document.getElementById('voice-status').textContent==='Complete'")
   assert js("return document.getElementById('reply-thinking').hidden")
   other=b.call('browsingContext.create',dict(type='tab'))['context'];b.call('browsingContext.navigate',dict(context=other,url=f'http://127.0.0.1:{port}/',wait='complete'));b.call('browsingContext.activate',dict(context=main));b.context=main
   js("document.getElementById('input').value='Another story please.';document.getElementById('chat').requestSubmit();return true");wait("document.getElementById('voice-status').textContent.toLowerCase().includes('playing')")
   b.call('browsingContext.activate',dict(context=other));b.context=other
   wait("document.getElementById('speaker').textContent.includes('2/2')&&document.getElementById('play').disabled&&document.getElementById('reply-thinking').hidden")
   wait("(await(await fetch('/api/state')).json()).speech.delivery.done")
   s=json.loads(js("return JSON.stringify(await(await fetch('/api/state')).json())"));assert s['speech']['delivery']['stage']=='played'and s['speech']['delivery']['heard']==len(s['speech']['text']);assert s['spending']['charged_or_reserved_micros']==0
   errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'and e.get('params',{}).get('source',{}).get('context')in[main,other]];assert not errors,errors
   r=dict(workflows=5,blocked_ending_retried=True,ending_retained=True,two_tabs_completed_both_sentences=True,thinking_cleared_after_delivery=True,page_console_errors=0,mock_text_calls=len(requests),real_model_calls=0,api_spend_usd=0,limits='Real Firefox DOM/Web Locks and host/native adapter, original mock CLI/model and simulated audio. No physical Piper quality or autoplay-policy guarantee.')
   if a.output:a.output.write_text(json.dumps(r,indent=2)+'\n')
   print(json.dumps(r))
  finally:
   if b:
    for ctx in [locals().get('main'),locals().get('other')]:
     if ctx:b.call('browsingContext.close',dict(context=ctx))
    b.call('session.end',{})
   host.terminate();host.wait(timeout=5);mock.shutdown();mock.server_close()
if __name__=='__main__':main()
