"""Manual paired installed-Piper synthesis turnaround, cap zero, no text LLM."""
import argparse,base64,http.client,io,json,pathlib,re,subprocess,tempfile,time,wave
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run(exe,installation,prompt,interruptions=0):
 with tempfile.TemporaryDirectory(prefix='buffer-live-')as tmp:
  d=pathlib.Path(tmp);subprocess.run([str(exe),'--configure-voice',str(installation),'--web-state',str(d)],check=True,capture_output=True)
  cfg=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());cfg['tts']='local_tts_http'
  for key,e in cfg['endpoints'].items():
   if key.startswith('local_'):e['url']='http://127.0.0.1:1/unused'
  (d/'providers.json').write_text(json.dumps(cfg));policy=json.loads((ROOT/'examples/dungeon/web/models.json').read_text());policy['enabled']=True;policy['profiles']['tts'].update(model='piper',backend='local_tts_http',ram_mib=300);(d/'models.json').write_text(json.dumps(policy))
  child=subprocess.Popen([str(exe),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   line=child.stdout.readline();port=int(re.search(r'127\.0\.0\.1:(\d+)',line)[1])
   def call(op=None,**args):
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();assert r.status==200,(r.status,v);return v['result']if op else v
   def settled():
    end=time.monotonic()+120
    while time.monotonic()<end:
     s=call()
     if not s['pending_jobs']:return s
     time.sleep(.02)
    raise AssertionError('did not finish')
   settled()
   for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   for _ in range(interruptions):
    call('chat',text='Hello.');s=settled();sid=s['speech']['id'];call('delivery',id=sid,event='queued',through=0,heard=0);call('delivery',id=sid,event='interrupted',through=0,heard=0)
   call('chat',text=prompt);s=settled();reply=s['speech'];sid=reply['id'];text=reply['text'];call('delivery',id=sid,event='queued',through=0,heard=0);requests=[];duration=0;checkpoints=[]
   while True:
    begin=time.perf_counter();call('synthesize',id=sid);s=settled();elapsed=time.perf_counter()-begin;audio=s['speech']['audio'];assert audio.get('mime')=='audio/wav',(s['notice'],audio)
    with wave.open(io.BytesIO(base64.b64decode(audio['audio'])))as w:duration+=w.getnframes()/w.getframerate();assert w.getnchannels()==1 and w.getsampwidth()==2
    requests.append(round(elapsed,4));checkpoints.extend(audio.get('boundaries',[]));through=audio['through'];call('delivery',id=sid,event='played',through=through,heard=through)
    if through==len(text):break
   call('delivery',id=sid,event='finished',through=len(text),heard=len(text));s=call();assert s['speech']['delivery']['done']and s['spending']['charged_or_reserved_micros']==0;assert not(d/'voice-job').exists()
   return dict(text=text,characters=len(text),synthesis_jobs=len(requests),synthesis_seconds=round(sum(requests),4),per_job_seconds=requests,audio_seconds=round(duration,4),prepared_checkpoints=len(checkpoints),api_spend_usd=0)
  finally:child.terminate();child.wait(timeout=5)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--installation',type=pathlib.Path,required=True);p.add_argument('--baseline',type=pathlib.Path);p.add_argument('--holdout-only',action='store_true');p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 if a.holdout_only:
  r=run(ROOT/'target/debug/examples/dungeon',a.installation.resolve(),'Tell me about your stock.',3);assert r['characters']>80 and r['synthesis_jobs']==1 and r['prepared_checkpoints']>=2;r['limits']='Fresh installed-Piper holdout: one host job prepares multiple bounded phrases and one joined WAV. Simulated completed-delivery receipts; no physical audio or broad quality/RSS measurement.'
  if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
  print(json.dumps(r));return
 assert a.baseline is not None
 baseline=run(a.baseline.resolve(),a.installation.resolve(),'Hello.');candidate=run(ROOT/'target/debug/examples/dungeon',a.installation.resolve(),'Hello.');assert candidate['text']==baseline['text']and candidate['synthesis_jobs']==1 and baseline['synthesis_jobs']>1
 r=dict(baseline_commit='758dcf2',baseline=baseline,buffered=candidate,synthesis_turnaround_speedup=round(baseline['synthesis_seconds']/candidate['synthesis_seconds'],2),actual_text_llm_calls=0,api_spend_usd=0,limits='Manual sequential installed-Piper host requests, four-thread user launcher. One paired line; synthesis turnaround excludes physical playback, subjective quality and network jitter. Host delivery receipts are simulated completed playback. Not a broad latency/RSS or heard-character benchmark.')
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
