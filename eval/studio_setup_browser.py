"""Real Firefox UI check against an isolated offline preview; no model calls."""
import argparse,base64,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--port',type=int,default=8788);p.add_argument('--webdriver',type=int,default=9223);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0 and 0<a.port<65536 and 0<a.webdriver<65536
 b=Bidi(a.webdriver);b.call('session.new',dict(capabilities={}));b.context=b.call('browsingContext.create',dict(type='tab'))['context'];b.call('session.subscribe',dict(events=['log.entryAdded']));cases=[]
 def js(expr):
  code=expr[:-4]+"return true;" if expr.endswith(";true") else "return ("+expr+");"
  return b.evaluate("(async()=>{"+code+"})()")
 def click(id):js(f"document.getElementById({json.dumps(id)}).click();true")
 def wait(expr):assert js("new Promise((resolve,reject)=>{const end=performance.now()+5000;const tick=()=>{if("+expr+")resolve(true);else if(performance.now()>end)reject(Error('UI check timed out'));else setTimeout(tick,50);};tick();})")
 def viewport(w,h):b.call('browsingContext.setViewport',dict(context=b.context,viewport=dict(width=w,height=h),devicePixelRatio=1))
 try:
  viewport(1366,630);b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{a.port}/',wait='complete'));wait("document.getElementById('room-title').textContent==='Entry'")
  assert js("JSON.parse(await (await fetch('/api/state')).text()).providers.text==='0b_fallback'")
  assert js("document.getElementById('installed-label').hidden&&getComputedStyle(document.getElementById('installed-label')).display==='none'");cases.append('offline start and hidden empty model list')
  for room in ['gallery','workshop']:
   js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();true");wait("document.getElementById('room-title').textContent.toLowerCase()==='"+room+"'")
  js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Talk to merchant').click();true");wait("!document.getElementById('current-reply').hidden");assert 'Offline reply' in js("document.getElementById('reply-origin').textContent");cases.append('talk button produces an explicit offline reply')
  input_bottom=js("document.getElementById('input').getBoundingClientRect().bottom");assert input_bottom<=630,(input_bottom,'composer below desktop fold');cases.append('reply and composer visible at 1366 by 630')
  click('play');wait("document.getElementById('play').disabled");assert js("(await (await fetch('/api/state')).json()).speech.delivery.done");cases.append('single click acknowledges text reply')
  click('save');wait("document.getElementById('notice').textContent.includes('saved')")
  js("document.getElementById('text-backend').value='local_ollama';document.getElementById('text-backend').dispatchEvent(new Event('change'));true");assert 'Not applied' in js("document.getElementById('settings-state').textContent");assert js("(await (await fetch('/api/state')).json()).providers.text==='0b_fallback'");cases.append('unsaved settings visible without changing applied provider')
  js("document.getElementById('text-backend').value='0b_fallback';document.getElementById('text-backend').dispatchEvent(new Event('change'));true")
  click('new-game');assert js("document.getElementById('reset-dialog').open");click('cancel-reset');assert js("document.getElementById('room-title').textContent==='Workshop'");cases.append('cancel new game preserves progress')
  click('new-game');click('confirm-reset');wait("document.getElementById('room-title').textContent==='Entry'&&!document.getElementById('reset-dialog').open");cases.append('confirmed new game resets current session')
  click('load');wait("document.getElementById('room-title').textContent==='Workshop'");cases.append('saved branch survives new game')
  b.call('browsingContext.captureScreenshot',dict(context=b.context))
  viewport(390,844);wait("innerWidth===390");assert js("document.documentElement.scrollWidth<=innerWidth"),'mobile horizontal overflow';cases.append('mobile has no horizontal overflow')
  errors=[e for e in b.events if e.get('method')=='log.entryAdded' and e.get('params',{}).get('level')=='error'];assert not errors,errors
  result=dict(browser='Firefox headless',browser_workflows=len(cases),cases=cases,desktop_viewport=[1366,630],composer_bottom_px=input_bottom,mobile_viewport=[390,844],page_console_errors=0,actual_provider_calls=0,api_spend_usd=0,limits='Real DOM, click and layout checks on an isolated offline preview; not physical audio/STT, image generation or an independent accessibility study. Local Firefox WebDriver service is a manual evaluation prerequisite, not a product or CI dependency.')
  if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
  print(json.dumps(result))
 finally:b.call('session.end',{})
if __name__=='__main__':main()
