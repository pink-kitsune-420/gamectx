"""Token-first local regression of subjective speech, with native source guards."""
import argparse,pathlib,sys,time
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import make_host,ROOT
import play_ai as ui
CASES=[('sky','what do you think of the sky?'),('feeling','how do you feel about being underground?')]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--tokens-only',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
 if a.cap!=0 or a.output.exists():p.error('require --cap 0 and fresh output')
 m=ui.LocalModel('arzaan789/qwen2.5-3b-uncensored:latest',2048,16,a.output,'focused','3');r=m.records;r.update(tokens_only=a.tokens_only,cases=[])
 if not a.tokens_only:
  prior=ui.strict(a.tokens.read_text());assert prior['complete'] and prior['tokens_only'] and prior['identity']==m.identity
 for id,q in CASES:
  h=make_host('workshop')
  try:
   offered=h.call('lore_speech_prepare',{'speaker':'merchant','input':q,'context':2048,'guidance':'focused','size':'3'});request=offered['request'];start=len(m.calls);begin=time.perf_counter_ns();raw=None;reply=None
   if a.tokens_only:
    probe=m.chat(request,1,'preflight');assert probe['prompt_eval_count']+128+256<=2048
   else:
    assert next(c for c in prior['cases'] if c['id']==id)['request']==request
    raw=m.generate(request,128);p=ui.strict(raw);assert set(p)=={'speech'}
    reply=h.call('speech_finish',{'ticket':offered['ticket'],'text':ui.encode({'speech':p['speech'],'tone':'neutral','fact_ids':[],'memory_ids':[]})})
   r['cases'].append({'id':id,'input':q,'request':request,'raw':raw,'reply':reply,'calls_start':start,'calls_end':len(m.calls),'turn_micros':(time.perf_counter_ns()-begin)//1000,'semantic_score':None});ui.checkpoint(a.output,r);print(id+': '+str(reply)+'; API spend $0',flush=True)
  finally:h.close()
 r['complete']=True;ui.checkpoint(a.output,r)
if __name__=='__main__':main()
