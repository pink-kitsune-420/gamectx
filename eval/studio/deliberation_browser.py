"""Real Firefox settings/autoplay with simulated browser audio; cap-zero real host."""
import argparse,json,pathlib,re,subprocess,sys,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()];from studio_browser_driver import Bidi
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 with tempfile.TemporaryDirectory(prefix='deliberation-browser-')as tmp:
  d=pathlib.Path(tmp);mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));c=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());c['tts']='browser_web_speech';(d/'providers.json').write_text(json.dumps(c));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);b=None
  try:
   line=host.stdout.readline();assert '127.0.0.1'in line,(line,host.stderr.read());port=int(re.search(r'127\.0\.0\.1:(\d+)',line)[1]);b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));b.context=b.call('browsingContext.create',dict(type='tab'))['context']
   b.call('script.addPreloadScript',dict(functionDeclaration="""()=>{window.__spoken=[];Object.defineProperty(window,'speechSynthesis',{value:{cancel(){},getVoices(){return[]},speak(u){window.__spoken.push(u.text);setTimeout(()=>{u.onboundary?.({charIndex:8});if(!window.__hold)u.onend?.()},100)}}});window.SpeechSynthesisUtterance=class{constructor(t){this.text=t}};}"""))
   b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{port}/',wait='complete'))
   def js(c):return b.evaluate('(async()=>{'+c+'})()')
   def wait(e):assert js("return new Promise((r,j)=>{const end=performance.now()+8000;const p=()=>{if("+e+")r(true);else if(performance.now()>end){console.warn(JSON.stringify({spoken:window.__spoken,status:document.getElementById('voice-status').textContent,notice:document.getElementById('notice').textContent}));j(Error('timeout'));}else setTimeout(p,30)};p()})")
   wait("document.querySelector('#quick-actions button')");js("localStorage.removeItem('lantern-auto-speak');document.getElementById('auto-speak').checked=false;return true")
   for room in ['gallery','workshop']:
    js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
   wait("document.getElementById('speech-text').textContent.includes('Ilen')");assert js('return window.__spoken.length')==0
   js("document.getElementById('show-setup').click();document.querySelector('[data-settings-tab=voice]').click();document.getElementById('auto-speak').checked=true;document.getElementById('auto-speak').dispatchEvent(new Event('change'));return true");wait("window.__spoken.length===1&&document.getElementById('play').disabled")
   assert js("return localStorage.getItem('lantern-auto-speak')")== 'true';js("document.querySelector('[data-settings-tab=models]').click();document.getElementById('reasoning-mode').value='auto';document.getElementById('reasoning-apply').click();return true");wait("!document.getElementById('reasoning-apply').disabled&&document.getElementById('reasoning-status').textContent.includes('auto')")
   assert js("return(await(await fetch('/api/state')).json()).deliberation.config.mode")=='auto'
   js("document.getElementById('settings-close').click();window.__hold=true;document.getElementById('input').value='*approaches the merchant*';document.getElementById('chat').requestSubmit();return true");wait("window.__spoken.length===2&&document.getElementById('voice-progress').value===8")
   js("document.getElementById('interrupt').click();return true");wait("document.getElementById('voice-status').textContent.includes('Interrupted')")
   s=json.loads(js("return JSON.stringify(await(await fetch('/api/state')).json())"));assert s['speech']['delivery']['heard']==8 and s['speech']['delivery']['stage']=='interrupted';assert s['spending']['charged_or_reserved_micros']==0
   errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
   r=dict(workflows=5,autoplay_opt_in=True,once_per_reply=True,preference_saved=True,settings_applied=True,interrupted_heard_offset=8,page_console_errors=0,provider_calls=0,api_spend_usd=0,limits='Real Firefox and host, simulated browser speech callbacks. No hardware audio or live reasoning quality claim.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
   print(json.dumps(r))
  except Exception:
   if b: print(js("return JSON.stringify({spoken:window.__spoken,status:document.getElementById('voice-status').textContent,notice:document.getElementById('notice').textContent,speech:(await(await fetch('/api/state')).json()).speech})"))
   raise
  finally:
   if b:b.call('session.end',{})
   host.terminate();host.wait(timeout=5)
if __name__=='__main__':main()
