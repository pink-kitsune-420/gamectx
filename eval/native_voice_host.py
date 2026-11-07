"""Offline native-engine lifecycle, cancellation and host HTTP smoke."""
import argparse,base64,http.client,json,os,pathlib,re,subprocess,sys,tempfile,time,wave
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 with tempfile.TemporaryDirectory(prefix='native-voice-')as tmp:
  d=pathlib.Path(tmp);engine=d/'engine';engine.write_text('#!'+sys.executable+'\n'+'''import os,pathlib,sys,time,wave,subprocess
args=sys.argv[1:];root=pathlib.Path(__file__).parent
(root/'engine.pid').write_text(str(os.getpid()))
if (root/'slow').exists():
 child=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)']);(root/'grandchild.pid').write_text(str(child.pid));time.sleep(60)
if '--output_file' in args:
 text=sys.stdin.read();assert text.endswith('\\n');p=args[args.index('--output_file')+1]
 with wave.open(p,'wb')as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(22050);w.writeframes(b'\\0\\0'*2205)
else:
 assert args[args.index('--threads')+1]=='4'
 pathlib.Path(args[args.index('--output-file')+1]+'.txt').write_text('Hello, merchant.')
''');engine.chmod(0o700);(d/'weights').write_text('original mock')
  native=dict(version=1,tts=dict(executable=str(engine),voices=dict(alloy=str(d/'weights'))),stt=dict(executable=str(engine),models={'whisper-tiny':str(d/'weights')}));(d/'native-voice.json').write_text(json.dumps(native))
  c=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());c.update(tts='local_tts_http',stt='local_stt_http');
  for k,e in c['endpoints'].items():
   if k.startswith('local_'):e['url']='http://127.0.0.1:1/unused'
  (d/'providers.json').write_text(json.dumps(c));policy=json.loads((ROOT/'examples/dungeon/web/models.json').read_text());policy.update(enabled=True,ram_limit_mib=256,reserve_mib=128)
  for p in policy['profiles'].values():p['ram_mib']=64
  policy['profiles']['tts'].update(model='piper',backend='local_tts_http');policy['profiles']['stt'].update(model='whisper-tiny',backend='local_stt_http');(d/'models.json').write_text(json.dumps(policy))
  child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);checks=0
  try:
   line=child.stdout.readline();port=int(re.search(r'127\.0\.0\.1:(\d+)',line)[1])
   def call(op=None,**args):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert r.status==200,(r.status,v);return v['result']if op else v
   def wait(test=lambda s:not s['pending_jobs']):
    end=time.monotonic()+8
    while time.monotonic()<end:
     s=call()
     if test(s):return s
     time.sleep(.03)
    raise AssertionError(('timeout',s['notice']))
   s=wait();assert 'tts'in s['model_manager']['runtime']['eligible']and'stt'in s['model_manager']['runtime']['eligible']
   for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   s=call();call('chat',text='Hello.');s=wait();sid=s['speech']['id'];call('synthesize',id=sid);s=wait();assert s['speech']['audio']['mime']=='audio/wav'and len(base64.b64decode(s['speech']['audio']['audio']))>44;assert not(d/'voice-job').exists()
   session=call('mic')['session'];wav=d/'capture.wav'
   with wave.open(str(wav),'wb')as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(16000);w.writeframes(b'\0\0'*1600)
   call('transcribe',session=session,mime='audio/wav',audio=base64.b64encode(wav.read_bytes()).decode());s=wait();assert 'Hello, merchant.'in json.dumps(s['recognition'])
   call('manager_action',action='stop',profile='tts');s=wait();assert 'tts'not in s['model_manager']['runtime']['eligible'];call('manager_action',action='start',profile='tts');wait()
   # Cancel a real process tree through the host control while the worker is running.
   (d/'slow').touch();call('chat',text='Hello again.');s=wait();call('synthesize',id=s['speech']['id']);wait(lambda s:(d/'grandchild.pid').exists());parent=int((d/'engine.pid').read_text());grand=int((d/'grandchild.pid').read_text());call('manager_action',action='stop',profile='tts');s=wait();assert 'tts'not in s['model_manager']['runtime']['eligible'];assert not(d/'voice-job').exists()
   def dead(pid):
    p=pathlib.Path('/proc')/str(pid)/'stat';return not p.exists()or p.read_text().split()[2]=='Z'
   assert dead(parent)and dead(grand),'cancelled process tree survived'
   assert s['spending']['charged_or_reserved_micros']==0
   call('manager_action',action='start',profile='tts');wait();(d/'grandchild.pid').unlink();call('chat',text='A final greeting.');s=wait();call('synthesize',id=s['speech']['id']);wait(lambda s:(d/'grandchild.pid').exists());parent=int((d/'engine.pid').read_text());grand=int((d/'grandchild.pid').read_text());child.terminate();child.wait(timeout=5)
   end=time.monotonic()+4
   while time.monotonic()<end and not(dead(parent)and dead(grand)):time.sleep(.05)
   assert dead(parent)and dead(grand),'process tree survived host exit'
   return dict(http_checks=checks,workflows=7,native_mock_jobs=4,parent_exit_process_tree_cleanup=True,cancelled_process_tree=True,temporary_capture_removed=True,api_spend_usd=0,actual_provider_calls=0,limits='Original CLI fixtures through the real supervisor and host. Tests WAV/text transport, one-shot cleanup, stop/start and cancellation; not installed engine quality or physical audio.')
  finally:child.terminate();child.wait(timeout=5)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
