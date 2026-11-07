import {
  VoicePlayer,prefix}
from './voice.mjs';
const $=id=>document.getElementById(id);
let recognitionSerial=Promise.resolve();
let state,configured=false,lastTranscript='',lastSpeech=0,mic=null,session=0;
const post=async(op,args={
}
)=>{
  const response=await fetch('/api/'+op,{
    method:'POST',headers:{
      'Content-Type':'application/json','X-Studio-Request':'1'}
    ,body:JSON.stringify(args)}
  );
  const reply=await response.json();
  if(!response.ok||!reply.ok)throw Error(reply.error||'The host rejected a stale request.');
  return reply;
}
;
function voiceView(v) {
  if(v.status)$('voice-status').textContent=v.status;
  if(v.text!==undefined){
    const through=v.through||0;
    const mark=document.createElement('mark');
    mark.textContent=prefix(v.text,through);
    $('speech-text').replaceChildren(mark,document.createTextNode(Array.from(v.text).slice(through).join('')));
    $('voice-progress').max=Math.max(1,Array.from(v.text).length);
    $('voice-progress').value=through;
  }
}
const player=new VoicePlayer({
  post,emit:voiceView}
);
async function invoke(op,args={
}
) {
  try {
    const result=await post(op,args);
    await refresh();
    return result;
  }
  catch(e){
    $('notice').textContent=e.message;
    throw e;
  }
}
function image(id,reference) {
  const img=$(id);
  if(reference){
    const url='/api/asset/'+reference.key+'?digest='+reference.digest;
    if(img.getAttribute('src')!==url)img.src=url;
    img.onerror=()=>{
      img.hidden=true;
    }
    ;
    img.hidden=false;
  }
  else{
    img.hidden=true;
    img.removeAttribute('src');
  }
}
function settings(s) {
  for(const m of ['text','image','tts','stt'])$(m+'-backend').value=s.providers[m];
  $('size').value=s.providers.size;
  $('speech-mode').value=s.providers.speech_mode;
  $('cap').value=s.spending.cap_micros/1e6;
  $('cap').max=s.spending.authorized_cap_micros/1e6;
  voiceSettings();
}
function voiceSettings(){
  if(!state)return;
  const v=state.providers.voices[$('npc').value];
  $('voice-id').value=v.voice_id;
  $('pitch').value=v.pitch;
  $('rate').value=v.rate;
}
function drawAsset(){
  if(!state)return;
  const a=$('asset-target').value==='scene'?state.scene_asset:state.portraits;
  $('asset-inspection').textContent=JSON.stringify(a.inspection,null,2);
  const candidate=$('candidate');
  if($('preview-candidate').checked&&a.inspection.candidate){
    candidate.src='/api/asset/'+a.selection.key+'?candidate=1';
    candidate.hidden=false;
  }
  else{
    candidate.hidden=true;
    candidate.removeAttribute('src');
  }
}
let refreshing=false;
async function refresh(){
  if(refreshing)return;
  refreshing=true;
  try{
    const response=await fetch('/api/state');
    if(!response.ok)throw Error('Host state unavailable.');
    const s=await response.json();
    state=s;
    $('resume-local').hidden=!s.local_paused;
    $('room-title').textContent=s.game.room.replace('room:','').replace(/^./,x=>x.toUpperCase());
    $('room-label').textContent=s.game.room.replace('room:','');
    $('npc-label').textContent=s.npc+(s.npc_present?' · present':' · not in this room');
    $('description').textContent=s.game.description;
    $('stats').textContent=`Party health ${s.game.hp}/6 · Shared coins ${s.game.coins} · Revision ${s.game.revision}`;
    $('player').value=s.player;
    $('npc').value=s.npc;
    $('presence').textContent=s.npc_present?'In speaking range':'NPC is elsewhere';
    const actions=s.game.choices.map(choice=>{
      const b=document.createElement('button');
      b.textContent=[choice.action,choice.target?.split(':').at(-1)].filter(Boolean).join(' ');
      b.onclick=async()=>{
        await player.interrupt();
        await invoke('act',{
          revision:s.game.revision,decision:choice}
        );
      }
      ;
      return b;
    }
    );
    $('actions').replaceChildren(...actions);
    const transcript=JSON.stringify(s.transcript);
    if(transcript!==lastTranscript){
      lastTranscript=transcript;
      $('transcript').replaceChildren(...s.transcript.map(row=>{
        const p=document.createElement('div');
        p.className='line'+(row.speaker==='host'?' host':'');
        const name=document.createElement('strong');
        name.textContent=row.speaker+' · '+row.kind;
        p.append(name,document.createTextNode(row.text));
        return p;
      }
      ));
      $('transcript').scrollTop=$('transcript').scrollHeight;
    }
    if(s.speech){
      $('speaker').textContent=s.speech.actor+' → '+s.speech.player;
      if(s.speech.id!==lastSpeech){
        lastSpeech=s.speech.id;
        voiceView({
          text:s.speech.text,through:s.speech.delivery.heard,status:s.speech.pending?'Model pending · fallback available':'Ready'}
        );
      }
      if(!player.current||player.current.id!==s.speech.id||player.current.closed) {
        if(s.speech.delivery.stage==='validated')voiceView({
          text:s.speech.text,through:0,status:s.speech.pending?'Model pending · fallback available':'Ready'}
        );
      }
      await player.chunk(s.speech);
    }
    image('scene-image',s.scene_asset.selection.asset);
    image('portrait-image',s.portraits.visible?s.portraits.selection.asset:null);
    drawAsset();
    $('attitudes').replaceChildren(...s.characters.find(c=>c.id===s.npc).life.effective_attitudes.filter(a=>a.definition.subject===s.player).map(a=>{
      const div=document.createElement('div');
      div.className='meter';
      const label=document.createElement('span');
      label.textContent=`${a.definition.kind}: ${a.level}`;
      const meter=document.createElement('meter');
      meter.min=-100;
      meter.max=100;
      meter.value=a.level;
      div.append(label,meter);
      return div;
    }
    ));
    $('memories').textContent=JSON.stringify({
      promises:s.promises,episodes:s.characters.map(c=>({
        id:c.id,episodes:c.episodes}
      ))}
    ,null,2);
    $('living').textContent=JSON.stringify({
      selected:s.living,rumors:s.rumors}
    ,null,2);
    $('receipts').textContent=JSON.stringify({
      ...s.spending,ledger:s.spending_receipts,pending_jobs:s.pending_jobs}
    ,null,2);
    $('spend').textContent=`Committed $${(s.spending.charged_or_reserved_micros/1e6).toFixed(6)} / $${(s.spending.cap_micros/1e6).toFixed(6)} cap · ${s.pending_jobs} queued`;
    $('notice').textContent=s.notice;
    if(s.recognition&&s.recognition.text!=='[speech detected]'){
      $('interim').textContent=`${s.recognition.status}: ${s.recognition.text}${s.recognition.uncertainty?' · '+s.recognition.uncertainty:''}`;
    }
    if(!configured){
      configured=true;
      settings(s);
    }
  }
  finally{
    refreshing=false;
  }
}
$('player').onchange=$('npc').onchange=async()=>{
  await player.interrupt();
  await invoke('select',{
    player:$('player').value,npc:$('npc').value}
  );
  voiceSettings();
}
;
$('chat').onsubmit=async e=>{
  e.preventDefault();
  const text=$('input').value.trim();
  if(!text)return;
  await player.interrupt();
  $('input').value='';
  await invoke('chat',{
    text}
  );
}
;
$('interrupt').onclick=()=>player.interrupt().then(refresh).catch(e=>$('notice').textContent=e.message);
$('play').onclick=async()=>{
  if(!state.speech)return;
  if(player.current?.id===state.speech.id&&!player.current.closed&&state.providers.tts==='off'){
    await player.read();
    await refresh();
    return;
  }
  if(state.speech.delivery.stage!=='validated')return;
  await player.start(state.speech,state.effective_backends.tts,state.providers.voices[state.speech.actor]);
  await refresh();
}
;
$('save').onclick=async()=>{
  await player.interrupt();
  await invoke('save',{
    slot:Number($('slot').value)}
  );
}
;
$('load').onclick=async()=>{
  await player.interrupt();
  await invoke('load',{
    slot:Number($('slot').value)}
  );
  lastSpeech=0;
}
;
$('promise').onclick=()=>invoke('promise');
$('settings').onclick=async()=>{
  await player.interrupt();
  const voices=structuredClone(state.providers.voices);
  voices[state.npc]={
    voice_id:$('voice-id').value,pitch:Number($('pitch').value),rate:Number($('rate').value)}
  ;
  await invoke('settings',{
    speech_mode:$('speech-mode').value,text:$('text-backend').value,image:$('image-backend').value,tts:$('tts-backend').value,stt:$('stt-backend').value,size:$('size').value,cap_micros:Math.round(Number($('cap').value)*1e6),voices}
  );
}
;
$('asset-target').onchange=$('preview-candidate').onchange=drawAsset;
$('generate').onclick=()=>invoke('generate',{
  target:$('asset-target').value}
);
$('approve').onclick=()=>{
  const a=$('asset-target').value==='scene'?state.scene_asset:state.portraits;
  return invoke('approve',{
    key:a.selection.key}
  );
}
;
function patch(){
  return {
    replace_tokens:[],add_tokens:[],negative_tokens:[],forbid_props:[],seed_delta:1,asset:null}
  ;
}
$('reroll').onclick=()=>invoke('repair',{
  target:$('asset-target').value,key:($('asset-target').value==='scene'?state.scene_asset:state.portraits).selection.key,patch:patch(),image:null}
);
function file64(file){
  return new Promise((resolve,reject)=>{
    const r=new FileReader();
    r.onload=()=>resolve(r.result.split(',')[1]);
    r.onerror=reject;
    r.readAsDataURL(file);
  }
  );
}
$('repair').onclick=async()=>{
  const file=$('pin-file').files[0];
  await invoke('repair',{
    target:$('asset-target').value,key:($('asset-target').value==='scene'?state.scene_asset:state.portraits).selection.key,patch:JSON.parse($('patch').value),image:file?await file64(file):null}
  );
}
;
async function beginMic(){
  await player.interrupt();
  const result=await post('mic',{
  }
  );
  session=result.result.session;
  $('interim').textContent='Listening · speech interrupts playback';
  return session;
}
function recognize(text,final,uncertainty=null,capturedSession=session){
  if(!text.trim())return Promise.resolve();
  const id=capturedSession,stopped=player.interrupt();
  $('interim').textContent=(final?'Final: ':'Interim: ')+text;
  recognitionSerial=recognitionSerial.catch(()=>{
  }
  ).then(async()=>{
    await stopped;
    await invoke('recognition',{
      session:id,text,final,uncertainty}
    );
  }
  );
  return recognitionSerial;
}
async function browserMic(){
  const Recognition=globalThis.SpeechRecognition||globalThis.webkitSpeechRecognition;
  if(!Recognition)throw Error('Browser STT unavailable. Select a configured local HTTP backend or use text.');
  const capture=await beginMic();
  const r=new Recognition();
  mic=r;
  r.continuous=false;
  r.interimResults=true;
  r.lang='en-US';
  r.onspeechstart=()=>recognize('[speech detected]',false,'Transcript pending.',capture);
  r.onresult=async event=>{
    const results=[...event.results];
    const final=results.some(x=>x.isFinal);
    const text=results.map(x=>x[0].transcript).join(' ');
    await recognize(text,final,results[0]?.[0]?.confidence<.7?'Low recognition confidence.':null,capture);
  }
  ;
  r.onerror=()=>{
    $('interim').textContent='Speech recognition unavailable or microphone denied.';
  }
  ;
  r.onend=()=>{
    if(mic===r){mic=null;$('mic').textContent='🎙 Mic';}
  }
  ;
  r.start();
  $('mic').textContent='■ Stop Mic';
}
// Normalize browser codecs before upload; the server independently validates the
// actual PCM frame count so STT ceilings can be priced for at most ten seconds.
async function captureWav(blob){
  const decoder=new AudioContext();
  try {
    const decoded=await decoder.decodeAudioData(await blob.arrayBuffer());
    const frames=Math.min(160000,Math.ceil(decoded.duration*16000));
    if(frames<1)throw Error('No audio captured.');
    const offline=new OfflineAudioContext(1,frames,16000),source=offline.createBufferSource();
    source.buffer=decoded;source.connect(offline.destination);source.start();
    const rendered=await offline.startRendering(),samples=rendered.getChannelData(0);
    const bytes=new ArrayBuffer(44+frames*2),view=new DataView(bytes);
    const ascii=(at,s)=>{for(let i=0;i<s.length;i++)view.setUint8(at+i,s.charCodeAt(i));};
    ascii(0,'RIFF');view.setUint32(4,36+frames*2,true);ascii(8,'WAVEfmt ');view.setUint32(16,16,true);view.setUint16(20,1,true);view.setUint16(22,1,true);view.setUint32(24,16000,true);view.setUint32(28,32000,true);view.setUint16(32,2,true);view.setUint16(34,16,true);ascii(36,'data');view.setUint32(40,frames*2,true);
    for(let i=0;i<frames;i++){const sample=Math.max(-1,Math.min(1,samples[i]));view.setInt16(44+i*2,Math.round(sample*(sample<0?32768:32767)),true);}
    return new Blob([bytes],{type:'audio/wav'});
  }finally{await decoder.close();}
}
async function httpMic(){
  if(!navigator.mediaDevices?.getUserMedia)throw Error('Microphone capture is unavailable.');
  const capture=await beginMic();
  const stream=await navigator.mediaDevices.getUserMedia({
    audio:true}
  );
  const type=['audio/webm','audio/ogg','audio/mp4'].find(x=>MediaRecorder.isTypeSupported(x));
  if(!type){
    stream.getTracks().forEach(t=>t.stop());
    throw Error('This browser cannot record supported WebM/Ogg audio.');
  }
  const recorder=new MediaRecorder(stream,{
    mimeType:type}
  );
  mic=recorder;
  let capturing=true;
  const chunks=[];
  let total=0;
  const context=new AudioContext();
  const source=context.createMediaStreamSource(stream),analyser=context.createAnalyser();
  source.connect(analyser);
  analyser.fftSize=512;
  const data=new Float32Array(512);
  let detected=false;
  const vad=()=>{
    if(!capturing)return;
    analyser.getFloatTimeDomainData(data);
    const rms=Math.sqrt(data.reduce((n,x)=>n+x*x,0)/data.length);
    if(rms>.04&&!detected){
      detected=true;
      recognize('[speech detected]',false,'Transcript pending.',capture).catch(()=>{
      }
      );
    }
    requestAnimationFrame(vad);
  }
  ;
  vad();
  const timeout=setTimeout(()=>{
    if(recorder.state==='recording')recorder.stop();
  }
  ,10000);
  recorder.ondataavailable=e=>{
    total+=e.data.size;
    if(total>2*1024*1024){
      if(recorder.state==='recording')recorder.stop();
    }
    else chunks.push(e.data);
  }
  ;
  recorder.onstop=async()=>{
    capturing=false;
    clearTimeout(timeout);
    source.disconnect();
    await context.close();
    stream.getTracks().forEach(t=>t.stop());
    if(mic===recorder){mic=null;$('mic').textContent='🎙 Mic';}
    if(total>2*1024*1024){
      $('interim').textContent='Recording exceeded the 2 MiB limit.';
      return;
    }
    const blob=new Blob(chunks,{
      type}
    );
    await invoke('transcribe',{
      session:capture,audio:await file64(await captureWav(blob)),mime:'audio/wav'}
    );
  }
  ;
  recorder.start(250);
  $('mic').textContent='■ Send Recording';
}
$('mic').onclick=async()=>{
  try{
    if(mic){
      mic.stop();
      return;
    }
    if(state.effective_backends.stt==='off')throw Error('Choose an STT backend first.');
    if(state.effective_backends.stt==='browser_speech_recognition')await browserMic();
    else await httpMic();
  }
  catch(e){
    $('notice').textContent=e.message;
  }
}
;
await refresh();
setInterval(()=>refresh().catch(e=>$('notice').textContent=e.message),600);

$('resume-local').onclick=()=>invoke('resume_local',{});

$('stock').onclick=async()=>{await player.interrupt();await invoke('chat',{text:'What stock do you sell?'});};
