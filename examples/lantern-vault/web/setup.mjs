// Pure presentation helpers: no provider calls or world actions.
export const channelNames={text:'Replies',image:'Pictures',tts:'Voice',stt:'Microphone'};
export function browserAvailability(row,env){
  if(row.backend==='browser_speech_recognition'&&!env.SpeechRecognition&&!env.webkitSpeechRecognition)return {...row,status:'browser_unavailable',detail:'This browser does not provide speech recognition. Type instead or configure a local transcription server.'};
  if(row.backend==='browser_web_speech'&&!env.speechSynthesis)return {...row,status:'browser_unavailable',detail:'This browser does not provide speech playback. Use text only or configure a local voice server.'};
  return row;
}
export function selectedModel(providers,backend=providers.text,size=providers.size){
  return providers.text_models?.[backend]?.[String(size).replace(/[bB]$/,'')]||'';
}
export function channelLabel(row){
  if(row.status==='offline')return ({'0b_fallback':'Offline replies · no LLM',cached_only:'Cached pictures',off:'Off',browser_web_speech:'Browser voice',browser_speech_recognition:'Browser recognition'})[row.backend]||row.backend;
  return ({unchecked:'Not checked',checking:'Checking…',ready:'Ready',reachable:'Server reachable',missing_model:'Model not installed',unconfigured:'Choose a model',unavailable:'Server unavailable',browser_unavailable:'Unavailable in this browser',paused:'Paused · resume needed',online:'Online · cap required'})[row.status]||row.status;
}
export function choiceLabel(choice,items=[]){
  const id=choice.target?.split(':').at(-1),item=items.find(i=>i.id===choice.target),name=item?.label||id?.replaceAll('_',' ');
  switch(choice.action){
    case 'go':return `Go to ${name}`;
    case 'buy':return `Buy ${name}${item?.price!==undefined?` · ${item.price} coins`:''}`;
    case 'talk':return `Approach ${name}`;
    case 'look':return 'Look around';
    case 'wait':return 'Wait a moment';
    default:return `${choice.action.replace(/^./,c=>c.toUpperCase())}${name?' '+name:''}`;
  }
}
export function draftKey(draft){return JSON.stringify([draft.text,draft.image,draft.tts,draft.stt,draft.size,draft.model||'',draft.speech_mode,draft.cap_micros,draft.voices]);}
