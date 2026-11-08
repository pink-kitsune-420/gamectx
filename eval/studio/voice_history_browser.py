"""Manual Firefox UI cohort with original local mocks, no physical audio/models."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,sys,tempfile,threading
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()]
from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 with tempfile.TemporaryDirectory(prefix='archive-browser-')as tmp:
  d=pathlib.Path(tmp);engine=d/'piper';engine.write_text('#!'+sys.executable+'\n'+'''import pathlib,json,sys,wave
root=pathlib.Path(__file__).parent
with(root/'native-count').open('a')as f:f.write('job\\n')
for line in sys.stdin:
 v=json.loads(line)
 with wave.open(v['output_file'],'wb')as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(22050);w.writeframes(b'\\0\\0'*2205)
''');engine.chmod(0o700);(d/'weight').write_text('fixture');(d/'native-voice.json').write_text(json.dumps(dict(version=1,tts=dict(executable=str(engine),voices=dict(alloy=str(d/'weight'))),stt=dict(executable='',models={}))))
  class Mock(http.server.BaseHTTPRequestHandler):
   def log_message(self,*a):pass
   def answer(self,v):
    b=json.dumps(v).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
   def do_GET(self):self.answer(dict(models=[dict(name='fixture:3b')])if self.path=='/api/tags'else dict(models=[]))
   def do_POST(self):
    v=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
    if self.path=='/api/generate':self.answer(dict(done=True));return
    speech=v['format']['properties']['speech']['enum'][0];self.answer(dict(message=dict(content=json.dumps(dict(speech=speech))),prompt_eval_count=123,eval_count=12))
  mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=mock.serve_forever,daemon=True).start()
  c=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());c.update(text='local_ollama',tts='local_tts_http');c['text_models']['local_ollama']['3']='fixture:3b'
  for k,e in c['endpoints'].items():
   if k.startswith('local_'):e['url']='http://127.0.0.1:1/unused'
  c['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(c));policy=json.loads((ROOT/'examples/lantern-vault/web/models.json').read_text());policy.update(enabled=True,ram_limit_mib=256,reserve_mib=128)
  for p in policy['profiles'].values():p['ram_mib']=64
  policy['profiles']['text']['model']='fixture:3b';policy['profiles']['tts'].update(model='piper',backend='local_tts_http');(d/'models.json').write_text(json.dumps(policy))
  host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);b=None
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1]);b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));b.context=b.call('browsingContext.create',dict(type='tab'))['context'];cases=[]
   def js(code):return b.evaluate('(async()=>{'+code+'})()')
   def wait(expr):assert js("return new Promise((resolve,reject)=>{const end=performance.now()+8000;const poll=()=>{if("+expr+")resolve(true);else if(performance.now()>end)reject(Error('UI timeout'));else setTimeout(poll,40)};poll()})")
   def click(id):js('document.getElementById('+json.dumps(id)+').click();return true')
   def api(op,args={}):return js('const r=await fetch("/api/'+op+'",{method:"POST",headers:{"Content-Type":"application/json","X-Studio-Request":"1"},body:JSON.stringify('+json.dumps(args)+')});if(!r.ok)throw Error(await r.text());return true')
   b.call('browsingContext.setViewport',dict(context=b.context,viewport=dict(width=1366,height=768),devicePixelRatio=1));b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{port}/',wait='complete'));wait("document.getElementById('room-title').textContent==='Entry'");assert js("return document.getElementById('debug-window').hidden&&!document.querySelector('main').contains(document.querySelector('.inspector'))");cases.append('debug starts minimized and does not consume gameplay column')
   click('show-debug');wait("!document.getElementById('debug-window').hidden");js("document.querySelector('[data-debug-tab=context]').click();return true");click('capture-prompts');wait("document.getElementById('capture-prompts').checked");api('profiler_settings',dict(capture_prompts=True));click('debug-minimize');cases.append('context capture is opt-in and debugger minimizes')
   click('show-setup');js("document.querySelector('[data-settings-tab=voice]').click();document.getElementById('archive-enabled').checked=true;document.getElementById('archive-limit').value='2';return true");click('archive-apply');wait("document.getElementById('archive-state').textContent.includes('enabled')");click('settings-close');cases.append('archive enabled with configurable backlog')
   for room in ['gallery','workshop']:
    js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
   api('chat',dict(text='Hello.'));wait("document.getElementById('speaker').textContent.includes('merchant')");wait("document.getElementById('work-status').textContent.includes('completed')");sid=js("return(await(await fetch('/api/state')).json()).speech.id");api('delivery',dict(id=sid,event='queued',through=0,heard=0));api('synthesize',dict(id=sid));wait("document.querySelectorAll('#voice-history-list button').length===1");length=js("return Array.from(document.getElementById('speech-text').textContent).length");api('delivery',dict(id=sid,event='played',through=length,heard=length));api('delivery',dict(id=sid,event='finished',through=length,heard=length))
   before=js("return JSON.stringify((await(await fetch('/api/state')).json()).transcript)");js("HTMLMediaElement.prototype.play=()=>Promise.resolve();document.querySelector('#voice-history-list button').click();return true");wait("!document.getElementById('review-audio').hidden");assert js("return document.getElementById('review-audio').getAttribute('src').startsWith('/api/voice/')");assert js("return JSON.stringify((await(await fetch('/api/state')).json()).transcript)")==before;assert len((d/'native-count').read_text().splitlines())==1;cases.append('recording replay makes no synthesis call or memory receipt')
   click('show-debug');js("document.querySelector('[data-debug-tab=context]').click();return true");wait("document.getElementById('context-profiler').textContent.includes('123')");assert js("return document.querySelector('#context-profiler table')!==null");js("document.querySelector('[data-debug-tab=execution]').click();return true");assert js("return document.getElementById('execution-profiler').textContent.includes('Queue')&&document.getElementById('execution-profiler').textContent.includes('Run')");cases.append('context/provider token counts and execution timeline render')
   assert js("return getComputedStyle(document.getElementById('debug-window')).resize==='both'");click('debug-minimize');click('new-game');click('confirm-reset');wait("document.getElementById('room-title').textContent==='Entry'&&document.querySelectorAll('#voice-history-list button').length===0");assert not list((d/'voice-history').glob('*.wav'));cases.append('New game clears recordings')
   b.call('browsingContext.setViewport',dict(context=b.context,viewport=dict(width=390,height=844),devicePixelRatio=1));wait('innerWidth===390');assert js('return document.documentElement.scrollWidth<=innerWidth');cases.append('mobile gameplay has no horizontal overflow')
   errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
   r=dict(browser='Firefox headless',workflows=len(cases),cases=cases,page_console_errors=0,actual_provider_calls=0,api_spend_usd=0,limits='Real DOM/layout with original mock AI/CLI and stubbed media play; no physical audio, real token benchmark, installed models or hardware timing measurement.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
   print(json.dumps(r))
  finally:
   if b:b.call('session.end',{})
   host.terminate();host.wait(timeout=5);mock.shutdown();mock.server_close()
if __name__=='__main__':main()
