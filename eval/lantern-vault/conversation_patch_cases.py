"""User-reported dialogue routes: independent policy byte parity and UI receipts."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import ROOT,go
from dungeon_knowledge_cases import host
from dungeon_knowledge_reference import build
from dungeon_knowledge_reference_v7 import build as before
from test_dungeon_ai import Replies
import dungeon_reference as game
import play_ai as ui
QUERIES=[('hello! what is your name?','merchant_identity'),('do you have any hinge oil?','sale_oil'),('do you have anything else?','stock'),('what do you have information on?','topics'),('anything else?','topics'),('where are we?','location'),('what time is it?','time_unknown'),('what do you think of the sky?','sky_opinion'),('what do you know about this world?','world_setting'),('when was the moon landing?',None),('this is a hot day today','weather_report'),('how many letters are in strawberry?','letter_count'),('strawberry has 10 letters','letter_count')]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args();assert a.cap==0
 world=ui.strict((ROOT/'examples/lantern-vault/world.json').read_text());old=ui.strict((ROOT/'examples/lantern-vault/world-v1.json').read_text());count=baseline=correct=0
 for seed in [0,7,101,202,303,409]:
  g=game.Game(seed)
  for r in ['gallery','workshop']:g.act(g.revision,game.pair('go','room:'+r))
  for q,target in QUERIES:
   h=host(('route',seed,'merchant',q,[],False,None))
   try:
    expected=build(g,world,'merchant',q);actual=h.call('knowledge_prepare',{'speaker':'merchant','input':q,'context':2048,'size':'3'})
    assert game.encode(actual['offer'])==game.encode(expected),(q,actual,expected)
    ids=[c['id'] for c in expected['definition']['cards']];assert ids==([target] if target else []),(q,ids)
    prior=before(g,old,'merchant',q);prior_ids=[c['id'] for c in prior.get('definition',{}).get('cards',[])];baseline+=prior_ids==([target] if target else [])
    before_state=h.call('state',{});r=ui.converse(h,Replies(),'merchant',q)
    assert h.call('state',{})==before_state
    if target:assert r['status']=='answered'
    else:assert r['status']=='unknown' and '1969' not in r['speech']
    count+=1;correct+=1
   finally:h.close()
 # The whole reported transfer/recall/count sequence uses actual engine receipts.
 h=host(('sequence',7,'merchant','',[],False,None))
 try:
  r=ui.converse(h,Replies(),'merchant','take bronze_key');assert r['speaker']=='game'
  assert next(i for i in h.call('state',{})['items'] if i['id']=='item:bronze_key')['owner']=='player'
  r=ui.converse(h,Replies(),'merchant','what should it do with the key?');assert 'unlock' in r['speech']
  r=ui.converse(h,Replies(),'merchant','how many letters are in strawberry?');assert r['speech']=='strawberry has 10 letters.'
  r=ui.converse(h,Replies(),'merchant','cake');assert r['speech']=='cake has 4 letters.'
  t=h.call('speech_prepare',{'speaker':'merchant','input':'my name is yuki','context':2048,'guidance':'focused'})
  h.call('speech_finish',{'ticket':t['ticket'],'text':'{"speech":"Welcome.","tone":"neutral","fact_ids":[],"memory_ids":[]}'})
  r=ui.converse(h,Replies(),'merchant','What is my name?');assert r['speech']=='You told me your name is yuki.'
  place=h.call('location',{'detail':'full','max_bytes':4096});assert place['location']['id']=='room:workshop'
 finally:h.close()
 print(ui.encode({'route_workflows':count,'baseline_exact_routes':baseline,'candidate_exact_routes':correct,'scripted_sequences':1,'api_spend_usd':0}))
if __name__=='__main__':main()
