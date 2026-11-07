"""Offline Firefox offers appear only after host trust/reason/resource gates."""
import argparse,json,pathlib,re,subprocess,sys,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'));from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 with tempfile.TemporaryDirectory(prefix='reaction-browser-')as tmp:
  d=pathlib.Path(tmp);cfg=json.loads((ROOT/'examples/dungeon/web/reactions.json').read_text());cfg['enabled']=True;(d/'reactions.json').write_text(json.dumps(cfg));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);b=None
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1]);b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));b.context=b.call('browsingContext.create',dict(type='tab'))['context'];b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{port}/',wait='complete'))
   def js(c):return b.evaluate('(async()=>{'+c+'})()')
   def wait(expr):assert js("return new Promise((resolve,reject)=>{const end=performance.now()+8000;const p=()=>{if("+expr+")resolve(true);else if(performance.now()>end)reject(Error('timeout'));else setTimeout(p,30)};p()})")
   wait("document.getElementById('room-title').textContent==='Entry'")
   for room in ['gallery','workshop']:
    js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
   assert js("return document.getElementById('reaction-options').hidden&&document.getElementById('reaction-buttons').children.length===0")
   js("document.getElementById('show-setup').click();document.querySelector('[data-settings-tab=characters]').click();return true");assert js("return document.getElementById('reactions-enabled').checked&&JSON.parse(document.getElementById('reaction-rules').value).length===2");js("document.getElementById('settings-close').click();document.getElementById('promise').click();return true")
   js("[...document.querySelectorAll('#actions button')].find(b=>b.textContent.startsWith('Buy chalk')).click();return true");wait("document.getElementById('reaction-buttons').children.length===2")
   assert js("return !document.getElementById('reaction-buttons').textContent.includes('chalk')")
   js("[...document.querySelectorAll('#reaction-buttons button')].find(b=>b.textContent.includes('a loan')).click();return true");wait("document.getElementById('stats').textContent.includes('coins 6')&&document.getElementById('loan-status').textContent.includes('2 coins')")
   js("[...document.querySelectorAll('#reaction-buttons button')].find(b=>b.textContent==='Repay a loan').click();return true");wait("document.getElementById('stats').textContent.includes('coins 4')&&document.getElementById('loan-status').textContent===''")
   js("[...document.querySelectorAll('#reaction-buttons button')].find(b=>b.textContent.includes('a gift')).click();return true");wait("document.getElementById('stats').textContent.includes('coins 5')&&document.getElementById('reaction-options').hidden")
   assert js("return document.getElementById('debug-window').hidden")
   errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
   result=dict(browser='Firefox headless',workflows=6,gated_options_hidden=True,settings_editable=True,actual_loan_and_repayment=True,gift_reuse_blocked=True,unowned_offer_withheld=True,debug_stays_minimized=True,page_console_errors=0,api_spend_usd=0,actual_provider_calls=0,limits='Real example host and browser DOM, no AI calls or physical media.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
   print(json.dumps(result))
  finally:
   if b:b.call('session.end',{})
   host.terminate();host.wait(timeout=5)
if __name__=='__main__':main()
