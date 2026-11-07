"""Offline owned-service start/stop and parent-exit cleanup; no models installed."""
import argparse,http.client,json,os,pathlib,re,socket,subprocess,sys,tempfile,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 with tempfile.TemporaryDirectory(prefix='manager-owned-') as d:
  d=pathlib.Path(d);sockets=[];ports=[]
  for _ in range(3):
   s=socket.socket();s.bind(('127.0.0.1',0));ports.append(s.getsockname()[1]);sockets.append(s)
  for s in sockets:s.close()
  service=d/'fixture_service.py';service.write_text('import http.server,os,pathlib,sys\npathlib.Path(sys.argv[2]).write_text(str(os.getpid()))\ns=http.server.HTTPServer(("127.0.0.1",int(sys.argv[1])),http.server.BaseHTTPRequestHandler)\ns.serve_forever()\n')
  c=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text())
  for backend in ['local_ollama','local_sd_cpp']:c['endpoints'][backend]['url']=f'http://127.0.0.1:{ports[0]}/unused'
  c['endpoints']['local_tts_http']['url']=f'http://127.0.0.1:{ports[1]}/v1/audio/speech';c['endpoints']['local_stt_http']['url']=f'http://127.0.0.1:{ports[2]}/v1/audio/transcriptions';config=d/'providers.json';config.write_text(json.dumps(c))
  launch=json.loads((ROOT/'examples/dungeon/web/launchers.json').read_text())
  for channel,port in [('tts',ports[1]),('stt',ports[2])]:launch['services'][channel]=dict(argv=[sys.executable,'-I',str(service),str(port),str(d/(channel+'.pid'))],env={})
  (d/'launchers.json').write_text(json.dumps(launch));policy=json.loads((ROOT/'examples/dungeon/web/models.json').read_text());policy.update(reserve_mib=128,ram_limit_mib=256)
  for p in policy['profiles'].values():p['ram_mib']=64
  policy['profiles']['tts']['backend']='local_tts_http';policy['profiles']['stt']['backend']='local_stt_http'
  (d/'models.json').write_text(json.dumps(policy));child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(config),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  checks=0
  try:
   line=child.stdout.readline();port=int(re.search(r'127\.0\.0\.1:(\d+)',line)[1])
   def call(action=None,profile=None,status=200):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=6);h.request('POST'if action else'GET','/api/manager_action'if action else'/api/state',json.dumps(dict(action=action,profile=profile))if action else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert r.status==status,(r.status,v);return v
   def settled():
    end=time.monotonic()+5
    while time.monotonic()<end:
     s=call()
     if not s['pending_jobs']:return s
     time.sleep(.03)
    raise AssertionError('owned service did not settle')
   def gone(pid):
    end=time.monotonic()+4
    while time.monotonic()<end:
     try:os.kill(pid,0)
     except ProcessLookupError:return True
     time.sleep(.05)
    return False
   call('start','tts');s=settled();assert any(v['channel']=='tts'and v['ownership']=='managed'for v in s['model_manager']['runtime']['services']),(s['notice'],s['model_manager'])
   first=int((d/'tts.pid').read_text());call('stop','tts');settled();assert gone(first)
   call('start','tts');settled();second=int((d/'tts.pid').read_text());assert second!=first
   call('start','stt');s=settled();assert gone(second);third=int((d/'stt.pid').read_text());assert any(v['channel']=='stt'and v['ownership']=='managed'for v in s['model_manager']['runtime']['services'])
   call('start','tts;echo-unsafe',status=400)
   child.terminate();child.wait(timeout=5);assert gone(third),'owned child survived parent host exit'
   return dict(http_checks=checks,owned_services_started=3,explicit_stop_verified=True,replaced_service_stopped=True,parent_exit_cleanup_verified=True,arbitrary_command_rejected=True,actual_provider_calls=0,api_spend_usd=0,limits='Original Python socket-service fixtures started via the actual Rust supervisor. Tests ownership/lifetime, not Kokoro/Whisper inference, third-party descendant process cleanup or model quality.')
  finally:
   if child.poll()is None:child.terminate();child.wait(timeout=5)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
