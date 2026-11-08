"""Independent native natural-request and stock routing checks. No model calls."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import ROOT,go
from dungeon_natural_reference import request
from dungeon_knowledge_reference import build
from dungeon_knowledge_cases import host
import dungeon_reference as game
import play_ai as ui
QUERIES=['hello, what do you have for sale?','What can I buy?','Can you tell me what you sell?','Show me your wares.','What merchandise is available?','What have you got?','What supplies do you carry?','What is in your stock?']
def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args();assert a.cap==0
    stock=natural=0
    for seed in [0,7,101,202,303,409]:
        for buy in [False,True]:
            for q in QUERIES:
                case=('stock',seed,'merchant',q,[],buy,'stock');h=host(case);g=game.Game(seed)
                for room in ['gallery','workshop']:g.act(g.revision,game.pair('go','room:'+room))
                if buy:g.act(g.revision,game.pair('buy','item:oil'))
                try:
                    before=h.call('state',{});assert h.call('trade',{'speaker':'merchant','input':q})['handled'] is False
                    offered=h.call('knowledge_prepare',{'speaker':'merchant','input':q,'context':2048,'size':'3'})
                    expected=build(g,ui.strict((ROOT/'examples/lantern-vault/world.json').read_text()),'merchant',q)
                    assert game.encode(offered['offer'])==game.encode(expected)
                    assert [c['id'] for c in expected['definition']['cards']]==['stock']
                    assert 'seal' not in expected['definition']['cards'][0]['text']
                    assert ('hinge oil' not in expected['definition']['cards'][0]['text'])==buy
                    assert h.call('state',{})==before;stock+=1
                finally:h.close()
        for worldfile in ['world.json','world-structured.json']:
            worldpath=ROOT/'examples/lantern-vault'/worldfile;world=ui.strict(worldpath.read_text())
            for q in [QUERIES[0],'What does the tonic do?','Who owns the seal?','Where is my home?','Tell me your story.','Hello!']:
                case=('natural',seed,'merchant',q,[],False,None);h=host(case,worldpath);g=game.Game(seed)
                for room in ['gallery','workshop']:g.act(g.revision,game.pair('go','room:'+room))
                try:
                    before=h.call('state',{});offered=h.call('natural_speech_prepare',{'speaker':'merchant','input':q,'context':2048,'guidance':'focused','size':'3'})
                    assert game.encode(offered['request'])==game.encode(request(g,world,'merchant',q)),(q,offered)
                    h.call('speech_finish',{'ticket':offered['ticket'],'text':ui.encode({'speech':'Noted.','tone':'neutral','fact_ids':[],'memory_ids':[]})})
                    assert h.call('state',{})==before;natural+=1
                finally:h.close()
    print(ui.encode({'stock_workflows':stock,'natural_requests':natural,'api_spend_usd':0}))
if __name__=='__main__':main()
