"""Offline scaled-render tests; frozen legacy diagnostics still use the legacy operation."""
import argparse,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_knowledge_cases import host
p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args();assert a.cap==0
checks=0
for size,recall,history in [('1',1,1),('1.5',1,1),('3',2,1),('8',3,2),('30',4,3),('70',6,3)]:
 h=host(('scaled',7,'merchant','Where was I born?',[],False,None))
 try:
  for _ in range(4):
   offered=h.call('natural_speech_prepare_scaled',dict(speaker='merchant',input='Where was I born?',context=32768,guidance='focused',size=size));messages=offered['request']['messages'];data=messages[-1]['content'];assert len(messages)<=history*2+2
   assert '"known":false' in data
   if size in ('1','1.5','3'):assert '"lore"' not in data
   h.call('speech_finish',dict(ticket=offered['ticket'],text='{"speech":"I do not know your birthplace.","tone":"neutral","fact_ids":[],"memory_ids":[]}'));checks+=1
 finally:h.close()
print(dict(scaled_requests=checks,model_calls=0,api_spend_usd=0))
