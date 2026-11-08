"""Real host/Firefox with mocked model and browser speech, zero API spend."""
import argparse,http.server,json,pathlib,re,subprocess,sys,tempfile,threading
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()];from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;model_calls=[]
 class Model(http.server.BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_POST(self):
   req=json.loads(self.rfile.read(int(self.headers['Content-Length'])));model_calls.append(req);out=dict(speech=['I dream of a quiet place to exchange ideas.','I hope for thoughtful company.','I value patient conversation.']);data=json.dumps(dict(message=dict(content=json.dumps(out)))).encode();self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
 mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Model);threading.Thread(target=mock.serve_forever,daemon=True).start()
 with tempfile.TemporaryDirectory(prefix='conversation-browser-')as tmp:
  d=pathlib.Path(tmp);mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));c=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());c['tts']='browser_web_speech';c['text']='local_ollama';c['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(c));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);b=None
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1]);b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));b.context=b.call('browsingContext.create',dict(type='tab'))['context'];b.call('script.addPreloadScript',dict(functionDeclaration="""()=>{window.__spoken=[];Object.defineProperty(window,'speechSynthesis',{value:{cancel(){},getVoices(){return[]},speak(u){window.__spoken.push(u.text);setTimeout(()=>{u.onboundary?.({charIndex:8});u.onend?.()},100)}}});window.SpeechSynthesisUtterance=class{constructor(t){this.text=t}};}"""));b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{port}/',wait='complete'))
   def js(c):return b.evaluate('(async()=>{'+c+'})()')
   def wait(e):assert js("return new Promise((r,j)=>{const end=performance.now()+10000;const p=()=>{if("+e+")r(true);else if(performance.now()>end)j(Error('timeout'));else setTimeout(p,30)};p()})")
   wait("document.querySelector('#quick-actions button')");js("localStorage.removeItem('lantern-auto-speak');document.getElementById('auto-speak').checked=false;return true")
   for room in ['gallery','workshop']:js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
   js("document.getElementById('show-setup').click();document.querySelector('[data-settings-tab=characters]').click();document.getElementById('reply-sentences').value='3';document.getElementById('conversation-apply').click();return true");wait("document.getElementById('conversation-state').textContent==='Applied'");js("document.querySelector('[data-settings-tab=voice]').click();document.getElementById('auto-speak').checked=true;document.getElementById('auto-speak').dispatchEvent(new Event('change'));document.getElementById('settings-close').click();return true");wait("document.getElementById('play').disabled")
   js("document.getElementById('input').value='Do you have dreams?';document.getElementById('input').dispatchEvent(new Event('input'));document.getElementById('input').dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true}));return true");wait("document.getElementById('speaker').textContent.includes('3/3')&&document.getElementById('play').disabled");assert js("return window.__spoken.filter(s=>['I dream of a quiet place to exchange ideas.','I hope for thoughtful company.','I value patient conversation.'].includes(s)).length")==3;assert len(model_calls)==1
   js("document.getElementById('input').value='*nods politely*';document.getElementById('chat').requestSubmit();return true");wait("document.getElementById('transcript').textContent.includes('nods politely')")
   js("[...document.querySelectorAll('#actions button')].find(b=>b.textContent.startsWith('Buy hinge oil')).click();return true");wait("window.__spoken.includes('Here is your hinge oil; thank you.')&&document.getElementById('play').disabled");assert js("return document.getElementById('transcript').textContent.includes('buys hinge oil')")
   js("document.getElementById('input').value='A'.repeat(1100);document.getElementById('input').dispatchEvent(new Event('input'));return true");assert js("return document.getElementById('input-count').textContent.includes('1100 / 2048')")
   errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
   r=dict(workflows=6,model_mock_calls=len(model_calls),real_model_calls=0,sentence_autoplay_in_order=True,one_text_call_for_three_sentences=True,roleplay_stage_visible=True,purchase_ack_spoken=True,multiline_character_counter=True,page_console_errors=0,api_spend_usd=0,limits='Real Firefox/host with deterministic model and speech mocks; no physical audio or performance claim.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
   print(json.dumps(r))
  finally:
   if b:b.call('session.end',{})
   host.terminate();host.wait(timeout=5);mock.shutdown();mock.server_close()
if __name__=='__main__':main()
