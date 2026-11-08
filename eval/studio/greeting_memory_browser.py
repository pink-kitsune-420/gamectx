"""Real Firefox contextual entry/approach greetings and stage-direction presentation."""
import argparse,json,pathlib,re,subprocess,sys,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()];from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 with tempfile.TemporaryDirectory(prefix='greeting-browser-')as tmp:
  d=pathlib.Path(tmp);mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);b=None
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1]);b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));b.context=b.call('browsingContext.create',dict(type='tab'))['context'];b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{port}/',wait='complete'))
   def js(c):return b.evaluate('(async()=>{'+c+'})()')
   def wait(expr):assert js("return new Promise((resolve,reject)=>{const end=performance.now()+8000;const p=()=>{if("+expr+")resolve(true);else if(performance.now()>end)reject(Error('timeout'));else setTimeout(p,30)};p()})")
   def go(room):js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
   def read():js("document.getElementById('play').click();return true");wait("document.getElementById('play').disabled")
   wait("document.getElementById('room-title').textContent==='Entry'");go('gallery');go('workshop');wait("document.getElementById('speech-text').textContent.includes('Ilen')");assert js("return document.getElementById('transcript').textContent.includes('enters the workshop')&&document.querySelector('.stage-direction')!==null");assert js("return !(await(await fetch('/api/state')).json()).transcript.some(r=>r.kind==='player')")
   before=js("return(await(await fetch('/api/state')).json()).speech.id");js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Approach merchant').click();return true");wait("document.activeElement.id==='input'");assert js("return(await(await fetch('/api/state')).json()).speech.id")==before;read()
   js("document.getElementById('input').value='*approaches the merchant*';document.getElementById('chat').requestSubmit();return true");wait("document.getElementById('speech-text').textContent.includes('still listening')");assert js("return document.getElementById('transcript').textContent.includes('approaches the merchant')");read();go('gallery');go('workshop');wait("document.getElementById('speech-text').textContent.toLowerCase().includes('back')");assert js("return !document.getElementById('speech-text').textContent.includes('Ilen')");read()
   go('gallery');go('cistern');wait("document.getElementById('speech-text').textContent.includes('Tovin')");assert js("return document.getElementById('speech-text').textContent.includes('cistern')");read()
   js("document.getElementById('player').value='player_2';document.getElementById('player').dispatchEvent(new Event('change'));return true");wait("document.getElementById('player').value==='player_2'&&document.getElementById('greeting-debug').textContent.includes('\"meeting\": \"first\"')");js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Approach keeper').click();return true");wait("document.getElementById('speaker').textContent.includes('player_2')&&document.getElementById('speech-text').textContent.includes('Tovin')");read()
   js("document.getElementById('show-setup').click();document.querySelector('[data-settings-tab=characters]').click();document.getElementById('greeting-gap').value='4';document.getElementById('greeting-apply').click();return true");wait("document.getElementById('greeting-status').textContent.includes('applied')");js("document.getElementById('settings-close').click();document.getElementById('show-debug').click();return true");wait("document.getElementById('greeting-debug').textContent.includes('continuing')");js("document.getElementById('debug-minimize').click();return true")
   errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
   result=dict(browser='Firefox headless',workflows=7,entry_stage_direction=True,first_name_introduction=True,approach_does_not_invent_hello=True,pending_introduction_reused=True,same_encounter_not_reintroduced=True,return_welcome=True,keeper_and_player_isolation=True,editable_gap=True,floating_debug=True,page_console_errors=0,actual_provider_calls=0,api_spend_usd=0,limits='Real host/DOM with text read receipts, no physical audio, LAN device or live AI quality measurement.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
   print(json.dumps(result))
  finally:
   if b:b.call('session.end',{})
   host.terminate();host.wait(timeout=5)
if __name__=='__main__':main()
