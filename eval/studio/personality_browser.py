"""Real Firefox check: old/fallback reply stays hidden during delayed generation."""
import argparse,http.server,json,pathlib,re,subprocess,sys,tempfile,threading,time
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()];from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 with tempfile.TemporaryDirectory(prefix='personality-browser-')as tmp:
  d=pathlib.Path(tmp);calls=[]
  class Mock(http.server.BaseHTTPRequestHandler):
   def log_message(self,*a):pass
   def do_POST(self):
    v=json.loads(self.rfile.read(int(self.headers['Content-Length'])));calls.append(v);time.sleep(1.2);speech='I prefer sketching little designs when I imagine a quiet moment.';b=json.dumps(dict(message=dict(content=json.dumps(dict(speech=speech))))).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();
    try:self.wfile.write(b)
    except BrokenPipeError:pass
  mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=mock.serve_forever,daemon=True).start();cfg=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());cfg['text']='local_ollama';cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(cfg));mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);b=None
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1]);b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));b.context=b.call('browsingContext.create',dict(type='tab'))['context'];b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{port}/',wait='complete'))
   def js(c):return b.evaluate('(async()=>{'+c+'})()')
   def wait(expr):assert js("return new Promise((resolve,reject)=>{const end=performance.now()+8000;const p=()=>{if("+expr+")resolve(true);else if(performance.now()>end)reject(Error('timeout'));else setTimeout(p,30)};p()})")
   wait("document.getElementById('room-title').textContent==='Entry'")
   for room in ['gallery','workshop']:
    js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
   js("document.getElementById('input').value='What do you do?';document.getElementById('chat').requestSubmit();return true");wait("!document.getElementById('current-reply').hidden");assert js("return document.getElementById('speech-text').textContent.includes('counter')")
   # Submission must hide the old reply synchronously, before the HTTP round trip.
   assert js("document.getElementById('input').value='Do you have any hobbies?';document.getElementById('chat').requestSubmit();return document.getElementById('current-reply').hidden&&!document.getElementById('reply-thinking').hidden")
   wait("document.getElementById('use-instant-reply').hidden===false");assert js("return document.getElementById('current-reply').hidden")
   wait("!document.getElementById('reply-thinking').hidden===false&&!document.getElementById('current-reply').hidden");assert js("return document.getElementById('speech-text').textContent.includes('sketching')");js("document.getElementById('play').click();return true");wait("document.getElementById('voice-status').textContent.includes('Read')||document.getElementById('voice-status').textContent.includes('Heard')||document.getElementById('play').disabled")
   js("document.getElementById('show-setup').click();document.querySelector('[data-settings-tab=characters]').click();return true");assert js("return !document.getElementById('settings-characters').hidden&&document.getElementById('mind-enabled').checked");js("document.getElementById('settings-close').click();return true")
   # A second topic can be skipped to its 0B personality fallback without old text flashing.
   js("document.getElementById('input').value='Do you have any dreams?';document.getElementById('chat').requestSubmit();return true");wait("!document.getElementById('use-instant-reply').hidden");js("document.getElementById('use-instant-reply').click();return true");wait("document.getElementById('reply-thinking').hidden&&!document.getElementById('current-reply').hidden");assert js("return document.getElementById('speech-text').textContent.startsWith('I hope')")
   errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
   r=dict(workflows=5,browser='Firefox headless',synchronous_old_reply_hidden=True,fallback_hidden_during_generation=True,new_subjective_reply_revealed=True,instant_reply_recovery=True,character_settings=True,page_console_errors=0,api_spend_usd=0,actual_provider_calls=0,limits='Original delayed HTTP mock and real DOM; no physical playback or live model quality measurement.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
   print(json.dumps(r))
  finally:
   if b:b.call('session.end',{})
   host.terminate();host.wait(timeout=5);mock.shutdown();mock.server_close()
if __name__=='__main__':main()
