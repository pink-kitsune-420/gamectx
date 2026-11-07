"""Manual installed-engine smoke through the game host; no downloads or paid calls."""
import argparse,base64,http.client,json,pathlib,re,subprocess,tempfile,time,wave
ROOT=pathlib.Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--installation',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 with tempfile.TemporaryDirectory(prefix='voice-live-')as d:
  d=pathlib.Path(d);exe=str(ROOT/'target/debug/examples/dungeon');subprocess.run([exe,'--configure-voice',str(a.installation),'--web-state',str(d)],check=True,capture_output=True)
  config=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());config.update(tts='local_tts_http',stt='local_stt_http');(d/'providers.json').write_text(json.dumps(config));policy=json.loads((ROOT/'examples/dungeon/web/models.json').read_text());policy['enabled']=True;policy['profiles']['tts'].update(model='piper',backend='local_tts_http',ram_mib=300);policy['profiles']['stt'].update(model='whisper-tiny',backend='local_stt_http',ram_mib=300);(d/'models.json').write_text(json.dumps(policy))
  child=subprocess.Popen([exe,'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',child.stdout.readline())[1])
   def call(op=None,**args):
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();assert r.status==200,(r.status,v);return v['result']if op else v
   def settled():
    end=time.monotonic()+100
    while time.monotonic()<end:
     s=call()
     if not s['pending_jobs']:return s
     time.sleep(.1)
    raise AssertionError('voice did not finish')
   settled()
   for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   call('chat',text='Hello.');s=settled();call('synthesize',id=s['speech']['id']);s=settled();audio=s['speech']['audio'];assert audio.get('mime')=='audio/wav',(s['notice'],audio);wav=d/'voice.wav';wav.write_bytes(base64.b64decode(audio['audio']))
   with wave.open(str(wav))as w:rate=w.getframerate();frames=w.getnframes()
   # A new original phrase, synthesized by the user's installed wrapper and normalized.
   phrase='Hello merchant. I would like some chalk.';src=d/'phrase.wav';subprocess.run([str(a.installation/'bin/voice'),'tts',phrase,'--voice','merchant','-o',str(src)],check=True,capture_output=True,timeout=90)
   capture=d/'capture.wav';subprocess.run(['ffmpeg','-nostdin','-loglevel','error','-i',str(src),'-ar','16000','-ac','1','-c:a','pcm_s16le',str(capture)],check=True,timeout=15)
   # Normalize ffmpeg's additional WAV chunks into the exact host PCM contract.
   with wave.open(str(capture))as w:data=w.readframes(w.getnframes())
   with wave.open(str(capture),'wb')as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(16000);w.writeframes(data)
   session=call('mic')['session'];call('transcribe',session=session,mime='audio/wav',audio=base64.b64encode(capture.read_bytes()).decode());s=settled();recognition=s['recognition'];assert 'chalk'in json.dumps(recognition).lower(),(s['notice'],recognition);assert not(d/'voice-job').exists()
   result=dict(installed_host_tts_jobs=1,installed_host_stt_jobs=1,reference_tts_jobs=1,tts_sample_rate=rate,tts_frames=frames,recognition=recognition,temporary_capture_removed=True,api_spend_usd=0,limits='Manual installed Piper and whisper.cpp with synthetic speech and separate user-installed weights/wrappers. One phrase only; no real microphone, subjective voice quality, broad accuracy, latency or RSS measurement.')
   if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
   print(json.dumps(result))
  finally:child.terminate();child.wait(timeout=5)
if __name__=='__main__':main()
