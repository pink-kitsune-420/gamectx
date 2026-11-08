"""Offline audit of retained unsuccessful speech prototypes. Never calls a model."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import make_host,ROOT
from conversation_patch_eval import CASES
from opinion_reference import request as v3
from opinion_reference_v2 import request as v2
import dungeon_reference as game
import play_ai as ui

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args();assert a.cap==0;checked=0
 for stem in ['conversation-patch','opinion-v2','opinion-v3']:
  prior=None
  for suffix in ['tokens','results']:
   data=ui.strict(next((ROOT/'eval').glob('*/'+stem+'-'+suffix+'.json')).read_text());assert data['complete'] and data['api_spend_usd']==0 and data['tokens_only']==(suffix=='tokens')
   if prior is not None:assert data['identity']==prior['identity']
   else:prior=data
   assert len(data['cases'])==len(CASES)
   for row,(id,q) in zip(data['cases'],CASES):
    assert row['id']==id and row['input']==q and row['semantic_score'] is None
    if stem=='conversation-patch':
     h=make_host('workshop')
     try:request=h.call('lore_speech_prepare',{'speaker':'merchant','input':q,'context':2048,'guidance':'focused','size':'3'})['request']
     finally:h.close()
    else:
     g=game.Game(7)
     for r in ['gallery','workshop']:g.act(g.revision,game.pair('go','room:'+r))
     request=(v2 if stem=='opinion-v2' else v3)(g,data['world'],'merchant',q)
    assert request==row['request'],(stem,id)
    calls=data['calls'][row['calls_start']:row['calls_end']];assert len(calls)==(1 if suffix=='tokens' else 2)
    for c in calls:
     assert c['status']=='complete' and c['elapsed_micros']>=0
     payload=c['payload'];assert all(payload[k]==request[k] for k in ['messages','format'])
     assert payload['options']['num_ctx']==2048 and payload['options']['num_gpu']==0 and payload['options']['num_thread']==4
     assert c['response']['model']==data['identity']['model'] and c['response']['prompt_eval_count']+(128 if stem=='conversation-patch' else 96)+256<=2048
    if suffix=='results':assert row['raw']==calls[-1]['response']['message']['content']
    checked+=1
 print(ui.encode({'retained_diagnostic_cases':checked,'api_spend_usd':0,'semantic_accuracy_claim':False}))
if __name__=='__main__':main()
