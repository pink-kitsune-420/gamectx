"""Offline native/rules/memory audit of saved character and interpretation calls."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[2]/'examples/lantern-vault'))
import play_ai as ui
import dungeon_social_reference as ref
from dungeon_social_eval import make_host,go

def check(path):
    r=ui.strict(path.read_text());assert r['complete'] and r['api_spend_usd']==0
    guidance=r.get('guidance','standard');calls=r['calls'];assert len(calls)<=64
    for c in calls:
        assert c['status']=='complete' and c['response']['done']
        assert c['payload']['model']==r['identity']['model'] and c['payload']['options']['num_ctx']==2048
        assert c['payload']['options']['num_thread']==4 and c['payload']['options']['num_gpu']==0
    cursor=0
    def generated(offer):
        nonlocal cursor
        while True:
            probe=calls[cursor];cursor+=1;generation=calls[cursor];cursor+=1
            assert probe['phase']=='preflight' and generation['phase']=='generation'
            p=probe['payload'];q=generation['payload'];original=offer['request']
            assert p['format']==original['format'] and p['messages'][:len(original['messages'])]==original['messages']
            assert p['options']['num_predict']==1 and q['options']['num_predict']==offer['budget']['reserved_output']
            assert q['messages']==p['messages'] and q['format']==p['format']
            assert probe['response']['prompt_eval_count']+offer['budget']['reserved_output']+256<=2048
            if generation['response'].get('done_reason')=='length':continue
            yield generation['response']['message']['content']
    if r['tokens_only']:
        for index,(speaker,room,input) in enumerate([('merchant','workshop','My name is Fen. What do you sell?'),('keeper','cistern','Where can I find the key?'),('narrator','entry','Describe the surroundings.'),(None,'entry','Please walk to the gallery.')]):
            h=make_host(room)
            try:
                offer=h.call('speech_prepare',{'speaker':speaker,'input':input,'context':2048,'guidance':guidance}) if speaker else h.call('interpret',{'input':input,'context':2048,'guidance':guidance})
                c=calls[index];assert c['phase']=='preflight'
                for key,value in offer['request'].items():assert c['payload'][key]==value
                assert c['response']['prompt_eval_count']+offer['budget']['reserved_output']+256<=2048
            finally:h.close()
        assert len(calls)==4
        return {'token_probes':4,'api_spend_usd':0}
    h=make_host('workshop');g=ref.game.Game(7);g.act(0,{'action':'go','target':'room:gallery'});g.act(1,{'action':'go','target':'room:workshop'});memory=ref.Social();speech=correct=0
    try:
        for obs in r['observations']:
            if obs['kind']!='speech':continue
            speaker=obs['speaker'];target='cistern' if speaker=='keeper' else 'workshop'
            if g.room!=target:
                for room in ['gallery',target]:go(h,room);g.act(g.revision,{'action':'go','target':'room:'+room})
            before=g.public();assert h.call('state',{})==before
            ticket=memory.prepare(g,speaker,obs['input'])
            offer=h.call('speech_prepare',{'speaker':speaker,'input':obs['input'],'context':2048,'guidance':guidance})
            for raw in generated(offer):
                submitted=raw
                if guidance=='focused':
                    v=ui.strict(raw);assert set(v)=={'speech'};submitted=ui.encode({'speech':v['speech'],'tone':'neutral','fact_ids':[],'memory_ids':[]})
                try:result=memory.finish(g,ticket['id'],submitted)
                except ValueError:
                    try:h.call('speech_finish',{'ticket':offer['ticket'],'text':submitted});assert False
                    except ui.HostError:pass
                    continue
                native=h.call('speech_finish',{'ticket':offer['ticket'],'text':submitted})
                assert native==result==obs['answer'];break
            assert h.call('state',{})==before
            assert h.call('speech_state',{'speaker':speaker})==memory.state(g,speaker)
            speech+=1
    finally:h.close()
    expectations=[('entry','Please walk to the gallery.','go_gallery'),('workshop',"Look at the bronze key; don't pick it up.",'inspect_bronze_key'),('workshop','Can I buy the expensive tonic?','clarify'),('gallery','Unlock the vault.','clarify')]
    observations=[o for o in r['observations'] if o['kind']=='interpret'];assert len(observations)==4
    for obs,(room,input,expected) in zip(observations,expectations):
        h=make_host(room)
        try:
            offer=h.call('interpret',{'input':input,'context':2048,'guidance':guidance});raw=next(generated(offer));v=ui.strict(raw)
            assert obs['input']==input and obs['expected']==expected and obs['actual']==v.get('choice')
            assert obs['correct']==(v=={'choice':expected});correct+=obs['correct']
        finally:h.close()
    assert speech==9 and cursor==len(calls)
    return {'speech_replies':speech,'world_changes_from_speech':0,'interpret_correct':correct,'interpret_cases':4,'api_spend_usd':0}

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('paths',type=pathlib.Path,nargs='+');a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    for path in a.paths:print(ui.encode(check(path)))
if __name__=='__main__':main()
