"""Independent dialogue trading regression across seeded states. No model calls."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import go,ROOT
import dungeon_reference as game
from dungeon_trade_reference import Trade
import play_ai as ui
SCENARIOS=[['I\'d like to buy the tonic.','yes','yes'],['I would like to buy hinge oil.','yes','yes'],
    ['buy chalk','no','yes'],['buy oil',{'act':'wait'},'yes'],['buy oil','How much is the tonic?','yes'],
    ['buy oil',{'cancel':True},'yes'],['buy salve'],['buy oil or chalk'],["I'd like chalk.",'yes'],
    ["Don't buy oil.",'yes'],['buy a ruby','yes']]
def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    cases=requests=0
    for seed in [0,1,7,101,202,303]:
        for scenario in SCENARIOS:
            h=ui.Host(ROOT/'target/debug/examples/dungeon',seed);g=game.Game(seed);t=Trade()
            try:
                for room in ['gallery','workshop']:go(h,room);g.act(g.revision,game.pair('go','room:'+room))
                for item in scenario:
                    if isinstance(item,str):
                        expected=t.exchange(g,'merchant',item);actual=h.call('trade',{'speaker':'merchant','input':item});requests+=1
                    elif 'act' in item:
                        expected=g.act(g.revision,game.pair('wait'));actual=h.call('act',{'revision':h.call('state',{})['revision'],'decision':game.pair('wait')})
                    else:expected=t.cancel();actual=h.call('trade_cancel',{})
                    assert game.encode(actual)==game.encode(expected),(seed,scenario,item,actual,expected)
                    assert h.call('state',{})==g.public(),(seed,scenario,item)
                cases+=1
            finally:h.close()
    print(game.encode({'workflows':cases,'trade_requests':requests,'model_calls':0,'api_spend_usd':0}))
if __name__=='__main__':main()
