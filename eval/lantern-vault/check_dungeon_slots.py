"""Independent entity resolver parity and offline saved local-call audit. No network."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import make_host,go,ROOT
from dungeon_slots_eval import CASES
import dungeon_slots_reference as ref
import dungeon_reference as game
import play_ai as ui

def args(input,modern):
    return {'input':input,'context':2048,'guidance':'focused','resolve_slots':True,'resolve_ambiguity':modern}

def filtered(state,input,modern,focus=None,hint=None):
    if modern:return ref.resolve(state['choices'],state['items'],input,focus,hint)
    return {'pairs':ref.filter_pairs(state['choices'],state['items'],input)}

def compare(offer,resolution,modern):
    assert [row['pair'] for row in offer['offer'] if row['pair'] is not None]==resolution['pairs']
    assert offer['offer'][-1]=={'name':'clarify','pair':None}
    if modern:assert offer['resolution']=={k:v for k,v in resolution.items() if k!='pairs'}

def audit(path):
    r=ui.strict(path.read_text());assert r['complete'] and r['api_spend_usd']==0
    modern=r.get('entity_context',False);calls=r['calls'];cursor=0;correct=deferrals=model_correct=0
    assert len(r['observations'])==len(CASES)
    for obs,(id,room,input,expected) in zip(r['observations'],CASES):
        assert (obs['id'],obs['room'],obs['input'],obs['expected'])==(id,room,input,expected)
        h=make_host(room)
        try:
            state=h.call('state',{});offer=h.call('interpret',args(input,modern));assert obs['offer']==offer
            resolution=filtered(state,input,modern);compare(offer,resolution,modern)
            local=not resolution['pairs'];assert obs['host_deferral']==local
            deferrals+=local;actual='clarify' if local else None
            if not local:
                probe=calls[cursor];cursor+=1
                assert probe['phase']=='preflight'
                for key,value in offer['request'].items():assert probe['payload'][key]==value
                assert probe['payload']['options']['num_predict']==1
                assert probe['response']['prompt_eval_count']+offer['budget']['reserved_output']+256<=2048
                if not r['tokens_only']:
                    c=calls[cursor];cursor+=1;assert c['phase']=='generation'
                    assert c['payload']['messages']==probe['payload']['messages'] and c['payload']['format']==probe['payload']['format']
                    assert c['payload']['options']['num_predict']==offer['budget']['reserved_output']
                    assert c['response'].get('done_reason')!='length'
                    raw=ui.strict(c['response']['message']['content']);assert set(raw)=={'choice'}
                    assert raw['choice'] in [row['name'] for row in offer['offer']]
                    actual=raw['choice'];model_correct+=actual==expected
            assert obs['actual']==actual
            assert obs['correct']==(None if r['tokens_only'] else actual==expected)
            correct+=obs['correct'] is True
        finally:h.close()
    assert cursor==len(calls)
    for c in calls:
        assert c['status']=='complete' and c['response']['done']
        assert c['payload']['model']==r['identity']['model']
        o=c['payload']['options'];assert o['num_ctx']==2048 and o['num_thread']==4 and o['num_gpu']==0
    return {'cases':len(CASES),'correct':None if r['tokens_only'] else correct,'host_deferrals':deferrals,
        'model_correct':None if r['tokens_only'] else model_correct,'calls':len(calls),'api_spend_usd':0}

def parity():
    texts=['Inspect the oil.','Inspect oil or chalk.','Take it.','Inspect the key.','Take the keeper seal.',
        'Talk to the keeper about the keeper seal.','Unlock the gate.','Buy the tonic.',
        'Please examine the oil without buying it.','Move into the gallery.','Inspect that one.',
        'Nothing matching here.','Inspect the bronze key; do not pick it up.','Inspect CAFÉ ☃.']
    checks=0
    for seed in [0,1,7,101,202,303]:
        for room in ['entry','gallery','workshop','cistern']:
            h=ui.Host(ROOT/'target/debug/examples/dungeon',seed);g=game.Game(seed)
            try:
                route=(['gallery'] if room!='entry' else [])+([room] if room in ['workshop','cistern'] else [])
                for step in route:go(h,step);g.act(g.revision,{'action':'go','target':'room:'+step})
                state=h.call('state',{});assert state==g.public()
                for text in texts:
                    offer=h.call('interpret',args(text,True));compare(offer,filtered(state,text,True),True);checks+=1
                candidates=state['items']
                if candidates:
                    target=candidates[0]['id'];offer=h.call('interpret',{**args('Inspect it.',True),'target_hint':target,'expected_revision':state['revision']})
                    compare(offer,filtered(state,'Inspect it.',True,hint=target),True);checks+=1
            finally:h.close()
    return {'entity_workflows':checks,'seeds':6,'api_spend_usd':0}

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--check',action='store_true');p.add_argument('paths',type=pathlib.Path,nargs='*');a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    if a.check:print(ui.encode(parity()))
    for path in a.paths:print(ui.encode(audit(path)))
if __name__=='__main__':main()
