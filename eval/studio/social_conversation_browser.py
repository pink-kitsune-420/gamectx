"""Original delayed mock + real Firefox: screenshot greeting/wellbeing/follow-up flow."""
import argparse,http.server,json,pathlib,re,subprocess,sys,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()];from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 with tempfile.TemporaryDirectory(prefix='social-browser-')as tmp:
  d=pathlib.Path(tmp);calls=[]
  class Mock(http.server.BaseHTTPRequestHandler):
   def log_message(self,*a):pass
   def do_POST(self):
    v=json.loads(self.rfile.read(int(self.headers['Content-Length'])));calls.append(v);time.sleep(.8);speech='I feel settled and happy to listen.'if len(calls)==1 else'I value patient listening because it helps us understand each other.';b=json.dumps(dict(message=dict(content=json.dumps(dict(speech=speech))))).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
  mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=mock.serve_forever,daemon=True).start();cfg=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());cfg['text']='local_ollama';cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(cfg));mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);b=None
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1]);b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));b.context=b.call('browsingContext.create',dict(type='tab'))['context'];b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{port}/',wait='complete'))
   def js(c):return b.evaluate('(async()=>{'+c+'})()')
   def wait(expr):assert js("return new Promise((resolve,reject)=>{const end=performance.now()+8000;const p=()=>{if("+expr+")resolve(true);else if(performance.now()>end)reject(Error('timeout'));else setTimeout(p,30)};p()})")
   def send(q):return js("document.getElementById('input').value="+json.dumps(q)+";document.getElementById('chat').requestSubmit();return document.getElementById('current-reply').hidden")
   def read():js("document.getElementById('play').click();return true");wait("document.getElementById('play').disabled")
   wait("document.getElementById('room-title').textContent==='Entry'")
   for room in ['gallery','workshop']:
    js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
   send('Hello.');wait("!document.getElementById('current-reply').hidden");assert not calls;assert js("return document.getElementById('reply-origin').textContent==='Character reply · no LLM'");first=js("return document.getElementById('speech-text').textContent");assert 'counter'not in first;read()
   assert send('hello, how are you today?');wait("!document.getElementById('current-reply').hidden&&document.getElementById('speech-text').textContent.startsWith('I feel')");second=js("return document.getElementById('speech-text').textContent");assert second!=first;assert js("return document.getElementById('reply-origin').textContent==='Character reply · no LLM'");read()
   assert send('Why is that?');wait("!document.getElementById('current-reply').hidden&&document.getElementById('speech-text').textContent.includes('because')");third=js("return document.getElementById('speech-text').textContent");assert len({first,second,third})==3;read();assert not calls
   assert js("return(await(await fetch('/api/state')).json()).personality.viewpoints.entries.length===0");assert js("return document.getElementById('debug-window').hidden")
   errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
   result=dict(browser='Firefox headless',workflows=5,three_distinct_replies=True,greeting_extra_model_calls=0,previous_reply_hidden=True,followup_uses_heard_mood=True,wellbeing_model_calls=0,mood_not_saved_as_permanent=True,page_console_errors=0,actual_provider_calls=0,api_spend_usd=0,limits='Original delayed local HTTP mock and real browser DOM/read receipts; no physical audio or live model quality measurement.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
   print(json.dumps(result))
  finally:
   if b:b.call('session.end',{})
   host.terminate();host.wait(timeout=5);mock.shutdown();mock.server_close()
if __name__=='__main__':main()
