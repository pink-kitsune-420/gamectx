"""Manual installed-provider lifecycle smoke; no inference or online API requests.
Requires existing loopback Ollama and SD.cpp with the normal configured weights.
"""
import argparse,http.client,json,pathlib,re,subprocess,tempfile,time,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--model',required=True);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 def get(url):
  with urllib.request.urlopen(url,timeout=10)as f:return json.load(f)
 with tempfile.TemporaryDirectory(prefix='model-manager-live-')as d:
  directory=pathlib.Path(d);c=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());c.update(text='local_ollama',image='local_sd_cpp',cpu_threads=4);c['text_models']['local_ollama']['3']=a.model;(directory/'providers.json').write_text(json.dumps(c))
  policy=json.loads((ROOT/'examples/lantern-vault/web/models.json').read_text());policy.update(enabled=True,mode='normal',idle_seconds=60);policy['profiles']['text']['model']=a.model;(directory/'models.json').write_text(json.dumps(policy))
  child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(directory/'providers.json'),'--web-state',d],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   line=child.stdout.readline();match=re.search(r'127\.0\.0\.1:(\d+)',line);assert match,line;port=int(match[1])
   def call(action=None,profile=None):
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=5);c.request('POST'if action else'GET','/api/manager_action'if action else'/api/state',json.dumps(dict(action=action,profile=profile))if action else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=c.getresponse();v=json.loads(r.read());c.close();assert r.status==200,v;return v
   def settled():
    end=time.monotonic()+180
    while time.monotonic()<end:
     s=call()
     if not s['pending_jobs']:return s
     time.sleep(.15)
    raise AssertionError('Lifecycle timed out')
   settled();states=[]
   for action,profile in [('prewarm','text'),('unload','text'),('prewarm','image'),('unload','image'),('prewarm','image')]:
    call(action,profile);s=settled();assert not s['model_manager']['runtime'].get('error'),s['notice'];text=get('http://127.0.0.1:11434/api/ps');image=get('http://127.0.0.1:7860/sdcpp/v1/models/status');names=[r['name']for r in text['models']];state=image['state'];states.append(dict(action=action,profile=profile,text_resident=a.model in names,image_state=state))
    if profile=='text':assert (a.model in names)==(action=='prewarm')
    else:assert state==('ready'if action=='prewarm'else'empty')and a.model not in names
   assert s['spending']['charged_or_reserved_micros']==0
   r=dict(lifecycle_operations=len(states),states=states,external_servers_still_reachable=True,text_context_threads=4,actual_text_generations=0,actual_image_generations=0,api_spend_usd=0,limits='Manual installed normal Qwen 3B and SD 1.5 lifecycle acknowledgements only; final normal image context restored. No image/text quality, latency/RAM benchmark, larger models or physical audio measurement. Not part of offline automated CI.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
   print(json.dumps(r))
  finally:child.terminate();child.wait(timeout=5)
if __name__=='__main__':main()
