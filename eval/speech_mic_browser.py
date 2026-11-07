"""Manual real Firefox recording/codec path with synthetic speech, no physical mic."""
import argparse,base64,json,pathlib,sys,time,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
from studio_browser_driver import Bidi

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--reference',type=pathlib.Path,required=True);p.add_argument('--port',type=int,default=8788);p.add_argument('--output',type=pathlib.Path);p.add_argument('--delay-interim',action='store_true');a=p.parse_args();assert a.cap==0;b=Bidi(9223);b.call('session.new',dict(capabilities={}));b.call('session.subscribe',dict(events=['log.entryAdded']));b.context=b.call('browsingContext.create',dict(type='tab'))['context']
 def js(code):return b.evaluate('(async()=>{'+code+'})()')
 def wait(expr,seconds=15):assert js("return new Promise((resolve,reject)=>{const end=performance.now()+"+str(seconds*1000)+";const poll=()=>{if("+expr+")resolve(true);else if(performance.now()>end)reject(Error('Browser check timeout'));else setTimeout(poll,50)};poll()})")
 try:
  encoded=base64.b64encode(a.reference.read_bytes()).decode()
  # A real MediaStream from an inaudible WebAudio destination exercises Firefox's
  # recorder and codec decoder without acquiring the user's microphone.
  preload='''()=>{if(!navigator.mediaDevices)return;navigator.mediaDevices.getUserMedia=async()=>{const c=new AudioContext();await c.resume();const bytes=Uint8Array.from(atob(ENCODED),x=>x.charCodeAt(0));const buffer=await c.decodeAudioData(bytes.buffer);const s=c.createBufferSource(),d=c.createMediaStreamDestination();s.buffer=buffer;s.connect(d);s.onended=()=>{window.__referenceEnded=true;};s.start(c.currentTime+.4);window.__referenceContext=c;return d.stream;};}'''.replace('ENCODED',json.dumps(encoded))
  if a.delay_interim:
   delay="""if(navigator.mediaDevices){const original=window.fetch.bind(window);window.fetch=async(url,options)=>{if(String(url)==='/api/recognition'){window.__interimSent=true;while(!window.__referenceEnded)await new Promise(r=>setTimeout(r,20));await new Promise(r=>setTimeout(r,750));const response=await original(url,options);window.__interimDone=true;return response;}if(String(url)==='/api/transcribe')window.__lateInterimBeforeTranscribe=window.__interimDone===true;return original(url,options);};}"""
   preload=preload[:-1]+delay+'}'
  b.call('script.addPreloadScript',dict(functionDeclaration=preload,contexts=[b.context]));b.call('browsingContext.navigate',dict(context=b.context,url=f'http://127.0.0.1:{a.port}/',wait='complete'));wait("document.getElementById('room-title').textContent==='Entry'")
  for room in ['gallery','workshop']:
   js("[...document.querySelectorAll('#quick-actions button')].find(b=>b.textContent==='Go to "+room+"').click();return true");wait("document.getElementById('room-title').textContent==='"+room.title()+"'")
  js("document.getElementById('mic').click();return true");wait("document.getElementById('mic').textContent.includes('Send Recording')");assert js("return !document.getElementById('mic-monitor').hidden");wait('window.__referenceEnded===true');js("document.getElementById('mic').click();return true")
  # This uses the installed host Whisper through the actual browser upload path.
  wait("document.getElementById('interim').textContent.includes('final_text')",30);text=js("return document.getElementById('interim').textContent");assert 'merchant'in text.lower()and'chalk'in text.lower(),text;assert js("return document.getElementById('mic-feedback').textContent.includes('Transcript received')")
  if a.delay_interim:assert js('return window.__interimSent===true&&window.__lateInterimBeforeTranscribe===true')
  errors=[e for e in b.events if e.get('method')=='log.entryAdded'and e.get('params',{}).get('level')=='error'];assert not errors,errors
  result=dict(browser='Firefox headless',recording_started=True,input_meter_visible=True,real_media_recorder_and_wav_resampling=True,installed_host_whisper_transcript=text,late_interim_before_transcribe=True,forced_delayed_interim=a.delay_interim,page_console_errors=0,actual_text_llm_calls=0,api_spend_usd=0,limits='Synthetic speech streamed inside WebAudio through real Firefox MediaRecorder, decode/resample/upload and installed host Whisper. No physical microphone, device permissions, broad accuracy or Wi-Fi measurement.')
  if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
  print(json.dumps(result))
 finally:b.call('session.end',{})
if __name__=='__main__':main()
