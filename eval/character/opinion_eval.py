"""Token-first diagnostic of compact opinion prompts, not semantic certification."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,pathlib,sys,time
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import ROOT
from opinion_reference import request
from conversation_patch_eval import CASES
import dungeon_reference as game
import play_ai as ui
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--tokens-only',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
 if a.cap!=0 or a.output.exists():p.error('require --cap 0 and fresh output')
 world=ui.strict((ROOT/'examples/lantern-vault/world.json').read_text());g=game.Game(7)
 for room in ['gallery','workshop']:g.act(g.revision,game.pair('go','room:'+room))
 m=ui.LocalModel('arzaan789/qwen2.5-3b-uncensored:latest',2048,12,a.output,'focused','3');r=m.records;r.update(opinion_version=3,world=world,tokens_only=a.tokens_only,cases=[])
 if not a.tokens_only:
  prior=ui.strict(a.tokens.read_text());assert prior['complete'] and prior['tokens_only'] and prior['identity']==m.identity and prior['world']==world
 for id,q in CASES:
  packet=request(g,world,'merchant',q);first=len(m.calls);raw=None
  if a.tokens_only:
   p=m.chat(packet,1,'preflight');assert p['prompt_eval_count']+96+256<=2048
  else:
   assert next(c for c in prior['cases'] if c['id']==id)['request']==packet;raw=m.generate(packet,96)
  r['cases'].append({'id':id,'input':q,'request':packet,'raw':raw,'calls_start':first,'calls_end':len(m.calls),'semantic_score':None});ui.checkpoint(a.output,r);print(id+': '+str(raw)+'; API spend $0',flush=True)
 r['complete']=True;ui.checkpoint(a.output,r)
if __name__=='__main__':main()
