"""Opt-in prototype for reusing exact-request token counts, never model answers."""
import hashlib
from dungeon_social_eval import ui
class CachedLocalModel(ui.LocalModel):
 def __init__(self,*args,**kwargs):
  self.capacity=kwargs.pop('capacity',8);super().__init__(*args,**kwargs);self.counts={};self.receipts=[];self.records['count_receipts']=self.receipts
 def generate(self,request,output):
  if self.identify()!=self.identity:self.counts.clear();raise ValueError('identity changed')
  key=hashlib.sha256(ui.encode(dict(identity=self.identity,context=self.context,output=output,request=request)).encode()).hexdigest()
  cached=self.counts.get(key);needed=1 if cached else 2
  if len(self.calls)+needed>self.maximum:raise RuntimeError('not enough calls for a complete turn')
  receipt=dict(key=key,reused=cached is not None,input_tokens=None,source_preflight=None,source_generation=None);self.receipts.append(receipt)
  if cached:count,first,second=cached
  else:
   first=len(self.calls);count=self.chat(request,1,'preflight').get('prompt_eval_count');second=None
  if type(count) is not int or count<0 or count+output+256>self.context:raise ValueError('request overflow')
  second_now=len(self.calls);response=self.chat(request,output,'generation');actual=response.get('prompt_eval_count')
  if type(actual) is not int or actual!=count:self.counts.pop(key,None);raise ValueError('count changed')
  if response.get('done_reason')=='length':raise ValueError('generation truncated')
  text=response.get('message',{}).get('content')
  if not isinstance(text,str) or len(text.encode())>4096:raise ValueError('invalid model text')
  if not cached:
   second=second_now
   if len(self.counts)>=self.capacity:self.counts.pop(next(iter(self.counts)))
   self.counts[key]=(count,first,second)
  receipt.update(input_tokens=count,source_preflight=first,source_generation=second)
  ui.checkpoint(self.log,self.records);return text
