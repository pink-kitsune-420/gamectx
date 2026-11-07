// Optimistic scene scope, captured at intent time. Receipts use their own one-use IDs.
export const scopedOperations=new Set(['chat','act','select','mic','promise','save','load','settings','generate','approve','repair','reset','check_providers','manager_settings','manager_action','manager_art','audio_settings','profiler_settings','cancel_text']);
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
