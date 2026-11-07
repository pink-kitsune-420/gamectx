// Bounded diagnostics outside gameplay columns. No tokenizer or inference calls.
export function attachProfiler({$,post,refresh}){
 const floating=$('debug-window'),bar=$('debug-titlebar');let drag,lastContext='',lastExecution='';
 $('show-debug').onclick=()=>{floating.hidden=false;};$('debug-minimize').onclick=()=>{floating.hidden=true;};
 for(const b of document.querySelectorAll('[data-debug-tab]'))b.onclick=()=>{for(const p of document.querySelectorAll('.debug-page'))p.hidden=p.id!=='debug-'+b.dataset.debugTab;};
 bar.onpointerdown=e=>{if(e.target.closest('button'))return;const r=floating.getBoundingClientRect();drag={x:e.clientX-r.left,y:e.clientY-r.top};bar.setPointerCapture(e.pointerId);};
 bar.onpointermove=e=>{if(!drag)return;floating.style.left=Math.max(0,Math.min(innerWidth-floating.offsetWidth,e.clientX-drag.x))+'px';floating.style.top=Math.max(0,Math.min(innerHeight-40,e.clientY-drag.y))+'px';floating.style.right='auto';};bar.onpointerup=()=>{drag=null;};
 $('capture-prompts').onchange=()=>post('profiler_settings',{capture_prompts:$('capture-prompts').checked}).then(refresh).catch(e=>$('notice').textContent=e.message);
 $('cancel-local-text').onclick=()=>post('cancel_text',{}).then(refresh).catch(e=>$('notice').textContent=e.message);
 const node=(tag,text,cls)=>{const n=document.createElement(tag);if(text!==undefined)n.textContent=text;if(cls)n.className=cls;return n;};
 const pre=value=>node('pre',JSON.stringify(value,null,2));
 const table=(headers,rows)=>{const t=node('table',undefined,'profile-table'),head=node('tr');for(const h of headers)head.append(node('th',h));t.append(head);for(const cells of rows){const r=node('tr');for(const c of cells){const cell=node('td');cell.append(typeof c==='string'||typeof c==='number'?document.createTextNode(String(c)):c);r.append(cell);}t.append(r);}return t;};
 const metrics=items=>{const d=node('div',undefined,'profile-metrics');for(const [name,value]of items){const p=node('div');p.append(node('strong',String(value)),node('small',name));d.append(p);}return d;};
 const seconds=n=>((n||0)/1000).toFixed(2)+'s';
 return s=>{
  const context=s.setup?.context,key=JSON.stringify(context);
  if(key!==lastContext){lastContext=key;const container=$('context-profiler');container.replaceChildren();
   if(!context)container.append(node('p','No compiled request yet. Profiling adds no model calls.'));
   else{const usage=context.provider_usage||{};container.append(node('p',`Latest request #${context.request_id} · ${context.model} · ${context.tier}B context class`),metrics([['Prepared JSON bytes',context.prepared_body_bytes],['Schema bytes',context.schema_bytes],['Prompt tokens (provider)',usage.prompt_eval_count??usage.online_usage?.prompt_tokens??'—'],['Output token cap',context.output_token_limit??'—']]));
    container.append(table(['Message','UTF-8 bytes','Selected JSON sections'],context.messages.map(m=>[`${m.index+1} · ${m.role}`,m.content_bytes,(m.sections||[]).map(v=>`${v.name}: ${v.json_bytes}B${v.entries!==null?' / '+v.entries+' entries':''}`).join('; ')||'Text'])));
    container.append(node('p',context.notes,'muted'));if(context.preview){const d=node('details');d.append(node('summary','Captured prompt (bounded preview)'),node('pre',context.preview));container.append(d);}
   }
  }
  const activity=s.setup?.activity||[];$('cancel-local-text').disabled=!activity.some(r=>r.channel==='text'&&r.backend==='local_ollama'&&r.status==='running');
  const execution=JSON.stringify([activity,s.pending_jobs,s.voice_history?.records.length]);if(execution!==lastExecution){lastExecution=execution;const maximum=Math.max(1,...activity.map(r=>(r.queue_ms||0)+(r.run_ms||0)));
   const rows=activity.slice(0,12).map(r=>{const span=node('div',undefined,'profile-timeline');for(const [kind,amount]of [['queue',r.queue_ms||0],['run',r.run_ms||0]]){const p=node('span',undefined,'phase-'+kind);p.style.width=(amount/maximum*100)+'%';p.title=kind+': '+seconds(amount);span.append(p);}return [`${r.channel} #${r.id} · ${r.status}`,seconds(r.queue_ms),seconds(r.run_ms),span];});
   $('execution-profiler').replaceChildren(metrics([['Pending',s.pending_jobs],['Cancelled (retained)',activity.filter(r=>r.status==='cancelled').length],['Saved audio',s.voice_history?.records.length||0]]),node('p','Amber: queued · Green: running. Provider time includes loading and transport, not only inference.','muted'),table(['Request','Queue','Run','Timeline'],rows));
   const d=node('details');d.append(node('summary','Provider/stage counters'),pre(activity.slice(0,12).map(r=>({id:r.id,model:r.model,provider_ms:r.provider_ms,completion_ms:r.completion_ms,usage:r.provider_usage}))));$('execution-profiler').append(d);
  }
 };
}
