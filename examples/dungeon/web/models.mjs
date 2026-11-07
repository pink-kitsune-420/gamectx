// Local manager controls. Only validated profile IDs/configuration reach the host;
// executable paths and arguments stay in the host-owned launchers file.
export function attachModels({$,post,refresh,current,interrupt=async()=>{}}){
 const ids=['text','dramatic_text','image','dramatic_image','tts','stt'];
 const labels={text:'Everyday conversation',dramatic_text:'Dramatic conversation',image:'Everyday artwork',dramatic_image:'Dramatic artwork',tts:'Voice playback · TTS',stt:'Listening · STT'};
 let loaded=false,key='',draft=false;
 const ch=id=>id.includes('text')?'text':id.includes('image')?'image':id;
 const input=(tag,id,label,value,opts)=>{
  const holder=document.createElement('label');holder.textContent=label;const e=document.createElement(tag);e.id=id;
  if(opts)for(const [value,text]of opts){const o=document.createElement('option');o.value=value;o.textContent=text;e.append(o);}
  e.value=value??'';if(tag==='input'){e.maxLength=256;if(typeof value==='number'){e.type='number';e.step='1';}}holder.append(e);e.addEventListener('input',()=>{draft=true;$('manager-draft').textContent='Not applied';});return holder;
 };
 function build(c){
  const cards=[];
  for(const id of ids){
   const p=c.profiles[id],channel=ch(id),d=document.createElement('details');d.className='model-card';if(id==='text')d.open=true;
   const title=document.createElement('summary');title.textContent=labels[id];d.append(title);
   const status=document.createElement('p');status.id='mm-'+id+'-status';status.className='muted';d.append(status);
   const options=[['inherit','Use connection settings'],...(channel==='text'?[['local_ollama','Local · Ollama'],['online_openai_compatible','Online · configured API']]:channel==='image'?[['local_sd_cpp','Local · Stable Diffusion'],['online_image_api','Online · configured API']]:channel==='tts'?[['off','Text only'],['browser_web_speech','Browser voice'],['local_tts_http','Local voice server'],['online_tts_api','Online voice API']]:[['off','Type instead'],['browser_speech_recognition','Browser recognition'],['local_stt_http','Local transcription server'],['online_stt_api','Online transcription API']])];
   d.append(input('select','mm-'+id+'-backend','Provider',p.backend,options));
   const model=input('input','mm-'+id+'-model','Model ID',p.model);if(channel==='text')model.lastElementChild.setAttribute('list','manager-text-list');d.append(model);
   if(channel==='text')d.append(input('select','mm-'+id+'-size','Context tier',p.size,['1','1.5','3','8','30','70'].map(v=>[v,v+'B class'])));
   if(channel==='image'){
    d.append(input('input','mm-'+id+'-recipe','Image-server recipe',p.recipe));
    const selection=input('textarea','mm-'+id+'-selection','Weight file IDs (JSON)',JSON.stringify(p.selection));selection.lastElementChild.rows=2;d.append(selection);
    const help=document.createElement('small');help.textContent='Choose IDs listed by Scan. Native SD.cpp controls load these weights; no files are downloaded automatically.';d.append(help);
    d.append(input('input','mm-'+id+'-steps','Generation steps',p.steps));
   }
   d.append(input('input','mm-'+id+'-ram_mib','Estimated working memory (MiB)',p.ram_mib));
   const controls=document.createElement('div');controls.className='button-row';
   for(const [action,label]of [['start','Start / Connect'],['prewarm','Load now'],['unload','Unload weights'],['stop','Stop owned server']]){const b=document.createElement('button');b.id='mm-'+id+'-'+action;b.textContent=label;b.type='button';b.onclick=()=>act(action,id);controls.append(b);}
   d.append(controls);cards.push(d);
  }
  $('manager-profiles').replaceChildren(...cards.slice(0,4));$('voice-profiles').replaceChildren(...cards.slice(4));
 }
 function write(c){
  $('manager-enabled').checked=c.enabled;$('manager-mode').value=c.mode;$('manager-text-dramatic').checked=c.text_escalation;$('manager-image-dramatic').checked=c.image_escalation;$('manager-residency').value=c.residency;$('manager-idle').value=c.idle_seconds;$('manager-prewarm').checked=c.prewarm;$('manager-ram').value=c.ram_limit_mib;$('manager-reserve').value=c.reserve_mib;
  for(const [k,v]of Object.entries(c.rules)){const e=$('mr-'+k);if(typeof v==='boolean')e.checked=v;else e.value=v;}
  build(c);draft=false;$('manager-draft').textContent='Applied';key=JSON.stringify(c);
 }
 function read(){
  const c=structuredClone(current().model_manager.config);
  Object.assign(c,{enabled:$('manager-enabled').checked,mode:$('manager-mode').value,text_escalation:$('manager-text-dramatic').checked,image_escalation:$('manager-image-dramatic').checked,residency:$('manager-residency').value,idle_seconds:Number($('manager-idle').value),prewarm:$('manager-prewarm').checked,ram_limit_mib:Number($('manager-ram').value),reserve_mib:Number($('manager-reserve').value)});
  for(const k of Object.keys(c.rules)){const e=$('mr-'+k);c.rules[k]=e.type==='checkbox'?e.checked:Number(e.value);}
  for(const id of ids){const p=c.profiles[id];for(const k of Object.keys(p)){const e=$('mm-'+id+'-'+k);p[k]=k==='selection'?JSON.parse(e.value):['ram_mib','steps'].includes(k)?Number(e.value):e.value.trim();}}
  for(const id of ids){const p=c.profiles[id];if(!p.model||!/^[-A-Za-z0-9_:./]+$/.test(p.model))throw Error(labels[id]+': use the exact model ID without spaces.');if(!Number.isInteger(p.ram_mib)||p.ram_mib<64||p.ram_mib>65536)throw Error(labels[id]+': RAM must be 64–65536 whole MiB.');}
  return c;
 }
 async function act(action,profile){try{if(draft&&['start','prewarm'].includes(action))throw Error('Apply profile changes before loading models.');await post('manager_action',{action,profile});await refresh();}catch(e){$('manager-message').textContent=e.message;}}
 $('manager-scan').onclick=()=>act('refresh','text');
 $('manager-apply').onclick=async()=>{
  try{const config=read();await interrupt();const result=await post('manager_settings',{config});await refresh(result.generation||0);if(result.result?.configured){write(current().model_manager.config);$('manager-message').textContent='Applied. Models load when requested; larger missing profiles fall back.';}}
  catch(e){$('manager-message').textContent=e.message;}
 };
 $('voice-native-preset').onclick=()=>{
  for(const [id,backend,model,ram]of [['tts','local_tts_http','piper',300],['stt','local_stt_http','whisper-tiny',300]]){
   $('mm-'+id+'-backend').value=backend;$('mm-'+id+'-model').value=model;$('mm-'+id+'-ram_mib').value=ram;
  }
  $('manager-enabled').checked=true;draft=true;$('manager-draft').textContent='Not applied';$('voice-message').textContent='Piper and Whisper selected. Apply, then check the installation. Requires the host native-voice.json configuration.';
 };
 $('voice-apply').onclick=()=>$('manager-apply').click();
 $('manager-art-apply').onclick=async()=>{try{await post('manager_art',{bible:JSON.parse($('manager-art').value)});await refresh();$('manager-message').textContent='Art bible validated. Changed art direction receives a new asset key.';}catch(e){$('manager-message').textContent=e.message;}};
 for(const e of document.querySelectorAll('#manager-settings input,#manager-settings select'))e.addEventListener('input',()=>{draft=true;$('manager-draft').textContent='Not applied';});
 return s=>{
  const m=s.model_manager;if(!m)return; $('voice-message').textContent=$('manager-message').textContent||$('voice-message').textContent;
  if(!loaded){loaded=true;write(m.config);$('manager-art').value=JSON.stringify(m.dramatic_art,null,2);}
  else if(!draft&&key!==JSON.stringify(m.config))write(m.config);
  const r=m.runtime||{},route=m.route||{},eligible=r.eligible||r.available||[];
  $('manager-state').textContent=!m.config.enabled?'Off · ordinary provider settings are in use':`${route.dramatic?'Dramatic scene':'Everyday scene'} · ${route.reasons?.join(', ')||'normal rules'} · text: ${route.text||'fallback'} · art: ${route.image||'cached'}`;
  $('manager-memory').textContent=`Free RAM: ${r.free_mib??'unknown'} MiB · manager active: ${r.loaded?.profile||'none'} · image server: ${r.observed_image||'not checked'}${r.error?' · '+r.error:''}`;
  $('manager-reason').textContent=(route.fallbacks||[]).map(v=>v.replace(':unavailable',': model, service or online allowance unavailable').replace(':ram_limit',': exceeds RAM allowance')).join('; ');
  $('manager-loaded').textContent=JSON.stringify({loaded:r.loaded,observed_text:r.observed_text,observed_image:r.observed_image,services:r.services},null,2);
  const names=r.text_models||[];$('manager-text-list').replaceChildren(...names.map(v=>{const o=document.createElement('option');o.value=v;return o;}));
  $('manager-files').textContent=(r.image_files||[]).join('\n')||'No image catalog yet. Scan or start the local image server.';
  for(const id of ids){
   const p=m.config.profiles[id],channel=ch(id),local=p.backend.startsWith('local_')||p.backend==='inherit'&&s.effective_backends[channel]?.startsWith('local_'),service=(r.services||[]).find(v=>v.channel===channel);
   $('mm-'+id+'-status').textContent=service?.native?'Native CLI · '+(eligible.includes(id)?'ready on demand · unloaded between requests':'stopped or installation missing'):p.backend.startsWith('online_')?(eligible.includes(id)?'API configured · request allowed under cap':'API credentials, price ceiling or spending allowance unavailable'):(eligible.includes(id)?'Available for requests':'Model files or service unavailable · scan after installing/configuring');
   for(const action of ['start','prewarm','unload','stop']){$('mm-'+id+'-'+action).disabled=!local||(s.pending_jobs>0&&!(service?.native&&action==='stop'))||draft&&['start','prewarm'].includes(action)||action==='stop'&&service?.ownership!=='managed'&&!service?.native;}
  }
 };
}
