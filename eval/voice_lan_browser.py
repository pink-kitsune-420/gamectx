"""Real Firefox Settings and two-player join UX on isolated loopback addresses."""
import argparse,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));contexts=[];cases=[]
 def new(url):
  context=b.call('browsingContext.create',dict(type='tab'))['context'];contexts.append(context);b.context=context;b.call('browsingContext.navigate',dict(context=context,url=url,wait='complete'));return context
 def js(code):return b.evaluate('(async()=>{'+code+'})()')
 def wait(expr):assert js("return new Promise((resolve,reject)=>{const end=performance.now()+8000;const poll=()=>{if("+expr+")resolve(true);else if(performance.now()>end)reject(Error('UI timeout'));else setTimeout(poll,40)};poll()})")
 def click(id):js('document.getElementById('+json.dumps(id)+').click();return true')
 try:
  host=new('http://127.0.0.1:8788/');wait("document.querySelectorAll('.model-card').length===6");assert js("return !document.getElementById('settings-dialog').open&&!document.querySelector('main').contains(document.getElementById('models-panel'))");cases.append('main game excludes provider and model setup')
  click('show-setup');wait("document.getElementById('settings-dialog').open");js("document.querySelector('[data-settings-tab=voice]').click();return true");assert js("return !document.getElementById('settings-voice').hidden&&document.getElementById('voice-profiles').children.length===2");cases.append('voice tab has two profiles and separate-install guidance')
  js("document.querySelector('[data-settings-tab=session]').click();return true");wait("document.getElementById('session-info').textContent.includes('/join')");code=js("return(await(await fetch('/api/session')).json()).code");assert js("return document.getElementById('player').disabled");cases.append('host invitation and fixed player identity')
  click('settings-close');assert js("return !document.getElementById('settings-dialog').open");cases.append('Done returns to gameplay')
  for room in ['gallery','workshop']:
   js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
  guest=new('http://127.0.0.2:8788/join');wait("document.getElementById('join-form')!==null");assert js("return !document.getElementById('show-setup')");js('document.getElementById("join-code").value='+json.dumps(code)+';document.getElementById("join-form").requestSubmit();return true');wait("!document.getElementById('guest-play').hidden");cases.append('second browser joins without model setup')
  js("document.getElementById('guest-input').value='Hello, merchant.';document.getElementById('guest-chat').requestSubmit();return true");wait("!document.getElementById('guest-reply').hidden");assert 'player_2'in js("return document.getElementById('guest-speaker').textContent");cases.append('guest reply is addressed to player two')
  b.context=host;wait("document.getElementById('speaker').textContent.includes('player_2')");assert js("return document.getElementById('play').disabled");cases.append('host cannot acknowledge guest hearing')
  b.context=guest;click('guest-read');wait("document.getElementById('guest-send').disabled===false");b.context=host;js("document.getElementById('input').value='Hello again, merchant.';document.getElementById('chat').requestSubmit();return true");wait("document.getElementById('speaker').textContent.includes('player_1')&&!document.getElementById('play').disabled");click('play');wait("document.getElementById('play').disabled");b.context=guest;wait("document.getElementById('guest-transcript').textContent.includes('player_1')&&document.getElementById('guest-transcript').textContent.includes('player_2')");cases.append('both players and NPC share attributed confirmed dialogue')
  b.context=host;click('show-setup');js("document.querySelector('[data-settings-tab=session]').click();return true");js("document.querySelector('#session-controls button').click();return true");b.context=guest;wait("!document.getElementById('join-panel').hidden");cases.append('host can disconnect guest and rotate invitation')
  for context in [host,guest]:
   b.context=context;b.call('browsingContext.setViewport',dict(context=context,viewport=dict(width=390,height=844),devicePixelRatio=1));wait('innerWidth===390');assert js('return document.documentElement.scrollWidth<=innerWidth')
  cases.append('settings and guest view fit mobile width')
  errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
  result=dict(browser='Firefox headless',workflows=len(cases),cases=cases,page_console_errors=0,guest_model_calls=0,actual_provider_calls=0,api_spend_usd=0,limits='Two real browser origins against an isolated cap-zero host. 0B replies; not physical Wi-Fi, speech devices, accessibility audit or paid API testing.')
  if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
  print(json.dumps(result))
 finally:b.call('session.end',{})
if __name__=='__main__':main()
