"""Native example/source-card byte parity against independent host policies. No network."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import ROOT,go
from dungeon_knowledge_reference import build
import dungeon_reference as game
import play_ai as ui
from lore_model_eval import CASES

def host(case,world=None):
    id,seed,speaker,q,memory,buy,expected=case;h=ui.Host(ROOT/'target/debug/examples/dungeon',seed,world)
    for room in ['gallery','workshop' if speaker=='merchant' else 'cistern']:go(h,room)
    if buy:
        s=h.call('state',{});h.call('act',{'revision':s['revision'],'decision':game.pair('buy','item:oil')})
    for m in memory:
        t=h.call('speech_prepare',{'speaker':speaker,'input':m['text'],'context':2048,'guidance':'focused'})
        h.call('speech_finish',{'ticket':t['ticket'],'text':ui.encode({'speech':'Noted.','tone':'neutral','fact_ids':[],'memory_ids':[]})})
    return h

def check():
    world=ui.strict((ROOT/'examples/lantern-vault/world.json').read_text());count=0
    for seed in [0,7,101,202,303]:
        for base in CASES:
            case=(base[0],seed,*base[2:]);h=host(case);g=game.Game(seed)
            for room in ['gallery','workshop' if case[2]=='merchant' else 'cistern']:g.act(g.revision,game.pair('go','room:'+room))
            if case[5]:g.act(g.revision,game.pair('buy','item:oil'))
            try:
                offer=h.call('knowledge_prepare',{'speaker':case[2],'input':case[3],'context':2048,'size':'3'})
                expected=build(g,world,case[2],case[3],case[4]);assert game.encode(offer['offer'])==game.encode(expected),(case[0],offer['offer'],expected)
                before=h.call('state',{});answer=expected['definition']['cards'][0]['id'] if expected['definition']['cards'] else 'unknown'
                reply=h.call('knowledge_finish',{'ticket':offer['ticket'],'text':ui.encode({'answer':answer})})
                assert h.call('state',{})==before
                try:h.call('knowledge_finish',{'ticket':offer['ticket'],'text':ui.encode({'answer':answer})});assert False
                except ui.HostError:pass
                count+=1
            finally:h.close()
    return {'workflows':count,'api_spend_usd':0}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    print(ui.encode(check()))
