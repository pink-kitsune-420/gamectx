"""Offline reconstruction of both natural-dialogue diagnostics, including failures."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,importlib,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_natural_eval import CASES,scenario
import play_ai as ui

def check(path):
    r=ui.strict(path.read_text());assert r['complete'] and r['api_spend_usd']==0
    ref=importlib.import_module('dungeon_natural_reference'+('_v1' if r['natural_version']==1 else '_v2' if r['natural_version']==2 else ''))
    assert len(r['cases'])==20;cursor=0;structural=0
    for (mode,world) in r['worlds'].items():
        for case in CASES:
            c=r['cases'][cursor if r['tokens_only'] else cursor//2];q=ref.request(scenario(case),world,case[1],case[2],case[4])
            assert c['id']==case[0] and c['mode']==mode and c['request']==q and c['calls_start']==cursor
            probe=r['calls'][cursor];cursor+=1;assert probe['phase']=='preflight'
            for k,v in q.items():assert probe['payload'][k]==v
            assert probe['response']['prompt_eval_count']+128+256<=2048
            if not r['tokens_only']:
                call=r['calls'][cursor];cursor+=1;assert call['phase']=='generation'
                for k,v in q.items():assert call['payload'][k]==v
                assert call['response']['message']['content']==c['raw']
                p=ui.strict(c['raw']);speech=p.get('speech')
                valid=set(p)=={'speech'} and isinstance(speech,str) and 0<len(speech)<=240 and not any(ord(x)<32 for x in speech)
                assert c['structurally_valid']==valid and c['speech']==speech;structural+=valid
            assert c['calls_end']==cursor and c['semantic_score'] is None
    assert len(r['calls'])==cursor
    for call in r['calls']:
        assert call['status']=='complete' and call['response']['done']
        opts=call['payload']['options'];assert opts['num_ctx']==2048 and opts['num_thread']==4 and opts['num_gpu']==0 and opts['temperature']==0 and opts['seed']==42
        assert opts['num_predict']==(1 if call['phase']=='preflight' else 128)
    return {'version':r['natural_version'],'cases':20,'calls':cursor,'structurally_valid':None if r['tokens_only'] else structural,'api_spend_usd':0}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('paths',type=pathlib.Path,nargs='+');a=p.parse_args();assert a.cap==0
    for path in a.paths:print(ui.encode(check(path)))
