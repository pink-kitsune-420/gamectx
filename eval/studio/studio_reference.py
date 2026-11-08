"""Original offline studio policy: shared cumulative reservations and voice receipts."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,json
from archetype_reference import fields,integer
BACKENDS={'text':['0b_fallback','local_ollama','online_openai_compatible'],'image':['cached_only','local_sd_cpp','online_image_api'],'tts':['off','browser_web_speech','local_tts_http','online_tts_api'],'stt':['off','browser_speech_recognition','local_stt_http','online_stt_api']}
FALLBACK={'text':'0b_fallback','image':'cached_only','tts':'browser_web_speech','stt':'browser_speech_recognition'}
def enc(x):return json.dumps(x,ensure_ascii=False,separators=(',',':'))
def routing(a):
 fields(a,'cap operations');integer(a['cap'],0,10**12);cap=a['cap'];spent=0;seq=0;entries={};out=[]
 for op in a['operations']:
  try:
   fields(op,'op args');v=op['args'];k=op['op']
   if k=='reserve':
    fields(v,'modality backend ceiling local_fallback');m=v['modality'];assert m in BACKENDS and v['backend'] in BACKENDS[m] and type(v['local_fallback']) is bool;integer(v['ceiling'],0,10**12);b=v['backend'];r=None;reason='selected'
    if b.startswith('online_'):
     if v['ceiling']==0 or spent+v['ceiling']>cap or len(entries)>=256:b=next(b for b in BACKENDS[m] if b.startswith('local_')) if v['local_fallback'] else FALLBACK[m];reason='cap_or_unpriced'
     else:seq+=1;r=seq;entries[r]=dict(amount=v['ceiling'],state='reserved');spent+=v['ceiling']
    result=dict(backend=b,reservation=r,reason=reason)
   elif k in ['dispatch','finish','cancel']:
    fields(v,'id');e=entries[v['id']]
    if k=='dispatch':assert e['state']=='reserved';e['state']='dispatched'
    elif k=='finish':assert e['state']=='dispatched';e['state']='settled'
    else:assert e['state']=='reserved';spent-=e['amount'];del entries[v['id']]
    result=dict(accepted=True)
   elif k=='limit':fields(v,'cap');integer(v['cap'],spent,a['cap']);cap=v['cap'];result=dict(cap_micros=cap)
   elif k=='inspect':fields(v,'');result=dict(cap_micros=cap,authorized_cap_micros=a['cap'],charged_or_reserved_micros=spent,pending=sum(e['state']!='settled' for e in entries.values()))
   else:raise ValueError(k)
   out.append(dict(ok=True,result=copy.deepcopy(result)))
  except (AssertionError,KeyError,TypeError,ValueError):out.append(dict(ok=False))
 return dict(results=out,actual_provider_calls=0)
def voice(a):
 fields(a,'text operations');assert isinstance(a['text'],str) and 0<len(a['text'])<=240
 text=a['text'];stage='validated';played=0;heard=0;done=False;out=[]
 for op in a['operations']:
  try:
   fields(op,'op args');v=op['args'];k=op['op']
   if k=='queue':fields(v,'');assert stage=='validated' and not done;stage='queued';result=dict(accepted=True)
   elif k in ['progress','interrupt','finish']:
    fields(v,'through heard');n=v['through'];h=v['heard'];integer(n,played,len(text));integer(h,heard,n);assert not done
    if k=='finish':assert n==len(text) and h==n
    if stage=='validated':assert k=='interrupt' and n==0 and h==0;stage='cancelled';done=True
    else:
     assert stage in ['queued','played'];stage='played';played=n;heard=h
     if k=='interrupt':stage='interrupted';done=True
     if k=='finish':assert n==len(text) and h==n;done=True
    result=dict(accepted=True)
   elif k=='recognition':
    fields(v,'text final uncertainty');assert isinstance(v['text'],str) and v['text'] and type(v['final']) is bool;result=dict(status='final_text' if v['final'] else 'interim',text=v['text'],uncertainty=v['uncertainty'],actions=[])
   elif k=='inspect':fields(v,'');result=dict(stage=stage,played=played,heard=heard,done=done,heard_text=text[:heard])
   else:raise ValueError(k)
   out.append(dict(ok=True,result=copy.deepcopy(result)))
  except (AssertionError,KeyError,TypeError,ValueError):out.append(dict(ok=False))
 return dict(results=out,actual_provider_calls=0)
