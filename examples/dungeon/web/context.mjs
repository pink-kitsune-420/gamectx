// Optimistic scene scope, captured at intent time. Receipts use their own one-use IDs.
export const scopedOperations=new Set(['chat','act','select','mic','promise','save','load','settings','generate','approve','repair','reset','check_providers','manager_settings','manager_action','manager_art','audio_settings','personality_settings','greeting_settings','deliberation_settings','conversation_settings','roleplay_settings','approach','profiler_settings','cancel_text','use_fallback','reaction','reaction_settings']);
export function makePost(fetch,current){
 return async(op,args={},generation=current()?.generation)=>{
  const headers={'Content-Type':'application/json','X-Studio-Request':'1'};
  if(scopedOperations.has(op)){
   if(!Number.isSafeInteger(generation)||generation<1)throw Error('Scene is still loading; try again.');
   headers['X-Studio-Generation']=String(generation);
  }
  const response=await fetch('/api/'+op,{method:'POST',headers,body:JSON.stringify(args)}),reply=await response.json();
  if(!response.ok||!reply.ok){const error=Error(reply.error||'Scene changed; try your action again.');error.generation=reply.generation;throw error;}
  return reply;
 };
}
// A command waits for a state read at least as new as its receipt. One extra read
// covers an older in-flight poll; a restarted host cannot cause an infinite retry loop.
export function coalescedRefresh(load,current){
 let running=null;
 return function refresh(minimum=0,attempt=0){
  if(!running)running=Promise.resolve().then(load).finally(()=>{running=null;});
  return running.then(()=>{
   if((current()?.generation||0)<minimum){
    if(attempt>=1)throw Error('Scene reset; reload the page.');
    return refresh(minimum,attempt+1);
   }
  });
 };
}

// Pending is a current undelivered text request, never completed speech or TTS work.
export function replyWaiting(state,starting=0,minimum=0){
 const s=state?.speech;
 return starting>0||(state?.generation||0)<minimum||Boolean(s?.pending&&!s.delivery.done&&s.delivery.stage==='validated');
}
// One browser tab drives live delivery. A second tab may inspect the same game
// without queuing/interruption receipts that cancel the first tab's next sentence.
export class AudioOwner{
 constructor(locks=globalThis.navigator?.locks,visible=()=>globalThis.document?.visibilityState!=='hidden'){
  Object.assign(this,{locks,visible,owned:false,claim:null,releaseLock:null});
 }
 async acquire(){
  if(this.owned)return true;
  if(!this.visible())return false;
  if(!this.locks)return true;
  if(this.claim)return this.claim;
  let resolve;this.claim=new Promise(r=>resolve=r);const ready=this.claim;
  this.locks.request('lantern-live-delivery',{ifAvailable:true},async lock=>{
   if(!lock){this.claim=null;resolve(false);return;}
   this.owned=true;resolve(true);
   await new Promise(r=>this.releaseLock=r);
   this.owned=false;this.releaseLock=null;this.claim=null;
  }).catch(()=>{this.claim=null;resolve(false);});
  return ready;
 }
 release(){const release=this.releaseLock;this.owned=false;this.claim=null;this.releaseLock=null;release?.();}
}
