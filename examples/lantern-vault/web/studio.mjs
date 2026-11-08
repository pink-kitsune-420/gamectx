import {AutoSpeech} from './autospeech.mjs';
import {attachProfiler} from './profiler.mjs';
import {attachModels} from './models.mjs';
import {
  VoicePlayer,prefix}
from './voice.mjs';
import {makePost,coalescedRefresh,scopedOperations,replyWaiting,AudioOwner} from './context.mjs';
import {selectedModel,channelLabel,channelNames,choiceLabel,draftKey,browserAvailability} from './setup.mjs';
const $=id=>document.getElementById(id);
const autoSpeech=new AutoSpeech();
const audioOwner=new AudioOwner();
addEventListener('pagehide',()=>audioOwner.release());
try{$('auto-speak').checked=localStorage.getItem('lantern-auto-speak')==='true';}catch{}
$('auto-speak').onchange=()=>{try{localStorage.setItem('lantern-auto-speak',String($('auto-speak').checked));}catch{}refresh();};
$('roleplay-apply').onclick=async()=>{$('roleplay-apply').disabled=true;$('roleplay-status').textContent='Registering…';try{await invoke('roleplay_settings',JSON.parse($('roleplay-registry').value));$('roleplay-status').textContent='Registered';}catch(e){$('roleplay-status').textContent=e.message;}finally{$('roleplay-apply').disabled=false;}};
$('conversation-apply').onclick=async()=>{$('conversation-apply').disabled=true;$('conversation-state').textContent='Applying…';try{await invoke('conversation_settings',{version:1,max_sentences:Number($('reply-sentences').value),max_player_chars:Number($('player-message-limit').value),summarize_long:$('player-summary').checked,ai_wording:$('ai-wording').checked});$('conversation-state').textContent='Applied';}catch(e){$('conversation-state').textContent=e.message;}finally{$('conversation-apply').disabled=false;}};
function inputCount(){const cap=state?.conversation_options?.max_player_chars||2048;$('input-count').textContent=`${Array.from($('input').value).length} / ${cap} characters · Enter to send, Shift+Enter for a new line`;}
$('input').oninput=inputCount;
$('input').onkeydown=e=>{if(e.key==='Enter'&&!e.shiftKey&&!e.isComposing){e.preventDefault();$('chat').requestSubmit();}};
$('reasoning-apply').onclick=async()=>{$('reasoning-apply').disabled=true;try{await invoke('deliberation_settings',{version:1,mode:$('reasoning-mode').value,effort:$('reasoning-effort').value,max_tokens:Number($('reasoning-tokens').value),hypotheses:$('reasoning-hypotheses').checked,online:JSON.parse($('reasoning-online').value)});}catch(e){$('reasoning-status').textContent=e.message;}finally{$('reasoning-apply').disabled=false;}};

let lastReactions='';
let reviewUrl=null,lastArchive='',chatStarting=0,requestedReplyGeneration=0;
let recognitionSerial=Promise.resolve(),micStarting=false;
let state,configured=false,lastTranscript='',lastSpeech=0,mic=null,session=0,appliedDraft='',clientNotice='';
const send=makePost(globalThis.fetch.bind(globalThis),()=>state);
const post=async(op,args={},generation=state?.generation)=>{
  if(scopedOperations.has(op)&&op!=='select'&&state&&($('player').value!==state.player||$('npc').value!==state.npc))throw Error('Participant selection is still applying; try again.');
  if(['chat','approach','act','select','mic','reset','load','settings'].includes(op))stopReview();
  if(op==='chat'){chatStarting++;drawPending();try{const reply=await send(op,args,generation);requestedReplyGeneration=Math.max(requestedReplyGeneration,reply.generation||0);return reply;}finally{chatStarting--;drawPending();}}
  return send(op,args,generation);
};
function drawPending(){const waiting=replyWaiting(state,chatStarting,requestedReplyGeneration);$('reply-thinking').hidden=!waiting;$('current-reply').hidden=waiting||!state?.speech;$('use-instant-reply').hidden=chatStarting>0||!state?.speech?.pending;}
$('use-instant-reply').onclick=()=>invoke('use_fallback').catch(()=>{});
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
  if(Number.isFinite(v.audio_time)&&Number.isFinite(v.audio_duration)&&v.audio_duration>0){
    $('voice-progress').max=v.audio_duration;$('voice-progress').value=Math.min(v.audio_time,v.audio_duration);
    $('voice-status').textContent=`Playing buffered speech · ${v.audio_time.toFixed(1)} / ${v.audio_duration.toFixed(1)}s`;
  }
}
const drawProfiler=attachProfiler({$,post,refresh:()=>refresh()});
const player=new VoicePlayer({
  post,emit:voiceView}
);
async function invoke(op,args={},generation=state?.generation) {
  try {
    const result=await post(op,args,generation);
    clientNotice='';
    await refresh(result.generation||0);
    return result;
  }
  catch(e){
    if(op==='chat')requestedReplyGeneration=0;
    await refresh(e.generation||0);
    clientNotice=e.message;
    $('notice').textContent=e.message;
    throw e;
  }
}
function stopReview(){const a=$('review-audio');a.pause();a.removeAttribute('src');a.load();a.hidden=true;if(reviewUrl){URL.revokeObjectURL(reviewUrl);reviewUrl=null;}}
async function replayAudio(reference,voice){await player.interrupt();stopReview();const a=$('review-audio');
 if(reference.key)a.src='/api/voice/'+reference.key;else{const bytes=Uint8Array.from(atob(reference.audio),x=>x.charCodeAt(0));reviewUrl=URL.createObjectURL(new Blob([bytes],{type:'audio/wav'}));a.src=reviewUrl;}
 a.preservesPitch=false;a.playbackRate=Number(voice?.pitch||1);a.hidden=false;await a.play();
}
function drawHistory(s){const h=s.voice_history;if(!h)return;
 if(!$('archive-enabled').dataset.initialized){$('archive-enabled').checked=h.config.enabled;$('archive-limit').value=h.config.limit;$('archive-enabled').dataset.initialized='1';}
 const key=JSON.stringify(h);if(key!==lastArchive){lastArchive=key;$('voice-history-count').textContent=h.records.length?String(h.records.length):'';$('voice-history-warning').textContent=h.warning||(!h.config.enabled?'Archiving is off. Enable it in Settings → Voice.':'');
 $('voice-history-list').replaceChildren(...[...h.records].reverse().map(row=>{const d=document.createElement('div'),b=document.createElement('button'),t=document.createElement('span');d.className='recording-row';b.textContent='Replay';b.onclick=()=>replayAudio(row,row.voice).catch(e=>$('notice').textContent=e.message);t.textContent=`${row.speaker} → ${row.player}: ${row.text}`;d.append(b,t);return d;}));}
 $('replay-current').hidden=!s.speech?.audio?.audio;
}
$('replay-current').onclick=()=>{if(state?.speech?.audio?.audio)replayAudio(state.speech.audio,state.speech.voice).catch(e=>$('notice').textContent=e.message);};
$('archive-apply').onclick=async()=>{try{await invoke('audio_settings',{enabled:$('archive-enabled').checked,limit:Number($('archive-limit').value)});$('archive-state').textContent=state.voice_history.config.enabled?'Archive enabled.':'Archive off; saved recordings cleared.';if(!state.voice_history.config.enabled)stopReview();}catch(e){$('archive-state').textContent=e.message;}};
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
  $('model').value=selectedModel(s.providers);
  voiceSettings();
  appliedDraft=draftKey(draftSettings());
  renderSetup(s);
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
function initTheme(){
  const toggle=$('theme-toggle');
  if(!toggle)return;
  let current='dark';
  try{current=localStorage.getItem('lantern-theme')||'dark';}catch{}
  document.documentElement.dataset.theme=current;
  toggle.textContent=current==='dark'?'☀️ Light':'🌙 Dark';
  toggle.onclick=()=>{
    const next=document.documentElement.dataset.theme==='dark'?'light':'dark';
    document.documentElement.dataset.theme=next;
    try{localStorage.setItem('lantern-theme',next);}catch{}
    toggle.textContent=next==='dark'?'☀️ Light':'🌙 Dark';
  };
}
initTheme();

function drawInventory(s){
  const container=$('inventory-items');
  if(!container)return;
  const carried=(s.game?.items||[]).filter(i=>i.owner==='player');
  if(!carried.length){
    container.innerHTML='<span class="muted empty-note">No items carried yet.</span>';
    return;
  }
  container.replaceChildren(...carried.map(item=>{
    const row=document.createElement('div');
    row.className='inventory-item-row';
    const info=document.createElement('div');
    const name=document.createElement('strong');
    name.textContent=item.label||item.id.replace('item:','').replaceAll('_',' ');
    const tag=document.createElement('span');
    tag.className='item-tag';
    tag.textContent=item.kind||'item';
    info.append(name,document.createTextNode(' '),tag);
    const btn=document.createElement('button');
    btn.type='button';
    btn.textContent='Inspect';
    btn.onclick=async()=>{
      try{
        await player.interrupt();
        await invoke('act',{revision:s.game.revision,decision:{action:'inspect',target:item.id}},s.generation);
      }catch(e){$('notice').textContent=e.message;}
    };
    row.append(info,btn);
    return row;
  }));
}

function drawCharactersPresent(s){
  const container=$('characters-list');
  if(!container)return;
  const list=[];
  const room=s.game?.room||'';
  if(room==='room:workshop'){
    const div=document.createElement('div');
    div.className='character-item';
    div.innerHTML='<div><strong>Ilen the Merchant</strong><br><span>Trading supplies at the counter</span></div>';
    const btn=document.createElement('button');
    btn.type='button';
    btn.textContent='Speak';
    btn.onclick=async()=>{
      try{
        await player.interrupt();
        let gen=s.generation;
        if(s.npc!=='merchant'){
          const rec=await invoke('select',{player:s.player,npc:'merchant'},gen);
          gen=rec.generation;
        }
        await invoke('approach',{npc:'merchant'},gen);
        $('input').focus();
      }catch{}
    };
    div.append(btn);
    list.push(div);
  } else if(room==='room:cistern'){
    const div=document.createElement('div');
    div.className='character-item';
    div.innerHTML='<div><strong>Keeper of the Seals</strong><br><span>Guarding the cistern inscription</span></div>';
    const btn=document.createElement('button');
    btn.type='button';
    btn.textContent='Speak';
    btn.onclick=async()=>{
      try{
        await player.interrupt();
        let gen=s.generation;
        if(s.npc!=='keeper'){
          const rec=await invoke('select',{player:s.player,npc:'keeper'},gen);
          gen=rec.generation;
        }
        await invoke('approach',{npc:'keeper'},gen);
        $('input').focus();
      }catch{}
    };
    div.append(btn);
    list.push(div);
  }
  if(s.game?.animal_following||(room==='room:cistern'&&s.game?.animal_friendly)){
    const div=document.createElement('div');
    div.className='character-item';
    div.innerHTML=`<div><strong>Cave Hound</strong><br><span>${s.game?.animal_following?'Faithful companion walking at your side':'Friendly creature resting by the fountain'}</span></div>`;
    list.push(div);
  }
  if(!list.length){
    container.innerHTML='<span class="muted" style="font-size:12px;padding:4px 0;">No characters currently in this chamber.</span>';
  } else {
    container.replaceChildren(...list);
  }
}

let lastJournal='';
function drawJournal(s){
  const container=$('journal-entries');
  if(!container)return;
  const entries=s.journal||[];
  const key=JSON.stringify(entries);
  if(key===lastJournal)return;
  lastJournal=key;
  if(!entries.length){
    container.innerHTML='<span class="muted" style="font-size:12px;">No chronicle entries recorded yet.</span>';
    return;
  }
  container.replaceChildren(...entries.map(entry=>{
    const card=document.createElement('div');
    card.className='journal-entry';
    const header=document.createElement('div');
    header.className='journal-header';
    const badge=document.createElement('span');
    badge.className=`journal-badge ${entry.kind||'history'}`;
    badge.textContent=entry.epoch?`Year ${entry.epoch}`:(entry.kind||'chronicle');
    const title=document.createElement('strong');
    title.className='journal-title';
    title.textContent=entry.title||entry.id;
    header.append(badge,title);
    const summary=document.createElement('p');
    summary.className='journal-summary';
    summary.textContent=entry.summary||'';
    card.append(header,summary);
    return card;
  }));
}

async function loadState(){
    const response=await fetch('/api/state');
    if(!response.ok)throw Error('Host state unavailable.');
    const s=await response.json();
    state=s;
    if(s.generation>=requestedReplyGeneration)requestedReplyGeneration=0;
    if(document.hidden&&(!player.current||player.current.closed))audioOwner.release();
    if(s.roleplay_registry&&!$('roleplay-registry').dataset.loaded){$('roleplay-registry').value=JSON.stringify(s.roleplay_registry,null,2);$('roleplay-registry').dataset.loaded='1';}
    if(s.conversation_options){const c=s.conversation_options;$('input').maxLength=c.max_player_chars;if(!$('reply-sentences').dataset.loaded){$('ai-wording').checked=Boolean(c.ai_wording);$('reply-sentences').value=c.max_sentences;$('player-message-limit').value=c.max_player_chars;$('player-summary').checked=c.summarize_long;$('reply-sentences').dataset.loaded='1';}inputCount();}

    if(s.deliberation){const d=s.deliberation;if(!$('reasoning-mode').dataset.loaded){$('reasoning-mode').value=d.config.mode;$('reasoning-effort').value=d.config.effort;$('reasoning-tokens').value=d.config.max_tokens;$('reasoning-hypotheses').checked=d.config.hypotheses;$('reasoning-online').value=JSON.stringify(d.config.online,null,2);$('reasoning-mode').dataset.loaded='1';}$('reasoning-status').textContent=`Policy: ${d.config.mode}. `+(d.latest?.plan?.active?'Native reasoning selected for the last difficult turn. ':'')+(d.capabilities||[]).map(c=>c.model+': '+(c.values?.some(v=>v!==false)?'advertised controls '+c.values.join(', '):'no advertised thinking control; provider default')).join('\n');$('reasoning-debug').textContent=JSON.stringify(d.latest,null,2);}

    drawHistory(s);drawProfiler(s);
    if(s.greetings){if(!$('greeting-gap').dataset.loaded){$('greeting-gap').value=s.greetings.config.long_gap_turns;$('greeting-gap').dataset.loaded='1';}$('greeting-debug').textContent=JSON.stringify(s.greetings,null,2);}
    if(s.reactions){
      if(!$('reaction-rules').dataset.loaded){$('reaction-rules').value=JSON.stringify(s.reactions.config.characters,null,2);$('reactions-enabled').checked=s.reactions.config.enabled;$('reaction-rules').dataset.loaded='1';}
      const r=s.reactions,key=JSON.stringify([r,s.generation,s.npc,s.player]);if(key!==lastReactions){lastReactions=key;$('reaction-debug').textContent=JSON.stringify({banks:r.banks,records:r.records},null,2);
        const open=r.records.filter(d=>d.op==='grant'&&d.handler==='loan'&&d.actor===s.npc&&d.listener===s.player&&!r.records.some(p=>p.op==='repay'&&p.actor===d.actor&&p.listener===d.listener&&p.id===d.id));
        const label=x=>x?x.kind==='coins'?`${x.amount} ${x.amount===1?'coin':'coins'}`:x.id.replace('item:','').replaceAll('_',' '):'support';
        $('reaction-options').hidden=!r.config.enabled||(!r.offers.length&&!open.length);$('loan-status').textContent=open.length?'Open loans: '+open.map(d=>label(d.resource)).join(', '):'';
        const buttons=r.offers.map(o=>{const b=document.createElement('button');b.textContent=o.handler==='support'?'Ask for reassurance':`Ask for ${o.handler==='loan'?'a loan':'a gift'} · ${label(o.resource)}`;b.onclick=async()=>{try{await player.interrupt();await invoke('reaction',{id:o.id,revision:o.revision},s.generation);}catch{}};return b;});
        if(open.length){const b=document.createElement('button');b.textContent='Repay a loan';b.onclick=async()=>{try{await player.interrupt();await invoke('chat',{text:'I would like to repay you.'},s.generation);}catch{}};buttons.push(b);}$('reaction-buttons').replaceChildren(...buttons);
      }
    }

    if(s.personality){$('viewpoints').textContent=JSON.stringify(s.personality.viewpoints.entries,null,2);if(!$('mind-profiles').dataset.loaded){$('mind-profiles').value=JSON.stringify(s.personality.config.profiles,null,2);$('mind-enabled').checked=s.personality.config.enabled;$('mind-profiles').dataset.loaded='1';}}
    $('resume-local').hidden=!s.local_paused;
    $('room-title').textContent=s.game.room.replace('room:','').replace(/^./,x=>x.toUpperCase());
    $('room-label').textContent=s.game.room.replace('room:','');
    $('npc-label').textContent=s.npc+(s.npc_present?' · present':' · not in this room');
    $('description').textContent=s.game.description;
    $('stats').textContent=`Party health ${s.game.hp}/6 · Shared coins ${s.game.coins}`;
    $('player').value=s.player;
    $('npc').value=s.npc;
    $('presence').textContent=s.npc_present?'In speaking range':'NPC is elsewhere';
    const actionButton=choice=>{
      const b=document.createElement('button');
      b.textContent=choiceLabel(choice,s.game.items);
      b.onclick=async()=>{try{
        await player.interrupt();
        if(choice.action==='talk'){
          let generation=s.generation;
          const npc=choice.target.split(':').at(-1);
          if(npc!==s.npc){const receipt=await invoke('select',{player:s.player,npc},generation);generation=receipt.generation;}
          await invoke('approach',{npc},generation);$('input').focus();
        } else await invoke('act',{revision:s.game.revision,decision:choice},s.generation);
      }catch{}};
      return b;
    };
    $('quick-actions').replaceChildren(...s.game.choices.filter(c=>['go','talk'].includes(c.action)).map(actionButton));
    $('actions').replaceChildren(...s.game.choices.map(actionButton));
    drawInventory(s);
    drawCharactersPresent(s);
    drawJournal(s);
    const transcript=JSON.stringify(s.transcript);
    if(transcript!==lastTranscript){
      lastTranscript=transcript;
      $('transcript').replaceChildren(...s.transcript.map(row=>{
        const p=document.createElement('div');
        p.className='line'+(row.speaker==='host'?' host':'')+(row.kind==='action'?' stage-direction':'');
        const name=document.createElement('strong');
        name.textContent=row.speaker+' · '+row.kind;
        p.append(name,document.createTextNode(row.kind==='action'?row.text.replace(/^\*(.*)\*$/,'$1'):row.text));
        return p;
      }
      ));
      $('transcript').scrollTop=$('transcript').scrollHeight;
    }
    if(s.speech){
      $('play').disabled=Boolean(sessionInfo.enabled&&s.speech.player==='player_2');
      $('interrupt').disabled=Boolean(sessionInfo.enabled&&s.speech.player==='player_2');
      $('reply-origin').textContent=({offline:'Offline reply · no LLM',pending:'Offline reply · model working',authorized_model:'Model selected approved wording',model:'Model reply · validated',subjective_model:'Personal reply · validated',authored_social:'Character reply · no LLM',authored_fiction:'Invented tale · no LLM',fiction_model:'Invented tale · model validated',wording_model:'AI character wording · source checked',cached_model:'Cached AI wording · matching situation',scope_model:'AI reply · shorter scope offered'})[s.speech.origin]||'Offline reply';
      $('speaker').textContent=s.speech.actor+' → '+s.speech.player+(s.speech.segments>1?` · ${s.speech.segment}/${s.speech.segments}`:'');
      if(s.speech.id!==lastSpeech){
        lastSpeech=s.speech.id;
        voiceView({
          text:s.speech.text,through:s.speech.delivery.heard,status:s.speech.playback_blocked?'Audio blocked · press Play reply':s.speech.pending?'Model pending · fallback available':'Ready'}
        );
      }
      if(!player.current||player.current.id!==s.speech.id||player.current.closed) {
        if(s.speech.delivery.stage==='validated')voiceView({
          text:s.speech.text,through:0,status:s.speech.playback_blocked?'Audio blocked · press Play reply':s.speech.pending?'Model pending · fallback available':'Ready'}
        );
      }
      await player.chunk(s.speech);
      const voiceAvailable=$('auto-speak').checked&&!mic&&!micStarting&&!chatStarting&&!s.speech.pending&&!s.speech.delivery.done&&s.speech.delivery.stage==='validated'?await audioOwner.acquire():audioOwner.owned;
      if(autoSpeech.take(s,{enabled:$('auto-speak').checked,host:voiceAvailable&&(!sessionInfo.enabled||s.speech.player!=='player_2'),busy:Boolean(mic||micStarting||chatStarting)})){
        try{await player.start(s.speech,s.effective_backends.tts,s.providers.voices[s.speech.actor]);}catch(e){clientNotice='Automatic playback could not start; use Play reply. '+e.message;}
      }
    }
    else { $('speech-text').textContent='Speak to a nearby character to start a conversation.'; $('reply-origin').textContent='No reply yet'; $('voice-status').textContent='Ready'; $('voice-progress').value=0; }
    drawPending();
    $('current-reply').classList.toggle('text-only',s.effective_backends.tts==='off');
    $('play').textContent=s.effective_backends.tts==='off'?'Read reply':'Play reply';
    $('play').disabled=!s.speech||s.speech.delivery.done||(s.effective_backends.tts!=='off'&&s.speech.delivery.stage!=='validated')||Boolean(sessionInfo.enabled&&s.speech.player==='player_2');
    $('stock').disabled=$('promise').disabled=!s.npc_present;
    const micUnsupported=s.effective_backends.stt==='browser_speech_recognition'&&!globalThis.SpeechRecognition&&!globalThis.webkitSpeechRecognition;
    $('mic').disabled=micStarting||s.effective_backends.stt==='off'||micUnsupported;
    $('mic').title=micUnsupported?'This browser does not support speech recognition. Type instead or use a local server.':s.effective_backends.stt==='off'?'Choose a microphone provider in Settings → Voice.':'';
    $('interrupt').disabled=!s.speech||s.speech.delivery.done;
    image('scene-image',s.scene_asset.selection.asset||s.scene_asset.preview_asset);
    if(!s.scene_asset.selection.asset&&s.scene_asset.preview_asset)$('room-label').textContent+=' · previous approved art';
    image('portrait-image',s.portraits.visible?s.portraits.selection.asset||s.portraits.preview_asset:null);
    if(s.portraits.visible&&!s.portraits.selection.asset&&s.portraits.preview_asset)$('npc-label').textContent+=' · previous approved art';
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
      promises:s.promises,player_claims:s.roleplay_memory,episodes:s.characters.map(c=>({
        id:c.id,episodes:c.episodes}
      ))}
    ,null,2);
    $('living').textContent=JSON.stringify({
      selected:s.living,rumors:s.rumors}
    ,null,2);
    $('receipts').textContent=JSON.stringify({
      ...s.spending,ledger:s.spending_receipts,pending_jobs:s.pending_jobs}
    ,null,2);
    const money=n=>n===0?'$0':'$'+(n/1e6).toFixed(6);
    $('spend').textContent=`${money(s.spending.charged_or_reserved_micros)} / ${money(s.spending.cap_micros)} cap · ${s.pending_jobs} queued`;
    $('notice').textContent=clientNotice||s.notice;
    if(!mic&&['final_text','no_speech','failed'].includes(s.recognition?.status)){
      $('mic-monitor').hidden=false;$('mic-feedback').textContent=s.recognition.status==='final_text'?'Transcript received: '+s.recognition.text:(s.recognition.uncertainty||s.recognition.status);
    }
    if(s.recognition&&s.recognition.text!=='[speech detected]'){
      $('interim').textContent=`${s.recognition.status}: ${s.recognition.text}${s.recognition.uncertainty?' · '+s.recognition.uncertainty:''}`;
    }
    if(!configured){
      configured=true;
      settings(s);
    }
    renderSetup(s);
    drawModels(s);
}
const refresh=coalescedRefresh(loadState,()=>state);
const drawModels=attachModels({$,post,refresh,current:()=>state,interrupt:()=>player.interrupt()});
$('player').onchange=$('npc').onchange=async()=>{
  const generation=state?.generation,selectedPlayer=$('player').value,selectedNpc=$('npc').value;
  await player.interrupt();
  await invoke('select',{player:selectedPlayer,npc:selectedNpc},generation);
  voiceSettings();
}
;
$('chat').onsubmit=async e=>{
  e.preventDefault();
  const text=$('input').value.trim();
  if(!text)return;
  const generation=state?.generation;
  chatStarting++;drawPending();
  try{await player.interrupt();$('input').value='';await invoke('chat',{text},generation);}
  catch(e){if(!$('input').value)$('input').value=text;}
  finally{chatStarting--;drawPending();}
}
;
$('interrupt').onclick=()=>player.interrupt().then(refresh).catch(e=>$('notice').textContent=e.message);
$('play').onclick=async()=>{
  try{
    if(!state.speech)return;
    if(!await audioOwner.acquire())throw Error('Live voice is playing in another tab; it will continue there.');
    if(state.effective_backends.tts==='off'){
      if(!player.current||player.current.id!==state.speech.id||player.current.closed){
        if(state.speech.delivery.stage!=='validated')return;
        await player.start(state.speech,'off',state.providers.voices[state.speech.actor]);
      }
      await player.read();await refresh();return;
    }
    if(state.speech.delivery.stage!=='validated')return;
    await player.start(state.speech,state.effective_backends.tts,state.providers.voices[state.speech.actor]);
    await refresh();
  }catch(e){clientNotice=e.message;$('notice').textContent=e.message;}
};
$('save').onclick=async()=>{
  const generation=state?.generation;
  await player.interrupt();
  await invoke('save',{slot:Number($('slot').value)},generation);
}
;
$('load').onclick=async()=>{
  const generation=state?.generation;
  await player.interrupt();
  await invoke('load',{slot:Number($('slot').value)},generation);
  lastSpeech=0;
}
;
$('promise').onclick=()=>invoke('promise');
function draftSettings(){
  const voices=structuredClone(state.providers.voices);
  voices[state.npc]={voice_id:$('voice-id').value,pitch:Number($('pitch').value),rate:Number($('rate').value)};
  const draft={speech_mode:$('speech-mode').value,text:$('text-backend').value,image:$('image-backend').value,tts:$('tts-backend').value,stt:$('stt-backend').value,size:$('size').value,cap_micros:Math.round(Number($('cap').value)*1e6),voices};
  if(['local_ollama','online_openai_compatible'].includes(draft.text))draft.model=$('model').value.trim();
  return draft;
}
function renderSetup(s){
  const dirty=appliedDraft&&draftKey(draftSettings())!==appliedDraft;
  $('settings-state').textContent=dirty?'Not applied · current settings still in use':'Applied · affects your next message';
  $('settings-state').className=dirty?'dirty':'';
  $('settings').textContent=dirty?'Apply changes':'Apply settings';
  $('model').disabled=!['local_ollama','online_openai_compatible'].includes($('text-backend').value);
  const rows=(s.setup?.channels||[]).map(row=>browserAvailability(row,globalThis));
  $('channel-status').replaceChildren(...rows.map(row=>{
    const badge=document.createElement('span');badge.className='status-chip '+(['ready','offline'].includes(row.status)?'ready':['unchecked','checking','online'].includes(row.status)?'':'warning');badge.textContent=channelNames[row.channel]+': '+channelLabel(row);badge.title=row.detail;return badge;
  }));
  const warnings=rows.filter(r=>['missing_model','unavailable','unconfigured','paused','browser_unavailable'].includes(r.status));
  document.querySelector('.setup-banner').classList.toggle('needs-attention',Boolean(dirty||warnings.length));
  $('setup-title').textContent=dirty?'You have unapplied model settings.':warnings.length?'Some selected services need attention.':s.pending_jobs?'Working in the background · you can keep playing.':'Play now, add models whenever you like.';
  $('setup-help').textContent=dirty?'Click Apply changes to use these settings for your next message.':warnings.length?warnings.map(r=>channelNames[r.channel]+': '+r.detail).join(' '):'Exploration and trade always work offline. Model setup can happen before or after meeting a character.';
  const text=rows.find(r=>r.channel==='text');
  $('model-detail').textContent=text?`Applied: ${text.model||channelLabel(text)}. ${text.detail}`:'';
  $('audio-checks').textContent=rows.filter(r=>r.channel!=='text').map(r=>channelNames[r.channel]+': '+channelLabel(r)+'. '+r.detail).join(' ');
  const models=s.setup?.installed_models||[];const list=$('installed-models');
  if(JSON.stringify(models)!==list.dataset.models){
    list.dataset.models=JSON.stringify(models);list.replaceChildren(...models.map(name=>{const option=document.createElement('option');option.value=name;return option;}));
    const picker=$('installed-choice');const placeholder=document.createElement('option');placeholder.value='';placeholder.textContent='Choose an installed model…';picker.replaceChildren(placeholder,...models.map(name=>{const option=document.createElement('option');option.value=name;option.textContent=name;return option;}));
  }
  $('installed-label').hidden=!models.length;
  $('check-providers').disabled=s.pending_jobs>0;
  const activity=s.setup?.activity||[];
  const current=activity.find(r=>r.status==='running')||activity.find(r=>r.status==='queued')||activity[0];
  $('work-status').hidden=!current;
  $('work-status').textContent=current?`${channelNames[current.channel]||current.channel}: ${current.status} · ${current.model||current.backend} · ${(current.run_ms/1000).toFixed(1)}s running · ${(current.queue_ms/1000).toFixed(1)}s queued`:'No model requests yet. You can explore, trade and get offline replies.';
  if(!activity.length)$('activity').textContent='No model requests yet. Movement, purchases and offline replies use game rules.';
  else $('activity').replaceChildren(...activity.slice(0,10).map(row=>{
    const div=document.createElement('div');div.className='activity-row';const heading=document.createElement('strong');heading.textContent=`${channelNames[row.channel]||row.channel} · ${row.status} · ${(row.run_ms/1000).toFixed(1)}s running · ${(row.queue_ms/1000).toFixed(1)}s queued`;const small=document.createElement('small');small.textContent=[row.backend,row.model].filter(Boolean).join(' · ');div.append(heading,document.createTextNode(row.detail),small);return div;
  }));
}
$('settings').onclick=async()=>{
  try{const generation=state.generation,args=draftSettings();await player.interrupt();await invoke('settings',args,generation);if(state.providers.text!=='0b_fallback')await invoke('conversation_settings',{...state.conversation_options,ai_wording:true});$('ai-wording').checked=Boolean(state.conversation_options.ai_wording);settings(state);}
  catch(e){clientNotice=e.message;}
};
for(const id of ['text-backend','size'])$(id).addEventListener('change',()=>{$('model').value=selectedModel(state.providers,$('text-backend').value,$('size').value);renderSetup(state);});
for(const id of ['text-backend','image-backend','tts-backend','stt-backend','size','speech-mode','model','cap','voice-id','pitch','rate'])$(id).addEventListener('input',()=>renderSetup(state));
$('installed-choice').onchange=()=>{if($('installed-choice').value){$('text-backend').value='local_ollama';$('model').value=$('installed-choice').value;renderSetup(state);}};
$('check-providers').onclick=()=>invoke('check_providers').catch(()=>{});
function settingsTab(name){
 for(const e of document.querySelectorAll('[data-settings-tab]'))e.setAttribute('aria-selected',String(e.dataset.settingsTab===name));
 for(const e of document.querySelectorAll('.settings-page'))e.hidden=e.id!=='settings-'+name;
 if(name==='connections')$('provider-panel').open=true;
}
for(const e of document.querySelectorAll('[data-settings-tab]'))e.onclick=()=>settingsTab(e.dataset.settingsTab);
let sessionInfo={enabled:false};
async function sessionSettings(){
 try {sessionInfo=await(await fetch('/api/session')).json();
  $('session-info').textContent=sessionInfo.enabled?`Share ${sessionInfo.url} and this session code with your friend: ${sessionInfo.code}`:'Local play. To invite a friend, start the host with --lan followed by its private IPv4 address.';
  $('session-controls').replaceChildren();
  if(sessionInfo.enabled){$('player').value='player_1';$('player').disabled=true;const b=document.createElement('button');b.textContent='Disconnect guest / New invitation';b.onclick=async()=>{await invoke('session_rotate');await sessionSettings();};$('session-controls').append(b);}
 }catch{$('session-info').textContent='Session status unavailable.';}
}
$('show-setup').onclick=()=>{$('settings-dialog').showModal();settingsTab('models');sessionSettings();};
$('voice-character-apply').onclick=()=>$('settings').click();
$('settings-close').onclick=()=>$('settings-dialog').close();
$('settings-dialog').addEventListener('click',e=>{if(e.target===$('settings-dialog')){const r=e.target.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)e.target.close();}});
let resetGeneration=0;
$('new-game').onclick=()=>{resetGeneration=state.generation;$('reset-dialog').showModal();};
$('cancel-reset').onclick=()=>$('reset-dialog').close();
$('confirm-reset').onclick=async()=>{
  try{if(mic){if(mic.abort)mic.abort();else mic.stop();mic=null;}await player.interrupt();await invoke('reset',{},resetGeneration);lastSpeech=0;$('input').value='';$('interim').textContent='';$('reset-dialog').close();}
  catch(e){clientNotice=e.message;}
};
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
  const generation=state?.generation;
  await player.interrupt();
  const result=await post('mic',{},generation);
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
async function findMicrophones(){
  if(!navigator.mediaDevices?.enumerateDevices)return;
  const selected=$('mic-device').value,devices=await navigator.mediaDevices.enumerateDevices();
  const options=[new Option('System default microphone',''),...devices.filter(d=>d.kind==='audioinput').map((d,i)=>new Option(d.label||`Microphone ${i+1}`,d.deviceId))];
  $('mic-device').replaceChildren(...options);if(options.some(o=>o.value===selected))$('mic-device').value=selected;
}
$('mic-find').onclick=()=>findMicrophones().catch(e=>$('voice-message').textContent=e.message);
async function httpMic(){
  if(!navigator.mediaDevices?.getUserMedia)throw Error('Microphone capture is unavailable.');
  const capture=await beginMic();
  const device=$('mic-device').value;
  const stream=await navigator.mediaDevices.getUserMedia({audio:device?{deviceId:{exact:device}}:true});
  await findMicrophones();
  $('mic-monitor').hidden=false;$('mic-level').value=0;$('mic-feedback').textContent='Recording · speak, then click Send recording';
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
  let detected=false,peak=0;
  const vad=()=>{
    if(!capturing)return;
    analyser.getFloatTimeDomainData(data);
    const rms=Math.sqrt(data.reduce((n,x)=>n+x*x,0)/data.length);
    peak=Math.max(peak,rms);$('mic-level').value=Math.min(1,rms*4);
    $('mic-feedback').textContent=`Recording · input ${Math.round(rms*100)}% · click Send recording when finished`;
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
   try{
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
    $('mic-feedback').textContent=peak<.005?'Very little microphone input · checking transcription…':'Preparing recording…';
    const blob=new Blob(chunks,{type});
    const wav=await captureWav(blob);
    await recognitionSerial; // Late speech-start receipts must not cancel active Whisper.
    if(capture!==session)return;
    $('mic-feedback').textContent='Transcribing on host…';
    await invoke('transcribe',{session:capture,audio:await file64(wav),mime:'audio/wav'});
   }catch(e){
    capturing=false;clearTimeout(timeout);stream.getTracks().forEach(t=>t.stop());
    if(mic===recorder){mic=null;$('mic').textContent='🎙 Mic';}
    $('mic-feedback').textContent='Recording/transcription failed: '+e.message;$('notice').textContent=e.message;
   }
  }
  ;
  recorder.start(250);
  $('mic').textContent='■ Send Recording';
}
$('mic').onclick=async()=>{
  if(micStarting)return;micStarting=true;$('mic').disabled=true;
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
    $('notice').textContent=e.message;$('mic-monitor').hidden=false;$('mic-feedback').textContent=e.message;
  }finally{micStarting=false;$('mic').disabled=state.effective_backends.stt==='off';}
}
;
await refresh();await sessionSettings();
setInterval(()=>refresh().catch(e=>$('notice').textContent=e.message),600);

$('resume-local').onclick=()=>invoke('resume_local',{});

$('stock').onclick=async()=>{$('actions').closest('details').open=true;await player.interrupt();await invoke('chat',{text:'What stock do you sell?'});};

$('mind-apply').onclick=async()=>{try{await invoke('personality_settings',{version:1,enabled:$('mind-enabled').checked,profiles:JSON.parse($('mind-profiles').value)});$('mind-status').textContent='Character settings applied.';}catch(e){$('mind-status').textContent=e.message;}};

$('reactions-apply').onclick=async()=>{try{await invoke('reaction_settings',{version:1,enabled:$('reactions-enabled').checked,characters:JSON.parse($('reaction-rules').value)});$('reactions-status').textContent='Host reaction rules applied.';}catch(e){$('reactions-status').textContent=e.message+' Existing transfers require the same rules; start New game before changing them.';}};

$('greeting-apply').onclick=async()=>{try{await invoke('greeting_settings',{version:1,long_gap_turns:Number($('greeting-gap').value)});$('greeting-status').textContent='Greeting behavior applied.';}catch(e){$('greeting-status').textContent=e.message;}};
