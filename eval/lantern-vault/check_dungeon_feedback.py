"""Audit user-reported dialogue as supplied text and independent host reproductions.

No model is called. An accepted speech packet is not a semantic correctness score.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import make_host
from dungeon_trade_reference import Trade
import dungeon_reference as game
import dungeon_social_reference as social
import play_ai as ui
ROOT=pathlib.Path(__file__).resolve().parents[2]

def check(path):
    report=ui.strict(path.read_text())
    assert report['version']==1 and report['source']=='user_report'
    for key in ['deployment_identity','source_seed','source_world_trace','source_usage']:assert report[key] is None
    assert report['reproduction_seed']==7
    cases=report['cases'];assert len(cases)==11 and len({c['id'] for c in cases})==11
    receipts=speeches=requests=purchases=0
    for case in cases:
        assert case['channel'] in ['speech','trade'] and case['assessment']['notes']
        h=make_host('workshop');g=game.Game(7);t=Trade();s=social.Social();quotes=[]
        for room in ['gallery','workshop']:g.act(g.revision,game.pair('go','room:'+room))
        try:
            assert h.call('state',{})==g.public()
            for turn in case['turns']:
                assert set(turn)=={'input','reply'} and isinstance(turn['input'],str) and isinstance(turn['reply'],str)
                if turn['input']=='/forget':
                    before=h.call('state',{});assert h.call('trade_cancel',{})==t.cancel()
                    assert h.call('speech_forget',{'speaker':'merchant'})==s.forget(g,'merchant')
                    assert h.call('state',{})==before;assert h.call('speech_state',{'speaker':'merchant'})['memories']==[]
                    quotes=[];continue
                before=g.public()
                if case['channel']=='trade':
                    actual=h.call('trade',{'speaker':'merchant','input':turn['input']});expected=t.exchange(g,'merchant',turn['input']);requests+=1
                    assert game.encode(actual)==game.encode(expected),(case['id'],actual,expected)
                    if actual['handled']:
                        assert actual['reply']==turn['reply'],(case['id'],actual['reply'],turn['reply'])
                        receipts+=1;purchases+='result' in actual
                        assert h.call('state',{})==g.public();continue
                # Feed the supplied raw speech through current validation. This checks
                # provenance/storage boundaries without pretending to rerun the model.
                ticket=s.prepare(g,'merchant',turn['input'])
                offered=h.call('speech_prepare',{'speaker':'merchant','input':turn['input'],'context':2048,'guidance':'focused'})
                text=ui.encode({'speech':turn['reply'],'tone':'neutral','fact_ids':[],'memory_ids':[]})
                assert h.call('speech_finish',{'ticket':offered['ticket'],'text':text})==s.finish(g,ticket['id'],text)
                assert h.call('state',{})==before
                state=h.call('speech_state',{'speaker':'merchant'});assert state==s.state(g,'merchant')
                quotes.append(turn['input'].strip());assert [m['text'] for m in state['memories']]==quotes
                speeches+=1
        finally:h.close()
    return {'reported_sequences':len(cases),'reported_turns':sum(len(c['turns']) for c in cases),
        'trade_requests':requests,'matched_local_receipts':receipts,'accepted_supplied_speech':speeches,
        'reproduced_purchases':purchases,'source_model_accuracy':None,'model_calls':0,'api_spend_usd':0}

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('path',type=pathlib.Path,nargs='?',default=ROOT/'eval/lantern-vault/dungeon-feedback.json');a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    print(ui.encode(check(a.path)))
if __name__=='__main__':main()
