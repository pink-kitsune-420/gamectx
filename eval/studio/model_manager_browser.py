"""Real Firefox model controls against an isolated, unbound-provider offline host."""
import argparse,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()]
from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--port',type=int,default=8788);p.add_argument('--webdriver',type=int,default=9223);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 b=Bidi(a.webdriver);b.call('session.new',dict(capabilities={}));b.context=b.call('browsingContext.create',dict(type='tab'))['context'];b.call('session.subscribe',dict(events=['log.entryAdded']));cases=[]
 def js(code):return b.evaluate('(async()=>{'+code+'})()')
 def wait(condition):assert js("return new Promise((resolve,reject)=>{const end=performance.now()+8000;const poll=()=>{if("+condition+")resolve(true);else if(performance.now()>end)reject(Error('UI timeout'));else setTimeout(poll,50)};poll()})")
 def change(id,value):js('const e=document.getElementById('+json.dumps(id)+');'+('e.checked=' if isinstance(value,bool)else'e.value=')+json.dumps(value)+";e.dispatchEvent(new Event('input',{bubbles:true}));return true")
 def click(id):js('document.getElementById('+json.dumps(id)+').click();return true')
 try:
  b.call('browsingContext.setViewport',dict(context=b.context,viewport=dict(width=1366,height=768),devicePixelRatio=1));b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{a.port}/',wait='complete'));wait("document.querySelectorAll('.model-card').length===6")
  assert js("return !document.getElementById('manager-enabled').checked");cases.append('six editable profiles, manager initially opt-in')
  change('manager-mode','dramatic');change('mr-low_health',2);wait("document.getElementById('manager-draft').textContent==='Not applied'")
  click('mm-text-prewarm');wait("document.getElementById('manager-message').textContent.includes('Apply profile changes')");cases.append('draft loading rejected until policy applied')
  click('manager-apply');wait("document.getElementById('manager-message').textContent.includes('Applied.')");wait("!document.getElementById('manager-apply').disabled")
  assert js("const s=await(await fetch('/api/state')).json();return s.model_manager.config.mode==='dramatic'&&s.model_manager.config.rules.low_health===2&&!s.model_manager.config.enabled");cases.append('manual drama and custom trigger persist through apply')
  change('mm-dramatic_text-backend','online_openai_compatible');change('mm-dramatic_text-model','configured-online');click('manager-apply');wait("document.getElementById('manager-message').textContent.includes('Applied.')&&document.getElementById('manager-draft').textContent==='Applied'")
  wait("document.getElementById('mm-dramatic_text-prewarm').disabled");assert js("return document.getElementById('mm-dramatic_text-start').disabled");cases.append('online profiles expose no local load/start controls')
  change('mm-image-selection','invalid-json');click('manager-apply');wait("document.getElementById('manager-message').textContent.toLowerCase().includes('json')");cases.append('invalid weight selection shows actionable parse error')
  change('mm-image-selection',json.dumps(dict(model='checkpoints/sd-v1-5-q8.gguf')));click('manager-apply');wait("document.getElementById('manager-draft').textContent==='Applied'")
  js("const e=document.getElementById('manager-art');const v=JSON.parse(e.value);v.style+='; deliberate layered ink';e.value=JSON.stringify(v);return true");click('manager-art-apply');wait("document.getElementById('manager-message').textContent.includes('validated')");cases.append('detailed dramatic art bible validated and applied')
  b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{a.port}/',wait='complete'));wait("document.querySelectorAll('.model-card').length===6");assert js("return document.getElementById('manager-mode').value==='dramatic'&&document.getElementById('manager-art').value.includes('deliberate layered ink')");cases.append('reload preserves profile policy and art direction')
  b.call('browsingContext.setViewport',dict(context=b.context,viewport=dict(width=390,height=844),devicePixelRatio=1));wait('innerWidth===390');assert js('return document.documentElement.scrollWidth<=innerWidth');cases.append('model controls fit mobile without horizontal overflow')
  errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
  r=dict(browser='Firefox headless',browser_workflows=len(cases),cases=cases,page_console_errors=0,actual_provider_calls=0,api_spend_usd=0,limits='Real DOM controls and layout against an isolated cap-zero host with unbound provider roots. No installed-model inference, physical voice or paid API calls. Manual Firefox prerequisite; not a product dependency.')
  if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
  print(json.dumps(r))
 finally:b.call('session.end',{})
if __name__=='__main__':main()
